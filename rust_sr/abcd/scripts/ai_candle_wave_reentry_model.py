#!/usr/bin/env python3
"""
Train a candle-wave re-entry layer after an exit overlay.

This layer does not replace the entry model or the 180-bar exit model. It starts
after the exit model has closed a trade and asks whether the original trend
repaired enough to justify one same-direction re-entry.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
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

import ai_candle_wave_exit_model as exit_model
import ai_candle_wave_scanner_model as scanner
import ai_wave_rider_research as wave


RUN_TABLE = "ai_candle_wave_reentry_model_runs"
TRADE_TABLE = "ai_candle_wave_reentry_model_trades"

DEFAULT_EXIT_RUN = "aicw-exit-dyn180s2-srcfb-v1-2m-2026"

REENTRY_CAT_FEATURES = list(
    dict.fromkeys(
        [
            "symbol",
            "root_symbol",
            "direction",
            "baseline_exit_reason",
            "model_exit_reason",
            *scanner.CAT_FEATURES,
        ]
    )
)

REENTRY_DECISION_NUM_FEATURES = [
    "source_predicted_r",
    "source_baseline_result_r",
    "source_model_result_r",
    "source_delta_r",
    "source_exit_score_r",
    "source_baseline_hold_minutes",
    "source_model_hold_minutes",
    "bars_after_exit",
    "minutes_after_exit",
    "price_from_exit_source_r",
    "post_exit_mfe_source_r",
    "post_exit_mae_source_r",
    "pullback_depth_source_r",
    "recovery_from_pullback_source_r",
    "signed_ret_3_source_r",
    "signed_ret_6_source_r",
    "range_6_source_r",
    "directional_close_position_6",
    "body_reentry_r",
    "upper_wick_reentry_r",
    "lower_wick_reentry_r",
    "volume_vs_exit",
    "reentry_risk_ticks",
    "reentry_risk_vs_source_r",
    "distance_to_reentry_stop_r",
]

REENTRY_NUM_FEATURES = list(dict.fromkeys([*scanner.NUM_FEATURES, *REENTRY_DECISION_NUM_FEATURES]))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exit-model-run-id", default=DEFAULT_EXIT_RUN)
    parser.add_argument("--run-prefix", default="aicw-reentry-dyn180-v1")
    parser.add_argument("--train-years", default="2024")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--replace-run", action="store_true")
    parser.add_argument("--max-train-trades", type=int, default=0)
    parser.add_argument("--max-threshold-trades", type=int, default=0)
    parser.add_argument("--max-valid-trades", type=int, default=0)
    parser.add_argument("--reentry-threshold", type=float, default=None)
    parser.add_argument("--min-threshold-trades", type=int, default=12)
    parser.add_argument("--threshold-dd-penalty", type=float, default=0.0)
    parser.add_argument("--watch-bars", type=int, default=120)
    parser.add_argument("--candidate-step-bars", type=int, default=2)
    parser.add_argument("--min-bars-after-exit", type=int, default=2)
    parser.add_argument("--breakout-bars", type=int, default=8)
    parser.add_argument("--stop-lookback-bars", type=int, default=10)
    parser.add_argument("--stop-pad-ticks", type=float, default=2.0)
    parser.add_argument("--min-reentry-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-reentry-risk-ticks", type=float, default=220.0)
    parser.add_argument("--min-hold-bars", type=int, default=6)
    parser.add_argument("--death-bars", type=int, default=3)
    parser.add_argument("--time-exit-bars", type=int, default=90)
    parser.add_argument("--profit-lock-trigger-r", type=float, default=1.0)
    parser.add_argument("--profit-lock-giveback-r", type=float, default=0.90)
    parser.add_argument("--slippage-entry-ticks", type=float, default=3.0)
    parser.add_argument("--slippage-exit-ticks", type=float, default=3.0)
    parser.add_argument("--iterations", type=int, default=550)
    parser.add_argument("--depth", type=int, default=5)
    parser.add_argument("--learning-rate", type=float, default=0.045)
    parser.add_argument("--l2-leaf-reg", type=float, default=12.0)
    parser.add_argument("--random-seed", type=int, default=97)
    return parser.parse_args()


def reentry_run_id(args: argparse.Namespace, timeframe: str) -> str:
    run_id = f"{args.run_prefix}-{timeframe}-{args.valid_year}"
    if len(run_id) > 64:
        raise ValueError(f"Run id too long: {run_id}")
    return run_id


def safe_float(value: Any, default: float = 0.0) -> float:
    try:
        result = float(value)
    except (TypeError, ValueError):
        return default
    if not math.isfinite(result):
        return default
    return result


def load_exit_run(conn, exit_model_run_id: str) -> dict[str, Any]:
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT exit_model_run_id, source_model_run_id, timeframe, exit_threshold,
                   model_path, metadata_json
            FROM {exit_model.RUN_TABLE}
            WHERE exit_model_run_id = %s
            """,
            (exit_model_run_id,),
        )
        row = cur.fetchone()
    if not row:
        raise ValueError(f"Exit model run not found: {exit_model_run_id}")
    row["metadata"] = json.loads(row.get("metadata_json") or "{}")
    return row


def load_exit_catboost_model(exit_run: dict[str, Any]) -> CatBoostRegressor:
    model_path = Path(str(exit_run.get("model_path") or "")) / "catboost_exit_model.cbm"
    if not model_path.exists():
        model_path = wave.ABCD_ROOT / "model_registry" / str(exit_run["exit_model_run_id"]) / "catboost_exit_model.cbm"
    if not model_path.exists():
        raise FileNotFoundError(f"Missing exit CatBoost model: {model_path}")
    model = CatBoostRegressor()
    model.load_model(str(model_path))
    return model


def exit_args_from_metadata(metadata: dict[str, Any]) -> argparse.Namespace:
    return argparse.Namespace(
        mode=metadata.get("mode", "dynamic"),
        min_hold_bars=int(metadata.get("min_hold_bars") or 4),
        decision_step_bars=int(metadata.get("decision_step_bars") or 2),
        dynamic_max_bars=int(metadata.get("dynamic_max_bars") or 180),
        dynamic_no_signal_exit=metadata.get("dynamic_no_signal_exit") or "source",
    )


def normalize_overlay_dates(frame: pd.DataFrame) -> pd.DataFrame:
    if frame.empty:
        return frame
    for col in ["entry_date", "exit_date", "exit_entry_date", "exit_baseline_exit_date", "exit_model_exit_date"]:
        if col in frame.columns:
            frame[col] = pd.to_datetime(frame[col], errors="coerce")
    for col in [
        "predicted_r",
        "risk_points",
        "risk_ticks",
        "tick_size",
        "exit_baseline_result_r",
        "exit_model_result_r",
        "exit_delta_r",
        "exit_exit_score_r",
        "exit_baseline_hold_minutes",
        "exit_model_hold_minutes",
        *scanner.NUM_FEATURES,
    ]:
        if col in frame.columns:
            frame[col] = pd.to_numeric(frame[col], errors="coerce")
    return frame


def selected_trades_for_years(
    conn,
    entry_model: CatBoostRegressor,
    entry_args: argparse.Namespace,
    source_model_run_id: str,
    years: list[int],
    valid_year: int,
    limit: int,
) -> pd.DataFrame:
    if years == [valid_year]:
        return exit_model.load_selected_trades(conn, source_model_run_id, limit)
    return exit_model.selected_like_trades(conn, entry_model, entry_args, years, limit)


def build_exit_overlay(
    conn,
    trades: pd.DataFrame,
    exit_cb_model: CatBoostRegressor,
    exit_threshold: float | None,
    exit_args: argparse.Namespace,
    table_name: str,
    timeframe_minutes: int,
    label: str,
) -> pd.DataFrame:
    if trades.empty:
        return trades
    decisions = exit_model.build_decision_rows(conn, trades, table_name, timeframe_minutes, exit_args, False, label)
    decisions = exit_model.add_exit_predictions(decisions, exit_cb_model)
    overlay = exit_model.apply_overlay(trades, decisions, exit_threshold, exit_args)
    overlay = overlay.rename(columns={col: f"exit_{col}" for col in overlay.columns if col != "candidate_uid"})
    merged = trades.merge(overlay, on="candidate_uid", how="inner")
    return normalize_overlay_dates(merged)


def close_position(values: pd.Series, idx: int, lookback: int) -> float:
    return exit_model.close_position(values, idx, lookback)


def reentry_uid(source_uid: str, entry_date: Any, direction: str, risk_ticks: float) -> str:
    raw = f"{source_uid}|{entry_date}|{direction}|{risk_ticks:.2f}"
    return hashlib.sha1(raw.encode("utf-8")).hexdigest()[:40]


def simulate_reentry(
    candles: pd.DataFrame,
    entry_idx: int,
    entry_price: float,
    stop_price: float,
    risk_points: float,
    direction: str,
    tick_size: float,
    timeframe_minutes: int,
    args: argparse.Namespace,
) -> dict[str, Any] | None:
    if risk_points <= 0 or tick_size <= 0 or entry_idx >= len(candles):
        return None

    sign = wave.direction_sign(direction)
    max_idx = min(len(candles) - 1, entry_idx + max(2, int(args.time_exit_bars)))
    slippage_r = ((float(args.slippage_entry_ticks) + float(args.slippage_exit_ticks)) * tick_size) / risk_points
    mfe = 0.0
    mae = 0.0
    adverse_count = 0
    exit_idx = max_idx
    exit_price = safe_float(candles["close"].iloc[max_idx])
    exit_reason = "reentry_time_exit"

    for idx in range(entry_idx, max_idx + 1):
        candle = candles.iloc[idx]
        high = safe_float(candle["high"])
        low = safe_float(candle["low"])
        close = safe_float(candle["close"])

        if direction == "LONG" and low <= stop_price:
            exit_idx = idx
            exit_price = stop_price
            exit_reason = "reentry_hard_stop"
            break
        if direction == "SHORT" and high >= stop_price:
            exit_idx = idx
            exit_price = stop_price
            exit_reason = "reentry_hard_stop"
            break

        favorable = (high - entry_price) / risk_points if direction == "LONG" else (entry_price - low) / risk_points
        adverse = (entry_price - low) / risk_points if direction == "LONG" else (high - entry_price) / risk_points
        mfe = max(mfe, favorable)
        mae = max(mae, adverse)

        if idx <= entry_idx:
            continue
        previous_close = safe_float(candles["close"].iloc[idx - 1])
        adverse_count = adverse_count + 1 if sign * (close - previous_close) < 0 else 0
        bars_held = idx - entry_idx
        if bars_held < int(args.min_hold_bars):
            continue

        current_raw = sign * (close - entry_price) / risk_points
        giveback = max(0.0, mfe - current_raw)
        should_profit_lock = (
            mfe >= float(args.profit_lock_trigger_r)
            and giveback >= float(args.profit_lock_giveback_r)
        )
        should_death_exit = adverse_count >= int(args.death_bars)
        if should_profit_lock or should_death_exit:
            next_idx = min(idx + 1, len(candles) - 1)
            exit_idx = next_idx
            exit_price = safe_float(candles["open"].iloc[next_idx]) if next_idx > idx else close
            exit_reason = "reentry_profit_giveback" if should_profit_lock else "reentry_trend_death"
            break

    result_r = sign * (exit_price - entry_price) / risk_points - slippage_r
    exit_date = candles["ts_utc"].iloc[exit_idx]
    entry_date = candles["ts_utc"].iloc[entry_idx]
    hold_minutes = int((pd.Timestamp(exit_date) - pd.Timestamp(entry_date)).total_seconds() / 60)
    return {
        "reentry_exit_date": exit_date,
        "reentry_exit_price": exit_price,
        "reentry_exit_reason": exit_reason,
        "reentry_result_r": result_r,
        "reentry_raw_result_r": sign * (exit_price - entry_price) / risk_points,
        "reentry_mfe_r": mfe,
        "reentry_mae_r": mae,
        "reentry_hold_minutes": hold_minutes,
    }


def build_reentry_candidates_for_trade(
    conn,
    row: pd.Series,
    table_name: str,
    timeframe_minutes: int,
    args: argparse.Namespace,
) -> list[dict[str, Any]]:
    exit_date = pd.Timestamp(row.get("exit_model_exit_date"))
    if pd.isna(exit_date):
        return []

    symbol = str(row.get("symbol") or "")
    direction = str(row.get("direction") or "").upper()
    if not symbol or direction not in {"LONG", "SHORT"}:
        return []

    source_risk = safe_float(row.get("risk_points"))
    tick_size = safe_float(row.get("tick_size"))
    exit_price = safe_float(row.get("exit_model_exit_price"))
    if source_risk <= 0 or tick_size <= 0 or exit_price <= 0:
        return []

    lookback_bars = max(int(args.breakout_bars), int(args.stop_lookback_bars), 12)
    end_extra = max(int(args.watch_bars), 1) + max(int(args.time_exit_bars), 1) + 5
    start_at = exit_date - pd.Timedelta(minutes=timeframe_minutes * lookback_bars)
    end_at = exit_date + pd.Timedelta(minutes=timeframe_minutes * end_extra)
    candles = exit_model.load_candles(conn, table_name, symbol, start_at, end_at)
    if candles.empty:
        return []

    exit_idx = exit_model.index_at_or_after(candles["ts_utc"], exit_date)
    if exit_idx is None:
        return []

    sign = wave.direction_sign(direction)
    rows: list[dict[str, Any]] = []
    high_values = candles["high"]
    low_values = candles["low"]
    close_values = candles["close"]
    volume_exit = max(1.0, safe_float(candles["volume"].iloc[exit_idx], 1.0))
    max_decision_idx = min(len(candles) - 2, exit_idx + max(1, int(args.watch_bars)))
    start_decision_idx = exit_idx + max(1, int(args.min_bars_after_exit))
    step = max(1, int(args.candidate_step_bars))

    for decision_idx in range(start_decision_idx, max_decision_idx + 1, step):
        entry_idx = decision_idx + 1
        if entry_idx >= len(candles):
            break

        close_price = safe_float(close_values.iloc[decision_idx])
        if close_price <= 0:
            continue
        breakout_start = max(exit_idx, decision_idx - int(args.breakout_bars))
        if decision_idx <= breakout_start:
            continue
        prior_high = safe_float(high_values.iloc[breakout_start:decision_idx].max())
        prior_low = safe_float(low_values.iloc[breakout_start:decision_idx].min())
        signed_ret_3 = sign * (close_price - safe_float(close_values.iloc[max(exit_idx, decision_idx - 3)])) / source_risk
        directional_position = close_position(close_values, decision_idx, 6)
        if direction == "SHORT":
            directional_position = 1.0 - directional_position

        repaired = (
            (direction == "LONG" and close_price >= prior_high and signed_ret_3 > 0.0 and directional_position >= 0.55)
            or (direction == "SHORT" and close_price <= prior_low and signed_ret_3 > 0.0 and directional_position >= 0.55)
        )
        if not repaired:
            continue

        entry_price = safe_float(candles["open"].iloc[entry_idx])
        if entry_price <= 0:
            continue
        stop_start = max(exit_idx, decision_idx - int(args.stop_lookback_bars) + 1)
        pad = float(args.stop_pad_ticks) * tick_size
        if direction == "LONG":
            stop_price = safe_float(low_values.iloc[stop_start : decision_idx + 1].min()) - pad
            min_stop = entry_price - float(args.min_reentry_risk_ticks) * tick_size
            stop_price = min(stop_price, min_stop)
            risk_points = entry_price - stop_price
        else:
            stop_price = safe_float(high_values.iloc[stop_start : decision_idx + 1].max()) + pad
            min_stop = entry_price + float(args.min_reentry_risk_ticks) * tick_size
            stop_price = max(stop_price, min_stop)
            risk_points = stop_price - entry_price
        risk_ticks = risk_points / tick_size if tick_size > 0 else 0.0
        if risk_ticks < float(args.min_reentry_risk_ticks) or risk_ticks > float(args.max_reentry_risk_ticks):
            continue

        result = simulate_reentry(
            candles,
            entry_idx,
            entry_price,
            stop_price,
            risk_points,
            direction,
            tick_size,
            timeframe_minutes,
            args,
        )
        if result is None:
            continue

        post_high = safe_float(high_values.iloc[exit_idx : decision_idx + 1].max())
        post_low = safe_float(low_values.iloc[exit_idx : decision_idx + 1].min())
        if direction == "LONG":
            post_mfe = (post_high - exit_price) / source_risk
            post_mae = (exit_price - post_low) / source_risk
            pullback_depth = max(0.0, (exit_price - post_low) / source_risk)
            recovery_from_pullback = (close_price - post_low) / source_risk
        else:
            post_mfe = (exit_price - post_low) / source_risk
            post_mae = (post_high - exit_price) / source_risk
            pullback_depth = max(0.0, (post_high - exit_price) / source_risk)
            recovery_from_pullback = (post_high - close_price) / source_risk

        last_open = safe_float(candles["open"].iloc[decision_idx])
        high = safe_float(candles["high"].iloc[decision_idx])
        low = safe_float(candles["low"].iloc[decision_idx])
        body = abs(close_price - last_open) / risk_points
        upper_wick = max(0.0, high - max(close_price, last_open)) / risk_points
        lower_wick = max(0.0, min(close_price, last_open) - low) / risk_points

        payload: dict[str, Any] = {
            "candidate_uid": row.get("candidate_uid"),
            "reentry_uid": reentry_uid(str(row.get("candidate_uid") or ""), candles["ts_utc"].iloc[entry_idx], direction, risk_ticks),
            "source_selected_index": int(row.get("selected_index") or 0),
            "formula": row.get("formula"),
            "timeframe": row.get("timeframe"),
            "valid_year": int(row.get("valid_year") or 0),
            "root_symbol": row.get("root_symbol"),
            "symbol": symbol,
            "direction": direction,
            "source_entry_date": row.get("entry_date"),
            "source_model_exit_date": exit_date,
            "reentry_decision_date": candles["ts_utc"].iloc[decision_idx],
            "reentry_entry_date": candles["ts_utc"].iloc[entry_idx],
            "reentry_entry_price": entry_price,
            "reentry_stop_price": stop_price,
            "reentry_risk_points": risk_points,
            "reentry_risk_ticks": risk_ticks,
            "tick_size": tick_size,
            "source_predicted_r": safe_float(row.get("predicted_r")),
            "source_baseline_result_r": safe_float(row.get("exit_baseline_result_r")),
            "source_model_result_r": safe_float(row.get("exit_model_result_r")),
            "source_delta_r": safe_float(row.get("exit_delta_r")),
            "source_exit_score_r": safe_float(row.get("exit_exit_score_r")),
            "source_baseline_hold_minutes": safe_float(row.get("exit_baseline_hold_minutes")),
            "source_model_hold_minutes": safe_float(row.get("exit_model_hold_minutes")),
            "baseline_exit_reason": row.get("exit_baseline_exit_reason"),
            "model_exit_reason": row.get("exit_model_exit_reason"),
            "bars_after_exit": decision_idx - exit_idx,
            "minutes_after_exit": (decision_idx - exit_idx) * timeframe_minutes,
            "price_from_exit_source_r": sign * (close_price - exit_price) / source_risk,
            "post_exit_mfe_source_r": post_mfe,
            "post_exit_mae_source_r": post_mae,
            "pullback_depth_source_r": pullback_depth,
            "recovery_from_pullback_source_r": recovery_from_pullback,
            "signed_ret_3_source_r": signed_ret_3,
            "signed_ret_6_source_r": sign
            * (close_price - safe_float(close_values.iloc[max(exit_idx, decision_idx - 6)]))
            / source_risk,
            "range_6_source_r": (
                safe_float(high_values.iloc[max(exit_idx, decision_idx - 5) : decision_idx + 1].max())
                - safe_float(low_values.iloc[max(exit_idx, decision_idx - 5) : decision_idx + 1].min())
            )
            / source_risk,
            "directional_close_position_6": directional_position,
            "body_reentry_r": body,
            "upper_wick_reentry_r": upper_wick,
            "lower_wick_reentry_r": lower_wick,
            "volume_vs_exit": safe_float(candles["volume"].iloc[decision_idx], 0.0) / volume_exit,
            "reentry_risk_vs_source_r": risk_points / source_risk,
            "distance_to_reentry_stop_r": abs(close_price - stop_price) / risk_points,
            **result,
        }
        for col in scanner.CAT_FEATURES + scanner.NUM_FEATURES:
            payload[col] = row.get(col)
        rows.append(payload)
    return rows


def build_reentry_candidates(
    conn,
    overlay_trades: pd.DataFrame,
    table_name: str,
    timeframe_minutes: int,
    args: argparse.Namespace,
    label: str,
) -> pd.DataFrame:
    rows: list[dict[str, Any]] = []
    total = len(overlay_trades)
    for index, (_, row) in enumerate(overlay_trades.iterrows(), start=1):
        rows.extend(build_reentry_candidates_for_trade(conn, row, table_name, timeframe_minutes, args))
        if index % 250 == 0:
            print(f"{label}: built re-entry candidates for {index:,}/{total:,} exits ({len(rows):,} rows)")
    frame = pd.DataFrame(rows)
    print(f"{label}: {len(frame):,} re-entry candidates from {total:,} exits")
    return frame


def prepare_reentry_pool(frame: pd.DataFrame, include_target: bool) -> Pool:
    work = frame.copy()
    for col in REENTRY_CAT_FEATURES:
        if col not in work.columns:
            work[col] = "unknown"
        work[col] = work[col].fillna("unknown").astype(str)
    for col in REENTRY_NUM_FEATURES:
        if col not in work.columns:
            work[col] = 0.0
        work[col] = pd.to_numeric(work[col], errors="coerce").replace([np.inf, -np.inf], np.nan).fillna(0.0)
    features = REENTRY_CAT_FEATURES + REENTRY_NUM_FEATURES
    if include_target:
        target = pd.to_numeric(work["reentry_result_r"], errors="coerce").replace([np.inf, -np.inf], np.nan).fillna(0.0)
        return Pool(work[features], label=target.clip(lower=-5.0, upper=8.0), cat_features=REENTRY_CAT_FEATURES)
    return Pool(work[features], cat_features=REENTRY_CAT_FEATURES)


def train_reentry_model(frame: pd.DataFrame, args: argparse.Namespace) -> CatBoostRegressor:
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
    model.fit(prepare_reentry_pool(frame, include_target=True))
    return model


def add_reentry_predictions(frame: pd.DataFrame, model: CatBoostRegressor) -> pd.DataFrame:
    if frame.empty:
        return frame
    scored = frame.copy()
    scored["reentry_score_r"] = model.predict(prepare_reentry_pool(scored, include_target=False))
    return scored


def summarize_results(values: pd.Series) -> dict[str, Any]:
    result = pd.to_numeric(values, errors="coerce").fillna(0.0)
    equity = result.cumsum()
    drawdown = equity.cummax() - equity
    wins = int((result > 0).sum())
    losses = int((result <= 0).sum())
    return {
        "trades": int(len(result)),
        "wins": wins,
        "losses": losses,
        "win_rate": wins / len(result) if len(result) else 0.0,
        "avg_r": float(result.mean()) if len(result) else 0.0,
        "sum_r": float(result.sum()) if len(result) else 0.0,
        "max_drawdown_r": float(drawdown.max()) if len(drawdown) else 0.0,
    }


def summarize_timed_events(values: pd.Series, dates: pd.Series) -> dict[str, Any]:
    frame = pd.DataFrame({"result_r": pd.to_numeric(values, errors="coerce").fillna(0.0), "date": pd.to_datetime(dates)})
    frame = frame.sort_values("date", kind="stable")
    return summarize_results(frame["result_r"])


def select_reentries(candidates: pd.DataFrame, threshold: float | None) -> pd.DataFrame:
    if candidates.empty:
        return candidates
    eligible = candidates.copy() if threshold is None else candidates[candidates["reentry_score_r"] >= threshold].copy()
    if eligible.empty:
        return eligible
    eligible = eligible.sort_values(["reentry_entry_date", "reentry_score_r"], ascending=[True, False])
    selected_indices: list[int] = []
    used_sources: set[str] = set()
    next_available: dict[str, pd.Timestamp] = {}
    for idx, row in eligible.iterrows():
        source_uid = str(row.get("candidate_uid") or "")
        symbol = str(row.get("symbol") or "")
        entry_at = pd.Timestamp(row.get("reentry_entry_date"))
        exit_at = pd.Timestamp(row.get("reentry_exit_date"))
        if source_uid in used_sources:
            continue
        if symbol and entry_at < next_available.get(symbol, pd.Timestamp.min):
            continue
        selected_indices.append(idx)
        used_sources.add(source_uid)
        if symbol:
            next_available[symbol] = exit_at
    selected = eligible.loc[selected_indices].copy()
    selected = selected.sort_values(["reentry_entry_date", "reentry_score_r"], ascending=[True, False]).reset_index(drop=True)
    selected["reentry_index"] = np.arange(1, len(selected) + 1)
    return selected


def choose_reentry_threshold(candidates: pd.DataFrame, args: argparse.Namespace) -> float | None:
    if args.reentry_threshold is not None:
        return float(args.reentry_threshold)
    scores = pd.to_numeric(candidates.get("reentry_score_r", pd.Series(dtype=float)), errors="coerce").dropna()
    if scores.empty:
        return float("inf")
    disabled_threshold = float(scores.max()) + max(1.0, abs(float(scores.max())) * 0.10)
    quantiles = sorted(set(float(v) for v in scores.quantile(np.linspace(0.55, 0.99, 30)).dropna()))
    candidates_thresholds = [0.0, *quantiles]
    best_threshold = disabled_threshold
    best_summary = summarize_results(pd.Series(dtype=float))
    best_objective = 0.0
    dd_penalty = float(args.threshold_dd_penalty or 0.0)
    print("Re-entry threshold calibration:")
    for threshold in candidates_thresholds:
        selected = select_reentries(candidates, threshold)
        summary = summarize_results(selected.get("reentry_result_r", pd.Series(dtype=float)))
        if summary["trades"] < int(args.min_threshold_trades):
            continue
        objective = summary["sum_r"] - dd_penalty * summary["max_drawdown_r"]
        print(
            f"  threshold {threshold:.4f}: trades={summary['trades']:,}, "
            f"sum={summary['sum_r']:.2f}R, dd={summary['max_drawdown_r']:.2f}R"
        )
        if (
            objective,
            summary["sum_r"],
            -summary["max_drawdown_r"],
            summary["avg_r"],
        ) > (
            best_objective,
            best_summary["sum_r"],
            -best_summary["max_drawdown_r"],
            best_summary["avg_r"],
        ):
            best_threshold = threshold
            best_summary = summary
            best_objective = objective
    if best_threshold == disabled_threshold:
        print("  selected disabled: no threshold improved the holdout.")
    else:
        print(
            f"  selected {best_threshold:.4f}: trades={best_summary['trades']:,}, "
            f"sum={best_summary['sum_r']:.2f}R, dd={best_summary['max_drawdown_r']:.2f}R"
        )
    return best_threshold


def ensure_tables(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            f"""
            CREATE TABLE IF NOT EXISTS {RUN_TABLE} (
                reentry_model_run_id VARCHAR(64) NOT NULL PRIMARY KEY,
                exit_model_run_id VARCHAR(64) NOT NULL,
                source_model_run_id VARCHAR(64) NOT NULL,
                timeframe VARCHAR(16) NOT NULL,
                train_years VARCHAR(64) NOT NULL,
                threshold_year INT NOT NULL,
                valid_year INT NOT NULL,
                reentry_threshold DOUBLE NULL,
                train_source_trades INT NOT NULL DEFAULT 0,
                train_candidates INT NOT NULL DEFAULT 0,
                threshold_source_trades INT NOT NULL DEFAULT 0,
                threshold_candidates INT NOT NULL DEFAULT 0,
                valid_source_trades INT NOT NULL DEFAULT 0,
                valid_candidates INT NOT NULL DEFAULT 0,
                reentry_trades INT NOT NULL DEFAULT 0,
                wins INT NOT NULL DEFAULT 0,
                losses INT NOT NULL DEFAULT 0,
                win_rate DOUBLE NOT NULL DEFAULT 0,
                avg_r DOUBLE NOT NULL DEFAULT 0,
                sum_r DOUBLE NOT NULL DEFAULT 0,
                max_drawdown_r DOUBLE NOT NULL DEFAULT 0,
                source_sum_r DOUBLE NOT NULL DEFAULT 0,
                combined_sum_r DOUBLE NOT NULL DEFAULT 0,
                combined_max_drawdown_r DOUBLE NOT NULL DEFAULT 0,
                model_path VARCHAR(255) NULL,
                metadata_json JSON NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
            )
            """
        )
        cur.execute(
            f"""
            CREATE TABLE IF NOT EXISTS {TRADE_TABLE} (
                reentry_model_run_id VARCHAR(64) NOT NULL,
                reentry_index INT NOT NULL,
                candidate_uid VARCHAR(40) NOT NULL,
                reentry_uid VARCHAR(40) NOT NULL,
                exit_model_run_id VARCHAR(64) NOT NULL,
                source_model_run_id VARCHAR(64) NOT NULL,
                source_selected_index INT NULL,
                formula VARCHAR(128) NULL,
                timeframe VARCHAR(16) NOT NULL,
                valid_year INT NOT NULL,
                root_symbol VARCHAR(32) NULL,
                symbol VARCHAR(64) NULL,
                direction VARCHAR(16) NULL,
                source_entry_date DATETIME NULL,
                source_model_exit_date DATETIME NULL,
                reentry_decision_date DATETIME NULL,
                reentry_entry_date DATETIME NULL,
                reentry_exit_date DATETIME NULL,
                baseline_exit_reason VARCHAR(64) NULL,
                model_exit_reason VARCHAR(64) NULL,
                reentry_exit_reason VARCHAR(64) NULL,
                source_model_result_r DOUBLE NOT NULL DEFAULT 0,
                reentry_result_r DOUBLE NOT NULL DEFAULT 0,
                combined_result_r DOUBLE NOT NULL DEFAULT 0,
                reentry_score_r DOUBLE NULL,
                reentry_entry_price DOUBLE NULL,
                reentry_stop_price DOUBLE NULL,
                reentry_exit_price DOUBLE NULL,
                reentry_risk_points DOUBLE NULL,
                reentry_risk_ticks DOUBLE NULL,
                tick_size DOUBLE NULL,
                reentry_mfe_r DOUBLE NULL,
                reentry_mae_r DOUBLE NULL,
                reentry_hold_minutes INT NULL,
                bars_after_exit INT NULL,
                minutes_after_exit INT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (reentry_model_run_id, reentry_uid),
                INDEX idx_reentry_model_year (reentry_model_run_id, valid_year, reentry_index),
                INDEX idx_reentry_model_result (reentry_model_run_id, reentry_result_r)
            )
            """
        )


def save_run(
    conn,
    run_id: str,
    args: argparse.Namespace,
    exit_run: dict[str, Any],
    timeframe: str,
    train_overlay: pd.DataFrame,
    train_candidates: pd.DataFrame,
    threshold_overlay: pd.DataFrame,
    threshold_candidates: pd.DataFrame,
    valid_overlay: pd.DataFrame,
    valid_candidates: pd.DataFrame,
    selected: pd.DataFrame,
    threshold: float | None,
    model: CatBoostRegressor,
) -> dict[str, Any]:
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id
    model_dir.mkdir(parents=True, exist_ok=True)
    model_file = model_dir / "catboost_reentry_model.cbm"
    model.save_model(str(model_file))

    reentry_summary = summarize_results(selected.get("reentry_result_r", pd.Series(dtype=float)))
    source_summary = summarize_timed_events(valid_overlay["exit_model_result_r"], valid_overlay["exit_model_exit_date"])
    source_events = pd.DataFrame(
        {
            "result_r": pd.to_numeric(valid_overlay["exit_model_result_r"], errors="coerce").fillna(0.0),
            "date": pd.to_datetime(valid_overlay["exit_model_exit_date"], errors="coerce"),
        }
    )
    reentry_events = pd.DataFrame(
        {
            "result_r": pd.to_numeric(selected.get("reentry_result_r", pd.Series(dtype=float)), errors="coerce").fillna(0.0),
            "date": pd.to_datetime(selected.get("reentry_exit_date", pd.Series(dtype="datetime64[ns]")), errors="coerce"),
        }
    )
    combined_events = pd.concat([source_events, reentry_events], ignore_index=True)
    combined_summary = summarize_timed_events(combined_events["result_r"], combined_events["date"])

    threshold_for_db = None if threshold is None or not math.isfinite(float(threshold)) else float(threshold)
    metadata = {
        "reentry_model_run_id": run_id,
        "exit_model_run_id": args.exit_model_run_id,
        "source_model_run_id": exit_run["source_model_run_id"],
        "timeframe": timeframe,
        "train_years": scanner.parse_years(args.train_years),
        "threshold_year": args.threshold_year,
        "valid_year": args.valid_year,
        "reentry_threshold": threshold_for_db,
        "watch_bars": args.watch_bars,
        "candidate_step_bars": args.candidate_step_bars,
        "min_bars_after_exit": args.min_bars_after_exit,
        "breakout_bars": args.breakout_bars,
        "stop_lookback_bars": args.stop_lookback_bars,
        "min_reentry_risk_ticks": args.min_reentry_risk_ticks,
        "max_reentry_risk_ticks": args.max_reentry_risk_ticks,
        "time_exit_bars": args.time_exit_bars,
        "profit_lock_trigger_r": args.profit_lock_trigger_r,
        "profit_lock_giveback_r": args.profit_lock_giveback_r,
        "slippage_entry_ticks": args.slippage_entry_ticks,
        "slippage_exit_ticks": args.slippage_exit_ticks,
        "cat_features": REENTRY_CAT_FEATURES,
        "num_features": REENTRY_NUM_FEATURES,
        "phase": "post_exit_same_direction_reentry",
        "notes": "First re-entry layer after the exit model. It looks for same-direction trend repair after the AI exit.",
    }
    (model_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=str))

    with conn.cursor() as cur:
        cur.execute(f"DELETE FROM {TRADE_TABLE} WHERE reentry_model_run_id = %s", (run_id,))
        cur.execute(f"DELETE FROM {RUN_TABLE} WHERE reentry_model_run_id = %s", (run_id,))
        cur.execute(
            f"""
            INSERT INTO {RUN_TABLE} (
                reentry_model_run_id, exit_model_run_id, source_model_run_id, timeframe,
                train_years, threshold_year, valid_year, reentry_threshold,
                train_source_trades, train_candidates, threshold_source_trades,
                threshold_candidates, valid_source_trades, valid_candidates,
                reentry_trades, wins, losses, win_rate, avg_r, sum_r,
                max_drawdown_r, source_sum_r, combined_sum_r,
                combined_max_drawdown_r, model_path, metadata_json
            )
            VALUES ({",".join(["%s"] * 26)})
            """,
            (
                run_id,
                args.exit_model_run_id,
                exit_run["source_model_run_id"],
                timeframe,
                args.train_years,
                int(args.threshold_year),
                int(args.valid_year),
                threshold_for_db,
                int(len(train_overlay)),
                int(len(train_candidates)),
                int(len(threshold_overlay)),
                int(len(threshold_candidates)),
                int(len(valid_overlay)),
                int(len(valid_candidates)),
                reentry_summary["trades"],
                reentry_summary["wins"],
                reentry_summary["losses"],
                reentry_summary["win_rate"],
                reentry_summary["avg_r"],
                reentry_summary["sum_r"],
                reentry_summary["max_drawdown_r"],
                source_summary["sum_r"],
                combined_summary["sum_r"],
                combined_summary["max_drawdown_r"],
                str(model_dir),
                json.dumps(metadata),
            ),
        )

        trade_columns = [
            "reentry_model_run_id",
            "reentry_index",
            "candidate_uid",
            "reentry_uid",
            "exit_model_run_id",
            "source_model_run_id",
            "source_selected_index",
            "formula",
            "timeframe",
            "valid_year",
            "root_symbol",
            "symbol",
            "direction",
            "source_entry_date",
            "source_model_exit_date",
            "reentry_decision_date",
            "reentry_entry_date",
            "reentry_exit_date",
            "baseline_exit_reason",
            "model_exit_reason",
            "reentry_exit_reason",
            "source_model_result_r",
            "reentry_result_r",
            "combined_result_r",
            "reentry_score_r",
            "reentry_entry_price",
            "reentry_stop_price",
            "reentry_exit_price",
            "reentry_risk_points",
            "reentry_risk_ticks",
            "tick_size",
            "reentry_mfe_r",
            "reentry_mae_r",
            "reentry_hold_minutes",
            "bars_after_exit",
            "minutes_after_exit",
        ]
        rows = []
        for record in selected.to_dict("records"):
            payload = {
                "reentry_model_run_id": run_id,
                "exit_model_run_id": args.exit_model_run_id,
                "source_model_run_id": exit_run["source_model_run_id"],
                "combined_result_r": safe_float(record.get("source_model_result_r"))
                + safe_float(record.get("reentry_result_r")),
                **record,
            }
            rows.append(tuple(scanner.clean(payload.get(column)) for column in trade_columns))
        if rows:
            cur.executemany(
                f"""
                INSERT INTO {TRADE_TABLE} ({", ".join(trade_columns)})
                VALUES ({",".join(["%s"] * len(trade_columns))})
                """,
                rows,
            )
    conn.commit()
    return {
        "reentry": reentry_summary,
        "source": source_summary,
        "combined": combined_summary,
        "model_path": str(model_dir),
    }


def main() -> int:
    args = parse_args()
    conn = wave.connect()
    try:
        ensure_tables(conn)
        exit_run = load_exit_run(conn, args.exit_model_run_id)
        exit_metadata = exit_run["metadata"]
        source = exit_model.load_source_run(conn, exit_run["source_model_run_id"])
        source_metadata = source["metadata"]
        entry_args = exit_model.entry_args_from_metadata(source_metadata)
        timeframe = str(entry_args.timeframe)
        table_name, timeframe_minutes = scanner.table_for_timeframe(timeframe)
        run_id = reentry_run_id(args, timeframe)
        if not args.replace_run:
            with conn.cursor() as cur:
                cur.execute(f"SELECT 1 FROM {RUN_TABLE} WHERE reentry_model_run_id = %s", (run_id,))
                if cur.fetchone():
                    raise ValueError(f"Re-entry model run already exists: {run_id}. Use --replace-run to overwrite.")

        entry_cb_model = exit_model.load_entry_model(source)
        exit_cb_model = load_exit_catboost_model(exit_run)
        exit_args = exit_args_from_metadata(exit_metadata)
        exit_threshold = exit_metadata.get("exit_threshold", exit_run.get("exit_threshold"))
        exit_threshold = None if exit_threshold is None else float(exit_threshold)

        train_years = scanner.parse_years(args.train_years)
        train_source = selected_trades_for_years(
            conn,
            entry_cb_model,
            entry_args,
            exit_run["source_model_run_id"],
            train_years,
            args.valid_year,
            args.max_train_trades,
        )
        threshold_source = selected_trades_for_years(
            conn,
            entry_cb_model,
            entry_args,
            exit_run["source_model_run_id"],
            [args.threshold_year],
            args.valid_year,
            args.max_threshold_trades,
        )
        valid_source = selected_trades_for_years(
            conn,
            entry_cb_model,
            entry_args,
            exit_run["source_model_run_id"],
            [args.valid_year],
            args.valid_year,
            args.max_valid_trades,
        )
        print(
            f"Re-entry model source_exit={args.exit_model_run_id} train_source={len(train_source):,} "
            f"threshold_source={len(threshold_source):,} valid_source={len(valid_source):,}"
        )

        train_overlay = build_exit_overlay(
            conn, train_source, exit_cb_model, exit_threshold, exit_args, table_name, timeframe_minutes, "exit-train"
        )
        threshold_overlay = build_exit_overlay(
            conn,
            threshold_source,
            exit_cb_model,
            exit_threshold,
            exit_args,
            table_name,
            timeframe_minutes,
            "exit-threshold",
        )
        valid_overlay = build_exit_overlay(
            conn, valid_source, exit_cb_model, exit_threshold, exit_args, table_name, timeframe_minutes, "exit-valid"
        )

        train_candidates = build_reentry_candidates(conn, train_overlay, table_name, timeframe_minutes, args, "train")
        if train_candidates.empty:
            raise ValueError("No train re-entry candidates were generated.")
        model = train_reentry_model(train_candidates, args)

        threshold_candidates = build_reentry_candidates(
            conn, threshold_overlay, table_name, timeframe_minutes, args, "threshold"
        )
        threshold_candidates = add_reentry_predictions(threshold_candidates, model)
        threshold = choose_reentry_threshold(threshold_candidates, args)

        valid_candidates = build_reentry_candidates(conn, valid_overlay, table_name, timeframe_minutes, args, "valid")
        valid_candidates = add_reentry_predictions(valid_candidates, model)
        selected = select_reentries(valid_candidates, threshold)

        summary = save_run(
            conn,
            run_id,
            args,
            exit_run,
            timeframe,
            train_overlay,
            train_candidates,
            threshold_overlay,
            threshold_candidates,
            valid_overlay,
            valid_candidates,
            selected,
            threshold,
            model,
        )

        print(
            f"Materialized {run_id}: source {summary['source']['sum_r']:.2f}R / "
            f"{summary['source']['max_drawdown_r']:.2f}R DD, re-entry "
            f"{summary['reentry']['trades']:,} trades for {summary['reentry']['sum_r']:.2f}R / "
            f"{summary['reentry']['max_drawdown_r']:.2f}R DD, combined "
            f"{summary['combined']['sum_r']:.2f}R / {summary['combined']['max_drawdown_r']:.2f}R DD, "
            f"threshold={'disabled' if threshold is None or not math.isfinite(float(threshold)) else f'{threshold:.4f}'}"
        )
    finally:
        conn.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
