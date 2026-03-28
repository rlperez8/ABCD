# Project Change Log Since `5ba1834`

Baseline:
- Last pushed commit before this wave of work: `5ba1834` (`Snapshot before further changes`)
- Scope of this document: the observable project changes and data-side operational work that happened after that point, across frontend, server, Rust jobs, and database workflow

## 1. Frontend (`client`)

### App structure reorganization
- The frontend moved away from a flat `client/src` layout into a feature-oriented structure:
  - `client/src/app`
  - `client/src/components`
  - `client/src/features`
  - `client/src/services`
  - `client/src/styles`
  - `client/src/utils`
- [`client/src/index.js`](./client/src/index.js) now boots the app through [`client/src/app/App.js`](./client/src/app/App.js) and the shared stylesheet at [`client/src/styles/index.css`](./client/src/styles/index.css).

### Research / dashboard UI shape
- The new [`client/src/app/App.js`](./client/src/app/App.js) is organized around three stations:
  - Current Setups
  - Research
  - Strategies
- The app now coordinates:
  - table pagination
  - current setup selection
  - setup comparison loading
  - dashboard bin selection
  - strategy ranking / workbench views
  - candle chart updates
- The frontend now depends on a shared API layer in [`client/src/services/patternApi.js`](./client/src/services/patternApi.js) instead of the older scattered route helpers.

### Feature modules
- Dashboard-related visualizations and metric helpers now live under:
  - [`client/src/features/dashboard`](./client/src/features/dashboard)
- Candle-chart behavior was broken into a feature module with internal helpers under:
  - [`client/src/features/candle-chart`](./client/src/features/candle-chart)
- Strategy views and strategy definitions now live under:
  - [`client/src/features/strategies`](./client/src/features/strategies)

### Package / dependency changes
- [`client/package.json`](./client/package.json) and [`client/package-lock.json`](./client/package-lock.json) changed.
- `react-window` is now included.
- `cross-env` is used for the build script.
- The build script is now `cross-env CI=false react-scripts build`.

### Legacy client cleanup visible in the diff
- A large number of older one-file components, chart helpers, docs assets, and legacy CSS files under the old `client/src` structure were removed or replaced by the newer feature layout.
- The generated docs bundle and old font assets under `client/src/docs` were removed from the active tree.

## 2. Rust Server (`rust_sr/server`)

### Server-side API expansion
- [`rust_sr/server/src/main.rs`](./rust_sr/server/src/main.rs) now exposes richer Rust-backed endpoints including:
  - `/patterns`
  - `/accuracy`
  - `/setup-comparison`
  - `/candles`
- The `/patterns` handler now supports:
  - harmonic-type filtering
  - market filtering
  - trade-result filtering
  - retracement filtering
  - recent-days filtering
  - limit / offset pagination
  - total-count and `has_more` responses
- The `/accuracy` handler now builds aggregated accuracy bins directly from `xabcd_patterns`.
- The `/setup-comparison` handler now returns cohort-level summary stats plus recent examples.

### Pattern payload updates
- [`rust_sr/server/src/pattern.rs`](./rust_sr/server/src/pattern.rs) reflects the richer pattern rows now stored in MySQL, including:
  - harmonic accuracy columns
  - pattern group id
  - date-typed pattern and trade fields

### Legacy Python server cleanup visible in the diff
- The older Python server/storage files under:
  - `rust_sr/server/server.py`
  - `rust_sr/server/storage.py`
  were removed from the current project diff in favor of the Rust server path being the active backend surface.

## 3. `rust_sr/alpha_vantage`

### Configuration and secrets
- Replaced hardcoded Alpha Vantage and MySQL credentials with environment-driven config.
- Added `dotenvy` support so local `.env` files can be used in development.
- Added [`rust_sr/alpha_vantage/.env.example`](./rust_sr/alpha_vantage/.env.example).

Files:
- [`rust_sr/alpha_vantage/Cargo.toml`](./rust_sr/alpha_vantage/Cargo.toml)
- [`rust_sr/alpha_vantage/Cargo.lock`](./rust_sr/alpha_vantage/Cargo.lock)
- [`rust_sr/alpha_vantage/src/main.rs`](./rust_sr/alpha_vantage/src/main.rs)

### Listing status sync and candle backfill controls
- Added helpers to wipe and reload `listing_status`.
- Added helpers to wipe `candles`, delete per-symbol candles, count per-symbol candles, and update `bugged` flags.
- Added environment-controlled modes for:
  - listing-status-only sync
  - candle reset
  - symbol offset / limit batching
  - start-symbol resume
  - refresh-only recent average volume
  - minimum candle-history requirement
- Changed the candle backfill flow to read active symbols from `listing_status` instead of relying on the older precomputed volume filter.

### API handling and bad-symbol detection
- Added custom Alpha Vantage response/error handling to distinguish:
  - temporary throttling / retryable failures
  - permanent per-symbol API failures
  - empty candle sets
- Added symbol-level `bugged` handling so unusable symbols can be skipped on later runs.
- Added a minimum-history gate so short-history symbols are rejected and marked `bugged`.

### Recent liquidity metric
- Fixed the scalar query used for the older volume filter flow.
- Renamed the DB-facing liquidity field from `average_volume` to `average_volume_30d`.
- Added recent-volume refresh logic based on the latest candle window.
- Added config for recent-volume lookback and minimum-volume threshold.

### Monitoring utility
- Added a PowerShell watcher script to summarize long-running backfill log progress into a text file:
  - [`rust_sr/alpha_vantage/monitor_backfill.ps1`](./rust_sr/alpha_vantage/monitor_backfill.ps1)

## 4. `rust_sr/abcd`

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

### Database write performance
- Replaced row-by-row inserts into `xabcd_patterns` with chunked batch inserts using `sqlx::QueryBuilder`.
- Replaced row-by-row inserts into `accuracies` with chunked batch inserts.
- Added empty-batch guards so flushes return early when there is nothing to write.

### Scanner runtime and memory behavior
- Refactored the main scan so symbol scans can run with bounded parallelism.
- Moved per-symbol scanning into a worker flow and added incremental flushes instead of waiting until the end of the full run.
- Removed the old single giant `all_patterns` accumulation step.
- Added progress logging for symbol completion and DB flushes.

### Hot-path data layout improvements
- Made intermediate pattern structs lighter and copyable where safe.
- Changed `Pivot` to store `NaiveDate` instead of owned date strings.
- Changed `Trade` to store `NaiveDate` and removed its owned symbol string.
- Changed `PatternAccuracy` to store `HarmonicType` instead of an owned label string.
- Changed finished pattern symbols to shared `Arc<str>` storage.
- Moved string formatting to the serialization / DB-write edge instead of the scan hot path.

### Support/resistance performance
- Added empty-candle and zero-range guards.
- Reworked scoring to update only nearby ticks around each candle low instead of scanning every tick against every candle.
- Added tick-buffer preallocation.

## 5. Database / Data Workflow

These steps were part of the working process even though they are not fully represented by tracked source files:

- Wiped and reloaded `listing_status` from Alpha Vantage.
- Wiped and backfilled `candles` in chunks.
- Added and used the `bugged` flow to exclude empty, bad, or too-short-history symbols.
- Enforced a default minimum history threshold of about 3 years during candle backfill.
- Renamed the working liquidity column in MySQL to `average_volume_30d`.
- Refreshed `average_volume_30d` from recent candles.
- Ran filtered `xabcd` scans using recent-volume thresholds.

## 6. Legacy Python / Artifact Cleanup Visible In The Diff

### Python cleanup
- The current diff since `5ba1834` removes older Python-oriented files including:
  - `engine/main.py`
  - `engine/storage.py`
  - `sr.py`
  - older Python server helpers under `rust_sr/server`

### Bytecode / ignore hygiene
- [`/.gitignore`](./.gitignore) now explicitly ignores:
  - `__pycache__/`
  - `*.pyc`
- Existing checked-in Python bytecode artifacts show up as removed in the repo diff after that cleanup.

## 7. Notes

- This document now tracks the broader project state since `5ba1834`, not only the Rust ingestion/scanner work.
- Some items above are code changes visible in the repository diff; others are operational data/database steps carried out during development.
- Temporary runtime logs such as backfill stdout/stderr snapshots were intentionally left out of the tracked change set.
