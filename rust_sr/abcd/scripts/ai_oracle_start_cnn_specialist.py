#!/usr/bin/env python3
"""Train visual CNN oracle-start specialists for Level 1/2/3 experiments.

Level 1 uses one root/timeframe. Level 2 can combine one root across many
timeframes. Level 3 can combine many roots across many timeframes. The model
input is still only the leak-safe rendered chart image ending at the signal
candle; oracle labels are used only as the historical answer key.
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


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_oracle_trend_start_model as start_model
import ai_oracle_trend_start_visual_cnn as visual_cnn
import ai_wave_rider_research as wave


DEFAULT_TIMEFRAMES = "1m,2m,3m,4m,5m,6m,7m,8m,9m,10m,11m,12m,13m,14m,15m,30m"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--level", choices=["l1", "l2", "l3"], required=True)
    parser.add_argument("--roots", default="NQ", help="Comma-separated roots. Empty means all roots.")
    parser.add_argument("--timeframes", default="2m")
    parser.add_argument("--oracle-run-id-template", default="oracle-perfect-trends-tfspec-v1-{timeframe}-2024_2026")
    parser.add_argument("--run-prefix", default="aicw-oracle-start-cnn-specialist-v1")
    parser.add_argument("--train-years", default="2024")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--lookback-bars", type=int, default=64)
    parser.add_argument("--height", type=int, default=40)
    parser.add_argument("--width", type=int, default=40)
    parser.add_argument("--min-candles", type=int, default=36)
    parser.add_argument("--positive-lag-bars", type=int, default=2)
    parser.add_argument("--negative-exclusion-bars", type=int, default=8)
    parser.add_argument("--negative-ratio", type=float, default=2.0)
    parser.add_argument("--base-negatives-per-symbol", type=int, default=12)
    parser.add_argument("--match-window-bars", type=int, default=4)
    parser.add_argument("--max-train-rows-per-tf", type=int, default=12000)
    parser.add_argument("--max-eval-rows-per-tf", type=int, default=12000)
    parser.add_argument("--max-combined-train-rows", type=int, default=0)
    parser.add_argument("--max-combined-eval-rows", type=int, default=0)
    parser.add_argument("--epochs", type=int, default=10)
    parser.add_argument("--batch-size", type=int, default=192)
    parser.add_argument("--learning-rate", type=float, default=0.001)
    parser.add_argument("--weight-decay", type=float, default=0.01)
    parser.add_argument("--dropout", type=float, default=0.20)
    parser.add_argument("--patience", type=int, default=4)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--min-threshold-precision", type=float, default=0.45)
    parser.add_argument("--min-threshold-picks", type=int, default=200)
    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=240.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    parser.add_argument("--replace-run", action="store_true")
    return parser.parse_args()


def parse_timeframes(raw: str) -> list[str]:
    values = [part.strip().lower() for part in str(raw or "").split(",") if part.strip()]
    if not values:
        raise ValueError("At least one timeframe is required.")
    return values


def root_label(raw: str) -> str:
    roots = start_model.parse_list(raw)
    if not roots:
        return "ALL"
    if len(roots) <= 5:
        return "_".join(roots)
    digest = hashlib.sha1(",".join(roots).encode("utf-8")).hexdigest()[:8]
    return f"{len(roots)}roots-{digest}"


def timeframe_label(timeframes: list[str]) -> str:
    if len(timeframes) == 1:
        return timeframes[0]
    digest = hashlib.sha1(",".join(timeframes).encode("utf-8")).hexdigest()[:8]
    return f"alltf-{digest}"


def run_id(args: argparse.Namespace) -> str:
    train = "_".join(str(year) for year in start_model.parse_years(args.train_years))
    raw = (
        f"{args.run_prefix}-{args.level}-{root_label(args.roots)}-"
        f"{timeframe_label(parse_timeframes(args.timeframes))}-tr{train}-v{int(args.valid_year)}"
    )
    if len(raw) <= 96:
        return raw
    digest = hashlib.sha1(raw.encode("utf-8")).hexdigest()[:10]
    return f"{args.run_prefix}-{args.level}-{digest}-v{int(args.valid_year)}"


def child_args(args: argparse.Namespace, timeframe: str, train_split: bool) -> argparse.Namespace:
    limit = int(args.max_train_rows_per_tf if train_split else args.max_eval_rows_per_tf)
    return argparse.Namespace(
        oracle_run_id=str(args.oracle_run_id_template).format(timeframe=timeframe),
        catboost_run_id="",
        run_prefix=args.run_prefix,
        timeframe=timeframe,
        train_years=args.train_years,
        threshold_year=int(args.threshold_year),
        valid_year=int(args.valid_year),
        roots=args.roots,
        lookback_bars=int(args.lookback_bars),
        height=int(args.height),
        width=int(args.width),
        min_candles=int(args.min_candles),
        positive_lag_bars=int(args.positive_lag_bars),
        negative_exclusion_bars=int(args.negative_exclusion_bars),
        negative_ratio=float(args.negative_ratio),
        base_negatives_per_symbol=int(args.base_negatives_per_symbol),
        match_window_bars=int(args.match_window_bars),
        max_train_rows=limit,
        max_eval_rows=limit,
        epochs=int(args.epochs),
        batch_size=int(args.batch_size),
        learning_rate=float(args.learning_rate),
        weight_decay=float(args.weight_decay),
        dropout=float(args.dropout),
        patience=int(args.patience),
        random_seed=int(args.random_seed),
        catboost_threshold=-1.0,
        min_threshold_precision=float(args.min_threshold_precision),
        min_threshold_picks=int(args.min_threshold_picks),
        entry_breakout_bars=int(args.entry_breakout_bars),
        trail_lookback_bars=int(args.trail_lookback_bars),
        atr_period=int(args.atr_period),
        atr_stop_pad=float(args.atr_stop_pad),
        min_risk_ticks=float(args.min_risk_ticks),
        max_risk_ticks=float(args.max_risk_ticks),
        min_relative_volume=float(args.min_relative_volume),
        replace_run=bool(args.replace_run),
    )


def build_split(
    conn,
    year: int,
    args: argparse.Namespace,
    rng: np.random.Generator,
    label: str,
    train_split: bool,
) -> tuple[np.ndarray, pd.DataFrame]:
    images: list[np.ndarray] = []
    frames: list[pd.DataFrame] = []
    for timeframe in parse_timeframes(args.timeframes):
        tf_args = child_args(args, timeframe, train_split=train_split)
        print(f"{label}: build {timeframe} oracle={tf_args.oracle_run_id}", flush=True)
        x, frame = visual_cnn.build_split(conn, int(year), tf_args, rng, f"{label}-{timeframe}")
        if len(frame):
            frame = frame.copy()
            frame["source_timeframe"] = timeframe
            frame["cnn_level"] = str(args.level)
            frame["candidate_uid"] = frame["candidate_uid"].map(lambda value: f"{timeframe}|{value}")
            images.append(x)
            frames.append(frame)
    if not images:
        raise ValueError(f"No CNN image rows built for {label}")
    combined_x = np.concatenate(images, axis=0)
    combined_frame = pd.concat(frames, ignore_index=True)
    limit = int(args.max_combined_train_rows if train_split else args.max_combined_eval_rows)
    if limit > 0 and len(combined_frame) > limit:
        positives = combined_frame[combined_frame["is_oracle_start"] == 1]
        negatives = combined_frame[combined_frame["is_oracle_start"] == 0]
        keep_pos_n = min(len(positives), limit // 2)
        keep_neg_n = min(len(negatives), limit - keep_pos_n)
        keep_idx = []
        if keep_pos_n:
            keep_idx.extend(positives.sample(n=keep_pos_n, random_state=int(args.random_seed)).index.tolist())
        if keep_neg_n:
            keep_idx.extend(negatives.sample(n=keep_neg_n, random_state=int(args.random_seed)).index.tolist())
        rng.shuffle(keep_idx)
        combined_x = combined_x[keep_idx]
        combined_frame = combined_frame.loc[keep_idx].reset_index(drop=True)
    print(
        f"{label}: combined rows={len(combined_frame):,} positives={int(combined_frame['is_oracle_start'].sum()):,}",
        flush=True,
    )
    return combined_x, combined_frame


def score_frame(frame: pd.DataFrame, scores: np.ndarray) -> pd.DataFrame:
    out = frame.copy()
    out["visual_score"] = scores.astype(float)
    return out


def to_jsonable(value: Any) -> Any:
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, pd.Timestamp):
        return value.isoformat()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def readable_metrics(metrics: dict[str, Any], split: str) -> dict[str, Any]:
    return {
        "split": split,
        "rows": metrics["rows"],
        "positives": metrics["positives"],
        "picks": metrics["picks"],
        "precision_pct": round(float(metrics["precision"]) * 100.0, 2),
        "recall_pct": round(float(metrics["recall"]) * 100.0, 2),
        "missed_pct": round((1.0 - float(metrics["recall"])) * 100.0, 2),
        "f1_pct": round(float(metrics["f1"]) * 100.0, 2),
        "auc": metrics.get("auc"),
        "average_precision": metrics.get("average_precision"),
        "threshold": metrics["threshold"],
    }


def write_readable(model_dir: Path, rows: list[dict[str, Any]]) -> None:
    fields = [
        "split",
        "rows",
        "positives",
        "picks",
        "precision_pct",
        "recall_pct",
        "missed_pct",
        "f1_pct",
        "auc",
        "average_precision",
        "threshold",
    ]
    with (model_dir / "cnn_readable_summary.csv").open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)


def main() -> int:
    args = parse_args()
    visual_cnn.set_seed(int(args.random_seed))
    rid = run_id(args)
    model_dir = wave.ABCD_ROOT / "model_registry" / rid
    if model_dir.exists() and not args.replace_run:
        raise ValueError(f"CNN specialist run already exists: {rid}. Use --replace-run.")
    model_dir.mkdir(parents=True, exist_ok=True)

    rng = np.random.default_rng(int(args.random_seed))
    train_years = start_model.parse_years(args.train_years)
    conn = wave.connect()
    try:
        train_images: list[np.ndarray] = []
        train_frames: list[pd.DataFrame] = []
        for year in train_years:
            x, frame = build_split(conn, int(year), args, rng, f"train-{year}", train_split=True)
            train_images.append(x)
            train_frames.append(frame)
        train_x = np.concatenate(train_images, axis=0) if len(train_images) > 1 else train_images[0]
        train_frame = pd.concat(train_frames, ignore_index=True) if len(train_frames) > 1 else train_frames[0]
        threshold_x, threshold_frame = build_split(conn, int(args.threshold_year), args, rng, "threshold", train_split=False)
        valid_x, valid_frame = build_split(conn, int(args.valid_year), args, rng, "valid", train_split=False)
    finally:
        conn.close()

    train_y = train_frame["is_oracle_start"].astype(int).to_numpy()
    threshold_y = threshold_frame["is_oracle_start"].astype(int).to_numpy()
    valid_y = valid_frame["is_oracle_start"].astype(int).to_numpy()
    print(
        f"Training CNN specialist run={rid} train={len(train_frame):,} threshold={len(threshold_frame):,} "
        f"valid={len(valid_frame):,} train_positive={train_y.mean():.3f}",
        flush=True,
    )
    model, history = visual_cnn.train_cnn(train_x, train_y, threshold_x, threshold_y, args)
    train_scores = visual_cnn.evaluate_scores(model, train_x, args)
    threshold_scores = visual_cnn.evaluate_scores(model, threshold_x, args)
    valid_scores = visual_cnn.evaluate_scores(model, valid_x, args)

    train_scored = score_frame(train_frame, train_scores)
    threshold_scored = score_frame(threshold_frame, threshold_scores)
    valid_scored = score_frame(valid_frame, valid_scores)
    visual_threshold, threshold_sweep = visual_cnn.choose_threshold(threshold_scored, "visual_score", args)

    metrics = {
        "train": visual_cnn.binary_metrics(train_y, train_scores, visual_threshold),
        str(args.threshold_year): visual_cnn.binary_metrics(threshold_y, threshold_scores, visual_threshold),
        str(args.valid_year): visual_cnn.binary_metrics(valid_y, valid_scores, visual_threshold),
        "valid_closeness": start_model.pick_closeness(valid_scored.rename(columns={"visual_score": "oracle_start_score"}), visual_threshold, args),
    }
    for key, value in metrics.items():
        print(f"{key}={json.dumps(value, default=to_jsonable)}", flush=True)

    torch.save(model.state_dict(), model_dir / "visual_oracle_start_cnn.pt")
    train_scored.to_csv(model_dir / "visual_oracle_start_scores_train.csv", index=False)
    threshold_scored.to_csv(model_dir / f"visual_oracle_start_scores_{args.threshold_year}.csv", index=False)
    valid_scored.to_csv(model_dir / f"visual_oracle_start_scores_{args.valid_year}.csv", index=False)
    threshold_sweep.to_csv(model_dir / f"visual_oracle_start_threshold_sweep_{args.threshold_year}.csv", index=False)
    readable = [
        readable_metrics(metrics["train"], "train"),
        readable_metrics(metrics[str(args.threshold_year)], str(args.threshold_year)),
        readable_metrics(metrics[str(args.valid_year)], str(args.valid_year)),
    ]
    write_readable(model_dir, readable)
    metadata = {
        "visual_model_run_id": rid,
        "model_type": "pytorch_cnn_oracle_start_specialist",
        "cnn_level": args.level,
        "roots": start_model.parse_list(args.roots),
        "root_scope": root_label(args.roots),
        "timeframes": parse_timeframes(args.timeframes),
        "oracle_run_id_template": args.oracle_run_id_template,
        "train_years": train_years,
        "threshold_year": int(args.threshold_year),
        "valid_year": int(args.valid_year),
        "lookback_bars": int(args.lookback_bars),
        "height": int(args.height),
        "width": int(args.width),
        "positive_lag_bars": int(args.positive_lag_bars),
        "negative_exclusion_bars": int(args.negative_exclusion_bars),
        "max_train_rows_per_tf": int(args.max_train_rows_per_tf),
        "max_eval_rows_per_tf": int(args.max_eval_rows_per_tf),
        "max_combined_train_rows": int(args.max_combined_train_rows),
        "max_combined_eval_rows": int(args.max_combined_eval_rows),
        "selected_visual_threshold": float(visual_threshold),
        "training_history": history,
        "metrics": metrics,
        "read": "CNN specialist over rendered chart images ending at the signal candle; oracle labels are offline-only.",
    }
    (model_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")
    print(f"Saved CNN specialist: {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
