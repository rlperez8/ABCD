#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import re
import time
from dataclasses import dataclass
from datetime import datetime, timedelta
from pathlib import Path
from typing import Any
from urllib.parse import urlparse
from zoneinfo import ZoneInfo

import pymysql


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


@dataclass(frozen=True)
class PromoteRow:
    root_symbol: str
    symbol: str
    ts_utc: datetime
    open: float
    high: float
    low: float
    close: float
    volume: int
    source_count: int
    source_first_ts: datetime
    source_last_ts: datetime


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
        description="Promote closed NinjaTrader candle bridge rows into futures_contract_*_candles."
    )
    parser.add_argument("--root", default="HO")
    parser.add_argument("--timeframe", default="2m")
    parser.add_argument("--instrument", default="", help="Optional exact NinjaTrader instrument filter.")
    parser.add_argument("--local-timezone", default="America/Chicago")
    parser.add_argument(
        "--timestamp-mode",
        choices=["close", "open"],
        default="close",
        help="NinjaTrader Time[0] is close time for this bridge; target tables use open/bucket time.",
    )
    parser.add_argument("--start-ts", default="", help="Optional inclusive UTC bucket timestamp.")
    parser.add_argument("--end-ts", default="", help="Optional exclusive UTC bucket timestamp.")
    parser.add_argument(
        "--include-overlap",
        action="store_true",
        help="Allow rows at/before the current target max timestamp. Default only appends after max.",
    )
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--watch", action="store_true", help="Keep polling and promoting new candles.")
    parser.add_argument("--poll-seconds", type=float, default=10.0)
    parser.add_argument("--max-loops", type=int, default=0, help="Stop after N watch loops. Zero runs forever.")
    return parser.parse_args()


def timeframe_minutes(timeframe: str) -> int:
    value = timeframe.strip().lower().replace(" ", "")
    match = re.fullmatch(r"(\d+)m", value)
    if not match:
        raise ValueError(f"Only minute timeframes are supported here; got {timeframe!r}")
    minutes = int(match.group(1))
    if minutes <= 0:
        raise ValueError("Timeframe minutes must be positive")
    return minutes


def target_table(timeframe: str) -> str:
    minutes = timeframe_minutes(timeframe)
    return f"futures_contract_{minutes}m_candles"


def parse_optional_ts(value: str) -> datetime | None:
    value = value.strip()
    if not value:
        return None
    return datetime.fromisoformat(value.replace("T", " "))


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


def local_to_utc_naive(value: datetime, timezone_name: str) -> datetime:
    zone = ZoneInfo(timezone_name)
    return value.replace(tzinfo=zone).astimezone(ZoneInfo("UTC")).replace(tzinfo=None)


def max_target_ts(connection: pymysql.Connection, table: str, root: str) -> datetime | None:
    with connection.cursor() as cursor:
        cursor.execute(
            f"SELECT MAX(ts_utc) AS max_ts FROM {table} WHERE root_symbol = %s",
            (root,),
        )
        row = cursor.fetchone()
    return row["max_ts"] if row else None


def load_nt_rows(
    connection: pymysql.Connection,
    *,
    root: str,
    timeframe: str,
    instrument: str,
) -> list[dict[str, Any]]:
    where = [
        "root_symbol = %s",
        "timeframe = %s",
        "candle_time IS NOT NULL",
        "open IS NOT NULL",
        "high IS NOT NULL",
        "low IS NOT NULL",
        "close IS NOT NULL",
    ]
    params: list[Any] = [root, timeframe]
    if instrument:
        where.append("instrument = %s")
        params.append(instrument)

    with connection.cursor() as cursor:
        cursor.execute(
            f"""
            SELECT id, instrument, root_symbol, timeframe, candle_time,
                   open, high, low, close, volume, received_at
            FROM ninjatrader_live_candles
            WHERE {' AND '.join(where)}
            ORDER BY candle_time ASC, received_at ASC, id ASC
            """,
            params,
        )
        return list(cursor.fetchall())


def prepare_rows(
    raw_rows: list[dict[str, Any]],
    *,
    root: str,
    minutes: int,
    timezone_name: str,
    timestamp_mode: str,
    lower_bound: datetime | None,
    upper_bound: datetime | None,
) -> list[PromoteRow]:
    by_key: dict[tuple[str, datetime], PromoteRow] = {}
    close_offset = timedelta(minutes=minutes) if timestamp_mode == "close" else timedelta()
    source_last_offset = timedelta(minutes=max(minutes - 1, 0))

    for raw in raw_rows:
        candle_time = raw["candle_time"]
        if not isinstance(candle_time, datetime):
            continue

        bucket_ts = local_to_utc_naive(candle_time, timezone_name) - close_offset
        if lower_bound is not None and bucket_ts < lower_bound:
            continue
        if upper_bound is not None and bucket_ts >= upper_bound:
            continue

        symbol = nt_instrument_to_model_symbol(str(raw.get("instrument") or ""), root)
        row = PromoteRow(
            root_symbol=root,
            symbol=symbol,
            ts_utc=bucket_ts,
            open=float(raw["open"]),
            high=float(raw["high"]),
            low=float(raw["low"]),
            close=float(raw["close"]),
            volume=int(round(float(raw.get("volume") or 0.0))),
            source_count=minutes,
            source_first_ts=bucket_ts,
            source_last_ts=bucket_ts + source_last_offset,
        )
        by_key[(row.symbol, row.ts_utc)] = row

    return [by_key[key] for key in sorted(by_key, key=lambda item: (item[1], item[0]))]


def insert_rows(connection: pymysql.Connection, table: str, rows: list[PromoteRow]) -> int:
    if not rows:
        return 0

    payload = [
        (
            row.root_symbol,
            row.symbol,
            row.ts_utc,
            row.open,
            row.high,
            row.low,
            row.close,
            row.volume,
            row.source_count,
            row.source_first_ts,
            row.source_last_ts,
        )
        for row in rows
    ]
    with connection.cursor() as cursor:
        cursor.executemany(
            f"""
            INSERT INTO {table} (
                root_symbol, symbol, ts_utc, open, high, low, close,
                volume, source_count, source_first_ts, source_last_ts
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            ON DUPLICATE KEY UPDATE
                root_symbol = VALUES(root_symbol),
                open = VALUES(open),
                high = VALUES(high),
                low = VALUES(low),
                close = VALUES(close),
                volume = VALUES(volume),
                source_count = VALUES(source_count),
                source_first_ts = VALUES(source_first_ts),
                source_last_ts = VALUES(source_last_ts),
                updated_at = CURRENT_TIMESTAMP
            """,
            payload,
        )
        affected = cursor.rowcount
    connection.commit()
    return affected


def promote_once(
    connection: pymysql.Connection,
    *,
    root: str,
    timeframe: str,
    minutes: int,
    table: str,
    args: argparse.Namespace,
) -> int:
    connection.ping(reconnect=True)
    existing_max = max_target_ts(connection, table, root)
    start_ts = parse_optional_ts(args.start_ts)
    end_ts = parse_optional_ts(args.end_ts)
    lower_bound = start_ts
    if not args.include_overlap and lower_bound is None:
        lower_bound = existing_max + timedelta(microseconds=1) if existing_max is not None else None

    raw_rows = load_nt_rows(
        connection,
        root=root,
        timeframe=timeframe,
        instrument=args.instrument.strip(),
    )
    rows = prepare_rows(
        raw_rows,
        root=root,
        minutes=minutes,
        timezone_name=args.local_timezone,
        timestamp_mode=args.timestamp_mode,
        lower_bound=lower_bound,
        upper_bound=end_ts,
    )

    now = datetime.now().replace(microsecond=0)
    print(
        f"[{now}] Promoting NT {root} {timeframe} into {table}: "
        f"raw={len(raw_rows):,} prepared={len(rows):,} existing_max={existing_max} "
        f"lower_bound={lower_bound} end_ts={end_ts} dry_run={args.dry_run}",
        flush=True,
    )
    if rows:
        print(f"first={rows[0].symbol} {rows[0].ts_utc} close={rows[0].close}", flush=True)
        print(f"last={rows[-1].symbol} {rows[-1].ts_utc} close={rows[-1].close}", flush=True)

    if args.dry_run:
        return len(rows)

    affected = insert_rows(connection, table, rows)
    if rows or affected:
        print(f"Inserted/updated affected rows: {affected:,}", flush=True)
    return len(rows)


def main() -> int:
    args = parse_args()
    repo_abcd_dir = Path(__file__).resolve().parents[1]
    load_dotenv(repo_abcd_dir / ".env")

    root = args.root.upper().strip()
    timeframe = args.timeframe.lower().strip()
    minutes = timeframe_minutes(timeframe)
    table = target_table(timeframe)

    database_url = os.getenv("ABCD_DATABASE_URL") or os.getenv("DATABASE_URL")
    if not database_url:
        raise RuntimeError("Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL")

    connection = pymysql.connect(**parse_database_url(database_url))
    try:
        loop_index = 0
        while True:
            loop_index += 1
            promote_once(
                connection,
                root=root,
                timeframe=timeframe,
                minutes=minutes,
                table=table,
                args=args,
            )
            if not args.watch:
                return 0
            if args.max_loops > 0 and loop_index >= args.max_loops:
                return 0
            time.sleep(max(1.0, args.poll_seconds))
    finally:
        connection.close()


if __name__ == "__main__":
    raise SystemExit(main())
