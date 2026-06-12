#!/usr/bin/env python3
"""Audit Cat/Light/XGB L4 agreement groups from cached Level 5 opinion rows."""

from __future__ import annotations

import argparse
import csv
import json
import sys
from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_oracle_start_stage1_root_alltf_model as root_alltf
import ai_wave_rider_research as wave


DEFAULT_CAT_L4_RUN = "aicw-oracle-start-level4-manager-v1-NQ-tr2025-v2026"
DEFAULT_LIGHT_L4_RUN = "aicw-oracle-start-lightgbm-level4-manager-v1-NQ-tr2025-v2026"
DEFAULT_XGB_L4_RUN = "aicw-oracle-start-xgboost-level4-manager-v1-NQ-tr2025-v2026"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--opinion-cache", required=True, help="Cached pkl.gz L4 opinion rows.")
    parser.add_argument("--metadata", default="", help="Cache metadata JSON. Defaults to <cache>.metadata.json.")
    parser.add_argument("--cat-threshold", type=float, default=None)
    parser.add_argument("--light-threshold", type=float, default=None)
    parser.add_argument("--xgb-threshold", type=float, default=None)
    parser.add_argument("--cat-l4-run-id", default=DEFAULT_CAT_L4_RUN)
    parser.add_argument("--light-l4-run-id", default=DEFAULT_LIGHT_L4_RUN)
    parser.add_argument("--xgb-l4-run-id", default=DEFAULT_XGB_L4_RUN)
    parser.add_argument("--cooldown-bars", type=int, default=6)
    parser.add_argument("--match-window-bars", type=int, default=4)
    parser.add_argument("--sample-rows", type=int, default=5000)
    parser.add_argument("--output-dir", default=str(wave.ABCD_ROOT / "reports" / "oracle_start_l5_disagreement"))
    return parser.parse_args()


def load_json(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


def selected_threshold(run_id: str, fallback: float = 0.5) -> float:
    metadata = load_json(wave.ABCD_ROOT / "model_registry" / run_id / "metadata.json")
    if metadata.get("selected_threshold") is not None:
        return float(metadata["selected_threshold"])
    policy = metadata.get("threshold_policy") or {}
    if policy.get("selected_threshold") is not None:
        return float(policy["selected_threshold"])
    return float(fallback)


def load_rows(path: Path) -> pd.DataFrame:
    if path.suffix.lower() == ".csv":
        return pd.read_csv(path)
    return pd.read_pickle(path, compression="gzip")


def cache_metadata_path(cache_path: Path, explicit: str) -> Path:
    if str(explicit or "").strip():
        return Path(explicit).expanduser().resolve()
    return cache_path.with_suffix(cache_path.suffix + ".metadata.json")


def source_summary(metadata: dict[str, Any], rows: pd.DataFrame) -> dict[str, Any]:
    summary = dict(metadata.get("source_summary") or {})
    if summary:
        return summary
    positives = rows[pd.to_numeric(rows["is_oracle_start"], errors="coerce").fillna(0).astype(int) == 1]
    oracle_count = (
        positives.get("nearest_oracle_trade_id", pd.Series(dtype=object))
        .dropna()
        .astype(str)
        .replace("", np.nan)
        .dropna()
        .nunique()
    )
    return {"oracle_starts": int(oracle_count)}


def pct(value: Any) -> float:
    return round(float(value or 0.0) * 100.0, 2)


def summarize_mask(
    rows: pd.DataFrame,
    mask: pd.Series,
    name: str,
    event_args: argparse.Namespace,
    summary: dict[str, Any],
) -> dict[str, Any]:
    scored = rows.copy()
    scored["oracle_start_score"] = mask.astype(int)
    result = root_alltf.event_summary(scored, 0.5, event_args, summary)
    return {
        "group": name,
        "raw_rows": int(mask.sum()),
        "picked_events": result["picked_events"],
        "matched_picks": result["matched_picks"],
        "trend_accuracy_pct": pct(result["pick_match_rate"]),
        "matched_oracle_starts": result["matched_oracle_starts"],
        "oracle_coverage_pct": pct(result["oracle_recall"]),
        "oracle_missed_pct": round(100.0 - pct(result["oracle_recall"]), 2),
        "picks_per_oracle": round(float(result["picks_per_oracle"]), 3),
        "median_abs_bars": result["median_abs_bars"],
        "mean_abs_bars": result["mean_abs_bars"],
    }


def sample_group(rows: pd.DataFrame, mask: pd.Series, limit: int) -> pd.DataFrame:
    if limit <= 0:
        return rows.loc[mask].copy()
    sample = rows.loc[mask].copy()
    if sample.empty:
        return sample
    sample["max_l4_score"] = sample[["cat_l4_score", "light_l4_score", "xgb_l4_score"]].max(axis=1)
    sample["is_oracle_start"] = pd.to_numeric(sample["is_oracle_start"], errors="coerce").fillna(0).astype(int)
    return sample.sort_values(["is_oracle_start", "max_l4_score"], ascending=[False, False]).head(limit)


def main() -> int:
    args = parse_args()
    cache_path = Path(args.opinion_cache).expanduser().resolve()
    metadata = load_json(cache_metadata_path(cache_path, args.metadata))
    rows = load_rows(cache_path)

    thresholds = metadata.get("lower_level4_selected_thresholds") or {}
    cat_threshold = float(args.cat_threshold if args.cat_threshold is not None else thresholds.get("catboost", selected_threshold(args.cat_l4_run_id)))
    light_threshold = float(args.light_threshold if args.light_threshold is not None else thresholds.get("lightgbm", selected_threshold(args.light_l4_run_id)))
    xgb_threshold = float(args.xgb_threshold if args.xgb_threshold is not None else thresholds.get("xgboost", selected_threshold(args.xgb_l4_run_id)))

    cat = pd.to_numeric(rows["cat_l4_score"], errors="coerce").fillna(0.0) >= cat_threshold
    light = pd.to_numeric(rows["light_l4_score"], errors="coerce").fillna(0.0) >= light_threshold
    xgb = pd.to_numeric(rows["xgb_l4_score"], errors="coerce").fillna(0.0) >= xgb_threshold
    votes = cat.astype(int) + light.astype(int) + xgb.astype(int)

    event_args = argparse.Namespace(cooldown_bars=int(args.cooldown_bars), match_window_bars=int(args.match_window_bars))
    summary = source_summary(metadata, rows)
    groups: dict[str, pd.Series] = {
        "all_three_agree": cat & light & xgb,
        "cat_light_only_xgb_no": cat & light & ~xgb,
        "cat_xgb_only_light_no": cat & ~light & xgb,
        "light_xgb_only_cat_no": ~cat & light & xgb,
        "cat_only": cat & ~light & ~xgb,
        "light_only": ~cat & light & ~xgb,
        "xgb_only": ~cat & ~light & xgb,
        "only_one_model": votes == 1,
        "two_or_more_models": votes >= 2,
        "any_model": votes >= 1,
        "rejected_rows_reference": votes == 0,
    }

    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    summary_rows = [summarize_mask(rows, mask, name, event_args, summary) for name, mask in groups.items()]
    summary_path = output_dir / f"{cache_path.stem}_disagreement_summary.csv"
    with summary_path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(summary_rows[0].keys()))
        writer.writeheader()
        writer.writerows(summary_rows)

    for name in ["all_three_agree", "cat_light_only_xgb_no", "xgb_only", "only_one_model", "two_or_more_models"]:
        sample = sample_group(rows, groups[name], int(args.sample_rows))
        sample.to_csv(output_dir / f"{cache_path.stem}_{name}_sample.csv", index=False)

    thresholds_path = output_dir / f"{cache_path.stem}_thresholds.json"
    thresholds_path.write_text(
        json.dumps(
            {
                "catboost": cat_threshold,
                "lightgbm": light_threshold,
                "xgboost": xgb_threshold,
                "source_cache": str(cache_path),
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    print(f"Wrote disagreement summary: {summary_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
