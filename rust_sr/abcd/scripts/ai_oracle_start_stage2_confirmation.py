#!/usr/bin/env python3
"""
Train a Stage 2 oracle-start confirmation / fakeout filter.

Stage 1 is intentionally wide: it says "something may be starting." Stage 2
waits a small number of candles, then decides whether the move is actually
behaving like a real oracle trend or should be canceled as a fakeout.

This is a proof-of-concept on the existing sampled four-model oracle rows. It
does not create trades or exits.
"""

from __future__ import annotations

import argparse
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
import ai_oracle_start_four_model_agreement as four_model
import ai_oracle_start_manager_model as manager_model
import ai_oracle_trend_start_model as start_model
import ai_wave_rider_research as wave


DEFAULT_FOUR_MODEL_RUN = "aicw-oracle-start-fourmodel-v1-v2026"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--four-model-run-id", default=DEFAULT_FOUR_MODEL_RUN)
    parser.add_argument("--run-prefix", default="aicw-oracle-start-stage2-confirm-v1")
    parser.add_argument("--train-label", default="train")
    parser.add_argument("--train-year", type=int, default=2024)
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--timeframe", default="2m")
    parser.add_argument("--roots", default="")
    parser.add_argument("--confirm-bars", type=int, default=8)
    parser.add_argument("--stage1-rule", choices=["catboost", "any2", "tabular_majority", "all4"], default="catboost")
    parser.add_argument("--stage1-threshold", type=float, default=0.385)
    parser.add_argument("--min-threshold-precision", type=float, default=0.70)
    parser.add_argument("--min-threshold-picks", type=int, default=300)
    parser.add_argument("--iterations", type=int, default=500)
    parser.add_argument("--depth", type=int, default=5)
    parser.add_argument("--learning-rate", type=float, default=0.045)
    parser.add_argument("--l2-leaf-reg", type=float, default=8.0)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--max-train-rows", type=int, default=0)
    parser.add_argument("--max-eval-rows", type=int, default=0)
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
    rid = f"{args.run_prefix}-{args.stage1_rule}-w{int(args.confirm_bars)}-v{args.valid_year}"
    if len(rid) > 64:
        raise ValueError(f"Run id too long: {rid}")
    return rid


def load_json(path: Path) -> dict[str, Any]:
    if not path.exists():
        raise FileNotFoundError(f"Missing JSON: {path}")
    return json.loads(path.read_text(encoding="utf-8"))


def source_dir(run_id_value: str) -> tuple[Path, dict[str, Any]]:
    model_dir = wave.ABCD_ROOT / "model_registry" / run_id_value
    metadata = load_json(model_dir / "metadata.json")
    return model_dir, metadata


def load_split(model_dir: Path, split: str | int, limit: int) -> pd.DataFrame:
    if str(split) == "train":
        path = model_dir / "four_model_scores_train.csv"
    else:
        path = model_dir / f"four_model_scores_{split}.csv"
    if not path.exists():
        raise FileNotFoundError(f"Missing four-model score split: {path}")
    frame = pd.read_csv(path)
    if limit > 0 and len(frame) > limit:
        positives = frame[frame["is_oracle_start"] == 1]
        negatives = frame[frame["is_oracle_start"] == 0]
        keep_pos = positives.sample(n=min(len(positives), limit // 2), random_state=73)
        keep_neg = negatives.sample(n=min(len(negatives), max(0, limit - len(keep_pos))), random_state=73)
        frame = pd.concat([keep_pos, keep_neg], ignore_index=True).sample(frac=1.0, random_state=73).reset_index(drop=True)
    frame["is_oracle_start"] = pd.to_numeric(frame["is_oracle_start"], errors="coerce").fillna(0).astype(int)
    frame["signal_date"] = pd.to_datetime(frame["signal_date"], errors="coerce")
    return frame.dropna(subset=["symbol", "signal_date", "direction"]).reset_index(drop=True)


def stage1_mask(frame: pd.DataFrame, thresholds: dict[str, float], args: argparse.Namespace) -> np.ndarray:
    cat = pd.to_numeric(frame["catboost_score"], errors="coerce").fillna(0.0).to_numpy() >= float(args.stage1_threshold)
    xgb = pd.to_numeric(frame["xgboost_score"], errors="coerce").fillna(0.0).to_numpy() >= float(thresholds.get("xgboost", 0.5))
    lgbm = pd.to_numeric(frame["lightgbm_score"], errors="coerce").fillna(0.0).to_numpy() >= float(thresholds.get("lightgbm", 0.5))
    visual = pd.to_numeric(frame["visual_score"], errors="coerce").fillna(0.0).to_numpy() >= float(thresholds.get("visual_cnn", 0.5))
    if args.stage1_rule == "catboost":
        return cat
    if args.stage1_rule == "tabular_majority":
        return (cat.astype(int) + xgb.astype(int) + lgbm.astype(int)) >= 2
    if args.stage1_rule == "all4":
        return cat & xgb & lgbm & visual
    return (cat.astype(int) + xgb.astype(int) + lgbm.astype(int) + visual.astype(int)) >= 2


def post_features(group: pd.DataFrame, idx: int, direction: str, confirm_bars: int, atr_hint: float | None) -> dict[str, Any] | None:
    if idx < 0 or idx >= len(group) - 1:
        return None
    end_idx = min(len(group) - 1, idx + int(confirm_bars))
    if end_idx <= idx:
        return None
    segment = group.iloc[idx + 1 : end_idx + 1]
    if segment.empty:
        return None
    signal = group.iloc[idx]
    sign = wave.direction_sign(direction)
    base = float(signal["close"])
    atr = wave.finite(atr_hint, None)
    if atr is None or atr <= 0:
        atr = wave.finite(signal.get("atr"), None)
    if atr is None or atr <= 0:
        atr = max(1e-9, float(signal["high"]) - float(signal["low"]))
    oriented_open = sign * segment["open"].to_numpy(dtype=float)
    oriented_close = sign * segment["close"].to_numpy(dtype=float)
    oriented_high = sign * segment["high"].to_numpy(dtype=float)
    oriented_low = sign * segment["low"].to_numpy(dtype=float)
    favorable_high = np.maximum(oriented_high, oriented_low)
    adverse_low = np.minimum(oriented_high, oriented_low)
    oriented_base = sign * base
    close_move = (oriented_close[-1] - oriented_base) / atr
    max_favorable = (float(np.max(favorable_high)) - oriented_base) / atr
    max_adverse = (oriented_base - float(np.min(adverse_low))) / atr
    best_close = (float(np.max(oriented_close)) - oriented_base) / atr
    worst_close = (oriented_base - float(np.min(oriented_close))) / atr
    body_alignment = oriented_close > oriented_open
    higher_close_steps = np.diff(np.concatenate([[oriented_base], oriented_close])) > 0
    first_half = segment.iloc[: max(1, len(segment) // 2)]
    second_half = segment.iloc[max(1, len(segment) // 2) :]
    first_close = sign * float(first_half["close"].iloc[-1])
    second_close = sign * float(second_half["close"].iloc[-1]) if len(second_half) else oriented_close[-1]
    return {
        "confirm_bars_available": int(len(segment)),
        "confirm_close_move_atr": close_move,
        "confirm_max_favorable_atr": max_favorable,
        "confirm_max_adverse_atr": max_adverse,
        "confirm_best_close_atr": best_close,
        "confirm_worst_close_atr": worst_close,
        "confirm_fav_minus_adv_atr": max_favorable - max_adverse,
        "confirm_close_minus_adv_atr": close_move - max_adverse,
        "confirm_efficiency": close_move / max(0.1, max_favorable + max_adverse),
        "confirm_body_align_share": float(np.mean(body_alignment)) if len(body_alignment) else 0.0,
        "confirm_close_step_align_share": float(np.mean(higher_close_steps)) if len(higher_close_steps) else 0.0,
        "confirm_second_half_vs_first_atr": (second_close - first_close) / atr,
        "confirm_last_body_atr": abs(float(segment["close"].iloc[-1]) - float(segment["open"].iloc[-1])) / atr,
        "confirm_last_wick_against_atr": (
            (float(segment["high"].iloc[-1]) - max(float(segment["open"].iloc[-1]), float(segment["close"].iloc[-1]))) / atr
            if direction == "SHORT"
            else (min(float(segment["open"].iloc[-1]), float(segment["close"].iloc[-1])) - float(segment["low"].iloc[-1])) / atr
        ),
    }


POST_FEATURES = [
    "confirm_bars_available",
    "confirm_close_move_atr",
    "confirm_max_favorable_atr",
    "confirm_max_adverse_atr",
    "confirm_best_close_atr",
    "confirm_worst_close_atr",
    "confirm_fav_minus_adv_atr",
    "confirm_close_minus_adv_atr",
    "confirm_efficiency",
    "confirm_body_align_share",
    "confirm_close_step_align_share",
    "confirm_second_half_vs_first_atr",
    "confirm_last_body_atr",
    "confirm_last_wick_against_atr",
]


def add_confirmation_features(conn, frame: pd.DataFrame, year: int, args: argparse.Namespace, label: str) -> pd.DataFrame:
    roots = start_model.parse_list(args.roots)
    candles = start_model.fetch_candles(conn, args.timeframe, int(year), roots)
    if candles.empty:
        raise ValueError(f"No candles available for {year}")
    rows_by_symbol = {
        str(symbol): group.sort_values("signal_date").reset_index()
        for symbol, group in frame.groupby("symbol", sort=False)
    }
    output: list[dict[str, Any]] = []
    seen = 0
    total = len(frame)
    for symbol_index, (symbol, raw_group) in enumerate(candles.groupby("symbol", sort=True), start=1):
        targets = rows_by_symbol.get(str(symbol))
        if targets is None or targets.empty:
            continue
        group = scanner.enrich_candles(raw_group[["ts_utc", "open", "high", "low", "close", "volume"]].copy(), args)
        times = pd.to_datetime(group["ts_utc"], errors="coerce").to_numpy(dtype="datetime64[ns]")
        for item in targets.itertuples(index=False):
            original = frame.iloc[int(item.index)].to_dict()
            signal_ts = pd.Timestamp(original["signal_date"])
            idx = int(np.searchsorted(times, np.datetime64(signal_ts), side="left"))
            if idx >= len(group):
                continue
            atr_hint = None
            tick_size = wave.tick_size_for(str(original["symbol"]), str(original.get("root_symbol") or ""))
            atr_ticks = wave.finite(original.get("atr_ticks"), None)
            if atr_ticks is not None and tick_size > 0:
                atr_hint = atr_ticks * tick_size
            features = post_features(group, idx, str(original["direction"]), int(args.confirm_bars), atr_hint)
            if features is None:
                continue
            original.update(features)
            output.append(original)
            seen += 1
        if symbol_index % 75 == 0:
            print(f"{label}: confirmation features symbols={symbol_index:,} rows={seen:,}/{total:,}", flush=True)
    print(f"{label}: confirmation features kept={len(output):,}/{total:,}", flush=True)
    return pd.DataFrame(output)


def feature_columns() -> tuple[list[str], list[str]]:
    cat_features = list(start_model.CAT_FEATURES)
    num_features = list(start_model.NUM_FEATURES) + manager_model.DERIVED_NUM_FEATURES + POST_FEATURES
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
        return Pool(work[cols], label=work["is_oracle_start"].astype(int), cat_features=cat_features)
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


def metrics(frame: pd.DataFrame, mask: np.ndarray, score_col: str | None = None, threshold: float | None = None) -> dict[str, Any]:
    y = frame["is_oracle_start"].astype(int).to_numpy()
    picks = int(mask.sum())
    true_picks = int((mask & (y == 1)).sum())
    precision = true_picks / picks if picks else 0.0
    recall = true_picks / int(y.sum()) if int(y.sum()) else 0.0
    f1 = (2.0 * precision * recall / (precision + recall)) if (precision + recall) > 0 else 0.0
    out = {
        "rows": int(len(frame)),
        "positives": int(y.sum()),
        "picks": picks,
        "true_picks": true_picks,
        "precision": precision,
        "recall": recall,
        "f1": f1,
    }
    if score_col is not None:
        score = pd.to_numeric(frame[score_col], errors="coerce").fillna(0.0).to_numpy()
        out.update(
            {
                "auc": safe_auc(y, score),
                "average_precision": safe_ap(y, score),
                "threshold": float(threshold),
            }
        )
    return out


def choose_threshold(frame: pd.DataFrame, stage1: np.ndarray, args: argparse.Namespace) -> tuple[float, pd.DataFrame]:
    subset = frame[stage1].copy()
    score = pd.to_numeric(subset["stage2_score"], errors="coerce").fillna(0.0).to_numpy()
    candidates = sorted(set(float(v) for v in np.quantile(score, np.linspace(0.05, 0.99, 80))))
    best_threshold = candidates[0]
    best_key: tuple[float, float, int] | None = None
    rows: list[dict[str, Any]] = []
    for threshold in candidates:
        mask = pd.to_numeric(frame["stage2_score"], errors="coerce").fillna(0.0).to_numpy() >= threshold
        final = stage1 & mask
        item = metrics(frame, final, "stage2_score", threshold)
        allowed = item["precision"] >= float(args.min_threshold_precision) and item["picks"] >= int(args.min_threshold_picks)
        rows.append({**item, "allowed": allowed})
        if allowed:
            key = (float(item.get("f1", 0.0)), float(item["precision"]), int(item["picks"]))
            if best_key is None or key > best_key:
                best_key = key
                best_threshold = threshold
    if best_key is None:
        best_threshold = max(candidates, key=lambda value: metrics(frame, stage1 & (pd.to_numeric(frame["stage2_score"], errors="coerce").fillna(0.0).to_numpy() >= value)).get("precision", 0.0))
    return best_threshold, pd.DataFrame(rows)


def evaluate_split(frame: pd.DataFrame, stage1: np.ndarray, threshold: float) -> dict[str, Any]:
    score = pd.to_numeric(frame["stage2_score"], errors="coerce").fillna(0.0).to_numpy()
    stage2 = score >= float(threshold)
    return {
        "stage1": metrics(frame, stage1),
        "stage1_then_stage2": metrics(frame, stage1 & stage2, "stage2_score", threshold),
        "stage2_all_rows": metrics(frame, stage2, "stage2_score", threshold),
    }


def to_jsonable(value: Any) -> Any:
    if isinstance(value, np.generic):
        return value.item()
    if isinstance(value, pd.Timestamp):
        return value.isoformat()
    if isinstance(value, float) and (math.isnan(value) or math.isinf(value)):
        return None
    return value


def main() -> int:
    args = parse_args()
    rid = run_id(args)
    output_dir = wave.ABCD_ROOT / "model_registry" / rid
    if output_dir.exists() and not args.replace_run:
        raise ValueError(f"Stage 2 run already exists: {rid}. Use --replace-run.")
    output_dir.mkdir(parents=True, exist_ok=True)

    model_dir, meta = source_dir(str(args.four_model_run_id))
    thresholds = {name: float(value) for name, value in meta["thresholds"].items()}
    conn = wave.connect()
    try:
        train_base = load_split(model_dir, args.train_label, int(args.max_train_rows))
        threshold_base = load_split(model_dir, int(args.threshold_year), int(args.max_eval_rows))
        valid_base = load_split(model_dir, int(args.valid_year), int(args.max_eval_rows))
        train = add_confirmation_features(conn, manager_model.add_manager_features(train_base, thresholds), int(args.train_year), args, "train")
        threshold = add_confirmation_features(conn, manager_model.add_manager_features(threshold_base, thresholds), int(args.threshold_year), args, "threshold")
        valid = add_confirmation_features(conn, manager_model.add_manager_features(valid_base, thresholds), int(args.valid_year), args, "valid")
    finally:
        conn.close()

    print(f"Training Stage 2 rows train={len(train):,} threshold={len(threshold):,} valid={len(valid):,}", flush=True)
    model = train_model(train, args)
    train_scored = add_scores(train, model)
    threshold_scored = add_scores(threshold, model)
    valid_scored = add_scores(valid, model)

    train_stage1 = stage1_mask(train_scored, thresholds, args)
    threshold_stage1 = stage1_mask(threshold_scored, thresholds, args)
    valid_stage1 = stage1_mask(valid_scored, thresholds, args)
    selected_threshold, sweep = choose_threshold(threshold_scored, threshold_stage1, args)
    print(f"selected_stage2_threshold={selected_threshold:.6f}", flush=True)
    results = {
        "train": evaluate_split(train_scored, train_stage1, selected_threshold),
        str(args.threshold_year): evaluate_split(threshold_scored, threshold_stage1, selected_threshold),
        str(args.valid_year): evaluate_split(valid_scored, valid_stage1, selected_threshold),
    }
    for split, item in results.items():
        print(f"{split}={json.dumps(item, default=to_jsonable)}", flush=True)

    model.save_model(str(output_dir / "catboost_stage2_confirmation.cbm"))
    train_scored.to_csv(output_dir / "stage2_scores_train.csv", index=False)
    threshold_scored.to_csv(output_dir / f"stage2_scores_{args.threshold_year}.csv", index=False)
    valid_scored.to_csv(output_dir / f"stage2_scores_{args.valid_year}.csv", index=False)
    sweep.to_csv(output_dir / f"stage2_threshold_sweep_{args.threshold_year}.csv", index=False)
    cat_features, num_features = feature_columns()
    metadata = {
        "stage2_run_id": rid,
        "model_type": "catboost_oracle_start_stage2_confirmation",
        "four_model_run_id": args.four_model_run_id,
        "stage1_rule": args.stage1_rule,
        "stage1_threshold": float(args.stage1_threshold),
        "confirm_bars": int(args.confirm_bars),
        "selected_stage2_threshold": float(selected_threshold),
        "features": {"cat": cat_features, "num": num_features},
        "results": results,
        "read": "Proof-of-concept fakeout filter. Features after the signal are live only after waiting confirm_bars candles.",
    }
    (output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=to_jsonable), encoding="utf-8")
    print(f"Saved Stage 2 confirmation model: {rid}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
