#!/usr/bin/env python3
"""
Export Stage 1 top-N candidate result IDs for the AI sampled train/validation sets.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import pandas as pd

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_build_ranking_eval as ranking
import ai_build_reranker_eval as reranker
import ai_build_search_catboost as base


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-run-id", default="eetc-1779837772298-38740")
    parser.add_argument("--results-table", default="entry_exit_template_results_eetc_1779837772298_38740")
    parser.add_argument("--train-start-year", type=int, default=2021)
    parser.add_argument("--train-end-year", type=int, default=2024)
    parser.add_argument("--valid-year", type=int, default=2025)
    parser.add_argument("--train-setups-per-year", type=int, default=2200)
    parser.add_argument("--valid-setups", type=int, default=3200)
    parser.add_argument("--setup-sample-mod", type=int, default=7)
    parser.add_argument("--train-sample-slot", type=int, default=0)
    parser.add_argument("--valid-sample-slot", type=int, default=3)
    parser.add_argument("--pre-feature-set", choices=["all", "core", "shape", "candidate", "none"], default="none")
    parser.add_argument("--aggregate-feature-set", choices=["basic", "all"], default="all")
    parser.add_argument("--skip-aggregate-features", action="store_true")
    parser.add_argument("--stage1-iterations", type=int, default=160)
    parser.add_argument("--stage1-depth", type=int, default=6)
    parser.add_argument("--stage1-learning-rate", type=float, default=0.035)
    parser.add_argument("--l2-leaf-reg", type=float, default=3.0)
    parser.add_argument("--random-strength", type=float, default=1.0)
    parser.add_argument("--random-seed", type=int, default=42)
    parser.add_argument("--rerank-top-n", type=int, default=10)
    parser.add_argument("--output", default=None)
    return parser.parse_args()


def ranked_shortlist(df: pd.DataFrame, predictions, split: str, top_n: int) -> pd.DataFrame:
    ranked = reranker.build_ranked_with_index(df, predictions)
    shortlist = ranked.loc[ranked["model_rank"] <= top_n].copy()
    return shortlist.loc[
        :,
        [
            "id",
            "setup_id",
            "template_uid",
            "symbol",
            "d_confirm_date",
            "model_rank",
            "predicted_expected_r",
            "result_r",
        ],
    ].rename(columns={"id": "result_id"}).assign(split=split)


def main() -> int:
    args = parse_args()
    args.results_table = base.safe_identifier(args.results_table)
    output = args.output
    if not output:
        output = str(base.ABCD_ROOT / "tmp_logs" / f"stage1_top{args.rerank_top_n}_result_ids.csv")

    conn = base.connect()
    try:
        reference_template = base.first_template_uid(conn, args.source_run_id)
        train_years = list(range(args.train_start_year, args.train_end_year + 1))
        train_setup_ids = base.load_setup_ids_for_years(
            conn,
            args.results_table,
            args.source_run_id,
            train_years,
            args.train_setups_per_year * len(train_years),
            args.train_setups_per_year,
            args.setup_sample_mod,
            args.train_sample_slot,
            reference_template,
        )
        valid_setup_ids = base.load_setup_ids(
            conn,
            args.results_table,
            args.source_run_id,
            args.valid_year,
            args.valid_setups,
            args.setup_sample_mod,
            args.valid_sample_slot,
            reference_template,
        )

        print(f"Train setups: {len(train_setup_ids):,}")
        print(f"Valid setups: {len(valid_setup_ids):,}")
        train_df = base.load_candidate_rows(conn, args.results_table, args.source_run_id, train_setup_ids)
        valid_df = base.load_candidate_rows(conn, args.results_table, args.source_run_id, valid_setup_ids)
        print(f"Train rows: {len(train_df):,}")
        print(f"Valid rows: {len(valid_df):,}")

        x_train, _, train_result_r = base.prepare_features(train_df)
        x_valid, _, _ = base.prepare_features(valid_df)
        x_train = ranking.apply_pre_feature_set(x_train, args.pre_feature_set)
        x_valid = ranking.apply_pre_feature_set(x_valid, args.pre_feature_set)
        if not args.skip_aggregate_features:
            x_train, x_valid = ranking.add_train_aggregate_features(
                x_train,
                train_result_r,
                x_valid,
                args.aggregate_feature_set,
            )

        stage1_model, train_predictions, valid_predictions = reranker.train_stage1(
            args,
            x_train,
            train_result_r,
            x_valid,
        )
        del stage1_model

        exported = pd.concat(
            [
                ranked_shortlist(train_df, train_predictions, "train", args.rerank_top_n),
                ranked_shortlist(valid_df, valid_predictions, "valid", args.rerank_top_n),
            ],
            ignore_index=True,
        )
        exported = exported.loc[:, ["split", "result_id", "setup_id", "template_uid", "symbol", "d_confirm_date", "model_rank", "predicted_expected_r", "result_r"]]
        Path(output).parent.mkdir(parents=True, exist_ok=True)
        exported.to_csv(output, index=False)
        print(f"Exported {len(exported):,} Stage 1 top-{args.rerank_top_n} result IDs")
        print(f"Output: {output}")
        return 0
    finally:
        conn.close()


if __name__ == "__main__":
    sys.exit(main())
