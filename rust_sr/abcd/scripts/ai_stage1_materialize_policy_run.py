#!/usr/bin/env python3
"""
Create a derived AI Stage 1 policy run from an existing multi-valid run.

The source run keeps every Stage 1 top pick. The derived policy run keeps only
rows that pass a score quantile and skips templates listed in metadata.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import pandas as pd

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_build_search_catboost as base


TRADE_DERIVED_TABLES = [
    "ai_stage1_trade_loss_windows",
    "ai_stage1_trade_family_contribution",
    "ai_stage1_trade_symbol_contribution",
    "ai_stage1_trade_workload",
    "ai_stage1_trade_cadence",
    "ai_stage1_trade_hourly",
    "ai_stage1_trade_daily_r",
    "ai_stage1_trade_template_performance",
    "ai_stage1_trade_summary",
    "ai_stage1_trade_rows",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-run-id", required=True)
    parser.add_argument("--policy-run-id", required=True)
    parser.add_argument("--metadata-json", required=True)
    parser.add_argument("--score-quantile", type=float, default=0.95)
    parser.add_argument("--model-path", default=None)
    parser.add_argument("--replace", action="store_true")
    return parser.parse_args()


def load_skip_templates(path: Path) -> list[str]:
    metadata = json.loads(path.read_text())
    policy = metadata.get("recommended_policy") or {}
    templates = policy.get("skip_template_names") or []
    return [str(value) for value in templates]


def delete_policy_rows(conn, policy_run_id: str) -> None:
    with conn.cursor() as cur:
        for table_name in [
            *TRADE_DERIVED_TABLES,
            "ai_stage1_multi_valid_eval_selected",
            "ai_stage1_multi_valid_eval_filter_summary",
            "ai_stage1_multi_valid_eval_slot_summary",
            "ai_stage1_multi_valid_eval_runs",
        ]:
            cur.execute(f"DELETE FROM {table_name} WHERE multi_valid_eval_run_id = %s", (policy_run_id,))


def insert_run_record(conn, source_run_id: str, policy_run_id: str, model_path: str | None) -> None:
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
                min_target_ticks, min_risk_ticks, model_path
            )
            SELECT
                %s, source_run_id, results_table,
                train_start_year, train_end_year, valid_year,
                train_sample_slot, 'top5_score_skip_bad_templates',
                train_setups, train_rows, iterations, depth, learning_rate,
                l2_leaf_reg, random_strength, random_seed,
                pre_feature_set, aggregate_feature_set, excluded_roots,
                slippage_entry_ticks, slippage_exit_ticks,
                slippage_winner_exit_ticks, slippage_loser_exit_ticks,
                min_target_ticks, min_risk_ticks, COALESCE(%s, model_path)
            FROM ai_stage1_multi_valid_eval_runs
            WHERE multi_valid_eval_run_id = %s
            """,
            (policy_run_id, model_path, source_run_id),
        )
        if cur.rowcount != 1:
            raise RuntimeError(f"Source run was not found or was not unique: {source_run_id}")


def selected_source_rows(conn, source_run_id: str) -> pd.DataFrame:
    sql = """
        SELECT
            valid_sample_slot,
            setup_id,
            pattern_id,
            pattern_group_id,
            symbol,
            market,
            pattern_family_key,
            d_confirm_date,
            template_uid,
            template_name,
            model_rank,
            predicted_expected_r,
            score_margin_top2,
            actual_result_r,
            actual_outcome,
            oracle_template_uid,
            oracle_result_r,
            oracle_rank
        FROM ai_stage1_multi_valid_eval_selected
        WHERE multi_valid_eval_run_id = %s
    """
    with conn.cursor() as cur:
        cur.execute(sql, (source_run_id,))
        frame = pd.DataFrame(cur.fetchall())
    if frame.empty:
        raise RuntimeError(f"No selected rows found for source run: {source_run_id}")
    frame["predicted_expected_r"] = pd.to_numeric(frame["predicted_expected_r"], errors="coerce").fillna(0.0)
    frame["actual_result_r"] = pd.to_numeric(frame["actual_result_r"], errors="coerce").fillna(0.0)
    return frame


def insert_selected_rows(conn, policy_run_id: str, frame: pd.DataFrame) -> None:
    rows = [(policy_run_id, *row) for row in frame.itertuples(index=False, name=None)]
    with conn.cursor() as cur:
        cur.executemany(
            """
            INSERT INTO ai_stage1_multi_valid_eval_selected (
                multi_valid_eval_run_id, valid_sample_slot, setup_id, pattern_id,
                pattern_group_id, symbol, market, pattern_family_key,
                d_confirm_date, template_uid, template_name, model_rank,
                predicted_expected_r, score_margin_top2, actual_result_r,
                actual_outcome, oracle_template_uid, oracle_result_r, oracle_rank
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            """,
            rows,
        )


def insert_summary(conn, policy_run_id: str, frame: pd.DataFrame) -> None:
    result = pd.to_numeric(frame["actual_result_r"], errors="coerce").fillna(0.0)
    total = float(result.sum())
    count = int(len(result))
    wins = int((result > 0.0).sum())
    losses = count - wins
    with conn.cursor() as cur:
        cur.execute(
            """
            INSERT INTO ai_stage1_multi_valid_eval_slot_summary (
                multi_valid_eval_run_id, valid_sample_slot, metric_name,
                setup_count, wins, losses, win_rate, avg_r, sum_r
            )
            VALUES (%s, NULL, 'policy_top5_skip_bad_templates', %s, %s, %s, %s, %s, %s)
            """,
            (
                policy_run_id,
                count,
                wins,
                losses,
                wins / count if count else 0.0,
                total / count if count else 0.0,
                total,
            ),
        )


def main() -> int:
    args = parse_args()
    metadata_path = Path(args.metadata_json)
    skip_templates = set(load_skip_templates(metadata_path))

    conn = base.connect()
    try:
        with conn.cursor() as cur:
            cur.execute(
                "SELECT COUNT(*) AS count FROM ai_stage1_multi_valid_eval_runs WHERE multi_valid_eval_run_id = %s",
                (args.policy_run_id,),
            )
            exists = int(cur.fetchone()["count"]) > 0
        if exists and not args.replace:
            raise RuntimeError(f"Policy run already exists. Use --replace to rebuild it: {args.policy_run_id}")

        frame = selected_source_rows(conn, args.source_run_id)
        threshold = float(frame["predicted_expected_r"].quantile(args.score_quantile))
        selected = frame[frame["predicted_expected_r"] >= threshold].copy()
        selected = selected[~selected["template_name"].fillna("").isin(skip_templates)].copy()

        delete_policy_rows(conn, args.policy_run_id)
        insert_run_record(conn, args.source_run_id, args.policy_run_id, args.model_path)
        insert_selected_rows(conn, args.policy_run_id, selected)
        insert_summary(conn, args.policy_run_id, selected)
        conn.commit()

        result = pd.to_numeric(selected["actual_result_r"], errors="coerce").fillna(0.0)
        wins = int((result > 0.0).sum())
        print(f"Policy run: {args.policy_run_id}")
        print(f"Source run: {args.source_run_id}")
        print(f"Score quantile: {args.score_quantile:.2f} threshold={threshold:.6f}")
        print(f"Selected rows: {len(selected):,}")
        print(
            f"Result: {wins:,} wins / {len(selected) - wins:,} losses, "
            f"{(wins / len(selected) * 100 if len(selected) else 0):.2f}% win, "
            f"{(float(result.mean()) if len(selected) else 0.0):.3f}R avg, {float(result.sum()):.1f}R sum"
        )
        return 0
    except Exception:
        conn.rollback()
        raise
    finally:
        conn.close()


if __name__ == "__main__":
    raise SystemExit(main())
