#!/usr/bin/env python3
"""
Train one Stage 1 expected-R model and evaluate it across multiple validation slots.

This is meant for robustness checks: train once, score several held-out setup
samples, store per-slot summaries, combined confidence-filter summaries, and the
selected top trade per setup.
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
    parser.add_argument("--valid-sample-slots", default="0,1,2,3,4,5,6")
    parser.add_argument("--pre-feature-set", choices=["all", "core", "shape", "candidate", "none"], default="none")
    parser.add_argument("--aggregate-feature-set", choices=["basic", "all"], default="all")
    parser.add_argument("--skip-aggregate-features", action="store_true")
    parser.add_argument("--iterations", type=int, default=160)
    parser.add_argument("--depth", type=int, default=6)
    parser.add_argument("--learning-rate", type=float, default=0.035)
    parser.add_argument("--l2-leaf-reg", type=float, default=3.0)
    parser.add_argument("--random-strength", type=float, default=1.0)
    parser.add_argument("--random-seed", type=int, default=42)
    parser.add_argument("--min-rule-trades", type=int, default=1000)
    parser.add_argument("--exclude-roots", default="", help="Comma-separated root symbols to skip in train and validation rows.")
    parser.add_argument("--slippage-entry-ticks", type=float, default=0.0)
    parser.add_argument("--slippage-exit-ticks", type=float, default=0.0)
    parser.add_argument("--slippage-winner-exit-ticks", type=float, default=None)
    parser.add_argument("--slippage-loser-exit-ticks", type=float, default=None)
    parser.add_argument("--min-target-ticks", type=float, default=0.0)
    parser.add_argument("--min-risk-ticks", type=float, default=0.0)
    parser.add_argument("--run-id", default=None)
    return parser.parse_args()


def make_run_id() -> str:
    return f"aimv-{int(time.time() * 1000)}-{os.getpid()}"


def ensure_tables(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_stage1_multi_valid_eval_runs (
                multi_valid_eval_run_id VARCHAR(64) PRIMARY KEY,
                source_run_id VARCHAR(64) NOT NULL,
                results_table VARCHAR(128) NOT NULL,
                train_start_year INT NOT NULL,
                train_end_year INT NOT NULL,
                valid_year INT NOT NULL,
                train_sample_slot INT NOT NULL,
                valid_sample_slots VARCHAR(255) NOT NULL,
                train_setups BIGINT NOT NULL,
                train_rows BIGINT NOT NULL,
                iterations INT NOT NULL,
                depth INT NOT NULL,
                learning_rate DOUBLE NOT NULL,
                l2_leaf_reg DOUBLE NOT NULL,
                random_strength DOUBLE NOT NULL,
                random_seed INT NOT NULL,
                pre_feature_set VARCHAR(32) NOT NULL,
                aggregate_feature_set VARCHAR(32) NULL,
                excluded_roots VARCHAR(255) NULL,
                model_path VARCHAR(255) NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            """
        )
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_stage1_multi_valid_eval_slot_summary (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                multi_valid_eval_run_id VARCHAR(64) NOT NULL,
                valid_sample_slot INT NULL,
                metric_name VARCHAR(64) NOT NULL,
                setup_count BIGINT NOT NULL,
                wins BIGINT NOT NULL,
                losses BIGINT NOT NULL,
                win_rate DOUBLE NULL,
                avg_r DOUBLE NULL,
                sum_r DOUBLE NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_aimv_slot_summary_run (multi_valid_eval_run_id, valid_sample_slot, metric_name)
            )
            """
        )
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_stage1_multi_valid_eval_filter_summary (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                multi_valid_eval_run_id VARCHAR(64) NOT NULL,
                scope_name VARCHAR(64) NOT NULL,
                rule_name VARCHAR(64) NOT NULL,
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
                INDEX idx_aimv_filter_run (multi_valid_eval_run_id, scope_name, sum_r)
            )
            """
        )
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_stage1_multi_valid_eval_selected (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                multi_valid_eval_run_id VARCHAR(64) NOT NULL,
                valid_sample_slot INT NOT NULL,
                setup_id VARCHAR(64) NOT NULL,
                pattern_id VARCHAR(64) NULL,
                pattern_group_id VARCHAR(64) NULL,
                symbol VARCHAR(32) NULL,
                market VARCHAR(32) NULL,
                pattern_family_key VARCHAR(255) NULL,
                d_confirm_date DATETIME NULL,
                template_uid VARCHAR(128) NULL,
                template_name VARCHAR(255) NULL,
                model_rank INT NULL,
                predicted_expected_r DOUBLE NULL,
                score_margin_top2 DOUBLE NULL,
                actual_result_r DOUBLE NULL,
                actual_outcome VARCHAR(64) NULL,
                oracle_template_uid VARCHAR(128) NULL,
                oracle_result_r DOUBLE NULL,
                oracle_rank INT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_aimv_selected_run (multi_valid_eval_run_id, valid_sample_slot),
                INDEX idx_aimv_selected_setup (setup_id)
            )
            """
        )
    conn.commit()
    base.ensure_column(conn, "ai_stage1_multi_valid_eval_runs", "excluded_roots", "VARCHAR(255) NULL")
    base.ensure_column(conn, "ai_stage1_multi_valid_eval_runs", "slippage_entry_ticks", "DOUBLE NULL")
    base.ensure_column(conn, "ai_stage1_multi_valid_eval_runs", "slippage_exit_ticks", "DOUBLE NULL")
    base.ensure_column(conn, "ai_stage1_multi_valid_eval_runs", "slippage_winner_exit_ticks", "DOUBLE NULL")
    base.ensure_column(conn, "ai_stage1_multi_valid_eval_runs", "slippage_loser_exit_ticks", "DOUBLE NULL")
    base.ensure_column(conn, "ai_stage1_multi_valid_eval_runs", "min_target_ticks", "DOUBLE NULL")
    base.ensure_column(conn, "ai_stage1_multi_valid_eval_runs", "min_risk_ticks", "DOUBLE NULL")
    conn.commit()


def parse_excluded_roots(value: str) -> set[str]:
    return {part.strip().upper() for part in value.split(",") if part.strip()}


def filter_excluded_roots(frame: pd.DataFrame, excluded_roots: set[str]) -> pd.DataFrame:
    if not excluded_roots or frame.empty:
        return frame
    roots = frame["symbol"].map(base.root_symbol).str.upper()
    return frame[~roots.isin(excluded_roots)].copy()


def apply_trade_costs_and_filters(frame: pd.DataFrame, args: argparse.Namespace) -> pd.DataFrame:
    if frame.empty:
        return frame

    work = frame.copy()
    work["root_symbol"] = work["symbol"].map(base.root_symbol)
    base.add_candidate_structure_features(work)

    target_ticks = pd.to_numeric(work.get("target_ticks"), errors="coerce").fillna(0.0)
    risk_ticks = pd.to_numeric(work.get("risk_ticks"), errors="coerce").fillna(0.0)
    if args.min_target_ticks > 0:
        work = work[target_ticks >= args.min_target_ticks].copy()
    if args.min_risk_ticks > 0:
        work = work[risk_ticks >= args.min_risk_ticks].copy()
    if work.empty:
        return work

    entry_ticks = float(args.slippage_entry_ticks or 0.0)
    default_exit_ticks = float(args.slippage_exit_ticks or 0.0)
    winner_exit_ticks = (
        default_exit_ticks
        if getattr(args, "slippage_winner_exit_ticks", None) is None
        else float(args.slippage_winner_exit_ticks or 0.0)
    )
    loser_exit_ticks = (
        default_exit_ticks
        if getattr(args, "slippage_loser_exit_ticks", None) is None
        else float(args.slippage_loser_exit_ticks or 0.0)
    )
    if max(entry_ticks, winner_exit_ticks, loser_exit_ticks) <= 0:
        return work

    tick_size = pd.to_numeric(work.get("tick_size"), errors="coerce").fillna(0.0)
    risk_points = pd.to_numeric(work.get("risk_points"), errors="coerce").replace(0.0, np.nan)
    clean_result_r = pd.to_numeric(work["result_r"], errors="coerce").fillna(0.0)
    exit_ticks = np.where(clean_result_r > 0.0, winner_exit_ticks, loser_exit_ticks)
    total_slippage_ticks = entry_ticks + exit_ticks
    slippage_cost_r = ((total_slippage_ticks * tick_size) / risk_points).replace([np.inf, -np.inf], np.nan).fillna(0.0)
    adjusted_result_r = clean_result_r - slippage_cost_r

    work["clean_result_r"] = clean_result_r
    work["slippage_cost_r"] = slippage_cost_r
    work["result_r"] = adjusted_result_r
    work["outcome"] = np.where(adjusted_result_r > 0.0, "pass", "fail")
    return work


def insert_slot_summary(conn, run_id: str, slot: int | None, metric_name: str, stats: dict) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            INSERT INTO ai_stage1_multi_valid_eval_slot_summary (
                multi_valid_eval_run_id, valid_sample_slot, metric_name,
                setup_count, wins, losses, win_rate, avg_r, sum_r
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)
            """,
            (
                run_id,
                slot,
                metric_name,
                stats["setup_count"],
                stats["wins"],
                stats["losses"],
                stats["win_rate"],
                stats["avg_r"],
                stats["sum_r"],
            ),
        )


def insert_filter_rows(conn, run_id: str, scope_name: str, rows: list[dict[str, object]]) -> None:
    payload = []
    for row in rows:
        payload.append(
            (
                run_id,
                scope_name,
                row["rule_name"],
                row["score_quantile"],
                row["margin_quantile"],
                row["score_threshold"],
                row["margin_threshold"],
                row["setup_count"],
                row["wins"],
                row["losses"],
                row["win_rate"],
                row["avg_r"],
                row["sum_r"],
            )
        )
    if not payload:
        return
    with conn.cursor() as cur:
        cur.executemany(
            """
            INSERT INTO ai_stage1_multi_valid_eval_filter_summary (
                multi_valid_eval_run_id, scope_name, rule_name,
                score_quantile, margin_quantile, score_threshold, margin_threshold,
                selected_trades, wins, losses, win_rate, avg_r, sum_r
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            """,
            payload,
        )


def insert_selected(conn, run_id: str, slot: int, top_one: pd.DataFrame) -> None:
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
        "score_margin_top2",
        "result_r",
        "outcome",
        "oracle_template_uid",
        "oracle_result_r",
        "oracle_rank",
    ]
    rows = []
    for row in top_one[selected_cols].itertuples(index=False):
        rows.append((run_id, slot, *[base.clean_db_value(value) for value in row]))
    if not rows:
        return
    with conn.cursor() as cur:
        cur.executemany(
            """
            INSERT INTO ai_stage1_multi_valid_eval_selected (
                multi_valid_eval_run_id, valid_sample_slot, setup_id,
                pattern_id, pattern_group_id, symbol, market, pattern_family_key,
                d_confirm_date, template_uid, template_name, model_rank,
                predicted_expected_r, score_margin_top2, actual_result_r,
                actual_outcome, oracle_template_uid, oracle_result_r, oracle_rank
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            """,
            rows,
        )


def train_model(args: argparse.Namespace, x_train: pd.DataFrame, train_result_r: pd.Series) -> CatBoostRegressor:
    cat_indexes = [x_train.columns.get_loc(col) for col in base.CAT_FEATURES if col in x_train.columns]
    train_pool = Pool(x_train, label=train_result_r, cat_features=cat_indexes)
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
    model.fit(train_pool, use_best_model=False)
    return model


def prepare_feature_frames(args: argparse.Namespace, train_df: pd.DataFrame, valid_df: pd.DataFrame) -> tuple[pd.DataFrame, pd.Series, pd.DataFrame]:
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
    x_valid = x_valid.reindex(columns=x_train.columns)
    return x_train, train_result_r, x_valid


def best_filter(rows: list[dict[str, object]], min_trades: int, sort_key: str) -> dict[str, object] | None:
    eligible = [row for row in rows if int(row["setup_count"]) >= min_trades]
    if not eligible:
        return None
    return max(
        eligible,
        key=lambda row: (
            float(row[sort_key] or -999999.0),
            float(row["sum_r"] or -999999.0),
            int(row["setup_count"]),
        ),
    )


def main() -> int:
    args = parse_args()
    args.results_table = base.safe_identifier(args.results_table)
    slots = [int(part.strip()) for part in args.valid_sample_slots.split(",") if part.strip()]
    excluded_roots = parse_excluded_roots(args.exclude_roots)
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
        print(f"AI Stage 1 multi-valid eval: {run_id}")
        print(f"Train setups: {len(train_setup_ids):,}")
        print(f"Valid slots: {slots}")

        train_df = base.load_candidate_rows(conn, args.results_table, args.source_run_id, train_setup_ids)
        train_df = filter_excluded_roots(train_df, excluded_roots)
        train_df = apply_trade_costs_and_filters(train_df, args)
        if train_df.empty:
            raise RuntimeError("Training rows were empty")
        print(f"Train rows: {len(train_df):,}")
        if excluded_roots:
            print(f"Excluded roots: {', '.join(sorted(excluded_roots))}")
        if args.slippage_entry_ticks or args.slippage_exit_ticks or args.slippage_winner_exit_ticks is not None or args.slippage_loser_exit_ticks is not None:
            winner_exit = args.slippage_exit_ticks if args.slippage_winner_exit_ticks is None else args.slippage_winner_exit_ticks
            loser_exit = args.slippage_exit_ticks if args.slippage_loser_exit_ticks is None else args.slippage_loser_exit_ticks
            print(
                "Training target: net R after "
                f"entry={args.slippage_entry_ticks:g}, winner_exit={winner_exit:g}, loser_exit={loser_exit:g} ticks"
            )
        if args.min_target_ticks or args.min_risk_ticks:
            print(f"Candidate filters: min target ticks={args.min_target_ticks:g}, min risk ticks={args.min_risk_ticks:g}")

        train_features, train_result_r, _ = prepare_feature_frames(args, train_df, train_df.head(0).copy())
        print("\nTraining model")
        model = train_model(args, train_features, train_result_r)

        model_dir = base.ABCD_ROOT / "tmp_logs"
        model_dir.mkdir(parents=True, exist_ok=True)
        model_path = model_dir / f"{run_id}.cbm"
        model.save_model(str(model_path))

        all_top_one = []
        cat_indexes = [train_features.columns.get_loc(col) for col in base.CAT_FEATURES if col in train_features.columns]
        for slot in slots:
            valid_setup_ids = base.load_setup_ids(
                conn,
                args.results_table,
                args.source_run_id,
                args.valid_year,
                args.valid_setups,
                args.setup_sample_mod,
                slot,
                reference_template,
            )
            valid_df = base.load_candidate_rows(conn, args.results_table, args.source_run_id, valid_setup_ids)
            valid_df = filter_excluded_roots(valid_df, excluded_roots)
            valid_df = apply_trade_costs_and_filters(valid_df, args)
            if valid_df.empty:
                print(f"\nSlot {slot}: no validation rows")
                continue
            _, _, x_valid = prepare_feature_frames(args, train_df, valid_df)
            valid_pool = Pool(x_valid, cat_features=cat_indexes)
            predictions = model.predict(valid_pool)
            ranked = ranking.build_ranked_frame(valid_df.assign(predicted_expected_r=predictions))
            top_one = ranked[ranked["model_rank"] == 1].copy()
            top_one["valid_sample_slot"] = slot
            stats = ranking.summarize(top_one["result_r"])
            insert_slot_summary(conn, run_id, slot, "ai_top_1", stats)
            insert_filter_rows(conn, run_id, f"slot_{slot}", ranking.evaluate_filter_rules(top_one))
            insert_selected(conn, run_id, slot, top_one)
            all_top_one.append(top_one)
            conn.commit()
            print(
                f"  slot {slot}: {stats['setup_count']} trades, {stats['win_rate']:.2%} win, "
                f"{stats['avg_r']:.3f}R avg, {stats['sum_r']:.1f}R sum"
            )

        combined = pd.concat(all_top_one, ignore_index=True) if all_top_one else pd.DataFrame()
        combined_stats = ranking.summarize(combined["result_r"] if not combined.empty else pd.Series(dtype=float))
        insert_slot_summary(conn, run_id, None, "combined_ai_top_1", combined_stats)
        combined_filter_rows = ranking.evaluate_filter_rules(combined) if not combined.empty else []
        insert_filter_rows(conn, run_id, "combined", combined_filter_rows)

        with conn.cursor() as cur:
            cur.execute(
                """
                INSERT INTO ai_stage1_multi_valid_eval_runs (
                    multi_valid_eval_run_id, source_run_id, results_table,
                    train_start_year, train_end_year, valid_year,
                    train_sample_slot, valid_sample_slots,
                    train_setups, train_rows, iterations, depth, learning_rate,
                    l2_leaf_reg, random_strength, random_seed,
                    pre_feature_set, aggregate_feature_set, excluded_roots,
                    slippage_entry_ticks, slippage_exit_ticks,
                    slippage_winner_exit_ticks, slippage_loser_exit_ticks,
                    min_target_ticks, min_risk_ticks,
                    model_path
                )
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                """,
                (
                    run_id,
                    args.source_run_id,
                    args.results_table,
                    args.train_start_year,
                    args.train_end_year,
                    args.valid_year,
                    args.train_sample_slot,
                    ",".join(str(slot) for slot in slots),
                    len(train_setup_ids),
                    len(train_df),
                    args.iterations,
                    args.depth,
                    args.learning_rate,
                    args.l2_leaf_reg,
                    args.random_strength,
                    args.random_seed,
                    args.pre_feature_set,
                    None if args.skip_aggregate_features else args.aggregate_feature_set,
                    ",".join(sorted(excluded_roots)) if excluded_roots else None,
                    args.slippage_entry_ticks,
                    args.slippage_exit_ticks,
                    args.slippage_winner_exit_ticks,
                    args.slippage_loser_exit_ticks,
                    args.min_target_ticks,
                    args.min_risk_ticks,
                    str(model_path),
                ),
            )
        conn.commit()

        print("\nCombined summary:")
        print(
            f"  all slots: {combined_stats['setup_count']} trades, {combined_stats['win_rate']:.2%} win, "
            f"{combined_stats['avg_r']:.3f}R avg, {combined_stats['sum_r']:.1f}R sum"
        )
        best_total = best_filter(combined_filter_rows, args.min_rule_trades, "sum_r")
        best_avg = best_filter(combined_filter_rows, args.min_rule_trades, "avg_r")
        if best_total:
            print(
                f"  best filter by total: {best_total['rule_name']} score q{best_total['score_quantile']:.2f}, "
                f"margin {best_total['margin_quantile']}; {best_total['setup_count']} trades, "
                f"{best_total['avg_r']:.3f}R avg, {best_total['sum_r']:.1f}R sum"
            )
        if best_avg:
            print(
                f"  best filter by avg: {best_avg['rule_name']} score q{best_avg['score_quantile']:.2f}, "
                f"margin {best_avg['margin_quantile']}; {best_avg['setup_count']} trades, "
                f"{best_avg['avg_r']:.3f}R avg, {best_avg['sum_r']:.1f}R sum"
            )
        print(f"\nStored multi-valid eval: {run_id}")
        print(f"  model: {model_path}")
        return 0
    finally:
        conn.close()


if __name__ == "__main__":
    sys.exit(main())
