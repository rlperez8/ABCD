#!/usr/bin/env python3
"""
Train a small CNN visual agreement model for candle-wave trend points.

This is the proper image-model version of the visual trend-point experiment.
It reuses the leak-safe image builders from ai_candle_wave_visual_trend_points.py
and trains a compact PyTorch CNN instead of a flat MLP.
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
import torch
from sklearn.metrics import accuracy_score, log_loss, roc_auc_score
from torch import nn
from torch.utils.data import DataLoader, TensorDataset


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_scanner_model as scanner
import ai_candle_wave_visual_trend_points as trend
import ai_wave_rider_research as wave


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--task", choices=["entry_start", "exit_end"], default="entry_start")
    parser.add_argument("--policy-run-id", default=trend.DEFAULT_POLICY_RUN)
    parser.add_argument("--run-prefix", default="aicw-visual-trendpoint-cnn-v1")
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
    parser.add_argument("--epochs", type=int, default=45)
    parser.add_argument("--batch-size", type=int, default=64)
    parser.add_argument("--learning-rate", type=float, default=0.001)
    parser.add_argument("--weight-decay", type=float, default=0.01)
    parser.add_argument("--dropout", type=float, default=0.20)
    parser.add_argument("--patience", type=int, default=8)
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


class SmallTrendCnn(nn.Module):
    def __init__(self, dropout: float) -> None:
        super().__init__()
        self.net = nn.Sequential(
            nn.Conv2d(3, 12, kernel_size=5, padding=2),
            nn.BatchNorm2d(12),
            nn.ReLU(inplace=True),
            nn.MaxPool2d(2),
            nn.Conv2d(12, 24, kernel_size=3, padding=1),
            nn.BatchNorm2d(24),
            nn.ReLU(inplace=True),
            nn.MaxPool2d(2),
            nn.Conv2d(24, 48, kernel_size=3, padding=1),
            nn.BatchNorm2d(48),
            nn.ReLU(inplace=True),
            nn.AdaptiveAvgPool2d((4, 4)),
            nn.Flatten(),
            nn.Dropout(float(dropout)),
            nn.Linear(48 * 4 * 4, 64),
            nn.ReLU(inplace=True),
            nn.Dropout(float(dropout)),
            nn.Linear(64, 1),
        )

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        return self.net(x).squeeze(1)


def set_seed(seed: int) -> None:
    np.random.seed(seed)
    torch.manual_seed(seed)
    torch.set_num_threads(max(1, min(8, torch.get_num_threads())))


def to_image_tensor(x: np.ndarray, height: int, width: int) -> torch.Tensor:
    return torch.from_numpy(x.reshape(len(x), 3, height, width).astype(np.float32))


def make_loader(x: np.ndarray, y: np.ndarray, args: argparse.Namespace, shuffle: bool) -> DataLoader:
    ds = TensorDataset(to_image_tensor(x, args.height, args.width), torch.from_numpy(y.astype(np.float32)))
    return DataLoader(ds, batch_size=int(args.batch_size), shuffle=shuffle)


def safe_auc(y_true: np.ndarray, y_score: np.ndarray) -> float | None:
    try:
        if len(set(int(v) for v in y_true)) < 2:
            return None
        return float(roc_auc_score(y_true, y_score))
    except ValueError:
        return None


def binary_metrics(y_true: np.ndarray, scores: np.ndarray) -> dict[str, Any]:
    pred = (scores >= 0.5).astype(int)
    out: dict[str, Any] = {
        "rows": int(len(y_true)),
        "positive_rate": float(y_true.mean()) if len(y_true) else 0.0,
        "accuracy_at_0p5": float(accuracy_score(y_true, pred)) if len(y_true) else 0.0,
        "auc": safe_auc(y_true, scores),
    }
    try:
        out["log_loss"] = float(log_loss(y_true, np.clip(scores, 1e-6, 1 - 1e-6)))
    except ValueError:
        out["log_loss"] = None
    return out


def evaluate_scores(model: nn.Module, x: np.ndarray, args: argparse.Namespace) -> np.ndarray:
    model.eval()
    scores: list[np.ndarray] = []
    with torch.no_grad():
        tensor = to_image_tensor(x, args.height, args.width)
        for start in range(0, len(tensor), int(args.batch_size)):
            logits = model(tensor[start : start + int(args.batch_size)])
            scores.append(torch.sigmoid(logits).cpu().numpy())
    return np.concatenate(scores) if scores else np.array([], dtype=float)


def train_cnn(train_x: np.ndarray, train_y: np.ndarray, threshold_x: np.ndarray, threshold_y: np.ndarray, args: argparse.Namespace) -> tuple[nn.Module, list[dict[str, Any]]]:
    model = SmallTrendCnn(float(args.dropout))
    pos = max(1.0, float(train_y.sum()))
    neg = max(1.0, float(len(train_y) - train_y.sum()))
    pos_weight = torch.tensor([neg / pos], dtype=torch.float32)
    loss_fn = nn.BCEWithLogitsLoss(pos_weight=pos_weight)
    optimizer = torch.optim.AdamW(model.parameters(), lr=float(args.learning_rate), weight_decay=float(args.weight_decay))
    train_loader = make_loader(train_x, train_y, args, shuffle=True)

    best_state: dict[str, torch.Tensor] | None = None
    best_key: tuple[float, float] | None = None
    stale = 0
    history: list[dict[str, Any]] = []
    for epoch in range(1, int(args.epochs) + 1):
        model.train()
        total_loss = 0.0
        seen = 0
        for xb, yb in train_loader:
            optimizer.zero_grad()
            logits = model(xb)
            loss = loss_fn(logits, yb)
            loss.backward()
            optimizer.step()
            total_loss += float(loss.item()) * len(yb)
            seen += len(yb)
        threshold_scores = evaluate_scores(model, threshold_x, args)
        metrics = binary_metrics(threshold_y, threshold_scores)
        auc = metrics["auc"] if metrics["auc"] is not None else 0.0
        ll = metrics["log_loss"] if metrics["log_loss"] is not None else 99.0
        key = (float(auc), -float(ll))
        history.append({"epoch": epoch, "train_loss": total_loss / max(1, seen), **metrics})
        print(
            f"epoch={epoch:03d} train_loss={total_loss / max(1, seen):.4f} "
            f"threshold_auc={auc:.4f} threshold_logloss={ll:.4f}"
        )
        if best_key is None or key > best_key:
            best_key = key
            best_state = {name: value.detach().clone() for name, value in model.state_dict().items()}
            stale = 0
        else:
            stale += 1
            if stale >= int(args.patience):
                print(f"early_stop epoch={epoch}")
                break
    if best_state is not None:
        model.load_state_dict(best_state)
    return model, history


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


def add_scores(rows: pd.DataFrame, scores: np.ndarray) -> pd.DataFrame:
    out = rows.copy()
    out["visual_score"] = scores.astype(float)
    return out


def choose_entry_threshold(scored: pd.DataFrame, args: argparse.Namespace) -> tuple[float | None, pd.DataFrame]:
    score = pd.to_numeric(scored["visual_score"], errors="coerce")
    candidates: list[float | None] = [None]
    candidates.extend(sorted(set(float(v) for v in score.quantile(np.linspace(0.05, 0.95, 37)).dropna())))
    min_keep = max(int(args.min_keep_rows), int(math.ceil(len(scored) * float(args.min_keep_share))))
    rows: list[dict[str, Any]] = []
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


def filter_by_threshold(scored: pd.DataFrame, threshold: float | None) -> pd.DataFrame:
    if threshold is None:
        return scored.copy()
    return scored[pd.to_numeric(scored["visual_score"], errors="coerce") >= float(threshold)].copy()


def print_result(label: str, rows: pd.DataFrame) -> None:
    stats = summarize_result(rows["result_r"])
    print(
        f"{label}: trades={stats['trades']:,} win={stats['win_rate'] * 100:.2f}% "
        f"sum={stats['sum_r']:.2f}R avg={stats['avg_r']:.4f}R dd={stats['max_drawdown_r']:.2f}R"
    )


def build_datasets(args: argparse.Namespace):
    policy_metadata = trend.load_policy_metadata(args.policy_run_id)
    source_model_run_id = str(policy_metadata["source_model_run_id"])
    exclude_roots = trend.parse_root_list(policy_metadata.get("policy_exclude_roots"))
    train_policy = trend.load_policy_rows(args.policy_run_id, args.train_year)
    threshold_policy = trend.load_policy_rows(args.policy_run_id, args.threshold_year)
    valid_policy = trend.load_policy_rows(args.policy_run_id, args.valid_year)

    with wave.connect() as conn:
        train_details, entry_args = trend.load_selected_details(conn, source_model_run_id, [args.train_year], exclude_roots)
        threshold_details, _ = trend.load_selected_details(conn, source_model_run_id, [args.threshold_year], exclude_roots)
        valid_details, _ = trend.load_selected_details(conn, source_model_run_id, [args.valid_year], exclude_roots)
        table_name, timeframe_minutes = scanner.table_for_timeframe(str(entry_args.timeframe))
        if args.task == "entry_start":
            train_rows = trend.merge_policy_details(train_policy, train_details)
            threshold_rows = trend.merge_policy_details(threshold_policy, threshold_details)
            valid_rows = trend.merge_policy_details(valid_policy, valid_details)
            train_x, train_frame = trend.build_entry_dataset(conn, train_rows, table_name, timeframe_minutes, args, "train")
            threshold_x, threshold_frame = trend.build_entry_dataset(conn, threshold_rows, table_name, timeframe_minutes, args, "threshold")
            valid_x, valid_frame = trend.build_entry_dataset(conn, valid_rows, table_name, timeframe_minutes, args, "valid")
        else:
            train_decisions = trend.build_decision_base(conn, train_policy, train_details, policy_metadata, table_name, timeframe_minutes, "train")
            threshold_decisions = trend.build_decision_base(conn, threshold_policy, threshold_details, policy_metadata, table_name, timeframe_minutes, "threshold")
            valid_decisions = trend.build_decision_base(conn, valid_policy, valid_details, policy_metadata, table_name, timeframe_minutes, "valid")
            train_x, train_frame = trend.build_exit_dataset(conn, train_decisions, table_name, timeframe_minutes, args, "train")
            threshold_x, threshold_frame = trend.build_exit_dataset(conn, threshold_decisions, table_name, timeframe_minutes, args, "threshold")
            valid_x, valid_frame = trend.build_exit_dataset(conn, valid_decisions, table_name, timeframe_minutes, args, "valid")
    return policy_metadata, exclude_roots, train_x, train_frame, threshold_x, threshold_frame, valid_x, valid_frame


def main() -> int:
    args = parse_args()
    set_seed(int(args.random_seed))
    rid = run_id(args)
    model_dir = wave.ABCD_ROOT / "model_registry" / rid
    if model_dir.exists() and not args.replace_run:
        raise ValueError(f"CNN visual run already exists: {rid}. Use --replace-run to overwrite.")
    model_dir.mkdir(parents=True, exist_ok=True)

    policy_metadata, exclude_roots, train_x, train_frame, threshold_x, threshold_frame, valid_x, valid_frame = build_datasets(args)
    if min(len(train_frame), len(threshold_frame), len(valid_frame)) < 100:
        raise ValueError(
            f"Not enough rows: train={len(train_frame)} threshold={len(threshold_frame)} valid={len(valid_frame)}"
        )

    train_y = pd.to_numeric(train_frame["visual_target"], errors="coerce").fillna(0).astype(int).to_numpy()
    threshold_y = pd.to_numeric(threshold_frame["visual_target"], errors="coerce").fillna(0).astype(int).to_numpy()
    valid_y = pd.to_numeric(valid_frame["visual_target"], errors="coerce").fillna(0).astype(int).to_numpy()
    print(
        f"Training CNN visual {args.task} run={rid} train_rows={len(train_frame):,} "
        f"threshold_rows={len(threshold_frame):,} valid_rows={len(valid_frame):,} "
        f"train_positive={train_y.mean():.3f}"
    )
    model, history = train_cnn(train_x, train_y, threshold_x, threshold_y, args)
    train_scores = evaluate_scores(model, train_x, args)
    threshold_scores = evaluate_scores(model, threshold_x, args)
    valid_scores = evaluate_scores(model, valid_x, args)

    train_scored = add_scores(train_frame, train_scores)
    threshold_scored = add_scores(threshold_frame, threshold_scores)
    valid_scored = add_scores(valid_frame, valid_scores)
    train_metrics = binary_metrics(train_y, train_scores)
    threshold_metrics = binary_metrics(threshold_y, threshold_scores)
    valid_metrics = binary_metrics(valid_y, valid_scores)
    print(f"train_label_metrics={train_metrics}")
    print(f"threshold_label_metrics={threshold_metrics}")
    print(f"valid_label_metrics={valid_metrics}")

    selected_threshold = None
    threshold_sweep = pd.DataFrame()
    threshold_consensus = pd.DataFrame()
    valid_consensus = pd.DataFrame()
    if args.task == "entry_start":
        selected_threshold, threshold_sweep = choose_entry_threshold(threshold_scored, args)
        threshold_consensus = filter_by_threshold(threshold_scored, selected_threshold)
        valid_consensus = filter_by_threshold(valid_scored, selected_threshold)
        print_result("threshold catboost_policy", threshold_scored)
        print_result("threshold catboost_cnn_consensus", threshold_consensus)
        print_result("valid catboost_policy", valid_scored)
        print_result("valid catboost_cnn_consensus", valid_consensus)
        print(f"selected_entry_consensus_threshold={'none' if selected_threshold is None else f'{selected_threshold:.6f}'}")

    torch.save(model.state_dict(), model_dir / f"{args.task}_cnn.pt")
    train_scored.to_csv(model_dir / f"visual_{args.task}_scores_{args.train_year}.csv", index=False)
    threshold_scored.to_csv(model_dir / f"visual_{args.task}_scores_{args.threshold_year}.csv", index=False)
    valid_scored.to_csv(model_dir / f"visual_{args.task}_scores_{args.valid_year}.csv", index=False)
    if not threshold_sweep.empty:
        threshold_sweep.to_csv(model_dir / f"visual_{args.task}_threshold_sweep_{args.threshold_year}.csv", index=False)

    metadata = {
        "visual_model_run_id": rid,
        "task": args.task,
        "model_type": "pytorch_small_cnn_flat_chart_image",
        "policy_run_id": args.policy_run_id,
        "source_model_run_id": str(policy_metadata["source_model_run_id"]),
        "exclude_roots": exclude_roots,
        "consensus_rule": "Entry requires the existing CatBoost/policy candidate plus CNN visual trend-start score >= threshold.",
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
        "epochs_requested": args.epochs,
        "training_history": history,
        "selected_threshold": selected_threshold,
        "train_label_metrics": train_metrics,
        "threshold_label_metrics": threshold_metrics,
        "valid_label_metrics": valid_metrics,
    }
    if args.task == "entry_start":
        metadata["threshold_baseline_summary"] = summarize_result(threshold_scored["result_r"])
        metadata["threshold_consensus_summary"] = summarize_result(threshold_consensus["result_r"])
        metadata["valid_baseline_summary"] = summarize_result(valid_scored["result_r"])
        metadata["valid_consensus_summary"] = summarize_result(valid_consensus["result_r"])
    (model_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=str), encoding="utf-8")
    print(f"Saved CNN visual trend-point model: {rid}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
