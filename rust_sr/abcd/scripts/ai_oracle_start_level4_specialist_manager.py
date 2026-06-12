#!/usr/bin/env python3
"""Train a Level 4 manager over Level 1, Level 2, and Level 3 specialist reports.

The manager does not read raw candles directly. It builds the same live-style
candidate rows only so the already-trained lower specialists can score them,
then trains from those specialist scores/agreement fields and oracle labels.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
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

import ai_oracle_start_stage1_global_alltf_model as global_alltf
import ai_oracle_start_stage1_live_grid_model as level1
import ai_oracle_start_stage1_root_alltf_model as level2
import ai_wave_rider_research as wave


DEFAULT_LEVEL1_PREFIX = "aicw-oracle-start-tfspec-v1"
DEFAULT_LEVEL2_RUN = "aicw-oracle-start-root-alltf-pilot1-v1-NQ-alltf-tr2024-v2026"
DEFAULT_LEVEL3_RUN = "aicw-oracle-start-global-alltf-pilot-lite-v1-NQ_CL_HO_RB_ES-alltf-tr2024-v2026"

CAT_FEATURES = [
    "root_symbol",
    "symbol",
    "direction",
    "source_timeframe",
    "l1_trend_label",
    "l1_directional_trend",
]

NUM_FEATURES = [
    "timeframe_minutes",
    "l1_score",
    "l2_score",
    "l3_score",
    "score_mean",
    "score_min",
    "score_max",
    "score_spread",
    "score_std",
    "l2_minus_l1",
    "l3_minus_l1",
    "l3_minus_l2",
    "l1_above_050",
    "l2_above_050",
    "l3_above_050",
    "levels_above_050",
    "all_levels_above_050",
    "l1_cross_tf_count",
    "l1_cross_tf_mean",
    "l1_cross_tf_max",
    "l1_cross_tf_min",
    "l1_cross_tf_spread",
    "l1_cross_tf_above_050",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default="NQ")
    parser.add_argument("--timeframes", default=level2.DEFAULT_TIMEFRAMES)
    parser.add_argument("--train-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--level1-prefix", default=DEFAULT_LEVEL1_PREFIX)
    parser.add_argument("--level2-run-id", default=DEFAULT_LEVEL2_RUN)
    parser.add_argument("--level3-run-id", default=DEFAULT_LEVEL3_RUN)
    parser.add_argument("--run-prefix", default="aicw-oracle-start-level4-manager-v1")
    parser.add_argument("--max-train-rows", type=int, default=300_000)
    parser.add_argument("--iterations", type=int, default=450)
    parser.add_argument("--depth", type=int, default=5)
    parser.add_argument("--learning-rate", type=float, default=0.045)
    parser.add_argument("--l2-leaf-reg", type=float, default=8.0)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--cooldown-bars", type=int, default=6)
    parser.add_argument("--match-window-bars", type=int, default=4)
    parser.add_argument("--thresholds", default="0.25,0.30,0.35,0.40,0.45,0.50,0.55,0.60,0.65,0.70,0.75,0.80,0.85,0.90")
    parser.add_argument("--min-oracle-recall", type=float, default=0.70)
    parser.add_argument("--max-picks-per-oracle", type=float, default=2.50)
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
    if len(raw) <= 100:
        return raw
    digest = hashlib.sha1(raw.encode("utf-8")).hexdigest()[:10]
    return f"{args.run_prefix}-{digest}-v{int(args.valid_year)}"


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def load_model(path: Path) -> CatBoostClassifier:
    model = CatBoostClassifier()
    model.load_model(str(path))
    return model


def level1_run_id(args: argparse.Namespace, timeframe: str) -> str:
    return f"{args.level1_prefix}-{timeframe}-{str(args.root).upper()}-tr2024-v2026"


def make_build_args(args: argparse.Namespace) -> argparse.Namespace:
    level3_meta = load_json(wave.ABCD_ROOT / "model_registry" / args.level3_run_id / "metadata.json")
    label_params = level3_meta.get("label_parameters") or {}
    train_params = level3_meta.get("training_parameters") or {}
    return argparse.Namespace(
        roots=str(args.root).upper(),
        timeframes=",".join(parse_timeframes(args.timeframes)),
        train_years="2024",
        threshold_year=2025,
        valid_year=int(args.valid_year),
        run_prefix="level4-row-build",
        oracle_run_id_template=level3_meta.get("oracle_run_id_template", "oracle-perfect-trends-tfspec-v1-{timeframe}-2024_2026"),
        iterations=train_params.get("iterations", 250),
        depth=train_params.get("depth", 8),
        learning_rate=train_params.get("learning_rate", 0.045),
        l2_leaf_reg=train_params.get("l2_leaf_reg", 8.0),
        positive_pre_bars=label_params.get("positive_pre_bars", 0),
        positive_post_bars=label_params.get("positive_post_bars", 2),
        negative_exclusion_bars=label_params.get("negative_exclusion_bars", 8),
        match_window_bars=int(args.match_window_bars),
        negative_ratio=train_params.get("negative_ratio", 8.0),
        base_negatives_per_symbol=train_params.get("base_negatives_per_symbol", 200),
        max_train_rows=0,
        cooldown_bars=int(args.cooldown_bars),
        min_oracle_recall=float(args.min_oracle_recall),
        max_picks_per_oracle=float(args.max_picks_per_oracle),
        thresholds=args.thresholds,
        random_seed=int(args.random_seed),
        limit_symbols_per_root=level3_meta.get("limit_symbols_per_root", 0),
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


def build_base_rows(year: int, args: argparse.Namespace) -> tuple[pd.DataFrame, dict[str, Any]]:
    rng = np.random.default_rng(int(args.random_seed) + int(year))
    build_args = make_build_args(args)
    conn = wave.connect()
    try:
        return global_alltf.build_year_rows(conn, int(year), build_args, rng, train_sample=False)
    finally:
        conn.close()


def score_lower_levels(frame: pd.DataFrame, args: argparse.Namespace) -> pd.DataFrame:
    root = str(args.root).upper()
    scored_parts: list[pd.DataFrame] = []
    for timeframe in parse_timeframes(args.timeframes):
        part = frame[frame["source_timeframe"].astype(str) == timeframe].copy()
        if part.empty:
            continue
        model_dir = wave.ABCD_ROOT / "model_registry" / level1_run_id(args, timeframe)
        model = load_model(model_dir / "catboost_oracle_start_live_grid_model.cbm")
        scored = level1.add_scores(part, model).rename(columns={"oracle_start_score": "l1_score"})
        scored_parts.append(scored)
    if not scored_parts:
        raise ValueError("No Level 1 scored rows were produced.")
    work = pd.concat(scored_parts, ignore_index=True)

    l2_model = load_model(wave.ABCD_ROOT / "model_registry" / args.level2_run_id / "catboost_oracle_start_root_alltf_model.cbm")
    work = level2.add_scores(work, l2_model).rename(columns={"oracle_start_score": "l2_score"})

    l3_model = load_model(wave.ABCD_ROOT / "model_registry" / args.level3_run_id / "catboost_oracle_start_global_alltf_model.cbm")
    work = level2.add_scores(work, l3_model).rename(columns={"oracle_start_score": "l3_score"})

    work["root_symbol"] = root
    work["l1_trend_label"] = work.get("trend_label", "unknown")
    work["l1_directional_trend"] = work.get("directional_trend", "unknown")
    for col in ["l1_score", "l2_score", "l3_score"]:
        work[col] = pd.to_numeric(work[col], errors="coerce").fillna(0.0)
    scores = work[["l1_score", "l2_score", "l3_score"]].to_numpy(dtype=float)
    work["score_mean"] = scores.mean(axis=1)
    work["score_min"] = scores.min(axis=1)
    work["score_max"] = scores.max(axis=1)
    work["score_spread"] = scores.max(axis=1) - scores.min(axis=1)
    work["score_std"] = scores.std(axis=1)
    work["l2_minus_l1"] = work["l2_score"] - work["l1_score"]
    work["l3_minus_l1"] = work["l3_score"] - work["l1_score"]
    work["l3_minus_l2"] = work["l3_score"] - work["l2_score"]
    for level in ["l1", "l2", "l3"]:
        work[f"{level}_above_050"] = (work[f"{level}_score"] >= 0.5).astype(int)
    work["levels_above_050"] = work[["l1_above_050", "l2_above_050", "l3_above_050"]].sum(axis=1)
    work["all_levels_above_050"] = (work["levels_above_050"] == 3).astype(int)

    grouped = work.groupby(["symbol", "direction", "signal_date"])["l1_score"].agg(["count", "mean", "max", "min"]).reset_index()
    grouped = grouped.rename(
        columns={
            "count": "l1_cross_tf_count",
            "mean": "l1_cross_tf_mean",
            "max": "l1_cross_tf_max",
            "min": "l1_cross_tf_min",
        }
    )
    grouped["l1_cross_tf_spread"] = grouped["l1_cross_tf_max"] - grouped["l1_cross_tf_min"]
    above = (
        work.assign(l1_above_tmp=(work["l1_score"] >= 0.5).astype(int))
        .groupby(["symbol", "direction", "signal_date"])["l1_above_tmp"]
        .sum()
        .reset_index()
        .rename(columns={"l1_above_tmp": "l1_cross_tf_above_050"})
    )
    grouped = grouped.merge(above, on=["symbol", "direction", "signal_date"], how="left")
    work = work.merge(grouped, on=["symbol", "direction", "signal_date"], how="left")
    return work


def cap_train_rows(frame: pd.DataFrame, args: argparse.Namespace) -> pd.DataFrame:
    max_rows = int(args.max_train_rows)
    if max_rows <= 0 or len(frame) <= max_rows:
        return frame
    positives = frame[frame["is_oracle_start"] == 1]
    negatives = frame[frame["is_oracle_start"] == 0]
    keep_pos_n = min(len(positives), max_rows // 2)
    keep_neg_n = min(len(negatives), max_rows - keep_pos_n)
    keep_pos = positives.sample(n=keep_pos_n, random_state=int(args.random_seed)) if keep_pos_n else positives
    keep_neg = negatives.sample(n=keep_neg_n, random_state=int(args.random_seed)) if keep_neg_n else negatives
    return pd.concat([keep_pos, keep_neg], ignore_index=True).sample(frac=1.0, random_state=int(args.random_seed)).reset_index(drop=True)


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
    scored = frame.copy()
    scored["manager_score"] = model.predict_proba(prepare_pool(scored, include_target=False))[:, 1]
    return scored


def event_summary(frame: pd.DataFrame, threshold: float, args: argparse.Namespace, source_summary: dict[str, Any]) -> dict[str, Any]:
    work = frame.rename(columns={"manager_score": "oracle_start_score"})
    return level2.event_summary(work, float(threshold), args, source_summary)


def threshold_candidates(frame: pd.DataFrame, args: argparse.Namespace) -> list[float]:
    fixed = [float(part.strip()) for part in str(args.thresholds or "").split(",") if part.strip()]
    scores = pd.to_numeric(frame["manager_score"], errors="coerce").fillna(0.0).to_numpy()
    quantiles = [float(value) for value in np.quantile(scores, np.linspace(0.55, 0.99, 18))]
    return sorted(set(round(value, 8) for value in [*fixed, *quantiles] if 0.0 <= value <= 1.0))


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
    return round(float(value) * 100.0, 2)


def write_readable_report(output_dir: Path, rows: list[dict[str, Any]]) -> None:
    path = output_dir / "level4_readable_summary.csv"
    fields = [
        "split",
        "threshold",
        "picked_events",
        "matched_picks",
        "trend_accuracy_pct",
        "matched_oracle_starts",
        "oracle_missed_pct",
        "picks_per_oracle",
        "median_abs_bars",
        "mean_abs_bars",
    ]
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)


def main() -> int:
    args = parse_args()
    rid = run_id(args)
    output_dir = wave.ABCD_ROOT / "model_registry" / rid
    if output_dir.exists() and not args.replace_run:
        raise ValueError(f"Run already exists: {rid}. Use --replace-run.")
    output_dir.mkdir(parents=True, exist_ok=True)

    print(f"Building Level 4 train reports from lower levels year={args.train_year}", flush=True)
    train_base, train_summary = build_base_rows(int(args.train_year), args)
    train_reports = score_lower_levels(train_base, args)
    train = cap_train_rows(train_reports, args)
    print(
        f"Training Level 4 manager rows={len(train):,} positives={int(train['is_oracle_start'].sum()):,}",
        flush=True,
    )
    model = train_manager(train, args)
    train_scored = add_manager_scores(train_reports, model)
    train_sweep = [event_summary(train_scored, value, args, train_summary) for value in threshold_candidates(train_scored, args)]
    selected_threshold = choose_threshold(train_sweep, args)

    print(f"Building Level 4 validation reports from lower levels year={args.valid_year}", flush=True)
    valid_base, valid_summary = build_base_rows(int(args.valid_year), args)
    valid_reports = score_lower_levels(valid_base, args)
    valid_scored = add_manager_scores(valid_reports, model)
    valid_sweep = [event_summary(valid_scored, float(row["threshold"]), args, valid_summary) for row in train_sweep]
    selected_train = event_summary(train_scored, selected_threshold, args, train_summary)
    selected_valid = event_summary(valid_scored, selected_threshold, args, valid_summary)

    model.save_model(str(output_dir / "catboost_oracle_start_level4_manager.cbm"))
    pd.DataFrame(train_sweep).to_csv(output_dir / f"level4_threshold_sweep_{args.train_year}.csv", index=False)
    pd.DataFrame(valid_sweep).to_csv(output_dir / f"level4_threshold_sweep_{args.valid_year}.csv", index=False)
    if args.save_score_csvs:
        train_scored.to_csv(output_dir / f"level4_lower_reports_scored_{args.train_year}.csv", index=False)
        valid_scored.to_csv(output_dir / f"level4_lower_reports_scored_{args.valid_year}.csv", index=False)

    readable_rows = []
    for split, summary in [(str(args.train_year), selected_train), (str(args.valid_year), selected_valid)]:
        readable_rows.append(
            {
                "split": split,
                "threshold": summary["threshold"],
                "picked_events": summary["picked_events"],
                "matched_picks": summary["matched_picks"],
                "trend_accuracy_pct": pct(summary["pick_match_rate"]),
                "matched_oracle_starts": summary["matched_oracle_starts"],
                "oracle_missed_pct": round(100.0 - pct(summary["oracle_recall"]), 2),
                "picks_per_oracle": round(float(summary["picks_per_oracle"]), 3),
                "median_abs_bars": summary["median_abs_bars"],
                "mean_abs_bars": summary["mean_abs_bars"],
            }
        )
    write_readable_report(output_dir, readable_rows)

    metadata = {
        "run_id": rid,
        "model_type": "catboost_oracle_start_level4_specialist_manager",
        "root": str(args.root).upper(),
        "timeframes": parse_timeframes(args.timeframes),
        "lower_level_runs": {
            "level1_prefix": args.level1_prefix,
            "level2_run_id": args.level2_run_id,
            "level3_run_id": args.level3_run_id,
        },
        "train_year": int(args.train_year),
        "valid_year": int(args.valid_year),
        "training_parameters": {
            "max_train_rows": int(args.max_train_rows),
            "iterations": int(args.iterations),
            "depth": int(args.depth),
            "learning_rate": float(args.learning_rate),
            "l2_leaf_reg": float(args.l2_leaf_reg),
            "random_seed": int(args.random_seed),
        },
        "threshold_policy": {
            "selected_threshold": float(selected_threshold),
            "min_oracle_recall": float(args.min_oracle_recall),
            "max_picks_per_oracle": float(args.max_picks_per_oracle),
            "cooldown_bars": int(args.cooldown_bars),
        },
        "features": {"cat": CAT_FEATURES, "num": NUM_FEATURES},
        "source_summaries": {str(args.train_year): train_summary, str(args.valid_year): valid_summary},
        "selected_train_summary": selected_train,
        "selected_valid_summary": selected_valid,
        "read": "Level 4 manager trained on lower-level specialist reports from 2025 and tested on 2026.",
    }
    (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=level2.to_jsonable), encoding="utf-8")

    print(f"selected_level4_threshold={selected_threshold:.6f}", flush=True)
    print(json.dumps({"train_selected": selected_train, "valid_selected": selected_valid}, indent=2, default=level2.to_jsonable), flush=True)
    print(f"Saved Level 4 manager: {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
