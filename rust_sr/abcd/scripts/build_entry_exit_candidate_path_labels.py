#!/usr/bin/env python3
"""
Build forward path labels for entry/exit candidate rows.

The labels are keyed to a candidate result row and use only candles at or after
that candidate's entry. They are stored in the DB so AI/modeling code can use
them without recomputing path details in the UI.
"""

from __future__ import annotations

import argparse
import math
import sys
from datetime import timedelta
from pathlib import Path

import numpy as np
import pandas as pd

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_build_search_catboost as base


DEFAULT_SOURCE_RUN_ID = "eetc-1779837772298-38740"
DEFAULT_RESULTS_TABLE = "entry_exit_template_results_eetc_1779837772298_38740"
THRESHOLDS_R = [0.5, 1.0, 2.0, 3.0, 4.0]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-run-id", default=DEFAULT_SOURCE_RUN_ID)
    parser.add_argument("--results-table", default=DEFAULT_RESULTS_TABLE)
    parser.add_argument("--year", type=int, default=None)
    parser.add_argument("--start-year", type=int, default=None)
    parser.add_argument("--end-year", type=int, default=None)
    parser.add_argument("--limit", type=int, default=1000)
    parser.add_argument("--result-id-file", default=None)
    parser.add_argument("--setup-limit", type=int, default=0)
    parser.add_argument("--setup-sample-mod", type=int, default=7)
    parser.add_argument("--setup-sample-slot", type=int, default=3)
    parser.add_argument("--chunk-size", type=int, default=1000)
    parser.add_argument("--only-missing", action="store_true")
    return parser.parse_args()


def ensure_table(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS entry_exit_candidate_path_labels (
                source_run_id VARCHAR(64) NOT NULL,
                results_table VARCHAR(128) NOT NULL,
                result_id BIGINT NOT NULL,
                setup_id VARCHAR(64) NOT NULL,
                template_uid VARCHAR(128) NOT NULL,
                pattern_id VARCHAR(64) NULL,
                pattern_group_id VARCHAR(128) NULL,
                symbol VARCHAR(32) NOT NULL,
                d_confirm_date DATETIME NULL,
                entry_date DATETIME NOT NULL,
                engine_exit_date DATETIME NULL,
                path_end_date DATETIME NULL,
                entry_price DOUBLE NOT NULL,
                stop_price DOUBLE NOT NULL,
                target_price DOUBLE NOT NULL,
                risk_points DOUBLE NOT NULL,
                target_r DOUBLE NULL,
                trade_direction VARCHAR(16) NOT NULL,
                engine_outcome VARCHAR(16) NULL,
                engine_exit_reason VARCHAR(32) NULL,
                engine_result_r DOUBLE NULL,
                path_candle_count INT NOT NULL DEFAULT 0,
                path_minutes INT NULL,
                first_stop_date DATETIME NULL,
                first_target_date DATETIME NULL,
                first_stop_minutes INT NULL,
                first_target_minutes INT NULL,
                first_hit_outcome VARCHAR(16) NULL,
                both_hit_same_candle TINYINT NOT NULL DEFAULT 0,
                mfe_r DOUBLE NULL,
                mae_r DOUBLE NULL,
                best_close_r DOUBLE NULL,
                worst_close_r DOUBLE NULL,
                end_close_r DOUBLE NULL,
                minutes_to_mfe INT NULL,
                minutes_to_mae INT NULL,
                mae_before_target_r DOUBLE NULL,
                mfe_before_stop_r DOUBLE NULL,
                hit_pos_0_5r_minutes INT NULL,
                hit_pos_1_0r_minutes INT NULL,
                hit_pos_2_0r_minutes INT NULL,
                hit_pos_3_0r_minutes INT NULL,
                hit_pos_4_0r_minutes INT NULL,
                hit_neg_0_5r_minutes INT NULL,
                hit_neg_1_0r_minutes INT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                PRIMARY KEY (source_run_id, results_table, result_id),
                INDEX idx_eec_path_setup (setup_id),
                INDEX idx_eec_path_symbol_entry (symbol, entry_date),
                INDEX idx_eec_path_template (source_run_id, template_uid, mfe_r)
            )
            """
        )
    conn.commit()


def candidate_select_sql(table: str, join_missing: str = "", force_index: str = "FORCE INDEX (idx_entry_exit_template_results_run_setup)") -> str:
    return f"""
        SELECT
            r.id AS result_id,
            r.run_id,
            r.template_uid,
            r.setup_id,
            r.pattern_id,
            r.pattern_group_id,
            r.symbol,
            r.d_confirm_date,
            r.entry_date,
            r.exit_date AS engine_exit_date,
            r.entry_price,
            r.stop_price,
            r.target_price,
            r.exit_price,
            r.risk_points,
            r.trade_direction,
            r.outcome AS engine_outcome,
            r.exit_reason AS engine_exit_reason,
            r.result_r AS engine_result_r,
            t.max_hold_multiple,
            ps.full_pattern_length
        FROM {table} r {force_index}
        JOIN entry_exit_templates t
          ON t.template_uid = r.template_uid
        LEFT JOIN pattern_setups ps
          ON ps.setup_id = r.setup_id
        {join_missing}
    """


def normalize_candidate_frame(frame: pd.DataFrame) -> pd.DataFrame:
    if frame.empty:
        return frame
    for date_col in ["d_confirm_date", "entry_date", "engine_exit_date"]:
        frame[date_col] = pd.to_datetime(frame[date_col], errors="coerce")
    for col in [
        "entry_price",
        "stop_price",
        "target_price",
        "exit_price",
        "risk_points",
        "engine_result_r",
        "max_hold_multiple",
        "full_pattern_length",
    ]:
        frame[col] = pd.to_numeric(frame[col], errors="coerce")
    return frame.dropna(subset=["result_id", "symbol", "entry_date", "entry_price", "stop_price", "target_price", "risk_points"])


def load_candidate_rows_for_setups(conn, args: argparse.Namespace, setup_ids: list[str]) -> pd.DataFrame:
    if not setup_ids:
        return pd.DataFrame()
    table = base.safe_identifier(args.results_table)
    frames: list[pd.DataFrame] = []
    valid_filters = """
          AND r.entry_date IS NOT NULL
          AND r.entry_price IS NOT NULL
          AND r.stop_price IS NOT NULL
          AND r.target_price IS NOT NULL
          AND r.risk_points IS NOT NULL
          AND r.risk_points > 0
    """
    join_missing = ""
    for ids in base.chunked(setup_ids, 40):
        placeholders = ",".join(["%s"] * len(ids))
        params: list[object] = [args.source_run_id, *ids]
        if args.only_missing:
            join_missing = """
                LEFT JOIN entry_exit_candidate_path_labels pl
                  ON pl.source_run_id = r.run_id
                 AND pl.results_table = %s
                 AND pl.result_id = r.id
            """
            params.insert(0, args.results_table)
        missing_filter = "AND pl.result_id IS NULL" if args.only_missing else ""
        query = f"""
            {candidate_select_sql(table, join_missing)}
            WHERE r.run_id = %s
              AND r.setup_id IN ({placeholders})
              {valid_filters}
              {missing_filter}
            ORDER BY r.symbol ASC, r.entry_date ASC, r.id ASC
        """
        with conn.cursor() as cur:
            cur.execute(query, params)
            rows = cur.fetchall()
        if rows:
            frames.append(pd.DataFrame(rows))
    if not frames:
        return pd.DataFrame()
    return normalize_candidate_frame(pd.concat(frames, ignore_index=True))


def load_result_ids_from_file(path: str) -> list[int]:
    frame = pd.read_csv(path)
    if "result_id" not in frame.columns:
        raise ValueError(f"{path} must contain a result_id column")
    result_ids = pd.to_numeric(frame["result_id"], errors="coerce").dropna().astype(np.int64)
    return sorted(set(int(value) for value in result_ids.tolist()))


def load_candidate_rows_for_result_ids(conn, args: argparse.Namespace, result_ids: list[int]) -> pd.DataFrame:
    if not result_ids:
        return pd.DataFrame()
    table = base.safe_identifier(args.results_table)
    frames: list[pd.DataFrame] = []
    valid_filters = """
          AND r.run_id = %s
          AND r.entry_date IS NOT NULL
          AND r.entry_price IS NOT NULL
          AND r.stop_price IS NOT NULL
          AND r.target_price IS NOT NULL
          AND r.risk_points IS NOT NULL
          AND r.risk_points > 0
    """
    join_missing = ""
    for ids in base.chunked([str(value) for value in result_ids], 500):
        placeholders = ",".join(["%s"] * len(ids))
        params: list[object] = [*ids, args.source_run_id]
        if args.only_missing:
            join_missing = """
                LEFT JOIN entry_exit_candidate_path_labels pl
                  ON pl.source_run_id = r.run_id
                 AND pl.results_table = %s
                 AND pl.result_id = r.id
            """
            params.insert(0, args.results_table)
        missing_filter = "AND pl.result_id IS NULL" if args.only_missing else ""
        query = f"""
            {candidate_select_sql(table, join_missing, "FORCE INDEX (PRIMARY)")}
            WHERE r.id IN ({placeholders})
              {valid_filters}
              {missing_filter}
            ORDER BY r.symbol ASC, r.entry_date ASC, r.id ASC
        """
        with conn.cursor() as cur:
            cur.execute(query, params)
            rows = cur.fetchall()
        if rows:
            frames.append(pd.DataFrame(rows))
    if not frames:
        return pd.DataFrame()
    return normalize_candidate_frame(pd.concat(frames, ignore_index=True))


def setup_ids_from_args(conn, args: argparse.Namespace) -> list[str]:
    if not args.setup_limit or args.setup_limit <= 0:
        return []
    reference_template = base.first_template_uid(conn, args.source_run_id)
    if args.year is not None:
        return base.load_setup_ids(
            conn,
            args.results_table,
            args.source_run_id,
            args.year,
            args.setup_limit,
            args.setup_sample_mod,
            args.setup_sample_slot,
            reference_template,
        )
    if args.start_year is not None and args.end_year is not None:
        years = list(range(args.start_year, args.end_year + 1))
        per_year = max(1, args.setup_limit // len(years))
        return base.load_setup_ids_for_years(
            conn,
            args.results_table,
            args.source_run_id,
            years,
            args.setup_limit,
            per_year,
            args.setup_sample_mod,
            args.setup_sample_slot,
            reference_template,
        )
    raise ValueError("--setup-limit requires --year or --start-year/--end-year")


def load_candidate_rows(conn, args: argparse.Namespace) -> pd.DataFrame:
    if args.result_id_file:
        result_ids = load_result_ids_from_file(args.result_id_file)
        return load_candidate_rows_for_result_ids(conn, args, result_ids)

    setup_ids = setup_ids_from_args(conn, args)
    if setup_ids:
        return load_candidate_rows_for_setups(conn, args, setup_ids)

    table = base.safe_identifier(args.results_table)
    where = [
        "r.run_id = %s",
        "r.entry_date IS NOT NULL",
        "r.entry_price IS NOT NULL",
        "r.stop_price IS NOT NULL",
        "r.target_price IS NOT NULL",
        "r.risk_points IS NOT NULL",
        "r.risk_points > 0",
    ]
    params: list[object] = [args.source_run_id]
    if args.year is not None:
        where.append("r.d_confirm_date >= %s AND r.d_confirm_date < %s")
        params.extend([f"{args.year}-01-01", f"{args.year + 1}-01-01"])
    elif args.start_year is not None and args.end_year is not None:
        where.append("r.d_confirm_date >= %s AND r.d_confirm_date < %s")
        params.extend([f"{args.start_year}-01-01", f"{args.end_year + 1}-01-01"])
    if args.only_missing:
        where.append("pl.result_id IS NULL")
    limit_sql = ""
    if args.limit and args.limit > 0:
        limit_sql = "LIMIT %s"
        params.append(args.limit)

    join_missing = ""
    if args.only_missing:
        join_missing = """
            LEFT JOIN entry_exit_candidate_path_labels pl
              ON pl.source_run_id = r.run_id
             AND pl.results_table = %s
             AND pl.result_id = r.id
        """
        params.insert(0, args.results_table)

    query = f"""
        {candidate_select_sql(table, join_missing)}
        WHERE {" AND ".join(where)}
        ORDER BY r.symbol ASC, r.entry_date ASC, r.id ASC
        {limit_sql}
    """
    with conn.cursor() as cur:
        cur.execute(query, params)
        rows = cur.fetchall()
    return normalize_candidate_frame(pd.DataFrame(rows))


def load_symbol_candles(conn, symbol: str, start_dt, end_dt) -> pd.DataFrame:
    query = """
        SELECT ts_utc, open, high, low, close
        FROM futures_contract_1m_candles FORCE INDEX (idx_futures_contract_1m_symbol_time)
        WHERE symbol = %s
          AND ts_utc >= %s
          AND ts_utc <= %s
        ORDER BY ts_utc ASC
    """
    with conn.cursor() as cur:
        cur.execute(query, (symbol, start_dt, end_dt))
        rows = cur.fetchall()
    if not rows:
        return pd.DataFrame(columns=["ts_utc", "open", "high", "low", "close"])
    candles = pd.DataFrame(rows)
    candles["ts_utc"] = pd.to_datetime(candles["ts_utc"], errors="coerce")
    for col in ["open", "high", "low", "close"]:
        candles[col] = pd.to_numeric(candles[col], errors="coerce")
    return candles.dropna(subset=["ts_utc", "open", "high", "low", "close"])


def clean(value: object) -> object:
    if value is None:
        return None
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, pd.Timestamp):
        return value.to_pydatetime()
    if pd.isna(value):
        return None
    return value


def horizon_minutes(row: pd.Series) -> int:
    full_pattern_length = row.get("full_pattern_length")
    max_hold_multiple = row.get("max_hold_multiple")
    if full_pattern_length is not None and not pd.isna(full_pattern_length) and float(full_pattern_length) > 0:
        multiple = 5.0
        if max_hold_multiple is not None and not pd.isna(max_hold_multiple):
            multiple = max(float(max_hold_multiple), 5.0)
        return max(1, int(math.ceil(float(full_pattern_length) * multiple)))

    entry = row.get("entry_date")
    exit_date = row.get("engine_exit_date")
    if isinstance(entry, pd.Timestamp) and isinstance(exit_date, pd.Timestamp):
        return max(1, int(math.ceil((exit_date - entry).total_seconds() / 60.0)) + 1)
    return 120


def compute_path_label(row: pd.Series, candles: pd.DataFrame) -> dict[str, object]:
    entry_date = pd.Timestamp(row["entry_date"])
    max_minutes = horizon_minutes(row)
    path_end = entry_date + timedelta(minutes=max_minutes)

    ts = candles["ts_utc"].to_numpy(dtype="datetime64[ns]")
    start_index = int(np.searchsorted(ts, np.datetime64(entry_date), side="left"))
    end_index = int(np.searchsorted(ts, np.datetime64(path_end), side="right"))
    segment = candles.iloc[start_index:end_index].copy()

    direction_text = str(row.get("trade_direction") or "").upper()
    direction = -1.0 if direction_text == "SHORT" else 1.0
    entry_price = float(row["entry_price"])
    stop_price = float(row["stop_price"])
    target_price = float(row["target_price"])
    risk_points = abs(float(row["risk_points"]))
    target_r = ((target_price - entry_price) * direction) / risk_points if risk_points > 0 else None

    base_row: dict[str, object] = {
        "source_run_id": row["run_id"],
        "results_table": row.get("results_table"),
        "result_id": int(row["result_id"]),
        "setup_id": row["setup_id"],
        "template_uid": row["template_uid"],
        "pattern_id": row.get("pattern_id"),
        "pattern_group_id": row.get("pattern_group_id"),
        "symbol": row["symbol"],
        "d_confirm_date": row.get("d_confirm_date"),
        "entry_date": entry_date,
        "engine_exit_date": row.get("engine_exit_date"),
        "path_end_date": path_end,
        "entry_price": entry_price,
        "stop_price": stop_price,
        "target_price": target_price,
        "risk_points": risk_points,
        "target_r": target_r,
        "trade_direction": direction_text or ("SHORT" if direction < 0 else "LONG"),
        "engine_outcome": row.get("engine_outcome"),
        "engine_exit_reason": row.get("engine_exit_reason"),
        "engine_result_r": row.get("engine_result_r"),
        "path_candle_count": int(len(segment)),
    }
    if segment.empty or risk_points <= 0:
        return {
            **base_row,
            "first_hit_outcome": "no_path",
        }

    candle_minutes = ((segment["ts_utc"] - entry_date).dt.total_seconds() / 60.0).round().astype(int)
    if direction > 0:
        favorable_r = (segment["high"] - entry_price) / risk_points
        adverse_r = (entry_price - segment["low"]) / risk_points
        close_r = (segment["close"] - entry_price) / risk_points
        stop_hit = segment["low"] <= stop_price
        target_hit = segment["high"] >= target_price
    else:
        favorable_r = (entry_price - segment["low"]) / risk_points
        adverse_r = (segment["high"] - entry_price) / risk_points
        close_r = (entry_price - segment["close"]) / risk_points
        stop_hit = segment["high"] >= stop_price
        target_hit = segment["low"] <= target_price

    stop_positions = np.flatnonzero(stop_hit.to_numpy())
    target_positions = np.flatnonzero(target_hit.to_numpy())
    first_stop_pos = int(stop_positions[0]) if len(stop_positions) else None
    first_target_pos = int(target_positions[0]) if len(target_positions) else None
    first_stop_date = segment["ts_utc"].iloc[first_stop_pos] if first_stop_pos is not None else None
    first_target_date = segment["ts_utc"].iloc[first_target_pos] if first_target_pos is not None else None
    first_stop_minutes = int(candle_minutes.iloc[first_stop_pos]) if first_stop_pos is not None else None
    first_target_minutes = int(candle_minutes.iloc[first_target_pos]) if first_target_pos is not None else None

    both_hit_same = bool(
        first_stop_pos is not None
        and first_target_pos is not None
        and first_stop_pos == first_target_pos
    )
    if first_stop_pos is None and first_target_pos is None:
        first_hit = "none"
    elif first_stop_pos is not None and (first_target_pos is None or first_stop_pos <= first_target_pos):
        first_hit = "stop"
    else:
        first_hit = "target"

    mfe = pd.to_numeric(favorable_r, errors="coerce").replace([np.inf, -np.inf], np.nan)
    mae = pd.to_numeric(adverse_r, errors="coerce").replace([np.inf, -np.inf], np.nan)
    close = pd.to_numeric(close_r, errors="coerce").replace([np.inf, -np.inf], np.nan)
    mfe_pos = int(mfe.idxmax()) if not mfe.empty and not mfe.isna().all() else None
    mae_pos = int(mae.idxmax()) if not mae.empty and not mae.isna().all() else None

    hit_minutes: dict[str, int | None] = {}
    for threshold in THRESHOLDS_R:
        hit = np.flatnonzero((mfe >= threshold).fillna(False).to_numpy())
        hit_minutes[f"hit_pos_{str(threshold).replace('.', '_')}r_minutes"] = int(candle_minutes.iloc[int(hit[0])]) if len(hit) else None
    for threshold in [0.5, 1.0]:
        hit = np.flatnonzero((mae >= threshold).fillna(False).to_numpy())
        hit_minutes[f"hit_neg_{str(threshold).replace('.', '_')}r_minutes"] = int(candle_minutes.iloc[int(hit[0])]) if len(hit) else None

    target_slice = mae.iloc[: first_target_pos + 1] if first_target_pos is not None else pd.Series(dtype=float)
    stop_slice = mfe.iloc[: first_stop_pos + 1] if first_stop_pos is not None else pd.Series(dtype=float)

    return {
        **base_row,
        "path_end_date": segment["ts_utc"].iloc[-1],
        "path_candle_count": int(len(segment)),
        "path_minutes": int(candle_minutes.iloc[-1]),
        "first_stop_date": first_stop_date,
        "first_target_date": first_target_date,
        "first_stop_minutes": first_stop_minutes,
        "first_target_minutes": first_target_minutes,
        "first_hit_outcome": first_hit,
        "both_hit_same_candle": 1 if both_hit_same else 0,
        "mfe_r": float(mfe.max()) if not mfe.empty else None,
        "mae_r": float(mae.max()) if not mae.empty else None,
        "best_close_r": float(close.max()) if not close.empty else None,
        "worst_close_r": float(close.min()) if not close.empty else None,
        "end_close_r": float(close.iloc[-1]) if not close.empty else None,
        "minutes_to_mfe": int(candle_minutes.loc[mfe_pos]) if mfe_pos is not None else None,
        "minutes_to_mae": int(candle_minutes.loc[mae_pos]) if mae_pos is not None else None,
        "mae_before_target_r": float(target_slice.max()) if not target_slice.empty else None,
        "mfe_before_stop_r": float(stop_slice.max()) if not stop_slice.empty else None,
        **hit_minutes,
    }


def flush_rows(conn, rows: list[dict[str, object]]) -> None:
    if not rows:
        return
    columns = [
        "source_run_id",
        "results_table",
        "result_id",
        "setup_id",
        "template_uid",
        "pattern_id",
        "pattern_group_id",
        "symbol",
        "d_confirm_date",
        "entry_date",
        "engine_exit_date",
        "path_end_date",
        "entry_price",
        "stop_price",
        "target_price",
        "risk_points",
        "target_r",
        "trade_direction",
        "engine_outcome",
        "engine_exit_reason",
        "engine_result_r",
        "path_candle_count",
        "path_minutes",
        "first_stop_date",
        "first_target_date",
        "first_stop_minutes",
        "first_target_minutes",
        "first_hit_outcome",
        "both_hit_same_candle",
        "mfe_r",
        "mae_r",
        "best_close_r",
        "worst_close_r",
        "end_close_r",
        "minutes_to_mfe",
        "minutes_to_mae",
        "mae_before_target_r",
        "mfe_before_stop_r",
        "hit_pos_0_5r_minutes",
        "hit_pos_1_0r_minutes",
        "hit_pos_2_0r_minutes",
        "hit_pos_3_0r_minutes",
        "hit_pos_4_0r_minutes",
        "hit_neg_0_5r_minutes",
        "hit_neg_1_0r_minutes",
    ]
    placeholders = ", ".join(["%s"] * len(columns))
    updates = ", ".join(
        [f"{column}=VALUES({column})" for column in columns if column not in {"source_run_id", "results_table", "result_id"}]
        + ["updated_at=CURRENT_TIMESTAMP"]
    )
    query = f"""
        INSERT INTO entry_exit_candidate_path_labels ({", ".join(columns)})
        VALUES ({placeholders})
        ON DUPLICATE KEY UPDATE {updates}
    """
    values = [tuple(clean(row.get(column)) for column in columns) for row in rows]
    with conn.cursor() as cur:
        cur.executemany(query, values)
    conn.commit()


def main() -> int:
    args = parse_args()
    args.results_table = base.safe_identifier(args.results_table)
    conn = base.connect()
    try:
        ensure_table(conn)
        candidates = load_candidate_rows(conn, args)
        if candidates.empty:
            print("No candidate rows found.")
            return 0
        candidates["results_table"] = args.results_table
        print(f"Path-label candidates: {len(candidates):,}")
        print(f"Symbols: {candidates['symbol'].nunique():,}")

        pending: list[dict[str, object]] = []
        processed = 0
        for symbol, symbol_rows in candidates.groupby("symbol", sort=True):
            horizons = symbol_rows.apply(horizon_minutes, axis=1)
            start_dt = symbol_rows["entry_date"].min()
            end_dt = (symbol_rows["entry_date"] + pd.to_timedelta(horizons, unit="m")).max()
            candles = load_symbol_candles(conn, symbol, start_dt.to_pydatetime(), end_dt.to_pydatetime())
            if candles.empty:
                print(f"{symbol}: no candles for {len(symbol_rows)} candidates")
                continue

            for _, row in symbol_rows.iterrows():
                pending.append(compute_path_label(row, candles))
                processed += 1
                if len(pending) >= args.chunk_size:
                    flush_rows(conn, pending)
                    pending.clear()
            print(f"{symbol}: processed {len(symbol_rows):,} candidates, total {processed:,}/{len(candidates):,}")

        flush_rows(conn, pending)
        print(f"Stored path labels for {processed:,} candidates.")
        return 0
    finally:
        conn.close()


if __name__ == "__main__":
    sys.exit(main())
