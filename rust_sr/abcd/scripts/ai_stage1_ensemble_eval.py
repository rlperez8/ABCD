#!/usr/bin/env python3
"""
Evaluate Stage 1 expected-R ensembles across CatBoost random seeds.
"""

from __future__ import annotations

import argparse
import os
import sys
import time
from pathlib import Path

import numpy as np
import pandas as pd
from catboost import CatBoostRegressor, Pool

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_build_ranking_eval as ranking
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
    parser.add_argument("--iterations", type=int, default=160)
    parser.add_argument("--depth", type=int, default=6)
    parser.add_argument("--learning-rate", type=float, default=0.035)
    parser.add_argument("--l2-leaf-reg", type=float, default=3.0)
    parser.add_argument("--random-strength", type=float, default=1.0)
    parser.add_argument("--seeds", default="42,7,123")
    parser.add_argument("--run-id", default=None)
    return parser.parse_args()


def make_run_id() -> str:
    return f"aise-{int(time.time() * 1000)}-{os.getpid()}"


def ensure_tables(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_stage1_ensemble_eval_runs (
                ensemble_eval_run_id VARCHAR(64) PRIMARY KEY,
                source_run_id VARCHAR(64) NOT NULL,
                results_table VARCHAR(128) NOT NULL,
                train_start_year INT NOT NULL,
                train_end_year INT NOT NULL,
                valid_year INT NOT NULL,
                train_setups BIGINT NOT NULL,
                valid_setups BIGINT NOT NULL,
                train_rows BIGINT NOT NULL,
                valid_rows BIGINT NOT NULL,
                seeds VARCHAR(255) NOT NULL,
                iterations INT NOT NULL,
                depth INT NOT NULL,
                learning_rate DOUBLE NOT NULL,
                pre_feature_set VARCHAR(32) NOT NULL,
                aggregate_feature_set VARCHAR(32) NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            """
        )
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_stage1_ensemble_eval_summary (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                ensemble_eval_run_id VARCHAR(64) NOT NULL,
                metric_name VARCHAR(64) NOT NULL,
                setup_count BIGINT NOT NULL,
                wins BIGINT NOT NULL,
                losses BIGINT NOT NULL,
                win_rate DOUBLE NULL,
                avg_r DOUBLE NULL,
                sum_r DOUBLE NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_aise_summary_run (ensemble_eval_run_id, metric_name)
            )
            """
        )
    conn.commit()


def train_predict(seed: int, args: argparse.Namespace, x_train: pd.DataFrame, train_result_r: pd.Series, x_valid: pd.DataFrame) -> np.ndarray:
    cat_indexes = [x_train.columns.get_loc(col) for col in base.CAT_FEATURES if col in x_train.columns]
    train_pool = Pool(x_train, label=train_result_r, cat_features=cat_indexes)
    valid_pool = Pool(x_valid, cat_features=cat_indexes)
    model = CatBoostRegressor(
        iterations=args.iterations,
        depth=args.depth,
        learning_rate=args.learning_rate,
        loss_function="RMSE",
        eval_metric="RMSE",
        random_seed=seed,
        l2_leaf_reg=args.l2_leaf_reg,
        random_strength=args.random_strength,
        verbose=100,
    )
    model.fit(train_pool, use_best_model=False)
    return model.predict(valid_pool)


def insert_summary(conn, run_id: str, metric_name: str, stats: dict) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            INSERT INTO ai_stage1_ensemble_eval_summary (
                ensemble_eval_run_id, metric_name, setup_count,
                wins, losses, win_rate, avg_r, sum_r
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s)
            """,
            (
                run_id,
                metric_name,
                stats["setup_count"],
                stats["wins"],
                stats["losses"],
                stats["win_rate"],
                stats["avg_r"],
                stats["sum_r"],
            ),
        )


def main() -> int:
    args = parse_args()
    args.results_table = base.safe_identifier(args.results_table)
    seeds = [int(part.strip()) for part in args.seeds.split(",") if part.strip()]
    run_id = args.run_id or make_run_id()

    conn = base.connect()
    try:
        ensure_tables(conn)
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
        print(f"AI Stage 1 ensemble eval: {run_id}")
        print(f"Seeds: {seeds}")
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

        prediction_columns = []
        single_stats = []
        for seed in seeds:
            print(f"\nTraining seed {seed}")
            predictions = train_predict(seed, args, x_train, train_result_r, x_valid)
            prediction_columns.append(predictions)
            ranked = ranking.build_ranked_frame(valid_df.assign(predicted_expected_r=predictions))
            top_one = ranked[ranked["model_rank"] == 1]
            stats = ranking.summarize(top_one["result_r"])
            single_stats.append((seed, stats))
            print(
                f"  seed {seed}: {stats['setup_count']} trades, {stats['win_rate']:.2%} win, "
                f"{stats['avg_r']:.3f}R avg, {stats['sum_r']:.1f}R sum"
            )

        mean_predictions = np.mean(np.vstack(prediction_columns), axis=0)
        median_predictions = np.median(np.vstack(prediction_columns), axis=0)
        mean_ranked = ranking.build_ranked_frame(valid_df.assign(predicted_expected_r=mean_predictions))
        median_ranked = ranking.build_ranked_frame(valid_df.assign(predicted_expected_r=median_predictions))
        mean_top_one = mean_ranked[mean_ranked["model_rank"] == 1]
        median_top_one = median_ranked[median_ranked["model_rank"] == 1]
        mean_stats = ranking.summarize(mean_top_one["result_r"])
        median_stats = ranking.summarize(median_top_one["result_r"])

        with conn.cursor() as cur:
            cur.execute(
                """
                INSERT INTO ai_stage1_ensemble_eval_runs (
                    ensemble_eval_run_id, source_run_id, results_table,
                    train_start_year, train_end_year, valid_year,
                    train_setups, valid_setups, train_rows, valid_rows,
                    seeds, iterations, depth, learning_rate,
                    pre_feature_set, aggregate_feature_set
                )
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                """,
                (
                    run_id,
                    args.source_run_id,
                    args.results_table,
                    args.train_start_year,
                    args.train_end_year,
                    args.valid_year,
                    len(train_setup_ids),
                    len(valid_setup_ids),
                    len(train_df),
                    len(valid_df),
                    ",".join(str(seed) for seed in seeds),
                    args.iterations,
                    args.depth,
                    args.learning_rate,
                    args.pre_feature_set,
                    None if args.skip_aggregate_features else args.aggregate_feature_set,
                ),
            )
        for seed, stats in single_stats:
            insert_summary(conn, run_id, f"seed_{seed}", stats)
        insert_summary(conn, run_id, "ensemble_mean", mean_stats)
        insert_summary(conn, run_id, "ensemble_median", median_stats)
        conn.commit()

        print("\nEnsemble summary:")
        print(
            f"  mean: {mean_stats['setup_count']} trades, {mean_stats['win_rate']:.2%} win, "
            f"{mean_stats['avg_r']:.3f}R avg, {mean_stats['sum_r']:.1f}R sum"
        )
        print(
            f"  median: {median_stats['setup_count']} trades, {median_stats['win_rate']:.2%} win, "
            f"{median_stats['avg_r']:.3f}R avg, {median_stats['sum_r']:.1f}R sum"
        )
        print(f"\nStored ensemble eval: {run_id}")
        return 0
    finally:
        conn.close()


if __name__ == "__main__":
    sys.exit(main())
