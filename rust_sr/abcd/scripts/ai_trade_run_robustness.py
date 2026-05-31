#!/usr/bin/env python3
"""
Stress-test AI trade runs for outlier and slippage fragility.

The results are stored in ai_trade_run_robustness so durability checks can be
read from the DB later instead of recalculated in the UI.
"""

from __future__ import annotations

import argparse
import math
import os
import re
import sys
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

import numpy as np
import pandas as pd
import pymysql


ABCD_ROOT = Path(__file__).resolve().parents[1]
ROBUSTNESS_TABLE = "ai_trade_run_robustness"

ROOT_TICK_SIZE = {
    "ES": 0.25,
    "MES": 0.25,
    "NQ": 0.25,
    "MNQ": 0.25,
    "YM": 1.0,
    "MYM": 1.0,
    "CL": 0.01,
    "MCL": 0.01,
    "QM": 0.025,
    "RTY": 0.1,
    "M2K": 0.1,
    "EMD": 0.1,
    "NKD": 5.0,
    "ZL": 0.01,
    "GF": 0.025,
    "LE": 0.025,
    "HE": 0.025,
    "QG": 0.005,
    "NG": 0.001,
    "HO": 0.0001,
    "RB": 0.0001,
    "ZS": 0.25,
    "ZM": 0.1,
    "ZW": 0.25,
    "ZC": 0.25,
    "GC": 0.1,
    "MGC": 0.1,
    "SI": 0.005,
    "HG": 0.0005,
    "PL": 0.1,
    "ZN": 0.015625,
    "ZB": 0.03125,
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--run-ids",
        default="aimv-market-wave-meta-v1-2026,aimv-market-entry-context-v7-2026,aimv-wave-rider-broad-model-v3-2026",
    )
    parser.add_argument("--caps", default="3,5,8,10")
    parser.add_argument("--remove-top", default="1,3,5,10")
    parser.add_argument("--slippage-pairs", default="5+5,8+8,10+10")
    parser.add_argument("--replace", action="store_true")
    return parser.parse_args()


def read_database_url() -> str:
    if os.environ.get("DATABASE_URL"):
        return os.environ["DATABASE_URL"]
    if os.environ.get("ABCD_DATABASE_URL"):
        return os.environ["ABCD_DATABASE_URL"]

    env_path = ABCD_ROOT / ".env"
    if not env_path.exists():
        raise FileNotFoundError(f"Could not find DATABASE_URL or {env_path}")
    for line in env_path.read_text().splitlines():
        match = re.match(r"\s*(?:DATABASE_URL|ABCD_DATABASE_URL)\s*=\s*(.+?)\s*$", line)
        if match:
            return match.group(1).strip().strip('"').strip("'")
    raise ValueError(f"DATABASE_URL was not found in {env_path}")


def connect() -> pymysql.connections.Connection:
    parsed = urlparse(read_database_url())
    return pymysql.connect(
        host=parsed.hostname,
        user=parsed.username,
        password=parsed.password,
        database=parsed.path.lstrip("/"),
        port=parsed.port or 3306,
        autocommit=False,
        cursorclass=pymysql.cursors.DictCursor,
    )


def parse_list(value: str) -> list[str]:
    return [part.strip() for part in value.split(",") if part.strip()]


def parse_float_list(value: str) -> list[float]:
    return [float(part) for part in parse_list(value)]


def parse_int_list(value: str) -> list[int]:
    return [int(part) for part in parse_list(value)]


def parse_slippage_pairs(value: str) -> list[tuple[float, float]]:
    pairs: list[tuple[float, float]] = []
    for part in parse_list(value):
        match = re.fullmatch(r"(\d+(?:\.\d+)?)\s*\+\s*(\d+(?:\.\d+)?)", part)
        if not match:
            raise ValueError(f"Invalid slippage pair: {part!r}. Expected like 5+5")
        pairs.append((float(match.group(1)), float(match.group(2))))
    return pairs


def clean(value: Any) -> Any:
    if isinstance(value, np.generic):
        value = value.item()
    if isinstance(value, pd.Timestamp):
        if pd.isna(value):
            return None
        return value.to_pydatetime()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    if value is None:
        return None
    try:
        if pd.isna(value):
            return None
    except TypeError:
        pass
    return value


def root_symbol(symbol: object) -> str:
    if symbol is None:
        return ""
    return re.sub(r"[FGHJKMNQUVXZ][0-9]+$", "", str(symbol).upper())


def tick_size_for(symbol: object, root: object = None) -> float:
    value = root_symbol(root or symbol)
    if value in ROOT_TICK_SIZE:
        return ROOT_TICK_SIZE[value]
    for candidate, tick_size in ROOT_TICK_SIZE.items():
        if value.startswith(candidate):
            return tick_size
    return 0.25


def ensure_table(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            f"""
            CREATE TABLE IF NOT EXISTS {ROBUSTNESS_TABLE} (
                multi_valid_eval_run_id VARCHAR(64) NOT NULL,
                valid_year INT NULL,
                scenario_type VARCHAR(32) NOT NULL,
                scenario_name VARCHAR(64) NOT NULL,
                cap_r DOUBLE NULL,
                remove_top_n INT NULL,
                target_entry_slippage_ticks DOUBLE NULL,
                target_exit_slippage_ticks DOUBLE NULL,
                extra_roundtrip_ticks DOUBLE NULL,
                total_rows INT NOT NULL DEFAULT 0,
                taken_trades INT NOT NULL DEFAULT 0,
                no_entries INT NOT NULL DEFAULT 0,
                wins INT NOT NULL DEFAULT 0,
                losses INT NOT NULL DEFAULT 0,
                win_rate DOUBLE NOT NULL DEFAULT 0,
                avg_r DOUBLE NOT NULL DEFAULT 0,
                sum_r DOUBLE NOT NULL DEFAULT 0,
                best_r DOUBLE NOT NULL DEFAULT 0,
                worst_r DOUBLE NOT NULL DEFAULT 0,
                sum_without_best DOUBLE NOT NULL DEFAULT 0,
                avg_r_taken DOUBLE NOT NULL DEFAULT 0,
                formula VARCHAR(128) NOT NULL DEFAULT 'ai_trade_run_robustness_v1',
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                PRIMARY KEY (multi_valid_eval_run_id, scenario_type, scenario_name),
                INDEX idx_ai_robustness_rank (multi_valid_eval_run_id, sum_r),
                INDEX idx_ai_robustness_year (valid_year, scenario_type)
            )
            """
        )
    conn.commit()


def fetch_run_rows(conn, run_id: str) -> tuple[dict[str, Any], pd.DataFrame]:
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT *
            FROM ai_stage1_multi_valid_eval_runs
            WHERE multi_valid_eval_run_id = %s
            """,
            (run_id,),
        )
        run = cur.fetchone()
        if not run:
            raise RuntimeError(f"Missing AI run: {run_id}")
        cur.execute(
            """
            SELECT
                multi_valid_eval_run_id,
                setup_id,
                symbol,
                root_symbol,
                trade_at,
                result_r,
                outcome,
                risk_points,
                trade_direction
            FROM ai_stage1_trade_rows
            WHERE multi_valid_eval_run_id = %s
            ORDER BY trade_at, setup_id
            """,
            (run_id,),
        )
        rows = cur.fetchall()
    frame = pd.DataFrame(rows)
    if frame.empty:
        return run, frame
    frame["result_r"] = pd.to_numeric(frame["result_r"], errors="coerce").fillna(0.0)
    frame["risk_points"] = pd.to_numeric(frame["risk_points"], errors="coerce")
    frame["outcome"] = frame["outcome"].fillna("unknown").astype(str)
    frame["tick_size"] = [tick_size_for(symbol, root) for symbol, root in zip(frame["symbol"], frame["root_symbol"])]
    frame["risk_ticks"] = frame["risk_points"] / frame["tick_size"].replace(0.0, np.nan)
    return run, frame


def scenario_stats(
    run: dict[str, Any],
    frame: pd.DataFrame,
    scenario_type: str,
    scenario_name: str,
    adjusted_taken_r: pd.Series,
    total_rows: int,
    no_entries: int,
    cap_r: float | None = None,
    remove_top_n: int | None = None,
    target_entry_slippage_ticks: float | None = None,
    target_exit_slippage_ticks: float | None = None,
    extra_roundtrip_ticks: float | None = None,
) -> dict[str, Any]:
    result = pd.to_numeric(adjusted_taken_r, errors="coerce").dropna()
    taken = int(len(result))
    wins = int((result > 0.0).sum())
    losses = int(taken - wins)
    total = float(result.sum()) if taken else 0.0
    avg_taken = float(result.mean()) if taken else 0.0
    best = float(result.max()) if taken else 0.0
    worst = float(result.min()) if taken else 0.0
    return {
        "multi_valid_eval_run_id": run["multi_valid_eval_run_id"],
        "valid_year": run.get("valid_year"),
        "scenario_type": scenario_type,
        "scenario_name": scenario_name,
        "cap_r": cap_r,
        "remove_top_n": remove_top_n,
        "target_entry_slippage_ticks": target_entry_slippage_ticks,
        "target_exit_slippage_ticks": target_exit_slippage_ticks,
        "extra_roundtrip_ticks": extra_roundtrip_ticks,
        "total_rows": total_rows,
        "taken_trades": taken,
        "no_entries": no_entries + max(0, total_rows - no_entries - taken),
        "wins": wins,
        "losses": losses,
        "win_rate": wins / taken if taken else 0.0,
        "avg_r": total / total_rows if total_rows else 0.0,
        "sum_r": total,
        "best_r": best,
        "worst_r": worst,
        "sum_without_best": total - best if taken else 0.0,
        "avg_r_taken": avg_taken,
    }


def build_scenarios(run: dict[str, Any], frame: pd.DataFrame, caps: list[float], remove_top: list[int], slippage_pairs: list[tuple[float, float]]) -> list[dict[str, Any]]:
    if frame.empty:
        return []
    total_rows = int(len(frame))
    no_entries = int((frame["outcome"] == "no_entry").sum())
    taken = frame[frame["outcome"] != "no_entry"].copy()
    base = pd.to_numeric(taken["result_r"], errors="coerce").fillna(0.0)
    scenarios = [
        scenario_stats(run, frame, "baseline", "stored", base, total_rows, no_entries),
    ]

    for cap in caps:
        capped = base.clip(upper=cap)
        scenarios.append(
            scenario_stats(
                run,
                frame,
                "cap_winners",
                f"cap_{cap:g}r",
                capped,
                total_rows,
                no_entries,
                cap_r=cap,
            )
        )

    sorted_base = base.sort_values(ascending=False)
    for count in remove_top:
        keep = sorted_base.iloc[min(count, len(sorted_base)) :]
        scenarios.append(
            scenario_stats(
                run,
                frame,
                "remove_top_winners",
                f"remove_top_{count}",
                keep,
                total_rows,
                no_entries,
                remove_top_n=count,
            )
        )

    current_entry = float(run.get("slippage_entry_ticks") or 0.0)
    current_exit = float(run.get("slippage_exit_ticks") or 0.0)
    risk_ticks = pd.to_numeric(taken["risk_ticks"], errors="coerce").replace([np.inf, -np.inf], np.nan)
    for entry_ticks, exit_ticks in slippage_pairs:
        extra = max(0.0, entry_ticks - current_entry) + max(0.0, exit_ticks - current_exit)
        adjusted = base - (extra / risk_ticks).fillna(0.0)
        scenarios.append(
            scenario_stats(
                run,
                frame,
                "slippage_stress",
                f"{entry_ticks:g}+{exit_ticks:g}",
                adjusted,
                total_rows,
                no_entries,
                target_entry_slippage_ticks=entry_ticks,
                target_exit_slippage_ticks=exit_ticks,
                extra_roundtrip_ticks=extra,
            )
        )
        for cap in caps:
            adjusted_capped = adjusted.clip(upper=cap)
            scenarios.append(
                scenario_stats(
                    run,
                    frame,
                    "cap_and_slippage",
                    f"cap_{cap:g}r_{entry_ticks:g}+{exit_ticks:g}",
                    adjusted_capped,
                    total_rows,
                    no_entries,
                    cap_r=cap,
                    target_entry_slippage_ticks=entry_ticks,
                    target_exit_slippage_ticks=exit_ticks,
                    extra_roundtrip_ticks=extra,
                )
            )
    return scenarios


def replace_rows(conn, run_ids: list[str]) -> None:
    if not run_ids:
        return
    placeholders = ", ".join(["%s"] * len(run_ids))
    with conn.cursor() as cur:
        cur.execute(
            f"DELETE FROM {ROBUSTNESS_TABLE} WHERE multi_valid_eval_run_id IN ({placeholders})",
            run_ids,
        )
    conn.commit()


def insert_rows(conn, rows: list[dict[str, Any]]) -> None:
    if not rows:
        return
    columns = [
        "multi_valid_eval_run_id",
        "valid_year",
        "scenario_type",
        "scenario_name",
        "cap_r",
        "remove_top_n",
        "target_entry_slippage_ticks",
        "target_exit_slippage_ticks",
        "extra_roundtrip_ticks",
        "total_rows",
        "taken_trades",
        "no_entries",
        "wins",
        "losses",
        "win_rate",
        "avg_r",
        "sum_r",
        "best_r",
        "worst_r",
        "sum_without_best",
        "avg_r_taken",
    ]
    placeholders = ", ".join(["%s"] * len(columns))
    updates = ", ".join(
        [f"{column}=VALUES({column})" for column in columns if column not in {"multi_valid_eval_run_id", "scenario_type", "scenario_name"}]
        + ["updated_at=CURRENT_TIMESTAMP"]
    )
    with conn.cursor() as cur:
        cur.executemany(
            f"""
            INSERT INTO {ROBUSTNESS_TABLE} ({", ".join(columns)})
            VALUES ({placeholders})
            ON DUPLICATE KEY UPDATE {updates}
            """,
            [tuple(clean(row.get(column)) for column in columns) for row in rows],
        )
    conn.commit()


def print_summary(rows: list[dict[str, Any]]) -> None:
    key_scenarios = {
        "stored",
        "cap_3r",
        "cap_5r",
        "cap_8r",
        "remove_top_1",
        "remove_top_3",
        "5+5",
        "8+8",
        "10+10",
        "cap_5r_5+5",
        "cap_5r_8+8",
    }
    for run_id in dict.fromkeys(row["multi_valid_eval_run_id"] for row in rows):
        print(run_id)
        for row in rows:
            if row["multi_valid_eval_run_id"] != run_id or row["scenario_name"] not in key_scenarios:
                continue
            print(
                f"  {row['scenario_name']:<16} taken={row['taken_trades']:>4} "
                f"win={row['win_rate'] * 100:>5.1f}% avgTaken={row['avg_r_taken']:>6.3f}R "
                f"sum={row['sum_r']:>7.1f}R noBest={row['sum_without_best']:>7.1f}R"
            )


def main() -> int:
    args = parse_args()
    run_ids = parse_list(args.run_ids)
    caps = parse_float_list(args.caps)
    remove_top = parse_int_list(args.remove_top)
    slippage_pairs = parse_slippage_pairs(args.slippage_pairs)

    conn = connect()
    all_rows: list[dict[str, Any]] = []
    try:
        ensure_table(conn)
        if args.replace:
            replace_rows(conn, run_ids)
        for run_id in run_ids:
            run, frame = fetch_run_rows(conn, run_id)
            scenarios = build_scenarios(run, frame, caps, remove_top, slippage_pairs)
            all_rows.extend(scenarios)
        insert_rows(conn, all_rows)
    finally:
        conn.close()

    print(f"Stored {len(all_rows):,} robustness rows in {ROBUSTNESS_TABLE}.")
    print_summary(all_rows)
    return 0


if __name__ == "__main__":
    sys.exit(main())
