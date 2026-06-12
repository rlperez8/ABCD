# Profit Push Symbol Group Research

Date: 2026-06-06

## Goal

Search for higher profit / higher R symbol groups while keeping drawdown manageable.

CNN was intentionally skipped for this pass. This research only uses the fast terminal-stage rows and global live-style filters.

## Source Outputs

Detailed files:

`rust_sr/abcd/model_registry/prototype-live-stage2-terminal-runner-profit-push-symbol-groups`

Important outputs:

- `phase1_curated_param_search.csv`
- `phase1_summary.json`
- `phase2_exhaustive_root_groups_leading_params.csv`
- `phase2_summary.json`
- `phase2_top_worst_pnl_dd250.csv`
- `phase2_top_worst_pnl_dd350.csv`
- `phase2_top_both_pnl_dd350.csv`
- `phase2_best_score_to_dd.csv`

## Validation Scope

This pass uses available terminal-stage rows for:

- 2025
- 2026

This is not full-history proof yet. The same terminal rows still need to be generated for 2021-2024 before promoting one of these groups to main.

## Shared Assumptions

- Starting cash: `$1,000`
- Risk: `1%` of current cash per trade
- Slippage: `3 ticks entry + 3 ticks exit`
- Directions: both long and short
- Entry hours: all hours
- Symbol overlap: off
- Filters are global, not year-specific

## Best Balanced Push Candidate

Roots:

`CL, EMD, ES, HO, NQ, YM`

Rules:

- Minimum score: `0.915`
- Risk ticks: `60-240`
- Max open positions: `2`
- Stop taking new trades for the day after `1` closed loss

Results:

| Year | PnL | Drawdown | Sum R |
| --- | ---: | ---: | ---: |
| 2025 | `$806.90` | `$275.88` | `68.72R` |
| 2026 | `$843.76` | `$233.12` | `66.79R` |

Combined PnL: `$1,650.66`

Read: best current balance between strong repeat profit and manageable drawdown under the aggressive search. Needs earlier-year validation because `CL` and `ES` were weak individually in earlier symbol-only checks.

## Best Higher Profit Candidate

Roots:

`ES, HO, NQ, RTY, YM`

Rules:

- Minimum score: `0.915`
- Risk ticks: `60-240`
- Max open positions: `2`
- Stop taking new trades for the day after `1` closed loss

Results:

| Year | PnL | Drawdown | Sum R |
| --- | ---: | ---: | ---: |
| 2025 | `$587.61` | `$267.60` | `55.81R` |
| 2026 | `$1,237.05` | `$294.24` | `87.64R` |

Combined PnL: `$1,824.66`

Read: highest combined PnL found with worst-year DD under `$350`, but the year balance is uneven. This is more aggressive and may be leaning harder on 2026.

## Best Drawdown-Control Candidate

Roots:

`CL, EMD, HO, NQ, RTY, YM`

Rules:

- Minimum score: `0.915`
- Risk ticks: `60-240`
- Max open positions: `2`
- Stop taking new trades for the day after `1` closed loss

Results:

| Year | PnL | Drawdown | Sum R |
| --- | ---: | ---: | ---: |
| 2025 | `$761.37` | `$249.82` | `66.20R` |
| 2026 | `$778.86` | `$218.02` | `63.13R` |

Combined PnL: `$1,540.23`

Read: best repeat PnL with worst-year DD under `$250`. Cleaner than the highest-profit candidate, but still needs earlier-year validation.

## Best Risk-Adjusted Candidate

Roots:

`HO, NG, NQ`

Rules:

- Minimum score: `0.930`
- Risk ticks: `48-192`
- Max open positions: `2`
- Stop taking new trades for the day after `3` closed losses

Results:

| Year | PnL | Drawdown | Sum R |
| --- | ---: | ---: | ---: |
| 2025 | `$1,005.16` | `$177.64` | `75.60R` |
| 2026 | `$568.35` | `$117.33` | `48.71R` |

Combined PnL: `$1,573.51`

Read: strongest drawdown-adjusted candidate from the phase 2 pass. Less exciting than the profit push candidates, but more stable.

## Official Runner Check

An official full-2026 candle-by-candle validation was started for:

`CL, EMD, ES, HO, NQ, YM`

It did not finish inside the 10-minute shell timeout and produced no completed output, so it was stopped. The cached terminal-stage results above are saved, but the official runner needs either a shorter slice validation or a speed pass before using it to validate every candidate.

## Current Takeaway

The best profit-push lane is:

`score >= 0.915`, risk `60-240` ticks, max open `2`, stop after `1` closed loss per day.

The best groups to keep testing:

1. `CL, EMD, ES, HO, NQ, YM`
2. `CL, EMD, HO, NQ, RTY, YM`
3. `ES, HO, NQ, RTY, YM`
4. `HO, NG, NQ`

The next real validation step is to generate 2021-2024 terminal-stage rows and rerun this exact symbol group search without changing the rules.

## Pure R / No Capital Cap Follow-Up

Added after checking the same idea without fixed account size.

Mode:

- Every qualifying trade is counted.
- No max-open cap.
- No cash reservation limit.
- No daily stop.
- Overlapping trades are allowed.
- Results are measured in R, not dollars.

Detailed files:

- `pure_r_no_cap_exhaustive_root_groups.csv`
- `pure_r_no_cap_summary.json`
- `pure_r_specific_candidates.csv`
- `pure_r_top_both_sumr_dd100.csv`
- `pure_r_top_both_sumr_dd150.csv`
- `pure_r_top_worst_year_dd150.csv`
- `pure_r_best_score_to_dd.csv`

Key finding:

The earlier profit-push candidate was helped heavily by the governor. When every trade is counted, it is no longer the cleanest edge.

### Prior Profit-Push Candidate As Pure R

Roots:

`CL, EMD, ES, HO, NQ, YM`

Rules:

- Minimum score: `0.915`
- Risk ticks: `60-240`

Pure R results:

| Year | Sum R | Drawdown R | Trades | Max Concurrent |
| --- | ---: | ---: | ---: | ---: |
| 2025 | `42.64R` | `44.06R` | 688 | 5 |
| 2026 | `-4.20R` | `92.97R` | 1,018 | 9 |

Read: this group should not be treated as a raw edge without the daily-stop/max-open governor.

### Best Pure R With Higher Total R

Roots:

`CL, EMD, HO, NG, NQ, RB`

Rules:

- Minimum score: `0.910`
- Risk ticks: `72-144`

Pure R results:

| Year | Sum R | Drawdown R | Trades | Max Concurrent |
| --- | ---: | ---: | ---: | ---: |
| 2025 | `56.13R` | `30.55R` | 533 | 5 |
| 2026 | `92.49R` | `61.56R` | 749 | 7 |

Combined: `148.62R`

Read: best total R found in this no-cap pass, but 2026 drawdown is still large.

### Best Repeat / Lower Drawdown Pure R Candidate

Roots:

`CL, EMD, HO, NG, RB`

Rules:

- Minimum score: `0.910`
- Risk ticks: `72-144`

Pure R results:

| Year | Sum R | Drawdown R | Trades | Max Concurrent |
| --- | ---: | ---: | ---: | ---: |
| 2025 | `73.60R` | `19.67R` | 293 | 5 |
| 2026 | `70.53R` | `31.23R` | 414 | 6 |

Combined: `144.12R`

Read: this is the cleaner no-cap candidate because both years are close and drawdown is much lower than the max-R version.

### Best Risk-Adjusted Pure R Candidate

Roots:

`HO, NG`

Rules:

- Minimum score: `0.930`
- Risk ticks: `72-192`

Pure R results:

| Year | Sum R | Drawdown R | Trades | Max Concurrent |
| --- | ---: | ---: | ---: | ---: |
| 2025 | `39.67R` | `3.56R` | 20 | 1 |
| 2026 | `19.89R` | `4.21R` | 13 | 1 |

Combined: `59.56R`

Read: much lower opportunity count, but extremely clean relative to drawdown.

### Pure R Takeaway

If the goal is pure edge quality without account-size limits, the better direction is:

`score >= 0.910`, risk `72-144` ticks, roots around `CL, EMD, HO, NG, RB`, with `NQ` added only if we accept higher drawdown for more R.

This suggests the earlier max-open `2` setting was acting as a risk governor, not proof that the raw stream could safely take every trade.
