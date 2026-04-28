# Engine Speed Handoff

This note summarizes the speed-focused engine work from the recent chat.

## Current Direction

The engine is moving to a single canonical prop outcome fact table.

The active output path is:

```text
pattern_setups
pattern_outcomes_prop
prop_strategy_family_yearly
prop_strategy_family_summary
```

`pattern_outcomes_prop` stores both setup outcome routes:

```text
outcome_model = "D"
outcome_model = "DReversal"
```

The following outputs still exist, but are legacy or optional research routes:

```text
pattern_harmonic_scores
pattern_outcomes_swing
xabcd_patterns
```

The old split reversal table is legacy and should not be used for new work:

```text
pattern_outcomes_prop_reversal
```

## Main Design Decisions

1. `pattern_setups` remains the core raw detected XABCD setup table.
2. `pattern_outcomes_prop` is the canonical outcome table for family rollups.
3. Outcome rows store their own dominant harmonic fields:
   - `harmonic_type`
   - `bin`
   - `time_bin`
   - prop strategy id
   - outcome model
4. `pattern_harmonic_scores` is no longer required for normal runs because the strategy routes use only the dominant harmonic.
5. The engine now supports build-table rebuilds:
   - write into `_build` tables
   - rebuild indexes on build tables
   - swap build tables into final table names
6. The build-table swap keeps final tables untouched until the run is complete.
7. Index rebuild is now route-aware, so inactive optional tables do not have their indexes rebuilt.
8. Family rollups are concrete strategy rows only. The engine no longer writes wildcard `All` dimensions into `prop_strategy_family_yearly` or `prop_strategy_family_summary`.

## Environment Flags

Default family-outcome run:

```powershell
$env:ABCD_USE_BUILD_TABLES='true'; $env:ABCD_SYMBOL_LIMIT='10'; $env:ABCD_WRITE_BATCH_SIZE='25000'; $env:ABCD_SCAN_CONCURRENCY='8'; cargo run --bin abcd
```

Build-table mode:

```powershell
$env:ABCD_USE_BUILD_TABLES='true'
```

Optional outputs:

```powershell
$env:ABCD_WRITE_HARMONIC_SCORES='true'
$env:ABCD_WRITE_SWING_OUTCOMES='true'
$env:ABCD_WRITE_XABCD_MIRROR='true'
```

Unset/false means those optional tables are swapped in empty by design.

## Legacy 10-Symbol Output Counts

These counts came from the older split-table prop-reversal route and are no longer the target architecture:

```text
xabcd_patterns:                  0
pattern_setups:                  168,086
pattern_harmonic_scores:         0
pattern_outcomes_swing:          0
pattern_outcomes_prop:           0
pattern_outcomes_prop_reversal:  22,519
```

## Timing Progression

Approximate 10-symbol timings as routes were narrowed:

```text
All scores + swing + prop:    ~48.52s
Scores off, swing on:         ~30.79s
Scores off, swing off:        ~24.00s
Prop reversal only:           ~18.02s
```

Latest 10-symbol prop-reversal-only run:

```text
Execution time:               18.02s
recreate_rebuild_indexes:      2.43s
swap_build_tables:             0.40s
```

100-symbol run before prop route was made optional:

```text
Execution time:               180.97s
recreate_rebuild_indexes:      38.29s
```

That 100-symbol result showed index rebuild and `write_pattern_setups` were the main costs.

## Important Files Changed

Main engine:

```text
rust_sr/abcd/src/main.rs
```

Database/write path:

```text
rust_sr/abcd/src/models/database.rs
```

Timing helper added:

```text
rust_sr/abcd/src/bin/latest_engine_phase_timings.rs
```

Handoff note:

```text
rust_sr/abcd/ENGINE_SPEED_HANDOFF.md
```

## Specific Code Changes

### Build Tables

Added build-table flow:

```text
recreate_build_output_tables()
swap_build_output_tables()
drop_build_rebuild_secondary_indexes(...)
recreate_build_rebuild_secondary_indexes(...)
```

The engine writes to:

```text
pattern_setups_build
pattern_harmonic_scores_build
pattern_outcomes_swing_build
pattern_outcomes_prop_build
xabcd_patterns_build, if mirror enabled
```

Then swaps them into final table names.

### Output Flags

Added route/output flags:

```text
ABCD_WRITE_HARMONIC_SCORES
ABCD_WRITE_SWING_OUTCOMES
ABCD_WRITE_XABCD_MIRROR
```

`pattern_outcomes_prop` is the canonical outcome table and is always written by the current engine path.

### Route-Aware Index Rebuild

Added `OutputWriteOptions`:

```rust
pub struct OutputWriteOptions {
    pub write_harmonic_scores: bool,
    pub write_swing_outcomes: bool,
    pub write_prop_outcomes: bool,
    pub write_xabcd_mirror: bool,
}
```

Index rebuild now skips inactive optional tables:

```text
pattern_harmonic_scores       skipped unless ABCD_WRITE_HARMONIC_SCORES=true
pattern_outcomes_swing        skipped unless ABCD_WRITE_SWING_OUTCOMES=true
xabcd_patterns                skipped unless ABCD_WRITE_XABCD_MIRROR=true
```

Always active:

```text
pattern_setups
pattern_outcomes_prop
```

### Harmonic Scores

`pattern_harmonic_scores` is still available, but default off.

Reason:

```text
The active routes use the dominant harmonic fields already stored in route outcomes.
All 7 harmonic score rows are useful for research/audit, but not required for normal family-outcome testing.
```

Also removed/reduced unnecessary harmonic score indexing:

```text
idx_pattern_harmonic_scores_setup_price
```

The unique key remains:

```text
uniq_pattern_harmonic_score (setup_id, harmonic_type)
```

### Swing Outcomes

`pattern_outcomes_swing` is now default off.

Enable with:

```powershell
$env:ABCD_WRITE_SWING_OUTCOMES='true'
```

### Prop Outcomes

`pattern_outcomes_prop` is the only current outcome fact table. Plain D rows use `outcome_model = "D"` and reversal-after-D rows use `outcome_model = "DReversal"`.

## Timing Queries

Latest run detail:

```sql
SELECT
    run_id,
    symbol,
    phase,
    row_count,
    duration_ms,
    ROUND(duration_ms / 1000, 2) AS seconds,
    note,
    created_at
FROM engine_phase_timings
WHERE run_id = (
    SELECT run_id
    FROM engine_phase_timings
    ORDER BY created_at DESC
    LIMIT 1
)
ORDER BY id;
```

Grouped phase summary:

```sql
SELECT
    phase,
    COUNT(*) AS phase_rows,
    SUM(COALESCE(row_count, 0)) AS total_rows,
    SUM(duration_ms) AS total_duration_ms,
    ROUND(SUM(duration_ms) / 1000, 2) AS total_seconds,
    ROUND(AVG(duration_ms) / 1000, 2) AS avg_seconds
FROM engine_phase_timings
WHERE run_id = (
    SELECT run_id
    FROM engine_phase_timings
    ORDER BY created_at DESC
    LIMIT 1
)
GROUP BY phase
ORDER BY total_duration_ms DESC;
```

Helper binary:

```powershell
cargo run --bin latest_engine_phase_timings
```

Table count helper:

```powershell
cargo run --bin inspect_pattern_mode_tables
```

## Current Bottleneck

`write_pattern_setups` is now the honest main cost.

Example:

```text
write_pattern_setups    56,573 rows    ~2.8s
```

That phase writes the core wide XABCD setup table:

```text
setup ids
symbol / market
x/a/b/c/d OHLC
x/a/b/c/d lengths
leg price lengths
min/max values
full pattern length
reversal booleans
trend flags
```

It is inserting roughly 55-60 columns per row. For 56k rows, that is about 3 million bound values.

## Possible Future Speed Work

Stop here for now unless speed becomes a blocker.

Future options:

1. Make `pattern_setups` thinner.
2. Remove `prop_strategy_id` from `pattern_setups` because strategy ids belong in route tables.
3. Test larger `PATTERN_INSERT_CHUNK_SIZE` values.
4. Use MySQL `LOAD DATA` bulk loading for `pattern_setups`.
5. Split raw setup persistence from active route persistence if strategy runs only need a smaller setup reference table.

## Verification Done

Commands run successfully:

```powershell
cargo fmt
cargo check --bin abcd
cargo run --bin abcd
cargo run --bin inspect_pattern_mode_tables
cargo run --bin latest_engine_phase_timings
```

Known compile warnings remain existing dead-code warnings.

## Git Note

No push was made in this chat.

The worktree contains many unrelated changes outside this speed work, so review/stage carefully before committing.
