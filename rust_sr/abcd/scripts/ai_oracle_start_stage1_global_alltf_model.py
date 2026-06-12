#!/usr/bin/env python3
"""Train a Level 3 global all-root all-timeframe oracle trend-start specialist."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_scanner_model as scanner
import ai_oracle_start_stage1_live_grid_model as stage1
import ai_oracle_start_stage1_root_alltf_model as root_alltf
import ai_wave_rider_research as wave


DEFAULT_TIMEFRAMES = root_alltf.DEFAULT_TIMEFRAMES


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--roots", required=True, help="Comma-separated roots for the global specialist.")
    parser.add_argument("--timeframes", default=DEFAULT_TIMEFRAMES)
    parser.add_argument("--train-years", default="2024")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--run-prefix", default="aicw-oracle-start-global-alltf-v1")
    parser.add_argument(
        "--oracle-run-id-template",
        default="oracle-perfect-trends-tfspec-v1-{timeframe}-2024_2026",
    )
    parser.add_argument("--iterations", type=int, default=650)
    parser.add_argument("--depth", type=int, default=8)
    parser.add_argument("--learning-rate", type=float, default=0.045)
    parser.add_argument("--l2-leaf-reg", type=float, default=8.0)
    parser.add_argument("--positive-pre-bars", type=int, default=0)
    parser.add_argument("--positive-post-bars", type=int, default=2)
    parser.add_argument("--negative-exclusion-bars", type=int, default=8)
    parser.add_argument("--match-window-bars", type=int, default=4)
    parser.add_argument("--negative-ratio", type=float, default=8.0)
    parser.add_argument("--base-negatives-per-symbol", type=int, default=200)
    parser.add_argument("--max-train-rows", type=int, default=750_000)
    parser.add_argument("--cooldown-bars", type=int, default=6)
    parser.add_argument("--min-oracle-recall", type=float, default=0.90)
    parser.add_argument("--max-picks-per-oracle", type=float, default=5.0)
    parser.add_argument("--thresholds", default="0.25,0.30,0.35,0.40,0.45,0.50,0.55,0.60,0.65,0.70,0.75,0.80,0.85,0.90")
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--limit-symbols-per-root", type=int, default=0)
    parser.add_argument("--save-row-csvs", action="store_true")
    parser.add_argument("--replace-run", action="store_true")

    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=240.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    return parser.parse_args()


def parse_roots(raw: str) -> list[str]:
    roots = [part.strip().upper() for part in str(raw or "").split(",") if part.strip()]
    if not roots:
        raise ValueError("At least one root is required.")
    return roots


def parse_timeframes(raw: str) -> list[str]:
    timeframes = [part.strip().lower() for part in str(raw or "").split(",") if part.strip()]
    if not timeframes:
        raise ValueError("At least one timeframe is required.")
    return timeframes


def parse_years(raw: str) -> list[int]:
    years = [int(part) for part in stage1.parse_list(raw)]
    if not years:
        raise ValueError("At least one train year is required.")
    return years


def run_id(args: argparse.Namespace) -> str:
    train = "_".join(str(year) for year in parse_years(args.train_years))
    roots = "_".join(parse_roots(args.roots))
    raw = f"{args.run_prefix}-{roots}-alltf-tr{train}-v{int(args.valid_year)}"
    if len(raw) <= 120:
        return raw
    digest = hashlib.sha1(raw.encode("utf-8")).hexdigest()[:12]
    return f"{args.run_prefix}-{digest}-alltf-tr{train}-v{int(args.valid_year)}"


def child_args(args: argparse.Namespace, root: str, timeframe: str) -> argparse.Namespace:
    return argparse.Namespace(
        roots=str(root).upper(),
        symbols="",
        timeframe=timeframe,
        oracle_run_id=str(args.oracle_run_id_template).format(timeframe=timeframe),
        positive_pre_bars=int(args.positive_pre_bars),
        positive_post_bars=int(args.positive_post_bars),
        negative_exclusion_bars=int(args.negative_exclusion_bars),
        match_window_bars=int(args.match_window_bars),
        negative_ratio=float(args.negative_ratio),
        base_negatives_per_symbol=int(args.base_negatives_per_symbol),
        max_train_rows=0,
        random_seed=int(args.random_seed),
        limit_symbols=int(args.limit_symbols_per_root),
        entry_breakout_bars=int(args.entry_breakout_bars),
        trail_lookback_bars=int(args.trail_lookback_bars),
        atr_period=int(args.atr_period),
        atr_stop_pad=float(args.atr_stop_pad),
        min_risk_ticks=float(args.min_risk_ticks),
        max_risk_ticks=float(args.max_risk_ticks),
        min_relative_volume=float(args.min_relative_volume),
    )


def add_global_features(frame: pd.DataFrame, root: str, timeframe: str) -> pd.DataFrame:
    _, minutes = scanner.table_for_timeframe(timeframe)
    work = frame.copy()
    work["source_timeframe"] = str(timeframe)
    work["timeframe_minutes"] = float(minutes)
    work["candidate_uid"] = work["candidate_uid"].map(lambda value: f"{root}|{timeframe}|{value}")
    return work


def cap_training_rows(frame: pd.DataFrame, args: argparse.Namespace, year: int) -> pd.DataFrame:
    max_rows = int(args.max_train_rows)
    if max_rows <= 0 or len(frame) <= max_rows:
        return frame
    positives = frame[frame["is_oracle_start"] == 1]
    negatives = frame[frame["is_oracle_start"] == 0]
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
    return pd.concat([keep_pos, keep_neg], ignore_index=True).sample(
        frac=1.0,
        random_state=int(args.random_seed) + int(year),
    ).reset_index(drop=True)


def build_year_rows(
    conn,
    year: int,
    args: argparse.Namespace,
    rng: np.random.Generator,
    train_sample: bool,
) -> tuple[pd.DataFrame, dict[str, Any]]:
    frames: list[pd.DataFrame] = []
    summaries: dict[str, dict[str, Any]] = {}
    for root in parse_roots(args.roots):
        summaries[root] = {}
        for timeframe in parse_timeframes(args.timeframes):
            print(f"{year}: global-alltf {root} {timeframe} build start", flush=True)
            frame, summary = stage1.build_year_rows(
                conn,
                int(year),
                child_args(args, root, timeframe),
                rng,
                train_sample=train_sample,
            )
            frames.append(add_global_features(frame, root, timeframe))
            summaries[root][timeframe] = summary

    combined = pd.concat(frames, ignore_index=True)
    if train_sample:
        combined = combined.sample(frac=1.0, random_state=int(args.random_seed) + int(year)).reset_index(drop=True)
        combined = cap_training_rows(combined, args, int(year))

    flat_summaries = [summary for root_summary in summaries.values() for summary in root_summary.values()]
    summary = {
        "year": int(year),
        "roots": parse_roots(args.roots),
        "timeframes": parse_timeframes(args.timeframes),
        "oracle_starts": int(sum(int(item.get("oracle_starts", 0) or 0) for item in flat_summaries)),
        "oracle_starts_available": int(sum(int(item.get("oracle_starts_available", 0) or 0) for item in flat_summaries)),
        "eligible_candidates": int(sum(int(item.get("eligible_candidates", 0) or 0) for item in flat_summaries)),
        "positive_rows": int(sum(int(item.get("positive_rows", 0) or 0) for item in flat_summaries)),
        "materialized_rows": int(len(combined)),
        "materialized_positives": int(pd.to_numeric(combined["is_oracle_start"], errors="coerce").fillna(0).sum()),
        "train_sample": bool(train_sample),
        "root_timeframe_summaries": summaries,
    }
    printable = {key: value for key, value in summary.items() if key != "root_timeframe_summaries"}
    print(f"{year}: global-alltf {json.dumps(printable, default=root_alltf.to_jsonable)}", flush=True)
    return combined, summary


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
        train_parts: list[pd.DataFrame] = []
        train_summaries: dict[str, Any] = {}
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
        f"Training global-alltf Stage 1 model rows={len(train):,} positives={int(train['is_oracle_start'].sum()):,}",
        flush=True,
    )
    model = root_alltf.train_model(train, args)
    threshold_scored = root_alltf.add_scores(threshold, model)
    valid_scored = root_alltf.add_scores(valid, model)
    threshold_sweep = [
        root_alltf.event_summary(threshold_scored, value, args, threshold_summary)
        for value in root_alltf.threshold_candidates(args, threshold_scored)
    ]
    selected_threshold = root_alltf.choose_threshold(threshold_sweep, args)
    valid_sweep = [
        root_alltf.event_summary(valid_scored, float(row["threshold"]), args, valid_summary)
        for row in threshold_sweep
    ]
    selected_valid = root_alltf.event_summary(valid_scored, selected_threshold, args, valid_summary)

    print(f"selected_global_alltf_stage1_threshold={selected_threshold:.6f}", flush=True)
    print(json.dumps({"threshold_year": threshold_sweep, "valid_selected": selected_valid}, indent=2, default=root_alltf.to_jsonable), flush=True)

    model.save_model(str(output_dir / "catboost_oracle_start_global_alltf_model.cbm"))
    pd.DataFrame(threshold_sweep).to_csv(output_dir / f"threshold_sweep_{args.threshold_year}.csv", index=False)
    pd.DataFrame(valid_sweep).to_csv(output_dir / f"threshold_sweep_{args.valid_year}.csv", index=False)
    if args.save_row_csvs:
        train.to_csv(output_dir / "global_alltf_rows_train_sample.csv", index=False)
        threshold_scored.to_csv(output_dir / f"global_alltf_rows_{args.threshold_year}.csv", index=False)
        valid_scored.to_csv(output_dir / f"global_alltf_rows_{args.valid_year}.csv", index=False)

    metadata = {
        "run_id": rid,
        "model_type": "catboost_oracle_start_global_alltf_stage1",
        "oracle_run_id_template": args.oracle_run_id_template,
        "roots": parse_roots(args.roots),
        "timeframes": parse_timeframes(args.timeframes),
        "train_years": parse_years(args.train_years),
        "threshold_year": int(args.threshold_year),
        "valid_year": int(args.valid_year),
        "limit_symbols_per_root": int(args.limit_symbols_per_root),
        "label_parameters": {
            "positive_pre_bars": int(args.positive_pre_bars),
            "positive_post_bars": int(args.positive_post_bars),
            "negative_exclusion_bars": int(args.negative_exclusion_bars),
            "match_window_bars": int(args.match_window_bars),
        },
        "training_parameters": {
            "negative_ratio": float(args.negative_ratio),
            "base_negatives_per_symbol": int(args.base_negatives_per_symbol),
            "max_train_rows": int(args.max_train_rows),
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
        "features": {"cat": root_alltf.CAT_FEATURES, "num": root_alltf.NUM_FEATURES},
        "source_summaries": {
            "train": train_summaries,
            str(args.threshold_year): threshold_summary,
            str(args.valid_year): valid_summary,
        },
        "selected_valid_summary": selected_valid,
        "read": "Level 3 Stage 1 specialist trained across selected roots and selected timeframes.",
    }
    (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=root_alltf.to_jsonable), encoding="utf-8")
    print(f"Saved global-alltf Stage 1 model: {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
