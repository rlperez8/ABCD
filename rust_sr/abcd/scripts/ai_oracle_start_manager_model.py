#!/usr/bin/env python3
"""
Train an oracle-start manager model over the four specialist model scores.

The specialist models each answer "does this candle look like an oracle trend
start?" from their own point of view:

* CatBoost numeric model
* XGBoost numeric model
* LightGBM numeric model
* Visual CNN image model

This manager learns from oracle labels using those scores, score disagreements,
agreement counts, and market context. It does not create trades or exits.
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

from sklearn.metrics import average_precision_score, precision_recall_fscore_support, roc_auc_score


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_oracle_start_four_model_agreement as four_model
import ai_oracle_trend_start_model as start_model
import ai_wave_rider_research as wave


DEFAULT_FOUR_MODEL_RUN = "aicw-oracle-start-fourmodel-v1-v2026"
SCORE_COLUMNS = ["catboost_score", "xgboost_score", "lightgbm_score", "visual_score"]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--four-model-run-id", default=DEFAULT_FOUR_MODEL_RUN)
    parser.add_argument("--run-prefix", default="aicw-oracle-start-manager-v1")
    parser.add_argument("--train-label", default="train")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--max-train-rows", type=int, default=0)
    parser.add_argument("--max-eval-rows", type=int, default=0)
    parser.add_argument("--min-threshold-precision", type=float, default=0.68)
    parser.add_argument("--min-threshold-picks", type=int, default=300)
    parser.add_argument("--iterations", type=int, default=550)
    parser.add_argument("--depth", type=int, default=5)
    parser.add_argument("--learning-rate", type=float, default=0.045)
    parser.add_argument("--l2-leaf-reg", type=float, default=8.0)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--replace-run", action="store_true")
    return parser.parse_args()


def run_id(args: argparse.Namespace) -> str:
    rid = f"{args.run_prefix}-v{args.valid_year}"
    if len(rid) > 64:
        raise ValueError(f"Run id too long: {rid}")
    return rid


def load_four_metadata(four_model_run_id: str) -> tuple[Path, dict[str, Any]]:
    model_dir = wave.ABCD_ROOT / "model_registry" / four_model_run_id
    metadata_path = model_dir / "metadata.json"
    if not metadata_path.exists():
        raise FileNotFoundError(f"Missing four-model metadata: {metadata_path}")
    return model_dir, json.loads(metadata_path.read_text(encoding="utf-8"))


def load_split(model_dir: Path, split: str | int, limit: int) -> pd.DataFrame:
    if str(split) == "train":
        path = model_dir / "four_model_scores_train.csv"
    else:
        path = model_dir / f"four_model_scores_{split}.csv"
    if not path.exists():
        raise FileNotFoundError(f"Missing four-model score split: {path}")
    frame = pd.read_csv(path)
    if limit > 0 and len(frame) > limit:
        positives = frame[frame["is_oracle_start"] == 1]
        negatives = frame[frame["is_oracle_start"] == 0]
        keep_pos = positives.sample(n=min(len(positives), limit // 2), random_state=73)
        keep_neg = negatives.sample(n=min(len(negatives), max(0, limit - len(keep_pos))), random_state=73)
        frame = pd.concat([keep_pos, keep_neg], ignore_index=True).sample(frac=1.0, random_state=73).reset_index(drop=True)
    frame["is_oracle_start"] = pd.to_numeric(frame["is_oracle_start"], errors="coerce").fillna(0).astype(int)
    return frame


def add_manager_features(frame: pd.DataFrame, thresholds: dict[str, float]) -> pd.DataFrame:
    work = frame.copy()
    score_values = []
    for col in SCORE_COLUMNS:
        work[col] = pd.to_numeric(work.get(col), errors="coerce").fillna(0.0)
        score_values.append(work[col].to_numpy(dtype=float))
    scores = np.vstack(score_values).T
    tabular_scores = scores[:, :3]
    work["score_mean"] = scores.mean(axis=1)
    work["score_min"] = scores.min(axis=1)
    work["score_max"] = scores.max(axis=1)
    work["score_spread"] = scores.max(axis=1) - scores.min(axis=1)
    work["score_std"] = scores.std(axis=1)
    work["tabular_mean"] = tabular_scores.mean(axis=1)
    work["tabular_min"] = tabular_scores.min(axis=1)
    work["tabular_max"] = tabular_scores.max(axis=1)
    work["tabular_spread"] = tabular_scores.max(axis=1) - tabular_scores.min(axis=1)
    work["visual_minus_tabular_mean"] = work["visual_score"] - work["tabular_mean"]
    work["catboost_minus_tabular_mean"] = work["catboost_score"] - work["tabular_mean"]
    work["xgboost_minus_tabular_mean"] = work["xgboost_score"] - work["tabular_mean"]
    work["lightgbm_minus_tabular_mean"] = work["lightgbm_score"] - work["tabular_mean"]

    pick_cols = {
        "catboost_pick": ("catboost_score", thresholds.get("catboost", 0.5)),
        "xgboost_pick": ("xgboost_score", thresholds.get("xgboost", 0.5)),
        "lightgbm_pick": ("lightgbm_score", thresholds.get("lightgbm", 0.5)),
        "visual_pick": ("visual_score", thresholds.get("visual_cnn", thresholds.get("visual", 0.5))),
    }
    for out_col, (score_col, threshold) in pick_cols.items():
        work[out_col] = (work[score_col] >= float(threshold)).astype(int)
    work["tabular_pick_count"] = work[["catboost_pick", "xgboost_pick", "lightgbm_pick"]].sum(axis=1)
    work["all_pick_count"] = work[["catboost_pick", "xgboost_pick", "lightgbm_pick", "visual_pick"]].sum(axis=1)
    work["tabular_majority"] = (work["tabular_pick_count"] >= 2).astype(int)
    work["tabular_all"] = (work["tabular_pick_count"] == 3).astype(int)
    work["all_4_agree"] = (work["all_pick_count"] == 4).astype(int)
    work["image_and_tabular_majority"] = ((work["visual_pick"] == 1) & (work["tabular_pick_count"] >= 2)).astype(int)
    work["cat_visual_agree"] = ((work["catboost_pick"] == 1) & (work["visual_pick"] == 1)).astype(int)
    work["visual_lgbm_agree"] = ((work["visual_pick"] == 1) & (work["lightgbm_pick"] == 1)).astype(int)
    work["visual_xgb_agree"] = ((work["visual_pick"] == 1) & (work["xgboost_pick"] == 1)).astype(int)
    return work


DERIVED_NUM_FEATURES = [
    *SCORE_COLUMNS,
    "score_mean",
    "score_min",
    "score_max",
    "score_spread",
    "score_std",
    "tabular_mean",
    "tabular_min",
    "tabular_max",
    "tabular_spread",
    "visual_minus_tabular_mean",
    "catboost_minus_tabular_mean",
    "xgboost_minus_tabular_mean",
    "lightgbm_minus_tabular_mean",
    "catboost_pick",
    "xgboost_pick",
    "lightgbm_pick",
    "visual_pick",
    "tabular_pick_count",
    "all_pick_count",
    "tabular_majority",
    "tabular_all",
    "all_4_agree",
    "image_and_tabular_majority",
    "cat_visual_agree",
    "visual_lgbm_agree",
    "visual_xgb_agree",
]


def feature_columns() -> tuple[list[str], list[str]]:
    cat_features = list(start_model.CAT_FEATURES)
    num_features = list(start_model.NUM_FEATURES) + DERIVED_NUM_FEATURES
    return cat_features, num_features


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
        return Pool(work[cols], label=work["is_oracle_start"].astype(int), cat_features=cat_features)
    return Pool(work[cols], cat_features=cat_features)


def train_manager(train: pd.DataFrame, args: argparse.Namespace) -> CatBoostClassifier:
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
    scored["manager_score"] = model.predict_proba(prepare_pool(scored, include_target=False))[:, 1]
    return scored


def safe_auc(y_true: np.ndarray, score: np.ndarray) -> float | None:
    try:
        if len(set(int(v) for v in y_true)) < 2:
            return None
        return float(roc_auc_score(y_true, score))
    except ValueError:
        return None


def safe_ap(y_true: np.ndarray, score: np.ndarray) -> float | None:
    try:
        if len(set(int(v) for v in y_true)) < 2:
            return None
        return float(average_precision_score(y_true, score))
    except ValueError:
        return None


def score_metrics(frame: pd.DataFrame, score_col: str, threshold: float) -> dict[str, Any]:
    y = frame["is_oracle_start"].astype(int).to_numpy()
    score = pd.to_numeric(frame[score_col], errors="coerce").fillna(0.0).to_numpy()
    pred = score >= float(threshold)
    precision, recall, f1, _ = precision_recall_fscore_support(y, pred.astype(int), average="binary", zero_division=0)
    return {
        "rows": int(len(frame)),
        "positives": int(y.sum()),
        "picks": int(pred.sum()),
        "true_picks": int((pred & (y == 1)).sum()),
        "precision": float(precision),
        "recall": float(recall),
        "f1": float(f1),
        "auc": safe_auc(y, score),
        "average_precision": safe_ap(y, score),
        "threshold": float(threshold),
    }


def choose_threshold(frame: pd.DataFrame, args: argparse.Namespace) -> tuple[float, pd.DataFrame]:
    score = pd.to_numeric(frame["manager_score"], errors="coerce").fillna(0.0).to_numpy()
    candidates = sorted(set(float(v) for v in np.quantile(score, np.linspace(0.05, 0.99, 80))))
    best_threshold = candidates[0]
    best_key: tuple[float, float, int] | None = None
    rows: list[dict[str, Any]] = []
    for threshold in candidates:
        metrics = score_metrics(frame, "manager_score", threshold)
        allowed = metrics["precision"] >= float(args.min_threshold_precision) and metrics["picks"] >= int(args.min_threshold_picks)
        rows.append({**metrics, "allowed": allowed})
        if allowed:
            key = (float(metrics["f1"]), float(metrics["precision"]), int(metrics["picks"]))
            if best_key is None or key > best_key:
                best_key = key
                best_threshold = threshold
    if best_key is None:
        best_threshold = max(candidates, key=lambda value: score_metrics(frame, "manager_score", value)["f1"])
    return best_threshold, pd.DataFrame(rows)


def agreement_baselines(frame: pd.DataFrame, thresholds: dict[str, float]) -> pd.DataFrame:
    return four_model.agreement_metrics(frame, thresholds)


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
    rid = run_id(args)
    output_dir = wave.ABCD_ROOT / "model_registry" / rid
    if output_dir.exists() and not args.replace_run:
        raise ValueError(f"Manager run already exists: {rid}. Use --replace-run.")
    output_dir.mkdir(parents=True, exist_ok=True)

    source_dir, four_meta = load_four_metadata(str(args.four_model_run_id))
    thresholds = {name: float(value) for name, value in four_meta["thresholds"].items()}
    train = add_manager_features(load_split(source_dir, args.train_label, int(args.max_train_rows)), thresholds)
    threshold = add_manager_features(load_split(source_dir, int(args.threshold_year), int(args.max_eval_rows)), thresholds)
    valid = add_manager_features(load_split(source_dir, int(args.valid_year), int(args.max_eval_rows)), thresholds)
    print(f"Loaded manager rows train={len(train):,} threshold={len(threshold):,} valid={len(valid):,}", flush=True)

    model = train_manager(train, args)
    train_scored = add_scores(train, model)
    threshold_scored = add_scores(threshold, model)
    valid_scored = add_scores(valid, model)
    selected_threshold, sweep = choose_threshold(threshold_scored, args)
    print(f"selected_manager_threshold={selected_threshold:.6f}", flush=True)

    metrics: dict[str, Any] = {}
    baseline_rules: dict[str, Any] = {}
    for split_name, frame in [("train", train_scored), (str(args.threshold_year), threshold_scored), (str(args.valid_year), valid_scored)]:
        manager_metrics = score_metrics(frame, "manager_score", selected_threshold)
        baselines = agreement_baselines(frame, thresholds)
        baseline_rules[split_name] = baselines.to_dict(orient="records")
        metrics[split_name] = {
            "manager": manager_metrics,
            "baselines": baseline_rules[split_name],
        }
        baselines.to_csv(output_dir / f"manager_baseline_agreement_{split_name}.csv", index=False)
        print(f"{split_name}_manager={json.dumps(manager_metrics, default=to_jsonable)}", flush=True)
        print(f"{split_name}_baseline_top={baselines.sort_values('precision', ascending=False).head(6).to_dict(orient='records')}", flush=True)

    model.save_model(str(output_dir / "catboost_oracle_start_manager.cbm"))
    train_scored.to_csv(output_dir / "manager_scores_train.csv", index=False)
    threshold_scored.to_csv(output_dir / f"manager_scores_{args.threshold_year}.csv", index=False)
    valid_scored.to_csv(output_dir / f"manager_scores_{args.valid_year}.csv", index=False)
    sweep.to_csv(output_dir / f"manager_threshold_sweep_{args.threshold_year}.csv", index=False)
    cat_features, num_features = feature_columns()
    metadata = {
        "manager_run_id": rid,
        "model_type": "catboost_oracle_start_manager",
        "four_model_run_id": args.four_model_run_id,
        "train_label": args.train_label,
        "threshold_year": int(args.threshold_year),
        "valid_year": int(args.valid_year),
        "specialist_thresholds": thresholds,
        "selected_manager_threshold": float(selected_threshold),
        "features": {"cat": cat_features, "num": num_features},
        "metrics": metrics,
        "read": "Manager trains on oracle labels and learns how specialist model scores/agreement map to true oracle starts.",
    }
    (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")
    print(f"Saved oracle-start manager: {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
