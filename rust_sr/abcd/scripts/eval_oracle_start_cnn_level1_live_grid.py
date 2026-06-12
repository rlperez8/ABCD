#!/usr/bin/env python3
"""Evaluate CNN Level 1 specialists with the same live-grid rules as CatBoost.

This is not the sampled CNN train/validation summary. It rebuilds the
live-style candle grid, scores each row with the saved CNN, chooses the
threshold on the threshold year using the CatBoost Stage 1 event rules, then
applies that same threshold to the validation year.
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
import torch


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_scanner_model as scanner
import ai_oracle_start_cnn_specialist as cnn_specialist
import ai_oracle_start_stage1_live_grid_model as live_grid
import ai_oracle_trend_start_model as sampled_stage1
import ai_oracle_trend_start_visual_cnn as visual_cnn
import ai_wave_rider_research as wave
import eval_oracle_trend_start_full_grid as grid_eval


DEFAULT_LEVEL1_PREFIX = "aicw-oracle-start-cnn-tower-v1"
DEFAULT_TIMEFRAMES = cnn_specialist.DEFAULT_TIMEFRAMES


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default="NQ")
    parser.add_argument("--timeframes", default=DEFAULT_TIMEFRAMES)
    parser.add_argument("--level1-prefix", default=DEFAULT_LEVEL1_PREFIX)
    parser.add_argument("--oracle-run-id-template", default="oracle-perfect-trends-tfspec-v1-{timeframe}-2024_2026")
    parser.add_argument("--run-prefix", default="aicw-oracle-start-cnn-l1-livegrid-eval-v1")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--match-window-bars", type=int, default=4)
    parser.add_argument("--cooldown-bars", type=int, default=6)
    parser.add_argument("--min-oracle-recall", type=float, default=0.90)
    parser.add_argument("--max-picks-per-oracle", type=float, default=5.0)
    parser.add_argument("--thresholds", default="0.20,0.25,0.30,0.35,0.40,0.45,0.50,0.55,0.60,0.65,0.70,0.75,0.80,0.85,0.90")
    parser.add_argument("--batch-size", type=int, default=1024)
    parser.add_argument("--max-rows-per-split", type=int, default=0, help="Debug cap only. 0 means full live grid.")
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--save-score-csvs", action="store_true")
    parser.add_argument("--replace-run", action="store_true")
    return parser.parse_args()


def parse_timeframes(raw: str) -> list[str]:
    values = [part.strip().lower() for part in str(raw or "").split(",") if part.strip()]
    if not values:
        raise ValueError("At least one timeframe is required")
    return values


def run_id(args: argparse.Namespace) -> str:
    root = str(args.root).upper()
    raw = f"{args.run_prefix}-{root}-th{int(args.threshold_year)}-v{int(args.valid_year)}"
    if len(raw) <= 96:
        return raw
    digest = hashlib.sha1(raw.encode("utf-8")).hexdigest()[:10]
    return f"{args.run_prefix}-{digest}-v{int(args.valid_year)}"


def l1_run_id(args: argparse.Namespace, timeframe: str) -> str:
    return f"{args.level1_prefix}-l1-{str(args.root).upper()}-{timeframe}-tr2024-v2026"


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def load_cnn(run_id_value: str) -> tuple[visual_cnn.OracleStartCnn, dict[str, Any]]:
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id_value
    metadata = load_json(model_dir / "metadata.json")
    model = visual_cnn.OracleStartCnn(float(metadata.get("dropout", 0.20)))
    state = torch.load(model_dir / "visual_oracle_start_cnn.pt", map_location="cpu")
    model.load_state_dict(state)
    model.eval()
    return model, metadata


def build_args(args: argparse.Namespace, timeframe: str) -> argparse.Namespace:
    return argparse.Namespace(
        oracle_run_id=str(args.oracle_run_id_template).format(timeframe=timeframe),
        timeframe=timeframe,
        roots=str(args.root).upper(),
        symbols="",
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
        limit_symbols=0,
        entry_breakout_bars=8,
        trail_lookback_bars=18,
        atr_period=14,
        atr_stop_pad=0.35,
        min_risk_ticks=12.0,
        max_risk_ticks=240.0,
        min_relative_volume=0.45,
    )


def render_model_args(args: argparse.Namespace, metadata: dict[str, Any], timeframe: str) -> argparse.Namespace:
    return argparse.Namespace(
        roots=str(args.root).upper(),
        timeframe=timeframe,
        height=int(metadata.get("height") or 40),
        width=int(metadata.get("width") or 40),
        lookback_bars=int(metadata.get("lookback_bars") or 64),
        min_candles=int(metadata.get("min_candles") or 36),
        batch_size=int(args.batch_size),
        entry_breakout_bars=8,
        trail_lookback_bars=18,
        atr_period=14,
        atr_stop_pad=0.35,
        min_risk_ticks=12.0,
        max_risk_ticks=240.0,
        min_relative_volume=0.45,
    )


def maybe_cap_rows(frame: pd.DataFrame, args: argparse.Namespace, year: int) -> pd.DataFrame:
    cap = int(args.max_rows_per_split)
    if cap <= 0 or len(frame) <= cap:
        return frame
    positives = frame[frame["is_oracle_start"] == 1]
    negatives = frame[frame["is_oracle_start"] == 0]
    keep_pos_n = min(len(positives), cap // 2)
    keep_neg_n = min(len(negatives), cap - keep_pos_n)
    keep_pos = positives.sample(n=keep_pos_n, random_state=int(args.random_seed) + int(year)) if keep_pos_n else positives
    keep_neg = negatives.sample(n=keep_neg_n, random_state=int(args.random_seed) + int(year)) if keep_neg_n else negatives
    return pd.concat([keep_pos, keep_neg], ignore_index=True).sample(
        frac=1.0,
        random_state=int(args.random_seed) + int(year),
    ).reset_index(drop=True)


def score_live_rows(
    conn,
    frame: pd.DataFrame,
    year: int,
    timeframe: str,
    model: visual_cnn.OracleStartCnn,
    metadata: dict[str, Any],
    args: argparse.Namespace,
) -> pd.DataFrame:
    model_args = render_model_args(args, metadata, timeframe)
    roots = sampled_stage1.parse_list(model_args.roots)
    candles = sampled_stage1.fetch_candles(conn, timeframe, int(year), roots)
    if candles.empty:
        raise ValueError(f"No candles found for {timeframe} {year}")

    work = frame.copy().reset_index(drop=True)
    work["signal_date"] = pd.to_datetime(work["signal_date"], errors="coerce")
    scores = np.full(len(work), np.nan, dtype=np.float32)
    by_symbol_rows = {
        str(symbol): group.sort_values("signal_date")
        for symbol, group in work.reset_index().groupby("symbol", sort=False)
    }
    image_batch: list[np.ndarray] = []
    index_batch: list[int] = []

    def flush_batch() -> None:
        nonlocal image_batch, index_batch
        if not image_batch:
            return
        images = np.stack(image_batch).astype(np.float32)
        with torch.no_grad():
            tensor = visual_cnn.to_tensor(images, model_args)
            batch_scores: list[np.ndarray] = []
            for start in range(0, len(tensor), int(model_args.batch_size)):
                logits = model(tensor[start : start + int(model_args.batch_size)])
                batch_scores.append(torch.sigmoid(logits).cpu().numpy())
        merged = np.concatenate(batch_scores) if batch_scores else np.array([], dtype=float)
        scores[np.array(index_batch, dtype=int)] = merged.astype(np.float32)
        image_batch = []
        index_batch = []

    seen = 0
    kept = 0
    for symbol_index, (symbol, raw_group) in enumerate(candles.groupby("symbol", sort=True), start=1):
        symbol_rows = by_symbol_rows.get(str(symbol))
        if symbol_rows is None or symbol_rows.empty:
            continue
        group = scanner.enrich_candles(raw_group[["ts_utc", "open", "high", "low", "close", "volume"]].copy(), model_args)
        times = pd.to_datetime(group["ts_utc"], errors="coerce").to_numpy(dtype="datetime64[ns]")
        for item in symbol_rows.to_dict("records"):
            seen += 1
            signal_ts = pd.Timestamp(item["signal_date"])
            idx = int(np.searchsorted(times, np.datetime64(signal_ts), side="left"))
            if idx >= len(group):
                continue
            start_idx = max(0, idx - int(model_args.lookback_bars) + 1)
            context = group.iloc[start_idx : idx + 1].reset_index(drop=True)
            if len(context) < int(model_args.min_candles):
                continue
            image = visual_cnn.visual.render_chart_image(context, str(item["direction"]), int(model_args.height), int(model_args.width))
            image_batch.append(image.reshape(-1))
            index_batch.append(int(item["index"]))
            kept += 1
            if len(image_batch) >= int(model_args.batch_size):
                flush_batch()
        if symbol_index % 10 == 0:
            print(
                f"{year} {timeframe}: rendered symbols={symbol_index:,} seen={seen:,}/{len(work):,} scored={kept:,}",
                flush=True,
            )
    flush_batch()
    scored = work.loc[~np.isnan(scores)].copy()
    scored["oracle_start_score"] = scores[~np.isnan(scores)].astype(float)
    print(f"{year} {timeframe}: scored rows={len(scored):,}/{len(work):,}", flush=True)
    return scored


def threshold_candidates(args: argparse.Namespace, scored: pd.DataFrame) -> list[float]:
    values = [float(part.strip()) for part in str(args.thresholds or "").split(",") if part.strip()]
    scores = pd.to_numeric(scored["oracle_start_score"], errors="coerce").fillna(0.0).to_numpy()
    quantiles = [float(value) for value in np.quantile(scores, np.linspace(0.55, 0.99, 18))]
    return sorted(set(round(value, 8) for value in [*values, *quantiles] if 0.0 <= value <= 1.0))


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


def readable_row(timeframe: str, split: str, summary: dict[str, Any], rows: int) -> dict[str, Any]:
    return {
        "timeframe": timeframe,
        "split": split,
        "threshold": summary["threshold"],
        "scored_rows": rows,
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
    fields = [
        "timeframe",
        "split",
        "threshold",
        "scored_rows",
        "picked_events",
        "matched_picks",
        "trend_accuracy_pct",
        "matched_oracle_starts",
        "oracle_coverage_pct",
        "oracle_missed_pct",
        "picks_per_oracle",
        "median_abs_bars",
        "mean_abs_bars",
    ]
    with (output_dir / "cnn_l1_livegrid_readable_summary.csv").open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)


def to_jsonable(value: Any) -> Any:
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, pd.Timestamp):
        return value.isoformat()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def evaluate_timeframe(
    conn,
    timeframe: str,
    args: argparse.Namespace,
    output_dir: Path,
) -> dict[str, Any]:
    build_args_for_tf = build_args(args, timeframe)
    rng = np.random.default_rng(int(args.random_seed))
    rid = l1_run_id(args, timeframe)
    model, metadata = load_cnn(rid)
    print(f"Evaluating CNN L1 live-grid {timeframe} model={rid}", flush=True)

    threshold_rows, threshold_source = live_grid.build_year_rows(
        conn,
        int(args.threshold_year),
        build_args_for_tf,
        rng,
        train_sample=False,
    )
    threshold_rows = maybe_cap_rows(threshold_rows, args, int(args.threshold_year))
    threshold_scored = score_live_rows(conn, threshold_rows, int(args.threshold_year), timeframe, model, metadata, args)
    threshold_sweep = [
        live_grid.event_summary(threshold_scored, value, build_args_for_tf, threshold_source)
        for value in threshold_candidates(args, threshold_scored)
    ]
    selected_threshold = choose_threshold(threshold_sweep, args)

    valid_rows, valid_source = live_grid.build_year_rows(conn, int(args.valid_year), build_args_for_tf, rng, train_sample=False)
    valid_rows = maybe_cap_rows(valid_rows, args, int(args.valid_year))
    valid_scored = score_live_rows(conn, valid_rows, int(args.valid_year), timeframe, model, metadata, args)
    valid_sweep = [
        live_grid.event_summary(valid_scored, float(row["threshold"]), build_args_for_tf, valid_source)
        for row in threshold_sweep
    ]
    selected_threshold_summary = live_grid.event_summary(threshold_scored, selected_threshold, build_args_for_tf, threshold_source)
    selected_valid_summary = live_grid.event_summary(valid_scored, selected_threshold, build_args_for_tf, valid_source)

    pd.DataFrame(threshold_sweep).to_csv(output_dir / f"cnn_l1_livegrid_threshold_sweep_{timeframe}_{args.threshold_year}.csv", index=False)
    pd.DataFrame(valid_sweep).to_csv(output_dir / f"cnn_l1_livegrid_threshold_sweep_{timeframe}_{args.valid_year}.csv", index=False)
    if args.save_score_csvs:
        threshold_scored.to_csv(output_dir / f"cnn_l1_livegrid_rows_{timeframe}_{args.threshold_year}.csv", index=False)
        valid_scored.to_csv(output_dir / f"cnn_l1_livegrid_rows_{timeframe}_{args.valid_year}.csv", index=False)

    return {
        "timeframe": timeframe,
        "model_run_id": rid,
        "threshold_source_summary": threshold_source,
        "valid_source_summary": valid_source,
        "selected_threshold_summary": selected_threshold_summary,
        "selected_valid_summary": selected_valid_summary,
        "threshold_scored_rows": int(len(threshold_scored)),
        "valid_scored_rows": int(len(valid_scored)),
    }


def main() -> int:
    args = parse_args()
    rid = run_id(args)
    output_dir = wave.ABCD_ROOT / "model_registry" / rid
    if output_dir.exists() and not args.replace_run:
        raise ValueError(f"Run already exists: {rid}. Use --replace-run.")
    output_dir.mkdir(parents=True, exist_ok=True)

    rows: list[dict[str, Any]] = []
    results: list[dict[str, Any]] = []
    conn = wave.connect()
    try:
        for timeframe in parse_timeframes(args.timeframes):
            result = evaluate_timeframe(conn, timeframe, args, output_dir)
            results.append(result)
            rows.append(
                readable_row(
                    timeframe,
                    str(args.threshold_year),
                    result["selected_threshold_summary"],
                    int(result["threshold_scored_rows"]),
                )
            )
            rows.append(
                readable_row(
                    timeframe,
                    str(args.valid_year),
                    result["selected_valid_summary"],
                    int(result["valid_scored_rows"]),
                )
            )
            write_readable(output_dir, rows)
            (output_dir / "metadata.json").write_text(
                json.dumps(
                    {
                        "run_id": rid,
                        "model_type": "cnn_l1_live_grid_event_eval",
                        "root": str(args.root).upper(),
                        "timeframes": parse_timeframes(args.timeframes),
                        "threshold_year": int(args.threshold_year),
                        "valid_year": int(args.valid_year),
                        "level1_prefix": args.level1_prefix,
                        "oracle_run_id_template": args.oracle_run_id_template,
                        "event_rules": {
                            "match_window_bars": int(args.match_window_bars),
                            "cooldown_bars": int(args.cooldown_bars),
                            "min_oracle_recall": float(args.min_oracle_recall),
                            "max_picks_per_oracle": float(args.max_picks_per_oracle),
                            "thresholds": args.thresholds,
                        },
                        "max_rows_per_split": int(args.max_rows_per_split),
                        "results": results,
                        "read": "CNN Level 1 evaluated on live-grid rows using the same event summary rules as CatBoost Stage 1.",
                    },
                    indent=2,
                    default=to_jsonable,
                ),
                encoding="utf-8",
            )
    finally:
        conn.close()

    print(f"Saved CNN L1 live-grid summary: {rid}", flush=True)
    print(json.dumps(rows, indent=2, default=to_jsonable), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
