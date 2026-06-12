#!/usr/bin/env python3
"""
Train a Stage 2B rejector over adaptive Stage 2 confirmations.

Adaptive Stage 2 says "confirm." This model learns from those confirmed rows
and tries to reject false confirmations without changing Stage 1 or Stage 2.
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
    from catboost import CatBoostClassifier, Pool
except ImportError as exc:  # pragma: no cover
    raise SystemExit("CatBoost is required. Run with .venv_ai\\Scripts\\python.exe") from exc

from sklearn.metrics import average_precision_score, roc_auc_score


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_oracle_start_manager_model as manager_model
import ai_oracle_start_stage2_adaptive_confirmation as adaptive_stage2
import ai_oracle_start_stage2_confirmation as fixed_stage2
import ai_oracle_trend_start_model as start_model
import ai_wave_rider_research as wave


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage2-run-id", required=True)
    parser.add_argument("--run-prefix", default="aicw-stage2-confirm-rejector-v1")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--base-stage2-threshold", type=float, default=-1.0)
    parser.add_argument("--min-threshold-precision", type=float, default=1.0)
    parser.add_argument("--min-threshold-picks", type=int, default=25)
    parser.add_argument("--iterations", type=int, default=450)
    parser.add_argument("--depth", type=int, default=5)
    parser.add_argument("--learning-rate", type=float, default=0.04)
    parser.add_argument("--l2-leaf-reg", type=float, default=10.0)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--replace-run", action="store_true")
    return parser.parse_args()


def to_jsonable(value: Any) -> Any:
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def source_dir(run_id: str) -> tuple[Path, dict[str, Any]]:
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id
    metadata_path = model_dir / "metadata.json"
    if not metadata_path.exists():
        raise FileNotFoundError(f"Missing Stage 2 metadata: {metadata_path}")
    return model_dir, json.loads(metadata_path.read_text(encoding="utf-8"))


def load_rows(model_dir: Path, split: str | int) -> pd.DataFrame:
    path = model_dir / ("stage2_adaptive_rows_train.csv" if str(split) == "train" else f"stage2_adaptive_rows_{split}.csv")
    if not path.exists():
        raise FileNotFoundError(f"Missing expanded Stage 2 rows: {path}")
    frame = pd.read_csv(path)
    frame["candidate_uid"] = frame["candidate_uid"].astype(str)
    for col in ["stage2_score", "stage2_target", "is_oracle_start", "confirm_offset_bars"]:
        frame[col] = pd.to_numeric(frame.get(col), errors="coerce").fillna(0.0)
    return frame.sort_values(["candidate_uid", "confirm_offset_bars"]).reset_index(drop=True)


def first_confirmations(rows: pd.DataFrame, base_threshold: float) -> pd.DataFrame:
    hits = rows[pd.to_numeric(rows["stage2_score"], errors="coerce").fillna(0.0) >= float(base_threshold)].copy()
    if hits.empty:
        return hits
    hits = hits.sort_values(["candidate_uid", "confirm_offset_bars"]).drop_duplicates("candidate_uid").reset_index(drop=True)
    hits["rejector_target"] = pd.to_numeric(hits["stage2_target"], errors="coerce").fillna(0).astype(int)
    return hits


def event_counts(rows: pd.DataFrame) -> dict[str, int]:
    events = rows.drop_duplicates("candidate_uid").copy()
    target_events = rows.groupby("candidate_uid")["stage2_target"].max()
    return {
        "events": int(events["candidate_uid"].nunique()),
        "oracle_events": int(events["is_oracle_start"].sum()),
        "target_events": int(target_events.sum()),
    }


def feature_columns() -> tuple[list[str], list[str]]:
    cat_features = list(start_model.CAT_FEATURES)
    num_features = list(start_model.NUM_FEATURES) + manager_model.DERIVED_NUM_FEATURES
    num_features += fixed_stage2.POST_FEATURES + adaptive_stage2.ADAPTIVE_NUM_FEATURES
    num_features += ["stage2_score"]
    return cat_features, sorted(set(num_features))


def prepare_pool(frame: pd.DataFrame, include_target: bool) -> Pool:
    cat_features, num_features = feature_columns()
    work = frame.copy()
    for col in cat_features:
        if col not in work.columns:
            work[col] = "unknown"
        work[col] = work[col].fillna("unknown").astype(str)
    for col in num_features:
        if col not in work.columns:
            work[col] = 0.0
        work[col] = pd.to_numeric(work[col], errors="coerce").replace([np.inf, -np.inf], np.nan).fillna(0.0)
    cols = cat_features + num_features
    if include_target:
        return Pool(work[cols], label=work["rejector_target"].astype(int), cat_features=cat_features)
    return Pool(work[cols], cat_features=cat_features)


def train_model(train: pd.DataFrame, args: argparse.Namespace) -> CatBoostClassifier:
    model = CatBoostClassifier(
        loss_function="Logloss",
        eval_metric="AUC",
        iterations=int(args.iterations),
        depth=int(args.depth),
        learning_rate=float(args.learning_rate),
        l2_leaf_reg=float(args.l2_leaf_reg),
        random_seed=int(args.random_seed),
        auto_class_weights="Balanced",
        verbose=100,
        allow_writing_files=False,
    )
    model.fit(prepare_pool(train, include_target=True))
    return model


def add_scores(frame: pd.DataFrame, model: CatBoostClassifier) -> pd.DataFrame:
    scored = frame.copy()
    scored["rejector_score"] = model.predict_proba(prepare_pool(scored, include_target=False))[:, 1]
    return scored


def safe_auc(y: np.ndarray, score: np.ndarray) -> float | None:
    try:
        if len(set(int(v) for v in y)) < 2:
            return None
        return float(roc_auc_score(y, score))
    except ValueError:
        return None


def safe_ap(y: np.ndarray, score: np.ndarray) -> float | None:
    try:
        if len(set(int(v) for v in y)) < 2:
            return None
        return float(average_precision_score(y, score))
    except ValueError:
        return None


def metrics(frame: pd.DataFrame, source_counts: dict[str, int], threshold: float) -> dict[str, Any]:
    score = pd.to_numeric(frame["rejector_score"], errors="coerce").fillna(0.0)
    selected = frame[score >= float(threshold)].copy()
    picks = int(len(selected))
    true_picks = int((selected["rejector_target"] == 1).sum()) if picks else 0
    oracle_picks = int((selected["is_oracle_start"] == 1).sum()) if picks else 0
    positives = int(source_counts.get("target_events", 0))
    oracle_total = int(source_counts.get("oracle_events", 0))
    precision = true_picks / picks if picks else 0.0
    recall = true_picks / positives if positives else 0.0
    f1 = (2.0 * precision * recall / (precision + recall)) if precision + recall > 0 else 0.0
    y = frame["rejector_target"].astype(int).to_numpy()
    raw_score = score.to_numpy(dtype=float)
    return {
        "base_confirmed_events": int(len(frame)),
        "confirmed_events": picks,
        "true_confirmed_events": true_picks,
        "false_confirmed_events": int(picks - true_picks),
        "precision": precision,
        "recall": recall,
        "f1": f1,
        "oracle_confirmed_events": oracle_picks,
        "oracle_precision": oracle_picks / picks if picks else 0.0,
        "total_oracle_recall": oracle_picks / oracle_total if oracle_total else 0.0,
        "auc": safe_auc(y, raw_score),
        "average_precision": safe_ap(y, raw_score),
        "threshold": float(threshold),
    }


def choose_threshold(frame: pd.DataFrame, source_counts: dict[str, int], args: argparse.Namespace) -> tuple[float, pd.DataFrame]:
    score = pd.to_numeric(frame["rejector_score"], errors="coerce").fillna(0.0).to_numpy(dtype=float)
    quantiles = np.concatenate(
        [
            np.linspace(0.05, 0.99, 90),
            np.array([0.991, 0.992, 0.993, 0.994, 0.995, 0.996, 0.997, 0.998, 0.999, 0.9995, 0.9998, 0.9999]),
        ]
    )
    candidates = set(float(v) for v in np.quantile(score, quantiles))
    unique = np.unique(score)
    if len(unique):
        tail = unique[unique >= float(np.quantile(unique, 0.98))]
        if len(tail) > 500:
            tail = tail[np.linspace(0, len(tail) - 1, 500).astype(int)]
        candidates.update(float(v) for v in tail)
    rows: list[dict[str, Any]] = []
    best_threshold = 0.5
    best_key: tuple[float, float, int] | None = None
    for threshold in sorted(candidates):
        item = metrics(frame, source_counts, threshold)
        allowed = item["precision"] >= float(args.min_threshold_precision) and item["confirmed_events"] >= int(args.min_threshold_picks)
        rows.append({**item, "allowed": allowed})
        if allowed:
            key = (float(item["f1"]), float(item["precision"]), int(item["confirmed_events"]))
            if best_key is None or key > best_key:
                best_key = key
                best_threshold = float(threshold)
    if best_key is None:
        best_threshold = max(
            candidates,
            key=lambda threshold: (
                metrics(frame, source_counts, float(threshold))["precision"],
                metrics(frame, source_counts, float(threshold))["confirmed_events"],
            ),
        )
    return best_threshold, pd.DataFrame(rows)


def run_id(args: argparse.Namespace) -> str:
    raw = f"{args.run_prefix}-{args.stage2_run_id[-10:]}-v{args.valid_year}"
    if len(raw) <= 64:
        return raw
    return raw[:64]


def main() -> int:
    args = parse_args()
    stage2_dir, stage2_meta = source_dir(args.stage2_run_id)
    base_threshold = float(args.base_stage2_threshold)
    if base_threshold < 0:
        base_threshold = float(stage2_meta["selected_stage2_threshold"])
    rid = run_id(args)
    out_dir = wave.ABCD_ROOT / "model_registry" / rid
    if out_dir.exists() and not args.replace_run:
        raise ValueError(f"Rejector run already exists: {rid}. Use --replace-run.")
    out_dir.mkdir(parents=True, exist_ok=True)

    train_rows = load_rows(stage2_dir, "train")
    threshold_rows = load_rows(stage2_dir, int(args.threshold_year))
    valid_rows = load_rows(stage2_dir, int(args.valid_year))
    source_counts = {
        "train": event_counts(train_rows),
        str(args.threshold_year): event_counts(threshold_rows),
        str(args.valid_year): event_counts(valid_rows),
    }
    train = first_confirmations(train_rows, base_threshold)
    threshold = first_confirmations(threshold_rows, base_threshold)
    valid = first_confirmations(valid_rows, base_threshold)
    if train.empty or threshold.empty or valid.empty:
        raise ValueError("Base Stage 2 threshold produced empty confirmations.")

    print(
        f"Training rejector rows train={len(train):,} threshold={len(threshold):,} valid={len(valid):,} "
        f"base_stage2_threshold={base_threshold:.6f}",
        flush=True,
    )
    model = train_model(train, args)
    train_scored = add_scores(train, model)
    threshold_scored = add_scores(threshold, model)
    valid_scored = add_scores(valid, model)
    selected_threshold, sweep = choose_threshold(threshold_scored, source_counts[str(args.threshold_year)], args)
    results = {
        "train": metrics(train_scored, source_counts["train"], selected_threshold),
        str(args.threshold_year): metrics(threshold_scored, source_counts[str(args.threshold_year)], selected_threshold),
        str(args.valid_year): metrics(valid_scored, source_counts[str(args.valid_year)], selected_threshold),
    }
    print(f"selected_rejector_threshold={selected_threshold:.6f}", flush=True)
    print(json.dumps(results, indent=2, default=to_jsonable), flush=True)

    model.save_model(str(out_dir / "catboost_stage2_confirmation_rejector.cbm"))
    train_scored.to_csv(out_dir / "stage2_rejector_events_train.csv", index=False)
    threshold_scored.to_csv(out_dir / f"stage2_rejector_events_{args.threshold_year}.csv", index=False)
    valid_scored.to_csv(out_dir / f"stage2_rejector_events_{args.valid_year}.csv", index=False)
    sweep.to_csv(out_dir / f"stage2_rejector_threshold_sweep_{args.threshold_year}.csv", index=False)
    cat_features, num_features = feature_columns()
    metadata = {
        "run_id": rid,
        "model_type": "catboost_stage2_confirmation_rejector",
        "stage2_run_id": args.stage2_run_id,
        "base_stage2_threshold": base_threshold,
        "selected_rejector_threshold": selected_threshold,
        "threshold_year": int(args.threshold_year),
        "valid_year": int(args.valid_year),
        "features": {"cat": cat_features, "num": num_features},
        "source_counts": source_counts,
        "results": results,
    }
    (out_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")
    print(f"Saved Stage 2 rejector: {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
