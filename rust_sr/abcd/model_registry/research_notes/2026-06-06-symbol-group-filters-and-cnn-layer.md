# Symbol Group Filter Research And CNN Layer Notes

Date: 2026-06-06

## Source Outputs

Detailed output files:

`rust_sr/abcd/model_registry/prototype-live-stage2-terminal-runner-symbol-group-research`

Key files:

- `best_symbol_group_summary.json`
- `individual_root_results.csv`
- `root_group_exhaustive_results.csv`
- `best_score_to_dd_2025_monthly.csv`
- `best_score_to_dd_2026_monthly.csv`
- `best_worst_pnl_2025_monthly.csv`
- `best_worst_pnl_2026_monthly.csv`
- `small_core_2025_monthly.csv`
- `small_core_2026_monthly.csv`

## Current Validation Scope

This pass only used terminal-stage rows available for 2025 and 2026.

Before treating any symbol group as final, generate the same terminal-stage rows for 2021-2024 and rerun the exact same symbol group tests without changing the rules.

## Fixed Rule Used

The symbol groups were tested under one global rule:

- Minimum score: `0.93`
- Risk ticks: `48` to `192`
- Max open positions: `3`
- Daily stop: after `2` closed losses in the same day
- Directions: both long and short for selected roots
- Hours: all hours

No root-direction picking and no hour filter were used in this pass.

## Best Clean Group So Far

Roots:

`EMD, HO, NG, NQ, RTY, YM`

Results:

| Year | PnL | Drawdown | Trades | Sum R |
| --- | ---: | ---: | ---: | ---: |
| 2025 | `$1,228.26` | `$167.62` | 153 | `88.62R` |
| 2026 | `$781.02` | `$137.28` | 144 | `63.41R` |

Combined PnL: `$2,009.28`

Read: this is the most defensible group from the current pass because it had the best risk-adjusted robustness across both available years.

## Best Worst-Year PnL Group

Roots:

`EMD, HO, NG, NQ, RB, RTY, YM`

Results:

| Year | PnL | Drawdown | Trades | Sum R |
| --- | ---: | ---: | ---: | ---: |
| 2025 | `$959.98` | `$181.52` | 169 | `75.92R` |
| 2026 | `$848.39` | `$149.29` | 155 | `67.30R` |

Combined PnL: `$1,808.37`

Read: this has the best worst-year PnL, but `RB` is not clean by itself, so this group may be more diversification than pure symbol edge.

## Smaller Core Group

Roots:

`HO, NQ, RTY, YM`

Results:

| Year | PnL | Drawdown | Trades | Sum R |
| --- | ---: | ---: | ---: | ---: |
| 2025 | `$950.77` | `$263.60` | 145 | `74.53R` |
| 2026 | `$667.76` | `$160.61` | 131 | `56.30R` |

Combined PnL: `$1,618.53`

Read: usable as a smaller group, but it had a weaker drawdown profile than the 6-root group.

## Individual Root Read

Strongest repeat roots:

- `NQ`
- `HO`

Low-confidence or avoid for now:

- `ES`
- `CL`
- `GF`
- `LE`
- `NKD`

Mixed:

- `RB` helped one group but was negative individually in 2025.
- `RTY` and `YM` were weak individually but helped some group paths, likely through diversification.

## Next Validation Steps

1. Generate terminal-stage rows for 2021-2024 using the same live-style logic.
2. Rerun the exact same symbol-group scan with no rule changes.
3. Compare whether the same root groups survive across 2021-2026.
4. Only then promote a root group into a main candidate.

## CNN Layer Notes

CNN should not be used as the first market-wide scanner.

Best use:

- Use tree models first to scan every symbol quickly.
- Only send high-quality candidate events into CNN.
- CNN acts as a visual confirmation layer, not the first gate.

Why:

- Running CNN on every candle for every symbol live can slow the system a lot, especially on CPU.
- Running CNN only on filtered candidates should be much more realistic.
- The live target is to finish each full market scan before the next 2-minute candle.

Recommended design:

1. Stage 1 and Stage 2 tree models scan all roots on the selected timeframe.
2. Symbol/root group filter applies.
3. CNN receives only candidate windows that passed the fast filters.
4. Final manager combines tree score, CNN score, risk distance, current account state, and open trade state.

Open benchmark task:

- Measure CNN candidate inference time on CPU and GPU.
- Test 10, 100, and 500 candidate windows.
- Confirm worst-case full live cycle stays under 2 minutes.

