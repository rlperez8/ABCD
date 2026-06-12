#!/usr/bin/env python3
"""
Run Stage 1 live-grid training one timeframe at a time.

This is intentionally sequential and resumable. Each timeframe writes its own
stdout/stderr log and model_registry run folder, so a long all-timeframe pass can
be stopped and restarted without losing completed specialists.
"""

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

import ai_oracle_start_stage1_live_grid_model as stage1
import ai_wave_rider_research as wave


DEFAULT_TIMEFRAMES = "1m,2m,3m,4m,5m,6m,7m,8m,9m,10m,11m,12m,13m,14m,15m,30m"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--timeframes", default=DEFAULT_TIMEFRAMES)
    parser.add_argument("--roots", default="NQ")
    parser.add_argument("--symbols", default="")
    parser.add_argument("--train-years", default="2024")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--run-prefix", default="aicw-oracle-start-livegrid-alltf-v1")
    parser.add_argument(
        "--oracle-run-id-template",
        default="",
        help="Optional per-timeframe oracle template, e.g. oracle-perfect-trends-tfspec-v1-{timeframe}-2024_2026.",
    )
    parser.add_argument("--iterations", type=int, default=650)
    parser.add_argument("--depth", type=int, default=6)
    parser.add_argument("--learning-rate", type=float, default=0.045)
    parser.add_argument("--l2-leaf-reg", type=float, default=10.0)
    parser.add_argument("--positive-pre-bars", type=int, default=0)
    parser.add_argument("--positive-post-bars", type=int, default=2)
    parser.add_argument("--negative-ratio", type=float, default=8.0)
    parser.add_argument("--base-negatives-per-symbol", type=int, default=200)
    parser.add_argument("--min-oracle-recall", type=float, default=0.90)
    parser.add_argument("--max-picks-per-oracle", type=float, default=5.0)
    parser.add_argument("--replace-run", action="store_true")
    parser.add_argument("--continue-on-error", action="store_true")
    return parser.parse_args()


def parse_timeframes(raw: str) -> list[str]:
    values = [part.strip().lower() for part in str(raw or "").split(",") if part.strip()]
    if not values:
        raise ValueError("At least one timeframe is required")
    return values


def child_run_id(args: argparse.Namespace, timeframe: str) -> str:
    child = argparse.Namespace(
        run_prefix=args.run_prefix,
        timeframe=timeframe,
        roots=args.roots,
        train_years=args.train_years,
        valid_year=args.valid_year,
    )
    return stage1.run_id(child)


def load_metadata(run_id: str) -> dict[str, Any]:
    path = wave.ABCD_ROOT / "model_registry" / run_id / "metadata.json"
    if not path.exists():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


def oracle_run_id(args: argparse.Namespace, timeframe: str) -> str:
    template = str(getattr(args, "oracle_run_id_template", "") or "").strip()
    if not template:
        return ""
    return template.format(timeframe=timeframe)


def main() -> int:
    args = parse_args()
    timeframes = parse_timeframes(args.timeframes)
    script_path = SCRIPT_DIR / "ai_oracle_start_stage1_live_grid_model.py"
    log_dir = wave.ABCD_ROOT / "logs" / "stage1_livegrid_timeframes"
    log_dir.mkdir(parents=True, exist_ok=True)
    batch_started = time.time()
    rows: list[dict[str, Any]] = []

    for index, timeframe in enumerate(timeframes, start=1):
        run_id = child_run_id(args, timeframe)
        model_dir = wave.ABCD_ROOT / "model_registry" / run_id
        out_log = log_dir / f"{run_id}.out.log"
        err_log = log_dir / f"{run_id}.err.log"
        if model_dir.exists() and not args.replace_run:
            print(f"[{index}/{len(timeframes)}] {timeframe}: skip existing {run_id}", flush=True)
            meta = load_metadata(run_id)
            rows.append(
                {
                    "timeframe": timeframe,
                    "run_id": run_id,
                    "status": "skipped_existing",
                    "selected_valid_summary": meta.get("selected_valid_summary"),
                }
            )
            continue

        command = [
            sys.executable,
            str(script_path),
            "--roots",
            args.roots,
            "--timeframe",
            timeframe,
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
            "--base-negatives-per-symbol",
            str(args.base_negatives_per_symbol),
            "--min-oracle-recall",
            str(args.min_oracle_recall),
            "--max-picks-per-oracle",
            str(args.max_picks_per_oracle),
        ]
        oracle_id = oracle_run_id(args, timeframe)
        if oracle_id:
            command.extend(["--oracle-run-id", oracle_id])
        if args.symbols:
            command.extend(["--symbols", args.symbols])
        if args.replace_run:
            command.append("--replace-run")

        print(f"[{index}/{len(timeframes)}] {timeframe}: start {run_id}", flush=True)
        started = time.time()
        with out_log.open("w", encoding="utf-8") as stdout, err_log.open("w", encoding="utf-8") as stderr:
            result = subprocess.run(command, cwd=str(wave.ABCD_ROOT.parents[0]), stdout=stdout, stderr=stderr)
        elapsed = time.time() - started
        status = "ok" if result.returncode == 0 else f"failed:{result.returncode}"
        print(f"[{index}/{len(timeframes)}] {timeframe}: {status} elapsed={elapsed/60:.1f}m", flush=True)
        meta = load_metadata(run_id)
        rows.append(
            {
                "timeframe": timeframe,
                "run_id": run_id,
                "status": status,
                "elapsed_seconds": elapsed,
                "selected_valid_summary": meta.get("selected_valid_summary"),
                "out_log": str(out_log),
                "err_log": str(err_log),
            }
        )
        summary_path = log_dir / f"{args.run_prefix}-{args.roots.replace(',', '_')}-batch-summary.json"
        summary_path.write_text(
            json.dumps(
                {
                    "roots": args.roots,
                    "symbols": args.symbols,
                    "train_years": args.train_years,
                    "threshold_year": args.threshold_year,
                    "valid_year": args.valid_year,
                    "oracle_run_id_template": args.oracle_run_id_template,
                    "elapsed_seconds": time.time() - batch_started,
                    "runs": rows,
                },
                indent=2,
            ),
            encoding="utf-8",
        )
        if result.returncode != 0 and not args.continue_on_error:
            return result.returncode

    print(f"Done. elapsed={(time.time() - batch_started)/60:.1f}m", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
