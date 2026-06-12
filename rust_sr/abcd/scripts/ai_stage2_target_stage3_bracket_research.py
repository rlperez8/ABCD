#!/usr/bin/env python3
"""
Research Stage 2 target forecasts plus Stage 3 bracket exits.

Flow:
    Stage 2 confirms a trend-start event.
    Stage 2.5 predicts how much favorable movement is likely from the live
    entry snapshot.
    Stage 3 enters next candle open with:
        - fixed hard stop from swing/ATR logic,
        - predicted take-profit target,
        - live-only early fade exit.

This script is source-free and live-clean for the tested mechanics: it does not
use old harmonic exits, old source-exit fallback, or any future field as a
feature. Future candles are only used to label training targets and score the
validation simulation.
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
except ImportError as exc:  # pragma: no cover
    raise SystemExit("CatBoost is required. Run with .venv_ai\\Scripts\\python.exe") from exc


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_scanner_model as scanner
import ai_oracle_start_stage2_l2_confirmation as stage2_l2
import ai_oracle_trend_start_model as start_model
import ai_wave_rider_research as wave


DEFAULT_STAGE2_RUN = "aicw-os-stage2-l2-confirm-v1-2m-m-tr2025-v2026-m16-v2026"

ROOT_GROUPS = {
    "all": "CL,EMD,ES,GF,HO,LE,NG,NKD,NQ,RB,RTY,YM",
    "quality_v2": "CL,EMD,HO,NG,RB",
    "quality_v2_nq": "CL,EMD,HO,NG,NQ,RB",
}

BASE_RULES = [
    {"base_rule": "quality_v2_lowdd", "score_min": 0.910, "risk_min": 72.0, "risk_max": 144.0},
    {"base_rule": "clean_0930_r48_192", "score_min": 0.930, "risk_min": 48.0, "risk_max": 192.0},
    {"base_rule": "push_0915_r60_240", "score_min": 0.915, "risk_min": 60.0, "risk_max": 240.0},
]

TARGET_FACTORS = [0.60, 0.75]
PREDICTED_MFE_MINIMUMS = [0.75, 1.00, 1.25]
MIN_TARGETS_R = [0.75, 1.00]
MAX_TARGETS_R = [1.50, 2.00, 3.00]

EXTRA_CAT_FEATURES = [
    "source_timeframe",
    "entry_weekday",
    "entry_month",
    "entry_hour_bucket",
    "risk_bucket",
    "score_bucket",
    "top_l1_block",
    "l1_leader",
    "l1_agreement_bucket",
    "confirm_offset_bucket",
]

EXTRA_NUM_FEATURES = [
    "entry_hour",
    "entry_month_num",
    "stage1_score",
    "level2_score",
    "stage2_score",
    "cat_l1_score",
    "light_l1_score",
    "xgb_l1_score",
    "l1_mean",
    "l1_std",
    "l1_spread",
    "l1_min",
    "l1_max",
    "confirm_offset_bars",
    "confirm_offset_abs",
    "risk_ticks",
    "slippage_r",
    "score_x_risk",
    "score_to_slippage",
    "risk_log",
    "is_rth",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage2-run-id", default=DEFAULT_STAGE2_RUN)
    parser.add_argument("--run-id", default="aicw-stage2-target-stage3-bracket-v1-2m-tr2025-v2026")
    parser.add_argument("--replace", action="store_true")
    parser.add_argument("--timeframe", default="2m", choices=sorted(scanner.TIMEFRAME_TABLES))
    parser.add_argument("--train-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--valid-split", default="")
    parser.add_argument("--max-train-events", type=int, default=0)
    parser.add_argument("--max-threshold-events", type=int, default=0)
    parser.add_argument("--max-valid-events", type=int, default=0)
    parser.add_argument("--min-threshold-trades", type=int, default=80)
    parser.add_argument("--top-rules", type=int, default=24)
    parser.add_argument("--target-clip-high", type=float, default=5.0)
    parser.add_argument("--target-step-r", type=float, default=0.25)
    parser.add_argument("--iterations", type=int, default=450)
    parser.add_argument("--depth", type=int, default=5)
    parser.add_argument("--learning-rate", type=float, default=0.04)
    parser.add_argument("--l2-leaf-reg", type=float, default=12.0)
    parser.add_argument("--random-seed", type=int, default=73)

    # Entry/risk defaults matching the current 2m candle-wave work.
    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=240.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    parser.add_argument("--max-forward-bars", type=int, default=180)
    parser.add_argument("--time-exit-bars", type=int, default=0)
    parser.add_argument("--min-hold-bars", type=int, default=2)
    parser.add_argument("--death-bars", type=int, default=3)
    parser.add_argument("--fade-min-mfe-r", type=float, default=0.75)
    parser.add_argument("--fade-giveback-r", type=float, default=0.80)
    parser.add_argument("--slippage-entry-ticks", type=float, default=3.0)
    parser.add_argument("--slippage-exit-ticks", type=float, default=3.0)
    return parser.parse_args()


def to_jsonable(value: Any) -> Any:
    if isinstance(value, pd.Timestamp):
        return None if pd.isna(value) else value.isoformat()
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def finite(value: Any, default: float = 0.0) -> float:
    try:
        numeric = float(value)
    except (TypeError, ValueError):
        return default
    return numeric if math.isfinite(numeric) else default


def stage2_dir(run_id_value: str) -> Path:
    path = Path(run_id_value)
    if path.exists():
        return path.resolve()
    return wave.ABCD_ROOT / "model_registry" / run_id_value


def score_bucket(value: float) -> str:
    if value >= 0.94:
        return "094+"
    if value >= 0.93:
        return "093-094"
    if value >= 0.92:
        return "092-093"
    if value >= 0.91:
        return "091-092"
    if value >= 0.90:
        return "090-091"
    return "lt090"


def risk_bucket(value: float) -> str:
    if value < 48:
        return "lt48"
    if value < 72:
        return "048-072"
    if value < 96:
        return "072-096"
    if value < 144:
        return "096-144"
    if value < 192:
        return "144-192"
    return "192+"


def hour_bucket(hour: int) -> str:
    if 0 <= hour <= 5:
        return "overnight"
    if 6 <= hour <= 8:
        return "pre_rth"
    if 9 <= hour <= 11:
        return "morning"
    if 12 <= hour <= 14:
        return "midday"
    if 15 <= hour <= 17:
        return "afternoon"
    return "evening"


def confirm_offset_bucket(value: float) -> str:
    if not math.isfinite(value):
        return "missing"
    if value <= 1:
        return "01"
    if value <= 2:
        return "02"
    if value <= 4:
        return "03-04"
    if value <= 8:
        return "05-08"
    if value <= 16:
        return "09-16"
    return "17+"


def l1_agreement_bucket(spread: float) -> str:
    if not math.isfinite(spread):
        return "missing"
    if spread <= 0.03:
        return "tight"
    if spread <= 0.07:
        return "medium"
    return "wide"


def feature_columns() -> tuple[list[str], list[str]]:
    cat = list(dict.fromkeys(list(start_model.CAT_FEATURES) + EXTRA_CAT_FEATURES))
    num = list(dict.fromkeys(list(start_model.NUM_FEATURES) + EXTRA_NUM_FEATURES))
    return cat, num


def prepare_pool(frame: pd.DataFrame, include_target: bool) -> Pool:
    cat_features, num_features = feature_columns()
    work = frame.copy()
    for column in cat_features:
        if column not in work.columns:
            work[column] = "missing"
        work[column] = work[column].fillna("missing").astype(str)
    for column in num_features:
        if column not in work.columns:
            work[column] = 0.0
        work[column] = pd.to_numeric(work[column], errors="coerce").replace([np.inf, -np.inf], np.nan).fillna(0.0)
    columns = cat_features + num_features
    if include_target:
        return Pool(work[columns], label=work["target_mfe_r"], cat_features=cat_features)
    return Pool(work[columns], cat_features=cat_features)


def load_stage2_events(model_dir: Path, split: str, limit: int, seed: int) -> pd.DataFrame:
    path = model_dir / f"stage2_l2_events_{split}.csv"
    if not path.exists():
        raise FileNotFoundError(f"Missing Stage 2 events: {path}")
    frame = pd.read_csv(path)
    frame = frame[pd.to_numeric(frame.get("confirmed"), errors="coerce").fillna(0).astype(int) == 1].copy()
    frame["direction"] = frame["direction"].astype(str).str.upper()
    for column in ["signal_date", "confirm_date"]:
        frame[column] = pd.to_datetime(frame[column], errors="coerce")
    frame = frame.dropna(subset=["candidate_uid", "root_symbol", "symbol", "direction", "confirm_date"])
    frame = frame[frame["direction"].isin(["LONG", "SHORT"])].copy()
    if int(limit) > 0 and len(frame) > int(limit):
        frame = frame.sample(n=int(limit), random_state=int(seed)).sort_values(["confirm_date", "candidate_uid"])
    return frame.reset_index(drop=True)


def fetch_candles_for_symbols(conn, timeframe: str, year: int, symbols: list[str]) -> pd.DataFrame:
    return stage2_l2.fetch_candles_for_symbols(conn, timeframe, int(year), symbols)


def add_event_features(row: dict[str, Any]) -> dict[str, Any]:
    out: dict[str, Any] = {}
    for column in ["stage1_score", "level2_score", "stage2_score", "cat_l1_score", "light_l1_score", "xgb_l1_score"]:
        out[column] = finite(row.get(column), 0.0)
    scores = [out["cat_l1_score"], out["light_l1_score"], out["xgb_l1_score"]]
    out["l1_mean"] = float(np.mean(scores))
    out["l1_std"] = float(np.std(scores))
    out["l1_min"] = float(np.min(scores))
    out["l1_max"] = float(np.max(scores))
    out["l1_spread"] = out["l1_max"] - out["l1_min"]
    labels = ["catboost", "lightgbm", "xgboost"]
    out["l1_leader"] = labels[int(np.argmax(scores))]
    out["l1_agreement_bucket"] = l1_agreement_bucket(out["l1_spread"])
    offset = finite(row.get("confirm_offset_bars"), float("nan"))
    out["confirm_offset_bars"] = offset if math.isfinite(offset) else 0.0
    out["confirm_offset_abs"] = abs(out["confirm_offset_bars"])
    out["confirm_offset_bucket"] = confirm_offset_bucket(out["confirm_offset_abs"])
    out["top_l1_block"] = str(row.get("top_l1_block") or "missing")
    return out


def target_price(entry_price: float, risk_points: float, target_r: float, direction: str) -> float:
    sign = wave.direction_sign(direction)
    return float(entry_price + sign * float(target_r) * float(risk_points))


def round_target(value: float, step: float) -> float:
    if step <= 0:
        return float(value)
    return float(max(step, math.floor(float(value) / step) * step))


def terminal_path_metrics(
    candles: pd.DataFrame,
    entry_idx: int,
    direction: str,
    entry_price: float,
    stop_price: float,
    risk_points: float,
    tick_size: float,
    args: argparse.Namespace,
) -> dict[str, Any]:
    max_idx = min(len(candles) - 1, int(entry_idx) + max(2, int(args.max_forward_bars)))
    sign = wave.direction_sign(direction)
    mfe_r = 0.0
    mae_r = 0.0
    hard_stop_idx = None
    for candle_idx in range(int(entry_idx), max_idx + 1):
        candle = candles.iloc[candle_idx]
        high = float(candle["high"])
        low = float(candle["low"])
        if direction == "LONG":
            mfe_r = max(mfe_r, (high - entry_price) / risk_points)
            mae_r = max(mae_r, (entry_price - low) / risk_points)
            if low <= stop_price:
                hard_stop_idx = candle_idx
                break
        else:
            mfe_r = max(mfe_r, (entry_price - low) / risk_points)
            mae_r = max(mae_r, (high - entry_price) / risk_points)
            if high >= stop_price:
                hard_stop_idx = candle_idx
                break

    if hard_stop_idx is not None:
        exit_idx = int(hard_stop_idx)
        exit_price = float(stop_price)
        reason = "dynamic_hard_stop"
    else:
        exit_idx = int(max_idx)
        exit_price = float(candles["close"].iloc[exit_idx])
        reason = "dynamic_time_exit"
    slippage_r = ((float(args.slippage_entry_ticks) + float(args.slippage_exit_ticks)) * tick_size) / risk_points
    raw_result_r = sign * (exit_price - entry_price) / risk_points
    result_r = raw_result_r - slippage_r
    return {
        "target_mfe_r": float(max(0.0, min(float(args.target_clip_high), mfe_r))),
        "path_mfe_r": float(mfe_r),
        "path_mae_r": float(mae_r),
        "terminal_exit_idx": int(exit_idx),
        "terminal_exit_date": candles["ts_utc"].iloc[int(exit_idx)],
        "terminal_exit_price": float(exit_price),
        "terminal_exit_reason": reason,
        "terminal_result_r": float(result_r),
        "terminal_raw_result_r": float(raw_result_r),
        "slippage_r": float(slippage_r),
    }


def simulate_bracket_exit(
    row: pd.Series,
    candles: pd.DataFrame,
    args: argparse.Namespace,
) -> dict[str, Any] | None:
    entry_idx = int(row["entry_index"])
    if entry_idx >= len(candles):
        return None
    direction = str(row["direction"]).upper()
    entry_price = float(row["entry_price"])
    stop_price = float(row["stop_price"])
    risk_points = float(row["risk_points"])
    tick_size = float(row["tick_size"])
    target_r = float(row["planned_target_r"])
    target = target_price(entry_price, risk_points, target_r, direction)
    sign = wave.direction_sign(direction)
    max_idx = min(len(candles) - 1, entry_idx + max(2, int(args.max_forward_bars)))
    max_favorable = 0.0
    max_adverse = 0.0
    death_count = 0
    exit_idx = max_idx
    exit_price = float(candles["close"].iloc[max_idx])
    exit_reason = "bracket_time_exit"

    for candle_idx in range(entry_idx, max_idx + 1):
        candle = candles.iloc[candle_idx]
        high = float(candle["high"])
        low = float(candle["low"])
        close = float(candle["close"])
        if direction == "LONG":
            max_favorable = max(max_favorable, (high - entry_price) / risk_points)
            max_adverse = max(max_adverse, (entry_price - low) / risk_points)
            stop_hit = low <= stop_price
            target_hit = high >= target
            current_r = (close - entry_price) / risk_points
        else:
            max_favorable = max(max_favorable, (entry_price - low) / risk_points)
            max_adverse = max(max_adverse, (high - entry_price) / risk_points)
            stop_hit = high >= stop_price
            target_hit = low <= target
            current_r = (entry_price - close) / risk_points

        if stop_hit and target_hit:
            exit_idx = candle_idx
            exit_price = stop_price
            exit_reason = "bracket_ambiguous_stop_first"
            break
        if stop_hit:
            exit_idx = candle_idx
            exit_price = stop_price
            exit_reason = "bracket_hard_stop"
            break
        if target_hit:
            exit_idx = candle_idx
            exit_price = target
            exit_reason = "bracket_take_profit"
            break

        if candle_idx - entry_idx >= int(args.min_hold_bars):
            if wave.momentum_dead(candles, candle_idx, direction):
                death_count += 1
            else:
                death_count = 0
            gave_back = (
                max_favorable >= float(args.fade_min_mfe_r)
                and current_r <= max_favorable - float(args.fade_giveback_r)
            )
            if death_count >= int(args.death_bars) or gave_back:
                next_idx = min(candle_idx + 1, max_idx)
                exit_idx = int(next_idx)
                exit_price = float(candles["open"].iloc[next_idx])
                exit_reason = "bracket_momentum_fade" if death_count >= int(args.death_bars) else "bracket_giveback_fade"
                break

    raw_result_r = sign * (float(exit_price) - float(entry_price)) / risk_points
    slippage_r = ((float(args.slippage_entry_ticks) + float(args.slippage_exit_ticks)) * tick_size) / risk_points
    result_r = raw_result_r - slippage_r
    return {
        "target_price": float(target),
        "bracket_exit_idx": int(exit_idx),
        "bracket_exit_date": candles["ts_utc"].iloc[int(exit_idx)],
        "bracket_exit_price": float(exit_price),
        "bracket_exit_reason": exit_reason,
        "bracket_result_r": float(result_r),
        "bracket_raw_result_r": float(raw_result_r),
        "bracket_mfe_r": float(max_favorable),
        "bracket_mae_r": float(max_adverse),
    }


def build_entry_rows(conn, events: pd.DataFrame, year: int, args: argparse.Namespace, label: str) -> pd.DataFrame:
    if events.empty:
        return pd.DataFrame()
    candles = fetch_candles_for_symbols(conn, args.timeframe, int(year), events["symbol"].astype(str).unique().tolist())
    if candles.empty:
        raise ValueError(f"{label}: no candles available")
    event_groups = {
        str(symbol): group.sort_values("confirm_date").reset_index(drop=True)
        for symbol, group in events.groupby("symbol", sort=False)
    }
    rows: list[dict[str, Any]] = []
    for symbol_index, (symbol, raw_group) in enumerate(candles.groupby("symbol", sort=True), start=1):
        symbol_events = event_groups.get(str(symbol))
        if symbol_events is None or symbol_events.empty:
            continue
        group = scanner.enrich_candles(raw_group[["ts_utc", "open", "high", "low", "close", "volume"]].copy(), args)
        times = pd.to_datetime(group["ts_utc"], errors="coerce").to_numpy(dtype="datetime64[ns]")
        root = str(raw_group["root_symbol"].iloc[0]).upper()
        for event in symbol_events.to_dict("records"):
            confirm_date = pd.Timestamp(event["confirm_date"])
            confirm_idx = int(np.searchsorted(times, np.datetime64(confirm_date), side="left"))
            entry_idx = confirm_idx + 1
            if entry_idx >= len(group):
                continue
            direction = str(event["direction"]).upper()
            tick_size = wave.tick_size_for(str(symbol), root)
            stop_price, risk_points = wave.initial_stop(group, entry_idx, direction, tick_size, args)
            entry_price = wave.finite(group["open"].iloc[entry_idx], None)
            if stop_price is None or risk_points is None or entry_price is None:
                continue
            risk_ticks = float(risk_points) / float(tick_size) if tick_size > 0 else 0.0
            if risk_ticks < float(args.min_risk_ticks) or risk_ticks > float(args.max_risk_ticks):
                continue
            features = start_model.feature_row(int(year), root, str(symbol), group, int(entry_idx), direction)
            event_features = add_event_features(event)
            entry_date = pd.Timestamp(group["ts_utc"].iloc[entry_idx])
            path = terminal_path_metrics(
                group,
                int(entry_idx),
                direction,
                float(entry_price),
                float(stop_price),
                float(risk_points),
                float(tick_size),
                args,
            )
            row = {
                **features,
                **event_features,
                **path,
                "candidate_uid": str(event.get("candidate_uid") or ""),
                "stage2_candidate_uid": str(event.get("candidate_uid") or ""),
                "valid_year": int(year),
                "source_timeframe": str(args.timeframe),
                "root_symbol": root,
                "symbol": str(symbol).upper(),
                "direction": direction,
                "signal_date": event.get("signal_date"),
                "confirm_date": confirm_date,
                "entry_date": entry_date,
                "entry_index": int(entry_idx),
                "entry_price": float(entry_price),
                "stop_price": float(stop_price),
                "risk_points": float(risk_points),
                "risk_ticks": float(risk_ticks),
                "tick_size": float(tick_size),
                "entry_hour": int(entry_date.hour),
                "entry_month_num": int(entry_date.month),
                "entry_weekday": entry_date.day_name(),
                "entry_month": entry_date.month_name(),
                "entry_hour_bucket": hour_bucket(int(entry_date.hour)),
                "risk_bucket": risk_bucket(float(risk_ticks)),
                "score_bucket": score_bucket(float(event_features["stage2_score"])),
                "score_x_risk": float(event_features["stage2_score"]) * float(risk_ticks),
                "score_to_slippage": float(event_features["stage2_score"]) / max(float(path["slippage_r"]), 1e-9),
                "risk_log": float(np.log1p(max(0.0, risk_ticks))),
                "is_rth": 1.0 if 8 <= int(entry_date.hour) <= 15 else 0.0,
            }
            rows.append(row)
        if symbol_index % 50 == 0:
            print(f"{label}: symbols={symbol_index:,} rows={len(rows):,}/{len(events):,}", flush=True)
    frame = pd.DataFrame(rows).sort_values("entry_date").reset_index(drop=True)
    print(f"{label}: built {len(frame):,} entry rows from {len(events):,} Stage 2 confirmations", flush=True)
    return frame


def train_target_model(train: pd.DataFrame, args: argparse.Namespace) -> CatBoostRegressor:
    model = CatBoostRegressor(
        loss_function="RMSE",
        iterations=int(args.iterations),
        depth=int(args.depth),
        learning_rate=float(args.learning_rate),
        l2_leaf_reg=float(args.l2_leaf_reg),
        random_seed=int(args.random_seed),
        verbose=100,
        allow_writing_files=False,
    )
    model.fit(prepare_pool(train, include_target=True))
    return model


def add_target_predictions(frame: pd.DataFrame, model: CatBoostRegressor) -> pd.DataFrame:
    scored = frame.copy()
    scored["predicted_mfe_r"] = np.maximum(0.0, model.predict(prepare_pool(scored, include_target=False)))
    return scored


def max_drawdown(values: list[float]) -> float:
    equity = 0.0
    peak = 0.0
    max_dd = 0.0
    for value in values:
        equity += float(value)
        peak = max(peak, equity)
        max_dd = max(max_dd, peak - equity)
    return float(max_dd)


def max_concurrent(frame: pd.DataFrame, entry_col: str, exit_col: str) -> int:
    events: list[tuple[pd.Timestamp, int]] = []
    for row in frame[[entry_col, exit_col]].itertuples(index=False):
        events.append((pd.Timestamp(row[0]), 1))
        events.append((pd.Timestamp(row[1]), -1))
    events.sort(key=lambda item: (item[0], item[1]))
    current = 0
    max_seen = 0
    for _, delta in events:
        current += delta
        max_seen = max(max_seen, current)
    return int(max_seen)


def summarize(frame: pd.DataFrame, result_col: str, exit_col: str) -> dict[str, Any]:
    if frame.empty:
        return {
            "trades": 0,
            "wins": 0,
            "losses": 0,
            "win_rate": 0.0,
            "sum_r": 0.0,
            "avg_r": 0.0,
            "dd_r": 0.0,
            "score_to_dd_r": 0.0,
            "max_concurrent": 0,
        }
    ordered = frame.sort_values(exit_col)
    values = [float(value) for value in ordered[result_col]]
    wins = sum(1 for value in values if value > 0)
    dd = max_drawdown(values)
    total = float(sum(values))
    return {
        "trades": int(len(values)),
        "wins": int(wins),
        "losses": int(len(values) - wins),
        "win_rate": float(wins / len(values)) if values else 0.0,
        "sum_r": total,
        "avg_r": float(np.mean(values)) if values else 0.0,
        "dd_r": dd,
        "score_to_dd_r": float(total / dd) if dd > 0 else (999.0 if total > 0 else 0.0),
        "max_concurrent": max_concurrent(frame, "entry_date", exit_col),
    }


def apply_base(frame: pd.DataFrame, roots: set[str], base_rule: dict[str, Any], pred_min: float) -> pd.DataFrame:
    return frame[
        frame["root_symbol"].isin(roots)
        & (pd.to_numeric(frame["stage2_score"], errors="coerce") >= float(base_rule["score_min"]))
        & (pd.to_numeric(frame["risk_ticks"], errors="coerce") >= float(base_rule["risk_min"]))
        & (pd.to_numeric(frame["risk_ticks"], errors="coerce") <= float(base_rule["risk_max"]))
        & (pd.to_numeric(frame["predicted_mfe_r"], errors="coerce") >= float(pred_min))
    ].copy()


def evaluate_rule(
    frame: pd.DataFrame,
    candles_by_symbol: dict[str, pd.DataFrame],
    roots: set[str],
    base_rule: dict[str, Any],
    pred_min: float,
    target_factor: float,
    min_target_r: float,
    max_target_r: float,
    args: argparse.Namespace,
) -> tuple[pd.DataFrame, dict[str, Any]]:
    selected = apply_base(frame, roots, base_rule, pred_min)
    if selected.empty:
        return selected, summarize(selected, "bracket_result_r", "bracket_exit_date")
    rows = []
    for _, row in selected.iterrows():
        planned = min(float(max_target_r), max(float(min_target_r), float(row["predicted_mfe_r"]) * float(target_factor)))
        planned = round_target(planned, float(args.target_step_r))
        if planned <= 0:
            continue
        working = row.copy()
        working["planned_target_r"] = planned
        group = candles_by_symbol.get(str(row["symbol"]))
        if group is None:
            continue
        bracket = simulate_bracket_exit(working, group, args)
        if bracket is None:
            continue
        payload = working.to_dict()
        payload.update(bracket)
        rows.append(payload)
    out = pd.DataFrame(rows)
    return out, summarize(out, "bracket_result_r", "bracket_exit_date")


def candles_by_symbol_for_rows(conn, rows: pd.DataFrame, year: int, args: argparse.Namespace) -> dict[str, pd.DataFrame]:
    candles = fetch_candles_for_symbols(conn, args.timeframe, int(year), rows["symbol"].dropna().astype(str).unique().tolist())
    output = {}
    for symbol, raw_group in candles.groupby("symbol", sort=False):
        output[str(symbol)] = scanner.enrich_candles(raw_group[["ts_utc", "open", "high", "low", "close", "volume"]].copy(), args)
    return output


def sweep_rules(
    frame: pd.DataFrame,
    candles_by_symbol: dict[str, pd.DataFrame],
    args: argparse.Namespace,
    label: str,
) -> pd.DataFrame:
    rows = []
    for group_name, roots_csv in ROOT_GROUPS.items():
        roots = {part.strip().upper() for part in roots_csv.split(",") if part.strip()}
        for base_rule in BASE_RULES:
            for pred_min in PREDICTED_MFE_MINIMUMS:
                for target_factor in TARGET_FACTORS:
                    for min_target_r in MIN_TARGETS_R:
                        for max_target_r in MAX_TARGETS_R:
                            if max_target_r < min_target_r:
                                continue
                            selected, stats = evaluate_rule(
                                frame,
                                candles_by_symbol,
                                roots,
                                base_rule,
                                pred_min,
                                target_factor,
                                min_target_r,
                                max_target_r,
                                args,
                            )
                            if stats["trades"] < int(args.min_threshold_trades):
                                continue
                            terminal_stats = summarize(selected, "terminal_result_r", "terminal_exit_date")
                            rows.append(
                                {
                                    "label": label,
                                    "root_group": group_name,
                                    "roots": ",".join(sorted(roots)),
                                    **base_rule,
                                    "predicted_mfe_min": float(pred_min),
                                    "target_factor": float(target_factor),
                                    "min_target_r": float(min_target_r),
                                    "max_target_r": float(max_target_r),
                                    **{f"bracket_{key}": value for key, value in stats.items()},
                                    **{f"terminal_{key}": value for key, value in terminal_stats.items()},
                                    "delta_sum_r": float(stats["sum_r"] - terminal_stats["sum_r"]),
                                    "delta_dd_r": float(stats["dd_r"] - terminal_stats["dd_r"]),
                                }
                            )
    result = pd.DataFrame(rows)
    if result.empty:
        return result
    result = result.sort_values(
        ["bracket_score_to_dd_r", "bracket_sum_r", "bracket_avg_r", "bracket_trades"],
        ascending=[False, False, False, False],
        kind="mergesort",
    ).reset_index(drop=True)
    result["rule_rank"] = result.index + 1
    return result


def apply_top_rules(
    valid_frame: pd.DataFrame,
    valid_candles: dict[str, pd.DataFrame],
    threshold_rules: pd.DataFrame,
    args: argparse.Namespace,
) -> tuple[pd.DataFrame, pd.DataFrame]:
    summaries = []
    best_trades = pd.DataFrame()
    for rule in threshold_rules.head(int(args.top_rules)).itertuples(index=False):
        roots = {part.strip().upper() for part in str(rule.roots).split(",") if part.strip()}
        base_rule = {
            "base_rule": rule.base_rule,
            "score_min": float(rule.score_min),
            "risk_min": float(rule.risk_min),
            "risk_max": float(rule.risk_max),
        }
        trades, stats = evaluate_rule(
            valid_frame,
            valid_candles,
            roots,
            base_rule,
            float(rule.predicted_mfe_min),
            float(rule.target_factor),
            float(rule.min_target_r),
            float(rule.max_target_r),
            args,
        )
        terminal_stats = summarize(trades, "terminal_result_r", "terminal_exit_date")
        summary = {
            "threshold_rule_rank": int(rule.rule_rank),
            "root_group": rule.root_group,
            "roots": rule.roots,
            **base_rule,
            "predicted_mfe_min": float(rule.predicted_mfe_min),
            "target_factor": float(rule.target_factor),
            "min_target_r": float(rule.min_target_r),
            "max_target_r": float(rule.max_target_r),
            **{f"bracket_{key}": value for key, value in stats.items()},
            **{f"terminal_{key}": value for key, value in terminal_stats.items()},
            "delta_sum_r": float(stats["sum_r"] - terminal_stats["sum_r"]),
            "delta_dd_r": float(stats["dd_r"] - terminal_stats["dd_r"]),
        }
        summaries.append(summary)
        if best_trades.empty or (
            stats["score_to_dd_r"],
            stats["sum_r"],
            stats["avg_r"],
        ) > (
            summarize(best_trades, "bracket_result_r", "bracket_exit_date")["score_to_dd_r"] if not best_trades.empty else -999,
            summarize(best_trades, "bracket_result_r", "bracket_exit_date")["sum_r"] if not best_trades.empty else -999,
            summarize(best_trades, "bracket_result_r", "bracket_exit_date")["avg_r"] if not best_trades.empty else -999,
        ):
            best_trades = trades.copy()
    results = pd.DataFrame(summaries)
    if not results.empty:
        results = results.sort_values(
            ["bracket_score_to_dd_r", "bracket_sum_r", "bracket_avg_r", "bracket_trades"],
            ascending=[False, False, False, False],
            kind="mergesort",
        ).reset_index(drop=True)
        results["valid_rule_rank"] = results.index + 1
    return results, best_trades


def main() -> int:
    args = parse_args()
    scanner.configure_wave_args(args)
    output_dir = wave.ABCD_ROOT / "model_registry" / str(args.run_id)
    if output_dir.exists() and any(output_dir.iterdir()) and not args.replace:
        raise SystemExit(f"Output exists: {output_dir}. Use --replace.")
    output_dir.mkdir(parents=True, exist_ok=True)

    model_dir = stage2_dir(str(args.stage2_run_id))
    valid_split = str(args.valid_split or args.valid_year)
    train_events = load_stage2_events(model_dir, "train", int(args.max_train_events), int(args.random_seed))
    threshold_events = load_stage2_events(model_dir, "threshold", int(args.max_threshold_events), int(args.random_seed) + 1)
    valid_events = load_stage2_events(model_dir, valid_split, int(args.max_valid_events), int(args.random_seed) + 2)
    print(
        f"Loaded confirmed Stage 2 events train={len(train_events):,} "
        f"threshold={len(threshold_events):,} valid={len(valid_events):,}",
        flush=True,
    )

    conn = wave.connect()
    try:
        train = build_entry_rows(conn, train_events, int(args.train_year), args, "train")
        threshold = build_entry_rows(conn, threshold_events, int(args.train_year), args, "threshold")
        valid = build_entry_rows(conn, valid_events, int(args.valid_year), args, "valid")

        model = train_target_model(train, args)
        model.save_model(str(output_dir / "catboost_stage2_target_mfe_regressor.cbm"))
        train_scored = add_target_predictions(train, model)
        threshold_scored = add_target_predictions(threshold, model)
        valid_scored = add_target_predictions(valid, model)

        threshold_candles = candles_by_symbol_for_rows(conn, threshold_scored, int(args.train_year), args)
        valid_candles = candles_by_symbol_for_rows(conn, valid_scored, int(args.valid_year), args)
        threshold_rules = sweep_rules(threshold_scored, threshold_candles, args, "threshold")
        threshold_rules.to_csv(output_dir / "bracket_threshold_rule_sweep.csv", index=False)
        valid_results, best_valid_trades = apply_top_rules(valid_scored, valid_candles, threshold_rules, args)
        valid_results.to_csv(output_dir / "bracket_valid_results_from_threshold_rules.csv", index=False)
        best_valid_trades.to_csv(output_dir / "bracket_best_valid_trades.csv", index=False)

        train_scored.to_csv(output_dir / "target_scored_train_entries.csv", index=False)
        threshold_scored.to_csv(output_dir / "target_scored_threshold_entries.csv", index=False)
        valid_scored.to_csv(output_dir / "target_scored_valid_entries.csv", index=False)

        top_threshold = threshold_rules.head(10).to_dict("records") if not threshold_rules.empty else []
        top_valid = valid_results.head(10).to_dict("records") if not valid_results.empty else []
        metadata = {
            "run_id": str(args.run_id),
            "model_type": "stage2_target_forecast_stage3_bracket_research",
            "stage2_run_id": str(args.stage2_run_id),
            "train_year": int(args.train_year),
            "valid_year": int(args.valid_year),
            "timeframe": str(args.timeframe),
            "live_compatible_design": True,
            "entry_rule": "Stage 2 confirmed event, enter next candle open",
            "stop_rule": "fixed initial swing/ATR hard stop",
            "target_rule": "CatBoost predicted MFE converted into fixed bracket take-profit",
            "early_exit_rule": "live-only momentum fade or giveback fade after minimum hold",
            "slippage_ticks": {
                "entry": float(args.slippage_entry_ticks),
                "exit": float(args.slippage_exit_ticks),
            },
            "target_grid": {
                "predicted_mfe_minimums": PREDICTED_MFE_MINIMUMS,
                "target_factors": TARGET_FACTORS,
                "min_targets_r": MIN_TARGETS_R,
                "max_targets_r": MAX_TARGETS_R,
            },
            "rows": {
                "train": int(len(train_scored)),
                "threshold": int(len(threshold_scored)),
                "valid": int(len(valid_scored)),
            },
            "top_threshold_rules": top_threshold,
            "top_valid_results": top_valid,
        }
        (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")
        print(json.dumps(metadata, indent=2, default=to_jsonable), flush=True)
    finally:
        conn.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
