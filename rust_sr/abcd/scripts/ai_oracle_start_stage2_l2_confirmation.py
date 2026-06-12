#!/usr/bin/env python3
"""
Train a live-safe Stage 2 confirmation filter on top of the 2m Level 2 trio manager.

Stage 1 source:
    Level 2 CatBoost manager over Cat/Light/XGB Level 1 opinions.

Stage 2 behavior:
    For every Level 2 picked event, create confirmation rows at offset
    1..max_confirm_bars. In live use, the model can score each new candle as it
    arrives and confirm on the first offset above threshold.

This is signal research only. It does not create entries, exits, or orders.
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
import ai_oracle_start_level2_three_l1_manager as level2
import ai_oracle_start_stage2_adaptive_confirmation as adaptive_stage2
import ai_oracle_start_stage2_confirmation as fixed_stage2
import ai_oracle_trend_start_model as start_model
import ai_wave_rider_research as wave


DEFAULT_LEVEL2_RUN = "aicw-os-l2-three-l1-2m-p1p1n6m3-v1-ALL-2m-tr2025-v2026"
DEFAULT_TRAIN_CACHE = (
    "rust_sr/abcd/model_registry/_level2_l1_opinion_cache/"
    "l1_opinions_ALL_2m_2025_40697b59947a648b.pkl.gz"
)
DEFAULT_VALID_CACHE = (
    "rust_sr/abcd/model_registry/_level2_l1_opinion_cache/"
    "l1_opinions_ALL_2m_2026_2ed115f14e34985b.pkl.gz"
)

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
    parser.add_argument("--level2-run-id", default=DEFAULT_LEVEL2_RUN)
    parser.add_argument("--train-opinion-cache", default=DEFAULT_TRAIN_CACHE)
    parser.add_argument("--valid-opinion-cache", default=DEFAULT_VALID_CACHE)
    parser.add_argument("--run-prefix", default="aicw-os-stage2-l2-confirm-v1")
    parser.add_argument("--train-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--timeframe", default="2m")
    parser.add_argument("--roots", default="")
    parser.add_argument("--manager-engines", default="")
    parser.add_argument("--level2-threshold", type=float, default=None)
    parser.add_argument("--stage1-match-window-bars", type=int, default=None)
    parser.add_argument("--stage1-cooldown-bars", type=int, default=None)
    parser.add_argument("--max-confirm-bars", type=int, default=16)
    parser.add_argument("--oracle-run-id", default="")
    parser.add_argument("--oracle-detail-match-bars", type=int, default=4)
    parser.add_argument("--target-mode", choices=["event", "proof"], default="proof")
    parser.add_argument("--min-confirm-close-atr", type=float, default=0.05)
    parser.add_argument("--min-confirm-favorable-atr", type=float, default=0.15)
    parser.add_argument("--max-confirm-adverse-atr", type=float, default=1.50)
    parser.add_argument("--min-remaining-r", type=float, default=0.25)
    parser.add_argument("--min-remaining-bars", type=int, default=1)
    parser.add_argument("--threshold-split", type=float, default=0.30)
    parser.add_argument("--min-threshold-precision", type=float, default=0.55)
    parser.add_argument("--min-threshold-picks", type=int, default=300)
    parser.add_argument("--threshold-tail-candidates", type=int, default=450)
    parser.add_argument("--iterations", type=int, default=450)
    parser.add_argument("--depth", type=int, default=5)
    parser.add_argument("--learning-rate", type=float, default=0.04)
    parser.add_argument("--l2-leaf-reg", type=float, default=10.0)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--max-cache-rows", type=int, default=0)
    parser.add_argument("--max-events-per-year", type=int, default=0)
    parser.add_argument("--max-train-rows", type=int, default=0)
    parser.add_argument("--save-expanded-rows", action="store_true")
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


def to_jsonable(value: Any) -> Any:
    if isinstance(value, pd.Timestamp):
        return value.isoformat()
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def load_json(path: Path) -> dict[str, Any]:
    if not path.exists():
        raise FileNotFoundError(f"Missing JSON: {path}")
    return json.loads(path.read_text(encoding="utf-8"))


def run_id(args: argparse.Namespace) -> str:
    raw = (
        f"{args.run_prefix}-{args.timeframe}-"
        f"{Path(str(args.level2_run_id)).name[-14:]}-m{int(args.max_confirm_bars)}-v{int(args.valid_year)}"
    )
    if len(raw) <= 100:
        return raw
    digest = hashlib.sha1(raw.encode("utf-8")).hexdigest()[:12]
    return f"{args.run_prefix[:44]}-{args.timeframe}-m{int(args.max_confirm_bars)}-{digest}"


def unique(values: list[str]) -> list[str]:
    seen: set[str] = set()
    out: list[str] = []
    for value in values:
        if value not in seen:
            out.append(value)
            seen.add(value)
    return out


def level2_dir(run_id_value: str) -> Path:
    path = Path(run_id_value)
    if path.exists():
        return path.resolve()
    return wave.ABCD_ROOT / "model_registry" / run_id_value


def level2_model_path(model_dir: Path) -> Path:
    preferred = model_dir / "catboost_oracle_start_level2_three_l1_manager.cbm"
    if preferred.exists():
        return preferred
    matches = sorted(model_dir.glob("catboost_oracle_start_level2*_manager.cbm"))
    if not matches:
        raise FileNotFoundError(f"Missing Level 2 manager model in {model_dir}")
    return matches[0]


def load_level2(args: argparse.Namespace) -> tuple[Path, dict[str, Any], CatBoostClassifier, argparse.Namespace]:
    model_dir = level2_dir(str(args.level2_run_id))
    metadata = load_json(model_dir / "metadata.json")
    model = CatBoostClassifier()
    model.load_model(str(level2_model_path(model_dir)))
    engines = str(args.manager_engines or "").strip()
    if not engines:
        engines = ",".join(str(item) for item in metadata.get("manager_engines") or ["cat", "light", "xgb"])
    l2_args = argparse.Namespace(manager_engines=engines)
    return model_dir, metadata, model, l2_args


def cache_metadata_path(path: Path) -> Path:
    return path.with_suffix(path.suffix + ".metadata.json")


def balanced_cap(frame: pd.DataFrame, limit: int, seed: int) -> pd.DataFrame:
    if int(limit) <= 0 or len(frame) <= int(limit):
        return frame
    labels = pd.to_numeric(frame.get("is_oracle_start"), errors="coerce").fillna(0).astype(int)
    positives = frame[labels == 1]
    negatives = frame[labels == 0]
    keep_pos = positives.sample(n=min(len(positives), int(limit) // 2), random_state=seed)
    keep_neg = negatives.sample(n=min(len(negatives), int(limit) - len(keep_pos)), random_state=seed)
    return pd.concat([keep_pos, keep_neg], ignore_index=True).sample(frac=1.0, random_state=seed).reset_index(drop=True)


def load_opinion_cache(path_raw: str, max_rows: int, seed: int) -> tuple[pd.DataFrame, dict[str, Any]]:
    path = Path(path_raw).expanduser().resolve()
    frame = pd.read_pickle(path, compression="gzip")
    metadata = load_json(cache_metadata_path(path)) if cache_metadata_path(path).exists() else {}
    summary = dict(metadata.get("source_summary") or {})
    before = len(frame)
    frame = balanced_cap(frame, int(max_rows), int(seed))
    if len(frame) != before:
        positives = frame[pd.to_numeric(frame.get("nearest_abs_bars"), errors="coerce").fillna(999999) <= 3]
        represented = positives.get("nearest_oracle_trade_id", pd.Series(dtype=object)).dropna().astype(str)
        represented = represented[represented != ""].nunique()
        summary = {**summary, "oracle_starts": int(represented), "sampled_for_debug": True}
        print(f"debug-capped opinion cache {path.name}: {before:,} -> {len(frame):,}", flush=True)
    frame["signal_date"] = pd.to_datetime(frame["signal_date"], errors="coerce")
    frame["is_oracle_start"] = pd.to_numeric(frame["is_oracle_start"], errors="coerce").fillna(0).astype(int)
    return frame.dropna(subset=["candidate_uid", "symbol", "signal_date", "direction"]).reset_index(drop=True), summary


def selected_level2_threshold(args: argparse.Namespace, metadata: dict[str, Any]) -> float:
    if args.level2_threshold is not None:
        return float(args.level2_threshold)
    return float(metadata["selected_threshold"])


def stage1_event_rules(args: argparse.Namespace, metadata: dict[str, Any]) -> tuple[int, int]:
    rules = metadata.get("event_rules") or {}
    match_window = int(args.stage1_match_window_bars if args.stage1_match_window_bars is not None else rules.get("match_window_bars", 3))
    cooldown = int(args.stage1_cooldown_bars if args.stage1_cooldown_bars is not None else rules.get("cooldown_bars", 6))
    return match_window, cooldown


def score_level2(frame: pd.DataFrame, model: CatBoostClassifier, l2_args: argparse.Namespace) -> pd.DataFrame:
    scored = frame.copy()
    scored["level2_score"] = model.predict_proba(level2.prepare_pool(scored, include_target=False, args=l2_args))[:, 1]
    return scored


def select_level2_events(
    frame: pd.DataFrame,
    threshold: float,
    match_window_bars: int,
    cooldown_bars: int,
    max_events: int,
    seed: int,
) -> pd.DataFrame:
    work = frame.copy()
    work["score_num"] = pd.to_numeric(work["level2_score"], errors="coerce").fillna(0.0)
    work["signal_idx_num"] = pd.to_numeric(work["signal_idx"], errors="coerce").fillna(-1).astype(int)
    work = work[work["score_num"] >= float(threshold)].sort_values(
        ["symbol", "direction", "signal_idx_num", "score_num"],
        ascending=[True, True, True, False],
    )
    kept: list[pd.Series] = []
    for (_, _), group in work.groupby(["symbol", "direction"], sort=False):
        next_allowed = -1
        for _, row in group.iterrows():
            idx = int(row["signal_idx_num"])
            if idx < next_allowed:
                continue
            kept.append(row)
            next_allowed = idx + int(cooldown_bars) + 1
    if not kept:
        return pd.DataFrame(columns=frame.columns)
    events = pd.DataFrame(kept).reset_index(drop=True)
    nearest_abs = pd.to_numeric(events.get("nearest_abs_bars"), errors="coerce")
    events["candidate_label_is_oracle_start"] = pd.to_numeric(events["is_oracle_start"], errors="coerce").fillna(0).astype(int)
    events["is_oracle_start"] = (nearest_abs <= int(match_window_bars)).fillna(False).astype(int)
    events["stage1_pick"] = 1
    events["stage1_score"] = pd.to_numeric(events["level2_score"], errors="coerce").fillna(0.0)
    if int(max_events) > 0 and len(events) > int(max_events):
        labels = events["is_oracle_start"].astype(int)
        positives = events[labels == 1]
        negatives = events[labels == 0]
        keep_pos = positives.sample(n=min(len(positives), int(max_events) // 2), random_state=seed)
        keep_neg = negatives.sample(n=min(len(negatives), int(max_events) - len(keep_pos)), random_state=seed)
        events = pd.concat([keep_pos, keep_neg], ignore_index=True).sample(frac=1.0, random_state=seed).reset_index(drop=True)
    return events


def fetch_candles_for_symbols(conn, timeframe: str, year: int, symbols: list[str]) -> pd.DataFrame:
    if not symbols:
        return pd.DataFrame()
    table_name, _minutes = scanner.table_for_timeframe(timeframe)
    table = scanner.safe_identifier(table_name)
    start = pd.Timestamp(year=year, month=1, day=1) - pd.Timedelta(days=3)
    end = pd.Timestamp(year=year + 1, month=1, day=1) + pd.Timedelta(days=3)
    chunks: list[pd.DataFrame] = []
    symbols = sorted(set(str(symbol) for symbol in symbols if str(symbol).strip()))
    for start_idx in range(0, len(symbols), 80):
        chunk = symbols[start_idx : start_idx + 80]
        params: list[Any] = [start.to_pydatetime(), end.to_pydatetime(), *chunk]
        symbol_sql = ",".join(["%s"] * len(chunk))
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
                  AND symbol IN ({symbol_sql})
                ORDER BY root_symbol, symbol, ts_utc
                """,
                params,
            )
            rows = cur.fetchall()
        if rows:
            chunks.append(pd.DataFrame(rows))
    frame = pd.concat(chunks, ignore_index=True) if chunks else pd.DataFrame()
    if frame.empty:
        return frame
    frame["ts_utc"] = pd.to_datetime(frame["ts_utc"], errors="coerce")
    for col in ["open", "high", "low", "close", "volume"]:
        frame[col] = pd.to_numeric(frame[col], errors="coerce")
    return frame.dropna(subset=["root_symbol", "symbol", "ts_utc", "open", "high", "low", "close"]).reset_index(drop=True)


def build_confirmation_rows(
    conn,
    events: pd.DataFrame,
    source_summary: dict[str, Any],
    year: int,
    args: argparse.Namespace,
    label: str,
) -> tuple[pd.DataFrame, dict[str, Any]]:
    roots = start_model.parse_list(args.roots)
    oracle_exact, oracle_by_key = adaptive_stage2.fetch_oracle_details(conn, int(year), roots, str(args.oracle_run_id))
    candles = fetch_candles_for_symbols(conn, args.timeframe, int(year), events["symbol"].dropna().astype(str).unique().tolist())
    if candles.empty:
        raise ValueError(f"{label}: no candles available for selected Level 2 events")

    rows_by_symbol = {
        str(symbol): group.sort_values("signal_date").reset_index(drop=True)
        for symbol, group in events.groupby("symbol", sort=False)
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
        for original in targets.to_dict("records"):
            signal_ts = pd.Timestamp(original["signal_date"])
            idx = int(np.searchsorted(times, np.datetime64(signal_ts), side="left"))
            if idx >= len(group):
                continue
            tick_size = wave.tick_size_for(str(original["symbol"]), str(original.get("root_symbol") or ""))
            atr_ticks = wave.finite(original.get("atr_ticks"), None)
            atr_hint = atr_ticks * tick_size if atr_ticks is not None and tick_size > 0 else None
            oracle_info = adaptive_stage2.match_oracle_info(oracle_exact, oracle_by_key, original, signal_ts, args)
            event_rows = 0
            for offset in range(1, int(args.max_confirm_bars) + 1):
                features = adaptive_stage2.post_features_for_offset(
                    group,
                    idx,
                    str(original["direction"]),
                    offset,
                    int(args.max_confirm_bars),
                    atr_hint,
                )
                if features is None:
                    continue
                target, target_debug = adaptive_stage2.proof_target(original, features, oracle_info, args)
                payload = dict(original)
                payload.update(features)
                payload.update(target_debug)
                payload["stage2_target"] = int(target)
                output.append(payload)
                event_rows += 1
            if event_rows:
                seen_events += 1
                expanded_rows += event_rows
        if symbol_index % 50 == 0:
            print(
                f"{label}: confirmation symbols={symbol_index:,} "
                f"events={seen_events:,}/{len(events):,} rows={expanded_rows:,}",
                flush=True,
            )

    expanded = pd.DataFrame(output)
    if expanded.empty:
        raise ValueError(f"{label}: no Stage 2 confirmation rows were built.")
    counts = expanded.groupby("candidate_uid")["candidate_uid"].transform("count").astype(float)
    expanded["event_row_weight"] = 1.0 / counts.replace(0.0, 1.0)
    oracle_total = int(source_summary.get("oracle_starts") or 0)
    summary = {
        "source_rows": int(source_summary.get("row_count") or len(events)),
        "source_positives": oracle_total,
        "stage1_events": int(len(events)),
        "stage1_positive_events": int(events["is_oracle_start"].astype(int).sum()),
        "stage1_precision": float(events["is_oracle_start"].astype(int).mean()) if len(events) else 0.0,
        "stage1_total_oracle_recall": float(events["is_oracle_start"].astype(int).sum() / oracle_total) if oracle_total else 0.0,
        "expanded_events": int(expanded["candidate_uid"].nunique()),
        "expanded_rows": int(len(expanded)),
        "target_positive_rows": int(expanded["stage2_target"].sum()),
        "target_positive_events": int(expanded.loc[expanded["stage2_target"] == 1, "candidate_uid"].nunique()),
        "target_reason_counts": {
            str(key): int(value) for key, value in expanded["stage2_target_reason"].value_counts(dropna=False).items()
        },
    }
    print(f"{label}: built Stage 2 rows {json.dumps(summary, default=to_jsonable)}", flush=True)
    return expanded, summary


def feature_columns() -> tuple[list[str], list[str]]:
    cat_features = unique(list(start_model.CAT_FEATURES) + ["source_timeframe", "top_l1_block"])
    l2_cat, l2_num = level2.feature_columns(argparse.Namespace(manager_engines="cat,light,xgb"))
    num_features = unique(
        list(start_model.NUM_FEATURES)
        + list(l2_num)
        + ["stage1_score", "level2_score"]
        + list(fixed_stage2.POST_FEATURES)
        + ADAPTIVE_NUM_FEATURES
    )
    return unique(cat_features + [col for col in l2_cat if col not in cat_features]), num_features


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


def cap_train_rows(frame: pd.DataFrame, limit: int, seed: int) -> pd.DataFrame:
    if int(limit) <= 0 or len(frame) <= int(limit):
        return frame.sample(frac=1.0, random_state=seed).reset_index(drop=True)
    labels = frame["stage2_target"].astype(int)
    positives = frame[labels == 1]
    negatives = frame[labels == 0]
    keep_pos = positives.sample(n=min(len(positives), int(limit) // 2), random_state=seed)
    keep_neg = negatives.sample(n=min(len(negatives), int(limit) - len(keep_pos)), random_state=seed)
    return pd.concat([keep_pos, keep_neg], ignore_index=True).sample(frac=1.0, random_state=seed).reset_index(drop=True)


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


def add_stage2_scores(frame: pd.DataFrame, model: CatBoostClassifier) -> pd.DataFrame:
    scored = frame.copy()
    scored["stage2_score"] = model.predict_proba(prepare_pool(scored, include_target=False))[:, 1]
    return scored


def event_predictions(frame: pd.DataFrame, threshold: float) -> pd.DataFrame:
    base_cols = [
        "candidate_uid",
        "valid_year",
        "root_symbol",
        "symbol",
        "signal_date",
        "direction",
        "is_oracle_start",
        "stage1_score",
        "level2_score",
        "nearest_oracle_trade_id",
        "nearest_delta_bars",
        "nearest_abs_bars",
        "cat_l1_score",
        "light_l1_score",
        "xgb_l1_score",
        "top_l1_block",
    ]
    available = [col for col in base_cols if col in frame.columns]
    events = frame.sort_values(["candidate_uid", "confirm_offset_bars"]).drop_duplicates("candidate_uid")
    events = events[available].copy()
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
    source_positives = int(source_summary.get("source_positives", source_summary.get("oracle_starts", 0)) or 0)
    true_oracle_confirms = int((confirmed & (oracle_y == 1)).sum())
    offsets = pd.to_numeric(events.loc[events["confirmed"] == 1, "confirm_offset_bars"], errors="coerce").dropna()
    true_offsets = pd.to_numeric(
        events.loc[(events["confirmed"] == 1) & (events["confirm_row_target"] == 1), "confirm_offset_bars"],
        errors="coerce",
    ).dropna()
    return {
        "events": int(len(events)),
        "stage1_positive_events": int(oracle_y.sum()),
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
    return float(best_threshold), pd.DataFrame(rows)


def split_train_threshold(rows: pd.DataFrame, split: float, seed: int) -> tuple[pd.DataFrame, pd.DataFrame]:
    event_ids = rows["candidate_uid"].dropna().astype(str).drop_duplicates()
    threshold_ids = set(event_ids.sample(frac=max(0.05, min(0.90, float(split))), random_state=seed).tolist())
    mask = rows["candidate_uid"].astype(str).isin(threshold_ids)
    return rows[~mask].reset_index(drop=True), rows[mask].reset_index(drop=True)


def split_source_summary(rows: pd.DataFrame) -> dict[str, Any]:
    events = rows.sort_values(["candidate_uid", "confirm_offset_bars"]).drop_duplicates("candidate_uid")
    return {
        "source_positives": int(events["is_oracle_start"].astype(int).sum()),
        "stage1_events": int(len(events)),
        "stage1_positive_events": int(events["is_oracle_start"].astype(int).sum()),
    }


def main() -> int:
    args = parse_args()
    model_dir, l2_meta, l2_model, l2_args = load_level2(args)
    if not args.oracle_run_id:
        args.oracle_run_id = str(l2_meta.get("oracle_run_id") or "oracle-perfect-trends-tfspec-v1-2m-2024_2026")
    if not args.roots:
        roots = l2_meta.get("roots") or []
        args.roots = ",".join(str(root) for root in roots)
    if not args.timeframe:
        args.timeframe = str(l2_meta.get("timeframe") or "2m")

    rid = run_id(args)
    output_dir = wave.ABCD_ROOT / "model_registry" / rid
    if output_dir.exists() and not args.replace_run:
        raise ValueError(f"Stage 2 L2 confirmation run exists: {rid}. Use --replace-run.")
    output_dir.mkdir(parents=True, exist_ok=True)

    l2_threshold = selected_level2_threshold(args, l2_meta)
    match_window, cooldown = stage1_event_rules(args, l2_meta)
    print(
        f"Level 2 source={model_dir.name} threshold={l2_threshold:.6f} "
        f"match_window={match_window} cooldown={cooldown}",
        flush=True,
    )

    train_cache, train_cache_summary = load_opinion_cache(args.train_opinion_cache, int(args.max_cache_rows), int(args.random_seed))
    valid_cache, valid_cache_summary = load_opinion_cache(args.valid_opinion_cache, int(args.max_cache_rows), int(args.random_seed) + 1)
    train_scored_l2 = score_level2(train_cache, l2_model, l2_args)
    valid_scored_l2 = score_level2(valid_cache, l2_model, l2_args)
    train_events = select_level2_events(
        train_scored_l2,
        l2_threshold,
        match_window,
        cooldown,
        int(args.max_events_per_year),
        int(args.random_seed),
    )
    valid_events = select_level2_events(
        valid_scored_l2,
        l2_threshold,
        match_window,
        cooldown,
        int(args.max_events_per_year),
        int(args.random_seed) + 1,
    )
    print(
        f"Level 2 picked train={len(train_events):,} "
        f"valid={len(valid_events):,} valid_oracle={int(valid_events['is_oracle_start'].sum()) if len(valid_events) else 0:,}",
        flush=True,
    )
    if train_events.empty or valid_events.empty:
        raise ValueError("Level 2 produced empty train or valid event set.")

    conn = wave.connect()
    try:
        train_rows_all, train_source_all = build_confirmation_rows(
            conn, train_events, train_cache_summary, int(args.train_year), args, "train"
        )
        valid_rows, valid_source = build_confirmation_rows(
            conn, valid_events, valid_cache_summary, int(args.valid_year), args, "valid"
        )
    finally:
        conn.close()

    train_rows, threshold_rows = split_train_threshold(train_rows_all, float(args.threshold_split), int(args.random_seed))
    train_source = split_source_summary(train_rows)
    threshold_source = split_source_summary(threshold_rows)

    train_sample = cap_train_rows(train_rows, int(args.max_train_rows), int(args.random_seed))
    print(
        f"Training Stage 2 rows train={len(train_sample):,}/{len(train_rows):,} "
        f"threshold={len(threshold_rows):,} valid={len(valid_rows):,}",
        flush=True,
    )
    model = train_model(train_sample, args)
    train_scored = add_stage2_scores(train_rows, model)
    threshold_scored = add_stage2_scores(threshold_rows, model)
    valid_scored = add_stage2_scores(valid_rows, model)
    selected_threshold, sweep = choose_threshold(threshold_scored, threshold_source, args)
    print(f"selected_stage2_l2_threshold={selected_threshold:.6f}", flush=True)

    results = {
        "train": {
            "source": train_source,
            "row": row_metrics(train_scored),
            "event": event_metrics(train_scored, train_source, selected_threshold),
        },
        "threshold": {
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

    model.save_model(str(output_dir / "catboost_stage2_l2_confirmation.cbm"))
    if args.save_expanded_rows:
        train_scored.to_csv(output_dir / "stage2_l2_rows_train.csv", index=False)
        threshold_scored.to_csv(output_dir / "stage2_l2_rows_threshold.csv", index=False)
        valid_scored.to_csv(output_dir / f"stage2_l2_rows_{args.valid_year}.csv", index=False)
    event_predictions(train_scored, selected_threshold).to_csv(output_dir / "stage2_l2_events_train.csv", index=False)
    event_predictions(threshold_scored, selected_threshold).to_csv(output_dir / "stage2_l2_events_threshold.csv", index=False)
    event_predictions(valid_scored, selected_threshold).to_csv(output_dir / f"stage2_l2_events_{args.valid_year}.csv", index=False)
    sweep.to_csv(output_dir / "stage2_l2_threshold_sweep.csv", index=False)

    cat_features, num_features = feature_columns()
    metadata = {
        "stage2_run_id": rid,
        "model_type": "catboost_oracle_start_stage2_l2_confirmation",
        "level2_run_id": str(args.level2_run_id),
        "level2_threshold": float(l2_threshold),
        "stage1_match_window_bars": int(match_window),
        "stage1_cooldown_bars": int(cooldown),
        "train_year": int(args.train_year),
        "valid_year": int(args.valid_year),
        "timeframe": args.timeframe,
        "roots": start_model.parse_list(args.roots),
        "oracle_run_id": args.oracle_run_id,
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
            "Stage 2 confirmation layer fed by Level 2 trio manager picks. "
            "It scores offsets after a Level 2 pick and confirms on the first score above threshold."
        ),
    }
    (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")
    print(f"Saved Stage 2 L2 confirmation model: {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
