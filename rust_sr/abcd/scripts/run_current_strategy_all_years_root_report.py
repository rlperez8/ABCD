#!/usr/bin/env python3
"""
Run the current live-style 2m strategy across every available year/root.

Current strategy chain:
    L1 Cat/Light/XGB 2m all-root models
    -> L2 trio manager
    -> Stage 2 confirmation model
    -> Stage 3 terminal hold with fixed hard stop / 180-bar window

The script is intentionally a runner/report harness. It does not retrain the
current models. It scores each requested year, writes per-year trade files, then
exports Excel-friendly root/symbol summaries including zero-trade roots/symbols
from the candle universe.
"""

from __future__ import annotations

import argparse
import gc
import json
import math
import re
import sys
import time
from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_exit_model as exit_model
import ai_candle_wave_scanner_model as scanner
import ai_oracle_stage3_trend_manager as stage3
import ai_oracle_start_level2_three_l1_manager as level2
import ai_oracle_start_stage2_l2_confirmation as stage2_l2
import ai_wave_rider_research as wave


DEFAULT_LEVEL2_RUN = "aicw-os-l2-three-l1-2m-p1p1n6m3-v1-ALL-2m-tr2025-v2026"
DEFAULT_STAGE2_RUN = "aicw-os-stage2-l2-confirm-v1-2m-m-tr2025-v2026-m16-v2026"
DEFAULT_RUN_ID = "current-strategy-allroots-2m-allyears-v1"
DEFAULT_CANDIDATE_SET = "oscand-allroots-2m-p1p1-n6-m3-v1"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-id", default=DEFAULT_RUN_ID)
    parser.add_argument("--years", default="", help="Comma-separated years. Empty means all years in the 2m candle table.")
    parser.add_argument("--timeframe", default="2m", choices=sorted(scanner.TIMEFRAME_TABLES))
    parser.add_argument("--level2-run-id", default=DEFAULT_LEVEL2_RUN)
    parser.add_argument("--stage2-run-id", default=DEFAULT_STAGE2_RUN)
    parser.add_argument("--candidate-set-id", default=DEFAULT_CANDIDATE_SET)
    parser.add_argument("--oracle-run-id", default="")
    parser.add_argument("--replace", action="store_true")
    parser.add_argument("--skip-existing-years", action="store_true", default=True)
    parser.add_argument("--rebuild-existing-years", action="store_true")
    parser.add_argument("--max-events-per-year", type=int, default=0, help="Debug cap. 0 means full year.")
    parser.add_argument("--max-cache-rows", type=int, default=0, help="Debug cap. 0 means full cache.")
    return parser.parse_args()


def to_jsonable(value: Any) -> Any:
    if isinstance(value, pd.Timestamp):
        return None if pd.isna(value) else value.isoformat()
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def model_dir(run_id_value: str) -> Path:
    path = Path(run_id_value)
    if path.exists():
        return path.resolve()
    return wave.ABCD_ROOT / "model_registry" / run_id_value


def parse_years(raw: str) -> list[int]:
    values = [part.strip() for part in str(raw or "").split(",") if part.strip()]
    years = sorted({int(value) for value in values})
    for year in years:
        if year < 2000 or year > 2100:
            raise ValueError(f"Bad year: {year}")
    return years


def available_candle_years(timeframe: str) -> list[int]:
    table_name, _minutes = scanner.table_for_timeframe(timeframe)
    table = scanner.safe_identifier(table_name)
    conn = wave.connect()
    try:
        with conn.cursor() as cur:
            cur.execute(f"SELECT DISTINCT YEAR(ts_utc) AS year FROM {table} ORDER BY year")
            rows = cur.fetchall()
    finally:
        conn.close()
    return [int(row["year"]) for row in rows if row.get("year") is not None]


def output_dir_for(run_id: str) -> Path:
    return wave.ABCD_ROOT / "model_registry" / run_id


def make_l2_args(l2_meta: dict[str, Any], args: argparse.Namespace) -> argparse.Namespace:
    l1_runs = dict(l2_meta.get("level1_model_runs") or {})
    rules = dict(l2_meta.get("event_rules") or {})
    return argparse.Namespace(
        timeframe=str(l2_meta.get("timeframe") or args.timeframe),
        roots=",".join(str(root) for root in (l2_meta.get("roots") or [])),
        oracle_run_id=str(args.oracle_run_id or l2_meta.get("oracle_run_id") or ""),
        level1_train_years="2024",
        level1_valid_year=int(l2_meta.get("valid_year") or 2026),
        train_year=int(l2_meta.get("train_year") or 2025),
        valid_year=int(l2_meta.get("valid_year") or 2026),
        cat_l1_prefix=level2.DEFAULT_CAT_L1_PREFIX,
        light_l1_prefix=level2.DEFAULT_LIGHT_L1_PREFIX,
        xgb_l1_prefix=level2.DEFAULT_XGB_L1_PREFIX,
        cat_l1_run_id=str(l1_runs.get("catboost") or ""),
        light_l1_run_id=str(l1_runs.get("lightgbm") or ""),
        xgb_l1_run_id=str(l1_runs.get("xgboost") or ""),
        manager_engines="cat,light,xgb",
        run_prefix="current-strategy-opinion-cache",
        max_train_rows=500_000,
        max_rows_per_year=int(args.max_cache_rows),
        limit_symbols=0,
        iterations=550,
        depth=6,
        learning_rate=0.04,
        l2_leaf_reg=10.0,
        random_seed=73,
        cooldown_bars=int(rules.get("cooldown_bars", 6)),
        match_window_bars=int(rules.get("match_window_bars", 3)),
        thresholds=str(rules.get("thresholds") or ""),
        threshold_quantile_count=int(rules.get("threshold_quantile_count", 28)),
        min_oracle_recall=float(rules.get("min_oracle_recall", 0.80)),
        max_picks_per_oracle=float(rules.get("max_picks_per_oracle", 3.0)),
        save_score_csvs=False,
        candidate_set_id=str(l2_meta.get("candidate_set_id") or args.candidate_set_id),
        candidate_cache_dir="",
        read_candidate_cache=True,
        write_candidate_cache=True,
        replace_candidate_cache=False,
        opinion_cache_dir="",
        train_opinion_cache="",
        valid_opinion_cache="",
        read_opinion_cache=True,
        write_opinion_cache=True,
        only_build_opinion_cache=False,
        replace_run=False,
        positive_pre_bars=1,
        positive_post_bars=1,
        negative_exclusion_bars=6,
        negative_ratio=8.0,
        base_negatives_per_symbol=200,
        entry_breakout_bars=8,
        trail_lookback_bars=18,
        atr_period=14,
        atr_stop_pad=0.35,
        min_risk_ticks=12.0,
        max_risk_ticks=240.0,
        min_relative_volume=0.45,
    )


def make_stage2_args(stage2_meta: dict[str, Any], l2_args: argparse.Namespace, args: argparse.Namespace) -> argparse.Namespace:
    target_params = dict(stage2_meta.get("target_parameters") or {})
    return argparse.Namespace(
        level2_run_id=str(args.level2_run_id),
        train_opinion_cache="",
        valid_opinion_cache="",
        run_prefix="current-strategy-stage2-eval",
        train_year=2025,
        valid_year=2026,
        timeframe=str(stage2_meta.get("timeframe") or args.timeframe),
        roots=l2_args.roots,
        manager_engines="cat,light,xgb",
        level2_threshold=None,
        stage1_match_window_bars=stage2_meta.get("stage1_match_window_bars"),
        stage1_cooldown_bars=stage2_meta.get("stage1_cooldown_bars"),
        max_confirm_bars=int(stage2_meta.get("max_confirm_bars") or 16),
        oracle_run_id=str(args.oracle_run_id or stage2_meta.get("oracle_run_id") or l2_args.oracle_run_id),
        oracle_detail_match_bars=4,
        target_mode=str(stage2_meta.get("target_mode") or "proof"),
        min_confirm_close_atr=float(target_params.get("min_confirm_close_atr", 0.05)),
        min_confirm_favorable_atr=float(target_params.get("min_confirm_favorable_atr", 0.15)),
        max_confirm_adverse_atr=float(target_params.get("max_confirm_adverse_atr", 1.50)),
        min_remaining_r=float(target_params.get("min_remaining_r", 0.25)),
        min_remaining_bars=int(target_params.get("min_remaining_bars", 1)),
        threshold_split=0.30,
        min_threshold_precision=0.55,
        min_threshold_picks=300,
        threshold_tail_candidates=450,
        iterations=450,
        depth=5,
        learning_rate=0.04,
        l2_leaf_reg=10.0,
        random_seed=73,
        max_cache_rows=int(args.max_cache_rows),
        max_events_per_year=int(args.max_events_per_year),
        max_train_rows=0,
        save_expanded_rows=False,
        replace_run=False,
        entry_breakout_bars=l2_args.entry_breakout_bars,
        trail_lookback_bars=l2_args.trail_lookback_bars,
        atr_period=l2_args.atr_period,
        atr_stop_pad=l2_args.atr_stop_pad,
        min_risk_ticks=l2_args.min_risk_ticks,
        max_risk_ticks=l2_args.max_risk_ticks,
        min_relative_volume=l2_args.min_relative_volume,
    )


def make_stage3_args(stage3_meta: dict[str, Any], args: argparse.Namespace) -> argparse.Namespace:
    return argparse.Namespace(
        stage2_run_id=str(args.stage2_run_id),
        run_prefix="current-strategy-stage3-eval",
        timeframe=str(stage3_meta.get("timeframe") or args.timeframe),
        valid_year=2026,
        valid_split="",
        replace_run=True,
        max_train_events=0,
        max_threshold_events=0,
        max_valid_events=0,
        event_directions="LONG,SHORT",
        terminal_only=True,
        save_decision_rows=False,
        min_hold_bars=int(stage3_meta.get("min_hold_bars") or 2),
        decision_step_bars=int(stage3_meta.get("decision_step_bars") or 1),
        dynamic_max_bars=int(stage3_meta.get("dynamic_max_bars") or 180),
        exit_threshold=None,
        max_threshold_dd_r=0.0,
        threshold_dd_penalty=0.0,
        min_threshold_trades=80,
        iterations=350,
        depth=5,
        learning_rate=0.04,
        l2_leaf_reg=12.0,
        random_seed=73,
        entry_breakout_bars=8,
        trail_lookback_bars=18,
        atr_period=14,
        atr_stop_pad=0.35,
        min_risk_ticks=12.0,
        max_risk_ticks=240.0,
        min_relative_volume=0.45,
        death_bars=4,
        profit_lock_trigger_r=0.0,
        profit_lock_giveback_r=1.5,
        profit_lock_min_r=0.25,
        max_forward_bars=int(stage3_meta.get("dynamic_max_bars") or 180),
        time_exit_bars=0,
        slippage_entry_ticks=float(stage3_meta.get("slippage_entry_ticks") or 3.0),
        slippage_exit_ticks=float(stage3_meta.get("slippage_exit_ticks") or 3.0),
    )


def load_or_build_l1_opinion_rows(year: int, l2_args: argparse.Namespace) -> tuple[pd.DataFrame, dict[str, Any]]:
    try:
        return level2.load_opinion_cache(l2_args, int(year))
    except FileNotFoundError:
        print(f"{year}: opinion cache missing; building it now", flush=True)
        rows, summary = level2.build_l1_opinion_rows(int(year), l2_args)
        level2.write_opinion_cache(rows, summary, l2_args, int(year))
        return rows, summary


def load_level2_model(l2_dir: Path) -> level2.CatBoostClassifier:
    model = level2.CatBoostClassifier()
    preferred = l2_dir / "catboost_oracle_start_level2_three_l1_manager.cbm"
    if not preferred.exists():
        matches = sorted(l2_dir.glob("catboost_oracle_start_level2*_manager.cbm"))
        if not matches:
            raise FileNotFoundError(f"Missing Level 2 manager model in {l2_dir}")
        preferred = matches[0]
    model.load_model(str(preferred))
    return model


def load_stage2_model(stage2_dir: Path) -> stage2_l2.CatBoostClassifier:
    model = stage2_l2.CatBoostClassifier()
    model.load_model(str(stage2_dir / "catboost_stage2_l2_confirmation.cbm"))
    return model


def chunks(values: list[str], size: int) -> list[list[str]]:
    return [values[index : index + size] for index in range(0, len(values), size)]


def numeric_event_column(frame: pd.DataFrame, column: str, default: float = 0.0) -> pd.Series:
    if column in frame.columns:
        return pd.to_numeric(frame[column], errors="coerce").fillna(default)
    return pd.Series(default, index=frame.index)


def event_metrics_from_predictions(events: pd.DataFrame, source_summary: dict[str, Any], threshold: float) -> dict[str, Any]:
    if events.empty:
        return {
            "events": 0,
            "stage1_positive_events": 0,
            "target_positive_events": 0,
            "confirmed_events": 0,
            "true_confirmed_events": 0,
            "false_confirmed_events": 0,
            "rejected_events": 0,
            "rejected_true_events": 0,
            "precision": 0.0,
            "recall_within_stage1": 0.0,
            "f1_within_stage1": 0.0,
            "oracle_confirmed_events": 0,
            "oracle_precision": 0.0,
            "total_oracle_recall": 0.0,
            "avg_confirm_offset": None,
            "median_confirm_offset": None,
            "avg_true_confirm_offset": None,
            "median_true_confirm_offset": None,
            "threshold": float(threshold),
        }
    y = numeric_event_column(events, "stage2_target").astype(int).to_numpy()
    oracle_y = numeric_event_column(events, "is_oracle_start").astype(int).to_numpy()
    confirmed = numeric_event_column(events, "confirmed").astype(int).to_numpy() == 1
    confirm_row_target = numeric_event_column(events, "confirm_row_target").astype(int).to_numpy()
    positives = int(y.sum())
    picks = int(confirmed.sum())
    true_picks = int((confirmed & (confirm_row_target == 1)).sum())
    false_picks = int((confirmed & (confirm_row_target == 0)).sum())
    precision = true_picks / picks if picks else 0.0
    recall = true_picks / positives if positives else 0.0
    f1 = (2.0 * precision * recall / (precision + recall)) if precision + recall > 0 else 0.0
    source_positives = int(source_summary.get("source_positives", source_summary.get("oracle_starts", 0)) or 0)
    true_oracle_confirms = int((confirmed & (oracle_y == 1)).sum())
    offsets = pd.to_numeric(events.loc[events["confirmed"] == 1, "confirm_offset_bars"], errors="coerce").dropna()
    true_offsets = pd.to_numeric(
        events.loc[(events["confirmed"] == 1) & (events["confirm_row_target"] == 1), "confirm_offset_bars"],
        errors="coerce",
    ).dropna()
    return {
        "events": int(len(events)),
        "stage1_positive_events": int(oracle_y.sum()),
        "target_positive_events": positives,
        "confirmed_events": picks,
        "true_confirmed_events": true_picks,
        "false_confirmed_events": false_picks,
        "rejected_events": int(len(events) - picks),
        "rejected_true_events": int(((~confirmed) & (y == 1)).sum()),
        "precision": precision,
        "recall_within_stage1": recall,
        "f1_within_stage1": f1,
        "oracle_confirmed_events": true_oracle_confirms,
        "oracle_precision": float(true_oracle_confirms / picks) if picks else 0.0,
        "total_oracle_recall": float(true_oracle_confirms / source_positives) if source_positives else 0.0,
        "avg_confirm_offset": float(offsets.mean()) if len(offsets) else None,
        "median_confirm_offset": float(offsets.median()) if len(offsets) else None,
        "avg_true_confirm_offset": float(true_offsets.mean()) if len(true_offsets) else None,
        "median_true_confirm_offset": float(true_offsets.median()) if len(true_offsets) else None,
        "threshold": float(threshold),
    }


def score_year_stage2_events(
    year: int,
    conn,
    output_dir: Path,
    l2_model: level2.CatBoostClassifier,
    stage2_model: stage2_l2.CatBoostClassifier,
    l2_args: argparse.Namespace,
    stage2_args: argparse.Namespace,
    l2_threshold: float,
    stage2_threshold: float,
    match_window: int,
    cooldown: int,
) -> tuple[pd.DataFrame, dict[str, Any]]:
    events_path = output_dir / f"stage2_l2_events_{year}.csv"
    confirmed_path = output_dir / f"stage2_l2_confirmed_events_{year}.csv"
    if events_path.exists() and confirmed_path.exists() and events_path.stat().st_size > 4 and confirmed_path.stat().st_size > 4:
        events = pd.read_csv(events_path)
        confirmed = pd.read_csv(confirmed_path)
        print(
            f"{year}: loaded existing Stage2 CSVs events={len(events):,} confirmed={len(confirmed):,}",
            flush=True,
        )
        source_summary = {"source_positives": int(numeric_event_column(events, "is_oracle_start").sum())}
        return confirmed, {
            "year": int(year),
            "l2_events": int(len(events)),
            "stage2_rows": None,
            "stage2_events": int(len(events)),
            "confirmed_events": int(len(confirmed)),
            "stage2_event_metrics": event_metrics_from_predictions(events, source_summary, float(stage2_threshold)),
        }

    opinion_rows, opinion_summary = load_or_build_l1_opinion_rows(int(year), l2_args)
    l2_scored = stage2_l2.score_level2(opinion_rows, l2_model, l2_args)
    l2_events = stage2_l2.select_level2_events(
        l2_scored,
        float(l2_threshold),
        int(match_window),
        int(cooldown),
        int(stage2_args.max_events_per_year),
        int(stage2_args.random_seed) + int(year),
    )
    print(f"{year}: L2 picked events={len(l2_events):,}", flush=True)
    if l2_events.empty:
        return pd.DataFrame(), {"year": int(year), "l2_events": 0, "confirmed_events": 0}

    source_summary = {
        "source_rows": int(opinion_summary.get("row_count") or len(opinion_rows)),
        "source_positives": int(opinion_summary.get("oracle_starts") or 0),
        "oracle_starts": int(opinion_summary.get("oracle_starts") or 0),
    }
    event_frames: list[pd.DataFrame] = []
    stage2_rows = 0
    symbols = sorted(l2_events["symbol"].dropna().astype(str).unique().tolist())
    symbol_chunks = chunks(symbols, 5)
    print(
        f"{year}: Stage2 chunking {len(l2_events):,} events across "
        f"{len(symbols):,} symbols in {len(symbol_chunks):,} chunks",
        flush=True,
    )
    for chunk_index, symbol_chunk in enumerate(symbol_chunks, start=1):
        chunk_events = l2_events[l2_events["symbol"].astype(str).isin(symbol_chunk)].copy()
        if chunk_events.empty:
            continue
        print(
            f"{year}: Stage2 starting chunk {chunk_index:,}/{len(symbol_chunks):,} "
            f"symbols={','.join(symbol_chunk[:3])}{'...' if len(symbol_chunk) > 3 else ''} "
            f"events={len(chunk_events):,}",
            flush=True,
        )
        expanded, _chunk_summary = stage2_l2.build_confirmation_rows(
            conn,
            chunk_events,
            opinion_summary,
            int(year),
            stage2_args,
            f"{year} chunk {chunk_index}/{len(symbol_chunks)}",
        )
        scored = stage2_l2.add_stage2_scores(expanded, stage2_model)
        event_frames.append(stage2_l2.event_predictions(scored, float(stage2_threshold)))
        stage2_rows += int(len(scored))
        confirmed_so_far = int(sum(pd.to_numeric(frame.get("confirmed"), errors="coerce").fillna(0).astype(int).sum() for frame in event_frames))
        print(
            f"{year}: Stage2 chunk {chunk_index:,}/{len(symbol_chunks):,} "
            f"events={sum(len(frame) for frame in event_frames):,}/{len(l2_events):,} "
            f"confirmed_so_far={confirmed_so_far:,}",
            flush=True,
        )
        del expanded, scored
        gc.collect()

    events = pd.concat(event_frames, ignore_index=True) if event_frames else pd.DataFrame()
    events.to_csv(output_dir / f"stage2_l2_events_{year}.csv", index=False)
    confirmed = events[pd.to_numeric(events["confirmed"], errors="coerce").fillna(0).astype(int) == 1].copy()
    confirmed.to_csv(output_dir / f"stage2_l2_confirmed_events_{year}.csv", index=False)
    metrics = {
        "year": int(year),
        "l2_events": int(len(l2_events)),
        "stage2_rows": int(stage2_rows),
        "stage2_events": int(len(events)),
        "confirmed_events": int(len(confirmed)),
        "stage2_event_metrics": event_metrics_from_predictions(events, source_summary, float(stage2_threshold)),
    }
    print(f"{year}: Stage2 confirmed events={len(confirmed):,}", flush=True)
    return confirmed, metrics


def build_stage3_trades_chunked(
    year: int,
    conn,
    confirmed: pd.DataFrame,
    stage3_args: argparse.Namespace,
) -> pd.DataFrame:
    symbols = sorted(confirmed["symbol"].dropna().astype(str).unique().tolist())
    symbol_chunks = chunks(symbols, 5)
    print(
        f"{year}: Stage3 trade chunking {len(confirmed):,} confirmations across "
        f"{len(symbols):,} symbols in {len(symbol_chunks):,} chunks",
        flush=True,
    )
    frames: list[pd.DataFrame] = []
    for chunk_index, symbol_chunk in enumerate(symbol_chunks, start=1):
        chunk_events = confirmed[confirmed["symbol"].astype(str).isin(symbol_chunk)].copy()
        if chunk_events.empty:
            continue
        print(
            f"{year}: Stage3 starting chunk {chunk_index:,}/{len(symbol_chunks):,} "
            f"symbols={','.join(symbol_chunk[:3])}{'...' if len(symbol_chunk) > 3 else ''} "
            f"confirmations={len(chunk_events):,}",
            flush=True,
        )
        trades = stage3.build_trades(conn, chunk_events, int(year), stage3_args, f"{year} trade chunk {chunk_index}/{len(symbol_chunks)}")
        frames.append(trades)
        print(
            f"{year}: Stage3 chunk {chunk_index:,}/{len(symbol_chunks):,} "
            f"trades_so_far={sum(len(frame) for frame in frames):,}",
            flush=True,
        )
        del trades
        gc.collect()
    frame = pd.concat(frames, ignore_index=True) if frames else pd.DataFrame()
    if not frame.empty:
        frame["selected_index"] = np.arange(1, len(frame) + 1)
    print(f"{year}: Stage3 built {len(frame):,} trade rows total", flush=True)
    return frame


def run_year(
    year: int,
    output_dir: Path,
    conn,
    l2_model: level2.CatBoostClassifier,
    stage2_model: stage2_l2.CatBoostClassifier,
    l2_args: argparse.Namespace,
    stage2_args: argparse.Namespace,
    stage3_args: argparse.Namespace,
    l2_threshold: float,
    stage2_threshold: float,
    match_window: int,
    cooldown: int,
) -> dict[str, Any]:
    print(f"{year}: starting current strategy evaluation", flush=True)
    year_started = time.time()
    confirmed, stage2_metrics = score_year_stage2_events(
        year,
        conn,
        output_dir,
        l2_model,
        stage2_model,
        l2_args,
        stage2_args,
        l2_threshold,
        stage2_threshold,
        match_window,
        cooldown,
    )

    if confirmed.empty:
        pd.DataFrame().to_csv(output_dir / f"stage3_rule_baseline_trades_{year}.csv", index=False)
        pd.DataFrame().to_csv(output_dir / f"stage3_terminal_hold_trades_{year}.csv", index=False)
        return {**stage2_metrics, "trades": 0, "elapsed_seconds": round(time.time() - year_started, 2)}

    table_name, timeframe_minutes = scanner.table_for_timeframe(stage3_args.timeframe)
    trades = build_stage3_trades_chunked(int(year), conn, confirmed, stage3_args)
    terminal = stage3.direct_terminal_overlay(conn, trades, table_name, timeframe_minutes, stage3_args, str(year))
    trades.to_csv(output_dir / f"stage3_rule_baseline_trades_{year}.csv", index=False)
    terminal.to_csv(output_dir / f"stage3_terminal_hold_trades_{year}.csv", index=False)
    summary = stage3.summarize_overlay("terminal_hold", terminal) if not terminal.empty else None
    print(f"{year}: terminal trades={len(terminal):,}", flush=True)
    return {
        **stage2_metrics,
        "trades": int(len(terminal)),
        "terminal_summary": summary,
        "elapsed_seconds": round(time.time() - year_started, 2),
    }


def max_drawdown(values: list[float]) -> float:
    equity = 0.0
    peak = 0.0
    dd = 0.0
    for value in values:
        equity += float(value)
        peak = max(peak, equity)
        dd = max(dd, peak - equity)
    return dd


def summarize_frame(frame: pd.DataFrame, keys: list[str], result_col: str = "model_result_r") -> pd.DataFrame:
    if frame.empty:
        return pd.DataFrame(columns=[*keys, "trades", "sum_r", "max_drawdown_r", "avg_r", "win_rate"])
    work = frame.copy()
    work["result_num"] = pd.to_numeric(work[result_col], errors="coerce").fillna(0.0)
    work["entry_dt"] = pd.to_datetime(work["entry_date"], errors="coerce")
    rows: list[dict[str, Any]] = []
    for values, group in work.groupby(keys, dropna=False):
        if not isinstance(values, tuple):
            values = (values,)
        ordered = group.sort_values(["entry_dt", "symbol"])
        results = ordered["result_num"].astype(float).tolist()
        trades = len(results)
        wins = sum(1 for value in results if value > 0)
        losses = sum(1 for value in results if value < 0)
        row = {key: value for key, value in zip(keys, values)}
        gross_win = sum(value for value in results if value > 0)
        gross_loss = sum(value for value in results if value < 0)
        row.update(
            {
                "trades": trades,
                "wins": wins,
                "losses": losses,
                "flats": trades - wins - losses,
                "sum_r": sum(results),
                "max_drawdown_r": max_drawdown(results),
                "avg_r": sum(results) / trades if trades else 0.0,
                "win_rate": wins / trades if trades else 0.0,
                "profit_factor_r": gross_win / abs(gross_loss) if gross_loss else None,
                "first_entry": ordered["entry_dt"].min(),
                "last_entry": ordered["entry_dt"].max(),
            }
        )
        rows.append(row)
    return pd.DataFrame(rows).sort_values(["sum_r", "avg_r"], ascending=[False, False]).reset_index(drop=True)


def candle_universe(timeframe: str) -> tuple[pd.DataFrame, pd.DataFrame]:
    table_name, _minutes = scanner.table_for_timeframe(timeframe)
    table = scanner.safe_identifier(table_name)
    conn = wave.connect()
    try:
        with conn.cursor() as cur:
            cur.execute(
                f"""
                SELECT
                    YEAR(ts_utc) AS year,
                    root_symbol,
                    COUNT(DISTINCT symbol) AS symbols_in_candle_data,
                    COUNT(*) AS candle_count,
                    MIN(ts_utc) AS first_ts,
                    MAX(ts_utc) AS last_ts
                FROM {table}
                GROUP BY YEAR(ts_utc), root_symbol
                """
            )
            root_year = pd.DataFrame(cur.fetchall())
            cur.execute(
                f"""
                SELECT
                    YEAR(ts_utc) AS year,
                    root_symbol,
                    symbol,
                    COUNT(*) AS candle_count,
                    MIN(ts_utc) AS first_ts,
                    MAX(ts_utc) AS last_ts
                FROM {table}
                GROUP BY YEAR(ts_utc), root_symbol, symbol
                """
            )
            symbol_year = pd.DataFrame(cur.fetchall())
    finally:
        conn.close()
    for frame in [root_year, symbol_year]:
        if frame.empty:
            continue
        frame["year"] = pd.to_numeric(frame["year"], errors="coerce").fillna(0).astype(int)
        frame["root_symbol"] = frame["root_symbol"].astype(str).str.upper()
        if "symbol" in frame.columns:
            frame["symbol"] = frame["symbol"].astype(str).str.upper()
        frame["first_ts"] = pd.to_datetime(frame["first_ts"], errors="coerce")
        frame["last_ts"] = pd.to_datetime(frame["last_ts"], errors="coerce")
    return root_year, symbol_year


def merge_coverage(universe: pd.DataFrame, summary: pd.DataFrame, keys: list[str]) -> pd.DataFrame:
    out = universe.merge(summary, on=keys, how="left")
    out["strategy_status"] = out["trades"].fillna(0).map(lambda value: "traded" if int(value) > 0 else "no_trades")
    for column in ["trades", "wins", "losses", "flats"]:
        if column in out.columns:
            out[column] = out[column].fillna(0).astype(int)
    for column in ["sum_r", "max_drawdown_r", "avg_r", "win_rate", "profit_factor_r"]:
        if column in out.columns:
            out[column] = out[column].fillna(0.0)
    return out.sort_values([*keys]).reset_index(drop=True)


def write_combined_reports(output_dir: Path, timeframe: str, years: list[int], year_metrics: list[dict[str, Any]]) -> dict[str, Any]:
    trade_frames = []
    for year in years:
        path = output_dir / f"stage3_terminal_hold_trades_{year}.csv"
        if path.exists() and path.stat().st_size > 4:
            try:
                trade_frames.append(pd.read_csv(path))
            except pd.errors.EmptyDataError:
                pass
    all_trades = pd.concat(trade_frames, ignore_index=True) if trade_frames else pd.DataFrame()
    if not all_trades.empty:
        if "year" not in all_trades.columns and "valid_year" in all_trades.columns:
            all_trades["year"] = pd.to_numeric(all_trades["valid_year"], errors="coerce").fillna(0).astype(int)
        all_trades.to_csv(output_dir / "stage3_terminal_hold_trades_all_years.csv", index=False)

    root_year_universe, symbol_year_universe = candle_universe(timeframe)
    root_year_universe = root_year_universe[root_year_universe["year"].isin(years)].copy()
    symbol_year_universe = symbol_year_universe[symbol_year_universe["year"].isin(years)].copy()

    root_year = summarize_frame(all_trades, ["year", "root_symbol"])
    symbol_year = summarize_frame(all_trades, ["year", "root_symbol", "symbol"])
    root_year_coverage = merge_coverage(root_year_universe, root_year, ["year", "root_symbol"])
    symbol_year_coverage = merge_coverage(symbol_year_universe, symbol_year, ["year", "root_symbol", "symbol"])

    root_combined = summarize_frame(all_trades, ["root_symbol"])
    symbol_combined = summarize_frame(all_trades, ["root_symbol", "symbol"])
    root_universe = (
        root_year_universe.groupby("root_symbol", dropna=False)
        .agg(
            symbols_in_candle_data=("symbols_in_candle_data", "max"),
            candle_count=("candle_count", "sum"),
            first_year=("year", "min"),
            last_year=("year", "max"),
            first_ts=("first_ts", "min"),
            last_ts=("last_ts", "max"),
        )
        .reset_index()
    )
    symbol_universe = (
        symbol_year_universe.groupby(["root_symbol", "symbol"], dropna=False)
        .agg(
            candle_count=("candle_count", "sum"),
            first_year=("year", "min"),
            last_year=("year", "max"),
            first_ts=("first_ts", "min"),
            last_ts=("last_ts", "max"),
        )
        .reset_index()
    )
    root_combined_coverage = merge_coverage(root_universe, root_combined, ["root_symbol"])
    symbol_combined_coverage = merge_coverage(symbol_universe, symbol_combined, ["root_symbol", "symbol"])

    root_year_coverage.to_csv(output_dir / "excel_root_year_summary.csv", index=False)
    root_combined_coverage.to_csv(output_dir / "excel_root_combined_summary.csv", index=False)
    symbol_year_coverage.to_csv(output_dir / "excel_symbol_year_summary.csv", index=False)
    symbol_combined_coverage.to_csv(output_dir / "excel_symbol_combined_summary.csv", index=False)
    pd.DataFrame(year_metrics).to_csv(output_dir / "excel_year_pipeline_summary.csv", index=False)

    metadata = {
        "run_id": output_dir.name,
        "timeframe": timeframe,
        "years": years,
        "trade_rows": int(len(all_trades)),
        "root_rows": int(len(root_combined_coverage)),
        "symbol_rows": int(len(symbol_combined_coverage)),
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "files": {
            "all_trades": "stage3_terminal_hold_trades_all_years.csv",
            "root_year": "excel_root_year_summary.csv",
            "root_combined": "excel_root_combined_summary.csv",
            "symbol_year": "excel_symbol_year_summary.csv",
            "symbol_combined": "excel_symbol_combined_summary.csv",
            "pipeline_year": "excel_year_pipeline_summary.csv",
        },
    }
    (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")
    return metadata


def main() -> int:
    args = parse_args()
    if args.rebuild_existing_years:
        args.skip_existing_years = False

    years = parse_years(args.years) or available_candle_years(args.timeframe)
    output_dir = output_dir_for(args.run_id)
    if output_dir.exists() and args.replace:
        # Keep this scoped. The caller opted into replacing this report folder.
        for path in output_dir.glob("*"):
            if path.is_file():
                path.unlink()
    output_dir.mkdir(parents=True, exist_ok=True)

    l2_dir = model_dir(args.level2_run_id)
    stage2_dir = model_dir(args.stage2_run_id)
    l2_meta = load_json(l2_dir / "metadata.json")
    stage2_meta = load_json(stage2_dir / "metadata.json")
    stage3_meta = load_json(model_dir("aicw-os-stage3-terminal-both-full-v2-2m-m180-v2026") / "metadata.json")

    l2_args = make_l2_args(l2_meta, args)
    stage2_args = make_stage2_args(stage2_meta, l2_args, args)
    stage3_args = make_stage3_args(stage3_meta, args)
    scanner.configure_wave_args(stage3_args)

    l2_threshold = float(l2_meta["selected_threshold"])
    stage2_threshold = float(stage2_meta["selected_stage2_threshold"])
    match_window, cooldown = stage2_l2.stage1_event_rules(stage2_args, l2_meta)
    l2_model = load_level2_model(l2_dir)
    stage2_model = load_stage2_model(stage2_dir)

    print(
        f"Current strategy all-year run years={years} l2_threshold={l2_threshold:.6f} "
        f"stage2_threshold={stage2_threshold:.6f}",
        flush=True,
    )

    conn = wave.connect()
    year_metrics: list[dict[str, Any]] = []
    try:
        for year in years:
            terminal_path = output_dir / f"stage3_terminal_hold_trades_{year}.csv"
            if args.skip_existing_years and terminal_path.exists() and terminal_path.stat().st_size > 4:
                print(f"{year}: skipping existing year output", flush=True)
                year_metrics.append({"year": int(year), "status": "skipped_existing"})
                continue
            metrics = run_year(
                int(year),
                output_dir,
                conn,
                l2_model,
                stage2_model,
                l2_args,
                stage2_args,
                stage3_args,
                l2_threshold,
                stage2_threshold,
                match_window,
                cooldown,
            )
            metrics["status"] = "ok"
            year_metrics.append(metrics)
            (output_dir / "progress.json").write_text(
                json.dumps({"years": years, "completed": year_metrics}, indent=2, default=to_jsonable),
                encoding="utf-8",
            )
    finally:
        conn.close()

    metadata = write_combined_reports(output_dir, args.timeframe, years, year_metrics)
    print(json.dumps(metadata, indent=2), flush=True)
    print(f"Saved current strategy all-year root report: {output_dir}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
