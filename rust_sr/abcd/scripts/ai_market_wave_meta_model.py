#!/usr/bin/env python3
"""
Combine market-entry and wave-rider models with a meta approval layer.

The market-entry model supplies the setup universe and setup/template scores.
The wave-rider model proposes one dynamic entry/exit per setup. The meta model
then learns whether to approve that proposal.

Training is stacked with expanding out-of-fold wave proposals:
  2023 proposal model trains on 2022
  2024 proposal model trains on 2022-2023
  2025 proposal model trains on 2022-2024

The final 2026 run trains the wave proposal model on 2022-2025, scores 2026,
then applies the meta approval model trained on the out-of-fold proposals.
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
    from catboost import CatBoostRegressor, Pool
except ImportError as exc:  # pragma: no cover - runtime environment message
    raise SystemExit("CatBoost is required. Run with .venv_ai\\Scripts\\python.exe") from exc


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_wave_rider_model as wrm
import ai_wave_rider_research as wave


META_DIAGNOSTICS_TABLE = "ai_market_wave_meta_diagnostics"

META_CAT_FEATURES = [
    *wrm.CAT_FEATURES,
    "source_template_uid",
    "wave_exit_reason",
]

META_NUM_FEATURES = [
    *wrm.NUM_FEATURES,
    "wave_predicted_r",
    "wave_rank_score",
    "wave_market_score_spread",
    "wave_market_margin_product",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-prefix", default="aimv-market-entry-ensemble")
    parser.add_argument("--candidate-years", default="2022,2023,2024,2025,2026")
    parser.add_argument("--meta-years", default="2023,2024,2025")
    parser.add_argument("--train-years", default="2022,2023,2024,2025")
    parser.add_argument("--valid-year", type=int, default=2026)
    parser.add_argument("--threshold-year", type=int, default=2025)
    parser.add_argument("--run-prefix", default="aimv-market-wave-meta-v1")
    parser.add_argument("--source-run-prefix", default="market-wave-meta-v1")
    parser.add_argument("--replace-run", action="store_true")
    parser.add_argument("--threshold", type=float, default=None)
    parser.add_argument("--min-threshold-trades", type=int, default=35)
    parser.add_argument("--iterations", type=int, default=700)
    parser.add_argument("--depth", type=int, default=6)
    parser.add_argument("--learning-rate", type=float, default=0.04)
    parser.add_argument("--l2-leaf-reg", type=float, default=8.0)
    parser.add_argument("--random-seed", type=int, default=61)
    parser.add_argument("--allow-duplicate-entry", action="store_true")
    parser.add_argument("--api-refresh-url", default="http://localhost:8080/patterns/ai-stage1-trades")
    parser.add_argument("--skip-api-refresh", action="store_true")
    parser.add_argument("--slippage-entry-ticks", type=float, default=3.0)
    parser.add_argument("--slippage-exit-ticks", type=float, default=3.0)
    parser.add_argument("--min-risk-ticks", type=float, default=12.0)
    parser.add_argument("--profit-lock-trigger-r", type=float, default=0.0)
    parser.add_argument("--profit-lock-giveback-r", type=float, default=1.5)
    parser.add_argument("--profit-lock-min-r", type=float, default=0.25)
    parser.add_argument("--time-exit-minutes", type=int, default=0)
    parser.add_argument("--target-clip-min", type=float, default=-1.5)
    parser.add_argument("--wave-target-clip-max", type=float, default=None)
    parser.add_argument("--meta-target-clip-max", type=float, default=None)
    parser.add_argument(
        "--threshold-cap-r",
        type=float,
        default=None,
        help="Optionally choose the approval threshold using winners capped at this R value.",
    )
    return parser.parse_args()


def clean_feature_frame(frame: pd.DataFrame, cat_features: list[str], num_features: list[str]) -> pd.DataFrame:
    work = frame.copy()
    for col in cat_features:
        if col not in work.columns:
            work[col] = "Unknown"
        work[col] = work[col].fillna("Unknown").astype(str)
    for col in num_features:
        if col not in work.columns:
            work[col] = 0.0
        work[col] = pd.to_numeric(work[col], errors="coerce").replace([np.inf, -np.inf], np.nan).fillna(0.0)
    return work


def train_wave_model(frame: pd.DataFrame, args: argparse.Namespace) -> CatBoostRegressor:
    if args.wave_target_clip_max is None:
        return wrm.train_model(frame, args)
    work = frame.copy()
    for col in wrm.CAT_FEATURES:
        work[col] = work[col].fillna("Unknown").astype(str)
    for col in wrm.NUM_FEATURES:
        work[col] = pd.to_numeric(work[col], errors="coerce").replace([np.inf, -np.inf], np.nan).fillna(0.0)
    target = pd.to_numeric(work["result_r"], errors="coerce").fillna(0.0).clip(
        lower=args.target_clip_min,
        upper=args.wave_target_clip_max,
    )
    pool = Pool(work[wrm.CAT_FEATURES + wrm.NUM_FEATURES], label=target, cat_features=wrm.CAT_FEATURES)
    model = CatBoostRegressor(
        loss_function="RMSE",
        iterations=args.iterations,
        depth=args.depth,
        learning_rate=args.learning_rate,
        l2_leaf_reg=args.l2_leaf_reg,
        random_seed=args.random_seed,
        verbose=False,
        allow_writing_files=False,
    )
    model.fit(pool)
    return model


def train_meta_model(frame: pd.DataFrame, args: argparse.Namespace) -> CatBoostRegressor:
    work = clean_feature_frame(frame, META_CAT_FEATURES, META_NUM_FEATURES)
    target = pd.to_numeric(work["result_r"], errors="coerce").fillna(0.0)
    if args.meta_target_clip_max is not None:
        target = target.clip(lower=args.target_clip_min, upper=args.meta_target_clip_max)
    pool = Pool(work[META_CAT_FEATURES + META_NUM_FEATURES], label=target, cat_features=META_CAT_FEATURES)
    model = CatBoostRegressor(
        loss_function="RMSE",
        iterations=args.iterations,
        depth=args.depth,
        learning_rate=args.learning_rate,
        l2_leaf_reg=args.l2_leaf_reg,
        random_seed=args.random_seed,
        verbose=False,
        allow_writing_files=False,
    )
    model.fit(pool)
    return model


def add_meta_predictions(frame: pd.DataFrame, model: CatBoostRegressor) -> pd.DataFrame:
    if frame.empty:
        return frame
    work = clean_feature_frame(frame, META_CAT_FEATURES, META_NUM_FEATURES)
    scored = frame.copy()
    scored["meta_predicted_r"] = model.predict(Pool(work[META_CAT_FEATURES + META_NUM_FEATURES], cat_features=META_CAT_FEATURES))
    return scored


def build_top_wave_proposals(candidates: pd.DataFrame, wave_model: CatBoostRegressor) -> pd.DataFrame:
    if candidates.empty:
        return pd.DataFrame()
    scored = wrm.add_predictions(candidates, wave_model)
    top = wrm.top_per_setup(scored)
    top = top.rename(columns={"predicted_wave_r": "wave_predicted_r"})
    top["wave_rank_score"] = top["wave_predicted_r"]
    top["source_template_uid"] = top.get("source_template_uid", "Unknown")
    top["wave_exit_reason"] = top.get("exit_reason", "unknown").fillna("unknown").astype(str)
    top["wave_market_score_spread"] = pd.to_numeric(top["wave_predicted_r"], errors="coerce").fillna(0.0) - pd.to_numeric(
        top.get("source_predicted_expected_r", 0.0), errors="coerce"
    ).fillna(0.0)
    top["wave_market_margin_product"] = pd.to_numeric(top["wave_predicted_r"], errors="coerce").fillna(0.0) * pd.to_numeric(
        top.get("source_score_margin_top2", 0.0), errors="coerce"
    ).fillna(0.0)
    return top


def build_oof_meta_frame(conn, args: argparse.Namespace) -> pd.DataFrame:
    meta_years = wave.years_from_arg(args.meta_years)
    frames: list[pd.DataFrame] = []
    for year in meta_years:
        prior_years = [candidate_year for candidate_year in wave.years_from_arg(args.candidate_years) if candidate_year < year]
        if not prior_years:
            continue
        train_ids = [wave.safe_run_id(args.source_prefix, prior_year) for prior_year in prior_years]
        valid_id = wave.safe_run_id(args.source_prefix, year)
        train_candidates = wrm.load_candidates(conn, train_ids)
        valid_candidates = wrm.load_candidates(conn, [valid_id])
        if train_candidates.empty or valid_candidates.empty:
            continue
        print(f"OOF wave proposals for {year}: train {prior_years} -> {len(valid_candidates):,} candidates")
        model = train_wave_model(train_candidates, args)
        proposals = build_top_wave_proposals(valid_candidates, model)
        proposals["proposal_year"] = year
        frames.append(proposals)
    if not frames:
        return pd.DataFrame()
    return pd.concat(frames, ignore_index=True)


def choose_threshold(meta_frame: pd.DataFrame, args: argparse.Namespace) -> float | None:
    if args.threshold is not None:
        return args.threshold
    threshold_rows = meta_frame[meta_frame["proposal_year"] == args.threshold_year].copy()
    if threshold_rows.empty:
        print("Missing threshold-year meta proposals; using no meta threshold.")
        return None
    candidates = sorted(
        set(float(value) for value in threshold_rows["meta_predicted_r"].quantile(np.linspace(0.0, 0.95, 20)).dropna())
    )
    candidates = [None] + candidates
    summaries = [summarize_meta_selection(threshold_rows, value, args) for value in candidates]
    viable = [item for item in summaries if item["selected"] >= args.min_threshold_trades]
    if not viable:
        viable = summaries
    best = max(viable, key=lambda item: (item["threshold_score_sum_r"], item["threshold_score_avg_r"], item["selected"]))
    print("Meta threshold calibration on", args.threshold_year)
    for item in sorted(summaries, key=lambda row: row["threshold_score_sum_r"], reverse=True)[:8]:
        label = "all" if item["threshold"] is None else f"{item['threshold']:.3f}"
        score_label = "sum" if args.threshold_cap_r is None else f"cap{args.threshold_cap_r:g} sum"
        print(
            f"  threshold {label}: selected={item['selected']}, win={item['win_rate'] * 100:.2f}%, "
            f"avg={item['avg_r']:.3f}R, sum={item['sum_r']:.1f}R, {score_label}={item['threshold_score_sum_r']:.1f}R"
        )
    label = "all" if best["threshold"] is None else f"{best['threshold']:.3f}"
    print(f"Selected meta threshold: {label}")
    return best["threshold"]


def summarize_meta_selection(frame: pd.DataFrame, threshold: float | None, args: argparse.Namespace) -> dict[str, Any]:
    selected = frame.copy() if threshold is None else frame[frame["meta_predicted_r"] >= threshold].copy()
    if not args.allow_duplicate_entry and not selected.empty:
        selected = selected.sort_values(["meta_predicted_r", "wave_predicted_r", "result_r"], ascending=[False, False, False])
        selected = selected.drop_duplicates(subset=["symbol", "entry_date", "direction"], keep="first")
    result = pd.to_numeric(selected["result_r"], errors="coerce").fillna(0.0)
    threshold_score_result = result.clip(upper=args.threshold_cap_r) if args.threshold_cap_r is not None else result
    wins = int((result > 0.0).sum())
    count = int(len(result))
    return {
        "threshold": threshold,
        "selected": count,
        "wins": wins,
        "losses": int(count - wins),
        "win_rate": wins / count if count else 0.0,
        "avg_r": float(result.mean()) if count else 0.0,
        "sum_r": float(result.sum()) if count else 0.0,
        "threshold_score_avg_r": float(threshold_score_result.mean()) if count else 0.0,
        "threshold_score_sum_r": float(threshold_score_result.sum()) if count else 0.0,
    }


def ensure_meta_diagnostics_table(conn) -> None:
    with conn.cursor() as cur:
        cur.execute(
            f"""
            CREATE TABLE IF NOT EXISTS {META_DIAGNOSTICS_TABLE} (
                multi_valid_eval_run_id VARCHAR(64) NOT NULL,
                source_ai_run_id VARCHAR(64) NOT NULL,
                setup_id VARCHAR(64) NOT NULL,
                template_uid VARCHAR(128) NOT NULL,
                symbol VARCHAR(32) NULL,
                entry_date DATETIME NULL,
                direction VARCHAR(16) NULL,
                decision VARCHAR(64) NOT NULL,
                source_predicted_expected_r DOUBLE NULL,
                source_score_margin_top2 DOUBLE NULL,
                wave_predicted_r DOUBLE NULL,
                meta_predicted_r DOUBLE NULL,
                actual_result_r DOUBLE NULL,
                outcome VARCHAR(32) NULL,
                duplicate_suppressed TINYINT(1) NOT NULL DEFAULT 0,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                PRIMARY KEY (multi_valid_eval_run_id, setup_id, template_uid),
                INDEX idx_meta_diag_score (multi_valid_eval_run_id, meta_predicted_r),
                INDEX idx_meta_diag_symbol_time (symbol, entry_date)
            )
            """
        )
    conn.commit()


def delete_meta_diagnostics(conn, run_id: str) -> None:
    with conn.cursor() as cur:
        cur.execute(f"DELETE FROM {META_DIAGNOSTICS_TABLE} WHERE multi_valid_eval_run_id = %s", (run_id,))


def insert_meta_diagnostics(conn, rows: list[dict[str, Any]]) -> None:
    if not rows:
        return
    columns = [
        "multi_valid_eval_run_id",
        "source_ai_run_id",
        "setup_id",
        "template_uid",
        "symbol",
        "entry_date",
        "direction",
        "decision",
        "source_predicted_expected_r",
        "source_score_margin_top2",
        "wave_predicted_r",
        "meta_predicted_r",
        "actual_result_r",
        "outcome",
        "duplicate_suppressed",
    ]
    placeholders = ", ".join(["%s"] * len(columns))
    updates = ", ".join(
        [f"{column}=VALUES({column})" for column in columns if column not in {"multi_valid_eval_run_id", "setup_id", "template_uid"}]
        + ["updated_at=CURRENT_TIMESTAMP"]
    )
    with conn.cursor() as cur:
        cur.executemany(
            f"""
            INSERT INTO {META_DIAGNOSTICS_TABLE} ({", ".join(columns)})
            VALUES ({placeholders})
            ON DUPLICATE KEY UPDATE {updates}
            """,
            [tuple(wave.clean(row.get(column)) for column in columns) for row in rows],
        )


def wave_result_from_meta_row(row: pd.Series) -> wave.WaveResult:
    return wrm.wave_result_from_candidate(row)


def materialize_combined_run(
    conn,
    final_proposals: pd.DataFrame,
    meta_model: CatBoostRegressor,
    threshold: float | None,
    args: argparse.Namespace,
) -> dict[str, Any]:
    year = args.valid_year
    source_ai_run_id = wave.safe_run_id(args.source_prefix, year)
    source_run = wave.fetch_run(conn, source_ai_run_id)
    source_rows = wave.fetch_source_rows(conn, source_ai_run_id, 0)
    scored = add_meta_predictions(final_proposals, meta_model)
    scored_by_setup = {str(row.setup_id): row for row in scored.itertuples(index=False)}

    eligible = scored.copy()
    decision_by_setup: dict[str, str] = {}
    duplicate_by_setup: set[str] = set()
    if threshold is not None:
        below = eligible[eligible["meta_predicted_r"] < threshold]
        decision_by_setup.update({str(row.setup_id): "meta_score_below_threshold" for row in below.itertuples()})
        eligible = eligible[eligible["meta_predicted_r"] >= threshold]
    if not args.allow_duplicate_entry and not eligible.empty:
        eligible = eligible.sort_values(["meta_predicted_r", "wave_predicted_r", "result_r"], ascending=[False, False, False])
        duplicate_mask = eligible.duplicated(subset=["symbol", "entry_date", "direction"], keep="first")
        duplicates = eligible[duplicate_mask]
        duplicate_by_setup = {str(row.setup_id) for row in duplicates.itertuples()}
        decision_by_setup.update({setup_id: "meta_duplicate_entry_suppressed" for setup_id in duplicate_by_setup})
        eligible = eligible[~duplicate_mask]
    eligible_by_setup = {str(row.setup_id): row for row in eligible.itertuples(index=False)}

    run_id = wave.safe_run_id(args.run_prefix, year)
    source_run_id = wave.safe_run_id(args.source_run_prefix, year)
    result_table = wave.result_table_for(args.source_run_prefix, year)
    template_uid = f"tpl-{args.source_run_prefix}"
    template_name = "Market + Wave Meta V1"

    model_dir = wave.ABCD_ROOT / "model_registry" / run_id
    model_dir.mkdir(parents=True, exist_ok=True)
    meta_model.save_model(str(model_dir / "meta_model.cbm"))
    metadata_path = model_dir / "metadata.json"

    wave.ensure_result_table(conn, result_table)
    wave.ensure_diagnostics_table(conn)
    ensure_meta_diagnostics_table(conn)
    with conn.cursor() as cur:
        cur.execute("SELECT COUNT(*) AS count FROM ai_stage1_multi_valid_eval_runs WHERE multi_valid_eval_run_id = %s", (run_id,))
        exists = int(cur.fetchone()["count"]) > 0
    if exists and not args.replace_run:
        raise RuntimeError(f"Run already exists. Use --replace-run to rebuild it: {run_id}")
    wave.delete_existing(conn, run_id, source_run_id, result_table)
    delete_meta_diagnostics(conn, run_id)

    result_rows: list[dict[str, Any]] = []
    selected_rows: list[dict[str, Any]] = []
    wave_diagnostics: list[dict[str, Any]] = []
    meta_diagnostics: list[dict[str, Any]] = []
    selected_count = 0

    for _, source_row in source_rows.iterrows():
        setup_id = str(source_row.get("setup_id"))
        proposal = eligible_by_setup.get(setup_id)
        scored_proposal = scored_by_setup.get(setup_id)
        if proposal is None:
            reason = decision_by_setup.get(setup_id, "meta_no_wave_candidate")
            result = wave.no_entry(reason)
            predicted = getattr(scored_proposal, "meta_predicted_r", None) if scored_proposal is not None else None
            wave_score = getattr(scored_proposal, "wave_predicted_r", None) if scored_proposal is not None else None
        else:
            candidate = pd.Series(proposal._asdict())
            result = wave_result_from_meta_row(candidate)
            predicted = float(candidate["meta_predicted_r"])
            wave_score = float(candidate["wave_predicted_r"])
            selected_count += 1
        result_rows.append(wave.result_row(source_run_id, template_uid, source_row, result))
        selected = wave.selected_row(template_uid, template_name, source_row, result)
        selected["predicted_expected_r"] = predicted
        selected["score_margin_top2"] = wave_score
        selected_rows.append(selected)
        wave_diagnostics.append(wave.diagnostic_row(run_id, source_ai_run_id, source_run_id, template_uid, "market_wave_meta", source_row, result))
        meta_diagnostics.append(
            {
                "multi_valid_eval_run_id": run_id,
                "source_ai_run_id": source_ai_run_id,
                "setup_id": setup_id,
                "template_uid": template_uid,
                "symbol": source_row.get("symbol"),
                "entry_date": result.entry_date,
                "direction": result.trade_direction,
                "decision": "approved" if proposal is not None else decision_by_setup.get(setup_id, "meta_no_wave_candidate"),
                "source_predicted_expected_r": source_row.get("source_predicted_expected_r"),
                "source_score_margin_top2": source_row.get("source_score_margin_top2"),
                "wave_predicted_r": wave_score,
                "meta_predicted_r": predicted,
                "actual_result_r": result.result_r,
                "outcome": result.outcome,
                "duplicate_suppressed": 1 if setup_id in duplicate_by_setup else 0,
            }
        )

    wave.insert_result_rows(conn, result_table, result_rows)
    wave.insert_run_record(conn, source_run, run_id, source_run_id, result_table, year, "market_wave_meta", args)
    with conn.cursor() as cur:
        cur.execute(
            "UPDATE ai_stage1_multi_valid_eval_runs SET model_path = %s WHERE multi_valid_eval_run_id = %s",
            (str(model_dir), run_id),
        )
    wave.insert_selected_rows(conn, run_id, selected_rows)
    summary = wave.insert_filter_summaries(conn, run_id, selected_rows, "market_wave_meta_v1")
    wave.insert_diagnostics(conn, wave_diagnostics)
    insert_meta_diagnostics(conn, meta_diagnostics)

    metadata_path.write_text(
        json.dumps(
            {
                "run_id": run_id,
                "source_ai_prefix": args.source_prefix,
                "source_run_id": source_run_id,
                "valid_year": year,
                "meta_years": wave.years_from_arg(args.meta_years),
                "train_years": wave.years_from_arg(args.train_years),
                "threshold_year": args.threshold_year,
                "threshold": threshold,
                "allow_duplicate_entry": bool(args.allow_duplicate_entry),
                "meta_cat_features": META_CAT_FEATURES,
                "meta_num_features": META_NUM_FEATURES,
                "iterations": args.iterations,
                "depth": args.depth,
                "learning_rate": args.learning_rate,
                "l2_leaf_reg": args.l2_leaf_reg,
                "random_seed": args.random_seed,
                "target_clip_min": args.target_clip_min,
                "wave_target_clip_max": args.wave_target_clip_max,
                "meta_target_clip_max": args.meta_target_clip_max,
                "threshold_cap_r": args.threshold_cap_r,
                "profit_lock_trigger_r": args.profit_lock_trigger_r,
                "profit_lock_giveback_r": args.profit_lock_giveback_r,
                "profit_lock_min_r": args.profit_lock_min_r,
                "time_exit_minutes": args.time_exit_minutes,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )
    conn.commit()
    summary.update(
        {
            "year": year,
            "run_id": run_id,
            "source_run_id": source_run_id,
            "result_table": result_table,
            "model_selected": selected_count,
            "threshold": threshold,
        }
    )
    return summary


def main() -> int:
    args = parse_args()
    conn = wave.connect()
    try:
        wrm.ensure_candidate_table(conn)
        oof_meta = build_oof_meta_frame(conn, args)
        if oof_meta.empty:
            raise RuntimeError("No out-of-fold meta proposal rows were built.")
        meta_train_raw = oof_meta[oof_meta["proposal_year"] < args.threshold_year].copy()
        meta_threshold_raw = oof_meta[oof_meta["proposal_year"] == args.threshold_year].copy()
        if meta_train_raw.empty or meta_threshold_raw.empty:
            threshold = args.threshold
            label = "all" if threshold is None else f"{threshold:.3f}"
            print(f"Skipping preliminary threshold calibration; using threshold {label}.")
        else:
            print(f"Training preliminary meta model on {len(meta_train_raw):,} OOF proposal rows")
            preliminary_meta = train_meta_model(meta_train_raw, args)
            threshold_scored = add_meta_predictions(meta_threshold_raw, preliminary_meta)
            threshold = choose_threshold(pd.concat([meta_train_raw, threshold_scored], ignore_index=True), args)

        meta_train = oof_meta[oof_meta["proposal_year"] <= args.threshold_year].copy()
        if meta_train.empty:
            meta_train = oof_meta.copy()
        print(f"Training final meta model on {len(meta_train):,} OOF proposal rows")
        final_meta = train_meta_model(meta_train, args)

        train_ids = [wave.safe_run_id(args.source_prefix, year) for year in wave.years_from_arg(args.train_years)]
        valid_id = wave.safe_run_id(args.source_prefix, args.valid_year)
        train_candidates = wrm.load_candidates(conn, train_ids)
        valid_candidates = wrm.load_candidates(conn, [valid_id])
        print(f"Training final wave proposal model on {len(train_candidates):,} candidates from {args.train_years}")
        final_wave = train_wave_model(train_candidates, args)
        final_proposals = build_top_wave_proposals(valid_candidates, final_wave)
        summary = materialize_combined_run(conn, final_proposals, final_meta, threshold, args)
    except Exception:
        conn.rollback()
        raise
    finally:
        conn.close()

    label = "all" if summary["threshold"] is None else f"{summary['threshold']:.3f}"
    print(
        f"Materialized {summary['run_id']} threshold={label}: {summary['model_selected']:,} approved entries, "
        f"{summary['wins']:,} wins / {summary['losses']:,} losses / {summary['no_entries']:,} no-entry, "
        f"{summary['win_rate'] * 100:.2f}% win, {summary['avg_r']:.3f}R avg, {summary['sum_r']:.1f}R sum"
    )
    if not args.skip_api_refresh:
        wave.api_refresh(summary["run_id"], int(summary["year"]), args.api_refresh_url)
    return 0


if __name__ == "__main__":
    sys.exit(main())
