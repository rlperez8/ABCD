#!/usr/bin/env python3
"""
Train an image CNN to recognize oracle trend-start candles.

This is the visual companion to ai_oracle_trend_start_model.py. It does not
create trades or exits. It teaches an image model to answer:

    "Does this chart image, ending at the current closed candle, look like the
     start of a hindsight-perfect oracle trend?"

The script also scores the numeric CatBoost oracle-start model on the same
rows so we can measure how often the two independent views agree.
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
from sklearn.metrics import average_precision_score, precision_recall_fscore_support, roc_auc_score
from torch import nn
from torch.utils.data import DataLoader, TensorDataset


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_scanner_model as scanner
import ai_candle_wave_visual_entry_model as visual
import ai_oracle_trend_start_model as start_model
import ai_wave_rider_research as wave


DEFAULT_CATBOOST_RUN = "aicw-oracle-start-v1-2m-tr2024-v2026"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle-run-id", default=start_model.DEFAULT_ORACLE_RUN)
    parser.add_argument("--catboost-run-id", default=DEFAULT_CATBOOST_RUN)
    parser.add_argument("--run-prefix", default="aicw-oracle-start-visual-cnn-v1")
    parser.add_argument("--timeframe", default="2m", choices=sorted(scanner.TIMEFRAME_TABLES))
    parser.add_argument("--train-years", default="2024")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--roots", default="")
    parser.add_argument("--lookback-bars", type=int, default=64)
    parser.add_argument("--height", type=int, default=48)
    parser.add_argument("--width", type=int, default=48)
    parser.add_argument("--min-candles", type=int, default=40)
    parser.add_argument("--positive-lag-bars", type=int, default=2)
    parser.add_argument("--negative-exclusion-bars", type=int, default=8)
    parser.add_argument("--negative-ratio", type=float, default=2.0)
    parser.add_argument("--base-negatives-per-symbol", type=int, default=12)
    parser.add_argument("--match-window-bars", type=int, default=4)
    parser.add_argument("--max-train-rows", type=int, default=40000)
    parser.add_argument("--max-eval-rows", type=int, default=40000)
    parser.add_argument("--epochs", type=int, default=24)
    parser.add_argument("--batch-size", type=int, default=128)
    parser.add_argument("--learning-rate", type=float, default=0.001)
    parser.add_argument("--weight-decay", type=float, default=0.01)
    parser.add_argument("--dropout", type=float, default=0.20)
    parser.add_argument("--patience", type=int, default=6)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--catboost-threshold", type=float, default=-1.0)
    parser.add_argument("--min-threshold-precision", type=float, default=0.45)
    parser.add_argument("--min-threshold-picks", type=int, default=300)
    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=240.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    parser.add_argument("--replace-run", action="store_true")
    return parser.parse_args()


def run_id(args: argparse.Namespace) -> str:
    train = "_".join(str(year) for year in start_model.parse_years(args.train_years))
    rid = f"{args.run_prefix}-{args.timeframe}-tr{train}-v{args.valid_year}"
    if len(rid) > 64:
        raise ValueError(f"Run id too long: {rid}")
    return rid


class OracleStartCnn(nn.Module):
    def __init__(self, dropout: float) -> None:
        super().__init__()
        self.net = nn.Sequential(
            nn.Conv2d(3, 16, kernel_size=5, padding=2),
            nn.BatchNorm2d(16),
            nn.ReLU(inplace=True),
            nn.MaxPool2d(2),
            nn.Conv2d(16, 32, kernel_size=3, padding=1),
            nn.BatchNorm2d(32),
            nn.ReLU(inplace=True),
            nn.MaxPool2d(2),
            nn.Conv2d(32, 64, kernel_size=3, padding=1),
            nn.BatchNorm2d(64),
            nn.ReLU(inplace=True),
            nn.AdaptiveAvgPool2d((4, 4)),
            nn.Flatten(),
            nn.Dropout(float(dropout)),
            nn.Linear(64 * 4 * 4, 96),
            nn.ReLU(inplace=True),
            nn.Dropout(float(dropout)),
            nn.Linear(96, 1),
        )

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        return self.net(x).squeeze(1)


def set_seed(seed: int) -> None:
    np.random.seed(seed)
    torch.manual_seed(seed)
    torch.set_num_threads(max(1, min(8, torch.get_num_threads())))


def to_tensor(images: np.ndarray, args: argparse.Namespace) -> torch.Tensor:
    return torch.from_numpy(images.reshape(len(images), 3, int(args.height), int(args.width)).astype(np.float32))


def make_loader(images: np.ndarray, labels: np.ndarray, args: argparse.Namespace, shuffle: bool) -> DataLoader:
    ds = TensorDataset(to_tensor(images, args), torch.from_numpy(labels.astype(np.float32)))
    return DataLoader(ds, batch_size=int(args.batch_size), shuffle=shuffle)


def render_images_for_rows(conn, rows: pd.DataFrame, year: int, args: argparse.Namespace, label: str) -> tuple[np.ndarray, pd.DataFrame]:
    roots = start_model.parse_list(args.roots)
    candles = start_model.fetch_candles(conn, args.timeframe, int(year), roots)
    if candles.empty:
        raise ValueError(f"No candles available for {year}")
    rows = rows.copy()
    rows["signal_date"] = pd.to_datetime(rows["signal_date"], errors="coerce")
    rows = rows.dropna(subset=["symbol", "signal_date", "direction"]).reset_index(drop=True)
    by_symbol_rows = {
        str(symbol): group.sort_values("signal_date").reset_index(drop=True)
        for symbol, group in rows.groupby("symbol", sort=False)
    }
    images: list[np.ndarray] = []
    kept: list[dict[str, Any]] = []
    total_targets = len(rows)
    seen = 0
    for symbol_index, (symbol, raw_group) in enumerate(candles.groupby("symbol", sort=True), start=1):
        symbol_rows = by_symbol_rows.get(str(symbol))
        if symbol_rows is None or symbol_rows.empty:
            continue
        group = scanner.enrich_candles(raw_group[["ts_utc", "open", "high", "low", "close", "volume"]].copy(), args)
        times = pd.to_datetime(group["ts_utc"], errors="coerce").to_numpy(dtype="datetime64[ns]")
        for item in symbol_rows.itertuples(index=False):
            seen += 1
            signal_ts = pd.Timestamp(item.signal_date)
            idx = int(np.searchsorted(times, np.datetime64(signal_ts), side="left"))
            if idx >= len(group):
                continue
            start_idx = max(0, idx - int(args.lookback_bars) + 1)
            context = group.iloc[start_idx : idx + 1].reset_index(drop=True)
            if len(context) < int(args.min_candles):
                continue
            image = visual.render_chart_image(context, str(item.direction), int(args.height), int(args.width))
            images.append(image.reshape(-1))
            kept.append(item._asdict())
        if symbol_index % 50 == 0:
            print(f"{label}: rendered symbols={symbol_index:,} seen_rows={seen:,}/{total_targets:,} kept={len(kept):,}", flush=True)
    print(f"{label}: rendered seen_rows={seen:,}/{total_targets:,} kept={len(kept):,}", flush=True)
    if not images:
        return np.empty((0, 3 * int(args.height) * int(args.width)), dtype=np.float32), pd.DataFrame()
    return np.stack(images).astype(np.float32), pd.DataFrame(kept)


def build_split(conn, year: int, args: argparse.Namespace, rng: np.random.Generator, label: str) -> tuple[np.ndarray, pd.DataFrame]:
    candidate_rows = start_model.build_year_dataset(conn, int(year), args, rng)
    images, kept = render_images_for_rows(conn, candidate_rows, int(year), args, label)
    return images, kept


def safe_auc(y_true: np.ndarray, scores: np.ndarray) -> float | None:
    try:
        if len(set(int(v) for v in y_true)) < 2:
            return None
        return float(roc_auc_score(y_true, scores))
    except ValueError:
        return None


def safe_ap(y_true: np.ndarray, scores: np.ndarray) -> float | None:
    try:
        if len(set(int(v) for v in y_true)) < 2:
            return None
        return float(average_precision_score(y_true, scores))
    except ValueError:
        return None


def binary_metrics(y_true: np.ndarray, scores: np.ndarray, threshold: float) -> dict[str, Any]:
    pred = (scores >= float(threshold)).astype(int)
    precision, recall, f1, _ = precision_recall_fscore_support(y_true, pred, average="binary", zero_division=0)
    return {
        "rows": int(len(y_true)),
        "positives": int(y_true.sum()),
        "picks": int(pred.sum()),
        "precision": float(precision),
        "recall": float(recall),
        "f1": float(f1),
        "auc": safe_auc(y_true, scores),
        "average_precision": safe_ap(y_true, scores),
        "threshold": float(threshold),
    }


def evaluate_scores(model: nn.Module, images: np.ndarray, args: argparse.Namespace) -> np.ndarray:
    model.eval()
    scores: list[np.ndarray] = []
    with torch.no_grad():
        tensor = to_tensor(images, args)
        for start in range(0, len(tensor), int(args.batch_size)):
            logits = model(tensor[start : start + int(args.batch_size)])
            scores.append(torch.sigmoid(logits).cpu().numpy())
    return np.concatenate(scores) if scores else np.array([], dtype=float)


def train_cnn(
    train_x: np.ndarray,
    train_y: np.ndarray,
    threshold_x: np.ndarray,
    threshold_y: np.ndarray,
    args: argparse.Namespace,
) -> tuple[nn.Module, list[dict[str, Any]]]:
    model = OracleStartCnn(float(args.dropout))
    pos = max(1.0, float(train_y.sum()))
    neg = max(1.0, float(len(train_y) - train_y.sum()))
    loss_fn = nn.BCEWithLogitsLoss(pos_weight=torch.tensor([neg / pos], dtype=torch.float32))
    optimizer = torch.optim.AdamW(model.parameters(), lr=float(args.learning_rate), weight_decay=float(args.weight_decay))
    loader = make_loader(train_x, train_y, args, shuffle=True)
    best_state: dict[str, torch.Tensor] | None = None
    best_key: tuple[float, float] | None = None
    stale = 0
    history: list[dict[str, Any]] = []
    for epoch in range(1, int(args.epochs) + 1):
        model.train()
        total_loss = 0.0
        seen = 0
        for xb, yb in loader:
            optimizer.zero_grad()
            logits = model(xb)
            loss = loss_fn(logits, yb)
            loss.backward()
            optimizer.step()
            total_loss += float(loss.item()) * len(yb)
            seen += len(yb)
        threshold_scores = evaluate_scores(model, threshold_x, args)
        metrics = binary_metrics(threshold_y, threshold_scores, 0.5)
        auc = metrics["auc"] if metrics["auc"] is not None else 0.0
        ap = metrics["average_precision"] if metrics["average_precision"] is not None else 0.0
        key = (float(auc), float(ap))
        item = {"epoch": epoch, "train_loss": total_loss / max(1, seen), **metrics}
        history.append(item)
        print(
            f"epoch={epoch:03d} train_loss={item['train_loss']:.4f} "
            f"threshold_auc={auc:.4f} threshold_ap={ap:.4f}",
            flush=True,
        )
        if best_key is None or key > best_key:
            best_key = key
            best_state = {name: value.detach().clone() for name, value in model.state_dict().items()}
            stale = 0
        else:
            stale += 1
            if stale >= int(args.patience):
                print(f"early_stop epoch={epoch}", flush=True)
                break
    if best_state is not None:
        model.load_state_dict(best_state)
    return model, history


def add_catboost_scores(frame: pd.DataFrame, catboost_model: Any | None) -> pd.DataFrame:
    out = frame.copy()
    if catboost_model is None:
        out["catboost_score"] = np.nan
        return out
    out["catboost_score"] = catboost_model.predict_proba(start_model.prepare_pool(out, include_target=False))[:, 1]
    return out


def choose_threshold(scored: pd.DataFrame, score_col: str, args: argparse.Namespace) -> tuple[float, pd.DataFrame]:
    y = scored["is_oracle_start"].astype(int).to_numpy()
    score = pd.to_numeric(scored[score_col], errors="coerce").fillna(0.0).to_numpy()
    candidates = sorted(set(float(v) for v in np.quantile(score, np.linspace(0.05, 0.99, 60))))
    best_threshold = candidates[0]
    best_key: tuple[float, float, int] | None = None
    rows: list[dict[str, Any]] = []
    for threshold in candidates:
        metrics = binary_metrics(y, score, threshold)
        allowed = metrics["precision"] >= float(args.min_threshold_precision) and metrics["picks"] >= int(args.min_threshold_picks)
        rows.append({**metrics, "allowed": allowed})
        if allowed:
            key = (float(metrics["f1"]), float(metrics["precision"]), int(metrics["picks"]))
            if best_key is None or key > best_key:
                best_key = key
                best_threshold = threshold
    if best_key is None:
        best_threshold = max(candidates, key=lambda threshold: binary_metrics(y, score, threshold)["f1"])
    return best_threshold, pd.DataFrame(rows)


def combined_metrics(scored: pd.DataFrame, visual_threshold: float, catboost_threshold: float) -> dict[str, Any]:
    y = scored["is_oracle_start"].astype(int).to_numpy()
    visual_score = pd.to_numeric(scored["visual_score"], errors="coerce").fillna(0.0).to_numpy()
    cat_score = pd.to_numeric(scored["catboost_score"], errors="coerce").fillna(0.0).to_numpy()
    visual_pick = visual_score >= float(visual_threshold)
    cat_pick = cat_score >= float(catboost_threshold)
    both_pick = visual_pick & cat_pick
    union_pick = visual_pick | cat_pick

    def pick_stats(mask: np.ndarray) -> dict[str, Any]:
        picks = int(mask.sum())
        true = int((mask & (y == 1)).sum())
        return {
            "picks": picks,
            "true_picks": true,
            "precision": true / picks if picks else 0.0,
            "recall": true / int(y.sum()) if int(y.sum()) else 0.0,
        }

    both = pick_stats(both_pick)
    visual_only = pick_stats(visual_pick & ~cat_pick)
    cat_only = pick_stats(cat_pick & ~visual_pick)
    return {
        "rows": int(len(scored)),
        "positives": int(y.sum()),
        "visual_threshold": float(visual_threshold),
        "catboost_threshold": float(catboost_threshold),
        "visual": pick_stats(visual_pick),
        "catboost": pick_stats(cat_pick),
        "both_agree": both,
        "either": pick_stats(union_pick),
        "visual_only": visual_only,
        "catboost_only": cat_only,
        "agreement_share_of_visual": both["picks"] / int(visual_pick.sum()) if int(visual_pick.sum()) else 0.0,
        "agreement_share_of_catboost": both["picks"] / int(cat_pick.sum()) if int(cat_pick.sum()) else 0.0,
        "pick_jaccard": both["picks"] / int(union_pick.sum()) if int(union_pick.sum()) else 0.0,
    }


def score_frame(frame: pd.DataFrame, visual_scores: np.ndarray, catboost_model: Any | None) -> pd.DataFrame:
    scored = frame.copy()
    scored["visual_score"] = visual_scores.astype(float)
    scored = add_catboost_scores(scored, catboost_model)
    return scored


def load_catboost_model(run_id: str) -> tuple[Any | None, float | None]:
    if not run_id:
        return None, None
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id
    metadata_path = model_dir / "metadata.json"
    model_path = model_dir / "catboost_oracle_start_model.cbm"
    if not metadata_path.exists() or not model_path.exists():
        raise FileNotFoundError(f"Missing CatBoost oracle-start run: {run_id}")
    from catboost import CatBoostClassifier

    model = CatBoostClassifier()
    model.load_model(str(model_path))
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    return model, float(metadata.get("selected_threshold", 0.5))


def to_jsonable(value: Any) -> Any:
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, pd.Timestamp):
        return value.isoformat()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def main() -> int:
    args = parse_args()
    set_seed(int(args.random_seed))
    rid = run_id(args)
    model_dir = wave.ABCD_ROOT / "model_registry" / rid
    if model_dir.exists() and not args.replace_run:
        raise ValueError(f"Visual oracle-start run already exists: {rid}. Use --replace-run.")
    model_dir.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(int(args.random_seed))
    train_years = start_model.parse_years(args.train_years)
    catboost_model, catboost_default_threshold = load_catboost_model(str(args.catboost_run_id)) if args.catboost_run_id else (None, None)
    catboost_threshold = float(args.catboost_threshold) if float(args.catboost_threshold) >= 0 else float(catboost_default_threshold or 0.5)

    conn = wave.connect()
    try:
        train_parts: list[np.ndarray] = []
        train_frames: list[pd.DataFrame] = []
        for year in train_years:
            x, frame = build_split(conn, int(year), args, rng, f"train-{year}")
            train_parts.append(x)
            train_frames.append(frame)
        train_x = np.concatenate(train_parts) if len(train_parts) > 1 else train_parts[0]
        train_frame = pd.concat(train_frames, ignore_index=True) if len(train_frames) > 1 else train_frames[0]
        threshold_x, threshold_frame = build_split(conn, int(args.threshold_year), args, rng, "threshold")
        valid_x, valid_frame = build_split(conn, int(args.valid_year), args, rng, "valid")
    finally:
        conn.close()

    if min(len(train_frame), len(threshold_frame), len(valid_frame)) < 100:
        raise ValueError(
            f"Not enough image rows: train={len(train_frame)} threshold={len(threshold_frame)} valid={len(valid_frame)}"
        )
    train_y = train_frame["is_oracle_start"].astype(int).to_numpy()
    threshold_y = threshold_frame["is_oracle_start"].astype(int).to_numpy()
    valid_y = valid_frame["is_oracle_start"].astype(int).to_numpy()
    print(
        f"Training visual oracle-start CNN run={rid} train={len(train_frame):,} "
        f"threshold={len(threshold_frame):,} valid={len(valid_frame):,} "
        f"train_positive={train_y.mean():.3f}",
        flush=True,
    )
    model, history = train_cnn(train_x, train_y, threshold_x, threshold_y, args)
    train_scores = evaluate_scores(model, train_x, args)
    threshold_scores = evaluate_scores(model, threshold_x, args)
    valid_scores = evaluate_scores(model, valid_x, args)

    train_scored = score_frame(train_frame, train_scores, catboost_model)
    threshold_scored = score_frame(threshold_frame, threshold_scores, catboost_model)
    valid_scored = score_frame(valid_frame, valid_scores, catboost_model)
    visual_threshold, visual_sweep = choose_threshold(threshold_scored, "visual_score", args)
    print(f"selected_visual_threshold={visual_threshold:.6f}", flush=True)

    metrics = {
        "train_visual": binary_metrics(train_y, train_scores, visual_threshold),
        "threshold_visual": binary_metrics(threshold_y, threshold_scores, visual_threshold),
        "valid_visual": binary_metrics(valid_y, valid_scores, visual_threshold),
        "train_catboost": binary_metrics(train_y, pd.to_numeric(train_scored["catboost_score"], errors="coerce").fillna(0.0).to_numpy(), catboost_threshold),
        "threshold_catboost": binary_metrics(threshold_y, pd.to_numeric(threshold_scored["catboost_score"], errors="coerce").fillna(0.0).to_numpy(), catboost_threshold),
        "valid_catboost": binary_metrics(valid_y, pd.to_numeric(valid_scored["catboost_score"], errors="coerce").fillna(0.0).to_numpy(), catboost_threshold),
        "train_consensus": combined_metrics(train_scored, visual_threshold, catboost_threshold),
        "threshold_consensus": combined_metrics(threshold_scored, visual_threshold, catboost_threshold),
        "valid_consensus": combined_metrics(valid_scored, visual_threshold, catboost_threshold),
    }
    for key, value in metrics.items():
        print(f"{key}={json.dumps(value, default=to_jsonable)}", flush=True)

    torch.save(model.state_dict(), model_dir / "visual_oracle_start_cnn.pt")
    train_scored.to_csv(model_dir / "visual_oracle_start_scores_train.csv", index=False)
    threshold_scored.to_csv(model_dir / f"visual_oracle_start_scores_{args.threshold_year}.csv", index=False)
    valid_scored.to_csv(model_dir / f"visual_oracle_start_scores_{args.valid_year}.csv", index=False)
    visual_sweep.to_csv(model_dir / f"visual_oracle_start_threshold_sweep_{args.threshold_year}.csv", index=False)
    metadata = {
        "visual_model_run_id": rid,
        "model_type": "pytorch_cnn_oracle_trend_start_image",
        "oracle_run_id": args.oracle_run_id,
        "catboost_run_id": args.catboost_run_id,
        "timeframe": args.timeframe,
        "train_years": train_years,
        "threshold_year": int(args.threshold_year),
        "valid_year": int(args.valid_year),
        "roots": start_model.parse_list(args.roots),
        "lookback_bars": int(args.lookback_bars),
        "height": int(args.height),
        "width": int(args.width),
        "positive_lag_bars": int(args.positive_lag_bars),
        "negative_exclusion_bars": int(args.negative_exclusion_bars),
        "selected_visual_threshold": float(visual_threshold),
        "catboost_threshold": float(catboost_threshold),
        "training_history": history,
        "metrics": metrics,
        "leak_safety": "Images end at the current signal candle. Oracle future path is used only for offline labels.",
    }
    (model_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")
    print(f"Saved visual oracle-start CNN: {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
