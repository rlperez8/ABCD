#!/usr/bin/env python3
"""
Train a candle-wave action model on original/reverse counterfactual rows.

The scanner creates candidates and stores both the original trade result and
the reverse-side result. This script treats those as labels and trains a
separate policy layer that can choose original, reverse, or skip.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from collections import defaultdict
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

import ai_candle_wave_scanner_model as scanner
import ai_wave_rider_research as wave


ACTION_RUN_TABLE = "ai_candle_wave_action_model_runs"
ACTION_SELECTED_TABLE = "ai_candle_wave_action_selected_trades"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--timeframe", choices=sorted(scanner.TIMEFRAME_TABLES), default="2m")
    parser.add_argument("--context-timeframes", default=scanner.DEFAULT_CONTEXT_TIMEFRAMES)
    parser.add_argument("--context-lookback-bars", type=int, default=80)
    parser.add_argument("--train-years", default="2024,2025")
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--run-prefix", default="aicw-act-v1")
    parser.add_argument("--replace-run", action="store_true")
    parser.add_argument("--iterations", type=int, default=800)
    parser.add_argument("--depth", type=int, default=6)
    parser.add_argument("--learning-rate", type=float, default=0.035)
    parser.add_argument("--l2-leaf-reg", type=float, default=10.0)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--target-clip-min", type=float, default=-3.0)
    parser.add_argument("--target-clip-max", type=float, default=10.0)
    parser.add_argument("--min-threshold-trades", type=int, default=80)
    parser.add_argument("--max-dd-multiple", type=float, default=1.15)
    parser.add_argument("--min-cap5-fraction", type=float, default=0.50)

    parser.add_argument("--original-threshold", type=float, default=None)
    parser.add_argument("--reverse-threshold", type=float, default=None)
    parser.add_argument("--reverse-margin", type=float, default=None)
    parser.add_argument("--original-danger-margin", type=float, default=None)
    parser.add_argument("--original-threshold-grid", default="0,0.01,0.014,0.02,0.03,0.05,0.08,0.10")
    parser.add_argument("--reverse-threshold-grid", default="0.5,1.0,1.5,2.0,3.0,999")
    parser.add_argument("--reverse-margin-grid", default="0.5,1.0,1.5,2.0,3.0")
    parser.add_argument("--original-danger-margin-grid", default="0.5,1.0,1.5,2.0,999")

    parser.add_argument("--candidate-min-gap-bars", type=int, default=15)
    parser.add_argument("--cooldown-minutes", type=int, default=0)
    parser.add_argument("--allowed-roots", default="")
    parser.add_argument("--max-root-trades-per-day", type=int, default=3)
    parser.add_argument("--max-energy-trades-per-day", type=int, default=4)

    # Scanner knobs used only to derive the base formula/metadata.
    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=240.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    parser.add_argument("--death-bars", type=int, default=4)
    parser.add_argument("--min-hold-bars", type=int, default=4)
    parser.add_argument("--profit-lock-trigger-r", type=float, default=0.0)
    parser.add_argument("--profit-lock-giveback-r", type=float, default=1.5)
    parser.add_argument("--profit-lock-min-r", type=float, default=0.25)
    parser.add_argument("--max-forward-bars", type=int, default=1440)
    parser.add_argument("--time-exit-bars", type=int, default=0)
    parser.add_argument("--slippage-entry-ticks", type=float, default=3.0)
    parser.add_argument("--slippage-exit-ticks", type=float, default=3.0)
    parser.add_argument("--loose-triggers", action="store_true")
    parser.add_argument("--mtf-entry-min-aligned", type=int, default=1)
    parser.add_argument("--mtf-exit-governor", action="store_true", default=True)
    parser.add_argument("--mtf-exit-min-aligned", type=int, default=1)
    parser.add_argument(
        "--mtf-exit-mode",
        choices=["hard-veto", "delay", "tighten", "delay-tighten"],
        default="tighten",
    )
    parser.add_argument("--mtf-exit-extra-death-bars", type=int, default=1)
    parser.add_argument("--mtf-exit-tighten-lookback-bars", type=int, default=3)
    parser.add_argument("--mtf-exit-tighten-pad-ticks", type=float, default=1.0)
    return parser.parse_args()


def parse_float_grid(value: str) -> list[float]:
    return [float(part.strip()) for part in str(value or "").split(",") if part.strip()]


def configure_args(args: argparse.Namespace) -> None:
    scanner.configure_wave_args(args)
    args.formula = scanner.formula_for_args(args)


def run_id_for(args: argparse.Namespace) -> str:
    run_id = f"{args.run_prefix}-{args.timeframe}-{args.valid_year}"
    if len(run_id) > 64:
        raise ValueError(f"Run id too long: {run_id}")
    return run_id


def action_selected_column_definitions() -> dict[str, str]:
    return {
        "predicted_original_r": "DOUBLE NULL",
        "predicted_reverse_r": "DOUBLE NULL",
        "predicted_action_r": "DOUBLE NULL",
        "selected_action": "VARCHAR(16) NULL",
        "candidate_direction": "VARCHAR(16) NULL",
        "selected_direction": "VARCHAR(16) NULL",
        "original_result_r": "DOUBLE NULL",
        "original_exit_reason": "VARCHAR(64) NULL",
        "original_exit_date": "DATETIME NULL",
        "original_hold_minutes": "INT NULL",
    }


def ensure_tables(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            f"""
            CREATE TABLE IF NOT EXISTS {ACTION_RUN_TABLE} (
                action_model_run_id VARCHAR(64) NOT NULL PRIMARY KEY,
                base_formula VARCHAR(128) NOT NULL,
                timeframe VARCHAR(16) NOT NULL,
                train_years VARCHAR(64) NOT NULL,
                threshold_year INT NOT NULL,
                valid_year INT NOT NULL,
                original_threshold DOUBLE NOT NULL,
                reverse_threshold DOUBLE NOT NULL,
                reverse_margin DOUBLE NOT NULL,
                original_danger_margin DOUBLE NOT NULL,
                candidate_rows INT NOT NULL DEFAULT 0,
                selected_trades INT NOT NULL DEFAULT 0,
                original_trades INT NOT NULL DEFAULT 0,
                reverse_trades INT NOT NULL DEFAULT 0,
                wins INT NOT NULL DEFAULT 0,
                losses INT NOT NULL DEFAULT 0,
                win_rate DOUBLE NOT NULL DEFAULT 0,
                avg_r DOUBLE NOT NULL DEFAULT 0,
                sum_r DOUBLE NOT NULL DEFAULT 0,
                max_drawdown_r DOUBLE NOT NULL DEFAULT 0,
                cap5_sum_r DOUBLE NOT NULL DEFAULT 0,
                cap10_sum_r DOUBLE NOT NULL DEFAULT 0,
                model_path VARCHAR(255) NULL,
                metadata_json JSON NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
            )
            """
        )
        cur.execute(
            f"""
            CREATE TABLE IF NOT EXISTS {ACTION_SELECTED_TABLE} (
                action_model_run_id VARCHAR(64) NOT NULL,
                selected_index INT NOT NULL,
                candidate_uid VARCHAR(40) NOT NULL,
                formula VARCHAR(128) NOT NULL,
                timeframe VARCHAR(16) NOT NULL,
                valid_year INT NOT NULL,
                root_symbol VARCHAR(32) NOT NULL,
                symbol VARCHAR(64) NOT NULL,
                signal_date DATETIME NOT NULL,
                entry_date DATETIME NOT NULL,
                exit_date DATETIME NULL,
                selected_action VARCHAR(16) NOT NULL,
                candidate_direction VARCHAR(16) NOT NULL,
                selected_direction VARCHAR(16) NOT NULL,
                exit_reason VARCHAR(64) NULL,
                entry_price DOUBLE NULL,
                stop_price DOUBLE NULL,
                exit_price DOUBLE NULL,
                risk_points DOUBLE NULL,
                result_r DOUBLE NOT NULL DEFAULT 0,
                raw_result_r DOUBLE NULL,
                mfe_r DOUBLE NULL,
                mae_r DOUBLE NULL,
                hold_minutes INT NULL,
                risk_ticks DOUBLE NULL,
                tick_size DOUBLE NULL,
                predicted_original_r DOUBLE NULL,
                predicted_reverse_r DOUBLE NULL,
                predicted_action_r DOUBLE NULL,
                original_result_r DOUBLE NULL,
                original_exit_reason VARCHAR(64) NULL,
                original_exit_date DATETIME NULL,
                original_hold_minutes INT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (action_model_run_id, candidate_uid),
                INDEX idx_candle_wave_action_selected_time (action_model_run_id, entry_date),
                INDEX idx_candle_wave_action_selected_result (action_model_run_id, result_r)
            )
            """
        )
        scanner.ensure_missing_columns(cur, ACTION_SELECTED_TABLE, action_selected_column_definitions())
        scanner.ensure_missing_columns(cur, ACTION_SELECTED_TABLE, scanner.model_feature_column_definitions())
        scanner.ensure_missing_columns(cur, ACTION_SELECTED_TABLE, scanner.counterfactual_column_definitions())
    conn.commit()


def delete_run(conn, run_id: str) -> None:
    with conn.cursor() as cur:
        cur.execute(f"DELETE FROM {ACTION_SELECTED_TABLE} WHERE action_model_run_id = %s", (run_id,))
        cur.execute(f"DELETE FROM {ACTION_RUN_TABLE} WHERE action_model_run_id = %s", (run_id,))
    conn.commit()


def clipped_target(frame: pd.DataFrame, target_col: str, args: argparse.Namespace) -> pd.Series:
    target = pd.to_numeric(frame[target_col], errors="coerce")
    if args.target_clip_min is not None:
        target = target.clip(lower=float(args.target_clip_min))
    if args.target_clip_max is not None:
        target = target.clip(upper=float(args.target_clip_max))
    return target


def train_regressor(frame: pd.DataFrame, target_col: str, args: argparse.Namespace) -> CatBoostRegressor:
    work = frame.dropna(subset=[target_col]).copy()
    if work.empty:
        raise RuntimeError(f"No training rows for target {target_col}")
    work["result_r"] = clipped_target(work, target_col, args)
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
    model.fit(scanner.prepare_pool(work, include_target=True, args=args))
    return model


def score_frame(frame: pd.DataFrame, original_model: CatBoostRegressor, reverse_model: CatBoostRegressor) -> pd.DataFrame:
    scored = frame.copy()
    scored["predicted_original_r"] = original_model.predict(scanner.prepare_pool(scored, include_target=False))
    scored["predicted_reverse_r"] = -999.0
    reverse_ready = scored.dropna(subset=["reverse_result_r"]).copy()
    if not reverse_ready.empty:
        scored.loc[reverse_ready.index, "predicted_reverse_r"] = reverse_model.predict(
            scanner.prepare_pool(reverse_ready, include_target=False)
        )
    return scored


def choose_action(row: pd.Series, policy: dict[str, float]) -> str:
    original_pred = float(row.get("predicted_original_r") or -999.0)
    reverse_pred = float(row.get("predicted_reverse_r") or -999.0)
    has_reverse = not pd.isna(row.get("reverse_result_r"))
    reverse_ok = (
        has_reverse
        and reverse_pred >= policy["reverse_threshold"]
        and reverse_pred >= original_pred + policy["reverse_margin"]
    )
    if reverse_ok:
        return "reverse"
    original_ok = (
        original_pred >= policy["original_threshold"]
        and reverse_pred <= original_pred + policy["original_danger_margin"]
    )
    if original_ok:
        return "original"
    return "skip"


def max_drawdown(values: list[float]) -> float:
    equity = 0.0
    peak = 0.0
    drawdown = 0.0
    for value in values:
        equity += float(value)
        peak = max(peak, equity)
        drawdown = max(drawdown, peak - equity)
    return drawdown


def row_for_action(row: pd.Series, action: str) -> dict[str, Any]:
    if action == "reverse":
        direction = str(row.get("reverse_direction") or scanner.opposite_direction(str(row.get("direction"))))
        return {
            "entry_date": row.get("reverse_entry_date") if not pd.isna(row.get("reverse_entry_date")) else row.get("entry_date"),
            "exit_date": row.get("reverse_exit_date"),
            "selected_direction": direction,
            "exit_reason": row.get("reverse_exit_reason"),
            "entry_price": row.get("reverse_entry_price"),
            "stop_price": row.get("reverse_stop_price"),
            "exit_price": row.get("reverse_exit_price"),
            "risk_points": row.get("reverse_risk_points"),
            "result_r": row.get("reverse_result_r"),
            "raw_result_r": row.get("reverse_raw_result_r"),
            "mfe_r": row.get("reverse_mfe_r"),
            "mae_r": row.get("reverse_mae_r"),
            "hold_minutes": row.get("reverse_hold_minutes"),
            "risk_ticks": row.get("reverse_risk_ticks"),
        }
    return {
        "entry_date": row.get("entry_date"),
        "exit_date": row.get("exit_date"),
        "selected_direction": row.get("direction"),
        "exit_reason": row.get("exit_reason"),
        "entry_price": row.get("entry_price"),
        "stop_price": row.get("stop_price"),
        "exit_price": row.get("exit_price"),
        "risk_points": row.get("risk_points"),
        "result_r": row.get("result_r"),
        "raw_result_r": row.get("raw_result_r"),
        "mfe_r": row.get("mfe_r"),
        "mae_r": row.get("mae_r"),
        "hold_minutes": row.get("hold_minutes"),
        "risk_ticks": row.get("risk_ticks"),
    }


def select_actions(scored: pd.DataFrame, policy: dict[str, float], args: argparse.Namespace) -> pd.DataFrame:
    if scored.empty:
        return scored
    allowed_roots = {root.upper() for root in scanner.parse_list(args.allowed_roots)}
    work = scored.copy()
    if allowed_roots:
        work = work[work["root_symbol"].astype(str).str.upper().isin(allowed_roots)].copy()
    work["selected_action"] = work.apply(lambda row: choose_action(row, policy), axis=1)
    work = work[work["selected_action"] != "skip"].copy()
    if work.empty:
        return work
    work["predicted_action_r"] = np.where(
        work["selected_action"] == "reverse",
        work["predicted_reverse_r"],
        work["predicted_original_r"],
    )
    work = work.sort_values(["entry_date", "predicted_action_r", "signal_score"], ascending=[True, False, False])
    selected_rows: list[dict[str, Any]] = []
    next_available: dict[str, pd.Timestamp] = {}
    root_day_counts: dict[tuple[str, Any], int] = defaultdict(int)
    energy_day_counts: dict[tuple[str, Any], int] = defaultdict(int)
    cooldown = pd.Timedelta(minutes=max(0, int(args.cooldown_minutes)))
    max_root_day = max(0, int(args.max_root_trades_per_day or 0))
    max_energy_day = max(0, int(args.max_energy_trades_per_day or 0))

    for _, row in work.iterrows():
        action = str(row["selected_action"])
        action_fields = row_for_action(row, action)
        result_r = pd.to_numeric(action_fields.get("result_r"), errors="coerce")
        if pd.isna(result_r):
            continue
        entry_at = pd.Timestamp(action_fields["entry_date"])
        exit_value = action_fields.get("exit_date")
        exit_at = pd.Timestamp(exit_value) if not pd.isna(exit_value) else entry_at
        symbol = str(row.get("symbol") or "")
        root = str(row.get("root_symbol") or "").upper()
        trade_day = entry_at.date()
        if symbol and entry_at < next_available.get(symbol, pd.Timestamp.min):
            continue
        root_key = (root, trade_day)
        if max_root_day > 0 and root and root_day_counts[root_key] >= max_root_day:
            continue
        energy_key_name = scanner.energy_throttle_key(root)
        energy_key = (energy_key_name, trade_day)
        if max_energy_day > 0 and energy_key_name and energy_day_counts[energy_key] >= max_energy_day:
            continue

        payload = row.to_dict()
        payload.update(action_fields)
        payload["candidate_direction"] = row.get("direction")
        payload["original_result_r"] = row.get("result_r")
        payload["original_exit_reason"] = row.get("exit_reason")
        payload["original_exit_date"] = row.get("exit_date")
        payload["original_hold_minutes"] = row.get("hold_minutes")
        payload["selected_action"] = action
        payload["predicted_action_r"] = row.get("predicted_action_r")
        payload["result_r"] = float(result_r)
        selected_rows.append(payload)

        if symbol:
            next_available[symbol] = exit_at + cooldown
        if max_root_day > 0 and root:
            root_day_counts[root_key] += 1
        if max_energy_day > 0 and energy_key_name:
            energy_day_counts[energy_key] += 1

    selected = pd.DataFrame(selected_rows)
    if selected.empty:
        return selected
    selected = selected.sort_values(["entry_date", "predicted_action_r"], ascending=[True, False]).reset_index(drop=True)
    selected["selected_index"] = np.arange(1, len(selected) + 1)
    return selected


def summarize(selected: pd.DataFrame) -> dict[str, Any]:
    result = pd.to_numeric(selected.get("result_r", pd.Series(dtype=float)), errors="coerce").fillna(0.0)
    wins = int((result > 0.0).sum())
    losses = int((result <= 0.0).sum())
    values = [float(value) for value in result]
    return {
        "selected_trades": int(len(result)),
        "original_trades": int((selected.get("selected_action", pd.Series(dtype=str)) == "original").sum()) if len(result) else 0,
        "reverse_trades": int((selected.get("selected_action", pd.Series(dtype=str)) == "reverse").sum()) if len(result) else 0,
        "wins": wins,
        "losses": losses,
        "win_rate": wins / len(result) if len(result) else 0.0,
        "avg_r": float(result.mean()) if len(result) else 0.0,
        "sum_r": float(result.sum()) if len(result) else 0.0,
        "max_drawdown_r": max_drawdown(values),
        "cap5_sum_r": float(result.clip(upper=5.0).sum()) if len(result) else 0.0,
        "cap10_sum_r": float(result.clip(upper=10.0).sum()) if len(result) else 0.0,
    }


def score_summary(summary: dict[str, Any]) -> tuple[float, float, float]:
    drawdown = max(float(summary["max_drawdown_r"]), 0.01)
    return (
        float(summary["cap10_sum_r"]) / drawdown,
        float(summary["cap5_sum_r"]),
        float(summary["sum_r"]),
    )


def policy_from_args(args: argparse.Namespace) -> dict[str, float] | None:
    values = [args.original_threshold, args.reverse_threshold, args.reverse_margin, args.original_danger_margin]
    if any(value is None for value in values):
        return None
    return {
        "original_threshold": float(args.original_threshold),
        "reverse_threshold": float(args.reverse_threshold),
        "reverse_margin": float(args.reverse_margin),
        "original_danger_margin": float(args.original_danger_margin),
    }


def choose_policy(conn, args: argparse.Namespace) -> dict[str, float]:
    explicit = policy_from_args(args)
    if explicit is not None:
        return explicit

    threshold_year = int(args.threshold_year)
    train_years = [year for year in scanner.parse_years(args.train_years) if year < threshold_year]
    if not train_years:
        raise RuntimeError("No prior training years are available for action threshold calibration.")
    train = scanner.load_candidates(conn, args.timeframe, train_years, args.formula)
    holdout = scanner.load_candidates(conn, args.timeframe, [threshold_year], args.formula)
    if train.empty or holdout.empty:
        raise RuntimeError("Missing action-model threshold calibration rows.")

    original_model = train_regressor(train, "result_r", args)
    reverse_model = train_regressor(train, "reverse_result_r", args)
    scored = score_frame(holdout, original_model, reverse_model)

    baseline_policy = {
        "original_threshold": 0.014,
        "reverse_threshold": 999.0,
        "reverse_margin": 999.0,
        "original_danger_margin": 999.0,
    }
    baseline_summary = summarize(select_actions(scored, baseline_policy, args))
    max_dd_allowed = baseline_summary["max_drawdown_r"] * max(1.0, float(args.max_dd_multiple))
    min_cap5_allowed = baseline_summary["cap5_sum_r"] * max(0.0, float(args.min_cap5_fraction))

    summaries: list[tuple[dict[str, float], dict[str, Any]]] = []
    for original_threshold in parse_float_grid(args.original_threshold_grid):
        for reverse_threshold in parse_float_grid(args.reverse_threshold_grid):
            for reverse_margin in parse_float_grid(args.reverse_margin_grid):
                for original_danger_margin in parse_float_grid(args.original_danger_margin_grid):
                    policy = {
                        "original_threshold": original_threshold,
                        "reverse_threshold": reverse_threshold,
                        "reverse_margin": reverse_margin,
                        "original_danger_margin": original_danger_margin,
                    }
                    summary = summarize(select_actions(scored, policy, args))
                    summaries.append((policy, summary))

    viable = [
        (policy, summary)
        for policy, summary in summaries
        if summary["selected_trades"] >= args.min_threshold_trades
        and summary["sum_r"] > 0
        and summary["max_drawdown_r"] <= max_dd_allowed
        and summary["cap5_sum_r"] >= min_cap5_allowed
    ]
    if not viable:
        viable = [
            (policy, summary)
            for policy, summary in summaries
            if summary["selected_trades"] >= args.min_threshold_trades and summary["sum_r"] > 0
        ]
    if not viable:
        raise RuntimeError("No viable action policies found during calibration.")

    best_policy, _ = max(viable, key=lambda item: score_summary(item[1]))
    print(f"Action policy calibration on {threshold_year}")
    print(
        "  baseline original-only: "
        f"trades={baseline_summary['selected_trades']:,}, sum={baseline_summary['sum_r']:.1f}R, "
        f"dd={baseline_summary['max_drawdown_r']:.1f}R, cap5={baseline_summary['cap5_sum_r']:.1f}R"
    )
    print("  top policies:")
    for policy, summary in sorted(viable, key=lambda item: score_summary(item[1]), reverse=True)[:10]:
        print(
            "    "
            f"ot={policy['original_threshold']:.4f} rt={policy['reverse_threshold']:.2f} "
            f"rm={policy['reverse_margin']:.2f} odm={policy['original_danger_margin']:.2f}: "
            f"trades={summary['selected_trades']:,}, rev={summary['reverse_trades']:,}, "
            f"win={summary['win_rate'] * 100:.2f}%, sum={summary['sum_r']:.1f}R, "
            f"dd={summary['max_drawdown_r']:.1f}R, cap5={summary['cap5_sum_r']:.1f}R, "
            f"cap10={summary['cap10_sum_r']:.1f}R"
        )
    return best_policy


def insert_selected(conn, run_id: str, selected: pd.DataFrame) -> None:
    if selected.empty:
        return
    base_columns = [
        "action_model_run_id",
        "selected_index",
        "candidate_uid",
        "formula",
        "timeframe",
        "valid_year",
        "root_symbol",
        "symbol",
        "signal_date",
        "entry_date",
        "exit_date",
        "selected_action",
        "candidate_direction",
        "selected_direction",
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
        "predicted_original_r",
        "predicted_reverse_r",
        "predicted_action_r",
        "original_result_r",
        "original_exit_reason",
        "original_exit_date",
        "original_hold_minutes",
        *scanner.counterfactual_column_definitions().keys(),
        *scanner.CAT_FEATURES,
        *scanner.NUM_FEATURES,
    ]
    columns = list(dict.fromkeys(base_columns))
    rows = []
    for row in selected.to_dict("records"):
        payload = {"action_model_run_id": run_id, **row}
        rows.append(tuple(scanner.clean(payload.get(column)) for column in columns))
    placeholders = ",".join(["%s"] * len(columns))
    with conn.cursor() as cur:
        cur.executemany(
            f"""
            INSERT INTO {ACTION_SELECTED_TABLE} ({", ".join(columns)})
            VALUES ({placeholders})
            """,
            rows,
        )


def save_run(
    conn,
    run_id: str,
    args: argparse.Namespace,
    policy: dict[str, float],
    selected: pd.DataFrame,
    original_model: CatBoostRegressor,
    reverse_model: CatBoostRegressor,
) -> dict[str, Any]:
    summary = summarize(selected)
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id
    model_dir.mkdir(parents=True, exist_ok=True)
    original_model.save_model(str(model_dir / "original_model.cbm"))
    reverse_model.save_model(str(model_dir / "reverse_model.cbm"))
    metadata = {
        "action_model_run_id": run_id,
        "base_formula": args.formula,
        "timeframe": args.timeframe,
        "train_years": scanner.parse_years(args.train_years),
        "threshold_year": args.threshold_year,
        "valid_year": args.valid_year,
        "policy": policy,
        "target_clip_min": args.target_clip_min,
        "target_clip_max": args.target_clip_max,
        "context_timeframes": scanner.requested_context_timeframes(args),
        "max_root_trades_per_day": args.max_root_trades_per_day,
        "max_energy_trades_per_day": args.max_energy_trades_per_day,
    }
    (model_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n")
    candidate_rows = scanner.count_candidates(conn, args.timeframe, int(args.valid_year), args.formula)
    with conn.cursor() as cur:
        cur.execute(
            f"""
            INSERT INTO {ACTION_RUN_TABLE} (
                action_model_run_id, base_formula, timeframe, train_years, threshold_year, valid_year,
                original_threshold, reverse_threshold, reverse_margin, original_danger_margin,
                candidate_rows, selected_trades, original_trades, reverse_trades, wins, losses, win_rate,
                avg_r, sum_r, max_drawdown_r, cap5_sum_r, cap10_sum_r, model_path, metadata_json
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, CAST(%s AS JSON))
            """,
            (
                run_id,
                args.formula,
                args.timeframe,
                ",".join(str(year) for year in scanner.parse_years(args.train_years)),
                int(args.threshold_year),
                int(args.valid_year),
                policy["original_threshold"],
                policy["reverse_threshold"],
                policy["reverse_margin"],
                policy["original_danger_margin"],
                candidate_rows,
                summary["selected_trades"],
                summary["original_trades"],
                summary["reverse_trades"],
                summary["wins"],
                summary["losses"],
                summary["win_rate"],
                summary["avg_r"],
                summary["sum_r"],
                summary["max_drawdown_r"],
                summary["cap5_sum_r"],
                summary["cap10_sum_r"],
                str(model_dir),
                json.dumps(metadata, sort_keys=True),
            ),
        )
    insert_selected(conn, run_id, selected)
    conn.commit()
    summary["candidate_rows"] = candidate_rows
    summary["action_model_run_id"] = run_id
    summary["policy"] = policy
    summary["model_path"] = str(model_dir)
    return summary


def main() -> int:
    args = parse_args()
    configure_args(args)
    run_id = run_id_for(args)
    conn = wave.connect()
    try:
        scanner.ensure_tables(conn)
        ensure_tables(conn)
        with conn.cursor() as cur:
            cur.execute(f"SELECT COUNT(*) AS count FROM {ACTION_RUN_TABLE} WHERE action_model_run_id = %s", (run_id,))
            exists = int(cur.fetchone()["count"]) > 0
        if exists and not args.replace_run:
            raise RuntimeError(f"Action run already exists. Use --replace-run to rebuild it: {run_id}")
        delete_run(conn, run_id)
        policy = choose_policy(conn, args)
        train = scanner.load_candidates(conn, args.timeframe, scanner.parse_years(args.train_years), args.formula)
        valid = scanner.load_candidates(conn, args.timeframe, [int(args.valid_year)], args.formula)
        if train.empty:
            raise RuntimeError("No action training candidates found.")
        if valid.empty:
            raise RuntimeError("No action validation candidates found.")
        print(f"Training action models on {len(train):,} candidates from {scanner.parse_years(args.train_years)}")
        original_model = train_regressor(train, "result_r", args)
        reverse_model = train_regressor(train, "reverse_result_r", args)
        scored = score_frame(valid, original_model, reverse_model)
        selected = select_actions(scored, policy, args)
        summary = save_run(conn, run_id, args, policy, selected, original_model, reverse_model)
    except Exception:
        conn.rollback()
        raise
    finally:
        conn.close()

    print(
        f"Materialized {summary['action_model_run_id']}: "
        f"{summary['selected_trades']:,}/{summary['candidate_rows']:,} selected, "
        f"{summary['original_trades']:,} original / {summary['reverse_trades']:,} reverse, "
        f"{summary['wins']} wins / {summary['losses']} losses, "
        f"{summary['win_rate'] * 100:.2f}% win, {summary['avg_r']:.3f}R avg, "
        f"{summary['sum_r']:.1f}R sum, {summary['max_drawdown_r']:.1f}R max DD, "
        f"cap5={summary['cap5_sum_r']:.1f}R, cap10={summary['cap10_sum_r']:.1f}R"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
