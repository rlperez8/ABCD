#!/usr/bin/env python3
"""
Run trained Stage 1 live-grid specialists over a full live-style candle grid.

This materializes the "fires" from each trained specialist model. These rows are
the candidate universe for the later Stage 2 confirmation/fakeout model.
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

import ai_candle_wave_scanner_model as scanner
import ai_wave_rider_research as wave


DEFAULT_TIMEFRAMES = "1m,2m,3m,4m,5m,6m,7m,8m,9m,10m,11m,12m,13m,14m,15m,30m"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-prefix", default="aicw-oracle-start-livegrid-alltf-v1")
    parser.add_argument("--year", type=int, default=2026)
    parser.add_argument("--roots", default="", help="Optional comma-separated root filter.")
    parser.add_argument("--timeframes", default=DEFAULT_TIMEFRAMES)
    parser.add_argument("--output-prefix", default="stage1_livegrid_fires")
    parser.add_argument("--batch-size", type=int, default=50000)
    parser.add_argument("--save-picks-limit", type=int, default=0)
    parser.add_argument("--limit-models", type=int, default=0)
    parser.add_argument("--skip-existing", action="store_true")
    parser.add_argument("--continue-on-error", action="store_true")
    return parser.parse_args()


def parse_list(raw: str) -> list[str]:
    return [part.strip().upper() for part in str(raw or "").split(",") if part.strip()]


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def model_dirs(args: argparse.Namespace) -> list[tuple[str, Path, dict[str, Any]]]:
    wanted_roots = set(parse_list(args.roots))
    wanted_timeframes = set(parse_list(args.timeframes))
    rows: list[tuple[str, Path, dict[str, Any]]] = []
    for path in sorted((wave.ABCD_ROOT / "model_registry").iterdir(), key=lambda item: item.name):
        if not path.is_dir() or not path.name.startswith(str(args.run_prefix)):
            continue
        metadata_path = path / "metadata.json"
        model_path = path / "catboost_oracle_start_live_grid_model.cbm"
        if not metadata_path.exists() or not model_path.exists():
            continue
        metadata = load_json(metadata_path)
        if str(metadata.get("model_type") or "") != "catboost_oracle_start_live_grid_stage1":
            continue
        timeframe = str(metadata.get("timeframe") or "")
        roots = [str(root).upper() for root in (metadata.get("roots") or [])]
        if wanted_timeframes and timeframe.upper() not in wanted_timeframes:
            continue
        if wanted_roots and not any(root in wanted_roots for root in roots):
            continue
        rows.append((path.name, path, metadata))
    if args.limit_models and len(rows) > int(args.limit_models):
        rows = rows[: int(args.limit_models)]
    return rows


def summary_file(model_dir: Path, args: argparse.Namespace) -> Path:
    return model_dir / f"{args.output_prefix}_{int(args.year)}.summary.json"


def picks_file(model_dir: Path, args: argparse.Namespace) -> Path:
    return model_dir / f"{args.output_prefix}_{int(args.year)}.picks.csv"


def selected_summary(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {}
    payload = load_json(path)
    summaries = payload.get("summaries") or []
    if summaries:
        return dict(summaries[0])
    return {}


def main() -> int:
    args = parse_args()
    runs = model_dirs(args)
    if not runs:
        raise ValueError("No Stage 1 live-grid specialist models matched the filters.")

    log_dir = wave.ABCD_ROOT / "logs" / "stage1_livegrid_fires"
    log_dir.mkdir(parents=True, exist_ok=True)
    batch_summary_path = log_dir / f"{args.run_prefix}-{int(args.year)}-fire-batch-summary.json"
    eval_script = SCRIPT_DIR / "eval_oracle_trend_start_full_grid.py"

    print(f"Stage 1 fire batch models={len(runs)} year={args.year}", flush=True)
    results: list[dict[str, Any]] = []
    started = time.time()

    for index, (run_id, model_dir, metadata) in enumerate(runs, start=1):
        existing_summary = summary_file(model_dir, args)
        existing_picks = picks_file(model_dir, args)
        if args.skip_existing and existing_summary.exists() and existing_picks.exists():
            row = {
                "model_run_id": run_id,
                "status": "skipped_existing",
                "timeframe": metadata.get("timeframe"),
                "roots": metadata.get("roots"),
                **selected_summary(existing_summary),
            }
            results.append(row)
            print(f"[{index}/{len(runs)}] {run_id}: skipped existing", flush=True)
            continue

        out_log = log_dir / f"{run_id}.out.log"
        err_log = log_dir / f"{run_id}.err.log"
        command = [
            sys.executable,
            str(eval_script),
            "--model-run-id",
            run_id,
            "--year",
            str(int(args.year)),
            "--output-prefix",
            str(args.output_prefix),
            "--batch-size",
            str(int(args.batch_size)),
            "--save-picks-limit",
            str(int(args.save_picks_limit)),
        ]
        model_roots = ",".join(str(root).upper() for root in (metadata.get("roots") or []) if str(root).strip())
        if model_roots:
            command.extend(["--roots", model_roots])

        print(f"[{index}/{len(runs)}] {run_id}: scoring", flush=True)
        run_started = time.time()
        with out_log.open("w", encoding="utf-8") as stdout, err_log.open("w", encoding="utf-8") as stderr:
            completed = subprocess.run(command, cwd=str(wave.ABCD_ROOT), stdout=stdout, stderr=stderr)
        elapsed = round(time.time() - run_started, 2)

        row = {
            "model_run_id": run_id,
            "status": "ok" if completed.returncode == 0 else "failed",
            "returncode": int(completed.returncode),
            "elapsed_seconds": elapsed,
            "timeframe": metadata.get("timeframe"),
            "roots": metadata.get("roots"),
            "summary_path": str(existing_summary),
            "picks_path": str(existing_picks),
            "out_log": str(out_log),
            "err_log": str(err_log),
        }
        if completed.returncode == 0:
            row.update(selected_summary(existing_summary))
            print(
                f"[{index}/{len(runs)}] {run_id}: ok "
                f"picks={row.get('picked_events')} match={row.get('pick_match_rate')} "
                f"recall={row.get('oracle_recall')} elapsed={elapsed}s",
                flush=True,
            )
        else:
            print(f"[{index}/{len(runs)}] {run_id}: failed rc={completed.returncode} elapsed={elapsed}s", flush=True)
            if not args.continue_on_error:
                results.append(row)
                batch_summary_path.write_text(json.dumps(results, indent=2), encoding="utf-8")
                return int(completed.returncode)
        results.append(row)
        batch_summary_path.write_text(json.dumps(results, indent=2), encoding="utf-8")

    print(f"Done. elapsed={round(time.time() - started, 2)}s summary={batch_summary_path}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
