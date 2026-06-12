#!/usr/bin/env python3
"""Run the visual CNN oracle-start tower for Level 1, Level 2, and Level 3.

Level 1 runs one NQ/root timeframe specialist at a time. Level 2 runs one
root-all-timeframe specialist. Level 3 runs one all-root/all-timeframe
specialist. Each child run writes its own log and model_registry folder.
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

import ai_oracle_start_cnn_specialist as cnn
import ai_oracle_trend_start_model as start_model
import ai_wave_rider_research as wave


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--timeframes", default=cnn.DEFAULT_TIMEFRAMES)
    parser.add_argument("--level1-roots", default="NQ")
    parser.add_argument("--level2-roots", default="NQ")
    parser.add_argument("--level3-roots", default="", help="Comma-separated roots. Empty means all roots.")
    parser.add_argument("--oracle-run-id-template", default="oracle-perfect-trends-tfspec-v1-{timeframe}-2024_2026")
    parser.add_argument("--run-prefix", default="aicw-oracle-start-cnn-tower-v1")
    parser.add_argument("--train-years", default="2024")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--lookback-bars", type=int, default=64)
    parser.add_argument("--height", type=int, default=40)
    parser.add_argument("--width", type=int, default=40)
    parser.add_argument("--min-candles", type=int, default=36)
    parser.add_argument("--max-train-rows-per-tf", type=int, default=1000)
    parser.add_argument("--max-eval-rows-per-tf", type=int, default=1000)
    parser.add_argument("--max-combined-train-rows", type=int, default=60000)
    parser.add_argument("--max-combined-eval-rows", type=int, default=60000)
    parser.add_argument("--epochs", type=int, default=6)
    parser.add_argument("--batch-size", type=int, default=192)
    parser.add_argument("--learning-rate", type=float, default=0.001)
    parser.add_argument("--weight-decay", type=float, default=0.01)
    parser.add_argument("--dropout", type=float, default=0.20)
    parser.add_argument("--patience", type=int, default=4)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--min-threshold-picks", type=int, default=100)
    parser.add_argument("--replace-run", action="store_true")
    parser.add_argument("--continue-on-error", action="store_true")
    return parser.parse_args()


def parse_roots(raw: str) -> list[str]:
    return start_model.parse_list(raw)


def run_id_for(
    args: argparse.Namespace,
    level: str,
    roots: str,
    timeframes: str,
) -> str:
    child = argparse.Namespace(
        level=level,
        roots=roots,
        timeframes=timeframes,
        run_prefix=args.run_prefix,
        train_years=args.train_years,
        valid_year=args.valid_year,
    )
    return cnn.run_id(child)


def child_command(args: argparse.Namespace, level: str, roots: str, timeframes: str) -> list[str]:
    command = [
        sys.executable,
        str(SCRIPT_DIR / "ai_oracle_start_cnn_specialist.py"),
        "--level",
        level,
        "--roots",
        roots,
        "--timeframes",
        timeframes,
        "--oracle-run-id-template",
        args.oracle_run_id_template,
        "--run-prefix",
        args.run_prefix,
        "--train-years",
        args.train_years,
        "--threshold-year",
        str(args.threshold_year),
        "--valid-year",
        str(args.valid_year),
        "--lookback-bars",
        str(args.lookback_bars),
        "--height",
        str(args.height),
        "--width",
        str(args.width),
        "--min-candles",
        str(args.min_candles),
        "--max-train-rows-per-tf",
        str(args.max_train_rows_per_tf),
        "--max-eval-rows-per-tf",
        str(args.max_eval_rows_per_tf),
        "--max-combined-train-rows",
        str(args.max_combined_train_rows),
        "--max-combined-eval-rows",
        str(args.max_combined_eval_rows),
        "--epochs",
        str(args.epochs),
        "--batch-size",
        str(args.batch_size),
        "--learning-rate",
        str(args.learning_rate),
        "--weight-decay",
        str(args.weight_decay),
        "--dropout",
        str(args.dropout),
        "--patience",
        str(args.patience),
        "--random-seed",
        str(args.random_seed),
        "--min-threshold-picks",
        str(args.min_threshold_picks),
    ]
    if args.replace_run:
        command.append("--replace-run")
    return command


def load_summary(run_id: str) -> dict[str, Any]:
    metadata_path = wave.ABCD_ROOT / "model_registry" / run_id / "metadata.json"
    if not metadata_path.exists():
        return {}
    try:
        metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {}
    metrics = metadata.get("metrics") or {}
    valid = metrics.get(str(metadata.get("valid_year") or "")) or metrics.get("2026") or {}
    return {
        "threshold": metadata.get("selected_visual_threshold"),
        "valid_rows": valid.get("rows"),
        "valid_picks": valid.get("picks"),
        "valid_precision": valid.get("precision"),
        "valid_recall": valid.get("recall"),
        "valid_missed_pct": None if valid.get("recall") is None else 1.0 - float(valid.get("recall")),
    }


def run_child(args: argparse.Namespace, level: str, roots: str, timeframes: str, index: int, total: int) -> dict[str, Any]:
    run_id = run_id_for(args, level, roots, timeframes)
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id
    log_dir = wave.ABCD_ROOT / "logs" / "oracle_start_cnn_levels"
    log_dir.mkdir(parents=True, exist_ok=True)
    safe_run_id = run_id.replace("|", "_")
    out_log = log_dir / f"{safe_run_id}.out.log"
    err_log = log_dir / f"{safe_run_id}.err.log"

    if model_dir.exists() and not args.replace_run:
        print(f"[{index}/{total}] {level} roots={roots or 'ALL'} tf={timeframes}: skip {run_id}", flush=True)
        return {
            "level": level,
            "roots": roots or "ALL",
            "timeframes": timeframes,
            "run_id": run_id,
            "status": "skipped_existing",
            **load_summary(run_id),
        }

    command = child_command(args, level, roots, timeframes)
    print(f"[{index}/{total}] {level} roots={roots or 'ALL'} tf={timeframes}: start {run_id}", flush=True)
    started = time.time()
    with out_log.open("w", encoding="utf-8") as stdout, err_log.open("w", encoding="utf-8") as stderr:
        completed = subprocess.run(command, cwd=str(wave.ABCD_ROOT.parents[1]), stdout=stdout, stderr=stderr)
    elapsed = time.time() - started
    status = "ok" if completed.returncode == 0 else f"failed:{completed.returncode}"
    row = {
        "level": level,
        "roots": roots or "ALL",
        "timeframes": timeframes,
        "run_id": run_id,
        "status": status,
        "elapsed_seconds": elapsed,
        "out_log": str(out_log),
        "err_log": str(err_log),
        **load_summary(run_id),
    }
    print(
        f"[{index}/{total}] {level} roots={roots or 'ALL'} tf={timeframes}: {status} "
        f"elapsed={elapsed/60:.1f}m precision={row.get('valid_precision')} recall={row.get('valid_recall')}",
        flush=True,
    )
    return row


def main() -> int:
    args = parse_args()
    timeframes = cnn.parse_timeframes(args.timeframes)
    runs: list[tuple[str, str, str]] = []

    for root in parse_roots(args.level1_roots):
        for timeframe in timeframes:
            runs.append(("l1", root, timeframe))
    for root in parse_roots(args.level2_roots):
        runs.append(("l2", root, ",".join(timeframes)))
    runs.append(("l3", args.level3_roots, ",".join(timeframes)))

    log_dir = wave.ABCD_ROOT / "logs" / "oracle_start_cnn_levels"
    log_dir.mkdir(parents=True, exist_ok=True)
    summary_path = log_dir / f"{args.run_prefix}-summary.json"
    rows: list[dict[str, Any]] = []
    started_all = time.time()

    for index, (level, roots, timeframes_value) in enumerate(runs, start=1):
        row = run_child(args, level, roots, timeframes_value, index, len(runs))
        rows.append(row)
        summary_path.write_text(
            json.dumps(
                {
                    "run_prefix": args.run_prefix,
                    "train_years": args.train_years,
                    "threshold_year": args.threshold_year,
                    "valid_year": args.valid_year,
                    "timeframes": timeframes,
                    "elapsed_seconds": time.time() - started_all,
                    "runs": rows,
                },
                indent=2,
            ),
            encoding="utf-8",
        )
        if str(row.get("status")).startswith("failed") and not args.continue_on_error:
            return 1

    print(f"Done. elapsed={(time.time() - started_all)/60:.1f}m summary={summary_path}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
