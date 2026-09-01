#!/usr/bin/env python3
"""
Live monitor for NinjaTrader candles.

The monitor watches `ninjatrader_live_candles`, scores each new closed candle
with the current Stage 1 -> Level 2 -> Stage 2 trend model, and logs events to
`ninjatrader_trend_model_events`. By default it is read-only. With
`--queue-orders`, it queues demo NinjaTrader order signals after Stage 2 confirms.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import math
import re
import sys
import time
from hashlib import sha1
from pathlib import Path
from typing import Any
from urllib import request
from urllib.error import URLError
from zoneinfo import ZoneInfo

import numpy as np
import pandas as pd

try:
    from catboost import CatBoostClassifier
except ImportError as exc:  # pragma: no cover
    raise SystemExit("CatBoost is required. Run with .venv_ai\\Scripts\\python.exe") from exc


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_scanner_model as scanner
import ai_oracle_start_lightgbm_utils as lgbm_utils
import ai_oracle_start_stage2_l2_confirmation as stage2_l2
import ai_oracle_start_xgboost_utils as xgb_utils
import ai_oracle_trend_start_model as start_model
import ai_wave_rider_research as wave
import notify_ninjatrader_completed_trades as trade_email
import prototype_live_stage2_terminal_runner as live_runner


DEFAULT_GOVERNED_RUN = "aicw-os-stage3-terminal-governed-clnq-gflong-v2"
DEFAULT_LEVEL2_RUN = "aicw-os-l2-three-l1-2m-p1p1n6m3-v1-ALL-2m-tr2025-v2026"
DEFAULT_STAGE2_RUN = "aicw-os-stage2-l2-confirm-v1-2m-m-tr2025-v2026-m16-v2026"

MONTH_CODES = {
    "JAN": "F",
    "FEB": "G",
    "MAR": "H",
    "APR": "J",
    "MAY": "K",
    "JUN": "M",
    "JUL": "N",
    "AUG": "Q",
    "SEP": "U",
    "OCT": "V",
    "NOV": "X",
    "DEC": "Z",
    "01": "F",
    "02": "G",
    "03": "H",
    "04": "J",
    "05": "K",
    "06": "M",
    "07": "N",
    "08": "Q",
    "09": "U",
    "10": "V",
    "11": "X",
    "12": "Z",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-id", default="nt-live-ho-2m-trend-monitor-v1")
    parser.add_argument("--root", default="HO")
    parser.add_argument("--timeframe", default="2m")
    parser.add_argument("--instrument", default="", help="Optional exact NT instrument. Empty uses the latest instrument for root/timeframe.")
    parser.add_argument("--year", type=int, default=2026)
    parser.add_argument("--local-timezone", default="America/Chicago")
    parser.add_argument("--poll-seconds", type=float, default=5.0)
    parser.add_argument("--heartbeat-minutes", type=float, default=30.0)
    parser.add_argument("--lookback-rows", type=int, default=5000)
    parser.add_argument("--backfill-cycles", type=int, default=0, help="For smoke tests only; process this many existing closed candles.")
    parser.add_argument("--max-loops", type=int, default=0, help="Stop after N polling loops. Zero runs forever.")
    parser.add_argument("--max-new-cycles-per-loop", type=int, default=20)
    parser.add_argument("--stale-feed-seconds", type=float, default=360.0, help="Pause new trend/trade decisions when the newest closed candle is older than this many seconds. Zero disables.")
    parser.add_argument("--heartbeat-stale-seconds", type=float, default=20.0, help="Pause new trend/trade decisions when the latest NT heartbeat is older than this many seconds. Zero disables.")
    parser.add_argument("--require-heartbeat", action=argparse.BooleanOptionalAction, default=True, help="Require a fresh NT heartbeat before scoring live candles.")
    parser.add_argument("--gap-tolerance-bars", type=float, default=1.5, help="Reset live scanner state when new closed candles jump by more than this many expected bars.")

    parser.add_argument("--governed-run-id", default=DEFAULT_GOVERNED_RUN)
    parser.add_argument("--level2-run-id", default=DEFAULT_LEVEL2_RUN)
    parser.add_argument("--stage2-run-id", default=DEFAULT_STAGE2_RUN)
    parser.add_argument("--allowed-root-directions", default="HO_LONG,HO_SHORT")

    parser.add_argument("--queue-orders", action="store_true", help="Queue demo NinjaTrader signals when Stage 2 confirms.")
    parser.add_argument("--server-url", default="http://127.0.0.1:8080")
    parser.add_argument("--signal-account", default="DEMO5859105")
    parser.add_argument("--signal-instrument", default="", help="Execution instrument. Empty uses the candle instrument.")
    parser.add_argument("--execution-root", default="", help="Optional execution root, e.g. MHO while detecting on HO.")
    parser.add_argument("--signal-quantity", type=int, default=1)
    parser.add_argument("--max-active-signals", type=int, default=1)
    parser.add_argument("--sim-account-cash", type=float, default=0.0, help="Optional account cash guard. Zero disables the guard.")
    parser.add_argument("--execution-tick-value", type=float, default=0.0, help="Dollar value per execution tick. Zero disables dollar risk checks.")
    parser.add_argument("--accounting-instrument", default="", help="Optional synthetic accounting instrument, e.g. MHO JUL26 while demo orders are routed on HO JUL26.")
    parser.add_argument("--accounting-root", default="", help="Optional synthetic accounting root. Empty derives it from accounting-instrument.")
    parser.add_argument("--accounting-tick-value", type=float, default=0.0, help="Dollar value per tick for internal accounting. Use 0.42 to track HO fills as MHO-equivalent risk.")
    parser.add_argument("--accounting-size-ratio", type=float, default=0.0, help="Optional size ratio versus execution instrument, e.g. 0.1 for micro-equivalent accounting.")
    parser.add_argument("--max-accounting-risk-dollars", type=float, default=0.0, help="Optional max allowed accounting stop risk per signal. Zero disables this guard.")
    parser.add_argument("--disable-completed-trade-email", action="store_true", help="Disable automatic completed-trade email notifications.")

    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=240.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    parser.add_argument("--dynamic-max-bars", type=int, default=180)
    parser.add_argument("--max-forward-bars", type=int, default=576)
    parser.add_argument("--time-exit-bars", type=int, default=0)
    parser.add_argument("--slippage-entry-ticks", type=float, default=3.0)
    parser.add_argument("--slippage-exit-ticks", type=float, default=3.0)
    return parser.parse_args()


def model_dir(run_id: str) -> Path:
    path = Path(run_id)
    if path.exists():
        return path.resolve()
    return wave.ABCD_ROOT / "model_registry" / run_id


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def load_catboost_model(path: Path) -> CatBoostClassifier:
    model = CatBoostClassifier()
    model.load_model(str(path))
    return model


def parse_csv_strings(value: str) -> set[str]:
    return {part.strip().upper() for part in str(value or "").split(",") if part.strip()}


def to_jsonable(value: Any) -> Any:
    if isinstance(value, pd.Timestamp):
        return None if pd.isna(value) else value.isoformat()
    if isinstance(value, (dt.datetime, dt.date)):
        return value.isoformat()
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def nt_instrument_to_model_symbol(instrument: str, root: str) -> str:
    text = str(instrument or "").upper().strip()
    root = str(root or "").upper().strip()
    compact = re.sub(r"\s+", " ", text)

    month_match = re.search(r"\b([FGHJKMNQUVXZ])(\d{1,2})\b", compact)
    if month_match:
        return f"{root}{month_match.group(1)}{month_match.group(2)[-1]}"

    name_match = re.search(r"\b(JAN|FEB|MAR|APR|MAY|JUN|JUL|AUG|SEP|OCT|NOV|DEC)(\d{2,4})\b", compact)
    if name_match:
        return f"{root}{MONTH_CODES[name_match.group(1)]}{name_match.group(2)[-1]}"

    numeric_match = re.search(r"\b(\d{1,2})[-/](\d{2,4})\b", compact)
    if numeric_match:
        month = numeric_match.group(1).zfill(2)
        return f"{root}{MONTH_CODES.get(month, month)}{numeric_match.group(2)[-1]}"

    safe_tail = re.sub(r"[^A-Z0-9]", "", compact.replace(root, "", 1))
    return f"{root}{safe_tail}" if safe_tail else root


def local_naive_to_utc_naive(values: pd.Series, timezone_name: str) -> pd.Series:
    zone = ZoneInfo(timezone_name)
    local_times = pd.to_datetime(values, errors="coerce")
    return local_times.map(
        lambda value: pd.NaT
        if pd.isna(value)
        else pd.Timestamp(value).tz_localize(zone).tz_convert("UTC").tz_localize(None)
    )


def ensure_event_table(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ninjatrader_trend_model_events (
                id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                run_id VARCHAR(128) NOT NULL,
                event_uid VARCHAR(255) NOT NULL,
                event_type VARCHAR(64) NOT NULL,
                instrument VARCHAR(128) NULL,
                root_symbol VARCHAR(32) NULL,
                model_symbol VARCHAR(64) NULL,
                timeframe VARCHAR(16) NULL,
                candle_time DATETIME NULL,
                ts_utc DATETIME NULL,
                direction VARCHAR(16) NULL,
                level2_score DOUBLE NULL,
                stage2_score DOUBLE NULL,
                entry_price DOUBLE NULL,
                stop_price DOUBLE NULL,
                risk_ticks DOUBLE NULL,
                status VARCHAR(64) NULL,
                details_json LONGTEXT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                UNIQUE KEY ux_nt_trend_event_uid (event_uid),
                KEY idx_nt_trend_run_created (run_id, created_at),
                KEY idx_nt_trend_symbol_time (root_symbol, timeframe, ts_utc),
                KEY idx_nt_trend_type (event_type, status)
            )
            """
        )
    conn.commit()


def ensure_route_table(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ninjatrader_live_screener_routes (
                id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                route_uid VARCHAR(255) NOT NULL,
                run_id VARCHAR(128) NOT NULL,
                instrument VARCHAR(128) NULL,
                root_symbol VARCHAR(32) NULL,
                model_symbol VARCHAR(64) NULL,
                timeframe VARCHAR(16) NULL,
                candle_time DATETIME NULL,
                ts_utc DATETIME NULL,
                candle_id BIGINT NULL,
                candle_revision_id BIGINT NULL,
                candle_payload_hash VARCHAR(64) NULL,
                open DOUBLE NULL,
                high DOUBLE NULL,
                low DOUBLE NULL,
                close DOUBLE NULL,
                volume DOUBLE NULL,
                stage1_rows INT NULL,
                stage1_status VARCHAR(64) NULL,
                level2_threshold DOUBLE NULL,
                level2_long_score DOUBLE NULL,
                level2_short_score DOUBLE NULL,
                level2_best_direction VARCHAR(16) NULL,
                level2_best_score DOUBLE NULL,
                level2_picks INT NULL,
                stage2_rows INT NULL,
                stage2_status VARCHAR(64) NULL,
                stage2_threshold DOUBLE NULL,
                stage2_best_direction VARCHAR(16) NULL,
                stage2_best_score DOUBLE NULL,
                stage2_confirmed_direction VARCHAR(16) NULL,
                stage2_confirmed_score DOUBLE NULL,
                stage2_confirm_offset_bars INT NULL,
                pending_after INT NULL,
                scheduled_entries_after INT NULL,
                order_signals INT NULL,
                order_rejects INT NULL,
                decision_status VARCHAR(64) NULL,
                details_json LONGTEXT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                UNIQUE KEY ux_nt_live_route_uid (route_uid),
                KEY idx_nt_live_route_run_time (run_id, ts_utc),
                KEY idx_nt_live_route_symbol_time (root_symbol, timeframe, ts_utc),
                KEY idx_nt_live_route_status (decision_status, stage2_status),
                KEY idx_nt_live_route_candle_revision (candle_revision_id)
            )
            """
        )
    conn.commit()


def ensure_feed_health_tables(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ninjatrader_feed_heartbeats (
                id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                external_key VARCHAR(255) NOT NULL,
                source VARCHAR(64) NOT NULL DEFAULT 'ninjatrader',
                bridge_version VARCHAR(32) NULL,
                client_id VARCHAR(128) NULL,
                instrument VARCHAR(64) NULL,
                root_symbol VARCHAR(32) NULL,
                exchange_name VARCHAR(32) NULL,
                timeframe VARCHAR(32) NULL,
                bars_period_type VARCHAR(32) NULL,
                bars_period_value BIGINT NULL,
                heartbeat_time_utc DATETIME(6) NULL,
                last_candle_time DATETIME(6) NULL,
                last_candle_time_utc DATETIME(6) NULL,
                last_snapshot_time_utc DATETIME(6) NULL,
                last_price DOUBLE NULL,
                tick_size DOUBLE NULL,
                point_value DOUBLE NULL,
                is_realtime TINYINT NULL,
                connection_status VARCHAR(64) NULL,
                raw_payload_json LONGTEXT NULL,
                received_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
                updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
                UNIQUE KEY uq_nt_feed_heartbeat_external_key (external_key),
                KEY idx_nt_feed_heartbeat_root_received (root_symbol, timeframe, received_at),
                KEY idx_nt_feed_heartbeat_instrument_received (instrument, timeframe, received_at),
                KEY idx_nt_feed_heartbeat_received (received_at)
            )
            """
        )
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ninjatrader_feed_health_events (
                id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                run_id VARCHAR(128) NOT NULL,
                event_uid VARCHAR(255) NOT NULL,
                event_type VARCHAR(64) NOT NULL,
                instrument VARCHAR(128) NULL,
                root_symbol VARCHAR(32) NULL,
                timeframe VARCHAR(16) NULL,
                status VARCHAR(64) NULL,
                detected_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
                restored_at DATETIME(6) NULL,
                duration_seconds DOUBLE NULL,
                heartbeat_received_at DATETIME(6) NULL,
                heartbeat_age_seconds DOUBLE NULL,
                latest_candle_ts_utc DATETIME NULL,
                latest_candle_close_ts_utc DATETIME NULL,
                skipped_candles INT NULL,
                estimated_missing_bars INT NULL,
                details_json LONGTEXT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                UNIQUE KEY ux_nt_feed_health_event_uid (event_uid),
                KEY idx_nt_feed_health_run_created (run_id, created_at),
                KEY idx_nt_feed_health_type_status (event_type, status),
                KEY idx_nt_feed_health_symbol_time (root_symbol, timeframe, latest_candle_ts_utc)
            )
            """
        )
    conn.commit()


def ensure_candle_slot_audit_table(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ninjatrader_candle_slot_audit (
                id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                external_key VARCHAR(255) NOT NULL,
                source VARCHAR(64) NOT NULL DEFAULT 'ninjatrader',
                bridge_version VARCHAR(32) NULL,
                client_id VARCHAR(128) NULL,
                instrument VARCHAR(64) NULL,
                root_symbol VARCHAR(32) NULL,
                exchange_name VARCHAR(32) NULL,
                timeframe VARCHAR(32) NULL,
                bars_period_type VARCHAR(32) NULL,
                bars_period_value BIGINT NULL,
                slot_time DATETIME(6) NOT NULL,
                expected_close_time DATETIME(6) NULL,
                nt_connected TINYINT NULL,
                candle_received TINYINT NOT NULL DEFAULT 0,
                candle_external_key VARCHAR(255) NULL,
                candle_received_at DATETIME(6) NULL,
                arrival_status VARCHAR(32) NULL,
                scanner_run_id VARCHAR(128) NULL,
                scanner_status VARCHAR(32) NULL,
                trade_allowed TINYINT NULL,
                blocked_reason VARCHAR(128) NULL,
                details_json LONGTEXT NULL,
                created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
                updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
                UNIQUE KEY uq_nt_candle_slot_audit_external_key (external_key),
                KEY idx_nt_candle_slot_audit_root_slot (root_symbol, timeframe, slot_time),
                KEY idx_nt_candle_slot_audit_instrument_slot (instrument, timeframe, slot_time),
                KEY idx_nt_candle_slot_audit_arrival (arrival_status, slot_time),
                KEY idx_nt_candle_slot_audit_scanner (scanner_run_id, scanner_status, slot_time),
                KEY idx_nt_candle_slot_audit_updated (updated_at)
            )
            """
        )
    conn.commit()


def ensure_live_candle_scanner_lock_columns(conn) -> None:
    columns = {
        "scanner_locked_at": "DATETIME(6) NULL",
        "scanner_lock_run_id": "VARCHAR(128) NULL",
    }
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT COLUMN_NAME
            FROM information_schema.columns
            WHERE table_schema = DATABASE()
              AND table_name = 'ninjatrader_live_candles'
            """
        )
        existing = {str(row["COLUMN_NAME"]) for row in cur.fetchall()}
        for name, definition in columns.items():
            if name not in existing:
                cur.execute(f"ALTER TABLE ninjatrader_live_candles ADD COLUMN {name} {definition}")
        if "scanner_locked_at" not in existing:
            cur.execute(
                "CREATE INDEX idx_nt_live_candle_scanner_lock ON ninjatrader_live_candles (scanner_locked_at)"
            )
    conn.commit()


def ensure_order_signal_accounting_columns(conn) -> None:
    """Add monitor-owned accounting columns without requiring a server rebuild."""

    columns = {
        "accounting_instrument": "VARCHAR(64) NULL",
        "accounting_root_symbol": "VARCHAR(32) NULL",
        "accounting_tick_value": "DOUBLE NULL",
        "accounting_size_ratio": "DOUBLE NULL",
        "accounting_risk_dollars": "DOUBLE NULL",
        "execution_tick_value": "DOUBLE NULL",
        "execution_risk_dollars": "DOUBLE NULL",
        "exit_order_id": "VARCHAR(128) NULL",
        "exit_price": "DOUBLE NULL",
        "exit_action": "VARCHAR(32) NULL",
        "exit_order_name": "VARCHAR(255) NULL",
        "exit_received_at": "DATETIME(6) NULL",
        "realized_ticks": "DOUBLE NULL",
        "realized_execution_dollars": "DOUBLE NULL",
        "realized_accounting_dollars": "DOUBLE NULL",
    }
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT COLUMN_NAME
            FROM information_schema.columns
            WHERE table_schema = DATABASE()
              AND table_name = 'ninjatrader_order_signals'
            """
        )
        existing = {str(row["COLUMN_NAME"]) for row in cur.fetchall()}
        for name, definition in columns.items():
            if name not in existing:
                cur.execute(f"ALTER TABLE ninjatrader_order_signals ADD COLUMN {name} {definition}")
    conn.commit()


def root_from_instrument(instrument: str, fallback: str = "") -> str:
    match = re.match(r"^([A-Z]+)", str(instrument or "").strip().upper())
    return match.group(1) if match else str(fallback or "").strip().upper()


def accounting_config(args: argparse.Namespace, execution_instrument: str, execution_root: str) -> dict[str, Any]:
    accounting_instrument = str(args.accounting_instrument or "").strip().upper() or str(execution_instrument).upper()
    accounting_root = str(args.accounting_root or "").strip().upper() or root_from_instrument(
        accounting_instrument,
        execution_root,
    )
    execution_tick_value = float(args.execution_tick_value or 0.0)
    accounting_tick_value = float(args.accounting_tick_value or 0.0)
    if accounting_tick_value <= 0:
        accounting_tick_value = execution_tick_value
    size_ratio = float(args.accounting_size_ratio or 0.0)
    if size_ratio <= 0 and accounting_tick_value > 0 and execution_tick_value > 0:
        size_ratio = accounting_tick_value / execution_tick_value
    if size_ratio <= 0:
        size_ratio = 1.0
    mode = "execution"
    if accounting_instrument != str(execution_instrument).upper() or (
        accounting_tick_value > 0 and execution_tick_value > 0 and abs(accounting_tick_value - execution_tick_value) > 1e-9
    ):
        mode = "synthetic_accounting"
    return {
        "accounting_instrument": accounting_instrument,
        "accounting_root": accounting_root,
        "accounting_tick_value": accounting_tick_value,
        "accounting_size_ratio": size_ratio,
        "execution_tick_value": execution_tick_value,
        "accounting_mode": mode,
    }


def short_signal_uid(run_id: str, candidate_uid: str, direction: str) -> str:
    digest = sha1(f"{run_id}|{candidate_uid}|{direction}".encode("utf-8")).hexdigest()[:14]
    return f"td{digest}"


def execution_instrument_for(args: argparse.Namespace, instrument: str) -> tuple[str, str]:
    source_root = str(args.root or "").upper()
    if str(args.signal_instrument or "").strip():
        signal_instrument = str(args.signal_instrument).strip().upper()
    elif str(args.execution_root or "").strip():
        execution_root = str(args.execution_root).strip().upper()
        source_instrument = str(instrument).upper()
        signal_instrument = (
            execution_root + source_instrument[len(source_root) :]
            if source_instrument.startswith(source_root)
            else f"{execution_root} {source_instrument}"
        )
    else:
        signal_instrument = str(instrument).upper()

    match = re.match(r"^([A-Z]+)", signal_instrument)
    execution_root = match.group(1) if match else source_root
    return signal_instrument, execution_root


def active_signal_count(conn, args: argparse.Namespace, instrument: str) -> int:
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT COUNT(*) AS count_rows
            FROM information_schema.tables
            WHERE table_schema = DATABASE()
              AND table_name = 'ninjatrader_order_signals'
            """
        )
        exists = cur.fetchone()
        if not exists or int(exists.get("count_rows") or 0) <= 0:
            return 0

        cur.execute(
            """
            SELECT COUNT(*) AS count_rows
            FROM ninjatrader_order_signals
            WHERE status IN ('queued', 'claimed', 'triggered')
              AND account_name = %s
              AND instrument = %s
            """,
            (str(args.signal_account), str(instrument).upper()),
        )
        row = cur.fetchone()
    return int(row.get("count_rows") or 0) if row else 0


def reconcile_completed_signals(
    conn,
    args: argparse.Namespace,
    instrument: str,
    *,
    email_config: dict[str, Any] | None = None,
    email_enabled: bool = False,
) -> int:
    """Clear one-lot active signals when NT has recorded the matching exit fill."""
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT id, signal_uid, side, quantity, expected_price, tick_size,
                   actual_trigger_price, accounting_tick_value, execution_tick_value,
                   triggered_at, created_at
            FROM ninjatrader_order_signals
            WHERE status IN ('triggered')
              AND account_name = %s
              AND instrument = %s
            ORDER BY triggered_at ASC, created_at ASC
            LIMIT 10
            """,
            (str(args.signal_account), str(instrument).upper()),
        )
        signals = cur.fetchall()

    completed = 0
    completed_signal_uids: list[str] = []
    for signal in signals:
        side = str(signal.get("side") or "").upper()
        exit_action = "Sell" if side.startswith("L") else "BuyToCover"
        trigger_time = signal.get("triggered_at") or signal.get("created_at")
        with conn.cursor() as cur:
            cur.execute(
                """
                SELECT order_id, price, execution_time, received_at, order_action, order_name
                FROM ninjatrader_execution_fills
                WHERE account_name = %s
                  AND instrument = %s
                  AND received_at >= COALESCE(%s, '1970-01-01')
                  AND order_action = %s
                  AND (
                    order_name IS NULL
                    OR order_name NOT LIKE 'ABCD|sig=%%'
                  )
                ORDER BY received_at ASC, id ASC
                LIMIT 1
                """,
                (str(args.signal_account), str(instrument).upper(), trigger_time, exit_action),
            )
            fill = cur.fetchone()
        if not fill:
            continue

        tick_size = float(signal.get("tick_size") or 0.0)
        entry_price = wave.finite(signal.get("actual_trigger_price"), None)
        if entry_price is None:
            entry_price = wave.finite(signal.get("expected_price"), None)
        exit_price = wave.finite(fill.get("price"), None)
        quantity = max(1, int(signal.get("quantity") or 1))
        realized_ticks = None
        realized_execution_dollars = None
        realized_accounting_dollars = None
        if entry_price is not None and exit_price is not None and tick_size > 0:
            pnl_points = float(exit_price) - float(entry_price) if side.startswith("L") else float(entry_price) - float(exit_price)
            realized_ticks = pnl_points / tick_size
            execution_tick_value = float(signal.get("execution_tick_value") or args.execution_tick_value or 0.0)
            accounting_tick_value = float(signal.get("accounting_tick_value") or args.accounting_tick_value or execution_tick_value or 0.0)
            if execution_tick_value > 0:
                realized_execution_dollars = realized_ticks * execution_tick_value * quantity
            if accounting_tick_value > 0:
                realized_accounting_dollars = realized_ticks * accounting_tick_value * quantity

        with conn.cursor() as cur:
            cur.execute(
                """
                UPDATE ninjatrader_order_signals
                SET status = 'completed',
                    order_id = COALESCE(%s, order_id),
                    exit_order_id = %s,
                    exit_price = %s,
                    exit_action = %s,
                    exit_order_name = %s,
                    exit_received_at = %s,
                    realized_ticks = %s,
                    realized_execution_dollars = %s,
                    realized_accounting_dollars = %s,
                    status_message = %s,
                    updated_at = CURRENT_TIMESTAMP(6)
                WHERE signal_uid = %s
                  AND status IN ('triggered')
                """,
                (
                    fill.get("order_id"),
                    fill.get("order_id"),
                    fill.get("price"),
                    fill.get("order_action"),
                    fill.get("order_name"),
                    fill.get("received_at"),
                    realized_ticks,
                    realized_execution_dollars,
                    realized_accounting_dollars,
                    f"completion reconciled from execution fill {fill.get('order_name') or fill.get('order_action')}",
                    signal.get("signal_uid"),
                ),
            )
            changed = int(cur.rowcount or 0)
            completed += changed
            if changed:
                completed_signal_uids.append(str(signal.get("signal_uid")))
    if completed:
        conn.commit()
        print(
            json.dumps(
                {
                    "event": "signal_completion_reconciled",
                    "instrument": instrument,
                    "completed": completed,
                },
                default=to_jsonable,
            ),
            flush=True,
        )
        if email_enabled:
            for signal_uid in completed_signal_uids:
                trade_email.notify_completed_signal(conn, signal_uid, config=email_config)
    return completed


def repair_completed_signal_details(conn, args: argparse.Namespace, instrument: str) -> int:
    """Fill entry/exit/PnL columns for signals NT already marked completed."""

    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT id, signal_uid, side, quantity, expected_price, tick_size,
                   accounting_tick_value, execution_tick_value, triggered_at, created_at
            FROM ninjatrader_order_signals
            WHERE status = 'completed'
              AND account_name = %s
              AND instrument = %s
              AND (exit_price IS NULL OR realized_ticks IS NULL)
            ORDER BY updated_at ASC, id ASC
            LIMIT 20
            """,
            (str(args.signal_account), str(instrument).upper()),
        )
        signals = cur.fetchall()

    repaired = 0
    for signal in signals:
        signal_uid = str(signal.get("signal_uid") or "")
        side = str(signal.get("side") or "").upper()
        entry_action = "Buy" if side.startswith("L") else "SellShort"
        exit_action = "Sell" if side.startswith("L") else "BuyToCover"
        with conn.cursor() as cur:
            cur.execute(
                """
                SELECT order_id, price, received_at, order_action, order_name
                FROM ninjatrader_execution_fills
                WHERE account_name = %s
                  AND instrument = %s
                  AND order_name LIKE %s
                  AND order_action = %s
                ORDER BY received_at ASC, id ASC
                LIMIT 1
                """,
                (
                    str(args.signal_account),
                    str(instrument).upper(),
                    f"%sig={signal_uid}%",
                    entry_action,
                ),
            )
            entry_fill = cur.fetchone()
        if not entry_fill:
            continue

        with conn.cursor() as cur:
            cur.execute(
                """
                SELECT order_id, price, received_at, order_action, order_name
                FROM ninjatrader_execution_fills
                WHERE account_name = %s
                  AND instrument = %s
                  AND received_at >= %s
                  AND order_action = %s
                  AND (
                    order_name IS NULL
                    OR order_name NOT LIKE 'ABCD|sig=%%'
                  )
                ORDER BY received_at ASC, id ASC
                LIMIT 1
                """,
                (
                    str(args.signal_account),
                    str(instrument).upper(),
                    entry_fill.get("received_at"),
                    exit_action,
                ),
            )
            exit_fill = cur.fetchone()
        if not exit_fill:
            continue

        tick_size = float(signal.get("tick_size") or 0.0)
        entry_price = wave.finite(entry_fill.get("price"), None)
        exit_price = wave.finite(exit_fill.get("price"), None)
        quantity = max(1, int(signal.get("quantity") or 1))
        realized_ticks = None
        realized_execution_dollars = None
        realized_accounting_dollars = None
        if entry_price is not None and exit_price is not None and tick_size > 0:
            pnl_points = float(exit_price) - float(entry_price) if side.startswith("L") else float(entry_price) - float(exit_price)
            realized_ticks = pnl_points / tick_size
            execution_tick_value = float(signal.get("execution_tick_value") or args.execution_tick_value or 0.0)
            accounting_tick_value = float(signal.get("accounting_tick_value") or args.accounting_tick_value or execution_tick_value or 0.0)
            if execution_tick_value > 0:
                realized_execution_dollars = realized_ticks * execution_tick_value * quantity
            if accounting_tick_value > 0:
                realized_accounting_dollars = realized_ticks * accounting_tick_value * quantity

        with conn.cursor() as cur:
            cur.execute(
                """
                UPDATE ninjatrader_order_signals
                SET order_id = COALESCE(%s, order_id),
                    actual_trigger_price = %s,
                    exit_order_id = %s,
                    exit_price = %s,
                    exit_action = %s,
                    exit_order_name = %s,
                    exit_received_at = %s,
                    realized_ticks = %s,
                    realized_execution_dollars = %s,
                    realized_accounting_dollars = %s,
                    status_message = CONCAT(COALESCE(status_message, ''), ' | details repaired from execution fills'),
                    updated_at = CURRENT_TIMESTAMP(6)
                WHERE signal_uid = %s
                  AND status = 'completed'
                  AND (exit_price IS NULL OR realized_ticks IS NULL)
                """,
                (
                    entry_fill.get("order_id"),
                    entry_price,
                    exit_fill.get("order_id"),
                    exit_price,
                    exit_fill.get("order_action"),
                    exit_fill.get("order_name"),
                    exit_fill.get("received_at"),
                    realized_ticks,
                    realized_execution_dollars,
                    realized_accounting_dollars,
                    signal_uid,
                ),
            )
            repaired += int(cur.rowcount or 0)

    if repaired:
        conn.commit()
        print(
            json.dumps({"event": "completed_signal_details_repaired", "instrument": instrument, "repaired": repaired}),
            flush=True,
        )
    return repaired


def notify_completed_trades_once(
    conn,
    args: argparse.Namespace,
    instrument: str,
    *,
    email_config: dict[str, Any] | None,
    email_enabled: bool,
    updated_after: Any,
) -> int:
    if not email_enabled:
        return 0
    email_args = argparse.Namespace(
        signal_uid="",
        account=str(args.signal_account),
        instrument=str(instrument).upper(),
        limit=20,
        updated_after=updated_after,
        dry_run=False,
        mark_dry_run=False,
    )
    return trade_email.process_once(conn, email_args, email_config or trade_email.email_config(), dry_run=False)


def post_order_signal(server_url: str, payload: dict[str, Any]) -> dict[str, Any]:
    body = json.dumps(payload, default=to_jsonable).encode("utf-8")
    endpoint = server_url.rstrip("/") + "/ninjatrader/signals"
    http_request = request.Request(
        endpoint,
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with request.urlopen(http_request, timeout=3.0) as response:
        return json.loads(response.read().decode("utf-8"))


def update_signal_accounting_columns(conn, signal_uid: str, details: dict[str, Any]) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            UPDATE ninjatrader_order_signals
            SET accounting_instrument = %s,
                accounting_root_symbol = %s,
                accounting_tick_value = %s,
                accounting_size_ratio = %s,
                accounting_risk_dollars = %s,
                execution_tick_value = %s,
                execution_risk_dollars = %s
            WHERE signal_uid = %s
            """,
            (
                details.get("accounting_instrument"),
                details.get("accounting_root"),
                details.get("accounting_tick_value"),
                details.get("accounting_size_ratio"),
                details.get("accounting_risk_dollars"),
                details.get("execution_tick_value"),
                details.get("execution_risk_dollars"),
                signal_uid,
            ),
        )
    conn.commit()


def build_live_order_signal(
    *,
    args: argparse.Namespace,
    model_args: argparse.Namespace,
    bundle: dict[str, Any],
    instrument: str,
    execution_instrument: str,
    execution_root: str,
    root: str,
    symbol: str,
    direction: str,
    current_idx: int,
    candle_time: pd.Timestamp,
    ts_utc: pd.Timestamp,
    candles: pd.DataFrame,
    enriched: pd.DataFrame,
    row: dict[str, Any],
    stage2_score: float,
) -> tuple[dict[str, Any] | None, dict[str, Any]]:
    tick_size = wave.tick_size_for(symbol, root)
    entry_price = wave.finite(enriched["close"].iloc[current_idx], None)
    if entry_price is None or tick_size <= 0:
        return None, {"reject_reason": "missing_entry_or_tick"}

    stop_frame = enriched.copy()
    stop_frame.loc[current_idx, "open"] = float(entry_price)
    stop_price, risk_points = wave.initial_stop(stop_frame, current_idx, direction, tick_size, model_args)
    risk_ticks = (risk_points / tick_size) if risk_points is not None and tick_size > 0 else None
    risk_ok = (
        stop_price is not None
        and risk_points is not None
        and risk_ticks is not None
        and float(bundle["rules"]["risk_ticks_min"]) <= float(risk_ticks) <= float(bundle["rules"]["risk_ticks_max"])
    )
    quantity = max(1, int(args.signal_quantity))
    acct = accounting_config(args, execution_instrument, execution_root)
    execution_risk_dollars = None
    accounting_risk_dollars = None
    risk_dollars = None
    if risk_ticks is not None and float(args.execution_tick_value or 0.0) > 0:
        execution_risk_dollars = float(risk_ticks) * float(args.execution_tick_value) * quantity
    if risk_ticks is not None and float(acct["accounting_tick_value"] or 0.0) > 0:
        accounting_risk_dollars = float(risk_ticks) * float(acct["accounting_tick_value"]) * quantity
    risk_dollars = accounting_risk_dollars if accounting_risk_dollars is not None else execution_risk_dollars
    if not risk_ok:
        return None, {
            "reject_reason": "rejected_risk",
            "entry_price": entry_price,
            "stop_price": stop_price,
            "risk_ticks": risk_ticks,
            "risk_dollars": risk_dollars,
            "execution_risk_dollars": execution_risk_dollars,
            "accounting_risk_dollars": accounting_risk_dollars,
            "min_risk_ticks": bundle["rules"]["risk_ticks_min"],
            "max_risk_ticks": bundle["rules"]["risk_ticks_max"],
            "signal_candle": candle_audit_snapshot(candles, row.get("signal_idx")),
            "decision_candle": candle_audit_snapshot(candles, current_idx),
            **acct,
        }
    if (
        float(args.sim_account_cash or 0.0) > 0
        and risk_dollars is not None
        and float(risk_dollars) > float(args.sim_account_cash)
    ):
        return None, {
            "reject_reason": "insufficient_sim_cash_for_stop_risk",
            "entry_price": entry_price,
            "stop_price": stop_price,
            "risk_ticks": risk_ticks,
            "risk_dollars": risk_dollars,
            "execution_risk_dollars": execution_risk_dollars,
            "accounting_risk_dollars": accounting_risk_dollars,
            "sim_account_cash": float(args.sim_account_cash),
            "execution_tick_value": float(args.execution_tick_value),
            "quantity": quantity,
            "signal_candle": candle_audit_snapshot(candles, row.get("signal_idx")),
            "decision_candle": candle_audit_snapshot(candles, current_idx),
            **acct,
        }
    if (
        float(args.max_accounting_risk_dollars or 0.0) > 0
        and accounting_risk_dollars is not None
        and float(accounting_risk_dollars) > float(args.max_accounting_risk_dollars)
    ):
        return None, {
            "reject_reason": "max_accounting_risk_exceeded",
            "entry_price": entry_price,
            "stop_price": stop_price,
            "risk_ticks": risk_ticks,
            "risk_dollars": risk_dollars,
            "execution_risk_dollars": execution_risk_dollars,
            "accounting_risk_dollars": accounting_risk_dollars,
            "max_accounting_risk_dollars": float(args.max_accounting_risk_dollars),
            "quantity": quantity,
            "signal_candle": candle_audit_snapshot(candles, row.get("signal_idx")),
            "decision_candle": candle_audit_snapshot(candles, current_idx),
            **acct,
        }

    signal_uid = short_signal_uid(args.run_id, str(row["candidate_uid"]), direction)
    expected_time = pd.Timestamp(candle_time).strftime("%Y-%m-%d %H:%M:%S.%f")
    notes = {
        "trigger_mode": "market_now",
        "source": "live_ninjatrader_trend_monitor",
        "detect_instrument": instrument,
        "detect_root": root,
        "detect_symbol": symbol,
        "execution_instrument": execution_instrument,
        "execution_root": execution_root,
        "accounting_mode": acct["accounting_mode"],
        "accounting_instrument": acct["accounting_instrument"],
        "accounting_root": acct["accounting_root"],
        "timeframe": args.timeframe,
        "candidate_uid": row.get("candidate_uid"),
        "confirm_date": row.get("confirm_date") or ts_utc,
        "stage2_score": stage2_score,
        "risk_ticks": risk_ticks,
        "risk_dollars": risk_dollars,
        "execution_risk_dollars": execution_risk_dollars,
        "accounting_risk_dollars": accounting_risk_dollars,
        "sim_account_cash": float(args.sim_account_cash or 0.0) or None,
        "execution_tick_value": acct["execution_tick_value"] or None,
        "accounting_tick_value": acct["accounting_tick_value"] or None,
        "accounting_size_ratio": acct["accounting_size_ratio"],
        "signal_candle": candle_audit_snapshot(candles, row.get("signal_idx")),
        "decision_candle": candle_audit_snapshot(candles, current_idx),
        "read": "Live mimic signal. Market-now means NT submits on the next queue poll/tick, not a historical fill.",
    }

    return {
        "signal_uid": signal_uid,
        "account_name": str(args.signal_account),
        "instrument": execution_instrument,
        "root_symbol": execution_root,
        "side": direction,
        "quantity": quantity,
        "expected_price": float(entry_price),
        "expected_time": expected_time,
        "stop_price": float(stop_price),
        "target_price": None,
        "tick_size": float(tick_size),
        "expected_ai_run_id": str(args.run_id)[:64],
        "expected_setup_id": str(row.get("candidate_uid") or "")[:64],
        "expected_template_uid": "live_trend_market_now",
        "notes": json.dumps(notes, default=to_jsonable, sort_keys=True),
    }, {
        "entry_price": float(entry_price),
        "stop_price": float(stop_price),
        "risk_ticks": float(risk_ticks),
        "risk_dollars": None if risk_dollars is None else float(risk_dollars),
        "execution_risk_dollars": None if execution_risk_dollars is None else float(execution_risk_dollars),
        "accounting_risk_dollars": None if accounting_risk_dollars is None else float(accounting_risk_dollars),
        "signal_uid": signal_uid,
        "execution_instrument": execution_instrument,
        "execution_root": execution_root,
        "signal_candle": candle_audit_snapshot(candles, row.get("signal_idx")),
        "decision_candle": candle_audit_snapshot(candles, current_idx),
        **acct,
    }


def insert_event(
    conn,
    *,
    run_id: str,
    event_uid: str,
    event_type: str,
    instrument: str | None,
    root_symbol: str | None,
    model_symbol: str | None,
    timeframe: str | None,
    candle_time: Any,
    ts_utc: Any,
    direction: str | None,
    level2_score: float | None = None,
    stage2_score: float | None = None,
    entry_price: float | None = None,
    stop_price: float | None = None,
    risk_ticks: float | None = None,
    status: str | None = None,
    details: dict[str, Any] | None = None,
) -> bool:
    payload = json.dumps(details or {}, default=to_jsonable, sort_keys=True)
    with conn.cursor() as cur:
        cur.execute(
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
            (
                run_id,
                event_uid,
                event_type,
                instrument,
                root_symbol,
                model_symbol,
                timeframe,
                pd.Timestamp(candle_time).to_pydatetime() if candle_time is not None and not pd.isna(candle_time) else None,
                pd.Timestamp(ts_utc).to_pydatetime() if ts_utc is not None and not pd.isna(ts_utc) else None,
                direction,
                level2_score,
                stage2_score,
                entry_price,
                stop_price,
                risk_ticks,
                status,
                payload,
            ),
        )
        changed = cur.rowcount == 1
    conn.commit()
    return changed


def insert_route_row(
    conn,
    *,
    args: argparse.Namespace,
    route_uid: str,
    instrument: str,
    model_symbol: str,
    candle_snapshot: dict[str, Any] | None,
    cycle: dict[str, Any],
    pending_after: int,
    scheduled_entries_after: int,
    details: dict[str, Any] | None = None,
) -> None:
    payload = json.dumps(details or {}, default=to_jsonable, sort_keys=True)
    candle_snapshot = candle_snapshot or {}
    with conn.cursor() as cur:
        cur.execute(
            """
            INSERT INTO ninjatrader_live_screener_routes (
                route_uid, run_id, instrument, root_symbol, model_symbol, timeframe,
                candle_time, ts_utc, candle_id, candle_revision_id, candle_payload_hash,
                open, high, low, close, volume, stage1_rows, stage1_status,
                level2_threshold, level2_long_score, level2_short_score,
                level2_best_direction, level2_best_score, level2_picks,
                stage2_rows, stage2_status, stage2_threshold, stage2_best_direction,
                stage2_best_score, stage2_confirmed_direction, stage2_confirmed_score,
                stage2_confirm_offset_bars, pending_after, scheduled_entries_after,
                order_signals, order_rejects, decision_status, details_json
            )
            VALUES (
                %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s,
                %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s,
                %s, %s
            )
            ON DUPLICATE KEY UPDATE
                instrument = VALUES(instrument),
                root_symbol = VALUES(root_symbol),
                model_symbol = VALUES(model_symbol),
                timeframe = VALUES(timeframe),
                candle_time = VALUES(candle_time),
                ts_utc = VALUES(ts_utc),
                candle_id = VALUES(candle_id),
                candle_revision_id = VALUES(candle_revision_id),
                candle_payload_hash = VALUES(candle_payload_hash),
                open = VALUES(open),
                high = VALUES(high),
                low = VALUES(low),
                close = VALUES(close),
                volume = VALUES(volume),
                stage1_rows = VALUES(stage1_rows),
                stage1_status = VALUES(stage1_status),
                level2_threshold = VALUES(level2_threshold),
                level2_long_score = VALUES(level2_long_score),
                level2_short_score = VALUES(level2_short_score),
                level2_best_direction = VALUES(level2_best_direction),
                level2_best_score = VALUES(level2_best_score),
                level2_picks = VALUES(level2_picks),
                stage2_rows = VALUES(stage2_rows),
                stage2_status = VALUES(stage2_status),
                stage2_threshold = VALUES(stage2_threshold),
                stage2_best_direction = VALUES(stage2_best_direction),
                stage2_best_score = VALUES(stage2_best_score),
                stage2_confirmed_direction = VALUES(stage2_confirmed_direction),
                stage2_confirmed_score = VALUES(stage2_confirmed_score),
                stage2_confirm_offset_bars = VALUES(stage2_confirm_offset_bars),
                pending_after = VALUES(pending_after),
                scheduled_entries_after = VALUES(scheduled_entries_after),
                order_signals = VALUES(order_signals),
                order_rejects = VALUES(order_rejects),
                decision_status = VALUES(decision_status),
                details_json = VALUES(details_json),
                updated_at = CURRENT_TIMESTAMP
            """,
            (
                route_uid,
                args.run_id,
                instrument,
                str(args.root).upper(),
                model_symbol,
                args.timeframe,
                pd.Timestamp(cycle.get("candle_time")).to_pydatetime()
                if cycle.get("candle_time") is not None and not pd.isna(cycle.get("candle_time"))
                else None,
                pd.Timestamp(cycle.get("ts_utc")).to_pydatetime()
                if cycle.get("ts_utc") is not None and not pd.isna(cycle.get("ts_utc"))
                else None,
                candle_snapshot.get("candle_id"),
                candle_snapshot.get("revision_id"),
                candle_snapshot.get("payload_hash"),
                wave.finite(candle_snapshot.get("open"), None),
                wave.finite(candle_snapshot.get("high"), None),
                wave.finite(candle_snapshot.get("low"), None),
                wave.finite(candle_snapshot.get("close"), None),
                wave.finite(candle_snapshot.get("volume"), None),
                int(cycle.get("stage1_rows") or 0),
                cycle.get("stage1_status"),
                wave.finite(cycle.get("level2_threshold"), None),
                wave.finite(cycle.get("level2_long_score"), None),
                wave.finite(cycle.get("level2_short_score"), None),
                cycle.get("level2_best_direction"),
                wave.finite(cycle.get("level2_best_score"), None),
                int(cycle.get("level2_picks") or 0),
                int(cycle.get("stage2_rows") or 0),
                cycle.get("stage2_status"),
                wave.finite(cycle.get("stage2_threshold"), None),
                cycle.get("stage2_best_direction"),
                wave.finite(cycle.get("stage2_best_score"), None),
                cycle.get("stage2_confirmed_direction"),
                wave.finite(cycle.get("stage2_confirmed_score"), None),
                cycle.get("stage2_confirm_offset_bars"),
                int(pending_after),
                int(scheduled_entries_after),
                int(cycle.get("order_signals") or 0),
                int(cycle.get("order_rejects") or 0),
                cycle.get("decision_status"),
                payload,
            ),
        )
    conn.commit()


def latest_instrument(conn, root: str, timeframe: str) -> str | None:
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT instrument
            FROM ninjatrader_live_candles
            WHERE root_symbol = %s
              AND timeframe = %s
            ORDER BY received_at DESC, candle_time DESC, id DESC
            LIMIT 1
            """,
            (root, timeframe),
        )
        row = cur.fetchone()
    return str(row["instrument"]) if row and row.get("instrument") else None


def fetch_live_candles(conn, args: argparse.Namespace, instrument: str) -> pd.DataFrame:
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT id, instrument, root_symbol, timeframe, candle_time,
                   CAST(open AS DOUBLE) AS open,
                   CAST(high AS DOUBLE) AS high,
                   CAST(low AS DOUBLE) AS low,
                   CAST(close AS DOUBLE) AS close,
                   CAST(volume AS DOUBLE) AS volume,
                   is_realtime, received_at,
                   latest_revision_id, latest_payload_hash,
                   scanner_locked_at, scanner_lock_run_id
            FROM ninjatrader_live_candles
            WHERE root_symbol = %s
              AND timeframe = %s
              AND instrument = %s
            ORDER BY candle_time DESC, id DESC
            LIMIT %s
            """,
            (args.root, args.timeframe, instrument, int(args.lookback_rows)),
        )
        rows = cur.fetchall()
    frame = pd.DataFrame(rows)
    if frame.empty:
        return frame
    frame = frame.sort_values(["candle_time", "id"]).drop_duplicates("candle_time", keep="last").reset_index(drop=True)
    timeframe_minutes = int(scanner.table_for_timeframe(args.timeframe)[1])
    frame["ts_utc"] = (
        local_naive_to_utc_naive(frame["candle_time"], args.local_timezone)
        - pd.to_timedelta(timeframe_minutes, unit="m")
    )
    frame["symbol"] = nt_instrument_to_model_symbol(instrument, args.root)
    for col in ["open", "high", "low", "close", "volume"]:
        frame[col] = pd.to_numeric(frame[col], errors="coerce")
    return frame.dropna(subset=["ts_utc", "open", "high", "low", "close"]).reset_index(drop=True)


def candle_audit_snapshot(candles: pd.DataFrame, idx: int | None) -> dict[str, Any] | None:
    if idx is None:
        return None
    idx_value = wave.finite(idx, None)
    if idx_value is None:
        return None
    idx_int = int(idx_value)
    if idx_int < 0 or idx_int >= len(candles):
        return None
    row = candles.iloc[idx_int]
    return {
        "candle_id": to_jsonable(row.get("id")),
        "revision_id": to_jsonable(row.get("latest_revision_id")),
        "payload_hash": to_jsonable(row.get("latest_payload_hash")),
        "candle_time": to_jsonable(row.get("candle_time")),
        "ts_utc": to_jsonable(row.get("ts_utc")),
        "open": wave.finite(row.get("open"), None),
        "high": wave.finite(row.get("high"), None),
        "low": wave.finite(row.get("low"), None),
        "close": wave.finite(row.get("close"), None),
        "volume": wave.finite(row.get("volume"), None),
        "is_realtime": to_jsonable(row.get("is_realtime")),
        "received_at": to_jsonable(row.get("received_at")),
        "scanner_locked_at": to_jsonable(row.get("scanner_locked_at")),
        "scanner_lock_run_id": to_jsonable(row.get("scanner_lock_run_id")),
    }


def fetch_latest_heartbeat(conn, args: argparse.Namespace, instrument: str) -> dict[str, Any] | None:
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT id, instrument, root_symbol, timeframe, heartbeat_time_utc,
                   last_candle_time, last_candle_time_utc, last_snapshot_time_utc,
                   last_price, connection_status, received_at
            FROM ninjatrader_feed_heartbeats
            WHERE root_symbol = %s
              AND timeframe = %s
              AND instrument = %s
            ORDER BY received_at DESC, id DESC
            LIMIT 1
            """,
            (args.root, args.timeframe, instrument),
        )
        row = cur.fetchone()
    if not row:
        return None
    payload = dict(row)
    heartbeat_ts = payload.get("heartbeat_time_utc") or payload.get("received_at")
    payload["age_seconds"] = utc_age_seconds(heartbeat_ts)
    return payload


def utc_now_naive() -> pd.Timestamp:
    return pd.Timestamp.now(tz="UTC").tz_localize(None)


def utc_age_seconds(value: Any, now_utc: pd.Timestamp | None = None) -> float | None:
    if value is None or pd.isna(value):
        return None
    ts = pd.Timestamp(value)
    if pd.isna(ts):
        return None
    if ts.tzinfo is not None:
        ts = ts.tz_convert("UTC").tz_localize(None)
    now = now_utc if now_utc is not None else utc_now_naive()
    return max(0.0, float((now - ts).total_seconds()))


def timeframe_delta(timeframe: str) -> pd.Timedelta:
    return pd.to_timedelta(int(scanner.table_for_timeframe(timeframe)[1]), unit="m")


def as_utc_naive(value: Any) -> pd.Timestamp | None:
    if value is None or pd.isna(value):
        return None
    ts = pd.Timestamp(value)
    if pd.isna(ts):
        return None
    if ts.tzinfo is not None:
        ts = ts.tz_convert("UTC").tz_localize(None)
    return ts


def slot_external_key(args: argparse.Namespace, instrument: str, slot_time: Any) -> str:
    slot_ts = as_utc_naive(slot_time)
    slot_text = "unknown" if slot_ts is None else slot_ts.isoformat()
    raw = f"ninjatrader|{instrument}|{args.timeframe}|{slot_text}"
    if len(raw) <= 240:
        return raw
    digest = sha1(raw.encode("utf-8")).hexdigest()[:16]
    return f"ninjatrader|{str(instrument)[:120]}|{args.timeframe}|{digest}"


def upsert_candle_slot_audit(
    conn,
    *,
    args: argparse.Namespace,
    instrument: str,
    slot_time: Any,
    expected_close_time: Any = None,
    nt_connected: bool | None = None,
    candle_received: bool | None = None,
    candle_external_key: str | None = None,
    candle_received_at: Any = None,
    arrival_status: str | None = None,
    scanner_status: str | None = None,
    trade_allowed: bool | None = None,
    blocked_reason: str | None = None,
    details: dict[str, Any] | None = None,
) -> None:
    slot_ts = as_utc_naive(slot_time)
    if slot_ts is None:
        return
    expected_close_ts = as_utc_naive(expected_close_time) or slot_ts
    payload = json.dumps(details or {}, default=to_jsonable, sort_keys=True) if details else None
    with conn.cursor() as cur:
        cur.execute(
            """
            INSERT INTO ninjatrader_candle_slot_audit (
                external_key, instrument, root_symbol, timeframe, bars_period_value,
                slot_time, expected_close_time, nt_connected, candle_received,
                candle_external_key, candle_received_at, arrival_status,
                scanner_run_id, scanner_status, trade_allowed, blocked_reason, details_json
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            ON DUPLICATE KEY UPDATE
                instrument = VALUES(instrument),
                root_symbol = VALUES(root_symbol),
                timeframe = VALUES(timeframe),
                bars_period_value = VALUES(bars_period_value),
                expected_close_time = COALESCE(VALUES(expected_close_time), expected_close_time),
                nt_connected = COALESCE(VALUES(nt_connected), nt_connected),
                candle_received = GREATEST(candle_received, VALUES(candle_received)),
                candle_external_key = COALESCE(VALUES(candle_external_key), candle_external_key),
                candle_received_at = COALESCE(VALUES(candle_received_at), candle_received_at),
                arrival_status = COALESCE(VALUES(arrival_status), arrival_status),
                scanner_run_id = COALESCE(VALUES(scanner_run_id), scanner_run_id),
                scanner_status = COALESCE(VALUES(scanner_status), scanner_status),
                trade_allowed = COALESCE(VALUES(trade_allowed), trade_allowed),
                blocked_reason = COALESCE(VALUES(blocked_reason), blocked_reason),
                details_json = COALESCE(VALUES(details_json), details_json),
                updated_at = CURRENT_TIMESTAMP(6)
            """,
            (
                slot_external_key(args, instrument, slot_ts),
                instrument,
                str(args.root).upper(),
                args.timeframe,
                int(scanner.table_for_timeframe(args.timeframe)[1]),
                slot_ts.to_pydatetime(),
                expected_close_ts.to_pydatetime(),
                None if nt_connected is None else int(bool(nt_connected)),
                int(bool(candle_received)) if candle_received is not None else 0,
                candle_external_key,
                as_utc_naive(candle_received_at).to_pydatetime()
                if as_utc_naive(candle_received_at) is not None
                else None,
                arrival_status,
                args.run_id if scanner_status or trade_allowed is not None or blocked_reason else None,
                scanner_status,
                None if trade_allowed is None else int(bool(trade_allowed)),
                blocked_reason,
                payload,
            ),
        )
    conn.commit()


def record_missing_slot_range(
    conn,
    *,
    args: argparse.Namespace,
    instrument: str,
    start_close_ts: Any,
    end_close_ts: Any,
    expected_delta: pd.Timedelta,
    nt_connected: bool,
    blocked_reason: str,
    details: dict[str, Any] | None = None,
) -> int:
    start_ts = as_utc_naive(start_close_ts)
    end_ts = as_utc_naive(end_close_ts)
    if start_ts is None or end_ts is None or expected_delta <= pd.Timedelta(0):
        return 0
    if start_ts > end_ts:
        return 0
    count = 0
    slot_ts = start_ts
    max_slots = 720
    while slot_ts <= end_ts + pd.Timedelta(seconds=1) and count < max_slots:
        upsert_candle_slot_audit(
            conn,
            args=args,
            instrument=instrument,
            slot_time=slot_ts,
            expected_close_time=slot_ts,
            nt_connected=nt_connected,
            candle_received=False,
            arrival_status="missing",
            scanner_status="not_scanned",
            trade_allowed=False,
            blocked_reason=blocked_reason,
            details=details,
        )
        count += 1
        slot_ts += expected_delta
    return count


def latest_expected_close_slot(now_utc: Any, expected_delta: pd.Timedelta) -> pd.Timestamp | None:
    now_ts = as_utc_naive(now_utc)
    if now_ts is None or expected_delta <= pd.Timedelta(0):
        return None
    delta_seconds = int(expected_delta.total_seconds())
    if delta_seconds <= 0:
        return None
    epoch_seconds = int(now_ts.timestamp())
    floored = (epoch_seconds // delta_seconds) * delta_seconds
    return pd.Timestamp.utcfromtimestamp(floored).tz_localize(None)


def record_candle_arrival_slots(
    conn,
    *,
    args: argparse.Namespace,
    instrument: str,
    candles: pd.DataFrame,
    expected_delta: pd.Timedelta,
    only_last_rows: int = 80,
) -> None:
    if candles.empty:
        return
    recent = candles.tail(max(1, int(only_last_rows)))
    tolerance = max(expected_delta, pd.Timedelta(seconds=float(args.poll_seconds or 0) + 5.0))
    for _, row in recent.iterrows():
        ts_utc = as_utc_naive(row.get("ts_utc"))
        if ts_utc is None:
            continue
        close_ts = ts_utc + expected_delta
        received_ts = as_utc_naive(row.get("received_at"))
        arrival_status = "on_time"
        if received_ts is not None and received_ts > close_ts + tolerance:
            arrival_status = "late"
        external_key = f"ninjatrader|{instrument}|{args.timeframe}|{to_jsonable(row.get('candle_time'))}"
        upsert_candle_slot_audit(
            conn,
            args=args,
            instrument=instrument,
            slot_time=close_ts,
            expected_close_time=close_ts,
            nt_connected=True,
            candle_received=True,
            candle_external_key=external_key,
            candle_received_at=received_ts,
            arrival_status=arrival_status,
            details={
                "candle_id": to_jsonable(row.get("id")),
                "candle_time": to_jsonable(row.get("candle_time")),
                "ts_utc": to_jsonable(ts_utc),
                "received_at": to_jsonable(received_ts),
                "arrival_rule": "on_time if first received before the next expected slot window passed",
            },
        )


def mark_scanner_slot_status(
    conn,
    *,
    args: argparse.Namespace,
    instrument: str,
    ts_utc: Any,
    expected_delta: pd.Timedelta,
    scanner_status: str,
    trade_allowed: bool,
    blocked_reason: str | None = None,
    details: dict[str, Any] | None = None,
) -> None:
    ts = as_utc_naive(ts_utc)
    if ts is None:
        return
    close_ts = ts + expected_delta
    upsert_candle_slot_audit(
        conn,
        args=args,
        instrument=instrument,
        slot_time=close_ts,
        expected_close_time=close_ts,
        nt_connected=True,
        candle_received=True,
        scanner_status=scanner_status,
        trade_allowed=trade_allowed,
        blocked_reason=blocked_reason,
        details=details,
    )


def lock_scanner_candle_row(
    conn,
    *,
    args: argparse.Namespace,
    candle_snapshot: dict[str, Any] | None,
) -> None:
    if not candle_snapshot:
        return
    candle_id = wave.finite(candle_snapshot.get("candle_id"), None)
    if candle_id is None:
        return
    with conn.cursor() as cur:
        cur.execute(
            """
            UPDATE ninjatrader_live_candles
            SET scanner_locked_at = COALESCE(scanner_locked_at, CURRENT_TIMESTAMP(6)),
                scanner_lock_run_id = COALESCE(scanner_lock_run_id, %s)
            WHERE id = %s
            """,
            (str(args.run_id), int(candle_id)),
        )
    conn.commit()


def live_event_uid(run_id: str, event_type: str, count: int | None = None) -> str:
    raw = f"{run_id}|{event_type}|{count if count is not None else 'x'}|{time.time_ns()}"
    if len(raw) <= 240:
        return raw
    digest = sha1(raw.encode("utf-8")).hexdigest()[:16]
    return f"{str(run_id)[:170]}|{event_type}|{count if count is not None else 'x'}|{digest}"


def is_market_session_open(now_utc: pd.Timestamp, timezone_name: str) -> bool:
    try:
        tz = ZoneInfo(timezone_name)
    except Exception:
        tz = ZoneInfo("America/Chicago")
    ts = pd.Timestamp(now_utc)
    if ts.tzinfo is None:
        ts = ts.tz_localize("UTC")
    else:
        ts = ts.tz_convert("UTC")
    local = ts.tz_convert(tz)
    weekday = int(local.weekday())  # Monday = 0, Sunday = 6
    minutes = local.hour * 60 + local.minute + (local.second / 60.0)
    if weekday == 5:
        return False
    if weekday == 6:
        return minutes >= 17 * 60
    if weekday in {0, 1, 2, 3}:
        return not (16 * 60 <= minutes < 17 * 60)
    if weekday == 4:
        return minutes < 16 * 60
    return False


def heartbeat_latest_close_ts(heartbeat: dict[str, Any] | None) -> pd.Timestamp | None:
    if not heartbeat:
        return None
    value = heartbeat.get("last_candle_time_utc")
    if value is None or pd.isna(value):
        return None
    ts = pd.Timestamp(value)
    if pd.isna(ts):
        return None
    if ts.tzinfo is not None:
        ts = ts.tz_convert("UTC").tz_localize(None)
    return ts


def find_open_session_candle_gap(
    times: pd.Series,
    expected_delta: pd.Timedelta,
    timezone_name: str,
    latest_close_ts: pd.Timestamp | None = None,
) -> dict[str, Any] | None:
    clean_times = (
        pd.to_datetime(times, errors="coerce")
        .dropna()
        .drop_duplicates()
        .sort_values()
        .reset_index(drop=True)
    )
    if len(clean_times) < 2 or expected_delta <= pd.Timedelta(0):
        return None

    # Check only the current live area. Sparse older historical/no-trade bars are
    # not relevant to whether the scanner can safely score the next candle.
    if latest_close_ts is not None and not pd.isna(latest_close_ts):
        latest_close = pd.Timestamp(latest_close_ts)
        if latest_close.tzinfo is not None:
            latest_close = latest_close.tz_convert("UTC").tz_localize(None)
        live_window = max(expected_delta * 12, pd.Timedelta(minutes=30))
        recent_times = clean_times[clean_times >= latest_close - live_window - expected_delta].reset_index(drop=True)
    else:
        recent_times = clean_times.iloc[-60:].reset_index(drop=True)
    tolerance = pd.Timedelta(seconds=1)
    max_steps = 300
    for idx in range(1, len(recent_times)):
        previous_ts = pd.Timestamp(recent_times.iloc[idx - 1])
        current_ts = pd.Timestamp(recent_times.iloc[idx])
        expected_ts = previous_ts + expected_delta
        missing_open_bars = 0
        first_missing_ts = None
        steps = 0
        while expected_ts < current_ts - tolerance and steps < max_steps:
            expected_close_ts = expected_ts + expected_delta
            if is_market_session_open(expected_close_ts, timezone_name):
                missing_open_bars += 1
                if first_missing_ts is None:
                    first_missing_ts = expected_ts
            expected_ts += expected_delta
            steps += 1
        if missing_open_bars:
            return {
                "previous_ts_utc": previous_ts,
                "next_seen_ts_utc": current_ts,
                "first_missing_ts_utc": first_missing_ts,
                "estimated_missing_bars": missing_open_bars,
            }
    return None


def evaluate_candle_preflight(
    *,
    heartbeat: dict[str, Any] | None,
    times: pd.Series,
    latest_ts: pd.Timestamp,
    latest_close_ts: pd.Timestamp,
    expected_delta: pd.Timedelta,
    args: argparse.Namespace,
) -> dict[str, Any] | None:
    heartbeat_close_ts = heartbeat_latest_close_ts(heartbeat)
    if heartbeat_close_ts is not None and latest_close_ts < heartbeat_close_ts - pd.Timedelta(seconds=1):
        return {
            "reason": "db_behind_heartbeat",
            "heartbeat_latest_close_ts_utc": heartbeat_close_ts,
            "db_latest_ts_utc": latest_ts,
            "db_latest_close_ts_utc": latest_close_ts,
            "estimated_missing_bars": max(
                1,
                int(math.ceil((heartbeat_close_ts - latest_close_ts) / expected_delta)),
            )
            if expected_delta > pd.Timedelta(0)
            else None,
            "read": "NT heartbeat sees a newer closed candle than the DB. The monitor is waiting for candle storage/backfill before scanning.",
        }

    return None


def reset_live_decision_state(
    pending: dict[str, live_runner.PendingEvent],
    scheduled_entries: dict[tuple[str, int, str], live_runner.ScheduledEntry],
    next_allowed_stage1: dict[tuple[str, str], int],
) -> dict[str, int]:
    cleared = {
        "pending_events_cleared": len(pending),
        "scheduled_entries_cleared": len(scheduled_entries),
        "cooldowns_cleared": len(next_allowed_stage1),
    }
    pending.clear()
    scheduled_entries.clear()
    next_allowed_stage1.clear()
    return cleared


def insert_feed_health_event(
    conn,
    *,
    run_id: str,
    event_uid: str,
    event_type: str,
    instrument: str | None,
    root_symbol: str | None,
    timeframe: str | None,
    status: str | None,
    restored_at: Any = None,
    duration_seconds: float | None = None,
    heartbeat: dict[str, Any] | None = None,
    heartbeat_age_seconds: float | None = None,
    latest_candle_ts_utc: Any = None,
    latest_candle_close_ts_utc: Any = None,
    skipped_candles: int | None = None,
    estimated_missing_bars: int | None = None,
    details: dict[str, Any] | None = None,
) -> None:
    payload = json.dumps(details or {}, default=to_jsonable, sort_keys=True)
    heartbeat_received_at = heartbeat.get("received_at") if heartbeat else None
    if heartbeat_age_seconds is None and heartbeat:
        heartbeat_age_seconds = wave.finite(heartbeat.get("age_seconds"), None)

    with conn.cursor() as cur:
        cur.execute(
            """
            INSERT INTO ninjatrader_feed_health_events (
                run_id, event_uid, event_type, instrument, root_symbol, timeframe, status,
                restored_at, duration_seconds, heartbeat_received_at, heartbeat_age_seconds,
                latest_candle_ts_utc, latest_candle_close_ts_utc, skipped_candles,
                estimated_missing_bars, details_json
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            ON DUPLICATE KEY UPDATE
                instrument = VALUES(instrument),
                root_symbol = VALUES(root_symbol),
                timeframe = VALUES(timeframe),
                status = VALUES(status),
                restored_at = VALUES(restored_at),
                duration_seconds = VALUES(duration_seconds),
                heartbeat_received_at = VALUES(heartbeat_received_at),
                heartbeat_age_seconds = VALUES(heartbeat_age_seconds),
                latest_candle_ts_utc = VALUES(latest_candle_ts_utc),
                latest_candle_close_ts_utc = VALUES(latest_candle_close_ts_utc),
                skipped_candles = VALUES(skipped_candles),
                estimated_missing_bars = VALUES(estimated_missing_bars),
                details_json = VALUES(details_json),
                updated_at = CURRENT_TIMESTAMP
            """,
            (
                run_id,
                event_uid,
                event_type,
                instrument,
                root_symbol,
                timeframe,
                status,
                pd.Timestamp(restored_at).to_pydatetime() if restored_at is not None and not pd.isna(restored_at) else None,
                duration_seconds,
                heartbeat_received_at,
                heartbeat_age_seconds,
                pd.Timestamp(latest_candle_ts_utc).to_pydatetime()
                if latest_candle_ts_utc is not None and not pd.isna(latest_candle_ts_utc)
                else None,
                pd.Timestamp(latest_candle_close_ts_utc).to_pydatetime()
                if latest_candle_close_ts_utc is not None and not pd.isna(latest_candle_close_ts_utc)
                else None,
                skipped_candles,
                estimated_missing_bars,
                payload,
            ),
        )
    conn.commit()


def maintain_order_signal_state(
    conn,
    args: argparse.Namespace,
    instrument: str,
    *,
    email_config: dict[str, Any] | None,
    email_enabled: bool,
    email_started_at: Any,
) -> int:
    if not args.queue_orders:
        return 0
    execution_instrument, _ = execution_instrument_for(args, instrument)
    reconcile_completed_signals(
        conn,
        args,
        execution_instrument,
        email_config=email_config,
        email_enabled=email_enabled,
    )
    repair_completed_signal_details(conn, args, execution_instrument)
    return notify_completed_trades_once(
        conn,
        args,
        execution_instrument,
        email_config=email_config,
        email_enabled=email_enabled,
        updated_after=email_started_at,
    )


def build_model_args(args: argparse.Namespace) -> argparse.Namespace:
    return argparse.Namespace(
        year=int(args.year),
        timeframe=str(args.timeframe),
        entry_breakout_bars=int(args.entry_breakout_bars),
        trail_lookback_bars=int(args.trail_lookback_bars),
        atr_period=int(args.atr_period),
        atr_stop_pad=float(args.atr_stop_pad),
        min_risk_ticks=float(args.min_risk_ticks),
        max_risk_ticks=float(args.max_risk_ticks),
        min_relative_volume=float(args.min_relative_volume),
        dynamic_max_bars=int(args.dynamic_max_bars),
        max_forward_bars=int(args.max_forward_bars),
        time_exit_bars=int(args.time_exit_bars),
        slippage_entry_ticks=float(args.slippage_entry_ticks),
        slippage_exit_ticks=float(args.slippage_exit_ticks),
    )


def load_model_bundle(args: argparse.Namespace) -> dict[str, Any]:
    governed = load_json(model_dir(args.governed_run_id) / "metadata.json")
    rules = dict(governed["rules"])
    allowed_root_directions = parse_csv_strings(args.allowed_root_directions)
    if allowed_root_directions:
        rules["root_directions"] = sorted(allowed_root_directions)

    l2_dir = model_dir(args.level2_run_id)
    l2_meta = load_json(l2_dir / "metadata.json")
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
        level1_valid_year=int(args.year),
    )

    stage2_dir = model_dir(args.stage2_run_id)
    stage2_meta = load_json(stage2_dir / "metadata.json")
    models = {
        "cat": load_catboost_model(model_dir(l2_run_ids["catboost"]) / "catboost_oracle_start_live_grid_model.cbm"),
        "light": lgbm_utils.load_model(model_dir(l2_run_ids["lightgbm"])),
        "xgb": xgb_utils.load_model(model_dir(l2_run_ids["xgboost"])),
        "level2": load_catboost_model(l2_dir / "catboost_oracle_start_level2_three_l1_manager.cbm"),
        "stage2": load_catboost_model(stage2_dir / "catboost_stage2_l2_confirmation.cbm"),
    }
    return {
        "rules": rules,
        "l2_meta": l2_meta,
        "l2_args": l2_args,
        "stage2_meta": stage2_meta,
        "models": models,
    }


def timestamp_key(value: Any) -> str | None:
    if value is None:
        return None
    try:
        timestamp = pd.Timestamp(value)
    except (TypeError, ValueError):
        return None
    if pd.isna(timestamp):
        return None
    if timestamp.tzinfo is not None:
        timestamp = timestamp.tz_convert("UTC").tz_localize(None)
    return timestamp.floor("s").isoformat()


def timestamp_index(frame: pd.DataFrame, column: str = "ts_utc") -> dict[str, int]:
    if column not in frame.columns:
        return {}
    out: dict[str, int] = {}
    for idx, value in enumerate(frame[column]):
        key = timestamp_key(value)
        if key:
            out[key] = int(idx)
    return out


def reanchor_pending_signal_indexes(
    pending: dict[str, live_runner.PendingEvent],
    enriched: pd.DataFrame,
) -> list[str]:
    by_time = timestamp_index(enriched, "ts_utc")
    missing: list[str] = []
    for uid, item in list(pending.items()):
        key = timestamp_key(item.event.get("signal_date") or item.event.get("ts_utc"))
        if not key:
            continue
        signal_idx = by_time.get(key)
        if signal_idx is None:
            missing.append(uid)
            continue
        item.signal_idx = int(signal_idx)
    return missing


def process_cycle(
    conn,
    *,
    args: argparse.Namespace,
    model_args: argparse.Namespace,
    bundle: dict[str, Any],
    instrument: str,
    candles: pd.DataFrame,
    enriched: pd.DataFrame,
    current_idx: int,
    pending: dict[str, live_runner.PendingEvent],
    scheduled_entries: dict[tuple[str, int, str], live_runner.ScheduledEntry],
    next_allowed_stage1: dict[tuple[str, str], str],
    email_config: dict[str, Any] | None = None,
    email_enabled: bool = False,
    email_started_at: Any = None,
) -> dict[str, Any]:
    ts_utc = pd.Timestamp(enriched["ts_utc"].iloc[current_idx])
    candle_time = pd.Timestamp(candles["candle_time"].iloc[current_idx])
    root = str(args.root).upper()
    symbol = str(candles["symbol"].iloc[current_idx])
    current_indices = {symbol: int(current_idx)}
    symbol_groups = {symbol: enriched}
    enriched_time_index = timestamp_index(enriched, "ts_utc")
    directions = live_runner.direction_for_root(root, set(bundle["rules"]["root_directions"]))
    max_confirm_bars = int(bundle["stage2_meta"]["max_confirm_bars"])

    cycle = {
        "ts_utc": ts_utc,
        "candle_time": candle_time,
        "stage1_rows": 0,
        "level2_picks": 0,
        "stage2_rows": 0,
        "trend_confirms": 0,
        "order_signals": 0,
        "order_rejects": 0,
        "email_notifications": 0,
        "paper_entries": 0,
        "paper_rejects": 0,
        "level2_threshold": None,
        "level2_best_score": None,
        "level2_best_direction": None,
        "level2_long_score": None,
        "level2_short_score": None,
        "stage1_status": None,
        "stage2_status": None,
        "stage2_threshold": None,
        "stage2_best_score": None,
        "stage2_best_direction": None,
        "stage2_confirmed_score": None,
        "stage2_confirmed_direction": None,
        "stage2_confirm_offset_bars": None,
        "decision_status": None,
    }

    if args.queue_orders:
        execution_instrument, _ = execution_instrument_for(args, instrument)
        reconcile_completed_signals(
            conn,
            args,
            execution_instrument,
            email_config=email_config,
            email_enabled=email_enabled,
        )
        repair_completed_signal_details(conn, args, execution_instrument)
        cycle["email_notifications"] += notify_completed_trades_once(
            conn,
            args,
            execution_instrument,
            email_config=email_config,
            email_enabled=email_enabled,
            updated_after=email_started_at,
        )

    # Delayed audit of a previously scheduled next-candle entry. This is not an
    # order; it only records what the entry open/risk became once the bar exists.
    for direction in directions:
        scheduled = scheduled_entries.pop((symbol, current_idx, direction), None)
        if scheduled is None:
            continue
        tick_size = wave.tick_size_for(symbol, root)
        stop_price, risk_points = wave.initial_stop(enriched, current_idx, direction, tick_size, model_args)
        entry_price = wave.finite(enriched["open"].iloc[current_idx], None)
        risk_ticks = (risk_points / tick_size) if risk_points is not None and tick_size > 0 else None
        risk_ok = (
            entry_price is not None
            and stop_price is not None
            and risk_points is not None
            and risk_ticks is not None
            and float(bundle["rules"]["risk_ticks_min"]) <= float(risk_ticks) <= float(bundle["rules"]["risk_ticks_max"])
        )
        status = "accepted" if risk_ok else "rejected_risk"
        changed = insert_event(
            conn,
            run_id=args.run_id,
            event_uid=f"{args.run_id}|paper_entry|{scheduled.event.get('candidate_uid')}|{current_idx}",
            event_type="paper_entry_audit",
            instrument=instrument,
            root_symbol=root,
            model_symbol=symbol,
            timeframe=args.timeframe,
            candle_time=candle_time,
            ts_utc=ts_utc,
            direction=direction,
            level2_score=wave.finite(scheduled.event.get("stage1_score"), None),
            stage2_score=float(scheduled.stage2_score),
            entry_price=float(entry_price) if entry_price is not None else None,
            stop_price=float(stop_price) if stop_price is not None else None,
            risk_ticks=float(risk_ticks) if risk_ticks is not None else None,
            status=status,
            details={
                "read": "Delayed audit only. This row does not submit an order.",
                "confirm_date": scheduled.confirm_date,
                "candidate_uid": scheduled.event.get("candidate_uid"),
                "entry_candle": candle_audit_snapshot(candles, current_idx),
                "confirm_candle": candle_audit_snapshot(candles, int(current_idx) - 1),
                "signal_candle": candle_audit_snapshot(candles, scheduled.event.get("signal_idx")),
            },
        )
        if changed and risk_ok:
            print(
                json.dumps(
                    {
                        "event": "paper_entry_audit",
                        "status": status,
                        "instrument": instrument,
                        "symbol": symbol,
                        "direction": direction,
                        "candle_time": to_jsonable(candle_time),
                        "entry_price": entry_price,
                        "stop_price": stop_price,
                        "risk_ticks": risk_ticks,
                        "stage2_score": float(scheduled.stage2_score),
                    },
                    default=to_jsonable,
                ),
                flush=True,
            )
        cycle["paper_entries" if risk_ok else "paper_rejects"] += 1

    base_rows: list[dict[str, Any]] = []
    signal_idx = int(current_idx) - 1
    if signal_idx >= 60 and signal_idx < len(enriched) - 1:
        for direction in directions:
            row = start_model.feature_row(int(args.year), root, symbol, enriched, signal_idx, direction)
            row["signal_idx"] = int(signal_idx)
            row["source_timeframe"] = str(args.timeframe)
            row["timeframe_minutes"] = float(scanner.table_for_timeframe(args.timeframe)[1])
            base_rows.append(row)
    cycle["stage1_rows"] = len(base_rows)
    cycle["stage1_status"] = "scored" if base_rows else "not_enough_warmup_or_no_direction"
    cycle["decision_status"] = "scored_no_pick" if base_rows else "not_scored"

    if base_rows:
        scored = live_runner.score_level1_and_level2(base_rows, bundle["models"], bundle["l2_args"], args.timeframe)
        l2_threshold = float(bundle["l2_meta"]["selected_threshold"])
        cooldown_bars = int(bundle["l2_meta"]["event_rules"]["cooldown_bars"])
        cycle["level2_threshold"] = l2_threshold
        scored_records = scored.to_dict("records")
        for scored_row in scored_records:
            scored_direction = str(scored_row.get("direction") or "").upper()
            scored_score = wave.finite(scored_row.get("level2_score"), None)
            if scored_score is None:
                continue
            if scored_direction == "LONG":
                cycle["level2_long_score"] = float(scored_score)
            elif scored_direction == "SHORT":
                cycle["level2_short_score"] = float(scored_score)
            if cycle["level2_best_score"] is None or float(scored_score) > float(cycle["level2_best_score"]):
                cycle["level2_best_score"] = float(scored_score)
                cycle["level2_best_direction"] = scored_direction or None
        for row in scored_records:
            score = wave.finite(row.get("level2_score"), 0.0) or 0.0
            if score < l2_threshold:
                continue
            key = (str(row["symbol"]), str(row["direction"]))
            row_signal_idx = int(row["signal_idx"])
            row_signal_time_key = timestamp_key(row.get("signal_date") or enriched["ts_utc"].iloc[row_signal_idx])
            previous_signal_time_key = next_allowed_stage1.get(key)
            previous_signal_idx = enriched_time_index.get(previous_signal_time_key) if previous_signal_time_key else None
            if previous_signal_idx is not None and row_signal_idx - int(previous_signal_idx) <= cooldown_bars:
                continue
            if row_signal_time_key:
                next_allowed_stage1[key] = row_signal_time_key
            row["stage1_pick"] = 1
            row["stage1_score"] = score
            pending[str(row["candidate_uid"])] = live_runner.PendingEvent(event=row, signal_idx=row_signal_idx)
            cycle["level2_picks"] += 1
            cycle["decision_status"] = "watching_stage2"
            cycle["stage2_status"] = "watching"
            insert_event(
                conn,
                run_id=args.run_id,
                event_uid=f"{args.run_id}|level2_pick|{row['candidate_uid']}",
                event_type="level2_pick",
                instrument=instrument,
                root_symbol=root,
                model_symbol=symbol,
                timeframe=args.timeframe,
                candle_time=candle_time,
                ts_utc=ts_utc,
                direction=str(row["direction"]),
                level2_score=float(score),
                status="watching_stage2",
                details={
                    "signal_date": row.get("signal_date"),
                    "signal_idx": row_signal_idx,
                    "signal_time_key": row_signal_time_key,
                    "max_confirm_bars": max_confirm_bars,
                    "candidate_uid": row.get("candidate_uid"),
                    "signal_candle": candle_audit_snapshot(candles, row_signal_idx),
                    "scanner_candle": candle_audit_snapshot(candles, current_idx),
                    "scores": {
                        "cat_l1_score": wave.finite(row.get("cat_l1_score"), None),
                        "light_l1_score": wave.finite(row.get("light_l1_score"), None),
                        "xgb_l1_score": wave.finite(row.get("xgb_l1_score"), None),
                        "level2_score": float(score),
                    },
                },
            )

    expired_missing = reanchor_pending_signal_indexes(pending, enriched)
    for uid in expired_missing:
        item = pending.pop(uid, None)
        if item is None:
            continue
        insert_event(
            conn,
            run_id=args.run_id,
            event_uid=f"{args.run_id}|stage2_expired|{uid}",
            event_type="stage2_expired",
            instrument=instrument,
            root_symbol=root,
            model_symbol=symbol,
            timeframe=args.timeframe,
            candle_time=candle_time,
            ts_utc=ts_utc,
            direction=str(item.event.get("direction")),
            level2_score=wave.finite(item.event.get("stage1_score"), None),
            status="expired_missing_signal",
            details={
                "candidate_uid": uid,
                "max_confirm_bars": max_confirm_bars,
                "read": "Stage 2 watch expired because its signal candle left the live lookback window.",
                "signal_candle": candle_audit_snapshot(candles, item.signal_idx),
                "scanner_candle": candle_audit_snapshot(candles, current_idx),
            },
        )

    pending_rows = live_runner.stage2_rows_for_pending(
        list(pending.values()),
        symbol_groups,
        current_indices,
        max_confirm_bars,
    )
    cycle["stage2_rows"] = int(len(pending_rows))
    confirmed_uids: set[str] = set()
    if not pending_rows.empty:
        pending_rows["stage2_score"] = bundle["models"]["stage2"].predict_proba(
            stage2_l2.prepare_pool(pending_rows, include_target=False)
        )[:, 1]
        stage2_threshold = float(bundle["stage2_meta"]["selected_stage2_threshold"])
        min_stage2_score = float(bundle["rules"]["min_stage2_score"])
        cycle["stage2_threshold"] = stage2_threshold
        for stage2_row in pending_rows.to_dict("records"):
            score = wave.finite(stage2_row.get("stage2_score"), None)
            if score is None:
                continue
            if cycle["stage2_best_score"] is None or float(score) > float(cycle["stage2_best_score"]):
                cycle["stage2_best_score"] = float(score)
                cycle["stage2_best_direction"] = str(stage2_row.get("direction") or "").upper() or None
        if cycle["stage2_status"] is None:
            cycle["stage2_status"] = "watching"
        hits = pending_rows[
            pd.to_numeric(pending_rows["stage2_score"], errors="coerce").fillna(0.0) >= stage2_threshold
        ].copy()
        for row in hits.sort_values(["candidate_uid", "confirm_offset_bars"]).drop_duplicates("candidate_uid").to_dict("records"):
            uid = str(row["candidate_uid"])
            confirmed_uids.add(uid)
            stage2_score = wave.finite(row.get("stage2_score"), 0.0) or 0.0
            direction = str(row["direction"])
            status = "confirmed" if stage2_score >= min_stage2_score else "rejected_min_stage2_score"
            if stage2_score >= min_stage2_score:
                cycle["stage2_status"] = "confirmed"
                cycle["stage2_confirmed_score"] = float(stage2_score)
                cycle["stage2_confirmed_direction"] = direction
                offset = wave.finite(row.get("confirm_offset_bars"), None)
                cycle["stage2_confirm_offset_bars"] = int(offset) if offset is not None else None
                cycle["decision_status"] = "confirmed"
            else:
                cycle["stage2_status"] = "rejected_min_stage2_score"
                cycle["decision_status"] = "stage2_rejected"
            changed = insert_event(
                conn,
                run_id=args.run_id,
                event_uid=f"{args.run_id}|trend_confirmed|{uid}",
                event_type="trend_confirmed",
                instrument=instrument,
                root_symbol=root,
                model_symbol=symbol,
                timeframe=args.timeframe,
                candle_time=candle_time,
                ts_utc=ts_utc,
                direction=direction,
                level2_score=wave.finite(row.get("stage1_score"), None),
                stage2_score=float(stage2_score),
                status=status,
                details={
                    "signal_date": row.get("signal_date"),
                    "confirm_date": row.get("confirm_date") or ts_utc,
                    "confirm_offset_bars": row.get("confirm_offset_bars"),
                    "planned_entry_read": "If this were enabled for trading, the next candle open is the intended entry point.",
                    "candidate_uid": uid,
                    "signal_candle": candle_audit_snapshot(candles, row.get("signal_idx")),
                    "confirm_candle": candle_audit_snapshot(candles, current_idx),
                    "scores": {
                        "cat_l1_score": wave.finite(row.get("cat_l1_score"), None),
                        "light_l1_score": wave.finite(row.get("light_l1_score"), None),
                        "xgb_l1_score": wave.finite(row.get("xgb_l1_score"), None),
                        "level2_score": wave.finite(row.get("stage1_score"), None),
                        "stage2_score": float(stage2_score),
                    },
                },
            )
            if stage2_score < min_stage2_score:
                continue
            cycle["trend_confirms"] += 1
            signal_details: dict[str, Any] = {}
            if args.queue_orders:
                execution_instrument, execution_root = execution_instrument_for(args, instrument)
                reconcile_completed_signals(conn, args, execution_instrument)
                active_count = active_signal_count(conn, args, execution_instrument)
                if active_count >= int(args.max_active_signals):
                    cycle["order_rejects"] += 1
                    cycle["decision_status"] = "blocked_active_signal"
                    signal_details = {
                        "reject_reason": "active_signal_limit",
                        "active_signals": active_count,
                        "max_active_signals": int(args.max_active_signals),
                        "execution_instrument": execution_instrument,
                    }
                    insert_event(
                        conn,
                        run_id=args.run_id,
                        event_uid=f"{args.run_id}|order_signal|{uid}",
                        event_type="order_signal",
                        instrument=execution_instrument,
                        root_symbol=execution_root,
                        model_symbol=symbol,
                        timeframe=args.timeframe,
                        candle_time=candle_time,
                        ts_utc=ts_utc,
                        direction=direction,
                        level2_score=wave.finite(row.get("stage1_score"), None),
                        stage2_score=float(stage2_score),
                        status="blocked_active_signal",
                        details={**signal_details, "candidate_uid": uid},
                    )
                else:
                    payload, signal_details = build_live_order_signal(
                        args=args,
                        model_args=model_args,
                        bundle=bundle,
                        instrument=instrument,
                        execution_instrument=execution_instrument,
                        execution_root=execution_root,
                        root=root,
                        symbol=symbol,
                        direction=direction,
                        current_idx=int(current_idx),
                        candle_time=candle_time,
                        ts_utc=ts_utc,
                        candles=candles,
                        enriched=enriched,
                        row=row,
                        stage2_score=float(stage2_score),
                    )
                    if payload is None:
                        cycle["order_rejects"] += 1
                        cycle["decision_status"] = str(signal_details.get("reject_reason") or "order_rejected")
                        insert_event(
                            conn,
                            run_id=args.run_id,
                            event_uid=f"{args.run_id}|order_signal|{uid}",
                            event_type="order_signal",
                            instrument=execution_instrument,
                            root_symbol=execution_root,
                            model_symbol=symbol,
                            timeframe=args.timeframe,
                            candle_time=candle_time,
                            ts_utc=ts_utc,
                            direction=direction,
                            level2_score=wave.finite(row.get("stage1_score"), None),
                            stage2_score=float(stage2_score),
                            entry_price=wave.finite(signal_details.get("entry_price"), None),
                            stop_price=wave.finite(signal_details.get("stop_price"), None),
                            risk_ticks=wave.finite(signal_details.get("risk_ticks"), None),
                            status=str(signal_details.get("reject_reason") or "rejected"),
                            details={**signal_details, "candidate_uid": uid},
                        )
                    else:
                        try:
                            response = post_order_signal(str(args.server_url), payload)
                            update_signal_accounting_columns(conn, signal_details["signal_uid"], signal_details)
                            cycle["order_signals"] += 1
                            cycle["decision_status"] = "queued_order"
                            insert_event(
                                conn,
                                run_id=args.run_id,
                                event_uid=f"{args.run_id}|order_signal|{uid}",
                                event_type="order_signal",
                                instrument=execution_instrument,
                                root_symbol=execution_root,
                                model_symbol=symbol,
                                timeframe=args.timeframe,
                                candle_time=candle_time,
                                ts_utc=ts_utc,
                                direction=direction,
                                level2_score=wave.finite(row.get("stage1_score"), None),
                                stage2_score=float(stage2_score),
                                entry_price=float(signal_details["entry_price"]),
                                stop_price=float(signal_details["stop_price"]),
                                risk_ticks=float(signal_details["risk_ticks"]),
                                status="queued",
                                details={
                                    **signal_details,
                                    "candidate_uid": uid,
                                    "server_response": response,
                                    "payload": payload,
                                },
                            )
                            print(
                                json.dumps(
                                    {
                                        "event": "ORDER_SIGNAL_QUEUED",
                                        "signal_uid": payload["signal_uid"],
                                        "detect_instrument": instrument,
                                        "execution_instrument": execution_instrument,
                                        "direction": direction,
                                        "entry_price": signal_details["entry_price"],
                                        "stop_price": signal_details["stop_price"],
                                        "risk_ticks": signal_details["risk_ticks"],
                                        "stage2_score": float(stage2_score),
                                    },
                                    default=to_jsonable,
                                ),
                                flush=True,
                            )
                        except (URLError, TimeoutError, OSError, ValueError) as exc:
                            cycle["order_rejects"] += 1
                            cycle["decision_status"] = "queue_failed"
                            insert_event(
                                conn,
                                run_id=args.run_id,
                                event_uid=f"{args.run_id}|order_signal|{uid}",
                                event_type="order_signal",
                                instrument=execution_instrument,
                                root_symbol=execution_root,
                                model_symbol=symbol,
                                timeframe=args.timeframe,
                                candle_time=candle_time,
                                ts_utc=ts_utc,
                                direction=direction,
                                level2_score=wave.finite(row.get("stage1_score"), None),
                                stage2_score=float(stage2_score),
                                entry_price=wave.finite(signal_details.get("entry_price"), None),
                                stop_price=wave.finite(signal_details.get("stop_price"), None),
                                risk_ticks=wave.finite(signal_details.get("risk_ticks"), None),
                                status="queue_failed",
                                details={**signal_details, "candidate_uid": uid, "error": str(exc)},
                            )
            else:
                entry_idx = int(current_idx) + 1
                scheduled_entries[(symbol, entry_idx, direction)] = live_runner.ScheduledEntry(
                    event=row,
                    entry_idx=entry_idx,
                    stage2_score=float(stage2_score),
                    confirm_date=pd.Timestamp(row.get("confirm_date") or ts_utc),
                )
            if changed:
                print(
                    json.dumps(
                        {
                            "event": "TREND_FOUND",
                            "instrument": instrument,
                            "symbol": symbol,
                            "timeframe": args.timeframe,
                            "direction": direction,
                            "candle_time": to_jsonable(candle_time),
                            "ts_utc": to_jsonable(ts_utc),
                            "level2_score": wave.finite(row.get("stage1_score"), None),
                            "stage2_score": float(stage2_score),
                            "planned_action": "queued demo order signal" if args.queue_orders else "watch next candle open; no order submitted",
                            "order_signal": signal_details or None,
                        },
                        default=to_jsonable,
                    ),
                    flush=True,
                )

    expired = []
    for uid, item in pending.items():
        if uid in confirmed_uids:
            expired.append(uid)
            continue
        if current_idx - int(item.signal_idx) >= max_confirm_bars:
            expired.append(uid)
            cycle["stage2_status"] = "expired"
            if cycle.get("decision_status") in {None, "watching_stage2", "scored_no_pick"}:
                cycle["decision_status"] = "stage2_expired"
            insert_event(
                conn,
                run_id=args.run_id,
                event_uid=f"{args.run_id}|stage2_expired|{uid}",
                event_type="stage2_expired",
                instrument=instrument,
                root_symbol=root,
                model_symbol=symbol,
                timeframe=args.timeframe,
                candle_time=candle_time,
                ts_utc=ts_utc,
                direction=str(item.event.get("direction")),
                level2_score=wave.finite(item.event.get("stage1_score"), None),
                status="expired",
                details={"candidate_uid": uid, "max_confirm_bars": max_confirm_bars},
            )
    for uid in expired:
        pending.pop(uid, None)
    return cycle


def main() -> int:
    args = parse_args()
    model_args = build_model_args(args)
    scanner.configure_wave_args(model_args)

    print(
        json.dumps(
            {
                "event": "monitor_starting",
                "run_id": args.run_id,
                "root": args.root,
                "timeframe": args.timeframe,
                "allowed_root_directions": sorted(parse_csv_strings(args.allowed_root_directions)),
                "queue_orders": bool(args.queue_orders),
                "signal_account": args.signal_account if args.queue_orders else None,
                "signal_instrument": args.signal_instrument or None,
                "execution_root": args.execution_root or None,
                "accounting_instrument": args.accounting_instrument or None,
                "accounting_root": args.accounting_root or None,
                "sim_account_cash": float(args.sim_account_cash or 0.0) or None,
                "execution_tick_value": float(args.execution_tick_value or 0.0) or None,
                "accounting_tick_value": float(args.accounting_tick_value or 0.0) or None,
                "accounting_size_ratio": float(args.accounting_size_ratio or 0.0) or None,
                "max_accounting_risk_dollars": float(args.max_accounting_risk_dollars or 0.0) or None,
                "completed_trade_email": not bool(args.disable_completed_trade_email),
                "note": "queues demo NinjaTrader signals when Stage 2 confirms" if args.queue_orders else "read-only monitor; no orders are submitted",
            }
        ),
        flush=True,
    )

    bundle = load_model_bundle(args)
    conn = wave.connect()
    conn.autocommit(True)
    ensure_event_table(conn)
    ensure_route_table(conn)
    ensure_feed_health_tables(conn)
    ensure_candle_slot_audit_table(conn)
    ensure_live_candle_scanner_lock_columns(conn)
    ensure_order_signal_accounting_columns(conn)
    trade_email.ensure_columns(conn)

    email_config = trade_email.email_config()
    email_missing = trade_email.config_missing(email_config)
    email_error = None
    if not args.disable_completed_trade_email and not email_missing:
        email_error = trade_email.validate_smtp_login(email_config)
    email_enabled = not bool(args.disable_completed_trade_email) and not email_missing and not email_error
    with conn.cursor() as cur:
        cur.execute("SELECT CURRENT_TIMESTAMP(6) AS started_at")
        email_started_at = cur.fetchone()["started_at"]
    if args.disable_completed_trade_email:
        print(json.dumps({"event": "completed_trade_email_disabled"}), flush=True)
    elif email_missing:
        print(
            json.dumps(
                {
                    "event": "completed_trade_email_not_configured",
                    "missing": email_missing,
                    "read": "Set SMTP environment variables before starting the monitor to send completion emails automatically.",
                }
            ),
            flush=True,
        )
    elif email_error:
        print(
            json.dumps(
                {
                    "event": "completed_trade_email_smtp_failed",
                    "error": email_error,
                    "read": "Email disabled for this monitor run; fix SMTP credentials and restart the monitor.",
                }
            ),
            flush=True,
        )
    else:
        print(json.dumps({"event": "completed_trade_email_ready", "started_at": str(email_started_at)}), flush=True)

    instrument = args.instrument.strip() or latest_instrument(conn, args.root, args.timeframe)
    if not instrument:
        raise ValueError(f"No live candles found for {args.root} {args.timeframe}; cannot choose instrument.")

    pending: dict[str, live_runner.PendingEvent] = {}
    scheduled_entries: dict[tuple[str, int, str], live_runner.ScheduledEntry] = {}
    next_allowed_stage1: dict[tuple[str, str], int] = {}
    last_processed_ts: pd.Timestamp | None = None
    last_heartbeat = time.time()
    loops = 0
    cycles_seen = 0
    feed_outage_count = 0
    feed_paused = False
    feed_paused_since: pd.Timestamp | None = None
    feed_pause_reason = ""
    market_closed_logged = False
    market_resume_pending = False
    market_open_wait_logged = False
    candle_sync_paused = False
    candle_sync_wait_key = ""
    expected_delta = timeframe_delta(args.timeframe)

    try:
        while True:
            loops += 1
            if args.instrument.strip():
                instrument = args.instrument.strip()
            else:
                latest = latest_instrument(conn, args.root, args.timeframe)
                if latest:
                    instrument = latest

            now_utc = utc_now_naive()
            heartbeat = fetch_latest_heartbeat(conn, args, instrument)
            heartbeat_age_seconds = wave.finite(heartbeat.get("age_seconds"), None) if heartbeat else None

            if not is_market_session_open(now_utc, args.local_timezone):
                raw = fetch_live_candles(conn, args, instrument)
                loop_email_notifications = maintain_order_signal_state(
                    conn,
                    args,
                    instrument,
                    email_config=email_config,
                    email_enabled=email_enabled,
                    email_started_at=email_started_at,
                )
                latest_ts = None
                latest_close_ts = None
                latest_candle_time = None
                if not raw.empty:
                    times = pd.to_datetime(raw["ts_utc"], errors="coerce")
                    latest_ts = pd.Timestamp(times.max())
                    latest_close_ts = latest_ts + expected_delta
                    latest_candle_time = raw["candle_time"].iloc[-1]
                    last_processed_ts = latest_ts
                if not market_closed_logged:
                    cleared = reset_live_decision_state(pending, scheduled_entries, next_allowed_stage1)
                    event_uid = live_event_uid(args.run_id, "market_closed", feed_outage_count)
                    details = {
                        **cleared,
                        "feed_outage_count": feed_outage_count,
                        "heartbeat_age_seconds": heartbeat_age_seconds,
                        "heartbeat_received_at": heartbeat.get("received_at") if heartbeat else None,
                        "heartbeat_connection_status": heartbeat.get("connection_status") if heartbeat else None,
                        "latest_close_ts_utc": latest_close_ts,
                        "loop_email_notifications": loop_email_notifications,
                        "read": "Market is closed, so no new closed candle is expected. The monitor is waiting and will resume from the current candle when the session opens.",
                    }
                    insert_event(
                        conn,
                        run_id=args.run_id,
                        event_uid=event_uid,
                        event_type="market_closed",
                        instrument=instrument,
                        root_symbol=args.root,
                        model_symbol=nt_instrument_to_model_symbol(instrument, args.root),
                        timeframe=args.timeframe,
                        candle_time=latest_candle_time,
                        ts_utc=latest_ts,
                        direction=None,
                        status="waiting",
                        details=details,
                    )
                    insert_feed_health_event(
                        conn,
                        run_id=args.run_id,
                        event_uid=event_uid,
                        event_type="market_closed",
                        instrument=instrument,
                        root_symbol=args.root,
                        timeframe=args.timeframe,
                        status="waiting",
                        heartbeat=heartbeat,
                        heartbeat_age_seconds=heartbeat_age_seconds,
                        latest_candle_ts_utc=latest_ts,
                        latest_candle_close_ts_utc=latest_close_ts,
                        details=details,
                    )
                    current_close_slot = latest_expected_close_slot(now_utc, expected_delta)
                    missing_start = (
                        pd.Timestamp(last_processed_ts) + expected_delta
                        if last_processed_ts is not None
                        else current_close_slot
                    )
                    record_missing_slot_range(
                        conn,
                        args=args,
                        instrument=instrument,
                        start_close_ts=missing_start,
                        end_close_ts=current_close_slot,
                        expected_delta=expected_delta,
                        nt_connected=False,
                        blocked_reason="nt_connection_lost",
                        details={
                            "event_uid": event_uid,
                            "heartbeat_age_seconds": heartbeat_age_seconds,
                            "pause_reason": feed_pause_reason,
                            "read": "Expected candle slot while NT bridge heartbeat was stale or missing.",
                        },
                    )
                    print(
                        json.dumps(
                            {
                                "event": "market_closed",
                                "instrument": instrument,
                                "heartbeat_age_seconds": heartbeat_age_seconds,
                                "latest_ts_utc": to_jsonable(latest_ts),
                                **cleared,
                            },
                            default=to_jsonable,
                        ),
                        flush=True,
                    )
                    market_closed_logged = True
                    market_resume_pending = True
                    market_open_wait_logged = False
                    candle_sync_paused = False
                    candle_sync_wait_key = ""
                    feed_paused = False
                    feed_paused_since = None
                    feed_pause_reason = ""
                if int(args.max_loops) > 0 and loops >= int(args.max_loops):
                    break
                time.sleep(max(1.0, float(args.poll_seconds)))
                continue

            market_closed_logged = False
            heartbeat_limit = float(args.heartbeat_stale_seconds or 0.0)
            heartbeat_unhealthy = bool(
                args.require_heartbeat
                and heartbeat_limit > 0
                and (heartbeat is None or heartbeat_age_seconds is None or heartbeat_age_seconds > heartbeat_limit)
            )
            if heartbeat_unhealthy:
                if not feed_paused:
                    feed_outage_count += 1
                    feed_paused = True
                    feed_paused_since = utc_now_naive()
                    feed_pause_reason = "heartbeat_stale" if heartbeat else "heartbeat_missing"
                    cleared = reset_live_decision_state(pending, scheduled_entries, next_allowed_stage1)
                    event_uid = live_event_uid(args.run_id, "feed_heartbeat_stale", feed_outage_count)
                    details = {
                        **cleared,
                        "feed_outage_count": feed_outage_count,
                        "pause_reason": feed_pause_reason,
                        "heartbeat_stale_seconds": heartbeat_limit,
                        "heartbeat_age_seconds": heartbeat_age_seconds,
                        "heartbeat_received_at": heartbeat.get("received_at") if heartbeat else None,
                        "heartbeat_connection_status": heartbeat.get("connection_status") if heartbeat else None,
                        "last_processed_ts_utc": last_processed_ts,
                        "read": "NT heartbeat is stale or missing. New trend/trade decisions are paused so any missed setup is logged as feed downtime, not scanner failure.",
                    }
                    insert_event(
                        conn,
                        run_id=args.run_id,
                        event_uid=event_uid,
                        event_type="feed_heartbeat_stale",
                        instrument=instrument,
                        root_symbol=args.root,
                        model_symbol=nt_instrument_to_model_symbol(instrument, args.root),
                        timeframe=args.timeframe,
                        candle_time=None,
                        ts_utc=last_processed_ts,
                        direction=None,
                        status="paused",
                        details=details,
                    )
                    insert_feed_health_event(
                        conn,
                        run_id=args.run_id,
                        event_uid=event_uid,
                        event_type="feed_heartbeat_stale",
                        instrument=instrument,
                        root_symbol=args.root,
                        timeframe=args.timeframe,
                        status="paused",
                        heartbeat=heartbeat,
                        heartbeat_age_seconds=heartbeat_age_seconds,
                        latest_candle_ts_utc=last_processed_ts,
                        details=details,
                    )
                    print(
                        json.dumps(
                            {
                                "event": "feed_heartbeat_stale",
                                "instrument": instrument,
                                "feed_outage_count": feed_outage_count,
                                "heartbeat_age_seconds": heartbeat_age_seconds,
                                "heartbeat_stale_seconds": heartbeat_limit,
                                **cleared,
                            },
                            default=to_jsonable,
                        ),
                        flush=True,
                    )
                if int(args.max_loops) > 0 and loops >= int(args.max_loops):
                    break
                time.sleep(max(1.0, float(args.poll_seconds)))
                continue

            raw = fetch_live_candles(conn, args, instrument)
            if raw.empty:
                current_close_slot = latest_expected_close_slot(now_utc, expected_delta)
                record_missing_slot_range(
                    conn,
                    args=args,
                    instrument=instrument,
                    start_close_ts=current_close_slot,
                    end_close_ts=current_close_slot,
                    expected_delta=expected_delta,
                    nt_connected=True,
                    blocked_reason="no_candles_received",
                    details={
                        "heartbeat_age_seconds": heartbeat_age_seconds,
                        "heartbeat_received_at": heartbeat.get("received_at") if heartbeat else None,
                        "read": "NT bridge is connected, but no stored candles were available to the monitor.",
                    },
                )
                print(json.dumps({"event": "waiting_for_candles", "instrument": instrument}), flush=True)
            else:
                loop_email_notifications = maintain_order_signal_state(
                    conn,
                    args,
                    instrument,
                    email_config=email_config,
                    email_enabled=email_enabled,
                    email_started_at=email_started_at,
                )
                enriched = scanner.enrich_candles(
                    raw[["ts_utc", "open", "high", "low", "close", "volume"]].copy().reset_index(drop=True),
                    model_args,
                )
                record_candle_arrival_slots(
                    conn,
                    args=args,
                    instrument=instrument,
                    candles=raw,
                    expected_delta=expected_delta,
                )
                times = pd.to_datetime(enriched["ts_utc"], errors="coerce")
                latest_ts = pd.Timestamp(times.max())
                latest_close_ts = latest_ts + expected_delta
                now_utc = utc_now_naive()
                feed_age_seconds = max(0.0, (now_utc - latest_close_ts).total_seconds())
                heartbeat_close_ts = heartbeat_latest_close_ts(heartbeat)
                heartbeat_is_fresh = (
                    heartbeat is not None
                    and heartbeat_age_seconds is not None
                    and (heartbeat_limit <= 0 or heartbeat_age_seconds <= heartbeat_limit)
                )
                nt_reports_db_current = (
                    heartbeat_close_ts is not None
                    and latest_close_ts >= heartbeat_close_ts - pd.Timedelta(seconds=1)
                )
                no_new_nt_candle_expected = heartbeat_is_fresh and nt_reports_db_current
                preflight_issue = evaluate_candle_preflight(
                    heartbeat=heartbeat,
                    times=times,
                    latest_ts=latest_ts,
                    latest_close_ts=latest_close_ts,
                    expected_delta=expected_delta,
                    args=args,
                )
                if preflight_issue:
                    reason = str(preflight_issue.get("reason") or "candle_sync_waiting")
                    sync_key = "|".join(
                        [
                            reason,
                            str(to_jsonable(preflight_issue.get("heartbeat_latest_close_ts_utc"))),
                            str(to_jsonable(preflight_issue.get("db_latest_close_ts_utc"))),
                            str(to_jsonable(preflight_issue.get("first_missing_ts_utc"))),
                        ]
                    )
                    if not candle_sync_paused or sync_key != candle_sync_wait_key:
                        feed_outage_count += 1
                        candle_sync_paused = True
                        candle_sync_wait_key = sync_key
                        cleared = reset_live_decision_state(pending, scheduled_entries, next_allowed_stage1)
                        event_uid = live_event_uid(args.run_id, "candle_sync_waiting", feed_outage_count)
                        details = {
                            **cleared,
                            **preflight_issue,
                            "feed_outage_count": feed_outage_count,
                            "heartbeat_age_seconds": heartbeat_age_seconds,
                            "heartbeat_received_at": heartbeat.get("received_at") if heartbeat else None,
                            "heartbeat_connection_status": heartbeat.get("connection_status") if heartbeat else None,
                            "feed_age_seconds": round(feed_age_seconds, 3),
                            "resume_rule": "wait_for_db_to_match_nt_heartbeat",
                        }
                        insert_event(
                            conn,
                            run_id=args.run_id,
                            event_uid=event_uid,
                            event_type="candle_sync_waiting",
                            instrument=instrument,
                            root_symbol=args.root,
                            model_symbol=nt_instrument_to_model_symbol(instrument, args.root),
                            timeframe=args.timeframe,
                            candle_time=raw["candle_time"].iloc[-1],
                            ts_utc=latest_ts,
                            direction=None,
                            status="waiting",
                            details=details,
                        )
                        insert_feed_health_event(
                            conn,
                            run_id=args.run_id,
                            event_uid=event_uid,
                            event_type="candle_sync_waiting",
                            instrument=instrument,
                            root_symbol=args.root,
                            timeframe=args.timeframe,
                            status="waiting",
                            heartbeat=heartbeat,
                            heartbeat_age_seconds=heartbeat_age_seconds,
                            latest_candle_ts_utc=latest_ts,
                            latest_candle_close_ts_utc=latest_close_ts,
                            estimated_missing_bars=(
                                int(preflight_issue["estimated_missing_bars"])
                                if preflight_issue.get("estimated_missing_bars") is not None
                                else None
                            ),
                            details=details,
                        )
                        first_missing = preflight_issue.get("first_missing_ts_utc")
                        if first_missing is None:
                            first_missing = latest_close_ts + expected_delta
                        heartbeat_close = preflight_issue.get("heartbeat_latest_close_ts_utc") or heartbeat_latest_close_ts(heartbeat)
                        record_missing_slot_range(
                            conn,
                            args=args,
                            instrument=instrument,
                            start_close_ts=first_missing,
                            end_close_ts=heartbeat_close,
                            expected_delta=expected_delta,
                            nt_connected=True,
                            blocked_reason="connected_no_candle",
                            details={
                                **preflight_issue,
                                "event_uid": event_uid,
                                "read": "NT bridge is connected and reports a newer closed candle than the DB has stored.",
                            },
                        )
                        print(
                            json.dumps(
                                {
                                    "event": "candle_sync_waiting",
                                    "instrument": instrument,
                                    "reason": reason,
                                    "feed_outage_count": feed_outage_count,
                                    "latest_ts_utc": to_jsonable(latest_ts),
                                    "heartbeat_latest_close_ts_utc": to_jsonable(preflight_issue.get("heartbeat_latest_close_ts_utc")),
                                    "estimated_missing_bars": preflight_issue.get("estimated_missing_bars"),
                                    **cleared,
                                },
                                default=to_jsonable,
                            ),
                            flush=True,
                        )
                    last_processed_ts = latest_ts
                    if int(args.max_loops) > 0 and loops >= int(args.max_loops):
                        break
                    time.sleep(max(1.0, float(args.poll_seconds)))
                    continue

                if candle_sync_paused:
                    cleared = reset_live_decision_state(pending, scheduled_entries, next_allowed_stage1)
                    event_uid = live_event_uid(args.run_id, "candle_sync_ready", feed_outage_count)
                    details = {
                        **cleared,
                        "feed_outage_count": feed_outage_count,
                        "heartbeat_age_seconds": heartbeat_age_seconds,
                        "heartbeat_received_at": heartbeat.get("received_at") if heartbeat else None,
                        "heartbeat_connection_status": heartbeat.get("connection_status") if heartbeat else None,
                        "db_latest_ts_utc": latest_ts,
                        "db_latest_close_ts_utc": latest_close_ts,
                        "heartbeat_latest_close_ts_utc": heartbeat_latest_close_ts(heartbeat),
                        "resume_rule": "skip_to_current_candle",
                        "read": "Candle preflight is clean. The monitor reset state and will resume from the next clean candle.",
                    }
                    insert_event(
                        conn,
                        run_id=args.run_id,
                        event_uid=event_uid,
                        event_type="candle_sync_ready",
                        instrument=instrument,
                        root_symbol=args.root,
                        model_symbol=nt_instrument_to_model_symbol(instrument, args.root),
                        timeframe=args.timeframe,
                        candle_time=raw["candle_time"].iloc[-1],
                        ts_utc=latest_ts,
                        direction=None,
                        status="resumed_next_candle",
                        details=details,
                    )
                    insert_feed_health_event(
                        conn,
                        run_id=args.run_id,
                        event_uid=event_uid,
                        event_type="candle_sync_ready",
                        instrument=instrument,
                        root_symbol=args.root,
                        timeframe=args.timeframe,
                        status="resumed_next_candle",
                        restored_at=now_utc,
                        heartbeat=heartbeat,
                        heartbeat_age_seconds=heartbeat_age_seconds,
                        latest_candle_ts_utc=latest_ts,
                        latest_candle_close_ts_utc=latest_close_ts,
                        details=details,
                    )
                    print(
                        json.dumps(
                            {
                                "event": "candle_sync_ready",
                                "instrument": instrument,
                                "latest_ts_utc": to_jsonable(latest_ts),
                                **cleared,
                            },
                            default=to_jsonable,
                        ),
                        flush=True,
                    )
                    candle_sync_paused = False
                    candle_sync_wait_key = ""
                    last_processed_ts = latest_ts
                    if int(args.max_loops) > 0 and loops >= int(args.max_loops):
                        break
                    time.sleep(max(1.0, float(args.poll_seconds)))
                    continue

                if last_processed_ts is None:
                    if int(args.backfill_cycles) > 0 and len(times) > int(args.backfill_cycles):
                        last_processed_ts = pd.Timestamp(times.iloc[-int(args.backfill_cycles) - 1])
                    else:
                        last_processed_ts = latest_ts
                    print(
                        json.dumps(
                            {
                                "event": "monitor_ready",
                                "instrument": instrument,
                                "model_symbol": nt_instrument_to_model_symbol(instrument, args.root),
                                "rows_loaded": int(len(raw)),
                                "latest_candle_time": to_jsonable(raw["candle_time"].iloc[-1]),
                                "latest_ts_utc": to_jsonable(latest_ts),
                                "feed_age_seconds": round(feed_age_seconds, 1),
                                "processing_mode": "start_now" if int(args.backfill_cycles) <= 0 else "backfill_smoke",
                            },
                            default=to_jsonable,
                        ),
                        flush=True,
                    )

                stale_limit = float(args.stale_feed_seconds or 0.0)
                candle_stale_requires_pause = (
                    stale_limit > 0
                    and feed_age_seconds > stale_limit
                    and not no_new_nt_candle_expected
                )
                if market_resume_pending:
                    if candle_stale_requires_pause:
                        if not market_open_wait_logged:
                            event_uid = live_event_uid(args.run_id, "market_open_waiting", feed_outage_count)
                            details = {
                                "feed_outage_count": feed_outage_count,
                                "feed_age_seconds": round(feed_age_seconds, 3),
                                "stale_feed_seconds": stale_limit,
                                "latest_close_ts_utc": latest_close_ts,
                                "read": "The session is open again, but a fresh closed candle has not arrived yet. The monitor is waiting instead of calling this a feed outage.",
                            }
                            insert_event(
                                conn,
                                run_id=args.run_id,
                                event_uid=event_uid,
                                event_type="market_open_waiting",
                                instrument=instrument,
                                root_symbol=args.root,
                                model_symbol=nt_instrument_to_model_symbol(instrument, args.root),
                                timeframe=args.timeframe,
                                candle_time=raw["candle_time"].iloc[-1],
                                ts_utc=latest_ts,
                                direction=None,
                                status="waiting",
                                details=details,
                            )
                            insert_feed_health_event(
                                conn,
                                run_id=args.run_id,
                                event_uid=event_uid,
                                event_type="market_open_waiting",
                                instrument=instrument,
                                root_symbol=args.root,
                                timeframe=args.timeframe,
                                status="waiting",
                                heartbeat=heartbeat,
                                heartbeat_age_seconds=heartbeat_age_seconds,
                                latest_candle_ts_utc=latest_ts,
                                latest_candle_close_ts_utc=latest_close_ts,
                                details=details,
                            )
                            print(
                                json.dumps(
                                    {
                                        "event": "market_open_waiting",
                                        "instrument": instrument,
                                        "feed_age_seconds": round(feed_age_seconds, 1),
                                        "latest_ts_utc": to_jsonable(latest_ts),
                                    },
                                    default=to_jsonable,
                                ),
                                flush=True,
                            )
                            market_open_wait_logged = True
                        last_processed_ts = latest_ts
                        if int(args.max_loops) > 0 and loops >= int(args.max_loops):
                            break
                        time.sleep(max(1.0, float(args.poll_seconds)))
                        continue

                    cleared = reset_live_decision_state(pending, scheduled_entries, next_allowed_stage1)
                    event_uid = live_event_uid(args.run_id, "market_resumed", feed_outage_count)
                    details = {
                        **cleared,
                        "feed_outage_count": feed_outage_count,
                        "feed_age_seconds": round(feed_age_seconds, 3),
                        "latest_close_ts_utc": latest_close_ts,
                        "resume_rule": "skip_to_current_candle",
                        "read": "The market has a fresh closed candle again. The monitor reset state and will resume from the next clean candle.",
                    }
                    insert_event(
                        conn,
                        run_id=args.run_id,
                        event_uid=event_uid,
                        event_type="market_resumed",
                        instrument=instrument,
                        root_symbol=args.root,
                        model_symbol=nt_instrument_to_model_symbol(instrument, args.root),
                        timeframe=args.timeframe,
                        candle_time=raw["candle_time"].iloc[-1],
                        ts_utc=latest_ts,
                        direction=None,
                        status="resumed_next_candle",
                        details=details,
                    )
                    insert_feed_health_event(
                        conn,
                        run_id=args.run_id,
                        event_uid=event_uid,
                        event_type="market_resumed",
                        instrument=instrument,
                        root_symbol=args.root,
                        timeframe=args.timeframe,
                        status="resumed_next_candle",
                        restored_at=now_utc,
                        heartbeat=heartbeat,
                        heartbeat_age_seconds=heartbeat_age_seconds,
                        latest_candle_ts_utc=latest_ts,
                        latest_candle_close_ts_utc=latest_close_ts,
                        details=details,
                    )
                    print(
                        json.dumps(
                            {
                                "event": "market_resumed",
                                "instrument": instrument,
                                "feed_age_seconds": round(feed_age_seconds, 1),
                                "latest_ts_utc": to_jsonable(latest_ts),
                                **cleared,
                            },
                            default=to_jsonable,
                        ),
                        flush=True,
                    )
                    market_resume_pending = False
                    market_open_wait_logged = False
                    last_processed_ts = latest_ts
                    if int(args.max_loops) > 0 and loops >= int(args.max_loops):
                        break
                        time.sleep(max(1.0, float(args.poll_seconds)))
                        continue

                if candle_stale_requires_pause:
                    if not feed_paused:
                        feed_outage_count += 1
                        feed_paused = True
                        feed_paused_since = now_utc
                        feed_pause_reason = "stale_feed"
                        cleared = reset_live_decision_state(pending, scheduled_entries, next_allowed_stage1)
                        event_uid = live_event_uid(args.run_id, "feed_stale", feed_outage_count)
                        insert_event(
                            conn,
                            run_id=args.run_id,
                            event_uid=event_uid,
                            event_type="feed_stale",
                            instrument=instrument,
                            root_symbol=args.root,
                            model_symbol=nt_instrument_to_model_symbol(instrument, args.root),
                            timeframe=args.timeframe,
                            candle_time=raw["candle_time"].iloc[-1],
                            ts_utc=latest_ts,
                            direction=None,
                            status="paused",
                            details={
                                **cleared,
                                "feed_outage_count": feed_outage_count,
                                "feed_age_seconds": round(feed_age_seconds, 3),
                                "stale_feed_seconds": stale_limit,
                                "latest_close_ts_utc": latest_close_ts,
                                "loop_email_notifications": loop_email_notifications,
                                "read": "Feed is stale. New trend/trade decisions are paused and pending watches were cleared.",
                            },
                        )
                        insert_feed_health_event(
                            conn,
                            run_id=args.run_id,
                            event_uid=event_uid,
                            event_type="feed_stale",
                            instrument=instrument,
                            root_symbol=args.root,
                            timeframe=args.timeframe,
                            status="paused",
                            heartbeat=heartbeat,
                            heartbeat_age_seconds=heartbeat_age_seconds,
                            latest_candle_ts_utc=latest_ts,
                            latest_candle_close_ts_utc=latest_close_ts,
                            details={
                                **cleared,
                                "feed_outage_count": feed_outage_count,
                                "feed_age_seconds": round(feed_age_seconds, 3),
                                "stale_feed_seconds": stale_limit,
                                "latest_close_ts_utc": latest_close_ts,
                                "loop_email_notifications": loop_email_notifications,
                                "read": "Closed candles stopped advancing. New trend/trade decisions are paused.",
                            },
                        )
                        current_close_slot = latest_expected_close_slot(now_utc, expected_delta)
                        record_missing_slot_range(
                            conn,
                            args=args,
                            instrument=instrument,
                            start_close_ts=latest_close_ts + expected_delta,
                            end_close_ts=current_close_slot,
                            expected_delta=expected_delta,
                            nt_connected=True,
                            blocked_reason="connected_no_candle",
                            details={
                                "event_uid": event_uid,
                                "feed_age_seconds": round(feed_age_seconds, 3),
                                "stale_feed_seconds": stale_limit,
                                "latest_close_ts_utc": latest_close_ts,
                                "read": "NT bridge was connected, but closed candles stopped advancing.",
                            },
                        )
                        print(
                            json.dumps(
                                {
                                    "event": "feed_stale",
                                    "instrument": instrument,
                                    "feed_outage_count": feed_outage_count,
                                    "feed_age_seconds": round(feed_age_seconds, 1),
                                    "latest_ts_utc": to_jsonable(latest_ts),
                                    **cleared,
                                },
                                default=to_jsonable,
                            ),
                            flush=True,
                        )
                    last_processed_ts = latest_ts
                    if int(args.max_loops) > 0 and loops >= int(args.max_loops):
                        break
                    time.sleep(max(1.0, float(args.poll_seconds)))
                    continue

                if feed_paused:
                    duration_seconds = (
                        (now_utc - feed_paused_since).total_seconds()
                        if feed_paused_since is not None
                        else None
                    )
                    cleared = reset_live_decision_state(pending, scheduled_entries, next_allowed_stage1)
                    event_uid = live_event_uid(args.run_id, "feed_restored", feed_outage_count)
                    insert_event(
                        conn,
                        run_id=args.run_id,
                        event_uid=event_uid,
                        event_type="feed_restored",
                        instrument=instrument,
                        root_symbol=args.root,
                        model_symbol=nt_instrument_to_model_symbol(instrument, args.root),
                        timeframe=args.timeframe,
                        candle_time=raw["candle_time"].iloc[-1],
                        ts_utc=latest_ts,
                        direction=None,
                        status="resumed_next_candle",
                        details={
                            **cleared,
                            "feed_outage_count": feed_outage_count,
                            "pause_reason": feed_pause_reason,
                            "duration_seconds": None if duration_seconds is None else round(duration_seconds, 3),
                            "feed_age_seconds": round(feed_age_seconds, 3),
                            "latest_close_ts_utc": latest_close_ts,
                            "read": "Feed is fresh again. The monitor reset its live state and will resume on the next clean candle.",
                        },
                    )
                    insert_feed_health_event(
                        conn,
                        run_id=args.run_id,
                        event_uid=event_uid,
                        event_type="feed_restored",
                        instrument=instrument,
                        root_symbol=args.root,
                        timeframe=args.timeframe,
                        status="resumed_next_candle",
                        restored_at=now_utc,
                        duration_seconds=None if duration_seconds is None else round(duration_seconds, 3),
                        heartbeat=heartbeat,
                        heartbeat_age_seconds=heartbeat_age_seconds,
                        latest_candle_ts_utc=latest_ts,
                        latest_candle_close_ts_utc=latest_close_ts,
                        details={
                            **cleared,
                            "feed_outage_count": feed_outage_count,
                            "pause_reason": feed_pause_reason,
                            "duration_seconds": None if duration_seconds is None else round(duration_seconds, 3),
                            "feed_age_seconds": round(feed_age_seconds, 3),
                            "latest_close_ts_utc": latest_close_ts,
                            "resume_rule": "skip_to_current_candle",
                            "read": "Feed is healthy again. The monitor skipped stale backlog, reset state, and will resume from the next clean candle.",
                        },
                    )
                    print(
                        json.dumps(
                            {
                                "event": "feed_restored",
                                "instrument": instrument,
                                "feed_outage_count": feed_outage_count,
                                "duration_seconds": None if duration_seconds is None else round(duration_seconds, 1),
                                "resume_rule": "next_clean_candle",
                                **cleared,
                            },
                            default=to_jsonable,
                        ),
                        flush=True,
                    )
                    feed_paused = False
                    feed_paused_since = None
                    feed_pause_reason = ""
                    last_processed_ts = latest_ts
                    now = time.time()
                    if now - last_heartbeat < float(args.heartbeat_minutes) * 60.0:
                        if int(args.max_loops) > 0 and loops >= int(args.max_loops):
                            break
                        time.sleep(max(1.0, float(args.poll_seconds)))
                        continue

                all_new_indices = [idx for idx, ts in enumerate(times) if pd.Timestamp(ts) > last_processed_ts]
                new_indices = all_new_indices
                max_cycles = int(args.max_new_cycles_per_loop)
                if max_cycles > 0 and len(all_new_indices) > max_cycles:
                    feed_outage_count += 1
                    cleared = reset_live_decision_state(pending, scheduled_entries, next_allowed_stage1)
                    skipped = len(all_new_indices)
                    last_processed_ts = latest_ts
                    event_uid = live_event_uid(args.run_id, "feed_backlog_reset", feed_outage_count)
                    insert_event(
                        conn,
                        run_id=args.run_id,
                        event_uid=event_uid,
                        event_type="feed_backlog_reset",
                        instrument=instrument,
                        root_symbol=args.root,
                        model_symbol=nt_instrument_to_model_symbol(instrument, args.root),
                        timeframe=args.timeframe,
                        candle_time=raw["candle_time"].iloc[-1],
                        ts_utc=latest_ts,
                        direction=None,
                        status="state_reset",
                        details={
                            **cleared,
                            "feed_outage_count": feed_outage_count,
                            "skipped_new_candles": skipped,
                            "max_new_cycles_per_loop": max_cycles,
                            "read": "Too many unprocessed candles arrived at once. Old decisions were skipped so no stale trade can be queued.",
                        },
                    )
                    insert_feed_health_event(
                        conn,
                        run_id=args.run_id,
                        event_uid=event_uid,
                        event_type="feed_backlog_reset",
                        instrument=instrument,
                        root_symbol=args.root,
                        timeframe=args.timeframe,
                        status="state_reset",
                        heartbeat=heartbeat,
                        heartbeat_age_seconds=heartbeat_age_seconds,
                        latest_candle_ts_utc=latest_ts,
                        latest_candle_close_ts_utc=latest_close_ts,
                        skipped_candles=skipped,
                        details={
                            **cleared,
                            "feed_outage_count": feed_outage_count,
                            "skipped_new_candles": skipped,
                            "max_new_cycles_per_loop": max_cycles,
                            "resume_rule": "skip_to_current_candle",
                            "read": "Too many candles arrived at once. The monitor skipped stale catch-up work and reset state.",
                        },
                    )
                    for skipped_idx in all_new_indices:
                        mark_scanner_slot_status(
                            conn,
                            args=args,
                            instrument=instrument,
                            ts_utc=times.iloc[skipped_idx],
                            expected_delta=expected_delta,
                            scanner_status="skipped_backlog",
                            trade_allowed=False,
                            blocked_reason="too_many_recovery_candles",
                            details={
                                "event_uid": event_uid,
                                "max_new_cycles_per_loop": max_cycles,
                                "skipped_new_candles": skipped,
                                "read": "This candle arrived in a backlog batch and was skipped so no stale live trade could be queued.",
                            },
                        )
                    print(
                        json.dumps(
                            {
                                "event": "feed_backlog_reset",
                                "instrument": instrument,
                                "feed_outage_count": feed_outage_count,
                                "skipped_new_candles": skipped,
                                **cleared,
                            },
                            default=to_jsonable,
                        ),
                        flush=True,
                    )
                    new_indices = []

                # Sparse instruments like HO may not print a candle for every 2m slot.
                # A gap between two stored NT candles is only normal no-trade time; the
                # real missing-candle guard is the preflight check above, where NT's
                # heartbeat has a newer closed candle than the DB.

                for idx in new_indices:
                    started = time.perf_counter()
                    scanner_candle = candle_audit_snapshot(raw, int(idx))
                    lock_scanner_candle_row(conn, args=args, candle_snapshot=scanner_candle)
                    cycle = process_cycle(
                        conn,
                        args=args,
                        model_args=model_args,
                        bundle=bundle,
                        instrument=instrument,
                        candles=raw,
                        enriched=enriched,
                        current_idx=int(idx),
                        pending=pending,
                        scheduled_entries=scheduled_entries,
                        next_allowed_stage1=next_allowed_stage1,
                        email_config=email_config,
                        email_enabled=email_enabled,
                        email_started_at=email_started_at,
                    )
                    elapsed_ms = (time.perf_counter() - started) * 1000.0
                    cycles_seen += 1
                    last_processed_ts = pd.Timestamp(times.iloc[idx])
                    current_close_ts = pd.Timestamp(times.iloc[idx]) + expected_delta
                    is_latest_tradable_slot = current_close_ts >= latest_close_ts - pd.Timedelta(seconds=1)
                    scanner_status = "scanned_live" if is_latest_tradable_slot else "scanned_recovery"
                    mark_scanner_slot_status(
                        conn,
                        args=args,
                        instrument=instrument,
                        ts_utc=times.iloc[idx],
                        expected_delta=expected_delta,
                        scanner_status=scanner_status,
                        trade_allowed=bool(is_latest_tradable_slot),
                        blocked_reason=None if is_latest_tradable_slot else "late_candle_not_latest",
                        details={
                            "cycle_ts_utc": to_jsonable(cycle["ts_utc"]),
                            "latest_close_ts_utc": to_jsonable(latest_close_ts),
                            "decision_status": cycle.get("decision_status"),
                            "stage1_rows": cycle.get("stage1_rows"),
                            "level2_picks": cycle.get("level2_picks"),
                            "stage2_rows": cycle.get("stage2_rows"),
                            "trend_confirms": cycle.get("trend_confirms"),
                            "order_signals": cycle.get("order_signals"),
                            "order_rejects": cycle.get("order_rejects"),
                            "read": "Closed candle scored by the live monitor. Recovery candles are logged but are not the latest tradable slot.",
                        },
                    )
                    insert_event(
                        conn,
                        run_id=args.run_id,
                        event_uid=f"{args.run_id}|cycle_scored|{pd.Timestamp(cycle['ts_utc']).isoformat()}",
                        event_type="cycle_scored",
                        instrument=instrument,
                        root_symbol=args.root,
                        model_symbol=nt_instrument_to_model_symbol(instrument, args.root),
                        timeframe=args.timeframe,
                        candle_time=cycle["candle_time"],
                        ts_utc=cycle["ts_utc"],
                        direction=None,
                        status="scored",
                        details={
                            "elapsed_ms": round(elapsed_ms, 3),
                            "stage1_rows": cycle["stage1_rows"],
                            "level2_picks": cycle["level2_picks"],
                            "stage2_rows": cycle["stage2_rows"],
                            "trend_confirms": cycle["trend_confirms"],
                            "order_signals": cycle["order_signals"],
                            "order_rejects": cycle["order_rejects"],
                            "email_notifications": cycle.get("email_notifications", 0),
                            "level2_threshold": cycle.get("level2_threshold"),
                            "level2_best_score": cycle.get("level2_best_score"),
                            "level2_best_direction": cycle.get("level2_best_direction"),
                            "level2_long_score": cycle.get("level2_long_score"),
                            "level2_short_score": cycle.get("level2_short_score"),
                            "pending": len(pending),
                            "scheduled_entries": len(scheduled_entries),
                            "scanner_candle": scanner_candle,
                            "read": "Closed candle scored by the live trend detector.",
                        },
                    )
                    route_uid = f"{args.run_id}|{instrument}|{args.timeframe}|{pd.Timestamp(cycle['ts_utc']).isoformat()}"
                    insert_route_row(
                        conn,
                        args=args,
                        route_uid=route_uid,
                        instrument=instrument,
                        model_symbol=nt_instrument_to_model_symbol(instrument, args.root),
                        candle_snapshot=scanner_candle,
                        cycle=cycle,
                        pending_after=len(pending),
                        scheduled_entries_after=len(scheduled_entries),
                        details={
                            "elapsed_ms": round(elapsed_ms, 3),
                            "stage1_rows": cycle["stage1_rows"],
                            "stage1_status": cycle.get("stage1_status"),
                            "level2_picks": cycle["level2_picks"],
                            "stage2_rows": cycle["stage2_rows"],
                            "stage2_status": cycle.get("stage2_status"),
                            "trend_confirms": cycle["trend_confirms"],
                            "order_signals": cycle["order_signals"],
                            "order_rejects": cycle["order_rejects"],
                            "email_notifications": cycle.get("email_notifications", 0),
                            "scanner_candle": scanner_candle,
                            "read": "One-row route trace for this closed candle. Replay should match this when using the same candle revision.",
                        },
                    )
                    print(
                        json.dumps(
                            {
                                "event": "cycle_scored",
                                "instrument": instrument,
                                "candle_time": to_jsonable(cycle["candle_time"]),
                                "elapsed_ms": round(elapsed_ms, 3),
                                "stage1_rows": cycle["stage1_rows"],
                                "level2_picks": cycle["level2_picks"],
                                "stage2_rows": cycle["stage2_rows"],
                                "trend_confirms": cycle["trend_confirms"],
                                "order_signals": cycle["order_signals"],
                                "order_rejects": cycle["order_rejects"],
                                "email_notifications": cycle.get("email_notifications", 0),
                                "level2_threshold": cycle.get("level2_threshold"),
                                "level2_best_score": cycle.get("level2_best_score"),
                                "level2_best_direction": cycle.get("level2_best_direction"),
                                "level2_long_score": cycle.get("level2_long_score"),
                                "level2_short_score": cycle.get("level2_short_score"),
                                "pending": len(pending),
                                "scheduled_entries": len(scheduled_entries),
                            },
                            default=to_jsonable,
                        ),
                        flush=True,
                    )

            now = time.time()
            if now - last_heartbeat >= float(args.heartbeat_minutes) * 60.0:
                insert_event(
                    conn,
                    run_id=args.run_id,
                    event_uid=f"{args.run_id}|heartbeat|{int(now // (float(args.heartbeat_minutes) * 60.0))}",
                    event_type="heartbeat",
                    instrument=instrument,
                    root_symbol=args.root,
                    model_symbol=nt_instrument_to_model_symbol(instrument, args.root),
                    timeframe=args.timeframe,
                    candle_time=None,
                    ts_utc=last_processed_ts,
                    direction=None,
                    status="running",
                    details={
                        "cycles_seen": cycles_seen,
                        "pending_events": len(pending),
                        "scheduled_entries": len(scheduled_entries),
                        "feed_outage_count": feed_outage_count,
                        "feed_paused": bool(feed_paused),
                        "feed_pause_reason": feed_pause_reason or None,
                        "queue_orders": bool(args.queue_orders),
                        "read": "Monitor is running and waiting for Stage 2 trend confirmations.",
                    },
                )
                print(
                    json.dumps(
                        {
                            "event": "heartbeat",
                            "instrument": instrument,
                            "last_processed_ts": to_jsonable(last_processed_ts),
                            "cycles_seen": cycles_seen,
                            "pending": len(pending),
                            "scheduled_entries": len(scheduled_entries),
                            "feed_outage_count": feed_outage_count,
                            "feed_paused": bool(feed_paused),
                            "queue_orders": bool(args.queue_orders),
                        },
                        default=to_jsonable,
                    ),
                    flush=True,
                )
                last_heartbeat = now

            if int(args.max_loops) > 0 and loops >= int(args.max_loops):
                break
            time.sleep(max(1.0, float(args.poll_seconds)))
    finally:
        conn.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
