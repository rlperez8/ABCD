#!/usr/bin/env python3
"""
Create derived AI Stage 1 runs from an existing AI trade run plus stored market context.

This is for objective post-model policy filters such as "skip trades that agree
with the 4h trend". The derived run keeps the same raw result table/source run,
but only copies selected rows that pass the context rule. The normal AI Trades
API can then rebuild summary, workload, contribution, and raw trade sections.
"""

from __future__ import annotations

import argparse
import math
import os
import re
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

import pandas as pd
import pymysql


ABCD_ROOT = Path(__file__).resolve().parents[1]

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

SELECTED_COLUMNS = [
    "valid_sample_slot",
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
    "actual_result_r",
    "actual_outcome",
    "oracle_template_uid",
    "oracle_result_r",
    "oracle_rank",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-prefix", default="aimv-market-entry-ensemble")
    parser.add_argument("--policy-prefix", default="aimv-market-entry-context-v3")
    parser.add_argument("--years", default="2022,2023,2024,2025,2026")
    parser.add_argument(
        "--policy",
        choices=[
            "4h-not-with-trend",
            "4h-not-with-trend-skip-08-13",
            "4h-not-with-trend-skip-weak-templates",
            "4h-not-with-trend-skip-weak-templates-structure-room",
            "4h-not-with-trend-skip-weak-templates-slope-volume",
            "4h-not-with-trend-skip-weak-templates-slope-volume-skip-pattern-c2-2r",
        ],
        default="4h-not-with-trend",
    )
    parser.add_argument("--replace", action="store_true")
    return parser.parse_args()


def read_database_url() -> str:
    if os.environ.get("DATABASE_URL"):
        return os.environ["DATABASE_URL"]
    if os.environ.get("ABCD_DATABASE_URL"):
        return os.environ["ABCD_DATABASE_URL"]

    env_path = ABCD_ROOT / ".env"
    if not env_path.exists():
        raise FileNotFoundError(f"Could not find DATABASE_URL or {env_path}")
    for line in env_path.read_text().splitlines():
        match = re.match(r"\s*(?:DATABASE_URL|ABCD_DATABASE_URL)\s*=\s*(.+?)\s*$", line)
        if match:
            return match.group(1).strip().strip('"').strip("'")
    raise ValueError(f"DATABASE_URL was not found in {env_path}")


def connect() -> pymysql.connections.Connection:
    parsed = urlparse(read_database_url())
    return pymysql.connect(
        host=parsed.hostname,
        user=parsed.username,
        password=parsed.password,
        database=parsed.path.lstrip("/"),
        port=parsed.port or 3306,
        autocommit=False,
        cursorclass=pymysql.cursors.DictCursor,
    )


def clean(value: Any) -> Any:
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    if pd.isna(value):
        return None
    return value


def years_from_arg(value: str) -> list[int]:
    years = [int(part.strip()) for part in value.split(",") if part.strip()]
    if not years:
        raise ValueError("At least one year is required")
    return years


def safe_run_id(prefix: str, year: int) -> str:
    value = f"{prefix}-{year}"
    if len(value) > 64:
        raise ValueError(f"Run id is too long for DB column: {value}")
    return value


def policy_sql(policy: str) -> tuple[str, str, str]:
    not_with_4h = """
        (
            c4.trend_label IS NULL
            OR c4.trend_label = 'neutral'
            OR NOT (
            (LOWER(COALESCE(c4.trade_direction, '')) = 'long' AND c4.trend_label = 'bullish')
            OR
            (LOWER(COALESCE(c4.trade_direction, '')) = 'short' AND c4.trend_label = 'bearish')
            )
        )
    """
    joins = """
        INNER JOIN ai_stage1_trade_market_context c4
          ON c4.multi_valid_eval_run_id = s.multi_valid_eval_run_id
         AND c4.setup_id = s.setup_id
         AND c4.template_uid = s.template_uid
         AND c4.timeframe = '4h'
    """
    weak_template_filter = """
        s.template_uid NOT IN ('tpl-eetc-mktwide-v2-0022', 'tpl-eetc-mktwide-v1-0009')
    """
    if policy == "4h-not-with-trend":
        return "context_4h_not_with_trend", joins, not_with_4h
    if policy == "4h-not-with-trend-skip-08-13":
        return "context_4h_not_with_trend_skip_08_13", joins, f"{not_with_4h} AND HOUR(c4.trade_at) NOT IN (8, 13)"
    if policy == "4h-not-with-trend-skip-weak-templates":
        return (
            "context_4h_not_with_trend_skip_weak_templates",
            joins,
            f"""
            {not_with_4h}
            AND {weak_template_filter}
            """,
        )
    if policy == "4h-not-with-trend-skip-weak-templates-structure-room":
        joins_with_structure = (
            joins
            + """
        INNER JOIN ai_stage1_trade_market_context c3
          ON c3.multi_valid_eval_run_id = s.multi_valid_eval_run_id
         AND c3.setup_id = s.setup_id
         AND c3.template_uid = s.template_uid
         AND c3.timeframe = '3m'
        INNER JOIN ai_stage1_trade_market_context c1
          ON c1.multi_valid_eval_run_id = s.multi_valid_eval_run_id
         AND c1.setup_id = s.setup_id
         AND c1.template_uid = s.template_uid
         AND c1.timeframe = '1m'
            """
        )
        return (
            "context_4h_not_with_trend_skip_weak_templates_structure_room",
            joins_with_structure,
            f"""
            {not_with_4h}
            AND {weak_template_filter}
            AND c3.support_distance_ticks >= 58.2
            AND c1.stop_side_sr_distance_r >= 0.3072
            """,
        )
    if policy == "4h-not-with-trend-skip-weak-templates-slope-volume":
        joins_with_slope_volume = (
            joins
            + """
        INNER JOIN ai_stage1_trade_market_context c1h
          ON c1h.multi_valid_eval_run_id = s.multi_valid_eval_run_id
         AND c1h.setup_id = s.setup_id
         AND c1h.template_uid = s.template_uid
         AND c1h.timeframe = '1h'
            """
        )
        return (
            "context_4h_not_with_trend_skip_weak_templates_slope_volume",
            joins_with_slope_volume,
            f"""
            {not_with_4h}
            AND {weak_template_filter}
            AND c1h.linreg_slope_pct_per_candle <= 0.07936
            AND c1h.relative_volume_20 <= 3.857
            """,
        )
    if policy == "4h-not-with-trend-skip-weak-templates-slope-volume-skip-pattern-c2-2r":
        joins_with_slope_volume = (
            joins
            + """
        INNER JOIN ai_stage1_trade_market_context c1h
          ON c1h.multi_valid_eval_run_id = s.multi_valid_eval_run_id
         AND c1h.setup_id = s.setup_id
         AND c1h.template_uid = s.template_uid
         AND c1h.timeframe = '1h'
            """
        )
        return (
            "context_v7_slope_volume_skip_pattern_c2_2r",
            joins_with_slope_volume,
            f"""
            {not_with_4h}
            AND {weak_template_filter}
            AND c1h.linreg_slope_pct_per_candle <= 0.07936
            AND c1h.relative_volume_20 <= 3.857
            AND s.template_uid <> 'tpl-eetc-mktwide-v1-0001'
            """,
        )
    raise ValueError(f"Unsupported policy: {policy}")


def run_exists(conn, run_id: str) -> bool:
    with conn.cursor() as cur:
        cur.execute(
            "SELECT COUNT(*) AS count FROM ai_stage1_multi_valid_eval_runs WHERE multi_valid_eval_run_id = %s",
            (run_id,),
        )
        return int(cur.fetchone()["count"]) > 0


def delete_policy_rows(conn, policy_run_id: str) -> None:
    with conn.cursor() as cur:
        for table_name in [
            *TRADE_DERIVED_TABLES,
            "ai_stage1_trade_market_context",
            "ai_stage1_multi_valid_eval_selected",
            "ai_stage1_multi_valid_eval_filter_summary",
            "ai_stage1_multi_valid_eval_slot_summary",
            "ai_stage1_multi_valid_eval_runs",
        ]:
            cur.execute(f"DELETE FROM {table_name} WHERE multi_valid_eval_run_id = %s", (policy_run_id,))


def insert_run_record(conn, source_run_id: str, policy_run_id: str, valid_sample_slots: str) -> None:
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
                train_sample_slot, %s,
                train_setups, train_rows, iterations, depth, learning_rate,
                l2_leaf_reg, random_strength, random_seed,
                pre_feature_set, aggregate_feature_set, excluded_roots,
                slippage_entry_ticks, slippage_exit_ticks,
                slippage_winner_exit_ticks, slippage_loser_exit_ticks,
                min_target_ticks, min_risk_ticks, model_path
            FROM ai_stage1_multi_valid_eval_runs
            WHERE multi_valid_eval_run_id = %s
            """,
            (policy_run_id, valid_sample_slots, source_run_id),
        )
        if cur.rowcount != 1:
            raise RuntimeError(f"Source run was not found or was not unique: {source_run_id}")


def selected_source_rows(conn, source_run_id: str, policy_joins: str, policy_where: str) -> pd.DataFrame:
    sql = f"""
        SELECT
            s.valid_sample_slot,
            s.setup_id,
            s.pattern_id,
            s.pattern_group_id,
            s.symbol,
            s.market,
            s.pattern_family_key,
            s.d_confirm_date,
            s.template_uid,
            s.template_name,
            s.model_rank,
            s.predicted_expected_r,
            s.score_margin_top2,
            s.actual_result_r,
            s.actual_outcome,
            s.oracle_template_uid,
            s.oracle_result_r,
            s.oracle_rank
        FROM ai_stage1_multi_valid_eval_selected s
        {policy_joins}
        WHERE s.multi_valid_eval_run_id = %s
          AND {policy_where}
        ORDER BY s.d_confirm_date, s.setup_id, s.template_uid
    """
    with conn.cursor() as cur:
        cur.execute(sql, (source_run_id,))
        frame = pd.DataFrame(cur.fetchall())
    if frame.empty:
        raise RuntimeError(f"No selected rows passed the context policy for source run: {source_run_id}")
    return frame


def insert_selected_rows(conn, policy_run_id: str, frame: pd.DataFrame) -> None:
    rows = []
    for row in frame[SELECTED_COLUMNS].itertuples(index=False, name=None):
        rows.append((policy_run_id, *[clean(value) for value in row]))
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


def insert_summary(conn, policy_run_id: str, frame: pd.DataFrame, metric_name: str) -> dict[str, Any]:
    result = pd.to_numeric(frame["actual_result_r"], errors="coerce").fillna(0.0)
    count = int(len(result))
    wins = int((result > 0.0).sum())
    losses = count - wins
    total = float(result.sum())
    avg = float(result.mean()) if count else 0.0
    win_rate = wins / count if count else 0.0
    with conn.cursor() as cur:
        cur.execute(
            """
            INSERT INTO ai_stage1_multi_valid_eval_slot_summary (
                multi_valid_eval_run_id, valid_sample_slot, metric_name,
                setup_count, wins, losses, win_rate, avg_r, sum_r
            )
            VALUES (%s, NULL, %s, %s, %s, %s, %s, %s, %s)
            """,
            (policy_run_id, metric_name, count, wins, losses, win_rate, avg, total),
        )
        cur.execute(
            """
            INSERT INTO ai_stage1_multi_valid_eval_filter_summary (
                multi_valid_eval_run_id, scope_name, rule_name,
                score_quantile, margin_quantile, score_threshold, margin_threshold,
                selected_trades, wins, losses, win_rate, avg_r, sum_r
            )
            VALUES (%s, 'combined', %s, NULL, NULL, NULL, NULL, %s, %s, %s, %s, %s, %s)
            """,
            (policy_run_id, metric_name, count, wins, losses, win_rate, avg, total),
        )
    return {
        "trades": count,
        "wins": wins,
        "losses": losses,
        "win_rate": win_rate,
        "avg_r": avg,
        "sum_r": total,
    }


def copy_market_context(conn, source_run_id: str, policy_run_id: str) -> None:
    with conn.cursor() as cur:
        cur.execute("SHOW COLUMNS FROM ai_stage1_trade_market_context")
        columns = [
            row["Field"]
            for row in cur.fetchall()
            if row["Field"] not in {"created_at", "updated_at"}
        ]
        select_columns = [
            "%s AS multi_valid_eval_run_id" if column == "multi_valid_eval_run_id" else f"c.{column}"
            for column in columns
        ]
        cur.execute(
            f"""
            INSERT INTO ai_stage1_trade_market_context ({", ".join(columns)})
            SELECT {", ".join(select_columns)}
            FROM ai_stage1_trade_market_context c
            INNER JOIN ai_stage1_multi_valid_eval_selected s
              ON s.multi_valid_eval_run_id = %s
             AND s.setup_id = c.setup_id
             AND s.template_uid = c.template_uid
            WHERE c.multi_valid_eval_run_id = %s
            """,
            (policy_run_id, policy_run_id, source_run_id),
        )


def materialize_year(
    conn,
    year: int,
    args: argparse.Namespace,
    metric_name: str,
    policy_joins: str,
    policy_where: str,
) -> dict[str, Any]:
    source_run_id = safe_run_id(args.source_prefix, year)
    policy_run_id = safe_run_id(args.policy_prefix, year)
    if run_exists(conn, policy_run_id) and not args.replace:
        raise RuntimeError(f"Policy run already exists. Use --replace to rebuild it: {policy_run_id}")

    frame = selected_source_rows(conn, source_run_id, policy_joins, policy_where)
    delete_policy_rows(conn, policy_run_id)
    insert_run_record(conn, source_run_id, policy_run_id, metric_name)
    insert_selected_rows(conn, policy_run_id, frame)
    summary = insert_summary(conn, policy_run_id, frame, metric_name)
    copy_market_context(conn, source_run_id, policy_run_id)
    conn.commit()
    return {
        "year": year,
        "source_run_id": source_run_id,
        "policy_run_id": policy_run_id,
        **summary,
    }


def main() -> int:
    args = parse_args()
    years = years_from_arg(args.years)
    metric_name, policy_joins, policy_where = policy_sql(args.policy)
    conn = connect()
    summaries: list[dict[str, Any]] = []
    try:
        for year in years:
            summaries.append(materialize_year(conn, year, args, metric_name, policy_joins, policy_where))
    except Exception:
        conn.rollback()
        raise
    finally:
        conn.close()

    print(f"Materialized context policy: {metric_name}")
    for summary in summaries:
        print(
            f"  {summary['year']}: {summary['policy_run_id']} from {summary['source_run_id']} -> "
            f"{summary['trades']:,} trades, {summary['wins']:,} wins / {summary['losses']:,} losses, "
            f"{summary['win_rate'] * 100:.2f}% win, {summary['avg_r']:.3f}R avg, "
            f"{summary['sum_r']:.1f}R sum"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
