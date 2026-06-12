#!/usr/bin/env python3
"""
Run all-timeframe Stage 1 live-grid training root by root.

For each root this wrapper first rebuilds derived candles from the 1m source,
then runs run_oracle_start_stage1_live_grid_timeframes.py. It is intentionally
sequential so the user can stop between roots and inspect results.
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

import ai_wave_rider_research as wave
import run_oracle_start_stage1_live_grid_timeframes as tf_runner


DERIVED_TIMEFRAMES = "2m,3m,4m,5m,6m,7m,8m,9m,10m,11m,12m,13m,14m,15m,30m,1h,4h,12h,1d"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--roots", required=True, help="Comma-separated root symbols, run sequentially.")
    parser.add_argument("--timeframes", default=tf_runner.DEFAULT_TIMEFRAMES)
    parser.add_argument("--train-years", default="2024")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--start-ts", default="2024-01-01T00:00:00")
    parser.add_argument("--end-ts", default="2027-01-01T00:00:00")
    parser.add_argument("--run-prefix", default="aicw-oracle-start-livegrid-alltf-v1")
    parser.add_argument(
        "--oracle-run-id-template",
        default="",
        help="Optional per-timeframe oracle template passed to the timeframe trainer.",
    )
    parser.add_argument("--iterations", type=int, default=650)
    parser.add_argument("--positive-pre-bars", type=int, default=0)
    parser.add_argument("--positive-post-bars", type=int, default=2)
    parser.add_argument("--negative-ratio", type=float, default=8.0)
    parser.add_argument("--replace-run", action="store_true")
    parser.add_argument("--continue-on-error", action="store_true")
    parser.add_argument("--skip-rebuild", action="store_true")
    return parser.parse_args()


def parse_roots(raw: str) -> list[str]:
    roots = [part.strip().upper() for part in str(raw or "").split(",") if part.strip()]
    if not roots:
        raise ValueError("At least one root is required")
    return roots


def run_command(command: list[str], out_log: Path, err_log: Path) -> int:
    out_log.parent.mkdir(parents=True, exist_ok=True)
    with out_log.open("w", encoding="utf-8") as stdout, err_log.open("w", encoding="utf-8") as stderr:
        result = subprocess.run(command, cwd=str(wave.ABCD_ROOT.parents[0]), stdout=stdout, stderr=stderr)
    return int(result.returncode)


def main() -> int:
    args = parse_args()
    roots = parse_roots(args.roots)
    log_dir = wave.ABCD_ROOT / "logs" / "stage1_livegrid_roots"
    log_dir.mkdir(parents=True, exist_ok=True)
    summary_path = log_dir / f"{args.run_prefix}-root-batch-summary.json"
    rows: list[dict[str, Any]] = []
    started_all = time.time()

    rebuild_script = SCRIPT_DIR / "rebuild_futures_timeframes_for_window.py"
    timeframe_script = SCRIPT_DIR / "run_oracle_start_stage1_live_grid_timeframes.py"

    for index, root in enumerate(roots, start=1):
        print(f"[root {index}/{len(roots)}] {root}: start", flush=True)
        started = time.time()
        rebuild_status = "skipped"
        if not args.skip_rebuild:
            rebuild_command = [
                sys.executable,
                str(rebuild_script),
                "--root",
                root,
                "--start-ts",
                args.start_ts,
                "--end-ts",
                args.end_ts,
                "--timeframes",
                DERIVED_TIMEFRAMES,
            ]
            rebuild_code = run_command(
                rebuild_command,
                log_dir / f"{root.lower()}-rebuild.out.log",
                log_dir / f"{root.lower()}-rebuild.err.log",
            )
            rebuild_status = "ok" if rebuild_code == 0 else f"failed:{rebuild_code}"
            print(f"[root {index}/{len(roots)}] {root}: rebuild {rebuild_status}", flush=True)
            if rebuild_code != 0 and not args.continue_on_error:
                return rebuild_code

        train_command = [
            sys.executable,
            str(timeframe_script),
            "--roots",
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
            "--oracle-run-id-template",
            args.oracle_run_id_template,
            "--iterations",
            str(args.iterations),
            "--positive-pre-bars",
            str(args.positive_pre_bars),
            "--positive-post-bars",
            str(args.positive_post_bars),
            "--negative-ratio",
            str(args.negative_ratio),
            "--continue-on-error",
        ]
        if args.replace_run:
            train_command.append("--replace-run")
        train_code = run_command(
            train_command,
            log_dir / f"{root.lower()}-alltf.out.log",
            log_dir / f"{root.lower()}-alltf.err.log",
        )
        train_status = "ok" if train_code == 0 else f"failed:{train_code}"
        elapsed = time.time() - started
        print(f"[root {index}/{len(roots)}] {root}: train {train_status} elapsed={elapsed/60:.1f}m", flush=True)
        rows.append(
            {
                "root": root,
                "rebuild_status": rebuild_status,
                "train_status": train_status,
                "elapsed_seconds": elapsed,
            }
        )
        summary_path.write_text(
            json.dumps(
                {
                    "roots": roots,
                    "timeframes": args.timeframes,
                    "train_years": args.train_years,
                    "threshold_year": args.threshold_year,
                    "valid_year": args.valid_year,
                    "oracle_run_id_template": args.oracle_run_id_template,
                    "elapsed_seconds": time.time() - started_all,
                    "runs": rows,
                },
                indent=2,
            ),
            encoding="utf-8",
        )
        if train_code != 0 and not args.continue_on_error:
            return train_code

    print(f"Done. elapsed={(time.time() - started_all)/60:.1f}m", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
