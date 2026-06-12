#!/usr/bin/env python3
"""
Train XGBoost and LightGBM on the oracle-start sample, then compare four models.

Inputs come from the visual oracle-start CNN run so every model is evaluated on
the same rows:

* CatBoost score from the numeric oracle-start model
* Visual CNN score from the chart image model
* XGBoost score trained here on the same tabular features
* LightGBM score trained here on the same tabular features

This is still signal research only. It does not create trades or exits.
"""

from __future__ import annotations

import argparse
import itertools
import json
import math
import sys
import warnings
from pathlib import Path
from typing import Any

import joblib
import numpy as np
import pandas as pd
from sklearn.compose import ColumnTransformer
from sklearn.impute import SimpleImputer
from sklearn.metrics import average_precision_score, precision_recall_fscore_support, roc_auc_score
from sklearn.pipeline import Pipeline
from sklearn.preprocessing import OneHotEncoder

try:
    from lightgbm import LGBMClassifier
except ImportError as exc:  # pragma: no cover
    raise SystemExit("LightGBM is required. Run `.venv_ai\\Scripts\\python.exe -m pip install lightgbm`.") from exc

try:
    from xgboost import XGBClassifier
except ImportError as exc:  # pragma: no cover
    raise SystemExit("XGBoost is required. Run `.venv_ai\\Scripts\\python.exe -m pip install xgboost`.") from exc


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_oracle_trend_start_model as start_model
import ai_wave_rider_research as wave


DEFAULT_VISUAL_RUN = "aicw-oracle-start-visual-cnn-v1-2m-tr2024-v2026"
warnings.filterwarnings("ignore", message="X does not have valid feature names.*")

MODEL_COLUMNS = {
    "catboost": "catboost_score",
    "visual_cnn": "visual_score",
    "xgboost": "xgboost_score",
    "lightgbm": "lightgbm_score",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--visual-run-id", default=DEFAULT_VISUAL_RUN)
    parser.add_argument("--run-prefix", default="aicw-oracle-start-fourmodel-v1")
    parser.add_argument("--train-label", default="train")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--max-train-rows", type=int, default=0)
    parser.add_argument("--max-eval-rows", type=int, default=0)
    parser.add_argument("--xgb-estimators", type=int, default=450)
    parser.add_argument("--lgbm-estimators", type=int, default=650)
    parser.add_argument("--learning-rate", type=float, default=0.045)
    parser.add_argument("--max-depth", type=int, default=4)
    parser.add_argument("--min-threshold-precision", type=float, default=0.55)
    parser.add_argument("--min-threshold-picks", type=int, default=300)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--replace-run", action="store_true")
    return parser.parse_args()


def run_id(args: argparse.Namespace) -> str:
    rid = f"{args.run_prefix}-v{args.valid_year}"
    if len(rid) > 64:
        raise ValueError(f"Run id too long: {rid}")
    return rid


def visual_dir(run_id_value: str) -> Path:
    path = wave.ABCD_ROOT / "model_registry" / run_id_value
    if not path.exists():
        raise FileNotFoundError(f"Missing visual run: {path}")
    return path


def load_split(model_dir: Path, split: str | int, limit: int) -> pd.DataFrame:
    if str(split) == "train":
        path = model_dir / "visual_oracle_start_scores_train.csv"
    else:
        path = model_dir / f"visual_oracle_start_scores_{split}.csv"
    if not path.exists():
        raise FileNotFoundError(f"Missing score split: {path}")
    frame = pd.read_csv(path)
    if limit > 0 and len(frame) > limit:
        positives = frame[frame["is_oracle_start"] == 1]
        negatives = frame[frame["is_oracle_start"] == 0]
        keep_pos = positives.sample(n=min(len(positives), limit // 2), random_state=73)
        keep_neg = negatives.sample(n=min(len(negatives), max(0, limit - len(keep_pos))), random_state=73)
        frame = pd.concat([keep_pos, keep_neg], ignore_index=True).sample(frac=1.0, random_state=73).reset_index(drop=True)
    frame["is_oracle_start"] = pd.to_numeric(frame["is_oracle_start"], errors="coerce").fillna(0).astype(int)
    return frame


def one_hot_encoder() -> OneHotEncoder:
    try:
        return OneHotEncoder(handle_unknown="ignore", sparse_output=True)
    except TypeError:  # pragma: no cover - older sklearn
        return OneHotEncoder(handle_unknown="ignore", sparse=True)


def build_preprocessor() -> ColumnTransformer:
    return ColumnTransformer(
        transformers=[
            ("cat", one_hot_encoder(), start_model.CAT_FEATURES),
            ("num", SimpleImputer(strategy="median"), start_model.NUM_FEATURES),
        ],
        remainder="drop",
        sparse_threshold=0.3,
    )


def clean_features(frame: pd.DataFrame) -> pd.DataFrame:
    work = frame.copy()
    for col in start_model.CAT_FEATURES:
        if col not in work.columns:
            work[col] = "unknown"
        work[col] = work[col].fillna("unknown").astype(str)
    for col in start_model.NUM_FEATURES:
        if col not in work.columns:
            work[col] = np.nan
        work[col] = pd.to_numeric(work[col], errors="coerce").replace([np.inf, -np.inf], np.nan)
    return work


def train_xgboost(x_train: Any, y_train: np.ndarray, args: argparse.Namespace) -> XGBClassifier:
    model = XGBClassifier(
        n_estimators=int(args.xgb_estimators),
        max_depth=int(args.max_depth),
        learning_rate=float(args.learning_rate),
        subsample=0.9,
        colsample_bytree=0.9,
        min_child_weight=2.0,
        reg_lambda=4.0,
        objective="binary:logistic",
        eval_metric="logloss",
        tree_method="hist",
        n_jobs=-1,
        random_state=int(args.random_seed),
    )
    model.fit(x_train, y_train)
    return model


def train_lightgbm(x_train: Any, y_train: np.ndarray, args: argparse.Namespace) -> LGBMClassifier:
    model = LGBMClassifier(
        n_estimators=int(args.lgbm_estimators),
        max_depth=-1,
        num_leaves=31,
        learning_rate=float(args.learning_rate),
        subsample=0.9,
        colsample_bytree=0.9,
        reg_lambda=4.0,
        min_child_samples=30,
        objective="binary",
        n_jobs=-1,
        random_state=int(args.random_seed),
        verbose=-1,
    )
    model.fit(x_train, y_train)
    return model


def add_boost_scores(
    train: pd.DataFrame,
    threshold: pd.DataFrame,
    valid: pd.DataFrame,
    preprocessor: ColumnTransformer,
    xgb: XGBClassifier,
    lgbm: LGBMClassifier,
) -> tuple[pd.DataFrame, pd.DataFrame, pd.DataFrame]:
    scored_frames: list[pd.DataFrame] = []
    for frame in [train, threshold, valid]:
        clean = clean_features(frame)
        x = preprocessor.transform(clean)
        out = frame.copy()
        out["xgboost_score"] = xgb.predict_proba(x)[:, 1]
        out["lightgbm_score"] = lgbm.predict_proba(x)[:, 1]
        scored_frames.append(out)
    return scored_frames[0], scored_frames[1], scored_frames[2]


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


def choose_threshold(frame: pd.DataFrame, score_col: str, args: argparse.Namespace) -> tuple[float, pd.DataFrame]:
    score = pd.to_numeric(frame[score_col], errors="coerce").fillna(0.0).to_numpy()
    candidates = sorted(set(float(v) for v in np.quantile(score, np.linspace(0.05, 0.99, 60))))
    best_threshold = candidates[0]
    best_key: tuple[float, float, int] | None = None
    rows: list[dict[str, Any]] = []
    for threshold in candidates:
        metrics = score_metrics(frame, score_col, threshold)
        allowed = metrics["precision"] >= float(args.min_threshold_precision) and metrics["picks"] >= int(args.min_threshold_picks)
        rows.append({"model": score_col, **metrics, "allowed": allowed})
        if allowed:
            key = (float(metrics["f1"]), float(metrics["precision"]), int(metrics["picks"]))
            if best_key is None or key > best_key:
                best_key = key
                best_threshold = threshold
    if best_key is None:
        best_threshold = max(candidates, key=lambda value: score_metrics(frame, score_col, value)["f1"])
    return best_threshold, pd.DataFrame(rows)


def masks(frame: pd.DataFrame, thresholds: dict[str, float]) -> dict[str, np.ndarray]:
    out: dict[str, np.ndarray] = {}
    for model_name, score_col in MODEL_COLUMNS.items():
        score = pd.to_numeric(frame[score_col], errors="coerce").fillna(0.0).to_numpy()
        out[model_name] = score >= float(thresholds[model_name])
    return out


def mask_metrics(frame: pd.DataFrame, name: str, mask: np.ndarray) -> dict[str, Any]:
    y = frame["is_oracle_start"].astype(int).to_numpy()
    picks = int(mask.sum())
    true_picks = int((mask & (y == 1)).sum())
    return {
        "rule": name,
        "rows": int(len(frame)),
        "positives": int(y.sum()),
        "picks": picks,
        "true_picks": true_picks,
        "precision": true_picks / picks if picks else 0.0,
        "recall": true_picks / int(y.sum()) if int(y.sum()) else 0.0,
    }


def agreement_metrics(frame: pd.DataFrame, thresholds: dict[str, float]) -> pd.DataFrame:
    model_masks = masks(frame, thresholds)
    rows: list[dict[str, Any]] = []
    for model_name, mask in model_masks.items():
        rows.append(mask_metrics(frame, model_name, mask))
    for left, right in itertools.combinations(MODEL_COLUMNS, 2):
        rows.append(mask_metrics(frame, f"{left}+{right}", model_masks[left] & model_masks[right]))
    tabular_names = ["catboost", "xgboost", "lightgbm"]
    tabular_count = sum(model_masks[name].astype(int) for name in tabular_names)
    all_count = sum(mask.astype(int) for mask in model_masks.values())
    rows.extend(
        [
            mask_metrics(frame, "tabular_majority_2_of_3", tabular_count >= 2),
            mask_metrics(frame, "tabular_all_3", tabular_count == 3),
            mask_metrics(frame, "image_and_tabular_majority", model_masks["visual_cnn"] & (tabular_count >= 2)),
            mask_metrics(frame, "any_2_of_4", all_count >= 2),
            mask_metrics(frame, "any_3_of_4", all_count >= 3),
            mask_metrics(frame, "all_4", all_count == 4),
        ]
    )
    return pd.DataFrame(rows)


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
        raise ValueError(f"Four-model run already exists: {rid}. Use --replace-run.")
    output_dir.mkdir(parents=True, exist_ok=True)

    source_dir = visual_dir(str(args.visual_run_id))
    train = load_split(source_dir, str(args.train_label), int(args.max_train_rows))
    threshold = load_split(source_dir, int(args.threshold_year), int(args.max_eval_rows))
    valid = load_split(source_dir, int(args.valid_year), int(args.max_eval_rows))
    print(f"Loaded rows train={len(train):,} threshold={len(threshold):,} valid={len(valid):,}", flush=True)

    preprocessor = build_preprocessor()
    x_train = preprocessor.fit_transform(clean_features(train))
    y_train = train["is_oracle_start"].astype(int).to_numpy()
    print(f"Training XGBoost features={x_train.shape}", flush=True)
    xgb = train_xgboost(x_train, y_train, args)
    print("Training LightGBM", flush=True)
    lgbm = train_lightgbm(x_train, y_train, args)
    train_scored, threshold_scored, valid_scored = add_boost_scores(train, threshold, valid, preprocessor, xgb, lgbm)

    thresholds: dict[str, float] = {}
    sweeps: list[pd.DataFrame] = []
    for model_name, score_col in MODEL_COLUMNS.items():
        threshold_value, sweep = choose_threshold(threshold_scored, score_col, args)
        thresholds[model_name] = float(threshold_value)
        sweep["model_name"] = model_name
        sweeps.append(sweep)
        print(f"selected_{model_name}_threshold={threshold_value:.6f}", flush=True)

    split_frames = {"train": train_scored, str(args.threshold_year): threshold_scored, str(args.valid_year): valid_scored}
    split_metrics: dict[str, Any] = {}
    for split_name, frame in split_frames.items():
        model_items = {
            model_name: score_metrics(frame, score_col, thresholds[model_name])
            for model_name, score_col in MODEL_COLUMNS.items()
        }
        agreement = agreement_metrics(frame, thresholds)
        split_metrics[split_name] = {
            "models": model_items,
            "agreement": agreement.to_dict(orient="records"),
        }
        agreement.to_csv(output_dir / f"agreement_metrics_{split_name}.csv", index=False)
        print(f"{split_name}_agreement_top={agreement.sort_values('precision', ascending=False).head(8).to_dict(orient='records')}", flush=True)

    train_scored.to_csv(output_dir / "four_model_scores_train.csv", index=False)
    threshold_scored.to_csv(output_dir / f"four_model_scores_{args.threshold_year}.csv", index=False)
    valid_scored.to_csv(output_dir / f"four_model_scores_{args.valid_year}.csv", index=False)
    pd.concat(sweeps, ignore_index=True).to_csv(output_dir / f"threshold_sweeps_{args.threshold_year}.csv", index=False)
    joblib.dump(preprocessor, output_dir / "tabular_preprocessor.joblib")
    joblib.dump(xgb, output_dir / "xgboost_oracle_start.joblib")
    joblib.dump(lgbm, output_dir / "lightgbm_oracle_start.joblib")
    metadata = {
        "run_id": rid,
        "model_type": "oracle_start_four_model_agreement",
        "visual_run_id": args.visual_run_id,
        "train_label": args.train_label,
        "threshold_year": int(args.threshold_year),
        "valid_year": int(args.valid_year),
        "feature_source": "visual oracle-start score rows; all four models evaluated on identical sampled rows",
        "features": {"cat": start_model.CAT_FEATURES, "num": start_model.NUM_FEATURES},
        "thresholds": thresholds,
        "metrics": split_metrics,
        "caution": "Sampled balanced oracle-start rows only. A full candle-grid consensus pass is still needed before treating this as live entry logic.",
    }
    (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")
    print(f"Saved four-model oracle agreement run: {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
