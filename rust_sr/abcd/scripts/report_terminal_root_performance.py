#!/usr/bin/env python3
"""
Summarize current live-style terminal trade performance by futures root.

The report intentionally reads the same terminal trade files used by the
current trade-quality research harness, plus the saved quality-layer selections.
That keeps root comparison focused on the active candidate data instead of
mixing in older abandoned experiment artifacts.
"""

from __future__ import annotations

import argparse
import os
import re
from pathlib import Path
from typing import Iterable
from urllib.parse import urlparse

import pandas as pd
import pymysql


SCRIPT_DIR = Path(__file__).resolve().parent
ABCD_ROOT = SCRIPT_DIR.parent
MODEL_REGISTRY = ABCD_ROOT / "model_registry"

TERMINAL_INPUTS = {
    "terminal_all_2025": MODEL_REGISTRY
    / "aicw-os-stage3-terminal-both-threshold-v2-2m-m180-v2025"
    / "stage3_terminal_hold_trades_2025.csv",
    "terminal_all_2026": MODEL_REGISTRY
    / "aicw-os-stage3-terminal-both-full-v2-2m-m180-v2026"
    / "stage3_terminal_hold_trades_2026.csv",
}

QUALITY_INPUTS = {
    "quality_v2_valid2025": MODEL_REGISTRY
    / "aicw-terminal-trade-quality-v2-stage2features-2025-2026"
    / "quality_best_valid_trades_train2026_valid2025.csv",
    "quality_v2_valid2026": MODEL_REGISTRY
    / "aicw-terminal-trade-quality-v2-stage2features-2025-2026"
    / "quality_best_valid_trades_train2025_valid2026.csv",
}

DEFAULT_OUTPUT = MODEL_REGISTRY / "root_performance_reports" / "current_terminal_2025_2026"

TIMEFRAME_TABLES = {
    "1m": "futures_contract_1m_candles",
    "2m": "futures_contract_2m_candles",
    "3m": "futures_contract_3m_candles",
    "4m": "futures_contract_4m_candles",
    "5m": "futures_contract_5m_candles",
    "6m": "futures_contract_6m_candles",
    "7m": "futures_contract_7m_candles",
    "8m": "futures_contract_8m_candles",
    "9m": "futures_contract_9m_candles",
    "10m": "futures_contract_10m_candles",
    "11m": "futures_contract_11m_candles",
    "12m": "futures_contract_12m_candles",
    "13m": "futures_contract_13m_candles",
    "14m": "futures_contract_14m_candles",
    "15m": "futures_contract_15m_candles",
    "30m": "futures_contract_30m_candles",
    "1h": "futures_contract_1h_candles",
    "4h": "futures_contract_4h_candles",
    "12h": "futures_contract_12h_candles",
    "1d": "futures_contract_1d_candles",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--timeframe", default="2m", choices=sorted(TIMEFRAME_TABLES))
    parser.add_argument("--replace", action="store_true")
    return parser.parse_args()


def infer_year(label: str, path: Path) -> int:
    candidates = [label, path.stem, path.parent.name]
    for value in candidates:
        match = re.search(r"(?:valid|v)?(20\d{2})", value)
        if match:
            return int(match.group(1))
    raise ValueError(f"Could not infer year from {label}: {path}")


def first_present(columns: Iterable[str], options: list[str]) -> str | None:
    column_set = set(columns)
    for option in options:
        if option in column_set:
            return option
    return None


def read_database_url() -> str:
    for key in ["DATABASE_URL", "ABCD_DATABASE_URL"]:
        if os.environ.get(key):
            return str(os.environ[key])

    env_path = ABCD_ROOT / ".env"
    if not env_path.exists():
        raise FileNotFoundError(f"Could not find DATABASE_URL, ABCD_DATABASE_URL, or {env_path}")

    for line in env_path.read_text(encoding="utf-8").splitlines():
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
        autocommit=True,
        cursorclass=pymysql.cursors.DictCursor,
    )


def safe_identifier(value: str) -> str:
    if not re.fullmatch(r"[A-Za-z0-9_]+", value):
        raise ValueError(f"Unsafe SQL identifier: {value!r}")
    return value


def max_drawdown_r(results: list[float]) -> float:
    equity = 0.0
    peak = 0.0
    max_dd = 0.0
    for result in results:
        equity += float(result)
        peak = max(peak, equity)
        max_dd = max(max_dd, peak - equity)
    return max_dd


def load_trade_file(label: str, path: Path) -> pd.DataFrame:
    if not path.exists():
        raise FileNotFoundError(path)

    frame = pd.read_csv(path)
    result_column = first_present(frame.columns, ["result_r", "model_result_r", "terminal_result_r"])
    entry_column = first_present(frame.columns, ["entry_date", "entry_time", "signal_date"])
    exit_column = first_present(frame.columns, ["exit_date", "model_exit_date", "terminal_exit_date"])
    required = {
        "root_symbol": "root_symbol",
        "symbol": "symbol",
        "direction": "direction",
        "result_r": result_column,
        "entry_date": entry_column,
        "exit_date": exit_column,
    }
    missing = [name for name, column in required.items() if column is None or column not in frame.columns]
    if missing:
        raise ValueError(f"{path} missing required fields: {missing}")

    out = pd.DataFrame(
        {
            "dataset": label,
            "source_file": str(path),
            "year": infer_year(label, path),
            "root_symbol": frame[required["root_symbol"]].astype(str).str.upper(),
            "symbol": frame[required["symbol"]].astype(str).str.upper(),
            "direction": frame[required["direction"]].astype(str).str.upper(),
            "entry_date": pd.to_datetime(frame[required["entry_date"]], errors="coerce"),
            "exit_date": pd.to_datetime(frame[required["exit_date"]], errors="coerce"),
            "result_r": pd.to_numeric(frame[required["result_r"]], errors="coerce"),
        }
    )
    if "risk_ticks" in frame.columns:
        out["risk_ticks"] = pd.to_numeric(frame["risk_ticks"], errors="coerce")
    if "stage2_score" in frame.columns:
        out["stage2_score"] = pd.to_numeric(frame["stage2_score"], errors="coerce")
    if "predicted_r" in frame.columns:
        out["stage2_score"] = pd.to_numeric(frame["predicted_r"], errors="coerce")
    if "quality_score" in frame.columns:
        out["quality_score"] = pd.to_numeric(frame["quality_score"], errors="coerce")
    if "model_hold_minutes" in frame.columns:
        out["hold_minutes"] = pd.to_numeric(frame["model_hold_minutes"], errors="coerce")
    elif "hold_minutes" in frame.columns:
        out["hold_minutes"] = pd.to_numeric(frame["hold_minutes"], errors="coerce")

    out = out.dropna(subset=["entry_date", "exit_date", "result_r"])
    return out.sort_values(["entry_date", "exit_date", "symbol"]).reset_index(drop=True)


def load_candle_universe(timeframe: str) -> pd.DataFrame:
    table_name = safe_identifier(TIMEFRAME_TABLES[timeframe])
    with connect() as conn:
        with conn.cursor() as cur:
            cur.execute(
                f"""
                SELECT
                    root_symbol,
                    symbol,
                    COUNT(*) AS candle_count,
                    MIN(ts_utc) AS first_ts,
                    MAX(ts_utc) AS last_ts,
                    MIN(YEAR(ts_utc)) AS first_year,
                    MAX(YEAR(ts_utc)) AS last_year,
                    COUNT(DISTINCT YEAR(ts_utc)) AS year_count
                FROM {table_name}
                GROUP BY root_symbol, symbol
                ORDER BY root_symbol, symbol
                """
            )
            rows = cur.fetchall()
    frame = pd.DataFrame(rows)
    if frame.empty:
        return frame
    frame["root_symbol"] = frame["root_symbol"].astype(str).str.upper()
    frame["symbol"] = frame["symbol"].astype(str).str.upper()
    frame["first_ts"] = pd.to_datetime(frame["first_ts"], errors="coerce")
    frame["last_ts"] = pd.to_datetime(frame["last_ts"], errors="coerce")
    for column in ["candle_count", "first_year", "last_year", "year_count"]:
        frame[column] = pd.to_numeric(frame[column], errors="coerce").fillna(0).astype(int)
    return frame


def summarize_group(group: pd.DataFrame, keys: list[str]) -> dict[str, object]:
    ordered = group.sort_values(["entry_date", "exit_date", "symbol"])
    results = ordered["result_r"].astype(float).tolist()
    trades = len(results)
    wins = sum(1 for result in results if result > 0)
    losses = sum(1 for result in results if result < 0)
    flats = trades - wins - losses
    gross_win = sum(result for result in results if result > 0)
    gross_loss = sum(result for result in results if result < 0)
    row: dict[str, object] = {
        "trades": trades,
        "wins": wins,
        "losses": losses,
        "flats": flats,
        "win_rate": wins / trades if trades else 0.0,
        "sum_r": sum(results),
        "avg_r": sum(results) / trades if trades else 0.0,
        "gross_win_r": gross_win,
        "gross_loss_r": gross_loss,
        "profit_factor_r": gross_win / abs(gross_loss) if gross_loss else None,
        "max_drawdown_r": max_drawdown_r(results),
        "first_entry": ordered["entry_date"].min(),
        "last_exit": ordered["exit_date"].max(),
        "unique_symbols": ordered["symbol"].nunique(),
        "long_trades": int((ordered["direction"] == "LONG").sum()),
        "short_trades": int((ordered["direction"] == "SHORT").sum()),
    }
    if "risk_ticks" in ordered.columns:
        row["avg_risk_ticks"] = ordered["risk_ticks"].mean()
        row["median_risk_ticks"] = ordered["risk_ticks"].median()
    if "stage2_score" in ordered.columns:
        row["avg_stage2_score"] = ordered["stage2_score"].mean()
    if "quality_score" in ordered.columns:
        row["avg_quality_score"] = ordered["quality_score"].mean()
    if "hold_minutes" in ordered.columns:
        row["avg_hold_minutes"] = ordered["hold_minutes"].mean()
        row["median_hold_minutes"] = ordered["hold_minutes"].median()

    for key in keys:
        row[key] = ordered[key].iloc[0]
    return row


def summarize_by_keys(frame: pd.DataFrame, keys: list[str]) -> pd.DataFrame:
    rows = [summarize_group(group, keys) for _, group in frame.groupby(keys, dropna=False)]
    out = pd.DataFrame(rows)
    lead_cols = keys + [
        "trades",
        "sum_r",
        "max_drawdown_r",
        "avg_r",
        "win_rate",
        "profit_factor_r",
        "wins",
        "losses",
        "flats",
    ]
    ordered_cols = [column for column in lead_cols if column in out.columns]
    ordered_cols += [column for column in out.columns if column not in ordered_cols]
    return out[ordered_cols].sort_values(["sum_r", "avg_r"], ascending=[False, False]).reset_index(drop=True)


def summarize_combined(frame: pd.DataFrame, dataset_name: str) -> pd.DataFrame:
    combined = summarize_by_keys(frame.assign(dataset_group=dataset_name), ["dataset_group", "root_symbol"])
    yearly = summarize_by_keys(frame.assign(dataset_group=dataset_name), ["dataset_group", "root_symbol", "year"])
    year_stats = (
        yearly.groupby(["dataset_group", "root_symbol"], dropna=False)
        .agg(
            years=("year", lambda values: ",".join(str(int(value)) for value in sorted(values))),
            positive_years=("sum_r", lambda values: int((values > 0).sum())),
            negative_years=("sum_r", lambda values: int((values < 0).sum())),
            min_year_r=("sum_r", "min"),
            max_year_r=("sum_r", "max"),
        )
        .reset_index()
    )
    out = combined.merge(year_stats, on=["dataset_group", "root_symbol"], how="left")
    out["consistency_pct"] = out["positive_years"] / (
        out["positive_years"] + out["negative_years"]
    ).clip(lower=1)
    return out.sort_values(["sum_r", "avg_r"], ascending=[False, False]).reset_index(drop=True)


def summarize_universe_roots(candle_universe: pd.DataFrame, timeframe: str) -> pd.DataFrame:
    if candle_universe.empty:
        return candle_universe
    grouped = (
        candle_universe.groupby("root_symbol", dropna=False)
        .agg(
            timeframe=("root_symbol", lambda _: timeframe),
            symbols_in_candle_data=("symbol", "nunique"),
            candle_count=("candle_count", "sum"),
            first_ts=("first_ts", "min"),
            last_ts=("last_ts", "max"),
            first_year=("first_year", "min"),
            last_year=("last_year", "max"),
            max_symbol_year_count=("year_count", "max"),
        )
        .reset_index()
    )
    return grouped.sort_values(["root_symbol"]).reset_index(drop=True)


def add_candle_coverage(root_summary: pd.DataFrame, trade_summary: pd.DataFrame) -> pd.DataFrame:
    if root_summary.empty:
        return root_summary
    trade_columns = [
        "root_symbol",
        "years",
        "trades",
        "sum_r",
        "max_drawdown_r",
        "avg_r",
        "win_rate",
        "profit_factor_r",
        "positive_years",
        "negative_years",
        "min_year_r",
        "max_year_r",
    ]
    trade_view = trade_summary[[column for column in trade_columns if column in trade_summary.columns]].copy()
    out = root_summary.merge(trade_view, on="root_symbol", how="left")
    out["terminal_status"] = out["trades"].fillna(0).map(lambda value: "tested" if int(value) > 0 else "not_tested")
    for column in ["trades", "positive_years", "negative_years"]:
        if column in out.columns:
            out[column] = out[column].fillna(0).astype(int)
    for column in ["sum_r", "max_drawdown_r", "avg_r", "win_rate", "profit_factor_r", "min_year_r", "max_year_r"]:
        if column in out.columns:
            out[column] = out[column].fillna(0.0)
    out["years"] = out["years"].fillna("")
    lead = [
        "root_symbol",
        "terminal_status",
        "timeframe",
        "symbols_in_candle_data",
        "candle_count",
        "first_year",
        "last_year",
        "first_ts",
        "last_ts",
        "trades",
        "sum_r",
        "max_drawdown_r",
        "avg_r",
        "win_rate",
    ]
    columns = [column for column in lead if column in out.columns]
    columns += [column for column in out.columns if column not in columns]
    return out[columns].sort_values(["terminal_status", "sum_r", "root_symbol"], ascending=[True, False, True])


def add_symbol_coverage(candle_universe: pd.DataFrame, terminal: pd.DataFrame) -> pd.DataFrame:
    if candle_universe.empty:
        return candle_universe
    symbol_trades = summarize_by_keys(
        terminal.assign(dataset_group="terminal_all_available"),
        ["dataset_group", "root_symbol", "symbol"],
    )
    trade_columns = [
        "root_symbol",
        "symbol",
        "trades",
        "sum_r",
        "max_drawdown_r",
        "avg_r",
        "win_rate",
        "profit_factor_r",
    ]
    trade_view = symbol_trades[[column for column in trade_columns if column in symbol_trades.columns]].copy()
    out = candle_universe.merge(trade_view, on=["root_symbol", "symbol"], how="left")
    out["terminal_status"] = out["trades"].fillna(0).map(lambda value: "tested" if int(value) > 0 else "not_tested")
    out["trades"] = out["trades"].fillna(0).astype(int)
    for column in ["sum_r", "max_drawdown_r", "avg_r", "win_rate", "profit_factor_r"]:
        if column in out.columns:
            out[column] = out[column].fillna(0.0)
    lead = [
        "root_symbol",
        "symbol",
        "terminal_status",
        "candle_count",
        "first_year",
        "last_year",
        "first_ts",
        "last_ts",
        "trades",
        "sum_r",
        "max_drawdown_r",
        "avg_r",
        "win_rate",
    ]
    columns = [column for column in lead if column in out.columns]
    columns += [column for column in out.columns if column not in columns]
    return out[columns].sort_values(["root_symbol", "symbol"]).reset_index(drop=True)


def format_table(frame: pd.DataFrame, columns: list[str], limit: int = 20) -> str:
    available = [column for column in columns if column in frame.columns]
    view = frame[available].head(limit).copy()
    for column in view.columns:
        if pd.api.types.is_float_dtype(view[column]):
            view[column] = view[column].map(lambda value: "" if pd.isna(value) else f"{value:.3f}")
    view = view.fillna("").astype(str)
    widths = {
        column: max(len(column), *(len(value) for value in view[column].tolist()))
        for column in view.columns
    }
    header = "| " + " | ".join(column.ljust(widths[column]) for column in view.columns) + " |"
    divider = "| " + " | ".join("-" * widths[column] for column in view.columns) + " |"
    rows = [
        "| " + " | ".join(row[column].ljust(widths[column]) for column in view.columns) + " |"
        for _, row in view.iterrows()
    ]
    return "\n".join([header, divider, *rows])


def write_report(
    output_dir: Path,
    terminal: pd.DataFrame,
    quality: pd.DataFrame,
    candle_universe: pd.DataFrame,
    timeframe: str,
) -> None:
    terminal_year = summarize_by_keys(terminal, ["dataset", "year", "root_symbol"])
    terminal_combined = summarize_combined(terminal, "terminal_all_available")
    quality_year = summarize_by_keys(quality, ["dataset", "year", "root_symbol"])
    quality_combined = summarize_combined(quality, "quality_v2_selected_available")
    universe_roots = summarize_universe_roots(candle_universe, timeframe)
    terminal_root_coverage = add_candle_coverage(universe_roots, terminal_combined)
    terminal_symbol_coverage = add_symbol_coverage(candle_universe, terminal)

    terminal_year.to_csv(output_dir / "terminal_root_year_summary.csv", index=False)
    terminal_combined.to_csv(output_dir / "terminal_root_combined_summary.csv", index=False)
    quality_year.to_csv(output_dir / "quality_v2_root_year_summary.csv", index=False)
    quality_combined.to_csv(output_dir / "quality_v2_root_combined_summary.csv", index=False)
    universe_roots.to_csv(output_dir / f"candle_universe_{timeframe}_root_summary.csv", index=False)
    candle_universe.to_csv(output_dir / f"candle_universe_{timeframe}_symbol_summary.csv", index=False)
    terminal_root_coverage.to_csv(output_dir / f"terminal_vs_candle_universe_{timeframe}_root_coverage.csv", index=False)
    terminal_symbol_coverage.to_csv(output_dir / f"terminal_vs_candle_universe_{timeframe}_symbol_coverage.csv", index=False)
    terminal.to_csv(output_dir / "terminal_trade_rows_used.csv", index=False)
    quality.to_csv(output_dir / "quality_v2_trade_rows_used.csv", index=False)

    tested_roots = int((terminal_root_coverage["terminal_status"] == "tested").sum()) if not terminal_root_coverage.empty else 0
    untested_roots = int((terminal_root_coverage["terminal_status"] == "not_tested").sum()) if not terminal_root_coverage.empty else 0
    tested_symbols = int((terminal_symbol_coverage["terminal_status"] == "tested").sum()) if not terminal_symbol_coverage.empty else 0
    untested_symbols = int((terminal_symbol_coverage["terminal_status"] == "not_tested").sum()) if not terminal_symbol_coverage.empty else 0

    note = "\n".join(
        [
            "# Root Performance Report",
            "",
            "This report shows each individual root using the current saved trade rows.",
            "",
            f"Important: performance only exists where terminal trade rows were actually generated. The {timeframe} candle universe has {len(universe_roots):,} roots and {len(candle_universe):,} contract symbols; the saved terminal rows currently cover {tested_roots:,} roots and {tested_symbols:,} contract symbols.",
            "",
            f"Untested in current terminal rows: {untested_roots:,} roots and {untested_symbols:,} contract symbols. Those roots/symbols have candle data but no saved terminal performance yet.",
            "",
            "## Terminal Coverage Vs Candle Universe",
            "",
            format_table(
                terminal_root_coverage,
                [
                    "root_symbol",
                    "terminal_status",
                    "symbols_in_candle_data",
                    "candle_count",
                    "first_year",
                    "last_year",
                    "trades",
                    "sum_r",
                    "max_drawdown_r",
                    "avg_r",
                    "win_rate",
                ],
                limit=80,
            ),
            "",
            "## Terminal All Available Roots",
            "",
            format_table(
                terminal_combined,
                [
                    "root_symbol",
                    "years",
                    "trades",
                    "sum_r",
                    "max_drawdown_r",
                    "avg_r",
                    "win_rate",
                    "profit_factor_r",
                    "positive_years",
                    "negative_years",
                    "min_year_r",
                    "max_year_r",
                ],
            ),
            "",
            "## Terminal Root By Year",
            "",
            format_table(
                terminal_year,
                [
                    "year",
                    "root_symbol",
                    "trades",
                    "sum_r",
                    "max_drawdown_r",
                    "avg_r",
                    "win_rate",
                    "profit_factor_r",
                ],
                limit=50,
            ),
            "",
            "## Quality V2 Selected Roots",
            "",
            format_table(
                quality_combined,
                [
                    "root_symbol",
                    "years",
                    "trades",
                    "sum_r",
                    "max_drawdown_r",
                    "avg_r",
                    "win_rate",
                    "profit_factor_r",
                    "positive_years",
                    "negative_years",
                    "min_year_r",
                    "max_year_r",
                ],
            ),
            "",
            "## Files",
            "",
            "- terminal_root_combined_summary.csv",
            "- terminal_root_year_summary.csv",
            "- quality_v2_root_combined_summary.csv",
            "- quality_v2_root_year_summary.csv",
            f"- candle_universe_{timeframe}_root_summary.csv",
            f"- candle_universe_{timeframe}_symbol_summary.csv",
            f"- terminal_vs_candle_universe_{timeframe}_root_coverage.csv",
            f"- terminal_vs_candle_universe_{timeframe}_symbol_coverage.csv",
            "- terminal_trade_rows_used.csv",
            "- quality_v2_trade_rows_used.csv",
        ]
    )
    (output_dir / "summary.md").write_text(note + "\n", encoding="utf-8")


def main() -> None:
    args = parse_args()
    output_dir = args.output_dir
    if output_dir.exists() and not args.replace:
        raise SystemExit(f"{output_dir} already exists. Use --replace.")
    output_dir.mkdir(parents=True, exist_ok=True)

    terminal = pd.concat(
        [load_trade_file(label, path) for label, path in TERMINAL_INPUTS.items()],
        ignore_index=True,
    )
    quality = pd.concat(
        [load_trade_file(label, path) for label, path in QUALITY_INPUTS.items()],
        ignore_index=True,
    )
    candle_universe = load_candle_universe(args.timeframe)

    write_report(output_dir, terminal, quality, candle_universe, args.timeframe)
    print(f"Wrote root report: {output_dir}")
    print(f"Terminal rows: {len(terminal):,}")
    print(f"Quality V2 rows: {len(quality):,}")
    print(f"{args.timeframe} candle universe roots: {candle_universe['root_symbol'].nunique():,}")
    print(f"{args.timeframe} candle universe symbols: {candle_universe['symbol'].nunique():,}")


if __name__ == "__main__":
    main()
