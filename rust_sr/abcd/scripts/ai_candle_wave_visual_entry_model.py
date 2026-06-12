#!/usr/bin/env python3
"""
Train a leak-safe visual entry model for candle-wave policy trades.

This is a first image-neural-net add-on that uses only dependencies already in
the project environment. It renders candles that were fully closed before the
entry candle, flips SHORT trades so favorable movement is visually "up", and
trains an sklearn MLPRegressor on the flattened chart image.

The model is used as a trade filter research layer:

* Train visual score on a train-year policy CSV.
* Choose a visual-score threshold on a threshold-year policy CSV.
* Validate the same threshold on a valid-year policy CSV.
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
from PIL import Image
from sklearn.metrics import accuracy_score, log_loss, mean_absolute_error, mean_squared_error, roc_auc_score
from sklearn.neural_network import MLPClassifier, MLPRegressor


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_candle_wave_exit_model as exit_model
import ai_candle_wave_scanner_model as scanner
import ai_wave_rider_research as wave


DEFAULT_POLICY_RUN = "aicw-src-manager-180-livefix-rootgate25-v1-2m-2026"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--policy-run-id", default=DEFAULT_POLICY_RUN)
    parser.add_argument("--run-prefix", default="aicw-visual-entry-mlp-v1")
    parser.add_argument("--timeframe", default="2m")
    parser.add_argument("--train-year", type=int, default=2024)
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--lookback-bars", type=int, default=96)
    parser.add_argument("--height", type=int, default=64)
    parser.add_argument("--width", type=int, default=96)
    parser.add_argument("--min-candles", type=int, default=72)
    parser.add_argument("--history-multiplier", type=int, default=4)
    parser.add_argument("--hidden-layers", default="96,24")
    parser.add_argument("--target-mode", choices=["regression", "win_classifier"], default="regression")
    parser.add_argument("--good-r-threshold", type=float, default=0.0)
    parser.add_argument("--max-iter", type=int, default=120)
    parser.add_argument("--alpha", type=float, default=0.003)
    parser.add_argument("--learning-rate", type=float, default=0.001)
    parser.add_argument("--random-seed", type=int, default=73)
    parser.add_argument("--target-clip-min", type=float, default=-2.0)
    parser.add_argument("--target-clip-max", type=float, default=4.0)
    parser.add_argument("--min-keep-share", type=float, default=0.35)
    parser.add_argument("--min-keep-trades", type=int, default=150)
    parser.add_argument("--dd-penalty", type=float, default=0.0)
    parser.add_argument("--max-trades-per-year", type=int, default=0)
    parser.add_argument("--save-preview-count", type=int, default=12)
    parser.add_argument("--replace-run", action="store_true")
    return parser.parse_args()


def run_id(args: argparse.Namespace) -> str:
    rid = f"{args.run_prefix}-{args.timeframe}-{args.valid_year}"
    if len(rid) > 64:
        raise ValueError(f"Run id too long: {rid}")
    return rid


def hidden_layers(raw: str) -> tuple[int, ...]:
    layers = tuple(int(part.strip()) for part in raw.split(",") if part.strip())
    if not layers:
        raise ValueError("--hidden-layers must contain at least one layer size")
    return layers


def policy_csv(policy_run_id: str, year: int) -> Path:
    path = wave.ABCD_ROOT / "logs" / f"{policy_run_id}_full_policy_{year}.csv"
    if not path.exists():
        raise FileNotFoundError(f"Missing policy CSV: {path}")
    return path


def load_policy_rows(policy_run_id: str, year: int, limit: int) -> pd.DataFrame:
    rows = pd.read_csv(policy_csv(policy_run_id, year))
    required = {"candidate_uid", "entry_date", "symbol", "root_symbol", "direction", "result_r"}
    missing = sorted(required - set(rows.columns))
    if missing:
        raise ValueError(f"Policy CSV for {year} is missing columns: {missing}")
    rows["entry_date"] = pd.to_datetime(rows["entry_date"], errors="coerce")
    rows["result_r"] = pd.to_numeric(rows["result_r"], errors="coerce")
    rows = rows.dropna(subset=["candidate_uid", "entry_date", "symbol", "direction", "result_r"]).copy()
    rows = rows.sort_values(["entry_date", "candidate_uid"]).reset_index(drop=True)
    if int(limit or 0) > 0:
        rows = rows.head(int(limit)).copy()
    return rows


def y_from_price(value: float, low: float, span: float, height: int) -> int:
    if span <= 0:
        return height // 2
    scaled = (value - low) / span
    return int(np.clip(round((height - 1) - scaled * (height - 1)), 0, height - 1))


def fill_vertical(canvas: np.ndarray, channel: int, x: int, y0: int, y1: int, value: float) -> None:
    top = max(0, min(y0, y1))
    bottom = min(canvas.shape[1] - 1, max(y0, y1))
    canvas[channel, top : bottom + 1, x] = np.maximum(canvas[channel, top : bottom + 1, x], value)


def fill_rect(canvas: np.ndarray, channel: int, x0: int, x1: int, y0: int, y1: int, value: float) -> None:
    left = max(0, min(x0, x1))
    right = min(canvas.shape[2] - 1, max(x0, x1))
    top = max(0, min(y0, y1))
    bottom = min(canvas.shape[1] - 1, max(y0, y1))
    canvas[channel, top : bottom + 1, left : right + 1] = np.maximum(
        canvas[channel, top : bottom + 1, left : right + 1],
        value,
    )


def render_chart_image(candles: pd.DataFrame, direction: str, height: int, width: int) -> np.ndarray:
    """Render OHLC context to C,H,W float32 image."""
    if candles.empty:
        return np.zeros((3, height, width), dtype=np.float32)

    work = candles.tail(width).copy().reset_index(drop=True)
    sign = 1.0 if str(direction).upper() == "LONG" else -1.0

    opens = sign * work["open"].to_numpy(dtype=float)
    closes = sign * work["close"].to_numpy(dtype=float)
    highs_raw = sign * work["high"].to_numpy(dtype=float)
    lows_raw = sign * work["low"].to_numpy(dtype=float)
    highs = np.maximum(highs_raw, lows_raw)
    lows = np.minimum(highs_raw, lows_raw)

    price_low = float(np.nanmin(lows))
    price_high = float(np.nanmax(highs))
    span = price_high - price_low
    if not math.isfinite(span) or span <= 0:
        span = max(1e-9, abs(price_high) * 0.001)
    pad = span * 0.06
    price_low -= pad
    price_high += pad
    span = price_high - price_low

    canvas = np.zeros((3, height, width), dtype=np.float32)
    offset = width - len(work)
    candle_half = 1 if width >= 80 else 0

    for idx in range(len(work)):
        x = offset + idx
        if x < 0 or x >= width:
            continue
        open_y = y_from_price(float(opens[idx]), price_low, span, height)
        close_y = y_from_price(float(closes[idx]), price_low, span, height)
        high_y = y_from_price(float(highs[idx]), price_low, span, height)
        low_y = y_from_price(float(lows[idx]), price_low, span, height)
        up_channel = 0 if closes[idx] >= opens[idx] else 1
        fill_vertical(canvas, 2, x, high_y, low_y, 0.45)
        fill_rect(canvas, up_channel, x - candle_half, x + candle_half, open_y, close_y, 1.0)
        fill_vertical(canvas, up_channel, x, open_y, close_y, 1.0)

    last_close_y = y_from_price(float(closes[-1]), price_low, span, height)
    canvas[2, max(0, last_close_y - 1) : min(height, last_close_y + 2), :] = np.maximum(
        canvas[2, max(0, last_close_y - 1) : min(height, last_close_y + 2), :],
        0.35,
    )

    if "volume" in work.columns:
        volume = pd.to_numeric(work["volume"], errors="coerce").fillna(0).to_numpy(dtype=float)
        if np.nanmax(volume) > 0:
            vol = np.log1p(volume)
            vol = vol / max(1e-9, float(np.nanmax(vol)))
            strip = max(3, height // 12)
            for idx, value in enumerate(vol):
                x = offset + idx
                bar = int(round(value * strip))
                if 0 <= x < width and bar > 0:
                    canvas[2, height - bar : height, x] = np.maximum(canvas[2, height - bar : height, x], 0.8)

    return canvas


def load_context_candles(
    conn,
    table_name: str,
    symbol: str,
    entry_date: pd.Timestamp,
    timeframe_minutes: int,
    lookback_bars: int,
    history_multiplier: int,
) -> pd.DataFrame:
    end_at = entry_date - pd.Timedelta(microseconds=1)
    start_at = entry_date - pd.Timedelta(minutes=timeframe_minutes * lookback_bars * max(1, history_multiplier))
    candles = exit_model.load_candles(conn, table_name, symbol, start_at, end_at)
    if candles.empty:
        return candles
    return candles[candles["ts_utc"] < entry_date].tail(lookback_bars).reset_index(drop=True)


def build_image_matrix(
    conn,
    rows: pd.DataFrame,
    table_name: str,
    timeframe_minutes: int,
    args: argparse.Namespace,
    label: str,
) -> tuple[np.ndarray, pd.DataFrame]:
    images: list[np.ndarray] = []
    kept: list[dict[str, Any]] = []
    total = len(rows)
    for count, row in enumerate(rows.itertuples(index=False), start=1):
        candles = load_context_candles(
            conn,
            table_name,
            str(row.symbol),
            pd.Timestamp(row.entry_date),
            timeframe_minutes,
            args.lookback_bars,
            args.history_multiplier,
        )
        if len(candles) >= int(args.min_candles):
            image = render_chart_image(candles, str(row.direction), int(args.height), int(args.width))
            images.append(image.reshape(-1))
            kept.append(row._asdict())
        if count % 250 == 0 or count == total:
            print(f"{label}: rendered {count:,}/{total:,} rows, kept={len(kept):,}")
    if not images:
        return np.empty((0, 3 * args.height * args.width), dtype=np.float32), pd.DataFrame()
    return np.stack(images).astype(np.float32), pd.DataFrame(kept)


def summarize(values: pd.Series) -> dict[str, Any]:
    result = pd.to_numeric(values, errors="coerce").fillna(0.0)
    equity = result.cumsum()
    drawdown = equity.cummax() - equity
    wins = int((result > 0).sum())
    return {
        "trades": int(len(result)),
        "wins": wins,
        "losses": int((result <= 0).sum()),
        "win_rate": wins / len(result) if len(result) else 0.0,
        "avg_r": float(result.mean()) if len(result) else 0.0,
        "sum_r": float(result.sum()) if len(result) else 0.0,
        "max_drawdown_r": float(drawdown.max()) if len(drawdown) else 0.0,
    }


def score_frame(frame: pd.DataFrame, scores: np.ndarray) -> pd.DataFrame:
    scored = frame.copy()
    scored["visual_score_r"] = scores.astype(float)
    return scored


def choose_threshold(scored: pd.DataFrame, args: argparse.Namespace) -> tuple[float | None, pd.DataFrame]:
    if scored.empty:
        return None, pd.DataFrame()
    score = pd.to_numeric(scored["visual_score_r"], errors="coerce")
    candidates: list[float | None] = [None]
    candidates.extend(sorted(set(float(v) for v in score.quantile(np.linspace(0.05, 0.95, 37)).dropna())))
    min_keep = max(int(args.min_keep_trades), int(math.ceil(len(scored) * float(args.min_keep_share))))
    rows: list[dict[str, Any]] = []
    best_threshold: float | None = None
    best_key: tuple[float, float, float, int] | None = None
    for threshold in candidates:
        if threshold is None:
            subset = scored
        else:
            subset = scored[score >= threshold]
        stats = summarize(subset["result_r"])
        keep = int(stats["trades"])
        allowed = keep >= min_keep
        objective = stats["sum_r"] - float(args.dd_penalty) * stats["max_drawdown_r"]
        rows.append({"threshold": threshold, "allowed": allowed, "objective": objective, **stats})
        if not allowed:
            continue
        key = (objective, -stats["max_drawdown_r"], stats["avg_r"], keep)
        if best_key is None or key > best_key:
            best_key = key
            best_threshold = threshold
    return best_threshold, pd.DataFrame(rows)


def apply_threshold(scored: pd.DataFrame, threshold: float | None) -> pd.DataFrame:
    if threshold is None:
        return scored.copy()
    return scored[pd.to_numeric(scored["visual_score_r"], errors="coerce") >= float(threshold)].copy()


def print_summary(label: str, frame: pd.DataFrame) -> None:
    stats = summarize(frame["result_r"])
    print(
        f"{label}: trades={stats['trades']:,} win={stats['win_rate'] * 100:.2f}% "
        f"sum={stats['sum_r']:.2f}R avg={stats['avg_r']:.4f}R dd={stats['max_drawdown_r']:.2f}R"
    )


def safe_auc(y_true: np.ndarray, y_score: np.ndarray) -> float | None:
    try:
        if len(set(int(v) for v in y_true)) < 2:
            return None
        return float(roc_auc_score(y_true, y_score))
    except ValueError:
        return None


def prediction_fit_summary(
    mode: str,
    train_y_raw: np.ndarray,
    train_target: np.ndarray,
    train_scores: np.ndarray,
    threshold_frame: pd.DataFrame,
    threshold_scores: np.ndarray,
    valid_frame: pd.DataFrame,
    valid_scores: np.ndarray,
    good_r_threshold: float,
) -> dict[str, Any]:
    if mode == "win_classifier":
        threshold_target = (pd.to_numeric(threshold_frame["result_r"], errors="coerce").fillna(0.0).to_numpy() > good_r_threshold).astype(int)
        valid_target = (pd.to_numeric(valid_frame["result_r"], errors="coerce").fillna(0.0).to_numpy() > good_r_threshold).astype(int)
        train_pred = (train_scores >= 0.5).astype(int)
        threshold_pred = (threshold_scores >= 0.5).astype(int)
        valid_pred = (valid_scores >= 0.5).astype(int)
        result: dict[str, Any] = {
            "train_accuracy": float(accuracy_score(train_target, train_pred)),
            "threshold_accuracy": float(accuracy_score(threshold_target, threshold_pred)),
            "valid_accuracy": float(accuracy_score(valid_target, valid_pred)),
            "train_auc": safe_auc(train_target, train_scores),
            "threshold_auc": safe_auc(threshold_target, threshold_scores),
            "valid_auc": safe_auc(valid_target, valid_scores),
        }
        for name, target, scores in [
            ("train_log_loss", train_target, train_scores),
            ("threshold_log_loss", threshold_target, threshold_scores),
            ("valid_log_loss", valid_target, valid_scores),
        ]:
            try:
                result[name] = float(log_loss(target, np.clip(scores, 1e-6, 1 - 1e-6)))
            except ValueError:
                result[name] = None
        return result
    return {
        "train_mae": float(mean_absolute_error(train_y_raw, train_scores)),
        "threshold_mae": float(mean_absolute_error(threshold_frame["result_r"], threshold_scores)),
        "valid_mae": float(mean_absolute_error(valid_frame["result_r"], valid_scores)),
        "valid_rmse": float(mean_squared_error(valid_frame["result_r"], valid_scores) ** 0.5),
    }


def image_to_png(image: np.ndarray, height: int, width: int) -> Image.Image:
    hwc = np.moveaxis(image.reshape(3, height, width), 0, -1)
    arr = np.clip(hwc * 255.0, 0, 255).astype(np.uint8)
    return Image.fromarray(arr, mode="RGB")


def write_preview_images(images: np.ndarray, rows: pd.DataFrame, output_dir: Path, args: argparse.Namespace) -> None:
    count = min(int(args.save_preview_count), len(rows))
    if count <= 0:
        return
    preview_dir = output_dir / "previews"
    preview_dir.mkdir(parents=True, exist_ok=True)
    for idx in range(count):
        row = rows.iloc[idx]
        safe_uid = str(row["candidate_uid"])[:12]
        result = float(row["result_r"])
        name = f"{idx + 1:03d}_{row['symbol']}_{row['direction']}_{result:+.2f}R_{safe_uid}.png"
        image_to_png(images[idx], int(args.height), int(args.width)).save(preview_dir / name)


def main() -> int:
    args = parse_args()
    rid = run_id(args)
    model_dir = wave.ABCD_ROOT / "model_registry" / rid
    if model_dir.exists() and not args.replace_run:
        raise ValueError(f"Visual model run already exists: {rid}. Use --replace-run to overwrite.")
    model_dir.mkdir(parents=True, exist_ok=True)

    table_name, timeframe_minutes = scanner.table_for_timeframe(str(args.timeframe))
    train_rows = load_policy_rows(args.policy_run_id, args.train_year, args.max_trades_per_year)
    threshold_rows = load_policy_rows(args.policy_run_id, args.threshold_year, args.max_trades_per_year)
    valid_rows = load_policy_rows(args.policy_run_id, args.valid_year, args.max_trades_per_year)

    with wave.connect() as conn:
        train_x, train_kept = build_image_matrix(conn, train_rows, table_name, timeframe_minutes, args, "train")
        threshold_x, threshold_kept = build_image_matrix(conn, threshold_rows, table_name, timeframe_minutes, args, "threshold")
        valid_x, valid_kept = build_image_matrix(conn, valid_rows, table_name, timeframe_minutes, args, "valid")

    if len(train_kept) < 100:
        raise ValueError(f"Not enough train image rows: {len(train_kept)}")
    if len(threshold_kept) < 100:
        raise ValueError(f"Not enough threshold image rows: {len(threshold_kept)}")
    if len(valid_kept) < 100:
        raise ValueError(f"Not enough valid image rows: {len(valid_kept)}")

    train_y_raw = pd.to_numeric(train_kept["result_r"], errors="coerce").fillna(0.0).to_numpy(dtype=float)
    if args.target_mode == "win_classifier":
        train_y = (train_y_raw > float(args.good_r_threshold)).astype(int)
        model = MLPClassifier(
            hidden_layer_sizes=hidden_layers(args.hidden_layers),
            activation="relu",
            solver="adam",
            alpha=float(args.alpha),
            learning_rate_init=float(args.learning_rate),
            max_iter=int(args.max_iter),
            early_stopping=True,
            validation_fraction=0.20,
            n_iter_no_change=12,
            random_state=int(args.random_seed),
            verbose=False,
        )
    else:
        train_y = np.clip(train_y_raw, float(args.target_clip_min), float(args.target_clip_max))
        model = MLPRegressor(
            hidden_layer_sizes=hidden_layers(args.hidden_layers),
            activation="relu",
            solver="adam",
            alpha=float(args.alpha),
            learning_rate_init=float(args.learning_rate),
            max_iter=int(args.max_iter),
            early_stopping=True,
            validation_fraction=0.20,
            n_iter_no_change=12,
            random_state=int(args.random_seed),
            verbose=False,
        )
    print(
        f"Training visual MLP run={rid} train_rows={len(train_kept):,} "
        f"features={train_x.shape[1]:,} layers={hidden_layers(args.hidden_layers)} target_mode={args.target_mode}"
    )
    model.fit(train_x, train_y)

    if args.target_mode == "win_classifier":
        train_scores = model.predict_proba(train_x)[:, 1]
        threshold_scores = model.predict_proba(threshold_x)[:, 1]
        valid_scores = model.predict_proba(valid_x)[:, 1]
    else:
        train_scores = model.predict(train_x)
        threshold_scores = model.predict(threshold_x)
        valid_scores = model.predict(valid_x)

    train_scored = score_frame(train_kept, train_scores)
    threshold_scored = score_frame(threshold_kept, threshold_scores)
    valid_scored = score_frame(valid_kept, valid_scores)

    threshold, sweep = choose_threshold(threshold_scored, args)
    threshold_filtered = apply_threshold(threshold_scored, threshold)
    valid_filtered = apply_threshold(valid_scored, threshold)

    print_summary("train baseline", train_scored)
    print_summary("threshold baseline", threshold_scored)
    print_summary("threshold visual_filter", threshold_filtered)
    print_summary("valid baseline", valid_scored)
    print_summary("valid visual_filter", valid_filtered)
    print(f"selected_visual_threshold={'none' if threshold is None else f'{threshold:.6f}'}")
    fit_summary = prediction_fit_summary(
        args.target_mode,
        train_y_raw,
        train_y,
        train_scores,
        threshold_scored,
        threshold_scores,
        valid_scored,
        valid_scores,
        float(args.good_r_threshold),
    )
    print("prediction_fit " + " ".join(f"{key}={value}" for key, value in fit_summary.items()))

    joblib.dump(model, model_dir / "visual_entry_mlp.joblib")
    train_scored.to_csv(model_dir / f"visual_scores_{args.train_year}.csv", index=False)
    threshold_scored.to_csv(model_dir / f"visual_scores_{args.threshold_year}.csv", index=False)
    valid_scored.to_csv(model_dir / f"visual_scores_{args.valid_year}.csv", index=False)
    sweep.to_csv(model_dir / f"threshold_sweep_{args.threshold_year}.csv", index=False)
    write_preview_images(train_x, train_kept, model_dir, args)

    metadata = {
        "visual_model_run_id": rid,
        "model_type": "sklearn_mlp_regressor_flat_chart_image",
        "target_mode": args.target_mode,
        "policy_run_id": args.policy_run_id,
        "timeframe": args.timeframe,
        "train_year": args.train_year,
        "threshold_year": args.threshold_year,
        "valid_year": args.valid_year,
        "lookback_bars": args.lookback_bars,
        "height": args.height,
        "width": args.width,
        "min_candles": args.min_candles,
        "hidden_layers": hidden_layers(args.hidden_layers),
        "target": "policy result_r clipped for training" if args.target_mode == "regression" else "policy result_r > good_r_threshold",
        "good_r_threshold": args.good_r_threshold,
        "target_clip_min": args.target_clip_min,
        "target_clip_max": args.target_clip_max,
        "leak_safety": "Only candles with ts_utc < entry_date are rendered. SHORT charts are price-flipped.",
        "selected_visual_threshold": threshold,
        "train_summary": summarize(train_scored["result_r"]),
        "threshold_baseline_summary": summarize(threshold_scored["result_r"]),
        "threshold_filter_summary": summarize(threshold_filtered["result_r"]),
        "valid_baseline_summary": summarize(valid_scored["result_r"]),
        "valid_filter_summary": summarize(valid_filtered["result_r"]),
        "prediction_fit": {
            **fit_summary,
            "iterations": int(getattr(model, "n_iter_", 0)),
            "loss": float(getattr(model, "loss_", 0.0)),
        },
    }
    (model_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, default=str), encoding="utf-8")
    print(f"Saved visual model: {rid}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
