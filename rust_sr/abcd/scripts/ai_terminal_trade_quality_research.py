#!/usr/bin/env python3
"""
Train a live-compatible trade-quality layer over terminal Stage 2/3 candidates.

This is a research harness for the final "should we actually take this trade?"
layer. It intentionally only uses fields known at entry time:

    root/symbol/direction, entry time, Stage 2 score, risk ticks, and slippage R.

It does not use exit reason, hold duration, model exit date, or any future path
feature. The script trains on one available year and validates on the other,
then repeats in reverse so the result is harder to fool with one-year noise.
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd
from catboost import CatBoostClassifier, CatBoostRegressor, Pool


SCRIPT_DIR = Path(__file__).resolve().parent
ABCD_ROOT = SCRIPT_DIR.parent
MODEL_REGISTRY = ABCD_ROOT / "model_registry"

DEFAULT_FILES = {
    2025: MODEL_REGISTRY
    / "aicw-os-stage3-terminal-both-threshold-v2-2m-m180-v2025"
    / "stage3_terminal_hold_trades_2025.csv",
    2026: MODEL_REGISTRY
    / "aicw-os-stage3-terminal-both-full-v2-2m-m180-v2026"
    / "stage3_terminal_hold_trades_2026.csv",
}

DEFAULT_STAGE2_EVENTS = {
    2025: MODEL_REGISTRY
    / "aicw-os-stage2-l2-confirm-v1-2m-m-tr2025-v2026-m16-v2026"
    / "stage2_l2_events_threshold.csv",
    2026: MODEL_REGISTRY
    / "aicw-os-stage2-l2-confirm-v1-2m-m-tr2025-v2026-m16-v2026"
    / "stage2_l2_events_2026.csv",
}

CAT_FEATURES = [
    "root_symbol",
    "symbol",
    "direction",
    "entry_hour_bucket",
    "entry_weekday",
    "entry_month",
    "risk_bucket",
    "score_bucket",
    "top_l1_block",
    "l1_leader",
    "l1_agreement_bucket",
    "confirm_offset_bucket",
]

NUM_FEATURES = [
    "stage2_score",
    "event_stage1_score",
    "event_level2_score",
    "event_stage2_score",
    "cat_l1_score",
    "light_l1_score",
    "xgb_l1_score",
    "l1_mean",
    "l1_std",
    "l1_spread",
    "l1_min",
    "l1_max",
    "confirm_offset_bars",
    "confirm_offset_abs",
    "risk_ticks",
    "slippage_r",
    "score_x_risk",
    "score_to_slippage",
    "risk_log",
    "entry_hour",
    "is_rth",
]

FEATURES = CAT_FEATURES + NUM_FEATURES

ROOT_GROUPS = {
    "all": "CL,EMD,ES,GF,HO,LE,NG,NKD,NQ,RB,RTY,YM",
    "pure_r_clean": "CL,EMD,HO,NG,RB",
    "pure_r_clean_nq": "CL,EMD,HO,NG,NQ,RB",
    "risk_adjusted": "HO,NG,NQ",
    "prior_clean": "EMD,HO,NG,NQ",
    "energy_clean": "HO,NG",
    "nq_ho": "HO,NQ",
}

BASE_RULES = [
    {"base_rule": "lowdd_0910_r72_144", "score_min": 0.910, "risk_min": 72.0, "risk_max": 144.0},
    {"base_rule": "clean_0930_r48_192", "score_min": 0.930, "risk_min": 48.0, "risk_max": 192.0},
    {"base_rule": "tight_0930_r72_192", "score_min": 0.930, "risk_min": 72.0, "risk_max": 192.0},
    {"base_rule": "push_0915_r60_240", "score_min": 0.915, "risk_min": 60.0, "risk_max": 240.0},
    {"base_rule": "wide_0920_r48_240", "score_min": 0.920, "risk_min": 48.0, "risk_max": 240.0},
]

RANK_MODES = ["all", "top1_per_entry_time", "top2_per_entry_time", "top3_per_entry_time"]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-id", default="aicw-terminal-trade-quality-v1-2025-2026")
    parser.add_argument("--replace", action="store_true")
    parser.add_argument("--slippage-total-ticks", type=float, default=6.0)
    parser.add_argument("--target-clip-low", type=float, default=-1.5)
    parser.add_argument("--target-clip-high", type=float, default=8.0)
    parser.add_argument("--iterations", type=int, default=300)
    parser.add_argument("--depth", type=int, default=4)
    parser.add_argument("--learning-rate", type=float, default=0.045)
    parser.add_argument("--min-train-trades", type=int, default=80)
    parser.add_argument("--top-rules-per-fold", type=int, default=24)
    return parser.parse_args()


def finite(value: Any, default: float = 0.0) -> float:
    try:
        numeric = float(value)
    except (TypeError, ValueError):
        return default
    if not math.isfinite(numeric):
        return default
    return numeric


def to_jsonable(value: Any) -> Any:
    if isinstance(value, pd.Timestamp):
        return None if pd.isna(value) else value.isoformat()
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def score_bucket(value: float) -> str:
    if value >= 0.94:
        return "094+"
    if value >= 0.93:
        return "093-094"
    if value >= 0.92:
        return "092-093"
    if value >= 0.91:
        return "091-092"
    if value >= 0.90:
        return "090-091"
    return "lt090"


def risk_bucket(value: float) -> str:
    if value < 48:
        return "lt48"
    if value < 72:
        return "048-072"
    if value < 96:
        return "072-096"
    if value < 144:
        return "096-144"
    if value < 192:
        return "144-192"
    return "192+"


def confirm_offset_bucket(value: float) -> str:
    if not math.isfinite(value):
        return "missing"
    if value <= 1:
        return "01"
    if value <= 2:
        return "02"
    if value <= 4:
        return "03-04"
    if value <= 8:
        return "05-08"
    if value <= 16:
        return "09-16"
    return "17+"


def l1_agreement_bucket(spread: float) -> str:
    if not math.isfinite(spread):
        return "missing"
    if spread <= 0.03:
        return "tight"
    if spread <= 0.07:
        return "medium"
    return "wide"


def hour_bucket(hour: int) -> str:
    if 0 <= hour <= 5:
        return "overnight"
    if 6 <= hour <= 8:
        return "pre_rth"
    if 9 <= hour <= 11:
        return "morning"
    if 12 <= hour <= 14:
        return "midday"
    if 15 <= hour <= 17:
        return "afternoon"
    return "evening"


def load_terminal_rows(path: Path, year: int, slippage_total_ticks: float) -> pd.DataFrame:
    if not path.exists():
        raise FileNotFoundError(path)
    frame = pd.read_csv(path)
    frame = frame.rename(
        columns={
            "predicted_r": "stage2_score",
            "model_result_r": "result_r",
            "model_exit_date": "exit_date",
        }
    )
    required = [
        "selected_index",
        "candidate_uid",
        "root_symbol",
        "symbol",
        "direction",
        "entry_date",
        "exit_date",
        "stage2_score",
        "risk_ticks",
        "result_r",
    ]
    missing = [column for column in required if column not in frame.columns]
    if missing:
        raise ValueError(f"{path} missing columns: {missing}")
    frame = frame[required].copy()
    frame["year"] = int(year)
    frame["entry_date"] = pd.to_datetime(frame["entry_date"], errors="coerce")
    frame["exit_date"] = pd.to_datetime(frame["exit_date"], errors="coerce")
    frame["root_symbol"] = frame["root_symbol"].astype(str).str.upper()
    frame["symbol"] = frame["symbol"].astype(str).str.upper()
    frame["direction"] = frame["direction"].astype(str).str.upper()
    frame["stage2_score"] = pd.to_numeric(frame["stage2_score"], errors="coerce").fillna(0.0)
    frame["risk_ticks"] = pd.to_numeric(frame["risk_ticks"], errors="coerce").fillna(0.0)
    frame["result_r"] = pd.to_numeric(frame["result_r"], errors="coerce").fillna(0.0)
    frame = frame.dropna(subset=["entry_date", "exit_date"]).reset_index(drop=True)

    frame["entry_hour"] = frame["entry_date"].dt.hour.astype(int)
    frame["entry_weekday"] = frame["entry_date"].dt.day_name().astype(str)
    frame["entry_month"] = frame["entry_date"].dt.month_name().astype(str)
    frame["entry_hour_bucket"] = frame["entry_hour"].map(hour_bucket)
    frame["risk_bucket"] = frame["risk_ticks"].map(risk_bucket)
    frame["score_bucket"] = frame["stage2_score"].map(score_bucket)
    frame["slippage_r"] = slippage_total_ticks / frame["risk_ticks"].clip(lower=1e-9)
    frame["score_x_risk"] = frame["stage2_score"] * frame["risk_ticks"]
    frame["score_to_slippage"] = frame["stage2_score"] / frame["slippage_r"].clip(lower=1e-9)
    frame["risk_log"] = np.log1p(frame["risk_ticks"].clip(lower=0.0))
    frame["is_rth"] = frame["entry_hour"].between(8, 15).astype(float)
    return frame.sort_values("entry_date").reset_index(drop=True)


def load_stage2_events(path: Path) -> pd.DataFrame:
    if not path.exists():
        raise FileNotFoundError(path)
    usecols = [
        "candidate_uid",
        "stage1_score",
        "level2_score",
        "cat_l1_score",
        "light_l1_score",
        "xgb_l1_score",
        "top_l1_block",
        "confirm_offset_bars",
        "stage2_score",
    ]
    events = pd.read_csv(path, usecols=lambda column: column in usecols)
    events = events.rename(
        columns={
            "stage1_score": "event_stage1_score",
            "level2_score": "event_level2_score",
            "stage2_score": "event_stage2_score",
        }
    )
    events["candidate_uid"] = events["candidate_uid"].astype(str)
    for column in [
        "event_stage1_score",
        "event_level2_score",
        "event_stage2_score",
        "cat_l1_score",
        "light_l1_score",
        "xgb_l1_score",
        "confirm_offset_bars",
    ]:
        if column not in events.columns:
            events[column] = np.nan
        events[column] = pd.to_numeric(events[column], errors="coerce")
    if "top_l1_block" not in events.columns:
        events["top_l1_block"] = "missing"
    events["top_l1_block"] = events["top_l1_block"].astype(str).replace({"nan": "missing"})
    score_cols = ["cat_l1_score", "light_l1_score", "xgb_l1_score"]
    events["l1_mean"] = events[score_cols].mean(axis=1)
    events["l1_std"] = events[score_cols].std(axis=1).fillna(0.0)
    events["l1_min"] = events[score_cols].min(axis=1)
    events["l1_max"] = events[score_cols].max(axis=1)
    events["l1_spread"] = events["l1_max"] - events["l1_min"]
    events["l1_leader"] = events[score_cols].idxmax(axis=1).str.replace("_l1_score", "", regex=False)
    events["l1_agreement_bucket"] = events["l1_spread"].map(l1_agreement_bucket)
    events["confirm_offset_abs"] = events["confirm_offset_bars"].abs()
    events["confirm_offset_bucket"] = events["confirm_offset_abs"].map(confirm_offset_bucket)
    keep = [
        "candidate_uid",
        "event_stage1_score",
        "event_level2_score",
        "event_stage2_score",
        "cat_l1_score",
        "light_l1_score",
        "xgb_l1_score",
        "top_l1_block",
        "l1_mean",
        "l1_std",
        "l1_spread",
        "l1_min",
        "l1_max",
        "l1_leader",
        "l1_agreement_bucket",
        "confirm_offset_bars",
        "confirm_offset_abs",
        "confirm_offset_bucket",
    ]
    return events[keep].drop_duplicates("candidate_uid", keep="last")


def add_stage2_event_features(frame: pd.DataFrame, events: pd.DataFrame) -> pd.DataFrame:
    merged = frame.merge(events, on="candidate_uid", how="left")
    for column in [
        "event_stage1_score",
        "event_level2_score",
        "event_stage2_score",
        "cat_l1_score",
        "light_l1_score",
        "xgb_l1_score",
        "l1_mean",
        "l1_std",
        "l1_spread",
        "l1_min",
        "l1_max",
        "confirm_offset_bars",
        "confirm_offset_abs",
    ]:
        merged[column] = pd.to_numeric(merged.get(column), errors="coerce").replace([np.inf, -np.inf], np.nan)
    merged["event_stage1_score"] = merged["event_stage1_score"].fillna(merged["stage2_score"])
    merged["event_level2_score"] = merged["event_level2_score"].fillna(merged["stage2_score"])
    merged["event_stage2_score"] = merged["event_stage2_score"].fillna(merged["stage2_score"])
    for column in ["cat_l1_score", "light_l1_score", "xgb_l1_score"]:
        merged[column] = merged[column].fillna(merged["event_level2_score"])
    score_cols = ["cat_l1_score", "light_l1_score", "xgb_l1_score"]
    merged["l1_mean"] = merged["l1_mean"].fillna(merged[score_cols].mean(axis=1))
    merged["l1_std"] = merged["l1_std"].fillna(merged[score_cols].std(axis=1).fillna(0.0))
    merged["l1_min"] = merged["l1_min"].fillna(merged[score_cols].min(axis=1))
    merged["l1_max"] = merged["l1_max"].fillna(merged[score_cols].max(axis=1))
    merged["l1_spread"] = merged["l1_spread"].fillna(merged["l1_max"] - merged["l1_min"])
    merged["confirm_offset_bars"] = merged["confirm_offset_bars"].fillna(0.0)
    merged["confirm_offset_abs"] = merged["confirm_offset_abs"].fillna(merged["confirm_offset_bars"].abs())
    for column, default in [
        ("top_l1_block", "missing"),
        ("l1_leader", "missing"),
        ("l1_agreement_bucket", "missing"),
        ("confirm_offset_bucket", "missing"),
    ]:
        merged[column] = merged.get(column, default)
        merged[column] = merged[column].astype(str).fillna(default).replace({"nan": default})
    return merged


def prepare_features(frame: pd.DataFrame) -> pd.DataFrame:
    work = frame.copy()
    for column in CAT_FEATURES:
        work[column] = work[column].astype(str).fillna("missing")
    for column in NUM_FEATURES:
        work[column] = pd.to_numeric(work[column], errors="coerce").replace([np.inf, -np.inf], np.nan).fillna(0.0)
    return work[FEATURES]


def train_models(train: pd.DataFrame, args: argparse.Namespace) -> tuple[CatBoostRegressor, CatBoostClassifier]:
    x_train = prepare_features(train)
    target_r = train["result_r"].clip(float(args.target_clip_low), float(args.target_clip_high))
    target_win = (train["result_r"] > 0).astype(int)
    cat_indices = [FEATURES.index(column) for column in CAT_FEATURES]

    regressor = CatBoostRegressor(
        iterations=int(args.iterations),
        depth=int(args.depth),
        learning_rate=float(args.learning_rate),
        loss_function="RMSE",
        random_seed=73,
        verbose=False,
        allow_writing_files=False,
    )
    regressor.fit(Pool(x_train, label=target_r, cat_features=cat_indices))

    classifier = CatBoostClassifier(
        iterations=int(args.iterations),
        depth=int(args.depth),
        learning_rate=float(args.learning_rate),
        loss_function="Logloss",
        random_seed=137,
        verbose=False,
        allow_writing_files=False,
    )
    classifier.fit(Pool(x_train, label=target_win, cat_features=cat_indices))
    return regressor, classifier


def add_quality_scores(frame: pd.DataFrame, regressor: CatBoostRegressor, classifier: CatBoostClassifier) -> pd.DataFrame:
    scored = frame.copy()
    x = prepare_features(scored)
    cat_indices = [FEATURES.index(column) for column in CAT_FEATURES]
    pool = Pool(x, cat_features=cat_indices)
    scored["quality_pred_r"] = regressor.predict(pool)
    scored["quality_win_prob"] = classifier.predict_proba(pool)[:, 1]
    # Blend expected R with win probability as a tie-breaker. The R prediction
    # remains dominant because the objective is total R, not win rate.
    scored["quality_score"] = scored["quality_pred_r"] + 0.35 * (scored["quality_win_prob"] - 0.5)
    return scored


def max_drawdown_r(results: list[float]) -> float:
    peak = 0.0
    current = 0.0
    max_dd = 0.0
    for result in results:
        current += float(result)
        peak = max(peak, current)
        max_dd = max(max_dd, peak - current)
    return float(max_dd)


def max_concurrent(frame: pd.DataFrame) -> int:
    if frame.empty:
        return 0
    events: list[tuple[pd.Timestamp, int]] = []
    for row in frame[["entry_date", "exit_date"]].itertuples(index=False):
        events.append((pd.Timestamp(row.entry_date), 1))
        events.append((pd.Timestamp(row.exit_date), -1))
    events.sort(key=lambda item: (item[0], item[1]))
    current = 0
    max_seen = 0
    for _, delta in events:
        current += int(delta)
        max_seen = max(max_seen, current)
    return int(max_seen)


def apply_rank_mode(frame: pd.DataFrame, rank_mode: str) -> pd.DataFrame:
    if frame.empty or rank_mode == "all":
        return frame.copy()
    if rank_mode.startswith("top") and rank_mode.endswith("_per_entry_time"):
        top_n = int(rank_mode.split("_", 1)[0].replace("top", ""))
        ranked = frame.sort_values(
            ["entry_date", "quality_score", "quality_pred_r", "stage2_score", "risk_ticks"],
            ascending=[True, False, False, False, True],
            kind="mergesort",
        ).copy()
        ranked["entry_time_rank"] = ranked.groupby("entry_date").cumcount() + 1
        return ranked[ranked["entry_time_rank"] <= top_n].copy()
    raise ValueError(f"Unknown rank mode: {rank_mode}")


def summarize(frame: pd.DataFrame) -> dict[str, Any]:
    if frame.empty:
        return {
            "trades": 0,
            "wins": 0,
            "losses": 0,
            "win_rate": 0.0,
            "sum_r": 0.0,
            "avg_r": 0.0,
            "dd_r": 0.0,
            "score_to_dd_r": 0.0,
            "max_concurrent": 0,
            "avg_quality_score": 0.0,
            "avg_stage2_score": 0.0,
            "avg_risk_ticks": 0.0,
        }
    ordered = frame.sort_values("exit_date", kind="mergesort")
    results = [float(value) for value in ordered["result_r"]]
    wins = sum(1 for value in results if value > 0)
    losses = len(results) - wins
    dd_r = max_drawdown_r(results)
    sum_r = float(sum(results))
    return {
        "trades": int(len(results)),
        "wins": int(wins),
        "losses": int(losses),
        "win_rate": float(wins / len(results)) if results else 0.0,
        "sum_r": sum_r,
        "avg_r": float(np.mean(results)) if results else 0.0,
        "dd_r": dd_r,
        "score_to_dd_r": float(sum_r / dd_r) if dd_r > 0 else (999.0 if sum_r > 0 else 0.0),
        "max_concurrent": max_concurrent(frame),
        "avg_quality_score": float(frame["quality_score"].mean()) if "quality_score" in frame else 0.0,
        "avg_stage2_score": float(frame["stage2_score"].mean()),
        "avg_risk_ticks": float(frame["risk_ticks"].mean()),
    }


def filter_base(frame: pd.DataFrame, roots: set[str], base_rule: dict[str, Any]) -> pd.DataFrame:
    return frame[
        frame["root_symbol"].isin(roots)
        & (frame["stage2_score"] >= float(base_rule["score_min"]))
        & (frame["risk_ticks"] >= float(base_rule["risk_min"]))
        & (frame["risk_ticks"] <= float(base_rule["risk_max"]))
    ].copy()


def threshold_candidates(frame: pd.DataFrame) -> list[float]:
    if frame.empty:
        return [float("inf")]
    quantiles = [0.0, 0.25, 0.50, 0.60, 0.70, 0.80, 0.85, 0.90, 0.925, 0.95, 0.975, 0.99]
    values = sorted(set(float(value) for value in frame["quality_score"].quantile(quantiles).dropna()))
    return values


def evaluate_rule(
    frame: pd.DataFrame,
    roots: set[str],
    base_rule: dict[str, Any],
    quality_min: float | None,
    rank_mode: str,
) -> tuple[pd.DataFrame, dict[str, Any]]:
    selected = filter_base(frame, roots, base_rule)
    if quality_min is not None:
        selected = selected[selected["quality_score"] >= float(quality_min)].copy()
    selected = apply_rank_mode(selected, rank_mode)
    return selected, summarize(selected)


def select_rules(train_scored: pd.DataFrame, args: argparse.Namespace) -> pd.DataFrame:
    rows = []
    for group_name, root_csv in ROOT_GROUPS.items():
        roots = {part.strip().upper() for part in root_csv.split(",") if part.strip()}
        for base_rule in BASE_RULES:
            base_frame = filter_base(train_scored, roots, base_rule)
            for rank_mode in RANK_MODES:
                # Baseline row with no quality threshold.
                _, stats = evaluate_rule(train_scored, roots, base_rule, None, rank_mode)
                rows.append(
                    {
                        "root_group": group_name,
                        "roots": ",".join(sorted(roots)),
                        **base_rule,
                        "quality_min": None,
                        "rank_mode": rank_mode,
                        "uses_quality_filter": False,
                        **stats,
                    }
                )
                for quality_min in threshold_candidates(base_frame):
                    _, stats = evaluate_rule(train_scored, roots, base_rule, float(quality_min), rank_mode)
                    rows.append(
                        {
                            "root_group": group_name,
                            "roots": ",".join(sorted(roots)),
                            **base_rule,
                            "quality_min": float(quality_min),
                            "rank_mode": rank_mode,
                            "uses_quality_filter": True,
                            **stats,
                        }
                    )
    rules = pd.DataFrame(rows)
    rules = rules[rules["trades"] >= int(args.min_train_trades)].copy()
    rules = rules.sort_values(
        ["score_to_dd_r", "sum_r", "avg_r", "trades"],
        ascending=[False, False, False, False],
        kind="mergesort",
    ).reset_index(drop=True)
    rules["train_rule_rank"] = rules.index + 1
    return rules.head(int(args.top_rules_per_fold)).copy()


def apply_selected_rules(valid_scored: pd.DataFrame, selected_rules: pd.DataFrame) -> pd.DataFrame:
    rows = []
    for rule in selected_rules.itertuples(index=False):
        roots = {part.strip().upper() for part in str(rule.roots).split(",") if part.strip()}
        base_rule = {
            "base_rule": rule.base_rule,
            "score_min": float(rule.score_min),
            "risk_min": float(rule.risk_min),
            "risk_max": float(rule.risk_max),
        }
        quality_min = None if pd.isna(rule.quality_min) else float(rule.quality_min)
        selected, stats = evaluate_rule(valid_scored, roots, base_rule, quality_min, str(rule.rank_mode))
        row = {
            "train_rule_rank": int(rule.train_rule_rank),
            "root_group": rule.root_group,
            "roots": rule.roots,
            "base_rule": rule.base_rule,
            "score_min": float(rule.score_min),
            "risk_min": float(rule.risk_min),
            "risk_max": float(rule.risk_max),
            "quality_min": quality_min,
            "rank_mode": rule.rank_mode,
            "uses_quality_filter": bool(rule.uses_quality_filter),
            "selected_candidate_uids": ",".join(selected["candidate_uid"].astype(str).head(50).to_list()),
            **stats,
        }
        rows.append(row)
    return pd.DataFrame(rows)


def score_and_compare(
    train_year: int,
    valid_year: int,
    data_by_year: dict[int, pd.DataFrame],
    output_dir: Path,
    args: argparse.Namespace,
) -> dict[str, Any]:
    train = data_by_year[train_year].copy()
    valid = data_by_year[valid_year].copy()
    regressor, classifier = train_models(train, args)
    regressor.save_model(str(output_dir / f"quality_regressor_train{train_year}_valid{valid_year}.cbm"))
    classifier.save_model(str(output_dir / f"quality_classifier_train{train_year}_valid{valid_year}.cbm"))

    train_scored = add_quality_scores(train, regressor, classifier)
    valid_scored = add_quality_scores(valid, regressor, classifier)
    train_scored.to_csv(output_dir / f"quality_scored_train{train_year}.csv", index=False)
    valid_scored.to_csv(output_dir / f"quality_scored_valid{valid_year}_from{train_year}.csv", index=False)

    selected_train_rules = select_rules(train_scored, args)
    selected_train_rules.to_csv(output_dir / f"quality_selected_rules_train{train_year}.csv", index=False)
    valid_results = apply_selected_rules(valid_scored, selected_train_rules)
    valid_results.to_csv(output_dir / f"quality_valid_results_train{train_year}_valid{valid_year}.csv", index=False)

    top_valid = valid_results.sort_values(
        ["score_to_dd_r", "sum_r", "avg_r", "trades"],
        ascending=[False, False, False, False],
        kind="mergesort",
    ).head(10)
    top_valid.to_csv(output_dir / f"quality_top_valid_train{train_year}_valid{valid_year}.csv", index=False)

    # Also save the actual trade rows for the best validation rule so the UI or
    # follow-up inspection can load the candidate IDs directly.
    if not top_valid.empty:
        best = top_valid.iloc[0]
        roots = {part.strip().upper() for part in str(best["roots"]).split(",") if part.strip()}
        base_rule = {
            "base_rule": best["base_rule"],
            "score_min": float(best["score_min"]),
            "risk_min": float(best["risk_min"]),
            "risk_max": float(best["risk_max"]),
        }
        quality_min = None if pd.isna(best["quality_min"]) else float(best["quality_min"])
        best_rows, _ = evaluate_rule(valid_scored, roots, base_rule, quality_min, str(best["rank_mode"]))
        best_rows.to_csv(output_dir / f"quality_best_valid_trades_train{train_year}_valid{valid_year}.csv", index=False)

    return {
        "train_year": train_year,
        "valid_year": valid_year,
        "train_rows": int(len(train)),
        "valid_rows": int(len(valid)),
        "selected_rules": int(len(selected_train_rules)),
        "top_valid": top_valid.to_dict("records"),
    }


def main() -> int:
    args = parse_args()
    output_dir = MODEL_REGISTRY / args.run_id
    if output_dir.exists() and any(output_dir.iterdir()) and not args.replace:
        raise SystemExit(f"Output exists: {output_dir}. Use --replace.")
    output_dir.mkdir(parents=True, exist_ok=True)

    data_by_year = {
        year: load_terminal_rows(path, year, float(args.slippage_total_ticks))
        for year, path in DEFAULT_FILES.items()
    }
    event_by_year = {
        year: load_stage2_events(path)
        for year, path in DEFAULT_STAGE2_EVENTS.items()
    }
    data_by_year = {
        year: add_stage2_event_features(frame, event_by_year[year])
        for year, frame in data_by_year.items()
    }

    folds = [
        score_and_compare(2025, 2026, data_by_year, output_dir, args),
        score_and_compare(2026, 2025, data_by_year, output_dir, args),
    ]

    comparison_rows = []
    for fold in folds:
        train_year = fold["train_year"]
        valid_year = fold["valid_year"]
        top_path = output_dir / f"quality_top_valid_train{train_year}_valid{valid_year}.csv"
        if top_path.exists():
            top = pd.read_csv(top_path)
            top["train_year"] = train_year
            top["valid_year"] = valid_year
            comparison_rows.append(top)
    comparison = pd.concat(comparison_rows, ignore_index=True) if comparison_rows else pd.DataFrame()
    if not comparison.empty:
        comparison.to_csv(output_dir / "quality_cross_year_top_valid.csv", index=False)

    metadata = {
        "run_id": args.run_id,
        "model_type": "terminal_trade_quality_layer_research",
        "input_files": {str(year): str(path) for year, path in DEFAULT_FILES.items()},
        "stage2_event_files": {str(year): str(path) for year, path in DEFAULT_STAGE2_EVENTS.items()},
        "validation_scope": "2025 and 2026 terminal-stage rows only",
        "live_compatible_features_only": True,
        "excluded_as_leakage": [
            "exit_date except for realized drawdown ordering",
            "model_exit_reason",
            "baseline/model hold minutes",
            "future path features",
        ],
        "cat_features": CAT_FEATURES,
        "num_features": NUM_FEATURES,
        "base_rules": BASE_RULES,
        "root_groups": ROOT_GROUPS,
        "rank_modes": RANK_MODES,
        "folds": folds,
    }
    (output_dir / "metadata.json").write_text(
        json.dumps(metadata, indent=2, default=to_jsonable),
        encoding="utf-8",
    )
    print(json.dumps(metadata, indent=2, default=to_jsonable), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
