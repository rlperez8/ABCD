# Confirmed Codex Work Log Since `5ba1834`

Baseline:
- Last pushed commit before this Codex-assisted work: `5ba1834` (`Snapshot before further changes`)
- Purpose of this document: list the changes Codex can confidently attribute to this conversation/work session

## Attribution Rule

This file is intentionally stricter than a normal changelog.

- Included:
  - code changes Codex directly made in this thread
  - operational data / database work Codex directly ran in this thread
  - docs and utility scripts Codex added in this thread
- Not included unless clearly confirmed:
  - unrelated local frontend/server edits that were already in the worktree
  - older UI polish from prior sessions that is not evidenced in this thread
  - user-authored changes that happened outside the specific work Codex performed here

Because of that rule, smaller UI details like “dates were added on the canvas” are **not claimed here** unless they were part of the changes Codex actually made in this session.

## 1. `rust_sr/alpha_vantage`

### 1.1 Secret / config cleanup

Codex changed `alpha_vantage` from hardcoded credentials to environment-driven configuration.

What was changed:
- Added `dotenvy` to support local `.env` loading during development.
- Removed the hardcoded Alpha Vantage API key from the ingestion flow.
- Removed the hardcoded MySQL connection string from the ingestion flow.
- Added a checked-in env template:
  - [`rust_sr/alpha_vantage/.env.example`](./rust_sr/alpha_vantage/.env.example)

Environment variables introduced:
- `ALPHA_VANTAGE_API_KEY`
- `ALPHA_VANTAGE_DATABASE_URL`

Files changed:
- [`rust_sr/alpha_vantage/Cargo.toml`](./rust_sr/alpha_vantage/Cargo.toml)
- [`rust_sr/alpha_vantage/Cargo.lock`](./rust_sr/alpha_vantage/Cargo.lock)
- [`rust_sr/alpha_vantage/src/main.rs`](./rust_sr/alpha_vantage/src/main.rs)

### 1.2 Query fix for symbol selection

Codex fixed the old `above_500_volume` query shape.

What was changed:
- Replaced the brittle `SELECT *` + `query_scalar::<_, String>` pattern with an explicit symbol select.
- Clarified the meaning of the filter so the code matches the actual data being requested.

Result:
- Symbol selection no longer depends on the first returned column from `listing_status`.

Files changed:
- [`rust_sr/alpha_vantage/src/main.rs`](./rust_sr/alpha_vantage/src/main.rs)

### 1.3 Listing status sync workflow

Codex added tooling and code paths to wipe and reload `listing_status`.

What was changed:
- Added helpers to clear the table.
- Added helpers to fetch the Alpha Vantage listing feed.
- Added helpers to insert listing rows back into MySQL.
- Added a sync-only mode so listing status can be refreshed without touching candles.

Environment / control flags added:
- `ALPHA_VANTAGE_RESET_LISTING_STATUS`
- `ALPHA_VANTAGE_SYNC_LISTING_STATUS_ONLY`

Files changed:
- [`rust_sr/alpha_vantage/src/main.rs`](./rust_sr/alpha_vantage/src/main.rs)

### 1.4 Candle backfill controls

Codex turned the ingestion flow into a controllable batch backfill instead of a single blunt run.

What was changed:
- Added wipe/reset support for `candles`.
- Added batch controls so subsets of symbols can be run.
- Added resume controls so work can continue from a later symbol.
- Added per-symbol candle deletion helpers.
- Added per-symbol candle counts for resume/cleanup logic.

Environment / control flags added:
- `ALPHA_VANTAGE_RESET_CANDLES`
- `ALPHA_VANTAGE_SYMBOL_OFFSET`
- `ALPHA_VANTAGE_SYMBOL_LIMIT`
- `ALPHA_VANTAGE_START_SYMBOL`

Files changed:
- [`rust_sr/alpha_vantage/src/main.rs`](./rust_sr/alpha_vantage/src/main.rs)

### 1.5 API error handling and rate-limit distinction

Codex improved Alpha Vantage response handling so the job can distinguish temporary failures from bad symbols.

What was changed:
- Added custom API error handling in the Alpha Vantage model layer.
- Differentiated:
  - temporary / retryable throttling
  - permanent symbol errors
  - empty-candle responses
- Prevented temporary API issues from being treated the same as permanently bad symbols.

Why it mattered:
- Without this, rate-limited or malformed transient responses could get symbols treated as broken.

Files changed:
- [`rust_sr/alpha_vantage/src/models/alpha_vantage.rs`](./rust_sr/alpha_vantage/src/models/alpha_vantage.rs)
- [`rust_sr/alpha_vantage/src/main.rs`](./rust_sr/alpha_vantage/src/main.rs)

### 1.6 Bugged-symbol workflow

Codex added a system for marking symbols as bad and skipping them later.

What was changed:
- Added `bugged` status updates in the ingestion flow.
- Added helpers to mark symbols bugged or clear that status on success.
- Changed batch selection to skip symbols already marked `bugged`.
- Treated permanent API failures and empty candle sets as reasons to mark a symbol bugged.

Files changed:
- [`rust_sr/alpha_vantage/src/main.rs`](./rust_sr/alpha_vantage/src/main.rs)

### 1.7 Minimum-history gate

Codex added a history threshold so ultra-short-history symbols do not stay in the candle universe.

What was changed:
- Added a minimum-history check during candle load.
- If a symbol failed the threshold:
  - its candle rows were deleted
  - it was marked `bugged`
  - later runs skipped it

Default introduced:
- about 3 years of history (`365 * 3`)

Environment / control flag added:
- `ALPHA_VANTAGE_MIN_HISTORY_DAYS`

Files changed:
- [`rust_sr/alpha_vantage/src/main.rs`](./rust_sr/alpha_vantage/src/main.rs)

### 1.8 Recent liquidity metric refresh

Codex added the recent average-volume refresh path used later by the scanner filter.

What was changed:
- Renamed the DB-facing field from `average_volume` to `average_volume_30d`.
- Updated the Rust listing-status model to match that new name.
- Added a refresh flow that calculates recent average volume from stored candles.
- Added lookback configuration for the refresh.
- Fixed the SQL type handling for average-volume calculation so it decoded reliably.

Environment / control flags added:
- `ALPHA_VANTAGE_REFRESH_AVG_VOLUME_ONLY`
- `ALPHA_VANTAGE_AVG_VOLUME_LOOKBACK`
- `ALPHA_VANTAGE_MIN_AVG_VOLUME`

Files changed:
- [`rust_sr/alpha_vantage/src/main.rs`](./rust_sr/alpha_vantage/src/main.rs)
- [`rust_sr/alpha_vantage/src/models/listing_status.rs`](./rust_sr/alpha_vantage/src/models/listing_status.rs)

### 1.9 Monitoring helper script

Codex added a local PowerShell helper to summarize backfill progress from logs.

What was added:
- [`rust_sr/alpha_vantage/monitor_backfill.ps1`](./rust_sr/alpha_vantage/monitor_backfill.ps1)

Purpose:
- Read stdout/stderr logs from long backfill runs.
- Write a human-readable status summary into a text file.

## 2. `rust_sr/abcd`

### 2.1 Secret / config cleanup

Codex changed `abcd` from a hardcoded DB connection to environment-driven configuration.

What was changed:
- Added `dotenvy` support.
- Removed the hardcoded MySQL URL from the main runner.

Environment variables introduced:
- `ABCD_DATABASE_URL`
- `ABCD_MIN_AVG_VOLUME`
- `ABCD_RESET_OUTPUTS`

Files changed:
- [`rust_sr/abcd/Cargo.toml`](./rust_sr/abcd/Cargo.toml)
- [`rust_sr/abcd/Cargo.lock`](./rust_sr/abcd/Cargo.lock)
- [`rust_sr/abcd/src/main.rs`](./rust_sr/abcd/src/main.rs)

### 2.2 Scanner input changed to recent liquidity filter

Codex changed the scanner so it can run only on liquid symbols based on recent average volume.

What was changed:
- Added `get_symbols_above_average_volume(...)` to load eligible symbols from `listing_status`.
- Required:
  - `status = 'Active'`
  - `bugged IS NULL OR bugged = false`
  - `average_volume_30d IS NOT NULL`
  - `average_volume_30d >= threshold`
- Joined against existing candle symbols so the scanner only runs against symbols with stored candle data.

Files changed:
- [`rust_sr/abcd/src/models/database.rs`](./rust_sr/abcd/src/models/database.rs)
- [`rust_sr/abcd/src/main.rs`](./rust_sr/abcd/src/main.rs)

### 2.3 Reset support for generated outputs

Codex added a clean-start option for scanner outputs.

What was changed:
- Added `clear_generated_outputs()` to truncate:
  - `xabcd_patterns`
  - `accuracies`
- Added `ABCD_RESET_OUTPUTS` handling in `main`.

Files changed:
- [`rust_sr/abcd/src/models/database.rs`](./rust_sr/abcd/src/models/database.rs)
- [`rust_sr/abcd/src/main.rs`](./rust_sr/abcd/src/main.rs)

### 2.4 Batched DB writes

Codex replaced row-by-row inserts with chunked batch inserts.

What was changed:
- `insert_patterns()` now uses `sqlx::QueryBuilder` and chunked transactions.
- `insert_scatter_plot()` now uses `sqlx::QueryBuilder` and chunked transactions.
- Added empty-slice guards so no-op flushes return immediately.

Why it mattered:
- This cut a large amount of DB round-trip overhead during big scanner runs.

Files changed:
- [`rust_sr/abcd/src/models/database.rs`](./rust_sr/abcd/src/models/database.rs)

### 2.5 Main scanner runtime refactor

Codex refactored the scanner so it no longer waits until the very end to do all persistence.

What was changed:
- Added `scan_symbol(...)` as a dedicated per-symbol scan path.
- Added bounded parallel symbol scanning with `JoinSet`.
- Added `flush_pending_patterns(...)` so finished work gets written incrementally.
- Replaced the old giant `all_patterns` accumulation approach.
- Added progress output for completed symbols and DB flushes.

Environment variables introduced:
- `ABCD_SCAN_CONCURRENCY`
- `ABCD_WRITE_BATCH_SIZE`

Files changed:
- [`rust_sr/abcd/src/main.rs`](./rust_sr/abcd/src/main.rs)

### 2.6 Support/resistance optimization

Codex optimized the support/resistance calculation.

What was changed:
- Added an empty-candle guard.
- Added a zero-range guard.
- Reworked the score loop so it only updates nearby ticks around each candle low instead of comparing every tick to every candle.
- Added tick preallocation.

Why it mattered:
- This was one of the heavier repeated calculations in the scanner path.

Files changed:
- [`rust_sr/abcd/src/models/support_and_resistance.rs`](./rust_sr/abcd/src/models/support_and_resistance.rs)

### 2.7 Clone / allocation reduction in hot structs

Codex reduced memory churn in the scanner internals without changing the detection rules.

What was changed:
- `Pivot` now stores `NaiveDate` instead of an owned date string.
- `Trade` now stores `NaiveDate` instead of an owned date string.
- Removed the owned symbol string from `Trade`.
- Made intermediate pattern structs copyable where safe.
- Changed `PatternAccuracy` to store `HarmonicType` instead of an owned string label.
- Added `Default` for `HarmonicType`.
- Changed `PatternXABCD.symbol` to shared `Arc<str>` storage.
- Moved string formatting to the DB serialization edge.

Files changed:
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

### 2.8 Candle loading cleanup

Codex cleaned up part of the candle fetch path.

What was changed:
- Changed candle SQL ordering to `ORDER BY date` directly.
- Removed the extra reverse step after fetch.

Files changed:
- [`rust_sr/abcd/src/models/database.rs`](./rust_sr/abcd/src/models/database.rs)

## 3. Database / Operational Work Codex Directly Ran

These actions were performed by Codex during this thread and are part of the real work, even though they are not all represented as source diffs.

### 3.1 Listing status reload

Codex:
- wiped `listing_status`
- reloaded it from Alpha Vantage

Observed result during the run:
- `13,252` listing-status rows were loaded

### 3.2 Candle reset and chunked backfill

Codex:
- wiped `candles`
- ran the initial candle backfill in chunks
- added the 3-year history screen to prevent ultra-short datasets from surviving

Observed examples during the run:
- symbols like `AAOX` with only a handful of candles were filtered out and marked `bugged`

### 3.3 Resume handling for interrupted backfill

Codex:
- restarted a stopped candle run from the point it left off
- avoided blindly duplicating earlier loaded symbols
- cleaned up the partially loaded restart symbol before reloading it

### 3.4 Liquidity refresh

Codex:
- refreshed `average_volume_30d` from recent candles
- used that metric to determine which symbols were above the recent liquidity threshold

Observed result during the run:
- `8,693` symbols got a refreshed value
- `3,251` were above the `500,000` threshold at that time

### 3.5 Filtered `abcd` run

Codex:
- ran the `xabcd` flow against the filtered recent-volume universe
- later stopped the lingering process when it appeared to be stuck near the end
- rewired the runner so you could rerun it yourself with env vars

## 4. Documentation / Tracking Work

Codex created and updated this tracking document so project work could be audited later.

What was done:
- added the first project-change log
- then revised it after you asked for broader coverage
- now narrowed it again so it reflects **confirmed Codex work only**

File:
- [`CODEX_CHANGES_SINCE_5ba1834.md`](./CODEX_CHANGES_SINCE_5ba1834.md)

## 5. Things Codex Explicitly Reviewed But Did Not Fully Change

These came up in the review/conversation, but were not fully resolved in this session.

- In `abcd`, Codex flagged the `cd_bc_price_retracement` denominator guard in [`rust_sr/abcd/src/models/trade.rs`](./rust_sr/abcd/src/models/trade.rs) as a likely correctness bug.
- Codex flagged `pattern_group_id` uniqueness as fragile.
- Codex noted that exact float equality in the `three_month` support/resistance touch check is brittle.
- Codex suggested DB indexes for faster downstream queries, but did not add or migrate them in code here.

## 6. Not Claimed Here

To keep this honest, the following are **not** claimed in this file unless they are later verified:

- older frontend/UI tweaks from outside this thread
- canvas/date-label changes not performed in this session
- unrelated server/frontend refactors already sitting in the local worktree
- user-authored edits that were not part of the Codex changes above
