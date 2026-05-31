#!/usr/bin/env python3
"""
Train a candle-wave exit overlay.

Phase one is intentionally conservative by default: it does not invent new
entries. In early mode it only learns whether exiting before the source
rule-based exit would have helped. In dynamic mode it learns whether continuing
from the current candle has more expected value than exiting next bar, allowing
the overlay to exit before or after the source exit.
"""

from __future__ import annotations

import argparse
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

import ai_candle_wave_scanner_model as scanner
import ai_wave_rider_research as wave


RUN_TABLE = "ai_candle_wave_exit_model_runs"
TRADE_TABLE = "ai_candle_wave_exit_model_trades"

DEFAULT_SOURCE_RUN = "aicw-mtf-eg1-xtight-t014-rd3-en4-v1-2m-2026"

DECISION_NUM_FEATURES = [
    "predicted_r",
    "bars_held",
    "hold_minutes_so_far",
    "source_exit_signal_seen",
    "bars_since_source_exit_signal",
    "source_exit_result_seen_r",
    "current_vs_source_exit_r",
    "current_unrealized_r",
    "current_unrealized_raw_r",
    "mfe_so_far_r",
    "mae_so_far_r",
    "giveback_from_mfe_r",
    "distance_to_stop_r",
    "last_bar_r",
    "ret_3_r",
    "ret_6_r",
    "range_3_r",
    "range_6_r",
    "close_position_6",
    "body_r",
    "upper_wick_r",
    "lower_wick_r",
    "volume_vs_entry",
]

EXIT_CAT_FEATURES = list(dict.fromkeys(["symbol", *scanner.CAT_FEATURES]))
EXIT_NUM_FEATURES = list(dict.fromkeys([*scanner.NUM_FEATURES, *DECISION_NUM_FEATURES]))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-model-run-id", default=DEFAULT_SOURCE_RUN)
    parser.add_argument("--run-prefix", default="aicw-exit-early-v1")
    parser.add_argument("--mode", choices=["early", "dynamic"], default="early")
    parser.add_argument("--train-years", default="2024")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--replace-run", action="store_true")
    parser.add_argument("--max-train-trades", type=int, default=0)
    parser.add_argument("--max-threshold-trades", type=int, default=0)
    parser.add_argument("--max-valid-trades", type=int, default=0)
    parser.add_argument("--min-hold-bars", type=int, default=4)
    parser.add_argument("--decision-step-bars", type=int, default=1)
    parser.add_argument("--dynamic-max-bars", type=int, default=240)
    parser.add_argument("--dynamic-no-signal-exit", choices=["terminal", "source"], default="terminal")
    parser.add_argument("--exit-threshold", type=float, default=None)
    parser.add_argument("--max-threshold-dd-r", type=float, default=0.0)
    parser.add_argument("--threshold-dd-penalty", type=float, default=0.0)
    parser.add_argument("--min-threshold-trades", type=int, default=80)
    parser.add_argument("--iterations", type=int, default=450)
    parser.add_argument("--depth", type=int, default=5)
    parser.add_argument("--learning-rate", type=float, default=0.045)
    parser.add_argument("--l2-leaf-reg", type=float, default=12.0)
    parser.add_argument("--random-seed", type=int, default=73)
    return parser.parse_args()


def exit_run_id(args: argparse.Namespace, timeframe: str) -> str:
    run_id = f"{args.run_prefix}-{timeframe}-{args.valid_year}"
    if len(run_id) > 64:
        raise ValueError(f"Run id too long: {run_id}")
    return run_id


def load_source_run(conn, model_run_id: str) -> dict[str, Any]:
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT model_run_id, model_path, metadata_json
            FROM ai_candle_wave_model_runs
            WHERE model_run_id = %s
            """,
            (model_run_id,),
        )
        row = cur.fetchone()
    if not row:
        raise ValueError(f"Source model run not found: {model_run_id}")
    metadata = json.loads(row.get("metadata_json") or "{}")
    row["metadata"] = metadata
    return row


def entry_args_from_metadata(metadata: dict[str, Any]) -> argparse.Namespace:
    values = {
        "timeframe": metadata.get("timeframe", "2m"),
        "formula": metadata.get("formula") or scanner.formula_for(metadata.get("timeframe", "2m")),
        "allowed_roots": ",".join(metadata.get("allowed_roots") or []),
        "cooldown_minutes": int(metadata.get("cooldown_minutes") or 0),
        "max_root_trades_per_day": int(metadata.get("max_root_trades_per_day") or 0),
        "max_energy_trades_per_day": int(metadata.get("max_energy_trades_per_day") or 0),
        "energy_extra_slot_start": int(metadata.get("energy_extra_slot_start") or 0),
        "energy_extra_slot_min_score": float(metadata.get("energy_extra_slot_min_score") or 0.0),
        "loss_brake_r": float(metadata.get("loss_brake_r") or 0.0),
        "loss_brake_scope": metadata.get("loss_brake_scope") or "root",
        "loss_brake_min_trades": int(metadata.get("loss_brake_min_trades") or 1),
        "threshold": metadata.get("threshold"),
    }
    return argparse.Namespace(**values)


def load_entry_model(source: dict[str, Any]) -> CatBoostRegressor:
    model_path = Path(str(source.get("model_path") or "")) / "catboost_model.cbm"
    if not model_path.exists():
        metadata = source["metadata"]
        model_path = wave.ABCD_ROOT / "model_registry" / str(metadata["model_run_id"]) / "catboost_model.cbm"
    if not model_path.exists():
        raise FileNotFoundError(f"Missing source CatBoost model: {model_path}")
    model = CatBoostRegressor()
    model.load_model(str(model_path))
    return model


def load_selected_trades(conn, source_model_run_id: str, limit: int = 0) -> pd.DataFrame:
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
        *scanner.CAT_FEATURES,
        *scanner.NUM_FEATURES,
    ]
    columns = list(dict.fromkeys(columns))
    sql_limit = f"LIMIT {int(limit)}" if int(limit or 0) > 0 else ""
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT {", ".join(columns)}
            FROM ai_candle_wave_selected_trades
            WHERE model_run_id = %s
            ORDER BY selected_index
            {sql_limit}
            """,
            (source_model_run_id,),
        )
        rows = cur.fetchall()
    frame = pd.DataFrame(rows)
    return normalize_trade_frame(frame)


def normalize_trade_frame(frame: pd.DataFrame) -> pd.DataFrame:
    if frame.empty:
        return frame
    for col in ["signal_date", "entry_date", "exit_date"]:
        if col in frame.columns:
            frame[col] = pd.to_datetime(frame[col], errors="coerce")
    for col in [
        "predicted_r",
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
        *scanner.NUM_FEATURES,
    ]:
        if col in frame.columns:
            frame[col] = pd.to_numeric(frame[col], errors="coerce")
    return frame


def selected_like_trades(
    conn,
    entry_model: CatBoostRegressor,
    entry_args: argparse.Namespace,
    years: list[int],
    limit: int = 0,
) -> pd.DataFrame:
    candidates = scanner.load_candidates(conn, entry_args.timeframe, years, entry_args.formula)
    if candidates.empty:
        return candidates
    scored = scanner.add_predictions(candidates, entry_model)
    selected = scanner.select_chronological(scored, entry_args.threshold, entry_args)
    if int(limit or 0) > 0:
        selected = selected.head(int(limit)).copy()
    return normalize_trade_frame(selected)


def load_candles(
    conn,
    table_name: str,
    symbol: str,
    start_at: pd.Timestamp,
    end_at: pd.Timestamp,
) -> pd.DataFrame:
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT ts_utc, open, high, low, close, volume
            FROM {scanner.safe_identifier(table_name)}
            WHERE symbol = %s
              AND ts_utc >= %s
              AND ts_utc <= %s
            ORDER BY ts_utc
            """,
            (symbol, start_at.to_pydatetime(), end_at.to_pydatetime()),
        )
        rows = cur.fetchall()
    candles = pd.DataFrame(rows)
    if candles.empty:
        return candles
    candles["ts_utc"] = pd.to_datetime(candles["ts_utc"], errors="coerce")
    for col in ["open", "high", "low", "close", "volume"]:
        candles[col] = pd.to_numeric(candles[col], errors="coerce")
    return candles.dropna(subset=["ts_utc", "open", "high", "low", "close"]).reset_index(drop=True)


def index_at_or_after(times: pd.Series, value: pd.Timestamp) -> int | None:
    if pd.isna(value):
        return None
    arr = times.to_numpy(dtype="datetime64[ns]")
    idx = int(np.searchsorted(arr, np.datetime64(value), side="left"))
    if idx >= len(times):
        return None
    return idx


def safe_float(value: Any, default: float = 0.0) -> float:
    try:
        result = float(value)
    except (TypeError, ValueError):
        return default
    if not math.isfinite(result):
        return default
    return result


def close_position(values: pd.Series, idx: int, lookback: int) -> float:
    start = max(0, idx - lookback + 1)
    window = values.iloc[start : idx + 1]
    low = safe_float(window.min(), 0.0)
    high = safe_float(window.max(), low)
    close = safe_float(values.iloc[idx], low)
    spread = high - low
    if spread <= 0:
        return 0.5
    return (close - low) / spread


def static_feature_payload(row: pd.Series) -> dict[str, Any]:
    payload: dict[str, Any] = {}
    for col in EXIT_CAT_FEATURES + scanner.NUM_FEATURES:
        payload[col] = row.get(col)
    return payload


def build_decision_rows_for_trade(
    conn,
    row: pd.Series,
    table_name: str,
    timeframe_minutes: int,
    args: argparse.Namespace,
    include_target: bool,
) -> list[dict[str, Any]]:
    entry_date = pd.Timestamp(row.get("entry_date"))
    exit_date = pd.Timestamp(row.get("exit_date"))
    if pd.isna(entry_date) or pd.isna(exit_date) or exit_date <= entry_date:
        return []

    symbol = str(row.get("symbol") or "")
    direction = str(row.get("direction") or "").upper()
    if not symbol or direction not in {"LONG", "SHORT"}:
        return []

    risk_points = safe_float(row.get("risk_points"))
    tick_size = safe_float(row.get("tick_size"))
    entry_price = safe_float(row.get("entry_price"))
    stop_price = safe_float(row.get("stop_price"))
    baseline_result_r = safe_float(row.get("result_r"))
    if risk_points <= 0 or tick_size <= 0 or entry_price <= 0:
        return []

    mode = str(getattr(args, "mode", "early") or "early")
    if mode == "dynamic":
        end_at = entry_date + pd.Timedelta(minutes=timeframe_minutes * (max(2, int(args.dynamic_max_bars)) + 2))
    else:
        end_at = exit_date + pd.Timedelta(minutes=timeframe_minutes)
    candles = load_candles(conn, table_name, symbol, entry_date, end_at)
    if candles.empty:
        return []
    entry_idx = index_at_or_after(candles["ts_utc"], entry_date)
    source_exit_idx = index_at_or_after(candles["ts_utc"], exit_date)
    if entry_idx is None:
        return []

    sign = wave.direction_sign(direction)
    slippage_r = ((3.0 + 3.0) * tick_size) / risk_points
    max_idx = len(candles) - 1
    terminal_exit_reason = "source_exit"
    terminal_price = safe_float(row.get("exit_price"))
    terminal_date = exit_date
    if mode == "dynamic":
        max_idx = min(max_idx, entry_idx + max(2, int(args.dynamic_max_bars)))
        hard_stop_idx = None
        for candle_idx in range(entry_idx, max_idx + 1):
            candle = candles.iloc[candle_idx]
            if direction == "LONG" and safe_float(candle["low"]) <= stop_price:
                hard_stop_idx = candle_idx
                break
            if direction == "SHORT" and safe_float(candle["high"]) >= stop_price:
                hard_stop_idx = candle_idx
                break
        exit_idx = hard_stop_idx if hard_stop_idx is not None else max_idx
        if hard_stop_idx is not None:
            terminal_exit_reason = "dynamic_hard_stop"
            terminal_price = stop_price
            terminal_date = candles["ts_utc"].iloc[exit_idx]
        else:
            terminal_exit_reason = "dynamic_time_exit"
            terminal_price = safe_float(candles["close"].iloc[exit_idx])
            terminal_date = candles["ts_utc"].iloc[exit_idx]
    else:
        exit_idx = source_exit_idx
        if exit_idx is None or exit_idx <= entry_idx:
            return []
    if exit_idx is None or exit_idx <= entry_idx:
        return []

    terminal_result = sign * (terminal_price - entry_price) / risk_points - slippage_r
    base_payload = static_feature_payload(row)
    rows: list[dict[str, Any]] = []
    start_idx = entry_idx + max(1, int(args.min_hold_bars))
    step = max(1, int(args.decision_step_bars))
    high_values = candles["high"]
    low_values = candles["low"]
    close_values = candles["close"]
    volume_entry = max(1.0, safe_float(candles["volume"].iloc[entry_idx], 1.0))
    open_exit_results: dict[int, float] = {}
    for exit_open_idx in range(entry_idx + 1, exit_idx + 1):
        open_exit_results[exit_open_idx] = (
            sign * (safe_float(candles["open"].iloc[exit_open_idx]) - entry_price) / risk_points - slippage_r
        )
    suffix_best_result: dict[int, float] = {}
    suffix_best_index: dict[int, int] = {}
    current_best_result = -float("inf")
    current_best_index = exit_idx
    for exit_open_idx in range(exit_idx, entry_idx, -1):
        value = open_exit_results.get(exit_open_idx, -float("inf"))
        if value >= current_best_result:
            current_best_result = value
            current_best_index = exit_open_idx
        suffix_best_result[exit_open_idx] = current_best_result
        suffix_best_index[exit_open_idx] = current_best_index

    for decision_idx in range(start_idx, exit_idx, step):
        next_idx = decision_idx + 1
        if next_idx >= len(candles) or next_idx > exit_idx:
            break
        close_price = safe_float(candles["close"].iloc[decision_idx])
        open_next = safe_float(candles["open"].iloc[next_idx])
        if close_price <= 0 or open_next <= 0:
            continue

        high_so_far = safe_float(high_values.iloc[entry_idx : decision_idx + 1].max())
        low_so_far = safe_float(low_values.iloc[entry_idx : decision_idx + 1].min())
        if direction == "LONG":
            mfe = (high_so_far - entry_price) / risk_points
            mae = (entry_price - low_so_far) / risk_points
            distance_to_stop = (close_price - stop_price) / risk_points
        else:
            mfe = (entry_price - low_so_far) / risk_points
            mae = (high_so_far - entry_price) / risk_points
            distance_to_stop = (stop_price - close_price) / risk_points

        current_raw = sign * (close_price - entry_price) / risk_points
        current_unrealized = current_raw - slippage_r
        exit_now_raw = sign * (open_next - entry_price) / risk_points
        exit_now_result = exit_now_raw - slippage_r
        source_exit_signal_seen = (
            1.0 if source_exit_idx is not None and source_exit_idx >= 0 and decision_idx >= source_exit_idx else 0.0
        )
        bars_since_source_exit_signal = (
            max(0, decision_idx - int(source_exit_idx))
            if source_exit_signal_seen > 0 and source_exit_idx is not None
            else 0
        )
        source_exit_result_seen = baseline_result_r if source_exit_signal_seen > 0 else 0.0
        current_vs_source_exit = current_unrealized - baseline_result_r if source_exit_signal_seen > 0 else 0.0
        last_open = safe_float(candles["open"].iloc[decision_idx])
        last_bar_r = sign * (close_price - last_open) / risk_points
        ret_3 = sign * (close_price - safe_float(close_values.iloc[max(entry_idx, decision_idx - 3)])) / risk_points
        ret_6 = sign * (close_price - safe_float(close_values.iloc[max(entry_idx, decision_idx - 6)])) / risk_points
        range_3 = (
            safe_float(high_values.iloc[max(entry_idx, decision_idx - 2) : decision_idx + 1].max())
            - safe_float(low_values.iloc[max(entry_idx, decision_idx - 2) : decision_idx + 1].min())
        ) / risk_points
        range_6 = (
            safe_float(high_values.iloc[max(entry_idx, decision_idx - 5) : decision_idx + 1].max())
            - safe_float(low_values.iloc[max(entry_idx, decision_idx - 5) : decision_idx + 1].min())
        ) / risk_points
        high = safe_float(candles["high"].iloc[decision_idx])
        low = safe_float(candles["low"].iloc[decision_idx])
        body = abs(close_price - last_open) / risk_points
        upper_wick = max(0.0, high - max(close_price, last_open)) / risk_points
        lower_wick = max(0.0, min(close_price, last_open) - low) / risk_points

        payload = {
            **base_payload,
            "candidate_uid": row.get("candidate_uid"),
            "decision_index": decision_idx - entry_idx,
            "decision_date": candles["ts_utc"].iloc[decision_idx],
            "exit_now_date": candles["ts_utc"].iloc[next_idx],
            "exit_now_price": open_next,
            "exit_now_result_r": exit_now_result,
            "baseline_result_r": baseline_result_r,
            "terminal_exit_date": terminal_date,
            "terminal_exit_price": terminal_price,
            "terminal_result_r": terminal_result,
            "terminal_exit_reason": terminal_exit_reason,
            "bars_held": decision_idx - entry_idx,
            "hold_minutes_so_far": (decision_idx - entry_idx) * timeframe_minutes,
            "source_exit_signal_seen": source_exit_signal_seen,
            "bars_since_source_exit_signal": bars_since_source_exit_signal,
            "source_exit_result_seen_r": source_exit_result_seen,
            "current_vs_source_exit_r": current_vs_source_exit,
            "current_unrealized_r": current_unrealized,
            "current_unrealized_raw_r": current_raw,
            "mfe_so_far_r": mfe,
            "mae_so_far_r": mae,
            "giveback_from_mfe_r": max(0.0, mfe - current_raw),
            "distance_to_stop_r": distance_to_stop,
            "last_bar_r": last_bar_r,
            "ret_3_r": ret_3,
            "ret_6_r": ret_6,
            "range_3_r": range_3,
            "range_6_r": range_6,
            "close_position_6": close_position(close_values, decision_idx, 6),
            "body_r": body,
            "upper_wick_r": upper_wick,
            "lower_wick_r": lower_wick,
            "volume_vs_entry": safe_float(candles["volume"].iloc[decision_idx], 0.0) / volume_entry,
        }
        if include_target:
            if mode == "dynamic":
                best_open_result = suffix_best_result.get(next_idx, -float("inf"))
                if terminal_result >= best_open_result:
                    best_future_result = terminal_result
                    best_future_date = terminal_date
                    best_future_price = terminal_price
                else:
                    best_idx = suffix_best_index[next_idx]
                    best_future_result = best_open_result
                    best_future_date = candles["ts_utc"].iloc[best_idx]
                    best_future_price = safe_float(candles["open"].iloc[best_idx])
                payload["best_future_result_r"] = best_future_result
                payload["best_future_exit_date"] = best_future_date
                payload["best_future_exit_price"] = best_future_price
                payload["exit_edge_r"] = best_future_result - exit_now_result
            else:
                payload["exit_edge_r"] = exit_now_result - baseline_result_r
        rows.append(payload)
    return rows


def build_decision_rows(
    conn,
    trades: pd.DataFrame,
    table_name: str,
    timeframe_minutes: int,
    args: argparse.Namespace,
    include_target: bool,
    label: str,
) -> pd.DataFrame:
    rows: list[dict[str, Any]] = []
    total = len(trades)
    for index, (_, row) in enumerate(trades.iterrows(), start=1):
        rows.extend(build_decision_rows_for_trade(conn, row, table_name, timeframe_minutes, args, include_target))
        if index % 250 == 0:
            print(f"{label}: built decision rows for {index:,}/{total:,} trades ({len(rows):,} rows)")
    frame = pd.DataFrame(rows)
    print(f"{label}: {len(frame):,} decision rows from {total:,} trades")
    return frame


def prepare_exit_pool(frame: pd.DataFrame, include_target: bool) -> Pool:
    work = frame.copy()
    for col in EXIT_CAT_FEATURES:
        if col not in work.columns:
            work[col] = "unknown"
        work[col] = work[col].fillna("unknown").astype(str)
    for col in EXIT_NUM_FEATURES:
        if col not in work.columns:
            work[col] = 0.0
        work[col] = pd.to_numeric(work[col], errors="coerce").replace([np.inf, -np.inf], np.nan).fillna(0.0)
    features = EXIT_CAT_FEATURES + EXIT_NUM_FEATURES
    if include_target:
        target = pd.to_numeric(work["exit_edge_r"], errors="coerce").replace([np.inf, -np.inf], np.nan).fillna(0.0)
        target = target.clip(lower=-5.0, upper=5.0)
        return Pool(work[features], label=target, cat_features=EXIT_CAT_FEATURES)
    return Pool(work[features], cat_features=EXIT_CAT_FEATURES)


def train_exit_model(frame: pd.DataFrame, args: argparse.Namespace) -> CatBoostRegressor:
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
    model.fit(prepare_exit_pool(frame, include_target=True))
    return model


def add_exit_predictions(frame: pd.DataFrame, model: CatBoostRegressor) -> pd.DataFrame:
    if frame.empty:
        return frame
    scored = frame.copy()
    scored["exit_score_r"] = model.predict(prepare_exit_pool(scored, include_target=False))
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


def source_overlay_row(trade: pd.Series) -> dict[str, Any]:
    baseline_result = safe_float(trade.get("result_r"))
    baseline_exit_date = trade.get("exit_date")
    baseline_exit_price = trade.get("exit_price")
    entry_date = trade.get("entry_date")
    model_hold = None
    if not pd.isna(entry_date) and not pd.isna(baseline_exit_date):
        model_hold = int((pd.Timestamp(baseline_exit_date) - pd.Timestamp(entry_date)).total_seconds() / 60)
    return {
        "selected_index": int(trade.get("selected_index") or 0),
        "candidate_uid": str(trade.get("candidate_uid") or ""),
        "formula": trade.get("formula"),
        "timeframe": trade.get("timeframe"),
        "valid_year": int(trade.get("valid_year") or 0),
        "root_symbol": trade.get("root_symbol"),
        "symbol": trade.get("symbol"),
        "entry_date": entry_date,
        "baseline_exit_date": baseline_exit_date,
        "model_exit_date": baseline_exit_date,
        "direction": trade.get("direction"),
        "baseline_exit_reason": trade.get("exit_reason"),
        "model_exit_reason": str(trade.get("exit_reason") or "source_exit"),
        "baseline_result_r": baseline_result,
        "model_result_r": baseline_result,
        "delta_r": 0.0,
        "baseline_exit_price": baseline_exit_price,
        "model_exit_price": baseline_exit_price,
        "entry_price": trade.get("entry_price"),
        "stop_price": trade.get("stop_price"),
        "risk_points": trade.get("risk_points"),
        "risk_ticks": trade.get("risk_ticks"),
        "tick_size": trade.get("tick_size"),
        "predicted_r": trade.get("predicted_r"),
        "exit_score_r": None,
        "baseline_hold_minutes": trade.get("hold_minutes"),
        "model_hold_minutes": model_hold,
    }


def apply_overlay(trades: pd.DataFrame, decisions: pd.DataFrame, threshold: float | None, args: argparse.Namespace) -> pd.DataFrame:
    mode = str(getattr(args, "mode", "early") or "early")
    if threshold is None:
        return pd.DataFrame([source_overlay_row(trade) for _, trade in trades.iterrows()])

    decision_groups = {
        str(candidate_uid): group.sort_values("decision_index")
        for candidate_uid, group in decisions.groupby("candidate_uid", sort=False)
    }
    rows: list[dict[str, Any]] = []
    for _, trade in trades.iterrows():
        candidate_uid = str(trade.get("candidate_uid") or "")
        group = decision_groups.get(candidate_uid)
        chosen = None
        if group is not None and not group.empty:
            scores = pd.to_numeric(group["exit_score_r"], errors="coerce")
            if mode == "dynamic":
                eligible = group[scores.fillna(999.0) <= threshold]
            else:
                eligible = group[scores.fillna(-999.0) >= threshold]
            if not eligible.empty:
                chosen = eligible.iloc[0]

        baseline_result = safe_float(trade.get("result_r"))
        baseline_exit_date = trade.get("exit_date")
        baseline_exit_price = trade.get("exit_price")
        if chosen is None:
            no_signal_exit = str(getattr(args, "dynamic_no_signal_exit", "terminal") or "terminal")
            if mode == "dynamic" and no_signal_exit == "source":
                model_result = baseline_result
                model_exit_date = baseline_exit_date
                model_exit_price = baseline_exit_price
                exit_reason = str(trade.get("exit_reason") or "source_exit")
                exit_score = None
            elif mode == "dynamic" and group is not None and not group.empty:
                terminal = group.iloc[-1]
                model_result = safe_float(terminal.get("terminal_result_r"))
                model_exit_date = terminal.get("terminal_exit_date")
                model_exit_price = terminal.get("terminal_exit_price")
                exit_reason = str(terminal.get("terminal_exit_reason") or "dynamic_terminal")
                exit_score = None
            else:
                model_result = baseline_result
                model_exit_date = baseline_exit_date
                model_exit_price = baseline_exit_price
                exit_reason = str(trade.get("exit_reason") or "source_exit")
                exit_score = None
        else:
            model_result = safe_float(chosen.get("exit_now_result_r"))
            model_exit_date = chosen.get("exit_now_date")
            model_exit_price = chosen.get("exit_now_price")
            exit_reason = "model_dynamic_exit" if mode == "dynamic" else "model_early_exit"
            exit_score = safe_float(chosen.get("exit_score_r"))

        entry_date = trade.get("entry_date")
        model_hold = None
        if not pd.isna(entry_date) and not pd.isna(model_exit_date):
            model_hold = int((pd.Timestamp(model_exit_date) - pd.Timestamp(entry_date)).total_seconds() / 60)

        rows.append(
            {
                "selected_index": int(trade.get("selected_index") or len(rows) + 1),
                "candidate_uid": candidate_uid,
                "formula": trade.get("formula"),
                "timeframe": trade.get("timeframe"),
                "valid_year": int(trade.get("valid_year") or 0),
                "root_symbol": trade.get("root_symbol"),
                "symbol": trade.get("symbol"),
                "entry_date": trade.get("entry_date"),
                "baseline_exit_date": baseline_exit_date,
                "model_exit_date": model_exit_date,
                "direction": trade.get("direction"),
                "baseline_exit_reason": trade.get("exit_reason"),
                "model_exit_reason": exit_reason,
                "baseline_result_r": baseline_result,
                "model_result_r": model_result,
                "delta_r": model_result - baseline_result,
                "baseline_exit_price": baseline_exit_price,
                "model_exit_price": model_exit_price,
                "entry_price": trade.get("entry_price"),
                "stop_price": trade.get("stop_price"),
                "risk_points": trade.get("risk_points"),
                "risk_ticks": trade.get("risk_ticks"),
                "tick_size": trade.get("tick_size"),
                "predicted_r": trade.get("predicted_r"),
                "exit_score_r": exit_score,
                "baseline_hold_minutes": trade.get("hold_minutes"),
                "model_hold_minutes": model_hold,
            }
        )
    return pd.DataFrame(rows)


def choose_exit_threshold(trades: pd.DataFrame, decisions: pd.DataFrame, args: argparse.Namespace) -> float | None:
    if args.exit_threshold is not None:
        return float(args.exit_threshold)
    mode = str(getattr(args, "mode", "early") or "early")
    scores = pd.to_numeric(decisions.get("exit_score_r", pd.Series(dtype=float)), errors="coerce").dropna()
    if scores.empty:
        return None if mode == "dynamic" else 999.0
    if mode == "dynamic":
        quantiles = sorted(set(float(v) for v in scores.quantile(np.linspace(0.01, 0.90, 36)).dropna()))
        candidates = [*quantiles, 0.0]
    else:
        quantiles = sorted(set(float(v) for v in scores.quantile(np.linspace(0.70, 0.995, 30)).dropna()))
        candidates = [0.0, *quantiles]
    baseline = summarize_results(trades["result_r"])
    no_exit_threshold = None if mode == "dynamic" else float(scores.max()) + max(1.0, abs(float(scores.max())) * 0.10)
    best_threshold: float | None = no_exit_threshold
    best_summary: dict[str, Any] | None = baseline
    best_objective = baseline["sum_r"] - float(args.threshold_dd_penalty) * baseline["max_drawdown_r"]
    max_dd = float(getattr(args, "max_threshold_dd_r", 0.0) or 0.0)
    print("Exit threshold calibration:")
    for threshold in candidates:
        overlay = apply_overlay(trades, decisions, threshold, args)
        summary = summarize_results(overlay["model_result_r"])
        if summary["trades"] < int(args.min_threshold_trades):
            continue
        if max_dd > 0.0 and summary["max_drawdown_r"] > max_dd:
            continue
        objective = summary["sum_r"] - float(args.threshold_dd_penalty) * summary["max_drawdown_r"]
        if best_summary is None or (
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
            best_summary = summary
            best_threshold = threshold
            best_objective = objective
    print(
        f"  baseline: trades={baseline['trades']:,}, sum={baseline['sum_r']:.2f}R, "
        f"dd={baseline['max_drawdown_r']:.2f}R"
    )
    print(
        f"  selected threshold {'disabled' if best_threshold is None else f'{best_threshold:.4f}'}: trades={best_summary['trades']:,}, "
        f"sum={best_summary['sum_r']:.2f}R, dd={best_summary['max_drawdown_r']:.2f}R"
    )
    return None if best_threshold is None else float(best_threshold)


def ensure_tables(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            f"""
            CREATE TABLE IF NOT EXISTS {RUN_TABLE} (
                exit_model_run_id VARCHAR(64) NOT NULL PRIMARY KEY,
                source_model_run_id VARCHAR(64) NOT NULL,
                timeframe VARCHAR(16) NOT NULL,
                train_years VARCHAR(64) NOT NULL,
                threshold_year INT NOT NULL,
                valid_year INT NOT NULL,
                exit_threshold DOUBLE NULL,
                train_trades INT NOT NULL DEFAULT 0,
                train_decision_rows INT NOT NULL DEFAULT 0,
                threshold_trades INT NOT NULL DEFAULT 0,
                threshold_decision_rows INT NOT NULL DEFAULT 0,
                valid_trades INT NOT NULL DEFAULT 0,
                early_exits INT NOT NULL DEFAULT 0,
                wins INT NOT NULL DEFAULT 0,
                losses INT NOT NULL DEFAULT 0,
                win_rate DOUBLE NOT NULL DEFAULT 0,
                avg_r DOUBLE NOT NULL DEFAULT 0,
                sum_r DOUBLE NOT NULL DEFAULT 0,
                max_drawdown_r DOUBLE NOT NULL DEFAULT 0,
                baseline_sum_r DOUBLE NOT NULL DEFAULT 0,
                baseline_max_drawdown_r DOUBLE NOT NULL DEFAULT 0,
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
                exit_model_run_id VARCHAR(64) NOT NULL,
                selected_index INT NOT NULL,
                candidate_uid VARCHAR(40) NOT NULL,
                source_model_run_id VARCHAR(64) NOT NULL,
                formula VARCHAR(128) NOT NULL,
                timeframe VARCHAR(16) NOT NULL,
                valid_year INT NOT NULL,
                root_symbol VARCHAR(32) NULL,
                symbol VARCHAR(64) NULL,
                entry_date DATETIME NULL,
                baseline_exit_date DATETIME NULL,
                model_exit_date DATETIME NULL,
                direction VARCHAR(16) NULL,
                baseline_exit_reason VARCHAR(64) NULL,
                model_exit_reason VARCHAR(64) NULL,
                baseline_result_r DOUBLE NOT NULL DEFAULT 0,
                model_result_r DOUBLE NOT NULL DEFAULT 0,
                delta_r DOUBLE NOT NULL DEFAULT 0,
                baseline_exit_price DOUBLE NULL,
                model_exit_price DOUBLE NULL,
                entry_price DOUBLE NULL,
                stop_price DOUBLE NULL,
                risk_points DOUBLE NULL,
                risk_ticks DOUBLE NULL,
                tick_size DOUBLE NULL,
                predicted_r DOUBLE NULL,
                exit_score_r DOUBLE NULL,
                baseline_hold_minutes INT NULL,
                model_hold_minutes INT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (exit_model_run_id, candidate_uid),
                INDEX idx_exit_model_trades_year (exit_model_run_id, valid_year, selected_index),
                INDEX idx_exit_model_trades_result (exit_model_run_id, model_result_r)
            )
            """
        )


def save_run(
    conn,
    run_id: str,
    args: argparse.Namespace,
    source: dict[str, Any],
    timeframe: str,
    threshold: float | None,
    train_trades: pd.DataFrame,
    train_decisions: pd.DataFrame,
    threshold_trades: pd.DataFrame,
    threshold_decisions: pd.DataFrame,
    valid_overlay: pd.DataFrame,
    model: CatBoostRegressor,
) -> dict[str, Any]:
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id
    model_dir.mkdir(parents=True, exist_ok=True)
    model_path = model_dir / "catboost_exit_model.cbm"
    model.save_model(str(model_path))

    summary = summarize_results(valid_overlay["model_result_r"])
    baseline = summarize_results(valid_overlay["baseline_result_r"])
    model_exit_dates = pd.to_datetime(valid_overlay["model_exit_date"], errors="coerce")
    baseline_exit_dates = pd.to_datetime(valid_overlay["baseline_exit_date"], errors="coerce")
    early_exits = int((model_exit_dates < baseline_exit_dates).fillna(False).sum())
    late_exits = int((model_exit_dates > baseline_exit_dates).fillna(False).sum())
    model_exits = int(valid_overlay["model_exit_reason"].astype(str).str.startswith("model_").sum())
    metadata = {
        "exit_model_run_id": run_id,
        "source_model_run_id": args.source_model_run_id,
        "timeframe": timeframe,
        "mode": args.mode,
        "train_years": scanner.parse_years(args.train_years),
        "threshold_year": args.threshold_year,
        "valid_year": args.valid_year,
        "exit_threshold": threshold,
        "dynamic_max_bars": args.dynamic_max_bars,
        "decision_step_bars": args.decision_step_bars,
        "dynamic_no_signal_exit": args.dynamic_no_signal_exit,
        "max_threshold_dd_r": args.max_threshold_dd_r,
        "threshold_dd_penalty": args.threshold_dd_penalty,
        "cat_features": EXIT_CAT_FEATURES,
        "num_features": EXIT_NUM_FEATURES,
        "phase": "dynamic_hold_exit_overlay" if args.mode == "dynamic" else "early_exit_overlay",
        "early_exits": early_exits,
        "late_exits": late_exits,
        "model_exits": model_exits,
        "notes": (
            "Dynamic mode can exit before or after the source strategy exit while keeping the original hard stop."
            if args.mode == "dynamic"
            else "Early mode can only exit before the source strategy exit; it cannot hold longer."
        ),
    }
    (model_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=str))

    with conn.cursor() as cur:
        cur.execute(f"DELETE FROM {TRADE_TABLE} WHERE exit_model_run_id = %s", (run_id,))
        cur.execute(f"DELETE FROM {RUN_TABLE} WHERE exit_model_run_id = %s", (run_id,))
        cur.execute(
            f"""
            INSERT INTO {RUN_TABLE} (
                exit_model_run_id, source_model_run_id, timeframe, train_years,
                threshold_year, valid_year, exit_threshold, train_trades,
                train_decision_rows, threshold_trades, threshold_decision_rows,
                valid_trades, early_exits, wins, losses, win_rate, avg_r,
                sum_r, max_drawdown_r, baseline_sum_r, baseline_max_drawdown_r,
                model_path, metadata_json
            )
            VALUES ({",".join(["%s"] * 23)})
            """,
            (
                run_id,
                args.source_model_run_id,
                timeframe,
                args.train_years,
                int(args.threshold_year),
                int(args.valid_year),
                threshold,
                int(len(train_trades)),
                int(len(train_decisions)),
                int(len(threshold_trades)),
                int(len(threshold_decisions)),
                int(len(valid_overlay)),
                early_exits,
                summary["wins"],
                summary["losses"],
                summary["win_rate"],
                summary["avg_r"],
                summary["sum_r"],
                summary["max_drawdown_r"],
                baseline["sum_r"],
                baseline["max_drawdown_r"],
                str(model_dir),
                json.dumps(metadata),
            ),
        )

        trade_columns = [
            "exit_model_run_id",
            "selected_index",
            "candidate_uid",
            "source_model_run_id",
            "formula",
            "timeframe",
            "valid_year",
            "root_symbol",
            "symbol",
            "entry_date",
            "baseline_exit_date",
            "model_exit_date",
            "direction",
            "baseline_exit_reason",
            "model_exit_reason",
            "baseline_result_r",
            "model_result_r",
            "delta_r",
            "baseline_exit_price",
            "model_exit_price",
            "entry_price",
            "stop_price",
            "risk_points",
            "risk_ticks",
            "tick_size",
            "predicted_r",
            "exit_score_r",
            "baseline_hold_minutes",
            "model_hold_minutes",
        ]
        rows = []
        for record in valid_overlay.to_dict("records"):
            payload = {"exit_model_run_id": run_id, "source_model_run_id": args.source_model_run_id, **record}
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
        **summary,
        "baseline": baseline,
        "early_exits": early_exits,
        "late_exits": late_exits,
        "model_exits": model_exits,
        "model_path": str(model_dir),
    }


def main() -> int:
    args = parse_args()
    conn = wave.connect()
    try:
        ensure_tables(conn)
        source = load_source_run(conn, args.source_model_run_id)
        metadata = source["metadata"]
        entry_args = entry_args_from_metadata(metadata)
        timeframe = str(entry_args.timeframe)
        table_name, timeframe_minutes = scanner.table_for_timeframe(timeframe)
        run_id = exit_run_id(args, timeframe)
        if not args.replace_run:
            with conn.cursor() as cur:
                cur.execute(f"SELECT 1 FROM {RUN_TABLE} WHERE exit_model_run_id = %s", (run_id,))
                if cur.fetchone():
                    raise ValueError(f"Exit model run already exists: {run_id}. Use --replace-run to overwrite.")

        entry_model = load_entry_model(source)
        train_years = scanner.parse_years(args.train_years)
        train_trades = selected_like_trades(conn, entry_model, entry_args, train_years, args.max_train_trades)
        threshold_trades = selected_like_trades(conn, entry_model, entry_args, [args.threshold_year], args.max_threshold_trades)
        valid_trades = load_selected_trades(conn, args.source_model_run_id, args.max_valid_trades)

        print(
            f"Exit model source={args.source_model_run_id} train_trades={len(train_trades):,} "
            f"threshold_trades={len(threshold_trades):,} valid_trades={len(valid_trades):,}"
        )

        train_decisions = build_decision_rows(conn, train_trades, table_name, timeframe_minutes, args, True, "train")
        if train_decisions.empty:
            raise ValueError("No train decision rows were generated.")
        model = train_exit_model(train_decisions, args)

        threshold_decisions = build_decision_rows(conn, threshold_trades, table_name, timeframe_minutes, args, False, "threshold")
        threshold_decisions = add_exit_predictions(threshold_decisions, model)
        threshold = choose_exit_threshold(threshold_trades, threshold_decisions, args)

        valid_decisions = build_decision_rows(conn, valid_trades, table_name, timeframe_minutes, args, False, "valid")
        valid_decisions = add_exit_predictions(valid_decisions, model)
        valid_overlay = apply_overlay(valid_trades, valid_decisions, threshold, args)
        summary = save_run(
            conn,
            run_id,
            args,
            source,
            timeframe,
            threshold,
            train_trades,
            train_decisions,
            threshold_trades,
            threshold_decisions,
            valid_overlay,
            model,
        )

        baseline = summary["baseline"]
        print(
            f"Materialized {run_id}: baseline {baseline['sum_r']:.2f}R / "
            f"{baseline['max_drawdown_r']:.2f}R DD -> exit model {summary['sum_r']:.2f}R / "
            f"{summary['max_drawdown_r']:.2f}R DD, model exits={summary['model_exits']:,}, "
            f"early={summary['early_exits']:,}, late={summary['late_exits']:,}, "
            f"threshold={'disabled' if threshold is None else f'{threshold:.4f}'}"
        )
    finally:
        conn.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
