#!/usr/bin/env python3
"""
Train adaptive Stage 2 confirmation on full-grid Stage 1 events.

The sampled Stage 2 workflow trains from balanced oracle rows. This script uses
the live-like source instead: every eligible candle is scored by Stage 1, then
the Stage 1 events are expanded into adaptive confirmation rows.

Visual CNN scores are intentionally not used by default; this first full-grid
path focuses on tabular CatBoost/XGBoost/LightGBM signals so it can run without
rendering chart images for millions of candles.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from pathlib import Path
from typing import Any

import joblib
import numpy as np
import pandas as pd

try:
    from catboost import CatBoostClassifier
except ImportError as exc:  # pragma: no cover
    raise SystemExit("CatBoost is required. Run with .venv_ai\\Scripts\\python.exe") from exc


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_scanner_model as scanner
import ai_oracle_start_four_model_agreement as four_model
import ai_oracle_start_manager_model as manager_model
import ai_oracle_start_stage2_adaptive_confirmation as adaptive_stage2
import ai_oracle_trend_start_model as start_model
import ai_wave_rider_research as wave
import eval_oracle_trend_start_full_grid as catboost_grid


DEFAULT_FOUR_MODEL_RUN = "aicw-oracle-start-fourmodel-v1-v2026"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--four-model-run-id", default=DEFAULT_FOUR_MODEL_RUN)
    parser.add_argument("--oracle-run-id", default=start_model.DEFAULT_ORACLE_RUN)
    parser.add_argument("--run-prefix", default="aicw-stage2-fullgrid-v1")
    parser.add_argument("--train-year", type=int, default=2024)
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--timeframe", default="2m")
    parser.add_argument("--roots", default="")
    parser.add_argument("--stage1-rule", choices=["catboost", "tabular_majority", "tabular_all"], default="catboost")
    parser.add_argument("--stage1-threshold", type=float, default=0.55)
    parser.add_argument("--stage1-cooldown-bars", type=int, default=8)
    parser.add_argument("--max-confirm-bars", type=int, default=16)
    parser.add_argument("--oracle-detail-match-bars", type=int, default=4)
    parser.add_argument("--target-mode", choices=["event", "proof"], default="proof")
    parser.add_argument("--min-confirm-close-atr", type=float, default=0.05)
    parser.add_argument("--min-confirm-favorable-atr", type=float, default=0.15)
    parser.add_argument("--max-confirm-adverse-atr", type=float, default=1.50)
    parser.add_argument("--min-remaining-r", type=float, default=0.25)
    parser.add_argument("--min-remaining-bars", type=int, default=1)
    parser.add_argument("--min-threshold-precision", type=float, default=0.78)
    parser.add_argument("--min-threshold-picks", type=int, default=250)
    parser.add_argument("--threshold-tail-candidates", type=int, default=650)
    parser.add_argument("--tabular-batch-size", type=int, default=50000)
    parser.add_argument("--limit-symbols", type=int, default=0)
    parser.add_argument("--max-events-per-year", type=int, default=0)
    parser.add_argument("--sample-events-after-build", action="store_true")
    parser.add_argument("--iterations", type=int, default=450)
    parser.add_argument("--depth", type=int, default=5)
    parser.add_argument("--learning-rate", type=float, default=0.04)
    parser.add_argument("--l2-leaf-reg", type=float, default=10.0)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--skip-save-expanded-rows", action="store_true")
    parser.add_argument("--replace-run", action="store_true")

    # Candle feature defaults.
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=240.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    return parser.parse_args()


def run_id(args: argparse.Namespace) -> str:
    raw = (
        f"{args.run_prefix}-{args.stage1_rule}-s{str(args.stage1_threshold).replace('.', 'p')}"
        f"-m{int(args.max_confirm_bars)}-v{args.valid_year}"
    )
    if len(raw) <= 64:
        return raw
    digest = hashlib.sha1(raw.encode("utf-8")).hexdigest()[:10]
    return f"{args.run_prefix[:34]}-{args.stage1_rule}-{digest}-v{args.valid_year}"


def to_jsonable(value: Any) -> Any:
    if isinstance(value, pd.Timestamp):
        return value.isoformat()
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def load_json(path: Path) -> dict[str, Any]:
    if not path.exists():
        raise FileNotFoundError(f"Missing JSON: {path}")
    return json.loads(path.read_text(encoding="utf-8"))


def load_stage1_models(four_model_run_id: str) -> dict[str, Any]:
    four_dir = wave.ABCD_ROOT / "model_registry" / four_model_run_id
    four_meta = load_json(four_dir / "metadata.json")
    visual_dir = wave.ABCD_ROOT / "model_registry" / str(four_meta["visual_run_id"])
    visual_meta = load_json(visual_dir / "metadata.json")
    catboost_dir = wave.ABCD_ROOT / "model_registry" / str(visual_meta["catboost_run_id"])

    catboost = CatBoostClassifier()
    catboost.load_model(str(catboost_dir / "catboost_oracle_start_model.cbm"))
    return {
        "four_dir": four_dir,
        "four_meta": four_meta,
        "thresholds": {name: float(value) for name, value in four_meta["thresholds"].items()},
        "catboost": catboost,
        "preprocessor": joblib.load(four_dir / "tabular_preprocessor.joblib"),
        "xgboost": joblib.load(four_dir / "xgboost_oracle_start.joblib"),
        "lightgbm": joblib.load(four_dir / "lightgbm_oracle_start.joblib"),
    }


def stage1_mask(frame: pd.DataFrame, thresholds: dict[str, float], args: argparse.Namespace) -> np.ndarray:
    cat = pd.to_numeric(frame["catboost_score"], errors="coerce").fillna(0.0).to_numpy() >= float(args.stage1_threshold)
    xgb = pd.to_numeric(frame["xgboost_score"], errors="coerce").fillna(0.0).to_numpy() >= float(thresholds.get("xgboost", 0.5))
    lgbm = pd.to_numeric(frame["lightgbm_score"], errors="coerce").fillna(0.0).to_numpy() >= float(thresholds.get("lightgbm", 0.5))
    if args.stage1_rule == "catboost":
        return cat
    if args.stage1_rule == "tabular_all":
        return cat & xgb & lgbm
    return (cat.astype(int) + xgb.astype(int) + lgbm.astype(int)) >= 2


def score_feature_rows(rows: list[dict[str, Any]], models: dict[str, Any]) -> pd.DataFrame:
    frame = pd.DataFrame(rows)
    frame["catboost_score"] = models["catboost"].predict_proba(start_model.prepare_pool(frame, include_target=False))[:, 1]
    clean = four_model.clean_features(frame)
    x = models["preprocessor"].transform(clean)
    frame["xgboost_score"] = models["xgboost"].predict_proba(x)[:, 1]
    frame["lightgbm_score"] = models["lightgbm"].predict_proba(x)[:, 1]
    frame["visual_score"] = 0.0
    return frame


def score_indices(
    year: int,
    root: str,
    symbol: str,
    group: pd.DataFrame,
    direction: str,
    indices: np.ndarray,
    models: dict[str, Any],
    batch_size: int,
) -> pd.DataFrame:
    frames: list[pd.DataFrame] = []
    step = max(1000, int(batch_size))
    for start in range(0, len(indices), step):
        batch = indices[start : start + step].astype(int)
        rows = [start_model.feature_row(year, root, symbol, group, int(idx), direction) for idx in batch]
        if rows:
            frames.append(score_feature_rows(rows, models))
    return pd.concat(frames, ignore_index=True) if frames else pd.DataFrame()


def cooldown_events(scored: pd.DataFrame, mask: np.ndarray, cooldown_bars: int) -> pd.DataFrame:
    selected = scored.loc[mask].copy()
    if selected.empty:
        return selected
    selected["signal_idx"] = pd.to_numeric(selected["candidate_uid"].map(lambda _: np.nan), errors="coerce")
    selected = selected.sort_values("signal_date").reset_index(drop=True)
    kept: list[int] = []
    next_allowed_ts: pd.Timestamp | None = None
    # Signal indices are not in feature_row, so approximate cooldown by row time
    # spacing. For regular 2m candles this is equivalent enough for event thinning.
    minutes = 2
    for row_index, row in selected.iterrows():
        ts = pd.Timestamp(row["signal_date"])
        if next_allowed_ts is not None and ts < next_allowed_ts:
            continue
        kept.append(row_index)
        next_allowed_ts = ts + pd.Timedelta(minutes=minutes * (int(cooldown_bars) + 1))
    return selected.iloc[kept].reset_index(drop=True)


def build_year_rows(
    conn,
    year: int,
    args: argparse.Namespace,
    models: dict[str, Any],
) -> tuple[pd.DataFrame, dict[str, Any]]:
    roots = start_model.parse_list(args.roots)
    symbols = catboost_grid.fetch_symbols(conn, args.timeframe, int(year), roots)
    oracle = start_model.fetch_oracle_starts(conn, args.oracle_run_id, int(year), roots)
    oracle_exact, oracle_by_key = adaptive_stage2.fetch_oracle_details(conn, int(year), roots, str(args.oracle_run_id))
    if oracle.empty:
        raise ValueError(f"No oracle starts found for {year}")
    year_start = pd.Timestamp(year=int(year), month=1, day=1)
    year_end = pd.Timestamp(year=int(year) + 1, month=1, day=1)
    min_idx = 60
    event_rows: list[dict[str, Any]] = []
    total_candidates = 0
    stage1_events = 0
    stage1_positive_events = 0
    target_positive_events: set[str] = set()
    target_positive_rows = 0
    target_reason_counts: dict[str, int] = {}
    max_events = int(args.max_events_per_year)
    stop_at_event_cap = max_events > 0 and not bool(args.sample_events_after_build)

    for symbol_index, symbol_info in enumerate(symbols, start=1):
        if args.limit_symbols and symbol_index > int(args.limit_symbols):
            break
        root = str(symbol_info["root_symbol"])
        symbol = str(symbol_info["symbol"])
        raw_group = catboost_grid.fetch_symbol_candles(conn, args.timeframe, int(year), root, symbol)
        if raw_group.empty:
            continue
        group = scanner.enrich_candles(raw_group[["ts_utc", "open", "high", "low", "close", "volume"]].copy(), args)
        times = pd.to_datetime(group["ts_utc"], errors="coerce")
        valid_indices = np.where((times >= year_start) & (times < year_end))[0]
        valid_indices = valid_indices[(valid_indices >= min_idx) & (valid_indices < len(group) - int(args.max_confirm_bars) - 1)]
        if len(valid_indices) == 0:
            continue
        for direction in ["LONG", "SHORT"]:
            total_candidates += int(len(valid_indices))
            scored = score_indices(
                year=int(year),
                root=root,
                symbol=symbol,
                group=group,
                direction=direction,
                indices=valid_indices.astype(int),
                models=models,
                batch_size=int(args.tabular_batch_size),
            )
            if scored.empty:
                continue
            scored = manager_model.add_manager_features(scored, models["thresholds"])
            mask = stage1_mask(scored, models["thresholds"], args)
            selected = cooldown_events(scored, mask, int(args.stage1_cooldown_bars))
            if selected.empty:
                continue
            candle_times = pd.to_datetime(group["ts_utc"], errors="coerce").to_numpy(dtype="datetime64[ns]")
            for _, original_row in selected.iterrows():
                if stop_at_event_cap and stage1_events >= max_events:
                    break
                original = original_row.to_dict()
                signal_ts = pd.Timestamp(original["signal_date"])
                signal_idx = int(np.searchsorted(candle_times, np.datetime64(signal_ts), side="left"))
                if signal_idx >= len(group) - int(args.max_confirm_bars):
                    continue
                oracle_info = adaptive_stage2.match_oracle_info(oracle_exact, oracle_by_key, original, signal_ts, args)
                original["is_oracle_start"] = 1 if oracle_info else 0
                original["oracle_start_distance_bars"] = 0 if oracle_info else 999999
                original["stage1_pick"] = 1
                tick_size = wave.tick_size_for(symbol, root)
                atr_ticks = wave.finite(original.get("atr_ticks"), None)
                atr_hint = atr_ticks * tick_size if atr_ticks is not None and tick_size > 0 else None
                event_had_rows = False
                event_had_target = False
                for offset in range(1, int(args.max_confirm_bars) + 1):
                    features = adaptive_stage2.post_features_for_offset(
                        group,
                        signal_idx,
                        str(original["direction"]),
                        offset,
                        int(args.max_confirm_bars),
                        atr_hint,
                    )
                    if features is None:
                        continue
                    target, target_debug = adaptive_stage2.proof_target(original, features, oracle_info, args)
                    payload = dict(original)
                    payload.update(features)
                    payload.update(target_debug)
                    payload["stage2_target"] = int(target)
                    event_rows.append(payload)
                    target_reason_counts[str(payload["stage2_target_reason"])] = (
                        target_reason_counts.get(str(payload["stage2_target_reason"]), 0) + 1
                    )
                    target_positive_rows += int(target)
                    event_had_target = event_had_target or bool(target)
                    event_had_rows = True
                if event_had_rows:
                    stage1_events += 1
                    stage1_positive_events += int(bool(oracle_info))
                    if event_had_target:
                        target_positive_events.add(str(original["candidate_uid"]))
            if stop_at_event_cap and stage1_events >= max_events:
                break
        if symbol_index % 25 == 0:
            print(
                f"{year}: symbols {symbol_index:,}/{len(symbols):,} candidates={total_candidates:,} "
                f"stage1_events={stage1_events:,} rows={len(event_rows):,}",
                flush=True,
            )
        if stop_at_event_cap and stage1_events >= max_events:
            break

    frame = pd.DataFrame(event_rows)
    if frame.empty:
        raise ValueError(f"No Stage 2 rows built for {year}")
    raw_stage1_events = int(stage1_events)
    raw_stage1_positive_events = int(stage1_positive_events)
    raw_target_positive_events = int(len(target_positive_events))
    if bool(args.sample_events_after_build) and max_events > 0:
        event_ids = frame["candidate_uid"].drop_duplicates().astype(str).to_numpy()
        if len(event_ids) > max_events:
            rng = np.random.default_rng(int(args.random_seed) + int(year))
            keep = set(rng.choice(event_ids, size=max_events, replace=False).tolist())
            frame = frame[frame["candidate_uid"].astype(str).isin(keep)].reset_index(drop=True)
    counts = frame.groupby("candidate_uid")["candidate_uid"].transform("count").astype(float)
    frame["event_row_weight"] = 1.0 / counts.replace(0.0, 1.0)
    event_summary = frame.sort_values(["candidate_uid", "confirm_offset_bars"]).drop_duplicates("candidate_uid")
    sampled_stage1_events = int(event_summary["candidate_uid"].nunique())
    sampled_stage1_positive_events = int(pd.to_numeric(event_summary["is_oracle_start"], errors="coerce").fillna(0).sum())
    sampled_target_events = int(frame.loc[pd.to_numeric(frame["stage2_target"], errors="coerce").fillna(0) == 1, "candidate_uid"].nunique())
    sampled_target_rows = int(pd.to_numeric(frame["stage2_target"], errors="coerce").fillna(0).sum())
    sampled_reason_counts = {
        str(key): int(value) for key, value in frame["stage2_target_reason"].value_counts(dropna=False).items()
    }
    summary = {
        "source_rows": int(total_candidates),
        "source_positives": int(len(oracle)),
        "raw_stage1_events": raw_stage1_events,
        "raw_stage1_positive_events": raw_stage1_positive_events,
        "raw_target_positive_events": raw_target_positive_events,
        "stage1_events": sampled_stage1_events,
        "stage1_positive_events": sampled_stage1_positive_events,
        "expanded_events": int(frame["candidate_uid"].nunique()),
        "expanded_rows": int(len(frame)),
        "stage1_precision": float(sampled_stage1_positive_events / sampled_stage1_events) if sampled_stage1_events else 0.0,
        "stage1_total_oracle_recall": float(sampled_stage1_positive_events / len(oracle)) if len(oracle) else 0.0,
        "target_positive_rows": sampled_target_rows,
        "target_positive_events": sampled_target_events,
        "target_reason_counts": sampled_reason_counts,
    }
    print(f"{year}: full-grid rows {json.dumps(summary, default=to_jsonable)}", flush=True)
    return frame, summary


def main() -> int:
    args = parse_args()
    rid = run_id(args)
    output_dir = wave.ABCD_ROOT / "model_registry" / rid
    if output_dir.exists() and not args.replace_run:
        raise ValueError(f"Full-grid Stage 2 run already exists: {rid}. Use --replace-run.")
    output_dir.mkdir(parents=True, exist_ok=True)
    models = load_stage1_models(str(args.four_model_run_id))

    conn = wave.connect()
    try:
        train, train_source = build_year_rows(conn, int(args.train_year), args, models)
        threshold, threshold_source = build_year_rows(conn, int(args.threshold_year), args, models)
        valid, valid_source = build_year_rows(conn, int(args.valid_year), args, models)
    finally:
        conn.close()

    print(
        f"Training full-grid Stage 2 rows train={len(train):,} threshold={len(threshold):,} valid={len(valid):,}",
        flush=True,
    )
    model = adaptive_stage2.train_model(train, args)
    train_scored = adaptive_stage2.add_scores(train, model)
    threshold_scored = adaptive_stage2.add_scores(threshold, model)
    valid_scored = adaptive_stage2.add_scores(valid, model)
    selected_threshold, sweep = adaptive_stage2.choose_threshold(threshold_scored, threshold_source, args)
    results = {
        "train": {
            "source": train_source,
            "row": adaptive_stage2.row_metrics(train_scored),
            "event": adaptive_stage2.event_metrics(train_scored, train_source, selected_threshold),
        },
        str(args.threshold_year): {
            "source": threshold_source,
            "row": adaptive_stage2.row_metrics(threshold_scored),
            "event": adaptive_stage2.event_metrics(threshold_scored, threshold_source, selected_threshold),
        },
        str(args.valid_year): {
            "source": valid_source,
            "row": adaptive_stage2.row_metrics(valid_scored),
            "event": adaptive_stage2.event_metrics(valid_scored, valid_source, selected_threshold),
        },
    }
    print(f"selected_full_grid_stage2_threshold={selected_threshold:.6f}", flush=True)
    print(json.dumps(results, indent=2, default=to_jsonable), flush=True)

    model.save_model(str(output_dir / "catboost_stage2_full_grid_confirmation.cbm"))
    if not args.skip_save_expanded_rows:
        train_scored.to_csv(output_dir / "stage2_full_grid_rows_train.csv", index=False)
        threshold_scored.to_csv(output_dir / f"stage2_full_grid_rows_{args.threshold_year}.csv", index=False)
        valid_scored.to_csv(output_dir / f"stage2_full_grid_rows_{args.valid_year}.csv", index=False)
    adaptive_stage2.event_predictions(train_scored, selected_threshold).to_csv(output_dir / "stage2_full_grid_events_train.csv", index=False)
    adaptive_stage2.event_predictions(threshold_scored, selected_threshold).to_csv(
        output_dir / f"stage2_full_grid_events_{args.threshold_year}.csv",
        index=False,
    )
    adaptive_stage2.event_predictions(valid_scored, selected_threshold).to_csv(
        output_dir / f"stage2_full_grid_events_{args.valid_year}.csv",
        index=False,
    )
    sweep.to_csv(output_dir / f"stage2_full_grid_threshold_sweep_{args.threshold_year}.csv", index=False)
    cat_features, num_features = adaptive_stage2.feature_columns()
    metadata = {
        "run_id": rid,
        "model_type": "catboost_oracle_start_stage2_full_grid_confirmation",
        "four_model_run_id": args.four_model_run_id,
        "oracle_run_id": args.oracle_run_id,
        "timeframe": args.timeframe,
        "train_year": int(args.train_year),
        "threshold_year": int(args.threshold_year),
        "valid_year": int(args.valid_year),
        "stage1_rule": args.stage1_rule,
        "stage1_threshold": float(args.stage1_threshold),
        "stage1_cooldown_bars": int(args.stage1_cooldown_bars),
        "max_confirm_bars": int(args.max_confirm_bars),
        "target_parameters": {
            "min_confirm_close_atr": float(args.min_confirm_close_atr),
            "min_confirm_favorable_atr": float(args.min_confirm_favorable_atr),
            "max_confirm_adverse_atr": float(args.max_confirm_adverse_atr),
            "min_remaining_r": float(args.min_remaining_r),
            "min_remaining_bars": int(args.min_remaining_bars),
        },
        "selected_stage2_threshold": float(selected_threshold),
        "features": {"cat": cat_features, "num": num_features},
        "results": results,
        "read": "Full-grid Stage 2 training over live-like Stage 1 events. Visual score is set to 0.0.",
    }
    (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")
    print(f"Saved full-grid Stage 2 model: {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
