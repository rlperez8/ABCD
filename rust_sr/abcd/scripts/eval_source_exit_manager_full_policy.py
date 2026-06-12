#!/usr/bin/env python3
"""
Evaluate the source-exit manager as a full live policy.

Trades with a source exit inside the decision window use the manager:
EXIT_SOURCE or HOLD_FOR_AI.

Trades without a source-exit decision point use the live AI/window path:
AI exit if it fires, otherwise hard-stop/window terminal.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

import numpy as np
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
import ai_candle_wave_source_exit_manager_model as manager
import ai_wave_rider_research as wave


DEFAULT_MANAGER_RUN = "aicw-src-manager-180-v1-2m-2026"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manager-run-id", default=DEFAULT_MANAGER_RUN)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--max-valid-trades", type=int, default=0)
    parser.add_argument("--output", default="")
    parser.add_argument(
        "--manager-threshold",
        type=float,
        default=None,
        help="Override the saved manager threshold for policy evaluation.",
    )
    parser.add_argument(
        "--exit-threshold",
        type=float,
        default=None,
        help="Override the saved AI exit threshold for live-path evaluation.",
    )
    parser.add_argument("--sweep-thresholds", action="store_true")
    parser.add_argument("--sweep-exit-thresholds", action="store_true")
    parser.add_argument(
        "--exit-threshold-values",
        default="",
        help="Comma-separated AI exit thresholds to test when sweeping exit thresholds.",
    )
    parser.add_argument(
        "--exclude-roots",
        default="",
        help="Comma-separated root symbols to exclude from the evaluated live policy.",
    )
    return parser.parse_args()


def load_manager(run_id: str) -> tuple[dict[str, Any], CatBoostRegressor]:
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id
    metadata_path = model_dir / "metadata.json"
    model_path = model_dir / "catboost_manager_model.cbm"
    if not metadata_path.exists():
        raise FileNotFoundError(f"Missing manager metadata: {metadata_path}")
    if not model_path.exists():
        raise FileNotFoundError(f"Missing manager model: {model_path}")
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


def live_ai_path_results(decisions: pd.DataFrame, exit_threshold: float | None) -> dict[str, dict[str, Any]]:
    results: dict[str, dict[str, Any]] = {}
    if decisions.empty:
        return results
    work = decisions.copy()
    work["candidate_uid"] = work["candidate_uid"].astype(str)
    work["decision_index"] = pd.to_numeric(work["decision_index"], errors="coerce").fillna(0)
    work["exit_score_r"] = pd.to_numeric(work["exit_score_r"], errors="coerce")
    for candidate_uid, group in work.groupby("candidate_uid", sort=False):
        group = group.sort_values("decision_index")
        chosen = None
        if exit_threshold is not None:
            eligible = group[group["exit_score_r"].fillna(999.0) <= float(exit_threshold)]
            if not eligible.empty:
                chosen = eligible.iloc[0]
        if chosen is not None:
            results[str(candidate_uid)] = {
                "result_r": float(pd.to_numeric(chosen.get("exit_now_result_r"), errors="coerce")),
                "action": "AI_EXIT",
                "exit_reason": "model_dynamic_exit",
                "exit_date": chosen.get("exit_now_date"),
                "exit_score_r": chosen.get("exit_score_r"),
            }
        else:
            terminal = group.iloc[-1]
            results[str(candidate_uid)] = {
                "result_r": float(pd.to_numeric(terminal.get("terminal_result_r"), errors="coerce")),
                "action": "WINDOW_PATH",
                "exit_reason": str(terminal.get("terminal_exit_reason") or "dynamic_terminal"),
                "exit_date": terminal.get("terminal_exit_date"),
                "exit_score_r": None,
            }
    return results


def summarize(values: pd.Series) -> dict[str, Any]:
    result = pd.to_numeric(values, errors="coerce").fillna(0.0)
    equity = result.cumsum()
    drawdown = equity.cummax() - equity
    wins = int((result > 0).sum())
    return {
        "trades": int(len(result)),
        "wins": wins,
        "losses": int((result <= 0).sum()),
        "win_rate": wins / len(result) if len(result) else 0.0,
        "avg_r": float(result.mean()) if len(result) else 0.0,
        "sum_r": float(result.sum()) if len(result) else 0.0,
        "max_drawdown_r": float(drawdown.max()) if len(drawdown) else 0.0,
    }


def print_summary(label: str, rows: pd.DataFrame) -> None:
    stats = summarize(rows["result_r"])
    actions = rows["action"].value_counts(dropna=False).to_dict()
    print(
        f"{label}: trades={stats['trades']:,} win={stats['win_rate'] * 100:.2f}% "
        f"sum={stats['sum_r']:.2f}R avg={stats['avg_r']:.4f}R dd={stats['max_drawdown_r']:.2f}R "
        f"actions={actions}"
    )


def assemble_policy_frame(
    trades: pd.DataFrame,
    manager_scored: pd.DataFrame,
    live_ai: dict[str, dict[str, Any]],
    threshold: float | None,
) -> pd.DataFrame:
    manager_policy = manager.score_policy(manager_scored, threshold)
    manager_by_uid = {
        str(row.candidate_uid): row
        for row in manager_policy.itertuples(index=False)
    }
    records: list[dict[str, Any]] = []
    for trade in trades.itertuples(index=False):
        candidate_uid = str(trade.candidate_uid)
        managed = manager_by_uid.get(candidate_uid)
        if managed is not None:
            action = str(managed.manager_action)
            result_r = float(managed.manager_result_r)
            exit_reason = "source_exit" if action == "EXIT_SOURCE" else str(managed.hold_exit_reason)
            exit_date = managed.source_exit_date if action == "EXIT_SOURCE" else managed.hold_exit_date
        else:
            path = live_ai.get(candidate_uid)
            if path is None:
                action = "NO_DECISION_ROWS"
                result_r = float(getattr(trade, "result_r", 0.0) or 0.0)
                exit_reason = str(getattr(trade, "exit_reason", "unknown"))
                exit_date = getattr(trade, "exit_date", None)
            else:
                action = f"NO_SOURCE_{path['action']}"
                result_r = float(path["result_r"])
                exit_reason = str(path["exit_reason"])
                exit_date = path["exit_date"]
        records.append(
            {
                "candidate_uid": candidate_uid,
                "entry_date": getattr(trade, "entry_date", None),
                "symbol": getattr(trade, "symbol", None),
                "root_symbol": getattr(trade, "root_symbol", None),
                "direction": getattr(trade, "direction", None),
                "action": action,
                "exit_reason": exit_reason,
                "exit_date": exit_date,
                "result_r": result_r,
            }
        )
    frame = pd.DataFrame(records)
    for col in ["entry_date", "exit_date"]:
        frame[col] = pd.to_datetime(frame[col], errors="coerce")
    return frame.sort_values(["entry_date", "candidate_uid"]).reset_index(drop=True)


def main() -> int:
    args = parse_args()
    metadata, manager_model = load_manager(args.manager_run_id)
    source_model_run_id = str(metadata["source_model_run_id"])
    exit_model_run_id = str(metadata["exit_model_run_id"])
    threshold = args.manager_threshold if args.manager_threshold is not None else metadata.get("manager_threshold")

    conn = wave.connect()
    try:
        source = exit_model.load_source_run(conn, source_model_run_id)
        entry_args = exit_model.entry_args_from_metadata(source["metadata"])
        entry_ai = exit_model.load_entry_model(source)
        exit_metadata, exit_ai = manager.load_exit_metadata_and_model(exit_model_run_id)
        exit_ai_args = manager.exit_args_from_metadata(exit_metadata)
        if args.exit_threshold is not None:
            exit_ai_args.exit_threshold = float(args.exit_threshold)
        elif metadata.get("policy_exit_threshold") is not None:
            exit_ai_args.exit_threshold = float(metadata["policy_exit_threshold"])
        table_name, timeframe_minutes = scanner.table_for_timeframe(str(entry_args.timeframe))

        trades = exit_model.selected_like_trades(conn, entry_ai, entry_args, [args.valid_year], args.max_valid_trades)
        exclude_roots = parse_root_list(args.exclude_roots) or parse_root_list(metadata.get("policy_exclude_roots"))
        if exclude_roots and not trades.empty:
            before = len(trades)
            roots = trades["root_symbol"].fillna("").astype(str).str.upper()
            trades = trades[~roots.isin(exclude_roots)].copy()
            print(f"excluded_roots={exclude_roots} removed={before - len(trades):,} remaining={len(trades):,}")
        decisions = exit_model.build_decision_rows(conn, trades, table_name, timeframe_minutes, exit_ai_args, False, "valid")
        decisions = exit_model.add_exit_predictions(decisions, exit_ai, exit_ai_args)

        manager_rows = manager.manager_rows_from_decisions(trades, decisions, exit_ai_args.exit_threshold)
        manager_scored = manager.add_manager_predictions(manager_rows, manager_model)
        live_ai = live_ai_path_results(decisions, exit_ai_args.exit_threshold)

        frame = assemble_policy_frame(trades, manager_scored, live_ai, threshold)
        print(f"Full manager policy run={args.manager_run_id} trades={len(frame):,} manager_rows={len(manager_rows):,}")
        print_summary("full_manager_policy", frame)

        source_like = frame.copy()
        manager_policy = manager.score_policy(manager_scored, threshold)
        source_rows = manager_policy[["candidate_uid", "source_result_r"]].copy()
        source_map = dict(zip(source_rows["candidate_uid"].astype(str), pd.to_numeric(source_rows["source_result_r"], errors="coerce")))
        source_like["result_r"] = [
            float(source_map.get(str(row.candidate_uid), row.result_r))
            for row in frame.itertuples(index=False)
        ]
        source_like["action"] = [
            "EXIT_SOURCE" if str(row.candidate_uid) in source_map else row.action
            for row in frame.itertuples(index=False)
        ]
        print_summary("full_source_when_available", source_like)

        hold_like = frame.copy()
        hold_rows = manager_policy[["candidate_uid", "hold_result_if_ai_r"]].copy()
        hold_map = dict(zip(hold_rows["candidate_uid"].astype(str), pd.to_numeric(hold_rows["hold_result_if_ai_r"], errors="coerce")))
        hold_like["result_r"] = [
            float(hold_map.get(str(row.candidate_uid), row.result_r))
            for row in frame.itertuples(index=False)
        ]
        hold_like["action"] = [
            "HOLD_FOR_AI" if str(row.candidate_uid) in hold_map else row.action
            for row in frame.itertuples(index=False)
        ]
        print_summary("full_hold_when_available", hold_like)

        if args.sweep_thresholds:
            scores = pd.to_numeric(manager_scored["manager_score_r"], errors="coerce").dropna()
            candidates: list[float | None] = [None, 0.0]
            candidates.extend(sorted(set(float(value) for value in scores.quantile(np.linspace(0.01, 0.99, 45)).dropna())))
            rows = []
            for candidate in candidates:
                sweep_frame = assemble_policy_frame(trades, manager_scored, live_ai, candidate)
                stats = summarize(sweep_frame["result_r"])
                rows.append(
                    {
                        "threshold": candidate,
                        **stats,
                        "holds": int((sweep_frame["action"] == "HOLD_FOR_AI").sum()),
                        "sources": int((sweep_frame["action"] == "EXIT_SOURCE").sum()),
                    }
                )
            sweep = pd.DataFrame(rows)
            print("threshold sweep top by sum:")
            for row in sweep.sort_values(["sum_r", "avg_r"], ascending=[False, False]).head(12).itertuples(index=False):
                label = "source_only" if pd.isna(row.threshold) else f"{row.threshold:.4f}"
                print(
                    f"  {label}: sum={row.sum_r:.2f}R dd={row.max_drawdown_r:.2f}R "
                    f"win={row.win_rate * 100:.2f}% holds={row.holds} sources={row.sources}"
                )
            print("threshold sweep top by low DD with positive sum:")
            positive = sweep[sweep["sum_r"] > 0].copy()
            for row in positive.sort_values(["max_drawdown_r", "sum_r"], ascending=[True, False]).head(8).itertuples(index=False):
                label = "source_only" if pd.isna(row.threshold) else f"{row.threshold:.4f}"
                print(
                    f"  {label}: sum={row.sum_r:.2f}R dd={row.max_drawdown_r:.2f}R "
                    f"win={row.win_rate * 100:.2f}% holds={row.holds} sources={row.sources}"
                )

        if args.sweep_exit_thresholds:
            scores = pd.to_numeric(decisions["exit_score_r"], errors="coerce").dropna()
            if args.exit_threshold_values.strip():
                exit_candidates = [float(value.strip()) for value in args.exit_threshold_values.split(",") if value.strip()]
            else:
                exit_candidates: list[float | None] = [None, float(exit_ai_args.exit_threshold)]
                exit_candidates.extend(sorted(set(float(value) for value in scores.quantile(np.linspace(0.05, 0.95, 10)).dropna())))
            rows = []
            for exit_candidate in exit_candidates:
                candidate_manager_rows = manager.manager_rows_from_decisions(trades, decisions, exit_candidate)
                candidate_manager_scored = manager.add_manager_predictions(candidate_manager_rows, manager_model)
                candidate_live_ai = live_ai_path_results(decisions, exit_candidate)
                sweep_frame = assemble_policy_frame(trades, candidate_manager_scored, candidate_live_ai, threshold)
                stats = summarize(sweep_frame["result_r"])
                rows.append(
                    {
                        "exit_threshold": exit_candidate,
                        **stats,
                        "holds": int((sweep_frame["action"] == "HOLD_FOR_AI").sum()),
                        "sources": int((sweep_frame["action"] == "EXIT_SOURCE").sum()),
                        "no_source_window": int((sweep_frame["action"] == "NO_SOURCE_WINDOW_PATH").sum()),
                        "no_source_ai": int((sweep_frame["action"] == "NO_SOURCE_AI_EXIT").sum()),
                    }
                )
            exit_sweep = pd.DataFrame(rows)
            print("exit threshold sweep top by sum:")
            for row in exit_sweep.sort_values(["sum_r", "avg_r"], ascending=[False, False]).head(12).itertuples(index=False):
                label = "terminal_only" if pd.isna(row.exit_threshold) else f"{row.exit_threshold:.4f}"
                print(
                    f"  {label}: sum={row.sum_r:.2f}R dd={row.max_drawdown_r:.2f}R "
                    f"win={row.win_rate * 100:.2f}% holds={row.holds} sources={row.sources} "
                    f"no_source_window={row.no_source_window} no_source_ai={row.no_source_ai}"
                )
            print("exit threshold sweep top by low DD with positive sum:")
            positive_exit = exit_sweep[exit_sweep["sum_r"] > 0].copy()
            for row in positive_exit.sort_values(["max_drawdown_r", "sum_r"], ascending=[True, False]).head(8).itertuples(index=False):
                label = "terminal_only" if pd.isna(row.exit_threshold) else f"{row.exit_threshold:.4f}"
                print(
                    f"  {label}: sum={row.sum_r:.2f}R dd={row.max_drawdown_r:.2f}R "
                    f"win={row.win_rate * 100:.2f}% holds={row.holds} sources={row.sources} "
                    f"no_source_window={row.no_source_window} no_source_ai={row.no_source_ai}"
                )

        output = Path(args.output) if args.output else wave.ABCD_ROOT / "logs" / f"{args.manager_run_id}_full_policy_{args.valid_year}.csv"
        output.parent.mkdir(parents=True, exist_ok=True)
        frame.to_csv(output, index=False)
        print(f"wrote={output}")
    finally:
        conn.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
