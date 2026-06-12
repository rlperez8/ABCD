#!/usr/bin/env python3
"""
Train a Stage 3 trend manager from Stage 2 confirmed trend-start events.

Flow:
    Level 2 trio manager fires.
    Stage 2 confirms the trend.
    Stage 3 enters on the next candle open and manages the live trade.

The Stage 3 model is source-free: it does not use the old pattern entry model or
the old source-exit signal. It learns candle-by-candle whether exiting on the
next open is better than continuing inside a hard-stop/time-window path.
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


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_exit_model as exit_model
import ai_candle_wave_scanner_model as scanner
import ai_oracle_start_stage2_l2_confirmation as stage2_l2
import ai_oracle_trend_start_model as start_model
import ai_wave_rider_research as wave


DEFAULT_STAGE2_RUN = "aicw-os-stage2-l2-confirm-v1-2m-m-tr2025-v2026-m16-v2026"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage2-run-id", default=DEFAULT_STAGE2_RUN)
    parser.add_argument("--run-prefix", default="aicw-os-stage3-trend-manager-v1")
    parser.add_argument("--timeframe", default="2m", choices=sorted(scanner.TIMEFRAME_TABLES))
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument(
        "--valid-split",
        default="",
        help="Stage 2 event split to validate. Defaults to valid-year, but can be threshold.",
    )
    parser.add_argument("--replace-run", action="store_true")
    parser.add_argument("--max-train-events", type=int, default=0)
    parser.add_argument("--max-threshold-events", type=int, default=0)
    parser.add_argument("--max-valid-events", type=int, default=0)
    parser.add_argument(
        "--event-directions",
        default="LONG,SHORT",
        help="Comma-separated Stage 2 confirmed directions to keep, e.g. LONG or SHORT.",
    )
    parser.add_argument(
        "--terminal-only",
        action="store_true",
        help="Only validate the hard-stop/window terminal path; skip exit-overlay training.",
    )
    parser.add_argument("--save-decision-rows", action="store_true")

    parser.add_argument("--min-hold-bars", type=int, default=2)
    parser.add_argument("--decision-step-bars", type=int, default=1)
    parser.add_argument("--dynamic-max-bars", type=int, default=180)
    parser.add_argument("--exit-threshold", type=float, default=None)
    parser.add_argument("--max-threshold-dd-r", type=float, default=0.0)
    parser.add_argument("--threshold-dd-penalty", type=float, default=0.0)
    parser.add_argument("--min-threshold-trades", type=int, default=80)
    parser.add_argument("--iterations", type=int, default=350)
    parser.add_argument("--depth", type=int, default=5)
    parser.add_argument("--learning-rate", type=float, default=0.04)
    parser.add_argument("--l2-leaf-reg", type=float, default=12.0)
    parser.add_argument("--random-seed", type=int, default=73)

    # Rule-trend baseline / risk knobs.
    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=240.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    parser.add_argument("--death-bars", type=int, default=4)
    parser.add_argument("--profit-lock-trigger-r", type=float, default=0.0)
    parser.add_argument("--profit-lock-giveback-r", type=float, default=1.5)
    parser.add_argument("--profit-lock-min-r", type=float, default=0.25)
    parser.add_argument("--max-forward-bars", type=int, default=180)
    parser.add_argument("--time-exit-bars", type=int, default=0)
    parser.add_argument("--slippage-entry-ticks", type=float, default=3.0)
    parser.add_argument("--slippage-exit-ticks", type=float, default=3.0)
    return parser.parse_args()


def to_jsonable(value: Any) -> Any:
    if isinstance(value, pd.Timestamp):
        return value.isoformat()
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def stage2_dir(run_id_value: str) -> Path:
    path = Path(run_id_value)
    if path.exists():
        return path.resolve()
    return wave.ABCD_ROOT / "model_registry" / run_id_value


def run_id(args: argparse.Namespace) -> str:
    raw = f"{args.run_prefix}-{args.timeframe}-m{int(args.dynamic_max_bars)}-v{int(args.valid_year)}"
    if len(raw) <= 100:
        return raw
    return f"{args.run_prefix[:54]}-{args.timeframe}-v{int(args.valid_year)}"


def parse_directions(value: str) -> set[str]:
    directions = {item.strip().upper() for item in str(value or "").split(",") if item.strip()}
    invalid = directions - {"LONG", "SHORT"}
    if invalid:
        raise ValueError(f"Unsupported event direction(s): {sorted(invalid)}")
    return directions or {"LONG", "SHORT"}


def load_stage2_events(model_dir: Path, split: str, limit: int, seed: int, directions: set[str]) -> pd.DataFrame:
    path = model_dir / f"stage2_l2_events_{split}.csv"
    if not path.exists():
        raise FileNotFoundError(f"Missing Stage 2 event rows: {path}")
    frame = pd.read_csv(path)
    frame = frame[pd.to_numeric(frame.get("confirmed"), errors="coerce").fillna(0).astype(int) == 1].copy()
    frame["direction"] = frame["direction"].astype(str).str.upper()
    frame = frame[frame["direction"].isin(directions)].copy()
    for col in ["signal_date", "confirm_date"]:
        frame[col] = pd.to_datetime(frame[col], errors="coerce")
    frame = frame.dropna(subset=["candidate_uid", "symbol", "root_symbol", "direction", "confirm_date"]).reset_index(drop=True)
    if int(limit) > 0 and len(frame) > int(limit):
        frame = frame.sample(n=int(limit), random_state=seed).sort_values(["confirm_date", "candidate_uid"]).reset_index(drop=True)
    return frame


def fetch_candles_for_symbols(conn, timeframe: str, year: int, symbols: list[str]) -> pd.DataFrame:
    return stage2_l2.fetch_candles_for_symbols(conn, timeframe, int(year), symbols)


def entry_feature_payload(
    year: int,
    root: str,
    symbol: str,
    candles: pd.DataFrame,
    entry_idx: int,
    direction: str,
    source: dict[str, Any],
) -> dict[str, Any]:
    payload = start_model.feature_row(int(year), root, symbol, candles, int(entry_idx), direction)
    payload["candidate_uid"] = str(source.get("candidate_uid") or payload.get("candidate_uid") or "")
    payload["signal_date"] = source.get("signal_date")
    payload["confirmed_signal_date"] = source.get("confirm_date")
    payload["predicted_r"] = wave.finite(source.get("stage2_score"), wave.finite(source.get("level2_score"), 0.0))
    payload["stage1_score"] = wave.finite(source.get("stage1_score"), None)
    payload["level2_score"] = wave.finite(source.get("level2_score"), None)
    payload["stage2_score"] = wave.finite(source.get("stage2_score"), None)
    payload["stage2_confirm_offset_bars"] = wave.finite(source.get("confirm_offset_bars"), None)
    payload["top_l1_block"] = source.get("top_l1_block")
    return payload


def terminal_path_from_candles(
    candles: pd.DataFrame,
    entry_idx: int,
    direction: str,
    entry_price: float,
    stop_price: float,
    risk_points: float,
    tick_size: float,
    args: argparse.Namespace,
) -> dict[str, Any] | None:
    if entry_idx >= len(candles) or risk_points <= 0 or tick_size <= 0:
        return None
    max_idx = min(len(candles) - 1, int(entry_idx) + max(2, int(args.dynamic_max_bars)))
    hard_stop_idx = None
    for candle_idx in range(int(entry_idx), max_idx + 1):
        candle = candles.iloc[candle_idx]
        if direction == "LONG" and exit_model.safe_float(candle.get("low")) <= stop_price:
            hard_stop_idx = candle_idx
            break
        if direction == "SHORT" and exit_model.safe_float(candle.get("high")) >= stop_price:
            hard_stop_idx = candle_idx
            break

    if hard_stop_idx is not None:
        exit_idx = hard_stop_idx
        exit_price = stop_price
        exit_reason = "dynamic_hard_stop"
    else:
        exit_idx = max_idx
        exit_price = exit_model.safe_float(candles["close"].iloc[exit_idx])
        exit_reason = "dynamic_time_exit"

    sign = wave.direction_sign(direction)
    slippage_r = ((float(args.slippage_entry_ticks) + float(args.slippage_exit_ticks)) * tick_size) / risk_points
    raw_result = sign * (float(exit_price) - float(entry_price)) / risk_points
    result_r = raw_result - slippage_r
    entry_date = candles["ts_utc"].iloc[int(entry_idx)]
    exit_date = candles["ts_utc"].iloc[int(exit_idx)]
    hold_minutes = int((pd.Timestamp(exit_date) - pd.Timestamp(entry_date)).total_seconds() / 60)
    return {
        "terminal_exit_date": exit_date,
        "terminal_exit_price": exit_price,
        "terminal_exit_reason": exit_reason,
        "terminal_result_r": result_r,
        "terminal_raw_result_r": raw_result,
        "terminal_hold_minutes": hold_minutes,
        "terminal_exit_index": int(exit_idx),
    }


def trade_row_from_event(
    event: dict[str, Any],
    candles: pd.DataFrame,
    year: int,
    args: argparse.Namespace,
    selected_index: int,
) -> dict[str, Any] | None:
    times = pd.to_datetime(candles["ts_utc"], errors="coerce").to_numpy(dtype="datetime64[ns]")
    confirm_date = pd.Timestamp(event["confirm_date"])
    confirm_idx = int(np.searchsorted(times, np.datetime64(confirm_date), side="left"))
    entry_idx = confirm_idx + 1
    if entry_idx >= len(candles):
        return None

    root = str(event["root_symbol"])
    symbol = str(event["symbol"])
    direction = str(event["direction"]).upper()
    tick_size = wave.tick_size_for(symbol, root)
    result = wave.simulate_from_entry(candles, confirm_date, entry_idx, None, direction, tick_size, args)
    if result.outcome == "no_entry" or result.entry_date is None or result.exit_date is None:
        return None

    feature_payload = entry_feature_payload(int(year), root, symbol, candles, entry_idx, direction, event)
    terminal = terminal_path_from_candles(
        candles,
        int(result.entry_index if result.entry_index is not None else entry_idx),
        direction,
        float(result.entry_price),
        float(result.stop_price),
        float(result.risk_points),
        float(tick_size),
        args,
    )
    row = {
        **feature_payload,
        "selected_index": int(selected_index),
        "formula": "stage2_l2_confirm_next_open_stage3_trend_manager_v1",
        "timeframe": args.timeframe,
        "valid_year": int(year),
        "root_symbol": root,
        "symbol": symbol,
        "direction": direction,
        "entry_date": result.entry_date,
        "exit_date": result.exit_date,
        "entry_price": result.entry_price,
        "stop_price": result.stop_price,
        "exit_price": result.exit_price,
        "exit_reason": result.exit_reason,
        "risk_points": result.risk_points,
        "result_r": result.result_r,
        "raw_result_r": result.raw_result_r,
        "mfe_r": result.mfe_r,
        "mae_r": result.mae_r,
        "hold_minutes": result.hold_minutes,
        "risk_ticks": result.risk_ticks,
        "tick_size": tick_size,
        "entry_index": result.entry_index,
        "exit_index": result.exit_index,
        "stage2_candidate_uid": str(event.get("candidate_uid") or ""),
    }
    if terminal is not None:
        row.update(terminal)
    return row


def build_trades(conn, events: pd.DataFrame, year: int, args: argparse.Namespace, label: str) -> pd.DataFrame:
    if events.empty:
        return pd.DataFrame()
    candles = fetch_candles_for_symbols(conn, args.timeframe, int(year), events["symbol"].astype(str).unique().tolist())
    if candles.empty:
        raise ValueError(f"{label}: no candles for selected Stage 2 events")
    rows: list[dict[str, Any]] = []
    event_groups = {
        str(symbol): group.sort_values("confirm_date").reset_index(drop=True)
        for symbol, group in events.groupby("symbol", sort=False)
    }
    for symbol_index, (symbol, raw_group) in enumerate(candles.groupby("symbol", sort=True), start=1):
        group_events = event_groups.get(str(symbol))
        if group_events is None or group_events.empty:
            continue
        enriched = scanner.enrich_candles(raw_group[["ts_utc", "open", "high", "low", "close", "volume"]].copy(), args)
        root = str(raw_group["root_symbol"].iloc[0])
        for event in group_events.to_dict("records"):
            row = trade_row_from_event(event, enriched, int(year), args, len(rows) + 1)
            if row is not None:
                rows.append(row)
        if symbol_index % 50 == 0:
            print(f"{label}: built trades symbols={symbol_index:,} trades={len(rows):,}/{len(events):,}", flush=True)
    frame = pd.DataFrame(rows)
    print(f"{label}: built {len(frame):,} trade rows from {len(events):,} Stage 2 confirmations", flush=True)
    return exit_model.normalize_trade_frame(frame)


def exit_args(args: argparse.Namespace) -> argparse.Namespace:
    return argparse.Namespace(
        mode="dynamic",
        min_hold_bars=int(args.min_hold_bars),
        decision_step_bars=int(args.decision_step_bars),
        dynamic_max_bars=int(args.dynamic_max_bars),
        dynamic_no_signal_exit="terminal",
        exit_threshold=args.exit_threshold,
        exclude_source_exit_features=True,
        max_threshold_dd_r=float(args.max_threshold_dd_r),
        threshold_dd_penalty=float(args.threshold_dd_penalty),
        min_threshold_trades=int(args.min_threshold_trades),
        iterations=int(args.iterations),
        depth=int(args.depth),
        learning_rate=float(args.learning_rate),
        l2_leaf_reg=float(args.l2_leaf_reg),
        random_seed=int(args.random_seed),
    )


def summarize_overlay(name: str, overlay: pd.DataFrame) -> dict[str, Any]:
    summary = exit_model.summarize_results(overlay["model_result_r"])
    baseline = exit_model.summarize_results(overlay["baseline_result_r"])
    model_exit_dates = pd.to_datetime(overlay["model_exit_date"], errors="coerce")
    baseline_exit_dates = pd.to_datetime(overlay["baseline_exit_date"], errors="coerce")
    return {
        "name": name,
        "model": summary,
        "rule_baseline": baseline,
        "model_exits": int(overlay["model_exit_reason"].astype(str).str.startswith("model_").sum()),
        "earlier_than_rule": int((model_exit_dates < baseline_exit_dates).fillna(False).sum()),
        "later_than_rule": int((model_exit_dates > baseline_exit_dates).fillna(False).sum()),
    }


def direct_terminal_overlay_for_trade(
    conn,
    trade: pd.Series,
    table_name: str,
    timeframe_minutes: int,
    args: argparse.Namespace,
) -> dict[str, Any] | None:
    entry_date = pd.Timestamp(trade.get("entry_date"))
    if pd.isna(entry_date):
        return None
    precomputed_result = exit_model.safe_float(trade.get("terminal_result_r"), float("nan"))
    precomputed_date = pd.Timestamp(trade.get("terminal_exit_date")) if not pd.isna(trade.get("terminal_exit_date")) else pd.NaT
    if math.isfinite(precomputed_result) and not pd.isna(precomputed_date):
        base = exit_model.source_overlay_row(trade)
        base.update(
            {
                "model_exit_date": precomputed_date,
                "model_exit_reason": str(trade.get("terminal_exit_reason") or "dynamic_terminal"),
                "model_result_r": precomputed_result,
                "delta_r": precomputed_result - exit_model.safe_float(trade.get("result_r")),
                "model_exit_price": trade.get("terminal_exit_price"),
                "exit_score_r": None,
                "model_hold_minutes": trade.get("terminal_hold_minutes"),
            }
        )
        return base

    symbol = str(trade.get("symbol") or "")
    direction = str(trade.get("direction") or "").upper()
    if not symbol or direction not in {"LONG", "SHORT"}:
        return None

    risk_points = exit_model.safe_float(trade.get("risk_points"))
    tick_size = exit_model.safe_float(trade.get("tick_size"))
    entry_price = exit_model.safe_float(trade.get("entry_price"))
    stop_price = exit_model.safe_float(trade.get("stop_price"))
    if risk_points <= 0 or tick_size <= 0 or entry_price <= 0:
        return None

    max_bars = max(2, int(args.dynamic_max_bars))
    end_at = entry_date + pd.Timedelta(minutes=timeframe_minutes * (max_bars + 2))
    candles = exit_model.load_candles(conn, table_name, symbol, entry_date, end_at)
    if candles.empty:
        return None
    entry_idx = exit_model.index_at_or_after(candles["ts_utc"], entry_date)
    if entry_idx is None:
        return None

    max_idx = min(len(candles) - 1, entry_idx + max_bars)
    hard_stop_idx = None
    for candle_idx in range(entry_idx, max_idx + 1):
        candle = candles.iloc[candle_idx]
        if direction == "LONG" and exit_model.safe_float(candle["low"]) <= stop_price:
            hard_stop_idx = candle_idx
            break
        if direction == "SHORT" and exit_model.safe_float(candle["high"]) >= stop_price:
            hard_stop_idx = candle_idx
            break

    if hard_stop_idx is not None:
        exit_idx = hard_stop_idx
        terminal_reason = "dynamic_hard_stop"
        terminal_price = stop_price
    else:
        exit_idx = max_idx
        terminal_reason = "dynamic_time_exit"
        terminal_price = exit_model.safe_float(candles["close"].iloc[exit_idx])
    terminal_date = candles["ts_utc"].iloc[exit_idx]

    sign = wave.direction_sign(direction)
    slippage_r = ((float(args.slippage_entry_ticks) + float(args.slippage_exit_ticks)) * tick_size) / risk_points
    terminal_result = sign * (terminal_price - entry_price) / risk_points - slippage_r
    base = exit_model.source_overlay_row(trade)
    model_hold = None
    if not pd.isna(terminal_date):
        model_hold = int((pd.Timestamp(terminal_date) - entry_date).total_seconds() / 60)
    base.update(
        {
            "model_exit_date": terminal_date,
            "model_exit_reason": terminal_reason,
            "model_result_r": terminal_result,
            "delta_r": terminal_result - exit_model.safe_float(trade.get("result_r")),
            "model_exit_price": terminal_price,
            "exit_score_r": None,
            "model_hold_minutes": model_hold,
        }
    )
    return base


def direct_terminal_overlay(
    conn,
    trades: pd.DataFrame,
    table_name: str,
    timeframe_minutes: int,
    args: argparse.Namespace,
    label: str,
) -> pd.DataFrame:
    rows: list[dict[str, Any]] = []
    total = len(trades)
    for index, (_, trade) in enumerate(trades.iterrows(), start=1):
        row = direct_terminal_overlay_for_trade(conn, trade, table_name, timeframe_minutes, args)
        if row is not None:
            rows.append(row)
        if index % 500 == 0:
            print(f"{label}: terminal rows for {index:,}/{total:,} trades ({len(rows):,} kept)", flush=True)
    frame = pd.DataFrame(rows)
    print(f"{label}: {len(frame):,} direct terminal rows from {total:,} trades", flush=True)
    return frame


def main() -> int:
    args = parse_args()
    scanner.configure_wave_args(args)
    directions = parse_directions(args.event_directions)
    valid_split = str(args.valid_split or args.valid_year)
    model_dir = stage2_dir(str(args.stage2_run_id))
    rid = run_id(args)
    output_dir = wave.ABCD_ROOT / "model_registry" / rid
    if output_dir.exists() and not args.replace_run:
        raise ValueError(f"Stage 3 run exists: {rid}. Use --replace-run.")
    output_dir.mkdir(parents=True, exist_ok=True)

    if args.terminal_only:
        train_events = pd.DataFrame()
        threshold_events = pd.DataFrame()
    else:
        train_events = load_stage2_events(model_dir, "train", int(args.max_train_events), int(args.random_seed), directions)
        threshold_events = load_stage2_events(
            model_dir,
            "threshold",
            int(args.max_threshold_events),
            int(args.random_seed) + 1,
            directions,
        )
    valid_events = load_stage2_events(
        model_dir,
        valid_split,
        int(args.max_valid_events),
        int(args.random_seed) + 2,
        directions,
    )
    print(
        f"Loaded Stage 2 confirmations train={len(train_events):,} "
        f"threshold={len(threshold_events):,} valid={len(valid_events):,}",
        flush=True,
    )

    conn = wave.connect()
    try:
        table_name, timeframe_minutes = scanner.table_for_timeframe(args.timeframe)
        manager_args = exit_args(args)
        if args.terminal_only:
            train_trades = pd.DataFrame()
            threshold_trades = pd.DataFrame()
            train_decisions = pd.DataFrame()
            threshold_decisions = pd.DataFrame()
        else:
            train_trades = build_trades(conn, train_events, 2025, args, "train")
            threshold_trades = build_trades(conn, threshold_events, 2025, args, "threshold")
            train_decisions = exit_model.build_decision_rows(
                conn,
                train_trades,
                table_name,
                timeframe_minutes,
                manager_args,
                True,
                "train",
            )
            threshold_decisions = exit_model.build_decision_rows(
                conn,
                threshold_trades,
                table_name,
                timeframe_minutes,
                manager_args,
                True,
                "threshold",
            )
        valid_trades = build_trades(conn, valid_events, int(args.valid_year), args, "valid")
        if args.terminal_only:
            valid_decisions = pd.DataFrame()
            terminal_overlay = direct_terminal_overlay(conn, valid_trades, table_name, timeframe_minutes, args, "valid")
        else:
            valid_decisions = exit_model.build_decision_rows(conn, valid_trades, table_name, timeframe_minutes, manager_args, False, "valid")
            terminal_overlay = pd.DataFrame()
    finally:
        conn.close()

    if args.terminal_only and terminal_overlay.empty:
        raise ValueError("Stage 3 terminal overlay is empty.")
    if not args.terminal_only and (valid_decisions.empty or train_decisions.empty or threshold_decisions.empty):
        raise ValueError("Stage 3 decision rows are empty for train, threshold, or valid.")

    if args.terminal_only:
        terminal_overlay.to_csv(output_dir / f"stage3_terminal_hold_trades_{args.valid_year}.csv", index=False)
        valid_trades.to_csv(output_dir / f"stage3_rule_baseline_trades_{args.valid_year}.csv", index=False)
        if args.save_decision_rows:
            valid_decisions.to_csv(output_dir / f"stage3_decisions_{args.valid_year}.csv", index=False)

        metadata = {
            "stage3_run_id": rid,
            "model_type": "stage3_stage2_confirmed_terminal_hold_validation",
            "stage2_run_id": str(args.stage2_run_id),
            "timeframe": args.timeframe,
            "valid_year": int(args.valid_year),
            "valid_split": valid_split,
            "event_directions": sorted(directions),
            "terminal_only": True,
            "entry_rule": "enter next candle open after Stage 2 confirm_date",
            "exit_policy": "hard stop or terminal window close; no dynamic AI exit overlay",
            "selected_exit_threshold": None,
            "dynamic_max_bars": int(args.dynamic_max_bars),
            "min_hold_bars": int(args.min_hold_bars),
            "decision_step_bars": int(args.decision_step_bars),
            "slippage_entry_ticks": float(args.slippage_entry_ticks),
            "slippage_exit_ticks": float(args.slippage_exit_ticks),
            "train": {
                "events": 0,
                "trades": 0,
                "decision_rows": 0,
            },
            "threshold": {
                "events": 0,
                "trades": 0,
                "decision_rows": 0,
                "summary": None,
            },
            str(args.valid_year): {
                "split": valid_split,
                "events": int(len(valid_events)),
                "trades": int(len(valid_trades)),
                "decision_rows": 0,
                "terminal_hold_summary": summarize_overlay("terminal_hold", terminal_overlay),
                "rule_baseline_summary": exit_model.summarize_results(valid_trades["result_r"]),
            },
            "features": {
                "cat": exit_model.EXIT_CAT_FEATURES,
                "num": exit_model.exit_num_features(manager_args),
            },
        }
        (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")

        print(json.dumps(metadata[str(args.valid_year)], indent=2, default=to_jsonable), flush=True)
        print(f"Saved Stage 3 terminal-hold validation: {rid}", flush=True)
        return 0

    print(
        f"Training Stage 3 trend manager decisions train={len(train_decisions):,} "
        f"threshold={len(threshold_decisions):,} valid={len(valid_decisions):,}",
        flush=True,
    )
    model = exit_model.train_exit_model(train_decisions, manager_args)
    train_decisions = exit_model.add_exit_predictions(train_decisions, model, manager_args)
    threshold_decisions = exit_model.add_exit_predictions(threshold_decisions, model, manager_args)
    valid_decisions = exit_model.add_exit_predictions(valid_decisions, model, manager_args)
    selected_threshold = exit_model.choose_exit_threshold(threshold_trades, threshold_decisions, manager_args)
    valid_overlay = exit_model.apply_overlay(valid_trades, valid_decisions, selected_threshold, manager_args)
    threshold_overlay = exit_model.apply_overlay(threshold_trades, threshold_decisions, selected_threshold, manager_args)
    terminal_overlay = exit_model.apply_overlay(valid_trades, valid_decisions, None, manager_args)

    model.save_model(str(output_dir / "catboost_stage3_trend_manager.cbm"))
    valid_overlay.to_csv(output_dir / f"stage3_trend_manager_trades_{args.valid_year}.csv", index=False)
    threshold_overlay.to_csv(output_dir / "stage3_trend_manager_trades_threshold.csv", index=False)
    terminal_overlay.to_csv(output_dir / f"stage3_terminal_hold_trades_{args.valid_year}.csv", index=False)
    valid_trades.to_csv(output_dir / f"stage3_rule_baseline_trades_{args.valid_year}.csv", index=False)
    if args.save_decision_rows:
        train_decisions.to_csv(output_dir / "stage3_decisions_train.csv", index=False)
        threshold_decisions.to_csv(output_dir / "stage3_decisions_threshold.csv", index=False)
        valid_decisions.to_csv(output_dir / f"stage3_decisions_{args.valid_year}.csv", index=False)

    metadata = {
        "stage3_run_id": rid,
        "model_type": "catboost_stage3_stage2_confirmed_trend_manager",
        "stage2_run_id": str(args.stage2_run_id),
        "timeframe": args.timeframe,
        "valid_year": int(args.valid_year),
        "valid_split": valid_split,
        "event_directions": sorted(directions),
        "entry_rule": "enter next candle open after Stage 2 confirm_date",
        "exit_policy": "AI dynamic trend manager; if no AI exit, hard-stop/time-window terminal path",
        "selected_exit_threshold": selected_threshold,
        "dynamic_max_bars": int(args.dynamic_max_bars),
        "min_hold_bars": int(args.min_hold_bars),
        "decision_step_bars": int(args.decision_step_bars),
        "slippage_entry_ticks": float(args.slippage_entry_ticks),
        "slippage_exit_ticks": float(args.slippage_exit_ticks),
        "train": {
            "events": int(len(train_events)),
            "trades": int(len(train_trades)),
            "decision_rows": int(len(train_decisions)),
        },
        "threshold": {
            "events": int(len(threshold_events)),
            "trades": int(len(threshold_trades)),
            "decision_rows": int(len(threshold_decisions)),
            "summary": summarize_overlay("threshold", threshold_overlay),
        },
        str(args.valid_year): {
            "split": valid_split,
            "events": int(len(valid_events)),
            "trades": int(len(valid_trades)),
            "decision_rows": int(len(valid_decisions)),
            "ai_summary": summarize_overlay(str(args.valid_year), valid_overlay),
            "terminal_hold_summary": summarize_overlay("terminal_hold", terminal_overlay),
            "rule_baseline_summary": exit_model.summarize_results(valid_trades["result_r"]),
        },
        "features": {
            "cat": exit_model.EXIT_CAT_FEATURES,
            "num": exit_model.exit_num_features(manager_args),
        },
    }
    (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")

    print(json.dumps(metadata[str(args.valid_year)], indent=2, default=to_jsonable), flush=True)
    print(f"Saved Stage 3 trend manager: {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
