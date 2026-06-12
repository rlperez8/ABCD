#!/usr/bin/env python3
"""
Prototype a live-style candle-wave runner for one symbol.

This is intentionally a replay harness, not an order router. It reads historical
candles from the DB, then feeds them through the current candle-wave entry model
one candle at a time. The engine keeps live-like state:

* flat / in-trade
* entry score and threshold
* hard stop
* AI exit score
* pending exits filled at the next candle open

The goal is to surface live-readiness issues before NinjaTrader order wiring.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd

try:
    from catboost import CatBoostRegressor
except ImportError as exc:  # pragma: no cover - runtime environment message
    raise SystemExit("CatBoost is required. Run with .venv_ai\\Scripts\\python.exe") from exc


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_exit_model as exit_model
import ai_candle_wave_scanner_model as scanner
import ai_wave_rider_research as wave


DEFAULT_ENTRY_RUN_ID = "aicw-mtf-eg1-xtight-t014-rd3-en4-v1-2m-2026"
DEFAULT_EXIT_RUN_ID = "aicw-exit-dyn180s2-srcfb-v1-2m-2026"


@dataclass
class LiveTrade:
    trade_id: str
    candidate_uid: str
    symbol: str
    root_symbol: str
    direction: str
    signal_idx: int
    entry_idx: int
    entry_date: pd.Timestamp
    entry_price: float
    stop_price: float
    current_stop: float
    source_stop: float
    risk_points: float
    risk_ticks: float
    tick_size: float
    predicted_r: float
    feature_row: dict[str, Any]
    max_favorable_r: float = 0.0
    max_adverse_r: float = 0.0
    source_death_count: int = 0
    source_exit_signal_seen: bool = False
    source_exit_idx: int | None = None
    source_exit_date: pd.Timestamp | None = None
    source_exit_price: float | None = None
    source_exit_result_r: float = 0.0
    source_exit_reason: str | None = None
    pending_exit_reason: str | None = None
    pending_exit_score: float | None = None
    pending_exit_created_idx: int | None = None
    exit_idx: int | None = None
    exit_date: pd.Timestamp | None = None
    exit_price: float | None = None
    exit_reason: str | None = None
    result_r: float | None = None
    raw_result_r: float | None = None


@dataclass
class LiveStats:
    candles_seen: int = 0
    entry_candidates: int = 0
    approved_entries: int = 0
    rejected_entries: int = 0
    exits: int = 0
    wins: int = 0
    losses: int = 0
    sum_r: float = 0.0
    loop_ms: list[float] = field(default_factory=list)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--symbol", default="", help="Contract symbol, e.g. MNQM6. If blank, --root is resolved.")
    parser.add_argument("--root", default="MNQ", help="Root used when --symbol is blank.")
    parser.add_argument("--start", default="2026-05-01T00:00:00")
    parser.add_argument("--end", default="2026-05-30T00:00:00")
    parser.add_argument("--max-candles", type=int, default=1200)
    parser.add_argument("--entry-run-id", default=DEFAULT_ENTRY_RUN_ID)
    parser.add_argument("--exit-run-id", default=DEFAULT_EXIT_RUN_ID)
    parser.add_argument("--output", default="", help="Optional CSV path for the decision log.")
    parser.add_argument(
        "--source-exit-mode",
        choices=["none", "model-gated", "always-exit"],
        default="none",
        help=(
            "none ignores legacy source exits. model-gated logs them as context only. "
            "always-exit exits at the next candle open when the legacy source strategy says exit."
        ),
    )
    parser.add_argument(
        "--risk-mode",
        choices=["live-safe", "backtest-compatible"],
        default="live-safe",
        help=(
            "live-safe builds entry risk from data known at the entry open. "
            "backtest-compatible uses the historical initial_stop helper for comparison."
        ),
    )
    parser.add_argument("--print-events", type=int, default=40)
    return parser.parse_args()


def load_registry_metadata(run_id: str) -> tuple[Path, dict[str, Any]]:
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id
    metadata_path = model_dir / "metadata.json"
    if not metadata_path.exists():
        raise FileNotFoundError(f"Missing metadata: {metadata_path}")
    return model_dir, json.loads(metadata_path.read_text())


def load_regressor(model_path: Path) -> CatBoostRegressor:
    if not model_path.exists():
        raise FileNotFoundError(f"Missing model file: {model_path}")
    model = CatBoostRegressor()
    model.load_model(str(model_path))
    return model


def value_from_meta(metadata: dict[str, Any], key: str, default: Any) -> Any:
    value = metadata.get(key)
    return default if value is None else value


def entry_args_from_registry(metadata: dict[str, Any]) -> argparse.Namespace:
    timeframe = str(value_from_meta(metadata, "timeframe", "2m"))
    max_forward_bars = int(value_from_meta(metadata, "max_forward_bars", 1440))
    values = {
        "timeframe": timeframe,
        "context_timeframes": ",".join(value_from_meta(metadata, "context_timeframes", ["5m", "15m", "1h", "4h"])),
        "context_lookback_bars": int(value_from_meta(metadata, "context_lookback_bars", 80)),
        "candidate_min_gap_bars": int(value_from_meta(metadata, "candidate_min_gap_bars", 15)),
        "roots": "",
        "formula": str(value_from_meta(metadata, "formula", scanner.formula_for(timeframe))),
        "entry_breakout_bars": int(value_from_meta(metadata, "entry_breakout_bars", 8)),
        "trail_lookback_bars": int(value_from_meta(metadata, "trail_lookback_bars", 18)),
        "atr_period": int(value_from_meta(metadata, "atr_period", 14)),
        "atr_stop_pad": float(value_from_meta(metadata, "atr_stop_pad", 0.35)),
        "min_risk_ticks": float(value_from_meta(metadata, "min_risk_ticks", 12.0)),
        "max_risk_ticks": float(value_from_meta(metadata, "max_risk_ticks", 240.0)),
        "min_relative_volume": float(value_from_meta(metadata, "min_relative_volume", 0.45)),
        "death_bars": int(value_from_meta(metadata, "death_bars", 4)),
        "min_hold_bars": int(value_from_meta(metadata, "min_hold_bars", 4)),
        "profit_lock_trigger_r": float(value_from_meta(metadata, "profit_lock_trigger_r", 0.0)),
        "profit_lock_giveback_r": float(value_from_meta(metadata, "profit_lock_giveback_r", 1.5)),
        "profit_lock_min_r": float(value_from_meta(metadata, "profit_lock_min_r", 0.25)),
        "max_forward_bars": max_forward_bars,
        "max_forward_minutes": max_forward_bars,
        "time_exit_bars": int(value_from_meta(metadata, "time_exit_bars", 0)),
        "time_exit_minutes": int(value_from_meta(metadata, "time_exit_bars", 0)),
        "slippage_entry_ticks": float(value_from_meta(metadata, "slippage_entry_ticks", 3.0)),
        "slippage_exit_ticks": float(value_from_meta(metadata, "slippage_exit_ticks", 3.0)),
        "loose_triggers": bool(value_from_meta(metadata, "loose_triggers", False)),
        "mtf_entry_min_aligned": int(value_from_meta(metadata, "mtf_entry_min_aligned", 0)),
        "mtf_exit_governor": bool(value_from_meta(metadata, "mtf_exit_governor", False)),
        "mtf_exit_min_aligned": int(value_from_meta(metadata, "mtf_exit_min_aligned", 1)),
        "mtf_exit_mode": str(value_from_meta(metadata, "mtf_exit_mode", "hard-veto")),
        "mtf_exit_extra_death_bars": int(value_from_meta(metadata, "mtf_exit_extra_death_bars", 1)),
        "mtf_exit_tighten_lookback_bars": int(value_from_meta(metadata, "mtf_exit_tighten_lookback_bars", 3)),
        "mtf_exit_tighten_pad_ticks": float(value_from_meta(metadata, "mtf_exit_tighten_pad_ticks", 1.0)),
        "threshold": metadata.get("threshold"),
        "allowed_roots": ",".join(value_from_meta(metadata, "allowed_roots", [])),
        "cooldown_minutes": int(value_from_meta(metadata, "cooldown_minutes", 0)),
        "max_root_trades_per_day": int(value_from_meta(metadata, "max_root_trades_per_day", 0)),
        "max_energy_trades_per_day": int(value_from_meta(metadata, "max_energy_trades_per_day", 0)),
        "energy_extra_slot_start": int(value_from_meta(metadata, "energy_extra_slot_start", 0)),
        "energy_extra_slot_min_score": float(value_from_meta(metadata, "energy_extra_slot_min_score", 0.0)),
        "loss_brake_r": float(value_from_meta(metadata, "loss_brake_r", 0.0)),
        "loss_brake_scope": str(value_from_meta(metadata, "loss_brake_scope", "root")),
        "loss_brake_min_trades": int(value_from_meta(metadata, "loss_brake_min_trades", 1)),
    }
    return argparse.Namespace(**values)


def exit_args_from_registry(metadata: dict[str, Any]) -> argparse.Namespace:
    return argparse.Namespace(
        mode=str(value_from_meta(metadata, "mode", "dynamic")),
        exit_threshold=metadata.get("exit_threshold"),
        dynamic_max_bars=int(value_from_meta(metadata, "dynamic_max_bars", 180)),
        decision_step_bars=int(value_from_meta(metadata, "decision_step_bars", 2)),
        dynamic_no_signal_exit=str(value_from_meta(metadata, "dynamic_no_signal_exit", "source")),
        min_hold_bars=int(value_from_meta(metadata, "min_hold_bars", 4)),
        exclude_source_exit_features=bool(value_from_meta(metadata, "exclude_source_exit_features", False)),
    )


def resolve_symbol(conn, root: str, table_name: str, end: pd.Timestamp) -> str:
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT symbol, MAX(ts_utc) AS last_ts, COUNT(*) AS rows_seen
            FROM {scanner.safe_identifier(table_name)}
            WHERE root_symbol = %s
              AND ts_utc < %s
            GROUP BY symbol
            ORDER BY last_ts DESC, rows_seen DESC
            LIMIT 1
            """,
            (root.upper(), end.to_pydatetime()),
        )
        row = cur.fetchone()
    if not row:
        raise RuntimeError(f"Could not resolve a symbol for root {root!r} in {table_name}")
    return str(row["symbol"])


def load_candles(conn, table_name: str, symbol: str, start: pd.Timestamp, end: pd.Timestamp) -> pd.DataFrame:
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT root_symbol, symbol, ts_utc,
                   CAST(open AS DOUBLE) AS open,
                   CAST(high AS DOUBLE) AS high,
                   CAST(low AS DOUBLE) AS low,
                   CAST(close AS DOUBLE) AS close,
                   CAST(volume AS DOUBLE) AS volume
            FROM {scanner.safe_identifier(table_name)}
            WHERE symbol = %s
              AND ts_utc >= %s
              AND ts_utc < %s
            ORDER BY ts_utc
            """,
            (symbol, start.to_pydatetime(), end.to_pydatetime()),
        )
        rows = cur.fetchall()
    frame = pd.DataFrame(rows)
    if frame.empty:
        return frame
    frame["ts_utc"] = pd.to_datetime(frame["ts_utc"], errors="coerce")
    for col in ["open", "high", "low", "close", "volume"]:
        frame[col] = pd.to_numeric(frame[col], errors="coerce")
    return frame.dropna(subset=["ts_utc", "open", "high", "low", "close"]).reset_index(drop=True)


def live_safe_initial_stop(
    candles: pd.DataFrame,
    signal_idx: int,
    entry_idx: int,
    direction: str,
    tick_size: float,
    args: argparse.Namespace,
) -> tuple[float | None, float | None]:
    if entry_idx >= len(candles):
        return None, None
    entry_price = wave.finite(candles["open"].iloc[entry_idx])
    if entry_price is None:
        return None, None
    signal_row = candles.iloc[signal_idx]
    entry_row = candles.iloc[entry_idx]
    atr = wave.finite(signal_row.get("atr"), tick_size * args.min_risk_ticks) or tick_size * args.min_risk_ticks
    pad = max(tick_size, atr * args.atr_stop_pad)
    if direction == "LONG":
        swing = wave.finite(entry_row.get("swing_low"))
        raw_stop = (swing - pad) if swing is not None else entry_price - tick_size * args.min_risk_ticks
        risk = entry_price - raw_stop
        min_risk = tick_size * args.min_risk_ticks
        if risk < min_risk:
            raw_stop = entry_price - min_risk
            risk = min_risk
    else:
        swing = wave.finite(entry_row.get("swing_high"))
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


def candidate_result_for_live_entry(
    candles: pd.DataFrame,
    signal_idx: int,
    entry_idx: int,
    direction: str,
    tick_size: float,
    entry_args: argparse.Namespace,
    risk_mode: str,
) -> wave.WaveResult:
    if risk_mode == "backtest-compatible":
        stop_price, risk_points = wave.initial_stop(candles, entry_idx, direction, tick_size, entry_args)
    else:
        stop_price, risk_points = live_safe_initial_stop(candles, signal_idx, entry_idx, direction, tick_size, entry_args)
    entry_price = wave.finite(candles["open"].iloc[entry_idx]) if entry_idx < len(candles) else None
    if stop_price is None or risk_points is None or entry_price is None:
        return wave.no_entry("risk_rejected", signal_idx=signal_idx, candidate_count=1)
    risk_ticks = risk_points / tick_size if tick_size > 0 else None
    signal_date = pd.Timestamp(candles["ts_utc"].iloc[signal_idx])
    entry_date = pd.Timestamp(candles["ts_utc"].iloc[entry_idx])
    return wave.WaveResult(
        outcome="live_pending",
        exit_reason="open_trade",
        trade_direction=direction,
        entry_date=entry_date,
        exit_date=None,
        entry_price=float(entry_price),
        stop_price=float(stop_price),
        target_price=None,
        exit_price=None,
        risk_points=float(risk_points),
        result_r=0.0,
        raw_result_r=None,
        mfe_r=None,
        mae_r=None,
        signal_index=signal_idx,
        entry_index=entry_idx,
        exit_index=None,
        entry_delay_minutes=wave.minutes_between(signal_date, entry_date),
        hold_minutes=None,
        risk_ticks=float(risk_ticks) if risk_ticks is not None else None,
        trigger_score=wave.trigger_score(candles, signal_idx, direction),
        candidate_count=1,
        no_entry_reason=None,
    )


def score_entry_candidate(
    entry_model: CatBoostRegressor,
    year: int,
    root_symbol: str,
    symbol: str,
    candles: pd.DataFrame,
    context_frames: dict[str, pd.DataFrame],
    signal_idx: int,
    entry_idx: int,
    direction: str,
    tick_size: float,
    entry_args: argparse.Namespace,
    risk_mode: str,
) -> tuple[float | None, dict[str, Any] | None, wave.WaveResult]:
    result = candidate_result_for_live_entry(candles, signal_idx, entry_idx, direction, tick_size, entry_args, risk_mode)
    if result.outcome == "no_entry":
        return None, None, result
    signal_ts = pd.Timestamp(candles["ts_utc"].iloc[signal_idx])
    signal_minutes = scanner.table_for_timeframe(entry_args.timeframe)[1]
    context_features = scanner.all_context_features(context_frames, signal_ts, signal_minutes, direction, tick_size)
    aligned_count = scanner.context_alignment_count(context_features, entry_args)
    if not scanner.entry_context_allowed(context_features, entry_args):
        return None, None, wave.no_entry("mtf_entry_not_aligned", signal_idx=signal_idx, candidate_count=1)
    row = scanner.feature_row(
        year,
        entry_args.timeframe,
        entry_args.formula,
        root_symbol,
        symbol,
        candles,
        signal_idx,
        result,
        reverse_result=None,
        context_features=context_features,
        mtf_entry_aligned_count=aligned_count,
    )
    frame = pd.DataFrame([row])
    predicted = float(entry_model.predict(scanner.prepare_pool(frame, include_target=False))[0])
    row["predicted_r"] = predicted
    return predicted, row, result


def sign_for(direction: str) -> float:
    return -1.0 if direction == "SHORT" else 1.0


def slipped_result_r(trade: LiveTrade, exit_price: float) -> tuple[float, float]:
    raw = sign_for(trade.direction) * (exit_price - trade.entry_price) / trade.risk_points
    slipped = raw - ((3.0 + 3.0) * trade.tick_size) / trade.risk_points
    return float(raw), float(slipped)


def static_exit_payload(trade: LiveTrade) -> dict[str, Any]:
    payload = {}
    for col in exit_model.EXIT_CAT_FEATURES + scanner.NUM_FEATURES:
        payload[col] = trade.feature_row.get(col)
    payload["symbol"] = trade.symbol
    payload["predicted_r"] = trade.predicted_r
    return payload


def build_live_exit_decision_row(
    trade: LiveTrade,
    candles: pd.DataFrame,
    candle_idx: int,
    timeframe_minutes: int,
    exit_args: argparse.Namespace,
) -> dict[str, Any] | None:
    next_idx = candle_idx + 1
    if next_idx >= len(candles):
        return None
    open_next = wave.finite(candles["open"].iloc[next_idx])
    close_price = wave.finite(candles["close"].iloc[candle_idx])
    if open_next is None or close_price is None:
        return None
    entry_idx = trade.entry_idx
    risk_points = trade.risk_points
    sign = sign_for(trade.direction)
    high_values = candles["high"]
    low_values = candles["low"]
    close_values = candles["close"]
    high_so_far = float(high_values.iloc[entry_idx : candle_idx + 1].max())
    low_so_far = float(low_values.iloc[entry_idx : candle_idx + 1].min())
    if trade.direction == "LONG":
        mfe = (high_so_far - trade.entry_price) / risk_points
        mae = (trade.entry_price - low_so_far) / risk_points
        distance_to_stop = (close_price - trade.current_stop) / risk_points
    else:
        mfe = (trade.entry_price - low_so_far) / risk_points
        mae = (high_so_far - trade.entry_price) / risk_points
        distance_to_stop = (trade.current_stop - close_price) / risk_points
    current_raw = sign * (close_price - trade.entry_price) / risk_points
    slippage_r = ((3.0 + 3.0) * trade.tick_size) / risk_points
    current_unrealized = current_raw - slippage_r
    exit_now_raw = sign * (open_next - trade.entry_price) / risk_points
    exit_now_result = exit_now_raw - slippage_r
    bars_held = candle_idx - entry_idx
    max_exit_bars = max(1, int(exit_args.dynamic_max_bars))
    bars_remaining = max(0, max_exit_bars - bars_held)
    window_progress = max(0.0, min(1.0, bars_held / max_exit_bars))
    source_seen = 1.0 if trade.source_exit_signal_seen else 0.0
    bars_since_source = max(0, candle_idx - int(trade.source_exit_idx or candle_idx)) if trade.source_exit_signal_seen else 0
    last_open = wave.finite(candles["open"].iloc[candle_idx], close_price) or close_price
    high = wave.finite(candles["high"].iloc[candle_idx], close_price) or close_price
    low = wave.finite(candles["low"].iloc[candle_idx], close_price) or close_price
    volume_entry = max(1.0, wave.finite(candles["volume"].iloc[entry_idx], 1.0) or 1.0)
    payload = {
        **static_exit_payload(trade),
        "candidate_uid": trade.candidate_uid,
        "decision_index": candle_idx - entry_idx,
        "decision_date": candles["ts_utc"].iloc[candle_idx],
        "exit_now_date": candles["ts_utc"].iloc[next_idx],
        "exit_now_price": open_next,
        "exit_now_result_r": exit_now_result,
        "baseline_result_r": trade.source_exit_result_r,
        "terminal_exit_date": None,
        "terminal_exit_price": None,
        "terminal_result_r": None,
        "terminal_exit_reason": None,
        "bars_held": bars_held,
        "hold_minutes_so_far": bars_held * timeframe_minutes,
        "max_exit_bars": max_exit_bars,
        "bars_remaining": bars_remaining,
        "window_progress": window_progress,
        "near_window_end": 1.0 if bars_remaining <= max(4, int(exit_args.decision_step_bars) * 3) else 0.0,
        "source_exit_signal_seen": source_seen,
        "bars_since_source_exit_signal": bars_since_source,
        "source_exit_result_seen_r": trade.source_exit_result_r if trade.source_exit_signal_seen else 0.0,
        "current_vs_source_exit_r": current_unrealized - trade.source_exit_result_r if trade.source_exit_signal_seen else 0.0,
        "current_unrealized_r": current_unrealized,
        "current_unrealized_raw_r": current_raw,
        "mfe_so_far_r": mfe,
        "mae_so_far_r": mae,
        "giveback_from_mfe_r": max(0.0, mfe - current_raw),
        "distance_to_stop_r": distance_to_stop,
        "last_bar_r": sign * (close_price - last_open) / risk_points,
        "ret_3_r": sign * (close_price - (wave.finite(close_values.iloc[max(entry_idx, candle_idx - 3)], close_price) or close_price)) / risk_points,
        "ret_6_r": sign * (close_price - (wave.finite(close_values.iloc[max(entry_idx, candle_idx - 6)], close_price) or close_price)) / risk_points,
        "range_3_r": (float(high_values.iloc[max(entry_idx, candle_idx - 2) : candle_idx + 1].max()) - float(low_values.iloc[max(entry_idx, candle_idx - 2) : candle_idx + 1].min())) / risk_points,
        "range_6_r": (float(high_values.iloc[max(entry_idx, candle_idx - 5) : candle_idx + 1].max()) - float(low_values.iloc[max(entry_idx, candle_idx - 5) : candle_idx + 1].min())) / risk_points,
        "close_position_6": exit_model.close_position(close_values, candle_idx, 6),
        "body_r": abs(close_price - last_open) / risk_points,
        "upper_wick_r": max(0.0, high - max(close_price, last_open)) / risk_points,
        "lower_wick_r": max(0.0, min(close_price, last_open) - low) / risk_points,
        "volume_vs_entry": (wave.finite(candles["volume"].iloc[candle_idx], 0.0) or 0.0) / volume_entry,
    }
    return payload


def maybe_update_source_state(
    trade: LiveTrade,
    candles: pd.DataFrame,
    context_frames: dict[str, pd.DataFrame],
    candle_idx: int,
    timeframe_minutes: int,
    entry_args: argparse.Namespace,
) -> None:
    if trade.source_exit_signal_seen:
        return

    candle = candles.iloc[candle_idx]
    high = float(candle["high"])
    low = float(candle["low"])
    if trade.direction == "LONG" and low <= trade.source_stop:
        _, slipped = slipped_result_r(trade, trade.source_stop)
        trade.source_exit_signal_seen = True
        trade.source_exit_idx = candle_idx
        trade.source_exit_date = pd.Timestamp(candles["ts_utc"].iloc[candle_idx])
        trade.source_exit_price = float(trade.source_stop)
        trade.source_exit_result_r = slipped
        trade.source_exit_reason = "source_trailing_stop"
        return
    if trade.direction == "SHORT" and high >= trade.source_stop:
        _, slipped = slipped_result_r(trade, trade.source_stop)
        trade.source_exit_signal_seen = True
        trade.source_exit_idx = candle_idx
        trade.source_exit_date = pd.Timestamp(candles["ts_utc"].iloc[candle_idx])
        trade.source_exit_price = float(trade.source_stop)
        trade.source_exit_result_r = slipped
        trade.source_exit_reason = "source_trailing_stop"
        return

    if candle_idx - trade.entry_idx < entry_args.min_hold_bars:
        return
    if not wave.momentum_dead(candles, candle_idx, trade.direction):
        trade.source_death_count = 0
        trade.source_stop = wave.update_profit_lock_stop(
            trade.source_stop,
            trade.entry_price,
            trade.risk_points,
            trade.max_favorable_r,
            trade.direction,
            trade.tick_size,
            entry_args,
        )
        trade.source_stop = wave.update_trailing_stop(
            candles,
            trade.source_stop,
            trade.entry_idx,
            candle_idx,
            trade.direction,
            trade.tick_size,
            entry_args,
        )
        return
    exit_threshold = max(1, int(entry_args.death_bars))
    mtf_supported = scanner.higher_timeframes_support_hold(
        context_frames,
        pd.Timestamp(candles["ts_utc"].iloc[candle_idx]),
        timeframe_minutes,
        trade.direction,
        trade.tick_size,
        entry_args,
    )
    exit_mode = scanner.mtf_exit_mode(entry_args)
    if mtf_supported and exit_mode == "hard-veto":
        trade.source_death_count = 0
        return
    trade.source_death_count += 1
    if mtf_supported:
        if exit_mode in {"tighten", "delay-tighten"}:
            trade.source_stop = scanner.tighten_stop_for_mtf_hold(
                candles,
                trade.source_stop,
                trade.entry_idx,
                candle_idx,
                trade.direction,
                trade.tick_size,
                entry_args,
            )
        if exit_mode in {"delay", "delay-tighten"}:
            exit_threshold += max(1, int(entry_args.mtf_exit_extra_death_bars))
    if trade.source_death_count >= exit_threshold and not trade.source_exit_signal_seen:
        next_idx = min(candle_idx + 1, len(candles) - 1)
        exit_price = wave.finite(candles["open"].iloc[next_idx], candles["close"].iloc[next_idx])
        if exit_price is None:
            return
        _, slipped = slipped_result_r(trade, float(exit_price))
        trade.source_exit_signal_seen = True
        trade.source_exit_idx = candle_idx
        trade.source_exit_date = pd.Timestamp(candles["ts_utc"].iloc[next_idx])
        trade.source_exit_price = float(exit_price)
        trade.source_exit_result_r = slipped
        trade.source_exit_reason = "source_momentum_fade"
        return

    trade.source_stop = wave.update_profit_lock_stop(
        trade.source_stop,
        trade.entry_price,
        trade.risk_points,
        trade.max_favorable_r,
        trade.direction,
        trade.tick_size,
        entry_args,
    )
    trade.source_stop = wave.update_trailing_stop(
        candles,
        trade.source_stop,
        trade.entry_idx,
        candle_idx,
        trade.direction,
        trade.tick_size,
        entry_args,
    )


def close_trade(
    trade: LiveTrade,
    candles: pd.DataFrame,
    candle_idx: int,
    exit_price: float,
    exit_reason: str,
    exit_score: float | None,
    events: list[dict[str, Any]],
    stats: LiveStats,
) -> None:
    raw, slipped = slipped_result_r(trade, exit_price)
    trade.exit_idx = candle_idx
    trade.exit_date = pd.Timestamp(candles["ts_utc"].iloc[candle_idx])
    trade.exit_price = float(exit_price)
    trade.exit_reason = exit_reason
    trade.raw_result_r = raw
    trade.result_r = slipped
    stats.exits += 1
    stats.sum_r += slipped
    if slipped > 0:
        stats.wins += 1
    else:
        stats.losses += 1
    events.append(
        {
            "ts_utc": trade.exit_date,
            "event": "EXIT",
            "trade_id": trade.trade_id,
            "symbol": trade.symbol,
            "direction": trade.direction,
            "price": exit_price,
            "reason": exit_reason,
            "entry_price": trade.entry_price,
            "stop_price": trade.stop_price,
            "current_stop": trade.current_stop,
            "predicted_r": trade.predicted_r,
            "exit_score_r": exit_score,
            "raw_result_r": raw,
            "result_r": slipped,
            "mfe_r": trade.max_favorable_r,
            "mae_r": trade.max_adverse_r,
        }
    )


def append_event(events: list[dict[str, Any]], **kwargs: Any) -> None:
    events.append(kwargs)


def run_replay(
    candles: pd.DataFrame,
    context_frames: dict[str, pd.DataFrame],
    symbol: str,
    root_symbol: str,
    entry_model: CatBoostRegressor,
    exit_regressor: CatBoostRegressor,
    entry_args: argparse.Namespace,
    exit_args: argparse.Namespace,
    args: argparse.Namespace,
) -> tuple[LiveStats, list[dict[str, Any]]]:
    timeframe_minutes = scanner.table_for_timeframe(entry_args.timeframe)[1]
    tick_size = wave.tick_size_for(symbol, root_symbol)
    threshold = None if entry_args.threshold is None else float(entry_args.threshold)
    exit_threshold = None if exit_args.exit_threshold is None else float(exit_args.exit_threshold)
    stats = LiveStats()
    events: list[dict[str, Any]] = []
    active: LiveTrade | None = None
    closed_trades: list[LiveTrade] = []
    last_signal_by_direction = {"LONG": -10**9, "SHORT": -10**9}
    start_idx = max(30, int(entry_args.trail_lookback_bars) + 8)
    end_idx = len(candles) - 1
    if args.max_candles > 0:
        end_idx = min(end_idx, start_idx + args.max_candles)

    for candle_idx in range(start_idx + 1, end_idx):
        loop_start = time.perf_counter()
        stats.candles_seen += 1
        candle_ts = pd.Timestamp(candles["ts_utc"].iloc[candle_idx])

        if active is not None and active.pending_exit_reason:
            exit_price = wave.finite(candles["open"].iloc[candle_idx])
            if exit_price is not None:
                close_trade(
                    active,
                    candles,
                    candle_idx,
                    float(exit_price),
                    active.pending_exit_reason,
                    active.pending_exit_score,
                    events,
                    stats,
                )
                closed_trades.append(active)
                active = None

        if active is None:
            signal_idx = candle_idx - 1
            triggered: list[tuple[float, str, dict[str, Any], wave.WaveResult]] = []
            for direction in ["LONG", "SHORT"]:
                if signal_idx - last_signal_by_direction[direction] < entry_args.candidate_min_gap_bars:
                    continue
                if not wave.is_wave_trigger(candles, signal_idx, direction, entry_args, loose=bool(entry_args.loose_triggers)):
                    continue
                score, row, result = score_entry_candidate(
                    entry_model,
                    int(pd.Timestamp(candles["ts_utc"].iloc[signal_idx]).year),
                    root_symbol,
                    symbol,
                    candles,
                    context_frames,
                    signal_idx,
                    candle_idx,
                    direction,
                    tick_size,
                    entry_args,
                    args.risk_mode,
                )
                last_signal_by_direction[direction] = signal_idx
                stats.entry_candidates += 1
                if score is None or row is None or result.outcome == "no_entry":
                    stats.rejected_entries += 1
                    append_event(
                        events,
                        ts_utc=candle_ts,
                        event="ENTRY_REJECT",
                        trade_id="",
                        symbol=symbol,
                        direction=direction,
                        price=None,
                        reason=result.no_entry_reason or result.exit_reason,
                        entry_price=None,
                        stop_price=None,
                        current_stop=None,
                        predicted_r=score,
                        exit_score_r=None,
                        raw_result_r=None,
                        result_r=None,
                        mfe_r=None,
                        mae_r=None,
                    )
                    continue
                triggered.append((float(score), direction, row, result))
            triggered.sort(key=lambda item: item[0], reverse=True)
            if triggered:
                score, direction, row, result = triggered[0]
                if threshold is not None and score < threshold:
                    stats.rejected_entries += 1
                    append_event(
                        events,
                        ts_utc=candle_ts,
                        event="ENTRY_SKIP",
                        trade_id="",
                        symbol=symbol,
                        direction=direction,
                        price=result.entry_price,
                        reason="score_below_threshold",
                        entry_price=result.entry_price,
                        stop_price=result.stop_price,
                        current_stop=result.stop_price,
                        predicted_r=score,
                        exit_score_r=None,
                        raw_result_r=None,
                        result_r=None,
                        mfe_r=None,
                        mae_r=None,
                    )
                else:
                    stats.approved_entries += 1
                    candidate_uid = str(row["candidate_uid"])
                    active = LiveTrade(
                        trade_id=f"live-proto-{stats.approved_entries:04d}",
                        candidate_uid=candidate_uid,
                        symbol=symbol,
                        root_symbol=root_symbol,
                        direction=direction,
                        signal_idx=signal_idx,
                        entry_idx=candle_idx,
                        entry_date=pd.Timestamp(result.entry_date),
                        entry_price=float(result.entry_price),
                        stop_price=float(result.stop_price),
                        current_stop=float(result.stop_price),
                        source_stop=float(result.stop_price),
                        risk_points=float(result.risk_points),
                        risk_ticks=float(result.risk_ticks or 0.0),
                        tick_size=tick_size,
                        predicted_r=score,
                        feature_row=row,
                    )
                    append_event(
                        events,
                        ts_utc=candle_ts,
                        event="ENTRY",
                        trade_id=active.trade_id,
                        symbol=symbol,
                        direction=direction,
                        price=active.entry_price,
                        reason="model_approved",
                        entry_price=active.entry_price,
                        stop_price=active.stop_price,
                        current_stop=active.current_stop,
                        predicted_r=score,
                        exit_score_r=None,
                        raw_result_r=None,
                        result_r=None,
                        mfe_r=None,
                        mae_r=None,
                    )

        if active is not None:
            candle = candles.iloc[candle_idx]
            high = float(candle["high"])
            low = float(candle["low"])
            if active.direction == "LONG":
                active.max_favorable_r = max(active.max_favorable_r, (high - active.entry_price) / active.risk_points)
                active.max_adverse_r = max(active.max_adverse_r, (active.entry_price - low) / active.risk_points)
                if low <= active.current_stop:
                    close_trade(active, candles, candle_idx, active.current_stop, "hard_or_trailing_stop", None, events, stats)
                    closed_trades.append(active)
                    active = None
            else:
                active.max_favorable_r = max(active.max_favorable_r, (active.entry_price - low) / active.risk_points)
                active.max_adverse_r = max(active.max_adverse_r, (high - active.entry_price) / active.risk_points)
                if high >= active.current_stop:
                    close_trade(active, candles, candle_idx, active.current_stop, "hard_or_trailing_stop", None, events, stats)
                    closed_trades.append(active)
                    active = None

        if active is not None:
            if args.source_exit_mode != "none":
                source_seen_before = active.source_exit_signal_seen
                maybe_update_source_state(active, candles, context_frames, candle_idx, timeframe_minutes, entry_args)
                if active.source_exit_signal_seen and not source_seen_before:
                    append_event(
                        events,
                        ts_utc=active.source_exit_date,
                        event="SOURCE_EXIT_SIGNAL",
                        trade_id=active.trade_id,
                        symbol=symbol,
                        direction=active.direction,
                        price=active.source_exit_price,
                        reason=active.source_exit_reason or "source_exit",
                        entry_price=active.entry_price,
                        stop_price=active.stop_price,
                        current_stop=active.current_stop,
                        predicted_r=active.predicted_r,
                        exit_score_r=None,
                        raw_result_r=None,
                        result_r=active.source_exit_result_r,
                        mfe_r=active.max_favorable_r,
                        mae_r=active.max_adverse_r,
                    )
            if active.source_exit_signal_seen and args.source_exit_mode == "always-exit":
                active.pending_exit_reason = active.source_exit_reason or "source_exit"
                active.pending_exit_score = None
                active.pending_exit_created_idx = candle_idx
            elif candle_idx - active.entry_idx >= int(exit_args.min_hold_bars):
                decision = build_live_exit_decision_row(active, candles, candle_idx, timeframe_minutes, exit_args)
                if decision is not None and exit_threshold is not None:
                    scored = pd.DataFrame([decision])
                    exit_score = float(exit_regressor.predict(exit_model.prepare_exit_pool(scored, include_target=False, args=exit_args))[0])
                    if exit_score <= exit_threshold:
                        active.pending_exit_reason = "model_dynamic_exit"
                        active.pending_exit_score = exit_score
                        active.pending_exit_created_idx = candle_idx
                        append_event(
                            events,
                            ts_utc=candle_ts,
                            event="EXIT_SIGNAL",
                            trade_id=active.trade_id,
                            symbol=symbol,
                            direction=active.direction,
                            price=wave.finite(candles["close"].iloc[candle_idx]),
                            reason="model_dynamic_exit",
                            entry_price=active.entry_price,
                            stop_price=active.stop_price,
                            current_stop=active.current_stop,
                            predicted_r=active.predicted_r,
                            exit_score_r=exit_score,
                            raw_result_r=None,
                            result_r=None,
                            mfe_r=active.max_favorable_r,
                            mae_r=active.max_adverse_r,
                        )
            if active is not None:
                max_bars = int(exit_args.dynamic_max_bars)
                if candle_idx - active.entry_idx >= max_bars:
                    active.pending_exit_reason = "dynamic_time_exit"
                    active.pending_exit_score = None
                    active.pending_exit_created_idx = candle_idx

        stats.loop_ms.append((time.perf_counter() - loop_start) * 1000.0)

    if active is not None:
        exit_price = wave.finite(candles["close"].iloc[end_idx])
        if exit_price is not None:
            close_trade(active, candles, end_idx, float(exit_price), "replay_end_mark", None, events, stats)
            closed_trades.append(active)
    return stats, events


def write_events(path: Path, events: list[dict[str, Any]]) -> None:
    if not events:
        return
    columns = [
        "ts_utc",
        "event",
        "trade_id",
        "symbol",
        "direction",
        "price",
        "reason",
        "entry_price",
        "stop_price",
        "current_stop",
        "predicted_r",
        "exit_score_r",
        "raw_result_r",
        "result_r",
        "mfe_r",
        "mae_r",
    ]
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=columns, extrasaction="ignore")
        writer.writeheader()
        for event in events:
            row = {column: event.get(column) for column in columns}
            writer.writerow(row)


def fmt(value: Any, digits: int = 4) -> str:
    if value is None:
        return "n/a"
    try:
        number = float(value)
    except (TypeError, ValueError):
        return str(value)
    if not math.isfinite(number):
        return "n/a"
    return f"{number:.{digits}f}"


def main() -> int:
    args = parse_args()
    entry_dir, entry_metadata = load_registry_metadata(args.entry_run_id)
    exit_dir, exit_metadata = load_registry_metadata(args.exit_run_id)
    entry_args = entry_args_from_registry(entry_metadata)
    exit_args = exit_args_from_registry(exit_metadata)
    table_name, timeframe_minutes = scanner.table_for_timeframe(entry_args.timeframe)
    start = pd.Timestamp(args.start)
    end = pd.Timestamp(args.end)

    entry_regressor = load_regressor(entry_dir / "catboost_model.cbm")
    exit_regressor = load_regressor(exit_dir / "catboost_exit_model.cbm")

    with wave.connect() as conn:
        symbol = args.symbol.strip().upper() or resolve_symbol(conn, args.root, table_name, end)
        lookback_start = start - pd.Timedelta(minutes=max(300, entry_args.context_lookback_bars * 240))
        candles = load_candles(conn, table_name, symbol, lookback_start, end)
        if candles.empty:
            raise RuntimeError(f"No {entry_args.timeframe} candles found for {symbol}")
        candles = scanner.enrich_candles(candles[["ts_utc", "open", "high", "low", "close", "volume"]].copy(), entry_args)
        candles = candles[candles["ts_utc"] >= start].reset_index(drop=True)
        if len(candles) < 80:
            raise RuntimeError(f"Not enough replay candles for {symbol}: {len(candles)}")
        root_symbol = wave.root_symbol(symbol)
        context_frames = {}
        for timeframe in scanner.requested_context_timeframes(entry_args):
            context_frames[timeframe] = scanner.fetch_context_candles(conn, symbol, timeframe, start, end, entry_args)

    print("Live candle-wave prototype")
    print(f"  symbol={symbol} root={root_symbol} timeframe={entry_args.timeframe} candles={len(candles):,}")
    print(f"  entry_model={args.entry_run_id} threshold={fmt(entry_args.threshold)}")
    print(f"  exit_model={args.exit_run_id} threshold={fmt(exit_args.exit_threshold)} dynamic_max_bars={exit_args.dynamic_max_bars}")
    print(f"  risk_mode={args.risk_mode} source_exit_mode={args.source_exit_mode}")

    stats, events = run_replay(
        candles,
        context_frames,
        symbol,
        root_symbol,
        entry_regressor,
        exit_regressor,
        entry_args,
        exit_args,
        args,
    )

    output = Path(args.output) if args.output else wave.ABCD_ROOT / "logs" / f"live_proto_{symbol}_{entry_args.timeframe}.csv"
    write_events(output, events)

    avg_loop = float(np.mean(stats.loop_ms)) if stats.loop_ms else 0.0
    max_loop = float(np.max(stats.loop_ms)) if stats.loop_ms else 0.0
    projected_206 = avg_loop * 206.0 / 1000.0
    win_rate = stats.wins / stats.exits if stats.exits else 0.0
    print("Summary")
    print(f"  candles_seen={stats.candles_seen:,} candidates={stats.entry_candidates:,} entries={stats.approved_entries:,} exits={stats.exits:,}")
    print(f"  win_rate={win_rate * 100:.2f}% sum_r={stats.sum_r:.2f}R avg_r={(stats.sum_r / stats.exits) if stats.exits else 0.0:.4f}R")
    print(f"  loop_avg={avg_loop:.3f}ms loop_max={max_loop:.3f}ms projected_206_symbol_cycle={projected_206:.3f}s")
    print(f"  log={output}")
    if events and args.print_events > 0:
        print("Recent events")
        for event in events[-args.print_events :]:
            print(
                "  "
                f"{event.get('ts_utc')} {event.get('event')} {event.get('trade_id') or '-'} "
                f"{event.get('direction') or '-'} {event.get('reason') or '-'} "
                f"price={fmt(event.get('price'), 5)} score={fmt(event.get('predicted_r'), 4)} "
                f"exit_score={fmt(event.get('exit_score_r'), 4)} r={fmt(event.get('result_r'), 3)}"
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
