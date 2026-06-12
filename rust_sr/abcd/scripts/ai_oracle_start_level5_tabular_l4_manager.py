#!/usr/bin/env python3
"""Train a fast Level 5 overseer over CatBoost L4 and LightGBM L4 managers.

This is the scalable version of the L5 idea: no CNN image rendering. It builds
the same live-style candidate grid so the existing tabular L4 managers can
score each row, then trains an overseer on those two L4 opinions only.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
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


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_oracle_start_level4_specialist_manager as cat_l4
import ai_oracle_start_lightgbm_level4_manager as light_l4
import ai_oracle_start_lightgbm_utils as lgbm_utils
import ai_oracle_start_stage1_global_alltf_model as global_alltf
import ai_oracle_start_stage1_root_alltf_model as root_alltf
import ai_wave_rider_research as wave


DEFAULT_TIMEFRAMES = "1m,2m,3m,4m,5m,6m,7m,8m,9m,10m,11m,12m,13m,14m,15m,30m"
DEFAULT_CAT_L4_RUN = "aicw-oracle-start-level4-manager-v1-NQ-tr2025-v2026"
DEFAULT_LIGHT_L4_RUN = "aicw-oracle-start-lightgbm-level4-manager-v1-NQ-tr2025-v2026"

KEY_COLS = ["source_timeframe", "symbol", "direction", "signal_idx"]

CAT_FEATURES = [
    "root_symbol",
    "symbol",
    "direction",
    "source_timeframe",
    "top_l4_block",
]

NUM_FEATURES = [
    "timeframe_minutes",
    "cat_l4_score",
    "light_l4_score",
    "l4_score_mean",
    "l4_score_min",
    "l4_score_max",
    "l4_score_spread",
    "l4_score_std",
    "light_minus_cat",
    "cat_l4_above_selected",
    "light_l4_above_selected",
    "l4_votes_selected",
    "both_l4_selected",
    "any_l4_selected",
    "cat_l4_above_050",
    "light_l4_above_050",
    "l4_votes_050",
    "both_l4_above_050",
    "any_l4_above_050",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default="NQ")
    parser.add_argument("--timeframes", default=DEFAULT_TIMEFRAMES)
    parser.add_argument("--train-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--run-prefix", default="aicw-oracle-start-level5-tabular-l4-manager-v1")
    parser.add_argument("--cat-l4-run-id", default=DEFAULT_CAT_L4_RUN)
    parser.add_argument("--light-l4-run-id", default=DEFAULT_LIGHT_L4_RUN)
    parser.add_argument("--max-train-rows", type=int, default=500_000)
    parser.add_argument("--max-rows-per-timeframe", type=int, default=0, help="Debug cap only. 0 means full grid.")
    parser.add_argument("--iterations", type=int, default=550)
    parser.add_argument("--depth", type=int, default=6)
    parser.add_argument("--learning-rate", type=float, default=0.04)
    parser.add_argument("--l2-leaf-reg", type=float, default=10.0)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--cooldown-bars", type=int, default=6)
    parser.add_argument("--match-window-bars", type=int, default=4)
    parser.add_argument("--thresholds", default="0.10,0.12,0.14,0.16,0.18,0.20,0.22,0.25,0.30,0.35,0.40,0.45,0.50,0.55,0.60,0.65,0.70,0.75,0.80,0.85,0.90")
    parser.add_argument("--min-oracle-recall", type=float, default=0.90)
    parser.add_argument("--max-picks-per-oracle", type=float, default=5.0)
    parser.add_argument("--save-score-csvs", action="store_true")
    parser.add_argument("--replace-run", action="store_true")
    return parser.parse_args()


def parse_timeframes(raw: str) -> list[str]:
    values = [part.strip().lower() for part in str(raw or "").split(",") if part.strip()]
    if not values:
        raise ValueError("At least one timeframe is required.")
    return values


def run_id(args: argparse.Namespace) -> str:
    raw = f"{args.run_prefix}-{str(args.root).upper()}-tr{int(args.train_year)}-v{int(args.valid_year)}"
    if len(raw) <= 110:
        return raw
    digest = hashlib.sha1(raw.encode("utf-8")).hexdigest()[:12]
    return f"{args.run_prefix}-{digest}-v{int(args.valid_year)}"


def to_jsonable(value: Any) -> Any:
    if isinstance(value, pd.Timestamp):
        return value.isoformat()
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def load_catboost_model(path: Path) -> CatBoostClassifier:
    model = CatBoostClassifier()
    model.load_model(str(path))
    return model


def selected_threshold(run_id_value: str, fallback: float = 0.5) -> float:
    metadata_path = wave.ABCD_ROOT / "model_registry" / run_id_value / "metadata.json"
    if not metadata_path.exists():
        return float(fallback)
    metadata = load_json(metadata_path)
    if metadata.get("selected_threshold") is not None:
        return float(metadata["selected_threshold"])
    policy = metadata.get("threshold_policy") or {}
    if policy.get("selected_threshold") is not None:
        return float(policy["selected_threshold"])
    return float(fallback)


def make_base_build_args(args: argparse.Namespace) -> argparse.Namespace:
    return argparse.Namespace(
        roots=str(args.root).upper(),
        timeframes=",".join(parse_timeframes(args.timeframes)),
        train_years="2024",
        threshold_year=int(args.train_year),
        valid_year=int(args.valid_year),
        run_prefix="level5-tabular-row-build",
        oracle_run_id_template="oracle-perfect-trends-tfspec-v1-{timeframe}-2024_2026",
        iterations=650,
        depth=6,
        learning_rate=0.045,
        l2_leaf_reg=10.0,
        positive_pre_bars=0,
        positive_post_bars=2,
        negative_exclusion_bars=8,
        match_window_bars=int(args.match_window_bars),
        negative_ratio=8.0,
        base_negatives_per_symbol=200,
        max_train_rows=0,
        cooldown_bars=int(args.cooldown_bars),
        min_oracle_recall=float(args.min_oracle_recall),
        max_picks_per_oracle=float(args.max_picks_per_oracle),
        thresholds=args.thresholds,
        random_seed=int(args.random_seed),
        limit_symbols_per_root=0,
        save_row_csvs=False,
        replace_run=False,
        entry_breakout_bars=8,
        trail_lookback_bars=18,
        atr_period=14,
        atr_stop_pad=0.35,
        min_risk_ticks=12.0,
        max_risk_ticks=240.0,
        min_relative_volume=0.45,
    )


def make_cat_args(args: argparse.Namespace) -> argparse.Namespace:
    return argparse.Namespace(
        root=str(args.root).upper(),
        timeframes=",".join(parse_timeframes(args.timeframes)),
        train_year=int(args.train_year),
        valid_year=int(args.valid_year),
        level1_prefix=cat_l4.DEFAULT_LEVEL1_PREFIX,
        level2_run_id=cat_l4.DEFAULT_LEVEL2_RUN,
        level3_run_id=cat_l4.DEFAULT_LEVEL3_RUN,
        random_seed=int(args.random_seed),
        cooldown_bars=int(args.cooldown_bars),
        match_window_bars=int(args.match_window_bars),
        thresholds=args.thresholds,
        min_oracle_recall=float(args.min_oracle_recall),
        max_picks_per_oracle=float(args.max_picks_per_oracle),
    )


def make_light_args(args: argparse.Namespace) -> argparse.Namespace:
    return argparse.Namespace(
        root=str(args.root).upper(),
        timeframes=",".join(parse_timeframes(args.timeframes)),
        train_year=int(args.train_year),
        valid_year=int(args.valid_year),
        level1_prefix=light_l4.DEFAULT_LEVEL1_PREFIX,
        level2_run_id=light_l4.DEFAULT_LEVEL2_RUN,
        level3_run_id=light_l4.DEFAULT_LEVEL3_RUN,
        random_seed=int(args.random_seed),
        cooldown_bars=int(args.cooldown_bars),
        match_window_bars=int(args.match_window_bars),
        thresholds=args.thresholds,
        min_oracle_recall=float(args.min_oracle_recall),
        max_picks_per_oracle=float(args.max_picks_per_oracle),
    )


def normalize_keys(frame: pd.DataFrame) -> pd.DataFrame:
    work = frame.copy()
    work["source_timeframe"] = work["source_timeframe"].fillna("").astype(str)
    work["symbol"] = work["symbol"].fillna("").astype(str)
    work["direction"] = work["direction"].fillna("").astype(str)
    work["signal_idx"] = pd.to_numeric(work["signal_idx"], errors="coerce").fillna(-1).astype(int)
    return work


def maybe_cap_base_rows(frame: pd.DataFrame, args: argparse.Namespace, year: int) -> pd.DataFrame:
    cap = int(args.max_rows_per_timeframe)
    if cap <= 0:
        return frame
    parts: list[pd.DataFrame] = []
    for timeframe, group in frame.groupby("source_timeframe", sort=False):
        if len(group) <= cap:
            parts.append(group)
            continue
        positives = group[group["is_oracle_start"].astype(int) == 1]
        negatives = group[group["is_oracle_start"].astype(int) == 0]
        keep_pos_n = min(len(positives), max(1, cap // 2))
        keep_neg_n = min(len(negatives), cap - keep_pos_n)
        if keep_neg_n <= 0 and len(negatives) > 0 and keep_pos_n > 1:
            keep_pos_n -= 1
            keep_neg_n = 1
        keep_pos = positives.sample(n=keep_pos_n, random_state=int(args.random_seed) + int(year)) if keep_pos_n else positives
        keep_neg = negatives.sample(n=keep_neg_n, random_state=int(args.random_seed) + int(year)) if keep_neg_n else negatives
        part = pd.concat([keep_pos, keep_neg], ignore_index=True)
        print(f"{year} {timeframe}: debug-capped base rows {len(group):,} -> {len(part):,}", flush=True)
        parts.append(part)
    return pd.concat(parts, ignore_index=True).sample(frac=1.0, random_state=int(args.random_seed) + int(year)).reset_index(drop=True)


def build_base_rows(year: int, args: argparse.Namespace) -> tuple[pd.DataFrame, dict[str, Any]]:
    rng = np.random.default_rng(int(args.random_seed) + int(year))
    build_args = make_base_build_args(args)
    conn = wave.connect()
    try:
        frame, summary = global_alltf.build_year_rows(conn, int(year), build_args, rng, train_sample=False)
    finally:
        conn.close()
    frame = maybe_cap_base_rows(frame, args, int(year))
    return frame, summary


def slim_base(frame: pd.DataFrame) -> pd.DataFrame:
    wanted = [
        "candidate_uid",
        "root_symbol",
        "symbol",
        "direction",
        "source_timeframe",
        "timeframe_minutes",
        "signal_date",
        "signal_idx",
        "is_oracle_start",
        "nearest_oracle_trade_id",
        "nearest_oracle_date",
        "nearest_delta_bars",
        "nearest_abs_bars",
    ]
    keep = [col for col in wanted if col in frame.columns]
    return normalize_keys(frame[keep].copy())


def score_cat_l4(base: pd.DataFrame, args: argparse.Namespace) -> pd.DataFrame:
    model_path = wave.ABCD_ROOT / "model_registry" / args.cat_l4_run_id / "catboost_oracle_start_level4_manager.cbm"
    model = load_catboost_model(model_path)
    reports = cat_l4.score_lower_levels(base, make_cat_args(args))
    scored = cat_l4.add_manager_scores(reports, model)
    out = normalize_keys(scored[KEY_COLS + ["manager_score"]].copy())
    return out.rename(columns={"manager_score": "cat_l4_score"})


def score_light_l4(base: pd.DataFrame, args: argparse.Namespace) -> pd.DataFrame:
    model_dir = wave.ABCD_ROOT / "model_registry" / args.light_l4_run_id
    model, _cat_features, _num_features, category_maps = lgbm_utils.load_model(model_dir)
    reports = light_l4.score_lower_levels(base, make_light_args(args))
    scored = light_l4.add_manager_scores(reports, (model, category_maps))
    out = normalize_keys(scored[KEY_COLS + ["manager_score"]].copy())
    return out.rename(columns={"manager_score": "light_l4_score"})


def add_l5_features(frame: pd.DataFrame, args: argparse.Namespace) -> pd.DataFrame:
    work = frame.copy()
    work["root_symbol"] = work.get("root_symbol", str(args.root).upper())
    work["timeframe_minutes"] = pd.to_numeric(work.get("timeframe_minutes", 0.0), errors="coerce").fillna(0.0)
    for col in ["cat_l4_score", "light_l4_score"]:
        work[col] = pd.to_numeric(work[col], errors="coerce").fillna(0.0)
    scores = work[["cat_l4_score", "light_l4_score"]].to_numpy(dtype=float)
    work["l4_score_mean"] = scores.mean(axis=1)
    work["l4_score_min"] = scores.min(axis=1)
    work["l4_score_max"] = scores.max(axis=1)
    work["l4_score_spread"] = scores.max(axis=1) - scores.min(axis=1)
    work["l4_score_std"] = scores.std(axis=1)
    work["light_minus_cat"] = work["light_l4_score"] - work["cat_l4_score"]

    cat_selected = selected_threshold(args.cat_l4_run_id, 0.5)
    light_selected = selected_threshold(args.light_l4_run_id, 0.5)
    work["cat_l4_above_selected"] = (work["cat_l4_score"] >= cat_selected).astype(int)
    work["light_l4_above_selected"] = (work["light_l4_score"] >= light_selected).astype(int)
    work["l4_votes_selected"] = work[["cat_l4_above_selected", "light_l4_above_selected"]].sum(axis=1)
    work["both_l4_selected"] = (work["l4_votes_selected"] == 2).astype(int)
    work["any_l4_selected"] = (work["l4_votes_selected"] > 0).astype(int)

    work["cat_l4_above_050"] = (work["cat_l4_score"] >= 0.5).astype(int)
    work["light_l4_above_050"] = (work["light_l4_score"] >= 0.5).astype(int)
    work["l4_votes_050"] = work[["cat_l4_above_050", "light_l4_above_050"]].sum(axis=1)
    work["both_l4_above_050"] = (work["l4_votes_050"] == 2).astype(int)
    work["any_l4_above_050"] = (work["l4_votes_050"] > 0).astype(int)
    work["top_l4_block"] = np.where(work["cat_l4_score"] >= work["light_l4_score"], "catboost", "lightgbm")
    return work


def build_l4_opinion_rows(year: int, args: argparse.Namespace) -> tuple[pd.DataFrame, dict[str, Any]]:
    print(f"Building tabular Level 5 base rows year={year}", flush=True)
    base, summary = build_base_rows(int(year), args)
    merged = slim_base(base)

    print(f"{year}: scoring CatBoost L4", flush=True)
    merged = merged.merge(score_cat_l4(base, args), on=KEY_COLS, how="inner", validate="one_to_one")
    print(f"{year}: after CatBoost L4 merge rows={len(merged):,}", flush=True)

    print(f"{year}: scoring LightGBM L4", flush=True)
    merged = merged.merge(score_light_l4(base, args), on=KEY_COLS, how="inner", validate="one_to_one")
    print(f"{year}: after LightGBM L4 merge rows={len(merged):,}", flush=True)
    return add_l5_features(merged, args), summary


def cap_train_rows(frame: pd.DataFrame, args: argparse.Namespace) -> pd.DataFrame:
    max_rows = int(args.max_train_rows)
    if max_rows <= 0 or len(frame) <= max_rows:
        return frame.sample(frac=1.0, random_state=int(args.random_seed)).reset_index(drop=True)
    positives = frame[frame["is_oracle_start"].astype(int) == 1]
    negatives = frame[frame["is_oracle_start"].astype(int) == 0]
    if len(positives) >= max_rows:
        sampled = positives.sample(n=max_rows, random_state=int(args.random_seed))
    else:
        keep_neg_n = min(len(negatives), max_rows - len(positives))
        keep_neg = negatives.sample(n=keep_neg_n, random_state=int(args.random_seed)) if keep_neg_n else negatives
        sampled = pd.concat([positives, keep_neg], ignore_index=True)
    return sampled.sample(frac=1.0, random_state=int(args.random_seed)).reset_index(drop=True)


def prepare_pool(frame: pd.DataFrame, include_target: bool) -> Pool:
    work = frame.copy()
    for col in CAT_FEATURES:
        if col not in work.columns:
            work[col] = "unknown"
        work[col] = work[col].fillna("unknown").astype(str)
    for col in NUM_FEATURES:
        if col not in work.columns:
            work[col] = 0.0
        work[col] = pd.to_numeric(work[col], errors="coerce").replace([np.inf, -np.inf], np.nan).fillna(0.0)
    cols = CAT_FEATURES + NUM_FEATURES
    if include_target:
        return Pool(work[cols], label=work["is_oracle_start"].astype(int), cat_features=CAT_FEATURES)
    return Pool(work[cols], cat_features=CAT_FEATURES)


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


def add_manager_scores(frame: pd.DataFrame, model: CatBoostClassifier) -> pd.DataFrame:
    out = frame.copy()
    out["level5_score"] = model.predict_proba(prepare_pool(out, include_target=False))[:, 1]
    return out


def threshold_candidates(frame: pd.DataFrame, args: argparse.Namespace) -> list[float]:
    fixed = [float(part.strip()) for part in str(args.thresholds or "").split(",") if part.strip()]
    scores = pd.to_numeric(frame["level5_score"], errors="coerce").fillna(0.0).to_numpy()
    quantiles = [float(value) for value in np.quantile(scores, np.linspace(0.50, 0.995, 28))]
    return sorted(set(round(value, 8) for value in [*fixed, *quantiles] if 0.0 <= value <= 1.0))


def event_summary(scored: pd.DataFrame, threshold: float, args: argparse.Namespace, source_summary: dict[str, Any]) -> dict[str, Any]:
    work = scored.rename(columns={"level5_score": "oracle_start_score"})
    summary = root_alltf.event_summary(work, float(threshold), args, source_summary)
    summary["scored_rows"] = int(len(scored))
    return summary


def choose_threshold(summaries: list[dict[str, Any]], args: argparse.Namespace) -> float:
    allowed = [
        row
        for row in summaries
        if float(row["oracle_recall"]) >= float(args.min_oracle_recall)
        and float(row["picks_per_oracle"]) <= float(args.max_picks_per_oracle)
    ]
    if allowed:
        return float(max(allowed, key=lambda row: (float(row["pick_match_rate"]), -float(row["picks_per_oracle"])))["threshold"])
    return float(
        max(
            summaries,
            key=lambda row: (
                2
                * float(row["pick_match_rate"])
                * float(row["oracle_recall"])
                / max(float(row["pick_match_rate"]) + float(row["oracle_recall"]), 1e-9),
                float(row["oracle_recall"]),
            ),
        )["threshold"]
    )


def pct(value: Any) -> float:
    return round(float(value or 0.0) * 100.0, 2)


def readable_row(split: str, summary: dict[str, Any]) -> dict[str, Any]:
    return {
        "split": split,
        "threshold": summary["threshold"],
        "scored_rows": summary.get("scored_rows"),
        "picked_events": summary["picked_events"],
        "matched_picks": summary["matched_picks"],
        "trend_accuracy_pct": pct(summary["pick_match_rate"]),
        "matched_oracle_starts": summary["matched_oracle_starts"],
        "oracle_coverage_pct": pct(summary["oracle_recall"]),
        "oracle_missed_pct": round(100.0 - pct(summary["oracle_recall"]), 2),
        "picks_per_oracle": round(float(summary["picks_per_oracle"]), 3),
        "median_abs_bars": summary["median_abs_bars"],
        "mean_abs_bars": summary["mean_abs_bars"],
    }


def write_readable(output_dir: Path, rows: list[dict[str, Any]]) -> None:
    with (output_dir / "level5_tabular_l4_readable_summary.csv").open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0].keys()))
        writer.writeheader()
        writer.writerows(rows)


def write_metadata(
    output_dir: Path,
    rid: str,
    args: argparse.Namespace,
    selected_threshold_value: float,
    train_summary: dict[str, Any],
    valid_summary: dict[str, Any],
    selected_train: dict[str, Any],
    selected_valid: dict[str, Any],
    train_rows: int,
    train_sample_rows: int,
) -> None:
    metadata = {
        "run_id": rid,
        "model_type": "catboost_oracle_start_level5_tabular_l4_manager",
        "root": str(args.root).upper(),
        "timeframes": parse_timeframes(args.timeframes),
        "train_year": int(args.train_year),
        "valid_year": int(args.valid_year),
        "lower_level4_runs": {
            "catboost_l4_run_id": args.cat_l4_run_id,
            "lightgbm_l4_run_id": args.light_l4_run_id,
        },
        "lower_level4_selected_thresholds": {
            "catboost": selected_threshold(args.cat_l4_run_id, 0.5),
            "lightgbm": selected_threshold(args.light_l4_run_id, 0.5),
        },
        "event_rules": {
            "match_window_bars": int(args.match_window_bars),
            "cooldown_bars": int(args.cooldown_bars),
            "min_oracle_recall": float(args.min_oracle_recall),
            "max_picks_per_oracle": float(args.max_picks_per_oracle),
            "thresholds": args.thresholds,
        },
        "training_parameters": {
            "iterations": int(args.iterations),
            "depth": int(args.depth),
            "learning_rate": float(args.learning_rate),
            "l2_leaf_reg": float(args.l2_leaf_reg),
            "random_seed": int(args.random_seed),
            "max_train_rows": int(args.max_train_rows),
            "raw_train_rows": int(train_rows),
            "actual_train_rows": int(train_sample_rows),
        },
        "features": {"cat": CAT_FEATURES, "num": NUM_FEATURES},
        "source_summaries": {str(args.train_year): train_summary, str(args.valid_year): valid_summary},
        "selected_threshold": float(selected_threshold_value),
        "selected_train_summary": selected_train,
        "selected_valid_summary": selected_valid,
        "read": "Fast Level 5 trained only on CatBoost L4 and LightGBM L4 manager opinions.",
    }
    (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")


def main() -> int:
    args = parse_args()
    rid = run_id(args)
    output_dir = wave.ABCD_ROOT / "model_registry" / rid
    if output_dir.exists() and not args.replace_run:
        raise ValueError(f"Run already exists: {rid}. Use --replace-run.")
    output_dir.mkdir(parents=True, exist_ok=True)

    print(f"Building tabular Level 5 train rows year={args.train_year}", flush=True)
    train_rows, train_source = build_l4_opinion_rows(int(args.train_year), args)
    print(f"Building tabular Level 5 validation rows year={args.valid_year}", flush=True)
    valid_rows, valid_source = build_l4_opinion_rows(int(args.valid_year), args)

    train_sample = cap_train_rows(train_rows, args)
    print(
        f"Training tabular Level 5 rows={len(train_sample):,}/{len(train_rows):,} "
        f"positives={int(train_sample['is_oracle_start'].astype(int).sum()):,}",
        flush=True,
    )
    model = train_manager(train_sample, args)
    train_scored = add_manager_scores(train_rows, model)
    valid_scored = add_manager_scores(valid_rows, model)

    train_sweep = [event_summary(train_scored, value, args, train_source) for value in threshold_candidates(train_scored, args)]
    selected_threshold_value = choose_threshold(train_sweep, args)
    valid_sweep = [event_summary(valid_scored, float(row["threshold"]), args, valid_source) for row in train_sweep]
    selected_train = event_summary(train_scored, selected_threshold_value, args, train_source)
    selected_valid = event_summary(valid_scored, selected_threshold_value, args, valid_source)

    model.save_model(str(output_dir / "catboost_oracle_start_level5_tabular_l4_manager.cbm"))
    pd.DataFrame(train_sweep).to_csv(output_dir / f"level5_tabular_l4_threshold_sweep_{args.train_year}.csv", index=False)
    pd.DataFrame(valid_sweep).to_csv(output_dir / f"level5_tabular_l4_threshold_sweep_{args.valid_year}.csv", index=False)
    if args.save_score_csvs:
        train_scored.to_csv(output_dir / f"level5_tabular_l4_rows_{args.train_year}.csv", index=False)
        valid_scored.to_csv(output_dir / f"level5_tabular_l4_rows_{args.valid_year}.csv", index=False)

    readable = [
        readable_row(str(args.train_year), selected_train),
        readable_row(str(args.valid_year), selected_valid),
    ]
    write_readable(output_dir, readable)
    write_metadata(
        output_dir,
        rid,
        args,
        selected_threshold_value,
        train_source,
        valid_source,
        selected_train,
        selected_valid,
        len(train_rows),
        len(train_sample),
    )

    print(f"selected_level5_tabular_l4_threshold={selected_threshold_value:.6f}", flush=True)
    print(json.dumps({"train_selected": selected_train, "valid_selected": selected_valid}, indent=2, default=to_jsonable), flush=True)
    print(f"Saved tabular Level 5 manager: {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
