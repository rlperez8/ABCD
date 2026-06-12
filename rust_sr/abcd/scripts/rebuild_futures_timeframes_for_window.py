from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

import ai_wave_rider_research as wave


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Rebuild aggregate futures candles for symbols active in a DB time window.")
    parser.add_argument("--start-ts", required=True)
    parser.add_argument("--end-ts", required=True)
    parser.add_argument("--timeframes", default="2m,5m,15m,1h,4h")
    parser.add_argument("--root", default="", help="Optional single root filter.")
    parser.add_argument("--symbol", default="", help="Optional single symbol filter.")
    parser.add_argument("--limit", type=int, default=0)
    return parser.parse_args()


def selected_symbols(args: argparse.Namespace) -> list[str]:
    start_ts = args.start_ts.replace("T", " ")
    end_ts = args.end_ts.replace("T", " ")
    where = ["ts_utc >= %s", "ts_utc < %s"]
    params: list[object] = [start_ts, end_ts]
    if args.root:
        where.append("root_symbol = %s")
        params.append(args.root.upper())
    if args.symbol:
        where.append("symbol = %s")
        params.append(args.symbol.upper())
    sql = f"""
        SELECT DISTINCT symbol
        FROM futures_contract_1m_candles
        WHERE {' AND '.join(where)}
        ORDER BY symbol
    """
    if args.limit > 0:
        sql += " LIMIT %s"
        params.append(args.limit)
    conn = wave.connect()
    try:
        with conn.cursor() as cur:
            cur.execute(sql, params)
            return [str(row["symbol"]) for row in cur.fetchall()]
    finally:
        conn.close()


def main() -> int:
    args = parse_args()
    symbols = selected_symbols(args)
    if not symbols:
        print("No symbols found for requested window.")
        return 0

    exe = wave.ABCD_ROOT / "target" / "debug" / "build_futures_timeframe_candles.exe"
    if not exe.exists():
        raise FileNotFoundError(f"Missing aggregate candle builder: {exe}")

    env = os.environ.copy()
    env["ABCD_TIMEFRAMES"] = args.timeframes
    env["ABCD_START_TS"] = args.start_ts.replace("T", " ")
    env["ABCD_END_TS"] = args.end_ts.replace("T", " ")
    env["ABCD_REBUILD_TIMEFRAME_CANDLES"] = "1"
    env.pop("ABCD_FUTURES_ROOT", None)
    print(
        f"Rebuilding {args.timeframes} candles for {len(symbols):,} symbols "
        f"from {args.start_ts} to {args.end_ts}"
    )
    for index, symbol in enumerate(symbols, start=1):
        env["ABCD_SYMBOL"] = symbol
        print(f"[{index:,}/{len(symbols):,}] {symbol}", flush=True)
        result = subprocess.run([str(exe)], cwd=str(wave.ABCD_ROOT), env=env)
        if result.returncode != 0:
            print(f"Failed on {symbol} with exit={result.returncode}", file=sys.stderr)
            return result.returncode
    print("Done.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
