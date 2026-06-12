#!/usr/bin/env python3
"""Run the full simplified 2m oracle-start mainline.

This runs:

1. Level 1 CatBoost / LightGBM / XGBoost all-root 2m models.
2. Level 2 CatBoost manager over those three Level 1 opinion streams.

The script is intentionally thin. It delegates to the Level 1 runner and Level
2 manager so the two pieces can still be run independently while giving us a
single long-running entrypoint for all-symbol jobs.
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

import ai_oracle_start_level2_three_l1_manager as level2
import ai_wave_rider_research as wave
import run_oracle_start_2m_mainline_level1 as level1


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--timeframe", default="2m")
    parser.add_argument("--roots", default="", help="Comma-separated roots. Empty means all roots.")
    parser.add_argument("--oracle-run-id", default=level1.DEFAULT_ORACLE_RUN)
    parser.add_argument("--train-years", default="2024")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--max-train-rows", type=int, default=750_000)
    parser.add_argument("--level2-max-train-rows", type=int, default=500_000)
    parser.add_argument("--iterations", type=int, default=650)
    parser.add_argument("--level2-iterations", type=int, default=550)
    parser.add_argument("--limit-symbols", type=int, default=0)
    parser.add_argument("--candidate-set-id", default="")
    parser.add_argument("--candidate-cache-dir", default="")
    parser.add_argument("--skip-candidate-cache-build", action="store_true")
    parser.add_argument("--replace-candidate-cache", action="store_true")
    parser.add_argument("--replace-run", action="store_true")
    parser.add_argument("--skip-level1-existing", action="store_true")
    parser.add_argument("--status-file", default="")
    return parser.parse_args()


def write_status(args: argparse.Namespace, payload: dict[str, Any]) -> None:
    if not str(args.status_file or "").strip():
        return
    path = Path(args.status_file).expanduser().resolve()
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def run_stage(name: str, command: list[str], args: argparse.Namespace) -> int:
    print(f"{name} command: {' '.join(command)}", flush=True)
    write_status(
        args,
        {
            "stage": name,
            "status": "running",
            "updated_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
            "command": command,
        },
    )
    started = time.time()
    result = subprocess.run(command, cwd=str(wave.ABCD_ROOT))
    elapsed = round(time.time() - started, 2)
    if result.returncode:
        write_status(
            args,
            {
                "stage": name,
                "status": "failed",
                "exit_code": int(result.returncode),
                "elapsed_seconds": elapsed,
                "updated_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
                "command": command,
            },
        )
        return int(result.returncode)
    write_status(
        args,
        {
            "stage": name,
            "status": "ok",
            "elapsed_seconds": elapsed,
            "updated_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
            "command": command,
        },
    )
    return 0


def level1_command(args: argparse.Namespace) -> list[str]:
    command = [
        sys.executable,
        str(SCRIPT_DIR / "run_oracle_start_2m_mainline_level1.py"),
        "--timeframe",
        args.timeframe,
        "--roots",
        args.roots,
        "--oracle-run-id",
        args.oracle_run_id,
        "--train-years",
        args.train_years,
        "--threshold-year",
        str(int(args.threshold_year)),
        "--valid-year",
        str(int(args.valid_year)),
        "--max-train-rows",
        str(int(args.max_train_rows)),
        "--iterations",
        str(int(args.iterations)),
        "--limit-symbols",
        str(int(args.limit_symbols)),
        "--read-candidate-cache",
    ]
    if str(args.candidate_set_id or "").strip():
        command.extend(["--candidate-set-id", str(args.candidate_set_id)])
    if str(args.candidate_cache_dir or "").strip():
        command.extend(["--candidate-cache-dir", str(args.candidate_cache_dir)])
    if args.replace_run:
        command.append("--replace-run")
    if args.skip_level1_existing:
        command.append("--skip-existing")
    return command


def level2_command(args: argparse.Namespace) -> list[str]:
    command = [
        sys.executable,
        str(SCRIPT_DIR / "ai_oracle_start_level2_three_l1_manager.py"),
        "--timeframe",
        args.timeframe,
        "--roots",
        args.roots,
        "--oracle-run-id",
        args.oracle_run_id,
        "--level1-train-years",
        args.train_years,
        "--level1-valid-year",
        str(int(args.valid_year)),
        "--train-year",
        str(int(args.threshold_year)),
        "--valid-year",
        str(int(args.valid_year)),
        "--max-train-rows",
        str(int(args.level2_max_train_rows)),
        "--iterations",
        str(int(args.level2_iterations)),
        "--limit-symbols",
        str(int(args.limit_symbols)),
        "--write-opinion-cache",
        "--read-candidate-cache",
    ]
    if str(args.candidate_set_id or "").strip():
        command.extend(["--candidate-set-id", str(args.candidate_set_id)])
    if str(args.candidate_cache_dir or "").strip():
        command.extend(["--candidate-cache-dir", str(args.candidate_cache_dir)])
    if args.replace_run:
        command.append("--replace-run")
    return command


def candidate_cache_command(args: argparse.Namespace) -> list[str]:
    command = [
        sys.executable,
        str(SCRIPT_DIR / "ai_oracle_start_stage1_live_grid_model.py"),
        "--run-prefix",
        "aicw-os-candidate-cache-build-v1",
        "--timeframe",
        args.timeframe,
        "--roots",
        args.roots,
        "--oracle-run-id",
        args.oracle_run_id,
        "--train-years",
        args.train_years,
        "--threshold-year",
        str(int(args.threshold_year)),
        "--valid-year",
        str(int(args.valid_year)),
        "--limit-symbols",
        str(int(args.limit_symbols)),
        "--read-candidate-cache",
        "--write-candidate-cache",
        "--only-build-candidate-cache",
    ]
    if str(args.candidate_set_id or "").strip():
        command.extend(["--candidate-set-id", str(args.candidate_set_id)])
    if str(args.candidate_cache_dir or "").strip():
        command.extend(["--candidate-cache-dir", str(args.candidate_cache_dir)])
    if args.replace_candidate_cache:
        command.append("--replace-candidate-cache")
    if args.replace_run:
        command.append("--replace-run")
    return command


def main() -> int:
    args = parse_args()
    write_status(
        args,
        {
            "stage": "start",
            "status": "running",
            "started_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
            "design": "level1_all_roots_2m_per_engine__level2_three_engine_manager",
            "roots": level2.parse_roots(args.roots),
            "roots_empty_means_all": True,
            "timeframe": args.timeframe,
        },
    )
    if not args.skip_candidate_cache_build:
        code = run_stage("candidate_cache", candidate_cache_command(args), args)
        if code:
            return code
    code = run_stage("level1", level1_command(args), args)
    if code:
        return code
    code = run_stage("level2", level2_command(args), args)
    if code:
        return code
    write_status(
        args,
        {
            "stage": "complete",
            "status": "ok",
            "finished_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
            "design": "level1_all_roots_2m_per_engine__level2_three_engine_manager",
            "roots": level2.parse_roots(args.roots),
            "roots_empty_means_all": True,
            "timeframe": args.timeframe,
        },
    )
    print("Full simplified 2m oracle-start mainline completed.", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
