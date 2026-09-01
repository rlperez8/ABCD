#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import re
import time
from datetime import datetime, timedelta, timezone
from hashlib import sha1
from pathlib import Path
from typing import Any
from urllib.parse import urlparse
from zoneinfo import ZoneInfo

import pymysql


def load_dotenv(path: Path) -> None:
    if not path.exists():
        return
    for raw_line in path.read_text().splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        os.environ.setdefault(key.strip(), value.strip().strip('"').strip("'"))


def parse_database_url(database_url: str) -> dict[str, object]:
    parsed = urlparse(database_url)
    return {
        "host": parsed.hostname or "127.0.0.1",
        "port": parsed.port or 3306,
        "user": parsed.username or "",
        "password": parsed.password or "",
        "database": parsed.path.lstrip("/"),
        "charset": "utf8mb4",
        "autocommit": True,
        "cursorclass": pymysql.cursors.DictCursor,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Audit expected NinjaTrader candle close slots independently from the trend monitor."
    )
    parser.add_argument("--root", default="HO")
    parser.add_argument("--timeframe", default="2m")
    parser.add_argument("--instrument", default="", help="Optional exact NT instrument. Blank chooses latest feed contract.")
    parser.add_argument("--local-timezone", default="America/Chicago")
    parser.add_argument("--poll-seconds", type=float, default=5.0)
    parser.add_argument("--grace-seconds", type=float, default=8.0)
    parser.add_argument("--heartbeat-stale-seconds", type=float, default=20.0)
    parser.add_argument("--lookback-slots", type=int, default=180)
    parser.add_argument("--recent-candle-limit", type=int, default=500)
    parser.add_argument("--once", action="store_true")
    parser.add_argument("--max-loops", type=int, default=0)
    return parser.parse_args()


def timeframe_minutes(timeframe: str) -> int:
    value = str(timeframe or "").strip().lower().replace(" ", "")
    match = re.fullmatch(r"(\d+)m", value)
    if not match:
        raise ValueError(f"Only minute timeframes are supported here; got {timeframe!r}")
    minutes = int(match.group(1))
    if minutes <= 0:
        raise ValueError("Timeframe minutes must be positive")
    return minutes


def utc_now_naive() -> datetime:
    return datetime.now(timezone.utc).replace(tzinfo=None)


def as_utc_naive(value: Any) -> datetime | None:
    if value is None:
        return None
    if isinstance(value, datetime):
        if value.tzinfo is not None:
            return value.astimezone(timezone.utc).replace(tzinfo=None)
        return value
    text = str(value).strip()
    if not text:
        return None
    try:
        parsed = datetime.fromisoformat(text.replace("Z", "+00:00").replace(" ", "T"))
    except ValueError:
        return None
    if parsed.tzinfo is not None:
        return parsed.astimezone(timezone.utc).replace(tzinfo=None)
    return parsed


def local_to_utc_naive(value: datetime, timezone_name: str) -> datetime:
    if value.tzinfo is not None:
        return value.astimezone(timezone.utc).replace(tzinfo=None)
    zone = ZoneInfo(timezone_name)
    return value.replace(tzinfo=zone).astimezone(timezone.utc).replace(tzinfo=None)


def floor_to_timeframe(value: datetime, delta: timedelta) -> datetime:
    seconds = int(delta.total_seconds())
    epoch = int(value.replace(tzinfo=timezone.utc).timestamp())
    return datetime.fromtimestamp((epoch // seconds) * seconds, tz=timezone.utc).replace(tzinfo=None)


def is_market_session_open(close_time_utc: datetime, timezone_name: str) -> bool:
    local = close_time_utc.replace(tzinfo=timezone.utc).astimezone(ZoneInfo(timezone_name))
    weekday = local.weekday()  # Monday = 0, Sunday = 6
    minutes = local.hour * 60 + local.minute + local.second / 60.0
    if weekday == 5:
        return False
    if weekday == 6:
        return minutes >= 17 * 60
    if weekday in {0, 1, 2, 3}:
        return not (16 * 60 <= minutes < 17 * 60)
    if weekday == 4:
        return minutes < 16 * 60
    return False


def json_default(value: Any) -> Any:
    if isinstance(value, datetime):
        return value.isoformat(sep=" ")
    return str(value)


def ensure_candle_slot_audit_table(conn: pymysql.Connection) -> None:
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
                INDEX idx_nt_candle_slot_audit_root_slot (root_symbol, timeframe, slot_time),
                INDEX idx_nt_candle_slot_audit_instrument_slot (instrument, timeframe, slot_time),
                INDEX idx_nt_candle_slot_audit_arrival (arrival_status, slot_time),
                INDEX idx_nt_candle_slot_audit_scanner (scanner_run_id, scanner_status, slot_time),
                INDEX idx_nt_candle_slot_audit_updated (updated_at)
            )
            """
        )
    conn.commit()


def fetch_latest_heartbeat(
    conn: pymysql.Connection,
    *,
    root: str,
    timeframe: str,
    instrument: str | None,
) -> dict[str, Any] | None:
    params: list[Any] = [root, timeframe]
    instrument_filter = ""
    if instrument:
        instrument_filter = "AND instrument = %s"
        params.append(instrument)
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT id, instrument, root_symbol, exchange_name, timeframe, bars_period_type,
                   bars_period_value, heartbeat_time_utc, last_candle_time,
                   last_candle_time_utc, last_snapshot_time_utc, last_price,
                   connection_status, received_at, updated_at
            FROM ninjatrader_feed_heartbeats
            WHERE root_symbol = %s
              AND timeframe = %s
              {instrument_filter}
            ORDER BY received_at DESC, id DESC
            LIMIT 1
            """,
            params,
        )
        row = cur.fetchone()
    return dict(row) if row else None


def fetch_recent_live_candles(
    conn: pymysql.Connection,
    *,
    root: str,
    timeframe: str,
    instrument: str,
    limit: int,
) -> list[dict[str, Any]]:
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT *
            FROM (
                SELECT id, instrument, root_symbol, exchange_name, timeframe,
                       bars_period_type, bars_period_value, candle_time,
                       open, high, low, close, volume, tick_size, point_value,
                       is_realtime, received_at, updated_at
                FROM ninjatrader_live_candles
                WHERE root_symbol = %s
                  AND timeframe = %s
                  AND instrument = %s
                  AND candle_time IS NOT NULL
                ORDER BY candle_time DESC, id DESC
                LIMIT %s
            ) recent
            ORDER BY candle_time ASC, id ASC
            """,
            (root, timeframe, instrument, max(1, int(limit))),
        )
        return list(cur.fetchall())


def latest_audit_slot(
    conn: pymysql.Connection,
    *,
    root: str,
    timeframe: str,
    instrument: str,
) -> datetime | None:
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT MAX(slot_time) AS slot_time
            FROM ninjatrader_candle_slot_audit
            WHERE root_symbol = %s
              AND timeframe = %s
              AND instrument = %s
            """,
            (root, timeframe, instrument),
        )
        row = cur.fetchone()
    return as_utc_naive(row.get("slot_time")) if row else None


def slot_external_key(instrument: str, timeframe: str, slot_time: datetime) -> str:
    raw = f"ninjatrader|{instrument}|{timeframe}|{slot_time.isoformat()}"
    if len(raw) <= 240:
        return raw
    digest = sha1(raw.encode("utf-8")).hexdigest()[:16]
    return f"ninjatrader|{instrument[:120]}|{timeframe}|{digest}"


def upsert_slot(
    conn: pymysql.Connection,
    *,
    root: str,
    timeframe: str,
    bars_period_value: int,
    instrument: str,
    exchange_name: str | None,
    bars_period_type: str | None,
    slot_time: datetime,
    nt_connected: bool | None,
    candle_received: bool,
    candle_external_key: str | None,
    candle_received_at: datetime | None,
    arrival_status: str,
    blocked_reason: str | None,
    details: dict[str, Any] | None = None,
) -> None:
    payload = json.dumps(details or {}, default=json_default, sort_keys=True) if details else None
    with conn.cursor() as cur:
        cur.execute(
            """
            INSERT INTO ninjatrader_candle_slot_audit (
                external_key, source, instrument, root_symbol, exchange_name,
                timeframe, bars_period_type, bars_period_value, slot_time,
                expected_close_time, nt_connected, candle_received,
                candle_external_key, candle_received_at, arrival_status,
                scanner_status, trade_allowed, blocked_reason, details_json
            )
            VALUES (%s, 'ninjatrader_candle_feed', %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, 'not_scanned', 0, %s, %s)
            ON DUPLICATE KEY UPDATE
                source = VALUES(source),
                instrument = VALUES(instrument),
                root_symbol = VALUES(root_symbol),
                exchange_name = COALESCE(VALUES(exchange_name), exchange_name),
                timeframe = VALUES(timeframe),
                bars_period_type = COALESCE(VALUES(bars_period_type), bars_period_type),
                bars_period_value = VALUES(bars_period_value),
                expected_close_time = VALUES(expected_close_time),
                nt_connected = COALESCE(VALUES(nt_connected), nt_connected),
                candle_received = GREATEST(candle_received, VALUES(candle_received)),
                candle_external_key = COALESCE(VALUES(candle_external_key), candle_external_key),
                candle_received_at = COALESCE(VALUES(candle_received_at), candle_received_at),
                arrival_status = VALUES(arrival_status),
                blocked_reason = COALESCE(VALUES(blocked_reason), blocked_reason),
                details_json = COALESCE(VALUES(details_json), details_json),
                updated_at = CURRENT_TIMESTAMP(6)
            """,
            (
                slot_external_key(instrument, timeframe, slot_time),
                instrument,
                root,
                exchange_name,
                timeframe,
                bars_period_type,
                bars_period_value,
                slot_time,
                slot_time,
                None if nt_connected is None else int(bool(nt_connected)),
                int(bool(candle_received)),
                candle_external_key,
                candle_received_at,
                arrival_status,
                blocked_reason,
                payload,
            ),
        )


def heartbeat_age_seconds(heartbeat: dict[str, Any] | None, now_utc: datetime) -> float | None:
    if not heartbeat:
        return None
    heartbeat_time = as_utc_naive(heartbeat.get("heartbeat_time_utc") or heartbeat.get("received_at"))
    if heartbeat_time is None:
        return None
    return max(0.0, (now_utc - heartbeat_time).total_seconds())


def candle_close_slot(row: dict[str, Any], timezone_name: str) -> datetime | None:
    candle_time = row.get("candle_time")
    if not isinstance(candle_time, datetime):
        candle_time = as_utc_naive(candle_time)
    if candle_time is None:
        return None
    return local_to_utc_naive(candle_time, timezone_name)


def audit_once(conn: pymysql.Connection, args: argparse.Namespace) -> dict[str, Any]:
    try:
        conn.ping()
    except pymysql.MySQLError:
        conn.connect()
    ensure_candle_slot_audit_table(conn)

    root = str(args.root).upper().strip()
    timeframe = str(args.timeframe).lower().strip()
    minutes = timeframe_minutes(timeframe)
    delta = timedelta(minutes=minutes)
    grace = timedelta(seconds=max(0.0, float(args.grace_seconds)))
    now_utc = utc_now_naive()

    requested_instrument = str(args.instrument or "").upper().strip() or None
    heartbeat = fetch_latest_heartbeat(conn, root=root, timeframe=timeframe, instrument=requested_instrument)
    instrument = requested_instrument or str((heartbeat or {}).get("instrument") or "").upper().strip()
    if not instrument:
        return {"status": "no_instrument", "arrivals": 0, "missing": 0}

    candles = fetch_recent_live_candles(
        conn,
        root=root,
        timeframe=timeframe,
        instrument=instrument,
        limit=int(args.recent_candle_limit),
    )
    exchange_name = (heartbeat or {}).get("exchange_name") or (candles[-1].get("exchange_name") if candles else None)
    bars_period_type = (heartbeat or {}).get("bars_period_type") or (candles[-1].get("bars_period_type") if candles else None)

    heartbeat_age = heartbeat_age_seconds(heartbeat, now_utc)
    connected = bool(heartbeat is not None and heartbeat_age is not None and heartbeat_age <= float(args.heartbeat_stale_seconds))
    blocked_reason = "connected_no_candle" if connected else "no_connection_to_nt"

    arrived_slots: set[datetime] = set()
    arrival_count = 0
    for row in candles:
        slot_time = candle_close_slot(row, args.local_timezone)
        if slot_time is None:
            continue
        received_at = as_utc_naive(row.get("received_at") or row.get("updated_at"))
        arrival_status = "late" if received_at and received_at > slot_time + grace else "on_time"
        arrived_slots.add(slot_time)
        upsert_slot(
            conn,
            root=root,
            timeframe=timeframe,
            bars_period_value=minutes,
            instrument=instrument,
            exchange_name=row.get("exchange_name") or exchange_name,
            bars_period_type=row.get("bars_period_type") or bars_period_type,
            slot_time=slot_time,
            nt_connected=True,
            candle_received=True,
            candle_external_key=f"ninjatrader|{instrument}|{timeframe}|{row.get('candle_time')}",
            candle_received_at=received_at,
            arrival_status=arrival_status,
            blocked_reason=None,
            details={
                "audit_owner": "candle_feed",
                "candle_id": row.get("id"),
                "candle_time": row.get("candle_time"),
                "received_at": received_at,
                "arrival_rule": f"late if received after slot close + {int(grace.total_seconds())}s",
            },
        )
        arrival_count += 1

    latest_audit = latest_audit_slot(conn, root=root, timeframe=timeframe, instrument=instrument)
    first_recent_slot = min(arrived_slots) if arrived_slots else None
    lookback_start = floor_to_timeframe(now_utc - delta * max(1, int(args.lookback_slots)), delta)
    if latest_audit:
        start_slot = latest_audit + delta
    elif first_recent_slot is not None:
        start_slot = min(lookback_start, first_recent_slot)
    else:
        start_slot = lookback_start
    latest_auditable_slot = floor_to_timeframe(now_utc - grace, delta)

    missing_count = 0
    slot = start_slot
    while slot <= latest_auditable_slot:
        if is_market_session_open(slot, args.local_timezone) and slot not in arrived_slots:
            upsert_slot(
                conn,
                root=root,
                timeframe=timeframe,
                bars_period_value=minutes,
                instrument=instrument,
                exchange_name=exchange_name,
                bars_period_type=bars_period_type,
                slot_time=slot,
                nt_connected=connected,
                candle_received=False,
                candle_external_key=None,
                candle_received_at=None,
                arrival_status="missing",
                blocked_reason=blocked_reason,
                details={
                    "audit_owner": "candle_feed",
                    "heartbeat_age_seconds": heartbeat_age,
                    "heartbeat_received_at": (heartbeat or {}).get("received_at"),
                    "heartbeat_connection_status": (heartbeat or {}).get("connection_status"),
                    "rule": "expected open-session 2m close slot did not have a stored NT candle by the grace period",
                },
            )
            missing_count += 1
        slot += delta

    conn.commit()
    return {
        "status": "ok",
        "instrument": instrument,
        "connected": connected,
        "heartbeat_age_seconds": heartbeat_age,
        "arrivals": arrival_count,
        "missing_checked": missing_count,
        "latest_auditable_slot": latest_auditable_slot,
    }


def main() -> int:
    args = parse_args()
    repo_abcd_dir = Path(__file__).resolve().parents[1]
    load_dotenv(repo_abcd_dir / ".env")

    database_url = os.getenv("ABCD_DATABASE_URL") or os.getenv("DATABASE_URL")
    if not database_url:
        raise RuntimeError("Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL")

    conn = pymysql.connect(**parse_database_url(database_url))
    try:
        loop_index = 0
        while True:
            loop_index += 1
            result = audit_once(conn, args)
            print(
                f"[{datetime.now().replace(microsecond=0)}] candle slot audit "
                f"status={result.get('status')} instrument={result.get('instrument')} "
                f"connected={result.get('connected')} arrivals={result.get('arrivals')} "
                f"missing_checked={result.get('missing_checked')} "
                f"latest_slot={result.get('latest_auditable_slot')}",
                flush=True,
            )
            if args.once or (args.max_loops > 0 and loop_index >= args.max_loops):
                return 0
            time.sleep(max(1.0, float(args.poll_seconds)))
    finally:
        conn.close()


if __name__ == "__main__":
    raise SystemExit(main())
