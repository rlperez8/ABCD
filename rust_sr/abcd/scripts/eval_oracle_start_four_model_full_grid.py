#!/usr/bin/env python3
"""
Live-style full-grid evaluator for oracle trend-start model agreement.

This streams through the candle grid and scores:

* CatBoost numeric oracle-start model
* XGBoost tabular oracle-start model
* LightGBM tabular oracle-start model
* Visual CNN, scored only after a configurable tabular gate

It does not create trades or exits. It measures signal events against the
hindsight oracle trend starts.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path
from typing import Any

import joblib
import numpy as np
import pandas as pd
import torch

try:
    from catboost import CatBoostClassifier
except ImportError as exc:  # pragma: no cover
    raise SystemExit("CatBoost is required. Run with .venv_ai\\Scripts\\python.exe") from exc


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_scanner_model as scanner
import ai_candle_wave_visual_entry_model as visual_render
import ai_oracle_start_four_model_agreement as four_model
import ai_oracle_start_manager_model as manager_model
import ai_oracle_trend_start_model as start_model
import ai_oracle_trend_start_visual_cnn as visual_cnn
import ai_wave_rider_research as wave
import eval_oracle_trend_start_full_grid as catboost_grid


DEFAULT_FOUR_MODEL_RUN = "aicw-oracle-start-fourmodel-v1-v2026"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--four-model-run-id", default=DEFAULT_FOUR_MODEL_RUN)
    parser.add_argument("--year", type=int, default=2026)
    parser.add_argument("--roots", default="")
    parser.add_argument("--timeframe", default="")
    parser.add_argument("--oracle-run-id", default="")
    parser.add_argument("--image-gate", choices=["tabular_all", "tabular_majority", "catboost"], default="tabular_all")
    parser.add_argument("--cooldown-bars", type=int, default=8)
    parser.add_argument("--match-window-bars", type=int, default=4)
    parser.add_argument("--tabular-batch-size", type=int, default=50000)
    parser.add_argument("--image-batch-size", type=int, default=512)
    parser.add_argument("--limit-symbols", type=int, default=0)
    parser.add_argument("--save-picks-limit", type=int, default=250000)
    parser.add_argument("--output-prefix", default="full_grid_four_model_oracle_start")
    parser.add_argument("--catboost-threshold", type=float, default=-1.0)
    parser.add_argument("--xgboost-threshold", type=float, default=-1.0)
    parser.add_argument("--lightgbm-threshold", type=float, default=-1.0)
    parser.add_argument("--visual-threshold", type=float, default=-1.0)
    parser.add_argument("--manager-run-id", default="")
    parser.add_argument("--manager-threshold", type=float, default=-1.0)

    # Candle feature defaults used by scanner enrichment.
    parser.add_argument("--atr-period", type=int, default=14)
    parser.add_argument("--entry-breakout-bars", type=int, default=8)
    parser.add_argument("--trail-lookback-bars", type=int, default=18)
    parser.add_argument("--atr-stop-pad", type=float, default=0.35)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--max-risk-ticks", type=float, default=240.0)
    parser.add_argument("--min-relative-volume", type=float, default=0.45)
    return parser.parse_args()


def load_json(path: Path) -> dict[str, Any]:
    if not path.exists():
        raise FileNotFoundError(f"Missing JSON: {path}")
    return json.loads(path.read_text(encoding="utf-8"))


def load_models(four_model_run_id: str) -> dict[str, Any]:
    four_dir = wave.ABCD_ROOT / "model_registry" / four_model_run_id
    four_meta = load_json(four_dir / "metadata.json")
    visual_run_id = str(four_meta["visual_run_id"])
    visual_dir = wave.ABCD_ROOT / "model_registry" / visual_run_id
    visual_meta = load_json(visual_dir / "metadata.json")
    catboost_run_id = str(visual_meta["catboost_run_id"])
    catboost_dir = wave.ABCD_ROOT / "model_registry" / catboost_run_id

    catboost_model = CatBoostClassifier()
    catboost_model.load_model(str(catboost_dir / "catboost_oracle_start_model.cbm"))
    preprocessor = joblib.load(four_dir / "tabular_preprocessor.joblib")
    xgb = joblib.load(four_dir / "xgboost_oracle_start.joblib")
    lgbm = joblib.load(four_dir / "lightgbm_oracle_start.joblib")

    visual_model = visual_cnn.OracleStartCnn(float(visual_meta.get("dropout", 0.0)))
    state = torch.load(visual_dir / "visual_oracle_start_cnn.pt", map_location="cpu")
    visual_model.load_state_dict(state)
    visual_model.eval()

    return {
        "four_dir": four_dir,
        "four_meta": four_meta,
        "visual_dir": visual_dir,
        "visual_meta": visual_meta,
        "catboost_model": catboost_model,
        "preprocessor": preprocessor,
        "xgb": xgb,
        "lgbm": lgbm,
        "visual_model": visual_model,
    }


def load_manager(manager_run_id: str) -> tuple[CatBoostClassifier | None, float | None, dict[str, Any] | None]:
    if not manager_run_id:
        return None, None, None
    model_dir = wave.ABCD_ROOT / "model_registry" / manager_run_id
    metadata = load_json(model_dir / "metadata.json")
    model = CatBoostClassifier()
    model.load_model(str(model_dir / "catboost_oracle_start_manager.cbm"))
    return model, float(metadata["selected_manager_threshold"]), metadata


def visual_args_from_meta(meta: dict[str, Any]) -> argparse.Namespace:
    return argparse.Namespace(
        height=int(meta.get("height", 48)),
        width=int(meta.get("width", 48)),
        lookback_bars=int(meta.get("lookback_bars", 64)),
        min_candles=int(meta.get("min_candles", 40)),
        batch_size=512,
    )


def score_tabular_batch(
    rows: list[dict[str, Any]],
    models: dict[str, Any],
) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    frame = pd.DataFrame(rows)
    cat_score = models["catboost_model"].predict_proba(start_model.prepare_pool(frame, include_target=False))[:, 1]
    clean = four_model.clean_features(frame)
    x = models["preprocessor"].transform(clean)
    xgb_score = models["xgb"].predict_proba(x)[:, 1]
    lgbm_score = models["lgbm"].predict_proba(x)[:, 1]
    return np.asarray(cat_score, dtype=float), np.asarray(xgb_score, dtype=float), np.asarray(lgbm_score, dtype=float)


def score_tabular_indices(
    year: int,
    root: str,
    symbol: str,
    group: pd.DataFrame,
    direction: str,
    indices: np.ndarray,
    models: dict[str, Any],
    batch_size: int,
) -> dict[str, np.ndarray]:
    all_indices: list[np.ndarray] = []
    cat_scores: list[np.ndarray] = []
    xgb_scores: list[np.ndarray] = []
    lgbm_scores: list[np.ndarray] = []
    step = max(1000, int(batch_size))
    for start in range(0, len(indices), step):
        batch_indices = indices[start : start + step].astype(int)
        rows = [
            start_model.feature_row(year, root, str(symbol), group, int(idx), direction)
            for idx in batch_indices
        ]
        cat_score, xgb_score, lgbm_score = score_tabular_batch(rows, models)
        all_indices.append(batch_indices)
        cat_scores.append(cat_score)
        xgb_scores.append(xgb_score)
        lgbm_scores.append(lgbm_score)
    if not all_indices:
        empty = np.array([], dtype=float)
        return {"indices": np.array([], dtype=int), "catboost": empty, "xgboost": empty, "lightgbm": empty}
    return {
        "indices": np.concatenate(all_indices).astype(int),
        "catboost": np.concatenate(cat_scores),
        "xgboost": np.concatenate(xgb_scores),
        "lightgbm": np.concatenate(lgbm_scores),
    }


def image_gate_mask(tabular_masks: dict[str, np.ndarray], image_gate: str) -> np.ndarray:
    if image_gate == "tabular_all":
        return tabular_masks["tabular_all_3"]
    if image_gate == "tabular_majority":
        return tabular_masks["tabular_majority_2_of_3"]
    return tabular_masks["catboost"]


def score_visual_candidates(
    group: pd.DataFrame,
    direction: str,
    indices: np.ndarray,
    visual_model: torch.nn.Module,
    v_args: argparse.Namespace,
    batch_size: int,
) -> np.ndarray:
    scores = np.full(len(indices), np.nan, dtype=float)
    images: list[np.ndarray] = []
    positions: list[int] = []
    lookback = int(v_args.lookback_bars)
    min_candles = int(v_args.min_candles)

    def flush() -> None:
        if not images:
            return
        batch = np.stack(images).astype(np.float32)
        with torch.no_grad():
            tensor = torch.from_numpy(batch.reshape(len(batch), 3, int(v_args.height), int(v_args.width)))
            values = torch.sigmoid(visual_model(tensor)).cpu().numpy()
        for pos, value in zip(positions, values):
            scores[pos] = float(value)
        images.clear()
        positions.clear()

    for pos, idx in enumerate(indices.astype(int)):
        start_idx = max(0, int(idx) - lookback + 1)
        context = group.iloc[start_idx : int(idx) + 1].reset_index(drop=True)
        if len(context) < min_candles:
            continue
        image = visual_render.render_chart_image(context, direction, int(v_args.height), int(v_args.width))
        images.append(image.reshape(-1))
        positions.append(pos)
        if len(images) >= int(batch_size):
            flush()
    flush()
    return scores


def score_manager_candidates(
    year: int,
    root: str,
    symbol: str,
    group: pd.DataFrame,
    direction: str,
    indices: np.ndarray,
    tabular_scores: dict[str, np.ndarray],
    visual_scores: np.ndarray,
    thresholds: dict[str, float],
    manager: CatBoostClassifier,
) -> np.ndarray:
    rows: list[dict[str, Any]] = []
    for idx, cat_score, xgb_score, lgbm_score, image_score in zip(
        indices.astype(int),
        tabular_scores["catboost"],
        tabular_scores["xgboost"],
        tabular_scores["lightgbm"],
        visual_scores,
    ):
        row = start_model.feature_row(year, root, str(symbol), group, int(idx), direction)
        row.update(
            {
                "catboost_score": float(cat_score),
                "xgboost_score": float(xgb_score),
                "lightgbm_score": float(lgbm_score),
                "visual_score": float(image_score) if np.isfinite(image_score) else 0.0,
            }
        )
        rows.append(row)
    if not rows:
        return np.array([], dtype=float)
    frame = manager_model.add_manager_features(pd.DataFrame(rows), thresholds)
    return manager.predict_proba(manager_model.prepare_pool(frame, include_target=False))[:, 1]


def event_picks(
    rule: str,
    year: int,
    root: str,
    symbol: str,
    group: pd.DataFrame,
    direction: str,
    indices: np.ndarray,
    mask: np.ndarray,
    oracle_starts: list[dict[str, Any]],
    cooldown_bars: int,
    match_window_bars: int,
) -> list[dict[str, Any]]:
    picks: list[dict[str, Any]] = []
    next_allowed_idx = -1
    for idx, keep in zip(indices.astype(int), mask.astype(bool)):
        if not keep or int(idx) < next_allowed_idx:
            continue
        nearest = catboost_grid.nearest_oracle(int(idx), oracle_starts)
        matched = nearest["nearest_abs_bars"] is not None and int(nearest["nearest_abs_bars"]) <= int(match_window_bars)
        row = group.iloc[int(idx)]
        picks.append(
            {
                "rule": rule,
                "valid_year": year,
                "root_symbol": root,
                "symbol": str(symbol),
                "direction": direction,
                "signal_idx": int(idx),
                "signal_date": pd.Timestamp(row["ts_utc"]),
                "matched_oracle": bool(matched),
                **nearest,
            }
        )
        next_allowed_idx = int(idx) + int(cooldown_bars) + 1
    return picks


def summarize_rule(rule: str, picks: list[dict[str, Any]], oracle_total: int, match_window_bars: int, total_candidates: int) -> dict[str, Any]:
    frame = pd.DataFrame(picks)
    if frame.empty:
        return {
            "rule": rule,
            "total_candidates": int(total_candidates),
            "oracle_starts": int(oracle_total),
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
            "match_window_bars": int(match_window_bars),
        }
    matched = frame[frame["matched_oracle"].astype(bool)].copy()
    deltas = pd.to_numeric(matched["nearest_delta_bars"], errors="coerce").dropna()
    abs_deltas = deltas.abs()
    unique_oracles = matched["nearest_oracle_trade_id"].dropna().astype(str)
    unique_oracles = unique_oracles[unique_oracles != ""].nunique()
    return {
        "rule": rule,
        "total_candidates": int(total_candidates),
        "oracle_starts": int(oracle_total),
        "picked_events": int(len(frame)),
        "matched_picks": int(len(matched)),
        "pick_match_rate": float(len(matched) / len(frame)) if len(frame) else 0.0,
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
    models = load_models(str(args.four_model_run_id))
    manager, manager_default_threshold, manager_meta = load_manager(str(args.manager_run_id))
    four_meta = models["four_meta"]
    visual_meta = models["visual_meta"]
    thresholds = {name: float(value) for name, value in four_meta["thresholds"].items()}
    if float(args.catboost_threshold) >= 0:
        thresholds["catboost"] = float(args.catboost_threshold)
    if float(args.xgboost_threshold) >= 0:
        thresholds["xgboost"] = float(args.xgboost_threshold)
    if float(args.lightgbm_threshold) >= 0:
        thresholds["lightgbm"] = float(args.lightgbm_threshold)
    if float(args.visual_threshold) >= 0:
        thresholds["visual_cnn"] = float(args.visual_threshold)
    manager_threshold = float(args.manager_threshold) if float(args.manager_threshold) >= 0 else manager_default_threshold
    manager_feature_thresholds = (
        {name: float(value) for name, value in manager_meta.get("specialist_thresholds", {}).items()}
        if manager_meta is not None
        else thresholds
    )
    timeframe = args.timeframe or str(visual_meta.get("timeframe") or "2m")
    oracle_run_id = args.oracle_run_id or str(visual_meta.get("oracle_run_id") or start_model.DEFAULT_ORACLE_RUN)
    roots = start_model.parse_list(args.roots)
    v_args = visual_args_from_meta(visual_meta)

    conn = wave.connect()
    try:
        symbol_rows = catboost_grid.fetch_symbols(conn, timeframe, int(args.year), roots)
        oracle = start_model.fetch_oracle_starts(conn, oracle_run_id, int(args.year), roots)
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
    min_idx = max(60, int(v_args.lookback_bars) - 1)
    total_candidates = 0
    image_rows_scored = 0
    raw_rule_hits: dict[str, int] = {}
    rules = [
        "catboost",
        "xgboost",
        "lightgbm",
        "tabular_majority_2_of_3",
        "tabular_all_3",
        f"visual_confirmed_{args.image_gate}",
        "all_4",
    ]
    if manager is not None:
        rules.append("manager")
    all_picks: dict[str, list[dict[str, Any]]] = {rule: [] for rule in rules}

    try:
        for symbol_index, item in enumerate(symbol_rows, start=1):
            if args.limit_symbols and symbol_index > int(args.limit_symbols):
                break
            root = item["root_symbol"]
            symbol = item["symbol"]
            raw_group = catboost_grid.fetch_symbol_candles(conn, timeframe, int(args.year), root, symbol)
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
                scored = score_tabular_indices(
                    year=int(args.year),
                    root=root,
                    symbol=str(symbol),
                    group=group,
                    direction=direction,
                    indices=valid_indices,
                    models=models,
                    batch_size=int(args.tabular_batch_size),
                )
                indices = scored["indices"]
                cat_mask = scored["catboost"] >= thresholds["catboost"]
                xgb_mask = scored["xgboost"] >= thresholds["xgboost"]
                lgbm_mask = scored["lightgbm"] >= thresholds["lightgbm"]
                tab_count = cat_mask.astype(int) + xgb_mask.astype(int) + lgbm_mask.astype(int)
                tabular_masks = {
                    "catboost": cat_mask,
                    "xgboost": xgb_mask,
                    "lightgbm": lgbm_mask,
                    "tabular_majority_2_of_3": tab_count >= 2,
                    "tabular_all_3": tab_count == 3,
                }
                gate = image_gate_mask(tabular_masks, str(args.image_gate))
                visual_scores = np.full(len(indices), np.nan, dtype=float)
                if np.any(gate):
                    gated_positions = np.where(gate)[0]
                    image_rows_scored += int(len(gated_positions))
                    visual_scores[gated_positions] = score_visual_candidates(
                        group=group,
                        direction=direction,
                        indices=indices[gated_positions],
                        visual_model=models["visual_model"],
                        v_args=v_args,
                        batch_size=int(args.image_batch_size),
                    )
                visual_mask = np.nan_to_num(visual_scores, nan=-1.0) >= thresholds["visual_cnn"]
                masks = {
                    **tabular_masks,
                    f"visual_confirmed_{args.image_gate}": gate & visual_mask,
                    "all_4": tabular_masks["tabular_all_3"] & visual_mask,
                }
                if manager is not None:
                    manager_scores = np.full(len(indices), np.nan, dtype=float)
                    if np.any(gate):
                        gated_positions = np.where(gate)[0]
                        manager_scores[gated_positions] = score_manager_candidates(
                            year=int(args.year),
                            root=root,
                            symbol=str(symbol),
                            group=group,
                            direction=direction,
                            indices=indices[gated_positions],
                            tabular_scores={
                                "catboost": scored["catboost"][gated_positions],
                                "xgboost": scored["xgboost"][gated_positions],
                                "lightgbm": scored["lightgbm"][gated_positions],
                            },
                            visual_scores=visual_scores[gated_positions],
                            thresholds=manager_feature_thresholds,
                            manager=manager,
                        )
                    masks["manager"] = gate & (np.nan_to_num(manager_scores, nan=-1.0) >= float(manager_threshold))
                starts = catboost_grid.oracle_starts_for_symbol(oracle, group, str(symbol), direction)
                for rule, mask in masks.items():
                    raw_rule_hits[rule] = raw_rule_hits.get(rule, 0) + int(mask.sum())
                    all_picks[rule].extend(
                        event_picks(
                            rule=rule,
                            year=int(args.year),
                            root=root,
                            symbol=str(symbol),
                            group=group,
                            direction=direction,
                            indices=indices,
                            mask=mask,
                            oracle_starts=starts,
                            cooldown_bars=int(args.cooldown_bars),
                            match_window_bars=int(args.match_window_bars),
                        )
                    )
            if symbol_index % 25 == 0:
                print(
                    f"{args.year}: scored {symbol_index:,}/{len(symbol_rows):,} symbols "
                    f"candidates={total_candidates:,} image_rows={image_rows_scored:,}",
                    flush=True,
                )
    finally:
        conn.close()

    oracle_total = int(len(oracle))
    summaries = [
        {**summarize_rule(rule, all_picks[rule], oracle_total, int(args.match_window_bars), total_candidates), "raw_hits": raw_rule_hits.get(rule, 0)}
        for rule in rules
    ]
    output_base = models["four_dir"] / f"{args.output_prefix}_{args.image_gate}_{args.year}"
    summary_path = output_base.with_suffix(".summary.json")
    picks_path = output_base.with_suffix(".picks.csv")
    payload = {
        "four_model_run_id": args.four_model_run_id,
        "oracle_run_id": oracle_run_id,
        "timeframe": timeframe,
        "year": int(args.year),
        "roots": roots,
        "image_gate": args.image_gate,
        "cooldown_bars": int(args.cooldown_bars),
        "match_window_bars": int(args.match_window_bars),
        "thresholds": thresholds,
        "manager_run_id": args.manager_run_id or None,
        "manager_threshold": manager_threshold,
        "manager_feature_thresholds": manager_feature_thresholds,
        "manager_metadata": manager_meta,
        "total_candidates": int(total_candidates),
        "image_rows_scored": int(image_rows_scored),
        "summaries": summaries,
    }
    summary_path.write_text(json.dumps(payload, indent=2, default=to_jsonable), encoding="utf-8")
    pick_rows = [row for rule in rules for row in all_picks[rule]]
    if args.save_picks_limit and len(pick_rows) > int(args.save_picks_limit):
        pick_rows = pick_rows[: int(args.save_picks_limit)]
    pd.DataFrame(pick_rows).to_csv(picks_path, index=False)
    print(json.dumps(payload, indent=2, default=to_jsonable), flush=True)
    print(f"Saved summary: {summary_path}", flush=True)
    print(f"Saved picks: {picks_path}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
