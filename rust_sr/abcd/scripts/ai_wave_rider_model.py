#!/usr/bin/env python3
"""
Train and materialize a first dynamic wave-rider candidate model.

The companion ai_wave_rider_research.py script proved the ceiling by simulating
plausible wave entries. This script stores those candidate rows, trains CatBoost
to rank them, picks one candidate per setup, and writes a normal AI Trades run.
"""

from __future__ import annotations

import argparse
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


CANDIDATE_TABLE = "ai_wave_rider_candidate_rows"

CAT_FEATURES = [
    "symbol",
    "root_symbol",
    "market",
    "pattern_family_key",
    "harmonic_type",
    "family_bin",
    "family_size_bucket",
    "family_time_bin",
    "family_x_strictness",
    "direction",
    "source_trade_direction",
    "confirm_day_of_week",
    "confirm_month",
    "ctx_5m_trend_label",
    "ctx_15m_trend_label",
    "ctx_1h_trend_label",
    "ctx_4h_trend_label",
    "ctx_5m_alignment",
    "ctx_15m_alignment",
    "ctx_1h_alignment",
    "ctx_4h_alignment",
]

NUM_FEATURES = [
    "source_predicted_expected_r",
    "source_score_margin_top2",
    "entry_delay_minutes",
    "signal_score",
    "direction_matches_source",
    "confirm_hour",
    "risk_ticks",
    "atr_ticks",
    "prior_range_ticks",
    "ema_gap_atr",
    "ema_fast_slope_atr",
    "breakout_atr",
    "relative_volume",
    "compression",
    "close_location_prior_range",
    "xa_ticks",
    "cd_ticks",
    "full_pattern_length",
    "ctx_5m_trend_strength_pct",
    "ctx_5m_linreg_slope_pct_per_candle",
    "ctx_5m_relative_volume_20",
    "ctx_5m_range_position_pct",
    "ctx_5m_support_distance_r",
    "ctx_5m_resistance_distance_r",
    "ctx_15m_trend_strength_pct",
    "ctx_15m_linreg_slope_pct_per_candle",
    "ctx_15m_relative_volume_20",
    "ctx_15m_range_position_pct",
    "ctx_15m_support_distance_r",
    "ctx_15m_resistance_distance_r",
    "ctx_1h_trend_strength_pct",
    "ctx_1h_linreg_slope_pct_per_candle",
    "ctx_1h_relative_volume_20",
    "ctx_1h_range_position_pct",
    "ctx_1h_support_distance_r",
    "ctx_1h_resistance_distance_r",
    "ctx_4h_trend_strength_pct",
    "ctx_4h_linreg_slope_pct_per_candle",
    "ctx_4h_relative_volume_20",
    "ctx_4h_range_position_pct",
    "ctx_4h_support_distance_r",
    "ctx_4h_resistance_distance_r",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-prefix", default="aimv-market-entry-context-v7")
    parser.add_argument("--candidate-years", default="2022,2023,2024,2025,2026")
    parser.add_argument("--train-years", default="2022,2023,2024,2025")
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--run-prefix", default="aimv-wave-rider-model-v1")
    parser.add_argument("--source-run-prefix", default="wave-rider-model-v1")
    parser.add_argument("--replace-candidates", action="store_true")
    parser.add_argument("--replace-run", action="store_true")
    parser.add_argument("--skip-candidate-build", action="store_true")
    parser.add_argument("--threshold", type=float, default=None)
    parser.add_argument("--min-threshold-trades", type=int, default=35)
    parser.add_argument("--iterations", type=int, default=700)
    parser.add_argument("--depth", type=int, default=6)
    parser.add_argument("--learning-rate", type=float, default=0.04)
    parser.add_argument("--l2-leaf-reg", type=float, default=8.0)
    parser.add_argument("--random-seed", type=int, default=41)
    parser.add_argument("--api-refresh-url", default="http://localhost:8080/patterns/ai-stage1-trades")
    parser.add_argument("--skip-api-refresh", action="store_true")
    parser.add_argument(
        "--allow-duplicate-entry",
        action="store_true",
        help="Allow multiple setups to take the same symbol/direction/entry timestamp.",
    )

    # Wave-scanner knobs reused by ai_wave_rider_research.
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
    return parser.parse_args()


def ensure_candidate_table(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            f"""
            CREATE TABLE IF NOT EXISTS {CANDIDATE_TABLE} (
                source_ai_run_id VARCHAR(64) NOT NULL,
                valid_year INT NOT NULL,
                setup_id VARCHAR(64) NOT NULL,
                candidate_uid VARCHAR(64) NOT NULL,
                source_template_uid VARCHAR(128) NULL,
                symbol VARCHAR(32) NULL,
                root_symbol VARCHAR(32) NULL,
                market VARCHAR(32) NULL,
                pattern_family_key VARCHAR(255) NULL,
                harmonic_type VARCHAR(64) NULL,
                family_bin VARCHAR(64) NULL,
                family_size_bucket VARCHAR(64) NULL,
                family_time_bin VARCHAR(64) NULL,
                family_x_strictness VARCHAR(64) NULL,
                d_confirm_date DATETIME NULL,
                direction VARCHAR(16) NOT NULL,
                source_trade_direction VARCHAR(16) NULL,
                signal_index INT NULL,
                entry_index INT NULL,
                exit_index INT NULL,
                signal_date DATETIME NULL,
                entry_date DATETIME NULL,
                exit_date DATETIME NULL,
                entry_price DOUBLE NULL,
                stop_price DOUBLE NULL,
                exit_price DOUBLE NULL,
                risk_points DOUBLE NULL,
                risk_ticks DOUBLE NULL,
                tick_size DOUBLE NULL,
                exit_reason VARCHAR(64) NULL,
                result_r DOUBLE NOT NULL DEFAULT 0,
                raw_result_r DOUBLE NULL,
                mfe_r DOUBLE NULL,
                mae_r DOUBLE NULL,
                hold_minutes INT NULL,
                entry_delay_minutes INT NULL,
                signal_score DOUBLE NULL,
                source_predicted_expected_r DOUBLE NULL,
                source_score_margin_top2 DOUBLE NULL,
                direction_matches_source TINYINT(1) NOT NULL DEFAULT 0,
                confirm_hour INT NULL,
                confirm_day_of_week VARCHAR(16) NULL,
                confirm_month VARCHAR(16) NULL,
                atr_ticks DOUBLE NULL,
                prior_range_ticks DOUBLE NULL,
                ema_gap_atr DOUBLE NULL,
                ema_fast_slope_atr DOUBLE NULL,
                breakout_atr DOUBLE NULL,
                relative_volume DOUBLE NULL,
                compression DOUBLE NULL,
                close_location_prior_range DOUBLE NULL,
                xa_ticks DOUBLE NULL,
                cd_ticks DOUBLE NULL,
                full_pattern_length DOUBLE NULL,
                formula VARCHAR(128) NOT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                PRIMARY KEY (source_ai_run_id, setup_id, candidate_uid),
                INDEX idx_wave_candidates_year_result (valid_year, result_r),
                INDEX idx_wave_candidates_setup_score (source_ai_run_id, setup_id, signal_score),
                INDEX idx_wave_candidates_family (valid_year, pattern_family_key)
            )
            """
        )
    conn.commit()


def candidate_uid(entry_date: Any, direction: str) -> str:
    stamp = pd.Timestamp(entry_date).strftime("%Y%m%d%H%M")
    return f"{stamp}-{direction[0].lower()}"


def feature_row(
    source_ai_run_id: str,
    year: int,
    source_row: pd.Series,
    candles: pd.DataFrame,
    signal_idx: int,
    result: wave.WaveResult,
) -> dict[str, Any]:
    direction = result.trade_direction or "UNKNOWN"
    sign = wave.direction_sign(direction) if direction in {"LONG", "SHORT"} else 1.0
    signal = candles.iloc[signal_idx]
    tick_size = wave.tick_size_for(source_row.get("symbol"), source_row.get("root_symbol"))
    atr = wave.finite(signal.get("atr"), tick_size * 12.0) or tick_size * 12.0
    entry_high = wave.finite(signal.get("entry_high"), signal.get("close"))
    entry_low = wave.finite(signal.get("entry_low"), signal.get("close"))
    close = wave.finite(signal.get("close"), 0.0) or 0.0
    prior_range = wave.finite(signal.get("prior_range"), 0.0) or 0.0
    if prior_range > 0:
        close_location = (close - (entry_low or close)) / prior_range
    else:
        close_location = 0.5
    if direction == "LONG":
        breakout = (close - (entry_high or close)) / atr
    else:
        breakout = ((entry_low or close) - close) / atr
    source_direction = wave.normalize_direction(source_row.get("source_trade_direction"))
    d_confirm = pd.Timestamp(source_row["d_confirm_date"])
    return {
        "source_ai_run_id": source_ai_run_id,
        "valid_year": year,
        "setup_id": source_row.get("setup_id"),
        "candidate_uid": candidate_uid(result.entry_date, direction),
        "source_template_uid": source_row.get("source_template_uid"),
        "symbol": source_row.get("symbol"),
        "root_symbol": source_row.get("root_symbol") or wave.root_symbol(source_row.get("symbol")),
        "market": source_row.get("market"),
        "pattern_family_key": source_row.get("pattern_family_key"),
        "harmonic_type": source_row.get("pattern_family_harmonic_type") or source_row.get("harmonic_type"),
        "family_bin": source_row.get("pattern_family_bin"),
        "family_size_bucket": source_row.get("pattern_family_size_bucket"),
        "family_time_bin": source_row.get("pattern_family_time_bin"),
        "family_x_strictness": source_row.get("pattern_family_x_strictness"),
        "d_confirm_date": source_row.get("d_confirm_date"),
        "direction": direction,
        "source_trade_direction": source_direction,
        "signal_index": result.signal_index,
        "entry_index": result.entry_index,
        "exit_index": result.exit_index,
        "signal_date": candles["ts_utc"].iloc[signal_idx],
        "entry_date": result.entry_date,
        "exit_date": result.exit_date,
        "entry_price": result.entry_price,
        "stop_price": result.stop_price,
        "exit_price": result.exit_price,
        "risk_points": result.risk_points,
        "risk_ticks": result.risk_ticks,
        "tick_size": tick_size,
        "exit_reason": result.exit_reason,
        "result_r": result.result_r,
        "raw_result_r": result.raw_result_r,
        "mfe_r": result.mfe_r,
        "mae_r": result.mae_r,
        "hold_minutes": result.hold_minutes,
        "entry_delay_minutes": result.entry_delay_minutes,
        "signal_score": result.trigger_score,
        "source_predicted_expected_r": source_row.get("source_predicted_expected_r"),
        "source_score_margin_top2": source_row.get("source_score_margin_top2"),
        "direction_matches_source": 1 if source_direction and source_direction == direction else 0,
        "confirm_hour": int(d_confirm.hour),
        "confirm_day_of_week": str(d_confirm.dayofweek),
        "confirm_month": str(d_confirm.month),
        "atr_ticks": atr / tick_size if tick_size > 0 else None,
        "prior_range_ticks": prior_range / tick_size if tick_size > 0 else None,
        "ema_gap_atr": sign
        * ((wave.finite(signal.get("ema_fast"), close) or close) - (wave.finite(signal.get("ema_slow"), close) or close))
        / atr,
        "ema_fast_slope_atr": sign * (wave.finite(signal.get("ema_fast_slope"), 0.0) or 0.0) / atr,
        "breakout_atr": breakout,
        "relative_volume": wave.finite(signal.get("relative_volume"), 1.0),
        "compression": wave.finite(signal.get("compression"), 2.0),
        "close_location_prior_range": close_location,
        "xa_ticks": abs(wave.finite(source_row.get("xa_price_length"), 0.0) or 0.0) / tick_size if tick_size > 0 else None,
        "cd_ticks": abs(wave.finite(source_row.get("cd_price_length"), 0.0) or 0.0) / tick_size if tick_size > 0 else None,
        "full_pattern_length": wave.finite(source_row.get("full_pattern_length"), 0.0),
        "formula": "wave_candidate_ema8_21_breakout_trail_market_slip_v1",
    }


def insert_candidates(conn, rows: list[dict[str, Any]]) -> None:
    if not rows:
        return
    columns = list(rows[0].keys())
    placeholders = ", ".join(["%s"] * len(columns))
    updates = ", ".join(
        [f"{column}=VALUES({column})" for column in columns if column not in {"source_ai_run_id", "setup_id", "candidate_uid"}]
        + ["updated_at=CURRENT_TIMESTAMP"]
    )
    sql = f"""
        INSERT INTO {CANDIDATE_TABLE} ({", ".join(columns)})
        VALUES ({placeholders})
        ON DUPLICATE KEY UPDATE {updates}
    """
    values = [tuple(wave.clean(row.get(column)) for column in columns) for row in rows]
    with conn.cursor() as cur:
        cur.executemany(sql, values)


def delete_candidates_for_run(conn, source_ai_run_id: str) -> None:
    with conn.cursor() as cur:
        cur.execute(f"DELETE FROM {CANDIDATE_TABLE} WHERE source_ai_run_id = %s", (source_ai_run_id,))
    conn.commit()


def candidate_count(conn, source_ai_run_id: str) -> int:
    with conn.cursor() as cur:
        cur.execute(f"SELECT COUNT(*) AS count FROM {CANDIDATE_TABLE} WHERE source_ai_run_id = %s", (source_ai_run_id,))
        return int(cur.fetchone()["count"])


def build_candidates_for_year(conn, year: int, args: argparse.Namespace) -> dict[str, Any]:
    source_ai_run_id = wave.safe_run_id(args.source_prefix, year)
    existing = candidate_count(conn, source_ai_run_id)
    if existing and not args.replace_candidates:
        return {"year": year, "source_ai_run_id": source_ai_run_id, "candidates": existing, "rebuilt": False}

    if args.replace_candidates:
        delete_candidates_for_run(conn, source_ai_run_id)

    rows = wave.fetch_source_rows(conn, source_ai_run_id, 0)
    pending: list[dict[str, Any]] = []
    total_candidates = 0
    for index, source_row in rows.iterrows():
        d_confirm = pd.Timestamp(source_row["d_confirm_date"])
        start_dt = d_confirm - pd.Timedelta(minutes=args.indicator_lookback_minutes)
        end_dt = d_confirm + pd.Timedelta(minutes=args.max_forward_minutes + args.entry_scan_minutes + 10)
        candles = wave.fetch_candles(conn, str(source_row["symbol"]), start_dt, end_dt)
        candles = wave.prepare_candles(candles, args)
        if candles.empty:
            continue
        candidates = wave.plausible_oracle_candidates(source_row, candles, args)
        tick_size = wave.tick_size_for(source_row.get("symbol"), source_row.get("root_symbol"))
        for score, entry_idx, direction in candidates:
            result = wave.simulate_from_entry(
                candles,
                pd.Timestamp(source_row["d_confirm_date"]),
                entry_idx,
                entry_idx - 1,
                direction,
                tick_size,
                args,
            )
            if result.outcome == "no_entry":
                continue
            result = wave.WaveResult(**{**result.__dict__, "trigger_score": score, "candidate_count": len(candidates)})
            pending.append(feature_row(source_ai_run_id, year, source_row, candles, entry_idx - 1, result))
            total_candidates += 1
        if len(pending) >= 1000:
            insert_candidates(conn, pending)
            conn.commit()
            pending.clear()
        if (index + 1) % 50 == 0:
            print(f"  {source_ai_run_id}: built candidates for {index + 1:,}/{len(rows):,} setups")
    insert_candidates(conn, pending)
    conn.commit()
    return {"year": year, "source_ai_run_id": source_ai_run_id, "candidates": total_candidates, "rebuilt": True}


def load_candidates(conn, run_ids: list[str]) -> pd.DataFrame:
    if not run_ids:
        return pd.DataFrame()
    placeholders = ", ".join(["%s"] * len(run_ids))
    context_timeframes = ["5m", "15m", "1h", "4h"]
    joins = "\n".join(
        f"""
            LEFT JOIN ai_stage1_trade_market_context ctx_{timeframe.replace("h", "h").replace("m", "m")}
              ON ctx_{timeframe.replace("h", "h").replace("m", "m")}.multi_valid_eval_run_id = c.source_ai_run_id
             AND ctx_{timeframe.replace("h", "h").replace("m", "m")}.setup_id = c.setup_id
             AND ctx_{timeframe.replace("h", "h").replace("m", "m")}.template_uid = c.source_template_uid
             AND ctx_{timeframe.replace("h", "h").replace("m", "m")}.timeframe = '{timeframe}'
        """
        for timeframe in context_timeframes
    )
    context_columns = []
    for timeframe in context_timeframes:
        alias = f"ctx_{timeframe}"
        prefix = f"ctx_{timeframe}_"
        context_columns.extend(
            [
                f"{alias}.trend_label AS {prefix}trend_label",
                f"{alias}.trend_strength_pct AS {prefix}trend_strength_pct",
                f"{alias}.linreg_slope_pct_per_candle AS {prefix}linreg_slope_pct_per_candle",
                f"{alias}.relative_volume_20 AS {prefix}relative_volume_20",
                f"{alias}.range_position_pct AS {prefix}range_position_pct",
                f"{alias}.support_distance_r AS {prefix}support_distance_r",
                f"{alias}.resistance_distance_r AS {prefix}resistance_distance_r",
            ]
        )
    context_select = ",\n                " + ",\n                ".join(context_columns)
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT
                c.*
                {context_select}
            FROM {CANDIDATE_TABLE} c
            {joins}
            WHERE c.source_ai_run_id IN ({placeholders})
            ORDER BY c.valid_year, c.setup_id, c.entry_date, c.direction
            """,
            run_ids,
        )
        rows = cur.fetchall()
    frame = pd.DataFrame(rows)
    if frame.empty:
        return frame
    for col in ["d_confirm_date", "signal_date", "entry_date", "exit_date"]:
        frame[col] = pd.to_datetime(frame[col], errors="coerce")
    for col in NUM_FEATURES + ["result_r", "raw_result_r", "mfe_r", "mae_r"]:
        frame[col] = pd.to_numeric(frame[col], errors="coerce")
    for timeframe in context_timeframes:
        trend_col = f"ctx_{timeframe}_trend_label"
        alignment_col = f"ctx_{timeframe}_alignment"
        frame[alignment_col] = [
            trend_alignment(direction, trend)
            for direction, trend in zip(frame["direction"], frame.get(trend_col, pd.Series([""] * len(frame))))
        ]
    return frame


def trend_alignment(direction: object, trend: object) -> str:
    trade_direction = wave.normalize_direction(direction)
    label = str(trend or "").strip().lower()
    if not trade_direction or label in {"", "none", "nan"}:
        return "unknown"
    if label == "neutral":
        return "neutral"
    if trade_direction == "LONG" and label == "bullish":
        return "with"
    if trade_direction == "SHORT" and label == "bearish":
        return "with"
    if label in {"bullish", "bearish"}:
        return "fighting"
    return "unknown"


def prepare_pool(frame: pd.DataFrame, include_target: bool = True) -> Pool:
    work = frame.copy()
    for col in CAT_FEATURES:
        work[col] = work[col].fillna("Unknown").astype(str)
    for col in NUM_FEATURES:
        work[col] = pd.to_numeric(work[col], errors="coerce").replace([np.inf, -np.inf], np.nan).fillna(0.0)
    features = CAT_FEATURES + NUM_FEATURES
    if include_target:
        target = pd.to_numeric(work["result_r"], errors="coerce").fillna(0.0)
        return Pool(work[features], label=target, cat_features=CAT_FEATURES)
    return Pool(work[features], cat_features=CAT_FEATURES)


def train_model(frame: pd.DataFrame, args: argparse.Namespace) -> CatBoostRegressor:
    model = CatBoostRegressor(
        loss_function="RMSE",
        iterations=args.iterations,
        depth=args.depth,
        learning_rate=args.learning_rate,
        l2_leaf_reg=args.l2_leaf_reg,
        random_seed=args.random_seed,
        verbose=False,
        allow_writing_files=False,
    )
    model.fit(prepare_pool(frame, include_target=True))
    return model


def add_predictions(frame: pd.DataFrame, model: CatBoostRegressor) -> pd.DataFrame:
    scored = frame.copy()
    scored["predicted_wave_r"] = model.predict(prepare_pool(scored, include_target=False))
    scored["candidate_rank"] = scored.groupby("setup_id")["predicted_wave_r"].rank(method="first", ascending=False)
    return scored


def top_per_setup(scored: pd.DataFrame) -> pd.DataFrame:
    if scored.empty:
        return scored
    ordered = scored.sort_values(["setup_id", "predicted_wave_r", "signal_score"], ascending=[True, False, False])
    return ordered.drop_duplicates(subset=["setup_id"], keep="first").reset_index(drop=True)


def summarize_selection(
    top: pd.DataFrame,
    threshold: float | None,
    total_setups: int | None = None,
    allow_duplicate_entry: bool = True,
) -> dict[str, Any]:
    selected = top.copy() if threshold is None else top[top["predicted_wave_r"] >= threshold].copy()
    if not allow_duplicate_entry and not selected.empty:
        selected = selected.sort_values(["predicted_wave_r", "result_r"], ascending=[False, False])
        selected = selected.drop_duplicates(subset=["symbol", "entry_date", "direction"], keep="first")
    result = pd.to_numeric(selected["result_r"], errors="coerce").fillna(0.0)
    wins = int((result > 0.0).sum())
    count = int(len(result))
    return {
        "threshold": threshold,
        "setups": int(total_setups or top["setup_id"].nunique()),
        "selected": count,
        "no_entries": int((total_setups or top["setup_id"].nunique()) - count),
        "wins": wins,
        "losses": int(count - wins),
        "win_rate": wins / count if count else 0.0,
        "avg_r": float(result.mean()) if count else 0.0,
        "sum_r": float(result.sum()) if count else 0.0,
    }


def choose_threshold(conn, args: argparse.Namespace) -> float | None:
    if args.threshold is not None:
        return args.threshold
    threshold_year = args.threshold_year
    train_years = [year for year in wave.years_from_arg(args.train_years) if year < threshold_year]
    if not train_years:
        print("No earlier years for threshold selection; using no score threshold.")
        return None
    train_ids = [wave.safe_run_id(args.source_prefix, year) for year in train_years]
    threshold_id = wave.safe_run_id(args.source_prefix, threshold_year)
    train = load_candidates(conn, train_ids)
    holdout = load_candidates(conn, [threshold_id])
    if train.empty or holdout.empty:
        print("Missing candidate rows for threshold selection; using no score threshold.")
        return None
    model = train_model(train, args)
    top = top_per_setup(add_predictions(holdout, model))
    source_rows = wave.fetch_source_rows(conn, threshold_id, 0)
    total_setups = int(source_rows["setup_id"].nunique()) if not source_rows.empty else int(top["setup_id"].nunique())

    candidates = sorted(set(float(value) for value in top["predicted_wave_r"].quantile(np.linspace(0.0, 0.95, 20)).dropna()))
    candidates = [None] + candidates
    summaries = [
        summarize_selection(top, value, total_setups, allow_duplicate_entry=args.allow_duplicate_entry)
        for value in candidates
    ]
    viable = [item for item in summaries if item["selected"] >= args.min_threshold_trades]
    if not viable:
        viable = summaries
    best = max(viable, key=lambda item: (item["sum_r"], item["avg_r"], item["selected"]))
    print("Threshold calibration on", threshold_year)
    for item in sorted(summaries, key=lambda row: row["sum_r"], reverse=True)[:6]:
        label = "all" if item["threshold"] is None else f"{item['threshold']:.3f}"
        print(
            f"  threshold {label}: selected={item['selected']}, no-entry={item['no_entries']}, "
            f"win={item['win_rate'] * 100:.2f}%, avg={item['avg_r']:.3f}R, sum={item['sum_r']:.1f}R"
        )
    label = "all" if best["threshold"] is None else f"{best['threshold']:.3f}"
    print(f"Selected threshold: {label}")
    return best["threshold"]


def wave_result_from_candidate(row: pd.Series) -> wave.WaveResult:
    return wave.WaveResult(
        outcome="pass" if float(row["result_r"]) > 0.0 else "fail",
        exit_reason=str(row.get("exit_reason") or "model_exit"),
        trade_direction=str(row.get("direction") or "LONG"),
        entry_date=row.get("entry_date"),
        exit_date=row.get("exit_date"),
        entry_price=wave.finite(row.get("entry_price")),
        stop_price=wave.finite(row.get("stop_price")),
        target_price=None,
        exit_price=wave.finite(row.get("exit_price")),
        risk_points=wave.finite(row.get("risk_points")),
        result_r=float(row.get("result_r") or 0.0),
        raw_result_r=wave.finite(row.get("raw_result_r")),
        mfe_r=wave.finite(row.get("mfe_r")),
        mae_r=wave.finite(row.get("mae_r")),
        signal_index=int(row["signal_index"]) if not pd.isna(row.get("signal_index")) else None,
        entry_index=int(row["entry_index"]) if not pd.isna(row.get("entry_index")) else None,
        exit_index=int(row["exit_index"]) if not pd.isna(row.get("exit_index")) else None,
        entry_delay_minutes=int(row["entry_delay_minutes"]) if not pd.isna(row.get("entry_delay_minutes")) else None,
        hold_minutes=int(row["hold_minutes"]) if not pd.isna(row.get("hold_minutes")) else None,
        risk_ticks=wave.finite(row.get("risk_ticks")),
        trigger_score=wave.finite(row.get("signal_score")),
        candidate_count=1,
        no_entry_reason=None,
    )


def materialize_model_run(conn, model: CatBoostRegressor, threshold: float | None, args: argparse.Namespace) -> dict[str, Any]:
    year = args.valid_year
    source_ai_run_id = wave.safe_run_id(args.source_prefix, year)
    source_run = wave.fetch_run(conn, source_ai_run_id)
    source_rows = wave.fetch_source_rows(conn, source_ai_run_id, 0)
    candidates = load_candidates(conn, [source_ai_run_id])
    scored = add_predictions(candidates, model) if not candidates.empty else candidates
    top = top_per_setup(scored)
    candidate_reason_by_setup: dict[str, str] = {}
    if not top.empty:
        eligible = top.copy()
        if threshold is not None:
            below = eligible[eligible["predicted_wave_r"] < threshold]
            candidate_reason_by_setup.update({str(row.setup_id): "model_score_below_threshold" for row in below.itertuples()})
            eligible = eligible[eligible["predicted_wave_r"] >= threshold]
        if not args.allow_duplicate_entry and not eligible.empty:
            eligible = eligible.sort_values(["predicted_wave_r", "result_r"], ascending=[False, False])
            duplicate_mask = eligible.duplicated(subset=["symbol", "entry_date", "direction"], keep="first")
            duplicate_rows = eligible[duplicate_mask]
            candidate_reason_by_setup.update({str(row.setup_id): "model_duplicate_entry_suppressed" for row in duplicate_rows.itertuples()})
            eligible = eligible[~duplicate_mask]
        top_by_setup = {str(row.setup_id): row for row in eligible.itertuples(index=False)}
    else:
        top_by_setup = {}

    run_id = wave.safe_run_id(args.run_prefix, year)
    source_run_id = wave.safe_run_id(args.source_run_prefix, year)
    result_table = wave.result_table_for(args.source_run_prefix, year)
    template_uid = f"tpl-{args.source_run_prefix}"
    template_name = "Wave Rider Model V1"
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id
    model_dir.mkdir(parents=True, exist_ok=True)
    model_file = model_dir / "catboost_model.cbm"
    metadata_file = model_dir / "metadata.json"
    model.save_model(str(model_file))
    metadata_file.write_text(
        json.dumps(
            {
                "run_id": run_id,
                "source_ai_prefix": args.source_prefix,
                "source_run_id": source_run_id,
                "valid_year": year,
                "train_years": wave.years_from_arg(args.train_years),
                "threshold_year": args.threshold_year,
                "threshold": threshold,
                "allow_duplicate_entry": bool(args.allow_duplicate_entry),
                "cat_features": CAT_FEATURES,
                "num_features": NUM_FEATURES,
                "iterations": args.iterations,
                "depth": args.depth,
                "learning_rate": args.learning_rate,
                "l2_leaf_reg": args.l2_leaf_reg,
                "random_seed": args.random_seed,
                "entry_scan_minutes": args.entry_scan_minutes,
                "max_forward_minutes": args.max_forward_minutes,
                "min_risk_ticks": args.min_risk_ticks,
                "max_risk_ticks": args.max_risk_ticks,
                "slippage_entry_ticks": args.slippage_entry_ticks,
                "slippage_exit_ticks": args.slippage_exit_ticks,
                "profit_lock_trigger_r": args.profit_lock_trigger_r,
                "profit_lock_giveback_r": args.profit_lock_giveback_r,
                "profit_lock_min_r": args.profit_lock_min_r,
                "time_exit_minutes": args.time_exit_minutes,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )
    wave.ensure_result_table(conn, result_table)
    wave.ensure_diagnostics_table(conn)

    with conn.cursor() as cur:
        cur.execute("SELECT COUNT(*) AS count FROM ai_stage1_multi_valid_eval_runs WHERE multi_valid_eval_run_id = %s", (run_id,))
        exists = int(cur.fetchone()["count"]) > 0
    if exists and not args.replace_run:
        raise RuntimeError(f"Run already exists. Use --replace-run to rebuild it: {run_id}")
    wave.delete_existing(conn, run_id, source_run_id, result_table)

    result_rows: list[dict[str, Any]] = []
    selected_rows: list[dict[str, Any]] = []
    diagnostics: list[dict[str, Any]] = []
    selected_count = 0
    for _, source_row in source_rows.iterrows():
        setup_id = str(source_row.get("setup_id"))
        top_row = top_by_setup.get(setup_id)
        if top_row is None:
            original_top = top[top["setup_id"].astype(str) == setup_id] if not top.empty else pd.DataFrame()
            result = wave.no_entry(candidate_reason_by_setup.get(setup_id, "model_no_candidate"))
            predicted = float(original_top["predicted_wave_r"].iloc[0]) if not original_top.empty else None
        else:
            candidate = pd.Series(top_row._asdict())
            predicted = float(candidate.get("predicted_wave_r"))
            result = wave_result_from_candidate(candidate)
            selected_count += 1
        result_rows.append(wave.result_row(source_run_id, template_uid, source_row, result))
        selected = wave.selected_row(template_uid, template_name, source_row, result)
        selected["predicted_expected_r"] = predicted
        selected["score_margin_top2"] = predicted
        selected_rows.append(selected)
        diagnostics.append(wave.diagnostic_row(run_id, source_ai_run_id, source_run_id, template_uid, "model", source_row, result))

    wave.insert_result_rows(conn, result_table, result_rows)
    wave.insert_run_record(conn, source_run, run_id, source_run_id, result_table, year, "model", args)
    with conn.cursor() as cur:
        cur.execute(
            "UPDATE ai_stage1_multi_valid_eval_runs SET model_path = %s WHERE multi_valid_eval_run_id = %s",
            (str(model_dir), run_id),
        )
    wave.insert_selected_rows(conn, run_id, selected_rows)
    summary = wave.insert_filter_summaries(conn, run_id, selected_rows, "wave_rider_model_v1")
    wave.insert_diagnostics(conn, diagnostics)
    conn.commit()
    summary.update(
        {
            "year": year,
            "run_id": run_id,
            "source_run_id": source_run_id,
            "result_table": result_table,
            "model_selected": selected_count,
            "threshold": threshold,
        }
    )
    return summary


def main() -> int:
    args = parse_args()
    candidate_years = wave.years_from_arg(args.candidate_years)
    train_years = wave.years_from_arg(args.train_years)
    conn = wave.connect()
    try:
        ensure_candidate_table(conn)
        if not args.skip_candidate_build:
            print("Building/storing wave candidate rows:")
            for year in candidate_years:
                summary = build_candidates_for_year(conn, year, args)
                action = "rebuilt" if summary["rebuilt"] else "existing"
                print(f"  {summary['year']}: {summary['source_ai_run_id']} -> {summary['candidates']:,} candidates ({action})")

        threshold = choose_threshold(conn, args)
        train_ids = [wave.safe_run_id(args.source_prefix, year) for year in train_years]
        train = load_candidates(conn, train_ids)
        if train.empty:
            raise RuntimeError("No training candidates found.")
        print(f"Training candidate model on {len(train):,} rows from {train_years}")
        model = train_model(train, args)
        summary = materialize_model_run(conn, model, threshold, args)
    except Exception:
        conn.rollback()
        raise
    finally:
        conn.close()

    label = "all" if summary["threshold"] is None else f"{summary['threshold']:.3f}"
    print(
        f"Materialized {summary['run_id']} threshold={label}: {summary['model_selected']:,} model entries, "
        f"{summary['wins']:,} wins / {summary['losses']:,} losses / {summary['no_entries']:,} no-entry, "
        f"{summary['win_rate'] * 100:.2f}% win, {summary['avg_r']:.3f}R avg, {summary['sum_r']:.1f}R sum"
    )
    if not args.skip_api_refresh:
        wave.api_refresh(summary["run_id"], int(summary["year"]), args.api_refresh_url)
    return 0


if __name__ == "__main__":
    sys.exit(main())
