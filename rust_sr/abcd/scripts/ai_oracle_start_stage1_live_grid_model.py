#!/usr/bin/env python3
"""
Train a Stage 1 oracle trend-start detector from live-style candle rows.

This differs from ai_oracle_trend_start_model.py in one important way: rows are
built by walking the candle grid for a root/timeframe/year, then labeling each
eligible candle/direction against the hindsight oracle. Training can still
sample negatives, but validation is scored on the full live-style grid.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd

try:
    from catboost import CatBoostClassifier
except ImportError as exc:  # pragma: no cover
    raise SystemExit("CatBoost is required. Run with .venv_ai\\Scripts\\python.exe") from exc


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_scanner_model as scanner
import ai_oracle_start_candidate_cache as candidate_cache
import ai_oracle_trend_start_model as sampled_stage1
import ai_wave_rider_research as wave
import eval_oracle_trend_start_full_grid as grid_eval


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle-run-id", default=sampled_stage1.DEFAULT_ORACLE_RUN)
    parser.add_argument("--run-prefix", default="aicw-oracle-start-livegrid-v1")
    parser.add_argument("--timeframe", default="2m", choices=sorted(scanner.TIMEFRAME_TABLES))
    parser.add_argument("--roots", default="NQ")
    parser.add_argument("--symbols", default="", help="Optional exact contract symbols, comma separated.")
    parser.add_argument("--train-years", default="2024")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--positive-pre-bars", type=int, default=2)
    parser.add_argument("--positive-post-bars", type=int, default=2)
    parser.add_argument("--negative-exclusion-bars", type=int, default=8)
    parser.add_argument("--match-window-bars", type=int, default=4)
    parser.add_argument("--negative-ratio", type=float, default=8.0)
    parser.add_argument("--base-negatives-per-symbol", type=int, default=200)
    parser.add_argument("--max-train-rows", type=int, default=0)
    parser.add_argument("--cooldown-bars", type=int, default=8)
    parser.add_argument("--min-oracle-recall", type=float, default=0.90)
    parser.add_argument("--max-picks-per-oracle", type=float, default=5.0)
    parser.add_argument("--thresholds", default="0.25,0.30,0.35,0.40,0.45,0.50,0.55,0.60,0.65,0.70,0.75,0.80,0.85,0.90")
    parser.add_argument("--iterations", type=int, default=650)
    parser.add_argument("--depth", type=int, default=6)
    parser.add_argument("--learning-rate", type=float, default=0.045)
    parser.add_argument("--l2-leaf-reg", type=float, default=10.0)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--limit-symbols", type=int, default=0)
    parser.add_argument("--save-row-csvs", action="store_true")
    parser.add_argument("--candidate-set-id", default="", help="Versioned candidate row set id. Empty uses a deterministic id.")
    parser.add_argument("--candidate-cache-dir", default="", help="Optional candidate row cache directory.")
    parser.add_argument("--read-candidate-cache", action="store_true", help="Load full live-grid candidate rows from cache.")
    parser.add_argument("--write-candidate-cache", action="store_true", help="Write full live-grid candidate rows to cache after building.")
    parser.add_argument("--only-build-candidate-cache", action="store_true", help="Build/write candidate cache for all requested years and exit.")
    parser.add_argument("--replace-candidate-cache", action="store_true", help="Overwrite existing candidate cache files for this candidate set.")
    parser.add_argument("--replace-run", action="store_true")

    # Candle feature defaults matching the existing oracle-start model.
    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=240.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    return parser.parse_args()


def parse_list(raw: str) -> list[str]:
    return [part.strip().upper() for part in str(raw or "").split(",") if part.strip()]


def parse_years(raw: str) -> list[int]:
    years = [int(part) for part in parse_list(raw)]
    if not years:
        raise ValueError("At least one train year is required")
    return years


def run_id(args: argparse.Namespace) -> str:
    train = "_".join(str(year) for year in parse_years(args.train_years))
    roots = "_".join(parse_list(args.roots)) or "all"
    raw = f"{args.run_prefix}-{args.timeframe}-{roots}-tr{train}-v{args.valid_year}"
    if len(raw) <= 64:
        return raw
    digest = hashlib.sha1(raw.encode("utf-8")).hexdigest()[:10]
    return f"{args.run_prefix[:36]}-{args.timeframe}-{digest}-v{args.valid_year}"


def to_jsonable(value: Any) -> Any:
    if isinstance(value, pd.Timestamp):
        return value.isoformat()
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def fetch_symbols(conn, timeframe: str, year: int, roots: list[str], symbols: list[str]) -> list[dict[str, str]]:
    rows = grid_eval.fetch_symbols(conn, timeframe, year, roots)
    if symbols:
        allowed = set(symbols)
        rows = [row for row in rows if str(row["symbol"]).upper() in allowed]
    return rows


def oracle_index_rows(starts: pd.DataFrame, group: pd.DataFrame, symbol: str, direction: str) -> list[dict[str, Any]]:
    return grid_eval.oracle_starts_for_symbol(starts, group, symbol, direction)


def nearest_start(idx: int, starts: list[dict[str, Any]]) -> dict[str, Any]:
    nearest = grid_eval.nearest_oracle(int(idx), starts)
    return {
        "nearest_oracle_trade_id": nearest["nearest_oracle_trade_id"],
        "nearest_oracle_date": nearest["nearest_oracle_date"],
        "nearest_delta_bars": nearest["nearest_delta_bars"],
        "nearest_abs_bars": nearest["nearest_abs_bars"],
    }


def label_sets(
    valid_indices: np.ndarray,
    starts: list[dict[str, Any]],
    args: argparse.Namespace,
) -> tuple[set[int], set[int], set[int]]:
    valid = set(int(value) for value in valid_indices)
    positive: set[int] = set()
    exclusion: set[int] = set()
    oracle_indices: set[int] = set()
    for item in starts:
        start_idx = int(item["idx"])
        oracle_indices.add(start_idx)
        for offset in range(-int(args.positive_pre_bars), int(args.positive_post_bars) + 1):
            candidate = start_idx + offset
            if candidate in valid:
                positive.add(candidate)
        for offset in range(-int(args.negative_exclusion_bars), int(args.negative_exclusion_bars) + 1):
            candidate = start_idx + offset
            if candidate in valid:
                exclusion.add(candidate)
    return positive, exclusion, oracle_indices


def choose_train_indices(
    valid_indices: np.ndarray,
    positive: set[int],
    exclusion: set[int],
    args: argparse.Namespace,
    rng: np.random.Generator,
) -> np.ndarray:
    valid = set(int(value) for value in valid_indices)
    negative_pool = np.array(sorted(valid - exclusion - positive), dtype=int)
    target_negatives = int(
        max(int(args.base_negatives_per_symbol), math.ceil(len(positive) * float(args.negative_ratio)))
    )
    if len(negative_pool) > target_negatives:
        negatives = rng.choice(negative_pool, size=target_negatives, replace=False)
    else:
        negatives = negative_pool
    chosen = np.array(sorted(set(int(value) for value in negatives) | set(positive)), dtype=int)
    return chosen


def rows_for_indices(
    year: int,
    root: str,
    symbol: str,
    group: pd.DataFrame,
    direction: str,
    indices: np.ndarray,
    positive: set[int],
    exclusion: set[int],
    starts: list[dict[str, Any]],
    train_sample: bool,
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for idx in indices.astype(int):
        payload = sampled_stage1.feature_row(year, root, symbol, group, int(idx), direction)
        nearest = nearest_start(int(idx), starts)
        nearest_abs = nearest["nearest_abs_bars"]
        payload.update(
            {
                "signal_idx": int(idx),
                "is_oracle_start": 1 if int(idx) in positive else 0,
                "negative_allowed": 1 if int(idx) not in exclusion and int(idx) not in positive else 0,
                "oracle_start_distance_bars": int(nearest_abs) if nearest_abs is not None else 999999,
                "train_sample": 1 if train_sample else 0,
                **nearest,
            }
        )
        rows.append(payload)
    return rows


def build_year_rows(
    conn,
    year: int,
    args: argparse.Namespace,
    rng: np.random.Generator,
    train_sample: bool,
) -> tuple[pd.DataFrame, dict[str, Any]]:
    roots = parse_list(args.roots)
    symbols = parse_list(args.symbols)
    symbol_rows = fetch_symbols(conn, args.timeframe, year, roots, symbols)
    oracle = sampled_stage1.fetch_oracle_starts(conn, str(args.oracle_run_id), int(year), roots)
    if oracle.empty:
        print(
            f"{year}: no oracle starts found; building unlabeled live-grid candidate rows",
            flush=True,
        )
    year_start = pd.Timestamp(year=int(year), month=1, day=1)
    year_end = pd.Timestamp(year=int(year) + 1, month=1, day=1)
    min_idx = 60
    all_rows: list[dict[str, Any]] = []
    symbol_summaries: list[dict[str, Any]] = []
    total_candidates = 0
    total_positive_rows = 0

    for symbol_index, item in enumerate(symbol_rows, start=1):
        if args.limit_symbols and symbol_index > int(args.limit_symbols):
            break
        root = str(item["root_symbol"])
        symbol = str(item["symbol"])
        raw_group = grid_eval.fetch_symbol_candles(conn, args.timeframe, int(year), root, symbol)
        if raw_group.empty:
            continue
        group = scanner.enrich_candles(raw_group[["ts_utc", "open", "high", "low", "close", "volume"]].copy(), args)
        times = pd.to_datetime(group["ts_utc"], errors="coerce")
        valid_indices = np.where((times >= year_start) & (times < year_end))[0]
        valid_indices = valid_indices[(valid_indices >= min_idx) & (valid_indices < len(group) - 1)].astype(int)
        if len(valid_indices) == 0:
            continue
        for direction in ["LONG", "SHORT"]:
            starts = oracle_index_rows(oracle, group, symbol, direction)
            positive, exclusion, _ = label_sets(valid_indices, starts, args)
            total_candidates += int(len(valid_indices))
            total_positive_rows += int(len(positive))
            selected_indices = (
                choose_train_indices(valid_indices, positive, exclusion, args, rng)
                if train_sample
                else valid_indices
            )
            all_rows.extend(
                rows_for_indices(
                    year=int(year),
                    root=root,
                    symbol=symbol,
                    group=group,
                    direction=direction,
                    indices=selected_indices,
                    positive=positive,
                    exclusion=exclusion,
                    starts=starts,
                    train_sample=train_sample,
                )
            )
            symbol_summaries.append(
                {
                    "root_symbol": root,
                    "symbol": symbol,
                    "direction": direction,
                    "oracle_starts": int(len(starts)),
                    "eligible_candidates": int(len(valid_indices)),
                    "positive_rows": int(len(positive)),
                    "materialized_rows": int(len(selected_indices)),
                }
            )
        if symbol_index % 10 == 0:
            print(f"{year}: symbols {symbol_index:,}/{len(symbol_rows):,} rows={len(all_rows):,}", flush=True)

    frame = pd.DataFrame(all_rows)
    if frame.empty:
        raise ValueError(f"No rows built for {year}")
    if train_sample:
        frame = frame.sample(frac=1.0, random_state=int(args.random_seed) + int(year)).reset_index(drop=True)
        if int(args.max_train_rows) > 0 and len(frame) > int(args.max_train_rows):
            positives = frame[frame["is_oracle_start"] == 1]
            negatives = frame[frame["is_oracle_start"] == 0]
            max_rows = int(args.max_train_rows)
            if positives.empty or negatives.empty:
                frame = frame.sample(n=max_rows, random_state=int(args.random_seed)).reset_index(drop=True)
            else:
                keep_pos_n = min(len(positives), max(1, max_rows // 2))
                keep_neg_n = min(len(negatives), max_rows - keep_pos_n)
                if keep_neg_n <= 0:
                    keep_neg_n = 1
                    keep_pos_n = max(1, max_rows - keep_neg_n)
                keep_pos = positives.sample(n=keep_pos_n, random_state=int(args.random_seed)) if len(positives) > keep_pos_n else positives
                keep_neg = negatives.sample(n=keep_neg_n, random_state=int(args.random_seed)) if len(negatives) > keep_neg_n else negatives
                frame = pd.concat([keep_pos, keep_neg], ignore_index=True).sample(
                    frac=1.0,
                    random_state=int(args.random_seed),
                ).reset_index(drop=True)

    processed_oracle_starts = int(sum(int(row["oracle_starts"]) for row in symbol_summaries))
    summary = {
        "year": int(year),
        "roots": roots,
        "symbols_requested": symbols,
        "symbols_seen": int(len({row["symbol"] for row in symbol_summaries})),
        "oracle_starts": processed_oracle_starts,
        "oracle_starts_available": int(len(oracle)),
        "eligible_candidates": int(total_candidates),
        "positive_rows": int(total_positive_rows),
        "materialized_rows": int(len(frame)),
        "materialized_positives": int(pd.to_numeric(frame["is_oracle_start"], errors="coerce").fillna(0).sum()),
        "train_sample": bool(train_sample),
        "symbol_summaries": symbol_summaries,
    }
    print(f"{year}: {json.dumps({k: v for k, v in summary.items() if k != 'symbol_summaries'}, default=to_jsonable)}", flush=True)
    return frame, summary


def sample_training_rows_from_full(frame: pd.DataFrame, args: argparse.Namespace, year: int) -> pd.DataFrame:
    """Create the historical training sample from a full cached live grid."""
    labels = pd.to_numeric(frame["is_oracle_start"], errors="coerce").fillna(0).astype(int)
    if "negative_allowed" in frame.columns:
        negative_allowed = pd.to_numeric(frame["negative_allowed"], errors="coerce").fillna(0).astype(int)
    else:
        negative_allowed = pd.Series([1] * len(frame), index=frame.index, dtype=int)

    parts: list[pd.DataFrame] = []
    group_cols = [col for col in ["symbol", "direction"] if col in frame.columns]
    grouped = frame.groupby(group_cols, sort=False) if group_cols else [(None, frame)]
    for _, group in grouped:
        group_labels = labels.loc[group.index]
        group_allowed = negative_allowed.loc[group.index]
        positives = group[group_labels == 1]
        negatives = group[(group_labels == 0) & (group_allowed == 1)]
        target_negatives = int(
            max(
                int(args.base_negatives_per_symbol),
                math.ceil(len(positives) * float(args.negative_ratio)),
            )
        )
        if len(negatives) > target_negatives:
            negatives = negatives.sample(
                n=target_negatives,
                random_state=int(args.random_seed) + int(year),
            )
        if len(positives) or len(negatives):
            parts.append(pd.concat([positives, negatives], ignore_index=False))

    if not parts:
        raise ValueError(f"No train rows could be sampled from cached candidate rows for {year}")
    sampled = pd.concat(parts, ignore_index=False).sample(
        frac=1.0,
        random_state=int(args.random_seed) + int(year),
    ).reset_index(drop=True)

    if int(args.max_train_rows) > 0 and len(sampled) > int(args.max_train_rows):
        positives = sampled[pd.to_numeric(sampled["is_oracle_start"], errors="coerce").fillna(0).astype(int) == 1]
        negatives = sampled[pd.to_numeric(sampled["is_oracle_start"], errors="coerce").fillna(0).astype(int) == 0]
        max_rows = int(args.max_train_rows)
        if positives.empty or negatives.empty:
            sampled = sampled.sample(n=max_rows, random_state=int(args.random_seed)).reset_index(drop=True)
        else:
            keep_pos_n = min(len(positives), max(1, max_rows // 2))
            keep_neg_n = min(len(negatives), max_rows - keep_pos_n)
            if keep_neg_n <= 0:
                keep_neg_n = 1
                keep_pos_n = max(1, max_rows - keep_neg_n)
            keep_pos = positives.sample(n=keep_pos_n, random_state=int(args.random_seed)) if len(positives) > keep_pos_n else positives
            keep_neg = negatives.sample(n=keep_neg_n, random_state=int(args.random_seed)) if len(negatives) > keep_neg_n else negatives
            sampled = pd.concat([keep_pos, keep_neg], ignore_index=True).sample(
                frac=1.0,
                random_state=int(args.random_seed),
            ).reset_index(drop=True)

    sampled["train_sample"] = 1
    return sampled


def load_or_build_year_rows(
    conn,
    year: int,
    args: argparse.Namespace,
    rng: np.random.Generator,
    train_sample: bool,
) -> tuple[pd.DataFrame, dict[str, Any]]:
    wants_cache = bool(args.read_candidate_cache or args.write_candidate_cache or args.only_build_candidate_cache)
    if not wants_cache:
        return build_year_rows(conn, int(year), args, rng, train_sample=train_sample)

    if args.read_candidate_cache and candidate_cache.has_year(conn, args, wave.ABCD_ROOT, int(year)):
        full_frame, summary = candidate_cache.load_year(conn, args, wave.ABCD_ROOT, int(year))
    else:
        full_frame, summary = build_year_rows(conn, int(year), args, rng, train_sample=False)
        if args.write_candidate_cache or args.only_build_candidate_cache:
            candidate_cache.save_year(conn, args, wave.ABCD_ROOT, int(year), full_frame, summary)
            print(
                f"{year}: stored candidate cache set={candidate_cache.default_candidate_set_id(args)} "
                f"rows={len(full_frame):,}",
                flush=True,
            )

    if train_sample:
        sampled = sample_training_rows_from_full(full_frame, args, int(year))
        sampled_summary = dict(summary)
        sampled_summary["materialized_rows"] = int(len(sampled))
        sampled_summary["materialized_positives"] = int(pd.to_numeric(sampled["is_oracle_start"], errors="coerce").fillna(0).sum())
        sampled_summary["train_sample"] = True
        sampled_summary["source_candidate_set_id"] = candidate_cache.default_candidate_set_id(args)
        return sampled, sampled_summary

    summary = dict(summary)
    summary["source_candidate_set_id"] = candidate_cache.default_candidate_set_id(args)
    return full_frame, summary


def train_model(train: pd.DataFrame, args: argparse.Namespace) -> CatBoostClassifier:
    model = CatBoostClassifier(
        loss_function="Logloss",
        eval_metric="AUC",
        iterations=int(args.iterations),
        depth=int(args.depth),
        learning_rate=float(args.learning_rate),
        l2_leaf_reg=float(args.l2_leaf_reg),
        random_seed=int(args.random_seed),
        auto_class_weights="Balanced",
        verbose=100,
        allow_writing_files=False,
    )
    model.fit(sampled_stage1.prepare_pool(train, include_target=True))
    return model


def add_scores(frame: pd.DataFrame, model: CatBoostClassifier) -> pd.DataFrame:
    scored = frame.copy()
    scored["oracle_start_score"] = model.predict_proba(sampled_stage1.prepare_pool(scored, include_target=False))[:, 1]
    return scored


def threshold_candidates(args: argparse.Namespace, scored: pd.DataFrame) -> list[float]:
    values = [float(part.strip()) for part in str(args.thresholds or "").split(",") if part.strip()]
    scores = pd.to_numeric(scored["oracle_start_score"], errors="coerce").fillna(0.0).to_numpy()
    quantiles = [float(value) for value in np.quantile(scores, np.linspace(0.55, 0.99, 18))]
    return sorted(set(round(value, 8) for value in [*values, *quantiles] if 0.0 <= value <= 1.0))


def event_summary(scored: pd.DataFrame, threshold: float, args: argparse.Namespace, source_summary: dict[str, Any]) -> dict[str, Any]:
    picked_rows: list[dict[str, Any]] = []
    work = scored.copy()
    work["score_num"] = pd.to_numeric(work["oracle_start_score"], errors="coerce").fillna(0.0)
    work["signal_idx_num"] = pd.to_numeric(work["signal_idx"], errors="coerce").fillna(-1).astype(int)
    work = work[work["score_num"] >= float(threshold)].sort_values(
        ["symbol", "direction", "signal_idx_num", "score_num"],
        ascending=[True, True, True, False],
    )
    for (_, _), group in work.groupby(["symbol", "direction"], sort=False):
        next_allowed_idx = -1
        for _, row in group.iterrows():
            idx = int(row["signal_idx_num"])
            if idx < next_allowed_idx:
                continue
            nearest_abs = row.get("nearest_abs_bars")
            matched = nearest_abs is not None and pd.notna(nearest_abs) and int(nearest_abs) <= int(args.match_window_bars)
            picked_rows.append(
                {
                    "candidate_uid": str(row.get("candidate_uid") or ""),
                    "symbol": str(row.get("symbol") or ""),
                    "direction": str(row.get("direction") or ""),
                    "signal_date": row.get("signal_date"),
                    "signal_idx": idx,
                    "score": float(row["score_num"]),
                    "matched_oracle": bool(matched),
                    "nearest_oracle_trade_id": str(row.get("nearest_oracle_trade_id") or ""),
                    "nearest_delta_bars": row.get("nearest_delta_bars"),
                    "nearest_abs_bars": row.get("nearest_abs_bars"),
                }
            )
            next_allowed_idx = idx + int(args.cooldown_bars) + 1
    picks = pd.DataFrame(picked_rows)
    oracle_total = int(source_summary.get("oracle_starts") or 0)
    if picks.empty:
        return {
            "threshold": float(threshold),
            "picked_events": 0,
            "matched_picks": 0,
            "pick_match_rate": 0.0,
            "matched_oracle_starts": 0,
            "oracle_recall": 0.0,
            "picks_per_oracle": 0.0,
            "median_abs_bars": None,
            "mean_abs_bars": None,
        }
    matched = picks[picks["matched_oracle"].astype(bool)].copy()
    unique_oracles = matched["nearest_oracle_trade_id"].dropna().astype(str)
    unique_oracles = unique_oracles[unique_oracles != ""].nunique()
    abs_bars = pd.to_numeric(matched["nearest_abs_bars"], errors="coerce").dropna()
    return {
        "threshold": float(threshold),
        "picked_events": int(len(picks)),
        "matched_picks": int(len(matched)),
        "pick_match_rate": float(len(matched) / len(picks)) if len(picks) else 0.0,
        "matched_oracle_starts": int(unique_oracles),
        "oracle_recall": float(unique_oracles / oracle_total) if oracle_total else 0.0,
        "picks_per_oracle": float(len(picks) / oracle_total) if oracle_total else 0.0,
        "median_abs_bars": float(abs_bars.median()) if len(abs_bars) else None,
        "mean_abs_bars": float(abs_bars.mean()) if len(abs_bars) else None,
    }


def choose_threshold(summaries: list[dict[str, Any]], args: argparse.Namespace) -> float:
    allowed = [
        row
        for row in summaries
        if float(row["oracle_recall"]) >= float(args.min_oracle_recall)
        and float(row["picks_per_oracle"]) <= float(args.max_picks_per_oracle)
    ]
    if allowed:
        best = max(allowed, key=lambda row: (float(row["pick_match_rate"]), -float(row["picks_per_oracle"])))
        return float(best["threshold"])
    best = max(
        summaries,
        key=lambda row: (
            2
            * float(row["pick_match_rate"])
            * float(row["oracle_recall"])
            / max(float(row["pick_match_rate"]) + float(row["oracle_recall"]), 1e-9),
            float(row["oracle_recall"]),
        ),
    )
    return float(best["threshold"])


def main() -> int:
    args = parse_args()
    rid = run_id(args)
    output_dir = wave.ABCD_ROOT / "model_registry" / rid
    if not args.only_build_candidate_cache:
        if output_dir.exists() and not args.replace_run:
            raise ValueError(f"Run already exists: {rid}. Use --replace-run.")
        output_dir.mkdir(parents=True, exist_ok=True)

    rng = np.random.default_rng(int(args.random_seed))
    conn = wave.connect()
    try:
        if args.only_build_candidate_cache:
            years = sorted(set(parse_years(args.train_years) + [int(args.threshold_year), int(args.valid_year)]))
            for year in years:
                load_or_build_year_rows(conn, int(year), args, rng, train_sample=False)
            print(
                f"Built candidate cache only: {candidate_cache.default_candidate_set_id(args)} "
                f"years={years}",
                flush=True,
            )
            return 0

        train_parts = []
        train_summaries = {}
        for year in parse_years(args.train_years):
            frame, summary = load_or_build_year_rows(conn, int(year), args, rng, train_sample=True)
            train_parts.append(frame)
            train_summaries[str(year)] = summary
        threshold, threshold_summary = load_or_build_year_rows(conn, int(args.threshold_year), args, rng, train_sample=False)
        valid, valid_summary = load_or_build_year_rows(conn, int(args.valid_year), args, rng, train_sample=False)
    finally:
        conn.close()

    train = pd.concat(train_parts, ignore_index=True).sample(frac=1.0, random_state=int(args.random_seed)).reset_index(drop=True)
    print(
        f"Training Stage 1 live-grid model rows={len(train):,} positives={int(train['is_oracle_start'].sum()):,}",
        flush=True,
    )
    model = train_model(train, args)
    threshold_scored = add_scores(threshold, model)
    valid_scored = add_scores(valid, model)

    threshold_sweep = [event_summary(threshold_scored, value, args, threshold_summary) for value in threshold_candidates(args, threshold_scored)]
    selected_threshold = choose_threshold(threshold_sweep, args)
    valid_sweep = [event_summary(valid_scored, float(row["threshold"]), args, valid_summary) for row in threshold_sweep]
    selected_valid = event_summary(valid_scored, selected_threshold, args, valid_summary)

    print(f"selected_stage1_threshold={selected_threshold:.6f}", flush=True)
    print(json.dumps({"threshold_year": threshold_sweep, "valid_selected": selected_valid}, indent=2, default=to_jsonable), flush=True)

    model.save_model(str(output_dir / "catboost_oracle_start_live_grid_model.cbm"))
    pd.DataFrame(threshold_sweep).to_csv(output_dir / f"threshold_sweep_{args.threshold_year}.csv", index=False)
    pd.DataFrame(valid_sweep).to_csv(output_dir / f"threshold_sweep_{args.valid_year}.csv", index=False)
    if args.save_row_csvs:
        train.to_csv(output_dir / "live_grid_rows_train_sample.csv", index=False)
        threshold_scored.to_csv(output_dir / f"live_grid_rows_{args.threshold_year}.csv", index=False)
        valid_scored.to_csv(output_dir / f"live_grid_rows_{args.valid_year}.csv", index=False)

    metadata = {
        "run_id": rid,
        "model_type": "catboost_oracle_start_live_grid_stage1",
        "oracle_run_id": args.oracle_run_id,
        "timeframe": args.timeframe,
        "roots": parse_list(args.roots),
        "symbols": parse_list(args.symbols),
        "train_years": parse_years(args.train_years),
        "threshold_year": int(args.threshold_year),
        "valid_year": int(args.valid_year),
        "label_parameters": {
            "positive_pre_bars": int(args.positive_pre_bars),
            "positive_post_bars": int(args.positive_post_bars),
            "negative_exclusion_bars": int(args.negative_exclusion_bars),
            "match_window_bars": int(args.match_window_bars),
        },
        "training_parameters": {
            "negative_ratio": float(args.negative_ratio),
            "base_negatives_per_symbol": int(args.base_negatives_per_symbol),
            "iterations": int(args.iterations),
            "depth": int(args.depth),
            "learning_rate": float(args.learning_rate),
            "l2_leaf_reg": float(args.l2_leaf_reg),
            "random_seed": int(args.random_seed),
        },
        "threshold_policy": {
            "selected_threshold": float(selected_threshold),
            "min_oracle_recall": float(args.min_oracle_recall),
            "max_picks_per_oracle": float(args.max_picks_per_oracle),
            "cooldown_bars": int(args.cooldown_bars),
        },
        "features": {"cat": sampled_stage1.CAT_FEATURES, "num": sampled_stage1.NUM_FEATURES},
        "source_summaries": {
            "train": train_summaries,
            str(args.threshold_year): threshold_summary,
            str(args.valid_year): valid_summary,
        },
        "selected_valid_summary": selected_valid,
        "read": "Stage 1 trained from candle-grid rows. Oracle is used only for historical labels and validation grading.",
        "candidate_set_id": candidate_cache.default_candidate_set_id(args)
        if (args.read_candidate_cache or args.write_candidate_cache or args.only_build_candidate_cache)
        else None,
    }
    (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")
    print(f"Saved Stage 1 live-grid model: {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
