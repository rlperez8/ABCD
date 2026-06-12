#!/usr/bin/env python3
"""
Train leak-safe visual models for trend starts and trend endings.

This script focuses the image task on the chart moment itself:

* entry_start: image ends before the entry candle. Label asks whether price
  starts moving in the trade direction before meaningful adverse movement.
* exit_end: image ends at an exit decision candle. Label asks whether exiting
  now is close to the best future exit inside the live window.

The image never includes future candles. Future candles are only used for
offline labels.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path
from typing import Any

import joblib
import numpy as np
import pandas as pd
from sklearn.metrics import accuracy_score, log_loss, roc_auc_score
from sklearn.neural_network import MLPClassifier


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_exit_model as exit_model
import ai_candle_wave_scanner_model as scanner
import ai_candle_wave_source_exit_manager_model as manager
import ai_candle_wave_visual_entry_model as visual
import ai_wave_rider_research as wave


DEFAULT_POLICY_RUN = "aicw-src-manager-180-livefix-rootgate25-v1-2m-2026"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--task", choices=["entry_start", "exit_end"], default="entry_start")
    parser.add_argument("--policy-run-id", default=DEFAULT_POLICY_RUN)
    parser.add_argument("--run-prefix", default="aicw-visual-trendpoint-v1")
    parser.add_argument("--train-year", type=int, default=2024)
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--lookback-bars", type=int, default=64)
    parser.add_argument("--height", type=int, default=64)
    parser.add_argument("--width", type=int, default=64)
    parser.add_argument("--min-candles", type=int, default=40)
    parser.add_argument("--history-multiplier", type=int, default=4)
    parser.add_argument("--entry-label-bars", type=int, default=40)
    parser.add_argument("--entry-start-target-r", type=float, default=0.75)
    parser.add_argument("--entry-start-fail-r", type=float, default=0.50)
    parser.add_argument("--exit-end-margin-r", type=float, default=0.10)
    parser.add_argument("--max-decision-rows-per-trade", type=int, default=12)
    parser.add_argument("--max-rows-per-split", type=int, default=0)
    parser.add_argument("--hidden-layers", default="64,16")
    parser.add_argument("--max-iter", type=int, default=160)
    parser.add_argument("--alpha", type=float, default=0.004)
    parser.add_argument("--learning-rate", type=float, default=0.001)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--min-keep-share", type=float, default=0.35)
    parser.add_argument("--min-keep-rows", type=int, default=150)
    parser.add_argument("--replace-run", action="store_true")
    return parser.parse_args()


def run_id(args: argparse.Namespace) -> str:
    rid = f"{args.run_prefix}-{args.task}-2m-{args.valid_year}"
    if len(rid) > 64:
        raise ValueError(f"Run id too long: {rid}")
    return rid


def load_policy_metadata(policy_run_id: str) -> dict[str, Any]:
    path = wave.ABCD_ROOT / "model_registry" / policy_run_id / "metadata.json"
    if not path.exists():
        raise FileNotFoundError(f"Missing policy metadata: {path}")
    return json.loads(path.read_text(encoding="utf-8-sig"))


def parse_root_list(raw: Any) -> list[str]:
    if raw is None:
        return []
    values = raw if isinstance(raw, list) else str(raw).split(",")
    return sorted({str(value).strip().upper() for value in values if str(value).strip()})


def hidden_layers(raw: str) -> tuple[int, ...]:
    layers = tuple(int(part.strip()) for part in raw.split(",") if part.strip())
    if not layers:
        raise ValueError("--hidden-layers must contain at least one value")
    return layers


def load_policy_rows(policy_run_id: str, year: int, limit: int = 0) -> pd.DataFrame:
    path = wave.ABCD_ROOT / "logs" / f"{policy_run_id}_full_policy_{year}.csv"
    if not path.exists():
        raise FileNotFoundError(f"Missing policy CSV: {path}")
    rows = pd.read_csv(path)
    rows["entry_date"] = pd.to_datetime(rows["entry_date"], errors="coerce")
    rows["exit_date"] = pd.to_datetime(rows.get("exit_date"), errors="coerce")
    rows["result_r"] = pd.to_numeric(rows["result_r"], errors="coerce")
    rows = rows.dropna(subset=["candidate_uid", "entry_date", "symbol", "direction", "result_r"]).copy()
    rows["candidate_uid"] = rows["candidate_uid"].astype(str)
    rows = rows.sort_values(["entry_date", "candidate_uid"]).reset_index(drop=True)
    if limit > 0:
        rows = rows.head(limit).copy()
    return rows


def load_selected_details(
    conn,
    source_model_run_id: str,
    years: list[int],
    exclude_roots: list[str],
) -> tuple[pd.DataFrame, argparse.Namespace]:
    source = exit_model.load_source_run(conn, source_model_run_id)
    entry_args = exit_model.entry_args_from_metadata(source["metadata"])
    entry_ai = exit_model.load_entry_model(source)
    selected = exit_model.selected_like_trades(conn, entry_ai, entry_args, years, 0)
    if exclude_roots and not selected.empty:
        roots = selected["root_symbol"].fillna("").astype(str).str.upper()
        selected = selected[~roots.isin(exclude_roots)].copy()
    selected["candidate_uid"] = selected["candidate_uid"].astype(str)
    return selected, entry_args


def merge_policy_details(policy: pd.DataFrame, details: pd.DataFrame) -> pd.DataFrame:
    detail_cols = [
        "candidate_uid",
        "entry_price",
        "stop_price",
        "risk_points",
        "risk_ticks",
        "tick_size",
        "signal_date",
    ]
    available = [col for col in detail_cols if col in details.columns]
    merged = policy.merge(details[available], on="candidate_uid", how="left")
    for col in ["entry_price", "stop_price", "risk_points", "risk_ticks", "tick_size"]:
        if col in merged.columns:
            merged[col] = pd.to_numeric(merged[col], errors="coerce")
    return merged.dropna(subset=["entry_price", "risk_points"]).copy()


def favorable_adverse_r(row: pd.Series, candle: pd.Series) -> tuple[float, float]:
    direction = str(row["direction"]).upper()
    entry_price = float(row["entry_price"])
    risk_points = max(1e-9, float(row["risk_points"]))
    if direction == "LONG":
        favorable = (float(candle["high"]) - entry_price) / risk_points
        adverse = (entry_price - float(candle["low"])) / risk_points
    else:
        favorable = (entry_price - float(candle["low"])) / risk_points
        adverse = (float(candle["high"]) - entry_price) / risk_points
    return favorable, adverse


def entry_start_label(candles: pd.DataFrame, entry_idx: int, row: pd.Series, args: argparse.Namespace) -> tuple[int, str]:
    max_idx = min(len(candles) - 1, entry_idx + int(args.entry_label_bars))
    best_favorable = -float("inf")
    worst_adverse = -float("inf")
    for idx in range(entry_idx, max_idx + 1):
        favorable, adverse = favorable_adverse_r(row, candles.iloc[idx])
        best_favorable = max(best_favorable, favorable)
        worst_adverse = max(worst_adverse, adverse)
        if favorable >= float(args.entry_start_target_r):
            return 1, "target_first"
        if adverse >= float(args.entry_start_fail_r):
            return 0, "fail_first"
    if best_favorable >= float(args.entry_start_target_r) * 0.66 and worst_adverse < float(args.entry_start_fail_r):
        return 1, "partial_clean_start"
    return 0, "no_start"


def build_entry_dataset(
    conn,
    rows: pd.DataFrame,
    table_name: str,
    timeframe_minutes: int,
    args: argparse.Namespace,
    label: str,
) -> tuple[np.ndarray, pd.DataFrame]:
    images: list[np.ndarray] = []
    records: list[dict[str, Any]] = []
    total = len(rows)
    for count, row_tuple in enumerate(rows.itertuples(index=False), start=1):
        row = pd.Series(row_tuple._asdict())
        entry_date = pd.Timestamp(row["entry_date"])
        start_at = entry_date - pd.Timedelta(
            minutes=timeframe_minutes * int(args.lookback_bars) * max(1, int(args.history_multiplier))
        )
        end_at = entry_date + pd.Timedelta(minutes=timeframe_minutes * int(args.entry_label_bars + 2))
        candles = exit_model.load_candles(conn, table_name, str(row["symbol"]), start_at, end_at)
        if candles.empty:
            continue
        entry_idx = exit_model.index_at_or_after(candles["ts_utc"], entry_date)
        if entry_idx is None:
            continue
        context = candles[candles["ts_utc"] < entry_date].tail(int(args.lookback_bars)).reset_index(drop=True)
        if len(context) < int(args.min_candles):
            continue
        target, target_reason = entry_start_label(candles, entry_idx, row, args)
        image = visual.render_chart_image(context, str(row["direction"]), int(args.height), int(args.width))
        payload = row.to_dict()
        payload.update({"visual_target": int(target), "visual_target_reason": target_reason})
        images.append(image.reshape(-1))
        records.append(payload)
        if count % 250 == 0 or count == total:
            print(f"{label}: entry images {count:,}/{total:,}, kept={len(records):,}")
    if not images:
        return np.empty((0, 3 * args.height * args.width), dtype=np.float32), pd.DataFrame()
    return np.stack(images).astype(np.float32), pd.DataFrame(records)


def load_exit_args(policy_metadata: dict[str, Any]) -> argparse.Namespace:
    exit_run_id = str(policy_metadata["exit_model_run_id"])
    exit_metadata, _ = manager.load_exit_metadata_and_model(exit_run_id)
    exit_args = manager.exit_args_from_metadata(exit_metadata)
    if policy_metadata.get("policy_exit_threshold") is not None:
        exit_args.exit_threshold = float(policy_metadata["policy_exit_threshold"])
    return exit_args


def build_decision_base(
    conn,
    policy_rows: pd.DataFrame,
    selected_details: pd.DataFrame,
    policy_metadata: dict[str, Any],
    table_name: str,
    timeframe_minutes: int,
    label: str,
) -> pd.DataFrame:
    exit_run_id = str(policy_metadata["exit_model_run_id"])
    exit_metadata, exit_ai = manager.load_exit_metadata_and_model(exit_run_id)
    exit_args = manager.exit_args_from_metadata(exit_metadata)
    if policy_metadata.get("policy_exit_threshold") is not None:
        exit_args.exit_threshold = float(policy_metadata["policy_exit_threshold"])
    policy_ids = set(policy_rows["candidate_uid"].astype(str))
    trades = selected_details[selected_details["candidate_uid"].astype(str).isin(policy_ids)].copy()
    decisions = exit_model.build_decision_rows(conn, trades, table_name, timeframe_minutes, exit_args, True, label)
    decisions = exit_model.add_exit_predictions(decisions, exit_ai, exit_args)
    decisions["candidate_uid"] = decisions["candidate_uid"].astype(str)
    return decisions


def build_exit_dataset(
    conn,
    decisions: pd.DataFrame,
    table_name: str,
    timeframe_minutes: int,
    args: argparse.Namespace,
    label: str,
) -> tuple[np.ndarray, pd.DataFrame]:
    images: list[np.ndarray] = []
    records: list[dict[str, Any]] = []
    if decisions.empty:
        return np.empty((0, 3 * args.height * args.width), dtype=np.float32), pd.DataFrame()
    work = decisions.copy()
    work["decision_date"] = pd.to_datetime(work["decision_date"], errors="coerce")
    work["decision_index"] = pd.to_numeric(work["decision_index"], errors="coerce")
    work["exit_edge_r"] = pd.to_numeric(work["exit_edge_r"], errors="coerce")
    work = work.dropna(subset=["decision_date", "decision_index", "exit_edge_r"]).copy()
    sampled: list[pd.DataFrame] = []
    for _, group in work.groupby("candidate_uid", sort=False):
        group = group.sort_values("decision_index")
        if int(args.max_decision_rows_per_trade) > 0 and len(group) > int(args.max_decision_rows_per_trade):
            take_idx = np.linspace(0, len(group) - 1, int(args.max_decision_rows_per_trade)).round().astype(int)
            group = group.iloc[sorted(set(int(i) for i in take_idx))]
        sampled.append(group)
    work = pd.concat(sampled, ignore_index=True) if sampled else pd.DataFrame()
    if int(args.max_rows_per_split) > 0 and len(work) > int(args.max_rows_per_split):
        work = work.sort_values(["decision_date", "candidate_uid"]).head(int(args.max_rows_per_split)).copy()
    total = len(work)
    for count, row_tuple in enumerate(work.itertuples(index=False), start=1):
        row = pd.Series(row_tuple._asdict())
        decision_date = pd.Timestamp(row["decision_date"])
        start_at = decision_date - pd.Timedelta(
            minutes=timeframe_minutes * int(args.lookback_bars) * max(1, int(args.history_multiplier))
        )
        candles = exit_model.load_candles(conn, table_name, str(row["symbol"]), start_at, decision_date)
        if candles.empty:
            continue
        context = candles[candles["ts_utc"] <= decision_date].tail(int(args.lookback_bars)).reset_index(drop=True)
        if len(context) < int(args.min_candles):
            continue
        target = 1 if float(row["exit_edge_r"]) <= float(args.exit_end_margin_r) else 0
        reason = "exit_now_good" if target else "hold_has_edge"
        image = visual.render_chart_image(context, str(row["direction"]), int(args.height), int(args.width))
        payload = row.to_dict()
        payload.update({"visual_target": int(target), "visual_target_reason": reason})
        images.append(image.reshape(-1))
        records.append(payload)
        if count % 500 == 0 or count == total:
            print(f"{label}: exit images {count:,}/{total:,}, kept={len(records):,}")
    if not images:
        return np.empty((0, 3 * args.height * args.width), dtype=np.float32), pd.DataFrame()
    return np.stack(images).astype(np.float32), pd.DataFrame(records)


def fit_model(train_x: np.ndarray, train_y: np.ndarray, args: argparse.Namespace) -> MLPClassifier:
    model = MLPClassifier(
        hidden_layer_sizes=hidden_layers(args.hidden_layers),
        activation="relu",
        solver="adam",
        alpha=float(args.alpha),
        learning_rate_init=float(args.learning_rate),
        max_iter=int(args.max_iter),
        early_stopping=True,
        validation_fraction=0.20,
        n_iter_no_change=12,
        random_state=int(args.random_seed),
        verbose=False,
    )
    model.fit(train_x, train_y)
    return model


def safe_auc(y_true: np.ndarray, y_score: np.ndarray) -> float | None:
    try:
        if len(set(int(v) for v in y_true)) < 2:
            return None
        return float(roc_auc_score(y_true, y_score))
    except ValueError:
        return None


def label_metrics(rows: pd.DataFrame, scores: np.ndarray) -> dict[str, Any]:
    y = pd.to_numeric(rows["visual_target"], errors="coerce").fillna(0).astype(int).to_numpy()
    pred = (scores >= 0.5).astype(int)
    result: dict[str, Any] = {
        "rows": int(len(rows)),
        "positive_rate": float(y.mean()) if len(y) else 0.0,
        "accuracy_at_0p5": float(accuracy_score(y, pred)) if len(y) else 0.0,
        "auc": safe_auc(y, scores),
    }
    try:
        result["log_loss"] = float(log_loss(y, np.clip(scores, 1e-6, 1 - 1e-6)))
    except ValueError:
        result["log_loss"] = None
    return result


def summarize_result(values: pd.Series) -> dict[str, Any]:
    result = pd.to_numeric(values, errors="coerce").fillna(0.0)
    equity = result.cumsum()
    dd = equity.cummax() - equity
    wins = int((result > 0).sum())
    return {
        "trades": int(len(result)),
        "win_rate": wins / len(result) if len(result) else 0.0,
        "avg_r": float(result.mean()) if len(result) else 0.0,
        "sum_r": float(result.sum()) if len(result) else 0.0,
        "max_drawdown_r": float(dd.max()) if len(dd) else 0.0,
    }


def choose_entry_threshold(scored: pd.DataFrame, args: argparse.Namespace) -> tuple[float | None, pd.DataFrame]:
    score = pd.to_numeric(scored["visual_score"], errors="coerce")
    candidates: list[float | None] = [None]
    candidates.extend(sorted(set(float(v) for v in score.quantile(np.linspace(0.05, 0.95, 37)).dropna())))
    min_keep = max(int(args.min_keep_rows), int(math.ceil(len(scored) * float(args.min_keep_share))))
    rows = []
    best_threshold: float | None = None
    best_key: tuple[float, float, float, int] | None = None
    for threshold in candidates:
        subset = scored if threshold is None else scored[score >= threshold]
        stats = summarize_result(subset["result_r"])
        allowed = stats["trades"] >= min_keep
        rows.append({"threshold": threshold, "allowed": allowed, **stats})
        if allowed:
            key = (stats["sum_r"], -stats["max_drawdown_r"], stats["avg_r"], stats["trades"])
            if best_key is None or key > best_key:
                best_key = key
                best_threshold = threshold
    return best_threshold, pd.DataFrame(rows)


def apply_entry_threshold(scored: pd.DataFrame, threshold: float | None) -> pd.DataFrame:
    if threshold is None:
        return scored.copy()
    return scored[pd.to_numeric(scored["visual_score"], errors="coerce") >= float(threshold)].copy()


def print_entry_summary(label: str, frame: pd.DataFrame) -> None:
    stats = summarize_result(frame["result_r"])
    print(
        f"{label}: trades={stats['trades']:,} win={stats['win_rate'] * 100:.2f}% "
        f"sum={stats['sum_r']:.2f}R avg={stats['avg_r']:.4f}R dd={stats['max_drawdown_r']:.2f}R"
    )


def add_scores(rows: pd.DataFrame, scores: np.ndarray) -> pd.DataFrame:
    scored = rows.copy()
    scored["visual_score"] = scores.astype(float)
    return scored


def main() -> int:
    args = parse_args()
    rid = run_id(args)
    model_dir = wave.ABCD_ROOT / "model_registry" / rid
    if model_dir.exists() and not args.replace_run:
        raise ValueError(f"Visual trend-point run already exists: {rid}. Use --replace-run to overwrite.")
    model_dir.mkdir(parents=True, exist_ok=True)

    policy_metadata = load_policy_metadata(args.policy_run_id)
    source_model_run_id = str(policy_metadata["source_model_run_id"])
    exclude_roots = parse_root_list(policy_metadata.get("policy_exclude_roots"))
    train_policy = load_policy_rows(args.policy_run_id, args.train_year)
    threshold_policy = load_policy_rows(args.policy_run_id, args.threshold_year)
    valid_policy = load_policy_rows(args.policy_run_id, args.valid_year)

    with wave.connect() as conn:
        train_details, entry_args = load_selected_details(conn, source_model_run_id, [args.train_year], exclude_roots)
        threshold_details, _ = load_selected_details(conn, source_model_run_id, [args.threshold_year], exclude_roots)
        valid_details, _ = load_selected_details(conn, source_model_run_id, [args.valid_year], exclude_roots)
        table_name, timeframe_minutes = scanner.table_for_timeframe(str(entry_args.timeframe))
        if args.task == "entry_start":
            train_rows = merge_policy_details(train_policy, train_details)
            threshold_rows = merge_policy_details(threshold_policy, threshold_details)
            valid_rows = merge_policy_details(valid_policy, valid_details)
            train_x, train_frame = build_entry_dataset(conn, train_rows, table_name, timeframe_minutes, args, "train")
            threshold_x, threshold_frame = build_entry_dataset(conn, threshold_rows, table_name, timeframe_minutes, args, "threshold")
            valid_x, valid_frame = build_entry_dataset(conn, valid_rows, table_name, timeframe_minutes, args, "valid")
        else:
            train_decisions = build_decision_base(conn, train_policy, train_details, policy_metadata, table_name, timeframe_minutes, "train")
            threshold_decisions = build_decision_base(conn, threshold_policy, threshold_details, policy_metadata, table_name, timeframe_minutes, "threshold")
            valid_decisions = build_decision_base(conn, valid_policy, valid_details, policy_metadata, table_name, timeframe_minutes, "valid")
            train_x, train_frame = build_exit_dataset(conn, train_decisions, table_name, timeframe_minutes, args, "train")
            threshold_x, threshold_frame = build_exit_dataset(conn, threshold_decisions, table_name, timeframe_minutes, args, "threshold")
            valid_x, valid_frame = build_exit_dataset(conn, valid_decisions, table_name, timeframe_minutes, args, "valid")

    if min(len(train_frame), len(threshold_frame), len(valid_frame)) < 100:
        raise ValueError(
            f"Not enough rows: train={len(train_frame)} threshold={len(threshold_frame)} valid={len(valid_frame)}"
        )

    train_y = pd.to_numeric(train_frame["visual_target"], errors="coerce").fillna(0).astype(int).to_numpy()
    print(
        f"Training visual {args.task} model run={rid} rows={len(train_frame):,} "
        f"features={train_x.shape[1]:,} positive_rate={train_y.mean():.3f}"
    )
    model = fit_model(train_x, train_y, args)
    train_scores = model.predict_proba(train_x)[:, 1]
    threshold_scores = model.predict_proba(threshold_x)[:, 1]
    valid_scores = model.predict_proba(valid_x)[:, 1]

    train_scored = add_scores(train_frame, train_scores)
    threshold_scored = add_scores(threshold_frame, threshold_scores)
    valid_scored = add_scores(valid_frame, valid_scores)

    print(f"train_label_metrics={label_metrics(train_scored, train_scores)}")
    print(f"threshold_label_metrics={label_metrics(threshold_scored, threshold_scores)}")
    print(f"valid_label_metrics={label_metrics(valid_scored, valid_scores)}")

    threshold_value = None
    threshold_sweep = pd.DataFrame()
    threshold_filtered = pd.DataFrame()
    valid_filtered = pd.DataFrame()
    if args.task == "entry_start":
        threshold_value, threshold_sweep = choose_entry_threshold(threshold_scored, args)
        threshold_filtered = apply_entry_threshold(threshold_scored, threshold_value)
        valid_filtered = apply_entry_threshold(valid_scored, threshold_value)
        print_entry_summary("threshold catboost_policy", threshold_scored)
        print_entry_summary("threshold catboost_visual_consensus", threshold_filtered)
        print_entry_summary("valid catboost_policy", valid_scored)
        print_entry_summary("valid catboost_visual_consensus", valid_filtered)
        print(f"selected_entry_consensus_threshold={'none' if threshold_value is None else f'{threshold_value:.6f}'}")

    joblib.dump(model, model_dir / f"{args.task}_mlp.joblib")
    train_scored.to_csv(model_dir / f"visual_{args.task}_scores_{args.train_year}.csv", index=False)
    threshold_scored.to_csv(model_dir / f"visual_{args.task}_scores_{args.threshold_year}.csv", index=False)
    valid_scored.to_csv(model_dir / f"visual_{args.task}_scores_{args.valid_year}.csv", index=False)
    if not threshold_sweep.empty:
        threshold_sweep.to_csv(model_dir / f"visual_{args.task}_threshold_sweep_{args.threshold_year}.csv", index=False)

    metadata = {
        "visual_model_run_id": rid,
        "task": args.task,
        "model_type": "sklearn_mlp_classifier_flat_chart_image",
        "consensus_rule": "Entry requires the existing CatBoost/policy candidate plus visual trend-start score >= threshold.",
        "policy_run_id": args.policy_run_id,
        "source_model_run_id": source_model_run_id,
        "exclude_roots": exclude_roots,
        "train_year": args.train_year,
        "threshold_year": args.threshold_year,
        "valid_year": args.valid_year,
        "lookback_bars": args.lookback_bars,
        "height": args.height,
        "width": args.width,
        "min_candles": args.min_candles,
        "entry_label_bars": args.entry_label_bars,
        "entry_start_target_r": args.entry_start_target_r,
        "entry_start_fail_r": args.entry_start_fail_r,
        "exit_end_margin_r": args.exit_end_margin_r,
        "hidden_layers": hidden_layers(args.hidden_layers),
        "leak_safety": "Entry images use candles with ts_utc < entry_date. Exit images use candles with ts_utc <= decision_date. Future candles are labels only.",
        "threshold_value": threshold_value,
        "train_label_metrics": label_metrics(train_scored, train_scores),
        "threshold_label_metrics": label_metrics(threshold_scored, threshold_scores),
        "valid_label_metrics": label_metrics(valid_scored, valid_scores),
    }
    if args.task == "entry_start":
        metadata["threshold_baseline_summary"] = summarize_result(threshold_scored["result_r"])
        metadata["threshold_consensus_summary"] = summarize_result(threshold_filtered["result_r"])
        metadata["valid_baseline_summary"] = summarize_result(valid_scored["result_r"])
        metadata["valid_consensus_summary"] = summarize_result(valid_filtered["result_r"])
    (model_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=str), encoding="utf-8")
    print(f"Saved visual trend-point model: {rid}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
