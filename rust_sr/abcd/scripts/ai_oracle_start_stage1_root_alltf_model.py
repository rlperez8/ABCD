#!/usr/bin/env python3
"""
Train a Level 2 root all-timeframe oracle trend-start specialist.

Level 1 specialists learn one root + one timeframe. This Level 2 specialist
learns one root across many timeframes, with the source timeframe included as a
feature. It is still a Stage 1 model: its job is to find possible trend starts.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd

try:
    from catboost import CatBoostClassifier, Pool
except ImportError as exc:  # pragma: no cover
    raise SystemExit("CatBoost is required. Run with .venv_ai\\Scripts\\python.exe") from exc


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_scanner_model as scanner
import ai_oracle_start_stage1_live_grid_model as stage1
import ai_oracle_trend_start_model as start_model
import ai_wave_rider_research as wave


DEFAULT_TIMEFRAMES = "1m,2m,3m,4m,5m,6m,7m,8m,9m,10m,11m,12m,13m,14m,15m,30m"

CAT_FEATURES = list(start_model.CAT_FEATURES) + ["source_timeframe"]
NUM_FEATURES = list(start_model.NUM_FEATURES) + ["timeframe_minutes"]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True, help="Single root symbol for this all-TF specialist.")
    parser.add_argument("--timeframes", default=DEFAULT_TIMEFRAMES)
    parser.add_argument("--train-years", default="2024")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--run-prefix", default="aicw-oracle-start-root-alltf-v1")
    parser.add_argument(
        "--oracle-run-id-template",
        default="oracle-perfect-trends-tfspec-v1-{timeframe}-2024_2026",
    )
    parser.add_argument("--iterations", type=int, default=650)
    parser.add_argument("--depth", type=int, default=6)
    parser.add_argument("--learning-rate", type=float, default=0.045)
    parser.add_argument("--l2-leaf-reg", type=float, default=10.0)
    parser.add_argument("--positive-pre-bars", type=int, default=0)
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
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--limit-symbols", type=int, default=0)
    parser.add_argument("--save-row-csvs", action="store_true")
    parser.add_argument("--replace-run", action="store_true")

    # Candle feature defaults matching Level 1.
    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=240.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    return parser.parse_args()


def parse_list(raw: str) -> list[str]:
    return [part.strip().lower() for part in str(raw or "").split(",") if part.strip()]


def parse_years(raw: str) -> list[int]:
    years = [int(part) for part in stage1.parse_list(raw)]
    if not years:
        raise ValueError("At least one train year is required")
    return years


def to_jsonable(value: Any) -> Any:
    if isinstance(value, pd.Timestamp):
        return value.isoformat()
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def run_id(args: argparse.Namespace) -> str:
    train = "_".join(str(year) for year in parse_years(args.train_years))
    return f"{args.run_prefix}-{str(args.root).upper()}-alltf-tr{train}-v{int(args.valid_year)}"


def oracle_run_id(args: argparse.Namespace, timeframe: str) -> str:
    return str(args.oracle_run_id_template).format(timeframe=timeframe)


def child_args(args: argparse.Namespace, timeframe: str) -> argparse.Namespace:
    return argparse.Namespace(
        roots=str(args.root).upper(),
        symbols="",
        timeframe=timeframe,
        oracle_run_id=oracle_run_id(args, timeframe),
        positive_pre_bars=int(args.positive_pre_bars),
        positive_post_bars=int(args.positive_post_bars),
        negative_exclusion_bars=int(args.negative_exclusion_bars),
        match_window_bars=int(args.match_window_bars),
        negative_ratio=float(args.negative_ratio),
        base_negatives_per_symbol=int(args.base_negatives_per_symbol),
        max_train_rows=0,
        random_seed=int(args.random_seed),
        limit_symbols=int(args.limit_symbols),
        entry_breakout_bars=int(args.entry_breakout_bars),
        trail_lookback_bars=int(args.trail_lookback_bars),
        atr_period=int(args.atr_period),
        atr_stop_pad=float(args.atr_stop_pad),
        min_risk_ticks=float(args.min_risk_ticks),
        max_risk_ticks=float(args.max_risk_ticks),
        min_relative_volume=float(args.min_relative_volume),
    )


def add_timeframe_features(frame: pd.DataFrame, timeframe: str) -> pd.DataFrame:
    table_name, minutes = scanner.table_for_timeframe(timeframe)
    work = frame.copy()
    work["source_timeframe"] = str(timeframe)
    work["timeframe_minutes"] = float(minutes)
    work["candidate_uid"] = work["candidate_uid"].map(lambda value: f"{timeframe}|{value}")
    return work


def build_year_rows(
    conn,
    year: int,
    args: argparse.Namespace,
    rng: np.random.Generator,
    train_sample: bool,
) -> tuple[pd.DataFrame, dict[str, Any]]:
    frames: list[pd.DataFrame] = []
    summaries: dict[str, Any] = {}
    for timeframe in parse_list(args.timeframes):
        print(f"{year}: root-alltf {str(args.root).upper()} {timeframe} build start", flush=True)
        tf_args = child_args(args, timeframe)
        frame, summary = stage1.build_year_rows(conn, int(year), tf_args, rng, train_sample=train_sample)
        frames.append(add_timeframe_features(frame, timeframe))
        summaries[timeframe] = summary

    combined = pd.concat(frames, ignore_index=True)
    if train_sample:
        combined = combined.sample(frac=1.0, random_state=int(args.random_seed) + int(year)).reset_index(drop=True)
        if int(args.max_train_rows) > 0 and len(combined) > int(args.max_train_rows):
            positives = combined[combined["is_oracle_start"] == 1]
            negatives = combined[combined["is_oracle_start"] == 0]
            max_rows = int(args.max_train_rows)
            if len(positives) >= max_rows:
                keep_pos_n = max(1, min(len(positives), max_rows // 2))
                keep_neg_n = min(len(negatives), max_rows - keep_pos_n)
            else:
                keep_pos_n = len(positives)
                keep_neg_n = min(len(negatives), max_rows - keep_pos_n)
            if keep_neg_n == 0 and len(negatives) > 0 and keep_pos_n > 1:
                keep_pos_n -= 1
                keep_neg_n = 1
            keep_pos = positives.sample(n=keep_pos_n, random_state=int(args.random_seed) + int(year))
            keep_neg = negatives.sample(n=keep_neg_n, random_state=int(args.random_seed) + int(year))
            combined = pd.concat([keep_pos, keep_neg], ignore_index=True).sample(
                frac=1.0,
                random_state=int(args.random_seed) + int(year),
            ).reset_index(drop=True)

    summary = {
        "year": int(year),
        "root": str(args.root).upper(),
        "timeframes": parse_list(args.timeframes),
        "oracle_starts": int(sum(int(item.get("oracle_starts", 0) or 0) for item in summaries.values())),
        "oracle_starts_available": int(sum(int(item.get("oracle_starts_available", 0) or 0) for item in summaries.values())),
        "eligible_candidates": int(sum(int(item.get("eligible_candidates", 0) or 0) for item in summaries.values())),
        "positive_rows": int(sum(int(item.get("positive_rows", 0) or 0) for item in summaries.values())),
        "materialized_rows": int(len(combined)),
        "materialized_positives": int(pd.to_numeric(combined["is_oracle_start"], errors="coerce").fillna(0).sum()),
        "train_sample": bool(train_sample),
        "timeframe_summaries": summaries,
    }
    print(
        f"{year}: root-alltf {json.dumps({k: v for k, v in summary.items() if k != 'timeframe_summaries'}, default=to_jsonable)}",
        flush=True,
    )
    return combined, summary


def prepare_pool(frame: pd.DataFrame, include_target: bool) -> Pool:
    work = frame.copy()
    for col in CAT_FEATURES:
        if col not in work.columns:
            work[col] = "unknown"
        work[col] = work[col].fillna("unknown").astype(str)
    for col in NUM_FEATURES:
        if col not in work.columns:
            work[col] = 0.0
        work[col] = pd.to_numeric(work[col], errors="coerce").replace([np.inf, -np.inf], np.nan).fillna(0.0)
    cols = CAT_FEATURES + NUM_FEATURES
    if include_target:
        return Pool(work[cols], label=work["is_oracle_start"].astype(int), cat_features=CAT_FEATURES)
    return Pool(work[cols], cat_features=CAT_FEATURES)


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
    model.fit(prepare_pool(train, include_target=True))
    return model


def add_scores(frame: pd.DataFrame, model: CatBoostClassifier) -> pd.DataFrame:
    scored = frame.copy()
    scored["oracle_start_score"] = model.predict_proba(prepare_pool(scored, include_target=False))[:, 1]
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
        ["source_timeframe", "symbol", "direction", "signal_idx_num", "score_num"],
        ascending=[True, True, True, True, False],
    )
    for (_, _, _), group in work.groupby(["source_timeframe", "symbol", "direction"], sort=False):
        next_allowed_idx = -1
        for _, row in group.iterrows():
            idx = int(row["signal_idx_num"])
            if idx < next_allowed_idx:
                continue
            nearest_abs = row.get("nearest_abs_bars")
            matched = nearest_abs is not None and pd.notna(nearest_abs) and int(nearest_abs) <= int(args.match_window_bars)
            picked_rows.append(
                {
                    "source_timeframe": str(row.get("source_timeframe") or ""),
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
    if output_dir.exists() and not args.replace_run:
        raise ValueError(f"Run already exists: {rid}. Use --replace-run.")
    output_dir.mkdir(parents=True, exist_ok=True)

    rng = np.random.default_rng(int(args.random_seed))
    conn = wave.connect()
    try:
        train_parts = []
        train_summaries = {}
        for year in parse_years(args.train_years):
            frame, summary = build_year_rows(conn, int(year), args, rng, train_sample=True)
            train_parts.append(frame)
            train_summaries[str(year)] = summary
        threshold, threshold_summary = build_year_rows(conn, int(args.threshold_year), args, rng, train_sample=False)
        valid, valid_summary = build_year_rows(conn, int(args.valid_year), args, rng, train_sample=False)
    finally:
        conn.close()

    train = pd.concat(train_parts, ignore_index=True).sample(frac=1.0, random_state=int(args.random_seed)).reset_index(drop=True)
    print(
        f"Training root-alltf Stage 1 model rows={len(train):,} positives={int(train['is_oracle_start'].sum()):,}",
        flush=True,
    )
    model = train_model(train, args)
    threshold_scored = add_scores(threshold, model)
    valid_scored = add_scores(valid, model)
    threshold_sweep = [event_summary(threshold_scored, value, args, threshold_summary) for value in threshold_candidates(args, threshold_scored)]
    selected_threshold = choose_threshold(threshold_sweep, args)
    valid_sweep = [event_summary(valid_scored, float(row["threshold"]), args, valid_summary) for row in threshold_sweep]
    selected_valid = event_summary(valid_scored, selected_threshold, args, valid_summary)

    print(f"selected_root_alltf_stage1_threshold={selected_threshold:.6f}", flush=True)
    print(json.dumps({"threshold_year": threshold_sweep, "valid_selected": selected_valid}, indent=2, default=to_jsonable), flush=True)

    model.save_model(str(output_dir / "catboost_oracle_start_root_alltf_model.cbm"))
    pd.DataFrame(threshold_sweep).to_csv(output_dir / f"threshold_sweep_{args.threshold_year}.csv", index=False)
    pd.DataFrame(valid_sweep).to_csv(output_dir / f"threshold_sweep_{args.valid_year}.csv", index=False)
    if args.save_row_csvs:
        train.to_csv(output_dir / "root_alltf_rows_train_sample.csv", index=False)
        threshold_scored.to_csv(output_dir / f"root_alltf_rows_{args.threshold_year}.csv", index=False)
        valid_scored.to_csv(output_dir / f"root_alltf_rows_{args.valid_year}.csv", index=False)

    metadata = {
        "run_id": rid,
        "model_type": "catboost_oracle_start_root_alltf_stage1",
        "oracle_run_id_template": args.oracle_run_id_template,
        "root": str(args.root).upper(),
        "timeframes": parse_list(args.timeframes),
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
        "features": {"cat": CAT_FEATURES, "num": NUM_FEATURES},
        "source_summaries": {
            "train": train_summaries,
            str(args.threshold_year): threshold_summary,
            str(args.valid_year): valid_summary,
        },
        "selected_valid_summary": selected_valid,
        "read": "Level 2 Stage 1 specialist trained on one root across all selected timeframes.",
    }
    (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")
    print(f"Saved root-alltf Stage 1 model: {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
