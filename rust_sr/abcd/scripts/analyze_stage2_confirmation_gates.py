#!/usr/bin/env python3
"""
Search simple confirmation gates on top of adaptive Stage 2 scores.

This uses saved adaptive Stage 2 expanded rows. It chooses a gate on the
threshold split, then evaluates the same gate on the validation split.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--min-precision", type=float, default=1.0)
    parser.add_argument("--min-picks", type=int, default=25)
    parser.add_argument("--output-name", default="stage2_confirmation_gate_sweep")
    return parser.parse_args()


def model_dir(run_id: str) -> Path:
    root = Path(__file__).resolve().parents[1]
    path = root / "model_registry" / run_id
    if not path.exists():
        raise FileNotFoundError(f"Missing model dir: {path}")
    return path


def load_rows(base: Path, split: str | int) -> pd.DataFrame:
    path = base / ("stage2_adaptive_rows_train.csv" if str(split) == "train" else f"stage2_adaptive_rows_{split}.csv")
    if not path.exists():
        raise FileNotFoundError(f"Missing expanded rows: {path}")
    frame = pd.read_csv(path)
    frame["candidate_uid"] = frame["candidate_uid"].astype(str)
    for col in [
        "stage2_score",
        "stage2_target",
        "is_oracle_start",
        "confirm_offset_bars",
        "confirm_close_move_atr",
        "confirm_max_favorable_atr",
        "confirm_max_adverse_atr",
        "confirm_efficiency",
        "confirm_close_step_align_share",
    ]:
        frame[col] = pd.to_numeric(frame.get(col), errors="coerce").fillna(0.0)
    return frame.sort_values(["candidate_uid", "confirm_offset_bars"]).reset_index(drop=True)


def event_base(frame: pd.DataFrame) -> pd.DataFrame:
    first = frame.drop_duplicates("candidate_uid")[
        ["candidate_uid", "is_oracle_start"]
    ].copy()
    target = frame.groupby("candidate_uid", as_index=False)["stage2_target"].max().rename(
        columns={"stage2_target": "event_stage2_target"}
    )
    return first.merge(target, on="candidate_uid", how="left")


def metrics(frame: pd.DataFrame, base: pd.DataFrame, gate: dict[str, float]) -> dict[str, Any]:
    mask = (
        (frame["stage2_score"] >= gate["score_threshold"])
        & (frame["confirm_close_move_atr"] >= gate["min_close_move_atr"])
        & (frame["confirm_max_favorable_atr"] >= gate["min_favorable_atr"])
        & (frame["confirm_max_adverse_atr"] <= gate["max_adverse_atr"])
        & (frame["confirm_efficiency"] >= gate["min_efficiency"])
        & (frame["confirm_close_step_align_share"] >= gate["min_close_step_align_share"])
    )
    hits = frame.loc[mask].drop_duplicates("candidate_uid")[
        ["candidate_uid", "stage2_target", "is_oracle_start", "confirm_offset_bars"]
    ].copy()
    if hits.empty:
        return {
            **gate,
            "events": int(len(base)),
            "target_positive_events": int(base["event_stage2_target"].sum()),
            "confirmed_events": 0,
            "true_confirmed_events": 0,
            "false_confirmed_events": 0,
            "precision": 0.0,
            "recall": 0.0,
            "f1": 0.0,
            "oracle_precision": 0.0,
            "total_oracle_recall": 0.0,
            "avg_true_confirm_offset": None,
        }
    picks = int(len(hits))
    true_picks = int((hits["stage2_target"] == 1).sum())
    oracle_picks = int((hits["is_oracle_start"] == 1).sum())
    positives = int(base["event_stage2_target"].sum())
    oracle_total = int(base["is_oracle_start"].sum())
    precision = true_picks / picks if picks else 0.0
    recall = true_picks / positives if positives else 0.0
    f1 = (2.0 * precision * recall / (precision + recall)) if precision + recall > 0 else 0.0
    true_offsets = hits.loc[hits["stage2_target"] == 1, "confirm_offset_bars"]
    return {
        **gate,
        "events": int(len(base)),
        "target_positive_events": positives,
        "confirmed_events": picks,
        "true_confirmed_events": true_picks,
        "false_confirmed_events": int(picks - true_picks),
        "precision": precision,
        "recall": recall,
        "f1": f1,
        "oracle_confirmed_events": oracle_picks,
        "oracle_precision": oracle_picks / picks if picks else 0.0,
        "total_oracle_recall": oracle_picks / oracle_total if oracle_total else 0.0,
        "avg_true_confirm_offset": float(true_offsets.mean()) if len(true_offsets) else None,
    }


def candidate_scores(frame: pd.DataFrame) -> list[float]:
    score = frame["stage2_score"].to_numpy(dtype=float)
    quantiles = [
        0.50,
        0.60,
        0.70,
        0.80,
        0.85,
        0.90,
        0.925,
        0.95,
        0.965,
        0.975,
        0.985,
        0.99,
        0.995,
        0.998,
        0.999,
        0.9995,
    ]
    return sorted(set(float(v) for v in np.quantile(score, quantiles)))


def main() -> int:
    args = parse_args()
    base_dir = model_dir(args.run_id)
    threshold_rows = load_rows(base_dir, int(args.threshold_year))
    valid_rows = load_rows(base_dir, int(args.valid_year))
    threshold_base = event_base(threshold_rows)
    valid_base = event_base(valid_rows)

    gates: list[dict[str, float]] = []
    for score_threshold in candidate_scores(threshold_rows):
        for min_close in [0.0, 0.05, 0.10, 0.15, 0.25, 0.35]:
            for min_fav in [0.0, 0.15, 0.25, 0.35, 0.50]:
                for max_adv in [0.35, 0.50, 0.75, 1.00, 1.25, 1.50]:
                    for min_eff in [-1.0, 0.0, 0.05, 0.10]:
                        for min_steps in [0.0, 0.25, 0.50]:
                            gates.append(
                                {
                                    "score_threshold": score_threshold,
                                    "min_close_move_atr": min_close,
                                    "min_favorable_atr": min_fav,
                                    "max_adverse_atr": max_adv,
                                    "min_efficiency": min_eff,
                                    "min_close_step_align_share": min_steps,
                                }
                            )

    rows: list[dict[str, Any]] = []
    for index, gate in enumerate(gates, start=1):
        rows.append(metrics(threshold_rows, threshold_base, gate))
        if index % 2000 == 0:
            print(f"scored gates {index:,}/{len(gates):,}", flush=True)
    sweep = pd.DataFrame(rows)
    allowed = sweep[(sweep["precision"] >= float(args.min_precision)) & (sweep["confirmed_events"] >= int(args.min_picks))]
    if allowed.empty:
        allowed = sweep[sweep["confirmed_events"] >= int(args.min_picks)]
    if allowed.empty:
        allowed = sweep
    best = allowed.sort_values(["precision", "f1", "confirmed_events"], ascending=[False, False, False]).iloc[0].to_dict()
    gate_keys = [
        "score_threshold",
        "min_close_move_atr",
        "min_favorable_atr",
        "max_adverse_atr",
        "min_efficiency",
        "min_close_step_align_share",
    ]
    best_gate = {key: float(best[key]) for key in gate_keys}
    valid_result = metrics(valid_rows, valid_base, best_gate)

    out_prefix = base_dir / args.output_name
    sweep.sort_values(["precision", "f1", "confirmed_events"], ascending=[False, False, False]).to_csv(
        out_prefix.with_suffix(".csv"),
        index=False,
    )
    summary = {
        "run_id": args.run_id,
        "threshold_year": int(args.threshold_year),
        "valid_year": int(args.valid_year),
        "min_precision": float(args.min_precision),
        "min_picks": int(args.min_picks),
        "best_threshold_result": best,
        "valid_result": valid_result,
    }
    out_prefix.with_suffix(".json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(json.dumps(summary, indent=2), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
