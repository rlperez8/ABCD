#!/usr/bin/env python3
"""
Train a live-valid source-exit manager for candle-wave trades.

This model only makes a decision at the moment the source exit rule has fired:

* EXIT_SOURCE - take the source exit now.
* HOLD_FOR_AI - give up the source exit and let the 180 AI/hard-stop/window
  path manage the trade from here.

The old source fallback is never used after a HOLD decision. That keeps the
policy live-valid while still using the source-exit-aware 180 model as a
candidate hold path.
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

import ai_candle_wave_exit_model as exit_model
import ai_candle_wave_scanner_model as scanner
import ai_wave_rider_research as wave


DEFAULT_SOURCE_RUN = "aicw-mtf-eg1-xtight-t014-rd3-en4-v1-2m-2026"
DEFAULT_EXIT_RUN = "aicw-exit-dyn180s2-srcfb-v1-2m-2026"

MANAGER_CAT_FEATURES = list(exit_model.EXIT_CAT_FEATURES)
MANAGER_NUM_FEATURES = list(
    dict.fromkeys(
        [
            *exit_model.EXIT_NUM_FEATURES,
            "exit_score_r",
            "source_result_r",
            "hold_minus_source_r",
            "hold_result_if_ai_r",
        ]
    )
)
LEAKY_FEATURES = {"hold_minus_source_r", "hold_result_if_ai_r"}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-model-run-id", default=DEFAULT_SOURCE_RUN)
    parser.add_argument("--exit-model-run-id", default=DEFAULT_EXIT_RUN)
    parser.add_argument("--run-prefix", default="aicw-src-manager-180-v1")
    parser.add_argument("--train-years", default="2024")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--max-train-trades", type=int, default=0)
    parser.add_argument("--max-threshold-trades", type=int, default=0)
    parser.add_argument("--max-valid-trades", type=int, default=0)
    parser.add_argument("--iterations", type=int, default=550)
    parser.add_argument("--depth", type=int, default=5)
    parser.add_argument("--learning-rate", type=float, default=0.04)
    parser.add_argument("--l2-leaf-reg", type=float, default=12.0)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--target-clip-min", type=float, default=-5.0)
    parser.add_argument("--target-clip-max", type=float, default=5.0)
    parser.add_argument("--exit-threshold", type=float, default=None)
    parser.add_argument("--exclude-roots", default="")
    parser.add_argument("--replace-run", action="store_true")
    return parser.parse_args()


def manager_run_id(args: argparse.Namespace, timeframe: str) -> str:
    run_id = f"{args.run_prefix}-{timeframe}-{args.valid_year}"
    if len(run_id) > 64:
        raise ValueError(f"Run id too long: {run_id}")
    return run_id


def load_exit_metadata_and_model(run_id: str) -> tuple[dict[str, Any], CatBoostRegressor]:
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id
    metadata_path = model_dir / "metadata.json"
    model_path = model_dir / "catboost_exit_model.cbm"
    if not metadata_path.exists():
        raise FileNotFoundError(f"Missing exit metadata: {metadata_path}")
    if not model_path.exists():
        raise FileNotFoundError(f"Missing exit model: {model_path}")
    model = CatBoostRegressor()
    model.load_model(str(model_path))
    return json.loads(metadata_path.read_text(encoding="utf-8-sig")), model


def parse_root_list(raw: Any) -> list[str]:
    if raw is None:
        return []
    if isinstance(raw, list):
        values = raw
    else:
        values = str(raw).split(",")
    return sorted({str(value).strip().upper() for value in values if str(value).strip()})


def exit_args_from_metadata(metadata: dict[str, Any]) -> argparse.Namespace:
    return argparse.Namespace(
        mode=str(metadata.get("mode") or "dynamic"),
        dynamic_max_bars=int(metadata.get("dynamic_max_bars") or 180),
        decision_step_bars=int(metadata.get("decision_step_bars") or 2),
        dynamic_no_signal_exit="terminal",
        exit_threshold=metadata.get("exit_threshold"),
        min_hold_bars=int(metadata.get("min_hold_bars") or 4),
        exclude_source_exit_features=bool(metadata.get("exclude_source_exit_features") or False),
        min_threshold_trades=80,
        max_threshold_dd_r=0.0,
        threshold_dd_penalty=0.0,
    )


def prepare_manager_pool(frame: pd.DataFrame, include_target: bool, args: argparse.Namespace | None = None) -> Pool:
    work = frame.copy()
    num_features = [feature for feature in MANAGER_NUM_FEATURES if feature not in LEAKY_FEATURES]
    for col in MANAGER_CAT_FEATURES:
        if col not in work.columns:
            work[col] = "unknown"
        work[col] = work[col].fillna("unknown").astype(str)
    for col in num_features:
        if col not in work.columns:
            work[col] = 0.0
        work[col] = pd.to_numeric(work[col], errors="coerce").replace([np.inf, -np.inf], np.nan).fillna(0.0)
    features = MANAGER_CAT_FEATURES + num_features
    if include_target:
        target = pd.to_numeric(work["hold_minus_source_r"], errors="coerce").fillna(0.0)
        if args is not None and (args.target_clip_min is not None or args.target_clip_max is not None):
            target = target.clip(lower=args.target_clip_min, upper=args.target_clip_max)
        return Pool(work[features], label=target, cat_features=MANAGER_CAT_FEATURES)
    return Pool(work[features], cat_features=MANAGER_CAT_FEATURES)


def train_manager(frame: pd.DataFrame, args: argparse.Namespace) -> CatBoostRegressor:
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
    model.fit(prepare_manager_pool(frame, include_target=True, args=args))
    return model


def add_manager_predictions(frame: pd.DataFrame, model: CatBoostRegressor) -> pd.DataFrame:
    scored = frame.copy()
    if scored.empty:
        scored["manager_score_r"] = []
        return scored
    scored["manager_score_r"] = model.predict(prepare_manager_pool(scored, include_target=False))
    return scored


def summarize_result(values: pd.Series) -> dict[str, Any]:
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


def manager_rows_from_decisions(
    trades: pd.DataFrame,
    decisions: pd.DataFrame,
    exit_threshold: float | None,
) -> pd.DataFrame:
    if trades.empty or decisions.empty:
        return pd.DataFrame()
    trade_lookup = {str(row.candidate_uid): row for row in trades.itertuples(index=False)}
    rows: list[dict[str, Any]] = []
    work = decisions.copy()
    work["candidate_uid"] = work["candidate_uid"].astype(str)
    work["decision_index"] = pd.to_numeric(work["decision_index"], errors="coerce").fillna(0)
    work["source_exit_signal_seen"] = pd.to_numeric(work["source_exit_signal_seen"], errors="coerce").fillna(0.0)
    work["exit_score_r"] = pd.to_numeric(work["exit_score_r"], errors="coerce")

    for candidate_uid, group in work.groupby("candidate_uid", sort=False):
        trade = trade_lookup.get(str(candidate_uid))
        if trade is None:
            continue
        group = group.sort_values("decision_index").reset_index(drop=True)
        source_rows = group[group["source_exit_signal_seen"] > 0]
        if source_rows.empty:
            continue
        source = source_rows.iloc[0]
        source_decision_index = float(source["decision_index"])
        source_result = float(pd.to_numeric(source.get("baseline_result_r"), errors="coerce"))
        future = group[group["decision_index"] >= source_decision_index].copy()

        chosen = None
        if exit_threshold is not None:
            eligible = future[future["exit_score_r"].fillna(999.0) <= float(exit_threshold)]
            if not eligible.empty:
                chosen = eligible.iloc[0]
        if chosen is not None:
            hold_result = float(pd.to_numeric(chosen.get("exit_now_result_r"), errors="coerce"))
            hold_exit_reason = "model_dynamic_exit"
            hold_exit_date = chosen.get("exit_now_date")
            hold_exit_score = chosen.get("exit_score_r")
        else:
            terminal = group.iloc[-1]
            hold_result = float(pd.to_numeric(terminal.get("terminal_result_r"), errors="coerce"))
            hold_exit_reason = str(terminal.get("terminal_exit_reason") or "dynamic_terminal")
            hold_exit_date = terminal.get("terminal_exit_date")
            hold_exit_score = None

        if not math.isfinite(source_result) or not math.isfinite(hold_result):
            continue

        payload = source.to_dict()
        payload.update(
            {
                "source_result_r": source_result,
                "source_exit_date": getattr(trade, "exit_date", None),
                "hold_result_if_ai_r": hold_result,
                "hold_exit_reason": hold_exit_reason,
                "hold_exit_date": hold_exit_date,
                "hold_exit_score_r": hold_exit_score,
                "hold_minus_source_r": hold_result - source_result,
                "oracle_manager_action": "HOLD_FOR_AI" if hold_result > source_result else "EXIT_SOURCE",
                "entry_date": getattr(trade, "entry_date", None),
                "trade_symbol": getattr(trade, "symbol", None),
                "trade_root_symbol": getattr(trade, "root_symbol", None),
                "trade_direction": getattr(trade, "direction", None),
            }
        )
        rows.append(payload)
    frame = pd.DataFrame(rows)
    if frame.empty:
        return frame
    for col in ["decision_date", "source_exit_date", "hold_exit_date", "entry_date"]:
        if col in frame.columns:
            frame[col] = pd.to_datetime(frame[col], errors="coerce")
    return frame.sort_values(["source_exit_date", "decision_date", "candidate_uid"]).reset_index(drop=True)


def score_policy(frame: pd.DataFrame, threshold: float | None) -> pd.DataFrame:
    scored = frame.copy()
    if threshold is None:
        scored["manager_action"] = "EXIT_SOURCE"
    else:
        scored["manager_action"] = np.where(scored["manager_score_r"] >= threshold, "HOLD_FOR_AI", "EXIT_SOURCE")
    scored["manager_result_r"] = np.where(
        scored["manager_action"] == "HOLD_FOR_AI",
        pd.to_numeric(scored["hold_result_if_ai_r"], errors="coerce").fillna(0.0),
        pd.to_numeric(scored["source_result_r"], errors="coerce").fillna(0.0),
    )
    return scored


def choose_threshold(scored: pd.DataFrame) -> float | None:
    if scored.empty:
        return None
    score = pd.to_numeric(scored["manager_score_r"], errors="coerce").dropna()
    if score.empty:
        return None
    candidates: list[float | None] = [None, 0.0]
    candidates.extend(sorted(set(float(value) for value in score.quantile(np.linspace(0.01, 0.99, 40)).dropna())))
    best_threshold: float | None = None
    best_summary = summarize_result(score_policy(scored, None)["manager_result_r"])
    best_key = (
        best_summary["sum_r"],
        -best_summary["max_drawdown_r"],
        best_summary["avg_r"],
    )
    for threshold in candidates:
        result = score_policy(scored, threshold)
        summary = summarize_result(result["manager_result_r"])
        key = (summary["sum_r"], -summary["max_drawdown_r"], summary["avg_r"])
        if key > best_key:
            best_key = key
            best_summary = summary
            best_threshold = threshold
    print(
        f"Manager threshold selected {'EXIT_SOURCE only' if best_threshold is None else f'{best_threshold:.4f}'}: "
        f"sum={best_summary['sum_r']:.2f}R dd={best_summary['max_drawdown_r']:.2f}R"
    )
    return best_threshold


def print_policy_summary(label: str, frame: pd.DataFrame) -> None:
    summary = summarize_result(frame["manager_result_r"])
    actions = frame["manager_action"].value_counts(dropna=False).to_dict()
    print(
        f"{label}: trades={summary['trades']:,} win={summary['win_rate'] * 100:.2f}% "
        f"sum={summary['sum_r']:.2f}R avg={summary['avg_r']:.4f}R dd={summary['max_drawdown_r']:.2f}R "
        f"actions={actions}"
    )


def build_manager_rows_for_years(
    conn,
    entry_model: CatBoostRegressor,
    entry_args: argparse.Namespace,
    exit_ai_model: CatBoostRegressor,
    exit_ai_args: argparse.Namespace,
    years: list[int],
    limit: int,
    label: str,
    exclude_roots: list[str],
) -> pd.DataFrame:
    trades = exit_model.selected_like_trades(conn, entry_model, entry_args, years, limit)
    if exclude_roots and not trades.empty:
        before = len(trades)
        roots = trades["root_symbol"].fillna("").astype(str).str.upper()
        trades = trades[~roots.isin(exclude_roots)].copy()
        print(f"{label}: excluded_roots={exclude_roots} removed={before - len(trades):,} remaining={len(trades):,}")
    table_name, timeframe_minutes = scanner.table_for_timeframe(str(entry_args.timeframe))
    decisions = exit_model.build_decision_rows(conn, trades, table_name, timeframe_minutes, exit_ai_args, False, label)
    decisions = exit_model.add_exit_predictions(decisions, exit_ai_model, exit_ai_args)
    rows = manager_rows_from_decisions(trades, decisions, exit_ai_args.exit_threshold)
    print(f"{label}: manager rows={len(rows):,} from trades={len(trades):,}")
    return rows


def save_model_and_metadata(
    run_id: str,
    model: CatBoostRegressor,
    args: argparse.Namespace,
    threshold: float | None,
    threshold_summary: dict[str, Any],
    valid_summary: dict[str, Any],
) -> None:
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id
    if model_dir.exists() and not args.replace_run:
        raise ValueError(f"Manager run already exists: {run_id}. Use --replace-run to overwrite.")
    model_dir.mkdir(parents=True, exist_ok=True)
    model.save_model(str(model_dir / "catboost_manager_model.cbm"))
    metadata = {
        "manager_model_run_id": run_id,
        "source_model_run_id": args.source_model_run_id,
        "exit_model_run_id": args.exit_model_run_id,
        "train_years": scanner.parse_years(args.train_years),
        "threshold_year": args.threshold_year,
        "valid_year": args.valid_year,
        "manager_threshold": threshold,
        "policy_exit_threshold": args.exit_threshold,
        "policy_exclude_roots": parse_root_list(args.exclude_roots),
        "cat_features": MANAGER_CAT_FEATURES,
        "num_features": [feature for feature in MANAGER_NUM_FEATURES if feature not in LEAKY_FEATURES],
        "target": "hold_minus_source_r",
        "policy": "At source exit, choose EXIT_SOURCE or HOLD_FOR_AI. HOLD cannot use source fallback later.",
        "threshold_summary": threshold_summary,
        "valid_summary": valid_summary,
    }
    (model_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=str))


def main() -> int:
    args = parse_args()
    run_id = manager_run_id(args, "2m")
    conn = wave.connect()
    try:
        source = exit_model.load_source_run(conn, args.source_model_run_id)
        entry_args = exit_model.entry_args_from_metadata(source["metadata"])
        timeframe = str(entry_args.timeframe)
        run_id = manager_run_id(args, timeframe)
        entry_ai = exit_model.load_entry_model(source)
        exit_metadata, exit_ai = load_exit_metadata_and_model(args.exit_model_run_id)
        exit_ai_args = exit_args_from_metadata(exit_metadata)
        if args.exit_threshold is not None:
            exit_ai_args.exit_threshold = float(args.exit_threshold)
        if exit_ai_args.exit_threshold is None:
            raise ValueError("The source exit AI model must have an exit threshold.")
        exclude_roots = parse_root_list(args.exclude_roots)

        train_rows = build_manager_rows_for_years(
            conn,
            entry_ai,
            entry_args,
            exit_ai,
            exit_ai_args,
            scanner.parse_years(args.train_years),
            args.max_train_trades,
            "train",
            exclude_roots,
        )
        if train_rows.empty:
            raise ValueError("No train manager rows were generated.")
        model = train_manager(train_rows, args)

        threshold_rows = build_manager_rows_for_years(
            conn,
            entry_ai,
            entry_args,
            exit_ai,
            exit_ai_args,
            [args.threshold_year],
            args.max_threshold_trades,
            "threshold",
            exclude_roots,
        )
        threshold_scored = add_manager_predictions(threshold_rows, model)
        threshold = choose_threshold(threshold_scored)
        threshold_policy = score_policy(threshold_scored, threshold)
        print_policy_summary("threshold manager", threshold_policy)
        print_policy_summary("threshold always_source", score_policy(threshold_scored, None))
        threshold_always_hold = threshold_scored.copy()
        threshold_always_hold["manager_action"] = "HOLD_FOR_AI"
        threshold_always_hold["manager_result_r"] = threshold_always_hold["hold_result_if_ai_r"]
        print_policy_summary("threshold always_hold", threshold_always_hold)

        valid_rows = build_manager_rows_for_years(
            conn,
            entry_ai,
            entry_args,
            exit_ai,
            exit_ai_args,
            [args.valid_year],
            args.max_valid_trades,
            "valid",
            exclude_roots,
        )
        valid_scored = add_manager_predictions(valid_rows, model)
        valid_policy = score_policy(valid_scored, threshold)
        valid_source = score_policy(valid_scored, None)
        valid_hold = valid_scored.copy()
        valid_hold["manager_action"] = "HOLD_FOR_AI"
        valid_hold["manager_result_r"] = valid_hold["hold_result_if_ai_r"]
        print_policy_summary("valid manager", valid_policy)
        print_policy_summary("valid always_source", valid_source)
        print_policy_summary("valid always_hold", valid_hold)

        save_model_and_metadata(
            run_id,
            model,
            args,
            threshold,
            summarize_result(threshold_policy["manager_result_r"]),
            summarize_result(valid_policy["manager_result_r"]),
        )
        print(f"Saved manager model: {run_id}")
    finally:
        conn.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
