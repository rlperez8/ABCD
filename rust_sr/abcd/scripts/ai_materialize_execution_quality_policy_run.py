#!/usr/bin/env python3
"""
Materialize an AI trade run with a pre-entry execution-quality floor.

The source run is left untouched. Taken trades whose model-expected gross move
is smaller than N times their inferred slippage are converted to no-entry rows.
This keeps the existing AI Trades UI/rollups DB-backed while making the
tradability rule reproducible.
"""

from __future__ import annotations

import argparse
import json
import math
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
    direction_sign,
    ensure_result_table,
    insert_filter_summaries,
    insert_result_rows,
    insert_selected_rows,
    safe_identifier,
    tick_size_for,
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-run-id", default="aimv-mwmeta-v3-v7-twin-2026")
    parser.add_argument("--run-id", default="aimv-mwmeta-v3-v7-twin-xq2-2026")
    parser.add_argument("--source-result-run-id", default="")
    parser.add_argument("--results-table", default="entry_exit_template_results_mwmeta_v3_v7_twin_xq2_2026")
    parser.add_argument("--min-slippage-multiple", type=float, default=2.0)
    parser.add_argument("--fallback-slippage-ticks", type=float, default=6.0)
    parser.add_argument("--min-expected-gross-ticks", type=float, default=0.0)
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


def execution_metrics(
    selected: dict[str, Any],
    result: dict[str, Any],
    fallback_slippage_ticks: float,
) -> dict[str, float | None]:
    entry_price = finite(result.get("entry_price"))
    exit_price = finite(result.get("exit_price"))
    risk_points = finite(result.get("risk_points"))
    result_r = finite(result.get("result_r"), 0.0)
    predicted_r = finite(selected.get("predicted_expected_r"), 0.0)
    tick_size = tick_size_for(result.get("symbol") or selected.get("symbol"))
    if (
        entry_price is None
        or exit_price is None
        or risk_points is None
        or result_r is None
        or predicted_r is None
        or tick_size <= 0
        or risk_points <= 0
    ):
        return {
            "risk_ticks": None,
            "inferred_slippage_ticks": max(0.0, fallback_slippage_ticks),
            "expected_net_ticks": None,
            "expected_gross_ticks": None,
            "actual_exit_ticks": None,
        }

    risk_ticks = risk_points / tick_size
    sign = direction_sign(str(result.get("trade_direction") or selected.get("market") or "LONG"))
    raw_exit_ticks = sign * (exit_price - entry_price) / tick_size
    net_result_ticks = result_r * risk_ticks
    inferred_slippage_ticks = raw_exit_ticks - net_result_ticks
    if not math.isfinite(inferred_slippage_ticks) or inferred_slippage_ticks < 0:
        inferred_slippage_ticks = fallback_slippage_ticks
    expected_net_ticks = predicted_r * risk_ticks
    expected_gross_ticks = expected_net_ticks + inferred_slippage_ticks
    actual_exit_ticks = abs(exit_price - entry_price) / tick_size
    return {
        "risk_ticks": risk_ticks,
        "inferred_slippage_ticks": inferred_slippage_ticks,
        "expected_net_ticks": expected_net_ticks,
        "expected_gross_ticks": expected_gross_ticks,
        "actual_exit_ticks": actual_exit_ticks,
    }


def should_take_trade(metrics: dict[str, float | None], args: argparse.Namespace) -> bool:
    expected_gross_ticks = metrics.get("expected_gross_ticks")
    inferred_slippage_ticks = metrics.get("inferred_slippage_ticks")
    if expected_gross_ticks is None or inferred_slippage_ticks is None:
        return False
    required_ticks = max(
        float(args.min_expected_gross_ticks),
        float(args.min_slippage_multiple) * max(0.0, float(inferred_slippage_ticks)),
    )
    return float(expected_gross_ticks) >= required_ticks


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
        "formula": "execution_quality_floor_v1",
        "run_id": args.run_id,
        "source_run_id": args.source_run_id,
        "input_result_run_id": input_result_run_id,
        "target_result_run_id": target_source_run_id,
        "results_table": args.results_table,
        "min_slippage_multiple": args.min_slippage_multiple,
        "fallback_slippage_ticks": args.fallback_slippage_ticks,
        "min_expected_gross_ticks": args.min_expected_gross_ticks,
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
                f"execution_floor_{args.min_slippage_multiple:g}x",
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
        source_result_run_id = args.source_result_run_id or str(source_run.get("source_run_id") or "")
        if not source_result_run_id:
            raise RuntimeError("Could not resolve source result run id.")

        with conn.cursor() as cur:
            cur.execute("SELECT COUNT(*) AS count FROM ai_stage1_multi_valid_eval_runs WHERE multi_valid_eval_run_id = %s", (args.run_id,))
            exists = int(cur.fetchone()["count"]) > 0
        if exists and not args.replace:
            raise RuntimeError(f"Run already exists. Use --replace to rebuild it: {args.run_id}")

        selected_rows = fetch_selected_rows(conn, args.source_run_id)
        result_map = fetch_result_rows(conn, str(source_run.get("results_table")), source_result_run_id)
        if not selected_rows:
            raise RuntimeError(f"No selected rows for source run: {args.source_run_id}")
        if not result_map:
            raise RuntimeError(f"No result rows for source result run: {source_result_run_id}")

        target_source_run_id = args.run_id.replace("aimv-", "", 1)
        reject_reason = f"exec_floor_{args.min_slippage_multiple:g}x_slip"
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

            if is_taken:
                metrics = execution_metrics(selected, result, float(args.fallback_slippage_ticks))
                keep = should_take_trade(metrics, args)
            else:
                keep = False

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
        insert_run_record(conn, source_run, args, target_source_run_id, source_result_run_id)
        insert_selected_rows(conn, args.run_id, target_selected_rows)
        insert_result_rows(conn, args.results_table, target_result_rows)
        summary = insert_filter_summaries(conn, args.run_id, target_selected_rows, reject_reason)
        conn.commit()

        print(
            json.dumps(
                {
                    "run_id": args.run_id,
                    "source_run_id": args.source_run_id,
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
