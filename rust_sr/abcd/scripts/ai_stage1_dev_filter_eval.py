#!/usr/bin/env python3
"""
Evaluate simple dev-derived filters against a holdout Stage 1 multi-valid run.

The script learns no model. It only uses selected top trades already stored by
ai_stage1_multi_valid_eval.py, derives score/category filters from a dev run,
then applies those exact filters to a holdout run.
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

import pandas as pd

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_build_search_catboost as base


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dev-run-id", required=True)
    parser.add_argument("--holdout-run-id", required=True)
    parser.add_argument("--top", type=int, default=30)
    return parser.parse_args()


def root_symbol(symbol: object) -> str:
    return base.root_symbol(symbol)


def load_selected(conn, run_id: str) -> pd.DataFrame:
    sql = """
        SELECT
            multi_valid_eval_run_id,
            valid_sample_slot,
            setup_id,
            symbol,
            pattern_family_key,
            predicted_expected_r,
            score_margin_top2,
            actual_result_r
        FROM ai_stage1_multi_valid_eval_selected
        WHERE multi_valid_eval_run_id = %s
    """
    with conn.cursor() as cur:
        cur.execute(sql, (run_id,))
        frame = pd.DataFrame(cur.fetchall())
    if frame.empty:
        raise RuntimeError(f"No selected rows found for {run_id}")
    frame["actual_result_r"] = pd.to_numeric(frame["actual_result_r"], errors="coerce").fillna(0.0)
    frame["predicted_expected_r"] = pd.to_numeric(frame["predicted_expected_r"], errors="coerce").fillna(0.0)
    frame["score_margin_top2"] = pd.to_numeric(frame["score_margin_top2"], errors="coerce").fillna(0.0)
    frame["root_symbol"] = frame["symbol"].map(root_symbol)
    frame["root_family"] = frame["root_symbol"].fillna("") + "|" + frame["pattern_family_key"].fillna("")
    return frame


def summarize(frame: pd.DataFrame) -> dict[str, float | int]:
    result = pd.to_numeric(frame["actual_result_r"], errors="coerce").fillna(0.0)
    count = int(len(result))
    wins = int((result > 0).sum())
    total = float(result.sum())
    return {
        "trades": count,
        "wins": wins,
        "win_rate": wins / count if count else 0.0,
        "avg_r": total / count if count else 0.0,
        "sum_r": total,
    }


def bad_groups(dev: pd.DataFrame, column: str, min_count: int) -> set[str]:
    grouped = (
        dev.groupby(column, dropna=False)["actual_result_r"]
        .agg(["count", "sum", "mean"])
        .reset_index()
    )
    bad = grouped[(grouped["count"] >= min_count) & (grouped["sum"] < 0.0)]
    return set(str(value) for value in bad[column].fillna(""))


def apply_policy(frame: pd.DataFrame, skip: dict[str, set[str]], score_threshold: float | None, margin_threshold: float | None) -> pd.DataFrame:
    mask = pd.Series(True, index=frame.index)
    for column, values in skip.items():
        if values:
            mask &= ~frame[column].fillna("").astype(str).isin(values)
    if score_threshold is not None:
        mask &= frame["predicted_expected_r"] >= score_threshold
    if margin_threshold is not None:
        mask &= frame["score_margin_top2"] >= margin_threshold
    return frame[mask]


def make_policies(dev: pd.DataFrame) -> list[dict[str, object]]:
    policies: list[dict[str, object]] = [
        {
            "name": "none",
            "skip": {},
            "score_q": None,
            "margin_q": None,
            "score_threshold": None,
            "margin_threshold": None,
        }
    ]

    score_quantiles: list[float | None] = [None, 0.10, 0.25, 0.40, 0.50, 0.65, 0.75]
    margin_quantiles: list[float | None] = [None, 0.25, 0.50, 0.75]
    skip_specs = [
        ("root", "root_symbol", 500),
        ("root", "root_symbol", 1000),
        ("root", "root_symbol", 1500),
        ("family", "pattern_family_key", 250),
        ("family", "pattern_family_key", 500),
        ("family", "pattern_family_key", 1000),
        ("root_family", "root_family", 100),
        ("root_family", "root_family", 250),
        ("root_family", "root_family", 500),
    ]

    skip_options: list[tuple[str, dict[str, set[str]]]] = [("skip_none", {})]
    for label, column, min_count in skip_specs:
        groups = bad_groups(dev, column, min_count)
        skip_options.append((f"skip_bad_{label}_{min_count}", {column: groups}))

    for skip_name, skip in skip_options:
        for score_q in score_quantiles:
            score_threshold = None
            if score_q is not None:
                score_threshold = float(dev["predicted_expected_r"].quantile(score_q))
            for margin_q in margin_quantiles:
                margin_threshold = None
                if margin_q is not None:
                    margin_threshold = float(dev["score_margin_top2"].quantile(margin_q))
                name = skip_name
                if score_q is not None:
                    name += f"_score_q{score_q:.2f}"
                if margin_q is not None:
                    name += f"_margin_q{margin_q:.2f}"
                policies.append(
                    {
                        "name": name,
                        "skip": skip,
                        "score_q": score_q,
                        "margin_q": margin_q,
                        "score_threshold": score_threshold,
                        "margin_threshold": margin_threshold,
                    }
                )
    return policies


def format_stats(stats: dict[str, float | int]) -> str:
    return (
        f"{stats['trades']:>6} trades | "
        f"{stats['win_rate'] * 100:5.2f}% win | "
        f"{stats['avg_r']:7.3f}R avg | "
        f"{stats['sum_r']:8.1f}R sum"
    )


def main() -> int:
    args = parse_args()
    conn = base.connect()
    try:
        dev = load_selected(conn, args.dev_run_id)
        holdout = load_selected(conn, args.holdout_run_id)
    finally:
        conn.close()

    rows = []
    for policy in make_policies(dev):
        dev_selected = apply_policy(
            dev,
            policy["skip"],
            policy["score_threshold"],
            policy["margin_threshold"],
        )
        holdout_selected = apply_policy(
            holdout,
            policy["skip"],
            policy["score_threshold"],
            policy["margin_threshold"],
        )
        dev_stats = summarize(dev_selected)
        holdout_stats = summarize(holdout_selected)
        if dev_stats["trades"] < 1000 or holdout_stats["trades"] < 1000:
            continue
        rows.append(
            {
                "policy": policy["name"],
                "dev_trades": dev_stats["trades"],
                "dev_avg_r": dev_stats["avg_r"],
                "dev_sum_r": dev_stats["sum_r"],
                "holdout_trades": holdout_stats["trades"],
                "holdout_avg_r": holdout_stats["avg_r"],
                "holdout_sum_r": holdout_stats["sum_r"],
                "holdout_win_rate": holdout_stats["win_rate"],
            }
        )

    results = pd.DataFrame(rows)
    if results.empty:
        raise RuntimeError("No policies produced enough trades.")

    print(f"Dev baseline:     {format_stats(summarize(dev))}")
    print(f"Holdout baseline: {format_stats(summarize(holdout))}")

    print("\nTop by dev sum R:")
    for row in results.sort_values(["dev_sum_r", "holdout_sum_r"], ascending=False).head(args.top).itertuples(index=False):
        print(
            f"{row.policy:<55} dev {row.dev_sum_r:8.1f}R / {row.dev_avg_r:6.3f} "
            f"| holdout {row.holdout_sum_r:8.1f}R / {row.holdout_avg_r:6.3f} "
            f"({int(row.holdout_trades)} trades, {row.holdout_win_rate * 100:.2f}% win)"
        )

    print("\nTop by holdout sum R, for diagnosis only:")
    for row in results.sort_values(["holdout_sum_r", "dev_sum_r"], ascending=False).head(args.top).itertuples(index=False):
        print(
            f"{row.policy:<55} dev {row.dev_sum_r:8.1f}R / {row.dev_avg_r:6.3f} "
            f"| holdout {row.holdout_sum_r:8.1f}R / {row.holdout_avg_r:6.3f} "
            f"({int(row.holdout_trades)} trades, {row.holdout_win_rate * 100:.2f}% win)"
        )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
