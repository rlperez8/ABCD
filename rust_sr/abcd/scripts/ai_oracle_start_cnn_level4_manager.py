#!/usr/bin/env python3
"""Train a Level 4 manager over CNN Level 1, Level 2, and Level 3 reports.

The manager uses a shared row set so the lower CNN scores are directly
comparable. By default the shared rows come from the NQ Level 2 all-timeframe
CNN scored rows for 2025/2026. Level 1 and Level 3 are then re-scored on those
same chart images before the manager is trained on 2025 and tested on 2026.
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
import ai_oracle_trend_start_model as start_model
import ai_oracle_trend_start_visual_cnn as visual_cnn
import ai_wave_rider_research as wave


DEFAULT_TIMEFRAMES = cnn_specialist.DEFAULT_TIMEFRAMES
DEFAULT_LEVEL1_PREFIX = "aicw-oracle-start-cnn-tower-v1"
DEFAULT_LEVEL2_RUN = "aicw-oracle-start-cnn-tower-v1-l2-NQ-alltf-07cefd90-tr2024-v2026"
DEFAULT_LEVEL3_RUN = "aicw-oracle-start-cnn-tower-v1-l3-ALL-alltf-07cefd90-tr2024-v2026"

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
    parser.add_argument("--train-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--level1-prefix", default=DEFAULT_LEVEL1_PREFIX)
    parser.add_argument("--level2-run-id", default=DEFAULT_LEVEL2_RUN)
    parser.add_argument("--level3-run-id", default=DEFAULT_LEVEL3_RUN)
    parser.add_argument("--run-prefix", default="aicw-oracle-start-cnn-level4-manager-v1")
    parser.add_argument("--iterations", type=int, default=450)
    parser.add_argument("--depth", type=int, default=5)
    parser.add_argument("--learning-rate", type=float, default=0.045)
    parser.add_argument("--l2-leaf-reg", type=float, default=8.0)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--batch-size", type=int, default=512)
    parser.add_argument("--min-threshold-precision", type=float, default=0.60)
    parser.add_argument("--min-threshold-picks", type=int, default=100)
    parser.add_argument("--thresholds", default="0.20,0.25,0.30,0.35,0.40,0.45,0.50,0.55,0.60,0.65,0.70,0.75,0.80,0.85,0.90")
    parser.add_argument("--save-score-csvs", action="store_true")
    parser.add_argument("--replace-run", action="store_true")
    return parser.parse_args()


def parse_timeframes(raw: str) -> list[str]:
    values = [part.strip().lower() for part in str(raw or "").split(",") if part.strip()]
    if not values:
        raise ValueError("At least one timeframe is required.")
    return values


def run_id(args: argparse.Namespace) -> str:
    raw = f"{args.run_prefix}-{str(args.root).upper()}-tr{int(args.train_year)}-v{int(args.valid_year)}"
    if len(raw) <= 96:
        return raw
    digest = hashlib.sha1(raw.encode("utf-8")).hexdigest()[:10]
    return f"{args.run_prefix}-{digest}-v{int(args.valid_year)}"


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def l1_run_id(args: argparse.Namespace, timeframe: str) -> str:
    root = str(args.root).upper()
    return f"{args.level1_prefix}-l1-{root}-{timeframe}-tr2024-v2026"


def score_file_path(run_id_value: str, year: int) -> Path:
    return wave.ABCD_ROOT / "model_registry" / run_id_value / f"visual_oracle_start_scores_{int(year)}.csv"


def load_base_rows(args: argparse.Namespace, year: int) -> pd.DataFrame:
    path = score_file_path(str(args.level2_run_id), int(year))
    if not path.exists():
        raise FileNotFoundError(f"Missing shared Level 2 score rows: {path}")
    frame = pd.read_csv(path)
    frame = frame.rename(columns={"visual_score": "l2_score"})
    frame["signal_date"] = pd.to_datetime(frame["signal_date"], errors="coerce")
    frame = frame.dropna(subset=["candidate_uid", "source_timeframe", "symbol", "signal_date", "direction"])
    frame = frame[frame["root_symbol"].astype(str).str.upper() == str(args.root).upper()].copy()
    wanted = set(parse_timeframes(args.timeframes))
    frame = frame[frame["source_timeframe"].astype(str).isin(wanted)].copy()
    if frame.empty:
        raise ValueError(f"No base rows loaded for {year}")
    return frame.reset_index(drop=True)


def load_cnn_model(run_id_value: str, args: argparse.Namespace) -> tuple[visual_cnn.OracleStartCnn, dict[str, Any]]:
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id_value
    metadata = load_json(model_dir / "metadata.json")
    model = visual_cnn.OracleStartCnn(float(metadata.get("dropout", 0.20)))
    state = torch.load(model_dir / "visual_oracle_start_cnn.pt", map_location="cpu")
    model.load_state_dict(state)
    model.eval()
    return model, metadata


def render_args(base_args: argparse.Namespace, metadata: dict[str, Any], timeframe: str) -> argparse.Namespace:
    return argparse.Namespace(
        roots=str(base_args.root).upper(),
        timeframe=timeframe,
        height=int(metadata.get("height") or 40),
        width=int(metadata.get("width") or 40),
        lookback_bars=int(metadata.get("lookback_bars") or 64),
        min_candles=int(metadata.get("min_candles") or 36),
        batch_size=int(base_args.batch_size),
        entry_breakout_bars=8,
        trail_lookback_bars=18,
        atr_period=14,
        atr_stop_pad=0.35,
        min_risk_ticks=12.0,
        max_risk_ticks=240.0,
        min_relative_volume=0.45,
    )


def score_rows_with_model(
    conn,
    rows: pd.DataFrame,
    model: visual_cnn.OracleStartCnn,
    metadata: dict[str, Any],
    year: int,
    timeframe: str,
    args: argparse.Namespace,
    label: str,
) -> pd.DataFrame:
    tf_rows = rows[rows["source_timeframe"].astype(str) == timeframe].copy()
    if tf_rows.empty:
        return pd.DataFrame(columns=["candidate_uid", label])
    model_args = render_args(args, metadata, timeframe)
    images, kept = visual_cnn.render_images_for_rows(conn, tf_rows, int(year), model_args, f"{label}-{year}-{timeframe}")
    if kept.empty:
        return pd.DataFrame(columns=["candidate_uid", label])
    scores = visual_cnn.evaluate_scores(model, images, model_args)
    out = kept[["candidate_uid"]].copy()
    out[label] = scores.astype(float)
    return out


def score_lower_levels(args: argparse.Namespace, year: int) -> pd.DataFrame:
    base = load_base_rows(args, int(year))
    l2_meta = load_json(wave.ABCD_ROOT / "model_registry" / str(args.level2_run_id) / "metadata.json")
    l3_model, l3_meta = load_cnn_model(str(args.level3_run_id), args)
    l1_models: dict[str, tuple[visual_cnn.OracleStartCnn, dict[str, Any], float]] = {}
    for timeframe in parse_timeframes(args.timeframes):
        rid = l1_run_id(args, timeframe)
        model, meta = load_cnn_model(rid, args)
        l1_models[timeframe] = (model, meta, float(meta.get("selected_visual_threshold") or 0.5))

    l1_scores: list[pd.DataFrame] = []
    l3_scores: list[pd.DataFrame] = []
    conn = wave.connect()
    try:
        for timeframe in parse_timeframes(args.timeframes):
            model, meta, _ = l1_models[timeframe]
            l1_scores.append(score_rows_with_model(conn, base, model, meta, int(year), timeframe, args, "l1_score"))
            l3_scores.append(score_rows_with_model(conn, base, l3_model, l3_meta, int(year), timeframe, args, "l3_score"))
    finally:
        conn.close()

    l1_frame = pd.concat(l1_scores, ignore_index=True) if l1_scores else pd.DataFrame(columns=["candidate_uid", "l1_score"])
    l3_frame = pd.concat(l3_scores, ignore_index=True) if l3_scores else pd.DataFrame(columns=["candidate_uid", "l3_score"])
    work = base.merge(l1_frame, on="candidate_uid", how="inner").merge(l3_frame, on="candidate_uid", how="inner")
    if work.empty:
        raise ValueError(f"No shared lower-level CNN reports built for {year}")

    work["timeframe_minutes"] = work["source_timeframe"].map(timeframe_minutes).astype(float)
    thresholds = {
        "l1": work["source_timeframe"].map(lambda tf: l1_models[str(tf)][2]).astype(float),
        "l2": float(l2_meta.get("selected_visual_threshold") or 0.5),
        "l3": float(l3_meta.get("selected_visual_threshold") or 0.5),
    }
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
    work["l1_above_threshold"] = (work["l1_score"] >= thresholds["l1"]).astype(int)
    work["l2_above_threshold"] = (work["l2_score"] >= thresholds["l2"]).astype(int)
    work["l3_above_threshold"] = (work["l3_score"] >= thresholds["l3"]).astype(int)
    work["levels_above_threshold"] = work[["l1_above_threshold", "l2_above_threshold", "l3_above_threshold"]].sum(axis=1)
    work["all_levels_above_threshold"] = (work["levels_above_threshold"] == 3).astype(int)
    return work.reset_index(drop=True)


def timeframe_minutes(timeframe: str) -> int:
    text = str(timeframe).lower().strip()
    if text.endswith("m"):
        return int(text[:-1])
    if text.endswith("h"):
        return int(text[:-1]) * 60
    if text.endswith("d"):
        return int(text[:-1]) * 1440
    return 0


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


def binary_metrics(frame: pd.DataFrame, threshold: float) -> dict[str, Any]:
    y_true = frame["is_oracle_start"].astype(int).to_numpy()
    scores = pd.to_numeric(frame["manager_score"], errors="coerce").fillna(0.0).to_numpy()
    pred = scores >= float(threshold)
    positives = int(y_true.sum())
    picks = int(pred.sum())
    tp = int(((y_true == 1) & pred).sum())
    precision = 0.0 if picks == 0 else tp / picks
    recall = 0.0 if positives == 0 else tp / positives
    f1 = 0.0 if precision + recall == 0 else 2.0 * precision * recall / (precision + recall)
    return {
        "threshold": float(threshold),
        "rows": int(len(frame)),
        "positives": positives,
        "picks": picks,
        "matched_picks": tp,
        "precision": float(precision),
        "recall": float(recall),
        "missed_pct": float(1.0 - recall),
        "f1": float(f1),
    }


def threshold_candidates(frame: pd.DataFrame, args: argparse.Namespace) -> list[float]:
    fixed = [float(part.strip()) for part in str(args.thresholds or "").split(",") if part.strip()]
    scores = pd.to_numeric(frame["manager_score"], errors="coerce").fillna(0.0).to_numpy()
    quantiles = [float(value) for value in np.quantile(scores, np.linspace(0.50, 0.99, 24))]
    return sorted(set(round(value, 8) for value in [*fixed, *quantiles] if 0.0 <= value <= 1.0))


def choose_threshold(sweep: list[dict[str, Any]], args: argparse.Namespace) -> float:
    allowed = [
        row
        for row in sweep
        if float(row["precision"]) >= float(args.min_threshold_precision)
        and int(row["picks"]) >= int(args.min_threshold_picks)
    ]
    if allowed:
        return float(max(allowed, key=lambda row: (float(row["f1"]), float(row["precision"]), float(row["recall"])))["threshold"])
    return float(max(sweep, key=lambda row: (float(row["f1"]), float(row["precision"]), float(row["recall"])))["threshold"])


def to_jsonable(value: Any) -> Any:
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, pd.Timestamp):
        return value.isoformat()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def readable_row(split: str, metrics: dict[str, Any]) -> dict[str, Any]:
    return {
        "split": split,
        "threshold": metrics["threshold"],
        "rows": metrics["rows"],
        "positives": metrics["positives"],
        "picks": metrics["picks"],
        "matched_picks": metrics["matched_picks"],
        "trend_accuracy_pct": round(float(metrics["precision"]) * 100.0, 2),
        "oracle_coverage_pct": round(float(metrics["recall"]) * 100.0, 2),
        "oracle_missed_pct": round(float(metrics["missed_pct"]) * 100.0, 2),
        "f1_pct": round(float(metrics["f1"]) * 100.0, 2),
    }


def write_readable(output_dir: Path, rows: list[dict[str, Any]]) -> None:
    fields = [
        "split",
        "threshold",
        "rows",
        "positives",
        "picks",
        "matched_picks",
        "trend_accuracy_pct",
        "oracle_coverage_pct",
        "oracle_missed_pct",
        "f1_pct",
    ]
    with (output_dir / "cnn_level4_readable_summary.csv").open("w", newline="", encoding="utf-8") as handle:
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
    print(f"Building shared lower CNN reports for train year={args.train_year}", flush=True)
    train_reports = score_lower_levels(args, int(args.train_year))
    print(
        f"Training CNN Level 4 manager rows={len(train_reports):,} positives={int(train_reports['is_oracle_start'].sum()):,}",
        flush=True,
    )
    model = train_manager(train_reports, args)
    train_scored = add_manager_scores(train_reports, model)
    train_sweep = [binary_metrics(train_scored, value) for value in threshold_candidates(train_scored, args)]
    selected_threshold = choose_threshold(train_sweep, args)

    print(f"Building shared lower CNN reports for validation year={args.valid_year}", flush=True)
    valid_reports = score_lower_levels(args, int(args.valid_year))
    valid_scored = add_manager_scores(valid_reports, model)
    valid_sweep = [binary_metrics(valid_scored, float(row["threshold"])) for row in train_sweep]
    selected_train = binary_metrics(train_scored, selected_threshold)
    selected_valid = binary_metrics(valid_scored, selected_threshold)

    model.save_model(str(output_dir / "catboost_cnn_level4_manager.cbm"))
    pd.DataFrame(train_sweep).to_csv(output_dir / f"cnn_level4_threshold_sweep_{args.train_year}.csv", index=False)
    pd.DataFrame(valid_sweep).to_csv(output_dir / f"cnn_level4_threshold_sweep_{args.valid_year}.csv", index=False)
    if args.save_score_csvs:
        train_scored.to_csv(output_dir / f"cnn_level4_lower_reports_scored_{args.train_year}.csv", index=False)
        valid_scored.to_csv(output_dir / f"cnn_level4_lower_reports_scored_{args.valid_year}.csv", index=False)

    readable = [readable_row(str(args.train_year), selected_train), readable_row(str(args.valid_year), selected_valid)]
    write_readable(output_dir, readable)
    metadata = {
        "run_id": rid,
        "model_type": "catboost_cnn_level4_manager",
        "root": str(args.root).upper(),
        "timeframes": parse_timeframes(args.timeframes),
        "train_year": int(args.train_year),
        "valid_year": int(args.valid_year),
        "lower_level_runs": {
            "level1_prefix": args.level1_prefix,
            "level2_run_id": args.level2_run_id,
            "level3_run_id": args.level3_run_id,
        },
        "shared_row_source": "Level 2 scored rows re-scored by Level 1 and Level 3 CNNs.",
        "training_parameters": {
            "iterations": int(args.iterations),
            "depth": int(args.depth),
            "learning_rate": float(args.learning_rate),
            "l2_leaf_reg": float(args.l2_leaf_reg),
            "random_seed": int(args.random_seed),
        },
        "threshold_policy": {
            "selected_threshold": float(selected_threshold),
            "min_threshold_precision": float(args.min_threshold_precision),
            "min_threshold_picks": int(args.min_threshold_picks),
        },
        "features": {"cat": CAT_FEATURES, "num": NUM_FEATURES},
        "selected_train_summary": selected_train,
        "selected_valid_summary": selected_valid,
        "read": "CNN Level 4 manager trained on 2025 lower-level CNN reports and tested on 2026.",
    }
    (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")
    print(f"selected_cnn_level4_threshold={selected_threshold:.6f}", flush=True)
    print(json.dumps({"train_selected": selected_train, "valid_selected": selected_valid}, indent=2, default=to_jsonable), flush=True)
    print(f"Saved CNN Level 4 manager: {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
