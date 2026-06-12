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
                   is_realtime, received_at
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


def utc_now_naive() -> pd.Timestamp:
    return pd.Timestamp.now(tz="UTC").tz_localize(None)


def timeframe_delta(timeframe: str) -> pd.Timedelta:
    return pd.to_timedelta(int(scanner.table_for_timeframe(timeframe)[1]), unit="m")


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
    next_allowed_stage1: dict[tuple[str, str], int],
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
    directions = live_runner.direction_for_root(root, set(bundle["rules"]["root_directions"]))

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

    if base_rows:
        scored = live_runner.score_level1_and_level2(base_rows, bundle["models"], bundle["l2_args"], args.timeframe)
        l2_threshold = float(bundle["l2_meta"]["selected_threshold"])
        cooldown_bars = int(bundle["l2_meta"]["event_rules"]["cooldown_bars"])
        for row in scored.to_dict("records"):
            score = wave.finite(row.get("level2_score"), 0.0) or 0.0
            if score < l2_threshold:
                continue
            key = (str(row["symbol"]), str(row["direction"]))
            row_signal_idx = int(row["signal_idx"])
            if row_signal_idx < next_allowed_stage1.get(key, -1):
                continue
            next_allowed_stage1[key] = row_signal_idx + cooldown_bars + 1
            row["stage1_pick"] = 1
            row["stage1_score"] = score
            pending[str(row["candidate_uid"])] = live_runner.PendingEvent(event=row, signal_idx=row_signal_idx)
            cycle["level2_picks"] += 1
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
                    "candidate_uid": row.get("candidate_uid"),
                },
            )

    max_confirm_bars = int(bundle["stage2_meta"]["max_confirm_bars"])
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
        hits = pending_rows[
            pd.to_numeric(pending_rows["stage2_score"], errors="coerce").fillna(0.0) >= stage2_threshold
        ].copy()
        for row in hits.sort_values(["candidate_uid", "confirm_offset_bars"]).drop_duplicates("candidate_uid").to_dict("records"):
            uid = str(row["candidate_uid"])
            confirmed_uids.add(uid)
            stage2_score = wave.finite(row.get("stage2_score"), 0.0) or 0.0
            direction = str(row["direction"])
            status = "confirmed" if stage2_score >= min_stage2_score else "rejected_min_stage2_score"
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
                        enriched=enriched,
                        row=row,
                        stage2_score=float(stage2_score),
                    )
                    if payload is None:
                        cycle["order_rejects"] += 1
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

            raw = fetch_live_candles(conn, args, instrument)
            if raw.empty:
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
                times = pd.to_datetime(enriched["ts_utc"], errors="coerce")
                latest_ts = pd.Timestamp(times.max())
                latest_close_ts = latest_ts + expected_delta
                now_utc = utc_now_naive()
                feed_age_seconds = max(0.0, (now_utc - latest_close_ts).total_seconds())
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
                if stale_limit > 0 and feed_age_seconds > stale_limit:
                    if not feed_paused:
                        feed_outage_count += 1
                        feed_paused = True
                        feed_paused_since = now_utc
                        feed_pause_reason = "stale_feed"
                        cleared = reset_live_decision_state(pending, scheduled_entries, next_allowed_stage1)
                        insert_event(
                            conn,
                            run_id=args.run_id,
                            event_uid=f"{args.run_id}|feed_stale|{feed_outage_count}",
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
                    insert_event(
                        conn,
                        run_id=args.run_id,
                        event_uid=f"{args.run_id}|feed_restored|{feed_outage_count}",
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
                    insert_event(
                        conn,
                        run_id=args.run_id,
                        event_uid=f"{args.run_id}|feed_backlog_reset|{feed_outage_count}",
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

                if new_indices:
                    first_new_ts = pd.Timestamp(times.iloc[new_indices[0]])
                    gap_seconds = (first_new_ts - last_processed_ts).total_seconds()
                    expected_seconds = expected_delta.total_seconds()
                    if expected_seconds > 0 and gap_seconds > expected_seconds * float(args.gap_tolerance_bars):
                        feed_outage_count += 1
                        previous_processed_ts = last_processed_ts
                        missed_bars = max(0, int(round(gap_seconds / expected_seconds)) - 1)
                        cleared = reset_live_decision_state(pending, scheduled_entries, next_allowed_stage1)
                        last_processed_ts = latest_ts
                        insert_event(
                            conn,
                            run_id=args.run_id,
                            event_uid=f"{args.run_id}|feed_gap_detected|{feed_outage_count}",
                            event_type="feed_gap_detected",
                            instrument=instrument,
                            root_symbol=args.root,
                            model_symbol=nt_instrument_to_model_symbol(instrument, args.root),
                            timeframe=args.timeframe,
                            candle_time=raw["candle_time"].iloc[-1],
                            ts_utc=first_new_ts,
                            direction=None,
                            status="state_reset",
                            details={
                                **cleared,
                                "feed_outage_count": feed_outage_count,
                                "previous_processed_ts_utc": previous_processed_ts,
                                "first_new_ts_utc": first_new_ts,
                                "latest_ts_utc": latest_ts,
                                "gap_seconds": round(gap_seconds, 3),
                                "expected_seconds": round(expected_seconds, 3),
                                "estimated_missing_bars": missed_bars,
                                "read": "Candle sequence jumped. Pending watches were cleared and old decisions were skipped.",
                            },
                        )
                        print(
                            json.dumps(
                                {
                                    "event": "feed_gap_detected",
                                    "instrument": instrument,
                                    "feed_outage_count": feed_outage_count,
                                    "gap_seconds": round(gap_seconds, 1),
                                    "estimated_missing_bars": missed_bars,
                                    "resume_rule": "next_clean_candle",
                                    **cleared,
                                },
                                default=to_jsonable,
                            ),
                            flush=True,
                        )
                        new_indices = []

                for idx in new_indices:
                    started = time.perf_counter()
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
                            "pending": len(pending),
                            "scheduled_entries": len(scheduled_entries),
                            "read": "Closed candle scored by the live trend detector.",
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
