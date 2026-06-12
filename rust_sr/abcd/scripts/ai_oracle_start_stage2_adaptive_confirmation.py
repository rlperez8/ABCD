#!/usr/bin/env python3
"""
Train an adaptive Stage 2 oracle-start confirmation / fakeout filter.

Stage 1 is the wide radar. For every Stage 1 signal, this script creates
confirmation rows at offset 1, 2, 3, ... max_confirm_bars. The model is then
used live-style by scoring each offset as it arrives and confirming on the
first offset whose score clears the learned threshold.

This is signal research only. It does not create trades, entries, or exits.
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

from sklearn.metrics import average_precision_score, roc_auc_score


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_scanner_model as scanner
import ai_oracle_start_manager_model as manager_model
import ai_oracle_start_stage2_confirmation as fixed_stage2
import ai_oracle_trend_start_model as start_model
import ai_wave_rider_research as wave


DEFAULT_FOUR_MODEL_RUN = fixed_stage2.DEFAULT_FOUR_MODEL_RUN

ADAPTIVE_NUM_FEATURES = [
    "confirm_offset_bars",
    "confirm_offset_ratio",
    "confirm_favorable_to_adverse_ratio",
    "confirm_close_to_favorable_ratio",
    "confirm_giveback_from_favorable_atr",
    "confirm_best_close_giveback_atr",
    "confirm_net_cleanliness_atr",
    "confirm_close_drawup_efficiency",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--four-model-run-id", default=DEFAULT_FOUR_MODEL_RUN)
    parser.add_argument("--run-prefix", default="aicw-oracle-start-stage2-adaptive-v1")
    parser.add_argument("--train-label", default="train")
    parser.add_argument("--train-year", type=int, default=2024)
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--timeframe", default="2m")
    parser.add_argument("--roots", default="")
    parser.add_argument("--max-confirm-bars", type=int, default=16)
    parser.add_argument("--oracle-run-id", default=start_model.DEFAULT_ORACLE_RUN)
    parser.add_argument("--oracle-detail-match-bars", type=int, default=4)
    parser.add_argument("--target-mode", choices=["event", "proof"], default="proof")
    parser.add_argument("--min-confirm-close-atr", type=float, default=0.15)
    parser.add_argument("--min-confirm-favorable-atr", type=float, default=0.35)
    parser.add_argument("--max-confirm-adverse-atr", type=float, default=1.25)
    parser.add_argument("--min-remaining-r", type=float, default=0.75)
    parser.add_argument("--min-remaining-bars", type=int, default=2)
    parser.add_argument(
        "--stage1-rule",
        choices=["catboost", "any2", "tabular_majority", "all4", "image_tabular_majority"],
        default="catboost",
    )
    parser.add_argument("--stage1-threshold", type=float, default=0.385)
    parser.add_argument("--min-threshold-precision", type=float, default=0.78)
    parser.add_argument("--min-threshold-picks", type=int, default=300)
    parser.add_argument("--threshold-tail-candidates", type=int, default=350)
    parser.add_argument("--iterations", type=int, default=450)
    parser.add_argument("--depth", type=int, default=5)
    parser.add_argument("--learning-rate", type=float, default=0.045)
    parser.add_argument("--l2-leaf-reg", type=float, default=8.0)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--max-train-rows", type=int, default=0)
    parser.add_argument("--max-eval-rows", type=int, default=0)
    parser.add_argument("--skip-save-expanded-rows", action="store_true")
    parser.add_argument("--replace-run", action="store_true")

    # Candle enrichment defaults.
    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=240.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    return parser.parse_args()


def run_id(args: argparse.Namespace) -> str:
    raw = f"{args.run_prefix}-{args.stage1_rule}-m{int(args.max_confirm_bars)}-v{args.valid_year}"
    if len(raw) <= 64:
        return raw
    digest = hashlib.sha1(raw.encode("utf-8")).hexdigest()[:10]
    return f"{args.run_prefix[:34]}-{args.stage1_rule}-m{int(args.max_confirm_bars)}-{digest}"


def to_jsonable(value: Any) -> Any:
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, pd.Timestamp):
        return value.isoformat()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def source_dir(run_id_value: str) -> tuple[Path, dict[str, Any]]:
    return fixed_stage2.source_dir(run_id_value)


def load_split(model_dir: Path, split: str | int, limit: int) -> pd.DataFrame:
    frame = fixed_stage2.load_split(model_dir, split, limit)
    frame["signal_date"] = pd.to_datetime(frame["signal_date"], errors="coerce")
    return frame.dropna(subset=["candidate_uid", "symbol", "signal_date", "direction"]).reset_index(drop=True)


def stage1_mask(frame: pd.DataFrame, thresholds: dict[str, float], args: argparse.Namespace) -> np.ndarray:
    cat = pd.to_numeric(frame["catboost_score"], errors="coerce").fillna(0.0).to_numpy() >= float(args.stage1_threshold)
    xgb = pd.to_numeric(frame["xgboost_score"], errors="coerce").fillna(0.0).to_numpy() >= float(thresholds.get("xgboost", 0.5))
    lgbm = pd.to_numeric(frame["lightgbm_score"], errors="coerce").fillna(0.0).to_numpy() >= float(thresholds.get("lightgbm", 0.5))
    visual = pd.to_numeric(frame["visual_score"], errors="coerce").fillna(0.0).to_numpy() >= float(
        thresholds.get("visual_cnn", 0.5)
    )
    tabular_count = cat.astype(int) + xgb.astype(int) + lgbm.astype(int)
    if args.stage1_rule == "catboost":
        return cat
    if args.stage1_rule == "tabular_majority":
        return tabular_count >= 2
    if args.stage1_rule == "all4":
        return cat & xgb & lgbm & visual
    if args.stage1_rule == "image_tabular_majority":
        return visual & (tabular_count >= 2)
    return (tabular_count + visual.astype(int)) >= 2


def post_features_for_offset(
    group: pd.DataFrame,
    idx: int,
    direction: str,
    offset: int,
    max_confirm_bars: int,
    atr_hint: float | None,
) -> dict[str, Any] | None:
    if idx + int(offset) >= len(group):
        return None
    features = fixed_stage2.post_features(group, idx, direction, int(offset), atr_hint)
    if features is None:
        return None
    confirm_row = group.iloc[idx + int(offset)]
    features.update(
        {
            "confirm_offset_bars": int(offset),
            "confirm_offset_ratio": float(offset) / max(1.0, float(max_confirm_bars)),
            "confirm_date": pd.Timestamp(confirm_row["ts_utc"]),
            "confirm_price": float(confirm_row["close"]),
        }
    )
    close_move = float(features.get("confirm_close_move_atr", 0.0) or 0.0)
    max_favorable = float(features.get("confirm_max_favorable_atr", 0.0) or 0.0)
    max_adverse = float(features.get("confirm_max_adverse_atr", 0.0) or 0.0)
    best_close = float(features.get("confirm_best_close_atr", 0.0) or 0.0)
    features.update(
        {
            "confirm_favorable_to_adverse_ratio": max_favorable / max(0.1, max_adverse),
            "confirm_close_to_favorable_ratio": close_move / max(0.1, max_favorable),
            "confirm_giveback_from_favorable_atr": max_favorable - close_move,
            "confirm_best_close_giveback_atr": best_close - close_move,
            "confirm_net_cleanliness_atr": max_favorable - max_adverse,
            "confirm_close_drawup_efficiency": close_move / max(0.1, max_favorable + max_adverse),
        }
    )
    return features


def fetch_oracle_details(
    conn,
    year: int,
    roots: list[str],
    oracle_run_id: str,
) -> tuple[dict[tuple[str, str, pd.Timestamp], dict[str, Any]], dict[tuple[str, str], list[dict[str, Any]]]]:
    params: list[Any] = [oracle_run_id, int(year)]
    root_sql = ""
    if roots:
        root_sql = "AND root_symbol IN (" + ",".join(["%s"] * len(roots)) + ")"
        params.extend(roots)
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT
                oracle_trade_id,
                symbol,
                direction,
                entry_date,
                exit_date,
                CAST(exit_price AS DOUBLE) AS exit_price,
                CAST(risk_points AS DOUBLE) AS risk_points,
                duration_bars,
                CAST(result_r AS DOUBLE) AS result_r
            FROM ai_oracle_trend_trades
            WHERE run_id = %s
              AND valid_year = %s
              {root_sql}
            """,
            params,
        )
        rows = cur.fetchall()
    details: dict[tuple[str, str, pd.Timestamp], dict[str, Any]] = {}
    by_key: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for row in rows:
        entry = pd.Timestamp(row["entry_date"])
        symbol = str(row["symbol"])
        direction = str(row["direction"])
        key = (str(row["symbol"]), str(row["direction"]), entry)
        item = {
            "oracle_trade_id": str(row["oracle_trade_id"]),
            "oracle_entry_date": entry,
            "oracle_exit_date": pd.Timestamp(row["exit_date"]),
            "oracle_exit_price": wave.finite(row.get("exit_price"), None),
            "oracle_risk_points": wave.finite(row.get("risk_points"), None),
            "oracle_duration_bars": int(row["duration_bars"]) if row.get("duration_bars") is not None else None,
            "oracle_result_r": wave.finite(row.get("result_r"), None),
        }
        details[key] = item
        by_key.setdefault((symbol, direction), []).append(item)
    for values in by_key.values():
        values.sort(key=lambda value: pd.Timestamp(value["oracle_entry_date"]))
    return details, by_key


def match_oracle_info(
    exact: dict[tuple[str, str, pd.Timestamp], dict[str, Any]],
    by_key: dict[tuple[str, str], list[dict[str, Any]]],
    original: dict[str, Any],
    signal_ts: pd.Timestamp,
    args: argparse.Namespace,
) -> dict[str, Any] | None:
    symbol = str(original["symbol"])
    direction = str(original["direction"])
    direct = exact.get((symbol, direction, signal_ts))
    if direct is not None:
        return direct
    candidates = by_key.get((symbol, direction), [])
    if not candidates:
        return None
    timeframe_minutes = int(scanner.table_for_timeframe(args.timeframe)[1])
    max_minutes = max(0, int(args.oracle_detail_match_bars)) * max(1, timeframe_minutes)
    best: dict[str, Any] | None = None
    best_minutes: float | None = None
    for item in candidates:
        entry = pd.Timestamp(item["oracle_entry_date"])
        minutes = abs((signal_ts - entry).total_seconds()) / 60.0
        if minutes <= max_minutes and (best_minutes is None or minutes < best_minutes):
            best = item
            best_minutes = minutes
    return best


def proof_target(
    original: dict[str, Any],
    features: dict[str, Any],
    oracle_info: dict[str, Any] | None,
    args: argparse.Namespace,
) -> tuple[int, dict[str, Any]]:
    debug = {
        "oracle_trade_id": "",
        "oracle_exit_date": pd.NaT,
        "oracle_remaining_bars": None,
        "oracle_remaining_r": None,
        "stage2_target_reason": "negative_event",
    }
    if int(original.get("is_oracle_start", 0) or 0) != 1:
        return 0, debug
    if args.target_mode == "event":
        debug["stage2_target_reason"] = "oracle_event"
        return 1, debug
    if not oracle_info:
        debug["stage2_target_reason"] = "missing_oracle_detail"
        return 0, debug

    confirm_date = pd.Timestamp(features["confirm_date"])
    exit_date = pd.Timestamp(oracle_info["oracle_exit_date"])
    timeframe_minutes = int(scanner.table_for_timeframe(args.timeframe)[1])
    remaining_bars = max(0, int(round((exit_date - confirm_date).total_seconds() / 60.0 / max(1, timeframe_minutes))))
    risk_points = wave.finite(oracle_info.get("oracle_risk_points"), None)
    exit_price = wave.finite(oracle_info.get("oracle_exit_price"), None)
    confirm_price = wave.finite(features.get("confirm_price"), None)
    remaining_r = None
    if risk_points is not None and risk_points > 0 and exit_price is not None and confirm_price is not None:
        sign = wave.direction_sign(str(original["direction"]))
        remaining_r = sign * (exit_price - confirm_price) / risk_points

    debug.update(
        {
            "oracle_trade_id": oracle_info.get("oracle_trade_id", ""),
            "oracle_exit_date": exit_date,
            "oracle_remaining_bars": remaining_bars,
            "oracle_remaining_r": remaining_r,
        }
    )
    if remaining_bars < int(args.min_remaining_bars):
        debug["stage2_target_reason"] = "not_enough_bars_left"
        return 0, debug
    if remaining_r is None or remaining_r < float(args.min_remaining_r):
        debug["stage2_target_reason"] = "not_enough_r_left"
        return 0, debug
    if float(features.get("confirm_close_move_atr", 0.0) or 0.0) < float(args.min_confirm_close_atr):
        debug["stage2_target_reason"] = "weak_close_followthrough"
        return 0, debug
    if float(features.get("confirm_max_favorable_atr", 0.0) or 0.0) < float(args.min_confirm_favorable_atr):
        debug["stage2_target_reason"] = "weak_favorable_probe"
        return 0, debug
    if float(features.get("confirm_max_adverse_atr", 0.0) or 0.0) > float(args.max_confirm_adverse_atr):
        debug["stage2_target_reason"] = "too_much_adverse"
        return 0, debug

    debug["stage2_target_reason"] = "confirmed_oracle_proof"
    return 1, debug


def add_adaptive_confirmation_rows(
    conn,
    frame: pd.DataFrame,
    thresholds: dict[str, float],
    year: int,
    args: argparse.Namespace,
    label: str,
) -> tuple[pd.DataFrame, dict[str, Any]]:
    roots = start_model.parse_list(args.roots)
    oracle_exact, oracle_by_key = fetch_oracle_details(conn, int(year), roots, str(args.oracle_run_id))
    frame = frame.copy().reset_index(drop=True)
    frame["stage1_pick"] = stage1_mask(frame, thresholds, args).astype(int)
    source_positive_count = int(frame["is_oracle_start"].sum())
    stage1_events = frame[frame["stage1_pick"] == 1].copy().reset_index(drop=True)
    if stage1_events.empty:
        raise ValueError(f"{label}: Stage 1 did not pick any rows.")

    candles = start_model.fetch_candles(conn, args.timeframe, int(year), roots)
    if candles.empty:
        raise ValueError(f"{label}: no candles available for {year}")

    rows_by_symbol = {
        str(symbol): group.sort_values("signal_date").reset_index()
        for symbol, group in stage1_events.groupby("symbol", sort=False)
    }
    output: list[dict[str, Any]] = []
    seen_events = 0
    expanded_rows = 0
    for symbol_index, (symbol, raw_group) in enumerate(candles.groupby("symbol", sort=True), start=1):
        targets = rows_by_symbol.get(str(symbol))
        if targets is None or targets.empty:
            continue
        group = scanner.enrich_candles(raw_group[["ts_utc", "open", "high", "low", "close", "volume"]].copy(), args)
        times = pd.to_datetime(group["ts_utc"], errors="coerce").to_numpy(dtype="datetime64[ns]")
        for item in targets.itertuples(index=False):
            original = stage1_events.iloc[int(item.index)].to_dict()
            signal_ts = pd.Timestamp(original["signal_date"])
            idx = int(np.searchsorted(times, np.datetime64(signal_ts), side="left"))
            if idx >= len(group):
                continue
            tick_size = wave.tick_size_for(str(original["symbol"]), str(original.get("root_symbol") or ""))
            atr_ticks = wave.finite(original.get("atr_ticks"), None)
            atr_hint = atr_ticks * tick_size if atr_ticks is not None and tick_size > 0 else None
            oracle_info = match_oracle_info(oracle_exact, oracle_by_key, original, signal_ts, args)
            event_rows = 0
            for offset in range(1, int(args.max_confirm_bars) + 1):
                features = post_features_for_offset(
                    group,
                    idx,
                    str(original["direction"]),
                    offset,
                    int(args.max_confirm_bars),
                    atr_hint,
                )
                if features is None:
                    continue
                target, target_debug = proof_target(original, features, oracle_info, args)
                payload = dict(original)
                payload.update(features)
                payload.update(target_debug)
                payload["stage2_target"] = int(target)
                output.append(payload)
                event_rows += 1
            if event_rows:
                seen_events += 1
                expanded_rows += event_rows
        if symbol_index % 75 == 0:
            print(
                f"{label}: adaptive rows symbols={symbol_index:,} "
                f"events={seen_events:,}/{len(stage1_events):,} rows={expanded_rows:,}",
                flush=True,
            )

    expanded = pd.DataFrame(output)
    if expanded.empty:
        raise ValueError(f"{label}: no adaptive confirmation rows were built.")
    counts = expanded.groupby("candidate_uid")["candidate_uid"].transform("count").astype(float)
    expanded["event_row_weight"] = 1.0 / counts.replace(0.0, 1.0)
    summary = {
        "source_rows": int(len(frame)),
        "source_positives": source_positive_count,
        "stage1_events": int(len(stage1_events)),
        "stage1_positive_events": int(stage1_events["is_oracle_start"].sum()),
        "expanded_events": int(expanded["candidate_uid"].nunique()),
        "expanded_rows": int(len(expanded)),
        "stage1_precision": float(stage1_events["is_oracle_start"].mean()) if len(stage1_events) else 0.0,
        "stage1_total_oracle_recall": float(stage1_events["is_oracle_start"].sum() / source_positive_count)
        if source_positive_count
        else 0.0,
        "target_positive_rows": int(expanded["stage2_target"].sum()),
        "target_positive_events": int(expanded.loc[expanded["stage2_target"] == 1, "candidate_uid"].nunique()),
        "target_reason_counts": {
            str(key): int(value) for key, value in expanded["stage2_target_reason"].value_counts(dropna=False).items()
        },
    }
    print(f"{label}: built adaptive rows {json.dumps(summary, default=to_jsonable)}", flush=True)
    return expanded, summary


def feature_columns() -> tuple[list[str], list[str]]:
    cat_features = list(start_model.CAT_FEATURES)
    num_features = list(start_model.NUM_FEATURES) + manager_model.DERIVED_NUM_FEATURES
    num_features += fixed_stage2.POST_FEATURES + ADAPTIVE_NUM_FEATURES
    return cat_features, num_features


def prepare_pool(frame: pd.DataFrame, include_target: bool) -> Pool:
    cat_features, num_features = feature_columns()
    work = frame.copy()
    for col in cat_features:
        if col not in work.columns:
            work[col] = "unknown"
        work[col] = work[col].fillna("unknown").astype(str)
    for col in num_features:
        if col not in work.columns:
            work[col] = 0.0
        work[col] = pd.to_numeric(work[col], errors="coerce").replace([np.inf, -np.inf], np.nan).fillna(0.0)
    cols = cat_features + num_features
    if include_target:
        weights = pd.to_numeric(work.get("event_row_weight"), errors="coerce").replace([np.inf, -np.inf], np.nan).fillna(1.0)
        return Pool(work[cols], label=work["stage2_target"].astype(int), weight=weights, cat_features=cat_features)
    return Pool(work[cols], cat_features=cat_features)


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
    scored["stage2_score"] = model.predict_proba(prepare_pool(scored, include_target=False))[:, 1]
    return scored


def safe_auc(y: np.ndarray, score: np.ndarray) -> float | None:
    try:
        if len(set(int(v) for v in y)) < 2:
            return None
        return float(roc_auc_score(y, score))
    except ValueError:
        return None


def safe_ap(y: np.ndarray, score: np.ndarray) -> float | None:
    try:
        if len(set(int(v) for v in y)) < 2:
            return None
        return float(average_precision_score(y, score))
    except ValueError:
        return None


def row_metrics(frame: pd.DataFrame) -> dict[str, Any]:
    y = frame["stage2_target"].astype(int).to_numpy()
    score = pd.to_numeric(frame["stage2_score"], errors="coerce").fillna(0.0).to_numpy()
    return {
        "rows": int(len(frame)),
        "positive_rows": int(y.sum()),
        "auc": safe_auc(y, score),
        "average_precision": safe_ap(y, score),
    }


def event_predictions(frame: pd.DataFrame, threshold: float) -> pd.DataFrame:
    base_cols = [
        "candidate_uid",
        "valid_year",
        "root_symbol",
        "symbol",
        "signal_date",
        "direction",
        "is_oracle_start",
        "stage2_target",
        "oracle_start_distance_bars",
        "catboost_score",
        "xgboost_score",
        "lightgbm_score",
        "visual_score",
    ]
    available_base_cols = [col for col in base_cols if col in frame.columns]
    events = frame.sort_values(["candidate_uid", "confirm_offset_bars"]).drop_duplicates("candidate_uid")
    events = events[available_base_cols].copy()
    event_target = frame.groupby("candidate_uid")["stage2_target"].max().rename("event_stage2_target").reset_index()
    events = events.merge(event_target, on="candidate_uid", how="left")
    events["stage2_target"] = pd.to_numeric(events["event_stage2_target"], errors="coerce").fillna(0).astype(int)
    events = events.drop(columns=["event_stage2_target"])
    hits = frame[pd.to_numeric(frame["stage2_score"], errors="coerce").fillna(0.0) >= float(threshold)].copy()
    if hits.empty:
        events["confirmed"] = 0
        events["confirm_offset_bars"] = np.nan
        events["stage2_score"] = np.nan
        events["confirm_row_target"] = 0
        events["confirm_date"] = pd.NaT
        events["confirm_price"] = np.nan
        return events
    hits = hits.sort_values(["candidate_uid", "confirm_offset_bars"]).drop_duplicates("candidate_uid")
    hits["confirm_row_target"] = pd.to_numeric(hits["stage2_target"], errors="coerce").fillna(0).astype(int)
    hit_cols = ["candidate_uid", "confirm_offset_bars", "stage2_score", "confirm_row_target", "confirm_date", "confirm_price"]
    merged = events.merge(hits[hit_cols], on="candidate_uid", how="left")
    merged["confirmed"] = merged["stage2_score"].notna().astype(int)
    merged["confirm_row_target"] = pd.to_numeric(merged["confirm_row_target"], errors="coerce").fillna(0).astype(int)
    return merged


def event_metrics(frame: pd.DataFrame, source_summary: dict[str, Any], threshold: float) -> dict[str, Any]:
    events = event_predictions(frame, threshold)
    y = events["stage2_target"].astype(int).to_numpy()
    oracle_y = events["is_oracle_start"].astype(int).to_numpy()
    confirmed = events["confirmed"].astype(int).to_numpy() == 1
    confirm_row_target = events["confirm_row_target"].astype(int).to_numpy()
    positives = int(y.sum())
    picks = int(confirmed.sum())
    true_picks = int((confirmed & (confirm_row_target == 1)).sum())
    false_picks = int((confirmed & (confirm_row_target == 0)).sum())
    precision = true_picks / picks if picks else 0.0
    recall = true_picks / positives if positives else 0.0
    f1 = (2.0 * precision * recall / (precision + recall)) if precision + recall > 0 else 0.0
    offsets = pd.to_numeric(events.loc[events["confirmed"] == 1, "confirm_offset_bars"], errors="coerce").dropna()
    true_offsets = pd.to_numeric(
        events.loc[(events["confirmed"] == 1) & (events["confirm_row_target"] == 1), "confirm_offset_bars"],
        errors="coerce",
    ).dropna()
    source_positives = int(source_summary.get("source_positives", 0) or 0)
    stage1_oracle_positives = int(oracle_y.sum())
    true_oracle_confirms = int((confirmed & (oracle_y == 1)).sum())
    return {
        "events": int(len(events)),
        "stage1_positive_events": stage1_oracle_positives,
        "target_positive_events": positives,
        "confirmed_events": picks,
        "true_confirmed_events": true_picks,
        "false_confirmed_events": false_picks,
        "rejected_events": int(len(events) - picks),
        "rejected_true_events": int(((~confirmed) & (y == 1)).sum()),
        "precision": precision,
        "recall_within_stage1": recall,
        "f1_within_stage1": f1,
        "oracle_confirmed_events": true_oracle_confirms,
        "oracle_precision": float(true_oracle_confirms / picks) if picks else 0.0,
        "oracle_recall_within_stage1": float(true_oracle_confirms / stage1_oracle_positives) if stage1_oracle_positives else 0.0,
        "total_oracle_recall": float(true_oracle_confirms / source_positives) if source_positives else 0.0,
        "avg_confirm_offset": float(offsets.mean()) if len(offsets) else None,
        "median_confirm_offset": float(offsets.median()) if len(offsets) else None,
        "avg_true_confirm_offset": float(true_offsets.mean()) if len(true_offsets) else None,
        "median_true_confirm_offset": float(true_offsets.median()) if len(true_offsets) else None,
        "threshold": float(threshold),
    }


def choose_threshold(frame: pd.DataFrame, source_summary: dict[str, Any], args: argparse.Namespace) -> tuple[float, pd.DataFrame]:
    score = pd.to_numeric(frame["stage2_score"], errors="coerce").fillna(0.0).to_numpy()
    broad = np.linspace(0.05, 0.99, 90)
    tail = np.array([0.991, 0.992, 0.993, 0.994, 0.995, 0.996, 0.997, 0.998, 0.999, 0.9995, 0.9998, 0.9999])
    candidates = set(float(v) for v in np.quantile(score, np.concatenate([broad, tail])))
    finite_scores = np.unique(score[np.isfinite(score)])
    if len(finite_scores):
        candidates.add(float(finite_scores.min()))
        candidates.add(float(finite_scores.max()))
        tail_start = float(np.quantile(finite_scores, 0.98))
        tail_scores = finite_scores[finite_scores >= tail_start]
        if len(tail_scores) > int(args.threshold_tail_candidates):
            indices = np.linspace(0, len(tail_scores) - 1, int(args.threshold_tail_candidates)).astype(int)
            tail_scores = tail_scores[indices]
        candidates.update(float(v) for v in tail_scores)
    candidates = sorted(candidates)
    best_threshold = candidates[0]
    best_key: tuple[float, float, float, int] | None = None
    rows: list[dict[str, Any]] = []
    for threshold in candidates:
        item = event_metrics(frame, source_summary, threshold)
        allowed = item["precision"] >= float(args.min_threshold_precision) and item["confirmed_events"] >= int(args.min_threshold_picks)
        rows.append({**item, "allowed": allowed})
        if allowed:
            avg_offset = item["avg_true_confirm_offset"]
            offset_key = -float(avg_offset) if avg_offset is not None else -999.0
            key = (float(item["f1_within_stage1"]), float(item["precision"]), offset_key, int(item["confirmed_events"]))
            if best_key is None or key > best_key:
                best_key = key
                best_threshold = threshold
    if best_key is None:
        best_threshold = max(candidates, key=lambda threshold: event_metrics(frame, source_summary, threshold)["precision"])
    return best_threshold, pd.DataFrame(rows)


def main() -> int:
    args = parse_args()
    rid = run_id(args)
    output_dir = wave.ABCD_ROOT / "model_registry" / rid
    if output_dir.exists() and not args.replace_run:
        raise ValueError(f"Adaptive Stage 2 run already exists: {rid}. Use --replace-run.")
    output_dir.mkdir(parents=True, exist_ok=True)

    model_dir, meta = source_dir(str(args.four_model_run_id))
    thresholds = {name: float(value) for name, value in meta["thresholds"].items()}
    conn = wave.connect()
    try:
        train_base = manager_model.add_manager_features(load_split(model_dir, args.train_label, int(args.max_train_rows)), thresholds)
        threshold_base = manager_model.add_manager_features(load_split(model_dir, int(args.threshold_year), int(args.max_eval_rows)), thresholds)
        valid_base = manager_model.add_manager_features(load_split(model_dir, int(args.valid_year), int(args.max_eval_rows)), thresholds)
        train, train_source = add_adaptive_confirmation_rows(conn, train_base, thresholds, int(args.train_year), args, "train")
        threshold, threshold_source = add_adaptive_confirmation_rows(
            conn, threshold_base, thresholds, int(args.threshold_year), args, "threshold"
        )
        valid, valid_source = add_adaptive_confirmation_rows(conn, valid_base, thresholds, int(args.valid_year), args, "valid")
    finally:
        conn.close()

    print(
        f"Training adaptive Stage 2 rows train={len(train):,} "
        f"threshold={len(threshold):,} valid={len(valid):,}",
        flush=True,
    )
    model = train_model(train, args)
    train_scored = add_scores(train, model)
    threshold_scored = add_scores(threshold, model)
    valid_scored = add_scores(valid, model)
    selected_threshold, sweep = choose_threshold(threshold_scored, threshold_source, args)
    print(f"selected_adaptive_stage2_threshold={selected_threshold:.6f}", flush=True)

    results = {
        "train": {
            "source": train_source,
            "row": row_metrics(train_scored),
            "event": event_metrics(train_scored, train_source, selected_threshold),
        },
        str(args.threshold_year): {
            "source": threshold_source,
            "row": row_metrics(threshold_scored),
            "event": event_metrics(threshold_scored, threshold_source, selected_threshold),
        },
        str(args.valid_year): {
            "source": valid_source,
            "row": row_metrics(valid_scored),
            "event": event_metrics(valid_scored, valid_source, selected_threshold),
        },
    }
    for split, item in results.items():
        print(f"{split}={json.dumps(item, default=to_jsonable)}", flush=True)

    model.save_model(str(output_dir / "catboost_stage2_adaptive_confirmation.cbm"))
    if not args.skip_save_expanded_rows:
        train_scored.to_csv(output_dir / "stage2_adaptive_rows_train.csv", index=False)
        threshold_scored.to_csv(output_dir / f"stage2_adaptive_rows_{args.threshold_year}.csv", index=False)
        valid_scored.to_csv(output_dir / f"stage2_adaptive_rows_{args.valid_year}.csv", index=False)
    event_predictions(train_scored, selected_threshold).to_csv(output_dir / "stage2_adaptive_events_train.csv", index=False)
    event_predictions(threshold_scored, selected_threshold).to_csv(
        output_dir / f"stage2_adaptive_events_{args.threshold_year}.csv",
        index=False,
    )
    event_predictions(valid_scored, selected_threshold).to_csv(
        output_dir / f"stage2_adaptive_events_{args.valid_year}.csv",
        index=False,
    )
    sweep.to_csv(output_dir / f"stage2_adaptive_threshold_sweep_{args.threshold_year}.csv", index=False)
    cat_features, num_features = feature_columns()
    metadata = {
        "stage2_run_id": rid,
        "model_type": "catboost_oracle_start_stage2_adaptive_confirmation",
        "four_model_run_id": args.four_model_run_id,
        "stage1_rule": args.stage1_rule,
        "stage1_threshold": float(args.stage1_threshold),
        "max_confirm_bars": int(args.max_confirm_bars),
        "target_mode": args.target_mode,
        "target_parameters": {
            "min_confirm_close_atr": float(args.min_confirm_close_atr),
            "min_confirm_favorable_atr": float(args.min_confirm_favorable_atr),
            "max_confirm_adverse_atr": float(args.max_confirm_adverse_atr),
            "min_remaining_r": float(args.min_remaining_r),
            "min_remaining_bars": int(args.min_remaining_bars),
        },
        "selected_stage2_threshold": float(selected_threshold),
        "features": {"cat": cat_features, "num": num_features},
        "results": results,
        "read": (
            "Adaptive fakeout filter. After Stage 1 fires, score each candle offset up to max_confirm_bars "
            "and confirm on the first score above the learned threshold."
        ),
    }
    (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")
    print(f"Saved adaptive Stage 2 confirmation model: {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
