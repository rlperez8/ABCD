#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import re
import time
from datetime import datetime
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

import pymysql
import requests


def load_dotenv(path: Path) -> None:
    if not path.exists():
        return
    for raw_line in path.read_text().splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        os.environ.setdefault(key.strip(), value.strip().strip('"').strip("'"))


def parse_database_url(database_url: str) -> dict[str, Any]:
    parsed = urlparse(database_url)
    return {
        "host": parsed.hostname or "127.0.0.1",
        "port": parsed.port or 3306,
        "user": parsed.username or "",
        "password": parsed.password or "",
        "database": parsed.path.lstrip("/"),
        "charset": "utf8mb4",
        "autocommit": False,
        "cursorclass": pymysql.cursors.DictCursor,
    }


def connect() -> pymysql.Connection:
    load_dotenv(Path("rust_sr/abcd/.env"))
    database_url = os.getenv("ABCD_DATABASE_URL") or os.getenv("DATABASE_URL")
    if not database_url:
        raise RuntimeError("Missing DATABASE_URL")
    return pymysql.connect(**parse_database_url(database_url))


def parse_time(value: str | None) -> datetime | None:
    if not value:
        return None
    text = str(value).replace("Z", "").replace(" ", "T")
    try:
        return datetime.fromisoformat(text)
    except ValueError:
        return None


def fetch_trade(server_url: str, signal_uid: str, account_name: str, root_symbol: str) -> dict[str, Any]:
    response = requests.post(
        f"{server_url.rstrip('/')}/ninjatrader/signals/history",
        json={
            "account_name": account_name,
            "root_symbol": root_symbol,
            "include_cancelled": True,
            "limit": 50,
        },
        timeout=10,
    )
    response.raise_for_status()
    for row in response.json().get("rows", []):
        if row.get("signal_uid") == signal_uid:
            return row
    raise RuntimeError(f"Signal not found in history: {signal_uid}")


def fetch_candles(
    server_url: str,
    root_symbol: str,
    timeframe: str,
    instrument: str,
    start_date: str,
) -> list[dict[str, Any]]:
    response = requests.post(
        f"{server_url.rstrip('/')}/ninjatrader/live-candles",
        json={
            "root_symbol": root_symbol,
            "timeframe": timeframe,
            "instrument": instrument,
            "start_date": start_date,
            "limit": 5000,
        },
        timeout=20,
    )
    response.raise_for_status()
    return response.json().get("rows", [])


def complete_signal(conn: pymysql.Connection, trade: dict[str, Any], exit_candle: dict[str, Any]) -> dict[str, Any]:
    signal_uid = str(trade["signal_uid"])
    side = str(trade.get("side") or "").upper()
    entry_price = float(
        trade.get("entry_execution_price")
        or trade.get("actual_trigger_price")
        or trade.get("expected_price")
    )
    exit_price = float(exit_candle["close"])
    tick_size = float(trade.get("tick_size") or 0.0001)
    quantity = int(trade.get("quantity") or 1)
    accounting_tick_value = float(trade.get("accounting_tick_value") or 0.42)
    execution_tick_value = float(trade.get("execution_tick_value") or 4.2)
    if side == "SHORT":
        realized_ticks = (entry_price - exit_price) / tick_size
        exit_action = "BuyToCover"
    else:
        realized_ticks = (exit_price - entry_price) / tick_size
        exit_action = "Sell"

    realized_accounting = realized_ticks * accounting_tick_value * quantity
    realized_execution = realized_ticks * execution_tick_value * quantity
    exit_time = str(exit_candle["candle_time"]).replace("T", " ")
    exit_order_id = f"MANUAL_ACTUAL_BAR_180_EXIT_{signal_uid}"
    status_message = (
        "manual synthetic actual-180-bar time exit applied; "
        f"entry={entry_price:.4f}; exit={exit_price:.4f}; "
        f"ticks={realized_ticks:.1f}; exit_candle_id={exit_candle.get('id')}"
    )

    with conn.cursor() as cur:
        cur.execute(
            """
            UPDATE ninjatrader_order_signals
            SET status='completed',
                exit_order_id=%s,
                exit_price=%s,
                exit_action=%s,
                exit_order_name='ABCD_TIME_EXIT',
                exit_received_at=%s,
                realized_ticks=%s,
                realized_execution_dollars=%s,
                realized_accounting_dollars=%s,
                status_message=%s
            WHERE signal_uid=%s
            """,
            (
                exit_order_id,
                exit_price,
                exit_action,
                exit_time,
                realized_ticks,
                realized_execution,
                realized_accounting,
                status_message,
                signal_uid,
            ),
        )
    conn.commit()
    return {
        "signal_uid": signal_uid,
        "exit_time": exit_time,
        "exit_price": exit_price,
        "realized_ticks": realized_ticks,
        "realized_accounting_dollars": realized_accounting,
        "realized_execution_dollars": realized_execution,
    }


def reset_manual_completion(conn: pymysql.Connection, signal_uid: str) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            UPDATE ninjatrader_order_signals
            SET status='triggered',
                exit_order_id=NULL,
                exit_price=NULL,
                exit_action=NULL,
                exit_order_name=NULL,
                exit_received_at=NULL,
                realized_ticks=NULL,
                realized_execution_dollars=NULL,
                realized_accounting_dollars=NULL,
                status_message='execution received | waiting for actual 180-bar synthetic time exit'
            WHERE signal_uid=%s
              AND status='completed'
              AND COALESCE(exit_order_name, '')='ABCD_TIME_EXIT'
              AND COALESCE(status_message, '') LIKE 'manual synthetic 180-bar time exit applied%%'
            """,
            (signal_uid,),
        )
    conn.commit()


def main() -> int:
    parser = argparse.ArgumentParser(description="Wait for actual NT bar 180 and apply synthetic time exit.")
    parser.add_argument("--signal-uid", required=True)
    parser.add_argument("--server-url", default="http://127.0.0.1:8080")
    parser.add_argument("--account-name", default="DEMO5859105")
    parser.add_argument("--root-symbol", default="HO")
    parser.add_argument("--timeframe", default="2m")
    parser.add_argument("--target-bars", type=int, default=180)
    parser.add_argument("--poll-seconds", type=float, default=10.0)
    parser.add_argument("--reset-wrong-manual-exit", action="store_true")
    args = parser.parse_args()

    conn = connect()
    try:
        if args.reset_wrong_manual_exit:
            reset_manual_completion(conn, args.signal_uid)

        while True:
            trade = fetch_trade(args.server_url, args.signal_uid, args.account_name, args.root_symbol)
            if str(trade.get("status") or "").lower() == "completed":
                message = str(trade.get("status_message") or "")
                if "manual synthetic actual-180-bar" in message:
                    print({"event": "already_completed_by_watcher", "signal_uid": args.signal_uid}, flush=True)
                else:
                    print({"event": "already_completed_elsewhere", "signal_uid": args.signal_uid, "message": message}, flush=True)
                return 0

            entry_time = trade.get("entry_execution_time") or trade.get("expected_time")
            entry_dt = parse_time(entry_time)
            if entry_dt is None:
                raise RuntimeError(f"Could not parse entry time: {entry_time}")

            instrument = str(trade.get("instrument") or "").upper()
            candles = fetch_candles(
                args.server_url,
                args.root_symbol,
                args.timeframe,
                instrument,
                entry_dt.strftime("%Y-%m-%dT%H:%M:%S"),
            )
            after_entry = [
                row for row in candles
                if (parse_time(row.get("candle_time")) and parse_time(row.get("candle_time")) > entry_dt)
            ]
            count = len(after_entry)
            latest = after_entry[-1] if after_entry else None
            print(
                {
                    "event": "bar_count",
                    "signal_uid": args.signal_uid,
                    "bars": count,
                    "remaining": max(0, args.target_bars - count),
                    "latest": latest.get("candle_time") if latest else None,
                    "latest_close": latest.get("close") if latest else None,
                },
                flush=True,
            )
            if count >= args.target_bars:
                exit_candle = after_entry[args.target_bars - 1]
                result = complete_signal(conn, trade, exit_candle)
                print({"event": "completed", **result}, flush=True)
                return 0

            time.sleep(max(1.0, args.poll_seconds))
    finally:
        conn.close()


if __name__ == "__main__":
    raise SystemExit(main())
