#!/usr/bin/env python3
"""Shared LightGBM helpers for oracle trend-start models."""

from __future__ import annotations

import json
import math
from pathlib import Path
from typing import Any

import lightgbm as lgb
import numpy as np
import pandas as pd


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
) -> tuple[lgb.Booster, dict[str, dict[str, int]]]:
    category_maps = fit_category_maps(train, cat_features)
    features = prepare_features(train, cat_features, num_features, category_maps)
    labels = pd.to_numeric(train["is_oracle_start"], errors="coerce").fillna(0).astype(int)
    positive = int(labels.sum())
    negative = int(len(labels) - positive)
    scale_pos_weight = float(negative / positive) if positive > 0 else 1.0
    categorical_indices = [features.columns.get_loc(col) for col in cat_features]
    dataset = lgb.Dataset(
        features,
        label=labels,
        categorical_feature=categorical_indices,
        free_raw_data=False,
    )
    depth = int(getattr(args, "depth", 6))
    params = {
        "objective": "binary",
        "metric": ["auc", "binary_logloss"],
        "learning_rate": float(getattr(args, "learning_rate", 0.045)),
        "num_leaves": int(min(max(8, 2**max(depth, 1)), 255)),
        "max_depth": depth if depth > 0 else -1,
        "lambda_l2": float(getattr(args, "l2_leaf_reg", 10.0)),
        "scale_pos_weight": scale_pos_weight,
        "feature_fraction": 0.90,
        "bagging_fraction": 0.90,
        "bagging_freq": 1,
        "min_data_in_leaf": 40,
        "force_col_wise": True,
        "verbosity": -1,
        "seed": int(getattr(args, "random_seed", 73)),
        "num_threads": 0,
    }
    callbacks = [lgb.log_evaluation(period=100)]
    model = lgb.train(
        params,
        dataset,
        num_boost_round=int(getattr(args, "iterations", 650)),
        callbacks=callbacks,
    )
    return model, category_maps


def predict_scores(
    frame: pd.DataFrame,
    model: lgb.Booster,
    cat_features: list[str],
    num_features: list[str],
    category_maps: dict[str, dict[str, int]],
) -> np.ndarray:
    features = prepare_features(frame, cat_features, num_features, category_maps)
    return np.asarray(model.predict(features), dtype=float)


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


def load_model(model_dir: Path, model_name: str = "lightgbm_model.txt") -> tuple[lgb.Booster, list[str], list[str], dict[str, dict[str, int]]]:
    model = lgb.Booster(model_file=str(model_dir / model_name))
    cat_features, num_features, category_maps = load_feature_state(model_dir / "lightgbm_feature_state.json")
    return model, cat_features, num_features, category_maps
