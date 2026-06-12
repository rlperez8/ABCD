#!/usr/bin/env python3
"""Run the xgboost oracle-start level system sequentially and resumably."""

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

import ai_oracle_start_xgboost_level4_manager as l4
import ai_oracle_start_stage1_global_alltf_model as global_alltf
import ai_oracle_start_stage1_live_grid_model as stage1
import ai_oracle_start_stage1_root_alltf_model as root_alltf
import ai_wave_rider_research as wave


DEFAULT_TIMEFRAMES = "1m,2m,3m,4m,5m,6m,7m,8m,9m,10m,11m,12m,13m,14m,15m,30m"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default="NQ")
    parser.add_argument("--roots", default="", help="Optional roots for Level 3. Defaults to --root.")
    parser.add_argument("--timeframes", default=DEFAULT_TIMEFRAMES)
    parser.add_argument("--train-years", default="2024")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--l1-prefix", default="aicw-oracle-start-xgboost-tfspec-v1")
    parser.add_argument("--l2-prefix", default="aicw-oracle-start-xgboost-root-alltf-v1")
    parser.add_argument("--l3-prefix", default="aicw-oracle-start-xgboost-global-alltf-v1")
    parser.add_argument("--l4-prefix", default="aicw-oracle-start-xgboost-level4-manager-v1")
    parser.add_argument("--oracle-run-id-template", default="oracle-perfect-trends-tfspec-v1-{timeframe}-2024_2026")
    parser.add_argument("--iterations", type=int, default=650)
    parser.add_argument("--l4-iterations", type=int, default=550)
    parser.add_argument("--depth", type=int, default=6)
    parser.add_argument("--learning-rate", type=float, default=0.045)
    parser.add_argument("--l2-leaf-reg", type=float, default=10.0)
    parser.add_argument("--max-l3-train-rows", type=int, default=750_000)
    parser.add_argument("--max-l4-train-rows", type=int, default=500_000)
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


def run_path(run_id: str) -> Path:
    return wave.ABCD_ROOT / "model_registry" / run_id


def metadata(run_id: str) -> dict[str, Any]:
    path = run_path(run_id) / "metadata.json"
    if not path.exists():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


def replace_flag(args: argparse.Namespace) -> list[str]:
    return ["--replace-run"] if args.replace_run else []


def l1_run_id(args: argparse.Namespace, timeframe: str) -> str:
    child = argparse.Namespace(
        run_prefix=args.l1_prefix,
        timeframe=timeframe,
        roots=args.root,
        train_years=args.train_years,
        valid_year=args.valid_year,
    )
    return stage1.run_id(child)


def l2_run_id(args: argparse.Namespace) -> str:
    child = argparse.Namespace(run_prefix=args.l2_prefix, root=args.root, train_years=args.train_years, valid_year=args.valid_year)
    return root_alltf.run_id(child)


def l3_run_id(args: argparse.Namespace) -> str:
    roots = args.roots or args.root
    child = argparse.Namespace(run_prefix=args.l3_prefix, roots=roots, train_years=args.train_years, valid_year=args.valid_year)
    return global_alltf.run_id(child)


def l4_run_id(args: argparse.Namespace) -> str:
    child = argparse.Namespace(run_prefix=args.l4_prefix, root=args.root, train_year=args.threshold_year, valid_year=args.valid_year)
    return l4.run_id(child)


def run_step(
    name: str,
    run_id: str,
    command: list[str],
    log_dir: Path,
    args: argparse.Namespace,
) -> dict[str, Any]:
    out_log = log_dir / f"{run_id}.out.log"
    err_log = log_dir / f"{run_id}.err.log"
    if run_path(run_id).exists() and not args.replace_run:
        print(f"{name}: skip existing {run_id}", flush=True)
        return {"name": name, "run_id": run_id, "status": "skipped_existing", "metadata": metadata(run_id)}
    print(f"{name}: start {run_id}", flush=True)
    started = time.time()
    with out_log.open("w", encoding="utf-8") as stdout, err_log.open("w", encoding="utf-8") as stderr:
        result = subprocess.run(command, cwd=str(wave.ABCD_ROOT.parents[0]), stdout=stdout, stderr=stderr)
    elapsed = time.time() - started
    status = "ok" if result.returncode == 0 else f"failed:{result.returncode}"
    print(f"{name}: {status} elapsed={elapsed/60:.1f}m", flush=True)
    payload = {
        "name": name,
        "run_id": run_id,
        "status": status,
        "elapsed_seconds": elapsed,
        "out_log": str(out_log),
        "err_log": str(err_log),
        "metadata": metadata(run_id),
    }
    if result.returncode != 0 and not args.continue_on_error:
        raise SystemExit(result.returncode)
    return payload


def main() -> int:
    args = parse_args()
    timeframes = parse_timeframes(args.timeframes)
    roots_for_l3 = args.roots or args.root
    log_dir = wave.ABCD_ROOT / "logs" / "oracle_start_xgboost_levels"
    log_dir.mkdir(parents=True, exist_ok=True)
    summary_path = log_dir / f"{args.l4_prefix}-{args.root}-summary.json"
    started = time.time()
    rows: list[dict[str, Any]] = []

    l1_script = SCRIPT_DIR / "ai_oracle_start_xgboost_stage1_live_grid_model.py"
    for index, timeframe in enumerate(timeframes, start=1):
        rid = l1_run_id(args, timeframe)
        rows.append(
            run_step(
                f"L1 {index}/{len(timeframes)} {timeframe}",
                rid,
                [
                    sys.executable,
                    str(l1_script),
                    "--roots",
                    args.root,
                    "--timeframe",
                    timeframe,
                    "--train-years",
                    args.train_years,
                    "--threshold-year",
                    str(args.threshold_year),
                    "--valid-year",
                    str(args.valid_year),
                    "--run-prefix",
                    args.l1_prefix,
                    "--oracle-run-id",
                    args.oracle_run_id_template.format(timeframe=timeframe),
                    "--iterations",
                    str(args.iterations),
                    "--depth",
                    str(args.depth),
                    "--learning-rate",
                    str(args.learning_rate),
                    "--l2-leaf-reg",
                    str(args.l2_leaf_reg),
                    "--positive-pre-bars",
                    "0",
                    "--positive-post-bars",
                    "2",
                    "--min-oracle-recall",
                    str(args.min_oracle_recall),
                    "--max-picks-per-oracle",
                    str(args.max_picks_per_oracle),
                    *replace_flag(args),
                ],
                log_dir,
                args,
            )
        )
        summary_path.write_text(json.dumps({"elapsed_seconds": time.time() - started, "runs": rows}, indent=2), encoding="utf-8")

    l2_rid = l2_run_id(args)
    rows.append(
        run_step(
            "L2 root-alltf",
            l2_rid,
            [
                sys.executable,
                str(SCRIPT_DIR / "ai_oracle_start_xgboost_root_alltf_model.py"),
                "--root",
                args.root,
                "--timeframes",
                args.timeframes,
                "--train-years",
                args.train_years,
                "--threshold-year",
                str(args.threshold_year),
                "--valid-year",
                str(args.valid_year),
                "--run-prefix",
                args.l2_prefix,
                "--oracle-run-id-template",
                args.oracle_run_id_template,
                "--iterations",
                str(args.iterations),
                "--depth",
                str(args.depth),
                "--learning-rate",
                str(args.learning_rate),
                "--l2-leaf-reg",
                str(args.l2_leaf_reg),
                "--positive-pre-bars",
                "0",
                "--positive-post-bars",
                "2",
                "--min-oracle-recall",
                str(args.min_oracle_recall),
                "--max-picks-per-oracle",
                str(args.max_picks_per_oracle),
                *replace_flag(args),
            ],
            log_dir,
            args,
        )
    )
    summary_path.write_text(json.dumps({"elapsed_seconds": time.time() - started, "runs": rows}, indent=2), encoding="utf-8")

    l3_rid = l3_run_id(args)
    rows.append(
        run_step(
            "L3 global-alltf",
            l3_rid,
            [
                sys.executable,
                str(SCRIPT_DIR / "ai_oracle_start_xgboost_global_alltf_model.py"),
                "--roots",
                roots_for_l3,
                "--timeframes",
                args.timeframes,
                "--train-years",
                args.train_years,
                "--threshold-year",
                str(args.threshold_year),
                "--valid-year",
                str(args.valid_year),
                "--run-prefix",
                args.l3_prefix,
                "--oracle-run-id-template",
                args.oracle_run_id_template,
                "--iterations",
                str(args.iterations),
                "--depth",
                str(args.depth),
                "--learning-rate",
                str(args.learning_rate),
                "--l2-leaf-reg",
                str(args.l2_leaf_reg),
                "--max-train-rows",
                str(args.max_l3_train_rows),
                "--positive-pre-bars",
                "0",
                "--positive-post-bars",
                "2",
                "--min-oracle-recall",
                str(args.min_oracle_recall),
                "--max-picks-per-oracle",
                str(args.max_picks_per_oracle),
                *replace_flag(args),
            ],
            log_dir,
            args,
        )
    )
    summary_path.write_text(json.dumps({"elapsed_seconds": time.time() - started, "runs": rows}, indent=2), encoding="utf-8")

    l4_rid = l4_run_id(args)
    rows.append(
        run_step(
            "L4 manager",
            l4_rid,
            [
                sys.executable,
                str(SCRIPT_DIR / "ai_oracle_start_xgboost_level4_manager.py"),
                "--root",
                args.root,
                "--timeframes",
                args.timeframes,
                "--train-year",
                str(args.threshold_year),
                "--valid-year",
                str(args.valid_year),
                "--level1-prefix",
                args.l1_prefix,
                "--level2-run-id",
                l2_rid,
                "--level3-run-id",
                l3_rid,
                "--run-prefix",
                args.l4_prefix,
                "--max-train-rows",
                str(args.max_l4_train_rows),
                "--iterations",
                str(args.l4_iterations),
                "--depth",
                str(args.depth),
                "--learning-rate",
                str(args.learning_rate),
                "--l2-leaf-reg",
                str(args.l2_leaf_reg),
                "--min-oracle-recall",
                str(args.min_oracle_recall),
                "--max-picks-per-oracle",
                str(args.max_picks_per_oracle),
                *replace_flag(args),
            ],
            log_dir,
            args,
        )
    )
    summary_path.write_text(
        json.dumps(
            {
                "root": args.root,
                "roots_for_l3": roots_for_l3,
                "timeframes": timeframes,
                "elapsed_seconds": time.time() - started,
                "runs": rows,
            },
            indent=2,
            default=str,
        ),
        encoding="utf-8",
    )
    print(f"Done. summary={summary_path}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())


