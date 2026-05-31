#!/usr/bin/env python3
"""
Train a candle-level wave scanner with no pattern trigger.

This is a separate research lane from pattern-triggered AI trades. It scans
market candles directly, creates wave-style candidate entries from chart
conditions, trains CatBoost to approve/reject them, and stores the selected
2026 trades in DB-backed tables for inspection.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import sys
from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd

try:
    from catboost import CatBoostRegressor, Pool
except ImportError as exc:  # pragma: no cover - runtime environment message
    raise SystemExit("CatBoost is required. Run with .venv_ai\\Scripts\\python.exe") from exc


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_wave_rider_research as wave


CANDIDATE_TABLE = "ai_candle_wave_candidate_rows"
RUN_TABLE = "ai_candle_wave_model_runs"
SELECTED_TABLE = "ai_candle_wave_selected_trades"
FORMULA_TEMPLATE = "candle_wave_{timeframe}_mtf_ema8_21_breakout_trail_market_slip_v2"

TIMEFRAME_TABLES = {
    "1m": ("futures_contract_1m_candles", 1),
    "2m": ("futures_contract_2m_candles", 2),
    "3m": ("futures_contract_3m_candles", 3),
    "4m": ("futures_contract_4m_candles", 4),
    "5m": ("futures_contract_5m_candles", 5),
    "6m": ("futures_contract_6m_candles", 6),
    "7m": ("futures_contract_7m_candles", 7),
    "8m": ("futures_contract_8m_candles", 8),
    "9m": ("futures_contract_9m_candles", 9),
    "10m": ("futures_contract_10m_candles", 10),
    "11m": ("futures_contract_11m_candles", 11),
    "12m": ("futures_contract_12m_candles", 12),
    "13m": ("futures_contract_13m_candles", 13),
    "14m": ("futures_contract_14m_candles", 14),
    "15m": ("futures_contract_15m_candles", 15),
    "30m": ("futures_contract_30m_candles", 30),
    "1h": ("futures_contract_1h_candles", 60),
    "4h": ("futures_contract_4h_candles", 240),
    "12h": ("futures_contract_12h_candles", 720),
    "1d": ("futures_contract_1d_candles", 1440),
}

CONTEXT_TIMEFRAME_KEYS = ["3m", "5m", "15m", "30m", "1h", "4h", "12h", "1d"]
DEFAULT_CONTEXT_TIMEFRAMES = "5m,15m,1h,4h"

CONTEXT_CAT_METRICS = ["trend_label", "directional_trend"]
CONTEXT_NUM_METRICS = [
    "atr_ticks",
    "ema_gap_atr",
    "ema_fast_slope_atr",
    "relative_volume",
    "compression",
    "range_position_48",
    "distance_to_high_48_atr",
    "distance_to_low_48_atr",
]

CONTEXT_CAT_FEATURES = [
    f"ctx_{timeframe}_{metric}"
    for timeframe in CONTEXT_TIMEFRAME_KEYS
    for metric in CONTEXT_CAT_METRICS
]

CONTEXT_NUM_FEATURES = [
    f"ctx_{timeframe}_{metric}"
    for timeframe in CONTEXT_TIMEFRAME_KEYS
    for metric in CONTEXT_NUM_METRICS
]

CAT_FEATURES = [
    "root_symbol",
    "direction",
    "signal_day_of_week",
    "signal_month",
    "signal_hour_bucket",
    "trend_label",
    "directional_trend",
    "volume_bucket",
    "compression_bucket",
    *CONTEXT_CAT_FEATURES,
]

NUM_FEATURES = [
    "signal_hour",
    "signal_score",
    "mtf_entry_aligned_count",
    "risk_ticks",
    "atr_ticks",
    "atr_pct",
    "prior_range_ticks",
    "ema_gap_atr",
    "ema_fast_slope_atr",
    "breakout_atr",
    "relative_volume",
    "compression",
    "close_location_prior_range",
    "ret_3_atr",
    "ret_6_atr",
    "ret_12_atr",
    "distance_to_high_48_atr",
    "distance_to_low_48_atr",
    "range_position_48",
    "body_atr",
    "upper_wick_atr",
    "lower_wick_atr",
    *CONTEXT_NUM_FEATURES,
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--timeframe", choices=sorted(TIMEFRAME_TABLES), default="5m")
    parser.add_argument(
        "--context-timeframes",
        default=DEFAULT_CONTEXT_TIMEFRAMES,
        help="Closed candle context timeframes attached to each candidate.",
    )
    parser.add_argument("--context-lookback-bars", type=int, default=80)
    parser.add_argument("--candidate-years", default="2021,2022,2023,2024,2025,2026")
    parser.add_argument("--train-years", default="2021,2022,2023,2024,2025")
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--run-prefix", default="aicw-wave-scan-v1")
    parser.add_argument("--replace-candidates", action="store_true")
    parser.add_argument("--replace-run", action="store_true")
    parser.add_argument("--skip-candidate-build", action="store_true")
    parser.add_argument("--build-only", action="store_true")
    parser.add_argument("--threshold", type=float, default=None)
    parser.add_argument("--min-threshold-trades", type=int, default=80)
    parser.add_argument("--candidate-min-gap-bars", type=int, default=6)
    parser.add_argument("--cooldown-minutes", type=int, default=0)
    parser.add_argument("--roots", default="", help="Optional comma-separated root symbols for debugging.")
    parser.add_argument(
        "--allowed-roots",
        default="",
        help="Optional comma-separated root whitelist applied after model scoring.",
    )
    parser.add_argument(
        "--max-root-trades-per-day",
        type=int,
        default=0,
        help="Optional selected-trade throttle per root symbol per UTC day.",
    )
    parser.add_argument(
        "--max-energy-trades-per-day",
        type=int,
        default=0,
        help="Optional selected-trade throttle for the HO/RB/QM/NG energy cluster per UTC day.",
    )
    parser.add_argument(
        "--energy-extra-slot-start",
        type=int,
        default=0,
        help="Require a stronger model score after this many same-day energy trades; 0 disables it.",
    )
    parser.add_argument(
        "--energy-extra-slot-min-score",
        type=float,
        default=0.0,
        help="Minimum predicted R for energy trades selected after --energy-extra-slot-start.",
    )
    parser.add_argument(
        "--loss-brake-r",
        type=float,
        default=0.0,
        help="Pause a scope for the rest of the UTC day after selected trades lose this many R.",
    )
    parser.add_argument(
        "--loss-brake-scope",
        choices=["root", "risk-group", "all"],
        default="root",
        help="Scope used by --loss-brake-r.",
    )
    parser.add_argument(
        "--loss-brake-min-trades",
        type=int,
        default=1,
        help="Minimum selected trades in the scope/day before the loss brake can pause it.",
    )
    parser.add_argument(
        "--liquidity-lookback-hours",
        type=float,
        default=0.0,
        help="Optional pre-entry same-contract candle lookback window. 0 disables liquidity gating.",
    )
    parser.add_argument(
        "--min-liquidity-bars",
        type=int,
        default=0,
        help="Minimum same-contract candles in the liquidity lookback window.",
    )
    parser.add_argument(
        "--min-liquidity-volume",
        type=float,
        default=0.0,
        help="Minimum same-contract volume in the liquidity lookback window.",
    )
    parser.add_argument(
        "--liquidity-gate-train",
        action="store_true",
        help="Also apply the liquidity gate to training rows. By default it gates selection/validation only.",
    )
    parser.add_argument("--limit-symbols", type=int, default=0, help="Optional symbol limit per year for debugging.")
    parser.add_argument("--iterations", type=int, default=800)
    parser.add_argument("--depth", type=int, default=6)
    parser.add_argument("--learning-rate", type=float, default=0.035)
    parser.add_argument("--l2-leaf-reg", type=float, default=10.0)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--random-strength", type=float, default=None)
    parser.add_argument("--bootstrap-type", default="", help="Optional CatBoost bootstrap_type override, e.g. No.")
    parser.add_argument("--target-clip-min", type=float, default=None)
    parser.add_argument("--target-clip-max", type=float, default=None)
    parser.add_argument(
        "--threshold-score-cap-r",
        type=float,
        default=None,
        help="Choose the threshold by capped validation R while still reporting uncapped R.",
    )

    # Wave-scanner knobs. These names intentionally match ai_wave_rider_research
    # where possible so we can reuse its live-safe simulator.
    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=240.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    parser.add_argument("--death-bars", type=int, default=4)
    parser.add_argument("--min-hold-bars", type=int, default=4)
    parser.add_argument("--profit-lock-trigger-r", type=float, default=0.0)
    parser.add_argument("--profit-lock-giveback-r", type=float, default=1.5)
    parser.add_argument("--profit-lock-min-r", type=float, default=0.25)
    parser.add_argument("--max-forward-bars", type=int, default=576)
    parser.add_argument("--time-exit-bars", type=int, default=0)
    parser.add_argument("--slippage-entry-ticks", type=float, default=3.0)
    parser.add_argument("--slippage-exit-ticks", type=float, default=3.0)
    parser.add_argument("--loose-triggers", action="store_true")
    parser.add_argument(
        "--mtf-entry-min-aligned",
        type=int,
        default=0,
        help="Require this many higher context timeframes aligned with the trade before entry.",
    )
    parser.add_argument(
        "--mtf-exit-governor",
        action="store_true",
        help="Let aligned higher timeframes veto signal-timeframe momentum-fade exits.",
    )
    parser.add_argument(
        "--mtf-exit-min-aligned",
        type=int,
        default=1,
        help="Higher context timeframe alignment count needed to veto a momentum-fade exit.",
    )
    parser.add_argument(
        "--mtf-exit-mode",
        choices=["hard-veto", "delay", "tighten", "delay-tighten"],
        default="hard-veto",
        help="How higher timeframe support modifies a signal-timeframe fade exit.",
    )
    parser.add_argument(
        "--mtf-exit-extra-death-bars",
        type=int,
        default=1,
        help="Extra fade candles required when higher timeframes still support the trade.",
    )
    parser.add_argument(
        "--mtf-exit-tighten-lookback-bars",
        type=int,
        default=3,
        help="Recent signal candles used to tighten the stop when higher timeframes support holding.",
    )
    parser.add_argument(
        "--mtf-exit-tighten-pad-ticks",
        type=float,
        default=1.0,
        help="Ticks beyond the recent swing used by the MTF tighten-stop mode.",
    )
    return parser.parse_args()


def parse_list(value: str) -> list[str]:
    return [part.strip() for part in str(value or "").split(",") if part.strip()]


def parse_years(value: str) -> list[int]:
    years = [int(part) for part in parse_list(value)]
    if not years:
        raise ValueError("At least one year is required.")
    return years


def configure_wave_args(args: argparse.Namespace) -> None:
    args.formula = formula_for_args(args)
    args.max_forward_minutes = int(args.max_forward_bars)
    args.time_exit_minutes = int(args.time_exit_bars)
    args.entry_scan_minutes = 0
    args.indicator_lookback_minutes = 0
    args.directions = "both"


def run_id_for(args: argparse.Namespace) -> str:
    run_id = f"{args.run_prefix}-{args.timeframe}-{args.valid_year}"
    if len(run_id) > 64:
        raise ValueError(f"Run id too long: {run_id}")
    return run_id


def formula_for(timeframe: str) -> str:
    return FORMULA_TEMPLATE.format(timeframe=timeframe)


def formula_for_args(args: argparse.Namespace) -> str:
    formula = formula_for(args.timeframe)
    suffixes: list[str] = []
    if int(getattr(args, "mtf_entry_min_aligned", 0) or 0) > 0:
        suffixes.append(f"eg{int(args.mtf_entry_min_aligned)}")
    if bool(getattr(args, "mtf_exit_governor", False)):
        min_aligned = int(getattr(args, "mtf_exit_min_aligned", 1) or 1)
        mode = str(getattr(args, "mtf_exit_mode", "hard-veto") or "hard-veto")
        if mode == "hard-veto":
            suffixes.append(f"xgov{min_aligned}")
        elif mode == "delay":
            suffixes.append(f"xdel{min_aligned}_{int(getattr(args, 'mtf_exit_extra_death_bars', 1) or 1)}")
        elif mode == "tighten":
            suffixes.append(f"xtight{min_aligned}")
        elif mode == "delay-tighten":
            suffixes.append(f"xdt{min_aligned}_{int(getattr(args, 'mtf_exit_extra_death_bars', 1) or 1)}")
        else:
            raise ValueError(f"Unsupported mtf exit mode: {mode}")
    if suffixes:
        formula = f"{formula}_{'_'.join(suffixes)}"
    if len(formula) > 128:
        raise ValueError(f"Formula is too long for DB column: {formula}")
    return formula


def safe_identifier(value: str) -> str:
    if not re.fullmatch(r"[A-Za-z0-9_]+", value):
        raise ValueError(f"Unsafe SQL identifier: {value!r}")
    return value


def table_for_timeframe(timeframe: str) -> tuple[str, int]:
    if timeframe not in TIMEFRAME_TABLES:
        raise ValueError(f"Unsupported timeframe: {timeframe}")
    return TIMEFRAME_TABLES[timeframe]


def requested_context_timeframes(args: argparse.Namespace) -> list[str]:
    result: list[str] = []
    for value in parse_list(args.context_timeframes):
        key = value.lower().replace(" ", "")
        if key not in TIMEFRAME_TABLES:
            raise ValueError(f"Unsupported context timeframe: {value}")
        if key not in CONTEXT_TIMEFRAME_KEYS:
            raise ValueError(f"Context timeframe {value} is not in the modeled context set.")
        if key not in result:
            result.append(key)
    return result


def higher_context_timeframes(args: argparse.Namespace) -> list[str]:
    signal_minutes = table_for_timeframe(args.timeframe)[1]
    return [
        timeframe
        for timeframe in requested_context_timeframes(args)
        if table_for_timeframe(timeframe)[1] > signal_minutes
    ]


def context_column_definitions() -> dict[str, str]:
    definitions: dict[str, str] = {}
    for column in CONTEXT_CAT_FEATURES:
        definitions[column] = "VARCHAR(16) NULL"
    for column in CONTEXT_NUM_FEATURES:
        definitions[column] = "DOUBLE NULL"
    return definitions


def model_feature_column_definitions() -> dict[str, str]:
    definitions: dict[str, str] = {}
    for column in CAT_FEATURES:
        definitions[column] = "VARCHAR(64) NULL"
    for column in NUM_FEATURES:
        definitions[column] = "DOUBLE NULL"
    return definitions


def counterfactual_column_definitions() -> dict[str, str]:
    return {
        "reverse_direction": "VARCHAR(16) NULL",
        "reverse_outcome": "VARCHAR(32) NULL",
        "reverse_exit_reason": "VARCHAR(64) NULL",
        "reverse_no_entry_reason": "VARCHAR(64) NULL",
        "reverse_entry_date": "DATETIME NULL",
        "reverse_exit_date": "DATETIME NULL",
        "reverse_entry_price": "DOUBLE NULL",
        "reverse_stop_price": "DOUBLE NULL",
        "reverse_exit_price": "DOUBLE NULL",
        "reverse_risk_points": "DOUBLE NULL",
        "reverse_result_r": "DOUBLE NULL",
        "reverse_raw_result_r": "DOUBLE NULL",
        "reverse_mfe_r": "DOUBLE NULL",
        "reverse_mae_r": "DOUBLE NULL",
        "reverse_hold_minutes": "INT NULL",
        "reverse_risk_ticks": "DOUBLE NULL",
        "reverse_edge_r": "DOUBLE NULL",
        "best_action": "VARCHAR(16) NULL",
    }


def counterfactual_column_sql(indent: str = "                ") -> str:
    definitions = counterfactual_column_definitions()
    return "\n".join(f"{indent}{column} {definition}," for column, definition in definitions.items())


def context_column_sql(indent: str = "                ") -> str:
    definitions = context_column_definitions()
    return "\n".join(f"{indent}{column} {definition}," for column, definition in definitions.items())


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


def ensure_tables(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            f"""
            CREATE TABLE IF NOT EXISTS {CANDIDATE_TABLE} (
                formula VARCHAR(128) NOT NULL,
                timeframe VARCHAR(16) NOT NULL,
                valid_year INT NOT NULL,
                candidate_uid VARCHAR(40) NOT NULL,
                root_symbol VARCHAR(32) NOT NULL,
                symbol VARCHAR(64) NOT NULL,
                signal_date DATETIME NOT NULL,
                entry_date DATETIME NOT NULL,
                exit_date DATETIME NULL,
                direction VARCHAR(16) NOT NULL,
                outcome VARCHAR(32) NOT NULL,
                exit_reason VARCHAR(64) NULL,
                entry_price DOUBLE NULL,
                stop_price DOUBLE NULL,
                exit_price DOUBLE NULL,
                risk_points DOUBLE NULL,
                result_r DOUBLE NOT NULL DEFAULT 0,
                raw_result_r DOUBLE NULL,
                mfe_r DOUBLE NULL,
                mae_r DOUBLE NULL,
                hold_minutes INT NULL,
                risk_ticks DOUBLE NULL,
                tick_size DOUBLE NULL,
                signal_score DOUBLE NULL,
{counterfactual_column_sql()}
                signal_hour INT NULL,
                signal_day_of_week VARCHAR(16) NULL,
                signal_month VARCHAR(16) NULL,
                signal_hour_bucket VARCHAR(16) NULL,
                trend_label VARCHAR(16) NULL,
                directional_trend VARCHAR(16) NULL,
                volume_bucket VARCHAR(16) NULL,
                compression_bucket VARCHAR(16) NULL,
                atr_ticks DOUBLE NULL,
                atr_pct DOUBLE NULL,
                prior_range_ticks DOUBLE NULL,
                ema_gap_atr DOUBLE NULL,
                ema_fast_slope_atr DOUBLE NULL,
                breakout_atr DOUBLE NULL,
                relative_volume DOUBLE NULL,
                compression DOUBLE NULL,
                close_location_prior_range DOUBLE NULL,
                ret_3_atr DOUBLE NULL,
                ret_6_atr DOUBLE NULL,
                ret_12_atr DOUBLE NULL,
                distance_to_high_48_atr DOUBLE NULL,
                distance_to_low_48_atr DOUBLE NULL,
                range_position_48 DOUBLE NULL,
                body_atr DOUBLE NULL,
                upper_wick_atr DOUBLE NULL,
                lower_wick_atr DOUBLE NULL,
{context_column_sql()}
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                PRIMARY KEY (formula, candidate_uid),
                INDEX idx_candle_wave_year_result (formula, valid_year, result_r),
                INDEX idx_candle_wave_symbol_time (symbol, signal_date),
                INDEX idx_candle_wave_root_year (root_symbol, valid_year)
            )
            """
        )
        cur.execute(
            f"""
            CREATE TABLE IF NOT EXISTS {RUN_TABLE} (
                model_run_id VARCHAR(64) NOT NULL PRIMARY KEY,
                formula VARCHAR(128) NOT NULL,
                timeframe VARCHAR(16) NOT NULL,
                train_years VARCHAR(64) NOT NULL,
                threshold_year INT NOT NULL,
                valid_year INT NOT NULL,
                threshold DOUBLE NULL,
                candidate_rows INT NOT NULL DEFAULT 0,
                selected_trades INT NOT NULL DEFAULT 0,
                wins INT NOT NULL DEFAULT 0,
                losses INT NOT NULL DEFAULT 0,
                win_rate DOUBLE NOT NULL DEFAULT 0,
                avg_r DOUBLE NOT NULL DEFAULT 0,
                sum_r DOUBLE NOT NULL DEFAULT 0,
                max_drawdown_r DOUBLE NOT NULL DEFAULT 0,
                model_path VARCHAR(255) NULL,
                metadata_json JSON NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
            )
            """
        )
        cur.execute(
            f"""
            CREATE TABLE IF NOT EXISTS {SELECTED_TABLE} (
                model_run_id VARCHAR(64) NOT NULL,
                selected_index INT NOT NULL,
                candidate_uid VARCHAR(40) NOT NULL,
                predicted_r DOUBLE NULL,
                formula VARCHAR(128) NOT NULL,
                timeframe VARCHAR(16) NOT NULL,
                valid_year INT NOT NULL,
                root_symbol VARCHAR(32) NOT NULL,
                symbol VARCHAR(64) NOT NULL,
                signal_date DATETIME NOT NULL,
                entry_date DATETIME NOT NULL,
                exit_date DATETIME NULL,
                direction VARCHAR(16) NOT NULL,
                outcome VARCHAR(32) NOT NULL,
                exit_reason VARCHAR(64) NULL,
                entry_price DOUBLE NULL,
                stop_price DOUBLE NULL,
                exit_price DOUBLE NULL,
                risk_points DOUBLE NULL,
                result_r DOUBLE NOT NULL DEFAULT 0,
                raw_result_r DOUBLE NULL,
                mfe_r DOUBLE NULL,
                mae_r DOUBLE NULL,
                hold_minutes INT NULL,
                risk_ticks DOUBLE NULL,
                tick_size DOUBLE NULL,
                signal_score DOUBLE NULL,
{counterfactual_column_sql()}
{context_column_sql()}
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (model_run_id, candidate_uid),
                INDEX idx_candle_wave_selected_time (model_run_id, entry_date),
                INDEX idx_candle_wave_selected_result (model_run_id, result_r)
            )
            """
        )
        ensure_missing_columns(cur, CANDIDATE_TABLE, model_feature_column_definitions())
        ensure_missing_columns(cur, SELECTED_TABLE, model_feature_column_definitions())
        ensure_missing_columns(cur, CANDIDATE_TABLE, counterfactual_column_definitions())
        ensure_missing_columns(cur, SELECTED_TABLE, counterfactual_column_definitions())
    conn.commit()


def ensure_missing_columns(cur, table_name: str, definitions: dict[str, str]) -> None:
    cur.execute(f"SHOW COLUMNS FROM {safe_identifier(table_name)}")
    existing = {str(row["Field"]) for row in cur.fetchall()}
    for column, definition in definitions.items():
        if column in existing:
            continue
        cur.execute(f"ALTER TABLE {safe_identifier(table_name)} ADD COLUMN {column} {definition}")


def delete_candidates(conn, timeframe: str, years: list[int], formula: str | None = None) -> None:
    if not years:
        return
    placeholders = ",".join(["%s"] * len(years))
    with conn.cursor() as cur:
        cur.execute(
            f"DELETE FROM {CANDIDATE_TABLE} WHERE formula = %s AND timeframe = %s AND valid_year IN ({placeholders})",
            [formula or formula_for(timeframe), timeframe, *years],
        )
    conn.commit()


def delete_run(conn, run_id: str) -> None:
    with conn.cursor() as cur:
        cur.execute(f"DELETE FROM {SELECTED_TABLE} WHERE model_run_id = %s", (run_id,))
        cur.execute(f"DELETE FROM {RUN_TABLE} WHERE model_run_id = %s", (run_id,))
    conn.commit()


def fetch_year_candles(conn, year: int, args: argparse.Namespace) -> pd.DataFrame:
    table_name, timeframe_minutes = table_for_timeframe(args.timeframe)
    table = safe_identifier(table_name)
    start = pd.Timestamp(year=year, month=1, day=1) - pd.Timedelta(days=5)
    end = pd.Timestamp(year=year + 1, month=1, day=1) + pd.Timedelta(minutes=args.max_forward_bars * timeframe_minutes + 60)
    roots = [root.upper() for root in parse_list(args.roots)]
    root_sql = ""
    params: list[Any] = [start.to_pydatetime(), end.to_pydatetime()]
    if roots:
        root_sql = "AND root_symbol IN (" + ",".join(["%s"] * len(roots)) + ")"
        params.extend(roots)
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT
                root_symbol,
                symbol,
                ts_utc,
                CAST(open AS DOUBLE) AS open,
                CAST(high AS DOUBLE) AS high,
                CAST(low AS DOUBLE) AS low,
                CAST(close AS DOUBLE) AS close,
                CAST(volume AS DOUBLE) AS volume
            FROM {table}
            WHERE ts_utc >= %s
              AND ts_utc < %s
              {root_sql}
            ORDER BY root_symbol, symbol, ts_utc
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
    return frame.dropna(subset=["root_symbol", "symbol", "ts_utc", "open", "high", "low", "close"])


def enrich_candles(candles: pd.DataFrame, args: argparse.Namespace) -> pd.DataFrame:
    work = wave.prepare_candles(candles, args).copy()
    prev_close = work["close"].shift(1)
    work["ret_3"] = work["close"].pct_change(3)
    work["ret_6"] = work["close"].pct_change(6)
    work["ret_12"] = work["close"].pct_change(12)
    work["prior_high_48"] = work["high"].shift(1).rolling(48, min_periods=12).max()
    work["prior_low_48"] = work["low"].shift(1).rolling(48, min_periods=12).min()
    work["body"] = (work["close"] - work["open"]).abs()
    work["upper_wick"] = work["high"] - work[["open", "close"]].max(axis=1)
    work["lower_wick"] = work[["open", "close"]].min(axis=1) - work["low"]
    work["gap_from_prior_close"] = (work["open"] - prev_close).abs()
    return work


def fetch_context_candles(
    conn,
    symbol: str,
    timeframe: str,
    start_dt: pd.Timestamp,
    end_dt: pd.Timestamp,
    args: argparse.Namespace,
) -> pd.DataFrame:
    table_name, minutes = table_for_timeframe(timeframe)
    table = safe_identifier(table_name)
    lookback_minutes = max(1, minutes * max(1, int(args.context_lookback_bars)))
    start = start_dt - pd.Timedelta(minutes=lookback_minutes)
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
              AND ts_utc >= %s
              AND ts_utc <= %s
            ORDER BY ts_utc ASC
            """,
            (symbol, start.to_pydatetime(), end_dt.to_pydatetime()),
        )
        rows = cur.fetchall()
    frame = pd.DataFrame(rows)
    if frame.empty:
        return frame
    frame["ts_utc"] = pd.to_datetime(frame["ts_utc"], errors="coerce")
    for col in ["open", "high", "low", "close", "volume"]:
        frame[col] = pd.to_numeric(frame[col], errors="coerce")
    frame = frame.dropna(subset=["ts_utc", "open", "high", "low", "close"])
    return enrich_candles(frame.sort_values("ts_utc").reset_index(drop=True), args)


def fetch_symbol_contexts(
    conn,
    symbol: str,
    raw_group: pd.DataFrame,
    args: argparse.Namespace,
) -> dict[str, pd.DataFrame]:
    context_frames: dict[str, pd.DataFrame] = {}
    context_timeframes = requested_context_timeframes(args)
    if not context_timeframes:
        return context_frames
    start_dt = pd.Timestamp(raw_group["ts_utc"].min())
    end_dt = pd.Timestamp(raw_group["ts_utc"].max()) + pd.Timedelta(
        minutes=table_for_timeframe(args.timeframe)[1] * max(2, int(args.max_forward_bars) + 2)
    )
    for timeframe in context_timeframes:
        context_frames[timeframe] = fetch_context_candles(conn, symbol, timeframe, start_dt, end_dt, args)
    return context_frames


def blank_context_features() -> dict[str, Any]:
    values: dict[str, Any] = {}
    for column in CONTEXT_CAT_FEATURES:
        values[column] = "unknown"
    for column in CONTEXT_NUM_FEATURES:
        values[column] = None
    return values


def context_row_at(
    context: pd.DataFrame,
    signal_ts: pd.Timestamp,
    signal_minutes: int,
    context_minutes: int,
) -> pd.Series | None:
    if context.empty:
        return None
    available_at = signal_ts + pd.Timedelta(minutes=signal_minutes)
    last_closed_start = available_at - pd.Timedelta(minutes=context_minutes)
    ts_values = context["ts_utc"].to_numpy(dtype="datetime64[ns]")
    index = int(np.searchsorted(ts_values, np.datetime64(last_closed_start), side="right")) - 1
    if index < 0:
        return None
    return context.iloc[index]


def context_feature_values(
    timeframe: str,
    context: pd.DataFrame,
    signal_ts: pd.Timestamp,
    signal_minutes: int,
    direction: str,
    tick_size: float,
) -> dict[str, Any]:
    prefix = f"ctx_{timeframe}_"
    values: dict[str, Any] = {
        f"{prefix}trend_label": "unknown",
        f"{prefix}directional_trend": "unknown",
        f"{prefix}atr_ticks": None,
        f"{prefix}ema_gap_atr": None,
        f"{prefix}ema_fast_slope_atr": None,
        f"{prefix}relative_volume": None,
        f"{prefix}compression": None,
        f"{prefix}range_position_48": None,
        f"{prefix}distance_to_high_48_atr": None,
        f"{prefix}distance_to_low_48_atr": None,
    }
    _, context_minutes = table_for_timeframe(timeframe)
    row = context_row_at(context, signal_ts, signal_minutes, context_minutes)
    if row is None:
        return values
    label = trend_label(row)
    sign = wave.direction_sign(direction) if direction in {"LONG", "SHORT"} else 1.0
    close = wave.finite(row.get("close"), 0.0) or 0.0
    atr = wave.finite(row.get("atr"), tick_size * 12.0) or tick_size * 12.0
    high_48 = wave.finite(row.get("prior_high_48"))
    low_48 = wave.finite(row.get("prior_low_48"))
    range_48 = (high_48 - low_48) if high_48 is not None and low_48 is not None else None
    values.update(
        {
            f"{prefix}trend_label": label,
            f"{prefix}directional_trend": directional_trend(direction, label),
            f"{prefix}atr_ticks": atr / tick_size if tick_size > 0 else None,
            f"{prefix}ema_gap_atr": sign
            * ((wave.finite(row.get("ema_fast"), close) or close) - (wave.finite(row.get("ema_slow"), close) or close))
            / atr,
            f"{prefix}ema_fast_slope_atr": sign * (wave.finite(row.get("ema_fast_slope"), 0.0) or 0.0) / atr,
            f"{prefix}relative_volume": wave.finite(row.get("relative_volume"), 1.0),
            f"{prefix}compression": wave.finite(row.get("compression"), 2.0),
            f"{prefix}range_position_48": (close - low_48) / range_48 if low_48 is not None and range_48 and range_48 > 0 else None,
            f"{prefix}distance_to_high_48_atr": ((high_48 - close) / atr) if high_48 is not None else None,
            f"{prefix}distance_to_low_48_atr": ((close - low_48) / atr) if low_48 is not None else None,
        }
    )
    return values


def all_context_features(
    context_frames: dict[str, pd.DataFrame],
    signal_ts: pd.Timestamp,
    signal_minutes: int,
    direction: str,
    tick_size: float,
) -> dict[str, Any]:
    values = blank_context_features()
    for timeframe, context in context_frames.items():
        values.update(context_feature_values(timeframe, context, signal_ts, signal_minutes, direction, tick_size))
    return values


def context_alignment_count(context_features: dict[str, Any], args: argparse.Namespace) -> int:
    count = 0
    for timeframe in higher_context_timeframes(args):
        if str(context_features.get(f"ctx_{timeframe}_directional_trend") or "").lower() == "with":
            count += 1
    return count


def entry_context_allowed(context_features: dict[str, Any], args: argparse.Namespace) -> bool:
    minimum = max(0, int(getattr(args, "mtf_entry_min_aligned", 0) or 0))
    if minimum <= 0:
        return True
    return context_alignment_count(context_features, args) >= minimum


def higher_timeframes_support_hold(
    context_frames: dict[str, pd.DataFrame],
    candle_ts: pd.Timestamp,
    signal_minutes: int,
    direction: str,
    tick_size: float,
    args: argparse.Namespace,
) -> bool:
    if not bool(getattr(args, "mtf_exit_governor", False)):
        return False
    minimum = max(1, int(getattr(args, "mtf_exit_min_aligned", 1) or 1))
    aligned = 0
    for timeframe in higher_context_timeframes(args):
        context = context_frames.get(timeframe)
        if context is None or context.empty:
            continue
        features = context_feature_values(timeframe, context, candle_ts, signal_minutes, direction, tick_size)
        if str(features.get(f"ctx_{timeframe}_directional_trend") or "").lower() == "with":
            aligned += 1
    return aligned >= minimum


def mtf_exit_mode(args: argparse.Namespace) -> str:
    mode = str(getattr(args, "mtf_exit_mode", "hard-veto") or "hard-veto")
    if mode not in {"hard-veto", "delay", "tighten", "delay-tighten"}:
        raise ValueError(f"Unsupported mtf exit mode: {mode}")
    return mode


def tighten_stop_for_mtf_hold(
    candles: pd.DataFrame,
    current_stop: float,
    entry_idx: int,
    candle_idx: int,
    direction: str,
    tick_size: float,
    args: argparse.Namespace,
) -> float:
    lookback = max(1, int(getattr(args, "mtf_exit_tighten_lookback_bars", 3) or 3))
    pad = max(0.0, float(getattr(args, "mtf_exit_tighten_pad_ticks", 1.0) or 0.0)) * tick_size
    start_idx = max(entry_idx, candle_idx - lookback + 1)
    window = candles.iloc[start_idx : candle_idx + 1]
    if window.empty:
        return current_stop

    close_price = wave.finite(candles["close"].iloc[candle_idx])
    if close_price is None:
        return current_stop

    if direction == "LONG":
        swing_stop = wave.finite(window["low"].min())
        if swing_stop is None:
            return current_stop
        proposed = min(float(swing_stop) - pad, float(close_price) - tick_size)
        return max(float(current_stop), proposed)

    swing_stop = wave.finite(window["high"].max())
    if swing_stop is None:
        return current_stop
    proposed = max(float(swing_stop) + pad, float(close_price) + tick_size)
    return min(float(current_stop), proposed)


def simulate_from_entry_with_mtf_governor(
    candles: pd.DataFrame,
    context_frames: dict[str, pd.DataFrame],
    d_confirm_date: pd.Timestamp,
    entry_idx: int,
    signal_idx: int | None,
    direction: str,
    tick_size: float,
    args: argparse.Namespace,
) -> wave.WaveResult:
    if entry_idx >= len(candles):
        return wave.no_entry("entry_out_of_range", signal_idx=signal_idx, candidate_count=1)
    stop_price, risk_points = wave.initial_stop(candles, entry_idx, direction, tick_size, args)
    entry_price = wave.finite(candles["open"].iloc[entry_idx])
    if stop_price is None or risk_points is None or entry_price is None:
        return wave.no_entry("risk_rejected", signal_idx=signal_idx, candidate_count=1)

    signal_minutes = table_for_timeframe(args.timeframe)[1]
    sign = wave.direction_sign(direction)
    current_stop = stop_price
    requested_end_idx = entry_idx + max(1, int(args.max_forward_minutes))
    time_exit_minutes = int(getattr(args, "time_exit_minutes", 0) or 0)
    if time_exit_minutes > 0:
        requested_end_idx = min(requested_end_idx, entry_idx + time_exit_minutes)
    end_idx = min(len(candles) - 1, requested_end_idx)
    exit_idx = end_idx
    exit_price = wave.finite(candles["close"].iloc[end_idx])
    exit_reason = "time_exit" if time_exit_minutes > 0 and end_idx == entry_idx + time_exit_minutes else "path_end"
    death_count = 0
    max_favorable = 0.0
    max_adverse = 0.0
    exit_mode = mtf_exit_mode(args)
    base_death_bars = max(1, int(getattr(args, "death_bars", 4) or 4))
    extra_death_bars = max(1, int(getattr(args, "mtf_exit_extra_death_bars", 1) or 1))

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

        if candle_idx - entry_idx >= args.min_hold_bars and wave.momentum_dead(candles, candle_idx, direction):
            candle_ts = pd.Timestamp(candles["ts_utc"].iloc[candle_idx])
            exit_threshold = base_death_bars
            mtf_supported = higher_timeframes_support_hold(context_frames, candle_ts, signal_minutes, direction, tick_size, args)
            if mtf_supported and exit_mode == "hard-veto":
                death_count = 0
            else:
                death_count += 1
                if mtf_supported:
                    if exit_mode in {"tighten", "delay-tighten"}:
                        current_stop = tighten_stop_for_mtf_hold(
                            candles,
                            current_stop,
                            entry_idx,
                            candle_idx,
                            direction,
                            tick_size,
                            args,
                        )
                    if exit_mode in {"delay", "delay-tighten"}:
                        exit_threshold = base_death_bars + extra_death_bars
        else:
            death_count = 0
            exit_threshold = base_death_bars
        if death_count >= exit_threshold:
            next_idx = min(candle_idx + 1, end_idx)
            exit_idx = next_idx
            exit_price = wave.finite(candles["open"].iloc[next_idx], candles["close"].iloc[next_idx])
            exit_reason = "momentum_fade"
            break

        current_stop = wave.update_profit_lock_stop(
            current_stop,
            float(entry_price),
            float(risk_points),
            max_favorable,
            direction,
            tick_size,
            args,
        )
        current_stop = wave.update_trailing_stop(candles, current_stop, entry_idx, candle_idx, direction, tick_size, args)

    if exit_price is None:
        return wave.no_entry("missing_exit_price", signal_idx=signal_idx, candidate_count=1)

    raw_result = sign * (float(exit_price) - entry_price) / risk_points
    slippage_cost_r = ((args.slippage_entry_ticks + args.slippage_exit_ticks) * tick_size) / risk_points
    result_r = raw_result - slippage_cost_r
    outcome = "pass" if result_r > 0 else "fail"
    entry_date = candles["ts_utc"].iloc[entry_idx]
    exit_date = candles["ts_utc"].iloc[exit_idx]
    risk_ticks = risk_points / tick_size if tick_size > 0 else None
    return wave.WaveResult(
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
        entry_delay_minutes=wave.minutes_between(d_confirm_date, entry_date),
        hold_minutes=wave.minutes_between(entry_date, exit_date),
        risk_ticks=float(risk_ticks) if risk_ticks is not None else None,
        trigger_score=wave.trigger_score(candles, signal_idx, direction) if signal_idx is not None else None,
        candidate_count=1,
        no_entry_reason=None,
    )


def bucket(value: float | None, boundaries: list[float], labels: list[str]) -> str:
    if value is None or not math.isfinite(value):
        return "unknown"
    for boundary, label in zip(boundaries, labels):
        if value <= boundary:
            return label
    return labels[-1]


def trend_label(row: pd.Series) -> str:
    close = wave.finite(row.get("close"))
    ema_fast = wave.finite(row.get("ema_fast"))
    ema_slow = wave.finite(row.get("ema_slow"))
    slope = wave.finite(row.get("ema_fast_slope"))
    if close is None or ema_fast is None or ema_slow is None or slope is None:
        return "unknown"
    if close > ema_slow and ema_fast >= ema_slow and slope > 0:
        return "bullish"
    if close < ema_slow and ema_fast <= ema_slow and slope < 0:
        return "bearish"
    return "neutral"


def directional_trend(direction: str, label: str) -> str:
    if label == "neutral":
        return "neutral"
    if label == "unknown":
        return "unknown"
    if direction == "LONG" and label == "bullish":
        return "with"
    if direction == "SHORT" and label == "bearish":
        return "with"
    return "fighting"


def make_candidate_uid(symbol: str, signal_date: Any, direction: str, formula: str) -> str:
    raw = f"{symbol}|{pd.Timestamp(signal_date).isoformat()}|{direction}|{formula}"
    return hashlib.sha1(raw.encode("utf-8")).hexdigest()[:32]


def opposite_direction(direction: str) -> str:
    if direction == "LONG":
        return "SHORT"
    if direction == "SHORT":
        return "LONG"
    raise ValueError(f"Unsupported trade direction: {direction}")


def tradable_result_r(result: wave.WaveResult | None) -> float | None:
    if result is None or result.outcome == "no_entry" or result.result_r is None:
        return None
    value = float(result.result_r)
    if not math.isfinite(value):
        return None
    return value


def best_action_for_counterfactual(original_result: wave.WaveResult, reverse_result: wave.WaveResult | None) -> str:
    original_r = tradable_result_r(original_result)
    reverse_r = tradable_result_r(reverse_result)
    if original_r is not None and original_r > 0 and (reverse_r is None or original_r >= reverse_r):
        return "original"
    if reverse_r is not None and reverse_r > 0 and (original_r is None or reverse_r > original_r):
        return "reverse"
    return "skip"


def reverse_counterfactual_fields(
    original_result: wave.WaveResult,
    reverse_result: wave.WaveResult | None,
) -> dict[str, Any]:
    reverse_r = tradable_result_r(reverse_result)
    original_r = tradable_result_r(original_result)
    reverse_edge = reverse_r - original_r if reverse_r is not None and original_r is not None else None
    return {
        "reverse_direction": reverse_result.trade_direction if reverse_result is not None else None,
        "reverse_outcome": reverse_result.outcome if reverse_result is not None else None,
        "reverse_exit_reason": reverse_result.exit_reason if reverse_result is not None else None,
        "reverse_no_entry_reason": reverse_result.no_entry_reason if reverse_result is not None else None,
        "reverse_entry_date": reverse_result.entry_date if reverse_result is not None else None,
        "reverse_exit_date": reverse_result.exit_date if reverse_result is not None else None,
        "reverse_entry_price": reverse_result.entry_price if reverse_result is not None else None,
        "reverse_stop_price": reverse_result.stop_price if reverse_result is not None else None,
        "reverse_exit_price": reverse_result.exit_price if reverse_result is not None else None,
        "reverse_risk_points": reverse_result.risk_points if reverse_result is not None else None,
        "reverse_result_r": reverse_r,
        "reverse_raw_result_r": reverse_result.raw_result_r if reverse_result is not None else None,
        "reverse_mfe_r": reverse_result.mfe_r if reverse_result is not None else None,
        "reverse_mae_r": reverse_result.mae_r if reverse_result is not None else None,
        "reverse_hold_minutes": reverse_result.hold_minutes if reverse_result is not None else None,
        "reverse_risk_ticks": reverse_result.risk_ticks if reverse_result is not None else None,
        "reverse_edge_r": reverse_edge,
        "best_action": best_action_for_counterfactual(original_result, reverse_result),
    }


def feature_row(
    year: int,
    timeframe: str,
    formula: str,
    root_symbol: str,
    symbol: str,
    candles: pd.DataFrame,
    signal_idx: int,
    result: wave.WaveResult,
    reverse_result: wave.WaveResult | None = None,
    context_features: dict[str, Any] | None = None,
    mtf_entry_aligned_count: int = 0,
) -> dict[str, Any]:
    signal = candles.iloc[signal_idx]
    direction = result.trade_direction or "UNKNOWN"
    sign = wave.direction_sign(direction) if direction in {"LONG", "SHORT"} else 1.0
    tick_size = wave.tick_size_for(symbol, root_symbol)
    atr = wave.finite(signal.get("atr"), tick_size * 12.0) or tick_size * 12.0
    close = wave.finite(signal.get("close"), 0.0) or 0.0
    entry_high = wave.finite(signal.get("entry_high"), close) or close
    entry_low = wave.finite(signal.get("entry_low"), close) or close
    prior_range = wave.finite(signal.get("prior_range"), 0.0) or 0.0
    if direction == "LONG":
        breakout = (close - entry_high) / atr
    else:
        breakout = (entry_low - close) / atr
    close_location = ((close - entry_low) / prior_range) if prior_range > 0 else 0.5
    high_48 = wave.finite(signal.get("prior_high_48"))
    low_48 = wave.finite(signal.get("prior_low_48"))
    range_48 = (high_48 - low_48) if high_48 is not None and low_48 is not None else None
    range_position = (close - low_48) / range_48 if low_48 is not None and range_48 and range_48 > 0 else None
    label = trend_label(signal)
    ts = pd.Timestamp(signal["ts_utc"])
    rel_volume = wave.finite(signal.get("relative_volume"), 1.0)
    compression = wave.finite(signal.get("compression"), 2.0)
    row = {
        "formula": formula,
        "timeframe": timeframe,
        "valid_year": year,
        "candidate_uid": make_candidate_uid(symbol, ts, direction, formula),
        "root_symbol": root_symbol,
        "symbol": symbol,
        "signal_date": ts,
        "entry_date": result.entry_date,
        "exit_date": result.exit_date,
        "direction": direction,
        "outcome": result.outcome,
        "exit_reason": result.exit_reason,
        "entry_price": result.entry_price,
        "stop_price": result.stop_price,
        "exit_price": result.exit_price,
        "risk_points": result.risk_points,
        "result_r": result.result_r,
        "raw_result_r": result.raw_result_r,
        "mfe_r": result.mfe_r,
        "mae_r": result.mae_r,
        "hold_minutes": result.hold_minutes,
        "risk_ticks": result.risk_ticks,
        "tick_size": tick_size,
        "signal_score": result.trigger_score,
        "mtf_entry_aligned_count": mtf_entry_aligned_count,
        **reverse_counterfactual_fields(result, reverse_result),
        "signal_hour": int(ts.hour),
        "signal_day_of_week": str(ts.dayofweek),
        "signal_month": str(ts.month),
        "signal_hour_bucket": bucket(float(ts.hour), [6, 12, 18, 23], ["overnight", "morning", "afternoon", "evening"]),
        "trend_label": label,
        "directional_trend": directional_trend(direction, label),
        "volume_bucket": bucket(rel_volume, [0.75, 1.25, 2.0, 999.0], ["quiet", "normal", "active", "hot"]),
        "compression_bucket": bucket(compression, [1.0, 2.0, 4.0, 999.0], ["tight", "normal", "wide", "loose"]),
        "atr_ticks": atr / tick_size if tick_size > 0 else None,
        "atr_pct": atr / close * 100.0 if close else None,
        "prior_range_ticks": prior_range / tick_size if tick_size > 0 else None,
        "ema_gap_atr": sign
        * ((wave.finite(signal.get("ema_fast"), close) or close) - (wave.finite(signal.get("ema_slow"), close) or close))
        / atr,
        "ema_fast_slope_atr": sign * (wave.finite(signal.get("ema_fast_slope"), 0.0) or 0.0) / atr,
        "breakout_atr": breakout,
        "relative_volume": rel_volume,
        "compression": compression,
        "close_location_prior_range": close_location,
        "ret_3_atr": sign * ((wave.finite(signal.get("ret_3"), 0.0) or 0.0) * close) / atr,
        "ret_6_atr": sign * ((wave.finite(signal.get("ret_6"), 0.0) or 0.0) * close) / atr,
        "ret_12_atr": sign * ((wave.finite(signal.get("ret_12"), 0.0) or 0.0) * close) / atr,
        "distance_to_high_48_atr": ((high_48 - close) / atr) if high_48 is not None else None,
        "distance_to_low_48_atr": ((close - low_48) / atr) if low_48 is not None else None,
        "range_position_48": range_position,
        "body_atr": (wave.finite(signal.get("body"), 0.0) or 0.0) / atr,
        "upper_wick_atr": (wave.finite(signal.get("upper_wick"), 0.0) or 0.0) / atr,
        "lower_wick_atr": (wave.finite(signal.get("lower_wick"), 0.0) or 0.0) / atr,
    }
    row.update(context_features or blank_context_features())
    return row


def insert_candidates(conn, rows: list[dict[str, Any]]) -> None:
    if not rows:
        return
    columns = list(rows[0].keys())
    placeholders = ",".join(["%s"] * len(columns))
    update_columns = [column for column in columns if column not in {"formula", "candidate_uid"}]
    updates = ", ".join([f"{column}=VALUES({column})" for column in update_columns] + ["updated_at=CURRENT_TIMESTAMP"])
    values = [tuple(clean(row.get(column)) for column in columns) for row in rows]
    with conn.cursor() as cur:
        cur.executemany(
            f"""
            INSERT INTO {CANDIDATE_TABLE} ({", ".join(columns)})
            VALUES ({placeholders})
            ON DUPLICATE KEY UPDATE {updates}
            """,
            values,
        )


def build_candidates_for_year(conn, year: int, args: argparse.Namespace) -> int:
    frame = fetch_year_candles(conn, year, args)
    if frame.empty:
        return 0
    total = 0
    pending: list[dict[str, Any]] = []
    grouped = list(frame.groupby(["root_symbol", "symbol"], sort=False))
    if args.limit_symbols > 0:
        grouped = grouped[: args.limit_symbols]
    signal_minutes = table_for_timeframe(args.timeframe)[1]
    for symbol_index, ((root, symbol), raw_group) in enumerate(grouped, start=1):
        candles = raw_group[["ts_utc", "open", "high", "low", "close", "volume"]].copy()
        candles = enrich_candles(candles, args)
        if len(candles) < 60:
            continue
        tick_size = wave.tick_size_for(symbol, root)
        context_frames = fetch_symbol_contexts(conn, str(symbol), raw_group, args)
        last_signal_by_direction: dict[str, int] = {"LONG": -10**9, "SHORT": -10**9}
        years = candles["ts_utc"].dt.year.to_numpy()
        for signal_idx in range(25, len(candles) - 2):
            if int(years[signal_idx]) != year:
                continue
            for direction in ["LONG", "SHORT"]:
                if signal_idx - last_signal_by_direction[direction] < args.candidate_min_gap_bars:
                    continue
                if not wave.is_wave_trigger(candles, signal_idx, direction, args, loose=bool(args.loose_triggers)):
                    continue
                signal_ts = pd.Timestamp(candles["ts_utc"].iloc[signal_idx])
                context_features = all_context_features(context_frames, signal_ts, signal_minutes, direction, tick_size)
                aligned_count = context_alignment_count(context_features, args)
                if not entry_context_allowed(context_features, args):
                    continue
                if bool(getattr(args, "mtf_exit_governor", False)):
                    result = simulate_from_entry_with_mtf_governor(
                        candles,
                        context_frames,
                        signal_ts,
                        signal_idx + 1,
                        signal_idx,
                        direction,
                        tick_size,
                        args,
                    )
                else:
                    result = wave.simulate_from_entry(
                        candles,
                        signal_ts,
                        signal_idx + 1,
                        signal_idx,
                        direction,
                        tick_size,
                        args,
                    )
                if result.outcome == "no_entry":
                    continue
                reverse_direction = opposite_direction(direction)
                if bool(getattr(args, "mtf_exit_governor", False)):
                    reverse_result = simulate_from_entry_with_mtf_governor(
                        candles,
                        context_frames,
                        signal_ts,
                        signal_idx + 1,
                        signal_idx,
                        reverse_direction,
                        tick_size,
                        args,
                    )
                else:
                    reverse_result = wave.simulate_from_entry(
                        candles,
                        signal_ts,
                        signal_idx + 1,
                        signal_idx,
                        reverse_direction,
                        tick_size,
                        args,
                    )
                last_signal_by_direction[direction] = signal_idx
                pending.append(
                    feature_row(
                        year,
                        args.timeframe,
                        args.formula,
                        str(root),
                        str(symbol),
                        candles,
                        signal_idx,
                        result,
                        reverse_result=reverse_result,
                        context_features=context_features,
                        mtf_entry_aligned_count=aligned_count,
                    )
                )
                total += 1
                if len(pending) >= 1000:
                    insert_candidates(conn, pending)
                    conn.commit()
                    pending.clear()
        if symbol_index % 50 == 0:
            print(f"  {year}: scanned {symbol_index:,}/{len(grouped):,} symbols, candidates={total:,}")
    if pending:
        insert_candidates(conn, pending)
        conn.commit()
    return total


def count_candidates(conn, timeframe: str, year: int, formula: str | None = None) -> int:
    with conn.cursor() as cur:
        cur.execute(
            f"SELECT COUNT(*) AS count FROM {CANDIDATE_TABLE} WHERE formula = %s AND timeframe = %s AND valid_year = %s",
            (formula or formula_for(timeframe), timeframe, year),
        )
        return int(cur.fetchone()["count"])


def load_candidates(conn, timeframe: str, years: list[int], formula: str | None = None) -> pd.DataFrame:
    if not years:
        return pd.DataFrame()
    placeholders = ",".join(["%s"] * len(years))
    columns = [
        "formula",
        "candidate_uid",
        "timeframe",
        "valid_year",
        "root_symbol",
        "symbol",
        "signal_date",
        "entry_date",
        "exit_date",
        "direction",
        "outcome",
        "exit_reason",
        "entry_price",
        "stop_price",
        "exit_price",
        "risk_points",
        "result_r",
        "raw_result_r",
        "mfe_r",
        "mae_r",
        "hold_minutes",
        "risk_ticks",
        "tick_size",
        "signal_score",
        *counterfactual_column_definitions().keys(),
        *CAT_FEATURES,
        *NUM_FEATURES,
    ]
    # Remove duplicates where feature columns are already included explicitly.
    columns = list(dict.fromkeys(columns))
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT {", ".join(columns)}
            FROM {CANDIDATE_TABLE}
            WHERE formula = %s
              AND timeframe = %s
              AND valid_year IN ({placeholders})
            ORDER BY valid_year, entry_date, root_symbol, symbol, direction
            """,
            [formula or formula_for(timeframe), timeframe, *years],
        )
        rows = cur.fetchall()
    frame = pd.DataFrame(rows)
    if frame.empty:
        return frame
    for col in ["signal_date", "entry_date", "exit_date", "reverse_entry_date", "reverse_exit_date"]:
        frame[col] = pd.to_datetime(frame[col], errors="coerce")
    numeric_columns = [
        "result_r",
        "raw_result_r",
        "mfe_r",
        "mae_r",
        "risk_points",
        "entry_price",
        "stop_price",
        "exit_price",
        "reverse_entry_price",
        "reverse_stop_price",
        "reverse_exit_price",
        "reverse_risk_points",
        "reverse_result_r",
        "reverse_raw_result_r",
        "reverse_mfe_r",
        "reverse_mae_r",
        "reverse_hold_minutes",
        "reverse_risk_ticks",
        "reverse_edge_r",
    ]
    for col in NUM_FEATURES + numeric_columns:
        if col in frame.columns:
            frame[col] = pd.to_numeric(frame[col], errors="coerce")
    return frame


def liquidity_gate_enabled(args: argparse.Namespace) -> bool:
    return (
        float(getattr(args, "liquidity_lookback_hours", 0.0) or 0.0) > 0.0
        and (
            int(getattr(args, "min_liquidity_bars", 0) or 0) > 0
            or float(getattr(args, "min_liquidity_volume", 0.0) or 0.0) > 0.0
        )
    )


def apply_liquidity_gate(conn, frame: pd.DataFrame, args: argparse.Namespace, label: str) -> pd.DataFrame:
    if frame.empty or not liquidity_gate_enabled(args):
        return frame
    lookback_hours = float(args.liquidity_lookback_hours)
    min_bars = max(0, int(args.min_liquidity_bars or 0))
    min_volume = max(0.0, float(args.min_liquidity_volume or 0.0))
    table_name, _ = table_for_timeframe(args.timeframe)
    table_name = safe_identifier(table_name)
    work = frame.copy()
    work["preentry_liquidity_bars"] = 0
    work["preentry_liquidity_volume"] = 0.0
    entries = pd.to_datetime(work["entry_date"], errors="coerce")
    symbols = sorted(str(symbol) for symbol in work["symbol"].dropna().unique())
    if entries.dropna().empty or not symbols:
        return work.iloc[0:0].copy()

    start_at = entries.min() - pd.Timedelta(hours=lookback_hours)
    end_at = entries.max()
    candle_rows: list[dict[str, Any]] = []
    batch_size = 80
    with conn.cursor() as cur:
        for offset in range(0, len(symbols), batch_size):
            batch = symbols[offset : offset + batch_size]
            placeholders = ",".join(["%s"] * len(batch))
            cur.execute(
                f"""
                SELECT symbol, ts_utc, volume
                FROM {table_name}
                WHERE symbol IN ({placeholders})
                  AND ts_utc >= %s
                  AND ts_utc < %s
                ORDER BY symbol, ts_utc
                """,
                [*batch, start_at.to_pydatetime(), end_at.to_pydatetime()],
            )
            candle_rows.extend(cur.fetchall())
    candles = pd.DataFrame(candle_rows)
    if not candles.empty:
        candles["ts_utc"] = pd.to_datetime(candles["ts_utc"], errors="coerce")
        candles["volume"] = pd.to_numeric(candles["volume"], errors="coerce").fillna(0.0)
        lookback_ns = pd.Timedelta(hours=lookback_hours).value
        for symbol, candle_group in candles.groupby("symbol", sort=False):
            candidate_index = work.index[work["symbol"].astype(str) == str(symbol)]
            if candidate_index.empty:
                continue
            candle_group = candle_group.dropna(subset=["ts_utc"]).sort_values("ts_utc")
            if candle_group.empty:
                continue
            candle_times = candle_group["ts_utc"].to_numpy(dtype="datetime64[ns]").astype("int64")
            volumes = candle_group["volume"].to_numpy(dtype=float)
            cumulative_volume = np.concatenate([[0.0], np.cumsum(volumes)])
            candidate_entries = entries.loc[candidate_index].to_numpy(dtype="datetime64[ns]").astype("int64")
            left = np.searchsorted(candle_times, candidate_entries - lookback_ns, side="left")
            right = np.searchsorted(candle_times, candidate_entries, side="left")
            work.loc[candidate_index, "preentry_liquidity_bars"] = right - left
            work.loc[candidate_index, "preentry_liquidity_volume"] = cumulative_volume[right] - cumulative_volume[left]

    before = len(work)
    mask = (
        pd.to_numeric(work["preentry_liquidity_bars"], errors="coerce").fillna(0) >= min_bars
    ) & (
        pd.to_numeric(work["preentry_liquidity_volume"], errors="coerce").fillna(0.0) >= min_volume
    )
    gated = work[mask].copy()
    print(
        f"Liquidity gate {label}: kept {len(gated):,}/{before:,} rows "
        f"({lookback_hours:g}h, bars>={min_bars}, volume>={min_volume:g})"
    )
    return gated


def target_series(frame: pd.DataFrame, args: argparse.Namespace | None = None) -> pd.Series:
    target = pd.to_numeric(frame["result_r"], errors="coerce").fillna(0.0)
    if args is not None and (args.target_clip_min is not None or args.target_clip_max is not None):
        lower = args.target_clip_min if args.target_clip_min is not None else target.min()
        upper = args.target_clip_max if args.target_clip_max is not None else target.max()
        target = target.clip(lower=lower, upper=upper)
    return target


def prepare_pool(frame: pd.DataFrame, include_target: bool = True, args: argparse.Namespace | None = None) -> Pool:
    work = frame.copy()
    for col in CAT_FEATURES:
        if col not in work.columns:
            work[col] = "unknown"
        work[col] = work[col].fillna("unknown").astype(str)
    for col in NUM_FEATURES:
        if col not in work.columns:
            work[col] = 0.0
        work[col] = pd.to_numeric(work[col], errors="coerce").replace([np.inf, -np.inf], np.nan).fillna(0.0)
    features = CAT_FEATURES + NUM_FEATURES
    if include_target:
        target = target_series(work, args)
        return Pool(work[features], label=target, cat_features=CAT_FEATURES)
    return Pool(work[features], cat_features=CAT_FEATURES)


def train_model(frame: pd.DataFrame, args: argparse.Namespace) -> CatBoostRegressor:
    params = {
        "loss_function": "RMSE",
        "iterations": args.iterations,
        "depth": args.depth,
        "learning_rate": args.learning_rate,
        "l2_leaf_reg": args.l2_leaf_reg,
        "random_seed": args.random_seed,
        "verbose": False,
        "allow_writing_files": False,
    }
    if args.random_strength is not None:
        params["random_strength"] = args.random_strength
    if str(args.bootstrap_type or "").strip():
        params["bootstrap_type"] = str(args.bootstrap_type).strip()
    model = CatBoostRegressor(**params)
    model.fit(prepare_pool(frame, include_target=True, args=args))
    return model


def add_predictions(frame: pd.DataFrame, model: CatBoostRegressor) -> pd.DataFrame:
    if frame.empty:
        return frame
    scored = frame.copy()
    scored["predicted_r"] = model.predict(prepare_pool(scored, include_target=False))
    return scored


def energy_throttle_key(root_symbol: str) -> str | None:
    root = str(root_symbol or "").upper()
    if root in {"HO", "RB", "QM", "NG"}:
        return "energy"
    return None


def risk_governor_key(root_symbol: str, scope: str) -> str:
    root = str(root_symbol or "").upper()
    if scope == "all":
        return "all"
    if scope == "risk-group":
        return energy_throttle_key(root) or root
    return root


def select_chronological(scored: pd.DataFrame, threshold: float | None, args: argparse.Namespace) -> pd.DataFrame:
    if scored.empty:
        return scored
    eligible = scored.copy() if threshold is None else scored[scored["predicted_r"] >= threshold].copy()
    allowed_roots = {root.upper() for root in parse_list(getattr(args, "allowed_roots", ""))}
    if allowed_roots and not eligible.empty:
        eligible = eligible[eligible["root_symbol"].astype(str).str.upper().isin(allowed_roots)].copy()
    if eligible.empty:
        return eligible
    eligible = eligible.sort_values(["entry_date", "predicted_r", "signal_score"], ascending=[True, False, False])
    selected_indices: list[int] = []
    next_available: dict[str, pd.Timestamp] = {}
    cooldown = pd.Timedelta(minutes=max(0, int(args.cooldown_minutes)))
    root_day_counts: dict[tuple[str, Any], int] = {}
    energy_day_counts: dict[tuple[str, Any], int] = {}
    max_root_day = max(0, int(getattr(args, "max_root_trades_per_day", 0) or 0))
    max_energy_day = max(0, int(getattr(args, "max_energy_trades_per_day", 0) or 0))
    energy_extra_start = max(0, int(getattr(args, "energy_extra_slot_start", 0) or 0))
    energy_extra_min_score = float(getattr(args, "energy_extra_slot_min_score", 0.0) or 0.0)
    loss_brake_r = max(0.0, float(getattr(args, "loss_brake_r", 0.0) or 0.0))
    loss_brake_scope = str(getattr(args, "loss_brake_scope", "root") or "root")
    loss_brake_min_trades = max(1, int(getattr(args, "loss_brake_min_trades", 1) or 1))
    loss_brake_totals: dict[tuple[str, Any], float] = {}
    loss_brake_counts: dict[tuple[str, Any], int] = {}
    loss_brake_paused: set[tuple[str, Any]] = set()
    for idx, row in eligible.iterrows():
        entry_at = pd.Timestamp(row["entry_date"])
        exit_at = pd.Timestamp(row["exit_date"]) if not pd.isna(row.get("exit_date")) else entry_at
        symbol = str(row.get("symbol") or "")
        root = str(row.get("root_symbol") or "").upper()
        trade_day = entry_at.date()
        brake_key = (risk_governor_key(root, loss_brake_scope), trade_day)
        if loss_brake_r > 0 and brake_key in loss_brake_paused:
            continue
        if symbol and entry_at < next_available.get(symbol, pd.Timestamp.min):
            continue
        root_key = (root, trade_day)
        if max_root_day > 0 and root and root_day_counts.get(root_key, 0) >= max_root_day:
            continue
        energy_key_name = energy_throttle_key(root)
        energy_key = (energy_key_name, trade_day)
        if max_energy_day > 0 and energy_key_name and energy_day_counts.get(energy_key, 0) >= max_energy_day:
            continue
        if (
            energy_key_name
            and energy_extra_start > 0
            and energy_extra_min_score > 0.0
            and energy_day_counts.get(energy_key, 0) >= energy_extra_start
            and float(row.get("predicted_r") or 0.0) < energy_extra_min_score
        ):
            continue
        selected_indices.append(idx)
        if loss_brake_r > 0:
            realized_r = pd.to_numeric(row.get("result_r"), errors="coerce")
            realized_r = 0.0 if pd.isna(realized_r) else float(realized_r)
            loss_brake_totals[brake_key] = loss_brake_totals.get(brake_key, 0.0) + realized_r
            loss_brake_counts[brake_key] = loss_brake_counts.get(brake_key, 0) + 1
            if (
                loss_brake_counts[brake_key] >= loss_brake_min_trades
                and loss_brake_totals[brake_key] <= -loss_brake_r
            ):
                loss_brake_paused.add(brake_key)
        if symbol:
            next_available[symbol] = exit_at + cooldown
        if max_root_day > 0 and root:
            root_day_counts[root_key] = root_day_counts.get(root_key, 0) + 1
        if max_energy_day > 0 and energy_key_name:
            energy_day_counts[energy_key] = energy_day_counts.get(energy_key, 0) + 1
    selected = eligible.loc[selected_indices].copy()
    selected = selected.sort_values(["entry_date", "predicted_r"], ascending=[True, False]).reset_index(drop=True)
    selected["selected_index"] = np.arange(1, len(selected) + 1)
    return selected


def summarize(selected: pd.DataFrame, threshold_score_cap_r: float | None = None) -> dict[str, Any]:
    result = pd.to_numeric(selected.get("result_r", pd.Series(dtype=float)), errors="coerce").fillna(0.0)
    score_result = result.clip(upper=threshold_score_cap_r) if threshold_score_cap_r is not None else result
    wins = int((result > 0.0).sum())
    losses = int((result <= 0.0).sum())
    equity = result.cumsum()
    peak = equity.cummax()
    drawdown = peak - equity
    return {
        "selected_trades": int(len(result)),
        "wins": wins,
        "losses": losses,
        "win_rate": wins / len(result) if len(result) else 0.0,
        "avg_r": float(result.mean()) if len(result) else 0.0,
        "sum_r": float(result.sum()) if len(result) else 0.0,
        "score_sum_r": float(score_result.sum()) if len(score_result) else 0.0,
        "score_avg_r": float(score_result.mean()) if len(score_result) else 0.0,
        "max_drawdown_r": float(drawdown.max()) if len(drawdown) else 0.0,
    }


def choose_threshold(conn, args: argparse.Namespace) -> float | None:
    if args.threshold is not None:
        return args.threshold
    threshold_year = int(args.threshold_year)
    train_years = [year for year in parse_years(args.train_years) if year < threshold_year]
    if not train_years:
        print("No prior years for threshold calibration; using all scored candidates.")
        return None
    train = load_candidates(conn, args.timeframe, train_years, args.formula)
    holdout = load_candidates(conn, args.timeframe, [threshold_year], args.formula)
    if args.liquidity_gate_train:
        train = apply_liquidity_gate(conn, train, args, f"threshold-train-{','.join(str(year) for year in train_years)}")
    holdout = apply_liquidity_gate(conn, holdout, args, f"threshold-holdout-{threshold_year}")
    if train.empty or holdout.empty:
        print("Missing threshold calibration rows; using all scored candidates.")
        return None
    model = train_model(train, args)
    scored = add_predictions(holdout, model)
    quantiles = sorted(set(float(value) for value in scored["predicted_r"].quantile(np.linspace(0.0, 0.98, 25)).dropna()))
    candidates: list[float | None] = [None, 0.0, *quantiles]
    summaries: list[dict[str, Any]] = []
    for threshold in candidates:
        selected = select_chronological(scored, threshold, args)
        item = summarize(selected, args.threshold_score_cap_r)
        item["threshold"] = threshold
        summaries.append(item)
    viable = [item for item in summaries if item["selected_trades"] >= args.min_threshold_trades]
    if not viable:
        viable = summaries
    best = max(viable, key=lambda item: (item["score_sum_r"], item["score_avg_r"], item["sum_r"], item["selected_trades"]))
    print("Candle scanner threshold calibration on", threshold_year)
    print_key = "score_sum_r" if args.threshold_score_cap_r is not None else "sum_r"
    for item in sorted(summaries, key=lambda row: row[print_key], reverse=True)[:8]:
        label = "all" if item["threshold"] is None else f"{item['threshold']:.4f}"
        score_label = ""
        if args.threshold_score_cap_r is not None:
            score_label = f", cap{args.threshold_score_cap_r:g}={item['score_sum_r']:.1f}R"
        print(
            f"  threshold {label}: trades={item['selected_trades']:,}, win={item['win_rate'] * 100:.2f}%, "
            f"avg={item['avg_r']:.3f}R, sum={item['sum_r']:.1f}R, dd={item['max_drawdown_r']:.1f}R{score_label}"
        )
    label = "all" if best["threshold"] is None else f"{best['threshold']:.4f}"
    print(f"Selected candle threshold: {label}")
    return best["threshold"]


def insert_selected(conn, run_id: str, selected: pd.DataFrame) -> None:
    if selected.empty:
        return
    columns = [
        "model_run_id",
        "selected_index",
        "candidate_uid",
        "predicted_r",
        "formula",
        "timeframe",
        "valid_year",
        "root_symbol",
        "symbol",
        "signal_date",
        "entry_date",
        "exit_date",
        "direction",
        "outcome",
        "exit_reason",
        "entry_price",
        "stop_price",
        "exit_price",
        "risk_points",
        "result_r",
        "raw_result_r",
        "mfe_r",
        "mae_r",
        "hold_minutes",
        "risk_ticks",
        "tick_size",
        "signal_score",
        *counterfactual_column_definitions().keys(),
        *CAT_FEATURES,
        *NUM_FEATURES,
    ]
    columns = list(dict.fromkeys(columns))
    rows = []
    for row in selected.to_dict("records"):
        payload = {"model_run_id": run_id, **row}
        if not payload.get("formula"):
            payload["formula"] = formula_for(str(payload.get("timeframe") or "5m"))
        rows.append(tuple(clean(payload.get(column)) for column in columns))
    placeholders = ",".join(["%s"] * len(columns))
    with conn.cursor() as cur:
        cur.executemany(
            f"""
            INSERT INTO {SELECTED_TABLE} ({", ".join(columns)})
            VALUES ({placeholders})
            """,
            rows,
        )


def save_run(conn, run_id: str, args: argparse.Namespace, threshold: float | None, selected: pd.DataFrame, model: CatBoostRegressor) -> dict[str, Any]:
    summary = summarize(selected)
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id
    model_dir.mkdir(parents=True, exist_ok=True)
    model_path = model_dir / "catboost_model.cbm"
    model.save_model(str(model_path))
    metadata = {
        "model_run_id": run_id,
        "formula": args.formula,
        "timeframe": args.timeframe,
        "train_years": parse_years(args.train_years),
        "threshold_year": args.threshold_year,
        "valid_year": args.valid_year,
        "threshold": threshold,
        "cat_features": CAT_FEATURES,
        "num_features": NUM_FEATURES,
        "context_timeframes": requested_context_timeframes(args),
        "context_lookback_bars": args.context_lookback_bars,
        "candidate_min_gap_bars": args.candidate_min_gap_bars,
        "cooldown_minutes": args.cooldown_minutes,
        "slippage_entry_ticks": args.slippage_entry_ticks,
        "slippage_exit_ticks": args.slippage_exit_ticks,
        "min_risk_ticks": args.min_risk_ticks,
        "max_risk_ticks": args.max_risk_ticks,
        "max_forward_bars": args.max_forward_bars,
        "time_exit_bars": args.time_exit_bars,
        "iterations": args.iterations,
        "depth": args.depth,
        "learning_rate": args.learning_rate,
        "l2_leaf_reg": args.l2_leaf_reg,
        "random_seed": args.random_seed,
        "random_strength": args.random_strength,
        "bootstrap_type": args.bootstrap_type,
        "target_clip_min": args.target_clip_min,
        "target_clip_max": args.target_clip_max,
        "threshold_score_cap_r": args.threshold_score_cap_r,
        "allowed_roots": parse_list(args.allowed_roots),
        "max_root_trades_per_day": args.max_root_trades_per_day,
        "max_energy_trades_per_day": args.max_energy_trades_per_day,
        "energy_extra_slot_start": args.energy_extra_slot_start,
        "energy_extra_slot_min_score": args.energy_extra_slot_min_score,
        "loss_brake_r": args.loss_brake_r,
        "loss_brake_scope": args.loss_brake_scope,
        "loss_brake_min_trades": args.loss_brake_min_trades,
        "liquidity_lookback_hours": args.liquidity_lookback_hours,
        "min_liquidity_bars": args.min_liquidity_bars,
        "min_liquidity_volume": args.min_liquidity_volume,
        "liquidity_gate_train": bool(args.liquidity_gate_train),
        "mtf_entry_min_aligned": args.mtf_entry_min_aligned,
        "mtf_exit_governor": bool(args.mtf_exit_governor),
        "mtf_exit_min_aligned": args.mtf_exit_min_aligned,
        "mtf_exit_mode": args.mtf_exit_mode,
        "mtf_exit_extra_death_bars": args.mtf_exit_extra_death_bars,
        "mtf_exit_tighten_lookback_bars": args.mtf_exit_tighten_lookback_bars,
        "mtf_exit_tighten_pad_ticks": args.mtf_exit_tighten_pad_ticks,
    }
    (model_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n")
    candidate_rows = count_candidates(conn, args.timeframe, int(args.valid_year), args.formula)
    with conn.cursor() as cur:
        cur.execute(
            f"""
            INSERT INTO {RUN_TABLE} (
                model_run_id, formula, timeframe, train_years, threshold_year, valid_year,
                threshold, candidate_rows, selected_trades, wins, losses, win_rate,
                avg_r, sum_r, max_drawdown_r, model_path, metadata_json
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, CAST(%s AS JSON))
            """,
            (
                run_id,
                args.formula,
                args.timeframe,
                ",".join(str(year) for year in parse_years(args.train_years)),
                int(args.threshold_year),
                int(args.valid_year),
                threshold,
                candidate_rows,
                summary["selected_trades"],
                summary["wins"],
                summary["losses"],
                summary["win_rate"],
                summary["avg_r"],
                summary["sum_r"],
                summary["max_drawdown_r"],
                str(model_dir),
                json.dumps(metadata, sort_keys=True),
            ),
        )
    insert_selected(conn, run_id, selected)
    conn.commit()
    summary["candidate_rows"] = candidate_rows
    summary["model_run_id"] = run_id
    summary["threshold"] = threshold
    summary["model_path"] = str(model_dir)
    return summary


def main() -> int:
    args = parse_args()
    configure_wave_args(args)
    candidate_years = parse_years(args.candidate_years)
    train_years = parse_years(args.train_years)
    run_id = run_id_for(args)
    conn = wave.connect()
    try:
        ensure_tables(conn)
        if args.replace_candidates:
            delete_candidates(conn, args.timeframe, candidate_years, args.formula)
        if not args.skip_candidate_build:
            print("Building candle-wave candidates:")
            for year in candidate_years:
                existing = count_candidates(conn, args.timeframe, year, args.formula)
                if existing and not args.replace_candidates:
                    print(f"  {year}: {existing:,} existing candidates")
                    continue
                built = build_candidates_for_year(conn, year, args)
                print(f"  {year}: built {built:,} candidates")
        if args.build_only:
            return 0
        with conn.cursor() as cur:
            cur.execute(f"SELECT COUNT(*) AS count FROM {RUN_TABLE} WHERE model_run_id = %s", (run_id,))
            exists = int(cur.fetchone()["count"]) > 0
        if exists and not args.replace_run:
            raise RuntimeError(f"Run already exists. Use --replace-run to rebuild it: {run_id}")
        delete_run(conn, run_id)
        threshold = choose_threshold(conn, args)
        train = load_candidates(conn, args.timeframe, train_years, args.formula)
        valid = load_candidates(conn, args.timeframe, [int(args.valid_year)], args.formula)
        if args.liquidity_gate_train:
            train = apply_liquidity_gate(conn, train, args, f"train-{','.join(str(year) for year in train_years)}")
        valid = apply_liquidity_gate(conn, valid, args, f"valid-{args.valid_year}")
        if train.empty:
            raise RuntimeError("No training candidates found.")
        if valid.empty:
            raise RuntimeError("No validation candidates found.")
        print(f"Training candle scanner on {len(train):,} candidates from {train_years}")
        model = train_model(train, args)
        scored = add_predictions(valid, model)
        selected = select_chronological(scored, threshold, args)
        summary = save_run(conn, run_id, args, threshold, selected, model)
    except Exception:
        conn.rollback()
        raise
    finally:
        conn.close()
    label = "all" if summary["threshold"] is None else f"{summary['threshold']:.4f}"
    print(
        f"Materialized {summary['model_run_id']} threshold={label}: "
        f"{summary['selected_trades']:,}/{summary['candidate_rows']:,} valid candidates selected, "
        f"{summary['wins']:,} wins / {summary['losses']:,} losses, "
        f"{summary['win_rate'] * 100:.2f}% win, {summary['avg_r']:.3f}R avg, "
        f"{summary['sum_r']:.1f}R sum, {summary['max_drawdown_r']:.1f}R max DD"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
