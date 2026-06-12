#!/usr/bin/env python3
"""Build a compact scoreboard from oracle-start model registry runs."""

from __future__ import annotations

import argparse
import csv
import json
import sys
from pathlib import Path
from typing import Any

import pandas as pd

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_wave_rider_research as wave


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--registry-dir", default=str(wave.ABCD_ROOT / "model_registry"))
    parser.add_argument("--output", default=str(wave.ABCD_ROOT / "reports" / "oracle_start_model_scoreboard.csv"))
    parser.add_argument("--main-run-id", default="", help="Comma-separated run ids to label as main.")
    return parser.parse_args()


def load_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {}


def classify_run(run_id: str, metadata: dict[str, Any], main_ids: set[str]) -> str:
    lowered = run_id.lower()
    if run_id in main_ids:
        return "main"
    if "smoke" in lowered:
        return "smoke"
    if "debug" in lowered:
        return "debug"
    if "fastpass" in lowered or metadata.get("training_parameters", {}).get("fast_validation_rows"):
        return "fast_validation"
    if "level5" in lowered:
        return "research_manager"
    return "research"


def number(value: Any) -> Any:
    if value is None or value == "":
        return ""
    try:
        return float(value)
    except (TypeError, ValueError):
        return value


def readable_files(run_dir: Path) -> list[Path]:
    return sorted(run_dir.glob("*readable_summary.csv"))


def rows_for_run(run_dir: Path, main_ids: set[str]) -> list[dict[str, Any]]:
    metadata_path = run_dir / "metadata.json"
    metadata = load_json(metadata_path) if metadata_path.exists() else {}
    run_id = metadata.get("run_id") or run_dir.name
    files = readable_files(run_dir)
    rows: list[dict[str, Any]] = []
    for file in files:
        try:
            frame = pd.read_csv(file)
        except Exception:
            continue
        for row in frame.to_dict("records"):
            missed = number(row.get("oracle_missed_pct"))
            coverage = number(row.get("oracle_coverage_pct"))
            if coverage == "" and missed != "":
                coverage = round(100.0 - float(missed), 4)
            rows.append(
                {
                    "status": classify_run(str(run_id), metadata, main_ids),
                    "run_id": run_id,
                    "model_type": metadata.get("model_type", ""),
                    "root": metadata.get("root", ""),
                    "train_year": metadata.get("train_year", metadata.get("threshold_year", "")),
                    "valid_year": metadata.get("valid_year", ""),
                    "split": row.get("split", ""),
                    "threshold": row.get("threshold", ""),
                    "scored_rows": row.get("scored_rows", ""),
                    "picked_events": row.get("picked_events", ""),
                    "trend_accuracy_pct": row.get("trend_accuracy_pct", ""),
                    "oracle_coverage_pct": coverage,
                    "oracle_missed_pct": missed,
                    "picks_per_oracle": row.get("picks_per_oracle", ""),
                    "median_abs_bars": row.get("median_abs_bars", ""),
                    "mean_abs_bars": row.get("mean_abs_bars", ""),
                    "summary_file": str(file),
                }
            )
    if not rows and metadata:
        rows.append(
            {
                "status": classify_run(str(run_id), metadata, main_ids),
                "run_id": run_id,
                "model_type": metadata.get("model_type", ""),
                "root": metadata.get("root", ""),
                "train_year": metadata.get("train_year", metadata.get("threshold_year", "")),
                "valid_year": metadata.get("valid_year", ""),
                "split": "",
                "threshold": metadata.get("selected_threshold", ""),
                "scored_rows": "",
                "picked_events": "",
                "trend_accuracy_pct": "",
                "oracle_coverage_pct": "",
                "oracle_missed_pct": "",
                "picks_per_oracle": "",
                "median_abs_bars": "",
                "mean_abs_bars": "",
                "summary_file": "",
            }
        )
    return rows


def main() -> int:
    args = parse_args()
    registry = Path(args.registry_dir)
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    main_ids = {part.strip() for part in str(args.main_run_id or "").split(",") if part.strip()}

    rows: list[dict[str, Any]] = []
    for run_dir in sorted(path for path in registry.iterdir() if path.is_dir()):
        if not run_dir.name.startswith("aicw-oracle-start"):
            continue
        rows.extend(rows_for_run(run_dir, main_ids))

    fieldnames = [
        "status",
        "run_id",
        "model_type",
        "root",
        "train_year",
        "valid_year",
        "split",
        "threshold",
        "scored_rows",
        "picked_events",
        "trend_accuracy_pct",
        "oracle_coverage_pct",
        "oracle_missed_pct",
        "picks_per_oracle",
        "median_abs_bars",
        "mean_abs_bars",
        "summary_file",
    ]
    with output.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames)
        writer.writeheader()
        writer.writerows(rows)
    print(f"Wrote scoreboard rows={len(rows):,}: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
