#!/usr/bin/env python3
"""Sweep thresholds from already-scored oracle-start rows."""

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


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rows", required=True, help="CSV or pkl.gz rows with a score column.")
    parser.add_argument("--metadata", default="", help="Run/cache metadata JSON containing source_summaries.")
    parser.add_argument("--year", type=int, required=True)
    parser.add_argument("--score-column", default="level5_score")
    parser.add_argument("--thresholds", default="0.10,0.12,0.14,0.16,0.18,0.20,0.22,0.25,0.30,0.35,0.40,0.45,0.50,0.55,0.60,0.65,0.70,0.75,0.80,0.85,0.90")
    parser.add_argument("--cooldown-bars", type=int, default=6)
    parser.add_argument("--match-window-bars", type=int, default=4)
    parser.add_argument("--output", default=str(wave.ABCD_ROOT / "reports" / "oracle_start_threshold_sweep.csv"))
    return parser.parse_args()


def load_rows(path: Path) -> pd.DataFrame:
    if path.suffix.lower() == ".csv":
        return pd.read_csv(path)
    return pd.read_pickle(path, compression="gzip")


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def source_summary(metadata: dict[str, Any], year: int, rows: pd.DataFrame) -> dict[str, Any]:
    summaries = metadata.get("source_summaries") or {}
    summary = summaries.get(str(year)) or metadata.get("source_summary") or {}
    if summary:
        return dict(summary)
    if "is_oracle_start" in rows.columns:
        labels = pd.to_numeric(rows["is_oracle_start"], errors="coerce").fillna(0).astype(int)
    else:
        labels = pd.Series([0] * len(rows), index=rows.index)
    positives = rows[labels == 1]
    oracle_count = (
        positives.get("nearest_oracle_trade_id", pd.Series(dtype=object))
        .dropna()
        .astype(str)
        .replace("", np.nan)
        .dropna()
        .nunique()
    )
    return {"year": int(year), "oracle_starts": int(oracle_count)}


def threshold_values(raw: str, rows: pd.DataFrame, score_column: str) -> list[float]:
    fixed = [float(part.strip()) for part in str(raw or "").split(",") if part.strip()]
    scores = pd.to_numeric(rows[score_column], errors="coerce").fillna(0.0).to_numpy()
    quantiles = [float(value) for value in np.quantile(scores, np.linspace(0.50, 0.995, 28))]
    return sorted(set(round(value, 8) for value in [*fixed, *quantiles] if 0.0 <= value <= 1.0))


def pct(value: Any) -> float:
    return round(float(value or 0.0) * 100.0, 2)


def readable_row(summary: dict[str, Any]) -> dict[str, Any]:
    return {
        "threshold": summary["threshold"],
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


def main() -> int:
    args = parse_args()
    rows_path = Path(args.rows)
    rows = load_rows(rows_path)
    if args.score_column not in rows.columns:
        raise ValueError(f"Score column not found: {args.score_column}")
    metadata = load_json(Path(args.metadata)) if str(args.metadata or "").strip() else {}
    event_args = argparse.Namespace(
        cooldown_bars=int(args.cooldown_bars),
        match_window_bars=int(args.match_window_bars),
    )
    scored = rows.copy()
    scored["oracle_start_score"] = pd.to_numeric(scored[args.score_column], errors="coerce").fillna(0.0)
    summary = source_summary(metadata, int(args.year), scored)
    sweep = [root_alltf.event_summary(scored, value, event_args, summary) for value in threshold_values(args.thresholds, scored, args.score_column)]

    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    pd.DataFrame(sweep).to_csv(output, index=False)
    readable_output = output.with_name(output.stem + "_readable.csv")
    readable = [readable_row(row) for row in sweep]
    with readable_output.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(readable[0].keys()))
        writer.writeheader()
        writer.writerows(readable)
    best = max(
        sweep,
        key=lambda row: (
            2
            * float(row["pick_match_rate"])
            * float(row["oracle_recall"])
            / max(float(row["pick_match_rate"]) + float(row["oracle_recall"]), 1e-9),
            float(row["pick_match_rate"]),
        ),
    )
    print(f"Wrote threshold sweep rows={len(sweep):,}: {output}")
    print(f"Best balanced threshold={best['threshold']:.8f} accuracy={pct(best['pick_match_rate'])}% coverage={pct(best['oracle_recall'])}%")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
