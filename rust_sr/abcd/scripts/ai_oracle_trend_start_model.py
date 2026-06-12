#!/usr/bin/env python3
"""
Train a candle-level oracle trend-start detector.

This is not a trade model. It teaches a model to answer one question:

    "Does this live-safe candle snapshot look like the start of an oracle trend?"

Labels come from ai_oracle_trend_trades. Inputs only use candle data available
at the signal candle.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd

try:
    from catboost import CatBoostClassifier, Pool
except ImportError as exc:  # pragma: no cover
    raise SystemExit("CatBoost is required. Run with .venv_ai\\Scripts\\python.exe") from exc

from sklearn.metrics import average_precision_score, precision_recall_fscore_support, roc_auc_score


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_scanner_model as scanner
import ai_wave_rider_research as wave


DEFAULT_ORACLE_RUN = "oracle-perfect-trends-v1-2m-2024_2026"

CAT_FEATURES = [
    "root_symbol",
    "symbol",
    "direction",
    "signal_day_of_week",
    "signal_month",
    "signal_hour_bucket",
    "trend_label",
    "directional_trend",
    "volume_bucket",
    "compression_bucket",
]

NUM_FEATURES = [
    "signal_hour",
    "signal_score",
    "risk_ticks",
    "atr_ticks",
    "atr_pct",
    "prior_range_ticks",
    "ema_gap_atr",
    "ema_fast_slope_atr",
    "breakout_atr",
    "relative_volume",
    "compression",
    "close_location_prior_range",
    "ret_3_atr",
    "ret_6_atr",
    "ret_12_atr",
    "distance_to_high_48_atr",
    "distance_to_low_48_atr",
    "range_position_48",
    "body_atr",
    "upper_wick_atr",
    "lower_wick_atr",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle-run-id", default=DEFAULT_ORACLE_RUN)
    parser.add_argument("--run-prefix", default="aicw-oracle-start-v1")
    parser.add_argument("--timeframe", default="2m", choices=sorted(scanner.TIMEFRAME_TABLES))
    parser.add_argument("--train-years", default="2024")
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--roots", default="")
    parser.add_argument("--positive-lag-bars", type=int, default=2)
    parser.add_argument("--negative-exclusion-bars", type=int, default=8)
    parser.add_argument("--match-window-bars", type=int, default=4)
    parser.add_argument("--negative-ratio", type=float, default=2.0)
    parser.add_argument("--base-negatives-per-symbol", type=int, default=12)
    parser.add_argument("--min-threshold-precision", type=float, default=0.35)
    parser.add_argument("--min-threshold-picks", type=int, default=300)
    parser.add_argument("--iterations", type=int, default=650)
    parser.add_argument("--depth", type=int, default=6)
    parser.add_argument("--learning-rate", type=float, default=0.045)
    parser.add_argument("--l2-leaf-reg", type=float, default=10.0)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--max-train-rows", type=int, default=0)
    parser.add_argument("--max-eval-rows", type=int, default=0)
    parser.add_argument("--replace-run", action="store_true")

    # Feature defaults matching the candle-wave scanner.
    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=240.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    return parser.parse_args()


def parse_list(raw: str) -> list[str]:
    return [part.strip().upper() for part in str(raw or "").split(",") if part.strip()]


def parse_years(raw: str) -> list[int]:
    years = [int(part) for part in parse_list(raw)]
    if not years:
        raise ValueError("At least one train year is required")
    return years


def run_id(args: argparse.Namespace) -> str:
    train = "_".join(str(year) for year in parse_years(args.train_years))
    rid = f"{args.run_prefix}-{args.timeframe}-tr{train}-v{args.valid_year}"
    if len(rid) > 64:
        digest = hashlib.sha1(rid.encode("utf-8")).hexdigest()[:10]
        rid = f"{args.run_prefix[:38]}-{args.timeframe}-{digest}"
    return rid


def fetch_candles(conn, timeframe: str, year: int, roots: list[str]) -> pd.DataFrame:
    table_name, minutes = scanner.table_for_timeframe(timeframe)
    table = scanner.safe_identifier(table_name)
    start = pd.Timestamp(year=year, month=1, day=1) - pd.Timedelta(days=3)
    end = pd.Timestamp(year=year + 1, month=1, day=1) + pd.Timedelta(days=3)
    params: list[Any] = [start.to_pydatetime(), end.to_pydatetime()]
    root_sql = ""
    if roots:
        root_sql = "AND root_symbol IN (" + ",".join(["%s"] * len(roots)) + ")"
        params.extend(roots)
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT root_symbol, symbol, ts_utc,
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
    return frame.dropna(subset=["root_symbol", "symbol", "ts_utc", "open", "high", "low", "close"]).reset_index(drop=True)


def fetch_oracle_starts(conn, oracle_run_id: str, year: int, roots: list[str]) -> pd.DataFrame:
    params: list[Any] = [oracle_run_id, year]
    root_sql = ""
    if roots:
        root_sql = "AND root_symbol IN (" + ",".join(["%s"] * len(roots)) + ")"
        params.extend(roots)
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT oracle_trade_id, symbol, root_symbol, direction, entry_date, result_r, quality_score
            FROM ai_oracle_trend_trades
            WHERE run_id = %s
              AND valid_year = %s
              {root_sql}
            ORDER BY symbol, direction, entry_date
            """,
            params,
        )
        rows = cur.fetchall()
    frame = pd.DataFrame(rows)
    if frame.empty:
        return frame
    frame["entry_date"] = pd.to_datetime(frame["entry_date"], errors="coerce")
    frame["result_r"] = pd.to_numeric(frame["result_r"], errors="coerce")
    frame["quality_score"] = pd.to_numeric(frame["quality_score"], errors="coerce")
    return frame.dropna(subset=["symbol", "direction", "entry_date"]).reset_index(drop=True)


def candidate_uid(symbol: str, ts: pd.Timestamp, direction: str) -> str:
    raw = f"{symbol}|{ts.isoformat()}|{direction}|oracle_start"
    return hashlib.sha1(raw.encode("utf-8")).hexdigest()[:32]


def feature_row(year: int, root: str, symbol: str, candles: pd.DataFrame, idx: int, direction: str) -> dict[str, Any]:
    row = candles.iloc[idx]
    ts = pd.Timestamp(row["ts_utc"])
    tick_size = float(wave.tick_size_for(symbol, root))
    atr = wave.finite(row.get("atr"), tick_size * 12.0) or tick_size * 12.0
    close = wave.finite(row.get("close"), 0.0) or 0.0
    entry_high = wave.finite(row.get("entry_high"), close) or close
    entry_low = wave.finite(row.get("entry_low"), close) or close
    prior_range = wave.finite(row.get("prior_range"), 0.0) or 0.0
    sign = wave.direction_sign(direction)
    if direction == "LONG":
        breakout = (close - entry_high) / atr
    else:
        breakout = (entry_low - close) / atr
    close_location = ((close - entry_low) / prior_range) if prior_range > 0 else 0.5
    high_48 = wave.finite(row.get("prior_high_48"))
    low_48 = wave.finite(row.get("prior_low_48"))
    range_48 = (high_48 - low_48) if high_48 is not None and low_48 is not None else None
    range_position = (close - low_48) / range_48 if low_48 is not None and range_48 and range_48 > 0 else None
    label = scanner.trend_label(row)
    rel_volume = wave.finite(row.get("relative_volume"), 1.0)
    compression = wave.finite(row.get("compression"), 2.0)
    risk_ticks = None
    if idx + 1 < len(candles):
        stop, risk_points = wave.initial_stop(candles, idx + 1, direction, tick_size, argparse.Namespace(
            min_risk_ticks=12.0,
            max_risk_ticks=240.0,
            atr_stop_pad=0.35,
        ))
        if risk_points is not None and tick_size > 0:
            risk_ticks = risk_points / tick_size
    return {
        "candidate_uid": candidate_uid(symbol, ts, direction),
        "valid_year": year,
        "root_symbol": root,
        "symbol": symbol,
        "signal_date": ts,
        "direction": direction,
        "signal_hour": int(ts.hour),
        "signal_day_of_week": str(ts.dayofweek),
        "signal_month": str(ts.month),
        "signal_hour_bucket": scanner.bucket(float(ts.hour), [6, 12, 18, 23], ["overnight", "morning", "afternoon", "evening"]),
        "trend_label": label,
        "directional_trend": scanner.directional_trend(direction, label),
        "volume_bucket": scanner.bucket(rel_volume, [0.75, 1.25, 2.0, 999.0], ["quiet", "normal", "active", "hot"]),
        "compression_bucket": scanner.bucket(compression, [1.0, 2.0, 4.0, 999.0], ["tight", "normal", "wide", "loose"]),
        "signal_score": wave.trigger_score(candles, idx, direction),
        "risk_ticks": risk_ticks,
        "atr_ticks": atr / tick_size if tick_size > 0 else None,
        "atr_pct": atr / close * 100.0 if close else None,
        "prior_range_ticks": prior_range / tick_size if tick_size > 0 else None,
        "ema_gap_atr": sign * ((wave.finite(row.get("ema_fast"), close) or close) - (wave.finite(row.get("ema_slow"), close) or close)) / atr,
        "ema_fast_slope_atr": sign * (wave.finite(row.get("ema_fast_slope"), 0.0) or 0.0) / atr,
        "breakout_atr": breakout,
        "relative_volume": rel_volume,
        "compression": compression,
        "close_location_prior_range": close_location,
        "ret_3_atr": sign * (wave.finite(row.get("ret_3"), 0.0) or 0.0) * close / atr if atr > 0 else None,
        "ret_6_atr": sign * (wave.finite(row.get("ret_6"), 0.0) or 0.0) * close / atr if atr > 0 else None,
        "ret_12_atr": sign * (wave.finite(row.get("ret_12"), 0.0) or 0.0) * close / atr if atr > 0 else None,
        "distance_to_high_48_atr": sign * ((high_48 - close) / atr) if high_48 is not None and atr > 0 else None,
        "distance_to_low_48_atr": sign * ((close - low_48) / atr) if low_48 is not None and atr > 0 else None,
        "range_position_48": range_position,
        "body_atr": wave.finite(row.get("body"), 0.0) / atr if atr > 0 else None,
        "upper_wick_atr": wave.finite(row.get("upper_wick"), 0.0) / atr if atr > 0 else None,
        "lower_wick_atr": wave.finite(row.get("lower_wick"), 0.0) / atr if atr > 0 else None,
    }


def prepare_pool(frame: pd.DataFrame, include_target: bool) -> Pool:
    work = frame.copy()
    for col in CAT_FEATURES:
        if col not in work.columns:
            work[col] = "unknown"
        work[col] = work[col].fillna("unknown").astype(str)
    for col in NUM_FEATURES:
        if col not in work.columns:
            work[col] = 0.0
        work[col] = pd.to_numeric(work[col], errors="coerce").replace([np.inf, -np.inf], np.nan).fillna(0.0)
    features = CAT_FEATURES + NUM_FEATURES
    if include_target:
        return Pool(work[features], label=work["is_oracle_start"].astype(int), cat_features=CAT_FEATURES)
    return Pool(work[features], cat_features=CAT_FEATURES)


def build_year_dataset(conn, year: int, args: argparse.Namespace, rng: np.random.Generator) -> pd.DataFrame:
    roots = parse_list(args.roots)
    candles = fetch_candles(conn, args.timeframe, year, roots)
    oracle = fetch_oracle_starts(conn, args.oracle_run_id, year, roots)
    if candles.empty or oracle.empty:
        raise ValueError(f"Missing candles or oracle starts for {year}")
    print(f"{year}: candles={len(candles):,} symbols={candles['symbol'].nunique():,} oracle_starts={len(oracle):,}")
    oracle_by_key = {
        (str(symbol), str(direction)): group.sort_values("entry_date").reset_index(drop=True)
        for (symbol, direction), group in oracle.groupby(["symbol", "direction"], sort=False)
    }
    rows: list[dict[str, Any]] = []
    min_idx = 60
    year_start = pd.Timestamp(year=year, month=1, day=1)
    year_end = pd.Timestamp(year=year + 1, month=1, day=1)
    for symbol_index, (symbol, raw_group) in enumerate(candles.groupby("symbol", sort=True), start=1):
        root = str(raw_group["root_symbol"].iloc[0])
        group = scanner.enrich_candles(raw_group[["ts_utc", "open", "high", "low", "close", "volume"]].copy(), args)
        times = pd.to_datetime(group["ts_utc"], errors="coerce")
        valid_indices = np.where((times >= year_start) & (times < year_end))[0]
        valid_indices = valid_indices[(valid_indices >= min_idx) & (valid_indices < len(group) - 1)]
        if len(valid_indices) == 0:
            continue
        for direction in ["LONG", "SHORT"]:
            starts = oracle_by_key.get((str(symbol), direction))
            positive_indices: set[int] = set()
            exclusion_indices: set[int] = set()
            start_indices: list[int] = []
            if starts is not None and not starts.empty:
                arr = times.to_numpy(dtype="datetime64[ns]")
                for start_ts in starts["entry_date"]:
                    idx = int(np.searchsorted(arr, np.datetime64(pd.Timestamp(start_ts)), side="left"))
                    if idx >= len(group):
                        continue
                    start_indices.append(idx)
                    for lag in range(0, max(0, int(args.positive_lag_bars)) + 1):
                        candidate_idx = idx + lag
                        if candidate_idx in valid_indices:
                            positive_indices.add(candidate_idx)
                    for offset in range(-int(args.negative_exclusion_bars), int(args.negative_exclusion_bars) + 1):
                        exclusion_idx = idx + offset
                        if 0 <= exclusion_idx < len(group):
                            exclusion_indices.add(exclusion_idx)
            valid_set = set(int(i) for i in valid_indices)
            positive_indices &= valid_set
            negative_pool = np.array(sorted(valid_set - exclusion_indices - positive_indices), dtype=int)
            target_negatives = int(max(int(args.base_negatives_per_symbol), math.ceil(len(positive_indices) * float(args.negative_ratio))))
            if len(negative_pool) > target_negatives:
                negative_indices = rng.choice(negative_pool, size=target_negatives, replace=False)
            else:
                negative_indices = negative_pool
            for idx in sorted(positive_indices):
                payload = feature_row(year, root, str(symbol), group, idx, direction)
                payload.update({"is_oracle_start": 1, "oracle_start_distance_bars": 0})
                rows.append(payload)
            for idx in sorted(int(i) for i in negative_indices):
                payload = feature_row(year, root, str(symbol), group, idx, direction)
                nearest = min((abs(idx - sidx) for sidx in start_indices), default=999999)
                payload.update({"is_oracle_start": 0, "oracle_start_distance_bars": nearest})
                rows.append(payload)
        if symbol_index % 100 == 0:
            print(f"{year}: symbols {symbol_index:,}/{candles['symbol'].nunique():,} rows={len(rows):,}")
    frame = pd.DataFrame(rows)
    if frame.empty:
        raise ValueError(f"No training rows built for {year}")
    frame = frame.sample(frac=1.0, random_state=int(args.random_seed)).reset_index(drop=True)
    limit = int(args.max_train_rows if year in parse_years(args.train_years) else args.max_eval_rows)
    if limit > 0 and len(frame) > limit:
        positives = frame[frame["is_oracle_start"] == 1]
        negatives = frame[frame["is_oracle_start"] == 0]
        keep_pos = positives if len(positives) <= limit // 2 else positives.sample(n=limit // 2, random_state=int(args.random_seed))
        remaining = max(0, limit - len(keep_pos))
        keep_neg = negatives.sample(n=min(remaining, len(negatives)), random_state=int(args.random_seed))
        frame = pd.concat([keep_pos, keep_neg], ignore_index=True).sample(frac=1.0, random_state=int(args.random_seed)).reset_index(drop=True)
    print(f"{year}: built rows={len(frame):,} positives={int(frame['is_oracle_start'].sum()):,}")
    return frame


def train_model(train: pd.DataFrame, args: argparse.Namespace) -> CatBoostClassifier:
    model = CatBoostClassifier(
        loss_function="Logloss",
        eval_metric="AUC",
        iterations=int(args.iterations),
        depth=int(args.depth),
        learning_rate=float(args.learning_rate),
        l2_leaf_reg=float(args.l2_leaf_reg),
        random_seed=int(args.random_seed),
        auto_class_weights="Balanced",
        verbose=100,
        allow_writing_files=False,
    )
    model.fit(prepare_pool(train, include_target=True))
    return model


def add_scores(frame: pd.DataFrame, model: CatBoostClassifier) -> pd.DataFrame:
    scored = frame.copy()
    scored["oracle_start_score"] = model.predict_proba(prepare_pool(scored, include_target=False))[:, 1]
    return scored


def choose_threshold(scored: pd.DataFrame, args: argparse.Namespace) -> tuple[float, pd.DataFrame]:
    y = scored["is_oracle_start"].astype(int).to_numpy()
    score = pd.to_numeric(scored["oracle_start_score"], errors="coerce").fillna(0.0).to_numpy()
    candidates = sorted(set(float(v) for v in np.quantile(score, np.linspace(0.05, 0.99, 60))))
    rows = []
    best_threshold = candidates[0]
    best_key: tuple[float, float, int] | None = None
    for threshold in candidates:
        pred = (score >= threshold).astype(int)
        precision, recall, f1, _ = precision_recall_fscore_support(y, pred, average="binary", zero_division=0)
        picks = int(pred.sum())
        allowed = precision >= float(args.min_threshold_precision) and picks >= int(args.min_threshold_picks)
        rows.append({"threshold": threshold, "precision": precision, "recall": recall, "f1": f1, "picks": picks, "allowed": allowed})
        if allowed:
            key = (float(f1), float(precision), picks)
            if best_key is None or key > best_key:
                best_key = key
                best_threshold = threshold
    if best_key is None:
        best_threshold = max(candidates, key=lambda t: precision_recall_fscore_support(y, (score >= t).astype(int), average="binary", zero_division=0)[2])
    return best_threshold, pd.DataFrame(rows)


def label_metrics(scored: pd.DataFrame, threshold: float) -> dict[str, Any]:
    y = scored["is_oracle_start"].astype(int).to_numpy()
    score = pd.to_numeric(scored["oracle_start_score"], errors="coerce").fillna(0.0).to_numpy()
    pred = (score >= threshold).astype(int)
    precision, recall, f1, _ = precision_recall_fscore_support(y, pred, average="binary", zero_division=0)
    try:
        auc = float(roc_auc_score(y, score))
    except ValueError:
        auc = None
    try:
        ap = float(average_precision_score(y, score))
    except ValueError:
        ap = None
    return {
        "rows": int(len(scored)),
        "positives": int(y.sum()),
        "picks": int(pred.sum()),
        "precision": float(precision),
        "recall": float(recall),
        "f1": float(f1),
        "auc": auc,
        "average_precision": ap,
    }


def pick_closeness(scored: pd.DataFrame, threshold: float, args: argparse.Namespace) -> dict[str, Any]:
    selected = scored[pd.to_numeric(scored["oracle_start_score"], errors="coerce") >= threshold].copy()
    if selected.empty:
        return {"picks": 0, "matched_picks": 0, "match_rate": 0.0, "median_abs_bars": None, "mean_abs_bars": None}
    distances = pd.to_numeric(selected["oracle_start_distance_bars"], errors="coerce").fillna(999999)
    matched = distances <= int(args.match_window_bars)
    matched_distances = distances[matched]
    return {
        "picks": int(len(selected)),
        "matched_picks": int(matched.sum()),
        "match_rate": float(matched.mean()),
        "median_abs_bars": float(matched_distances.median()) if len(matched_distances) else None,
        "mean_abs_bars": float(matched_distances.mean()) if len(matched_distances) else None,
        "match_window_bars": int(args.match_window_bars),
    }


def main() -> int:
    args = parse_args()
    rid = run_id(args)
    model_dir = wave.ABCD_ROOT / "model_registry" / rid
    if model_dir.exists() and not args.replace_run:
        raise ValueError(f"Run already exists: {rid}. Use --replace-run.")
    model_dir.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(int(args.random_seed))
    train_years = parse_years(args.train_years)
    conn = wave.connect()
    try:
        train_frames = [build_year_dataset(conn, year, args, rng) for year in train_years]
        train = pd.concat(train_frames, ignore_index=True).sample(frac=1.0, random_state=int(args.random_seed)).reset_index(drop=True)
        threshold = build_year_dataset(conn, int(args.threshold_year), args, rng)
        valid = build_year_dataset(conn, int(args.valid_year), args, rng)
    finally:
        conn.close()
    model = train_model(train, args)
    train_scored = add_scores(train, model)
    threshold_scored = add_scores(threshold, model)
    valid_scored = add_scores(valid, model)
    selected_threshold, sweep = choose_threshold(threshold_scored, args)
    print(f"selected_threshold={selected_threshold:.6f}")
    for label, frame in [("train", train_scored), ("threshold", threshold_scored), ("valid", valid_scored)]:
        print(label, label_metrics(frame, selected_threshold), pick_closeness(frame, selected_threshold, args))
    model.save_model(str(model_dir / "catboost_oracle_start_model.cbm"))
    train_scored.to_csv(model_dir / f"oracle_start_scores_train.csv", index=False)
    threshold_scored.to_csv(model_dir / f"oracle_start_scores_{args.threshold_year}.csv", index=False)
    valid_scored.to_csv(model_dir / f"oracle_start_scores_{args.valid_year}.csv", index=False)
    sweep.to_csv(model_dir / f"threshold_sweep_{args.threshold_year}.csv", index=False)
    metadata = {
        "model_run_id": rid,
        "model_type": "catboost_oracle_trend_start_detector",
        "oracle_run_id": args.oracle_run_id,
        "timeframe": args.timeframe,
        "train_years": train_years,
        "threshold_year": args.threshold_year,
        "valid_year": args.valid_year,
        "features": {"cat": CAT_FEATURES, "num": NUM_FEATURES},
        "positive_lag_bars": args.positive_lag_bars,
        "negative_exclusion_bars": args.negative_exclusion_bars,
        "match_window_bars": args.match_window_bars,
        "selected_threshold": selected_threshold,
        "train_metrics": label_metrics(train_scored, selected_threshold),
        "threshold_metrics": label_metrics(threshold_scored, selected_threshold),
        "valid_metrics": label_metrics(valid_scored, selected_threshold),
        "valid_closeness": pick_closeness(valid_scored, selected_threshold, args),
        "leak_safety": "Labels come from oracle hindsight. Features use only signal candle and prior candle-derived indicators.",
    }
    (model_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=str), encoding="utf-8")
    print(f"Saved oracle trend-start detector: {rid}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
