#!/usr/bin/env python3
"""Versioned storage for oracle-start live-grid candidate rows.

The row payload is intentionally stored as compressed pandas pickle files
because this environment does not currently include a parquet engine. MySQL
stores the registry/metadata so runs can prove which candidate set they used
without forcing model training to read millions of wide feature rows from SQL.
"""

from __future__ import annotations

import hashlib
import json
import math
import os
import tempfile
from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd


SET_TABLE = "ai_oracle_start_candidate_sets"
YEAR_TABLE = "ai_oracle_start_candidate_years"
CACHE_VERSION = 1
DEFAULT_CACHE_DIRNAME = "_oracle_start_candidate_rows"


def to_jsonable(value: Any) -> Any:
    if isinstance(value, pd.Timestamp):
        return None if pd.isna(value) else value.isoformat()
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def parse_list_upper(raw: str) -> list[str]:
    return [part.strip().upper() for part in str(raw or "").split(",") if part.strip()]


def list_label(values: list[str], empty_label: str) -> str:
    if not values:
        return empty_label
    joined = "_".join(values)
    if len(joined) <= 42:
        return joined
    return hashlib.sha1(joined.encode("utf-8")).hexdigest()[:12]


def label_params(args: Any) -> dict[str, Any]:
    return {
        "positive_pre_bars": int(getattr(args, "positive_pre_bars", 0)),
        "positive_post_bars": int(getattr(args, "positive_post_bars", 0)),
        "negative_exclusion_bars": int(getattr(args, "negative_exclusion_bars", 0)),
        "match_window_bars": int(getattr(args, "match_window_bars", 0)),
    }


def feature_params(args: Any) -> dict[str, Any]:
    return {
        "entry_breakout_bars": int(getattr(args, "entry_breakout_bars", 8)),
        "trail_lookback_bars": int(getattr(args, "trail_lookback_bars", 18)),
        "atr_period": int(getattr(args, "atr_period", 14)),
        "atr_stop_pad": float(getattr(args, "atr_stop_pad", 0.35)),
        "min_risk_ticks": float(getattr(args, "min_risk_ticks", 12.0)),
        "max_risk_ticks": float(getattr(args, "max_risk_ticks", 240.0)),
        "min_relative_volume": float(getattr(args, "min_relative_volume", 0.45)),
    }


def candidate_set_payload(args: Any) -> dict[str, Any]:
    roots = parse_list_upper(getattr(args, "roots", ""))
    symbols = parse_list_upper(getattr(args, "symbols", ""))
    return {
        "cache_version": CACHE_VERSION,
        "row_type": "oracle_start_live_grid",
        "timeframe": str(getattr(args, "timeframe", "")).lower(),
        "roots": roots,
        "roots_empty_means_all": True,
        "symbols": symbols,
        "oracle_run_id": str(getattr(args, "oracle_run_id", "")),
        "limit_symbols": int(getattr(args, "limit_symbols", 0) or 0),
        "label_parameters": label_params(args),
        "feature_parameters": feature_params(args),
    }


def default_candidate_set_id(args: Any) -> str:
    explicit = str(getattr(args, "candidate_set_id", "") or "").strip()
    if explicit:
        return explicit
    payload = candidate_set_payload(args)
    roots = list_label(payload["roots"], "all")
    symbols = list_label(payload["symbols"], "all-symbols")
    raw = json.dumps(payload, sort_keys=True)
    digest = hashlib.sha1(raw.encode("utf-8")).hexdigest()[:12]
    return f"oscand-{payload['timeframe']}-{roots}-{symbols}-v{CACHE_VERSION}-{digest}"


def cache_root(args: Any, abcd_root: Path) -> Path:
    explicit = str(getattr(args, "candidate_cache_dir", "") or "").strip()
    if explicit:
        return Path(explicit).expanduser().resolve()
    return abcd_root / "model_registry" / DEFAULT_CACHE_DIRNAME


def candidate_set_dir(args: Any, abcd_root: Path) -> Path:
    return cache_root(args, abcd_root) / default_candidate_set_id(args)


def candidate_year_path(args: Any, abcd_root: Path, year: int) -> Path:
    return candidate_set_dir(args, abcd_root) / f"year_{int(year)}.pkl.gz"


def candidate_year_metadata_path(args: Any, abcd_root: Path, year: int) -> Path:
    return candidate_set_dir(args, abcd_root) / f"year_{int(year)}.metadata.json"


def ensure_tables(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            f"""
            CREATE TABLE IF NOT EXISTS {SET_TABLE} (
                candidate_set_id VARCHAR(128) NOT NULL PRIMARY KEY,
                cache_version INT NOT NULL,
                row_type VARCHAR(64) NOT NULL,
                storage_format VARCHAR(32) NOT NULL,
                cache_dir VARCHAR(768) NOT NULL,
                timeframe VARCHAR(16) NOT NULL,
                roots_json JSON NULL,
                roots_empty_means_all TINYINT(1) NOT NULL DEFAULT 1,
                symbols_json JSON NULL,
                oracle_run_id VARCHAR(160) NOT NULL,
                limit_symbols INT NOT NULL DEFAULT 0,
                label_parameters_json JSON NOT NULL,
                feature_parameters_json JSON NOT NULL,
                payload_hash VARCHAR(40) NOT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                INDEX idx_oracle_start_candidate_sets_tf (timeframe, oracle_run_id)
            )
            """
        )
        cur.execute(
            f"""
            CREATE TABLE IF NOT EXISTS {YEAR_TABLE} (
                candidate_set_id VARCHAR(128) NOT NULL,
                valid_year INT NOT NULL,
                cache_path VARCHAR(900) NOT NULL,
                storage_format VARCHAR(32) NOT NULL,
                row_count BIGINT NOT NULL DEFAULT 0,
                positive_rows BIGINT NOT NULL DEFAULT 0,
                negative_allowed_rows BIGINT NOT NULL DEFAULT 0,
                oracle_starts BIGINT NOT NULL DEFAULT 0,
                oracle_starts_available BIGINT NOT NULL DEFAULT 0,
                eligible_candidates BIGINT NOT NULL DEFAULT 0,
                symbols_seen INT NOT NULL DEFAULT 0,
                summary_json JSON NULL,
                status VARCHAR(32) NOT NULL DEFAULT 'ready',
                built_at DATETIME NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                PRIMARY KEY (candidate_set_id, valid_year),
                INDEX idx_oracle_start_candidate_years_status (status, valid_year),
                INDEX idx_oracle_start_candidate_years_rows (valid_year, row_count)
            )
            """
        )
    conn.commit()


def upsert_set(conn, args: Any, abcd_root: Path) -> str:
    ensure_tables(conn)
    candidate_set_id = default_candidate_set_id(args)
    payload = candidate_set_payload(args)
    payload_hash = hashlib.sha1(json.dumps(payload, sort_keys=True).encode("utf-8")).hexdigest()
    cache_dir = str(candidate_set_dir(args, abcd_root))
    with conn.cursor() as cur:
        cur.execute(
            f"""
            INSERT INTO {SET_TABLE} (
                candidate_set_id, cache_version, row_type, storage_format, cache_dir,
                timeframe, roots_json, roots_empty_means_all, symbols_json, oracle_run_id,
                limit_symbols, label_parameters_json, feature_parameters_json, payload_hash
            ) VALUES (%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s)
            ON DUPLICATE KEY UPDATE
                cache_version = VALUES(cache_version),
                row_type = VALUES(row_type),
                storage_format = VALUES(storage_format),
                cache_dir = VALUES(cache_dir),
                timeframe = VALUES(timeframe),
                roots_json = VALUES(roots_json),
                roots_empty_means_all = VALUES(roots_empty_means_all),
                symbols_json = VALUES(symbols_json),
                oracle_run_id = VALUES(oracle_run_id),
                limit_symbols = VALUES(limit_symbols),
                label_parameters_json = VALUES(label_parameters_json),
                feature_parameters_json = VALUES(feature_parameters_json),
                payload_hash = VALUES(payload_hash)
            """,
            (
                candidate_set_id,
                CACHE_VERSION,
                payload["row_type"],
                "pandas_pickle_gzip",
                cache_dir,
                payload["timeframe"],
                json.dumps(payload["roots"]),
                1,
                json.dumps(payload["symbols"]),
                payload["oracle_run_id"],
                payload["limit_symbols"],
                json.dumps(payload["label_parameters"], sort_keys=True),
                json.dumps(payload["feature_parameters"], sort_keys=True),
                payload_hash,
            ),
        )
    conn.commit()
    return candidate_set_id


def count_positive(frame: pd.DataFrame) -> int:
    if "is_oracle_start" not in frame.columns:
        return 0
    return int(pd.to_numeric(frame["is_oracle_start"], errors="coerce").fillna(0).astype(int).sum())


def count_negative_allowed(frame: pd.DataFrame) -> int:
    if "negative_allowed" not in frame.columns:
        if "is_oracle_start" not in frame.columns:
            return 0
        labels = pd.to_numeric(frame["is_oracle_start"], errors="coerce").fillna(0).astype(int)
        return int((labels == 0).sum())
    labels = pd.to_numeric(frame["is_oracle_start"], errors="coerce").fillna(0).astype(int)
    allowed = pd.to_numeric(frame["negative_allowed"], errors="coerce").fillna(0).astype(int)
    return int(((labels == 0) & (allowed == 1)).sum())


def write_frame_atomic(frame: pd.DataFrame, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=str(path.parent))
    os.close(fd)
    tmp_path = Path(tmp_name)
    try:
        frame.to_pickle(tmp_path, compression="gzip")
        tmp_path.replace(path)
    finally:
        if tmp_path.exists():
            tmp_path.unlink()


def save_year(conn, args: Any, abcd_root: Path, year: int, frame: pd.DataFrame, summary: dict[str, Any]) -> Path:
    candidate_set_id = upsert_set(conn, args, abcd_root)
    path = candidate_year_path(args, abcd_root, int(year))
    replace = bool(getattr(args, "replace_candidate_cache", False))
    if path.exists() and not replace:
        raise FileExistsError(f"Candidate cache already exists: {path}. Use --replace-candidate-cache.")
    write_frame_atomic(frame, path)

    row_count = int(len(frame))
    positive_rows = count_positive(frame)
    negative_allowed_rows = count_negative_allowed(frame)
    metadata = {
        "candidate_set_id": candidate_set_id,
        "valid_year": int(year),
        "cache_path": str(path),
        "storage_format": "pandas_pickle_gzip",
        "row_count": row_count,
        "positive_rows": positive_rows,
        "negative_allowed_rows": negative_allowed_rows,
        "summary": summary,
        "payload": candidate_set_payload(args),
    }
    candidate_year_metadata_path(args, abcd_root, int(year)).write_text(
        json.dumps(metadata, indent=2, default=to_jsonable),
        encoding="utf-8",
    )
    with conn.cursor() as cur:
        cur.execute(
            f"""
            INSERT INTO {YEAR_TABLE} (
                candidate_set_id, valid_year, cache_path, storage_format, row_count,
                positive_rows, negative_allowed_rows, oracle_starts,
                oracle_starts_available, eligible_candidates, symbols_seen,
                summary_json, status, built_at
            ) VALUES (%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,'ready',NOW())
            ON DUPLICATE KEY UPDATE
                cache_path = VALUES(cache_path),
                storage_format = VALUES(storage_format),
                row_count = VALUES(row_count),
                positive_rows = VALUES(positive_rows),
                negative_allowed_rows = VALUES(negative_allowed_rows),
                oracle_starts = VALUES(oracle_starts),
                oracle_starts_available = VALUES(oracle_starts_available),
                eligible_candidates = VALUES(eligible_candidates),
                symbols_seen = VALUES(symbols_seen),
                summary_json = VALUES(summary_json),
                status = 'ready',
                built_at = NOW()
            """,
            (
                candidate_set_id,
                int(year),
                str(path),
                "pandas_pickle_gzip",
                row_count,
                positive_rows,
                negative_allowed_rows,
                int(summary.get("oracle_starts", 0) or 0),
                int(summary.get("oracle_starts_available", 0) or 0),
                int(summary.get("eligible_candidates", 0) or 0),
                int(summary.get("symbols_seen", 0) or 0),
                json.dumps(summary, default=to_jsonable),
            ),
        )
    conn.commit()
    return path


def load_year(conn, args: Any, abcd_root: Path, year: int) -> tuple[pd.DataFrame, dict[str, Any]]:
    ensure_tables(conn)
    candidate_set_id = default_candidate_set_id(args)
    path = candidate_year_path(args, abcd_root, int(year))
    summary: dict[str, Any] = {}
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT cache_path, summary_json, status
            FROM {YEAR_TABLE}
            WHERE candidate_set_id = %s AND valid_year = %s
            """,
            (candidate_set_id, int(year)),
        )
        row = cur.fetchone()
    if row:
        if str(row.get("status") or "") != "ready":
            raise ValueError(f"Candidate cache is not ready: {candidate_set_id} {year} status={row.get('status')}")
        db_path = Path(str(row.get("cache_path") or "")).expanduser()
        if str(db_path):
            path = db_path
        raw_summary = row.get("summary_json")
        if isinstance(raw_summary, str) and raw_summary:
            summary = json.loads(raw_summary)
        elif isinstance(raw_summary, dict):
            summary = raw_summary
    if not path.exists():
        raise FileNotFoundError(f"Candidate cache not found: {path}")
    frame = pd.read_pickle(path, compression="gzip")
    if not summary:
        metadata_path = candidate_year_metadata_path(args, abcd_root, int(year))
        if metadata_path.exists():
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            summary = dict(metadata.get("summary") or {})
    if not summary:
        summary = {
            "year": int(year),
            "materialized_rows": int(len(frame)),
            "materialized_positives": count_positive(frame),
            "train_sample": False,
            "candidate_cache_loaded_without_summary": True,
        }
    print(f"{year}: loaded candidate cache {candidate_set_id} rows={len(frame):,} path={path}", flush=True)
    return frame, summary


def has_year(conn, args: Any, abcd_root: Path, year: int) -> bool:
    candidate_set_id = default_candidate_set_id(args)
    path = candidate_year_path(args, abcd_root, int(year))
    if path.exists():
        return True
    ensure_tables(conn)
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT cache_path
            FROM {YEAR_TABLE}
            WHERE candidate_set_id = %s AND valid_year = %s AND status = 'ready'
            """,
            (candidate_set_id, int(year)),
        )
        row = cur.fetchone()
    if not row:
        return False
    return Path(str(row.get("cache_path") or "")).exists()
