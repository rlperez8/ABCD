#!/usr/bin/env python3
"""
Evaluate an oracle trend-start detector on the full candle grid.

The training script uses sampled positives/negatives so it can train quickly.
This evaluator answers the live-style question:

    "If the model watched every eligible candle, where would it fire, and how
     close would those fired events be to the hindsight oracle trend starts?"

It does not create trades or exits.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

import numpy as np
import pandas as pd

try:
    from catboost import CatBoostClassifier
except ImportError as exc:  # pragma: no cover
    raise SystemExit("CatBoost is required. Run with .venv_ai\\Scripts\\python.exe") from exc


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_scanner_model as scanner
import ai_oracle_trend_start_model as start_model
import ai_wave_rider_research as wave


DEFAULT_MODEL_RUN_ID = "aicw-oracle-start-v1-2m-tr2024-v2026"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model-run-id", default=DEFAULT_MODEL_RUN_ID)
    parser.add_argument("--oracle-run-id", default="")
    parser.add_argument("--timeframe", default="")
    parser.add_argument("--year", type=int, default=2026)
    parser.add_argument("--roots", default="")
    parser.add_argument("--thresholds", default="", help="Comma-separated thresholds. Defaults to model metadata threshold.")
    parser.add_argument("--cooldown-bars", type=int, default=8)
    parser.add_argument("--match-window-bars", type=int, default=4)
    parser.add_argument("--batch-size", type=int, default=50000)
    parser.add_argument("--limit-symbols", type=int, default=0)
    parser.add_argument("--save-picks-limit", type=int, default=0, help="0 saves all picked events.")
    parser.add_argument("--output-prefix", default="full_grid_oracle_start_eval")
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    return parser.parse_args()


def parse_thresholds(raw: str, default_threshold: float) -> list[float]:
    if not raw:
        return [float(default_threshold)]
    thresholds = [float(part.strip()) for part in raw.split(",") if part.strip()]
    if not thresholds:
        raise ValueError("No thresholds parsed")
    return sorted(set(thresholds))


def load_metadata(model_dir: Path) -> dict[str, Any]:
    metadata_path = model_dir / "metadata.json"
    if not metadata_path.exists():
        raise FileNotFoundError(f"Missing metadata: {metadata_path}")
    return json.loads(metadata_path.read_text(encoding="utf-8"))


def load_model(model_dir: Path) -> CatBoostClassifier:
    model_path = model_dir / "catboost_oracle_start_model.cbm"
    if not model_path.exists():
        model_path = model_dir / "catboost_oracle_start_live_grid_model.cbm"
    if not model_path.exists():
        raise FileNotFoundError(f"Missing oracle-start model in: {model_dir}")
    model = CatBoostClassifier()
    model.load_model(str(model_path))
    return model


def selected_threshold(metadata: dict[str, Any]) -> float:
    if "selected_threshold" in metadata:
        return float(metadata["selected_threshold"])
    policy = metadata.get("threshold_policy") or {}
    if "selected_threshold" in policy:
        return float(policy["selected_threshold"])
    valid_selected = metadata.get("valid_selected") or {}
    if "threshold" in valid_selected:
        return float(valid_selected["threshold"])
    raise KeyError("metadata is missing selected threshold")


def fetch_symbols(conn, timeframe: str, year: int, roots: list[str]) -> list[dict[str, str]]:
    table_name, _ = scanner.table_for_timeframe(timeframe)
    table = scanner.safe_identifier(table_name)
    start = pd.Timestamp(year=year, month=1, day=1)
    end = pd.Timestamp(year=year + 1, month=1, day=1)
    params: list[Any] = [start.to_pydatetime(), end.to_pydatetime()]
    root_sql = ""
    if roots:
        root_sql = "AND root_symbol IN (" + ",".join(["%s"] * len(roots)) + ")"
        params.extend(roots)
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT root_symbol, symbol
            FROM {table}
            WHERE ts_utc >= %s
              AND ts_utc < %s
              {root_sql}
            GROUP BY root_symbol, symbol
            ORDER BY root_symbol, symbol
            """,
            params,
        )
        rows = cur.fetchall()
    return [{"root_symbol": str(row["root_symbol"]), "symbol": str(row["symbol"])} for row in rows]


def fetch_symbol_candles(conn, timeframe: str, year: int, root: str, symbol: str) -> pd.DataFrame:
    table_name, _ = scanner.table_for_timeframe(timeframe)
    table = scanner.safe_identifier(table_name)
    start = pd.Timestamp(year=year, month=1, day=1) - pd.Timedelta(days=3)
    end = pd.Timestamp(year=year + 1, month=1, day=1) + pd.Timedelta(days=3)
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
            WHERE root_symbol = %s
              AND symbol = %s
              AND ts_utc >= %s
              AND ts_utc < %s
            ORDER BY ts_utc
            """,
            (root, symbol, start.to_pydatetime(), end.to_pydatetime()),
        )
        rows = cur.fetchall()
    frame = pd.DataFrame(rows)
    if frame.empty:
        return frame
    frame["ts_utc"] = pd.to_datetime(frame["ts_utc"], errors="coerce")
    for col in ["open", "high", "low", "close", "volume"]:
        frame[col] = pd.to_numeric(frame[col], errors="coerce")
    return frame.dropna(subset=["root_symbol", "symbol", "ts_utc", "open", "high", "low", "close"]).reset_index(drop=True)


def nearest_oracle(pick_idx: int, starts: list[dict[str, Any]]) -> dict[str, Any]:
    if not starts:
        return {
            "nearest_oracle_trade_id": "",
            "nearest_oracle_idx": None,
            "nearest_oracle_date": None,
            "nearest_delta_bars": None,
            "nearest_abs_bars": None,
        }
    indices = [int(item["idx"]) for item in starts]
    pos = int(np.searchsorted(np.array(indices, dtype=int), int(pick_idx), side="left"))
    candidates = []
    if pos < len(starts):
        candidates.append(starts[pos])
    if pos > 0:
        candidates.append(starts[pos - 1])
    nearest = min(candidates, key=lambda item: abs(int(pick_idx) - int(item["idx"])))
    delta = int(pick_idx) - int(nearest["idx"])
    return {
        "nearest_oracle_trade_id": nearest["oracle_trade_id"],
        "nearest_oracle_idx": int(nearest["idx"]),
        "nearest_oracle_date": nearest["entry_date"],
        "nearest_delta_bars": delta,
        "nearest_abs_bars": abs(delta),
    }


def oracle_starts_for_symbol(starts: pd.DataFrame, group: pd.DataFrame, symbol: str, direction: str) -> list[dict[str, Any]]:
    if starts.empty:
        return []
    subset = starts[(starts["symbol"].astype(str) == str(symbol)) & (starts["direction"].astype(str) == direction)]
    if subset.empty:
        return []
    times = pd.to_datetime(group["ts_utc"], errors="coerce").to_numpy(dtype="datetime64[ns]")
    mapped: list[dict[str, Any]] = []
    for _, row in subset.sort_values("entry_date").iterrows():
        entry_date = pd.Timestamp(row["entry_date"])
        idx = int(np.searchsorted(times, np.datetime64(entry_date), side="left"))
        if idx >= len(group):
            continue
        mapped.append(
            {
                "idx": idx,
                "oracle_trade_id": str(row.get("oracle_trade_id") or ""),
                "entry_date": entry_date,
            }
        )
    return mapped


def score_indices(
    model: CatBoostClassifier,
    year: int,
    root: str,
    symbol: str,
    group: pd.DataFrame,
    direction: str,
    indices: np.ndarray,
    batch_size: int,
) -> tuple[np.ndarray, np.ndarray]:
    scored_indices: list[np.ndarray] = []
    scored_values: list[np.ndarray] = []
    step = max(1000, int(batch_size))
    for start in range(0, len(indices), step):
        batch_indices = indices[start : start + step]
        rows = [
            start_model.feature_row(year, root, str(symbol), group, int(idx), direction)
            for idx in batch_indices
        ]
        frame = pd.DataFrame(rows)
        scores = model.predict_proba(start_model.prepare_pool(frame, include_target=False))[:, 1]
        scored_indices.append(batch_indices.astype(int))
        scored_values.append(np.asarray(scores, dtype=float))
    if not scored_indices:
        return np.array([], dtype=int), np.array([], dtype=float)
    return np.concatenate(scored_indices), np.concatenate(scored_values)


def event_picks_for_threshold(
    threshold: float,
    cooldown_bars: int,
    year: int,
    root: str,
    symbol: str,
    group: pd.DataFrame,
    direction: str,
    indices: np.ndarray,
    scores: np.ndarray,
    oracle_starts: list[dict[str, Any]],
    match_window_bars: int,
) -> list[dict[str, Any]]:
    picks: list[dict[str, Any]] = []
    next_allowed_idx = -1
    for idx, score in zip(indices, scores):
        idx = int(idx)
        score = float(score)
        if score < threshold or idx < next_allowed_idx:
            continue
        nearest = nearest_oracle(idx, oracle_starts)
        matched = nearest["nearest_abs_bars"] is not None and int(nearest["nearest_abs_bars"]) <= int(match_window_bars)
        row = group.iloc[idx]
        picks.append(
            {
                "threshold": threshold,
                "valid_year": year,
                "root_symbol": root,
                "symbol": str(symbol),
                "direction": direction,
                "signal_idx": idx,
                "signal_date": pd.Timestamp(row["ts_utc"]),
                "score": score,
                "matched_oracle": bool(matched),
                **nearest,
            }
        )
        next_allowed_idx = idx + int(cooldown_bars) + 1
    return picks


def summarize(
    threshold: float,
    picks: list[dict[str, Any]],
    oracle_total: int,
    match_window_bars: int,
    total_candidates: int,
) -> dict[str, Any]:
    picked = pd.DataFrame(picks)
    if picked.empty:
        return {
            "threshold": threshold,
            "total_candidates": total_candidates,
            "oracle_starts": oracle_total,
            "picked_events": 0,
            "matched_picks": 0,
            "pick_match_rate": 0.0,
            "matched_oracle_starts": 0,
            "oracle_recall": 0.0,
            "median_abs_bars": None,
            "mean_abs_bars": None,
            "median_delta_bars": None,
            "mean_delta_bars": None,
            "early_picks": 0,
            "exact_picks": 0,
            "late_picks": 0,
            "match_window_bars": match_window_bars,
        }
    matched = picked[picked["matched_oracle"].astype(bool)].copy()
    deltas = pd.to_numeric(matched["nearest_delta_bars"], errors="coerce").dropna()
    abs_deltas = deltas.abs()
    unique_oracles = matched["nearest_oracle_trade_id"].dropna().astype(str)
    unique_oracles = unique_oracles[unique_oracles != ""].nunique()
    return {
        "threshold": threshold,
        "total_candidates": int(total_candidates),
        "oracle_starts": int(oracle_total),
        "picked_events": int(len(picked)),
        "matched_picks": int(len(matched)),
        "pick_match_rate": float(len(matched) / len(picked)) if len(picked) else 0.0,
        "matched_oracle_starts": int(unique_oracles),
        "oracle_recall": float(unique_oracles / oracle_total) if oracle_total else 0.0,
        "median_abs_bars": float(abs_deltas.median()) if len(abs_deltas) else None,
        "mean_abs_bars": float(abs_deltas.mean()) if len(abs_deltas) else None,
        "median_delta_bars": float(deltas.median()) if len(deltas) else None,
        "mean_delta_bars": float(deltas.mean()) if len(deltas) else None,
        "early_picks": int((deltas < 0).sum()) if len(deltas) else 0,
        "exact_picks": int((deltas == 0).sum()) if len(deltas) else 0,
        "late_picks": int((deltas > 0).sum()) if len(deltas) else 0,
        "match_window_bars": int(match_window_bars),
    }


def to_jsonable(value: Any) -> Any:
    if isinstance(value, pd.Timestamp):
        return value.isoformat()
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, float) and (np.isnan(value) or np.isinf(value)):
        return None
    return value


def main() -> int:
    args = parse_args()
    model_dir = wave.ABCD_ROOT / "model_registry" / args.model_run_id
    metadata = load_metadata(model_dir)
    oracle_run_id = args.oracle_run_id or metadata.get("oracle_run_id") or start_model.DEFAULT_ORACLE_RUN
    timeframe = args.timeframe or metadata.get("timeframe") or "2m"
    thresholds = parse_thresholds(args.thresholds, selected_threshold(metadata))
    model = load_model(model_dir)

    roots = start_model.parse_list(args.roots)
    if not roots:
        roots = [str(root).upper() for root in (metadata.get("roots") or []) if str(root).strip()]
    conn = wave.connect()
    try:
        symbol_rows = fetch_symbols(conn, timeframe, int(args.year), roots)
        oracle = start_model.fetch_oracle_starts(conn, str(oracle_run_id), int(args.year), roots)
    except Exception:
        conn.close()
        raise
    if not symbol_rows:
        conn.close()
        raise ValueError(f"No symbols found for {args.year}")
    if oracle.empty:
        conn.close()
        raise ValueError(f"No oracle starts found for {args.year}")

    year_start = pd.Timestamp(year=int(args.year), month=1, day=1)
    year_end = pd.Timestamp(year=int(args.year) + 1, month=1, day=1)
    min_idx = 60
    total_symbols = len(symbol_rows)
    total_candidates = 0
    all_picks: dict[float, list[dict[str, Any]]] = {threshold: [] for threshold in thresholds}

    try:
        for symbol_index, item in enumerate(symbol_rows, start=1):
            if args.limit_symbols and symbol_index > int(args.limit_symbols):
                break
            root = item["root_symbol"]
            symbol = item["symbol"]
            raw_group = fetch_symbol_candles(conn, timeframe, int(args.year), root, symbol)
            if raw_group.empty:
                continue
            group = scanner.enrich_candles(raw_group[["ts_utc", "open", "high", "low", "close", "volume"]].copy(), args)
            times = pd.to_datetime(group["ts_utc"], errors="coerce")
            valid_indices = np.where((times >= year_start) & (times < year_end))[0]
            valid_indices = valid_indices[(valid_indices >= min_idx) & (valid_indices < len(group) - 1)].astype(int)
            if len(valid_indices) == 0:
                continue
            for direction in ["LONG", "SHORT"]:
                total_candidates += int(len(valid_indices))
                indices, scores = score_indices(
                    model=model,
                    year=int(args.year),
                    root=root,
                    symbol=str(symbol),
                    group=group,
                    direction=direction,
                    indices=valid_indices,
                    batch_size=int(args.batch_size),
                )
                starts = oracle_starts_for_symbol(oracle, group, str(symbol), direction)
                for threshold in thresholds:
                    all_picks[threshold].extend(
                        event_picks_for_threshold(
                            threshold=threshold,
                            cooldown_bars=int(args.cooldown_bars),
                            year=int(args.year),
                            root=root,
                            symbol=str(symbol),
                            group=group,
                            direction=direction,
                            indices=indices,
                            scores=scores,
                            oracle_starts=starts,
                            match_window_bars=int(args.match_window_bars),
                        )
                    )
            if symbol_index % 25 == 0:
                print(f"{args.year}: scored {symbol_index:,}/{total_symbols:,} symbols candidates={total_candidates:,}", flush=True)
    finally:
        conn.close()

    oracle_total = int(len(oracle))
    summaries = [
        summarize(
            threshold=threshold,
            picks=all_picks[threshold],
            oracle_total=oracle_total,
            match_window_bars=int(args.match_window_bars),
            total_candidates=total_candidates,
        )
        for threshold in thresholds
    ]

    output_base = model_dir / f"{args.output_prefix}_{args.year}"
    summary_path = output_base.with_suffix(".summary.json")
    picks_path = output_base.with_suffix(".picks.csv")
    summary_payload = {
        "model_run_id": args.model_run_id,
        "oracle_run_id": oracle_run_id,
        "timeframe": timeframe,
        "year": int(args.year),
        "roots": roots,
        "cooldown_bars": int(args.cooldown_bars),
        "thresholds": thresholds,
        "summaries": summaries,
    }
    summary_path.write_text(json.dumps(summary_payload, indent=2, default=to_jsonable), encoding="utf-8")

    pick_rows = [row for threshold in thresholds for row in all_picks[threshold]]
    if args.save_picks_limit and len(pick_rows) > int(args.save_picks_limit):
        pick_rows = pick_rows[: int(args.save_picks_limit)]
    pd.DataFrame(pick_rows).to_csv(picks_path, index=False)

    print(json.dumps(summary_payload, indent=2, default=to_jsonable))
    print(f"Saved summary: {summary_path}")
    print(f"Saved picks: {picks_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
