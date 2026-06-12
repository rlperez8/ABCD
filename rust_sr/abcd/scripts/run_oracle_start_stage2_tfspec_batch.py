#!/usr/bin/env python3
"""Run Stage 2 timeframe-specific confirmation training for a list of Stage 1 models."""

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


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage1-run-ids", required=True, help="Comma-separated Stage 1 specialist run ids.")
    parser.add_argument("--run-prefix", default="aicw-stage2-tfspec-v1")
    parser.add_argument("--max-confirm-bars", type=int, default=16)
    parser.add_argument("--iterations", type=int, default=300)
    parser.add_argument("--min-threshold-picks", type=int, default=50)
    parser.add_argument("--min-threshold-precision", type=float, default=0.35)
    parser.add_argument("--max-events-per-year", type=int, default=0)
    parser.add_argument("--sample-events-after-build", action="store_true")
    parser.add_argument("--skip-save-expanded-rows", action="store_true")
    parser.add_argument("--replace-run", action="store_true")
    parser.add_argument("--continue-on-error", action="store_true")
    return parser.parse_args()


def parse_list(raw: str) -> list[str]:
    return [part.strip() for part in str(raw or "").split(",") if part.strip()]


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def find_output_metadata(run_prefix: str, stage1_run_id: str) -> tuple[Path | None, dict[str, Any] | None]:
    matches: list[tuple[float, Path, dict[str, Any]]] = []
    for path in (wave.ABCD_ROOT / "model_registry").glob(f"{run_prefix}*"):
        if not path.is_dir():
            continue
        metadata_path = path / "metadata.json"
        if not metadata_path.exists():
            continue
        try:
            metadata = load_json(metadata_path)
        except Exception:
            continue
        if str(metadata.get("stage1_run_id") or "") == str(stage1_run_id):
            matches.append((metadata_path.stat().st_mtime, path, metadata))
    if not matches:
        return None, None
    _, path, metadata = sorted(matches, key=lambda item: item[0], reverse=True)[0]
    return path, metadata


def valid_event_summary(metadata: dict[str, Any] | None) -> dict[str, Any]:
    if not metadata:
        return {}
    valid_year = str(metadata.get("valid_year") or "")
    results = metadata.get("results") or {}
    valid = results.get(valid_year) or {}
    source = valid.get("source") or {}
    event = valid.get("event") or {}
    return {
        "timeframe": metadata.get("timeframe"),
        "roots": metadata.get("roots"),
        "stage1_events": source.get("stage1_events"),
        "stage1_precision": source.get("stage1_precision"),
        "stage1_total_oracle_recall": source.get("stage1_total_oracle_recall"),
        "confirmed_events": event.get("confirmed_events"),
        "precision": event.get("precision"),
        "oracle_precision": event.get("oracle_precision"),
        "total_oracle_recall": event.get("total_oracle_recall"),
        "recall_within_stage1": event.get("recall_within_stage1"),
        "threshold": event.get("threshold"),
    }


def main() -> int:
    args = parse_args()
    stage1_run_ids = parse_list(args.stage1_run_ids)
    if not stage1_run_ids:
        raise ValueError("No Stage 1 run ids provided.")

    log_dir = wave.ABCD_ROOT / "logs" / "stage2_tfspec_batches"
    log_dir.mkdir(parents=True, exist_ok=True)
    summary_path = log_dir / f"{args.run_prefix}-batch-summary.json"
    trainer = SCRIPT_DIR / "ai_oracle_start_stage2_tfspec_live_grid_model.py"

    print(f"Stage 2 tfspec batch models={len(stage1_run_ids)}", flush=True)
    results: list[dict[str, Any]] = []
    started = time.time()

    for index, stage1_run_id in enumerate(stage1_run_ids, start=1):
        safe_name = stage1_run_id.replace("|", "_").replace("/", "_").replace("\\", "_")
        out_log = log_dir / f"{safe_name}.out.log"
        err_log = log_dir / f"{safe_name}.err.log"
        command = [
            sys.executable,
            str(trainer),
            "--stage1-run-id",
            stage1_run_id,
            "--run-prefix",
            str(args.run_prefix),
            "--max-confirm-bars",
            str(int(args.max_confirm_bars)),
            "--iterations",
            str(int(args.iterations)),
            "--min-threshold-picks",
            str(int(args.min_threshold_picks)),
            "--min-threshold-precision",
            str(float(args.min_threshold_precision)),
        ]
        if int(args.max_events_per_year) > 0:
            command.extend(["--max-events-per-year", str(int(args.max_events_per_year))])
        if args.sample_events_after_build:
            command.append("--sample-events-after-build")
        if args.skip_save_expanded_rows:
            command.append("--skip-save-expanded-rows")
        if args.replace_run:
            command.append("--replace-run")

        print(f"[{index}/{len(stage1_run_ids)}] {stage1_run_id}: train", flush=True)
        run_started = time.time()
        with out_log.open("w", encoding="utf-8") as stdout, err_log.open("w", encoding="utf-8") as stderr:
            completed = subprocess.run(command, cwd=str(wave.ABCD_ROOT), stdout=stdout, stderr=stderr)
        elapsed = round(time.time() - run_started, 2)
        output_dir, metadata = find_output_metadata(str(args.run_prefix), stage1_run_id)
        row = {
            "stage1_run_id": stage1_run_id,
            "status": "ok" if completed.returncode == 0 else "failed",
            "returncode": int(completed.returncode),
            "elapsed_seconds": elapsed,
            "output_run_id": output_dir.name if output_dir else None,
            "output_dir": str(output_dir) if output_dir else None,
            "out_log": str(out_log),
            "err_log": str(err_log),
            **valid_event_summary(metadata),
        }
        results.append(row)
        summary_path.write_text(json.dumps(results, indent=2), encoding="utf-8")
        if completed.returncode == 0:
            print(
                f"[{index}/{len(stage1_run_ids)}] {stage1_run_id}: ok "
                f"confirmed={row.get('confirmed_events')} precision={row.get('precision')} "
                f"oracle_recall={row.get('total_oracle_recall')} elapsed={elapsed}s",
                flush=True,
            )
        else:
            print(f"[{index}/{len(stage1_run_ids)}] {stage1_run_id}: failed rc={completed.returncode}", flush=True)
            if not args.continue_on_error:
                return int(completed.returncode)

    print(f"Done. elapsed={round(time.time() - started, 2)}s summary={summary_path}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
