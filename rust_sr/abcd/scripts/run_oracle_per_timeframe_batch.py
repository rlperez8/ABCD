#!/usr/bin/env python3
"""
Build separate hindsight trend-oracle runs for a list of timeframes.

Each timeframe gets its own run_id through build_oracle_perfect_trend_trades.py.
Those runs can then become the target labels for timeframe-specific Stage 1
specialists.
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


DEFAULT_TIMEFRAMES = "1m,2m,3m,4m,5m,6m,7m,8m,9m,10m,11m,12m,13m,14m,15m,30m"
DEFAULT_ROOTS = "ES,RTY,YM,EMD,CL,RB,HO,NG,GF,LE,NKD,NQ"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-prefix", default="oracle-perfect-trends-tfspec-v1")
    parser.add_argument("--timeframes", default=DEFAULT_TIMEFRAMES)
    parser.add_argument("--years", default="2024,2025,2026")
    parser.add_argument("--roots", default=DEFAULT_ROOTS)
    parser.add_argument("--replace-run", action="store_true")
    parser.add_argument("--continue-on-error", action="store_true")
    parser.add_argument("--limit-timeframes", type=int, default=0)
    parser.add_argument("--limit-symbols", type=int, default=0)
    parser.add_argument("--batch-size", type=int, default=1000)
    parser.add_argument("--pivot-window", type=int, default=4)
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--risk-atr-mult", type=float, default=1.0)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--slippage-entry-ticks", type=float, default=3.0)
    parser.add_argument("--slippage-exit-ticks", type=float, default=3.0)
    parser.add_argument("--min-result-r", type=float, default=2.0)
    parser.add_argument("--max-adverse-r", type=float, default=0.75)
    parser.add_argument("--min-efficiency", type=float, default=0.18)
    parser.add_argument("--min-bars", type=int, default=4)
    parser.add_argument("--max-bars", type=int, default=240)
    return parser.parse_args()


def parse_list(raw: str) -> list[str]:
    return [part.strip() for part in str(raw or "").split(",") if part.strip()]


def run_id_for(prefix: str, timeframe: str, years: str) -> str:
    parsed_years = [int(part.strip()) for part in years.split(",") if part.strip()]
    if len(parsed_years) == 1:
        suffix = str(parsed_years[0])
    else:
        suffix = f"{min(parsed_years)}_{max(parsed_years)}"
    run_id = f"{prefix}-{timeframe}-{suffix}"
    if len(run_id) <= 64:
        return run_id
    raise ValueError(f"Run id too long: {run_id}")


def main() -> int:
    args = parse_args()
    timeframes = parse_list(args.timeframes)
    if args.limit_timeframes and len(timeframes) > int(args.limit_timeframes):
        timeframes = timeframes[: int(args.limit_timeframes)]
    if not timeframes:
        raise ValueError("No timeframes requested.")

    log_dir = wave.ABCD_ROOT / "logs" / "oracle_per_timeframe"
    log_dir.mkdir(parents=True, exist_ok=True)
    summary_path = log_dir / f"{args.run_prefix}-batch-summary.json"
    builder = SCRIPT_DIR / "build_oracle_perfect_trend_trades.py"
    results: list[dict[str, Any]] = []

    print(
        f"Oracle per-timeframe batch timeframes={len(timeframes)} years={args.years} roots={args.roots}",
        flush=True,
    )
    started = time.time()
    for index, timeframe in enumerate(timeframes, start=1):
        run_id = run_id_for(str(args.run_prefix), timeframe, str(args.years))
        out_log = log_dir / f"{run_id}.out.log"
        err_log = log_dir / f"{run_id}.err.log"
        command = [
            sys.executable,
            str(builder),
            "--run-id",
            run_id,
            "--run-prefix",
            str(args.run_prefix),
            "--timeframe",
            timeframe,
            "--years",
            str(args.years),
            "--roots",
            str(args.roots),
            "--batch-size",
            str(int(args.batch_size)),
            "--pivot-window",
            str(int(args.pivot_window)),
            "--atr-period",
            str(int(args.atr_period)),
            "--risk-atr-mult",
            str(float(args.risk_atr_mult)),
            "--min-risk-ticks",
            str(float(args.min_risk_ticks)),
            "--slippage-entry-ticks",
            str(float(args.slippage_entry_ticks)),
            "--slippage-exit-ticks",
            str(float(args.slippage_exit_ticks)),
            "--min-result-r",
            str(float(args.min_result_r)),
            "--max-adverse-r",
            str(float(args.max_adverse_r)),
            "--min-efficiency",
            str(float(args.min_efficiency)),
            "--min-bars",
            str(int(args.min_bars)),
            "--max-bars",
            str(int(args.max_bars)),
        ]
        if args.replace_run:
            command.append("--replace-run")
        if int(args.limit_symbols or 0) > 0:
            command.extend(["--limit-symbols", str(int(args.limit_symbols))])

        print(f"[{index}/{len(timeframes)}] {run_id}: building", flush=True)
        item_started = time.time()
        with out_log.open("w", encoding="utf-8") as stdout, err_log.open("w", encoding="utf-8") as stderr:
            completed = subprocess.run(command, cwd=str(wave.ABCD_ROOT), stdout=stdout, stderr=stderr)
        elapsed = round(time.time() - item_started, 2)
        row = {
            "run_id": run_id,
            "timeframe": timeframe,
            "status": "ok" if completed.returncode == 0 else "failed",
            "returncode": int(completed.returncode),
            "elapsed_seconds": elapsed,
            "out_log": str(out_log),
            "err_log": str(err_log),
        }
        results.append(row)
        summary_path.write_text(json.dumps(results, indent=2), encoding="utf-8")
        print(f"[{index}/{len(timeframes)}] {run_id}: {row['status']} elapsed={elapsed}s", flush=True)
        if completed.returncode != 0 and not args.continue_on_error:
            return int(completed.returncode)

    print(f"Done. elapsed={round(time.time() - started, 2)}s summary={summary_path}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
