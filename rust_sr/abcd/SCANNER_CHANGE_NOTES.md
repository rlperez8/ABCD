# Scanner Change Notes

## Current Intended Architecture

The scanner should act as a pure setup discovery engine:

```text
candles
-> scanner finds XABCD setups
-> pattern_setups
-> pattern_outcomes_prop
   - outcome_model = "D"
   - outcome_model = "DReversal"
   - reversal_type
   - target metrics
   - prop_result
   - harmonic/bin/size/time/trend dimensions
-> prop_strategy_family_* rollups
```

The ranking, filtering, and family grouping should happen after raw setup discovery. The scanner should not rewrite a pattern's X/A/B/C/D pivots to make it look better.

Family rollups currently write only the aggregate UI tables: `prop_strategy_family_yearly` and `prop_strategy_family_summary`. They are concrete strategy rows with real dimension values only; wildcard `All` dimensions are not written. The per-outcome member table is deferred until the outcome data is stable enough to justify that larger drilldown index.

## Removed Scanner Mutation

The code previously had a "newer extreme" mutation path:

```rust
is_more_extreme_same_type(...)
reset_pivot_from_candle(...)
pattern.x = ...
pattern.a = ...
pattern.b = ...
pattern.c = ...
```

That behavior moved an existing X/A/B/C pivot to a later, more extreme candle of the same pivot type. It could alter candidate geometry while the scanner was still building a setup.

This has been removed. The scanner now goes back to incrementing active leg lengths as candles advance:

```rust
pattern.x.length += 1;
pattern.a.length += 1;
pattern.b.length += 1;
pattern.c.length += 1;
```

## Current Scanner-Related Changes To Know About

### X Index And X Bars Left

The scanner now tracks `x_index` through the candidate pipeline and calculates:

```text
x_bars_left
```

This does not move X/A/B/C/D pivots. It adds a ranking/filter dimension for how much room existed to the left of X.

### Optional X Bars Left Filter

The engine supports:

```text
ABCD_MAX_X_BARS_LEFT
```

If set to a positive number, patterns with `x_bars_left` greater than that value are dropped before DB writes. If unset, the default is unlimited.

### Leg Length Finalization

When candidates move between stages, some leg lengths are finalized from candle indexes:

```text
X -> XA
XA -> XAB
XAB -> XABC
XABC -> XABCD
```

This should preserve the same pivot geometry, but it can affect length-based metrics such as size bucket and time accuracy. This is worth reviewing separately if length behavior looks different from older runs.

### Target Candle Guard

The scanner now only assigns a target candle when there is another candle after the current index:

```rust
pattern.target_candle.is_none() && current_index + 1 < candles.len()
```

This can affect newest setups near the end of available candle history. It may leave the freshest setups as not target-ready.

### Expanded Accuracy And Time Dimensions

Accuracy now includes:

```text
Bat
AlternateBat
Butterfly
Gartley
Crab
DeepCrab
Shark
```

It also calculates time accuracy and assigns dominant route dimensions:

```text
harmonic_type
bin
time_bin
prop_strategy_id
```

These are ranking/grouping dimensions for outcomes and family rollups.

### Pattern IDs And Strategy IDs

Patterns now receive stable IDs after accuracy calculation:

```text
pattern_id
prop_strategy_id
```

`pattern_id` is based on canonical setup geometry. `prop_strategy_id` is based on the selected strategy dimensions.

### Write-Boundary Dedupe

Before writing `pattern_setups`, the DB layer dedupes serialized patterns by canonical setup ID.

This is a guardrail so duplicate canonical setups do not crash build-table inserts. It does not change scanner behavior; it only prevents writing the same setup twice.

## Things To Review Later

1. Confirm leg-length finalization matches the intended old scanner behavior.
2. Decide whether `ABCD_MAX_X_BARS_LEFT` should remain optional-only or become a UI/family ranking dimension only.
3. Keep DB dedupe as a guardrail, but consider adding stage-level scanner dedupe only if duplicate candidate growth becomes a speed problem.
4. Continue removing legacy split-output code:

```text
pattern_outcomes_prop_reversal
prop_reversal_strategy_*
old strategy_* caches
```
