#!/usr/bin/env python3
"""
Build pre-pattern candle context features for pattern setups.

The table is keyed by setup_id and only uses candles strictly before
d_confirm_date, so it can be safely joined into AI training/evaluation rows.
"""

from __future__ import annotations

import argparse
import math
import sys
from datetime import timedelta
from pathlib import Path

import numpy as np
import pandas as pd

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_build_search_catboost as base


DEFAULT_SOURCE_RUN_ID = "eetc-1779837772298-38740"
DEFAULT_RESULTS_TABLE = "entry_exit_template_results_eetc_1779837772298_38740"
WINDOWS = [5, 15, 30, 60, 120]
SHAPE_FEATURE_COLUMNS = [
    "pre_xa_points",
    "pre_ab_points",
    "pre_bc_points",
    "pre_cd_points",
    "pre_ab_to_xa",
    "pre_bc_to_ab",
    "pre_cd_to_xa",
    "pre_cd_to_bc",
    "pre_pattern_height_points",
    "pre_pattern_height_to_price",
    "pre_xa_minutes",
    "pre_ab_minutes",
    "pre_bc_minutes",
    "pre_cd_minutes",
    "pre_full_pattern_minutes",
    "pre_confirm_lag_minutes",
    "pre_xa_velocity",
    "pre_cd_velocity",
    "pre_cd_to_xa_velocity",
    "pre_d_range_to_xa",
    "pre_d_body_to_xa",
    "pre_d_close_location",
    "pre_d_upper_wick_ratio",
    "pre_d_lower_wick_ratio",
    "pre_reversal_signal_count",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-run-id", default=DEFAULT_SOURCE_RUN_ID)
    parser.add_argument("--results-table", default=DEFAULT_RESULTS_TABLE)
    parser.add_argument("--year", type=int, default=None)
    parser.add_argument("--start-year", type=int, default=None)
    parser.add_argument("--end-year", type=int, default=None)
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--chunk-size", type=int, default=1000)
    return parser.parse_args()


def ensure_table(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS pattern_pre_context_features (
                setup_id VARCHAR(64) PRIMARY KEY,
                pattern_id VARCHAR(64) NULL,
                pattern_group_id VARCHAR(128) NULL,
                symbol VARCHAR(32) NOT NULL,
                root_symbol VARCHAR(32) NULL,
                d_confirm_date DATETIME NOT NULL,
                pre_source_table VARCHAR(64) NOT NULL DEFAULT 'futures_contract_1m_candles',
                pre_max_window_minutes INT NOT NULL DEFAULT 120,
                pre_total_candles INT NOT NULL DEFAULT 0,

                pre_return_5m DOUBLE NULL,
                pre_range_5m DOUBLE NULL,
                pre_body_ratio_5m DOUBLE NULL,
                pre_directional_bias_5m DOUBLE NULL,
                pre_green_share_5m DOUBLE NULL,
                pre_red_share_5m DOUBLE NULL,
                pre_avg_range_5m DOUBLE NULL,
                pre_volatility_5m DOUBLE NULL,
                pre_candle_count_5m INT NOT NULL DEFAULT 0,

                pre_return_15m DOUBLE NULL,
                pre_range_15m DOUBLE NULL,
                pre_body_ratio_15m DOUBLE NULL,
                pre_directional_bias_15m DOUBLE NULL,
                pre_green_share_15m DOUBLE NULL,
                pre_red_share_15m DOUBLE NULL,
                pre_avg_range_15m DOUBLE NULL,
                pre_volatility_15m DOUBLE NULL,
                pre_candle_count_15m INT NOT NULL DEFAULT 0,

                pre_return_30m DOUBLE NULL,
                pre_range_30m DOUBLE NULL,
                pre_body_ratio_30m DOUBLE NULL,
                pre_directional_bias_30m DOUBLE NULL,
                pre_green_share_30m DOUBLE NULL,
                pre_red_share_30m DOUBLE NULL,
                pre_avg_range_30m DOUBLE NULL,
                pre_volatility_30m DOUBLE NULL,
                pre_candle_count_30m INT NOT NULL DEFAULT 0,

                pre_return_60m DOUBLE NULL,
                pre_range_60m DOUBLE NULL,
                pre_body_ratio_60m DOUBLE NULL,
                pre_directional_bias_60m DOUBLE NULL,
                pre_green_share_60m DOUBLE NULL,
                pre_red_share_60m DOUBLE NULL,
                pre_avg_range_60m DOUBLE NULL,
                pre_volatility_60m DOUBLE NULL,
                pre_candle_count_60m INT NOT NULL DEFAULT 0,

                pre_return_120m DOUBLE NULL,
                pre_range_120m DOUBLE NULL,
                pre_body_ratio_120m DOUBLE NULL,
                pre_directional_bias_120m DOUBLE NULL,
                pre_green_share_120m DOUBLE NULL,
                pre_red_share_120m DOUBLE NULL,
                pre_avg_range_120m DOUBLE NULL,
                pre_volatility_120m DOUBLE NULL,
                pre_candle_count_120m INT NOT NULL DEFAULT 0,

                pre_distance_to_60m_high DOUBLE NULL,
                pre_distance_to_60m_low DOUBLE NULL,
                pre_distance_to_120m_high DOUBLE NULL,
                pre_distance_to_120m_low DOUBLE NULL,

                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                INDEX idx_pre_context_symbol_confirm (symbol, d_confirm_date),
                INDEX idx_pre_context_root_confirm (root_symbol, d_confirm_date)
            )
            """
        )
    conn.commit()
    for column in SHAPE_FEATURE_COLUMNS:
        base.ensure_column(conn, "pattern_pre_context_features", column, "DOUBLE NULL")
    conn.commit()


def first_template_uid(conn, source_run_id: str) -> str:
    return base.first_template_uid(conn, source_run_id)


def load_setups(conn, args: argparse.Namespace) -> pd.DataFrame:
    table = base.safe_identifier(args.results_table)
    template_uid = first_template_uid(conn, args.source_run_id)
    where = [
        "r.run_id = %s",
        "r.template_uid = %s",
        "r.d_confirm_date IS NOT NULL",
    ]
    params: list[object] = [args.source_run_id, template_uid]
    if args.year is not None:
        where.append("r.d_confirm_date >= %s AND r.d_confirm_date < %s")
        params.extend([f"{args.year}-01-01", f"{args.year + 1}-01-01"])
    elif args.start_year is not None and args.end_year is not None:
        where.append("r.d_confirm_date >= %s AND r.d_confirm_date < %s")
        params.extend([f"{args.start_year}-01-01", f"{args.end_year + 1}-01-01"])
    limit_sql = "LIMIT %s" if args.limit and args.limit > 0 else ""
    if limit_sql:
        params.append(args.limit)

    query = f"""
        SELECT
            r.setup_id,
            r.pattern_id,
            r.pattern_group_id,
            r.symbol,
            ps.root_symbol,
            r.d_confirm_date,
            ps.x_date,
            ps.a_date,
            ps.b_date,
            ps.c_date,
            ps.d_date,
            ps.x_min_max,
            ps.a_min_max,
            ps.b_min_max,
            ps.c_min_max,
            ps.d_min_max,
            ps.xa_price_length,
            ps.ab_price_length,
            ps.bc_price_length,
            ps.cd_price_length,
            ps.x_length,
            ps.a_length,
            ps.b_length,
            ps.c_length,
            ps.d_length,
            ps.d_open,
            ps.d_high,
            ps.d_low,
            ps.d_close,
            ps.bullish_key_reversal,
            ps.bearish_key_reversal,
            ps.bullish_engulfing,
            ps.bearish_engulfing,
            ps.bullish_outside_reversal,
            ps.bearish_outside_reversal,
            ps.hammer,
            ps.shooting_star,
            ps.morning_star,
            ps.evening_star,
            ps.three_white_soldiers,
            ps.three_black_crows
        FROM {table} r FORCE INDEX (idx_entry_exit_template_results_run_setup)
        LEFT JOIN pattern_setups ps
          ON ps.setup_id = r.setup_id
        WHERE {" AND ".join(where)}
        ORDER BY r.symbol ASC, r.d_confirm_date ASC, r.setup_id ASC
        {limit_sql}
    """
    with conn.cursor() as cur:
        cur.execute(query, params)
        rows = cur.fetchall()
    return pd.DataFrame(rows)


def load_symbol_candles(conn, symbol: str, start_dt, end_dt) -> pd.DataFrame:
    query = """
        SELECT ts_utc, open, high, low, close, volume
        FROM futures_contract_1m_candles FORCE INDEX (idx_futures_contract_1m_symbol_time)
        WHERE symbol = %s
          AND ts_utc >= %s
          AND ts_utc < %s
        ORDER BY ts_utc ASC
    """
    with conn.cursor() as cur:
        cur.execute(query, (symbol, start_dt, end_dt))
        rows = cur.fetchall()
    if not rows:
        return pd.DataFrame(columns=["ts_utc", "open", "high", "low", "close", "volume"])
    candles = pd.DataFrame(rows)
    candles["ts_utc"] = pd.to_datetime(candles["ts_utc"], errors="coerce")
    for col in ["open", "high", "low", "close", "volume"]:
        candles[col] = pd.to_numeric(candles[col], errors="coerce")
    candles = candles.dropna(subset=["ts_utc", "open", "high", "low", "close"])
    return candles


def safe_div(numerator: float, denominator: float) -> float | None:
    if denominator is None or denominator == 0 or math.isnan(denominator):
        return None
    return numerator / denominator


def safe_float(value: object) -> float | None:
    if value is None or pd.isna(value):
        return None
    try:
        result = float(value)
    except (TypeError, ValueError):
        return None
    if math.isnan(result):
        return None
    return result


def abs_or_none(value: object) -> float | None:
    number = safe_float(value)
    return abs(number) if number is not None else None


def minutes_between(start: object, end: object) -> float | None:
    start_ts = pd.Timestamp(start) if start is not None and not pd.isna(start) else None
    end_ts = pd.Timestamp(end) if end is not None and not pd.isna(end) else None
    if start_ts is None or end_ts is None:
        return None
    minutes = (end_ts - start_ts).total_seconds() / 60.0
    return minutes if minutes >= 0.0 else None


def compute_shape_features(setup: pd.Series) -> dict[str, object]:
    xa = abs_or_none(setup.get("xa_price_length"))
    ab = abs_or_none(setup.get("ab_price_length"))
    bc = abs_or_none(setup.get("bc_price_length"))
    cd = abs_or_none(setup.get("cd_price_length"))
    price_points = [
        safe_float(setup.get(name))
        for name in ["x_min_max", "a_min_max", "b_min_max", "c_min_max", "d_min_max"]
    ]
    price_points = [value for value in price_points if value is not None]
    pattern_height = (max(price_points) - min(price_points)) if price_points else None
    d_close = safe_float(setup.get("d_close"))
    d_open = safe_float(setup.get("d_open"))
    d_high = safe_float(setup.get("d_high"))
    d_low = safe_float(setup.get("d_low"))
    d_range = (d_high - d_low) if d_high is not None and d_low is not None else None
    d_body = abs(d_close - d_open) if d_close is not None and d_open is not None else None
    d_upper = d_high - max(d_open, d_close) if None not in (d_high, d_open, d_close) else None
    d_lower = min(d_open, d_close) - d_low if None not in (d_low, d_open, d_close) else None

    xa_minutes = minutes_between(setup.get("x_date"), setup.get("a_date"))
    ab_minutes = minutes_between(setup.get("a_date"), setup.get("b_date"))
    bc_minutes = minutes_between(setup.get("b_date"), setup.get("c_date"))
    cd_minutes = minutes_between(setup.get("c_date"), setup.get("d_date"))
    full_minutes = minutes_between(setup.get("x_date"), setup.get("d_date"))
    confirm_lag = minutes_between(setup.get("d_date"), setup.get("d_confirm_date"))
    xa_velocity = safe_div(xa, xa_minutes) if xa is not None and xa_minutes is not None else None
    cd_velocity = safe_div(cd, cd_minutes) if cd is not None and cd_minutes is not None else None

    reversal_flags = [
        "bullish_key_reversal",
        "bearish_key_reversal",
        "bullish_engulfing",
        "bearish_engulfing",
        "bullish_outside_reversal",
        "bearish_outside_reversal",
        "hammer",
        "shooting_star",
        "morning_star",
        "evening_star",
        "three_white_soldiers",
        "three_black_crows",
    ]

    return {
        "pre_xa_points": xa,
        "pre_ab_points": ab,
        "pre_bc_points": bc,
        "pre_cd_points": cd,
        "pre_ab_to_xa": safe_div(ab, xa) if ab is not None and xa is not None else None,
        "pre_bc_to_ab": safe_div(bc, ab) if bc is not None and ab is not None else None,
        "pre_cd_to_xa": safe_div(cd, xa) if cd is not None and xa is not None else None,
        "pre_cd_to_bc": safe_div(cd, bc) if cd is not None and bc is not None else None,
        "pre_pattern_height_points": pattern_height,
        "pre_pattern_height_to_price": safe_div(pattern_height, d_close) if pattern_height is not None and d_close is not None else None,
        "pre_xa_minutes": xa_minutes,
        "pre_ab_minutes": ab_minutes,
        "pre_bc_minutes": bc_minutes,
        "pre_cd_minutes": cd_minutes,
        "pre_full_pattern_minutes": full_minutes,
        "pre_confirm_lag_minutes": confirm_lag,
        "pre_xa_velocity": xa_velocity,
        "pre_cd_velocity": cd_velocity,
        "pre_cd_to_xa_velocity": safe_div(cd_velocity, xa_velocity) if cd_velocity is not None and xa_velocity is not None else None,
        "pre_d_range_to_xa": safe_div(d_range, xa) if d_range is not None and xa is not None else None,
        "pre_d_body_to_xa": safe_div(d_body, xa) if d_body is not None and xa is not None else None,
        "pre_d_close_location": safe_div(d_close - d_low, d_range) if None not in (d_close, d_low, d_range) else None,
        "pre_d_upper_wick_ratio": safe_div(d_upper, d_range) if d_upper is not None and d_range is not None else None,
        "pre_d_lower_wick_ratio": safe_div(d_lower, d_range) if d_lower is not None and d_range is not None else None,
        "pre_reversal_signal_count": float(sum(int(safe_float(setup.get(flag)) or 0) for flag in reversal_flags)),
    }


def segment_features(segment: pd.DataFrame, window: int) -> dict[str, object]:
    def col(name: str) -> str:
        return f"pre_{name}_{window}m"

    values: dict[str, object] = {col("candle_count"): int(len(segment))}
    null_fields = [
        "return",
        "range",
        "body_ratio",
        "directional_bias",
        "green_share",
        "red_share",
        "avg_range",
        "volatility",
    ]
    if len(segment) == 0:
        for name in null_fields:
            values[col(name)] = None
        return values

    first_open = float(segment["open"].iloc[0])
    last_close = float(segment["close"].iloc[-1])
    high = float(segment["high"].max())
    low = float(segment["low"].min())
    range_abs = high - low
    candle_ranges = (segment["high"] - segment["low"]).astype(float)
    bodies = (segment["close"] - segment["open"]).abs().astype(float)
    close_returns = segment["close"].pct_change().replace([np.inf, -np.inf], np.nan).dropna()

    values[col("return")] = safe_div(last_close - first_open, first_open)
    values[col("range")] = safe_div(range_abs, last_close)
    values[col("body_ratio")] = safe_div(float(bodies.sum()), float(candle_ranges.sum()))
    values[col("directional_bias")] = safe_div(last_close - first_open, range_abs)
    values[col("green_share")] = float((segment["close"] > segment["open"]).mean())
    values[col("red_share")] = float((segment["close"] < segment["open"]).mean())
    values[col("avg_range")] = safe_div(float(candle_ranges.mean()), last_close)
    values[col("volatility")] = float(close_returns.std(ddof=0)) if len(close_returns) else 0.0
    return values


def compute_setup_features(setup: pd.Series, candles: pd.DataFrame) -> dict[str, object]:
    confirm = pd.Timestamp(setup["d_confirm_date"])
    ts = candles["ts_utc"].to_numpy(dtype="datetime64[ns]")
    end_index = int(np.searchsorted(ts, np.datetime64(confirm), side="left"))
    max_start = confirm - timedelta(minutes=max(WINDOWS))
    max_start_index = int(np.searchsorted(ts, np.datetime64(max_start), side="left"))
    total_candles = max(0, end_index - max_start_index)

    row: dict[str, object] = {
        "setup_id": setup["setup_id"],
        "pattern_id": setup.get("pattern_id"),
        "pattern_group_id": setup.get("pattern_group_id"),
        "symbol": setup["symbol"],
        "root_symbol": setup.get("root_symbol"),
        "d_confirm_date": confirm.to_pydatetime(),
        "pre_total_candles": total_candles,
    }
    row.update(compute_shape_features(setup))

    for window in WINDOWS:
        start = confirm - timedelta(minutes=window)
        start_index = int(np.searchsorted(ts, np.datetime64(start), side="left"))
        segment = candles.iloc[start_index:end_index]
        row.update(segment_features(segment, window))

    for window in [60, 120]:
        start = confirm - timedelta(minutes=window)
        start_index = int(np.searchsorted(ts, np.datetime64(start), side="left"))
        segment = candles.iloc[start_index:end_index]
        if len(segment) == 0:
            row[f"pre_distance_to_{window}m_high"] = None
            row[f"pre_distance_to_{window}m_low"] = None
            continue
        last_close = float(segment["close"].iloc[-1])
        high = float(segment["high"].max())
        low = float(segment["low"].min())
        row[f"pre_distance_to_{window}m_high"] = safe_div(high - last_close, last_close)
        row[f"pre_distance_to_{window}m_low"] = safe_div(last_close - low, last_close)

    return row


def clean(value: object) -> object:
    if pd.isna(value):
        return None
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, pd.Timestamp):
        return value.to_pydatetime()
    return value


def flush_rows(conn, rows: list[dict[str, object]]) -> None:
    if not rows:
        return
    columns = [
        "setup_id",
        "pattern_id",
        "pattern_group_id",
        "symbol",
        "root_symbol",
        "d_confirm_date",
        "pre_total_candles",
    ]
    columns.extend(SHAPE_FEATURE_COLUMNS)
    for window in WINDOWS:
        columns.extend(
            [
                f"pre_return_{window}m",
                f"pre_range_{window}m",
                f"pre_body_ratio_{window}m",
                f"pre_directional_bias_{window}m",
                f"pre_green_share_{window}m",
                f"pre_red_share_{window}m",
                f"pre_avg_range_{window}m",
                f"pre_volatility_{window}m",
                f"pre_candle_count_{window}m",
            ]
        )
    columns.extend(
        [
            "pre_distance_to_60m_high",
            "pre_distance_to_60m_low",
            "pre_distance_to_120m_high",
            "pre_distance_to_120m_low",
        ]
    )
    placeholders = ", ".join(["%s"] * len(columns))
    update_clause = ", ".join(
        [f"{column}=VALUES({column})" for column in columns if column != "setup_id"]
        + ["updated_at=CURRENT_TIMESTAMP"]
    )
    query = f"""
        INSERT INTO pattern_pre_context_features ({", ".join(columns)})
        VALUES ({placeholders})
        ON DUPLICATE KEY UPDATE {update_clause}
    """
    values = [tuple(clean(row.get(column)) for column in columns) for row in rows]
    with conn.cursor() as cur:
        cur.executemany(query, values)
    conn.commit()


def main() -> int:
    args = parse_args()
    args.results_table = base.safe_identifier(args.results_table)
    conn = base.connect()
    try:
        ensure_table(conn)
        setups = load_setups(conn, args)
        if setups.empty:
            print("No setups found.")
            return 0

        setups["d_confirm_date"] = pd.to_datetime(setups["d_confirm_date"], errors="coerce")
        setups = setups.dropna(subset=["setup_id", "symbol", "d_confirm_date"])
        print(f"Pre-context source setups: {len(setups):,}")
        print(f"Symbols: {setups['symbol'].nunique():,}")

        processed = 0
        pending: list[dict[str, object]] = []
        for symbol, symbol_setups in setups.groupby("symbol", sort=True):
            start_dt = symbol_setups["d_confirm_date"].min() - timedelta(minutes=max(WINDOWS) + 5)
            end_dt = symbol_setups["d_confirm_date"].max() + timedelta(minutes=1)
            candles = load_symbol_candles(conn, symbol, start_dt.to_pydatetime(), end_dt.to_pydatetime())
            if candles.empty:
                print(f"{symbol}: no candles for {len(symbol_setups)} setups")
                continue

            for _, setup in symbol_setups.iterrows():
                pending.append(compute_setup_features(setup, candles))
                processed += 1
                if len(pending) >= args.chunk_size:
                    flush_rows(conn, pending)
                    pending.clear()
            print(f"{symbol}: processed {len(symbol_setups):,} setups, total {processed:,}/{len(setups):,}")

        flush_rows(conn, pending)
        print(f"Stored pre-context features for {processed:,} setups.")
        return 0
    finally:
        conn.close()


if __name__ == "__main__":
    sys.exit(main())
