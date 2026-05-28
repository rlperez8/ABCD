#!/usr/bin/env python3
"""
Evaluate whether the AI ranks entry/exit templates well for each setup.

This trains an expected-R CatBoost model, scores every candidate template in the
validation sample, and stores top-k ranking quality next to baseline/oracle
comparisons.
"""

from __future__ import annotations

import argparse
import os
import sys
import time
from pathlib import Path

import numpy as np
import pandas as pd
from catboost import CatBoostClassifier, CatBoostRanker, CatBoostRegressor, Pool
from sklearn.metrics import mean_absolute_error, mean_squared_error

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_build_search_catboost as base

SHAPE_PRE_COLUMNS = {
    "pre_xa_points",
    "pre_ab_points",
    "pre_bc_points",
    "pre_cd_points",
    "pre_ab_to_xa",
    "pre_bc_to_ab",
    "pre_cd_to_xa",
    "pre_cd_to_bc",
    "pre_pattern_height_points",
    "pre_pattern_height_to_price",
    "pre_xa_minutes",
    "pre_ab_minutes",
    "pre_bc_minutes",
    "pre_cd_minutes",
    "pre_full_pattern_minutes",
    "pre_confirm_lag_minutes",
    "pre_xa_velocity",
    "pre_cd_velocity",
    "pre_cd_to_xa_velocity",
    "pre_d_range_to_xa",
    "pre_d_body_to_xa",
    "pre_d_close_location",
    "pre_d_upper_wick_ratio",
    "pre_d_lower_wick_ratio",
    "pre_reversal_signal_count",
}

CANDIDATE_PRE_COLUMNS = {
    "pre_candidate_target_distance_r",
    "pre_candidate_stop_distance_r",
    "pre_candidate_target_clearance_60m_r",
    "pre_candidate_stop_buffer_60m_r",
    "pre_candidate_entry_to_target_side_60m_r",
    "pre_candidate_entry_to_stop_side_60m_r",
    "pre_candidate_target_clearance_120m_r",
    "pre_candidate_stop_buffer_120m_r",
    "pre_candidate_entry_to_target_side_120m_r",
    "pre_candidate_entry_to_stop_side_120m_r",
}
SHAPE_PRE_COLUMNS.update(CANDIDATE_PRE_COLUMNS)


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
    parser.add_argument("--iterations", type=int, default=220)
    parser.add_argument("--depth", type=int, default=6)
    parser.add_argument("--learning-rate", type=float, default=0.035)
    parser.add_argument("--l2-leaf-reg", type=float, default=3.0)
    parser.add_argument("--random-strength", type=float, default=1.0)
    parser.add_argument(
        "--model-objective",
        choices=["regression", "advantage", "rank_percentile", "best_classifier", "ranker"],
        default="regression",
    )
    parser.add_argument("--rank-loss", default="YetiRank")
    parser.add_argument("--skip-aggregate-features", action="store_true")
    parser.add_argument("--aggregate-feature-set", choices=["basic", "all"], default="all")
    parser.add_argument("--pre-feature-set", choices=["all", "core", "shape", "candidate", "none"], default="all")
    parser.add_argument("--top-k", default="1,2,3,5,10")
    parser.add_argument("--min-rule-trades", type=int, default=250)
    parser.add_argument("--random-seed", type=int, default=42)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--no-use-best-model", action="store_true")
    return parser.parse_args()


def make_run_id() -> str:
    return f"aibr-{int(time.time() * 1000)}-{os.getpid()}"


def ensure_tables(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_build_ranking_eval_runs (
                ranking_eval_run_id VARCHAR(64) PRIMARY KEY,
                source_run_id VARCHAR(64) NOT NULL,
                results_table VARCHAR(128) NOT NULL,
                train_start_year INT NOT NULL,
                train_end_year INT NOT NULL,
                valid_year INT NOT NULL,
                train_setups BIGINT NOT NULL,
                valid_setups BIGINT NOT NULL,
                train_rows BIGINT NOT NULL,
                valid_rows BIGINT NOT NULL,
                valid_baseline_avg_r DOUBLE NULL,
                rmse DOUBLE NULL,
                mae DOUBLE NULL,
                best_train_template_uid VARCHAR(128) NULL,
                best_train_template_avg_r DOUBLE NULL,
                model_objective VARCHAR(32) NULL,
                rank_loss VARCHAR(64) NULL,
                model_path VARCHAR(255) NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            """
        )
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_build_ranking_eval_summary (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                ranking_eval_run_id VARCHAR(64) NOT NULL,
                metric_name VARCHAR(64) NOT NULL,
                top_k INT NULL,
                setup_count BIGINT NOT NULL,
                wins BIGINT NOT NULL,
                losses BIGINT NOT NULL,
                win_rate DOUBLE NULL,
                avg_r DOUBLE NULL,
                sum_r DOUBLE NULL,
                avg_rank_of_oracle DOUBLE NULL,
                oracle_in_top_k_rate DOUBLE NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_aibr_summary_run (ranking_eval_run_id, metric_name, top_k)
            )
            """
        )
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_build_ranking_eval_selected (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                ranking_eval_run_id VARCHAR(64) NOT NULL,
                setup_id VARCHAR(64) NOT NULL,
                pattern_id VARCHAR(64) NULL,
                pattern_group_id VARCHAR(128) NULL,
                symbol VARCHAR(32) NULL,
                market VARCHAR(16) NULL,
                pattern_family_key VARCHAR(64) NULL,
                d_confirm_date DATETIME NULL,
                template_uid VARCHAR(128) NOT NULL,
                template_name VARCHAR(255) NULL,
                model_rank INT NOT NULL,
                predicted_expected_r DOUBLE NOT NULL,
                actual_result_r DOUBLE NULL,
                actual_outcome VARCHAR(16) NULL,
                oracle_template_uid VARCHAR(128) NULL,
                oracle_result_r DOUBLE NULL,
                oracle_rank INT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_aibr_selected_run (ranking_eval_run_id),
                INDEX idx_aibr_selected_setup (setup_id)
            )
            """
        )
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_build_ranking_eval_filter_summary (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                ranking_eval_run_id VARCHAR(64) NOT NULL,
                rule_name VARCHAR(96) NOT NULL,
                score_quantile DOUBLE NULL,
                margin_quantile DOUBLE NULL,
                score_threshold DOUBLE NULL,
                margin_threshold DOUBLE NULL,
                selected_trades BIGINT NOT NULL,
                wins BIGINT NOT NULL,
                losses BIGINT NOT NULL,
                win_rate DOUBLE NULL,
                avg_r DOUBLE NULL,
                sum_r DOUBLE NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_aibr_filter_run (ranking_eval_run_id, sum_r)
            )
            """
        )
    conn.commit()
    base.ensure_column(conn, "ai_build_ranking_eval_runs", "model_objective", "VARCHAR(32) NULL")
    base.ensure_column(conn, "ai_build_ranking_eval_runs", "rank_loss", "VARCHAR(64) NULL")
    conn.commit()


def summarize(series: pd.Series) -> dict[str, float | int | None]:
    clean = pd.to_numeric(series, errors="coerce").fillna(0.0)
    count = int(len(clean))
    wins = int((clean > 0.0).sum())
    losses = int((clean <= 0.0).sum())
    return {
        "setup_count": count,
        "wins": wins,
        "losses": losses,
        "win_rate": wins / count if count else None,
        "avg_r": float(clean.mean()) if count else None,
        "sum_r": float(clean.sum()) if count else 0.0,
    }


def build_ranked_frame(scored: pd.DataFrame) -> pd.DataFrame:
    ranked = scored.copy()
    ranked["result_r"] = pd.to_numeric(ranked["result_r"], errors="coerce").fillna(0.0)
    ranked = ranked.sort_values(
        ["setup_id", "predicted_expected_r", "template_uid"],
        ascending=[True, False, True],
    )
    ranked["model_rank"] = ranked.groupby("setup_id").cumcount() + 1

    oracle = (
        ranked.sort_values(["setup_id", "result_r", "template_uid"], ascending=[True, False, True])
        .drop_duplicates("setup_id", keep="first")
        .loc[:, ["setup_id", "template_uid", "result_r"]]
        .rename(
            columns={
                "template_uid": "oracle_template_uid",
                "result_r": "oracle_result_r",
            }
        )
    )
    ranked = ranked.merge(oracle, on="setup_id", how="left")
    oracle_ranks = ranked.loc[
        ranked["template_uid"] == ranked["oracle_template_uid"],
        ["setup_id", "model_rank"],
    ].rename(columns={"model_rank": "oracle_rank"})
    ranked = ranked.merge(oracle_ranks, on="setup_id", how="left")
    top_two = ranked.loc[ranked["model_rank"] <= 2, ["setup_id", "model_rank", "predicted_expected_r"]].copy()
    top_two = top_two.pivot(index="setup_id", columns="model_rank", values="predicted_expected_r").reset_index()
    top_two = top_two.rename(columns={1: "top1_score", 2: "top2_score"})
    top_two["score_margin_top2"] = top_two["top1_score"] - top_two["top2_score"].fillna(top_two["top1_score"])
    ranked = ranked.merge(top_two, on="setup_id", how="left")
    return ranked


def best_train_template(train_df: pd.DataFrame) -> tuple[str, float]:
    grouped = (
        train_df.assign(result_r=pd.to_numeric(train_df["result_r"], errors="coerce").fillna(0.0))
        .groupby("template_uid")["result_r"]
        .mean()
        .sort_values(ascending=False)
    )
    if grouped.empty:
        raise RuntimeError("Could not find best training template")
    return str(grouped.index[0]), float(grouped.iloc[0])


def sorted_for_group_pool(features: pd.DataFrame, labels: pd.Series, setup_ids: pd.Series) -> tuple[pd.DataFrame, pd.Series, pd.Series]:
    order = pd.DataFrame({"setup_id": setup_ids.astype(str), "row_order": np.arange(len(setup_ids))})
    order = order.sort_values(["setup_id", "row_order"], kind="mergesort")
    index = order.index.to_numpy()
    return (
        features.iloc[index].reset_index(drop=True),
        labels.iloc[index].reset_index(drop=True),
        setup_ids.iloc[index].astype(str).reset_index(drop=True),
    )


def aggregate_key_frame(features: pd.DataFrame, key_columns: list[str]) -> pd.Series:
    parts = []
    for column in key_columns:
        if column not in features.columns:
            parts.append(pd.Series(["unknown"] * len(features), index=features.index))
        else:
            parts.append(features[column].fillna("unknown").astype(str))
    key = parts[0]
    for part in parts[1:]:
        key = key + "|" + part
    return key


def add_train_aggregate_features(
    x_train: pd.DataFrame,
    train_result_r: pd.Series,
    x_valid: pd.DataFrame,
    mode: str = "all",
) -> tuple[pd.DataFrame, pd.DataFrame]:
    train = x_train.copy()
    valid = x_valid.copy()
    target = pd.to_numeric(train_result_r, errors="coerce").fillna(0.0).reset_index(drop=True)
    global_avg = float(target.mean())
    global_win = float((target > 0.0).mean())

    aggregate_specs = {
        "template": ["template_uid"],
        "family": ["pattern_family_key"],
        "symbol": ["symbol"],
        "root": ["root_symbol"],
        "direction": ["trade_direction"],
    }
    if mode == "all":
        aggregate_specs.update(
            {
                "family_template": ["pattern_family_key", "template_uid"],
                "symbol_template": ["symbol", "template_uid"],
                "root_template": ["root_symbol", "template_uid"],
                "family_direction": ["pattern_family_key", "trade_direction"],
            }
        )

    for name, keys in aggregate_specs.items():
        train_key = aggregate_key_frame(train, keys)
        valid_key = aggregate_key_frame(valid, keys)
        stats = (
            pd.DataFrame({"key": train_key, "result_r": target, "win": (target > 0.0).astype(float)})
            .groupby("key")
            .agg(avg_r=("result_r", "mean"), win_rate=("win", "mean"), count=("result_r", "size"))
        )

        avg_col = f"hist_{name}_avg_r"
        win_col = f"hist_{name}_win_rate"
        count_col = f"hist_{name}_count"

        train[avg_col] = train_key.map(stats["avg_r"]).fillna(global_avg).astype(float)
        valid[avg_col] = valid_key.map(stats["avg_r"]).fillna(global_avg).astype(float)
        train[win_col] = train_key.map(stats["win_rate"]).fillna(global_win).astype(float)
        valid[win_col] = valid_key.map(stats["win_rate"]).fillna(global_win).astype(float)
        train[count_col] = np.log1p(train_key.map(stats["count"]).fillna(0.0).astype(float))
        valid[count_col] = np.log1p(valid_key.map(stats["count"]).fillna(0.0).astype(float))

    return train, valid


def setup_advantage_labels(df: pd.DataFrame) -> pd.Series:
    result = pd.to_numeric(df["result_r"], errors="coerce").fillna(0.0)
    setup_avg = result.groupby(df["setup_id"].astype(str)).transform("mean")
    return result - setup_avg


def setup_best_candidate_labels(df: pd.DataFrame) -> pd.Series:
    result = pd.to_numeric(df["result_r"], errors="coerce").fillna(0.0)
    setup_max = result.groupby(df["setup_id"].astype(str)).transform("max")
    return ((result == setup_max) & (setup_max > 0.0)).astype(int)


def setup_rank_percentile_labels(df: pd.DataFrame) -> pd.Series:
    result = pd.to_numeric(df["result_r"], errors="coerce").fillna(0.0)
    ranks = result.groupby(df["setup_id"].astype(str)).rank(method="average", ascending=False)
    counts = result.groupby(df["setup_id"].astype(str)).transform("size").astype(float)
    denominator = (counts - 1.0).replace(0.0, 1.0)
    return 1.0 - ((ranks - 1.0) / denominator)


def apply_pre_feature_set(features: pd.DataFrame, mode: str) -> pd.DataFrame:
    if mode == "all":
        return features

    pre_columns = [column for column in features.columns if column.startswith("pre_")]
    if mode == "none":
        return features.drop(columns=pre_columns, errors="ignore")
    if mode == "shape":
        drop_columns = [column for column in pre_columns if column not in SHAPE_PRE_COLUMNS]
        return features.drop(columns=drop_columns, errors="ignore")
    if mode == "candidate":
        drop_columns = [column for column in pre_columns if column not in CANDIDATE_PRE_COLUMNS]
        return features.drop(columns=drop_columns, errors="ignore")

    core_names = {
        "pre_total_candles",
        "pre_return_15m",
        "pre_range_15m",
        "pre_directional_bias_15m",
        "pre_volatility_15m",
        "pre_return_60m",
        "pre_range_60m",
        "pre_directional_bias_60m",
        "pre_volatility_60m",
        "pre_return_120m",
        "pre_range_120m",
        "pre_directional_bias_120m",
        "pre_volatility_120m",
        "pre_distance_to_60m_high",
        "pre_distance_to_60m_low",
        "pre_distance_to_120m_high",
        "pre_distance_to_120m_low",
    }
    drop_columns = [column for column in pre_columns if column not in core_names]
    return features.drop(columns=drop_columns, errors="ignore")


def insert_summary(conn, run_id: str, metric_name: str, top_k: int | None, stats: dict, extra: dict | None = None) -> None:
    extra = extra or {}
    with conn.cursor() as cur:
        cur.execute(
            """
            INSERT INTO ai_build_ranking_eval_summary (
                ranking_eval_run_id, metric_name, top_k, setup_count,
                wins, losses, win_rate, avg_r, sum_r,
                avg_rank_of_oracle, oracle_in_top_k_rate
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            """,
            (
                run_id,
                metric_name,
                top_k,
                stats["setup_count"],
                stats["wins"],
                stats["losses"],
                stats["win_rate"],
                stats["avg_r"],
                stats["sum_r"],
                extra.get("avg_rank_of_oracle"),
                extra.get("oracle_in_top_k_rate"),
            ),
        )


def insert_filter_summary(
    conn,
    run_id: str,
    rule_name: str,
    score_quantile: float | None,
    margin_quantile: float | None,
    score_threshold: float | None,
    margin_threshold: float | None,
    stats: dict,
) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            INSERT INTO ai_build_ranking_eval_filter_summary (
                ranking_eval_run_id, rule_name, score_quantile, margin_quantile,
                score_threshold, margin_threshold, selected_trades,
                wins, losses, win_rate, avg_r, sum_r
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            """,
            (
                run_id,
                rule_name,
                score_quantile,
                margin_quantile,
                score_threshold,
                margin_threshold,
                stats["setup_count"],
                stats["wins"],
                stats["losses"],
                stats["win_rate"],
                stats["avg_r"],
                stats["sum_r"],
            ),
        )


def evaluate_filter_rules(top_one: pd.DataFrame) -> list[dict[str, object]]:
    if top_one.empty:
        return []

    score = pd.to_numeric(top_one["predicted_expected_r"], errors="coerce").fillna(0.0)
    margin = pd.to_numeric(top_one["score_margin_top2"], errors="coerce").fillna(0.0)
    score_quantiles = [0.0, 0.25, 0.5, 0.65, 0.75, 0.85, 0.9, 0.95]
    margin_quantiles = [None, 0.0, 0.25, 0.5, 0.75, 0.9]
    rows: list[dict[str, object]] = []

    for score_q in score_quantiles:
        score_threshold = float(score.quantile(score_q))
        for margin_q in margin_quantiles:
            if margin_q is None:
                selected = top_one[score >= score_threshold]
                margin_threshold = None
                rule_name = "score_only"
            else:
                margin_threshold = float(margin.quantile(margin_q))
                selected = top_one[(score >= score_threshold) & (margin >= margin_threshold)]
                rule_name = "score_and_margin"
            stats = summarize(selected["result_r"])
            rows.append(
                {
                    "rule_name": rule_name,
                    "score_quantile": score_q,
                    "margin_quantile": margin_q,
                    "score_threshold": score_threshold,
                    "margin_threshold": margin_threshold,
                    **stats,
                }
            )
    return rows


def main() -> int:
    args = parse_args()
    args.results_table = base.safe_identifier(args.results_table)
    top_ks = [int(part.strip()) for part in args.top_k.split(",") if part.strip()]
    run_id = args.run_id or make_run_id()

    conn = base.connect()
    try:
        base.ensure_tables(conn)
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

        print(f"AI ranking eval: {run_id}")
        print(f"Source run: {args.source_run_id}")
        print(f"Results table: {args.results_table}")
        print(f"Train years: {args.train_start_year}-{args.train_end_year}")
        print(f"Train setups: {len(train_setup_ids)}")
        print(f"Valid setups: {len(valid_setup_ids)}")

        train_df = base.load_candidate_rows(conn, args.results_table, args.source_run_id, train_setup_ids)
        valid_df = base.load_candidate_rows(conn, args.results_table, args.source_run_id, valid_setup_ids)
        if train_df.empty or valid_df.empty:
            raise RuntimeError("Training or validation rows were empty")

        print(f"Train rows: {len(train_df):,}")
        print(f"Valid rows: {len(valid_df):,}")

        x_train, _, train_result_r = base.prepare_features(train_df)
        x_valid, _, valid_result_r = base.prepare_features(valid_df)
        x_train = apply_pre_feature_set(x_train, args.pre_feature_set)
        x_valid = apply_pre_feature_set(x_valid, args.pre_feature_set)
        if not args.skip_aggregate_features:
            x_train, x_valid = add_train_aggregate_features(
                x_train,
                train_result_r,
                x_valid,
                args.aggregate_feature_set,
            )

        cat_indexes = [x_train.columns.get_loc(col) for col in base.CAT_FEATURES]
        if args.model_objective == "advantage":
            train_label = setup_advantage_labels(train_df)
            valid_label = setup_advantage_labels(valid_df)
        elif args.model_objective == "rank_percentile":
            train_label = setup_rank_percentile_labels(train_df)
            valid_label = setup_rank_percentile_labels(valid_df)
        elif args.model_objective == "best_classifier":
            train_label = setup_best_candidate_labels(train_df)
            valid_label = setup_best_candidate_labels(valid_df)
        else:
            train_label = train_result_r
            valid_label = valid_result_r

        if args.model_objective == "ranker":
            x_train_pool, y_train_pool, train_groups = sorted_for_group_pool(
                x_train,
                train_label,
                train_df["setup_id"],
            )
            x_valid_pool, y_valid_pool, valid_groups = sorted_for_group_pool(
                x_valid,
                valid_label,
                valid_df["setup_id"],
            )
            train_pool = Pool(
                x_train_pool,
                label=y_train_pool,
                group_id=train_groups,
                cat_features=cat_indexes,
            )
            valid_pool = Pool(
                x_valid_pool,
                label=y_valid_pool,
                group_id=valid_groups,
                cat_features=cat_indexes,
            )
            model = CatBoostRanker(
                iterations=args.iterations,
                depth=args.depth,
                learning_rate=args.learning_rate,
                loss_function=args.rank_loss,
                random_seed=args.random_seed,
                l2_leaf_reg=args.l2_leaf_reg,
                random_strength=args.random_strength,
                verbose=100,
            )
            model.fit(train_pool, eval_set=valid_pool, use_best_model=not args.no_use_best_model)
            predictions = model.predict(Pool(x_valid, cat_features=cat_indexes))
        elif args.model_objective == "best_classifier":
            train_pool = Pool(x_train, label=train_label, cat_features=cat_indexes)
            valid_pool = Pool(x_valid, label=valid_label, cat_features=cat_indexes)
            model = CatBoostClassifier(
                iterations=args.iterations,
                depth=args.depth,
                learning_rate=args.learning_rate,
                loss_function="Logloss",
                eval_metric="AUC",
                random_seed=args.random_seed,
                l2_leaf_reg=args.l2_leaf_reg,
                random_strength=args.random_strength,
                auto_class_weights="Balanced",
                verbose=100,
            )
            model.fit(train_pool, eval_set=valid_pool, use_best_model=not args.no_use_best_model)
            predictions = model.predict_proba(valid_pool)[:, 1]
        else:
            train_pool = Pool(x_train, label=train_label, cat_features=cat_indexes)
            valid_pool = Pool(x_valid, label=valid_label, cat_features=cat_indexes)

            model = CatBoostRegressor(
                iterations=args.iterations,
                depth=args.depth,
                learning_rate=args.learning_rate,
                loss_function="RMSE",
                eval_metric="RMSE",
                random_seed=args.random_seed,
                l2_leaf_reg=args.l2_leaf_reg,
                random_strength=args.random_strength,
                verbose=100,
            )
            model.fit(train_pool, eval_set=valid_pool, use_best_model=not args.no_use_best_model)
            predictions = model.predict(valid_pool)
        rmse = float(mean_squared_error(valid_label, predictions) ** 0.5)
        mae = float(mean_absolute_error(valid_label, predictions))

        scored = valid_df.copy()
        scored["predicted_expected_r"] = predictions
        ranked = build_ranked_frame(scored)
        top_one = ranked[ranked["model_rank"] == 1].copy()
        best_template_uid, best_template_avg_r = best_train_template(train_df)
        baseline = ranked[ranked["template_uid"] == best_template_uid].copy()
        oracle = ranked.drop_duplicates("setup_id", keep="first").copy()
        oracle["result_r"] = oracle["oracle_result_r"]

        model_dir = base.ABCD_ROOT / "tmp_logs"
        model_dir.mkdir(parents=True, exist_ok=True)
        model_path = model_dir / f"{run_id}.cbm"
        model.save_model(str(model_path))

        with conn.cursor() as cur:
            cur.execute(
                """
                INSERT INTO ai_build_ranking_eval_runs (
                    ranking_eval_run_id, source_run_id, results_table,
                    train_start_year, train_end_year, valid_year,
                    train_setups, valid_setups, train_rows, valid_rows,
                    valid_baseline_avg_r, rmse, mae,
                    best_train_template_uid, best_train_template_avg_r,
                    model_objective, rank_loss, model_path
                )
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
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
                    float(valid_result_r.mean()),
                    rmse,
                    mae,
                    best_template_uid,
                    best_template_avg_r,
                    args.model_objective,
                    args.rank_loss if args.model_objective == "ranker" else None,
                    str(model_path),
                ),
            )

        insert_summary(conn, run_id, "ai_top_1", 1, summarize(top_one["result_r"]))
        insert_summary(conn, run_id, "best_train_template", None, summarize(baseline["result_r"]))
        insert_summary(conn, run_id, "oracle_best_all", None, summarize(oracle["result_r"]))
        filter_rows = evaluate_filter_rules(top_one)
        for row in filter_rows:
            insert_filter_summary(
                conn,
                run_id,
                str(row["rule_name"]),
                row["score_quantile"],
                row["margin_quantile"],
                row["score_threshold"],
                row["margin_threshold"],
                row,
            )

        avg_oracle_rank = float(top_one["oracle_rank"].mean()) if not top_one.empty else None
        for top_k in top_ks:
            top_k_frame = ranked[ranked["model_rank"] <= top_k].copy()
            top_k_best = (
                top_k_frame.sort_values(["setup_id", "result_r", "template_uid"], ascending=[True, False, True])
                .drop_duplicates("setup_id", keep="first")
            )
            oracle_in_top_k_rate = float((top_one["oracle_rank"] <= top_k).mean()) if not top_one.empty else None
            insert_summary(
                conn,
                run_id,
                "best_actual_within_ai_top_k",
                top_k,
                summarize(top_k_best["result_r"]),
                {
                    "avg_rank_of_oracle": avg_oracle_rank,
                    "oracle_in_top_k_rate": oracle_in_top_k_rate,
                },
            )

        selected_cols = [
            "setup_id",
            "pattern_id",
            "pattern_group_id",
            "symbol",
            "market",
            "pattern_family_key",
            "d_confirm_date",
            "template_uid",
            "template_name",
            "model_rank",
            "predicted_expected_r",
            "result_r",
            "outcome",
            "oracle_template_uid",
            "oracle_result_r",
            "oracle_rank",
        ]
        rows = []
        for row in top_one[selected_cols].itertuples(index=False):
            rows.append((run_id, *[base.clean_db_value(value) for value in row]))
        with conn.cursor() as cur:
            cur.executemany(
                """
                INSERT INTO ai_build_ranking_eval_selected (
                    ranking_eval_run_id, setup_id, pattern_id, pattern_group_id,
                    symbol, market, pattern_family_key, d_confirm_date,
                    template_uid, template_name, model_rank, predicted_expected_r,
                    actual_result_r, actual_outcome, oracle_template_uid,
                    oracle_result_r, oracle_rank
                )
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                """,
                rows,
            )
        conn.commit()

        print("\nRanking summary:")
        print(f"  validation candidate avg: {float(valid_result_r.mean()):.4f}R")
        print(f"  RMSE: {rmse:.4f}R")
        print(f"  MAE: {mae:.4f}R")
        print(f"  model objective: {args.model_objective}")
        if args.model_objective == "ranker":
            print(f"  rank loss: {args.rank_loss}")
        print(f"  best train template: {best_template_uid} ({best_template_avg_r:.4f}R train avg)")

        summaries = [
            ("AI top 1", summarize(top_one["result_r"])),
            ("Best train template", summarize(baseline["result_r"])),
            ("Oracle best all", summarize(oracle["result_r"])),
        ]
        for label, stats in summaries:
            print(
                f"  {label}: {stats['setup_count']} trades, "
                f"{stats['win_rate']:.2%} win, {stats['avg_r']:.3f}R avg, {stats['sum_r']:.1f}R sum"
            )

        print("\nRanking quality:")
        print(f"  average oracle rank: {avg_oracle_rank:.2f}")
        for top_k in top_ks:
            top_k_frame = ranked[ranked["model_rank"] <= top_k].copy()
            top_k_best = (
                top_k_frame.sort_values(["setup_id", "result_r", "template_uid"], ascending=[True, False, True])
                .drop_duplicates("setup_id", keep="first")
            )
            stats = summarize(top_k_best["result_r"])
            oracle_rate = float((top_one["oracle_rank"] <= top_k).mean()) if not top_one.empty else 0.0
            print(
                f"  Top {top_k}: oracle in top {top_k} {oracle_rate:.2%}; "
                f"best actual within top {top_k}: {stats['avg_r']:.3f}R avg, {stats['sum_r']:.1f}R sum"
            )

        eligible_filter_rows = [
            row for row in filter_rows if int(row["setup_count"]) >= args.min_rule_trades
        ]
        best_by_sum = max(
            eligible_filter_rows,
            key=lambda row: (float(row["sum_r"] or 0.0), float(row["avg_r"] or -999.0)),
            default=None,
        )
        best_by_avg = max(
            eligible_filter_rows,
            key=lambda row: (float(row["avg_r"] or -999.0), float(row["sum_r"] or 0.0)),
            default=None,
        )
        print("\nSelector rules:")
        if best_by_sum:
            margin_text = (
                "none"
                if best_by_sum["margin_quantile"] is None
                else f"q{best_by_sum['margin_quantile']:.2f} >= {best_by_sum['margin_threshold']:.4f}"
            )
            print(
                "  Best total R: "
                f"{best_by_sum['rule_name']} score q{best_by_sum['score_quantile']:.2f} >= "
                f"{best_by_sum['score_threshold']:.4f}, margin {margin_text}; "
                f"{best_by_sum['setup_count']} trades, {best_by_sum['win_rate']:.2%} win, "
                f"{best_by_sum['avg_r']:.3f}R avg, {best_by_sum['sum_r']:.1f}R sum"
            )
        if best_by_avg:
            margin_text = (
                "none"
                if best_by_avg["margin_quantile"] is None
                else f"q{best_by_avg['margin_quantile']:.2f} >= {best_by_avg['margin_threshold']:.4f}"
            )
            print(
                "  Best avg R: "
                f"{best_by_avg['rule_name']} score q{best_by_avg['score_quantile']:.2f} >= "
                f"{best_by_avg['score_threshold']:.4f}, margin {margin_text}; "
                f"{best_by_avg['setup_count']} trades, {best_by_avg['win_rate']:.2%} win, "
                f"{best_by_avg['avg_r']:.3f}R avg, {best_by_avg['sum_r']:.1f}R sum"
            )

        print(f"\nStored ranking eval: {run_id}")
        print(f"  model: {model_path}")
        return 0
    finally:
        conn.close()


if __name__ == "__main__":
    sys.exit(main())
