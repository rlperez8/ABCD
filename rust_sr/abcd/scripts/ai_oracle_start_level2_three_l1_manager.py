#!/usr/bin/env python3
"""Train the simplified Level 2 manager over Cat/Light/XGB Level 1 opinions.

New mainline design:

Level 1:
    CatBoost, LightGBM, and XGBoost each scan all roots on one trading
    timeframe, currently 2m.

Level 2:
    A CatBoost manager sees only those three Level 1 opinions plus simple
    agreement/spread fields. It does not use the old all-timeframe L2/L3/L4
    ladder.
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

import ai_candle_wave_scanner_model as scanner
import ai_oracle_start_lightgbm_stage1_live_grid_model as light_stage1
import ai_oracle_start_lightgbm_utils as lgbm_utils
import ai_oracle_start_stage1_live_grid_model as cat_stage1
import ai_oracle_start_xgboost_stage1_live_grid_model as xgb_stage1
import ai_oracle_start_xgboost_utils as xgb_utils
import ai_wave_rider_research as wave


DEFAULT_ORACLE_RUN = "oracle-perfect-trends-tfspec-v1-2m-2024_2026"
DEFAULT_CAT_L1_PREFIX = "aicw-os-cat-l1-2m-v1"
DEFAULT_LIGHT_L1_PREFIX = "aicw-os-light-l1-2m-v1"
DEFAULT_XGB_L1_PREFIX = "aicw-os-xgb-l1-2m-v1"

KEY_COLS = ["symbol", "direction", "signal_idx"]
ENGINE_ALIASES = {
    "cat": "catboost",
    "catboost": "catboost",
    "light": "lightgbm",
    "lightgbm": "lightgbm",
    "xgb": "xgboost",
    "xgboost": "xgboost",
}
ENGINE_COL_PREFIX = {
    "catboost": "cat",
    "lightgbm": "light",
    "xgboost": "xgb",
}
DEFAULT_MANAGER_ENGINES = ["catboost", "lightgbm", "xgboost"]

CAT_FEATURES = [
    "root_symbol",
    "direction",
    "source_timeframe",
    "top_l1_block",
]

NUM_FEATURES = [
    "timeframe_minutes",
    "cat_l1_score",
    "light_l1_score",
    "xgb_l1_score",
    "l1_score_mean",
    "l1_score_min",
    "l1_score_max",
    "l1_score_spread",
    "l1_score_std",
    "light_minus_cat",
    "xgb_minus_cat",
    "xgb_minus_light",
    "cat_l1_above_selected",
    "light_l1_above_selected",
    "xgb_l1_above_selected",
    "l1_votes_selected",
    "all_l1_selected",
    "two_or_more_l1_selected",
    "any_l1_selected",
    "cat_l1_above_050",
    "light_l1_above_050",
    "xgb_l1_above_050",
    "l1_votes_050",
    "all_l1_above_050",
    "two_or_more_l1_050",
    "any_l1_above_050",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--timeframe", default="2m", choices=sorted(scanner.TIMEFRAME_TABLES))
    parser.add_argument("--roots", default="", help="Comma-separated roots. Empty means all roots.")
    parser.add_argument("--oracle-run-id", default=DEFAULT_ORACLE_RUN)
    parser.add_argument("--level1-train-years", default="2024")
    parser.add_argument("--level1-valid-year", type=int, default=2026)
    parser.add_argument("--train-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--cat-l1-prefix", default=DEFAULT_CAT_L1_PREFIX)
    parser.add_argument("--light-l1-prefix", default=DEFAULT_LIGHT_L1_PREFIX)
    parser.add_argument("--xgb-l1-prefix", default=DEFAULT_XGB_L1_PREFIX)
    parser.add_argument("--cat-l1-run-id", default="")
    parser.add_argument("--light-l1-run-id", default="")
    parser.add_argument("--xgb-l1-run-id", default="")
    parser.add_argument(
        "--manager-engines",
        default="cat,light,xgb",
        help="Comma-separated Level 1 engines the Level 2 manager may use: cat,light,xgb.",
    )
    parser.add_argument("--run-prefix", default="aicw-oracle-start-level2-three-l1-manager-v1")
    parser.add_argument("--max-train-rows", type=int, default=500_000)
    parser.add_argument("--max-rows-per-year", type=int, default=0, help="Debug cap after row build/score. 0 means full year.")
    parser.add_argument("--limit-symbols", type=int, default=0)
    parser.add_argument("--iterations", type=int, default=550)
    parser.add_argument("--depth", type=int, default=6)
    parser.add_argument("--learning-rate", type=float, default=0.04)
    parser.add_argument("--l2-leaf-reg", type=float, default=10.0)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--cooldown-bars", type=int, default=6)
    parser.add_argument("--match-window-bars", type=int, default=4)
    parser.add_argument("--thresholds", default="0.10,0.12,0.14,0.16,0.18,0.20,0.22,0.25,0.30,0.35,0.40,0.45,0.50,0.55,0.60,0.65,0.70,0.75,0.80,0.85,0.90")
    parser.add_argument("--threshold-quantile-count", type=int, default=28)
    parser.add_argument("--min-oracle-recall", type=float, default=0.80)
    parser.add_argument("--max-picks-per-oracle", type=float, default=3.0)
    parser.add_argument("--save-score-csvs", action="store_true")
    parser.add_argument("--candidate-set-id", default="")
    parser.add_argument("--candidate-cache-dir", default="")
    parser.add_argument("--read-candidate-cache", action="store_true")
    parser.add_argument("--write-candidate-cache", action="store_true")
    parser.add_argument("--replace-candidate-cache", action="store_true")
    parser.add_argument("--opinion-cache-dir", default="")
    parser.add_argument("--train-opinion-cache", default="")
    parser.add_argument("--valid-opinion-cache", default="")
    parser.add_argument("--read-opinion-cache", action="store_true")
    parser.add_argument("--write-opinion-cache", action="store_true")
    parser.add_argument("--only-build-opinion-cache", action="store_true")
    parser.add_argument("--replace-run", action="store_true")

    parser.add_argument("--positive-pre-bars", type=int, default=0)
    parser.add_argument("--positive-post-bars", type=int, default=2)
    parser.add_argument("--negative-exclusion-bars", type=int, default=8)
    parser.add_argument("--negative-ratio", type=float, default=8.0)
    parser.add_argument("--base-negatives-per-symbol", type=int, default=200)
    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=240.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    return parser.parse_args()


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


def parse_roots(raw: str) -> list[str]:
    return cat_stage1.parse_list(raw)


def root_label(raw: str) -> str:
    roots = parse_roots(raw)
    if not roots:
        return "ALL"
    joined = "_".join(roots)
    if len(joined) <= 48:
        return joined
    return hashlib.sha1(joined.encode("utf-8")).hexdigest()[:12]


def manager_engines(args: argparse.Namespace) -> list[str]:
    raw = str(getattr(args, "manager_engines", "") or "")
    values: list[str] = []
    for part in raw.split(","):
        key = part.strip().lower()
        if not key:
            continue
        if key not in ENGINE_ALIASES:
            raise ValueError(f"Unknown manager engine '{part}'. Use cat, light, or xgb.")
        engine = ENGINE_ALIASES[key]
        if engine not in values:
            values.append(engine)
    if not values:
        values = list(DEFAULT_MANAGER_ENGINES)
    if len(values) < 1:
        raise ValueError("At least one manager engine is required.")
    return values


def manager_engine_label(args: argparse.Namespace) -> str:
    return "_".join(ENGINE_COL_PREFIX[engine] for engine in manager_engines(args))


def run_id(args: argparse.Namespace) -> str:
    engine_label = manager_engine_label(args)
    engine_part = "" if manager_engines(args) == DEFAULT_MANAGER_ENGINES else f"-{engine_label}"
    raw = (
        f"{args.run_prefix}-{root_label(args.roots)}-{args.timeframe}"
        f"{engine_part}-tr{int(args.train_year)}-v{int(args.valid_year)}"
    )
    if len(raw) <= 110:
        return raw
    digest = hashlib.sha1(raw.encode("utf-8")).hexdigest()[:12]
    return f"{args.run_prefix}-{digest}-{args.timeframe}-v{int(args.valid_year)}"


def default_l1_run_id(prefix: str, args: argparse.Namespace) -> str:
    child = argparse.Namespace(
        run_prefix=prefix,
        timeframe=args.timeframe,
        roots=args.roots,
        train_years=args.level1_train_years,
        valid_year=int(args.level1_valid_year),
    )
    return cat_stage1.run_id(child)


def l1_run_ids(args: argparse.Namespace) -> dict[str, str]:
    return {
        "catboost": args.cat_l1_run_id or default_l1_run_id(args.cat_l1_prefix, args),
        "lightgbm": args.light_l1_run_id or default_l1_run_id(args.light_l1_prefix, args),
        "xgboost": args.xgb_l1_run_id or default_l1_run_id(args.xgb_l1_prefix, args),
    }


def active_l1_run_ids(args: argparse.Namespace) -> dict[str, str]:
    ids = l1_run_ids(args)
    return {engine: ids[engine] for engine in manager_engines(args)}


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
    selected = metadata.get("selected_valid_summary") or {}
    if selected.get("threshold") is not None:
        return float(selected["threshold"])
    return float(fallback)


def engine_score_col(engine: str) -> str:
    return f"{ENGINE_COL_PREFIX[engine]}_l1_score"


def engine_selected_col(engine: str) -> str:
    return f"{ENGINE_COL_PREFIX[engine]}_l1_above_selected"


def engine_050_col(engine: str) -> str:
    return f"{ENGINE_COL_PREFIX[engine]}_l1_above_050"


def feature_columns(args: argparse.Namespace) -> tuple[list[str], list[str]]:
    engines = manager_engines(args)
    num_features = ["timeframe_minutes"]
    num_features.extend(engine_score_col(engine) for engine in engines)
    num_features.extend(
        [
            "l1_score_mean",
            "l1_score_min",
            "l1_score_max",
            "l1_score_spread",
            "l1_score_std",
        ]
    )
    pair_features = {
        frozenset(("catboost", "lightgbm")): "light_minus_cat",
        frozenset(("catboost", "xgboost")): "xgb_minus_cat",
        frozenset(("lightgbm", "xgboost")): "xgb_minus_light",
    }
    for pair, col in pair_features.items():
        if pair.issubset(set(engines)):
            num_features.append(col)
    num_features.extend(engine_selected_col(engine) for engine in engines)
    num_features.extend(
        [
            "l1_votes_selected",
            "all_l1_selected",
            "two_or_more_l1_selected",
            "any_l1_selected",
        ]
    )
    num_features.extend(engine_050_col(engine) for engine in engines)
    num_features.extend(
        [
            "l1_votes_050",
            "all_l1_above_050",
            "two_or_more_l1_050",
            "any_l1_above_050",
        ]
    )
    return list(CAT_FEATURES), num_features


def output_stem(args: argparse.Namespace) -> str:
    if manager_engines(args) == DEFAULT_MANAGER_ENGINES:
        return "level2_three_l1"
    return f"level2_{manager_engine_label(args)}_l1"


def build_args_for_year(args: argparse.Namespace) -> argparse.Namespace:
    return argparse.Namespace(
        roots=args.roots,
        symbols="",
        timeframe=args.timeframe,
        oracle_run_id=args.oracle_run_id,
        positive_pre_bars=int(args.positive_pre_bars),
        positive_post_bars=int(args.positive_post_bars),
        negative_exclusion_bars=int(args.negative_exclusion_bars),
        match_window_bars=int(args.match_window_bars),
        negative_ratio=float(args.negative_ratio),
        base_negatives_per_symbol=int(args.base_negatives_per_symbol),
        max_train_rows=0,
        random_seed=int(args.random_seed),
        limit_symbols=int(args.limit_symbols),
        entry_breakout_bars=int(args.entry_breakout_bars),
        trail_lookback_bars=int(args.trail_lookback_bars),
        atr_period=int(args.atr_period),
        atr_stop_pad=float(args.atr_stop_pad),
        min_risk_ticks=float(args.min_risk_ticks),
        max_risk_ticks=float(args.max_risk_ticks),
        min_relative_volume=float(args.min_relative_volume),
        candidate_set_id=str(args.candidate_set_id or ""),
        candidate_cache_dir=str(args.candidate_cache_dir or ""),
        read_candidate_cache=bool(args.read_candidate_cache),
        write_candidate_cache=bool(args.write_candidate_cache),
        only_build_candidate_cache=False,
        replace_candidate_cache=bool(args.replace_candidate_cache),
    )


def add_timeframe_columns(frame: pd.DataFrame, timeframe: str) -> pd.DataFrame:
    _, minutes = scanner.table_for_timeframe(timeframe)
    work = frame.copy()
    work["source_timeframe"] = str(timeframe)
    work["timeframe_minutes"] = float(minutes)
    return work


def cap_rows(frame: pd.DataFrame, max_rows: int, random_seed: int) -> pd.DataFrame:
    if int(max_rows) <= 0 or len(frame) <= int(max_rows):
        return frame
    positives = frame[pd.to_numeric(frame["is_oracle_start"], errors="coerce").fillna(0).astype(int) == 1]
    negatives = frame[pd.to_numeric(frame["is_oracle_start"], errors="coerce").fillna(0).astype(int) == 0]
    if positives.empty or negatives.empty:
        return frame.sample(n=int(max_rows), random_state=int(random_seed)).reset_index(drop=True)
    keep_pos_n = min(len(positives), max(1, int(max_rows) // 2))
    keep_neg_n = min(len(negatives), int(max_rows) - keep_pos_n)
    if keep_neg_n <= 0:
        keep_neg_n = 1
        keep_pos_n = max(1, int(max_rows) - keep_neg_n)
    keep_pos = positives.sample(n=keep_pos_n, random_state=int(random_seed)) if len(positives) > keep_pos_n else positives
    keep_neg = negatives.sample(n=keep_neg_n, random_state=int(random_seed)) if len(negatives) > keep_neg_n else negatives
    return pd.concat([keep_pos, keep_neg], ignore_index=True).sample(
        frac=1.0,
        random_state=int(random_seed),
    ).reset_index(drop=True)


def build_base_rows(year: int, args: argparse.Namespace) -> tuple[pd.DataFrame, dict[str, Any]]:
    rng = np.random.default_rng(int(args.random_seed) + int(year))
    conn = wave.connect()
    try:
        frame, summary = cat_stage1.load_or_build_year_rows(
            conn,
            int(year),
            build_args_for_year(args),
            rng,
            train_sample=False,
        )
    finally:
        conn.close()
    return add_timeframe_columns(frame, args.timeframe), summary


def normalize_keys(frame: pd.DataFrame) -> pd.DataFrame:
    work = frame.copy()
    work["symbol"] = work["symbol"].fillna("").astype(str)
    work["direction"] = work["direction"].fillna("").astype(str)
    work["signal_idx"] = pd.to_numeric(work["signal_idx"], errors="coerce").fillna(-1).astype(int)
    return work


def load_cat_model(run_id_value: str) -> CatBoostClassifier:
    model = CatBoostClassifier()
    model.load_model(str(wave.ABCD_ROOT / "model_registry" / run_id_value / "catboost_oracle_start_live_grid_model.cbm"))
    return model


def score_cat_l1(base: pd.DataFrame, run_id_value: str) -> pd.DataFrame:
    model = load_cat_model(run_id_value)
    scored = cat_stage1.add_scores(base, model).rename(columns={"oracle_start_score": "cat_l1_score"})
    return normalize_keys(scored[KEY_COLS + ["cat_l1_score"]].copy())


def score_light_l1(base: pd.DataFrame, run_id_value: str) -> pd.DataFrame:
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id_value
    model, _cat_features, _num_features, category_maps = lgbm_utils.load_model(model_dir)
    scored = light_stage1.add_scores(base, (model, category_maps)).rename(columns={"oracle_start_score": "light_l1_score"})
    return normalize_keys(scored[KEY_COLS + ["light_l1_score"]].copy())


def score_xgb_l1(base: pd.DataFrame, run_id_value: str) -> pd.DataFrame:
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id_value
    model, _cat_features, _num_features, category_maps = xgb_utils.load_model(model_dir)
    scored = xgb_stage1.add_scores(base, (model, category_maps)).rename(columns={"oracle_start_score": "xgb_l1_score"})
    return normalize_keys(scored[KEY_COLS + ["xgb_l1_score"]].copy())


def add_l2_features(frame: pd.DataFrame, args: argparse.Namespace) -> pd.DataFrame:
    work = frame.copy()
    ids = l1_run_ids(args)
    engines = manager_engines(args)
    score_cols = [engine_score_col(engine) for engine in engines]
    for col in ["cat_l1_score", "light_l1_score", "xgb_l1_score"]:
        work[col] = pd.to_numeric(work[col], errors="coerce").fillna(0.0)
    scores = work[score_cols].to_numpy(dtype=float)
    work["l1_score_mean"] = scores.mean(axis=1)
    work["l1_score_min"] = scores.min(axis=1)
    work["l1_score_max"] = scores.max(axis=1)
    work["l1_score_spread"] = scores.max(axis=1) - scores.min(axis=1)
    work["l1_score_std"] = scores.std(axis=1)
    work["light_minus_cat"] = work["light_l1_score"] - work["cat_l1_score"]
    work["xgb_minus_cat"] = work["xgb_l1_score"] - work["cat_l1_score"]
    work["xgb_minus_light"] = work["xgb_l1_score"] - work["light_l1_score"]

    for engine in DEFAULT_MANAGER_ENGINES:
        selected = selected_threshold(ids[engine], 0.5)
        prefix = ENGINE_COL_PREFIX[engine]
        work[f"{prefix}_l1_above_selected"] = (work[f"{prefix}_l1_score"] >= selected).astype(int)
    selected_cols = [engine_selected_col(engine) for engine in engines]
    work["l1_votes_selected"] = work[selected_cols].sum(axis=1)
    work["all_l1_selected"] = (work["l1_votes_selected"] == len(engines)).astype(int)
    work["two_or_more_l1_selected"] = (work["l1_votes_selected"] >= min(2, len(engines))).astype(int)
    work["any_l1_selected"] = (work["l1_votes_selected"] > 0).astype(int)

    work["cat_l1_above_050"] = (work["cat_l1_score"] >= 0.5).astype(int)
    work["light_l1_above_050"] = (work["light_l1_score"] >= 0.5).astype(int)
    work["xgb_l1_above_050"] = (work["xgb_l1_score"] >= 0.5).astype(int)
    above_050_cols = [engine_050_col(engine) for engine in engines]
    work["l1_votes_050"] = work[above_050_cols].sum(axis=1)
    work["all_l1_above_050"] = (work["l1_votes_050"] == len(engines)).astype(int)
    work["two_or_more_l1_050"] = (work["l1_votes_050"] >= min(2, len(engines))).astype(int)
    work["any_l1_above_050"] = (work["l1_votes_050"] > 0).astype(int)
    top_indices = np.argmax(scores, axis=1)
    work["top_l1_block"] = np.take(np.asarray(engines, dtype=object), top_indices)
    return work


def build_l1_opinion_rows(year: int, args: argparse.Namespace) -> tuple[pd.DataFrame, dict[str, Any]]:
    ids = l1_run_ids(args)
    print(f"{year}: building Level 2 base rows {root_label(args.roots)} {args.timeframe}", flush=True)
    base, summary = build_base_rows(int(year), args)
    if int(args.max_rows_per_year) > 0:
        before = len(base)
        base = cap_rows(base, int(args.max_rows_per_year), int(args.random_seed) + int(year))
        summary = source_summary_for_sample(summary, base)
        print(f"{year}: debug-capped base rows {before:,} -> {len(base):,}", flush=True)

    merged = normalize_keys(base.copy())
    print(f"{year}: scoring CatBoost L1 {ids['catboost']}", flush=True)
    merged = merged.merge(score_cat_l1(base, ids["catboost"]), on=KEY_COLS, how="inner", validate="one_to_one")
    print(f"{year}: scoring LightGBM L1 {ids['lightgbm']}", flush=True)
    merged = merged.merge(score_light_l1(base, ids["lightgbm"]), on=KEY_COLS, how="inner", validate="one_to_one")
    print(f"{year}: scoring XGBoost L1 {ids['xgboost']}", flush=True)
    merged = merged.merge(score_xgb_l1(base, ids["xgboost"]), on=KEY_COLS, how="inner", validate="one_to_one")
    return add_l2_features(merged, args), summary


def default_opinion_cache_dir(args: argparse.Namespace) -> Path:
    if str(args.opinion_cache_dir or "").strip():
        return Path(args.opinion_cache_dir).expanduser().resolve()
    return wave.ABCD_ROOT / "model_registry" / "_level2_l1_opinion_cache"


def opinion_cache_digest(args: argparse.Namespace, year: int) -> str:
    payload = {
        "cache_version": 1,
        "year": int(year),
        "roots": parse_roots(args.roots),
        "roots_empty_means_all": True,
        "timeframe": args.timeframe,
        "oracle_run_id": args.oracle_run_id,
        "l1_runs": l1_run_ids(args),
        "max_rows_per_year": int(args.max_rows_per_year),
        "limit_symbols": int(args.limit_symbols),
        "match_window_bars": int(args.match_window_bars),
        "cooldown_bars": int(args.cooldown_bars),
        "random_seed": int(args.random_seed),
    }
    return hashlib.sha1(json.dumps(payload, sort_keys=True).encode("utf-8")).hexdigest()[:16]


def default_opinion_cache_path(args: argparse.Namespace, year: int) -> Path:
    return default_opinion_cache_dir(args) / f"l1_opinions_{root_label(args.roots)}_{args.timeframe}_{int(year)}_{opinion_cache_digest(args, int(year))}.pkl.gz"


def opinion_cache_path_for_year(args: argparse.Namespace, year: int) -> Path:
    explicit = args.train_opinion_cache if int(year) == int(args.train_year) else args.valid_opinion_cache
    if str(explicit or "").strip():
        return Path(explicit).expanduser().resolve()
    return default_opinion_cache_path(args, int(year))


def write_opinion_cache(frame: pd.DataFrame, summary: dict[str, Any], args: argparse.Namespace, year: int) -> Path:
    path = opinion_cache_path_for_year(args, int(year))
    path.parent.mkdir(parents=True, exist_ok=True)
    frame.to_pickle(path, compression="gzip")
    cat_features, num_features = feature_columns(args)
    metadata = {
        "cache_version": 1,
        "cache_file": str(path),
        "year": int(year),
        "roots": parse_roots(args.roots),
        "roots_empty_means_all": True,
        "timeframe": args.timeframe,
        "row_count": int(len(frame)),
        "positive_rows": int(pd.to_numeric(frame["is_oracle_start"], errors="coerce").fillna(0).astype(int).sum()),
        "source_summary": summary,
        "level1_runs": l1_run_ids(args),
        "level1_selected_thresholds": {
            key: selected_threshold(value, 0.5) for key, value in l1_run_ids(args).items()
        },
        "features": {"cat": cat_features, "num": num_features},
    }
    path.with_suffix(path.suffix + ".metadata.json").write_text(
        json.dumps(metadata, indent=2, default=to_jsonable),
        encoding="utf-8",
    )
    print(f"Wrote Level 1 opinion cache year={year} rows={len(frame):,}: {path}", flush=True)
    return path


def load_opinion_cache(args: argparse.Namespace, year: int) -> tuple[pd.DataFrame, dict[str, Any]]:
    path = opinion_cache_path_for_year(args, int(year))
    if not path.exists():
        raise FileNotFoundError(f"Level 1 opinion cache not found: {path}")
    frame = pd.read_pickle(path, compression="gzip")
    metadata_path = path.with_suffix(path.suffix + ".metadata.json")
    if metadata_path.exists():
        metadata = load_json(metadata_path)
        summary = dict(metadata.get("source_summary") or {})
    else:
        summary = source_summary_for_sample({}, frame)
    if int(args.max_rows_per_year) > 0 and len(frame) > int(args.max_rows_per_year):
        before = len(frame)
        frame = cap_rows(frame, int(args.max_rows_per_year), int(args.random_seed) + int(year))
        summary = source_summary_for_sample(summary, frame)
        print(f"{year}: debug-capped opinion cache rows {before:,} -> {len(frame):,}", flush=True)
    print(f"Loaded Level 1 opinion cache year={year} rows={len(frame):,}: {path}", flush=True)
    return frame, summary


def source_summary_for_sample(source_summary: dict[str, Any], frame: pd.DataFrame) -> dict[str, Any]:
    summary = dict(source_summary or {})
    positives = frame[pd.to_numeric(frame["is_oracle_start"], errors="coerce").fillna(0).astype(int) == 1]
    represented = (
        positives.get("nearest_oracle_trade_id", pd.Series(dtype=object))
        .dropna()
        .astype(str)
        .replace("", np.nan)
        .dropna()
        .nunique()
    )
    summary["oracle_starts"] = int(represented)
    summary["oracle_starts_available"] = int(represented)
    summary["eligible_candidates"] = int(len(frame))
    summary["materialized_rows"] = int(len(frame))
    summary["materialized_positives"] = int(len(positives))
    summary["sampled_for_debug"] = True
    return summary


def prepare_pool(frame: pd.DataFrame, include_target: bool, args: argparse.Namespace) -> Pool:
    work = frame.copy()
    cat_features, num_features = feature_columns(args)
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


def cap_train_rows(frame: pd.DataFrame, args: argparse.Namespace) -> pd.DataFrame:
    return cap_rows(frame, int(args.max_train_rows), int(args.random_seed)).sample(
        frac=1.0,
        random_state=int(args.random_seed),
    ).reset_index(drop=True)


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
    model.fit(prepare_pool(train, include_target=True, args=args))
    return model


def add_manager_scores(frame: pd.DataFrame, model: CatBoostClassifier, args: argparse.Namespace) -> pd.DataFrame:
    out = frame.copy()
    out["level2_score"] = model.predict_proba(prepare_pool(out, include_target=False, args=args))[:, 1]
    return out


def threshold_candidates(frame: pd.DataFrame, args: argparse.Namespace) -> list[float]:
    fixed = [float(part.strip()) for part in str(args.thresholds or "").split(",") if part.strip()]
    scores = pd.to_numeric(frame["level2_score"], errors="coerce").fillna(0.0).to_numpy()
    quantile_count = max(0, int(getattr(args, "threshold_quantile_count", 28)))
    quantiles = []
    if quantile_count > 0:
        quantiles = [float(value) for value in np.quantile(scores, np.linspace(0.50, 0.995, quantile_count))]
    return sorted(set(round(value, 8) for value in [*fixed, *quantiles] if 0.0 <= value <= 1.0))


def event_summary(scored: pd.DataFrame, threshold: float, args: argparse.Namespace, source_summary: dict[str, Any]) -> dict[str, Any]:
    work = scored.rename(columns={"level2_score": "oracle_start_score"})
    summary = cat_stage1.event_summary(work, float(threshold), args, source_summary)
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
        best = max(allowed, key=lambda row: (float(row["pick_match_rate"]), -float(row["picks_per_oracle"])))
        return float(best["threshold"])
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
    stem = str(rows[0].get("_output_stem") or "level2_three_l1")
    clean_rows = [{k: v for k, v in row.items() if k != "_output_stem"} for row in rows]
    with (output_dir / f"{stem}_readable_summary.csv").open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(clean_rows[0].keys()))
        writer.writeheader()
        writer.writerows(clean_rows)


def write_metadata(
    output_dir: Path,
    rid: str,
    args: argparse.Namespace,
    selected_threshold_value: float,
    train_summary: dict[str, Any],
    valid_summary: dict[str, Any],
    selected_train: dict[str, Any],
    selected_valid: dict[str, Any],
    raw_train_rows: int,
    actual_train_rows: int,
) -> None:
    ids = l1_run_ids(args)
    active_ids = active_l1_run_ids(args)
    cat_features, num_features = feature_columns(args)
    engines = manager_engines(args)
    metadata = {
        "run_id": rid,
        "model_type": f"catboost_oracle_start_level2_{manager_engine_label(args)}_l1_manager",
        "design": "level1_all_roots_single_timeframe_per_engine__level2_selected_engine_manager",
        "timeframe": args.timeframe,
        "roots": parse_roots(args.roots),
        "roots_empty_means_all": True,
        "oracle_run_id": args.oracle_run_id,
        "candidate_set_id": args.candidate_set_id,
        "read_candidate_cache": bool(args.read_candidate_cache),
        "train_year": int(args.train_year),
        "valid_year": int(args.valid_year),
        "manager_engines": engines,
        "level1_model_runs": active_ids,
        "all_cached_level1_model_runs": ids,
        "level1_selected_thresholds": {
            key: selected_threshold(value, 0.5) for key, value in active_ids.items()
        },
        "event_rules": {
            "match_window_bars": int(args.match_window_bars),
            "cooldown_bars": int(args.cooldown_bars),
            "min_oracle_recall": float(args.min_oracle_recall),
            "max_picks_per_oracle": float(args.max_picks_per_oracle),
            "thresholds": args.thresholds,
            "threshold_quantile_count": int(args.threshold_quantile_count),
        },
        "training_parameters": {
            "iterations": int(args.iterations),
            "depth": int(args.depth),
            "learning_rate": float(args.learning_rate),
            "l2_leaf_reg": float(args.l2_leaf_reg),
            "random_seed": int(args.random_seed),
            "max_train_rows": int(args.max_train_rows),
            "raw_train_rows": int(raw_train_rows),
            "actual_train_rows": int(actual_train_rows),
        },
        "features": {"cat": cat_features, "num": num_features},
        "source_summaries": {str(args.train_year): train_summary, str(args.valid_year): valid_summary},
        "selected_threshold": float(selected_threshold_value),
        "selected_train_summary": selected_train,
        "selected_valid_summary": selected_valid,
        "read": f"Simplified Level 2 manager trained only on {', '.join(engines)} Level 1 {args.timeframe} opinions.",
    }
    (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")


def main() -> int:
    args = parse_args()
    rid = run_id(args)
    output_dir = wave.ABCD_ROOT / "model_registry" / rid
    if output_dir.exists() and not args.replace_run:
        raise ValueError(f"Run already exists: {rid}. Use --replace-run.")
    output_dir.mkdir(parents=True, exist_ok=True)

    if args.read_opinion_cache:
        train_rows, train_source = load_opinion_cache(args, int(args.train_year))
        valid_rows, valid_source = load_opinion_cache(args, int(args.valid_year))
    else:
        train_rows, train_source = build_l1_opinion_rows(int(args.train_year), args)
        valid_rows, valid_source = build_l1_opinion_rows(int(args.valid_year), args)
        if args.write_opinion_cache or args.only_build_opinion_cache:
            write_opinion_cache(train_rows, train_source, args, int(args.train_year))
            write_opinion_cache(valid_rows, valid_source, args, int(args.valid_year))

    if args.only_build_opinion_cache:
        print("Built Level 1 opinion caches only; skipping Level 2 training.", flush=True)
        return 0

    train_sample = cap_train_rows(train_rows, args)
    print(
        f"Training simplified Level 2 manager rows={len(train_sample):,}/{len(train_rows):,} "
        f"positives={int(train_sample['is_oracle_start'].astype(int).sum()):,}",
        flush=True,
    )
    model = train_manager(train_sample, args)
    train_scored = add_manager_scores(train_rows, model, args)
    valid_scored = add_manager_scores(valid_rows, model, args)

    train_sweep = [event_summary(train_scored, value, args, train_source) for value in threshold_candidates(train_scored, args)]
    selected_threshold_value = choose_threshold(train_sweep, args)
    valid_sweep = [event_summary(valid_scored, float(row["threshold"]), args, valid_source) for row in train_sweep]
    selected_train = event_summary(train_scored, selected_threshold_value, args, train_source)
    selected_valid = event_summary(valid_scored, selected_threshold_value, args, valid_source)

    stem = output_stem(args)
    model.save_model(str(output_dir / f"catboost_oracle_start_{stem}_manager.cbm"))
    pd.DataFrame(train_sweep).to_csv(output_dir / f"{stem}_threshold_sweep_{args.train_year}.csv", index=False)
    pd.DataFrame(valid_sweep).to_csv(output_dir / f"{stem}_threshold_sweep_{args.valid_year}.csv", index=False)
    if args.save_score_csvs:
        train_scored.to_csv(output_dir / f"{stem}_rows_{args.train_year}.csv", index=False)
        valid_scored.to_csv(output_dir / f"{stem}_rows_{args.valid_year}.csv", index=False)

    readable = [
        {**readable_row(str(args.train_year), selected_train), "_output_stem": stem},
        {**readable_row(str(args.valid_year), selected_valid), "_output_stem": stem},
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

    print(f"selected_{stem}_threshold={selected_threshold_value:.6f}", flush=True)
    print(json.dumps({"train_selected": selected_train, "valid_selected": selected_valid}, indent=2, default=to_jsonable), flush=True)
    print(f"Saved simplified Level 2 manager ({manager_engine_label(args)}): {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
