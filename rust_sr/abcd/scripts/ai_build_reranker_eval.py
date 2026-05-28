#!/usr/bin/env python3
"""
Evaluate a two-stage AI selector for entry/exit templates.

Stage 1 scores every candidate template with the current expected-R model.
Stage 2 only sees the Stage 1 shortlist for each setup and tries to choose the
best candidate inside that shortlist.
"""

from __future__ import annotations

import argparse
import os
import sys
import time
from pathlib import Path

import numpy as np
import pandas as pd
from catboost import CatBoostClassifier, CatBoostRegressor, Pool
from sklearn.metrics import mean_absolute_error, mean_squared_error

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_build_ranking_eval as ranking
import ai_build_search_catboost as base


RERANK_META_FEATURES = [
    "stage1_score",
    "stage1_rank",
    "stage1_rank_pct",
    "stage1_gap_to_top",
    "stage1_score_z",
    "stage1_top2_margin",
    "stage1_score_to_prev",
    "stage1_score_to_next",
    "stage1_shortlist_size",
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
    parser.add_argument("--stage1-oof-folds", type=int, default=0)
    parser.add_argument("--rerank-top-n", type=int, default=10)
    parser.add_argument(
        "--stage2-objective",
        choices=["best_classifier", "result_r", "advantage", "path_first_hit", "path_opportunity"],
        default="best_classifier",
    )
    parser.add_argument("--stage2-iterations", type=int, default=180)
    parser.add_argument("--stage2-depth", type=int, default=5)
    parser.add_argument("--stage2-learning-rate", type=float, default=0.035)
    parser.add_argument("--l2-leaf-reg", type=float, default=3.0)
    parser.add_argument("--random-strength", type=float, default=1.0)
    parser.add_argument("--random-seed", type=int, default=42)
    parser.add_argument("--top-k", default="1,2,3,5,10")
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--no-use-best-model", action="store_true")
    return parser.parse_args()


def make_run_id() -> str:
    return f"aibr2-{int(time.time() * 1000)}-{os.getpid()}"


def ensure_tables(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_build_reranker_eval_runs (
                reranker_eval_run_id VARCHAR(64) PRIMARY KEY,
                source_run_id VARCHAR(64) NOT NULL,
                results_table VARCHAR(128) NOT NULL,
                train_start_year INT NOT NULL,
                train_end_year INT NOT NULL,
                valid_year INT NOT NULL,
                train_setups BIGINT NOT NULL,
                valid_setups BIGINT NOT NULL,
                train_rows BIGINT NOT NULL,
                valid_rows BIGINT NOT NULL,
                rerank_top_n INT NOT NULL,
                pre_feature_set VARCHAR(32) NOT NULL,
                aggregate_feature_set VARCHAR(32) NULL,
                stage2_objective VARCHAR(32) NOT NULL,
                stage1_oof_folds INT NOT NULL DEFAULT 0,
                stage1_model_path VARCHAR(255) NULL,
                stage2_model_path VARCHAR(255) NULL,
                stage2_rmse DOUBLE NULL,
                stage2_mae DOUBLE NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            """
        )
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_build_reranker_eval_summary (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                reranker_eval_run_id VARCHAR(64) NOT NULL,
                metric_name VARCHAR(64) NOT NULL,
                top_k INT NULL,
                setup_count BIGINT NOT NULL,
                wins BIGINT NOT NULL,
                losses BIGINT NOT NULL,
                win_rate DOUBLE NULL,
                avg_r DOUBLE NULL,
                sum_r DOUBLE NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_aibr2_summary_run (reranker_eval_run_id, metric_name, top_k)
            )
            """
        )
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_build_reranker_eval_selected (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                reranker_eval_run_id VARCHAR(64) NOT NULL,
                setup_id VARCHAR(64) NOT NULL,
                pattern_id VARCHAR(64) NULL,
                pattern_group_id VARCHAR(128) NULL,
                symbol VARCHAR(32) NULL,
                market VARCHAR(16) NULL,
                pattern_family_key VARCHAR(64) NULL,
                d_confirm_date DATETIME NULL,
                template_uid VARCHAR(128) NOT NULL,
                template_name VARCHAR(255) NULL,
                stage1_rank INT NULL,
                stage1_score DOUBLE NULL,
                stage2_rank INT NULL,
                stage2_score DOUBLE NULL,
                actual_result_r DOUBLE NULL,
                actual_outcome VARCHAR(16) NULL,
                oracle_template_uid VARCHAR(128) NULL,
                oracle_result_r DOUBLE NULL,
                oracle_rank INT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_aibr2_selected_run (reranker_eval_run_id),
                INDEX idx_aibr2_selected_setup (setup_id)
            )
            """
        )
    conn.commit()
    base.ensure_column(conn, "ai_build_reranker_eval_runs", "stage1_oof_folds", "INT NOT NULL DEFAULT 0")
    conn.commit()


def insert_summary(conn, run_id: str, metric_name: str, top_k: int | None, stats: dict) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            INSERT INTO ai_build_reranker_eval_summary (
                reranker_eval_run_id, metric_name, top_k, setup_count,
                wins, losses, win_rate, avg_r, sum_r
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)
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
            ),
        )


def build_stage_features(args: argparse.Namespace, train_df: pd.DataFrame, valid_df: pd.DataFrame) -> tuple[pd.DataFrame, pd.DataFrame, pd.Series, pd.Series]:
    x_train, _, train_result_r = base.prepare_features(train_df)
    x_valid, _, valid_result_r = base.prepare_features(valid_df)
    x_train = ranking.apply_pre_feature_set(x_train, args.pre_feature_set)
    x_valid = ranking.apply_pre_feature_set(x_valid, args.pre_feature_set)
    if not args.skip_aggregate_features:
        x_train, x_valid = ranking.add_train_aggregate_features(
            x_train,
            train_result_r,
            x_valid,
            args.aggregate_feature_set,
        )
    return x_train, x_valid, train_result_r, valid_result_r


def train_stage1_model(
    args: argparse.Namespace,
    x_train: pd.DataFrame,
    train_result_r: pd.Series,
    verbose: int | bool = 100,
) -> tuple[CatBoostRegressor, Pool, list[int]]:
    cat_indexes = [x_train.columns.get_loc(col) for col in base.CAT_FEATURES if col in x_train.columns]
    train_pool = Pool(x_train, label=train_result_r, cat_features=cat_indexes)
    model = CatBoostRegressor(
        iterations=args.stage1_iterations,
        depth=args.stage1_depth,
        learning_rate=args.stage1_learning_rate,
        loss_function="RMSE",
        eval_metric="RMSE",
        random_seed=args.random_seed,
        l2_leaf_reg=args.l2_leaf_reg,
        random_strength=args.random_strength,
        verbose=verbose,
    )
    model.fit(train_pool, use_best_model=False)
    return model, train_pool, cat_indexes


def train_stage1(args: argparse.Namespace, x_train: pd.DataFrame, train_result_r: pd.Series, x_valid: pd.DataFrame) -> tuple[CatBoostRegressor, np.ndarray, np.ndarray]:
    model, train_pool, cat_indexes = train_stage1_model(args, x_train, train_result_r)
    train_predictions = model.predict(Pool(x_train, cat_features=cat_indexes))
    valid_predictions = model.predict(Pool(x_valid, cat_features=cat_indexes))
    return model, train_predictions, valid_predictions


def setup_fold_assignments(setup_ids: pd.Series, fold_count: int) -> pd.Series:
    unique = pd.Series(setup_ids.astype(str).unique()).sort_values(kind="mergesort").reset_index(drop=True)
    mapping = {setup_id: index % fold_count for index, setup_id in unique.items()}
    return setup_ids.astype(str).map(mapping).astype(int)


def train_stage1_oof_predictions(
    args: argparse.Namespace,
    x_train: pd.DataFrame,
    train_result_r: pd.Series,
    train_setup_ids: pd.Series,
) -> np.ndarray:
    fold_count = max(2, int(args.stage1_oof_folds))
    folds = setup_fold_assignments(train_setup_ids, fold_count)
    predictions = np.zeros(len(x_train), dtype=float)
    for fold in range(fold_count):
        valid_mask = folds == fold
        train_mask = ~valid_mask
        print(
            f"Stage 1 OOF fold {fold + 1}/{fold_count}: "
            f"train rows {int(train_mask.sum()):,}, score rows {int(valid_mask.sum()):,}"
        )
        fold_model, _, fold_cat_indexes = train_stage1_model(
            args,
            x_train.loc[train_mask].reset_index(drop=True),
            train_result_r.loc[train_mask].reset_index(drop=True),
            verbose=False,
        )
        predictions[valid_mask.to_numpy()] = fold_model.predict(
            Pool(x_train.loc[valid_mask].reset_index(drop=True), cat_features=fold_cat_indexes)
        )
    return predictions


def build_ranked_with_index(df: pd.DataFrame, predictions: np.ndarray) -> pd.DataFrame:
    scored = df.copy()
    scored["feature_row_index"] = np.arange(len(scored))
    scored["predicted_expected_r"] = predictions
    return ranking.build_ranked_frame(scored)


def shortlist_frame(ranked: pd.DataFrame, top_n: int) -> pd.DataFrame:
    return ranked.loc[ranked["model_rank"] <= top_n].copy().reset_index(drop=True)


def add_rerank_meta_features(features: pd.DataFrame, shortlist: pd.DataFrame) -> pd.DataFrame:
    result = features.reset_index(drop=True).copy()
    score = pd.to_numeric(shortlist["predicted_expected_r"], errors="coerce").fillna(0.0).reset_index(drop=True)
    rank = pd.to_numeric(shortlist["model_rank"], errors="coerce").fillna(0).astype(float).reset_index(drop=True)
    setup_ids = shortlist["setup_id"].astype(str).reset_index(drop=True)
    group = score.groupby(setup_ids)
    group_max = group.transform("max")
    group_mean = group.transform("mean")
    group_std = group.transform("std").replace(0.0, np.nan).fillna(1.0)
    group_count = group.transform("size").astype(float)

    sorted_scores = pd.DataFrame(
        {
            "setup_id": setup_ids,
            "stage1_rank": rank,
            "score": score,
        }
    ).sort_values(["setup_id", "stage1_rank"], kind="mergesort")
    sorted_scores["prev_score"] = sorted_scores.groupby("setup_id")["score"].shift(1)
    sorted_scores["next_score"] = sorted_scores.groupby("setup_id")["score"].shift(-1)
    sorted_scores = sorted_scores.sort_index()
    top_two = (
        sorted_scores[sorted_scores["stage1_rank"] <= 2]
        .pivot(index="setup_id", columns="stage1_rank", values="score")
        .rename(columns={1.0: "top1", 2.0: "top2"})
    )
    top2_margin = setup_ids.map((top_two["top1"] - top_two["top2"].fillna(top_two["top1"])).to_dict()).fillna(0.0)

    result["stage1_score"] = score
    result["stage1_rank"] = rank
    result["stage1_rank_pct"] = (rank - 1.0) / (group_count - 1.0).replace(0.0, 1.0)
    result["stage1_gap_to_top"] = group_max - score
    result["stage1_score_z"] = (score - group_mean) / group_std
    result["stage1_top2_margin"] = top2_margin.astype(float).to_numpy()
    result["stage1_score_to_prev"] = score - sorted_scores["prev_score"].fillna(score).reset_index(drop=True)
    result["stage1_score_to_next"] = score - sorted_scores["next_score"].fillna(score).reset_index(drop=True)
    result["stage1_shortlist_size"] = group_count
    return result


def best_in_shortlist_labels(shortlist: pd.DataFrame) -> pd.Series:
    work = shortlist.loc[:, ["setup_id", "template_uid", "result_r"]].copy()
    work["result_r"] = pd.to_numeric(work["result_r"], errors="coerce").fillna(0.0)
    best = (
        work.sort_values(["setup_id", "result_r", "template_uid"], ascending=[True, False, True])
        .drop_duplicates("setup_id", keep="first")
        .set_index("setup_id")["template_uid"]
    )
    return (work["template_uid"].astype(str) == work["setup_id"].map(best).astype(str)).astype(int)


def advantage_labels(shortlist: pd.DataFrame) -> pd.Series:
    result = pd.to_numeric(shortlist["result_r"], errors="coerce").fillna(0.0)
    setup_avg = result.groupby(shortlist["setup_id"].astype(str)).transform("mean")
    return result - setup_avg


def numeric_series(frame: pd.DataFrame, column: str, fallback: pd.Series | float = 0.0) -> pd.Series:
    if column in frame.columns:
        series = pd.to_numeric(frame[column], errors="coerce")
    else:
        series = pd.Series(np.nan, index=frame.index)
    if isinstance(fallback, pd.Series):
        return series.fillna(fallback)
    return series.fillna(float(fallback))


def path_label_coverage(frame: pd.DataFrame) -> float:
    if "path_mfe_r" not in frame.columns:
        return 0.0
    return float(pd.to_numeric(frame["path_mfe_r"], errors="coerce").notna().mean())


def path_first_hit_labels(shortlist: pd.DataFrame) -> pd.Series:
    result = pd.to_numeric(shortlist["result_r"], errors="coerce").fillna(0.0)
    target_r = numeric_series(shortlist, "path_target_r", numeric_series(shortlist, "target_r", result.clip(lower=0.0)))
    end_close_r = numeric_series(shortlist, "path_end_close_r", result)
    first_hit = shortlist.get("path_first_hit_outcome", pd.Series([""] * len(shortlist), index=shortlist.index)).fillna("").astype(str)
    labels = result.copy()
    labels = labels.mask(first_hit == "target", target_r)
    labels = labels.mask(first_hit == "stop", -1.0)
    labels = labels.mask(first_hit == "none", end_close_r)
    return labels.astype(float)


def path_opportunity_labels(shortlist: pd.DataFrame) -> pd.Series:
    result = pd.to_numeric(shortlist["result_r"], errors="coerce").fillna(0.0)
    target_r = numeric_series(shortlist, "path_target_r", numeric_series(shortlist, "target_r", result.clip(lower=0.0))).clip(lower=0.25)
    mfe_r = numeric_series(shortlist, "path_mfe_r", result.clip(lower=0.0)).clip(lower=0.0)
    mae_r = numeric_series(shortlist, "path_mae_r", (-result).clip(lower=0.0)).clip(lower=0.0)
    end_close_r = numeric_series(shortlist, "path_end_close_r", result)
    mae_before_target = numeric_series(shortlist, "path_mae_before_target_r", mae_r).clip(lower=0.0)
    mfe_before_stop = numeric_series(shortlist, "path_mfe_before_stop_r", mfe_r).clip(lower=0.0)
    first_hit = shortlist.get("path_first_hit_outcome", pd.Series([""] * len(shortlist), index=shortlist.index)).fillna("").astype(str)

    target_label = target_r - 0.15 * mae_before_target.clip(upper=2.0)
    stop_label = -1.0 + 0.25 * mfe_before_stop.clip(upper=target_r)
    open_label = end_close_r.clip(lower=-1.0, upper=target_r) + 0.15 * mfe_r.clip(upper=target_r) - 0.15 * mae_r.clip(upper=2.0)

    labels = result.copy()
    labels = labels.mask(first_hit == "target", target_label)
    labels = labels.mask(first_hit == "stop", stop_label)
    labels = labels.mask(first_hit == "none", open_label)
    return labels.astype(float)


def train_stage2(
    args: argparse.Namespace,
    x_train: pd.DataFrame,
    train_shortlist: pd.DataFrame,
    x_valid: pd.DataFrame,
    valid_shortlist: pd.DataFrame,
) -> tuple[CatBoostClassifier | CatBoostRegressor, np.ndarray, float, float]:
    cat_indexes = [x_train.columns.get_loc(col) for col in base.CAT_FEATURES if col in x_train.columns]
    train_result = pd.to_numeric(train_shortlist["result_r"], errors="coerce").fillna(0.0).reset_index(drop=True)

    if args.stage2_objective == "best_classifier":
        train_label = best_in_shortlist_labels(train_shortlist).reset_index(drop=True)
        valid_label = best_in_shortlist_labels(valid_shortlist).reset_index(drop=True)
        train_pool = Pool(x_train, label=train_label, cat_features=cat_indexes)
        valid_pool = Pool(x_valid, label=valid_label, cat_features=cat_indexes)
        model = CatBoostClassifier(
            iterations=args.stage2_iterations,
            depth=args.stage2_depth,
            learning_rate=args.stage2_learning_rate,
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
        rmse = float(mean_squared_error(valid_label, predictions) ** 0.5)
        mae = float(mean_absolute_error(valid_label, predictions))
        return model, predictions, rmse, mae

    if args.stage2_objective == "advantage":
        train_label = advantage_labels(train_shortlist).reset_index(drop=True)
        valid_label = advantage_labels(valid_shortlist).reset_index(drop=True)
    elif args.stage2_objective == "path_first_hit":
        train_label = path_first_hit_labels(train_shortlist).reset_index(drop=True)
        valid_label = path_first_hit_labels(valid_shortlist).reset_index(drop=True)
    elif args.stage2_objective == "path_opportunity":
        train_label = path_opportunity_labels(train_shortlist).reset_index(drop=True)
        valid_label = path_opportunity_labels(valid_shortlist).reset_index(drop=True)
    else:
        train_label = train_result
        valid_label = pd.to_numeric(valid_shortlist["result_r"], errors="coerce").fillna(0.0).reset_index(drop=True)

    train_pool = Pool(x_train, label=train_label, cat_features=cat_indexes)
    valid_pool = Pool(x_valid, label=valid_label, cat_features=cat_indexes)
    model = CatBoostRegressor(
        iterations=args.stage2_iterations,
        depth=args.stage2_depth,
        learning_rate=args.stage2_learning_rate,
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
    return model, predictions, rmse, mae


def select_stage2_top(valid_shortlist: pd.DataFrame, stage2_predictions: np.ndarray) -> pd.DataFrame:
    ranked = valid_shortlist.copy()
    ranked["stage2_score"] = stage2_predictions
    ranked = ranked.sort_values(
        ["setup_id", "stage2_score", "predicted_expected_r", "template_uid"],
        ascending=[True, False, False, True],
    )
    ranked["stage2_rank"] = ranked.groupby("setup_id").cumcount() + 1
    return ranked


def main() -> int:
    args = parse_args()
    args.results_table = base.safe_identifier(args.results_table)
    top_ks = [int(part.strip()) for part in args.top_k.split(",") if part.strip()]
    run_id = args.run_id or make_run_id()

    conn = base.connect()
    try:
        base.ensure_tables(conn)
        ranking.ensure_tables(conn)
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

        print(f"AI reranker eval: {run_id}")
        print(f"Source run: {args.source_run_id}")
        print(f"Results table: {args.results_table}")
        print(f"Train years: {args.train_start_year}-{args.train_end_year}")
        print(f"Train setups: {len(train_setup_ids)}")
        print(f"Valid setups: {len(valid_setup_ids)}")
        print(f"Shortlist: top {args.rerank_top_n}")
        print(f"Stage 2 objective: {args.stage2_objective}")

        train_df = base.load_candidate_rows(conn, args.results_table, args.source_run_id, train_setup_ids)
        valid_df = base.load_candidate_rows(conn, args.results_table, args.source_run_id, valid_setup_ids)
        if train_df.empty or valid_df.empty:
            raise RuntimeError("Training or validation rows were empty")

        print(f"Train rows: {len(train_df):,}")
        print(f"Valid rows: {len(valid_df):,}")

        x_train, x_valid, train_result_r, _ = build_stage_features(args, train_df, valid_df)
        stage1_model, train_stage1_fit_predictions, valid_stage1_predictions = train_stage1(
            args,
            x_train,
            train_result_r,
            x_valid,
        )
        if args.stage1_oof_folds and args.stage1_oof_folds > 1:
            train_stage1_predictions = train_stage1_oof_predictions(
                args,
                x_train,
                train_result_r,
                train_df["setup_id"],
            )
        else:
            train_stage1_predictions = train_stage1_fit_predictions

        train_ranked = build_ranked_with_index(train_df, train_stage1_predictions)
        valid_ranked = build_ranked_with_index(valid_df, valid_stage1_predictions)
        train_shortlist = shortlist_frame(train_ranked, args.rerank_top_n)
        valid_shortlist = shortlist_frame(valid_ranked, args.rerank_top_n)
        if args.stage2_objective.startswith("path_"):
            train_coverage = path_label_coverage(train_shortlist)
            valid_coverage = path_label_coverage(valid_shortlist)
            print(f"Path label coverage: train {train_coverage:.2%}, valid {valid_coverage:.2%}")
            if train_coverage < 0.95 or valid_coverage < 0.95:
                print("Warning: path objective is falling back to engine result_r for missing path labels.")

        train_short_x = x_train.iloc[train_shortlist["feature_row_index"].astype(int)].reset_index(drop=True)
        valid_short_x = x_valid.iloc[valid_shortlist["feature_row_index"].astype(int)].reset_index(drop=True)
        train_rerank_x = add_rerank_meta_features(train_short_x, train_shortlist)
        valid_rerank_x = add_rerank_meta_features(valid_short_x, valid_shortlist)

        stage2_model, stage2_predictions, stage2_rmse, stage2_mae = train_stage2(
            args,
            train_rerank_x,
            train_shortlist,
            valid_rerank_x,
            valid_shortlist,
        )
        reranked_valid = select_stage2_top(valid_shortlist, stage2_predictions)

        stage1_top_one = valid_ranked[valid_ranked["model_rank"] == 1].copy()
        reranker_top_one = reranked_valid[reranked_valid["stage2_rank"] == 1].copy()
        best_template_uid, best_template_avg_r = ranking.best_train_template(train_df)
        baseline = valid_ranked[valid_ranked["template_uid"] == best_template_uid].copy()
        oracle_all = valid_ranked.drop_duplicates("setup_id", keep="first").copy()
        oracle_all["result_r"] = oracle_all["oracle_result_r"]
        shortlist_oracle = (
            valid_shortlist.sort_values(["setup_id", "result_r", "template_uid"], ascending=[True, False, True])
            .drop_duplicates("setup_id", keep="first")
        )

        model_dir = base.ABCD_ROOT / "tmp_logs"
        model_dir.mkdir(parents=True, exist_ok=True)
        stage1_path = model_dir / f"{run_id}.stage1.cbm"
        stage2_path = model_dir / f"{run_id}.stage2.cbm"
        stage1_model.save_model(str(stage1_path))
        stage2_model.save_model(str(stage2_path))

        with conn.cursor() as cur:
            cur.execute(
                """
                INSERT INTO ai_build_reranker_eval_runs (
                    reranker_eval_run_id, source_run_id, results_table,
                    train_start_year, train_end_year, valid_year,
                    train_setups, valid_setups, train_rows, valid_rows,
                    rerank_top_n, pre_feature_set, aggregate_feature_set,
                    stage2_objective, stage1_oof_folds, stage1_model_path, stage2_model_path,
                    stage2_rmse, stage2_mae
                )
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
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
                    args.rerank_top_n,
                    args.pre_feature_set,
                    None if args.skip_aggregate_features else args.aggregate_feature_set,
                    args.stage2_objective,
                    int(args.stage1_oof_folds or 0),
                    str(stage1_path),
                    str(stage2_path),
                    stage2_rmse,
                    stage2_mae,
                ),
            )

        summary_items = [
            ("stage1_top_1", 1, ranking.summarize(stage1_top_one["result_r"])),
            ("stage2_top_1", 1, ranking.summarize(reranker_top_one["result_r"])),
            ("best_train_template", None, ranking.summarize(baseline["result_r"])),
            ("oracle_best_all", None, ranking.summarize(oracle_all["result_r"])),
            (f"oracle_within_stage1_top_{args.rerank_top_n}", args.rerank_top_n, ranking.summarize(shortlist_oracle["result_r"])),
        ]
        for metric_name, top_k, stats in summary_items:
            insert_summary(conn, run_id, metric_name, top_k, stats)

        for top_k in top_ks:
            top_k_frame = reranked_valid[reranked_valid["stage2_rank"] <= top_k].copy()
            top_k_best = (
                top_k_frame.sort_values(["setup_id", "result_r", "template_uid"], ascending=[True, False, True])
                .drop_duplicates("setup_id", keep="first")
            )
            insert_summary(conn, run_id, "best_actual_within_stage2_top_k", top_k, ranking.summarize(top_k_best["result_r"]))

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
            "stage2_rank",
            "stage2_score",
            "result_r",
            "outcome",
            "oracle_template_uid",
            "oracle_result_r",
            "oracle_rank",
        ]
        rows = []
        for row in reranker_top_one[selected_cols].itertuples(index=False):
            rows.append((run_id, *[base.clean_db_value(value) for value in row]))
        if rows:
            with conn.cursor() as cur:
                cur.executemany(
                    """
                    INSERT INTO ai_build_reranker_eval_selected (
                        reranker_eval_run_id, setup_id, pattern_id, pattern_group_id,
                        symbol, market, pattern_family_key, d_confirm_date,
                        template_uid, template_name, stage1_rank, stage1_score,
                        stage2_rank, stage2_score, actual_result_r, actual_outcome,
                        oracle_template_uid, oracle_result_r, oracle_rank
                    )
                    VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                    """,
                    rows,
                )
        conn.commit()

        print("\nTwo-stage summary:")
        print(f"  stage2 score RMSE/MAE: {stage2_rmse:.4f} / {stage2_mae:.4f}")
        print(f"  best train template: {best_template_uid} ({best_template_avg_r:.4f}R train avg)")
        for label, frame in [
            ("Stage 1 top 1", stage1_top_one),
            ("Stage 2 reranker top 1", reranker_top_one),
            ("Best train template", baseline),
            ("Oracle all", oracle_all),
            (f"Oracle inside Stage 1 top {args.rerank_top_n}", shortlist_oracle),
        ]:
            stats = ranking.summarize(frame["result_r"])
            print(
                f"  {label}: {stats['setup_count']} trades, "
                f"{stats['win_rate']:.2%} win, {stats['avg_r']:.3f}R avg, {stats['sum_r']:.1f}R sum"
            )

        print("\nStage 2 ranking quality:")
        for top_k in top_ks:
            top_k_frame = reranked_valid[reranked_valid["stage2_rank"] <= top_k].copy()
            top_k_best = (
                top_k_frame.sort_values(["setup_id", "result_r", "template_uid"], ascending=[True, False, True])
                .drop_duplicates("setup_id", keep="first")
            )
            stats = ranking.summarize(top_k_best["result_r"])
            print(f"  Top {top_k}: best actual within stage2 top {top_k}: {stats['avg_r']:.3f}R avg, {stats['sum_r']:.1f}R sum")

        print(f"\nStored reranker eval: {run_id}")
        print(f"  stage1 model: {stage1_path}")
        print(f"  stage2 model: {stage2_path}")
        return 0
    finally:
        conn.close()


if __name__ == "__main__":
    sys.exit(main())
