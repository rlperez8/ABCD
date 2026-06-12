#!/usr/bin/env python3
"""Run the simplified Level 1 oracle-start models.

New mainline design:

Level 1 = one all-root / one-timeframe model per tabular engine.

This wrapper runs the CatBoost, LightGBM, and XGBoost Stage 1 live-grid
trainers with the same 2m candle rows and same oracle label source. Passing an
empty --roots value means all roots available in the candle/oracle tables.
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

import ai_oracle_start_stage1_live_grid_model as cat_stage1
import ai_wave_rider_research as wave


DEFAULT_ORACLE_RUN = "oracle-perfect-trends-tfspec-v1-2m-2024_2026"
DEFAULT_ENGINES = "cat,light,xgb"
DEFAULT_CAT_PREFIX = "aicw-os-cat-l1-2m-v1"
DEFAULT_LIGHT_PREFIX = "aicw-os-light-l1-2m-v1"
DEFAULT_XGB_PREFIX = "aicw-os-xgb-l1-2m-v1"


ENGINE_CONFIG = {
    "cat": {
        "label": "CatBoost",
        "script": "ai_oracle_start_stage1_live_grid_model.py",
        "prefix_attr": "cat_prefix",
    },
    "light": {
        "label": "LightGBM",
        "script": "ai_oracle_start_lightgbm_stage1_live_grid_model.py",
        "prefix_attr": "light_prefix",
    },
    "xgb": {
        "label": "XGBoost",
        "script": "ai_oracle_start_xgboost_stage1_live_grid_model.py",
        "prefix_attr": "xgb_prefix",
    },
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engines", default=DEFAULT_ENGINES, help="Comma-separated engines: cat,light,xgb.")
    parser.add_argument("--timeframe", default="2m")
    parser.add_argument("--roots", default="", help="Comma-separated roots. Empty means all roots.")
    parser.add_argument("--symbols", default="")
    parser.add_argument("--oracle-run-id", default=DEFAULT_ORACLE_RUN)
    parser.add_argument("--train-years", default="2024")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--cat-prefix", default=DEFAULT_CAT_PREFIX)
    parser.add_argument("--light-prefix", default=DEFAULT_LIGHT_PREFIX)
    parser.add_argument("--xgb-prefix", default=DEFAULT_XGB_PREFIX)
    parser.add_argument("--positive-pre-bars", type=int, default=0)
    parser.add_argument("--positive-post-bars", type=int, default=2)
    parser.add_argument("--negative-exclusion-bars", type=int, default=8)
    parser.add_argument("--match-window-bars", type=int, default=4)
    parser.add_argument("--negative-ratio", type=float, default=8.0)
    parser.add_argument("--base-negatives-per-symbol", type=int, default=200)
    parser.add_argument("--max-train-rows", type=int, default=0)
    parser.add_argument("--cooldown-bars", type=int, default=8)
    parser.add_argument("--min-oracle-recall", type=float, default=0.90)
    parser.add_argument("--max-picks-per-oracle", type=float, default=5.0)
    parser.add_argument("--thresholds", default="0.25,0.30,0.35,0.40,0.45,0.50,0.55,0.60,0.65,0.70,0.75,0.80,0.85,0.90")
    parser.add_argument("--iterations", type=int, default=650)
    parser.add_argument("--depth", type=int, default=6)
    parser.add_argument("--learning-rate", type=float, default=0.045)
    parser.add_argument("--l2-leaf-reg", type=float, default=10.0)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--limit-symbols", type=int, default=0)
    parser.add_argument("--candidate-set-id", default="")
    parser.add_argument("--candidate-cache-dir", default="")
    parser.add_argument("--read-candidate-cache", action="store_true")
    parser.add_argument("--write-candidate-cache", action="store_true")
    parser.add_argument("--replace-candidate-cache", action="store_true")
    parser.add_argument("--save-row-csvs", action="store_true")
    parser.add_argument("--replace-run", action="store_true")
    parser.add_argument("--skip-existing", action="store_true")
    parser.add_argument("--dry-run", action="store_true")

    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=240.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    return parser.parse_args()


def parse_engines(raw: str) -> list[str]:
    engines = [part.strip().lower() for part in str(raw or "").split(",") if part.strip()]
    unknown = [engine for engine in engines if engine not in ENGINE_CONFIG]
    if unknown:
        raise ValueError(f"Unknown engine(s): {', '.join(unknown)}")
    if not engines:
        raise ValueError("At least one engine is required.")
    return engines


def stage1_run_id(prefix: str, args: argparse.Namespace) -> str:
    child = argparse.Namespace(
        run_prefix=prefix,
        timeframe=args.timeframe,
        roots=args.roots,
        train_years=args.train_years,
        valid_year=args.valid_year,
    )
    return cat_stage1.run_id(child)


def command_for(engine: str, args: argparse.Namespace) -> tuple[str, list[str]]:
    config = ENGINE_CONFIG[engine]
    prefix = str(getattr(args, config["prefix_attr"]))
    rid = stage1_run_id(prefix, args)
    command = [
        sys.executable,
        str(SCRIPT_DIR / config["script"]),
        "--run-prefix",
        prefix,
        "--timeframe",
        str(args.timeframe),
        "--roots",
        str(args.roots),
        "--symbols",
        str(args.symbols),
        "--oracle-run-id",
        str(args.oracle_run_id),
        "--train-years",
        str(args.train_years),
        "--threshold-year",
        str(int(args.threshold_year)),
        "--valid-year",
        str(int(args.valid_year)),
        "--positive-pre-bars",
        str(int(args.positive_pre_bars)),
        "--positive-post-bars",
        str(int(args.positive_post_bars)),
        "--negative-exclusion-bars",
        str(int(args.negative_exclusion_bars)),
        "--match-window-bars",
        str(int(args.match_window_bars)),
        "--negative-ratio",
        str(float(args.negative_ratio)),
        "--base-negatives-per-symbol",
        str(int(args.base_negatives_per_symbol)),
        "--max-train-rows",
        str(int(args.max_train_rows)),
        "--cooldown-bars",
        str(int(args.cooldown_bars)),
        "--min-oracle-recall",
        str(float(args.min_oracle_recall)),
        "--max-picks-per-oracle",
        str(float(args.max_picks_per_oracle)),
        "--thresholds",
        str(args.thresholds),
        "--iterations",
        str(int(args.iterations)),
        "--depth",
        str(int(args.depth)),
        "--learning-rate",
        str(float(args.learning_rate)),
        "--l2-leaf-reg",
        str(float(args.l2_leaf_reg)),
        "--random-seed",
        str(int(args.random_seed)),
        "--limit-symbols",
        str(int(args.limit_symbols)),
        "--entry-breakout-bars",
        str(int(args.entry_breakout_bars)),
        "--trail-lookback-bars",
        str(int(args.trail_lookback_bars)),
        "--atr-period",
        str(int(args.atr_period)),
        "--atr-stop-pad",
        str(float(args.atr_stop_pad)),
        "--min-risk-ticks",
        str(float(args.min_risk_ticks)),
        "--max-risk-ticks",
        str(float(args.max_risk_ticks)),
        "--min-relative-volume",
        str(float(args.min_relative_volume)),
    ]
    if str(args.candidate_set_id or "").strip():
        command.extend(["--candidate-set-id", str(args.candidate_set_id)])
    if str(args.candidate_cache_dir or "").strip():
        command.extend(["--candidate-cache-dir", str(args.candidate_cache_dir)])
    if args.read_candidate_cache:
        command.append("--read-candidate-cache")
    if args.write_candidate_cache:
        command.append("--write-candidate-cache")
    if args.replace_candidate_cache:
        command.append("--replace-candidate-cache")
    if args.save_row_csvs:
        command.append("--save-row-csvs")
    if args.replace_run:
        command.append("--replace-run")
    return rid, command


def write_summary(rows: list[dict[str, Any]], args: argparse.Namespace) -> Path:
    log_dir = wave.ABCD_ROOT / "logs" / "oracle_start_2m_mainline"
    log_dir.mkdir(parents=True, exist_ok=True)
    root_label = "_".join(cat_stage1.parse_list(args.roots)) or "all"
    train_label = "_".join(str(year) for year in cat_stage1.parse_years(args.train_years))
    path = log_dir / f"level1-{str(args.timeframe)}-{root_label}-tr{train_label}-v{int(args.valid_year)}-summary.json"
    payload = {
        "design": "level1_all_roots_single_timeframe_per_engine",
        "timeframe": args.timeframe,
        "roots": cat_stage1.parse_list(args.roots),
        "roots_empty_means_all": True,
        "oracle_run_id": args.oracle_run_id,
        "candidate_set_id": args.candidate_set_id,
        "read_candidate_cache": bool(args.read_candidate_cache),
        "write_candidate_cache": bool(args.write_candidate_cache),
        "train_years": cat_stage1.parse_years(args.train_years),
        "threshold_year": int(args.threshold_year),
        "valid_year": int(args.valid_year),
        "runs": rows,
    }
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    return path


def main() -> int:
    args = parse_args()
    rows: list[dict[str, Any]] = []
    for engine in parse_engines(args.engines):
        config = ENGINE_CONFIG[engine]
        rid, command = command_for(engine, args)
        output_dir = wave.ABCD_ROOT / "model_registry" / rid
        row: dict[str, Any] = {
            "engine": engine,
            "label": config["label"],
            "run_id": rid,
            "output_dir": str(output_dir),
            "status": "pending",
            "command": command,
        }
        if output_dir.exists() and args.skip_existing and not args.replace_run:
            print(f"{config['label']} Level 1 skip existing: {rid}", flush=True)
            row["status"] = "skipped_existing"
            rows.append(row)
            continue
        print(f"{config['label']} Level 1 start: {rid}", flush=True)
        if args.dry_run:
            print(" ".join(command), flush=True)
            row["status"] = "dry_run"
            rows.append(row)
            continue
        started = time.time()
        result = subprocess.run(command, cwd=str(wave.ABCD_ROOT))
        row["elapsed_seconds"] = round(time.time() - started, 2)
        row["returncode"] = int(result.returncode)
        row["status"] = "ok" if result.returncode == 0 else "failed"
        rows.append(row)
        if result.returncode != 0:
            write_summary(rows, args)
            return int(result.returncode)

    summary_path = write_summary(rows, args)
    print(f"Saved Level 1 mainline summary: {summary_path}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
