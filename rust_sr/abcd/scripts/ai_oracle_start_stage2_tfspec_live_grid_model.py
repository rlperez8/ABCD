#!/usr/bin/env python3
"""
Train a Stage 2 confirmation filter for one timeframe-specific Stage 1 model.

Stage 1 is the wide trend-start detector. This script loads one trained
live-grid Stage 1 specialist, rebuilds its live-style fired events, then expands
each event into confirmation rows at offset 1..max_confirm_bars. The Stage 2
model learns which confirmations look real enough to keep.

This is still signal research. It does not place trades or choose exits.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from pathlib import Path
from typing import Any

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
import ai_oracle_start_manager_model as manager_model
import ai_oracle_start_stage2_adaptive_confirmation as adaptive_stage2
import ai_oracle_trend_start_model as start_model
import ai_wave_rider_research as wave
import eval_oracle_trend_start_full_grid as grid_eval


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage1-run-id", required=True)
    parser.add_argument("--run-prefix", default="aicw-stage2-tfspec-livegrid-v1")
    parser.add_argument("--train-year", type=int, default=2024)
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--stage1-threshold", type=float, default=None)
    parser.add_argument("--stage1-cooldown-bars", type=int, default=None)
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
    parser.add_argument("--batch-size", type=int, default=50000)
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

    # Candle enrichment defaults.
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
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
    if not path.exists():
        raise FileNotFoundError(f"Missing JSON: {path}")
    return json.loads(path.read_text(encoding="utf-8"))


def load_stage1(run_id: str) -> tuple[Path, dict[str, Any], CatBoostClassifier]:
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id
    metadata = load_json(model_dir / "metadata.json")
    model_path = model_dir / "catboost_oracle_start_live_grid_model.cbm"
    if not model_path.exists():
        raise FileNotFoundError(f"Missing Stage 1 live-grid model: {model_path}")
    model = CatBoostClassifier()
    model.load_model(str(model_path))
    return model_dir, metadata, model


def selected_stage1_threshold(metadata: dict[str, Any], override: float | None) -> float:
    if override is not None:
        return float(override)
    return grid_eval.selected_threshold(metadata)


def selected_stage1_cooldown(metadata: dict[str, Any], override: int | None) -> int:
    if override is not None:
        return int(override)
    policy = metadata.get("threshold_policy") or {}
    return int(policy.get("cooldown_bars") or 8)


def run_id(args: argparse.Namespace, metadata: dict[str, Any]) -> str:
    timeframe = str(metadata.get("timeframe") or "tf")
    roots = "_".join(str(root).upper() for root in (metadata.get("roots") or [])) or "root"
    raw = f"{args.run_prefix}-{timeframe}-{roots}-m{int(args.max_confirm_bars)}-v{int(args.valid_year)}"
    if len(raw) <= 64:
        return raw
    digest = hashlib.sha1(raw.encode("utf-8")).hexdigest()[:10]
    return f"{args.run_prefix[:34]}-{timeframe}-{roots[:10]}-{digest}"


def score_indices(
    model: CatBoostClassifier,
    year: int,
    root: str,
    symbol: str,
    group: pd.DataFrame,
    direction: str,
    indices: np.ndarray,
    batch_size: int,
) -> pd.DataFrame:
    frames: list[pd.DataFrame] = []
    step = max(1000, int(batch_size))
    for start in range(0, len(indices), step):
        batch = indices[start : start + step].astype(int)
        rows = [start_model.feature_row(year, root, symbol, group, int(idx), direction) for idx in batch]
        if not rows:
            continue
        frame = pd.DataFrame(rows)
        frame["signal_idx"] = batch.astype(int)
        frame["catboost_score"] = model.predict_proba(start_model.prepare_pool(frame, include_target=False))[:, 1]
        frame["xgboost_score"] = 0.0
        frame["lightgbm_score"] = 0.0
        frame["visual_score"] = 0.0
        frames.append(frame)
    return pd.concat(frames, ignore_index=True) if frames else pd.DataFrame()


def cooldown_events(scored: pd.DataFrame, threshold: float, cooldown_bars: int) -> pd.DataFrame:
    work = scored.copy()
    work["score_num"] = pd.to_numeric(work["catboost_score"], errors="coerce").fillna(0.0)
    work["signal_idx_num"] = pd.to_numeric(work["signal_idx"], errors="coerce").fillna(-1).astype(int)
    work = work[work["score_num"] >= float(threshold)].sort_values(
        ["symbol", "direction", "signal_idx_num", "score_num"],
        ascending=[True, True, True, False],
    )
    kept: list[pd.Series] = []
    for (_, _), group in work.groupby(["symbol", "direction"], sort=False):
        next_allowed = -1
        for _, row in group.iterrows():
            idx = int(row["signal_idx_num"])
            if idx < next_allowed:
                continue
            kept.append(row)
            next_allowed = idx + int(cooldown_bars) + 1
    if not kept:
        return pd.DataFrame(columns=scored.columns)
    return pd.DataFrame(kept).reset_index(drop=True)


def build_year_rows(
    conn,
    year: int,
    args: argparse.Namespace,
    stage1_metadata: dict[str, Any],
    stage1_model: CatBoostClassifier,
    threshold: float,
    cooldown_bars: int,
) -> tuple[pd.DataFrame, dict[str, Any]]:
    timeframe = str(stage1_metadata.get("timeframe") or "")
    roots = [str(root).upper() for root in (stage1_metadata.get("roots") or []) if str(root).strip()]
    oracle_run_id = str(stage1_metadata.get("oracle_run_id") or "")
    if not timeframe or not roots or not oracle_run_id:
        raise ValueError("Stage 1 metadata must include timeframe, roots, and oracle_run_id")

    symbols = grid_eval.fetch_symbols(conn, timeframe, int(year), roots)
    oracle = start_model.fetch_oracle_starts(conn, oracle_run_id, int(year), roots)
    oracle_exact, oracle_by_key = adaptive_stage2.fetch_oracle_details(conn, int(year), roots, oracle_run_id)
    if oracle.empty:
        raise ValueError(f"No oracle starts found for {year}: {oracle_run_id}")

    year_start = pd.Timestamp(year=int(year), month=1, day=1)
    year_end = pd.Timestamp(year=int(year) + 1, month=1, day=1)
    min_idx = 60
    max_events = int(args.max_events_per_year)
    stop_at_event_cap = max_events > 0 and not bool(args.sample_events_after_build)

    event_rows: list[dict[str, Any]] = []
    total_candidates = 0
    stage1_events = 0
    stage1_positive_events = 0
    target_positive_events: set[str] = set()
    target_reason_counts: dict[str, int] = {}

    manager_thresholds = {
        "catboost": float(threshold),
        "xgboost": 1.0,
        "lightgbm": 1.0,
        "visual_cnn": 1.0,
    }

    for symbol_index, symbol_info in enumerate(symbols, start=1):
        if int(args.limit_symbols) > 0 and symbol_index > int(args.limit_symbols):
            break
        root = str(symbol_info["root_symbol"])
        symbol = str(symbol_info["symbol"])
        raw_group = grid_eval.fetch_symbol_candles(conn, timeframe, int(year), root, symbol)
        if raw_group.empty:
            continue
        group = scanner.enrich_candles(raw_group[["ts_utc", "open", "high", "low", "close", "volume"]].copy(), args)
        times = pd.to_datetime(group["ts_utc"], errors="coerce")
        valid_indices = np.where((times >= year_start) & (times < year_end))[0]
        valid_indices = valid_indices[
            (valid_indices >= min_idx) & (valid_indices < len(group) - int(args.max_confirm_bars) - 1)
        ].astype(int)
        if len(valid_indices) == 0:
            continue
        candle_times = pd.to_datetime(group["ts_utc"], errors="coerce").to_numpy(dtype="datetime64[ns]")
        for direction in ["LONG", "SHORT"]:
            total_candidates += int(len(valid_indices))
            scored = score_indices(
                stage1_model,
                int(year),
                root,
                symbol,
                group,
                direction,
                valid_indices,
                int(args.batch_size),
            )
            if scored.empty:
                continue
            selected = cooldown_events(scored, float(threshold), int(cooldown_bars))
            if selected.empty:
                continue
            selected = manager_model.add_manager_features(selected, manager_thresholds)
            for _, selected_row in selected.iterrows():
                if stop_at_event_cap and stage1_events >= max_events:
                    break
                original = selected_row.to_dict()
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
                    target_reason = str(payload["stage2_target_reason"])
                    target_reason_counts[target_reason] = target_reason_counts.get(target_reason, 0) + 1
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
    sampled_target_events = int(
        frame.loc[pd.to_numeric(frame["stage2_target"], errors="coerce").fillna(0) == 1, "candidate_uid"].nunique()
    )
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
        "expanded_events": sampled_stage1_events,
        "expanded_rows": int(len(frame)),
        "stage1_precision": float(sampled_stage1_positive_events / sampled_stage1_events) if sampled_stage1_events else 0.0,
        "stage1_total_oracle_recall": float(sampled_stage1_positive_events / len(oracle)) if len(oracle) else 0.0,
        "target_positive_rows": sampled_target_rows,
        "target_positive_events": sampled_target_events,
        "target_reason_counts": sampled_reason_counts,
    }
    print(f"{year}: tfspec Stage 2 rows {json.dumps(summary, default=to_jsonable)}", flush=True)
    return frame, summary


def main() -> int:
    args = parse_args()
    _, stage1_metadata, stage1_model = load_stage1(str(args.stage1_run_id))
    args.timeframe = str(stage1_metadata.get("timeframe") or "")
    threshold = selected_stage1_threshold(stage1_metadata, args.stage1_threshold)
    cooldown_bars = selected_stage1_cooldown(stage1_metadata, args.stage1_cooldown_bars)
    rid = run_id(args, stage1_metadata)
    output_dir = wave.ABCD_ROOT / "model_registry" / rid
    if output_dir.exists() and not args.replace_run:
        raise ValueError(f"Stage 2 tfspec run already exists: {rid}. Use --replace-run.")
    output_dir.mkdir(parents=True, exist_ok=True)

    conn = wave.connect()
    try:
        train, train_source = build_year_rows(conn, int(args.train_year), args, stage1_metadata, stage1_model, threshold, cooldown_bars)
        threshold_rows, threshold_source = build_year_rows(
            conn,
            int(args.threshold_year),
            args,
            stage1_metadata,
            stage1_model,
            threshold,
            cooldown_bars,
        )
        valid, valid_source = build_year_rows(conn, int(args.valid_year), args, stage1_metadata, stage1_model, threshold, cooldown_bars)
    finally:
        conn.close()

    print(
        f"Training tfspec Stage 2 rows train={len(train):,} "
        f"threshold={len(threshold_rows):,} valid={len(valid):,}",
        flush=True,
    )
    model = adaptive_stage2.train_model(train, args)
    train_scored = adaptive_stage2.add_scores(train, model)
    threshold_scored = adaptive_stage2.add_scores(threshold_rows, model)
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
    print(f"selected_tfspec_stage2_threshold={selected_threshold:.6f}", flush=True)
    print(json.dumps(results, indent=2, default=to_jsonable), flush=True)

    model.save_model(str(output_dir / "catboost_stage2_tfspec_confirmation.cbm"))
    if not args.skip_save_expanded_rows:
        train_scored.to_csv(output_dir / "stage2_tfspec_rows_train.csv", index=False)
        threshold_scored.to_csv(output_dir / f"stage2_tfspec_rows_{args.threshold_year}.csv", index=False)
        valid_scored.to_csv(output_dir / f"stage2_tfspec_rows_{args.valid_year}.csv", index=False)
    adaptive_stage2.event_predictions(train_scored, selected_threshold).to_csv(
        output_dir / "stage2_tfspec_events_train.csv",
        index=False,
    )
    adaptive_stage2.event_predictions(threshold_scored, selected_threshold).to_csv(
        output_dir / f"stage2_tfspec_events_{args.threshold_year}.csv",
        index=False,
    )
    adaptive_stage2.event_predictions(valid_scored, selected_threshold).to_csv(
        output_dir / f"stage2_tfspec_events_{args.valid_year}.csv",
        index=False,
    )
    sweep.to_csv(output_dir / f"stage2_tfspec_threshold_sweep_{args.threshold_year}.csv", index=False)
    cat_features, num_features = adaptive_stage2.feature_columns()
    metadata = {
        "run_id": rid,
        "model_type": "catboost_oracle_start_stage2_tfspec_confirmation",
        "stage1_run_id": str(args.stage1_run_id),
        "stage1_oracle_run_id": stage1_metadata.get("oracle_run_id"),
        "timeframe": stage1_metadata.get("timeframe"),
        "roots": stage1_metadata.get("roots"),
        "train_year": int(args.train_year),
        "threshold_year": int(args.threshold_year),
        "valid_year": int(args.valid_year),
        "stage1_threshold": float(threshold),
        "stage1_cooldown_bars": int(cooldown_bars),
        "max_confirm_bars": int(args.max_confirm_bars),
        "target_parameters": {
            "target_mode": args.target_mode,
            "oracle_detail_match_bars": int(args.oracle_detail_match_bars),
            "min_confirm_close_atr": float(args.min_confirm_close_atr),
            "min_confirm_favorable_atr": float(args.min_confirm_favorable_atr),
            "max_confirm_adverse_atr": float(args.max_confirm_adverse_atr),
            "min_remaining_r": float(args.min_remaining_r),
            "min_remaining_bars": int(args.min_remaining_bars),
        },
        "selected_stage2_threshold": float(selected_threshold),
        "features": {"cat": cat_features, "num": num_features},
        "results": results,
        "read": "Stage 2 specialist trained from one timeframe-specific Stage 1 live-grid model.",
    }
    (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")
    print(f"Saved tfspec Stage 2 model: {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
