#!/usr/bin/env python3
"""
Train a first-pass CatBoost model over entry/exit template candidate rows.

The model learns from completed template-result rows, then scores a validation
year and stores what would happen if we pick the highest-scored candidate for
each pattern setup.
"""

from __future__ import annotations

import argparse
import math
import os
import re
import sys
import time
from pathlib import Path
from typing import Iterable
from urllib.parse import urlparse

import numpy as np
import pandas as pd
import pymysql
from catboost import CatBoostClassifier, CatBoostRegressor, Pool
from sklearn.metrics import log_loss, mean_absolute_error, mean_squared_error, roc_auc_score


ROOT = Path(__file__).resolve().parents[3]
ABCD_ROOT = ROOT / "rust_sr" / "abcd"
DEFAULT_SOURCE_RUN_ID = "eetc-1779471316242-118840"
DEFAULT_RESULTS_TABLE = "entry_exit_template_results_eetc_1779471316242_118840"

CAT_FEATURES = [
    "template_uid",
    "pattern_family_key",
    "symbol",
    "root_symbol",
    "market",
    "trade_direction",
    "entry_kind",
    "direction_mode",
    "risk_basis",
    "harmonic_type",
    "family_bin",
    "reversal_type",
    "size_bucket",
    "time_bin",
    "x_strictness",
    "three_month_trend",
    "six_month_trend",
    "twelve_month_trend",
]

NUM_FEATURES = [
    "event_rank",
    "event_sister_count",
    "event_candidate_count",
    "event_live_candidate_count",
    "risk_multiple",
    "target_r",
    "max_hold_multiple",
    "risk_points",
    "confirm_hour",
    "confirm_day_of_week",
    "confirm_month",
    "confirm_day_of_month",
    "setup_count",
    "symbol_count",
    "pre_total_candles",
    "pre_xa_points",
    "pre_ab_points",
    "pre_bc_points",
    "pre_cd_points",
    "pre_ab_to_xa",
    "pre_bc_to_ab",
    "pre_cd_to_xa",
    "pre_cd_to_bc",
    "pre_pattern_height_points",
    "pre_pattern_height_to_price",
    "pre_xa_minutes",
    "pre_ab_minutes",
    "pre_bc_minutes",
    "pre_cd_minutes",
    "pre_full_pattern_minutes",
    "pre_confirm_lag_minutes",
    "pre_xa_velocity",
    "pre_cd_velocity",
    "pre_cd_to_xa_velocity",
    "pre_d_range_to_xa",
    "pre_d_body_to_xa",
    "pre_d_close_location",
    "pre_d_upper_wick_ratio",
    "pre_d_lower_wick_ratio",
    "pre_reversal_signal_count",
    "pre_candidate_target_distance_r",
    "pre_candidate_stop_distance_r",
    "pre_candidate_target_clearance_60m_r",
    "pre_candidate_stop_buffer_60m_r",
    "pre_candidate_entry_to_target_side_60m_r",
    "pre_candidate_entry_to_stop_side_60m_r",
    "pre_candidate_target_clearance_120m_r",
    "pre_candidate_stop_buffer_120m_r",
    "pre_candidate_entry_to_target_side_120m_r",
    "pre_candidate_entry_to_stop_side_120m_r",
    "pre_return_5m",
    "pre_range_5m",
    "pre_body_ratio_5m",
    "pre_directional_bias_5m",
    "pre_green_share_5m",
    "pre_red_share_5m",
    "pre_avg_range_5m",
    "pre_volatility_5m",
    "pre_candle_count_5m",
    "pre_return_15m",
    "pre_range_15m",
    "pre_body_ratio_15m",
    "pre_directional_bias_15m",
    "pre_green_share_15m",
    "pre_red_share_15m",
    "pre_avg_range_15m",
    "pre_volatility_15m",
    "pre_candle_count_15m",
    "pre_return_30m",
    "pre_range_30m",
    "pre_body_ratio_30m",
    "pre_directional_bias_30m",
    "pre_green_share_30m",
    "pre_red_share_30m",
    "pre_avg_range_30m",
    "pre_volatility_30m",
    "pre_candle_count_30m",
    "pre_return_60m",
    "pre_range_60m",
    "pre_body_ratio_60m",
    "pre_directional_bias_60m",
    "pre_green_share_60m",
    "pre_red_share_60m",
    "pre_avg_range_60m",
    "pre_volatility_60m",
    "pre_candle_count_60m",
    "pre_return_120m",
    "pre_range_120m",
    "pre_body_ratio_120m",
    "pre_directional_bias_120m",
    "pre_green_share_120m",
    "pre_red_share_120m",
    "pre_avg_range_120m",
    "pre_volatility_120m",
    "pre_candle_count_120m",
    "pre_distance_to_60m_high",
    "pre_distance_to_60m_low",
    "pre_distance_to_120m_high",
    "pre_distance_to_120m_low",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-run-id", default=DEFAULT_SOURCE_RUN_ID)
    parser.add_argument("--results-table", default=DEFAULT_RESULTS_TABLE)
    parser.add_argument("--train-year", type=int, default=2024)
    parser.add_argument("--train-start-year", type=int, default=None)
    parser.add_argument("--train-end-year", type=int, default=None)
    parser.add_argument("--valid-year", type=int, default=2025)
    parser.add_argument("--train-setups", type=int, default=3500)
    parser.add_argument("--train-setups-per-year", type=int, default=None)
    parser.add_argument("--valid-setups", type=int, default=2500)
    parser.add_argument("--setup-sample-mod", type=int, default=7)
    parser.add_argument("--train-sample-slot", type=int, default=0)
    parser.add_argument("--valid-sample-slot", type=int, default=3)
    parser.add_argument("--thresholds", default="0.50,0.55,0.60,0.65,0.70,0.75")
    parser.add_argument("--model-objective", choices=["win", "expected_r"], default="win")
    parser.add_argument("--iterations", type=int, default=650)
    parser.add_argument("--depth", type=int, default=6)
    parser.add_argument("--learning-rate", type=float, default=0.045)
    parser.add_argument("--min-selected-trades", type=int, default=100)
    parser.add_argument("--random-seed", type=int, default=42)
    parser.add_argument("--no-use-best-model", action="store_true")
    parser.add_argument("--run-id", default=None)
    return parser.parse_args()


def read_database_url() -> str:
    if os.environ.get("DATABASE_URL"):
        return os.environ["DATABASE_URL"]

    env_path = ABCD_ROOT / ".env"
    if not env_path.exists():
        raise FileNotFoundError(f"Could not find DATABASE_URL or {env_path}")

    for line in env_path.read_text().splitlines():
        match = re.match(r"\s*DATABASE_URL\s*=\s*(.+?)\s*$", line)
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


def safe_identifier(value: str) -> str:
    if not re.fullmatch(r"[A-Za-z0-9_]+", value):
        raise ValueError(f"Unsafe SQL identifier: {value!r}")
    return value


def chunked(items: list[str], size: int) -> Iterable[list[str]]:
    for index in range(0, len(items), size):
        yield items[index : index + size]


def make_run_id() -> str:
    return f"aibs-{int(time.time() * 1000)}-{os.getpid()}"


def ensure_tables(conn: pymysql.connections.Connection) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_build_search_runs (
                ai_build_run_id VARCHAR(64) PRIMARY KEY,
                source_run_id VARCHAR(64) NOT NULL,
                results_table VARCHAR(128) NOT NULL,
                model_type VARCHAR(64) NOT NULL,
                model_objective VARCHAR(32) NULL,
                train_year INT NOT NULL,
                train_start_year INT NULL,
                train_end_year INT NULL,
                valid_year INT NOT NULL,
                train_setups BIGINT NOT NULL,
                valid_setups BIGINT NOT NULL,
                train_rows BIGINT NOT NULL,
                valid_rows BIGINT NOT NULL,
                baseline_win_rate DOUBLE NULL,
                auc DOUBLE NULL,
                logloss DOUBLE NULL,
                rmse DOUBLE NULL,
                mae DOUBLE NULL,
                best_threshold DOUBLE NULL,
                selected_trades BIGINT NULL,
                selected_wins BIGINT NULL,
                selected_losses BIGINT NULL,
                selected_win_rate DOUBLE NULL,
                selected_avg_r DOUBLE NULL,
                selected_sum_r DOUBLE NULL,
                model_path VARCHAR(255) NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            """
        )
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_build_search_thresholds (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                ai_build_run_id VARCHAR(64) NOT NULL,
                selection_mode VARCHAR(32) NOT NULL,
                threshold DOUBLE NOT NULL,
                rows_seen BIGINT NOT NULL,
                wins BIGINT NOT NULL,
                losses BIGINT NOT NULL,
                win_rate DOUBLE NULL,
                avg_r DOUBLE NULL,
                sum_r DOUBLE NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_aibs_threshold_run (ai_build_run_id, selection_mode, threshold)
            )
            """
        )
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_build_search_template_recommendations (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                ai_build_run_id VARCHAR(64) NOT NULL,
                threshold DOUBLE NOT NULL,
                template_uid VARCHAR(128) NOT NULL,
                template_name VARCHAR(255) NULL,
                selected_rows BIGINT NOT NULL,
                wins BIGINT NOT NULL,
                losses BIGINT NOT NULL,
                win_rate DOUBLE NULL,
                avg_r DOUBLE NULL,
                sum_r DOUBLE NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_aibs_template_run (ai_build_run_id, threshold, template_uid)
            )
            """
        )
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_build_search_feature_importance (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                ai_build_run_id VARCHAR(64) NOT NULL,
                feature_name VARCHAR(128) NOT NULL,
                importance DOUBLE NOT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_aibs_feature_run (ai_build_run_id, importance)
            )
            """
        )
        cur.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_build_search_selected_trades (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                ai_build_run_id VARCHAR(64) NOT NULL,
                threshold DOUBLE NOT NULL,
                setup_id VARCHAR(64) NOT NULL,
                pattern_id VARCHAR(64) NULL,
                pattern_group_id VARCHAR(128) NULL,
                template_uid VARCHAR(128) NOT NULL,
                pattern_family_key VARCHAR(64) NULL,
                symbol VARCHAR(32) NULL,
                market VARCHAR(16) NULL,
                trade_direction VARCHAR(8) NULL,
                d_confirm_date DATETIME NULL,
                entry_date DATETIME NULL,
                exit_date DATETIME NULL,
                entry_price DOUBLE NULL,
                stop_price DOUBLE NULL,
                target_price DOUBLE NULL,
                exit_price DOUBLE NULL,
                risk_points DOUBLE NULL,
                predicted_win_probability DOUBLE NOT NULL,
                predicted_expected_r DOUBLE NULL,
                outcome VARCHAR(16) NULL,
                result_r DOUBLE NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_aibs_selected_run (ai_build_run_id, threshold),
                INDEX idx_aibs_selected_setup (setup_id)
            )
            """
        )
    ensure_column(conn, "ai_build_search_runs", "train_start_year", "INT NULL")
    ensure_column(conn, "ai_build_search_runs", "train_end_year", "INT NULL")
    ensure_column(conn, "ai_build_search_runs", "model_objective", "VARCHAR(32) NULL")
    ensure_column(conn, "ai_build_search_runs", "rmse", "DOUBLE NULL")
    ensure_column(conn, "ai_build_search_runs", "mae", "DOUBLE NULL")
    ensure_column(conn, "ai_build_search_selected_trades", "predicted_expected_r", "DOUBLE NULL")
    conn.commit()


def ensure_column(
    conn: pymysql.connections.Connection,
    table_name: str,
    column_name: str,
    definition: str,
) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT COUNT(*) AS column_count
            FROM information_schema.COLUMNS
            WHERE TABLE_SCHEMA = DATABASE()
              AND TABLE_NAME = %s
              AND COLUMN_NAME = %s
            """,
            (table_name, column_name),
        )
        row = cur.fetchone()
        if row and int(row["column_count"]) > 0:
            return
        cur.execute(f"ALTER TABLE {safe_identifier(table_name)} ADD COLUMN {safe_identifier(column_name)} {definition}")


def first_template_uid(conn: pymysql.connections.Connection, source_run_id: str) -> str:
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT template_uid
            FROM entry_exit_templates
            WHERE origin_run_id = %s
            ORDER BY template_uid
            LIMIT 1
            """,
            (source_run_id,),
        )
        row = cur.fetchone()
    if not row:
        raise RuntimeError(f"No templates found for {source_run_id}")
    return row["template_uid"]


def load_setup_ids(
    conn: pymysql.connections.Connection,
    table: str,
    source_run_id: str,
    year: int,
    limit: int,
    sample_mod: int,
    sample_slot: int,
    template_uid: str,
) -> list[str]:
    start = f"{year}-01-01"
    end = f"{year + 1}-01-01"
    query = f"""
        SELECT r.setup_id
        FROM {safe_identifier(table)} r
        WHERE r.run_id = %s
          AND r.template_uid = %s
          AND r.d_confirm_date >= %s
          AND r.d_confirm_date < %s
          AND MOD(r.id, %s) = %s
        ORDER BY r.id
        LIMIT %s
    """
    with conn.cursor() as cur:
        cur.execute(query, (source_run_id, template_uid, start, end, sample_mod, sample_slot, limit))
        return [row["setup_id"] for row in cur.fetchall()]


def load_setup_ids_for_years(
    conn: pymysql.connections.Connection,
    table: str,
    source_run_id: str,
    years: list[int],
    total_limit: int,
    per_year_limit: int | None,
    sample_mod: int,
    sample_slot: int,
    template_uid: str,
) -> list[str]:
    if not years:
        return []
    if len(years) == 1:
        return load_setup_ids(
            conn,
            table,
            source_run_id,
            years[0],
            total_limit,
            sample_mod,
            sample_slot,
            template_uid,
        )

    ids: list[str] = []
    for index, year in enumerate(years):
        limit = per_year_limit or max(1, total_limit // len(years))
        ids.extend(
            load_setup_ids(
                conn,
                table,
                source_run_id,
                year,
                limit,
                sample_mod,
                (sample_slot + index) % sample_mod,
                template_uid,
            )
        )
    return ids


def load_candidate_rows(
    conn: pymysql.connections.Connection,
    table: str,
    source_run_id: str,
    setup_ids: list[str],
) -> pd.DataFrame:
    if not setup_ids:
        return pd.DataFrame()

    frames: list[pd.DataFrame] = []
    base_cols = """
        r.id,
        r.run_id,
        r.template_uid,
        r.setup_id,
        r.pattern_id,
        r.pattern_group_id,
        r.event_rank,
        r.event_sister_count,
        r.event_candidate_count,
        r.event_live_candidate_count,
        r.pattern_family_key,
        r.symbol,
        r.market,
        r.d_confirm_date,
        r.outcome,
        r.result_r,
        r.entry_date,
        r.exit_date,
        r.entry_price,
        r.stop_price,
        r.target_price,
        r.exit_price,
        r.risk_points,
        r.trade_direction,
        t.template_name,
        t.entry_kind,
        t.direction_mode,
        t.risk_basis,
        t.risk_multiple,
        t.target_r,
        t.max_hold_multiple,
        f.harmonic_type,
        f.bin AS family_bin,
        f.reversal_type,
        f.size_bucket,
        f.time_bin,
        f.x_strictness,
        f.three_month_trend,
        f.six_month_trend,
        f.twelve_month_trend,
        f.setup_count,
        f.symbol_count,
        pc.pre_total_candles,
        pc.pre_xa_points,
        pc.pre_ab_points,
        pc.pre_bc_points,
        pc.pre_cd_points,
        pc.pre_ab_to_xa,
        pc.pre_bc_to_ab,
        pc.pre_cd_to_xa,
        pc.pre_cd_to_bc,
        pc.pre_pattern_height_points,
        pc.pre_pattern_height_to_price,
        pc.pre_xa_minutes,
        pc.pre_ab_minutes,
        pc.pre_bc_minutes,
        pc.pre_cd_minutes,
        pc.pre_full_pattern_minutes,
        pc.pre_confirm_lag_minutes,
        pc.pre_xa_velocity,
        pc.pre_cd_velocity,
        pc.pre_cd_to_xa_velocity,
        pc.pre_d_range_to_xa,
        pc.pre_d_body_to_xa,
        pc.pre_d_close_location,
        pc.pre_d_upper_wick_ratio,
        pc.pre_d_lower_wick_ratio,
        pc.pre_reversal_signal_count,
        pc.pre_return_5m,
        pc.pre_range_5m,
        pc.pre_body_ratio_5m,
        pc.pre_directional_bias_5m,
        pc.pre_green_share_5m,
        pc.pre_red_share_5m,
        pc.pre_avg_range_5m,
        pc.pre_volatility_5m,
        pc.pre_candle_count_5m,
        pc.pre_return_15m,
        pc.pre_range_15m,
        pc.pre_body_ratio_15m,
        pc.pre_directional_bias_15m,
        pc.pre_green_share_15m,
        pc.pre_red_share_15m,
        pc.pre_avg_range_15m,
        pc.pre_volatility_15m,
        pc.pre_candle_count_15m,
        pc.pre_return_30m,
        pc.pre_range_30m,
        pc.pre_body_ratio_30m,
        pc.pre_directional_bias_30m,
        pc.pre_green_share_30m,
        pc.pre_red_share_30m,
        pc.pre_avg_range_30m,
        pc.pre_volatility_30m,
        pc.pre_candle_count_30m,
        pc.pre_return_60m,
        pc.pre_range_60m,
        pc.pre_body_ratio_60m,
        pc.pre_directional_bias_60m,
        pc.pre_green_share_60m,
        pc.pre_red_share_60m,
        pc.pre_avg_range_60m,
        pc.pre_volatility_60m,
        pc.pre_candle_count_60m,
        pc.pre_return_120m,
        pc.pre_range_120m,
        pc.pre_body_ratio_120m,
        pc.pre_directional_bias_120m,
        pc.pre_green_share_120m,
        pc.pre_red_share_120m,
        pc.pre_avg_range_120m,
        pc.pre_volatility_120m,
        pc.pre_candle_count_120m,
        pc.pre_distance_to_60m_high,
        pc.pre_distance_to_60m_low,
        pc.pre_distance_to_120m_high,
        pc.pre_distance_to_120m_low,
        pl.first_hit_outcome AS path_first_hit_outcome,
        pl.both_hit_same_candle AS path_both_hit_same_candle,
        pl.mfe_r AS path_mfe_r,
        pl.mae_r AS path_mae_r,
        pl.best_close_r AS path_best_close_r,
        pl.worst_close_r AS path_worst_close_r,
        pl.end_close_r AS path_end_close_r,
        pl.target_r AS path_target_r,
        pl.first_target_minutes AS path_first_target_minutes,
        pl.first_stop_minutes AS path_first_stop_minutes,
        pl.minutes_to_mfe AS path_minutes_to_mfe,
        pl.minutes_to_mae AS path_minutes_to_mae,
        pl.mae_before_target_r AS path_mae_before_target_r,
        pl.mfe_before_stop_r AS path_mfe_before_stop_r
    """
    table_name = safe_identifier(table)

    for ids in chunked(setup_ids, 40):
        placeholders = ",".join(["%s"] * len(ids))
        query = f"""
            SELECT {base_cols}
            FROM {table_name} r FORCE INDEX (idx_entry_exit_template_results_run_setup)
            JOIN entry_exit_templates t
              ON t.template_uid = r.template_uid
            LEFT JOIN pattern_family_summary f
              ON f.family_key = r.pattern_family_key
            LEFT JOIN pattern_pre_context_features pc
              ON pc.setup_id = r.setup_id
            LEFT JOIN entry_exit_candidate_path_labels pl
              ON pl.source_run_id = r.run_id
             AND pl.results_table = %s
             AND pl.result_id = r.id
            WHERE r.run_id = %s
              AND r.setup_id IN ({placeholders})
        """
        params = [table_name, source_run_id, *ids]
        with conn.cursor() as cur:
            cur.execute(query, params)
            rows = cur.fetchall()
        if rows:
            frames.append(pd.DataFrame(rows))

    if not frames:
        return pd.DataFrame()
    return pd.concat(frames, ignore_index=True)


def root_symbol(symbol: object) -> str:
    if symbol is None or (isinstance(symbol, float) and math.isnan(symbol)):
        return ""
    text = str(symbol).upper()
    return re.sub(r"[FGHJKMNQUVXZ]\d+$", "", text)


def add_candidate_structure_features(work: pd.DataFrame) -> None:
    entry = pd.to_numeric(work.get("entry_price"), errors="coerce")
    stop = pd.to_numeric(work.get("stop_price"), errors="coerce")
    target = pd.to_numeric(work.get("target_price"), errors="coerce")
    risk = pd.to_numeric(work.get("risk_points"), errors="coerce").replace(0.0, np.nan)
    direction = work.get("trade_direction", pd.Series([""] * len(work), index=work.index)).fillna("").astype(str).str.upper()
    is_long = direction == "LONG"

    target_distance_r = ((target - entry).abs() / risk).replace([np.inf, -np.inf], np.nan)
    stop_distance_r = ((entry - stop).abs() / risk).replace([np.inf, -np.inf], np.nan)
    work["pre_candidate_target_distance_r"] = target_distance_r
    work["pre_candidate_stop_distance_r"] = stop_distance_r

    for window in [60, 120]:
        high_dist = pd.to_numeric(work.get(f"pre_distance_to_{window}m_high"), errors="coerce")
        low_dist = pd.to_numeric(work.get(f"pre_distance_to_{window}m_low"), errors="coerce")
        entry_to_high_r = ((high_dist * entry) / risk).replace([np.inf, -np.inf], np.nan)
        entry_to_low_r = ((low_dist * entry) / risk).replace([np.inf, -np.inf], np.nan)
        target_side = np.where(is_long, entry_to_high_r, entry_to_low_r)
        stop_side = np.where(is_long, entry_to_low_r, entry_to_high_r)
        work[f"pre_candidate_entry_to_target_side_{window}m_r"] = target_side
        work[f"pre_candidate_entry_to_stop_side_{window}m_r"] = stop_side
        work[f"pre_candidate_target_clearance_{window}m_r"] = target_distance_r - target_side
        work[f"pre_candidate_stop_buffer_{window}m_r"] = stop_side - stop_distance_r


def prepare_features(df: pd.DataFrame) -> tuple[pd.DataFrame, pd.Series, pd.Series]:
    work = df.copy()
    work["d_confirm_date"] = pd.to_datetime(work["d_confirm_date"], errors="coerce")
    work["root_symbol"] = work["symbol"].map(root_symbol)
    work["confirm_hour"] = work["d_confirm_date"].dt.hour.fillna(0).astype(int)
    work["confirm_day_of_week"] = work["d_confirm_date"].dt.dayofweek.fillna(0).astype(int)
    work["confirm_month"] = work["d_confirm_date"].dt.month.fillna(0).astype(int)
    work["confirm_day_of_month"] = work["d_confirm_date"].dt.day.fillna(0).astype(int)
    add_candidate_structure_features(work)

    for col in CAT_FEATURES:
        if col not in work.columns:
            work[col] = ""
        work[col] = work[col].fillna("unknown").astype(str)

    for col in NUM_FEATURES:
        if col not in work.columns:
            work[col] = 0.0
        work[col] = pd.to_numeric(work[col], errors="coerce").fillna(0.0)

    y = (pd.to_numeric(work["result_r"], errors="coerce").fillna(0.0) > 0.0).astype(int)
    result_r = pd.to_numeric(work["result_r"], errors="coerce").fillna(0.0)
    features = work[CAT_FEATURES + NUM_FEATURES]
    return features, y, result_r


def summarize_rows(rows: pd.DataFrame) -> dict[str, float]:
    count = int(len(rows))
    if count == 0:
        return {
            "rows_seen": 0,
            "wins": 0,
            "losses": 0,
            "win_rate": None,
            "avg_r": None,
            "sum_r": 0.0,
        }

    result = pd.to_numeric(rows["result_r"], errors="coerce").fillna(0.0)
    wins = int((result > 0.0).sum())
    losses = int((result <= 0.0).sum())
    return {
        "rows_seen": count,
        "wins": wins,
        "losses": losses,
        "win_rate": wins / count if count else None,
        "avg_r": float(result.mean()) if count else None,
        "sum_r": float(result.sum()),
    }


def threshold_summaries(
    scored: pd.DataFrame,
    thresholds: list[float],
    score_column: str,
) -> list[dict[str, object]]:
    summaries: list[dict[str, object]] = []
    for mode, frame in [
        ("candidate_rows", scored),
        ("top_candidate_per_setup", best_candidate_per_setup(scored, score_column)),
    ]:
        for threshold in thresholds:
            selected = frame[frame[score_column] >= threshold]
            summary = summarize_rows(selected)
            summaries.append({"selection_mode": mode, "threshold": threshold, **summary})
    return summaries


def best_candidate_per_setup(scored: pd.DataFrame, score_column: str) -> pd.DataFrame:
    ordered = scored.sort_values(
        ["setup_id", score_column, "template_uid"],
        ascending=[True, False, True],
    )
    return ordered.drop_duplicates("setup_id", keep="first").copy()


def choose_best_threshold(
    summaries: list[dict[str, object]],
    min_selected_trades: int,
) -> dict[str, object]:
    top_rows = [
        s
        for s in summaries
        if s["selection_mode"] == "top_candidate_per_setup"
        and int(s["rows_seen"]) >= min_selected_trades
    ]
    if not top_rows:
        top_rows = [s for s in summaries if s["selection_mode"] == "top_candidate_per_setup"]
    if not top_rows:
        raise RuntimeError("No threshold summaries were produced")
    return max(top_rows, key=lambda s: (float(s["sum_r"] or 0.0), float(s["avg_r"] or -999.0)))


def store_results(
    conn: pymysql.connections.Connection,
    args: argparse.Namespace,
    ai_build_run_id: str,
    train_setup_count: int,
    valid_setup_count: int,
    train_rows: int,
    valid_rows: int,
    baseline_win_rate: float,
    auc: float | None,
    model_logloss: float | None,
    rmse: float | None,
    mae: float | None,
    model_path: str,
    train_start_year: int,
    train_end_year: int,
    summaries: list[dict[str, object]],
    best_summary: dict[str, object],
    best_selected: pd.DataFrame,
    feature_importance: list[tuple[str, float]],
) -> None:
    best_threshold = float(best_summary["threshold"])
    with conn.cursor() as cur:
        cur.execute(
            """
            INSERT INTO ai_build_search_runs (
                ai_build_run_id, source_run_id, results_table, model_type, model_objective,
                train_year, train_start_year, train_end_year, valid_year, train_setups, valid_setups,
                train_rows, valid_rows, baseline_win_rate, auc, logloss, rmse, mae,
                best_threshold, selected_trades, selected_wins, selected_losses,
                selected_win_rate, selected_avg_r, selected_sum_r, model_path
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            """,
            (
                ai_build_run_id,
                args.source_run_id,
                args.results_table,
                "CatBoostRegressor" if args.model_objective == "expected_r" else "CatBoostClassifier",
                args.model_objective,
                train_start_year,
                train_start_year,
                train_end_year,
                args.valid_year,
                train_setup_count,
                valid_setup_count,
                train_rows,
                valid_rows,
                baseline_win_rate,
                auc,
                model_logloss,
                rmse,
                mae,
                best_threshold,
                best_summary["rows_seen"],
                best_summary["wins"],
                best_summary["losses"],
                best_summary["win_rate"],
                best_summary["avg_r"],
                best_summary["sum_r"],
                model_path,
            ),
        )

        threshold_rows = [
            (
                ai_build_run_id,
                s["selection_mode"],
                s["threshold"],
                s["rows_seen"],
                s["wins"],
                s["losses"],
                s["win_rate"],
                s["avg_r"],
                s["sum_r"],
            )
            for s in summaries
        ]
        cur.executemany(
            """
            INSERT INTO ai_build_search_thresholds (
                ai_build_run_id, selection_mode, threshold, rows_seen,
                wins, losses, win_rate, avg_r, sum_r
            )
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)
            """,
            threshold_rows,
        )

        template_groups = (
            best_selected.groupby(["template_uid", "template_name"], dropna=False)
            .apply(lambda g: pd.Series(summarize_rows(g)))
            .reset_index()
            .sort_values(["sum_r", "rows_seen"], ascending=[False, False])
        )
        template_rows = [
            (
                ai_build_run_id,
                best_threshold,
                row.template_uid,
                None if pd.isna(row.template_name) else row.template_name,
                int(row.rows_seen),
                int(row.wins),
                int(row.losses),
                None if pd.isna(row.win_rate) else float(row.win_rate),
                None if pd.isna(row.avg_r) else float(row.avg_r),
                float(row.sum_r),
            )
            for row in template_groups.itertuples(index=False)
        ]
        if template_rows:
            cur.executemany(
                """
                INSERT INTO ai_build_search_template_recommendations (
                    ai_build_run_id, threshold, template_uid, template_name,
                    selected_rows, wins, losses, win_rate, avg_r, sum_r
                )
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                """,
                template_rows,
            )

        feature_rows = [(ai_build_run_id, name, importance) for name, importance in feature_importance]
        if feature_rows:
            cur.executemany(
                """
                INSERT INTO ai_build_search_feature_importance (
                    ai_build_run_id, feature_name, importance
                )
                VALUES (%s, %s, %s)
                """,
                feature_rows,
            )

        selected_cols = [
            "setup_id",
            "pattern_id",
            "pattern_group_id",
            "template_uid",
            "pattern_family_key",
            "symbol",
            "market",
            "trade_direction",
            "d_confirm_date",
            "entry_date",
            "exit_date",
            "entry_price",
            "stop_price",
            "target_price",
            "exit_price",
            "risk_points",
            "predicted_win_probability",
            "predicted_expected_r",
            "outcome",
            "result_r",
        ]
        selected_rows = []
        for row in best_selected[selected_cols].itertuples(index=False):
            selected_rows.append((ai_build_run_id, best_threshold, *[clean_db_value(v) for v in row]))
        if selected_rows:
            cur.executemany(
                """
                INSERT INTO ai_build_search_selected_trades (
                    ai_build_run_id, threshold, setup_id, pattern_id, pattern_group_id,
                    template_uid, pattern_family_key, symbol, market, trade_direction,
                    d_confirm_date, entry_date, exit_date, entry_price, stop_price,
                    target_price, exit_price, risk_points, predicted_win_probability, predicted_expected_r,
                    outcome, result_r
                )
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                """,
                selected_rows,
            )
    conn.commit()


def clean_db_value(value: object) -> object:
    if pd.isna(value):
        return None
    if isinstance(value, pd.Timestamp):
        return value.to_pydatetime()
    if isinstance(value, np.generic):
        return value.item()
    return value


def main() -> int:
    args = parse_args()
    args.results_table = safe_identifier(args.results_table)
    thresholds = [float(part.strip()) for part in args.thresholds.split(",") if part.strip()]
    ai_build_run_id = args.run_id or make_run_id()
    train_start_year = args.train_start_year or args.train_year
    train_end_year = args.train_end_year or args.train_year
    if train_end_year < train_start_year:
        raise ValueError("--train-end-year must be greater than or equal to --train-start-year")
    train_years = list(range(train_start_year, train_end_year + 1))

    conn = connect()
    try:
        ensure_tables(conn)
        template_uid = first_template_uid(conn, args.source_run_id)

        print(f"AI build search: {ai_build_run_id}")
        print(f"Source run: {args.source_run_id}")
        print(f"Results table: {args.results_table}")
        print(f"Reference setup template: {template_uid}")

        train_setup_ids = load_setup_ids_for_years(
            conn,
            args.results_table,
            args.source_run_id,
            train_years,
            args.train_setups,
            args.train_setups_per_year,
            args.setup_sample_mod,
            args.train_sample_slot,
            template_uid,
        )
        valid_setup_ids = load_setup_ids(
            conn,
            args.results_table,
            args.source_run_id,
            args.valid_year,
            args.valid_setups,
            args.setup_sample_mod,
            args.valid_sample_slot,
            template_uid,
        )
        print(f"Train setups: {len(train_setup_ids)}")
        print(f"Train years: {train_start_year}-{train_end_year}")
        print(f"Valid setups: {len(valid_setup_ids)}")

        train_df = load_candidate_rows(conn, args.results_table, args.source_run_id, train_setup_ids)
        valid_df = load_candidate_rows(conn, args.results_table, args.source_run_id, valid_setup_ids)
        if train_df.empty or valid_df.empty:
            raise RuntimeError("Training or validation candidate rows were empty.")

        print(f"Train rows: {len(train_df):,}")
        print(f"Valid rows: {len(valid_df):,}")

        x_train, y_train, train_result_r = prepare_features(train_df)
        x_valid, y_valid, valid_result_r = prepare_features(valid_df)

        cat_indexes = [x_train.columns.get_loc(col) for col in CAT_FEATURES]
        if args.model_objective == "expected_r":
            train_pool = Pool(x_train, label=train_result_r, cat_features=cat_indexes)
            valid_pool = Pool(x_valid, label=valid_result_r, cat_features=cat_indexes)
            model = CatBoostRegressor(
                iterations=args.iterations,
                depth=args.depth,
                learning_rate=args.learning_rate,
                loss_function="RMSE",
                eval_metric="RMSE",
                random_seed=args.random_seed,
                verbose=100,
            )
        else:
            train_pool = Pool(x_train, label=y_train, cat_features=cat_indexes)
            valid_pool = Pool(x_valid, label=y_valid, cat_features=cat_indexes)
            model = CatBoostClassifier(
                iterations=args.iterations,
                depth=args.depth,
                learning_rate=args.learning_rate,
                loss_function="Logloss",
                eval_metric="AUC",
                random_seed=args.random_seed,
                auto_class_weights="Balanced",
                verbose=100,
            )
        model.fit(train_pool, eval_set=valid_pool, use_best_model=not args.no_use_best_model)

        baseline_win_rate = float(y_valid.mean())
        auc = None
        model_logloss = None
        rmse = None
        mae = None

        scored = valid_df.copy()
        if args.model_objective == "expected_r":
            valid_pred = model.predict(valid_pool)
            rmse = float(mean_squared_error(valid_result_r, valid_pred) ** 0.5)
            mae = float(mean_absolute_error(valid_result_r, valid_pred))
            scored["model_score"] = valid_pred
            scored["predicted_expected_r"] = valid_pred
            scored["predicted_win_probability"] = 0.0
            score_column = "model_score"
        else:
            valid_pred = model.predict_proba(valid_pool)[:, 1]
            auc = float(roc_auc_score(y_valid, valid_pred)) if y_valid.nunique() > 1 else None
            model_logloss = float(log_loss(y_valid, valid_pred)) if y_valid.nunique() > 1 else None
            scored["model_score"] = valid_pred
            scored["predicted_win_probability"] = valid_pred
            scored["predicted_expected_r"] = np.nan
            score_column = "model_score"

        summaries = threshold_summaries(scored, thresholds, score_column)
        best_summary = choose_best_threshold(summaries, args.min_selected_trades)
        top_candidates = best_candidate_per_setup(scored, score_column)
        best_selected = top_candidates[
            top_candidates[score_column] >= float(best_summary["threshold"])
        ].copy()

        model_dir = ABCD_ROOT / "tmp_logs"
        model_dir.mkdir(parents=True, exist_ok=True)
        model_path = model_dir / f"{ai_build_run_id}.cbm"
        model.save_model(str(model_path))

        importances = model.get_feature_importance(train_pool)
        feature_importance = sorted(
            zip(x_train.columns.tolist(), [float(v) for v in importances]),
            key=lambda item: item[1],
            reverse=True,
        )

        store_results(
            conn,
            args,
            ai_build_run_id,
            len(train_setup_ids),
            len(valid_setup_ids),
            len(train_df),
            len(valid_df),
            baseline_win_rate,
            auc,
            model_logloss,
            rmse,
            mae,
            str(model_path),
            train_start_year,
            train_end_year,
            summaries,
            best_summary,
            best_selected,
            feature_importance,
        )

        print("\nValidation baseline:")
        print(f"  win rate: {baseline_win_rate:.2%}")
        print(f"  avg R: {float(valid_result_r.mean()):.4f}R")
        if auc is not None:
            print(f"  AUC: {auc:.4f}")
        if model_logloss is not None:
            print(f"  logloss: {model_logloss:.4f}")
        if rmse is not None:
            print(f"  RMSE: {rmse:.4f}R")
        if mae is not None:
            print(f"  MAE: {mae:.4f}R")

        label = "predicted R" if args.model_objective == "expected_r" else "win probability"
        print(f"\nTop candidate per setup by {label}:")
        for summary in summaries:
            if summary["selection_mode"] != "top_candidate_per_setup":
                continue
            win_rate = summary["win_rate"]
            avg_r = summary["avg_r"]
            win_rate_text = "n/a" if win_rate is None else f"{win_rate:.2%}"
            avg_r_text = "n/a" if avg_r is None else f"{avg_r:.3f}R"
            print(
                f"  >= {summary['threshold']:.2f}: "
                f"{summary['rows_seen']} trades, "
                f"{win_rate_text} win, "
                f"{avg_r_text} avg, "
                f"{summary['sum_r']:.1f}R sum"
            )

        print("\nBest stored result:")
        print(
            f"  {ai_build_run_id} threshold >= {best_summary['threshold']:.2f}: "
            f"{best_summary['rows_seen']} trades, "
            f"{best_summary['win_rate']:.2%} win, "
            f"{best_summary['avg_r']:.3f}R avg, "
            f"{best_summary['sum_r']:.1f}R sum"
        )
        print(f"  model: {model_path}")
        return 0
    finally:
        conn.close()


if __name__ == "__main__":
    sys.exit(main())
