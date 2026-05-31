#!/usr/bin/env python3
"""
Materialize a union AI policy run from two existing AI trade runs.

The first run is treated as the primary policy. The second run contributes
taken trades that are not duplicates under the requested de-duplication mode.
This creates normal AI Stage 1 selected/result rows so the existing AI Trades UI
and robustness tooling can read the run from DB tables.
"""

from __future__ import annotations

import argparse
import json
import sys
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

from ai_wave_rider_research import (
    ABCD_ROOT,
    RESULT_COLUMNS,
    SELECTED_COLUMNS,
    clean,
    connect,
    delete_existing,
    ensure_result_table,
    insert_filter_summaries,
    insert_result_rows,
    insert_selected_rows,
    safe_identifier,
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--primary-run-id", default="aimv-market-wave-meta-v3-2026")
    parser.add_argument("--secondary-run-id", default="aimv-market-entry-context-v7-2026")
    parser.add_argument("--run-id", default="aimv-mwmeta-v3-plus-v7-2026")
    parser.add_argument("--source-run-id", default="mwmeta-v3-plus-v7-2026")
    parser.add_argument("--results-table", default="entry_exit_template_results_mwmeta_v3_plus_v7_2026")
    parser.add_argument(
        "--mode",
        choices=["exact", "symbol_entry_direction", "one_per_timestamp"],
        default="exact",
    )
    parser.add_argument("--replace", action="store_true")
    parser.add_argument("--slippage-entry-ticks", type=float, default=3.0)
    parser.add_argument("--slippage-exit-ticks", type=float, default=0.0)
    parser.add_argument("--api-refresh-url", default="http://localhost:8080/patterns/ai-stage1-trades")
    parser.add_argument("--skip-api-refresh", action="store_true")
    return parser.parse_args()


def fetch_run(conn, run_id: str) -> dict[str, Any]:
    with conn.cursor() as cur:
        cur.execute(
            "SELECT * FROM ai_stage1_multi_valid_eval_runs WHERE multi_valid_eval_run_id = %s",
            (run_id,),
        )
        row = cur.fetchone()
    if not row:
        raise RuntimeError(f"Missing AI run: {run_id}")
    return row


def fetch_selected(conn, run_id: str, only_taken: bool = False) -> list[dict[str, Any]]:
    where = "WHERE multi_valid_eval_run_id = %s"
    if only_taken:
        where += " AND actual_outcome <> 'no_entry'"
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT {", ".join(SELECTED_COLUMNS)}
            FROM ai_stage1_multi_valid_eval_selected
            {where}
            ORDER BY COALESCE(d_confirm_date, '1900-01-01'), setup_id, template_uid
            """,
            (run_id,),
        )
        return [dict(row) for row in cur.fetchall()]


def fetch_taken_trade_keys(conn, run_id: str) -> dict[tuple[str, str, str, str], str]:
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT setup_id, symbol, trade_at, trade_direction
            FROM ai_stage1_trade_rows
            WHERE multi_valid_eval_run_id = %s
              AND outcome <> 'no_entry'
            """,
            (run_id,),
        )
        rows = cur.fetchall()
    keys: dict[tuple[str, str, str, str], str] = {}
    for row in rows:
        trade_at = row.get("trade_at")
        key = (
            str(row.get("setup_id") or ""),
            str(row.get("symbol") or ""),
            str(trade_at or ""),
            str(row.get("trade_direction") or ""),
        )
        keys[key] = str(trade_at or "")
    return keys


def fetch_secondary_selected_with_trade_keys(conn, run_id: str) -> list[dict[str, Any]]:
    selected_columns = ", ".join([f"s.{column}" for column in SELECTED_COLUMNS])
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT
                {selected_columns},
                r.trade_at AS _trade_at,
                r.trade_direction AS _trade_direction
            FROM ai_stage1_multi_valid_eval_selected s
            JOIN ai_stage1_trade_rows r
              ON r.multi_valid_eval_run_id = s.multi_valid_eval_run_id
             AND r.setup_id = s.setup_id
             AND r.template_uid = s.template_uid
            WHERE s.multi_valid_eval_run_id = %s
              AND s.actual_outcome <> 'no_entry'
              AND r.outcome <> 'no_entry'
            ORDER BY r.trade_at, s.setup_id, s.template_uid
            """,
            (run_id,),
        )
        return [dict(row) for row in cur.fetchall()]


def clean_selected_row(row: dict[str, Any]) -> dict[str, Any]:
    return {column: row.get(column) for column in SELECTED_COLUMNS}


def trade_key_from_selected(row: dict[str, Any]) -> tuple[str, str, str, str]:
    return (
        str(row.get("setup_id") or ""),
        str(row.get("symbol") or ""),
        str(row.get("_trade_at") or ""),
        str(row.get("_trade_direction") or ""),
    )


def twin_key_from_selected(row: dict[str, Any]) -> tuple[str, str, str]:
    return (
        str(row.get("symbol") or ""),
        str(row.get("_trade_at") or ""),
        str(row.get("_trade_direction") or ""),
    )


def one_per_timestamp_primary_rows(
    primary_all: list[dict[str, Any]],
    primary_taken_with_keys: list[dict[str, Any]],
) -> tuple[list[dict[str, Any]], dict[tuple[str, str, str, str], str]]:
    no_entry_rows = [row for row in primary_all if str(row.get("actual_outcome") or "") == "no_entry"]
    kept_taken: list[dict[str, Any]] = []
    kept_keys: dict[tuple[str, str, str, str], str] = {}
    used_timestamps: set[str] = set()
    for row in primary_taken_with_keys:
        trade_at = str(row.get("_trade_at") or "")
        if trade_at in used_timestamps:
            continue
        used_timestamps.add(trade_at)
        key = trade_key_from_selected(row)
        kept_keys[key] = trade_at
        kept_taken.append(clean_selected_row(row))
    return no_entry_rows + kept_taken, kept_keys


def symbol_entry_direction_primary_rows(
    primary_all: list[dict[str, Any]],
    primary_taken_with_keys: list[dict[str, Any]],
) -> tuple[list[dict[str, Any]], set[tuple[str, str, str]]]:
    no_entry_rows = [row for row in primary_all if str(row.get("actual_outcome") or "") == "no_entry"]
    kept_taken: list[dict[str, Any]] = []
    used_keys: set[tuple[str, str, str]] = set()
    for row in primary_taken_with_keys:
        key = twin_key_from_selected(row)
        if key in used_keys:
            continue
        used_keys.add(key)
        kept_taken.append(clean_selected_row(row))
    return no_entry_rows + kept_taken, used_keys


def filter_secondary_rows(
    secondary_rows: list[dict[str, Any]],
    primary_keys: dict[tuple[str, str, str, str], str],
    primary_twin_keys: set[tuple[str, str, str]],
    mode: str,
) -> list[dict[str, Any]]:
    primary_timestamps = set(primary_keys.values())
    kept: list[dict[str, Any]] = []
    seen_secondary: set[tuple[str, str, str, str]] = set()
    seen_secondary_twins: set[tuple[str, str, str]] = set()
    used_timestamps = set(primary_timestamps)
    for row in secondary_rows:
        trade_at = str(row.get("_trade_at") or "")
        key = trade_key_from_selected(row)
        twin_key = twin_key_from_selected(row)
        if mode == "one_per_timestamp" and trade_at in used_timestamps:
            continue
        if mode == "symbol_entry_direction" and twin_key in primary_twin_keys:
            continue
        if mode == "exact" and key in primary_keys:
            continue
        if key in seen_secondary:
            continue
        if mode == "symbol_entry_direction" and twin_key in seen_secondary_twins:
            continue
        seen_secondary.add(key)
        seen_secondary_twins.add(twin_key)
        used_timestamps.add(trade_at)
        clean_row = clean_selected_row(row)
        clean_row["valid_sample_slot"] = 2
        kept.append(clean_row)
    return kept


def fetch_result_map(conn, result_table: str, source_run_id: str) -> dict[tuple[str, str], dict[str, Any]]:
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT {", ".join(RESULT_COLUMNS)}
            FROM {safe_identifier(result_table)}
            WHERE run_id = %s
            """,
            (source_run_id,),
        )
        rows = cur.fetchall()
    return {(str(row.get("setup_id") or ""), str(row.get("template_uid") or "")): dict(row) for row in rows}


def result_rows_for_selected(
    selected_rows: list[dict[str, Any]],
    result_map: dict[tuple[str, str], dict[str, Any]],
    new_source_run_id: str,
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    missing = 0
    for selected in selected_rows:
        key = (str(selected.get("setup_id") or ""), str(selected.get("template_uid") or ""))
        source = result_map.get(key)
        if source is None:
            missing += 1
            continue
        row = dict(source)
        row["run_id"] = new_source_run_id
        rows.append(row)
    if missing:
        print(f"Warning: skipped {missing} selected rows because result details were missing.")
    return rows


def insert_run_record(conn, primary_run: dict[str, Any], args: argparse.Namespace) -> None:
    model_dir = ABCD_ROOT / "model_registry" / args.run_id
    model_dir.mkdir(parents=True, exist_ok=True)
    metadata_path = model_dir / "metadata.json"
    metadata_path.write_text(
        json.dumps(
            {
                "run_id": args.run_id,
                "source_run_id": args.source_run_id,
                "primary_run_id": args.primary_run_id,
                "secondary_run_id": args.secondary_run_id,
                "mode": args.mode,
                "slippage_entry_ticks": args.slippage_entry_ticks,
                "slippage_exit_ticks": args.slippage_exit_ticks,
                "formula": "union_policy_v1",
            },
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )
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
                args.run_id,
                args.source_run_id,
                args.results_table,
                int(primary_run.get("train_start_year") or 0),
                int(primary_run.get("train_end_year") or 0),
                int(primary_run.get("valid_year") or 0),
                0,
                f"union_{args.mode}",
                int(primary_run.get("train_setups") or 0),
                int(primary_run.get("train_rows") or 0),
                int(primary_run.get("iterations") or 0),
                int(primary_run.get("depth") or 0),
                float(primary_run.get("learning_rate") or 0.0),
                float(primary_run.get("l2_leaf_reg") or 0.0),
                float(primary_run.get("random_strength") or 0.0),
                int(primary_run.get("random_seed") or 0),
                primary_run.get("pre_feature_set"),
                primary_run.get("aggregate_feature_set"),
                primary_run.get("excluded_roots"),
                float(args.slippage_entry_ticks),
                float(args.slippage_exit_ticks),
                0.0,
                float(args.slippage_exit_ticks),
                0.0,
                float(primary_run.get("min_risk_ticks") or 0.0),
                str(model_dir),
            ),
        )


def api_refresh(run_id: str, year: int, refresh_url: str) -> None:
    payload = json.dumps({"ai_run_id": run_id, "valid_year": year, "refresh": True, "limit": 1, "offset": 0}).encode()
    request = urllib.request.Request(refresh_url, data=payload, headers={"Content-Type": "application/json"}, method="POST")
    try:
        with urllib.request.urlopen(request, timeout=120) as response:
            response.read()
        print(f"Refreshed AI trade tables for {run_id}.")
    except (urllib.error.URLError, TimeoutError) as error:
        print(f"API refresh skipped for {run_id}: {error}")


def main() -> int:
    args = parse_args()
    conn = connect()
    try:
        primary_run = fetch_run(conn, args.primary_run_id)
        secondary_run = fetch_run(conn, args.secondary_run_id)
        valid_year = int(primary_run.get("valid_year") or 0)
        if int(secondary_run.get("valid_year") or 0) != valid_year:
            raise RuntimeError("Primary and secondary runs must share the same valid_year.")

        ensure_result_table(conn, args.results_table)
        with conn.cursor() as cur:
            cur.execute(
                "SELECT COUNT(*) AS count FROM ai_stage1_multi_valid_eval_runs WHERE multi_valid_eval_run_id = %s",
                (args.run_id,),
            )
            exists = int(cur.fetchone()["count"]) > 0
        if exists and not args.replace:
            raise RuntimeError(f"Run already exists. Use --replace to rebuild it: {args.run_id}")
        if args.replace:
            delete_existing(conn, args.run_id, args.source_run_id, args.results_table)

        primary_all = fetch_selected(conn, args.primary_run_id, only_taken=False)
        if args.mode == "one_per_timestamp":
            primary_taken_candidates = fetch_secondary_selected_with_trade_keys(conn, args.primary_run_id)
            primary_selected, primary_keys = one_per_timestamp_primary_rows(primary_all, primary_taken_candidates)
            primary_twin_keys: set[tuple[str, str, str]] = set()
        elif args.mode == "symbol_entry_direction":
            primary_taken_candidates = fetch_secondary_selected_with_trade_keys(conn, args.primary_run_id)
            primary_selected, primary_twin_keys = symbol_entry_direction_primary_rows(primary_all, primary_taken_candidates)
            primary_keys = fetch_taken_trade_keys(conn, args.primary_run_id)
        else:
            primary_selected = primary_all
            primary_keys = fetch_taken_trade_keys(conn, args.primary_run_id)
            primary_twin_keys = set()
        secondary_candidates = fetch_secondary_selected_with_trade_keys(conn, args.secondary_run_id)
        secondary_selected = filter_secondary_rows(secondary_candidates, primary_keys, primary_twin_keys, args.mode)
        selected_rows = primary_selected + secondary_selected

        primary_results = fetch_result_map(conn, primary_run["results_table"], primary_run["source_run_id"])
        secondary_results = fetch_result_map(conn, secondary_run["results_table"], secondary_run["source_run_id"])
        result_rows = (
            result_rows_for_selected(primary_selected, primary_results, args.source_run_id)
            + result_rows_for_selected(secondary_selected, secondary_results, args.source_run_id)
        )

        insert_result_rows(conn, args.results_table, result_rows)
        insert_run_record(conn, primary_run, args)
        insert_selected_rows(conn, args.run_id, selected_rows)
        summary = insert_filter_summaries(conn, args.run_id, selected_rows, "union_policy_v1")
        conn.commit()
    except Exception:
        conn.rollback()
        raise
    finally:
        conn.close()

    print(
        f"Materialized {args.run_id}: {len(primary_selected):,} primary rows + "
        f"{len(secondary_selected):,} secondary rows, {summary['wins']:,} wins / "
        f"{summary['losses']:,} losses / {summary['no_entries']:,} no-entry, "
        f"{summary['sum_r']:.1f}R selected sum"
    )
    if not args.skip_api_refresh:
        api_refresh(args.run_id, valid_year, args.api_refresh_url)
    return 0


if __name__ == "__main__":
    sys.exit(main())
