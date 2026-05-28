#!/usr/bin/env python3
"""
Train a second model that decides whether to take or skip Stage 1's top pick.
"""

from __future__ import annotations

import argparse
import os
import sys
import time
from pathlib import Path

import numpy as np
import pandas as pd
from catboost import CatBoostClassifier, Pool

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_build_ranking_eval as ranking
import ai_build_reranker_eval as reranker
import ai_build_search_catboost as base


FILTER_META_FEATURES = [
    "stage1_score",
    "stage1_top2_margin",
    "stage1_gap_to_top",
    "stage1_score_z",
]


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
    parser.add_argument("--filter-label", choices=["result_positive", "path_target_first"], default="path_target_first")
    parser.add_argument("--filter-iterations", type=int, default=180)
    parser.add_argument("--filter-depth", type=int, default=5)
    parser.add_argument("--filter-learning-rate", type=float, default=0.035)
    parser.add_argument("--l2-leaf-reg", type=float, default=3.0)
    parser.add_argument("--random-strength", type=float, default=1.0)
    parser.add_argument("--random-seed", type=int, default=42)
    parser.add_argument("--min-selected-trades", type=int, default=250)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--no-use-best-model", action="store_true")
    return parser.parse_args()


def make_run_id() -> str:
    return f"aift-{int(time.time() * 1000)}-{os.getpid()}"


def ensure_tables(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_stage1_trade_filter_eval_runs (
                trade_filter_run_id VARCHAR(64) PRIMARY KEY,
                source_run_id VARCHAR(64) NOT NULL,
                results_table VARCHAR(128) NOT NULL,
                train_start_year INT NOT NULL,
                train_end_year INT NOT NULL,
                valid_year INT NOT NULL,
                train_setups BIGINT NOT NULL,
                valid_setups BIGINT NOT NULL,
                filter_label VARCHAR(32) NOT NULL,
                model_path VARCHAR(255) NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            """
        )
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_stage1_trade_filter_eval_summary (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                trade_filter_run_id VARCHAR(64) NOT NULL,
                rule_name VARCHAR(64) NOT NULL,
                probability_quantile DOUBLE NULL,
                probability_threshold DOUBLE NULL,
                selected_trades BIGINT NOT NULL,
                skipped_trades BIGINT NOT NULL,
                wins BIGINT NOT NULL,
                losses BIGINT NOT NULL,
                win_rate DOUBLE NULL,
                avg_r DOUBLE NULL,
                sum_r DOUBLE NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_aift_summary_run (trade_filter_run_id, sum_r)
            )
            """
        )
    conn.commit()


def build_stage1_data(args: argparse.Namespace, conn):
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
    train_df = base.load_candidate_rows(conn, args.results_table, args.source_run_id, train_setup_ids)
    valid_df = base.load_candidate_rows(conn, args.results_table, args.source_run_id, valid_setup_ids)
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
    return train_setup_ids, valid_setup_ids, train_df, valid_df, x_train, x_valid, train_predictions, valid_predictions, stage1_model


def top_one_with_features(df: pd.DataFrame, features: pd.DataFrame, predictions: np.ndarray) -> tuple[pd.DataFrame, pd.DataFrame]:
    ranked = reranker.build_ranked_with_index(df, predictions)
    shortlist = ranked[ranked["model_rank"] <= 10].copy().reset_index(drop=True)
    shortlist_features = features.iloc[shortlist["feature_row_index"].astype(int)].reset_index(drop=True)
    shortlist_features = reranker.add_rerank_meta_features(shortlist_features, shortlist)
    top_mask = shortlist["model_rank"] == 1
    top_one = shortlist.loc[top_mask].copy().reset_index(drop=True)
    top_features = shortlist_features.loc[top_mask].copy().reset_index(drop=True)
    return top_one, top_features


def labels_for(frame: pd.DataFrame, mode: str) -> pd.Series:
    if mode == "result_positive":
        return (pd.to_numeric(frame["result_r"], errors="coerce").fillna(0.0) > 0.0).astype(int)
    first_hit = frame.get("path_first_hit_outcome", pd.Series([""] * len(frame), index=frame.index)).fillna("").astype(str)
    return (first_hit == "target").astype(int)


def insert_summary(conn, run_id: str, row: dict[str, object]) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            INSERT INTO ai_stage1_trade_filter_eval_summary (
                trade_filter_run_id, rule_name, probability_quantile,
                probability_threshold, selected_trades, skipped_trades,
                wins, losses, win_rate, avg_r, sum_r
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            """,
            (
                run_id,
                row["rule_name"],
                row["probability_quantile"],
                row["probability_threshold"],
                row["selected_trades"],
                row["skipped_trades"],
                row["wins"],
                row["losses"],
                row["win_rate"],
                row["avg_r"],
                row["sum_r"],
            ),
        )


def evaluate_rules(top_one: pd.DataFrame, probabilities: np.ndarray) -> list[dict[str, object]]:
    result = pd.to_numeric(top_one["result_r"], errors="coerce").fillna(0.0).reset_index(drop=True)
    probability = pd.Series(probabilities).fillna(0.0)
    rows = []
    for quantile in [0.0, 0.25, 0.5, 0.65, 0.75, 0.85, 0.9, 0.95]:
        threshold = float(probability.quantile(quantile))
        selected = result[probability >= threshold]
        stats = ranking.summarize(selected)
        rows.append(
            {
                "rule_name": "probability_quantile",
                "probability_quantile": quantile,
                "probability_threshold": threshold,
                "selected_trades": stats["setup_count"],
                "skipped_trades": int(len(result) - stats["setup_count"]),
                "wins": stats["wins"],
                "losses": stats["losses"],
                "win_rate": stats["win_rate"],
                "avg_r": stats["avg_r"],
                "sum_r": stats["sum_r"],
            }
        )
    return rows


def main() -> int:
    args = parse_args()
    args.results_table = base.safe_identifier(args.results_table)
    run_id = args.run_id or make_run_id()
    conn = base.connect()
    try:
        ensure_tables(conn)
        (
            train_setup_ids,
            valid_setup_ids,
            train_df,
            valid_df,
            x_train,
            x_valid,
            train_predictions,
            valid_predictions,
            _stage1_model,
        ) = build_stage1_data(args, conn)
        print(f"AI trade filter eval: {run_id}")
        print(f"Train setups: {len(train_setup_ids):,}")
        print(f"Valid setups: {len(valid_setup_ids):,}")
        print(f"Train rows: {len(train_df):,}")
        print(f"Valid rows: {len(valid_df):,}")

        train_top, train_top_x = top_one_with_features(train_df, x_train, train_predictions)
        valid_top, valid_top_x = top_one_with_features(valid_df, x_valid, valid_predictions)
        train_label = labels_for(train_top, args.filter_label)
        valid_label = labels_for(valid_top, args.filter_label)
        print(f"Filter label: {args.filter_label}")
        print(f"Train positive label rate: {float(train_label.mean()):.2%}")
        print(f"Valid positive label rate: {float(valid_label.mean()):.2%}")

        cat_indexes = [train_top_x.columns.get_loc(col) for col in base.CAT_FEATURES if col in train_top_x.columns]
        train_pool = Pool(train_top_x, label=train_label, cat_features=cat_indexes)
        valid_pool = Pool(valid_top_x, label=valid_label, cat_features=cat_indexes)
        model = CatBoostClassifier(
            iterations=args.filter_iterations,
            depth=args.filter_depth,
            learning_rate=args.filter_learning_rate,
            loss_function="Logloss",
            eval_metric="AUC",
            random_seed=args.random_seed,
            l2_leaf_reg=args.l2_leaf_reg,
            random_strength=args.random_strength,
            auto_class_weights="Balanced",
            verbose=100,
        )
        model.fit(train_pool, eval_set=valid_pool, use_best_model=not args.no_use_best_model)
        probabilities = model.predict_proba(valid_pool)[:, 1]

        model_dir = base.ABCD_ROOT / "tmp_logs"
        model_dir.mkdir(parents=True, exist_ok=True)
        model_path = model_dir / f"{run_id}.cbm"
        model.save_model(str(model_path))

        with conn.cursor() as cur:
            cur.execute(
                """
                INSERT INTO ai_stage1_trade_filter_eval_runs (
                    trade_filter_run_id, source_run_id, results_table,
                    train_start_year, train_end_year, valid_year,
                    train_setups, valid_setups, filter_label, model_path
                )
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
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
                    args.filter_label,
                    str(model_path),
                ),
            )

        baseline_stats = ranking.summarize(valid_top["result_r"])
        print("\nBaseline Stage 1 top 1:")
        print(
            f"  {baseline_stats['setup_count']} trades, {baseline_stats['win_rate']:.2%} win, "
            f"{baseline_stats['avg_r']:.3f}R avg, {baseline_stats['sum_r']:.1f}R sum"
        )

        rows = evaluate_rules(valid_top, probabilities)
        for row in rows:
            insert_summary(conn, run_id, row)
        conn.commit()

        eligible = [row for row in rows if int(row["selected_trades"]) >= args.min_selected_trades]
        best_total = max(eligible, key=lambda row: (float(row["sum_r"] or 0.0), float(row["avg_r"] or -999.0)), default=None)
        best_avg = max(eligible, key=lambda row: (float(row["avg_r"] or -999.0), float(row["sum_r"] or 0.0)), default=None)
        print("\nFilter rules:")
        for label, row in [("Best total R", best_total), ("Best avg R", best_avg)]:
            if not row:
                continue
            print(
                f"  {label}: q{row['probability_quantile']:.2f} >= {row['probability_threshold']:.4f}; "
                f"{row['selected_trades']} trades, {row['win_rate']:.2%} win, "
                f"{row['avg_r']:.3f}R avg, {row['sum_r']:.1f}R sum"
            )

        print(f"\nStored trade filter eval: {run_id}")
        print(f"  model: {model_path}")
        return 0
    finally:
        conn.close()


if __name__ == "__main__":
    sys.exit(main())
