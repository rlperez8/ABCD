from __future__ import annotations

import os
from dataclasses import dataclass
from datetime import date, datetime, timezone
from pathlib import Path
from typing import Iterable
from urllib.parse import urlparse

import databento as db
import pandas as pd
import pymysql


DATASET = os.getenv("ABCD_DATABENTO_DATASET", "GLBX.MDP3")
SCHEMA = "ohlcv-1m"
MONTH_CODES = "FGHJKMNQUVXZ"
DEFAULT_START = "2021-04-24"
DEFAULT_END = "2026-04-24"
INSERT_BATCH_SIZE = 5_000


@dataclass(frozen=True)
class RollWindow:
    root_symbol: str
    continuous_symbol: str
    instrument_id: int
    contract_symbol: str
    start: datetime
    end: datetime


def load_dotenv(path: Path) -> None:
    if not path.exists():
        return

    for raw_line in path.read_text().splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue

        key, value = line.split("=", 1)
        os.environ.setdefault(key.strip(), value.strip().strip('"').strip("'"))


def required_env(name: str) -> str:
    value = os.getenv(name, "").strip()
    if not value:
        raise RuntimeError(f"Missing required environment variable: {name}")
    return value


def parse_date_env(name: str, default: str) -> date:
    return date.fromisoformat(os.getenv(name, default).strip())


def parse_database_url(database_url: str) -> dict[str, object]:
    parsed = urlparse(database_url)
    return {
        "host": parsed.hostname or "127.0.0.1",
        "port": parsed.port or 3306,
        "user": parsed.username or "",
        "password": parsed.password or "",
        "database": parsed.path.lstrip("/"),
        "charset": "utf8mb4",
        "autocommit": False,
    }


def normalize_mapping_date(value: object) -> date:
    if isinstance(value, date):
        return value
    return date.fromisoformat(str(value))


def day_start(value: date) -> datetime:
    return datetime(value.year, value.month, value.day, tzinfo=timezone.utc)


def clamp_window_start(value: date, minimum: date) -> datetime:
    return day_start(max(value, minimum))


def clamp_window_end(value: date, maximum: date) -> datetime:
    return day_start(min(value, maximum))


def generate_contract_candidates(root: str, start: date, end: date) -> list[str]:
    years = range(start.year - 1, end.year + 2)
    symbols = [
        f"{root}{month_code}{str(year)[-1]}"
        for year in years
        for month_code in MONTH_CODES
    ]
    return sorted(set(symbols))


def resolve_contract_lookup(
    client: db.Historical,
    root: str,
    start: date,
    end: date,
) -> dict[tuple[int, date, date], str]:
    candidates = generate_contract_candidates(root, start, end)
    resolved = client.symbology.resolve(
        dataset=DATASET,
        symbols=candidates,
        stype_in="raw_symbol",
        stype_out="instrument_id",
        start_date=start.isoformat(),
        end_date=end.isoformat(),
    )

    lookup: dict[tuple[int, date, date], str] = {}
    for raw_symbol, mappings in resolved.get("result", {}).items():
        for mapping in mappings:
            instrument_id = int(mapping["s"])
            d0 = normalize_mapping_date(mapping["d0"])
            d1 = normalize_mapping_date(mapping["d1"])
            lookup[(instrument_id, d0, d1)] = raw_symbol

    return lookup


def windows_overlap(left_start: date, left_end: date, right_start: date, right_end: date) -> bool:
    return left_start < right_end and right_start < left_end


def find_contract_symbol(
    lookup: dict[tuple[int, date, date], str],
    instrument_id: int,
    start: date,
    end: date,
) -> str | None:
    for (candidate_id, d0, d1), raw_symbol in lookup.items():
        if candidate_id == instrument_id and windows_overlap(start, end, d0, d1):
            return raw_symbol
    return None


def resolve_roll_windows(
    client: db.Historical,
    root: str,
    start: date,
    end: date,
) -> list[RollWindow]:
    continuous_symbol = f"{root}.c.0"
    contract_lookup = resolve_contract_lookup(client, root, start, end)
    resolved = client.symbology.resolve(
        dataset=DATASET,
        symbols=[continuous_symbol],
        stype_in="continuous",
        stype_out="instrument_id",
        start_date=start.isoformat(),
        end_date=end.isoformat(),
    )

    windows: list[RollWindow] = []
    unresolved: list[tuple[int, date, date]] = []
    for mapping in resolved.get("result", {}).get(continuous_symbol, []):
        instrument_id = int(mapping["s"])
        raw_start = normalize_mapping_date(mapping["d0"])
        raw_end = normalize_mapping_date(mapping["d1"])
        window_start = clamp_window_start(raw_start, start)
        window_end = clamp_window_end(raw_end, end)
        if window_start >= window_end:
            continue

        contract_symbol = find_contract_symbol(
            contract_lookup,
            instrument_id,
            raw_start,
            raw_end,
        )
        if contract_symbol is None:
            unresolved.append((instrument_id, raw_start, raw_end))
            continue

        windows.append(
            RollWindow(
                root_symbol=root,
                continuous_symbol=continuous_symbol,
                instrument_id=instrument_id,
                contract_symbol=contract_symbol,
                start=window_start,
                end=window_end,
            )
        )

    if unresolved:
        preview = ", ".join(
            f"{instrument_id}:{d0}->{d1}" for instrument_id, d0, d1 in unresolved[:8]
        )
        raise RuntimeError(f"Could not map {len(unresolved)} roll windows for {root}: {preview}")

    return windows


def ensure_tables(connection: pymysql.Connection) -> None:
    with connection.cursor() as cursor:
        cursor.execute(
            """
            CREATE TABLE IF NOT EXISTS futures_contract_1m_candles (
                dataset VARCHAR(32) NOT NULL,
                root_symbol VARCHAR(32) NOT NULL,
                symbol VARCHAR(64) NOT NULL,
                ts_utc DATETIME(6) NOT NULL,
                open DOUBLE NOT NULL,
                high DOUBLE NOT NULL,
                low DOUBLE NOT NULL,
                close DOUBLE NOT NULL,
                volume BIGINT NOT NULL,
                synced_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                PRIMARY KEY (dataset, symbol, ts_utc),
                INDEX idx_futures_contract_1m_root_ts (root_symbol, ts_utc),
                INDEX idx_futures_contract_1m_symbol_ts (symbol, ts_utc)
            )
            """
        )
        cursor.execute(
            """
            CREATE TABLE IF NOT EXISTS futures_contract_rolls (
                dataset VARCHAR(32) NOT NULL,
                root_symbol VARCHAR(32) NOT NULL,
                continuous_symbol VARCHAR(64) NOT NULL,
                instrument_id BIGINT NOT NULL,
                contract_symbol VARCHAR(64) NOT NULL,
                roll_start_utc DATETIME(6) NOT NULL,
                roll_end_utc DATETIME(6) NOT NULL,
                synced_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                PRIMARY KEY (dataset, continuous_symbol, roll_start_utc),
                INDEX idx_futures_contract_rolls_root_start (root_symbol, roll_start_utc)
            )
            """
        )
    connection.commit()


def upsert_roll_window(connection: pymysql.Connection, window: RollWindow) -> None:
    with connection.cursor() as cursor:
        cursor.execute(
            """
            INSERT INTO futures_contract_rolls (
                dataset,
                root_symbol,
                continuous_symbol,
                instrument_id,
                contract_symbol,
                roll_start_utc,
                roll_end_utc
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s)
            ON DUPLICATE KEY UPDATE
                root_symbol = VALUES(root_symbol),
                instrument_id = VALUES(instrument_id),
                contract_symbol = VALUES(contract_symbol),
                roll_end_utc = VALUES(roll_end_utc),
                synced_at = CURRENT_TIMESTAMP
            """,
            (
                DATASET,
                window.root_symbol,
                window.continuous_symbol,
                window.instrument_id,
                window.contract_symbol,
                window.start.replace(tzinfo=None),
                window.end.replace(tzinfo=None),
            ),
        )
    connection.commit()


def dataframe_rows(df: pd.DataFrame, window: RollWindow) -> Iterable[tuple[object, ...]]:
    for row in df.itertuples():
        ts_utc = row.Index
        if hasattr(ts_utc, "to_pydatetime"):
            ts_utc = ts_utc.to_pydatetime()
        if ts_utc.tzinfo is not None:
            ts_utc = ts_utc.astimezone(timezone.utc).replace(tzinfo=None)

        yield (
            DATASET,
            window.root_symbol,
            window.contract_symbol,
            ts_utc,
            float(row.open),
            float(row.high),
            float(row.low),
            float(row.close),
            int(row.volume),
        )


def insert_candle_rows(connection: pymysql.Connection, rows: list[tuple[object, ...]]) -> None:
    if not rows:
        return

    with connection.cursor() as cursor:
        cursor.executemany(
            """
            INSERT INTO futures_contract_1m_candles (
                dataset,
                root_symbol,
                symbol,
                ts_utc,
                open,
                high,
                low,
                close,
                volume
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)
            ON DUPLICATE KEY UPDATE
                root_symbol = VALUES(root_symbol),
                open = VALUES(open),
                high = VALUES(high),
                low = VALUES(low),
                close = VALUES(close),
                volume = VALUES(volume),
                synced_at = CURRENT_TIMESTAMP
            """,
            rows,
        )
    connection.commit()


def load_window(
    client: db.Historical,
    connection: pymysql.Connection,
    window: RollWindow,
) -> int:
    store = client.timeseries.get_range(
        dataset=DATASET,
        schema=SCHEMA,
        symbols=[window.continuous_symbol],
        stype_in="continuous",
        start=window.start,
        end=window.end,
    )
    df = store.to_df(map_symbols=False)
    if df.empty:
        return 0

    inserted = 0
    batch: list[tuple[object, ...]] = []
    for item in dataframe_rows(df, window):
        batch.append(item)
        if len(batch) >= INSERT_BATCH_SIZE:
            insert_candle_rows(connection, batch)
            inserted += len(batch)
            batch.clear()

    if batch:
        insert_candle_rows(connection, batch)
        inserted += len(batch)

    return inserted


def main() -> None:
    repo_abcd_dir = Path(__file__).resolve().parents[1]
    load_dotenv(repo_abcd_dir / ".env")

    root = required_env("ABCD_FUTURES_ROOT").upper()
    api_key = required_env("DATABENTO_API_KEY")
    database_url = os.getenv("ABCD_DATABASE_URL") or os.getenv("DATABASE_URL")
    if not database_url:
        raise RuntimeError("Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL")

    start = parse_date_env("ABCD_LOAD_START", DEFAULT_START)
    end = parse_date_env("ABCD_LOAD_END", DEFAULT_END)
    dry_run = os.getenv("ABCD_DRY_RUN", "").strip().lower() in {"1", "true", "yes"}

    if start >= end:
        raise RuntimeError("ABCD_LOAD_START must be before ABCD_LOAD_END")

    print(f"Loading {root} from {start} to {end} using {DATASET} {SCHEMA}")
    client = db.Historical(api_key)
    windows = resolve_roll_windows(client, root, start, end)
    print(f"Resolved {len(windows)} roll windows")
    for window in windows:
        print(
            f"{window.contract_symbol}: {window.start:%Y-%m-%d %H:%M:%S} -> "
            f"{window.end:%Y-%m-%d %H:%M:%S} ({window.instrument_id})"
        )

    if dry_run:
        print("Dry run only. No candles were downloaded or written.")
        return

    connection = pymysql.connect(**parse_database_url(database_url))
    try:
        ensure_tables(connection)
        total_rows = 0
        for index, window in enumerate(windows, start=1):
            print(f"[{index}/{len(windows)}] {window.contract_symbol} downloading...")
            upsert_roll_window(connection, window)
            rows = load_window(client, connection, window)
            total_rows += rows
            print(f"[{index}/{len(windows)}] {window.contract_symbol} wrote {rows:,} rows")

        print(f"Finished {root}. Wrote {total_rows:,} rows.")
    finally:
        connection.close()


if __name__ == "__main__":
    main()
