#!/usr/bin/env python3
"""
Materialize AI trade runs with a pattern-length entry window.

Taken trades are allowed only if their entry happens within:

    full_pattern_length * timeframe_minutes * window_multiple

after the pattern confirmation candle. Trades outside the window are converted
to no-entry rows so the run remains fully DB-backed and inspectable in the
existing AI Trades UI.
"""

from __future__ import annotations

import argparse
import json
import math
import re
import urllib.error
import urllib.request
from typing import Any

import pandas as pd

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


TIMEFRAME_MINUTES = {
    "1m": 1,
    "3m": 3,
    "5m": 5,
    "15m": 15,
    "30m": 30,
    "1h": 60,
    "4h": 240,
    "12h": 720,
    "1d": 1440,
    "daily": 1440,
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-run-id", default="aimv-mwmeta-v3-v7-twin-xq3-2026")
    parser.add_argument("--source-result-run-id", default="")
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--results-table", required=True)
    parser.add_argument("--window-multiple", type=float, required=True)
    parser.add_argument("--replace", action="store_true")
    parser.add_argument("--api-refresh-url", default="http://localhost:8080/patterns/ai-stage1-trades")
    parser.add_argument("--skip-api-refresh", action="store_true")
    return parser.parse_args()


def finite(value: Any, default: float | None = None) -> float | None:
    try:
        numeric = float(value)
    except (TypeError, ValueError):
        return default
    if not math.isfinite(numeric):
        return default
    return numeric


def timeframe_minutes(value: Any) -> int:
    text = str(value or "").strip().lower()
    if text in TIMEFRAME_MINUTES:
        return TIMEFRAME_MINUTES[text]
    match = re.fullmatch(r"(\d+)(m|h|d)", text)
    if not match:
        return 1
    amount = int(match.group(1))
    unit = match.group(2)
    return amount * {"m": 1, "h": 60, "d": 1440}[unit]


def fetch_run(conn, run_id: str) -> dict[str, Any]:
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT *
            FROM ai_stage1_multi_valid_eval_runs
            WHERE multi_valid_eval_run_id = %s
            """,
            (run_id,),
        )
        row = cur.fetchone()
    if not row:
        raise RuntimeError(f"Missing source run: {run_id}")
    return dict(row)


def fetch_selected_rows(conn, run_id: str) -> list[dict[str, Any]]:
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT {", ".join(SELECTED_COLUMNS)}
            FROM ai_stage1_multi_valid_eval_selected
            WHERE multi_valid_eval_run_id = %s
            ORDER BY COALESCE(d_confirm_date, '1900-01-01'), setup_id, template_uid
            """,
            (run_id,),
        )
        return [dict(row) for row in cur.fetchall()]


def fetch_result_rows(conn, result_table: str, source_result_run_id: str) -> dict[tuple[str, str], dict[str, Any]]:
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT {", ".join(RESULT_COLUMNS)}
            FROM {safe_identifier(result_table)}
            WHERE run_id = %s
            """,
            (source_result_run_id,),
        )
        rows = cur.fetchall()
    return {(str(row.get("setup_id") or ""), str(row.get("template_uid") or "")): dict(row) for row in rows}


def fetch_setup_meta(conn, setup_ids: list[str]) -> dict[str, dict[str, Any]]:
    if not setup_ids:
        return {}
    rows: list[dict[str, Any]] = []
    with conn.cursor() as cur:
        for index in range(0, len(setup_ids), 900):
            chunk = setup_ids[index : index + 900]
            placeholders = ", ".join(["%s"] * len(chunk))
            cur.execute(
                f"""
                SELECT setup_id, source_timeframe, full_pattern_length, d_confirm_date
                FROM pattern_setups
                WHERE setup_id IN ({placeholders})
                """,
                chunk,
            )
            rows.extend(cur.fetchall())
    return {str(row.get("setup_id") or ""): dict(row) for row in rows}


def minutes_between(start: Any, end: Any) -> float | None:
    if start is None or end is None:
        return None
    start_ts = pd.Timestamp(start)
    end_ts = pd.Timestamp(end)
    if pd.isna(start_ts) or pd.isna(end_ts):
        return None
    return (end_ts - start_ts).total_seconds() / 60.0


def allowed_window_minutes(setup: dict[str, Any], multiple: float) -> float | None:
    full_pattern_length = finite(setup.get("full_pattern_length"))
    if full_pattern_length is None or full_pattern_length <= 0:
        return None
    return full_pattern_length * timeframe_minutes(setup.get("source_timeframe")) * multiple


def is_inside_window(
    selected: dict[str, Any],
    result: dict[str, Any],
    setup: dict[str, Any],
    multiple: float,
) -> tuple[bool, float | None, float | None]:
    confirm_date = result.get("d_confirm_date") or selected.get("d_confirm_date") or setup.get("d_confirm_date")
    entry_date = result.get("entry_date")
    delay_minutes = minutes_between(confirm_date, entry_date)
    window_minutes = allowed_window_minutes(setup, multiple)
    if delay_minutes is None or window_minutes is None:
        return False, delay_minutes, window_minutes
    return delay_minutes <= window_minutes, delay_minutes, window_minutes


def no_entry_selected(row: dict[str, Any]) -> dict[str, Any]:
    next_row = {column: row.get(column) for column in SELECTED_COLUMNS}
    next_row["actual_result_r"] = 0.0
    next_row["actual_outcome"] = "no_entry"
    return next_row


def no_entry_result(row: dict[str, Any], target_source_run_id: str, reason: str) -> dict[str, Any]:
    next_row = {column: row.get(column) for column in RESULT_COLUMNS}
    next_row["run_id"] = target_source_run_id
    next_row["outcome"] = "no_entry"
    next_row["exit_reason"] = reason
    next_row["result_r"] = 0.0
    for column in [
        "entry_date",
        "exit_date",
        "entry_price",
        "stop_price",
        "target_price",
        "exit_price",
        "risk_points",
        "trade_direction",
    ]:
        next_row[column] = None
    return next_row


def insert_run_record(
    conn,
    source_run: dict[str, Any],
    args: argparse.Namespace,
    target_source_run_id: str,
    input_result_run_id: str,
) -> None:
    model_dir = ABCD_ROOT / "model_registry" / args.run_id
    model_dir.mkdir(parents=True, exist_ok=True)
    metadata = {
        "formula": "pattern_window_v1",
        "run_id": args.run_id,
        "source_run_id": args.source_run_id,
        "input_result_run_id": input_result_run_id,
        "target_result_run_id": target_source_run_id,
        "results_table": args.results_table,
        "window_multiple": args.window_multiple,
        "window_formula": "full_pattern_length * timeframe_minutes * window_multiple",
    }
    (model_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n")
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
                target_source_run_id,
                args.results_table,
                int(source_run.get("train_start_year") or 0),
                int(source_run.get("train_end_year") or 0),
                int(source_run.get("valid_year") or 0),
                int(source_run.get("train_sample_slot") or 0),
                f"pattern_window_x{args.window_multiple:g}",
                int(source_run.get("train_setups") or 0),
                int(source_run.get("train_rows") or 0),
                int(source_run.get("iterations") or 0),
                int(source_run.get("depth") or 0),
                float(source_run.get("learning_rate") or 0.0),
                float(source_run.get("l2_leaf_reg") or 0.0),
                float(source_run.get("random_strength") or 0.0),
                int(source_run.get("random_seed") or 0),
                source_run.get("pre_feature_set"),
                source_run.get("aggregate_feature_set"),
                source_run.get("excluded_roots"),
                float(source_run.get("slippage_entry_ticks") or 0.0),
                float(source_run.get("slippage_exit_ticks") or 0.0),
                float(source_run.get("slippage_winner_exit_ticks") or 0.0),
                float(source_run.get("slippage_loser_exit_ticks") or 0.0),
                float(source_run.get("min_target_ticks") or 0.0),
                float(source_run.get("min_risk_ticks") or 0.0),
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
        source_run = fetch_run(conn, args.source_run_id)
        input_result_run_id = args.source_result_run_id or str(source_run.get("source_run_id") or "")
        if not input_result_run_id:
            raise RuntimeError("Could not resolve source result run id.")

        with conn.cursor() as cur:
            cur.execute("SELECT COUNT(*) AS count FROM ai_stage1_multi_valid_eval_runs WHERE multi_valid_eval_run_id = %s", (args.run_id,))
            exists = int(cur.fetchone()["count"]) > 0
        if exists and not args.replace:
            raise RuntimeError(f"Run already exists. Use --replace to rebuild it: {args.run_id}")

        selected_rows = fetch_selected_rows(conn, args.source_run_id)
        result_map = fetch_result_rows(conn, str(source_run.get("results_table")), input_result_run_id)
        setup_meta = fetch_setup_meta(conn, sorted({str(row.get("setup_id") or "") for row in selected_rows if row.get("setup_id")}))
        if not selected_rows:
            raise RuntimeError(f"No selected rows for source run: {args.source_run_id}")
        if not result_map:
            raise RuntimeError(f"No result rows for source result run: {input_result_run_id}")

        target_source_run_id = args.run_id.replace("aimv-", "", 1)
        reject_reason = f"patwin_x{args.window_multiple:g}_expired"
        if len(reject_reason) > 32:
            reject_reason = "patwin_expired"

        target_selected_rows: list[dict[str, Any]] = []
        target_result_rows: list[dict[str, Any]] = []
        taken_before = 0
        taken_after = 0
        sum_before = 0.0
        sum_after = 0.0
        rejected_taken = 0
        rejected_sum = 0.0

        for selected in selected_rows:
            key = (str(selected.get("setup_id") or ""), str(selected.get("template_uid") or ""))
            result = result_map.get(key)
            if result is None:
                continue

            is_taken = str(selected.get("actual_outcome") or result.get("outcome") or "") != "no_entry"
            result_r = finite(selected.get("actual_result_r"), finite(result.get("result_r"), 0.0)) or 0.0
            if is_taken:
                taken_before += 1
                sum_before += result_r

            keep = False
            if is_taken:
                setup = setup_meta.get(str(selected.get("setup_id") or ""), {})
                keep, _delay, _window = is_inside_window(selected, result, setup, float(args.window_multiple))

            if is_taken and not keep:
                rejected_taken += 1
                rejected_sum += result_r
                target_selected_rows.append(no_entry_selected(selected))
                target_result_rows.append(no_entry_result(result, target_source_run_id, reject_reason))
                continue

            next_selected = {column: selected.get(column) for column in SELECTED_COLUMNS}
            next_result = {column: result.get(column) for column in RESULT_COLUMNS}
            next_result["run_id"] = target_source_run_id
            target_selected_rows.append(next_selected)
            target_result_rows.append(next_result)
            if is_taken:
                taken_after += 1
                sum_after += result_r

        ensure_result_table(conn, args.results_table)
        delete_existing(conn, args.run_id, target_source_run_id, args.results_table)
        insert_run_record(conn, source_run, args, target_source_run_id, input_result_run_id)
        insert_selected_rows(conn, args.run_id, target_selected_rows)
        insert_result_rows(conn, args.results_table, target_result_rows)
        summary = insert_filter_summaries(conn, args.run_id, target_selected_rows, reject_reason)
        conn.commit()

        print(
            json.dumps(
                {
                    "run_id": args.run_id,
                    "source_run_id": args.source_run_id,
                    "window_multiple": args.window_multiple,
                    "taken_before": taken_before,
                    "taken_after": taken_after,
                    "rejected_taken": rejected_taken,
                    "sum_before": round(sum_before, 6),
                    "sum_after": round(sum_after, 6),
                    "rejected_sum": round(rejected_sum, 6),
                    "summary": summary,
                },
                indent=2,
                sort_keys=True,
            )
        )

        if not args.skip_api_refresh:
            api_refresh(args.run_id, int(source_run.get("valid_year") or 0), args.api_refresh_url)
        return 0
    finally:
        conn.close()


if __name__ == "__main__":
    raise SystemExit(main())
