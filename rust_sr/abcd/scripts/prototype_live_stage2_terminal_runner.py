#!/usr/bin/env python3
"""
Replay the current Stage 2 -> governed terminal strategy in a live-style loop.

This is a timing/live-paper harness, not a training script. It walks 2m candles
in chronological order, visits each configured symbol once per closed candle,
runs the Level 1/Level 2/Stage 2 reads, tracks open trades, and records whether
the scan pass fits inside the next 2m candle budget.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd

try:
    from catboost import CatBoostClassifier
except ImportError as exc:  # pragma: no cover
    raise SystemExit("CatBoost is required. Run with .venv_ai\\Scripts\\python.exe") from exc


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_scanner_model as scanner
import ai_oracle_start_level2_three_l1_manager as level2
import ai_oracle_start_lightgbm_utils as lgbm_utils
import ai_oracle_start_stage1_live_grid_model as cat_stage1
import ai_oracle_start_stage2_adaptive_confirmation as adaptive_stage2
import ai_oracle_start_stage2_l2_confirmation as stage2_l2
import ai_oracle_start_xgboost_utils as xgb_utils
import ai_oracle_trend_start_model as start_model
import ai_wave_rider_research as wave


DEFAULT_GOVERNED_RUN = "aicw-os-stage3-terminal-governed-clnq-gflong-v2"
DEFAULT_LEVEL2_RUN = "aicw-os-l2-three-l1-2m-p1p1n6m3-v1-ALL-2m-tr2025-v2026"
DEFAULT_STAGE2_RUN = "aicw-os-stage2-l2-confirm-v1-2m-m-tr2025-v2026-m16-v2026"


@dataclass
class PendingEvent:
    event: dict[str, Any]
    signal_idx: int


@dataclass
class ScheduledEntry:
    event: dict[str, Any]
    entry_idx: int
    stage2_score: float
    confirm_date: pd.Timestamp


@dataclass
class OpenPosition:
    trade_id: str
    event: dict[str, Any]
    root_symbol: str
    symbol: str
    direction: str
    entry_idx: int
    exit_idx: int
    entry_date: pd.Timestamp
    exit_date: pd.Timestamp
    entry_price: float
    stop_price: float
    exit_price: float
    risk_points: float
    risk_ticks: float
    result_r: float
    raw_result_r: float
    slippage_r: float
    exit_reason: str
    risk_amount: float
    reserved_amount: float
    cash_before_entry: float
    available_before_entry: float
    stage2_score: float
    confirm_date: pd.Timestamp


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--governed-run-id", default=DEFAULT_GOVERNED_RUN)
    parser.add_argument("--level2-run-id", default=DEFAULT_LEVEL2_RUN)
    parser.add_argument("--stage2-run-id", default=DEFAULT_STAGE2_RUN)
    parser.add_argument("--year", type=int, default=2026)
    parser.add_argument("--start", default="2026-04-01")
    parser.add_argument("--days", type=int, default=7)
    parser.add_argument("--timeframe", default="2m")
    parser.add_argument("--budget-ms", type=float, default=120_000.0)
    parser.add_argument("--max-cycles", type=int, default=0)
    parser.add_argument("--output-dir", default="")
    parser.add_argument("--replace-run", action="store_true")
    parser.add_argument("--progress-every-cycles", type=int, default=5000)

    # Live-paper account controls. Risk sizing uses realized cash only; open
    # trade risk is reserved and cannot be reused by later entries.
    parser.add_argument("--starting-cash", type=float, default=1000.0)
    parser.add_argument("--risk-pct", type=float, default=0.01)
    parser.add_argument("--allow-symbol-overlap", action="store_true")
    parser.add_argument("--allowed-root-directions", default="")
    parser.add_argument("--allowed-entry-hours", default="")
    parser.add_argument("--blocked-entry-hours", default="")
    parser.add_argument("--min-stage2-score-filter", type=float, default=None)
    parser.add_argument("--risk-ticks-min-filter", type=float, default=None)
    parser.add_argument("--risk-ticks-max-filter", type=float, default=None)
    parser.add_argument("--max-open-positions", type=int, default=0)
    parser.add_argument("--daily-loss-pct-stop", type=float, default=0.0)
    parser.add_argument("--max-daily-closed-losses", type=int, default=0)

    # Candle feature defaults used by the current model family.
    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=240.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    parser.add_argument("--max-forward-bars", type=int, default=576)
    parser.add_argument("--time-exit-bars", type=int, default=0)
    parser.add_argument("--dynamic-max-bars", type=int, default=180)
    parser.add_argument("--slippage-entry-ticks", type=float, default=3.0)
    parser.add_argument("--slippage-exit-ticks", type=float, default=3.0)
    return parser.parse_args()


def load_json(path: Path) -> dict[str, Any]:
    if not path.exists():
        raise FileNotFoundError(f"Missing JSON: {path}")
    return json.loads(path.read_text(encoding="utf-8"))


def model_dir(run_id: str) -> Path:
    path = Path(run_id)
    if path.exists():
        return path.resolve()
    return wave.ABCD_ROOT / "model_registry" / run_id


def to_jsonable(value: Any) -> Any:
    if isinstance(value, pd.Timestamp):
        return None if pd.isna(value) else value.isoformat()
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def load_catboost_model(path: Path) -> CatBoostClassifier:
    model = CatBoostClassifier()
    model.load_model(str(path))
    return model


def parse_csv_strings(value: str) -> set[str]:
    return {part.strip().upper() for part in str(value or "").split(",") if part.strip()}


def parse_csv_ints(value: str) -> set[int]:
    out = set()
    for part in str(value or "").split(","):
        part = part.strip()
        if part:
            out.add(int(part))
    return out


def fetch_candles(conn, timeframe: str, roots: list[str], start: pd.Timestamp, end: pd.Timestamp) -> pd.DataFrame:
    table_name, _minutes = scanner.table_for_timeframe(timeframe)
    table = scanner.safe_identifier(table_name)
    params: list[Any] = [
        (start - pd.Timedelta(days=3)).to_pydatetime(),
        (end + pd.Timedelta(days=3)).to_pydatetime(),
        *roots,
    ]
    root_sql = ",".join(["%s"] * len(roots))
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT root_symbol, symbol, ts_utc,
                   CAST(open AS DOUBLE) AS open,
                   CAST(high AS DOUBLE) AS high,
                   CAST(low AS DOUBLE) AS low,
                   CAST(close AS DOUBLE) AS close,
                   CAST(volume AS DOUBLE) AS volume
            FROM {table}
            WHERE ts_utc >= %s
              AND ts_utc < %s
              AND root_symbol IN ({root_sql})
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
    return frame.dropna(subset=["root_symbol", "symbol", "ts_utc", "open", "high", "low", "close"]).reset_index(drop=True)


def score_level1_and_level2(
    base_rows: list[dict[str, Any]],
    models: dict[str, Any],
    l2_args: argparse.Namespace,
    timeframe: str,
) -> pd.DataFrame:
    if not base_rows:
        return pd.DataFrame()
    base = pd.DataFrame(base_rows)
    base = level2.add_timeframe_columns(base, timeframe)

    scored = base.copy()
    scored["cat_l1_score"] = models["cat"].predict_proba(start_model.prepare_pool(base, include_target=False))[:, 1]
    light_model, light_cat, light_num, light_maps = models["light"]
    scored["light_l1_score"] = lgbm_utils.predict_scores(base, light_model, light_cat, light_num, light_maps)
    xgb_model, xgb_cat, xgb_num, xgb_maps = models["xgb"]
    scored["xgb_l1_score"] = xgb_utils.predict_scores(base, xgb_model, xgb_cat, xgb_num, xgb_maps)
    scored = level2.add_l2_features(scored, l2_args)
    scored["level2_score"] = models["level2"].predict_proba(level2.prepare_pool(scored, include_target=False, args=l2_args))[:, 1]
    return scored


def stage2_rows_for_pending(
    pending: list[PendingEvent],
    symbol_groups: dict[str, pd.DataFrame],
    current_indices: dict[str, int],
    max_confirm_bars: int,
) -> pd.DataFrame:
    rows: list[dict[str, Any]] = []
    for item in pending:
        original = item.event
        symbol = str(original["symbol"])
        current_idx = current_indices.get(symbol)
        if current_idx is None:
            continue
        offset = int(current_idx) - int(item.signal_idx)
        if offset < 1 or offset > int(max_confirm_bars):
            continue
        group = symbol_groups[symbol]
        tick_size = wave.tick_size_for(symbol, str(original.get("root_symbol") or ""))
        atr_ticks = wave.finite(original.get("atr_ticks"), None)
        atr_hint = atr_ticks * tick_size if atr_ticks is not None and tick_size > 0 else None
        features = adaptive_stage2.post_features_for_offset(
            group,
            int(item.signal_idx),
            str(original["direction"]),
            offset,
            int(max_confirm_bars),
            atr_hint,
        )
        if features is None:
            continue
        payload = dict(original)
        payload.update(features)
        rows.append(payload)
    return pd.DataFrame(rows)


def direction_for_root(root: str, allowed_root_directions: set[str]) -> list[str]:
    out = []
    for direction in ["LONG", "SHORT"]:
        if f"{root}_{direction}" in allowed_root_directions:
            out.append(direction)
    return out


def summarize(values: list[float]) -> dict[str, float]:
    if not values:
        return {"count": 0, "mean_ms": 0.0, "p50_ms": 0.0, "p95_ms": 0.0, "p99_ms": 0.0, "max_ms": 0.0}
    arr = np.asarray(values, dtype=float)
    return {
        "count": int(len(arr)),
        "mean_ms": float(arr.mean()),
        "p50_ms": float(np.percentile(arr, 50)),
        "p95_ms": float(np.percentile(arr, 95)),
        "p99_ms": float(np.percentile(arr, 99)),
        "max_ms": float(arr.max()),
    }


def terminal_path_from_entry(
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
        high = wave.finite(candle.get("high"), None)
        low = wave.finite(candle.get("low"), None)
        if high is None or low is None:
            continue
        if direction == "LONG" and float(low) <= stop_price:
            hard_stop_idx = candle_idx
            break
        if direction == "SHORT" and float(high) >= stop_price:
            hard_stop_idx = candle_idx
            break

    if hard_stop_idx is not None:
        exit_idx = int(hard_stop_idx)
        exit_price = float(stop_price)
        exit_reason = "dynamic_hard_stop"
    else:
        exit_idx = int(max_idx)
        exit_price = wave.finite(candles["close"].iloc[exit_idx], None)
        if exit_price is None:
            return None
        exit_price = float(exit_price)
        exit_reason = "dynamic_time_exit"

    sign = wave.direction_sign(direction)
    slippage_r = ((float(args.slippage_entry_ticks) + float(args.slippage_exit_ticks)) * tick_size) / risk_points
    raw_result_r = sign * (float(exit_price) - float(entry_price)) / float(risk_points)
    result_r = raw_result_r - slippage_r
    return {
        "exit_idx": exit_idx,
        "exit_date": pd.Timestamp(candles["ts_utc"].iloc[exit_idx]),
        "exit_price": float(exit_price),
        "exit_reason": exit_reason,
        "raw_result_r": float(raw_result_r),
        "result_r": float(result_r),
        "slippage_r": float(slippage_r),
    }


def drawdown_from_curve(values: list[float]) -> float:
    peak = None
    max_dd = 0.0
    for value in values:
        value = float(value)
        peak = value if peak is None else max(peak, value)
        max_dd = max(max_dd, peak - value)
    return float(max_dd)


def main() -> int:
    args = parse_args()
    scanner.configure_wave_args(args)
    start_at = pd.Timestamp(args.start)
    end_at = start_at + pd.Timedelta(days=int(args.days))

    governed_dir = model_dir(args.governed_run_id)
    governed = load_json(governed_dir / "metadata.json")
    rules = governed["rules"]
    allowed_root_directions = set(str(item) for item in rules["root_directions"])
    override_root_directions = parse_csv_strings(args.allowed_root_directions)
    if override_root_directions:
        allowed_root_directions = override_root_directions
    roots = sorted({item.split("_", 1)[0] for item in allowed_root_directions})
    min_stage2_score = float(rules["min_stage2_score"])
    if args.min_stage2_score_filter is not None:
        min_stage2_score = max(min_stage2_score, float(args.min_stage2_score_filter))
    risk_min = float(rules["risk_ticks_min"])
    risk_max = float(rules["risk_ticks_max"])
    if args.risk_ticks_min_filter is not None:
        risk_min = max(risk_min, float(args.risk_ticks_min_filter))
    if args.risk_ticks_max_filter is not None:
        risk_max = min(risk_max, float(args.risk_ticks_max_filter))
    max_trades_per_day_per_direction = int(rules["max_trades_per_day_per_direction"])
    cooldown = pd.Timedelta(minutes=int(rules["symbol_direction_cooldown_minutes"]))
    allowed_entry_hours = parse_csv_ints(args.allowed_entry_hours)
    blocked_entry_hours = parse_csv_ints(args.blocked_entry_hours)
    max_open_positions_filter = max(0, int(args.max_open_positions or 0))
    daily_loss_pct_stop = max(0.0, float(args.daily_loss_pct_stop or 0.0))
    max_daily_closed_losses = max(0, int(args.max_daily_closed_losses or 0))

    l2_dir = model_dir(args.level2_run_id)
    l2_meta = load_json(l2_dir / "metadata.json")
    l2_run_ids = l2_meta["level1_model_runs"]
    l2_threshold = float(l2_meta["selected_threshold"])
    l2_args = argparse.Namespace(
        manager_engines="cat,light,xgb",
        cat_l1_run_id=l2_run_ids["catboost"],
        light_l1_run_id=l2_run_ids["lightgbm"],
        xgb_l1_run_id=l2_run_ids["xgboost"],
        cat_l1_prefix="",
        light_l1_prefix="",
        xgb_l1_prefix="",
        timeframe=args.timeframe,
        roots="",
        level1_train_years="2024",
        level1_valid_year=int(args.year),
    )

    stage2_dir = model_dir(args.stage2_run_id)
    stage2_meta = load_json(stage2_dir / "metadata.json")
    stage2_threshold = float(stage2_meta["selected_stage2_threshold"])
    max_confirm_bars = int(stage2_meta["max_confirm_bars"])

    models = {
        "cat": load_catboost_model(
            model_dir(l2_run_ids["catboost"]) / "catboost_oracle_start_live_grid_model.cbm"
        ),
        "light": lgbm_utils.load_model(model_dir(l2_run_ids["lightgbm"])),
        "xgb": xgb_utils.load_model(model_dir(l2_run_ids["xgboost"])),
        "level2": load_catboost_model(l2_dir / "catboost_oracle_start_level2_three_l1_manager.cbm"),
        "stage2": load_catboost_model(stage2_dir / "catboost_stage2_l2_confirmation.cbm"),
    }

    load_t0 = time.perf_counter()
    conn = wave.connect()
    try:
        raw = fetch_candles(conn, args.timeframe, roots, start_at, end_at)
    finally:
        conn.close()
    if raw.empty:
        raise ValueError(f"No candles found for roots={roots} {start_at} to {end_at}")

    symbol_groups: dict[str, pd.DataFrame] = {}
    symbol_roots: dict[str, str] = {}
    time_to_indices: dict[pd.Timestamp, list[tuple[str, int]]] = {}
    for symbol, group_raw in raw.groupby("symbol", sort=True):
        root = str(group_raw["root_symbol"].iloc[0])
        enriched = scanner.enrich_candles(
            group_raw[["ts_utc", "open", "high", "low", "close", "volume"]].copy().sort_values("ts_utc").reset_index(drop=True),
            args,
        )
        times = pd.to_datetime(enriched["ts_utc"], errors="coerce")
        if not ((times >= start_at) & (times < end_at)).any():
            continue
        symbol = str(symbol)
        symbol_groups[symbol] = enriched
        symbol_roots[symbol] = root
        for idx, ts in enumerate(times):
            ts = pd.Timestamp(ts)
            if start_at <= ts < end_at:
                time_to_indices.setdefault(ts, []).append((symbol, int(idx)))
    load_seconds = time.perf_counter() - load_t0

    timestamps = sorted(time_to_indices)
    if int(args.max_cycles) > 0:
        timestamps = timestamps[: int(args.max_cycles)]

    pending: dict[str, PendingEvent] = {}
    scheduled_entries: dict[tuple[str, int, str], ScheduledEntry] = {}
    next_allowed_stage1: dict[tuple[str, str], int] = {}
    entry_day_direction_count: dict[tuple[str, str], int] = {}
    entry_last_symbol_direction: dict[tuple[str, str], pd.Timestamp] = {}

    cycles: list[dict[str, Any]] = []
    accepted_entries: list[dict[str, Any]] = []
    stage1_rows_total = 0
    level2_events_total = 0
    stage2_rows_total = 0
    stage2_confirms_total = 0
    rejected_risk = 0
    rejected_governor = 0
    rejected_cash = 0
    rejected_overlap = 0

    starting_cash = float(args.starting_cash)
    risk_pct = float(args.risk_pct)
    if starting_cash <= 0:
        raise ValueError("--starting-cash must be positive")
    if risk_pct <= 0 or risk_pct > 1:
        raise ValueError("--risk-pct must be > 0 and <= 1")

    cash_balance = starting_cash
    reserved_risk = 0.0
    max_reserved_risk = 0.0
    max_open_positions = 0
    min_available_cash = starting_cash
    open_positions: dict[str, OpenPosition] = {}
    open_symbols: set[str] = set()
    pending_exits: dict[tuple[str, int], list[OpenPosition]] = {}
    account_events: list[dict[str, Any]] = []
    trade_records: list[dict[str, Any]] = []
    closed_balance_curve: list[float] = [starting_cash]
    exits_this_cycle = 0
    day_start_cash: dict[str, float] = {}
    day_closed_losses: dict[str, int] = {}
    stopped_days: set[str] = set()

    def ensure_account_day(ts_value: Any) -> str:
        day_key = str(pd.Timestamp(ts_value).date())
        if day_key not in day_start_cash:
            day_start_cash[day_key] = float(cash_balance)
            day_closed_losses[day_key] = 0
        return day_key

    def close_position(position: OpenPosition) -> bool:
        nonlocal cash_balance, reserved_risk, exits_this_cycle
        if position.trade_id not in open_positions:
            return False
        pnl = position.risk_amount * position.result_r
        cash_before_exit = cash_balance
        cash_balance += pnl
        reserved_risk = max(0.0, reserved_risk - position.reserved_amount)
        exit_day_key = ensure_account_day(position.exit_date)
        if position.result_r <= 0:
            day_closed_losses[exit_day_key] = day_closed_losses.get(exit_day_key, 0) + 1
            if max_daily_closed_losses > 0 and day_closed_losses[exit_day_key] >= max_daily_closed_losses:
                stopped_days.add(exit_day_key)
        if daily_loss_pct_stop > 0 and cash_balance <= day_start_cash[exit_day_key] * (1.0 - daily_loss_pct_stop):
            stopped_days.add(exit_day_key)
        open_positions.pop(position.trade_id, None)
        if not any(pos.symbol == position.symbol for pos in open_positions.values()):
            open_symbols.discard(position.symbol)
        exits_this_cycle += 1
        closed_balance_curve.append(float(cash_balance))
        trade_record = {
            "trade_id": position.trade_id,
            "root_symbol": position.root_symbol,
            "symbol": position.symbol,
            "direction": position.direction,
            "entry_date": position.entry_date,
            "exit_date": position.exit_date,
            "entry_price": position.entry_price,
            "stop_price": position.stop_price,
            "exit_price": position.exit_price,
            "exit_reason": position.exit_reason,
            "risk_points": position.risk_points,
            "risk_ticks": position.risk_ticks,
            "result_r": position.result_r,
            "raw_result_r": position.raw_result_r,
            "slippage_r": position.slippage_r,
            "risk_amount": position.risk_amount,
            "reserved_amount": position.reserved_amount,
            "pnl": pnl,
            "cash_before_entry": position.cash_before_entry,
            "available_before_entry": position.available_before_entry,
            "cash_before_exit": cash_before_exit,
            "cash_after_exit": cash_balance,
            "stage2_score": position.stage2_score,
            "confirm_date": position.confirm_date,
            "candidate_uid": position.event.get("candidate_uid"),
        }
        trade_records.append(trade_record)
        account_events.append(
            {
                "event": "exit",
                "ts": position.exit_date,
                "trade_id": position.trade_id,
                "symbol": position.symbol,
                "direction": position.direction,
                "cash_balance": cash_balance,
                "reserved_risk": reserved_risk,
                "available_cash": cash_balance - reserved_risk,
                "pnl": pnl,
                "result_r": position.result_r,
            }
        )
        return True

    for cycle_index, ts in enumerate(timestamps, start=1):
        t0 = time.perf_counter()
        current_items = sorted(time_to_indices[ts])
        current_indices = {symbol: idx for symbol, idx in current_items}
        exits_this_cycle = 0
        entries_this_cycle = 0

        # Exit fills are only applied when the replay reaches the exit candle.
        for symbol, idx in current_items:
            for position in pending_exits.pop((symbol, idx), []):
                close_position(position)

        # Entries are checked at the next candle when the entry open is known.
        for symbol, idx in current_items:
            for direction in direction_for_root(symbol_roots[symbol], allowed_root_directions):
                scheduled = scheduled_entries.pop((symbol, idx, direction), None)
                if scheduled is None:
                    continue
                group = symbol_groups[symbol]
                tick_size = wave.tick_size_for(symbol, symbol_roots[symbol])
                stop_price, risk_points = wave.initial_stop(group, idx, direction, tick_size, args)
                entry_price = wave.finite(group["open"].iloc[idx], None)
                if stop_price is None or risk_points is None or entry_price is None or tick_size <= 0:
                    rejected_risk += 1
                    continue
                risk_ticks = risk_points / tick_size
                if risk_ticks < risk_min or risk_ticks > risk_max:
                    rejected_risk += 1
                    continue
                if not bool(args.allow_symbol_overlap) and symbol in open_symbols:
                    rejected_overlap += 1
                    continue
                entry_date = pd.Timestamp(group["ts_utc"].iloc[idx])
                if allowed_entry_hours and int(entry_date.hour) not in allowed_entry_hours:
                    rejected_governor += 1
                    continue
                if blocked_entry_hours and int(entry_date.hour) in blocked_entry_hours:
                    rejected_governor += 1
                    continue
                entry_account_day = ensure_account_day(entry_date)
                if entry_account_day in stopped_days:
                    rejected_governor += 1
                    continue
                if max_open_positions_filter > 0 and len(open_positions) >= max_open_positions_filter:
                    rejected_governor += 1
                    continue
                day_key = str(entry_date.date())
                dir_key = (day_key, direction)
                symbol_dir_key = (symbol, direction)
                if entry_day_direction_count.get(dir_key, 0) >= max_trades_per_day_per_direction:
                    rejected_governor += 1
                    continue
                last_entry = entry_last_symbol_direction.get(symbol_dir_key)
                if last_entry is not None and entry_date - last_entry < cooldown:
                    rejected_governor += 1
                    continue

                terminal = terminal_path_from_entry(
                    group,
                    idx,
                    direction,
                    float(entry_price),
                    float(stop_price),
                    float(risk_points),
                    float(tick_size),
                    args,
                )
                if terminal is None:
                    rejected_risk += 1
                    continue

                risk_amount = cash_balance * risk_pct
                reserved_amount = risk_amount * max(1.0, 1.0 + float(terminal["slippage_r"]))
                available_cash = cash_balance - reserved_risk
                min_available_cash = min(min_available_cash, available_cash)
                if cash_balance <= 0 or risk_amount <= 0 or available_cash < reserved_amount:
                    rejected_cash += 1
                    continue

                trade_id = f"paper-{int(cycle_index):06d}-{len(accepted_entries) + 1:06d}"
                position = OpenPosition(
                    trade_id=trade_id,
                    event=scheduled.event,
                    root_symbol=symbol_roots[symbol],
                    symbol=symbol,
                    direction=direction,
                    entry_idx=int(idx),
                    exit_idx=int(terminal["exit_idx"]),
                    entry_date=entry_date,
                    exit_date=pd.Timestamp(terminal["exit_date"]),
                    entry_price=float(entry_price),
                    stop_price=float(stop_price),
                    exit_price=float(terminal["exit_price"]),
                    risk_points=float(risk_points),
                    risk_ticks=float(risk_ticks),
                    result_r=float(terminal["result_r"]),
                    raw_result_r=float(terminal["raw_result_r"]),
                    slippage_r=float(terminal["slippage_r"]),
                    exit_reason=str(terminal["exit_reason"]),
                    risk_amount=float(risk_amount),
                    reserved_amount=float(reserved_amount),
                    cash_before_entry=float(cash_balance),
                    available_before_entry=float(available_cash),
                    stage2_score=float(scheduled.stage2_score),
                    confirm_date=scheduled.confirm_date,
                )

                reserved_risk += reserved_amount
                max_reserved_risk = max(max_reserved_risk, reserved_risk)
                open_positions[trade_id] = position
                open_symbols.add(symbol)
                if position.exit_date < end_at:
                    pending_exits.setdefault((symbol, position.exit_idx), []).append(position)
                max_open_positions = max(max_open_positions, len(open_positions))
                entries_this_cycle += 1
                entry_day_direction_count[dir_key] = entry_day_direction_count.get(dir_key, 0) + 1
                entry_last_symbol_direction[symbol_dir_key] = entry_date
                accepted_entries.append(
                    {
                        "trade_id": trade_id,
                        "entry_date": entry_date,
                        "planned_exit_date": position.exit_date,
                        "root_symbol": symbol_roots[symbol],
                        "symbol": symbol,
                        "direction": direction,
                        "entry_price": float(entry_price),
                        "stop_price": float(stop_price),
                        "planned_exit_price": position.exit_price,
                        "planned_exit_reason": position.exit_reason,
                        "risk_ticks": float(risk_ticks),
                        "risk_amount": float(risk_amount),
                        "reserved_amount": float(reserved_amount),
                        "cash_before_entry": float(cash_balance),
                        "reserved_risk_after_entry": float(reserved_risk),
                        "available_cash_after_entry": float(cash_balance - reserved_risk),
                        "planned_result_r": position.result_r,
                        "stage2_score": float(scheduled.stage2_score),
                        "confirm_date": scheduled.confirm_date,
                        "candidate_uid": scheduled.event.get("candidate_uid"),
                    }
                )
                account_events.append(
                    {
                        "event": "entry",
                        "ts": entry_date,
                        "trade_id": trade_id,
                        "symbol": symbol,
                        "direction": direction,
                        "cash_balance": cash_balance,
                        "reserved_risk": reserved_risk,
                        "available_cash": cash_balance - reserved_risk,
                        "risk_amount": risk_amount,
                        "reserved_amount": reserved_amount,
                    }
                )

        # Same-candle exits are known only after all entries for this timestamp
        # are admitted, so they release cash at the end of the candle cycle.
        for symbol, idx in current_items:
            for position in pending_exits.pop((symbol, idx), []):
                close_position(position)

        base_rows: list[dict[str, Any]] = []
        for symbol, current_idx in current_items:
            root = symbol_roots[symbol]
            directions = direction_for_root(root, allowed_root_directions)
            if not directions:
                continue
            signal_idx = current_idx - 1
            group = symbol_groups[symbol]
            if signal_idx < 60 or signal_idx >= len(group) - 1:
                continue
            for direction in directions:
                row = start_model.feature_row(int(args.year), root, symbol, group, signal_idx, direction)
                row["signal_idx"] = int(signal_idx)
                row["source_timeframe"] = str(args.timeframe)
                row["timeframe_minutes"] = float(scanner.table_for_timeframe(args.timeframe)[1])
                base_rows.append(row)

        l2_events = pd.DataFrame()
        if base_rows:
            scored = score_level1_and_level2(base_rows, models, l2_args, args.timeframe)
            stage1_rows_total += len(scored)
            for row in scored.to_dict("records"):
                score = wave.finite(row.get("level2_score"), 0.0) or 0.0
                if score < l2_threshold:
                    continue
                key = (str(row["symbol"]), str(row["direction"]))
                signal_idx = int(row["signal_idx"])
                if signal_idx < next_allowed_stage1.get(key, -1):
                    continue
                next_allowed_stage1[key] = signal_idx + int(l2_meta["event_rules"]["cooldown_bars"]) + 1
                row["stage1_pick"] = 1
                row["stage1_score"] = score
                pending[str(row["candidate_uid"])] = PendingEvent(event=row, signal_idx=signal_idx)
                level2_events_total += 1
            l2_events = scored

        # Score pending Stage 2 rows after adding any offset-1 events from this same closed candle.
        pending_rows = stage2_rows_for_pending(list(pending.values()), symbol_groups, current_indices, max_confirm_bars)
        confirmed_uids: set[str] = set()
        if not pending_rows.empty:
            stage2_rows_total += len(pending_rows)
            pending_rows["stage2_score"] = models["stage2"].predict_proba(stage2_l2.prepare_pool(pending_rows, include_target=False))[:, 1]
            hits = pending_rows[
                pd.to_numeric(pending_rows["stage2_score"], errors="coerce").fillna(0.0) >= stage2_threshold
            ].copy()
            for row in hits.sort_values(["candidate_uid", "confirm_offset_bars"]).drop_duplicates("candidate_uid").to_dict("records"):
                confirmed_uids.add(str(row["candidate_uid"]))
                stage2_confirms_total += 1
                stage2_score = wave.finite(row.get("stage2_score"), 0.0) or 0.0
                if stage2_score < min_stage2_score:
                    rejected_governor += 1
                    continue
                symbol = str(row["symbol"])
                direction = str(row["direction"])
                group = symbol_groups[symbol]
                current_idx = current_indices.get(symbol)
                if current_idx is None:
                    continue
                entry_idx = current_idx + 1
                if entry_idx >= len(group):
                    rejected_risk += 1
                    continue
                scheduled_entries[(symbol, entry_idx, direction)] = ScheduledEntry(
                    event=row,
                    entry_idx=entry_idx,
                    stage2_score=float(stage2_score),
                    confirm_date=pd.Timestamp(row.get("confirm_date") or ts),
                )

        # Drop confirmed or expired pending events.
        expired = []
        for uid, item in pending.items():
            current_idx = current_indices.get(str(item.event["symbol"]))
            if uid in confirmed_uids:
                expired.append(uid)
            elif current_idx is not None and current_idx - item.signal_idx >= max_confirm_bars:
                expired.append(uid)
        for uid in expired:
            pending.pop(uid, None)

        elapsed_ms = (time.perf_counter() - t0) * 1000.0
        cycles.append(
            {
                "cycle": int(cycle_index),
                "ts": ts,
                "elapsed_ms": float(elapsed_ms),
                "symbols_seen": int(len(current_items)),
                "candidate_rows": int(len(base_rows)),
                "level2_events_total": int(level2_events_total),
                "pending_events": int(len(pending)),
                "stage2_rows": int(len(pending_rows)),
                "accepted_entries_total": int(len(accepted_entries)),
                "entries_this_cycle": int(entries_this_cycle),
                "exits_this_cycle": int(exits_this_cycle),
                "open_positions": int(len(open_positions)),
                "cash_balance": float(cash_balance),
                "reserved_risk": float(reserved_risk),
                "available_cash": float(cash_balance - reserved_risk),
            }
        )
        if int(args.progress_every_cycles) > 0 and cycle_index % int(args.progress_every_cycles) == 0:
            print(
                json.dumps(
                    {
                        "progress": int(cycle_index),
                        "cycles_total": int(len(timestamps)),
                        "ts": to_jsonable(ts),
                        "cash_balance": float(cash_balance),
                        "open_positions": int(len(open_positions)),
                        "closed_trades": int(len(trade_records)),
                        "avg_cycle_ms_so_far": float(np.mean([float(row["elapsed_ms"]) for row in cycles])),
                    }
                ),
                flush=True,
            )

    elapsed_values = [float(row["elapsed_ms"]) for row in cycles]
    over_budget = [value for value in elapsed_values if value > float(args.budget_ms)]
    closed_pnl = float(cash_balance - starting_cash)
    closed_trade_rs = [float(row["result_r"]) for row in trade_records]
    winning_trades = [value for value in closed_trade_rs if value > 0]
    losing_trades = [value for value in closed_trade_rs if value <= 0]
    account_summary = {
        "starting_cash": float(starting_cash),
        "ending_closed_cash": float(cash_balance),
        "closed_pnl": closed_pnl,
        "closed_return_pct": float((closed_pnl / starting_cash) * 100.0) if starting_cash else 0.0,
        "risk_pct_per_trade": float(risk_pct),
        "open_positions_end": int(len(open_positions)),
        "reserved_risk_end": float(reserved_risk),
        "available_cash_end": float(cash_balance - reserved_risk),
        "max_reserved_risk": float(max_reserved_risk),
        "max_open_positions": int(max_open_positions),
        "min_available_cash": float(min(min_available_cash, cash_balance - reserved_risk)),
        "closed_trades": int(len(trade_records)),
        "accepted_entries": int(len(accepted_entries)),
        "closed_sum_r": float(sum(closed_trade_rs)) if closed_trade_rs else 0.0,
        "closed_avg_r": float(np.mean(closed_trade_rs)) if closed_trade_rs else 0.0,
        "closed_win_rate": float(len(winning_trades) / len(closed_trade_rs)) if closed_trade_rs else 0.0,
        "closed_losses": int(len(losing_trades)),
        "closed_wins": int(len(winning_trades)),
        "closed_balance_max_drawdown": drawdown_from_curve(closed_balance_curve),
        "rejected_cash": int(rejected_cash),
        "rejected_overlap": int(rejected_overlap),
        "sizing_read": (
            "Risk dollars are sized from realized closed cash only. Open positions reserve worst-stop cash "
            "including slippage and that reserved cash cannot be reused by later entries."
        ),
    }
    summary = {
        "run_type": "prototype_live_stage2_terminal_runner",
        "governed_run_id": args.governed_run_id,
        "level2_run_id": args.level2_run_id,
        "stage2_run_id": args.stage2_run_id,
        "year": int(args.year),
        "start": start_at,
        "end": end_at,
        "timeframe": args.timeframe,
        "budget_ms": float(args.budget_ms),
        "load_seconds": float(load_seconds),
        "roots": roots,
        "symbols": int(len(symbol_groups)),
        "cycles": int(len(cycles)),
        "first_cycle_ts": timestamps[0] if timestamps else None,
        "last_cycle_ts": timestamps[-1] if timestamps else None,
        "cycle_timing": summarize(elapsed_values),
        "cycles_over_budget": int(len(over_budget)),
        "max_budget_share": float(max(elapsed_values) / float(args.budget_ms)) if elapsed_values else 0.0,
        "stage1_rows_total": int(stage1_rows_total),
        "level2_events_total": int(level2_events_total),
        "stage2_rows_total": int(stage2_rows_total),
        "stage2_confirms_total": int(stage2_confirms_total),
        "accepted_entries": int(len(accepted_entries)),
        "closed_trades": int(len(trade_records)),
        "rejected_risk": int(rejected_risk),
        "rejected_governor": int(rejected_governor),
        "rejected_cash": int(rejected_cash),
        "rejected_overlap": int(rejected_overlap),
        "account": account_summary,
        "governor_overrides": {
            "allowed_root_directions": sorted(allowed_root_directions),
            "allowed_entry_hours": sorted(allowed_entry_hours),
            "blocked_entry_hours": sorted(blocked_entry_hours),
            "min_stage2_score": float(min_stage2_score),
            "risk_ticks_min": float(risk_min),
            "risk_ticks_max": float(risk_max),
            "max_open_positions": int(max_open_positions_filter),
            "daily_loss_pct_stop": float(daily_loss_pct_stop),
            "max_daily_closed_losses": int(max_daily_closed_losses),
            "allow_symbol_overlap": bool(args.allow_symbol_overlap),
        },
        "read": (
            "Each cycle represents a closed 2m candle timestamp. Stage 1 is scored one candle after "
            "the signal candle because current feature_row needs the next candle for risk hints; entries "
            "are then checked on the next candle open. Account sizing uses closed realized cash only."
        ),
    }

    output_dir = Path(args.output_dir).expanduser().resolve() if str(args.output_dir or "").strip() else (
        wave.ABCD_ROOT / "model_registry" / "prototype-live-stage2-terminal-runner"
    )
    if output_dir.exists() and not args.replace_run:
        raise ValueError(f"Output exists: {output_dir}. Use --replace-run.")
    output_dir.mkdir(parents=True, exist_ok=True)
    pd.DataFrame(cycles).to_csv(output_dir / "cycle_timing.csv", index=False)
    pd.DataFrame(accepted_entries).to_csv(output_dir / "accepted_entries.csv", index=False)
    pd.DataFrame(trade_records).to_csv(output_dir / "paper_closed_trades.csv", index=False)
    pd.DataFrame(account_events).to_csv(output_dir / "paper_account_events.csv", index=False)
    (output_dir / "metadata.json").write_text(json.dumps(summary, indent=2, default=to_jsonable), encoding="utf-8")
    print(json.dumps(summary, indent=2, default=to_jsonable), flush=True)
    print(f"Saved live prototype timing run: {output_dir}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
