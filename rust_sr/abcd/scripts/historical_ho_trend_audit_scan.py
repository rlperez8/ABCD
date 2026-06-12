#!/usr/bin/env python3
"""
Batch-create HO trend audit rows for the canvas audit table.

This uses the current live-safe Stage 1 -> Level 2 -> Stage 2 model stack, but
scores historical HO 2m candles contract-by-contract so we can audit the full
history without waiting on the real-time monitor loop.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
import time
from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_scanner_model as scanner
import ai_oracle_start_level2_three_l1_manager as level2
import ai_oracle_start_lightgbm_utils as lgbm_utils
import ai_oracle_start_stage2_l2_confirmation as stage2_l2
import ai_oracle_start_xgboost_utils as xgb_utils
import ai_oracle_trend_start_model as start_model
import ai_wave_rider_research as wave
import live_ninjatrader_trend_monitor as live_monitor
import prototype_live_stage2_terminal_runner as live_runner


DEFAULT_RUN_ID = "ho-mho-audit-full-2m-v1-20210425-20260608"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-id", default=DEFAULT_RUN_ID)
    parser.add_argument("--root", default="HO")
    parser.add_argument("--execution-root", default="MHO")
    parser.add_argument("--timeframe", default="2m", choices=sorted(scanner.TIMEFRAME_TABLES))
    parser.add_argument("--start", default="", help="Optional inclusive UTC start timestamp.")
    parser.add_argument("--end", default="", help="Optional exclusive UTC end timestamp.")
    parser.add_argument("--symbol", default="", help="Optional exact futures contract symbol for smoke tests.")
    parser.add_argument("--limit-symbols", type=int, default=0)
    parser.add_argument("--max-rows-per-symbol", type=int, default=0)
    parser.add_argument("--replace-run", action="store_true")
    parser.add_argument("--write-csv", action="store_true", default=True)
    parser.add_argument("--progress-every-symbols", type=int, default=1)
    parser.add_argument("--event-batch-size", type=int, default=1000)

    parser.add_argument("--governed-run-id", default=live_runner.DEFAULT_GOVERNED_RUN)
    parser.add_argument("--level2-run-id", default=live_runner.DEFAULT_LEVEL2_RUN)
    parser.add_argument("--stage2-run-id", default=live_runner.DEFAULT_STAGE2_RUN)
    parser.add_argument("--allowed-root-directions", default="HO_LONG,HO_SHORT")
    parser.add_argument("--min-stage2-score-filter", type=float, default=None)
    parser.add_argument("--risk-ticks-min-filter", type=float, default=None)
    parser.add_argument("--risk-ticks-max-filter", type=float, default=None)

    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=240.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    parser.add_argument("--max-forward-bars", type=int, default=576)
    parser.add_argument("--time-exit-bars", type=int, default=0)
    parser.add_argument("--dynamic-max-bars", type=int, default=180)
    parser.add_argument("--slippage-entry-ticks", type=float, default=3.0)
    parser.add_argument("--slippage-exit-ticks", type=float, default=3.0)
    return parser.parse_args()


def to_jsonable(value: Any) -> Any:
    if isinstance(value, pd.Timestamp):
        return None if pd.isna(value) else value.isoformat()
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def sql_clean(value: Any) -> Any:
    value = to_jsonable(value)
    if isinstance(value, str):
        return value
    if isinstance(value, pd.Timestamp):
        return None if pd.isna(value) else value.to_pydatetime()
    return value


def event_tuple(row: dict[str, Any]) -> tuple[Any, ...]:
    details = json.dumps(row.get("details") or {}, default=to_jsonable, sort_keys=True)
    return (
        row.get("run_id"),
        row.get("event_uid"),
        row.get("event_type"),
        row.get("instrument"),
        row.get("root_symbol"),
        row.get("model_symbol"),
        row.get("timeframe"),
        pd.Timestamp(row.get("candle_time")).to_pydatetime() if row.get("candle_time") is not None else None,
        pd.Timestamp(row.get("ts_utc")).to_pydatetime() if row.get("ts_utc") is not None else None,
        row.get("direction"),
        row.get("level2_score"),
        row.get("stage2_score"),
        row.get("entry_price"),
        row.get("stop_price"),
        row.get("risk_ticks"),
        row.get("status"),
        details,
    )


def flush_events(conn, rows: list[dict[str, Any]]) -> int:
    if not rows:
        return 0
    tuples = [event_tuple(row) for row in rows]
    with conn.cursor() as cur:
        cur.executemany(
            """
            INSERT INTO ninjatrader_trend_model_events (
                run_id, event_uid, event_type, instrument, root_symbol, model_symbol,
                timeframe, candle_time, ts_utc, direction, level2_score, stage2_score,
                entry_price, stop_price, risk_ticks, status, details_json
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            ON DUPLICATE KEY UPDATE
                instrument = VALUES(instrument),
                root_symbol = VALUES(root_symbol),
                model_symbol = VALUES(model_symbol),
                timeframe = VALUES(timeframe),
                candle_time = VALUES(candle_time),
                ts_utc = VALUES(ts_utc),
                direction = VALUES(direction),
                level2_score = VALUES(level2_score),
                stage2_score = VALUES(stage2_score),
                entry_price = VALUES(entry_price),
                stop_price = VALUES(stop_price),
                risk_ticks = VALUES(risk_ticks),
                status = VALUES(status),
                details_json = VALUES(details_json),
                updated_at = CURRENT_TIMESTAMP
            """,
            tuples,
        )
    conn.commit()
    return len(rows)


def fetch_range(conn, timeframe: str, root: str, start: str, end: str) -> tuple[pd.Timestamp, pd.Timestamp]:
    table_name, _minutes = scanner.table_for_timeframe(timeframe)
    table = scanner.safe_identifier(table_name)
    clauses = ["root_symbol = %s"]
    params: list[Any] = [root]
    if start:
        clauses.append("ts_utc >= %s")
        params.append(pd.Timestamp(start).to_pydatetime())
    if end:
        clauses.append("ts_utc < %s")
        params.append(pd.Timestamp(end).to_pydatetime())
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT MIN(ts_utc) AS min_ts, MAX(ts_utc) AS max_ts
            FROM {table}
            WHERE {" AND ".join(clauses)}
            """,
            params,
        )
        row = cur.fetchone() or {}
    min_ts = row.get("min_ts")
    max_ts = row.get("max_ts")
    if min_ts is None or max_ts is None:
        raise ValueError(f"No {root} {timeframe} candles found")
    return pd.Timestamp(min_ts), pd.Timestamp(max_ts) + pd.Timedelta(minutes=scanner.table_for_timeframe(timeframe)[1])


def fetch_candles(conn, args: argparse.Namespace, start: pd.Timestamp, end: pd.Timestamp) -> pd.DataFrame:
    table_name, _minutes = scanner.table_for_timeframe(args.timeframe)
    table = scanner.safe_identifier(table_name)
    clauses = ["root_symbol = %s", "ts_utc >= %s", "ts_utc < %s"]
    params: list[Any] = [args.root, start.to_pydatetime(), end.to_pydatetime()]
    if args.symbol:
        clauses.append("symbol = %s")
        params.append(args.symbol.upper())
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT root_symbol, symbol, ts_utc,
                   CAST(open AS DOUBLE) AS open,
                   CAST(high AS DOUBLE) AS high,
                   CAST(low AS DOUBLE) AS low,
                   CAST(close AS DOUBLE) AS close,
                   CAST(volume AS DOUBLE) AS volume
            FROM {table}
            WHERE {" AND ".join(clauses)}
            ORDER BY symbol, ts_utc
            """,
            params,
        )
        rows = cur.fetchall()
    frame = pd.DataFrame(rows)
    if frame.empty:
        return frame
    frame["ts_utc"] = pd.to_datetime(frame["ts_utc"], errors="coerce")
    for col in ["open", "high", "low", "close", "volume"]:
        frame[col] = pd.to_numeric(frame[col], errors="coerce")
    return frame.dropna(subset=["root_symbol", "symbol", "ts_utc", "open", "high", "low", "close"]).reset_index(drop=True)


def load_models(args: argparse.Namespace) -> dict[str, Any]:
    governed_dir = live_runner.model_dir(args.governed_run_id)
    governed = live_runner.load_json(governed_dir / "metadata.json")
    rules = governed["rules"]

    min_stage2_score = float(rules["min_stage2_score"])
    if args.min_stage2_score_filter is not None:
        min_stage2_score = max(min_stage2_score, float(args.min_stage2_score_filter))
    risk_min = float(rules["risk_ticks_min"])
    risk_max = float(rules["risk_ticks_max"])
    if args.risk_ticks_min_filter is not None:
        risk_min = max(risk_min, float(args.risk_ticks_min_filter))
    if args.risk_ticks_max_filter is not None:
        risk_max = min(risk_max, float(args.risk_ticks_max_filter))

    allowed_root_directions = live_runner.parse_csv_strings(args.allowed_root_directions)
    if not allowed_root_directions:
        allowed_root_directions = set(str(item) for item in rules["root_directions"])

    l2_dir = live_runner.model_dir(args.level2_run_id)
    l2_meta = live_runner.load_json(l2_dir / "metadata.json")
    l2_run_ids = l2_meta["level1_model_runs"]
    l2_args = argparse.Namespace(
        manager_engines="cat,light,xgb",
        cat_l1_run_id=l2_run_ids["catboost"],
        light_l1_run_id=l2_run_ids["lightgbm"],
        xgb_l1_run_id=l2_run_ids["xgboost"],
        cat_l1_prefix="",
        light_l1_prefix="",
        xgb_l1_prefix="",
        timeframe=args.timeframe,
        roots="",
        level1_train_years="2024",
        level1_valid_year=2026,
    )

    stage2_dir = live_runner.model_dir(args.stage2_run_id)
    stage2_meta = live_runner.load_json(stage2_dir / "metadata.json")

    return {
        "governed": governed,
        "rules": rules,
        "allowed_root_directions": allowed_root_directions,
        "risk_min": risk_min,
        "risk_max": risk_max,
        "l2_meta": l2_meta,
        "l2_args": l2_args,
        "l2_threshold": float(l2_meta["selected_threshold"]),
        "stage2_meta": stage2_meta,
        "stage2_threshold": float(stage2_meta["selected_stage2_threshold"]),
        "min_stage2_score": min_stage2_score,
        "models": {
            "cat": live_runner.load_catboost_model(
                live_runner.model_dir(l2_run_ids["catboost"]) / "catboost_oracle_start_live_grid_model.cbm"
            ),
            "light": lgbm_utils.load_model(live_runner.model_dir(l2_run_ids["lightgbm"])),
            "xgb": xgb_utils.load_model(live_runner.model_dir(l2_run_ids["xgboost"])),
            "level2": live_runner.load_catboost_model(l2_dir / "catboost_oracle_start_level2_three_l1_manager.cbm"),
            "stage2": live_runner.load_catboost_model(stage2_dir / "catboost_stage2_l2_confirmation.cbm"),
        },
    }


def score_contract(
    symbol: str,
    root: str,
    group_raw: pd.DataFrame,
    args: argparse.Namespace,
    bundle: dict[str, Any],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    group = scanner.enrich_candles(
        group_raw[["ts_utc", "open", "high", "low", "close", "volume"]].copy().sort_values("ts_utc").reset_index(drop=True),
        args,
    )
    if args.max_rows_per_symbol > 0:
        group = group.tail(int(args.max_rows_per_symbol)).reset_index(drop=True)
    if len(group) < 80:
        return [], [], {"symbol": symbol, "candles": int(len(group)), "base_rows": 0}

    directions = live_runner.direction_for_root(root, bundle["allowed_root_directions"])
    if not directions:
        return [], [], {"symbol": symbol, "candles": int(len(group)), "base_rows": 0, "skipped": "direction"}

    _table, timeframe_minutes = scanner.table_for_timeframe(args.timeframe)
    max_confirm_bars = int(bundle["stage2_meta"]["max_confirm_bars"])
    cooldown_bars = int(bundle["l2_meta"]["event_rules"]["cooldown_bars"])

    base_rows: list[dict[str, Any]] = []
    max_signal_idx = max(60, len(group) - max_confirm_bars - 3)
    for signal_idx in range(60, max_signal_idx):
        signal_year = int(pd.Timestamp(group["ts_utc"].iloc[signal_idx]).year)
        for direction in directions:
            row = start_model.feature_row(signal_year, root, symbol, group, signal_idx, direction)
            row["signal_idx"] = int(signal_idx)
            row["source_timeframe"] = str(args.timeframe)
            row["timeframe_minutes"] = float(timeframe_minutes)
            base_rows.append(row)

    if not base_rows:
        return [], [], {"symbol": symbol, "candles": int(len(group)), "base_rows": 0}

    scored = live_runner.score_level1_and_level2(base_rows, bundle["models"], bundle["l2_args"], args.timeframe)
    level2_events: list[dict[str, Any]] = []
    next_allowed: dict[tuple[str, str], int] = {}
    for row in scored.sort_values(["signal_idx", "direction"]).to_dict("records"):
        score = wave.finite(row.get("level2_score"), 0.0) or 0.0
        if score < float(bundle["l2_threshold"]):
            continue
        key = (str(row["symbol"]), str(row["direction"]))
        signal_idx = int(row["signal_idx"])
        if signal_idx < next_allowed.get(key, -1):
            continue
        next_allowed[key] = signal_idx + cooldown_bars + 1
        row["stage1_pick"] = 1
        row["stage1_score"] = score
        level2_events.append(row)

    stage2_rows: list[dict[str, Any]] = []
    for event in level2_events:
        signal_idx = int(event["signal_idx"])
        direction = str(event["direction"])
        tick_size = wave.tick_size_for(symbol, root)
        atr_ticks = wave.finite(event.get("atr_ticks"), None)
        atr_hint = atr_ticks * tick_size if atr_ticks is not None and tick_size > 0 else None
        for offset in range(1, max_confirm_bars + 1):
            features = live_runner.adaptive_stage2.post_features_for_offset(
                group,
                signal_idx,
                direction,
                offset,
                max_confirm_bars,
                atr_hint,
            )
            if features is None:
                continue
            payload = dict(event)
            payload.update(features)
            stage2_rows.append(payload)

    if not stage2_rows:
        return [], [], {
            "symbol": symbol,
            "candles": int(len(group)),
            "base_rows": int(len(base_rows)),
            "level2_events": int(len(level2_events)),
            "stage2_rows": 0,
        }

    stage2_frame = pd.DataFrame(stage2_rows)
    stage2_frame["stage2_score"] = bundle["models"]["stage2"].predict_proba(
        stage2_l2.prepare_pool(stage2_frame, include_target=False)
    )[:, 1]
    hits = stage2_frame[
        pd.to_numeric(stage2_frame["stage2_score"], errors="coerce").fillna(0.0) >= float(bundle["stage2_threshold"])
    ].copy()
    if hits.empty:
        return [], [], {
            "symbol": symbol,
            "candles": int(len(group)),
            "base_rows": int(len(base_rows)),
            "level2_events": int(len(level2_events)),
            "stage2_rows": int(len(stage2_rows)),
            "trend_confirms": 0,
        }

    confirmed = hits.sort_values(["candidate_uid", "confirm_offset_bars"]).drop_duplicates("candidate_uid")
    events: list[dict[str, Any]] = []
    trades: list[dict[str, Any]] = []
    instrument = f"{root}->{args.execution_root} AUDIT"
    for row in confirmed.to_dict("records"):
        uid = str(row["candidate_uid"])
        direction = str(row["direction"])
        signal_idx = int(row["signal_idx"])
        confirm_offset = int(row["confirm_offset_bars"])
        confirm_idx = signal_idx + confirm_offset
        entry_idx = confirm_idx + 1
        confirm_date = pd.Timestamp(row.get("confirm_date") or group["ts_utc"].iloc[confirm_idx])
        stage2_score = wave.finite(row.get("stage2_score"), 0.0) or 0.0
        status = "confirmed" if stage2_score >= float(bundle["min_stage2_score"]) else "rejected_min_stage2_score"

        events.append(
            {
                "run_id": args.run_id,
                "event_uid": f"{args.run_id}|trend_confirmed|{uid}",
                "event_type": "trend_confirmed",
                "instrument": instrument,
                "root_symbol": root,
                "model_symbol": symbol,
                "timeframe": args.timeframe,
                "candle_time": confirm_date,
                "ts_utc": confirm_date,
                "direction": direction,
                "level2_score": wave.finite(row.get("stage1_score"), None),
                "stage2_score": float(stage2_score),
                "status": status,
                "details": {
                    "candidate_uid": uid,
                    "signal_date": row.get("signal_date"),
                    "signal_idx": signal_idx,
                    "confirm_date": confirm_date,
                    "confirm_idx": confirm_idx,
                    "confirm_offset_bars": confirm_offset,
                    "execution_root": args.execution_root,
                    "execution_price_source": root,
                    "audit_note": "Historical HO signal audit. Intended execution contract is MHO.",
                },
            }
        )
        if status != "confirmed" or entry_idx >= len(group):
            continue

        tick_size = wave.tick_size_for(symbol, root)
        entry_price = wave.finite(group["open"].iloc[entry_idx], None)
        stop_price, risk_points = wave.initial_stop(group, entry_idx, direction, tick_size, args)
        risk_ticks = (risk_points / tick_size) if risk_points is not None and tick_size > 0 else None
        entry_date = pd.Timestamp(group["ts_utc"].iloc[entry_idx])
        risk_ok = (
            entry_price is not None
            and stop_price is not None
            and risk_points is not None
            and risk_ticks is not None
            and float(bundle["risk_min"]) <= float(risk_ticks) <= float(bundle["risk_max"])
        )
        entry_status = "accepted" if risk_ok else "rejected_risk"
        events.append(
            {
                "run_id": args.run_id,
                "event_uid": f"{args.run_id}|paper_entry|{uid}|{entry_idx}",
                "event_type": "paper_entry_audit",
                "instrument": instrument,
                "root_symbol": root,
                "model_symbol": symbol,
                "timeframe": args.timeframe,
                "candle_time": entry_date,
                "ts_utc": entry_date,
                "direction": direction,
                "level2_score": wave.finite(row.get("stage1_score"), None),
                "stage2_score": float(stage2_score),
                "entry_price": float(entry_price) if entry_price is not None else None,
                "stop_price": float(stop_price) if stop_price is not None else None,
                "risk_ticks": float(risk_ticks) if risk_ticks is not None else None,
                "status": entry_status,
                "details": {
                    "candidate_uid": uid,
                    "confirm_date": confirm_date,
                    "entry_idx": entry_idx,
                    "execution_root": args.execution_root,
                    "execution_price_source": root,
                },
            }
        )
        if not risk_ok:
            continue

        terminal = live_runner.terminal_path_from_entry(
            group,
            entry_idx,
            direction,
            float(entry_price),
            float(stop_price),
            float(risk_points),
            float(tick_size),
            args,
        )
        if terminal is None:
            continue
        exit_idx = int(terminal["exit_idx"])
        exit_date = pd.Timestamp(terminal["exit_date"])
        trade_id = f"{args.run_id}|{uid}|audit_terminal_180"
        trade = {
            "trade_id": trade_id,
            "candidate_uid": uid,
            "root_symbol": root,
            "execution_root": args.execution_root,
            "symbol": symbol,
            "timeframe": args.timeframe,
            "direction": direction,
            "signal_date": row.get("signal_date"),
            "confirm_date": confirm_date,
            "entry_date": entry_date,
            "exit_date": exit_date,
            "signal_idx": signal_idx,
            "confirm_idx": confirm_idx,
            "entry_idx": entry_idx,
            "exit_idx": exit_idx,
            "entry_price": float(entry_price),
            "stop_price": float(stop_price),
            "exit_price": float(terminal["exit_price"]),
            "risk_ticks": float(risk_ticks),
            "raw_result_r": float(terminal["raw_result_r"]),
            "slippage_r": float(terminal["slippage_r"]),
            "result_r": float(terminal["result_r"]),
            "exit_reason": str(terminal["exit_reason"]),
            "level2_score": wave.finite(row.get("stage1_score"), None),
            "stage2_score": float(stage2_score),
            "confirm_offset_bars": confirm_offset,
        }
        trades.append(trade)
        events.append(
            {
                "run_id": args.run_id,
                "event_uid": f"{args.run_id}|paper_exit|{uid}|{exit_idx}",
                "event_type": "paper_exit_audit",
                "instrument": instrument,
                "root_symbol": root,
                "model_symbol": symbol,
                "timeframe": args.timeframe,
                "candle_time": exit_date,
                "ts_utc": exit_date,
                "direction": direction,
                "level2_score": wave.finite(row.get("stage1_score"), None),
                "stage2_score": float(stage2_score),
                "entry_price": float(terminal["exit_price"]),
                "stop_price": float(stop_price),
                "risk_ticks": float(risk_ticks),
                "status": "closed",
                "details": {
                    "candidate_uid": uid,
                    "trade_id": trade_id,
                    "entry_date": entry_date,
                    "entry_price": float(entry_price),
                    "exit_idx": exit_idx,
                    "exit_reason": str(terminal["exit_reason"]),
                    "raw_result_r": float(terminal["raw_result_r"]),
                    "slippage_r": float(terminal["slippage_r"]),
                    "result_r": float(terminal["result_r"]),
                    "execution_root": args.execution_root,
                    "execution_price_source": root,
                },
            }
        )

    summary = {
        "symbol": symbol,
        "candles": int(len(group)),
        "base_rows": int(len(base_rows)),
        "level2_events": int(len(level2_events)),
        "stage2_rows": int(len(stage2_rows)),
        "trend_confirms": int(len(confirmed)),
        "accepted_trades": int(len(trades)),
        "sum_r": float(sum(float(item["result_r"]) for item in trades)),
    }
    return events, trades, summary


def main() -> int:
    args = parse_args()
    scanner.configure_wave_args(args)
    started = time.perf_counter()
    conn = wave.connect()
    try:
        live_monitor.ensure_event_table(conn)
        start_at, end_at = fetch_range(conn, args.timeframe, args.root.upper(), args.start, args.end)
        raw = fetch_candles(conn, args, start_at, end_at)
        if raw.empty:
            raise ValueError(f"No candles loaded for {args.root} {args.timeframe}")
        if args.replace_run:
            with conn.cursor() as cur:
                cur.execute("DELETE FROM ninjatrader_trend_model_events WHERE run_id = %s", (args.run_id,))
            conn.commit()

        bundle = load_models(args)
        symbols = sorted(raw["symbol"].dropna().astype(str).unique())
        if args.limit_symbols > 0:
            symbols = symbols[: int(args.limit_symbols)]
        output_dir = wave.ABCD_ROOT / "model_registry" / args.run_id
        output_dir.mkdir(parents=True, exist_ok=True)

        all_trades: list[dict[str, Any]] = []
        summaries: list[dict[str, Any]] = []
        buffered_events: list[dict[str, Any]] = []
        written_events = 0
        for symbol_index, symbol in enumerate(symbols, start=1):
            group_raw = raw[raw["symbol"].astype(str) == str(symbol)].copy()
            root = str(group_raw["root_symbol"].iloc[0]).upper()
            events, trades, summary = score_contract(str(symbol), root, group_raw, args, bundle)
            summaries.append(summary)
            all_trades.extend(trades)
            buffered_events.extend(events)
            if len(buffered_events) >= int(args.event_batch_size):
                written_events += flush_events(conn, buffered_events)
                buffered_events.clear()
            if int(args.progress_every_symbols) > 0 and symbol_index % int(args.progress_every_symbols) == 0:
                print(
                    json.dumps(
                        {
                            "progress_symbol": symbol_index,
                            "symbols_total": len(symbols),
                            "symbol": symbol,
                            "candles": summary.get("candles", 0),
                            "trend_confirms": summary.get("trend_confirms", 0),
                            "accepted_trades": summary.get("accepted_trades", 0),
                            "sum_r": summary.get("sum_r", 0.0),
                        },
                        default=to_jsonable,
                    ),
                    flush=True,
                )
        written_events += flush_events(conn, buffered_events)

        trades_frame = pd.DataFrame(all_trades)
        summary_frame = pd.DataFrame(summaries)
        if args.write_csv:
            trades_frame.to_csv(output_dir / "audit_trades.csv", index=False)
            summary_frame.to_csv(output_dir / "symbol_summary.csv", index=False)
        result_values = pd.to_numeric(trades_frame.get("result_r", pd.Series(dtype=float)), errors="coerce").fillna(0.0)
        metadata = {
            "run_id": args.run_id,
            "root_symbol": args.root.upper(),
            "execution_root": args.execution_root.upper(),
            "timeframe": args.timeframe,
            "start_at": start_at,
            "end_at": end_at,
            "symbols": len(symbols),
            "source_rows": int(len(raw)),
            "events_written": int(written_events),
            "trades": int(len(trades_frame)),
            "sum_r": float(result_values.sum()) if len(result_values) else 0.0,
            "avg_r": float(result_values.mean()) if len(result_values) else 0.0,
            "win_rate": float((result_values > 0).mean()) if len(result_values) else 0.0,
            "governed_run_id": args.governed_run_id,
            "level2_run_id": args.level2_run_id,
            "stage2_run_id": args.stage2_run_id,
            "elapsed_seconds": float(time.perf_counter() - started),
            "note": "HO candles and model prices; intended execution audit root is MHO.",
        }
        (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")
        print(json.dumps(metadata, default=to_jsonable), flush=True)
    finally:
        conn.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
