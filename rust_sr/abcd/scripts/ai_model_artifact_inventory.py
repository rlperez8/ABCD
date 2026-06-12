#!/usr/bin/env python3
"""Inventory model_registry artifacts without deleting anything."""

from __future__ import annotations

import argparse
import csv
import json
import sys
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_wave_rider_research as wave


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--registry-dir", default=str(wave.ABCD_ROOT / "model_registry"))
    parser.add_argument("--output", default=str(wave.ABCD_ROOT / "reports" / "model_artifact_inventory.csv"))
    parser.add_argument("--main-run-id", default="", help="Comma-separated run ids to label as main.")
    return parser.parse_args()


def load_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {}


def dir_size(path: Path) -> int:
    total = 0
    for file in path.rglob("*"):
        if file.is_file():
            try:
                total += file.stat().st_size
            except OSError:
                pass
    return total


def classify(run_id: str, metadata: dict[str, Any], size_bytes: int, main_ids: set[str]) -> str:
    lowered = run_id.lower()
    if run_id in main_ids:
        return "main"
    if not metadata:
        return "no_metadata"
    if size_bytes <= 0:
        return "empty"
    if "smoke" in lowered:
        return "smoke"
    if "debug" in lowered:
        return "debug"
    if "fastpass" in lowered or metadata.get("training_parameters", {}).get("fast_validation_rows"):
        return "fast_validation"
    if "failed" in lowered:
        return "failed_named"
    return "research"


def main() -> int:
    args = parse_args()
    registry = Path(args.registry_dir)
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    main_ids = {part.strip() for part in str(args.main_run_id or "").split(",") if part.strip()}
    rows: list[dict[str, Any]] = []

    for run_dir in sorted(path for path in registry.iterdir() if path.is_dir()):
        metadata_path = run_dir / "metadata.json"
        metadata = load_json(metadata_path) if metadata_path.exists() else {}
        run_id = metadata.get("run_id") or run_dir.name
        size_bytes = dir_size(run_dir)
        readable_count = len(list(run_dir.glob("*readable_summary.csv")))
        model_files = [file.name for file in run_dir.iterdir() if file.is_file() and file.suffix.lower() in {".cbm", ".json", ".txt", ".pkl", ".gz"}]
        rows.append(
            {
                "status": classify(str(run_id), metadata, size_bytes, main_ids),
                "run_id": run_id,
                "model_type": metadata.get("model_type", ""),
                "root": metadata.get("root", ""),
                "train_year": metadata.get("train_year", metadata.get("threshold_year", "")),
                "valid_year": metadata.get("valid_year", ""),
                "size_mb": round(size_bytes / (1024 * 1024), 3),
                "file_count": len(list(run_dir.iterdir())),
                "readable_summary_count": readable_count,
                "has_metadata": bool(metadata),
                "model_files": "|".join(model_files[:10]),
                "path": str(run_dir),
            }
        )

    fieldnames = [
        "status",
        "run_id",
        "model_type",
        "root",
        "train_year",
        "valid_year",
        "size_mb",
        "file_count",
        "readable_summary_count",
        "has_metadata",
        "model_files",
        "path",
    ]
    with output.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames)
        writer.writeheader()
        writer.writerows(rows)
    print(f"Wrote artifact inventory rows={len(rows):,}: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
