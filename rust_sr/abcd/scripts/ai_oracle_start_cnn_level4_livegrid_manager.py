#!/usr/bin/env python3
"""Train a CNN Level 4 manager on full live-grid lower-level reports.

Lower CNN levels are fixed models. This script asks them to score the same
2025/2026 live-style candle grid, trains a CatBoost manager on 2025 lower-level
reports, then evaluates 2026 with the same event rules used by the CatBoost
Stage 1 live-grid tests.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import sys
from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd
import torch

try:
    from catboost import CatBoostClassifier, Pool
except ImportError as exc:  # pragma: no cover
    raise SystemExit("CatBoost is required. Run with .venv_ai\\Scripts\\python.exe") from exc


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_oracle_start_cnn_specialist as cnn_specialist
import ai_oracle_start_stage1_live_grid_model as live_grid
import ai_candle_wave_scanner_model as scanner
import ai_oracle_trend_start_model as start_model
import ai_oracle_trend_start_visual_cnn as visual_cnn
import ai_wave_rider_research as wave


DEFAULT_LEVEL1_PREFIX = "aicw-oracle-start-cnn-tower-v1"
DEFAULT_LEVEL1_LIVEGRID_RUN = "aicw-oracle-start-cnn-l1-livegrid-eval-v1-NQ-th2025-v2026"
DEFAULT_LEVEL2_RUN = "aicw-oracle-start-cnn-tower-v1-l2-NQ-alltf-07cefd90-tr2024-v2026"
DEFAULT_LEVEL3_RUN = "aicw-oracle-start-cnn-tower-v1-l3-ALL-alltf-07cefd90-tr2024-v2026"
DEFAULT_TIMEFRAMES = cnn_specialist.DEFAULT_TIMEFRAMES


CAT_FEATURES = [
    "root_symbol",
    "symbol",
    "direction",
    "source_timeframe",
    "trend_label",
    "directional_trend",
    "volume_bucket",
    "compression_bucket",
]

NUM_FEATURES = [
    "timeframe_minutes",
    "signal_hour",
    "signal_day_of_week",
    "signal_month",
    "signal_score",
    "risk_ticks",
    "atr_ticks",
    "atr_pct",
    "prior_range_ticks",
    "ema_gap_atr",
    "ema_fast_slope_atr",
    "breakout_atr",
    "relative_volume",
    "compression",
    "close_location_prior_range",
    "ret_3_atr",
    "ret_6_atr",
    "ret_12_atr",
    "distance_to_high_48_atr",
    "distance_to_low_48_atr",
    "range_position_48",
    "body_atr",
    "upper_wick_atr",
    "lower_wick_atr",
    "l1_score",
    "l2_score",
    "l3_score",
    "score_mean",
    "score_min",
    "score_max",
    "score_spread",
    "score_std",
    "l2_minus_l1",
    "l3_minus_l1",
    "l3_minus_l2",
    "l1_above_threshold",
    "l2_above_threshold",
    "l3_above_threshold",
    "levels_above_threshold",
    "all_levels_above_threshold",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default="NQ")
    parser.add_argument("--timeframes", default=DEFAULT_TIMEFRAMES)
    parser.add_argument("--oracle-run-id-template", default="oracle-perfect-trends-tfspec-v1-{timeframe}-2024_2026")
    parser.add_argument("--level1-prefix", default=DEFAULT_LEVEL1_PREFIX)
    parser.add_argument("--level1-livegrid-run-id", default=DEFAULT_LEVEL1_LIVEGRID_RUN)
    parser.add_argument("--level2-run-id", default=DEFAULT_LEVEL2_RUN)
    parser.add_argument("--level3-run-id", default=DEFAULT_LEVEL3_RUN)
    parser.add_argument("--run-prefix", default="aicw-oracle-start-cnn-level4-livegrid-manager-v1")
    parser.add_argument("--train-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--match-window-bars", type=int, default=4)
    parser.add_argument("--cooldown-bars", type=int, default=6)
    parser.add_argument("--min-oracle-recall", type=float, default=0.90)
    parser.add_argument("--max-picks-per-oracle", type=float, default=5.0)
    parser.add_argument("--thresholds", default="0.20,0.25,0.30,0.35,0.40,0.45,0.50,0.55,0.60,0.65,0.70,0.75,0.80,0.85,0.90")
    parser.add_argument("--iterations", type=int, default=550)
    parser.add_argument("--depth", type=int, default=6)
    parser.add_argument("--learning-rate", type=float, default=0.04)
    parser.add_argument("--l2-leaf-reg", type=float, default=10.0)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--batch-size", type=int, default=1024)
    parser.add_argument("--max-manager-train-rows", type=int, default=500000)
    parser.add_argument("--max-rows-per-timeframe", type=int, default=0, help="Debug cap only. 0 means full live grid.")
    parser.add_argument("--save-score-csvs", action="store_true")
    parser.add_argument("--replace-run", action="store_true")
    return parser.parse_args()


def parse_timeframes(raw: str) -> list[str]:
    values = [part.strip().lower() for part in str(raw or "").split(",") if part.strip()]
    if not values:
        raise ValueError("At least one timeframe is required.")
    return values


def run_id(args: argparse.Namespace) -> str:
    root = str(args.root).upper()
    raw = f"{args.run_prefix}-{root}-tr{int(args.train_year)}-v{int(args.valid_year)}"
    if len(raw) <= 96:
        return raw
    digest = hashlib.sha1(raw.encode("utf-8")).hexdigest()[:10]
    return f"{args.run_prefix}-{digest}-v{int(args.valid_year)}"


def to_jsonable(value: Any) -> Any:
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, pd.Timestamp):
        return value.isoformat()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def l1_run_id(args: argparse.Namespace, timeframe: str) -> str:
    return f"{args.level1_prefix}-l1-{str(args.root).upper()}-{timeframe}-tr2024-v2026"


def load_cnn(run_id_value: str) -> tuple[visual_cnn.OracleStartCnn, dict[str, Any]]:
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id_value
    metadata = load_json(model_dir / "metadata.json")
    model = visual_cnn.OracleStartCnn(float(metadata.get("dropout", 0.20)))
    state = torch.load(model_dir / "visual_oracle_start_cnn.pt", map_location="cpu")
    model.load_state_dict(state)
    model.eval()
    return model, metadata


def timeframe_minutes(timeframe: str) -> int:
    text = str(timeframe).lower().strip()
    if text.endswith("m"):
        return int(text[:-1])
    if text.endswith("h"):
        return int(text[:-1]) * 60
    if text.endswith("d"):
        return int(text[:-1]) * 1440
    return 0


def build_args_for_timeframe(args: argparse.Namespace, timeframe: str) -> argparse.Namespace:
    return argparse.Namespace(
        oracle_run_id=str(args.oracle_run_id_template).format(timeframe=timeframe),
        timeframe=timeframe,
        roots=str(args.root).upper(),
        symbols="",
        positive_pre_bars=0,
        positive_post_bars=2,
        negative_exclusion_bars=8,
        match_window_bars=int(args.match_window_bars),
        negative_ratio=8.0,
        base_negatives_per_symbol=200,
        max_train_rows=0,
        cooldown_bars=int(args.cooldown_bars),
        min_oracle_recall=float(args.min_oracle_recall),
        max_picks_per_oracle=float(args.max_picks_per_oracle),
        thresholds=args.thresholds,
        random_seed=int(args.random_seed),
        limit_symbols=0,
        entry_breakout_bars=8,
        trail_lookback_bars=18,
        atr_period=14,
        atr_stop_pad=0.35,
        min_risk_ticks=12.0,
        max_risk_ticks=240.0,
        min_relative_volume=0.45,
    )


def render_args(args: argparse.Namespace, metadata: dict[str, Any], timeframe: str) -> argparse.Namespace:
    return argparse.Namespace(
        roots=str(args.root).upper(),
        timeframe=timeframe,
        height=int(metadata.get("height") or 40),
        width=int(metadata.get("width") or 40),
        lookback_bars=int(metadata.get("lookback_bars") or 64),
        min_candles=int(metadata.get("min_candles") or 36),
        batch_size=int(args.batch_size),
        entry_breakout_bars=8,
        trail_lookback_bars=18,
        atr_period=14,
        atr_stop_pad=0.35,
        min_risk_ticks=12.0,
        max_risk_ticks=240.0,
        min_relative_volume=0.45,
    )


def render_signature(metadata: dict[str, Any]) -> tuple[int, int, int, int]:
    return (
        int(metadata.get("height") or 40),
        int(metadata.get("width") or 40),
        int(metadata.get("lookback_bars") or 64),
        int(metadata.get("min_candles") or 36),
    )


def load_l1_thresholds(args: argparse.Namespace) -> dict[str, float]:
    path = wave.ABCD_ROOT / "model_registry" / str(args.level1_livegrid_run_id) / "cnn_l1_livegrid_readable_summary.csv"
    if not path.exists():
        return {}
    rows = pd.read_csv(path)
    rows = rows[rows["split"].astype(str) == str(args.train_year)]
    return {
        str(row["timeframe"]): float(row["threshold"])
        for _, row in rows.iterrows()
        if pd.notna(row.get("threshold"))
    }


def maybe_cap_rows(frame: pd.DataFrame, args: argparse.Namespace, year: int, timeframe: str) -> pd.DataFrame:
    cap = int(args.max_rows_per_timeframe)
    if cap <= 0 or len(frame) <= cap:
        return frame
    positives = frame[frame["is_oracle_start"].astype(int) == 1]
    negatives = frame[frame["is_oracle_start"].astype(int) == 0]
    keep_pos_n = min(len(positives), cap // 2)
    keep_neg_n = min(len(negatives), cap - keep_pos_n)
    keep_pos = positives.sample(n=keep_pos_n, random_state=int(args.random_seed) + int(year) + timeframe_minutes(timeframe)) if keep_pos_n else positives
    keep_neg = negatives.sample(n=keep_neg_n, random_state=int(args.random_seed) + int(year)) if keep_neg_n else negatives
    return pd.concat([keep_pos, keep_neg], ignore_index=True).sample(
        frac=1.0,
        random_state=int(args.random_seed) + int(year),
    ).reset_index(drop=True)


def load_lower_models(args: argparse.Namespace) -> tuple[dict[str, tuple[Any, dict[str, Any]]], tuple[Any, dict[str, Any]], tuple[Any, dict[str, Any]]]:
    l1_models: dict[str, tuple[Any, dict[str, Any]]] = {}
    for timeframe in parse_timeframes(args.timeframes):
        l1_models[timeframe] = load_cnn(l1_run_id(args, timeframe))
    l2_model = load_cnn(str(args.level2_run_id))
    l3_model = load_cnn(str(args.level3_run_id))
    return l1_models, l2_model, l3_model


def score_timeframe_rows(
    conn,
    rows: pd.DataFrame,
    year: int,
    timeframe: str,
    args: argparse.Namespace,
    l1_model: tuple[Any, dict[str, Any]],
    l2_model: tuple[Any, dict[str, Any]],
    l3_model: tuple[Any, dict[str, Any]],
) -> pd.DataFrame:
    l1, l1_meta = l1_model
    l2, l2_meta = l2_model
    l3, l3_meta = l3_model
    signature = render_signature(l1_meta)
    if render_signature(l2_meta) != signature or render_signature(l3_meta) != signature:
        raise ValueError(f"CNN render settings differ for {timeframe}; cannot share live-grid images safely.")
    model_args = render_args(args, l1_meta, timeframe)
    roots = start_model.parse_list(model_args.roots)
    candles = start_model.fetch_candles(conn, timeframe, int(year), roots)
    if candles.empty:
        raise ValueError(f"No candles found for {timeframe} {year}")

    work = rows.copy().reset_index(drop=True)
    work["signal_date"] = pd.to_datetime(work["signal_date"], errors="coerce")
    l1_scores = np.full(len(work), np.nan, dtype=np.float32)
    l2_scores = np.full(len(work), np.nan, dtype=np.float32)
    l3_scores = np.full(len(work), np.nan, dtype=np.float32)
    by_symbol_rows = {
        str(symbol): group.sort_values("signal_date")
        for symbol, group in work.reset_index().groupby("symbol", sort=False)
    }
    image_batch: list[np.ndarray] = []
    index_batch: list[int] = []

    def flush_batch() -> None:
        nonlocal image_batch, index_batch
        if not image_batch:
            return
        images = np.stack(image_batch).astype(np.float32)
        tensor = visual_cnn.to_tensor(images, model_args)
        idxs = np.array(index_batch, dtype=int)
        with torch.no_grad():
            l1_parts: list[np.ndarray] = []
            l2_parts: list[np.ndarray] = []
            l3_parts: list[np.ndarray] = []
            for start in range(0, len(tensor), int(model_args.batch_size)):
                batch = tensor[start : start + int(model_args.batch_size)]
                l1_parts.append(torch.sigmoid(l1(batch)).cpu().numpy())
                l2_parts.append(torch.sigmoid(l2(batch)).cpu().numpy())
                l3_parts.append(torch.sigmoid(l3(batch)).cpu().numpy())
        l1_scores[idxs] = np.concatenate(l1_parts).astype(np.float32)
        l2_scores[idxs] = np.concatenate(l2_parts).astype(np.float32)
        l3_scores[idxs] = np.concatenate(l3_parts).astype(np.float32)
        image_batch = []
        index_batch = []

    seen = 0
    kept = 0
    total_symbols = int(candles["symbol"].nunique())
    total_targets = len(work)
    for symbol_index, (symbol, raw_group) in enumerate(candles.groupby("symbol", sort=True), start=1):
        symbol_rows = by_symbol_rows.get(str(symbol))
        if symbol_rows is None or symbol_rows.empty:
            continue
        group = scanner.enrich_candles(raw_group[["ts_utc", "open", "high", "low", "close", "volume"]].copy(), model_args)
        times = pd.to_datetime(group["ts_utc"], errors="coerce").to_numpy(dtype="datetime64[ns]")
        for item in symbol_rows.to_dict("records"):
            seen += 1
            signal_ts = pd.Timestamp(item["signal_date"])
            idx = int(np.searchsorted(times, np.datetime64(signal_ts), side="left"))
            if idx >= len(group):
                continue
            start_idx = max(0, idx - int(model_args.lookback_bars) + 1)
            context = group.iloc[start_idx : idx + 1].reset_index(drop=True)
            if len(context) < int(model_args.min_candles):
                continue
            image = visual_cnn.visual.render_chart_image(context, str(item["direction"]), int(model_args.height), int(model_args.width))
            image_batch.append(image.reshape(-1))
            index_batch.append(int(item["index"]))
            kept += 1
            if len(image_batch) >= int(model_args.batch_size):
                flush_batch()
        flush_batch()
        print(
            f"cnn-l4-livegrid-{year}-{timeframe}: rendered symbols={symbol_index:,}/{total_symbols:,} "
            f"seen_rows={seen:,}/{total_targets:,} kept={kept:,}",
            flush=True,
        )

    valid = ~np.isnan(l1_scores) & ~np.isnan(l2_scores) & ~np.isnan(l3_scores)
    scored = work.loc[valid].copy()
    scored["l1_score"] = l1_scores[valid].astype(float)
    scored["l2_score"] = l2_scores[valid].astype(float)
    scored["l3_score"] = l3_scores[valid].astype(float)
    print(f"cnn-l4-livegrid-{year}-{timeframe}: scored rows={len(scored):,}/{len(work):,}", flush=True)
    return scored


def add_manager_features(frame: pd.DataFrame, l1_thresholds: dict[str, float], l2_threshold: float, l3_threshold: float) -> pd.DataFrame:
    work = frame.copy()
    work["timeframe_minutes"] = work["source_timeframe"].map(timeframe_minutes).astype(float)
    for col in ["l1_score", "l2_score", "l3_score"]:
        work[col] = pd.to_numeric(work[col], errors="coerce").fillna(0.0)
    score_values = work[["l1_score", "l2_score", "l3_score"]].to_numpy(dtype=float)
    work["score_mean"] = score_values.mean(axis=1)
    work["score_min"] = score_values.min(axis=1)
    work["score_max"] = score_values.max(axis=1)
    work["score_spread"] = score_values.max(axis=1) - score_values.min(axis=1)
    work["score_std"] = score_values.std(axis=1)
    work["l2_minus_l1"] = work["l2_score"] - work["l1_score"]
    work["l3_minus_l1"] = work["l3_score"] - work["l1_score"]
    work["l3_minus_l2"] = work["l3_score"] - work["l2_score"]
    work["l1_threshold"] = work["source_timeframe"].map(lambda tf: float(l1_thresholds.get(str(tf), 0.5)))
    work["l1_above_threshold"] = (work["l1_score"] >= work["l1_threshold"]).astype(int)
    work["l2_above_threshold"] = (work["l2_score"] >= float(l2_threshold)).astype(int)
    work["l3_above_threshold"] = (work["l3_score"] >= float(l3_threshold)).astype(int)
    work["levels_above_threshold"] = work[["l1_above_threshold", "l2_above_threshold", "l3_above_threshold"]].sum(axis=1)
    work["all_levels_above_threshold"] = (work["levels_above_threshold"] == 3).astype(int)
    return work.reset_index(drop=True)


def build_year_reports(
    conn,
    year: int,
    args: argparse.Namespace,
    l1_models: dict[str, tuple[Any, dict[str, Any]]],
    l2_model: tuple[Any, dict[str, Any]],
    l3_model: tuple[Any, dict[str, Any]],
    l1_thresholds: dict[str, float],
) -> tuple[pd.DataFrame, dict[str, Any]]:
    rng = np.random.default_rng(int(args.random_seed) + int(year))
    l2_threshold = float(l2_model[1].get("selected_visual_threshold") or 0.5)
    l3_threshold = float(l3_model[1].get("selected_visual_threshold") or 0.5)
    parts: list[pd.DataFrame] = []
    summaries: dict[str, Any] = {}
    total_oracles = 0
    total_rows = 0
    for timeframe in parse_timeframes(args.timeframes):
        tf_args = build_args_for_timeframe(args, timeframe)
        rows, summary = live_grid.build_year_rows(conn, int(year), tf_args, rng, train_sample=False)
        rows = maybe_cap_rows(rows, args, int(year), timeframe)
        rows["source_timeframe"] = timeframe
        scored = score_timeframe_rows(
            conn,
            rows,
            int(year),
            timeframe,
            args,
            l1_models[timeframe],
            l2_model,
            l3_model,
        )
        if scored.empty:
            continue
        scored["source_timeframe"] = timeframe
        scored = add_manager_features(scored, l1_thresholds, l2_threshold, l3_threshold)
        parts.append(scored)
        summaries[timeframe] = summary
        total_oracles += int(summary.get("oracle_starts") or 0)
        total_rows += int(len(scored))
        print(
            f"{year} {timeframe}: lower reports rows={len(scored):,} oracles={int(summary.get('oracle_starts') or 0):,}",
            flush=True,
        )
    if not parts:
        raise ValueError(f"No lower CNN reports built for {year}")
    reports = pd.concat(parts, ignore_index=True)
    source = {
        "year": int(year),
        "root": str(args.root).upper(),
        "timeframes": parse_timeframes(args.timeframes),
        "oracle_starts": int(total_oracles),
        "scored_rows": int(total_rows),
        "timeframe_summaries": summaries,
    }
    return reports, source


def manager_train_sample(frame: pd.DataFrame, args: argparse.Namespace) -> pd.DataFrame:
    cap = int(args.max_manager_train_rows)
    if cap <= 0 or len(frame) <= cap:
        return frame.sample(frac=1.0, random_state=int(args.random_seed)).reset_index(drop=True)
    positives = frame[frame["is_oracle_start"].astype(int) == 1]
    negatives = frame[frame["is_oracle_start"].astype(int) == 0]
    if len(positives) >= cap:
        sampled = positives.sample(n=cap, random_state=int(args.random_seed))
    else:
        remaining = cap - len(positives)
        keep_neg = negatives.sample(n=min(remaining, len(negatives)), random_state=int(args.random_seed))
        sampled = pd.concat([positives, keep_neg], ignore_index=True)
    return sampled.sample(frac=1.0, random_state=int(args.random_seed)).reset_index(drop=True)


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
    features = CAT_FEATURES + NUM_FEATURES
    if include_target:
        return Pool(work[features], label=work["is_oracle_start"].astype(int), cat_features=CAT_FEATURES)
    return Pool(work[features], cat_features=CAT_FEATURES)


def train_manager(train: pd.DataFrame, args: argparse.Namespace) -> CatBoostClassifier:
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


def add_manager_scores(frame: pd.DataFrame, model: CatBoostClassifier) -> pd.DataFrame:
    out = frame.copy()
    out["manager_score"] = model.predict_proba(prepare_pool(out, include_target=False))[:, 1]
    return out


def threshold_candidates(frame: pd.DataFrame, args: argparse.Namespace) -> list[float]:
    fixed = [float(part.strip()) for part in str(args.thresholds or "").split(",") if part.strip()]
    scores = pd.to_numeric(frame["manager_score"], errors="coerce").fillna(0.0).to_numpy()
    quantiles = [float(value) for value in np.quantile(scores, np.linspace(0.55, 0.99, 24))]
    return sorted(set(round(value, 8) for value in [*fixed, *quantiles] if 0.0 <= value <= 1.0))


def event_summary(scored: pd.DataFrame, threshold: float, args: argparse.Namespace, source_summary: dict[str, Any]) -> dict[str, Any]:
    work = scored.copy()
    work["score_num"] = pd.to_numeric(work["manager_score"], errors="coerce").fillna(0.0)
    work["signal_idx_num"] = pd.to_numeric(work["signal_idx"], errors="coerce").fillna(-1).astype(int)
    work = work[work["score_num"] >= float(threshold)].sort_values(
        ["source_timeframe", "symbol", "direction", "signal_idx_num", "score_num"],
        ascending=[True, True, True, True, False],
    )
    picked_rows: list[dict[str, Any]] = []
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
                    "candidate_uid": str(row.get("candidate_uid") or ""),
                    "source_timeframe": str(row.get("source_timeframe") or ""),
                    "symbol": str(row.get("symbol") or ""),
                    "direction": str(row.get("direction") or ""),
                    "signal_date": row.get("signal_date"),
                    "signal_idx": idx,
                    "score": float(row["score_num"]),
                    "matched_oracle": bool(matched),
                    "nearest_oracle_key": f"{row.get('source_timeframe')}|{row.get('nearest_oracle_trade_id')}",
                    "nearest_delta_bars": row.get("nearest_delta_bars"),
                    "nearest_abs_bars": row.get("nearest_abs_bars"),
                }
            )
            next_allowed_idx = idx + int(args.cooldown_bars) + 1
    oracle_total = int(source_summary.get("oracle_starts") or 0)
    if not picked_rows:
        return {
            "threshold": float(threshold),
            "scored_rows": int(len(scored)),
            "picked_events": 0,
            "matched_picks": 0,
            "pick_match_rate": 0.0,
            "matched_oracle_starts": 0,
            "oracle_recall": 0.0,
            "picks_per_oracle": 0.0,
            "median_abs_bars": None,
            "mean_abs_bars": None,
        }
    picks = pd.DataFrame(picked_rows)
    matched = picks[picks["matched_oracle"].astype(bool)].copy()
    unique_oracles = matched["nearest_oracle_key"].dropna().astype(str)
    unique_oracles = unique_oracles[(unique_oracles != "") & (~unique_oracles.str.endswith("|"))].nunique()
    abs_bars = pd.to_numeric(matched["nearest_abs_bars"], errors="coerce").dropna()
    return {
        "threshold": float(threshold),
        "scored_rows": int(len(scored)),
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
        return float(max(allowed, key=lambda row: (float(row["pick_match_rate"]), -float(row["picks_per_oracle"])))["threshold"])
    return float(
        max(
            summaries,
            key=lambda row: (
                2
                * float(row["pick_match_rate"])
                * float(row["oracle_recall"])
                / max(float(row["pick_match_rate"]) + float(row["oracle_recall"]), 1e-9),
                float(row["oracle_recall"]),
            ),
        )["threshold"]
    )


def pct(value: Any) -> float:
    return round(float(value) * 100.0, 2)


def readable_row(split: str, summary: dict[str, Any]) -> dict[str, Any]:
    return {
        "split": split,
        "threshold": summary["threshold"],
        "scored_rows": summary["scored_rows"],
        "picked_events": summary["picked_events"],
        "matched_picks": summary["matched_picks"],
        "trend_accuracy_pct": pct(summary["pick_match_rate"]),
        "matched_oracle_starts": summary["matched_oracle_starts"],
        "oracle_coverage_pct": pct(summary["oracle_recall"]),
        "oracle_missed_pct": round(100.0 - pct(summary["oracle_recall"]), 2),
        "picks_per_oracle": round(float(summary["picks_per_oracle"]), 3),
        "median_abs_bars": summary["median_abs_bars"],
        "mean_abs_bars": summary["mean_abs_bars"],
    }


def write_readable(output_dir: Path, rows: list[dict[str, Any]]) -> None:
    fields = [
        "split",
        "threshold",
        "scored_rows",
        "picked_events",
        "matched_picks",
        "trend_accuracy_pct",
        "matched_oracle_starts",
        "oracle_coverage_pct",
        "oracle_missed_pct",
        "picks_per_oracle",
        "median_abs_bars",
        "mean_abs_bars",
    ]
    with (output_dir / "cnn_level4_livegrid_readable_summary.csv").open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)


def main() -> int:
    args = parse_args()
    rid = run_id(args)
    output_dir = wave.ABCD_ROOT / "model_registry" / rid
    if output_dir.exists() and not args.replace_run:
        raise ValueError(f"Run already exists: {rid}. Use --replace-run.")
    output_dir.mkdir(parents=True, exist_ok=True)

    visual_cnn.set_seed(int(args.random_seed))
    l1_thresholds = load_l1_thresholds(args)
    l1_models, l2_model, l3_model = load_lower_models(args)

    conn = wave.connect()
    try:
        print(f"Building full lower CNN live-grid reports for train year={args.train_year}", flush=True)
        train_reports, train_source = build_year_reports(conn, int(args.train_year), args, l1_models, l2_model, l3_model, l1_thresholds)
        print(f"Building full lower CNN live-grid reports for validation year={args.valid_year}", flush=True)
        valid_reports, valid_source = build_year_reports(conn, int(args.valid_year), args, l1_models, l2_model, l3_model, l1_thresholds)
    finally:
        conn.close()

    train_sample = manager_train_sample(train_reports, args)
    print(
        f"Training CNN Level 4 live-grid manager rows={len(train_sample):,}/{len(train_reports):,} "
        f"positives={int(train_sample['is_oracle_start'].sum()):,}",
        flush=True,
    )
    model = train_manager(train_sample, args)
    train_scored = add_manager_scores(train_reports, model)
    valid_scored = add_manager_scores(valid_reports, model)

    train_sweep = [event_summary(train_scored, value, args, train_source) for value in threshold_candidates(train_scored, args)]
    selected_threshold = choose_threshold(train_sweep, args)
    valid_sweep = [event_summary(valid_scored, float(row["threshold"]), args, valid_source) for row in train_sweep]
    selected_train = event_summary(train_scored, selected_threshold, args, train_source)
    selected_valid = event_summary(valid_scored, selected_threshold, args, valid_source)

    model.save_model(str(output_dir / "catboost_cnn_level4_livegrid_manager.cbm"))
    pd.DataFrame(train_sweep).to_csv(output_dir / f"cnn_level4_livegrid_threshold_sweep_{args.train_year}.csv", index=False)
    pd.DataFrame(valid_sweep).to_csv(output_dir / f"cnn_level4_livegrid_threshold_sweep_{args.valid_year}.csv", index=False)
    if args.save_score_csvs:
        train_scored.to_csv(output_dir / f"cnn_level4_livegrid_rows_{args.train_year}.csv", index=False)
        valid_scored.to_csv(output_dir / f"cnn_level4_livegrid_rows_{args.valid_year}.csv", index=False)

    readable = [readable_row(str(args.train_year), selected_train), readable_row(str(args.valid_year), selected_valid)]
    write_readable(output_dir, readable)
    metadata = {
        "run_id": rid,
        "model_type": "catboost_cnn_level4_livegrid_manager",
        "root": str(args.root).upper(),
        "timeframes": parse_timeframes(args.timeframes),
        "train_year": int(args.train_year),
        "valid_year": int(args.valid_year),
        "lower_level_runs": {
            "level1_prefix": args.level1_prefix,
            "level1_livegrid_run_id": args.level1_livegrid_run_id,
            "level2_run_id": args.level2_run_id,
            "level3_run_id": args.level3_run_id,
        },
        "event_rules": {
            "match_window_bars": int(args.match_window_bars),
            "cooldown_bars": int(args.cooldown_bars),
            "min_oracle_recall": float(args.min_oracle_recall),
            "max_picks_per_oracle": float(args.max_picks_per_oracle),
            "thresholds": args.thresholds,
        },
        "training_parameters": {
            "iterations": int(args.iterations),
            "depth": int(args.depth),
            "learning_rate": float(args.learning_rate),
            "l2_leaf_reg": float(args.l2_leaf_reg),
            "random_seed": int(args.random_seed),
            "max_manager_train_rows": int(args.max_manager_train_rows),
            "actual_manager_train_rows": int(len(train_sample)),
        },
        "features": {"cat": CAT_FEATURES, "num": NUM_FEATURES},
        "train_source_summary": train_source,
        "valid_source_summary": valid_source,
        "selected_threshold": float(selected_threshold),
        "selected_train_summary": selected_train,
        "selected_valid_summary": selected_valid,
        "read": "CNN Level 4 manager trained on 2025 full live-grid lower-level reports and validated on 2026 event metrics.",
    }
    (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")
    print(f"selected_cnn_level4_livegrid_threshold={selected_threshold:.6f}", flush=True)
    print(json.dumps({"train_selected": selected_train, "valid_selected": selected_valid}, indent=2, default=to_jsonable), flush=True)
    print(f"Saved CNN Level 4 live-grid manager: {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
