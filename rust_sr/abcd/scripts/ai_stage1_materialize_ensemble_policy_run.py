#!/usr/bin/env python3
"""
Materialize the market-entry ensemble policy into standard AI Stage 1 run tables.

This creates one derived AI run per valid year. Each derived run combines selected
rows from the v1 and v2 model runs, dedupes by setup, and copies the matching raw
entry/exit rows into a single result table so the existing AI Trades API can
build the normal summary/daily/cadence/contribution tables.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import OrderedDict
from pathlib import Path
from typing import Any

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

RESULT_COLUMNS = [
    "run_id",
    "template_uid",
    "setup_id",
    "pattern_id",
    "pattern_group_id",
    "event_id",
    "event_rank",
    "event_sister_count",
    "event_decision_date",
    "event_candidate_count",
    "event_live_candidate_count",
    "pattern_family_key",
    "symbol",
    "market",
    "d_date",
    "d_confirm_date",
    "evaluation_order",
    "was_created_for_setup",
    "outcome",
    "exit_reason",
    "result_r",
    "entry_date",
    "exit_date",
    "entry_price",
    "stop_price",
    "target_price",
    "exit_price",
    "risk_points",
    "trade_direction",
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
    parser.add_argument("--metadata-json", required=True)
    parser.add_argument("--valid-years", default="2022,2023,2024,2025,2026")
    parser.add_argument("--run-prefix", default="aimv-market-entry-ensemble")
    parser.add_argument("--source-prefix", default="me-ensemble")
    parser.add_argument("--replace", action="store_true")
    return parser.parse_args()


def safe_run_id(prefix: str, year: int) -> str:
    value = f"{prefix}-{year}"
    if len(value) > 64:
        raise ValueError(f"Run id is too long for DB column: {value}")
    return value


def safe_result_table(year: int) -> str:
    return base.safe_identifier(f"entry_exit_template_results_me_ensemble_{year}")


def clean(value: Any) -> Any:
    return base.clean_db_value(value)


def fetch_run(conn, run_id: str) -> dict[str, Any]:
    with conn.cursor() as cur:
        cur.execute(
            "SELECT * FROM ai_stage1_multi_valid_eval_runs WHERE multi_valid_eval_run_id = %s",
            (run_id,),
        )
        row = cur.fetchone()
    if not row:
        raise RuntimeError(f"Missing source AI run: {run_id}")
    return row


def fetch_threshold(conn, run_id: str, score_quantile: float, margin_quantile: float) -> tuple[float, float]:
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT score_threshold, margin_threshold
            FROM ai_stage1_multi_valid_eval_filter_summary
            WHERE multi_valid_eval_run_id = %s
              AND scope_name = 'combined'
              AND rule_name = 'score_and_margin'
              AND score_quantile = %s
              AND margin_quantile = %s
            LIMIT 1
            """,
            (run_id, score_quantile, margin_quantile),
        )
        row = cur.fetchone()
    if not row:
        raise RuntimeError(f"Missing score/margin threshold for {run_id}")
    return float(row["score_threshold"]), float(row["margin_threshold"])


def fetch_v1_rows(conn, source_run_id: str, score_threshold: float, margin_threshold: float) -> pd.DataFrame:
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT
                1 AS valid_sample_slot,
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
                s.oracle_rank,
                %s AS leg_name,
                r.source_run_id AS raw_source_run_id,
                r.results_table AS raw_results_table
            FROM ai_stage1_multi_valid_eval_selected s
            JOIN ai_stage1_multi_valid_eval_runs r
              ON r.multi_valid_eval_run_id = s.multi_valid_eval_run_id
            JOIN entry_exit_templates t
              ON t.template_uid = s.template_uid
            WHERE s.multi_valid_eval_run_id = %s
              AND s.predicted_expected_r >= %s
              AND s.score_margin_top2 >= %s
              AND t.entry_kind <> 'pullback_retest_d'
            """,
            ("v1_stable_policy_no_pullback", source_run_id, score_threshold, margin_threshold),
        )
        return pd.DataFrame(cur.fetchall())


def fetch_v2_rows(conn, source_run_id: str) -> pd.DataFrame:
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT
                2 AS valid_sample_slot,
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
                s.oracle_rank,
                %s AS leg_name,
                r.source_run_id AS raw_source_run_id,
                r.results_table AS raw_results_table
            FROM ai_stage1_multi_valid_eval_selected s
            JOIN ai_stage1_multi_valid_eval_runs r
              ON r.multi_valid_eval_run_id = s.multi_valid_eval_run_id
            JOIN entry_exit_templates t
              ON t.template_uid = s.template_uid
            WHERE s.multi_valid_eval_run_id = %s
              AND t.target_r = 3.0
              AND t.direction_mode = 'inverse_pattern'
              AND REGEXP_REPLACE(s.symbol, '[FGHJKMNQUVXZ][0-9]+$', '') <> 'HO'
            """,
            ("v2_inverse_3r_no_ho", source_run_id),
        )
        return pd.DataFrame(cur.fetchall())


def dedupe_rows(frame: pd.DataFrame, priority: list[str]) -> pd.DataFrame:
    if frame.empty:
        return frame
    priority_index = {name: index for index, name in enumerate(priority)}
    work = frame.copy()
    work["_priority"] = work["leg_name"].map(lambda value: priority_index.get(str(value), 999))
    work["_predicted"] = pd.to_numeric(work["predicted_expected_r"], errors="coerce").fillna(-999999.0)
    work = work.sort_values(["setup_id", "_priority", "_predicted"], ascending=[True, True, False])
    return work.drop_duplicates(subset=["setup_id"], keep="first").drop(columns=["_priority", "_predicted"])


def delete_existing(conn, run_id: str, source_run_id: str, result_table: str) -> None:
    with conn.cursor() as cur:
        for table in [
            *TRADE_DERIVED_TABLES,
            "ai_stage1_multi_valid_eval_selected",
            "ai_stage1_multi_valid_eval_filter_summary",
            "ai_stage1_multi_valid_eval_slot_summary",
            "ai_stage1_multi_valid_eval_runs",
        ]:
            cur.execute(f"DELETE FROM {table} WHERE multi_valid_eval_run_id = %s", (run_id,))
        cur.execute(f"DELETE FROM {result_table} WHERE run_id = %s", (source_run_id,))


def ensure_result_table(conn, result_table: str) -> None:
    with conn.cursor() as cur:
        cur.execute(f"CREATE TABLE IF NOT EXISTS {result_table} LIKE entry_exit_template_results_eetc_mktwide_v1")


def insert_result_rows(conn, result_table: str, ensemble_source_run_id: str, selected: pd.DataFrame) -> None:
    if selected.empty:
        return

    with conn.cursor() as cur:
        cur.execute(
            """
            CREATE TEMPORARY TABLE IF NOT EXISTS tmp_ensemble_selected_keys (
                setup_id VARCHAR(64) NOT NULL,
                template_uid VARCHAR(128) NOT NULL,
                PRIMARY KEY (setup_id, template_uid)
            )
            """
        )

    insert_columns = ", ".join(RESULT_COLUMNS)
    select_columns = ", ".join(f"r.{column}" for column in RESULT_COLUMNS if column != "run_id")
    for (raw_table, raw_source_run_id), group in selected.groupby(["raw_results_table", "raw_source_run_id"]):
        raw_table = base.safe_identifier(str(raw_table))
        keys = [(str(row.setup_id), str(row.template_uid)) for row in group.itertuples()]
        with conn.cursor() as cur:
            cur.execute("TRUNCATE TABLE tmp_ensemble_selected_keys")
            cur.executemany(
                "INSERT INTO tmp_ensemble_selected_keys (setup_id, template_uid) VALUES (%s, %s)",
                keys,
            )
            cur.execute(
                f"""
                INSERT INTO {result_table} ({insert_columns})
                SELECT
                    %s AS run_id,
                    {select_columns}
                FROM {raw_table} r
                JOIN tmp_ensemble_selected_keys k
                  ON k.setup_id = r.setup_id
                 AND k.template_uid = r.template_uid
                WHERE r.run_id = %s
                """,
                (ensemble_source_run_id, raw_source_run_id),
            )
            if cur.rowcount != len(keys):
                raise RuntimeError(
                    f"Copied {cur.rowcount} raw rows from {raw_table}, expected {len(keys)}"
                )


def insert_run_record(
    conn,
    run_id: str,
    source_run_id: str,
    result_table: str,
    year: int,
    metadata_path: Path,
    selected: pd.DataFrame,
) -> None:
    model_path = str(metadata_path.parent)
    train_setups = 0
    train_rows = 0
    source_runs = []
    for source_id in [f"aimv-mktwide-v1-risk120-full-{year}", f"aimv-mktwide-v2-risk120-full-{year}"]:
        source_run = fetch_run(conn, source_id)
        source_runs.append(source_run)
        train_setups = max(train_setups, int(source_run["train_setups"] or 0))
        train_rows = max(train_rows, int(source_run["train_rows"] or 0))
    seed_run = source_runs[-1]
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
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            """,
            (
                run_id,
                source_run_id,
                result_table,
                int(seed_run["train_start_year"]),
                int(seed_run["train_end_year"]),
                year,
                0,
                "ensemble_v2_priority",
                train_setups,
                train_rows,
                int(seed_run["iterations"]),
                int(seed_run["depth"]),
                float(seed_run["learning_rate"]),
                float(seed_run["l2_leaf_reg"]),
                float(seed_run["random_strength"]),
                int(seed_run["random_seed"]),
                seed_run["pre_feature_set"],
                seed_run["aggregate_feature_set"],
                seed_run["excluded_roots"],
                seed_run["slippage_entry_ticks"],
                seed_run["slippage_exit_ticks"],
                seed_run["slippage_winner_exit_ticks"],
                seed_run["slippage_loser_exit_ticks"],
                seed_run["min_target_ticks"],
                seed_run["min_risk_ticks"],
                model_path,
            ),
        )


def insert_selected_rows(conn, run_id: str, selected: pd.DataFrame) -> None:
    rows = []
    for row in selected[SELECTED_COLUMNS].itertuples(index=False, name=None):
        rows.append((run_id, *[clean(value) for value in row]))
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


def insert_summaries(conn, run_id: str, selected: pd.DataFrame) -> None:
    result = pd.to_numeric(selected["actual_result_r"], errors="coerce").fillna(0.0)
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
            VALUES (%s, NULL, 'market_entry_ensemble', %s, %s, %s, %s, %s, %s)
            """,
            (run_id, count, wins, losses, win_rate, avg, total),
        )
        cur.execute(
            """
            INSERT INTO ai_stage1_multi_valid_eval_filter_summary (
                multi_valid_eval_run_id, scope_name, rule_name,
                score_quantile, margin_quantile, score_threshold, margin_threshold,
                selected_trades, wins, losses, win_rate, avg_r, sum_r
            )
            VALUES (%s, 'combined', 'market_entry_ensemble', NULL, NULL, NULL, NULL, %s, %s, %s, %s, %s, %s)
            """,
            (run_id, count, wins, losses, win_rate, avg, total),
        )


def materialize_year(conn, metadata: dict[str, Any], metadata_path: Path, year: int, args: argparse.Namespace) -> dict[str, Any]:
    run_id = safe_run_id(args.run_prefix, year)
    source_run_id = safe_run_id(args.source_prefix, year)
    result_table = safe_result_table(year)
    ensure_result_table(conn, result_table)

    with conn.cursor() as cur:
        cur.execute(
            "SELECT COUNT(*) AS count FROM ai_stage1_multi_valid_eval_runs WHERE multi_valid_eval_run_id = %s",
            (run_id,),
        )
        exists = int(cur.fetchone()["count"]) > 0
    if exists and not args.replace:
        raise RuntimeError(f"Run already exists. Use --replace to rebuild it: {run_id}")

    v1_source_run_id = f"aimv-mktwide-v1-risk120-full-{year}"
    v2_source_run_id = f"aimv-mktwide-v2-risk120-full-{year}"
    score_threshold, margin_threshold = fetch_threshold(conn, v1_source_run_id, 0.50, 0.75)
    v1 = fetch_v1_rows(conn, v1_source_run_id, score_threshold, margin_threshold)
    v2 = fetch_v2_rows(conn, v2_source_run_id)
    combined = pd.concat([v1, v2], ignore_index=True)
    priority = metadata.get("ensemble_rule", {}).get("dedupe_priority") or [
        "v2_inverse_3r_no_ho",
        "v1_stable_policy_no_pullback",
    ]
    selected = dedupe_rows(combined, [str(value) for value in priority])
    if selected.empty:
        raise RuntimeError(f"Ensemble selected no rows for {year}")

    delete_existing(conn, run_id, source_run_id, result_table)
    insert_result_rows(conn, result_table, source_run_id, selected)
    insert_run_record(conn, run_id, source_run_id, result_table, year, metadata_path, selected)
    insert_selected_rows(conn, run_id, selected)
    insert_summaries(conn, run_id, selected)
    conn.commit()

    result = pd.to_numeric(selected["actual_result_r"], errors="coerce").fillna(0.0)
    wins = int((result > 0.0).sum())
    return {
        "year": year,
        "run_id": run_id,
        "source_run_id": source_run_id,
        "result_table": result_table,
        "trades": int(len(selected)),
        "wins": wins,
        "losses": int(len(selected) - wins),
        "win_rate": wins / len(selected) if len(selected) else 0.0,
        "avg_r": float(result.mean()) if len(selected) else 0.0,
        "sum_r": float(result.sum()),
    }


def main() -> int:
    args = parse_args()
    metadata_path = Path(args.metadata_json)
    metadata = json.loads(metadata_path.read_text())
    years = [int(part.strip()) for part in args.valid_years.split(",") if part.strip()]

    conn = base.connect()
    summaries: list[dict[str, Any]] = []
    try:
        for year in years:
            summaries.append(materialize_year(conn, metadata, metadata_path, year, args))
    except Exception:
        conn.rollback()
        raise
    finally:
        conn.close()

    print("Materialized market-entry ensemble runs:")
    for summary in summaries:
        print(
            f"  {summary['year']}: {summary['run_id']} -> {summary['trades']:,} trades, "
            f"{summary['wins']:,} wins / {summary['losses']:,} losses, "
            f"{summary['win_rate'] * 100:.2f}% win, {summary['avg_r']:.3f}R avg, "
            f"{summary['sum_r']:.1f}R sum"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
