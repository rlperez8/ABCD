#!/usr/bin/env python3
"""
Train a Stage 3 candle-by-candle hold/exit policy.

This is the "no fixed strategy stop, no fixed take-profit" version:

    Stage 2 confirms a trend-start event.
    Stage 3 enters on the next candle open.
    Every closed candle after entry becomes a policy decision:
        HOLD for another candle, or EXIT on the next open.

The initial swing/ATR distance is still used as the R unit for sizing/scoring,
but it is not treated as a strategy stop. A separate disaster stop can be
enabled for live safety research, but defaults off in this experiment.
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
    from catboost import CatBoostClassifier, Pool
except ImportError as exc:  # pragma: no cover
    raise SystemExit("CatBoost is required. Run with .venv_ai\\Scripts\\python.exe") from exc


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_scanner_model as scanner
import ai_oracle_start_stage2_l2_confirmation as stage2_l2
import ai_oracle_trend_start_model as start_model
import ai_wave_rider_research as wave


DEFAULT_ENTRY_SOURCE = "aicw-stage2-target-stage3-bracket-v1-2m-tr2025-v2026"

CAT_FEATURES = [
    "root_symbol",
    "symbol",
    "direction",
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

NUM_FEATURES = [
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
    "predicted_mfe_r",
    "entry_hour",
    "is_rth",
    "hold_bars",
    "hold_ratio",
    "current_r",
    "exit_now_r",
    "max_favorable_r",
    "max_adverse_r",
    "giveback_r",
    "last_bar_r",
    "last_range_r",
    "last_body_r",
    "last_align_r",
    "ret_3_r",
    "ret_6_r",
    "ema_fast_dist_r",
    "ema_slow_dist_r",
    "ema_gap_r",
    "ema_fast_slope_r",
    "atr_r",
    "relative_volume_now",
    "compression_now",
    "momentum_dead_now",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--entry-source-run", default=DEFAULT_ENTRY_SOURCE)
    parser.add_argument("--run-id", default="aicw-stage3-candle-policy-v1-2m-tr2025-v2026")
    parser.add_argument("--replace", action="store_true")
    parser.add_argument("--timeframe", default="2m", choices=sorted(scanner.TIMEFRAME_TABLES))
    parser.add_argument("--train-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--roots", default="CL,EMD,HO,NG,RB")
    parser.add_argument("--score-min", type=float, default=0.910)
    parser.add_argument("--risk-min", type=float, default=72.0)
    parser.add_argument("--risk-max", type=float, default=144.0)
    parser.add_argument("--predicted-mfe-min", type=float, default=0.0)
    parser.add_argument("--max-forward-bars", type=int, default=180)
    parser.add_argument("--min-hold-bars", type=int, default=2)
    parser.add_argument("--decision-step-bars", type=int, default=1)
    parser.add_argument("--exit-label-margin-r", type=float, default=0.10)
    parser.add_argument("--disaster-stop-r", type=float, default=0.0)
    parser.add_argument("--min-threshold-trades", type=int, default=80)
    parser.add_argument("--iterations", type=int, default=400)
    parser.add_argument("--depth", type=int, default=5)
    parser.add_argument("--learning-rate", type=float, default=0.04)
    parser.add_argument("--l2-leaf-reg", type=float, default=12.0)
    parser.add_argument("--random-seed", type=int, default=73)

    # Candle enrichment defaults.
    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=240.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    parser.add_argument("--time-exit-bars", type=int, default=0)
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


def registry_dir(value: str) -> Path:
    path = Path(value)
    if path.exists():
        return path.resolve()
    return wave.ABCD_ROOT / "model_registry" / value


def parse_roots(value: str) -> set[str]:
    return {part.strip().upper() for part in str(value or "").split(",") if part.strip()}


def load_source_rows(model_dir: Path, split: str, args: argparse.Namespace) -> pd.DataFrame:
    path = model_dir / f"target_scored_{split}_entries.csv"
    if not path.exists():
        raise FileNotFoundError(path)
    frame = pd.read_csv(path)
    roots = parse_roots(args.roots)
    frame["root_symbol"] = frame["root_symbol"].astype(str).str.upper()
    frame["symbol"] = frame["symbol"].astype(str).str.upper()
    frame["direction"] = frame["direction"].astype(str).str.upper()
    for column in ["entry_date", "terminal_exit_date", "confirm_date", "signal_date"]:
        if column in frame.columns:
            frame[column] = pd.to_datetime(frame[column], errors="coerce")
    for column in ["stage2_score", "risk_ticks", "predicted_mfe_r", "risk_points", "tick_size", "entry_price"]:
        frame[column] = pd.to_numeric(frame.get(column), errors="coerce")
    frame = frame[
        frame["root_symbol"].isin(roots)
        & (frame["stage2_score"] >= float(args.score_min))
        & (frame["risk_ticks"] >= float(args.risk_min))
        & (frame["risk_ticks"] <= float(args.risk_max))
        & (frame["predicted_mfe_r"].fillna(0.0) >= float(args.predicted_mfe_min))
    ].copy()
    return frame.dropna(subset=["entry_date", "entry_price", "risk_points", "tick_size"]).sort_values("entry_date").reset_index(drop=True)


def fetch_candles_for_rows(conn, rows: pd.DataFrame, year: int, args: argparse.Namespace) -> dict[str, pd.DataFrame]:
    if rows.empty:
        return {}
    candles = stage2_l2.fetch_candles_for_symbols(conn, args.timeframe, int(year), rows["symbol"].dropna().unique().tolist())
    output: dict[str, pd.DataFrame] = {}
    for symbol, raw_group in candles.groupby("symbol", sort=False):
        enriched = scanner.enrich_candles(raw_group[["ts_utc", "open", "high", "low", "close", "volume"]].copy(), args)
        output[str(symbol).upper()] = enriched.reset_index(drop=True)
    return output


def sign_for(direction: str) -> int:
    return 1 if str(direction).upper() == "LONG" else -1


def decision_features(
    trade: pd.Series,
    candles: pd.DataFrame,
    decision_idx: int,
    exit_idx: int,
    max_idx: int,
    args: argparse.Namespace,
) -> dict[str, Any]:
    entry_idx = int(trade["entry_index"])
    entry = float(trade["entry_price"])
    risk = float(trade["risk_points"])
    tick = float(trade["tick_size"])
    sign = sign_for(str(trade["direction"]))
    row = candles.iloc[int(decision_idx)]
    prev_idx = max(entry_idx, decision_idx - 1)
    prev = candles.iloc[prev_idx]
    close = float(row["close"])
    open_next = float(candles["open"].iloc[int(exit_idx)])
    high_so_far = float(candles["high"].iloc[entry_idx : decision_idx + 1].max())
    low_so_far = float(candles["low"].iloc[entry_idx : decision_idx + 1].min())
    if sign > 0:
        max_favorable = (high_so_far - entry) / risk
        max_adverse = (entry - low_so_far) / risk
    else:
        max_favorable = (entry - low_so_far) / risk
        max_adverse = (high_so_far - entry) / risk
    current_r = sign * (close - entry) / risk
    exit_now_r = sign * (open_next - entry) / risk - float(trade["slippage_r"])
    last_bar_r = sign * (float(row["close"]) - float(prev["close"])) / risk
    ret_3_idx = max(entry_idx, decision_idx - 3)
    ret_6_idx = max(entry_idx, decision_idx - 6)
    return {
        "hold_bars": int(decision_idx - entry_idx),
        "hold_ratio": float((decision_idx - entry_idx) / max(1, int(args.max_forward_bars))),
        "current_r": float(current_r),
        "exit_now_r": float(exit_now_r),
        "exit_now_price": float(open_next),
        "max_favorable_r": float(max_favorable),
        "max_adverse_r": float(max_adverse),
        "giveback_r": float(max(0.0, max_favorable - current_r)),
        "last_bar_r": float(last_bar_r),
        "last_range_r": float((float(row["high"]) - float(row["low"])) / risk),
        "last_body_r": float(abs(float(row["close"]) - float(row["open"])) / risk),
        "last_align_r": float(sign * (float(row["close"]) - float(row["open"])) / risk),
        "ret_3_r": float(sign * (float(row["close"]) - float(candles["close"].iloc[ret_3_idx])) / risk),
        "ret_6_r": float(sign * (float(row["close"]) - float(candles["close"].iloc[ret_6_idx])) / risk),
        "ema_fast_dist_r": float(sign * (close - finite(row.get("ema_fast"), close)) / risk),
        "ema_slow_dist_r": float(sign * (close - finite(row.get("ema_slow"), close)) / risk),
        "ema_gap_r": float(sign * (finite(row.get("ema_fast"), close) - finite(row.get("ema_slow"), close)) / risk),
        "ema_fast_slope_r": float(sign * finite(row.get("ema_fast_slope"), 0.0) / risk),
        "atr_r": float(finite(row.get("atr"), tick * float(args.min_risk_ticks)) / risk),
        "relative_volume_now": float(finite(row.get("relative_volume"), 1.0)),
        "compression_now": float(finite(row.get("compression"), 0.0)),
        "momentum_dead_now": 1.0 if wave.momentum_dead(candles, int(decision_idx), str(trade["direction"])) else 0.0,
    }


def static_payload(trade: pd.Series) -> dict[str, Any]:
    payload: dict[str, Any] = {}
    for column in CAT_FEATURES + NUM_FEATURES:
        if column in trade.index and column not in {
            "hold_bars",
            "hold_ratio",
            "current_r",
            "exit_now_r",
            "max_favorable_r",
            "max_adverse_r",
            "giveback_r",
            "last_bar_r",
            "last_range_r",
            "last_body_r",
            "last_align_r",
            "ret_3_r",
            "ret_6_r",
            "ema_fast_dist_r",
            "ema_slow_dist_r",
            "ema_gap_r",
            "ema_fast_slope_r",
            "atr_r",
            "relative_volume_now",
            "compression_now",
            "momentum_dead_now",
        }:
            payload[column] = trade.get(column)
    return payload


def build_decision_rows(
    rows: pd.DataFrame,
    candles_by_symbol: dict[str, pd.DataFrame],
    args: argparse.Namespace,
    label: str,
) -> pd.DataFrame:
    output: list[dict[str, Any]] = []
    for trade_index, (_, trade) in enumerate(rows.iterrows(), start=1):
        symbol = str(trade["symbol"]).upper()
        candles = candles_by_symbol.get(symbol)
        if candles is None or candles.empty:
            continue
        entry_idx = int(trade["entry_index"])
        if entry_idx >= len(candles) - 2:
            continue
        max_idx = min(len(candles) - 1, entry_idx + max(2, int(args.max_forward_bars)))
        if max_idx <= entry_idx + int(args.min_hold_bars):
            continue
        entry = float(trade["entry_price"])
        risk = float(trade["risk_points"])
        sign = sign_for(str(trade["direction"]))
        slippage_r = float(trade["slippage_r"])
        static = static_payload(trade)
        time_exit_price = float(candles["close"].iloc[int(max_idx)])
        time_exit_r = sign * (time_exit_price - entry) / risk - slippage_r
        time_exit_date = candles["ts_utc"].iloc[int(max_idx)]
        future_open_r = sign * (candles["open"].iloc[entry_idx + 1 : max_idx + 1].to_numpy(dtype=float) - entry) / risk - slippage_r
        if len(future_open_r) == 0:
            continue
        for decision_idx in range(entry_idx + int(args.min_hold_bars), max_idx, int(args.decision_step_bars)):
            exit_idx = decision_idx + 1
            local_offset = max(0, exit_idx - (entry_idx + 1))
            future = future_open_r[local_offset:]
            if len(future) == 0:
                continue
            exit_now_r = float(future[0])
            best_future_r = float(np.max(future))
            target = 1 if exit_now_r >= best_future_r - float(args.exit_label_margin_r) else 0
            payload = {
                **static,
                **decision_features(trade, candles, decision_idx, exit_idx, max_idx, args),
                "candidate_uid": trade.get("candidate_uid"),
                "entry_date": trade.get("entry_date"),
                "entry_price": float(entry),
                "risk_points": float(risk),
                "decision_date": candles["ts_utc"].iloc[int(decision_idx)],
                "exit_now_date": candles["ts_utc"].iloc[int(exit_idx)],
                "time_exit_date": time_exit_date,
                "time_exit_price": float(time_exit_price),
                "time_exit_r": float(time_exit_r),
                "terminal_result_r": trade.get("terminal_result_r"),
                "terminal_exit_date": trade.get("terminal_exit_date"),
                "terminal_exit_reason": trade.get("terminal_exit_reason"),
                "oracle_best_future_r": best_future_r,
                "exit_target": int(target),
            }
            output.append(payload)
        if trade_index % 200 == 0:
            print(f"{label}: decision rows {trade_index:,}/{len(rows):,} -> {len(output):,}", flush=True)
    frame = pd.DataFrame(output)
    print(f"{label}: built {len(frame):,} decision rows from {len(rows):,} trades", flush=True)
    return frame


def prepare_pool(frame: pd.DataFrame, include_target: bool) -> Pool:
    work = frame.copy()
    for column in CAT_FEATURES:
        if column not in work.columns:
            work[column] = "missing"
        work[column] = work[column].fillna("missing").astype(str)
    for column in NUM_FEATURES:
        if column not in work.columns:
            work[column] = 0.0
        work[column] = pd.to_numeric(work[column], errors="coerce").replace([np.inf, -np.inf], np.nan).fillna(0.0)
    if include_target:
        weights = np.where(work["exit_target"].astype(int).to_numpy() == 1, 1.25, 1.0)
        return Pool(work[CAT_FEATURES + NUM_FEATURES], label=work["exit_target"].astype(int), weight=weights, cat_features=CAT_FEATURES)
    return Pool(work[CAT_FEATURES + NUM_FEATURES], cat_features=CAT_FEATURES)


def train_model(decisions: pd.DataFrame, args: argparse.Namespace) -> CatBoostClassifier:
    model = CatBoostClassifier(
        loss_function="Logloss",
        eval_metric="AUC",
        iterations=int(args.iterations),
        depth=int(args.depth),
        learning_rate=float(args.learning_rate),
        l2_leaf_reg=float(args.l2_leaf_reg),
        random_seed=int(args.random_seed),
        auto_class_weights="Balanced",
        verbose=100,
        allow_writing_files=False,
    )
    model.fit(prepare_pool(decisions, include_target=True))
    return model


def add_scores(decisions: pd.DataFrame, model: CatBoostClassifier) -> pd.DataFrame:
    scored = decisions.copy()
    scored["exit_score"] = model.predict_proba(prepare_pool(scored, include_target=False))[:, 1]
    return scored


def simulate_scored_policy(scored: pd.DataFrame, threshold: float) -> pd.DataFrame:
    if scored.empty:
        return pd.DataFrame()
    ordered = scored.sort_values(["candidate_uid", "decision_date"], kind="mergesort").copy()
    last_rows = ordered.drop_duplicates("candidate_uid", keep="last").copy()
    hits = ordered[pd.to_numeric(ordered["exit_score"], errors="coerce").fillna(0.0) >= float(threshold)].copy()
    hits = hits.drop_duplicates("candidate_uid", keep="first")
    hit_map = hits.set_index("candidate_uid")
    rows: list[dict[str, Any]] = []
    for _, last in last_rows.iterrows():
        uid = str(last["candidate_uid"])
        if uid in hit_map.index:
            hit = hit_map.loc[uid]
            exit_date = hit.get("exit_now_date")
            exit_price = finite(hit.get("exit_now_price"), finite(hit.get("entry_price"), 0.0))
            result_r = finite(hit.get("exit_now_r"), 0.0)
            exit_reason = "policy_model_exit"
            exit_score = finite(hit.get("exit_score"), 0.0)
            hold_bars = int(finite(hit.get("hold_bars"), 0.0)) + 1
        else:
            exit_date = last.get("time_exit_date")
            exit_price = finite(last.get("time_exit_price"), finite(last.get("entry_price"), 0.0))
            result_r = finite(last.get("time_exit_r"), 0.0)
            exit_reason = "policy_time_exit"
            exit_score = None
            hold_bars = int(finite(last.get("hold_bars"), 0.0)) + 1
        rows.append(
            {
                "candidate_uid": uid,
                "root_symbol": last.get("root_symbol"),
                "symbol": last.get("symbol"),
                "direction": last.get("direction"),
                "entry_date": last.get("entry_date"),
                "entry_price": finite(last.get("entry_price"), 0.0),
                "risk_points": finite(last.get("risk_points"), 0.0),
                "risk_ticks": finite(last.get("risk_ticks"), 0.0),
                "stage2_score": finite(last.get("stage2_score"), 0.0),
                "exit_date": exit_date,
                "exit_price": exit_price,
                "exit_reason": exit_reason,
                "exit_score": exit_score,
                "result_r": result_r,
                "hold_bars": hold_bars,
                "terminal_result_r": finite(last.get("terminal_result_r"), 0.0),
                "terminal_exit_date": last.get("terminal_exit_date"),
                "terminal_exit_reason": last.get("terminal_exit_reason"),
            }
        )
    frame = pd.DataFrame(rows)
    for column in ["entry_date", "exit_date", "terminal_exit_date"]:
        frame[column] = pd.to_datetime(frame[column], errors="coerce")
    return frame.dropna(subset=["entry_date", "exit_date"]).reset_index(drop=True)


def simulate_policy_for_trade(
    trade: pd.Series,
    candles: pd.DataFrame,
    model: CatBoostClassifier,
    threshold: float,
    args: argparse.Namespace,
) -> dict[str, Any] | None:
    entry_idx = int(trade["entry_index"])
    if entry_idx >= len(candles) - 2:
        return None
    max_idx = min(len(candles) - 1, entry_idx + max(2, int(args.max_forward_bars)))
    entry = float(trade["entry_price"])
    risk = float(trade["risk_points"])
    sign = sign_for(str(trade["direction"]))
    slippage_r = float(trade["slippage_r"])
    max_fav = 0.0
    max_adv = 0.0
    exit_idx = max_idx
    exit_price = float(candles["close"].iloc[max_idx])
    exit_reason = "policy_time_exit"
    exit_score = None

    for decision_idx in range(entry_idx + int(args.min_hold_bars), max_idx, int(args.decision_step_bars)):
        row = candles.iloc[int(decision_idx)]
        if sign > 0:
            current_r = (float(row["close"]) - entry) / risk
        else:
            current_r = (entry - float(row["close"])) / risk
        if float(args.disaster_stop_r) > 0 and current_r <= -float(args.disaster_stop_r):
            exit_idx = min(decision_idx + 1, max_idx)
            exit_price = float(candles["open"].iloc[exit_idx])
            exit_reason = "policy_disaster_guard"
            exit_score = None
            break
        payload = {
            **static_payload(trade),
            **decision_features(trade, candles, decision_idx, min(decision_idx + 1, max_idx), max_idx, args),
        }
        score_frame = pd.DataFrame([payload])
        score = float(model.predict_proba(prepare_pool(score_frame, include_target=False))[:, 1][0])
        if score >= float(threshold):
            exit_idx = min(decision_idx + 1, max_idx)
            exit_price = float(candles["open"].iloc[exit_idx])
            exit_reason = "policy_model_exit"
            exit_score = score
            break

    high_so_far = float(candles["high"].iloc[entry_idx : exit_idx + 1].max())
    low_so_far = float(candles["low"].iloc[entry_idx : exit_idx + 1].min())
    if sign > 0:
        max_fav = (high_so_far - entry) / risk
        max_adv = (entry - low_so_far) / risk
    else:
        max_fav = (entry - low_so_far) / risk
        max_adv = (high_so_far - entry) / risk
    raw_result = sign * (float(exit_price) - entry) / risk
    result = raw_result - slippage_r
    return {
        "candidate_uid": trade.get("candidate_uid"),
        "root_symbol": trade.get("root_symbol"),
        "symbol": trade.get("symbol"),
        "direction": trade.get("direction"),
        "entry_date": trade.get("entry_date"),
        "entry_price": float(entry),
        "risk_points": float(risk),
        "risk_ticks": float(trade.get("risk_ticks")),
        "stage2_score": float(trade.get("stage2_score")),
        "exit_date": candles["ts_utc"].iloc[int(exit_idx)],
        "exit_price": float(exit_price),
        "exit_reason": exit_reason,
        "exit_score": exit_score,
        "result_r": float(result),
        "raw_result_r": float(raw_result),
        "mfe_r": float(max_fav),
        "mae_r": float(max_adv),
        "hold_bars": int(exit_idx - entry_idx),
        "terminal_result_r": float(trade.get("terminal_result_r")),
        "terminal_exit_date": trade.get("terminal_exit_date"),
        "terminal_exit_reason": trade.get("terminal_exit_reason"),
    }


def simulate_policy(
    rows: pd.DataFrame,
    candles_by_symbol: dict[str, pd.DataFrame],
    model: CatBoostClassifier,
    threshold: float,
    args: argparse.Namespace,
) -> pd.DataFrame:
    output: list[dict[str, Any]] = []
    for _, trade in rows.iterrows():
        candles = candles_by_symbol.get(str(trade["symbol"]).upper())
        if candles is None:
            continue
        result = simulate_policy_for_trade(trade, candles, model, threshold, args)
        if result is not None:
            output.append(result)
    return pd.DataFrame(output)


def max_drawdown(values: list[float]) -> float:
    equity = 0.0
    peak = 0.0
    max_dd = 0.0
    for value in values:
        equity += float(value)
        peak = max(peak, equity)
        max_dd = max(max_dd, peak - equity)
    return float(max_dd)


def max_concurrent(frame: pd.DataFrame, exit_col: str = "exit_date") -> int:
    events: list[tuple[pd.Timestamp, int]] = []
    for row in frame[["entry_date", exit_col]].itertuples(index=False):
        events.append((pd.Timestamp(row.entry_date), 1))
        events.append((pd.Timestamp(row[1]), -1))
    events.sort(key=lambda item: (item[0], item[1]))
    current = 0
    max_seen = 0
    for _, delta in events:
        current += delta
        max_seen = max(max_seen, current)
    return int(max_seen)


def summarize(frame: pd.DataFrame, result_col: str = "result_r", exit_col: str = "exit_date") -> dict[str, Any]:
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
    results = [float(value) for value in ordered[result_col]]
    wins = sum(1 for value in results if value > 0)
    total = float(sum(results))
    dd = max_drawdown(results)
    return {
        "trades": int(len(results)),
        "wins": int(wins),
        "losses": int(len(results) - wins),
        "win_rate": float(wins / len(results)) if results else 0.0,
        "sum_r": total,
        "avg_r": float(np.mean(results)) if results else 0.0,
        "dd_r": dd,
        "score_to_dd_r": float(total / dd) if dd > 0 else (999.0 if total > 0 else 0.0),
        "max_concurrent": max_concurrent(frame, exit_col),
    }


def threshold_candidates(scored: pd.DataFrame) -> list[float]:
    values = pd.to_numeric(scored.get("exit_score"), errors="coerce").dropna()
    if values.empty:
        return [0.5]
    quantiles = [0.30, 0.40, 0.50, 0.60, 0.70, 0.80, 0.85, 0.90, 0.925, 0.95, 0.975]
    return sorted(set(float(value) for value in values.quantile(quantiles).dropna()))


def main() -> int:
    args = parse_args()
    scanner.configure_wave_args(args)
    source_dir = registry_dir(str(args.entry_source_run))
    output_dir = wave.ABCD_ROOT / "model_registry" / str(args.run_id)
    if output_dir.exists() and any(output_dir.iterdir()) and not args.replace:
        raise SystemExit(f"Output exists: {output_dir}. Use --replace.")
    output_dir.mkdir(parents=True, exist_ok=True)

    train_rows = load_source_rows(source_dir, "train", args)
    threshold_rows = load_source_rows(source_dir, "threshold", args)
    valid_rows = load_source_rows(source_dir, "valid", args)
    print(
        f"Loaded entry rows train={len(train_rows):,} threshold={len(threshold_rows):,} valid={len(valid_rows):,}",
        flush=True,
    )

    conn = wave.connect()
    try:
        train_candles = fetch_candles_for_rows(conn, train_rows, int(args.train_year), args)
        threshold_candles = fetch_candles_for_rows(conn, threshold_rows, int(args.train_year), args)
        valid_candles = fetch_candles_for_rows(conn, valid_rows, int(args.valid_year), args)
    finally:
        conn.close()

    train_decisions = build_decision_rows(train_rows, train_candles, args, "train")
    threshold_decisions = build_decision_rows(threshold_rows, threshold_candles, args, "threshold")
    valid_decisions = build_decision_rows(valid_rows, valid_candles, args, "valid")
    train_decisions.to_csv(output_dir / "candle_policy_train_decisions.csv", index=False)
    threshold_decisions.to_csv(output_dir / "candle_policy_threshold_decisions.csv", index=False)
    valid_decisions.to_csv(output_dir / "candle_policy_valid_decisions.csv", index=False)

    model = train_model(train_decisions, args)
    model.save_model(str(output_dir / "catboost_stage3_candle_policy_exit.cbm"))
    threshold_scored = add_scores(threshold_decisions, model)
    valid_scored = add_scores(valid_decisions, model)
    threshold_scored.to_csv(output_dir / "candle_policy_threshold_scored_decisions.csv", index=False)
    valid_scored.to_csv(output_dir / "candle_policy_valid_scored_decisions.csv", index=False)

    threshold_results = []
    for threshold in threshold_candidates(threshold_scored):
        trades = simulate_scored_policy(threshold_scored, threshold)
        stats = summarize(trades)
        if stats["trades"] < int(args.min_threshold_trades):
            continue
        terminal_stats = summarize(
            trades.rename(columns={"terminal_result_r": "terminal_tmp", "terminal_exit_date": "terminal_tmp_date"}),
            "terminal_tmp",
            "terminal_tmp_date",
        )
        threshold_results.append(
            {
                "exit_threshold": float(threshold),
                **{f"policy_{key}": value for key, value in stats.items()},
                **{f"terminal_{key}": value for key, value in terminal_stats.items()},
                "delta_sum_r": float(stats["sum_r"] - terminal_stats["sum_r"]),
                "delta_dd_r": float(stats["dd_r"] - terminal_stats["dd_r"]),
            }
        )
    threshold_table = pd.DataFrame(threshold_results).sort_values(
        ["policy_score_to_dd_r", "policy_sum_r", "policy_avg_r"],
        ascending=[False, False, False],
        kind="mergesort",
    )
    threshold_table["threshold_rank"] = range(1, len(threshold_table) + 1)
    threshold_table.to_csv(output_dir / "candle_policy_threshold_sweep.csv", index=False)

    valid_results = []
    best_valid_trades = pd.DataFrame()
    for rule in threshold_table.head(12).itertuples(index=False):
        threshold = float(rule.exit_threshold)
        trades = simulate_scored_policy(valid_scored, threshold)
        stats = summarize(trades)
        terminal_stats = summarize(
            trades.rename(columns={"terminal_result_r": "terminal_tmp", "terminal_exit_date": "terminal_tmp_date"}),
            "terminal_tmp",
            "terminal_tmp_date",
        )
        row = {
            "threshold_rank": int(rule.threshold_rank),
            "exit_threshold": threshold,
            **{f"policy_{key}": value for key, value in stats.items()},
            **{f"terminal_{key}": value for key, value in terminal_stats.items()},
            "delta_sum_r": float(stats["sum_r"] - terminal_stats["sum_r"]),
            "delta_dd_r": float(stats["dd_r"] - terminal_stats["dd_r"]),
        }
        valid_results.append(row)
        best_now = summarize(best_valid_trades) if not best_valid_trades.empty else {"score_to_dd_r": -999.0, "sum_r": -999.0}
        if (stats["score_to_dd_r"], stats["sum_r"]) > (best_now["score_to_dd_r"], best_now["sum_r"]):
            best_valid_trades = trades.copy()

    valid_table = pd.DataFrame(valid_results).sort_values(
        ["policy_score_to_dd_r", "policy_sum_r", "policy_avg_r"],
        ascending=[False, False, False],
        kind="mergesort",
    )
    valid_table["valid_rank"] = range(1, len(valid_table) + 1)
    valid_table.to_csv(output_dir / "candle_policy_valid_results.csv", index=False)
    best_valid_trades.to_csv(output_dir / "candle_policy_best_valid_trades.csv", index=False)

    metadata = {
        "run_id": str(args.run_id),
        "model_type": "stage3_candle_by_candle_hold_exit_policy",
        "entry_source_run": str(args.entry_source_run),
        "train_year": int(args.train_year),
        "valid_year": int(args.valid_year),
        "timeframe": str(args.timeframe),
        "roots": sorted(parse_roots(args.roots)),
        "filters": {
            "score_min": float(args.score_min),
            "risk_min": float(args.risk_min),
            "risk_max": float(args.risk_max),
            "predicted_mfe_min": float(args.predicted_mfe_min),
        },
        "strategy_stop": "none; model decides every candle",
        "strategy_take_profit": "none; model decides every candle",
        "risk_unit": "initial swing/ATR risk distance used only for R normalization",
        "disaster_stop_r": float(args.disaster_stop_r),
        "rows": {
            "train_entries": int(len(train_rows)),
            "threshold_entries": int(len(threshold_rows)),
            "valid_entries": int(len(valid_rows)),
            "train_decisions": int(len(train_decisions)),
            "threshold_decisions": int(len(threshold_decisions)),
            "valid_decisions": int(len(valid_decisions)),
        },
        "top_threshold": threshold_table.head(10).to_dict("records"),
        "top_valid": valid_table.head(10).to_dict("records"),
        "best_valid_exit_reasons": best_valid_trades["exit_reason"].value_counts().to_dict() if not best_valid_trades.empty else {},
    }
    (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")
    print(json.dumps(metadata, indent=2, default=to_jsonable), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
