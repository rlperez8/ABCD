#!/usr/bin/env python3
"""
Materialize non-leaking market context features for AI trade rows.

The table produced here is intentionally keyed by AI run + trade + timeframe so
the UI/modeling layers can read market context as source data instead of
rebuilding it on the fly.
"""

from __future__ import annotations

import argparse
import math
import os
import re
import sys
from dataclasses import dataclass
from datetime import datetime, timedelta
from pathlib import Path
from typing import Any, Iterable
from urllib.parse import urlparse

import numpy as np
import pandas as pd
import pymysql


FEATURE_TABLE = "ai_stage1_trade_market_context"
ABCD_ROOT = Path(__file__).resolve().parents[1]

ROOT_TICK_SIZE = {
    "ES": 0.25,
    "MES": 0.25,
    "NQ": 0.25,
    "MNQ": 0.25,
    "YM": 1.0,
    "MYM": 1.0,
    "CL": 0.01,
    "MCL": 0.01,
    "QM": 0.025,
    "RTY": 0.1,
    "M2K": 0.1,
    "EMD": 0.1,
    "NKD": 5.0,
    "ZL": 0.01,
    "GF": 0.025,
    "LE": 0.025,
    "HE": 0.025,
    "QG": 0.005,
    "NG": 0.001,
    "HO": 0.0001,
    "RB": 0.0001,
    "ZS": 0.25,
    "ZM": 0.1,
    "ZW": 0.25,
    "ZC": 0.25,
    "GC": 0.1,
    "MGC": 0.1,
    "SI": 0.005,
    "HG": 0.0005,
    "PL": 0.1,
    "ZN": 0.015625,
    "ZB": 0.03125,
}


@dataclass(frozen=True)
class TimeframeSpec:
    label: str
    source_table: str
    minutes: int
    lookback_candles: int


TIMEFRAMES: dict[str, TimeframeSpec] = {
    "1m": TimeframeSpec("1m", "futures_contract_1m_candles", 1, 240),
    "3m": TimeframeSpec("3m", "futures_contract_3m_candles", 3, 160),
    "5m": TimeframeSpec("5m", "futures_contract_5m_candles", 5, 120),
    "15m": TimeframeSpec("15m", "futures_contract_15m_candles", 15, 96),
    "30m": TimeframeSpec("30m", "futures_contract_30m_candles", 30, 72),
    "1h": TimeframeSpec("1h", "futures_contract_1h_candles", 60, 72),
    "4h": TimeframeSpec("4h", "futures_contract_4h_candles", 240, 48),
    "12h": TimeframeSpec("12h", "futures_contract_12h_candles", 720, 36),
    "1d": TimeframeSpec("1d", "futures_contract_1d_candles", 1440, 30),
}

DEFAULT_TIMEFRAMES = "3m,5m,15m,30m,1h,4h,1d"
DEFAULT_RUN_PREFIX = "aimv-market-entry-ensemble"
DEFAULT_YEARS = "2022,2023,2024,2025,2026"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--run-ids",
        default=None,
        help="Comma-separated AI run ids. Defaults to --run-prefix + --years.",
    )
    parser.add_argument("--run-prefix", default=DEFAULT_RUN_PREFIX)
    parser.add_argument("--years", default=DEFAULT_YEARS)
    parser.add_argument("--timeframes", default=DEFAULT_TIMEFRAMES)
    parser.add_argument("--replace", action="store_true")
    parser.add_argument("--batch-size", type=int, default=250)
    return parser.parse_args()


def parse_list(raw: str | None) -> list[str]:
    if not raw:
        return []
    return [item.strip() for item in raw.split(",") if item.strip()]


def requested_run_ids(args: argparse.Namespace) -> list[str]:
    explicit = parse_list(args.run_ids)
    if explicit:
        return explicit
    years = [int(value) for value in parse_list(args.years)]
    return [f"{args.run_prefix}-{year}" for year in years]


def requested_timeframes(args: argparse.Namespace) -> list[TimeframeSpec]:
    specs: list[TimeframeSpec] = []
    for value in parse_list(args.timeframes):
        key = value.lower().replace(" ", "")
        if key not in TIMEFRAMES:
            raise ValueError(f"Unsupported timeframe {value!r}. Available: {', '.join(TIMEFRAMES)}")
        spec = TIMEFRAMES[key]
        if spec not in specs:
            specs.append(spec)
    if not specs:
        raise ValueError("No timeframes requested")
    return specs


def floor_to_timeframe(value: datetime, minutes: int) -> datetime:
    bucket_seconds = max(minutes, 1) * 60
    midnight = value.replace(hour=0, minute=0, second=0, microsecond=0)
    seconds = int((value - midnight).total_seconds())
    floored = seconds // bucket_seconds * bucket_seconds
    return midnight + timedelta(seconds=floored)


def last_closed_candle_start(entry_at: datetime, spec: TimeframeSpec) -> datetime:
    return floor_to_timeframe(entry_at - timedelta(minutes=spec.minutes), spec.minutes)


def clean(value: Any) -> Any:
    if isinstance(value, np.generic):
        value = value.item()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    if pd.isna(value):
        return None
    return value


def read_database_url() -> str:
    if os.environ.get("DATABASE_URL"):
        return os.environ["DATABASE_URL"]
    if os.environ.get("ABCD_DATABASE_URL"):
        return os.environ["ABCD_DATABASE_URL"]

    env_path = ABCD_ROOT / ".env"
    if not env_path.exists():
        raise FileNotFoundError(f"Could not find DATABASE_URL or {env_path}")

    for line in env_path.read_text().splitlines():
        match = re.match(r"\s*(?:DATABASE_URL|ABCD_DATABASE_URL)\s*=\s*(.+?)\s*$", line)
        if match:
            return match.group(1).strip().strip('"').strip("'")
    raise ValueError(f"DATABASE_URL was not found in {env_path}")


def connect() -> pymysql.connections.Connection:
    parsed = urlparse(read_database_url())
    return pymysql.connect(
        host=parsed.hostname,
        user=parsed.username,
        password=parsed.password,
        database=parsed.path.lstrip("/"),
        port=parsed.port or 3306,
        autocommit=False,
        cursorclass=pymysql.cursors.DictCursor,
    )


def safe_identifier(value: str) -> str:
    if not re.fullmatch(r"[A-Za-z0-9_]+", value):
        raise ValueError(f"Unsafe SQL identifier: {value!r}")
    return value


def root_symbol(symbol: object) -> str:
    if symbol is None or (isinstance(symbol, float) and math.isnan(symbol)):
        return ""
    text = str(symbol).upper()
    return re.sub(r"[FGHJKMNQUVXZ]\d+$", "", text)


def root_tick_size(value: object) -> float:
    root = root_symbol(value)
    if root in ROOT_TICK_SIZE:
        return ROOT_TICK_SIZE[root]
    for candidate, tick_size in ROOT_TICK_SIZE.items():
        if root.startswith(candidate):
            return tick_size
    return 0.0


def ensure_feature_table(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            f"""
            CREATE TABLE IF NOT EXISTS {FEATURE_TABLE} (
                multi_valid_eval_run_id VARCHAR(64) NOT NULL,
                valid_sample_slot BIGINT NOT NULL DEFAULT 0,
                setup_id VARCHAR(64) NOT NULL,
                template_uid VARCHAR(128) NOT NULL,
                timeframe VARCHAR(16) NOT NULL,
                symbol VARCHAR(32) NULL,
                root_symbol VARCHAR(32) NULL,
                trade_at DATETIME NULL,
                entry_price DOUBLE NULL,
                stop_price DOUBLE NULL,
                target_price DOUBLE NULL,
                risk_points DOUBLE NULL,
                result_r DOUBLE NULL,
                outcome VARCHAR(32) NULL,
                trade_direction VARCHAR(16) NULL,
                candle_table VARCHAR(64) NOT NULL,
                requested_candle_ts_utc DATETIME NULL,
                context_candle_ts_utc DATETIME NULL,
                candle_open DOUBLE NULL,
                candle_high DOUBLE NULL,
                candle_low DOUBLE NULL,
                candle_close DOUBLE NULL,
                candle_volume DOUBLE NULL,
                ema_21 DOUBLE NULL,
                ema_21_slope DOUBLE NULL,
                close_to_ema_pct DOUBLE NULL,
                ema_slope_pct DOUBLE NULL,
                trend_strength_pct DOUBLE NULL,
                trend_label VARCHAR(16) NULL,
                linreg_slope_pct_per_candle DOUBLE NULL,
                linreg_r2 DOUBLE NULL,
                volume_avg_20 DOUBLE NULL,
                relative_volume_20 DOUBLE NULL,
                volume_z_20 DOUBLE NULL,
                prior_high DOUBLE NULL,
                prior_low DOUBLE NULL,
                prior_range_points DOUBLE NULL,
                range_position_pct DOUBLE NULL,
                support_price DOUBLE NULL,
                resistance_price DOUBLE NULL,
                support_distance_points DOUBLE NULL,
                resistance_distance_points DOUBLE NULL,
                support_distance_ticks DOUBLE NULL,
                resistance_distance_ticks DOUBLE NULL,
                support_distance_r DOUBLE NULL,
                resistance_distance_r DOUBLE NULL,
                target_side_sr_distance_r DOUBLE NULL,
                stop_side_sr_distance_r DOUBLE NULL,
                target_blocked_by_sr TINYINT(1) NOT NULL DEFAULT 0,
                stop_behind_sr TINYINT(1) NOT NULL DEFAULT 0,
                tick_size DOUBLE NULL,
                formula VARCHAR(128) NOT NULL DEFAULT 'ema21_sr_volume_closed_candles_v1',
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                PRIMARY KEY (multi_valid_eval_run_id, setup_id, template_uid, timeframe),
                INDEX idx_ai_market_context_run_tf (multi_valid_eval_run_id, timeframe, trend_label),
                INDEX idx_ai_market_context_symbol_tf_time (symbol, timeframe, context_candle_ts_utc),
                INDEX idx_ai_market_context_root_tf (root_symbol, timeframe, trend_label),
                INDEX idx_ai_market_context_blocked (multi_valid_eval_run_id, timeframe, target_blocked_by_sr)
            )
            """
        )
    conn.commit()


def fetch_trades(conn, run_ids: list[str]) -> list[dict[str, Any]]:
    placeholders = ",".join(["%s"] * len(run_ids))
    query = f"""
        SELECT
            multi_valid_eval_run_id,
            valid_sample_slot,
            setup_id,
            template_uid,
            symbol,
            root_symbol,
            trade_at,
            entry_price,
            stop_price,
            target_price,
            risk_points,
            result_r,
            outcome,
            trade_direction
        FROM ai_stage1_trade_rows
        WHERE multi_valid_eval_run_id IN ({placeholders})
          AND trade_at IS NOT NULL
          AND symbol IS NOT NULL
          AND entry_price IS NOT NULL
        ORDER BY multi_valid_eval_run_id, trade_at, setup_id, template_uid
    """
    with conn.cursor() as cur:
        cur.execute(query, run_ids)
        return list(cur.fetchall())


def delete_existing(conn, run_ids: list[str]) -> None:
    placeholders = ",".join(["%s"] * len(run_ids))
    with conn.cursor() as cur:
        cur.execute(
            f"DELETE FROM {FEATURE_TABLE} WHERE multi_valid_eval_run_id IN ({placeholders})",
            run_ids,
        )
    conn.commit()


def fetch_prior_candles(conn, spec: TimeframeSpec, symbol: str, cutoff: datetime, limit: int) -> pd.DataFrame:
    table = safe_identifier(spec.source_table)
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT
                ts_utc,
                CAST(open AS DOUBLE) AS open,
                CAST(high AS DOUBLE) AS high,
                CAST(low AS DOUBLE) AS low,
                CAST(close AS DOUBLE) AS close,
                CAST(volume AS DOUBLE) AS volume
            FROM {table}
            WHERE symbol = %s
              AND ts_utc <= %s
            ORDER BY ts_utc DESC
            LIMIT %s
            """,
            (symbol, cutoff, limit),
        )
        rows = cur.fetchall()
    if not rows:
        return pd.DataFrame()
    return pd.DataFrame(rows).sort_values("ts_utc").reset_index(drop=True)


def ema21(values: Iterable[float]) -> tuple[float | None, float | None]:
    closes = [float(value) for value in values if value is not None and math.isfinite(float(value))]
    period = 21
    if len(closes) < period + 1:
        return None, None

    previous = sum(closes[:period]) / period
    current = previous
    multiplier = 2.0 / (period + 1.0)
    for close in closes[period:]:
        prior = current
        current = (close - prior) * multiplier + prior
    return current, current - prior


def classify_trend(close: float | None, ema: float | None, slope: float | None) -> str | None:
    if close is None or ema is None or slope is None:
        return None
    if close > ema and slope > 0:
        return "bullish"
    if close < ema and slope < 0:
        return "bearish"
    return "neutral"


def regression_features(closes: pd.Series) -> tuple[float | None, float | None]:
    values = pd.to_numeric(closes, errors="coerce").dropna().tail(30).to_numpy(dtype=float)
    if len(values) < 5:
        return None, None
    x = np.arange(len(values), dtype=float)
    slope, intercept = np.polyfit(x, values, 1)
    fitted = slope * x + intercept
    total = float(np.sum((values - values.mean()) ** 2))
    residual = float(np.sum((values - fitted) ** 2))
    r2 = None if total == 0 else max(0.0, min(1.0, 1.0 - residual / total))
    last_close = float(values[-1])
    slope_pct = None if last_close == 0 else slope / last_close * 100.0
    return slope_pct, r2


def numeric_series(frame: pd.DataFrame, column: str) -> pd.Series:
    if frame.empty or column not in frame.columns:
        return pd.Series(dtype=float)
    return pd.to_numeric(frame[column], errors="coerce")


def build_context_row(trade: dict[str, Any], spec: TimeframeSpec, candles: pd.DataFrame) -> dict[str, Any]:
    entry_price = clean_float(trade.get("entry_price"))
    stop_price = clean_float(trade.get("stop_price"))
    target_price = clean_float(trade.get("target_price"))
    risk_points = clean_float(trade.get("risk_points"))
    result_r = clean_float(trade.get("result_r"))
    direction = str(trade.get("trade_direction") or "").lower()
    root = str(trade.get("root_symbol") or root_symbol(trade.get("symbol")) or "")
    tick_size = root_tick_size(root)

    context = candles.iloc[-1] if not candles.empty else None
    close = clean_float(context["close"]) if context is not None else None
    ema, slope = ema21(candles["close"].tolist()) if not candles.empty else (None, None)
    close_to_ema = None if close is None or ema in (None, 0.0) else (close - ema) / ema * 100.0
    ema_slope_pct = None if ema in (None, 0.0) or slope is None else slope / ema * 100.0
    strength = (
        None
        if close_to_ema is None or ema_slope_pct is None
        else abs(close_to_ema) + abs(ema_slope_pct)
    )
    trend_label = classify_trend(close, ema, slope)
    lin_slope, lin_r2 = regression_features(candles["close"]) if not candles.empty else (None, None)

    vol = numeric_series(candles, "volume").dropna().tail(20)
    volume = clean_float(context["volume"]) if context is not None else None
    volume_avg = float(vol.mean()) if len(vol) else None
    volume_std = float(vol.std(ddof=0)) if len(vol) else None
    relative_volume = None if volume is None or not volume_avg else volume / volume_avg
    volume_z = None if volume is None or not volume_std else (volume - volume_avg) / volume_std

    high_series = numeric_series(candles, "high").dropna()
    low_series = numeric_series(candles, "low").dropna()
    prior_high = float(high_series.max()) if len(high_series) else None
    prior_low = float(low_series.min()) if len(low_series) else None
    prior_range = None
    range_position = None
    support_distance = None
    resistance_distance = None
    if entry_price is not None and prior_high is not None and prior_low is not None:
        prior_range = prior_high - prior_low
        support_distance = entry_price - prior_low
        resistance_distance = prior_high - entry_price
        if prior_range and prior_range != 0:
            range_position = (entry_price - prior_low) / prior_range * 100.0

    support_distance_ticks = divide_or_none(support_distance, tick_size)
    resistance_distance_ticks = divide_or_none(resistance_distance, tick_size)
    support_distance_r = divide_or_none(support_distance, risk_points)
    resistance_distance_r = divide_or_none(resistance_distance, risk_points)

    target_distance_r = None
    stop_distance_r = None
    if entry_price is not None and risk_points not in (None, 0.0):
        if target_price is not None:
            target_distance_r = abs(target_price - entry_price) / risk_points
        if stop_price is not None:
            stop_distance_r = abs(entry_price - stop_price) / risk_points

    if direction == "long":
        target_side_sr_distance_r = resistance_distance_r
        stop_side_sr_distance_r = support_distance_r
    elif direction == "short":
        target_side_sr_distance_r = support_distance_r
        stop_side_sr_distance_r = resistance_distance_r
    else:
        target_side_sr_distance_r = None
        stop_side_sr_distance_r = None

    target_blocked = bool(
        target_distance_r is not None
        and target_side_sr_distance_r is not None
        and 0.0 <= target_side_sr_distance_r <= target_distance_r
    )
    stop_behind = bool(
        stop_distance_r is not None
        and stop_side_sr_distance_r is not None
        and 0.0 <= stop_side_sr_distance_r <= stop_distance_r
    )

    return {
        "multi_valid_eval_run_id": trade.get("multi_valid_eval_run_id"),
        "valid_sample_slot": trade.get("valid_sample_slot") or 0,
        "setup_id": trade.get("setup_id"),
        "template_uid": trade.get("template_uid"),
        "timeframe": spec.label,
        "symbol": trade.get("symbol"),
        "root_symbol": root,
        "trade_at": trade.get("trade_at"),
        "entry_price": entry_price,
        "stop_price": stop_price,
        "target_price": target_price,
        "risk_points": risk_points,
        "result_r": result_r,
        "outcome": trade.get("outcome"),
        "trade_direction": trade.get("trade_direction"),
        "candle_table": spec.source_table,
        "requested_candle_ts_utc": last_closed_candle_start(trade["trade_at"], spec),
        "context_candle_ts_utc": context["ts_utc"] if context is not None else None,
        "candle_open": clean_float(context["open"]) if context is not None else None,
        "candle_high": clean_float(context["high"]) if context is not None else None,
        "candle_low": clean_float(context["low"]) if context is not None else None,
        "candle_close": close,
        "candle_volume": volume,
        "ema_21": ema,
        "ema_21_slope": slope,
        "close_to_ema_pct": close_to_ema,
        "ema_slope_pct": ema_slope_pct,
        "trend_strength_pct": strength,
        "trend_label": trend_label,
        "linreg_slope_pct_per_candle": lin_slope,
        "linreg_r2": lin_r2,
        "volume_avg_20": volume_avg,
        "relative_volume_20": relative_volume,
        "volume_z_20": volume_z,
        "prior_high": prior_high,
        "prior_low": prior_low,
        "prior_range_points": prior_range,
        "range_position_pct": range_position,
        "support_price": prior_low,
        "resistance_price": prior_high,
        "support_distance_points": support_distance,
        "resistance_distance_points": resistance_distance,
        "support_distance_ticks": support_distance_ticks,
        "resistance_distance_ticks": resistance_distance_ticks,
        "support_distance_r": support_distance_r,
        "resistance_distance_r": resistance_distance_r,
        "target_side_sr_distance_r": target_side_sr_distance_r,
        "stop_side_sr_distance_r": stop_side_sr_distance_r,
        "target_blocked_by_sr": 1 if target_blocked else 0,
        "stop_behind_sr": 1 if stop_behind else 0,
        "tick_size": tick_size or None,
    }


def clean_float(value: Any) -> float | None:
    if value is None:
        return None
    try:
        result = float(value)
    except (TypeError, ValueError):
        return None
    if not math.isfinite(result):
        return None
    return result


def divide_or_none(numerator: float | None, denominator: float | None) -> float | None:
    if numerator is None or denominator is None or denominator == 0:
        return None
    return numerator / denominator


def insert_rows(conn, rows: list[dict[str, Any]]) -> None:
    if not rows:
        return
    columns = [
        "multi_valid_eval_run_id",
        "valid_sample_slot",
        "setup_id",
        "template_uid",
        "timeframe",
        "symbol",
        "root_symbol",
        "trade_at",
        "entry_price",
        "stop_price",
        "target_price",
        "risk_points",
        "result_r",
        "outcome",
        "trade_direction",
        "candle_table",
        "requested_candle_ts_utc",
        "context_candle_ts_utc",
        "candle_open",
        "candle_high",
        "candle_low",
        "candle_close",
        "candle_volume",
        "ema_21",
        "ema_21_slope",
        "close_to_ema_pct",
        "ema_slope_pct",
        "trend_strength_pct",
        "trend_label",
        "linreg_slope_pct_per_candle",
        "linreg_r2",
        "volume_avg_20",
        "relative_volume_20",
        "volume_z_20",
        "prior_high",
        "prior_low",
        "prior_range_points",
        "range_position_pct",
        "support_price",
        "resistance_price",
        "support_distance_points",
        "resistance_distance_points",
        "support_distance_ticks",
        "resistance_distance_ticks",
        "support_distance_r",
        "resistance_distance_r",
        "target_side_sr_distance_r",
        "stop_side_sr_distance_r",
        "target_blocked_by_sr",
        "stop_behind_sr",
        "tick_size",
    ]
    placeholders = ",".join(["%s"] * len(columns))
    update_clause = ", ".join(
        f"{column} = VALUES({column})"
        for column in columns
        if column not in {"multi_valid_eval_run_id", "setup_id", "template_uid", "timeframe"}
    )
    values = [tuple(clean(row.get(column)) for column in columns) for row in rows]
    with conn.cursor() as cur:
        cur.executemany(
            f"""
            INSERT INTO {FEATURE_TABLE} ({", ".join(columns)})
            VALUES ({placeholders})
            ON DUPLICATE KEY UPDATE
                {update_clause},
                formula = 'ema21_sr_volume_closed_candles_v1',
                updated_at = CURRENT_TIMESTAMP
            """,
            values,
        )


def print_summary(conn, run_ids: list[str]) -> None:
    placeholders = ",".join(["%s"] * len(run_ids))
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT
                multi_valid_eval_run_id,
                timeframe,
                COUNT(*) AS rows_seen,
                SUM(CASE WHEN context_candle_ts_utc IS NOT NULL THEN 1 ELSE 0 END) AS with_candles,
                SUM(CASE WHEN trend_label = 'bullish' THEN 1 ELSE 0 END) AS bullish,
                SUM(CASE WHEN trend_label = 'bearish' THEN 1 ELSE 0 END) AS bearish,
                SUM(CASE WHEN trend_label = 'neutral' THEN 1 ELSE 0 END) AS neutral,
                AVG(relative_volume_20) AS avg_rel_volume,
                AVG(target_blocked_by_sr) * 100 AS target_blocked_pct
            FROM {FEATURE_TABLE}
            WHERE multi_valid_eval_run_id IN ({placeholders})
            GROUP BY multi_valid_eval_run_id, timeframe
            ORDER BY multi_valid_eval_run_id,
                FIELD(timeframe, '1m', '3m', '5m', '15m', '30m', '1h', '4h', '12h', '1d'),
                timeframe
            """,
            run_ids,
        )
        rows = cur.fetchall()
    print("run_id\ttf\trows\twith_candles\tbull\tbear\tneutral\tavg_rel_vol\ttarget_blocked%")
    for row in rows:
        print(
            f"{row['multi_valid_eval_run_id']}\t{row['timeframe']}\t{row['rows_seen']}\t"
            f"{row['with_candles']}\t{row['bullish']}\t{row['bearish']}\t{row['neutral']}\t"
            f"{fmt(row['avg_rel_volume'])}\t{fmt(row['target_blocked_pct'])}"
        )


def fmt(value: Any) -> str:
    if value is None:
        return "-"
    try:
        return f"{float(value):.2f}"
    except (TypeError, ValueError):
        return str(value)


def main() -> None:
    args = parse_args()
    run_ids = requested_run_ids(args)
    specs = requested_timeframes(args)

    conn = connect()
    try:
        ensure_feature_table(conn)
        if args.replace:
            delete_existing(conn, run_ids)

        trades = fetch_trades(conn, run_ids)
        total_work = len(trades) * len(specs)
        print(
            f"Materializing market context for {len(trades)} trades, "
            f"{len(specs)} timeframes ({total_work} rows)"
        )

        pending: list[dict[str, Any]] = []
        done = 0
        for trade in trades:
            for spec in specs:
                cutoff = last_closed_candle_start(trade["trade_at"], spec)
                candles = fetch_prior_candles(
                    conn,
                    spec,
                    str(trade["symbol"]),
                    cutoff,
                    max(spec.lookback_candles, 40),
                )
                pending.append(build_context_row(trade, spec, candles))
                done += 1
                if len(pending) >= args.batch_size:
                    insert_rows(conn, pending)
                    conn.commit()
                    pending.clear()
                if done % 1000 == 0 or done == total_work:
                    print(f"market context: {done}/{total_work} rows")

        if pending:
            insert_rows(conn, pending)
            conn.commit()

        print_summary(conn, run_ids)
    finally:
        conn.close()


if __name__ == "__main__":
    main()
