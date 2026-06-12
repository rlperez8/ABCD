#!/usr/bin/env python3
"""Run Level 2 root all-timeframe Stage 1 specialists for a small root batch."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_wave_rider_research as wave
import ai_oracle_start_stage1_root_alltf_model as root_alltf


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--roots", required=True, help="Comma-separated root symbols.")
    parser.add_argument("--timeframes", default=root_alltf.DEFAULT_TIMEFRAMES)
    parser.add_argument("--train-years", default="2024")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--run-prefix", default="aicw-oracle-start-root-alltf-v1")
    parser.add_argument("--iterations", type=int, default=650)
    parser.add_argument("--depth", type=int, default=8)
    parser.add_argument("--learning-rate", type=float, default=0.045)
    parser.add_argument("--l2-leaf-reg", type=float, default=8.0)
    parser.add_argument("--positive-pre-bars", type=int, default=0)
    parser.add_argument("--positive-post-bars", type=int, default=2)
    parser.add_argument("--negative-ratio", type=float, default=8.0)
    parser.add_argument("--max-train-rows", type=int, default=500_000)
    parser.add_argument("--cooldown-bars", type=int, default=6)
    parser.add_argument("--min-oracle-recall", type=float, default=0.90)
    parser.add_argument("--max-picks-per-oracle", type=float, default=5.0)
    parser.add_argument("--limit-symbols", type=int, default=0)
    parser.add_argument("--replace-run", action="store_true")
    parser.add_argument("--continue-on-error", action="store_true")
    return parser.parse_args()


def parse_roots(raw: str) -> list[str]:
    roots = [part.strip().upper() for part in str(raw or "").split(",") if part.strip()]
    if not roots:
        raise ValueError("At least one root is required.")
    return roots


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def find_metadata(run_prefix: str, root: str, train_years: str, valid_year: int) -> tuple[Path | None, dict[str, Any] | None]:
    safe_train = str(train_years).replace(",", "_")
    expected = f"{run_prefix}-{root}-alltf-tr{safe_train}-v{valid_year}"
    expected_path = wave.ABCD_ROOT / "model_registry" / expected
    expected_metadata = expected_path / "metadata.json"
    if expected_metadata.exists():
        return expected_path, load_json(expected_metadata)

    matches: list[tuple[float, Path, dict[str, Any]]] = []
    for path in (wave.ABCD_ROOT / "model_registry").glob(f"{run_prefix}-{root}-alltf-*"):
        metadata_path = path / "metadata.json"
        if not metadata_path.exists():
            continue
        try:
            metadata = load_json(metadata_path)
        except Exception:
            continue
        if str(metadata.get("root") or "").upper() == root:
            matches.append((metadata_path.stat().st_mtime, path, metadata))
    if not matches:
        return None, None
    _, path, metadata = sorted(matches, key=lambda item: item[0], reverse=True)[0]
    return path, metadata


def selected_summary(metadata: dict[str, Any] | None) -> dict[str, Any]:
    if not metadata:
        return {}
    selected = metadata.get("selected_valid_summary") or {}
    train_summary = metadata.get("train_summary") or {}
    valid_summary = metadata.get("valid_summary") or {}
    return {
        "threshold": selected.get("threshold"),
        "picked_events": selected.get("picked_events"),
        "matched_picks": selected.get("matched_picks"),
        "trend_accuracy": selected.get("pick_match_rate"),
        "matched_oracle_starts": selected.get("matched_oracle_starts"),
        "oracle_accuracy": selected.get("oracle_recall"),
        "picks_per_oracle": selected.get("picks_per_oracle"),
        "median_abs_bars": selected.get("median_abs_bars"),
        "train_rows": train_summary.get("materialized_rows"),
        "train_positives": train_summary.get("materialized_positives"),
        "valid_rows": valid_summary.get("materialized_rows"),
        "valid_oracle_starts": valid_summary.get("oracle_starts"),
    }


def main() -> int:
    args = parse_args()
    roots = parse_roots(args.roots)
    log_dir = wave.ABCD_ROOT / "logs" / "stage1_root_alltf_batches"
    log_dir.mkdir(parents=True, exist_ok=True)
    summary_path = log_dir / f"{args.run_prefix}-batch-summary.json"
    trainer = SCRIPT_DIR / "ai_oracle_start_stage1_root_alltf_model.py"

    rows: list[dict[str, Any]] = []
    started_all = time.time()
    print(f"Root all-TF Stage 1 batch roots={roots} timeframes={args.timeframes}", flush=True)

    for index, root in enumerate(roots, start=1):
        out_log = log_dir / f"{root.lower()}-root-alltf.out.log"
        err_log = log_dir / f"{root.lower()}-root-alltf.err.log"
        command = [
            sys.executable,
            str(trainer),
            "--root",
            root,
            "--timeframes",
            args.timeframes,
            "--train-years",
            args.train_years,
            "--threshold-year",
            str(args.threshold_year),
            "--valid-year",
            str(args.valid_year),
            "--run-prefix",
            args.run_prefix,
            "--iterations",
            str(args.iterations),
            "--depth",
            str(args.depth),
            "--learning-rate",
            str(args.learning_rate),
            "--l2-leaf-reg",
            str(args.l2_leaf_reg),
            "--positive-pre-bars",
            str(args.positive_pre_bars),
            "--positive-post-bars",
            str(args.positive_post_bars),
            "--negative-ratio",
            str(args.negative_ratio),
            "--max-train-rows",
            str(args.max_train_rows),
            "--cooldown-bars",
            str(args.cooldown_bars),
            "--min-oracle-recall",
            str(args.min_oracle_recall),
            "--max-picks-per-oracle",
            str(args.max_picks_per_oracle),
            "--limit-symbols",
            str(args.limit_symbols),
        ]
        if args.replace_run:
            command.append("--replace-run")

        print(f"[{index}/{len(roots)}] {root}: train", flush=True)
        started = time.time()
        with out_log.open("w", encoding="utf-8") as stdout, err_log.open("w", encoding="utf-8") as stderr:
            completed = subprocess.run(command, cwd=str(wave.ABCD_ROOT.parents[1]), stdout=stdout, stderr=stderr)

        elapsed = round(time.time() - started, 2)
        output_dir, metadata = find_metadata(args.run_prefix, root, args.train_years, args.valid_year)
        row = {
            "root": root,
            "status": "ok" if completed.returncode == 0 else "failed",
            "returncode": int(completed.returncode),
            "elapsed_seconds": elapsed,
            "output_run_id": output_dir.name if output_dir else None,
            "output_dir": str(output_dir) if output_dir else None,
            "out_log": str(out_log),
            "err_log": str(err_log),
            **selected_summary(metadata),
        }
        rows.append(row)
        summary_path.write_text(json.dumps(rows, indent=2), encoding="utf-8")

        print(
            f"[{index}/{len(roots)}] {root}: {row['status']} "
            f"events={row.get('picked_events')} trend_accuracy={row.get('trend_accuracy')} "
            f"oracle_accuracy={row.get('oracle_accuracy')} elapsed={elapsed}s",
            flush=True,
        )
        if completed.returncode != 0 and not args.continue_on_error:
            return int(completed.returncode)

    print(f"Done. elapsed={round(time.time() - started_all, 2)}s summary={summary_path}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
