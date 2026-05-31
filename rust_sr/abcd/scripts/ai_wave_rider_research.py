#!/usr/bin/env python3
"""
Materialize dynamic "wave rider" AI trade runs.

This intentionally creates normal AI Stage 1 run/result rows so the existing
AI Trades UI can inspect the output. Two modes are supported:

* rule   - live-safe first valid wave trigger, then trailing/momentum exit.
* oracle - future-looking ceiling pass over plausible wave entries.

The oracle mode is not a live strategy. It is a research target that tells us
whether dynamic timing has enough upside to train a real model against.
"""

from __future__ import annotations

import argparse
import math
import os
import re
import sys
from dataclasses import dataclass
from datetime import timedelta
from pathlib import Path
from typing import Any, Iterable
from urllib.parse import urlparse

import numpy as np
import pandas as pd
import pymysql


ABCD_ROOT = Path(__file__).resolve().parents[1]

TRADE_DERIVED_TABLES = [
    "ai_stage1_trade_loss_windows",
    "ai_stage1_trade_family_contribution",
    "ai_stage1_trade_symbol_contribution",
    "ai_stage1_trade_workload",
    "ai_stage1_trade_cadence",
    "ai_stage1_trade_hourly",
    "ai_stage1_trade_daily_r",
    "ai_stage1_trade_template_performance",
    "ai_stage1_trade_summary",
    "ai_stage1_trade_rows",
]

RESULT_COLUMNS = [
    "run_id",
    "template_uid",
    "setup_id",
    "pattern_id",
    "pattern_group_id",
    "event_id",
    "event_rank",
    "event_sister_count",
    "event_decision_date",
    "event_candidate_count",
    "event_live_candidate_count",
    "pattern_family_key",
    "symbol",
    "market",
    "d_date",
    "d_confirm_date",
    "evaluation_order",
    "was_created_for_setup",
    "outcome",
    "exit_reason",
    "result_r",
    "entry_date",
    "exit_date",
    "entry_price",
    "stop_price",
    "target_price",
    "exit_price",
    "risk_points",
    "trade_direction",
]

SELECTED_COLUMNS = [
    "valid_sample_slot",
    "setup_id",
    "pattern_id",
    "pattern_group_id",
    "symbol",
    "market",
    "pattern_family_key",
    "d_confirm_date",
    "template_uid",
    "template_name",
    "model_rank",
    "predicted_expected_r",
    "score_margin_top2",
    "actual_result_r",
    "actual_outcome",
    "oracle_template_uid",
    "oracle_result_r",
    "oracle_rank",
]

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
class WaveResult:
    outcome: str
    exit_reason: str
    trade_direction: str | None
    entry_date: Any | None
    exit_date: Any | None
    entry_price: float | None
    stop_price: float | None
    target_price: float | None
    exit_price: float | None
    risk_points: float | None
    result_r: float
    raw_result_r: float | None
    mfe_r: float | None
    mae_r: float | None
    signal_index: int | None
    entry_index: int | None
    exit_index: int | None
    entry_delay_minutes: int | None
    hold_minutes: int | None
    risk_ticks: float | None
    trigger_score: float | None
    candidate_count: int
    no_entry_reason: str | None


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-prefix", default="aimv-market-entry-context-v7")
    parser.add_argument("--years", default="2022,2023,2024,2025,2026")
    parser.add_argument("--mode", choices=["rule", "oracle", "both"], default="both")
    parser.add_argument("--rule-run-prefix", default="aimv-wave-rider-rule-v1")
    parser.add_argument("--oracle-run-prefix", default="aimv-wave-rider-oracle-v1")
    parser.add_argument("--rule-source-prefix", default="wave-rider-rule-v1")
    parser.add_argument("--oracle-source-prefix", default="wave-rider-oracle-v1")
    parser.add_argument("--replace", action="store_true")
    parser.add_argument("--entry-scan-minutes", type=int, default=720)
    parser.add_argument("--max-forward-minutes", type=int, default=2880)
    parser.add_argument("--indicator-lookback-minutes", type=int, default=360)
    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=160.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    parser.add_argument("--death-bars", type=int, default=3)
    parser.add_argument("--min-hold-bars", type=int, default=8)
    parser.add_argument("--profit-lock-trigger-r", type=float, default=0.0)
    parser.add_argument("--profit-lock-giveback-r", type=float, default=1.5)
    parser.add_argument("--profit-lock-min-r", type=float, default=0.25)
    parser.add_argument("--time-exit-minutes", type=int, default=0)
    parser.add_argument("--slippage-entry-ticks", type=float, default=3.0)
    parser.add_argument("--slippage-exit-ticks", type=float, default=3.0)
    parser.add_argument("--directions", choices=["source", "both"], default="both")
    parser.add_argument("--oracle-max-candidates", type=int, default=80)
    parser.add_argument("--limit", type=int, default=0, help="Optional per-year row limit for debugging.")
    parser.add_argument("--refresh-url", default="http://localhost:8080/patterns/ai-stage1-trades")
    parser.add_argument("--skip-api-refresh", action="store_true")
    return parser.parse_args()


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


def clean(value: Any) -> Any:
    if isinstance(value, np.generic):
        value = value.item()
    if isinstance(value, pd.Timestamp):
        if pd.isna(value):
            return None
        return value.to_pydatetime()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    if value is None:
        return None
    try:
        if pd.isna(value):
            return None
    except TypeError:
        pass
    return value


def safe_identifier(value: str) -> str:
    if not re.fullmatch(r"[A-Za-z0-9_]+", value):
        raise ValueError(f"Unsafe SQL identifier: {value!r}")
    return value


def parse_list(value: str) -> list[str]:
    return [part.strip() for part in value.split(",") if part.strip()]


def years_from_arg(value: str) -> list[int]:
    years = [int(part) for part in parse_list(value)]
    if not years:
        raise ValueError("At least one year is required")
    return years


def safe_run_id(prefix: str, year: int) -> str:
    run_id = f"{prefix}-{year}"
    if len(run_id) > 64:
        raise ValueError(f"Run id is too long for DB column: {run_id}")
    return run_id


def result_table_for(prefix: str, year: int) -> str:
    suffix = re.sub(r"[^A-Za-z0-9]+", "_", prefix).strip("_")
    return safe_identifier(f"entry_exit_template_results_{suffix}_{year}")


def root_symbol(symbol: object) -> str:
    if symbol is None:
        return ""
    text = str(symbol).upper()
    return re.sub(r"[FGHJKMNQUVXZ][0-9]+$", "", text)


def tick_size_for(symbol: object, fallback: object = None) -> float:
    root = root_symbol(fallback or symbol)
    if root in ROOT_TICK_SIZE:
        return ROOT_TICK_SIZE[root]
    for candidate, tick_size in ROOT_TICK_SIZE.items():
        if root.startswith(candidate):
            return tick_size
    return 0.25


def normalize_direction(value: object) -> str | None:
    text = str(value or "").strip().upper()
    if text in {"LONG", "BUY", "BULL", "BULLISH"}:
        return "LONG"
    if text in {"SHORT", "SELL", "BEAR", "BEARISH"}:
        return "SHORT"
    return None


def direction_sign(direction: str) -> float:
    return -1.0 if direction.upper() == "SHORT" else 1.0


def directions_for_row(row: pd.Series, args: argparse.Namespace) -> list[str]:
    source = normalize_direction(row.get("source_trade_direction")) or normalize_direction(row.get("trade_direction"))
    if args.directions == "source" and source:
        return [source]
    if args.directions == "source":
        return ["LONG", "SHORT"]
    return ["LONG", "SHORT"]


def ensure_result_table(conn, result_table: str) -> None:
    with conn.cursor() as cur:
        cur.execute(f"CREATE TABLE IF NOT EXISTS {safe_identifier(result_table)} LIKE entry_exit_template_results_eetc_mktwide_v1")


def ensure_diagnostics_table(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_wave_rider_trade_diagnostics (
                multi_valid_eval_run_id VARCHAR(64) NOT NULL,
                source_ai_run_id VARCHAR(64) NOT NULL,
                source_run_id VARCHAR(64) NOT NULL,
                setup_id VARCHAR(64) NOT NULL,
                template_uid VARCHAR(128) NOT NULL,
                mode VARCHAR(16) NOT NULL,
                symbol VARCHAR(32) NULL,
                root_symbol VARCHAR(32) NULL,
                d_confirm_date DATETIME NULL,
                source_trade_direction VARCHAR(16) NULL,
                selected_trade_direction VARCHAR(16) NULL,
                entry_delay_minutes INT NULL,
                hold_minutes INT NULL,
                signal_index INT NULL,
                entry_index INT NULL,
                exit_index INT NULL,
                risk_ticks DOUBLE NULL,
                tick_size DOUBLE NULL,
                raw_result_r DOUBLE NULL,
                slipped_result_r DOUBLE NULL,
                mfe_r DOUBLE NULL,
                mae_r DOUBLE NULL,
                trigger_score DOUBLE NULL,
                candidate_count INT NOT NULL DEFAULT 0,
                no_entry_reason VARCHAR(64) NULL,
                exit_reason VARCHAR(64) NULL,
                formula VARCHAR(128) NOT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                PRIMARY KEY (multi_valid_eval_run_id, setup_id, template_uid),
                INDEX idx_wave_diag_mode_result (mode, slipped_result_r),
                INDEX idx_wave_diag_symbol_time (symbol, d_confirm_date),
                INDEX idx_wave_diag_source (source_ai_run_id)
            )
            """
        )


def fetch_run(conn, run_id: str) -> dict[str, Any]:
    with conn.cursor() as cur:
        cur.execute("SELECT * FROM ai_stage1_multi_valid_eval_runs WHERE multi_valid_eval_run_id = %s", (run_id,))
        row = cur.fetchone()
    if not row:
        raise RuntimeError(f"Missing AI run: {run_id}")
    return row


def fetch_source_rows(conn, source_ai_run_id: str, limit: int = 0) -> pd.DataFrame:
    run = fetch_run(conn, source_ai_run_id)
    result_table = safe_identifier(str(run["results_table"]))
    source_run_id = str(run["source_run_id"])
    limit_sql = f"LIMIT {int(limit)}" if limit and limit > 0 else ""
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT
                s.valid_sample_slot,
                s.setup_id,
                s.pattern_id,
                s.pattern_group_id,
                s.symbol,
                s.market,
                s.pattern_family_key,
                s.d_confirm_date,
                s.template_uid AS source_template_uid,
                s.template_name AS source_template_name,
                s.model_rank AS source_model_rank,
                s.predicted_expected_r AS source_predicted_expected_r,
                s.score_margin_top2 AS source_score_margin_top2,
                s.actual_result_r AS source_actual_result_r,
                s.actual_outcome AS source_actual_outcome,
                r.trade_direction AS source_trade_direction,
                r.entry_date AS source_entry_date,
                r.exit_date AS source_exit_date,
                r.entry_price AS source_entry_price,
                r.exit_price AS source_exit_price,
                r.risk_points AS source_risk_points,
                ps.root_symbol,
                ps.harmonic_type,
                ps.pattern_family_harmonic_type,
                ps.pattern_family_bin,
                ps.pattern_family_size_bucket,
                ps.pattern_family_time_bin,
                ps.pattern_family_x_strictness,
                ps.event_id,
                ps.event_rank,
                ps.event_sister_count,
                ps.d_date,
                ps.d_confirm_date AS setup_d_confirm_date,
                ps.xa_price_length,
                ps.cd_price_length,
                ps.full_pattern_length
            FROM ai_stage1_multi_valid_eval_selected s
            LEFT JOIN {result_table} r
              ON r.run_id = %s
             AND r.setup_id = s.setup_id
             AND r.template_uid = s.template_uid
            LEFT JOIN pattern_setups ps
              ON ps.setup_id = s.setup_id
            WHERE s.multi_valid_eval_run_id = %s
              AND COALESCE(s.symbol, '') <> ''
              AND COALESCE(s.d_confirm_date, ps.d_confirm_date) IS NOT NULL
            ORDER BY s.d_confirm_date, s.setup_id
            {limit_sql}
            """,
            (source_run_id, source_ai_run_id),
        )
        rows = cur.fetchall()
    frame = pd.DataFrame(rows)
    if frame.empty:
        return frame
    for col in ["d_confirm_date", "setup_d_confirm_date", "d_date", "source_entry_date", "source_exit_date"]:
        frame[col] = pd.to_datetime(frame[col], errors="coerce")
    for col in [
        "source_predicted_expected_r",
        "source_score_margin_top2",
        "source_actual_result_r",
        "source_entry_price",
        "source_exit_price",
        "source_risk_points",
        "xa_price_length",
        "cd_price_length",
        "full_pattern_length",
        "event_rank",
        "event_sister_count",
    ]:
        frame[col] = pd.to_numeric(frame[col], errors="coerce")
    frame["d_confirm_date"] = frame["d_confirm_date"].fillna(frame["setup_d_confirm_date"])
    return frame.dropna(subset=["d_confirm_date"])


def fetch_candles(conn, symbol: str, start_dt: pd.Timestamp, end_dt: pd.Timestamp) -> pd.DataFrame:
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT
                ts_utc,
                CAST(open AS DOUBLE) AS open,
                CAST(high AS DOUBLE) AS high,
                CAST(low AS DOUBLE) AS low,
                CAST(close AS DOUBLE) AS close,
                CAST(volume AS DOUBLE) AS volume
            FROM futures_contract_1m_candles FORCE INDEX (idx_futures_contract_1m_symbol_time)
            WHERE symbol = %s
              AND ts_utc >= %s
              AND ts_utc <= %s
            ORDER BY ts_utc ASC
            """,
            (symbol, start_dt.to_pydatetime(), end_dt.to_pydatetime()),
        )
        rows = cur.fetchall()
    if not rows:
        return pd.DataFrame(columns=["ts_utc", "open", "high", "low", "close", "volume"])
    candles = pd.DataFrame(rows)
    candles["ts_utc"] = pd.to_datetime(candles["ts_utc"], errors="coerce")
    for col in ["open", "high", "low", "close", "volume"]:
        candles[col] = pd.to_numeric(candles[col], errors="coerce")
    candles = candles.dropna(subset=["ts_utc", "open", "high", "low", "close"])
    return candles.sort_values("ts_utc").reset_index(drop=True)


def prepare_candles(candles: pd.DataFrame, args: argparse.Namespace) -> pd.DataFrame:
    if candles.empty:
        return candles
    work = candles.copy()
    work["ema_fast"] = work["close"].ewm(span=8, adjust=False, min_periods=8).mean()
    work["ema_slow"] = work["close"].ewm(span=21, adjust=False, min_periods=21).mean()
    work["ema_fast_slope"] = work["ema_fast"].diff()
    prev_close = work["close"].shift(1)
    true_range = pd.concat(
        [
            work["high"] - work["low"],
            (work["high"] - prev_close).abs(),
            (work["low"] - prev_close).abs(),
        ],
        axis=1,
    ).max(axis=1)
    work["atr"] = true_range.rolling(args.atr_period, min_periods=max(4, args.atr_period // 2)).mean()
    work["entry_high"] = work["high"].shift(1).rolling(args.entry_breakout_bars, min_periods=3).max()
    work["entry_low"] = work["low"].shift(1).rolling(args.entry_breakout_bars, min_periods=3).min()
    work["swing_low"] = work["low"].shift(1).rolling(args.trail_lookback_bars, min_periods=4).min()
    work["swing_high"] = work["high"].shift(1).rolling(args.trail_lookback_bars, min_periods=4).max()
    prior_volume = work["volume"].shift(1).rolling(20, min_periods=5).mean()
    work["relative_volume"] = work["volume"] / prior_volume.replace(0, np.nan)
    work["prior_range"] = work["high"].shift(1).rolling(args.entry_breakout_bars, min_periods=3).max() - work[
        "low"
    ].shift(1).rolling(args.entry_breakout_bars, min_periods=3).min()
    work["compression"] = work["prior_range"] / work["atr"].replace(0, np.nan)
    return work


def start_index(candles: pd.DataFrame, d_confirm_date: pd.Timestamp) -> int:
    ts = candles["ts_utc"].to_numpy(dtype="datetime64[ns]")
    return int(np.searchsorted(ts, np.datetime64(d_confirm_date), side="right"))


def minutes_between(left: Any, right: Any) -> int | None:
    if left is None or right is None:
        return None
    left_ts = pd.Timestamp(left)
    right_ts = pd.Timestamp(right)
    if pd.isna(left_ts) or pd.isna(right_ts):
        return None
    return int(round((right_ts - left_ts).total_seconds() / 60.0))


def finite(value: Any, default: float | None = None) -> float | None:
    try:
        result = float(value)
    except (TypeError, ValueError):
        return default
    if math.isnan(result) or math.isinf(result):
        return default
    return result


def trigger_score(candles: pd.DataFrame, signal_idx: int, direction: str) -> float:
    row = candles.iloc[signal_idx]
    atr = finite(row.get("atr"), 0.0) or 0.0
    rel_vol = finite(row.get("relative_volume"), 1.0) or 1.0
    compression = finite(row.get("compression"), 2.0) or 2.0
    if atr <= 0:
        atr = max(abs(float(row["close"]) * 0.0005), 1e-9)
    if direction == "LONG":
        breakout = (float(row["close"]) - (finite(row.get("entry_high"), row["close"]) or float(row["close"]))) / atr
        trend_gap = (float(row.get("ema_fast", row["close"])) - float(row.get("ema_slow", row["close"]))) / atr
    else:
        breakout = ((finite(row.get("entry_low"), row["close"]) or float(row["close"])) - float(row["close"])) / atr
        trend_gap = (float(row.get("ema_slow", row["close"])) - float(row.get("ema_fast", row["close"]))) / atr
    compression_bonus = max(0.0, 2.5 - compression) * 0.15
    return float(max(0.0, breakout) * 0.55 + max(0.0, trend_gap) * 0.30 + min(rel_vol, 3.0) * 0.10 + compression_bonus)


def is_wave_trigger(candles: pd.DataFrame, signal_idx: int, direction: str, args: argparse.Namespace, loose: bool = False) -> bool:
    if signal_idx <= 25 or signal_idx >= len(candles) - 1:
        return False
    row = candles.iloc[signal_idx]
    close = finite(row.get("close"))
    ema_fast = finite(row.get("ema_fast"))
    ema_slow = finite(row.get("ema_slow"))
    slope = finite(row.get("ema_fast_slope"))
    rel_vol = finite(row.get("relative_volume"), 1.0) or 1.0
    entry_high = finite(row.get("entry_high"))
    entry_low = finite(row.get("entry_low"))
    if close is None or ema_fast is None or ema_slow is None or slope is None:
        return False
    min_volume = args.min_relative_volume * (0.75 if loose else 1.0)
    if rel_vol < min_volume:
        return False
    if direction == "LONG":
        breakout_ok = entry_high is not None and close > entry_high
        trend_ok = ema_fast >= ema_slow and slope >= 0
        loose_ok = close > ema_fast and slope >= 0
        return bool((breakout_ok and trend_ok) or (loose and loose_ok and trend_ok))
    breakout_ok = entry_low is not None and close < entry_low
    trend_ok = ema_fast <= ema_slow and slope <= 0
    loose_ok = close < ema_fast and slope <= 0
    return bool((breakout_ok and trend_ok) or (loose and loose_ok and trend_ok))


def initial_stop(candles: pd.DataFrame, entry_idx: int, direction: str, tick_size: float, args: argparse.Namespace) -> tuple[float | None, float | None]:
    row = candles.iloc[entry_idx]
    entry_price = finite(row.get("open"))
    if entry_price is None:
        return None, None
    atr = finite(row.get("atr"), tick_size * args.min_risk_ticks) or tick_size * args.min_risk_ticks
    pad = max(tick_size, atr * args.atr_stop_pad)
    if direction == "LONG":
        swing = finite(row.get("swing_low"))
        raw_stop = (swing - pad) if swing is not None else entry_price - tick_size * args.min_risk_ticks
        risk = entry_price - raw_stop
        min_risk = tick_size * args.min_risk_ticks
        if risk < min_risk:
            raw_stop = entry_price - min_risk
            risk = min_risk
    else:
        swing = finite(row.get("swing_high"))
        raw_stop = (swing + pad) if swing is not None else entry_price + tick_size * args.min_risk_ticks
        risk = raw_stop - entry_price
        min_risk = tick_size * args.min_risk_ticks
        if risk < min_risk:
            raw_stop = entry_price + min_risk
            risk = min_risk
    risk_ticks = risk / tick_size if tick_size > 0 else None
    if risk <= 0 or risk_ticks is None or risk_ticks > args.max_risk_ticks:
        return None, None
    return float(raw_stop), float(risk)


def update_trailing_stop(
    candles: pd.DataFrame,
    current_stop: float,
    entry_idx: int,
    candle_idx: int,
    direction: str,
    tick_size: float,
    args: argparse.Namespace,
) -> float:
    if candle_idx - entry_idx < args.min_hold_bars:
        return current_stop
    low = candles["low"].iloc[max(entry_idx, candle_idx - args.trail_lookback_bars + 1) : candle_idx + 1].min()
    high = candles["high"].iloc[max(entry_idx, candle_idx - args.trail_lookback_bars + 1) : candle_idx + 1].max()
    atr = finite(candles["atr"].iloc[candle_idx], tick_size * args.min_risk_ticks) or tick_size * args.min_risk_ticks
    pad = max(tick_size, atr * args.atr_stop_pad)
    close = float(candles["close"].iloc[candle_idx])
    if direction == "LONG":
        proposed = float(low) - pad
        proposed = min(proposed, close - tick_size)
        return max(current_stop, proposed)
    proposed = float(high) + pad
    proposed = max(proposed, close + tick_size)
    return min(current_stop, proposed)


def update_profit_lock_stop(
    current_stop: float,
    entry_price: float,
    risk_points: float,
    max_favorable_r: float,
    direction: str,
    tick_size: float,
    args: argparse.Namespace,
) -> float:
    trigger_r = float(getattr(args, "profit_lock_trigger_r", 0.0) or 0.0)
    if trigger_r <= 0.0 or max_favorable_r < trigger_r:
        return current_stop
    giveback_r = max(0.0, float(getattr(args, "profit_lock_giveback_r", 1.5) or 0.0))
    min_lock_r = max(0.0, float(getattr(args, "profit_lock_min_r", 0.25) or 0.0))
    locked_r = max(min_lock_r, max_favorable_r - giveback_r)
    if direction == "LONG":
        proposed = entry_price + locked_r * risk_points
        return max(current_stop, proposed - tick_size)
    proposed = entry_price - locked_r * risk_points
    return min(current_stop, proposed + tick_size)


def momentum_dead(candles: pd.DataFrame, candle_idx: int, direction: str) -> bool:
    row = candles.iloc[candle_idx]
    close = finite(row.get("close"))
    ema_fast = finite(row.get("ema_fast"))
    ema_slow = finite(row.get("ema_slow"))
    slope = finite(row.get("ema_fast_slope"))
    if close is None or ema_fast is None or ema_slow is None or slope is None:
        return False
    if direction == "LONG":
        return close < ema_fast and (slope < 0 or close < ema_slow)
    return close > ema_fast and (slope > 0 or close > ema_slow)


def simulate_from_entry(
    candles: pd.DataFrame,
    d_confirm_date: pd.Timestamp,
    entry_idx: int,
    signal_idx: int | None,
    direction: str,
    tick_size: float,
    args: argparse.Namespace,
) -> WaveResult:
    if entry_idx >= len(candles):
        return no_entry("entry_out_of_range", signal_idx=signal_idx, candidate_count=1)
    stop_price, risk_points = initial_stop(candles, entry_idx, direction, tick_size, args)
    entry_price = finite(candles["open"].iloc[entry_idx])
    if stop_price is None or risk_points is None or entry_price is None:
        return no_entry("risk_rejected", signal_idx=signal_idx, candidate_count=1)

    sign = direction_sign(direction)
    current_stop = stop_price
    requested_end_idx = entry_idx + max(1, args.max_forward_minutes)
    time_exit_minutes = int(getattr(args, "time_exit_minutes", 0) or 0)
    if time_exit_minutes > 0:
        requested_end_idx = min(requested_end_idx, entry_idx + time_exit_minutes)
    end_idx = min(len(candles) - 1, requested_end_idx)
    exit_idx = end_idx
    exit_price = finite(candles["close"].iloc[end_idx])
    exit_reason = "time_exit" if time_exit_minutes > 0 and end_idx == entry_idx + time_exit_minutes else "path_end"
    death_count = 0
    max_favorable = 0.0
    max_adverse = 0.0

    for candle_idx in range(entry_idx, end_idx + 1):
        candle = candles.iloc[candle_idx]
        high = float(candle["high"])
        low = float(candle["low"])
        if direction == "LONG":
            max_favorable = max(max_favorable, (high - entry_price) / risk_points)
            max_adverse = max(max_adverse, (entry_price - low) / risk_points)
            if low <= current_stop:
                exit_idx = candle_idx
                exit_price = current_stop
                exit_reason = "trailing_stop"
                break
        else:
            max_favorable = max(max_favorable, (entry_price - low) / risk_points)
            max_adverse = max(max_adverse, (high - entry_price) / risk_points)
            if high >= current_stop:
                exit_idx = candle_idx
                exit_price = current_stop
                exit_reason = "trailing_stop"
                break

        if candle_idx - entry_idx >= args.min_hold_bars and momentum_dead(candles, candle_idx, direction):
            death_count += 1
        else:
            death_count = 0
        if death_count >= args.death_bars:
            next_idx = min(candle_idx + 1, end_idx)
            exit_idx = next_idx
            exit_price = finite(candles["open"].iloc[next_idx], candles["close"].iloc[next_idx])
            exit_reason = "momentum_fade"
            break

        current_stop = update_profit_lock_stop(
            current_stop,
            float(entry_price),
            float(risk_points),
            max_favorable,
            direction,
            tick_size,
            args,
        )
        current_stop = update_trailing_stop(candles, current_stop, entry_idx, candle_idx, direction, tick_size, args)

    if exit_price is None:
        return no_entry("missing_exit_price", signal_idx=signal_idx, candidate_count=1)

    raw_result = sign * (float(exit_price) - entry_price) / risk_points
    slippage_cost_r = ((args.slippage_entry_ticks + args.slippage_exit_ticks) * tick_size) / risk_points
    result_r = raw_result - slippage_cost_r
    outcome = "pass" if result_r > 0 else "fail"
    entry_date = candles["ts_utc"].iloc[entry_idx]
    exit_date = candles["ts_utc"].iloc[exit_idx]
    risk_ticks = risk_points / tick_size if tick_size > 0 else None
    return WaveResult(
        outcome=outcome,
        exit_reason=exit_reason,
        trade_direction=direction,
        entry_date=entry_date,
        exit_date=exit_date,
        entry_price=float(entry_price),
        stop_price=float(stop_price),
        target_price=None,
        exit_price=float(exit_price),
        risk_points=float(risk_points),
        result_r=float(result_r),
        raw_result_r=float(raw_result),
        mfe_r=float(max_favorable),
        mae_r=float(max_adverse),
        signal_index=signal_idx,
        entry_index=entry_idx,
        exit_index=exit_idx,
        entry_delay_minutes=minutes_between(d_confirm_date, entry_date),
        hold_minutes=minutes_between(entry_date, exit_date),
        risk_ticks=float(risk_ticks) if risk_ticks is not None else None,
        trigger_score=trigger_score(candles, signal_idx, direction) if signal_idx is not None else None,
        candidate_count=1,
        no_entry_reason=None,
    )


def no_entry(reason: str, signal_idx: int | None = None, candidate_count: int = 0) -> WaveResult:
    return WaveResult(
        outcome="no_entry",
        exit_reason=reason,
        trade_direction=None,
        entry_date=None,
        exit_date=None,
        entry_price=None,
        stop_price=None,
        target_price=None,
        exit_price=None,
        risk_points=None,
        result_r=0.0,
        raw_result_r=None,
        mfe_r=None,
        mae_r=None,
        signal_index=signal_idx,
        entry_index=None,
        exit_index=None,
        entry_delay_minutes=None,
        hold_minutes=None,
        risk_ticks=None,
        trigger_score=None,
        candidate_count=candidate_count,
        no_entry_reason=reason,
    )


def evaluate_rule(row: pd.Series, candles: pd.DataFrame, args: argparse.Namespace) -> WaveResult:
    d_confirm = pd.Timestamp(row["d_confirm_date"])
    start = start_index(candles, d_confirm)
    if start >= len(candles) - 2:
        return no_entry("no_forward_candles")
    end_signal = min(len(candles) - 2, start + max(1, args.entry_scan_minutes))
    directions = directions_for_row(row, args)
    for signal_idx in range(start, end_signal + 1):
        triggered: list[tuple[float, str]] = []
        for direction in directions:
            if is_wave_trigger(candles, signal_idx, direction, args, loose=False):
                triggered.append((trigger_score(candles, signal_idx, direction), direction))
        if not triggered:
            continue
        triggered.sort(reverse=True)
        direction = triggered[0][1]
        tick_size = tick_size_for(row.get("symbol"), row.get("root_symbol"))
        result = simulate_from_entry(candles, d_confirm, signal_idx + 1, signal_idx, direction, tick_size, args)
        if result.outcome != "no_entry":
            return result
    return no_entry("no_wave_trigger")


def plausible_oracle_candidates(
    row: pd.Series,
    candles: pd.DataFrame,
    args: argparse.Namespace,
) -> list[tuple[float, int, str]]:
    d_confirm = pd.Timestamp(row["d_confirm_date"])
    start = start_index(candles, d_confirm)
    if start >= len(candles) - 2:
        return []
    end_signal = min(len(candles) - 2, start + max(1, args.entry_scan_minutes))
    candidates: list[tuple[float, int, str]] = []
    directions = directions_for_row(row, args)
    for signal_idx in range(start, end_signal + 1):
        for direction in directions:
            if is_wave_trigger(candles, signal_idx, direction, args, loose=True):
                score = trigger_score(candles, signal_idx, direction)
                candidates.append((score, signal_idx + 1, direction))
    candidates.sort(reverse=True)
    deduped: list[tuple[float, int, str]] = []
    seen: set[tuple[int, str]] = set()
    for score, entry_idx, direction in candidates:
        key = (entry_idx, direction)
        if key in seen:
            continue
        seen.add(key)
        deduped.append((score, entry_idx, direction))
        if len(deduped) >= args.oracle_max_candidates:
            break
    return deduped


def evaluate_oracle(row: pd.Series, candles: pd.DataFrame, args: argparse.Namespace) -> WaveResult:
    d_confirm = pd.Timestamp(row["d_confirm_date"])
    tick_size = tick_size_for(row.get("symbol"), row.get("root_symbol"))
    candidates = plausible_oracle_candidates(row, candles, args)
    best: WaveResult | None = None
    for score, entry_idx, direction in candidates:
        result = simulate_from_entry(candles, d_confirm, entry_idx, entry_idx - 1, direction, tick_size, args)
        result = WaveResult(**{**result.__dict__, "trigger_score": score, "candidate_count": len(candidates)})
        if result.outcome == "no_entry":
            continue
        if best is None or result.result_r > best.result_r:
            best = result
    if best is None:
        return no_entry("no_oracle_candidate", candidate_count=len(candidates))
    return best


def delete_existing(conn, run_id: str, source_run_id: str, result_table: str) -> None:
    with conn.cursor() as cur:
        for table in [
            *TRADE_DERIVED_TABLES,
            "ai_stage1_multi_valid_eval_selected",
            "ai_stage1_multi_valid_eval_filter_summary",
            "ai_stage1_multi_valid_eval_slot_summary",
            "ai_stage1_multi_valid_eval_runs",
        ]:
            cur.execute(f"DELETE FROM {table} WHERE multi_valid_eval_run_id = %s", (run_id,))
        cur.execute("DELETE FROM ai_wave_rider_trade_diagnostics WHERE multi_valid_eval_run_id = %s", (run_id,))
        cur.execute(f"DELETE FROM {safe_identifier(result_table)} WHERE run_id = %s", (source_run_id,))


def insert_run_record(
    conn,
    source_run: dict[str, Any],
    run_id: str,
    source_run_id: str,
    result_table: str,
    year: int,
    mode: str,
    args: argparse.Namespace,
) -> None:
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
                run_id,
                source_run_id,
                result_table,
                int(source_run.get("train_start_year") or 0),
                int(source_run.get("train_end_year") or 0),
                year,
                0,
                f"wave_rider_{mode}",
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
                float(args.slippage_entry_ticks),
                float(args.slippage_exit_ticks),
                0.0,
                float(args.slippage_exit_ticks),
                0.0,
                float(args.min_risk_ticks),
                f"wave_rider/{mode}/ema8_21_breakout_trail_v1",
            ),
        )


def insert_result_rows(conn, result_table: str, rows: list[dict[str, Any]]) -> None:
    if not rows:
        return
    placeholders = ", ".join(["%s"] * len(RESULT_COLUMNS))
    update_columns = [column for column in RESULT_COLUMNS if column not in {"run_id", "template_uid", "setup_id"}]
    updates = ", ".join([f"{column}=VALUES({column})" for column in update_columns])
    sql = f"""
        INSERT INTO {safe_identifier(result_table)} ({", ".join(RESULT_COLUMNS)})
        VALUES ({placeholders})
        ON DUPLICATE KEY UPDATE {updates}
    """
    values = [tuple(clean(row.get(column)) for column in RESULT_COLUMNS) for row in rows]
    with conn.cursor() as cur:
        cur.executemany(sql, values)


def insert_selected_rows(conn, run_id: str, rows: list[dict[str, Any]]) -> None:
    if not rows:
        return
    placeholders = ", ".join(["%s"] * (len(SELECTED_COLUMNS) + 1))
    sql = f"""
        INSERT INTO ai_stage1_multi_valid_eval_selected (
            multi_valid_eval_run_id, {", ".join(SELECTED_COLUMNS)}
        )
        VALUES ({placeholders})
    """
    values = [(run_id, *[clean(row.get(column)) for column in SELECTED_COLUMNS]) for row in rows]
    with conn.cursor() as cur:
        cur.executemany(sql, values)


def insert_filter_summaries(conn, run_id: str, rows: list[dict[str, Any]], metric_name: str) -> dict[str, Any]:
    result = pd.to_numeric(pd.Series([row.get("actual_result_r") for row in rows]), errors="coerce").fillna(0.0)
    outcomes = pd.Series([row.get("actual_outcome") for row in rows]).fillna("")
    count = int(len(result))
    wins = int((outcomes == "pass").sum())
    losses = int((outcomes == "fail").sum())
    no_entries = int((outcomes == "no_entry").sum())
    total = float(result.sum()) if count else 0.0
    avg = float(result.mean()) if count else 0.0
    win_rate = wins / count if count else 0.0
    with conn.cursor() as cur:
        cur.execute(
            """
            INSERT INTO ai_stage1_multi_valid_eval_slot_summary (
                multi_valid_eval_run_id, valid_sample_slot, metric_name,
                setup_count, wins, losses, win_rate, avg_r, sum_r
            )
            VALUES (%s, NULL, %s, %s, %s, %s, %s, %s, %s)
            """,
            (run_id, metric_name, count, wins, losses, win_rate, avg, total),
        )
        cur.execute(
            """
            INSERT INTO ai_stage1_multi_valid_eval_filter_summary (
                multi_valid_eval_run_id, scope_name, rule_name,
                score_quantile, margin_quantile, score_threshold, margin_threshold,
                selected_trades, wins, losses, win_rate, avg_r, sum_r
            )
            VALUES (%s, 'combined', %s, NULL, NULL, NULL, NULL, %s, %s, %s, %s, %s, %s)
            """,
            (run_id, metric_name, count, wins, losses, win_rate, avg, total),
        )
    return {
        "trades": count,
        "wins": wins,
        "losses": losses,
        "no_entries": no_entries,
        "win_rate": win_rate,
        "avg_r": avg,
        "sum_r": total,
    }


def insert_diagnostics(conn, rows: list[dict[str, Any]]) -> None:
    if not rows:
        return
    columns = [
        "multi_valid_eval_run_id",
        "source_ai_run_id",
        "source_run_id",
        "setup_id",
        "template_uid",
        "mode",
        "symbol",
        "root_symbol",
        "d_confirm_date",
        "source_trade_direction",
        "selected_trade_direction",
        "entry_delay_minutes",
        "hold_minutes",
        "signal_index",
        "entry_index",
        "exit_index",
        "risk_ticks",
        "tick_size",
        "raw_result_r",
        "slipped_result_r",
        "mfe_r",
        "mae_r",
        "trigger_score",
        "candidate_count",
        "no_entry_reason",
        "exit_reason",
        "formula",
    ]
    placeholders = ", ".join(["%s"] * len(columns))
    updates = ", ".join(
        [f"{column}=VALUES({column})" for column in columns if column not in {"multi_valid_eval_run_id", "setup_id", "template_uid"}]
        + ["updated_at=CURRENT_TIMESTAMP"]
    )
    sql = f"""
        INSERT INTO ai_wave_rider_trade_diagnostics ({", ".join(columns)})
        VALUES ({placeholders})
        ON DUPLICATE KEY UPDATE {updates}
    """
    values = [tuple(clean(row.get(column)) for column in columns) for row in rows]
    with conn.cursor() as cur:
        cur.executemany(sql, values)


def result_row(source_run_id: str, template_uid: str, row: pd.Series, wave: WaveResult) -> dict[str, Any]:
    return {
        "run_id": source_run_id,
        "template_uid": template_uid,
        "setup_id": row.get("setup_id"),
        "pattern_id": row.get("pattern_id"),
        "pattern_group_id": row.get("pattern_group_id"),
        "event_id": row.get("event_id"),
        "event_rank": row.get("event_rank"),
        "event_sister_count": row.get("event_sister_count"),
        "event_decision_date": row.get("d_confirm_date"),
        "event_candidate_count": 1,
        "event_live_candidate_count": 1,
        "pattern_family_key": row.get("pattern_family_key"),
        "symbol": row.get("symbol"),
        "market": row.get("market"),
        "d_date": row.get("d_date"),
        "d_confirm_date": row.get("d_confirm_date"),
        "evaluation_order": 1,
        "was_created_for_setup": 1,
        "outcome": wave.outcome,
        "exit_reason": wave.exit_reason,
        "result_r": wave.result_r,
        "entry_date": wave.entry_date,
        "exit_date": wave.exit_date,
        "entry_price": wave.entry_price,
        "stop_price": wave.stop_price,
        "target_price": wave.target_price,
        "exit_price": wave.exit_price,
        "risk_points": wave.risk_points,
        "trade_direction": wave.trade_direction,
    }


def selected_row(template_uid: str, template_name: str, row: pd.Series, wave: WaveResult) -> dict[str, Any]:
    return {
        "valid_sample_slot": 1,
        "setup_id": row.get("setup_id"),
        "pattern_id": row.get("pattern_id"),
        "pattern_group_id": row.get("pattern_group_id"),
        "symbol": row.get("symbol"),
        "market": row.get("market"),
        "pattern_family_key": row.get("pattern_family_key"),
        "d_confirm_date": row.get("d_confirm_date"),
        "template_uid": template_uid,
        "template_name": template_name,
        "model_rank": 1,
        "predicted_expected_r": row.get("source_predicted_expected_r"),
        "score_margin_top2": wave.trigger_score if wave.trigger_score is not None else row.get("source_score_margin_top2"),
        "actual_result_r": wave.result_r,
        "actual_outcome": wave.outcome,
        "oracle_template_uid": row.get("source_template_uid"),
        "oracle_result_r": row.get("source_actual_result_r"),
        "oracle_rank": row.get("source_model_rank"),
    }


def diagnostic_row(
    run_id: str,
    source_ai_run_id: str,
    source_run_id: str,
    template_uid: str,
    mode: str,
    row: pd.Series,
    wave: WaveResult,
) -> dict[str, Any]:
    return {
        "multi_valid_eval_run_id": run_id,
        "source_ai_run_id": source_ai_run_id,
        "source_run_id": source_run_id,
        "setup_id": row.get("setup_id"),
        "template_uid": template_uid,
        "mode": mode,
        "symbol": row.get("symbol"),
        "root_symbol": row.get("root_symbol") or root_symbol(row.get("symbol")),
        "d_confirm_date": row.get("d_confirm_date"),
        "source_trade_direction": normalize_direction(row.get("source_trade_direction")),
        "selected_trade_direction": wave.trade_direction,
        "entry_delay_minutes": wave.entry_delay_minutes,
        "hold_minutes": wave.hold_minutes,
        "signal_index": wave.signal_index,
        "entry_index": wave.entry_index,
        "exit_index": wave.exit_index,
        "risk_ticks": wave.risk_ticks,
        "tick_size": tick_size_for(row.get("symbol"), row.get("root_symbol")),
        "raw_result_r": wave.raw_result_r,
        "slipped_result_r": wave.result_r,
        "mfe_r": wave.mfe_r,
        "mae_r": wave.mae_r,
        "trigger_score": wave.trigger_score,
        "candidate_count": wave.candidate_count,
        "no_entry_reason": wave.no_entry_reason,
        "exit_reason": wave.exit_reason,
        "formula": "ema8_21_breakout_trail_momentum_market_slip_v1",
    }


def materialize_mode_year(
    conn,
    source_ai_run_id: str,
    source_run: dict[str, Any],
    source_rows: pd.DataFrame,
    year: int,
    mode: str,
    run_prefix: str,
    source_prefix: str,
    args: argparse.Namespace,
) -> dict[str, Any]:
    run_id = safe_run_id(run_prefix, year)
    source_run_id = safe_run_id(source_prefix, year)
    result_table = result_table_for(source_prefix, year)
    template_uid = f"tpl-{source_prefix}"
    template_name = "Wave Rider Oracle" if mode == "oracle" else "Wave Rider Rule"
    metric_name = f"wave_rider_{mode}_v1"

    ensure_result_table(conn, result_table)
    ensure_diagnostics_table(conn)

    with conn.cursor() as cur:
        cur.execute("SELECT COUNT(*) AS count FROM ai_stage1_multi_valid_eval_runs WHERE multi_valid_eval_run_id = %s", (run_id,))
        exists = int(cur.fetchone()["count"]) > 0
    if exists and not args.replace:
        raise RuntimeError(f"Run already exists. Use --replace to rebuild it: {run_id}")

    delete_existing(conn, run_id, source_run_id, result_table)

    result_rows: list[dict[str, Any]] = []
    selected_rows: list[dict[str, Any]] = []
    diagnostics: list[dict[str, Any]] = []
    processed = 0

    for _, source_row in source_rows.iterrows():
        d_confirm = pd.Timestamp(source_row["d_confirm_date"])
        start_dt = d_confirm - timedelta(minutes=args.indicator_lookback_minutes)
        end_dt = d_confirm + timedelta(minutes=args.max_forward_minutes + args.entry_scan_minutes + 10)
        candles = fetch_candles(conn, str(source_row["symbol"]), start_dt, end_dt)
        candles = prepare_candles(candles, args)
        if candles.empty:
            wave = no_entry("no_candles")
        elif mode == "oracle":
            wave = evaluate_oracle(source_row, candles, args)
        else:
            wave = evaluate_rule(source_row, candles, args)

        result_rows.append(result_row(source_run_id, template_uid, source_row, wave))
        selected_rows.append(selected_row(template_uid, template_name, source_row, wave))
        diagnostics.append(diagnostic_row(run_id, source_ai_run_id, source_run_id, template_uid, mode, source_row, wave))
        processed += 1
        if processed % 50 == 0:
            print(f"  {run_id}: evaluated {processed:,}/{len(source_rows):,}")

    insert_result_rows(conn, result_table, result_rows)
    insert_run_record(conn, source_run, run_id, source_run_id, result_table, year, mode, args)
    insert_selected_rows(conn, run_id, selected_rows)
    summary = insert_filter_summaries(conn, run_id, selected_rows, metric_name)
    insert_diagnostics(conn, diagnostics)
    conn.commit()
    return {
        "year": year,
        "mode": mode,
        "run_id": run_id,
        "source_run_id": source_run_id,
        "result_table": result_table,
        **summary,
    }


def api_refresh(run_id: str, year: int, refresh_url: str) -> None:
    try:
        import requests
    except ImportError:
        print("requests is not installed; skipping API refresh.")
        return
    try:
        response = requests.post(
            refresh_url,
            json={"ai_run_id": run_id, "valid_year": year, "refresh": True, "limit": 1, "offset": 0},
            timeout=120,
        )
        if response.status_code >= 400:
            print(f"API refresh failed for {run_id}: HTTP {response.status_code} {response.text[:200]}")
        else:
            print(f"  refreshed AI Trades tables for {run_id}")
    except Exception as error:
        print(f"API refresh skipped for {run_id}: {error}")


def main() -> int:
    args = parse_args()
    years = years_from_arg(args.years)
    modes = ["rule", "oracle"] if args.mode == "both" else [args.mode]
    conn = connect()
    summaries: list[dict[str, Any]] = []
    try:
        for year in years:
            source_ai_run_id = safe_run_id(args.source_prefix, year)
            source_run = fetch_run(conn, source_ai_run_id)
            rows = fetch_source_rows(conn, source_ai_run_id, args.limit)
            if rows.empty:
                print(f"{source_ai_run_id}: no source rows")
                continue
            print(f"{source_ai_run_id}: {len(rows):,} source pattern opportunities")
            for mode in modes:
                run_prefix = args.oracle_run_prefix if mode == "oracle" else args.rule_run_prefix
                source_prefix = args.oracle_source_prefix if mode == "oracle" else args.rule_source_prefix
                summaries.append(
                    materialize_mode_year(
                        conn,
                        source_ai_run_id,
                        source_run,
                        rows,
                        year,
                        mode,
                        run_prefix,
                        source_prefix,
                        args,
                    )
                )
    except Exception:
        conn.rollback()
        raise
    finally:
        conn.close()

    print("Materialized wave rider runs:")
    for summary in summaries:
        print(
            f"  {summary['year']} {summary['mode']}: {summary['run_id']} -> "
            f"{summary['trades']:,} rows, {summary['wins']:,} wins / {summary['losses']:,} losses / "
            f"{summary['no_entries']:,} no-entry, {summary['win_rate'] * 100:.2f}% win, "
            f"{summary['avg_r']:.3f}R avg, {summary['sum_r']:.1f}R sum"
        )

    if not args.skip_api_refresh:
        for summary in summaries:
            api_refresh(summary["run_id"], int(summary["year"]), args.refresh_url)
    return 0


if __name__ == "__main__":
    sys.exit(main())
