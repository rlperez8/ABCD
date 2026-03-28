# Codex Change Log Since `5ba1834`

Baseline:
- Last pushed commit before this Codex-assisted work: `5ba1834` (`Snapshot before further changes`)
- Scope of this document: Rust crates and related operational work that Codex helped with after that push

## 1. `rust_sr/alpha_vantage`

### Configuration and secrets
- Replaced hardcoded Alpha Vantage and MySQL credentials with environment-driven config.
- Added `dotenvy` support so local `.env` files can be used in development.
- Added [`rust_sr/alpha_vantage/.env.example`](./rust_sr/alpha_vantage/.env.example).

Files:
- [`rust_sr/alpha_vantage/Cargo.toml`](./rust_sr/alpha_vantage/Cargo.toml)
- [`rust_sr/alpha_vantage/Cargo.lock`](./rust_sr/alpha_vantage/Cargo.lock)
- [`rust_sr/alpha_vantage/src/main.rs`](./rust_sr/alpha_vantage/src/main.rs)

### Listing status sync and backfill controls
- Added helpers to wipe and reload `listing_status`.
- Added helpers to wipe `candles`, delete per-symbol candles, count per-symbol candles, and update `bugged` flags.
- Added environment-controlled modes for:
  - listing-status-only sync
  - candle reset
  - symbol offset / limit batching
  - start-symbol resume
  - refresh-only recent average volume
  - minimum candle-history requirement
- Changed the candle backfill flow to read active symbols from `listing_status` instead of relying on the old prefilter query alone.

Files:
- [`rust_sr/alpha_vantage/src/main.rs`](./rust_sr/alpha_vantage/src/main.rs)

### API handling and bad-symbol detection
- Added custom Alpha Vantage response/error handling to distinguish:
  - temporary throttling / retryable failures
  - permanent per-symbol API failures
  - empty candle sets
- Added symbol-level `bugged` handling so bad or unusable symbols can be skipped on later runs.
- Added history gating so symbols with too little history are rejected and marked `bugged`.

Files:
- [`rust_sr/alpha_vantage/src/models/alpha_vantage.rs`](./rust_sr/alpha_vantage/src/models/alpha_vantage.rs)
- [`rust_sr/alpha_vantage/src/main.rs`](./rust_sr/alpha_vantage/src/main.rs)

### Recent liquidity metric
- Fixed the scalar query used for the older volume filter flow.
- Renamed the DB-facing liquidity field from `average_volume` to `average_volume_30d`.
- Added recent-volume refresh logic based on the latest candle window.
- Added config for recent-volume lookback and minimum-volume threshold.

Files:
- [`rust_sr/alpha_vantage/src/main.rs`](./rust_sr/alpha_vantage/src/main.rs)
- [`rust_sr/alpha_vantage/src/models/listing_status.rs`](./rust_sr/alpha_vantage/src/models/listing_status.rs)

### Local monitoring utility
- Added a small PowerShell watcher script to summarize long-running backfill log progress into a text file.

File:
- [`rust_sr/alpha_vantage/monitor_backfill.ps1`](./rust_sr/alpha_vantage/monitor_backfill.ps1)

## 2. `rust_sr/abcd`

### Configuration and filtered symbol selection
- Replaced the hardcoded DB URL with env-based config.
- Added support for:
  - `ABCD_DATABASE_URL`
  - `ABCD_MIN_AVG_VOLUME`
  - `ABCD_RESET_OUTPUTS`
  - `ABCD_SCAN_CONCURRENCY`
  - `ABCD_WRITE_BATCH_SIZE`
- Changed symbol selection to pull only active, non-bugged symbols whose `listing_status.average_volume_30d` meets the requested threshold.
- Added output-reset support to truncate generated pattern tables before a run when desired.

Files:
- [`rust_sr/abcd/Cargo.toml`](./rust_sr/abcd/Cargo.toml)
- [`rust_sr/abcd/Cargo.lock`](./rust_sr/abcd/Cargo.lock)
- [`rust_sr/abcd/src/main.rs`](./rust_sr/abcd/src/main.rs)
- [`rust_sr/abcd/src/models/database.rs`](./rust_sr/abcd/src/models/database.rs)

### Database write performance
- Replaced row-by-row inserts into `xabcd_patterns` with chunked batch inserts using `sqlx::QueryBuilder`.
- Replaced row-by-row inserts into `accuracies` with chunked batch inserts.
- Added empty-batch guards so flushes return early when there is nothing to write.

Files:
- [`rust_sr/abcd/src/models/database.rs`](./rust_sr/abcd/src/models/database.rs)

### Scanner runtime and memory behavior
- Refactored the main scan so symbol scans can run with bounded parallelism.
- Moved per-symbol scanning into a worker flow and added incremental flushes instead of waiting until the end of the full run.
- Removed the old single giant `all_patterns` accumulation step.
- Added progress logging for symbol completion and DB flushes.

Files:
- [`rust_sr/abcd/src/main.rs`](./rust_sr/abcd/src/main.rs)

### Hot-path data layout improvements
- Made intermediate pattern structs lighter and copyable where safe.
- Changed `Pivot` to store `NaiveDate` instead of owned date strings.
- Changed `Trade` to store `NaiveDate` and removed its owned symbol string.
- Changed `PatternAccuracy` to store `HarmonicType` instead of an owned label string.
- Changed finished pattern symbols to shared `Arc<str>` storage.
- Moved string formatting to the serialization / DB-write edge instead of the scan hot path.

Files:
- [`rust_sr/abcd/src/models/pivot.rs`](./rust_sr/abcd/src/models/pivot.rs)
- [`rust_sr/abcd/src/models/trade.rs`](./rust_sr/abcd/src/models/trade.rs)
- [`rust_sr/abcd/src/models/accuracy.rs`](./rust_sr/abcd/src/models/accuracy.rs)
- [`rust_sr/abcd/src/models/harmonic_types.rs`](./rust_sr/abcd/src/models/harmonic_types.rs)
- [`rust_sr/abcd/src/models/pattern_x.rs`](./rust_sr/abcd/src/models/pattern_x.rs)
- [`rust_sr/abcd/src/models/pattern_a.rs`](./rust_sr/abcd/src/models/pattern_a.rs)
- [`rust_sr/abcd/src/models/pattern_ab.rs`](./rust_sr/abcd/src/models/pattern_ab.rs)
- [`rust_sr/abcd/src/models/pattern_abc.rs`](./rust_sr/abcd/src/models/pattern_abc.rs)
- [`rust_sr/abcd/src/models/pattern_abcd.rs`](./rust_sr/abcd/src/models/pattern_abcd.rs)
- [`rust_sr/abcd/src/models/database.rs`](./rust_sr/abcd/src/models/database.rs)
- [`rust_sr/abcd/src/models/xabcd_csv.rs`](./rust_sr/abcd/src/models/xabcd_csv.rs)

### Support/resistance performance
- Added empty-candle and zero-range guards.
- Reworked scoring to update only nearby ticks around each candle low instead of scanning every tick against every candle.
- Added tick-buffer preallocation.

File:
- [`rust_sr/abcd/src/models/support_and_resistance.rs`](./rust_sr/abcd/src/models/support_and_resistance.rs)

## 3. Operational / Database Work Codex Helped With

These actions were performed locally and are not fully represented by git-tracked files:

- Wiped and reloaded `listing_status` from Alpha Vantage.
- Wiped and backfilled `candles` in chunks.
- Added and used the `bugged` flow to exclude empty / bad / too-short-history symbols.
- Enforced a default minimum history threshold of about 3 years during candle backfill.
- Renamed the working liquidity column in MySQL to `average_volume_30d`.
- Refreshed `average_volume_30d` from recent candles.
- Ran filtered `xabcd` scans using recent volume thresholds.

## 4. Notes

- This log is intended to track Codex-assisted work since `5ba1834`; it is not a full changelog for unrelated frontend, server, or user-authored workspace edits.
- Temporary logs such as backfill stdout/stderr snapshots were intentionally left out of the planned commit.
