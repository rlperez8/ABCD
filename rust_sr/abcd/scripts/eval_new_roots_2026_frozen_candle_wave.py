from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

import pandas as pd
from catboost import CatBoostRegressor

import ai_candle_wave_exit_model as exit_model
import ai_candle_wave_scanner_model as scanner
import ai_wave_rider_research as wave


SOURCE_MODEL_RUN_ID = "aicw-mtf-eg1-xtight-t014-rd3-en4-v1-2m-2026"
EXIT_MODEL_RUN_ID = "aicw-exit-dyn180s2-srcfb-v1-2m-2026"
VALID_YEAR = 2026
NEW_ROOTS = [
    "MES",
    "MNQ",
    "M2K",
    "MYM",
    "MCL",
    "MGC",
    "MHG",
    "QO",
    "QI",
    "ZF",
    "ZT",
    "UB",
    "M6A",
    "M6B",
    "M6E",
    "MCD",
    "6M",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Evaluate the frozen candle-wave entry + exit models on 2026 roots.")
    parser.add_argument("--roots", default=",".join(NEW_ROOTS), help="Comma-separated roots to evaluate.")
    parser.add_argument("--all-roots", action="store_true", help="Evaluate all roots with 2026 1m candles.")
    parser.add_argument("--replace-candidates", action="store_true", help="Delete/rebuild 2026 candidates first.")
    parser.add_argument("--skip-candidate-build", action="store_true")
    return parser.parse_args()


def parse_root_list(value: str) -> list[str]:
    return [part.strip().upper() for part in str(value or "").split(",") if part.strip()]


def all_2026_roots(conn) -> list[str]:
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT DISTINCT root_symbol
            FROM futures_contract_1m_candles
            WHERE ts_utc >= '2026-01-01'
              AND ts_utc < '2027-01-01'
            ORDER BY root_symbol
            """
        )
        return [str(row["root_symbol"]).upper() for row in cur.fetchall()]


def scanner_args_from_metadata(metadata: dict[str, Any], roots: list[str]) -> argparse.Namespace:
    argv = [
        "eval_new_roots_2026_frozen_candle_wave.py",
        "--timeframe",
        str(metadata.get("timeframe") or "2m"),
        "--context-timeframes",
        ",".join(metadata.get("context_timeframes") or ["5m", "15m", "1h", "4h"]),
        "--context-lookback-bars",
        str(metadata.get("context_lookback_bars") or 80),
        "--candidate-years",
        str(VALID_YEAR),
        "--train-years",
        ",".join(str(year) for year in metadata.get("train_years") or [2024, 2025]),
        "--valid-year",
        str(VALID_YEAR),
        "--threshold-year",
        str(metadata.get("threshold_year") or 2025),
        "--run-prefix",
        "newroots-frozen",
        "--threshold",
        str(metadata.get("threshold")),
        "--candidate-min-gap-bars",
        str(metadata.get("candidate_min_gap_bars") or 6),
        "--cooldown-minutes",
        str(metadata.get("cooldown_minutes") or 0),
        "--max-root-trades-per-day",
        str(metadata.get("max_root_trades_per_day") or 0),
        "--max-energy-trades-per-day",
        str(metadata.get("max_energy_trades_per_day") or 0),
        "--energy-extra-slot-start",
        str(metadata.get("energy_extra_slot_start") or 0),
        "--energy-extra-slot-min-score",
        str(metadata.get("energy_extra_slot_min_score") or 0.0),
        "--loss-brake-r",
        str(metadata.get("loss_brake_r") or 0.0),
        "--loss-brake-scope",
        str(metadata.get("loss_brake_scope") or "root"),
        "--loss-brake-min-trades",
        str(metadata.get("loss_brake_min_trades") or 1),
        "--iterations",
        str(metadata.get("iterations") or 800),
        "--depth",
        str(metadata.get("depth") or 6),
        "--learning-rate",
        str(metadata.get("learning_rate") or 0.035),
        "--l2-leaf-reg",
        str(metadata.get("l2_leaf_reg") or 10.0),
        "--random-seed",
        str(metadata.get("random_seed") or 73),
        "--entry-breakout-bars",
        "8",
        "--trail-lookback-bars",
        "18",
        "--atr-period",
        "14",
        "--atr-stop-pad",
        "0.35",
        "--min-risk-ticks",
        str(metadata.get("min_risk_ticks") or 12.0),
        "--max-risk-ticks",
        str(metadata.get("max_risk_ticks") or 240.0),
        "--max-forward-bars",
        str(metadata.get("max_forward_bars") or 576),
        "--time-exit-bars",
        str(metadata.get("time_exit_bars") or 0),
        "--slippage-entry-ticks",
        str(metadata.get("slippage_entry_ticks") or 3.0),
        "--slippage-exit-ticks",
        str(metadata.get("slippage_exit_ticks") or 3.0),
        "--mtf-entry-min-aligned",
        str(metadata.get("mtf_entry_min_aligned") or 0),
        "--mtf-exit-min-aligned",
        str(metadata.get("mtf_exit_min_aligned") or 1),
        "--mtf-exit-mode",
        str(metadata.get("mtf_exit_mode") or "hard-veto"),
        "--mtf-exit-extra-death-bars",
        str(metadata.get("mtf_exit_extra_death_bars") or 1),
        "--mtf-exit-tighten-lookback-bars",
        str(metadata.get("mtf_exit_tighten_lookback_bars") or 3),
        "--mtf-exit-tighten-pad-ticks",
        str(metadata.get("mtf_exit_tighten_pad_ticks") or 1.0),
    ]
    if roots:
        argv.extend(["--roots", ",".join(roots)])
    if metadata.get("mtf_exit_governor"):
        argv.append("--mtf-exit-governor")
    if metadata.get("random_strength") is not None:
        argv.extend(["--random-strength", str(metadata["random_strength"])])
    if metadata.get("bootstrap_type"):
        argv.extend(["--bootstrap-type", str(metadata["bootstrap_type"])])
    if metadata.get("target_clip_min") is not None:
        argv.extend(["--target-clip-min", str(metadata["target_clip_min"])])
    if metadata.get("target_clip_max") is not None:
        argv.extend(["--target-clip-max", str(metadata["target_clip_max"])])
    if metadata.get("threshold_score_cap_r") is not None:
        argv.extend(["--threshold-score-cap-r", str(metadata["threshold_score_cap_r"])])

    old_argv = sys.argv
    try:
        sys.argv = argv
        args = scanner.parse_args()
    finally:
        sys.argv = old_argv
    scanner.configure_wave_args(args)
    return args


def exit_args_from_metadata(metadata: dict[str, Any]) -> argparse.Namespace:
    argv = [
        "eval_new_roots_2026_frozen_candle_wave.py",
        "--source-model-run-id",
        str(metadata.get("source_model_run_id") or SOURCE_MODEL_RUN_ID),
        "--run-prefix",
        "newroots-exit180",
        "--mode",
        str(metadata.get("mode") or "dynamic"),
        "--train-years",
        ",".join(str(year) for year in metadata.get("train_years") or [2024]),
        "--threshold-year",
        str(metadata.get("threshold_year") or 2025),
        "--valid-year",
        str(VALID_YEAR),
        "--min-hold-bars",
        "4",
        "--decision-step-bars",
        str(metadata.get("decision_step_bars") or 2),
        "--dynamic-max-bars",
        str(metadata.get("dynamic_max_bars") or 180),
        "--dynamic-no-signal-exit",
        str(metadata.get("dynamic_no_signal_exit") or "source"),
        "--exit-threshold",
        str(metadata.get("exit_threshold")),
    ]
    old_argv = sys.argv
    try:
        sys.argv = argv
        return exit_model.parse_args()
    finally:
        sys.argv = old_argv


def load_registry_metadata(run_id: str) -> dict[str, Any]:
    path = wave.ABCD_ROOT / "model_registry" / run_id / "metadata.json"
    return json.loads(path.read_text())


def load_entry_model(source: dict[str, Any]) -> CatBoostRegressor:
    model = CatBoostRegressor()
    model.load_model(str(wave.ABCD_ROOT / "model_registry" / source["model_run_id"] / "catboost_model.cbm"))
    return model


def load_exit_model(run_id: str) -> CatBoostRegressor:
    model = CatBoostRegressor()
    model.load_model(str(wave.ABCD_ROOT / "model_registry" / run_id / "catboost_exit_model.cbm"))
    return model


def summarize_by_root(frame: pd.DataFrame, value_column: str) -> list[dict[str, Any]]:
    if frame.empty:
        return []
    rows: list[dict[str, Any]] = []
    for root, group in frame.groupby("root_symbol", sort=True):
        summary = exit_model.summarize_results(group[value_column])
        rows.append(
            {
                "root_symbol": root,
                "trades": summary["trades"],
                "wins": summary["wins"],
                "losses": summary["losses"],
                "win_rate_pct": round(summary["win_rate"] * 100.0, 2),
                "sum_r": round(summary["sum_r"], 2),
                "avg_r": round(summary["avg_r"], 4),
                "max_drawdown_r": round(summary["max_drawdown_r"], 2),
            }
        )
    rows.sort(key=lambda row: row["sum_r"], reverse=True)
    return rows


def main() -> int:
    cli_args = parse_args()
    conn = wave.connect()
    try:
        roots = [] if cli_args.all_roots else parse_root_list(cli_args.roots)
        display_roots = all_2026_roots(conn) if cli_args.all_roots else roots
        source_metadata = load_registry_metadata(SOURCE_MODEL_RUN_ID)
        exit_metadata = load_registry_metadata(EXIT_MODEL_RUN_ID)
        build_args = scanner_args_from_metadata(source_metadata, roots)
        print(f"Candidate formula: {build_args.formula}")
        print(
            f"Evaluating {VALID_YEAR} roots: "
            f"{'ALL' if cli_args.all_roots else ','.join(display_roots)}"
        )
        if cli_args.replace_candidates:
            print(f"Deleting existing {VALID_YEAR} candidates for formula/timeframe before rebuild.")
            scanner.delete_candidates(conn, build_args.timeframe, [VALID_YEAR], build_args.formula)
        if not cli_args.skip_candidate_build:
            print(f"Building/upserting {VALID_YEAR} candidates.")
            built = scanner.build_candidates_for_year(conn, VALID_YEAR, build_args)
            print(f"Candidate build pass produced {built:,} rows before upsert/dedupe.")

        candidates = scanner.load_candidates(conn, build_args.timeframe, [VALID_YEAR], build_args.formula)
        if roots:
            candidates = candidates[candidates["root_symbol"].astype(str).str.upper().isin(roots)].copy()
        print(f"Loaded {len(candidates):,} candidate rows for evaluation.")
        if candidates.empty:
            return 0

        source = {"model_run_id": SOURCE_MODEL_RUN_ID}
        entry_model = load_entry_model(source)
        entry_select_args = exit_model.entry_args_from_metadata(source_metadata)
        scored = scanner.add_predictions(candidates, entry_model)
        selected = scanner.select_chronological(scored, entry_select_args.threshold, entry_select_args)
        selected = exit_model.normalize_trade_frame(selected)
        print(f"Frozen entry model selected {len(selected):,} trades.")

        exit_args = exit_args_from_metadata(exit_metadata)
        table_name, timeframe_minutes = scanner.table_for_timeframe(build_args.timeframe)
        exit_cat = load_exit_model(EXIT_MODEL_RUN_ID)
        decisions = exit_model.build_decision_rows(
            conn,
            selected,
            table_name,
            timeframe_minutes,
            exit_args,
            False,
            "new-roots-2026-exit",
        )
        decisions = exit_model.add_exit_predictions(decisions, exit_cat)
        overlay = exit_model.apply_overlay(selected, decisions, exit_args.exit_threshold, exit_args)

        baseline = exit_model.summarize_results(selected["result_r"])
        final = exit_model.summarize_results(overlay["model_result_r"])
        delta = final["sum_r"] - baseline["sum_r"]
        print("--- SUMMARY ---")
        print(
            f"baseline trades={baseline['trades']:,} win={baseline['win_rate']*100:.2f}% "
            f"sum={baseline['sum_r']:.2f}R avg={baseline['avg_r']:.4f}R dd={baseline['max_drawdown_r']:.2f}R"
        )
        print(
            f"exit180 trades={final['trades']:,} win={final['win_rate']*100:.2f}% "
            f"sum={final['sum_r']:.2f}R avg={final['avg_r']:.4f}R dd={final['max_drawdown_r']:.2f}R "
            f"delta={delta:.2f}R"
        )
        print("--- ROOTS BASELINE ---")
        for row in summarize_by_root(selected, "result_r"):
            print(row)
        print("--- ROOTS EXIT180 ---")
        for row in summarize_by_root(overlay, "model_result_r"):
            print(row)
        print("--- EXIT REASONS ---")
        for reason, count in overlay["model_exit_reason"].astype(str).value_counts().sort_index().items():
            print(f"{reason}: {count}")
    finally:
        conn.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
