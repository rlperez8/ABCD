# AI Model Recovery Notes

This folder is the lightweight recovery bundle for the current AI research lane.

## Current pinned model

- Main candidate: `aicw-mtf-eg1-xtight-t014-rd3-en4-v1-2m-2026`
- Metrics: 402 trades, 43.03% win rate, 222.24R sum, 5.02R max drawdown.
- Pin file: `ai_model_registry_pins.json`
- Research log: `research_notes/2026-05-30-candle-wave-edge.md`

## What is recoverable from Git

- CatBoost model artifacts under each model run directory.
- Model metadata JSON files.
- Training/scanning/materialization scripts under `rust_sr/abcd/scripts/`.
- UI/API code that reads model/trade output.
- Research notes with the run IDs, metrics, and rejected/accepted experiments.

## What is not recoverable from Git

Raw MySQL candle data and large DB-derived tables are not stored in Git. If the
entire MySQL database is deleted, restore from a MySQL dump or reload the raw
candle source first.

Minimum DB backup targets for easy recovery:

- Raw candle tables, especially `futures_contract_1m_candles`.
- Derived timeframe candles if you do not want to rebuild them.
- AI run tables:
  - `ai_candle_wave_model_runs`
  - `ai_candle_wave_candidate_rows`
  - `ai_candle_wave_selected_trades`
  - `ai_candle_wave_exit_model_runs`
  - `ai_candle_wave_exit_model_trades`

## Rebuild outline

1. Restore or reload raw `1m` candle data.
2. Rebuild needed timeframe candles, especially `2m`, `5m`, `15m`, `1h`, and `4h`.
3. Rebuild candle-wave candidates with `ai_candle_wave_scanner_model.py`.
4. Re-run the pinned scanner settings from the model metadata if selected-trade
   DB rows need to be regenerated.
5. Use `ai_model_registry_pins.json` to identify which run should be shown as
   the protected main candidate.
