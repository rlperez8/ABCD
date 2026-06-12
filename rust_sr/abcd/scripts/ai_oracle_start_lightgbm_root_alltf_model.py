#!/usr/bin/env python3
"""Train a LightGBM Level 2 root/all-timeframe oracle trend-start specialist."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import numpy as np
import pandas as pd

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_oracle_start_lightgbm_utils as lgbm_utils
import ai_oracle_start_stage1_root_alltf_model as root_alltf
import ai_wave_rider_research as wave


def parse_args() -> argparse.Namespace:
    args = root_alltf.parse_args()
    if args.run_prefix == "aicw-oracle-start-root-alltf-v1":
        args.run_prefix = "aicw-oracle-start-lightgbm-root-alltf-v1"
    return args


def add_scores(frame: pd.DataFrame, model_bundle) -> pd.DataFrame:
    model, category_maps = model_bundle
    scored = frame.copy()
    scored["oracle_start_score"] = lgbm_utils.predict_scores(
        scored,
        model,
        root_alltf.CAT_FEATURES,
        root_alltf.NUM_FEATURES,
        category_maps,
    )
    return scored


def main() -> int:
    args = parse_args()
    rid = root_alltf.run_id(args)
    output_dir = wave.ABCD_ROOT / "model_registry" / rid
    if output_dir.exists() and not args.replace_run:
        raise ValueError(f"Run already exists: {rid}. Use --replace-run.")
    output_dir.mkdir(parents=True, exist_ok=True)

    rng = np.random.default_rng(int(args.random_seed))
    conn = wave.connect()
    try:
        train_parts = []
        train_summaries = {}
        for year in root_alltf.parse_years(args.train_years):
            frame, summary = root_alltf.build_year_rows(conn, int(year), args, rng, train_sample=True)
            train_parts.append(frame)
            train_summaries[str(year)] = summary
        threshold, threshold_summary = root_alltf.build_year_rows(conn, int(args.threshold_year), args, rng, train_sample=False)
        valid, valid_summary = root_alltf.build_year_rows(conn, int(args.valid_year), args, rng, train_sample=False)
    finally:
        conn.close()

    train = pd.concat(train_parts, ignore_index=True).sample(frac=1.0, random_state=int(args.random_seed)).reset_index(drop=True)
    print(f"Training LightGBM Level 2 rows={len(train):,} positives={int(train['is_oracle_start'].sum()):,}", flush=True)
    model, category_maps = lgbm_utils.train_binary(train, args, root_alltf.CAT_FEATURES, root_alltf.NUM_FEATURES)
    threshold_scored = add_scores(threshold, (model, category_maps))
    valid_scored = add_scores(valid, (model, category_maps))

    threshold_sweep = [root_alltf.event_summary(threshold_scored, value, args, threshold_summary) for value in root_alltf.threshold_candidates(args, threshold_scored)]
    selected_threshold = root_alltf.choose_threshold(threshold_sweep, args)
    valid_sweep = [root_alltf.event_summary(valid_scored, float(row["threshold"]), args, valid_summary) for row in threshold_sweep]
    selected_valid = root_alltf.event_summary(valid_scored, selected_threshold, args, valid_summary)

    print(f"selected_lightgbm_root_alltf_stage1_threshold={selected_threshold:.6f}", flush=True)
    print(json.dumps({"threshold_year": threshold_sweep, "valid_selected": selected_valid}, indent=2, default=root_alltf.to_jsonable), flush=True)

    model.save_model(str(output_dir / "lightgbm_model.txt"))
    lgbm_utils.save_feature_state(output_dir / "lightgbm_feature_state.json", root_alltf.CAT_FEATURES, root_alltf.NUM_FEATURES, category_maps)
    pd.DataFrame(threshold_sweep).to_csv(output_dir / f"threshold_sweep_{args.threshold_year}.csv", index=False)
    pd.DataFrame(valid_sweep).to_csv(output_dir / f"threshold_sweep_{args.valid_year}.csv", index=False)
    if args.save_row_csvs:
        train.to_csv(output_dir / "root_alltf_rows_train_sample.csv", index=False)
        threshold_scored.to_csv(output_dir / f"root_alltf_rows_{args.threshold_year}.csv", index=False)
        valid_scored.to_csv(output_dir / f"root_alltf_rows_{args.valid_year}.csv", index=False)

    metadata = {
        "run_id": rid,
        "model_type": "lightgbm_oracle_start_root_alltf_stage1",
        "oracle_run_id_template": args.oracle_run_id_template,
        "root": str(args.root).upper(),
        "timeframes": root_alltf.parse_list(args.timeframes),
        "train_years": root_alltf.parse_years(args.train_years),
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
        "read": "LightGBM Level 2 specialist trained on one root across selected timeframes.",
    }
    (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=root_alltf.to_jsonable), encoding="utf-8")
    print(f"Saved LightGBM Level 2 model: {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
