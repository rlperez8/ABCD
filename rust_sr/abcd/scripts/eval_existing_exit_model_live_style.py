#!/usr/bin/env python3
"""
Evaluate an existing candle-wave exit model in live-style mode.

This is meant to answer one question:

Can the source-exit-aware 180 overlay still work if we run it live, where the
source exit rule is calculated candle-by-candle but we cannot fall back to a
past source exit after choosing to hold?

It reuses the current entry model to select validation trades from candidate
rows, builds exit decisions, scores an existing exit model, then compares:

* source-fallback overlay: original research behavior
* live-terminal overlay: live-honest behavior after an AI hold/veto
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

import pandas as pd

try:
    from catboost import CatBoostRegressor
except ImportError as exc:  # pragma: no cover
    raise SystemExit("CatBoost is required. Run with .venv_ai\\Scripts\\python.exe") from exc


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_exit_model as exit_model
import ai_candle_wave_scanner_model as scanner
import ai_wave_rider_research as wave


DEFAULT_SOURCE_RUN = "aicw-mtf-eg1-xtight-t014-rd3-en4-v1-2m-2026"
DEFAULT_EXIT_RUN = "aicw-exit-dyn180s2-srcfb-v1-2m-2026"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-model-run-id", default=DEFAULT_SOURCE_RUN)
    parser.add_argument("--exit-model-run-id", default=DEFAULT_EXIT_RUN)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--max-valid-trades", type=int, default=0)
    parser.add_argument("--valid-from-candidates", action="store_true", default=True)
    parser.add_argument("--skip-source-fallback-compare", action="store_true")
    return parser.parse_args()


def load_exit_run_metadata(run_id: str) -> tuple[dict[str, Any], CatBoostRegressor]:
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id
    metadata_path = model_dir / "metadata.json"
    if not metadata_path.exists():
        raise FileNotFoundError(f"Missing exit metadata: {metadata_path}")
    metadata = json.loads(metadata_path.read_text())
    model_path = model_dir / "catboost_exit_model.cbm"
    if not model_path.exists():
        raise FileNotFoundError(f"Missing exit model: {model_path}")
    model = CatBoostRegressor()
    model.load_model(str(model_path))
    return metadata, model


def args_from_exit_metadata(metadata: dict[str, Any], no_signal_exit: str) -> argparse.Namespace:
    return argparse.Namespace(
        mode=str(metadata.get("mode") or "dynamic"),
        dynamic_max_bars=int(metadata.get("dynamic_max_bars") or 180),
        decision_step_bars=int(metadata.get("decision_step_bars") or 2),
        dynamic_no_signal_exit=no_signal_exit,
        exit_threshold=metadata.get("exit_threshold"),
        min_hold_bars=int(metadata.get("min_hold_bars") or 4),
        exclude_source_exit_features=bool(metadata.get("exclude_source_exit_features") or False),
        min_threshold_trades=80,
        max_threshold_dd_r=0.0,
        threshold_dd_penalty=0.0,
    )


def print_summary(label: str, frame: pd.DataFrame) -> None:
    summary = exit_model.summarize_results(frame["model_result_r"])
    reasons = (
        frame.groupby("model_exit_reason", dropna=False)["model_result_r"]
        .agg(["count", "sum", "mean"])
        .sort_values("count", ascending=False)
    )
    print(
        f"{label}: trades={summary['trades']:,} win={summary['win_rate'] * 100:.2f}% "
        f"sum={summary['sum_r']:.2f}R avg={summary['avg_r']:.4f}R dd={summary['max_drawdown_r']:.2f}R"
    )
    print(f"{label} exit reasons:")
    for reason, row in reasons.iterrows():
        print(f"  {reason}: count={int(row['count']):,} sum={float(row['sum']):.2f}R avg={float(row['mean']):.4f}R")


def main() -> int:
    args = parse_args()
    conn = wave.connect()
    try:
        source = exit_model.load_source_run(conn, args.source_model_run_id)
        entry_args = exit_model.entry_args_from_metadata(source["metadata"])
        timeframe = str(entry_args.timeframe)
        table_name, timeframe_minutes = scanner.table_for_timeframe(timeframe)
        entry_model = exit_model.load_entry_model(source)
        exit_metadata, model = load_exit_run_metadata(args.exit_model_run_id)
        threshold = exit_metadata.get("exit_threshold")

        if args.valid_from_candidates:
            trades = exit_model.selected_like_trades(conn, entry_model, entry_args, [args.valid_year], args.max_valid_trades)
        else:
            trades = exit_model.load_selected_trades(conn, args.source_model_run_id, args.max_valid_trades)
        print(
            f"Existing exit live-style eval source={args.source_model_run_id} "
            f"exit={args.exit_model_run_id} valid_trades={len(trades):,} threshold={threshold}"
        )

        live_args = args_from_exit_metadata(exit_metadata, "terminal")
        decisions = exit_model.build_decision_rows(conn, trades, table_name, timeframe_minutes, live_args, False, "valid")
        decisions = exit_model.add_exit_predictions(decisions, model, live_args)
        live_overlay = exit_model.apply_overlay(trades, decisions, threshold, live_args)
        print_summary("live_terminal", live_overlay)

        if not args.skip_source_fallback_compare:
            source_args = args_from_exit_metadata(exit_metadata, "source")
            source_overlay = exit_model.apply_overlay(trades, decisions, threshold, source_args)
            print_summary("source_fallback", source_overlay)

    finally:
        conn.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
