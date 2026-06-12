#!/usr/bin/env python3
"""
Build a DB-backed oracle table of hindsight-perfect winning trend trades.

This is not live trading logic. It intentionally cheats to create teacher rows:
clean trend starts and ends found from completed candles. Future models can use
these rows as labels while still only seeing live-available candles as inputs.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_scanner_model as scanner
import ai_wave_rider_research as wave


RUN_TABLE = "ai_oracle_trend_runs"
TRADE_TABLE = "ai_oracle_trend_trades"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-id", default="")
    parser.add_argument("--run-prefix", default="oracle-perfect-trends-v1")
    parser.add_argument("--timeframe", default="2m", choices=sorted(scanner.TIMEFRAME_TABLES))
    parser.add_argument("--years", default="2024,2025,2026")
    parser.add_argument("--roots", default="", help="Optional comma-separated root symbols.")
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
    parser.add_argument("--batch-size", type=int, default=1000)
    parser.add_argument("--replace-run", action="store_true")
    parser.add_argument("--limit-symbols", type=int, default=0)
    return parser.parse_args()


def parse_list(raw: str) -> list[str]:
    return [part.strip().upper() for part in str(raw or "").split(",") if part.strip()]


def parse_years(raw: str) -> list[int]:
    years = [int(part) for part in parse_list(raw)]
    if not years:
        raise ValueError("At least one year is required")
    return years


def make_run_id(args: argparse.Namespace, years: list[int]) -> str:
    if args.run_id:
        return args.run_id
    if len(years) == 1:
        suffix = str(years[0])
    else:
        suffix = f"{min(years)}_{max(years)}"
    run_id = f"{args.run_prefix}-{args.timeframe}-{suffix}"
    if len(run_id) > 64:
        digest = hashlib.sha1(run_id.encode("utf-8")).hexdigest()[:10]
        run_id = f"{args.run_prefix[:35]}-{args.timeframe}-{digest}"
    return run_id


def ensure_tables(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            f"""
            CREATE TABLE IF NOT EXISTS {RUN_TABLE} (
                run_id VARCHAR(64) NOT NULL PRIMARY KEY,
                created_at DATETIME NOT NULL,
                timeframe VARCHAR(16) NOT NULL,
                years VARCHAR(64) NOT NULL,
                roots TEXT NULL,
                parameters_json JSON NULL,
                total_trades INT NOT NULL DEFAULT 0,
                total_r DOUBLE NOT NULL DEFAULT 0,
                avg_r DOUBLE NOT NULL DEFAULT 0,
                avg_quality DOUBLE NOT NULL DEFAULT 0,
                min_result_r DOUBLE NOT NULL DEFAULT 0,
                max_result_r DOUBLE NOT NULL DEFAULT 0
            )
            """
        )
        cur.execute(
            f"""
            CREATE TABLE IF NOT EXISTS {TRADE_TABLE} (
                run_id VARCHAR(64) NOT NULL,
                oracle_trade_id VARCHAR(80) NOT NULL,
                timeframe VARCHAR(16) NOT NULL,
                valid_year INT NOT NULL,
                root_symbol VARCHAR(32) NULL,
                symbol VARCHAR(32) NOT NULL,
                direction VARCHAR(8) NOT NULL,
                entry_idx INT NOT NULL,
                exit_idx INT NOT NULL,
                entry_date DATETIME NOT NULL,
                exit_date DATETIME NOT NULL,
                entry_price DOUBLE NOT NULL,
                exit_price DOUBLE NOT NULL,
                stop_price DOUBLE NOT NULL,
                target_price DOUBLE NOT NULL,
                risk_points DOUBLE NOT NULL,
                risk_ticks DOUBLE NOT NULL,
                tick_size DOUBLE NOT NULL,
                entry_atr DOUBLE NULL,
                gross_points DOUBLE NOT NULL,
                gross_ticks DOUBLE NOT NULL,
                slippage_ticks DOUBLE NOT NULL,
                result_r DOUBLE NOT NULL,
                max_favorable_r DOUBLE NOT NULL,
                max_adverse_r DOUBLE NOT NULL,
                duration_bars INT NOT NULL,
                duration_minutes INT NOT NULL,
                efficiency DOUBLE NOT NULL,
                quality_score DOUBLE NOT NULL,
                start_pivot_type VARCHAR(8) NOT NULL,
                end_pivot_type VARCHAR(8) NOT NULL,
                outcome VARCHAR(16) NOT NULL DEFAULT 'WIN',
                PRIMARY KEY (run_id, oracle_trade_id),
                INDEX idx_oracle_trend_run_time (run_id, entry_date),
                INDEX idx_oracle_trend_run_root (run_id, root_symbol, valid_year),
                INDEX idx_oracle_trend_quality (run_id, quality_score),
                INDEX idx_oracle_trend_result (run_id, result_r)
            )
            """
        )
    conn.commit()


def delete_run(conn, run_id: str) -> None:
    with conn.cursor() as cur:
        cur.execute(f"DELETE FROM {TRADE_TABLE} WHERE run_id = %s", (run_id,))
        cur.execute(f"DELETE FROM {RUN_TABLE} WHERE run_id = %s", (run_id,))
    conn.commit()


def fetch_candles(conn, timeframe: str, years: list[int], roots: list[str]) -> pd.DataFrame:
    table_name, timeframe_minutes = scanner.table_for_timeframe(timeframe)
    table = scanner.safe_identifier(table_name)
    start = pd.Timestamp(year=min(years), month=1, day=1) - pd.Timedelta(days=3)
    end = pd.Timestamp(year=max(years) + 1, month=1, day=1) + pd.Timedelta(days=3)
    params: list[Any] = [start.to_pydatetime(), end.to_pydatetime()]
    root_sql = ""
    if roots:
        root_sql = "AND root_symbol IN (" + ",".join(["%s"] * len(roots)) + ")"
        params.extend(roots)
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT
                root_symbol,
                symbol,
                ts_utc,
                CAST(open AS DOUBLE) AS open,
                CAST(high AS DOUBLE) AS high,
                CAST(low AS DOUBLE) AS low,
                CAST(close AS DOUBLE) AS close,
                CAST(volume AS DOUBLE) AS volume
            FROM {table}
            WHERE ts_utc >= %s
              AND ts_utc < %s
              {root_sql}
            ORDER BY root_symbol, symbol, ts_utc
            """,
            params,
        )
        rows = cur.fetchall()
    frame = pd.DataFrame(rows)
    if frame.empty:
        return frame
    frame["ts_utc"] = pd.to_datetime(frame["ts_utc"], errors="coerce")
    for col in ["open", "high", "low", "close", "volume"]:
        frame[col] = pd.to_numeric(frame[col], errors="coerce")
    return frame.dropna(subset=["symbol", "ts_utc", "open", "high", "low", "close"]).reset_index(drop=True)


def add_atr(candles: pd.DataFrame, period: int) -> pd.DataFrame:
    work = candles.copy()
    prev_close = work["close"].shift(1)
    tr = pd.concat(
        [
            work["high"] - work["low"],
            (work["high"] - prev_close).abs(),
            (work["low"] - prev_close).abs(),
        ],
        axis=1,
    ).max(axis=1)
    work["atr"] = tr.rolling(int(period), min_periods=max(3, int(period) // 2)).mean()
    return work


def raw_pivots(candles: pd.DataFrame, window: int) -> list[dict[str, Any]]:
    highs = candles["high"].to_numpy(dtype=float)
    lows = candles["low"].to_numpy(dtype=float)
    pivots: list[dict[str, Any]] = []
    w = max(1, int(window))
    for idx in range(w, len(candles) - w):
        low_slice = lows[idx - w : idx + w + 1]
        high_slice = highs[idx - w : idx + w + 1]
        is_low = lows[idx] <= float(np.min(low_slice))
        is_high = highs[idx] >= float(np.max(high_slice))
        if is_low and is_high:
            continue
        if is_low:
            pivots.append({"idx": idx, "kind": "LOW", "price": float(lows[idx])})
        elif is_high:
            pivots.append({"idx": idx, "kind": "HIGH", "price": float(highs[idx])})
    return pivots


def alternating_pivots(pivots: list[dict[str, Any]]) -> list[dict[str, Any]]:
    clean: list[dict[str, Any]] = []
    for pivot in pivots:
        if not clean or clean[-1]["kind"] != pivot["kind"]:
            clean.append(pivot)
            continue
        last = clean[-1]
        if pivot["kind"] == "LOW" and float(pivot["price"]) < float(last["price"]):
            clean[-1] = pivot
        elif pivot["kind"] == "HIGH" and float(pivot["price"]) > float(last["price"]):
            clean[-1] = pivot
    return clean


def path_efficiency(candles: pd.DataFrame, start_idx: int, end_idx: int, gross_points: float) -> float:
    closes = candles["close"].iloc[start_idx : end_idx + 1].to_numpy(dtype=float)
    if len(closes) < 2:
        return 0.0
    path = float(np.sum(np.abs(np.diff(closes))))
    if path <= 0:
        return 0.0
    return max(0.0, min(1.0, float(gross_points) / path))


def oracle_id(symbol: str, direction: str, entry_date: pd.Timestamp, exit_date: pd.Timestamp) -> str:
    raw = f"{symbol}|{direction}|{entry_date.isoformat()}|{exit_date.isoformat()}"
    return hashlib.sha1(raw.encode("utf-8")).hexdigest()[:24]


def build_segments_for_symbol(
    run_id: str,
    timeframe: str,
    timeframe_minutes: int,
    symbol_rows: pd.DataFrame,
    years: set[int],
    args: argparse.Namespace,
) -> list[dict[str, Any]]:
    symbol_rows = add_atr(symbol_rows.sort_values("ts_utc").reset_index(drop=True), args.atr_period)
    pivots = alternating_pivots(raw_pivots(symbol_rows, args.pivot_window))
    rows: list[dict[str, Any]] = []
    if len(pivots) < 2:
        return rows
    symbol = str(symbol_rows["symbol"].iloc[0])
    root = str(symbol_rows["root_symbol"].iloc[0] or wave.root_symbol(symbol))
    tick_size = float(wave.tick_size_for(symbol, root))
    slippage_ticks = float(args.slippage_entry_ticks + args.slippage_exit_ticks)
    slippage_points = slippage_ticks * tick_size
    for left, right in zip(pivots, pivots[1:]):
        start_idx = int(left["idx"])
        end_idx = int(right["idx"])
        duration_bars = end_idx - start_idx
        if duration_bars < int(args.min_bars) or duration_bars > int(args.max_bars):
            continue
        entry_date = pd.Timestamp(symbol_rows["ts_utc"].iloc[start_idx])
        valid_year = int(entry_date.year)
        if valid_year not in years:
            continue
        if left["kind"] == "LOW" and right["kind"] == "HIGH":
            direction = "LONG"
            entry_price = float(symbol_rows["low"].iloc[start_idx])
            exit_price = float(symbol_rows["high"].iloc[end_idx])
            segment = symbol_rows.iloc[start_idx : end_idx + 1]
            gross_points = exit_price - entry_price
            max_favorable_points = float(segment["high"].max()) - entry_price
            max_adverse_points = max(0.0, entry_price - float(segment["low"].min()))
            start_kind = "LOW"
            end_kind = "HIGH"
        elif left["kind"] == "HIGH" and right["kind"] == "LOW":
            direction = "SHORT"
            entry_price = float(symbol_rows["high"].iloc[start_idx])
            exit_price = float(symbol_rows["low"].iloc[end_idx])
            segment = symbol_rows.iloc[start_idx : end_idx + 1]
            gross_points = entry_price - exit_price
            max_favorable_points = entry_price - float(segment["low"].min())
            max_adverse_points = max(0.0, float(segment["high"].max()) - entry_price)
            start_kind = "HIGH"
            end_kind = "LOW"
        else:
            continue
        if gross_points <= 0:
            continue
        atr = float(symbol_rows["atr"].iloc[start_idx]) if pd.notna(symbol_rows["atr"].iloc[start_idx]) else 0.0
        risk_points = max(atr * float(args.risk_atr_mult), float(args.min_risk_ticks) * tick_size)
        if risk_points <= 0:
            continue
        result_r = (gross_points - slippage_points) / risk_points
        max_adverse_r = max_adverse_points / risk_points
        max_favorable_r = max_favorable_points / risk_points
        efficiency = path_efficiency(symbol_rows, start_idx, end_idx, gross_points)
        if result_r < float(args.min_result_r):
            continue
        if max_adverse_r > float(args.max_adverse_r):
            continue
        if efficiency < float(args.min_efficiency):
            continue
        risk_ticks = risk_points / tick_size
        stop_price = entry_price - risk_points if direction == "LONG" else entry_price + risk_points
        quality_score = result_r * max(0.05, efficiency) / (1.0 + max_adverse_r)
        exit_date = pd.Timestamp(symbol_rows["ts_utc"].iloc[end_idx])
        rows.append(
            {
                "run_id": run_id,
                "oracle_trade_id": oracle_id(symbol, direction, entry_date, exit_date),
                "timeframe": timeframe,
                "valid_year": valid_year,
                "root_symbol": root,
                "symbol": symbol,
                "direction": direction,
                "entry_idx": start_idx,
                "exit_idx": end_idx,
                "entry_date": entry_date.to_pydatetime(),
                "exit_date": exit_date.to_pydatetime(),
                "entry_price": entry_price,
                "exit_price": exit_price,
                "stop_price": stop_price,
                "target_price": exit_price,
                "risk_points": risk_points,
                "risk_ticks": risk_ticks,
                "tick_size": tick_size,
                "entry_atr": atr,
                "gross_points": gross_points,
                "gross_ticks": gross_points / tick_size,
                "slippage_ticks": slippage_ticks,
                "result_r": result_r,
                "max_favorable_r": max_favorable_r,
                "max_adverse_r": max_adverse_r,
                "duration_bars": duration_bars,
                "duration_minutes": duration_bars * timeframe_minutes,
                "efficiency": efficiency,
                "quality_score": quality_score,
                "start_pivot_type": start_kind,
                "end_pivot_type": end_kind,
                "outcome": "WIN",
            }
        )
    return rows


INSERT_COLUMNS = [
    "run_id",
    "oracle_trade_id",
    "timeframe",
    "valid_year",
    "root_symbol",
    "symbol",
    "direction",
    "entry_idx",
    "exit_idx",
    "entry_date",
    "exit_date",
    "entry_price",
    "exit_price",
    "stop_price",
    "target_price",
    "risk_points",
    "risk_ticks",
    "tick_size",
    "entry_atr",
    "gross_points",
    "gross_ticks",
    "slippage_ticks",
    "result_r",
    "max_favorable_r",
    "max_adverse_r",
    "duration_bars",
    "duration_minutes",
    "efficiency",
    "quality_score",
    "start_pivot_type",
    "end_pivot_type",
    "outcome",
]


def insert_segments(conn, rows: list[dict[str, Any]], batch_size: int) -> None:
    if not rows:
        return
    placeholders = "(" + ",".join(["%s"] * len(INSERT_COLUMNS)) + ")"
    update_columns = [col for col in INSERT_COLUMNS if col not in {"run_id", "oracle_trade_id"}]
    sql = (
        f"INSERT INTO {TRADE_TABLE} ({','.join(INSERT_COLUMNS)}) VALUES "
        + ",".join([placeholders] * min(batch_size, len(rows)))
    )
    # Rebuild SQL per chunk so executemany is not needed with variable chunk sizes.
    with conn.cursor() as cur:
        for start in range(0, len(rows), batch_size):
            chunk = rows[start : start + batch_size]
            sql = (
                f"INSERT INTO {TRADE_TABLE} ({','.join(INSERT_COLUMNS)}) VALUES "
                + ",".join([placeholders] * len(chunk))
                + " ON DUPLICATE KEY UPDATE "
                + ",".join([f"{col}=VALUES({col})" for col in update_columns])
            )
            values: list[Any] = []
            for row in chunk:
                values.extend(row.get(col) for col in INSERT_COLUMNS)
            cur.execute(sql, values)
    conn.commit()


def summarize(rows: list[dict[str, Any]]) -> dict[str, float]:
    if not rows:
        return {"total": 0, "total_r": 0.0, "avg_r": 0.0, "avg_quality": 0.0, "min_r": 0.0, "max_r": 0.0}
    result = np.array([float(row["result_r"]) for row in rows], dtype=float)
    quality = np.array([float(row["quality_score"]) for row in rows], dtype=float)
    return {
        "total": int(len(rows)),
        "total_r": float(result.sum()),
        "avg_r": float(result.mean()),
        "avg_quality": float(quality.mean()),
        "min_r": float(result.min()),
        "max_r": float(result.max()),
    }


def insert_run(conn, run_id: str, args: argparse.Namespace, years: list[int], roots: list[str], summary: dict[str, float]) -> None:
    params = {
        "pivot_window": args.pivot_window,
        "atr_period": args.atr_period,
        "risk_atr_mult": args.risk_atr_mult,
        "min_risk_ticks": args.min_risk_ticks,
        "slippage_entry_ticks": args.slippage_entry_ticks,
        "slippage_exit_ticks": args.slippage_exit_ticks,
        "min_result_r": args.min_result_r,
        "max_adverse_r": args.max_adverse_r,
        "min_efficiency": args.min_efficiency,
        "min_bars": args.min_bars,
        "max_bars": args.max_bars,
    }
    with conn.cursor() as cur:
        cur.execute(
            f"""
            INSERT INTO {RUN_TABLE} (
                run_id, created_at, timeframe, years, roots, parameters_json,
                total_trades, total_r, avg_r, avg_quality, min_result_r, max_result_r
            )
            VALUES (%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s)
            ON DUPLICATE KEY UPDATE
                created_at=VALUES(created_at),
                timeframe=VALUES(timeframe),
                years=VALUES(years),
                roots=VALUES(roots),
                parameters_json=VALUES(parameters_json),
                total_trades=VALUES(total_trades),
                total_r=VALUES(total_r),
                avg_r=VALUES(avg_r),
                avg_quality=VALUES(avg_quality),
                min_result_r=VALUES(min_result_r),
                max_result_r=VALUES(max_result_r)
            """,
            (
                run_id,
                datetime.now(timezone.utc).replace(tzinfo=None),
                args.timeframe,
                ",".join(str(year) for year in years),
                ",".join(roots),
                json.dumps(params),
                int(summary["total"]),
                float(summary["total_r"]),
                float(summary["avg_r"]),
                float(summary["avg_quality"]),
                float(summary["min_r"]),
                float(summary["max_r"]),
            ),
        )
    conn.commit()


def main() -> int:
    args = parse_args()
    years = parse_years(args.years)
    roots = parse_list(args.roots)
    run_id = make_run_id(args, years)
    timeframe_minutes = scanner.table_for_timeframe(args.timeframe)[1]
    print(f"Building oracle perfect trend trades run={run_id} timeframe={args.timeframe} years={years} roots={roots or 'ALL'}")
    conn = wave.connect()
    try:
        ensure_tables(conn)
        if args.replace_run:
            delete_run(conn, run_id)
        else:
            with conn.cursor() as cur:
                cur.execute(f"SELECT COUNT(*) AS n FROM {RUN_TABLE} WHERE run_id = %s", (run_id,))
                if int(cur.fetchone()["n"]) > 0:
                    raise ValueError(f"Run already exists: {run_id}. Use --replace-run to overwrite.")
        candles = fetch_candles(conn, args.timeframe, years, roots)
        if candles.empty:
            raise ValueError("No candles found for oracle build.")
        print(f"Loaded candles={len(candles):,} symbols={candles['symbol'].nunique():,}")
        all_rows: list[dict[str, Any]] = []
        grouped = list(candles.groupby("symbol", sort=True))
        if int(args.limit_symbols or 0) > 0:
            grouped = grouped[: int(args.limit_symbols)]
        years_set = set(years)
        for idx, (symbol, group) in enumerate(grouped, start=1):
            rows = build_segments_for_symbol(run_id, args.timeframe, timeframe_minutes, group, years_set, args)
            all_rows.extend(rows)
            if idx % 50 == 0 or idx == len(grouped):
                print(f"symbols {idx:,}/{len(grouped):,} oracle_trades={len(all_rows):,}")
        insert_segments(conn, all_rows, max(1, int(args.batch_size)))
        summary = summarize(all_rows)
        insert_run(conn, run_id, args, years, roots, summary)
        print(
            f"saved run={run_id} oracle_trades={summary['total']:,} total_r={summary['total_r']:.2f}R "
            f"avg_r={summary['avg_r']:.2f}R min_r={summary['min_r']:.2f}R max_r={summary['max_r']:.2f}R"
        )
        if all_rows:
            frame = pd.DataFrame(all_rows)
            print("by_year")
            print(frame.groupby("valid_year")["result_r"].agg(["count", "sum", "mean"]).round(2).to_string())
            print("top_roots")
            print(frame.groupby("root_symbol")["result_r"].agg(["count", "sum", "mean"]).sort_values("sum", ascending=False).head(12).round(2).to_string())
    finally:
        conn.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
