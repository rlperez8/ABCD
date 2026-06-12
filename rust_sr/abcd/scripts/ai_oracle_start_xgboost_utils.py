#!/usr/bin/env python3
"""Shared XGBoost helpers for oracle trend-start models."""

from __future__ import annotations

import json
import math
from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd
import xgboost as xgb


def to_jsonable(value: Any) -> Any:
    if isinstance(value, pd.Timestamp):
        return value.isoformat()
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def fit_category_maps(frame: pd.DataFrame, cat_features: list[str]) -> dict[str, dict[str, int]]:
    maps: dict[str, dict[str, int]] = {}
    for col in cat_features:
        if col not in frame.columns:
            values: list[str] = []
        else:
            values = sorted(
                str(value)
                for value in frame[col].fillna("__missing__").astype(str).unique().tolist()
                if str(value) != ""
            )
        mapping = {"__unknown__": 0, "__missing__": 0}
        for index, value in enumerate(values, start=1):
            if value not in mapping:
                mapping[value] = index
        maps[col] = mapping
    return maps


def prepare_features(
    frame: pd.DataFrame,
    cat_features: list[str],
    num_features: list[str],
    category_maps: dict[str, dict[str, int]],
) -> pd.DataFrame:
    work = pd.DataFrame(index=frame.index)
    for col in cat_features:
        mapping = category_maps.get(col, {"__unknown__": 0, "__missing__": 0})
        if col not in frame.columns:
            series = pd.Series(["__missing__"] * len(frame), index=frame.index)
        else:
            series = frame[col].fillna("__missing__").astype(str)
        work[col] = series.map(mapping).fillna(0).astype(np.int32)
    for col in num_features:
        if col not in frame.columns:
            work[col] = 0.0
        else:
            work[col] = (
                pd.to_numeric(frame[col], errors="coerce")
                .replace([np.inf, -np.inf], np.nan)
                .fillna(0.0)
                .astype(np.float32)
            )
    return work[cat_features + num_features]


def train_binary(
    train: pd.DataFrame,
    args: Any,
    cat_features: list[str],
    num_features: list[str],
) -> tuple[xgb.XGBClassifier, dict[str, dict[str, int]]]:
    category_maps = fit_category_maps(train, cat_features)
    features = prepare_features(train, cat_features, num_features, category_maps)
    labels = pd.to_numeric(train["is_oracle_start"], errors="coerce").fillna(0).astype(int)
    positive = int(labels.sum())
    negative = int(len(labels) - positive)
    scale_pos_weight = float(negative / positive) if positive > 0 else 1.0
    depth = int(getattr(args, "depth", 6))
    model = xgb.XGBClassifier(
        objective="binary:logistic",
        eval_metric="logloss",
        tree_method="hist",
        n_estimators=int(getattr(args, "iterations", 650)),
        max_depth=depth if depth > 0 else 6,
        learning_rate=float(getattr(args, "learning_rate", 0.045)),
        reg_lambda=float(getattr(args, "l2_leaf_reg", 10.0)),
        subsample=0.90,
        colsample_bytree=0.90,
        min_child_weight=3.0,
        scale_pos_weight=scale_pos_weight,
        random_state=int(getattr(args, "random_seed", 73)),
        n_jobs=-1,
        verbosity=1,
    )
    model.fit(features, labels, verbose=False)
    return model, category_maps


def predict_scores(
    frame: pd.DataFrame,
    model: xgb.XGBClassifier,
    cat_features: list[str],
    num_features: list[str],
    category_maps: dict[str, dict[str, int]],
) -> np.ndarray:
    features = prepare_features(frame, cat_features, num_features, category_maps)
    return np.asarray(model.predict_proba(features)[:, 1], dtype=float)


def save_feature_state(path: Path, cat_features: list[str], num_features: list[str], category_maps: dict[str, dict[str, int]]) -> None:
    path.write_text(
        json.dumps(
            {
                "cat_features": cat_features,
                "num_features": num_features,
                "category_maps": category_maps,
            },
            indent=2,
            default=to_jsonable,
        ),
        encoding="utf-8",
    )


def load_feature_state(path: Path) -> tuple[list[str], list[str], dict[str, dict[str, int]]]:
    data = json.loads(path.read_text(encoding="utf-8"))
    return (
        list(data.get("cat_features") or []),
        list(data.get("num_features") or []),
        {
            str(col): {str(key): int(value) for key, value in dict(mapping).items()}
            for col, mapping in dict(data.get("category_maps") or {}).items()
        },
    )


def load_model(model_dir: Path, model_name: str = "xgboost_model.json") -> tuple[xgb.XGBClassifier, list[str], list[str], dict[str, dict[str, int]]]:
    model = xgb.XGBClassifier()
    model.load_model(str(model_dir / model_name))
    cat_features, num_features, category_maps = load_feature_state(model_dir / "xgboost_feature_state.json")
    return model, cat_features, num_features, category_maps
