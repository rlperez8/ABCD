# Terminal Trade Quality Layer Research

Date: 2026-06-06

## Goal

Build the next layer after Stage 2/Stage 3:

`Should we actually take this trade?`

This is separate from account-size rules. The goal is to clean the raw candidate stream before max-open, daily-stop, or cash-reservation rules touch it.

## Script

`rust_sr/abcd/scripts/ai_terminal_trade_quality_research.py`

## Runs

Thin feature pass:

`rust_sr/abcd/model_registry/aicw-terminal-trade-quality-v1-2025-2026`

Richer Stage 2 feature pass:

`rust_sr/abcd/model_registry/aicw-terminal-trade-quality-v2-stage2features-2025-2026`

## Features Used

Only live-compatible entry-time fields were used:

- Root
- Symbol
- Direction
- Entry hour / weekday / month
- Stage 2 score
- Risk ticks
- Slippage R burden
- Cat / Light / XGB Level 1 scores
- Level 2 score
- Top Level 1 model block
- Level 1 score agreement/spread
- Stage 2 confirmation offset

Excluded as leakage:

- Exit reason
- Hold duration
- Future path
- Final result
- Oracle labels

## Best 2025 To 2026 Result

Train:

`2025`

Validate:

`2026`

Roots:

`CL, EMD, HO, NG, RB`

Base rule:

- Stage 2 score `>= 0.910`
- Risk ticks `72-144`

Trade-quality rule:

- Quality score `>= -0.073264`
- Rank mode: `top1_per_entry_time`

Result:

| Year | Trades | Sum R | Drawdown R | Avg R | Win Rate | Max Concurrent |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 2026 | 305 | `117.51R` | `19.39R` | `0.385R` | `27.5%` | 5 |

Comparison to the earlier no-cap pure-R version of the same root/risk family:

| Variant | 2026 Sum R | 2026 Drawdown R | Trades |
| --- | ---: | ---: | ---: |
| Base pure R | `70.53R` | `31.23R` | 414 |
| Quality layer | `117.51R` | `19.39R` | 305 |

Read: this is a real improvement on 2026 when trained on 2025. It increased total R, reduced drawdown, and reduced noisy trades.

## Reverse Check

Train:

`2026`

Validate:

`2025`

Best validation result:

| Year | Roots | Trades | Sum R | Drawdown R | Avg R | Win Rate |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 2025 | All roots | 85 | `13.09R` | `15.79R` | `0.154R` | `37.6%` |

Same clean root group:

| Year | Roots | Trades | Sum R | Drawdown R | Avg R | Win Rate |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 2025 | `CL, EMD, HO, NG, RB` | 124 | `11.15R` | `21.30R` | `0.090R` | `33.9%` |

Read: this is positive but much weaker. The quality layer is promising, but not strong enough to promote without more years.

## Current Takeaway

The right architecture still looks like:

`Stage 1/2 trend detection -> terminal trade-quality selector -> rank conflicts -> account governor`

The Stage 2-feature quality layer is the best improvement from this pass. It is not ready to become the main strategy yet because the reverse fold did not transfer strongly.

## Existing Exit Artifact Check

I also checked the existing Stage 3 trend-manager exit artifacts.

2026 full files:

| Run | Trades | Model Sum R | Baseline Sum R | Delta R | Drawdown R |
| --- | ---: | ---: | ---: | ---: | ---: |
| `aicw-os-stage3-trend-manager-long-mid-v1-2m-m180-v2026` | 4,518 | `-217.88R` | `-209.23R` | `-8.65R` | `270.30R` |
| `aicw-os-stage3-trend-manager-mid-v1-2m-m180-v2026` | 4,493 | `-208.14R` | `-326.19R` | `+118.05R` | `221.75R` |
| `aicw-os-stage3-trend-manager-smoke-v1-2m-m120-v2026` | 446 | `-2.55R` | `-27.83R` | `+25.28R` | `42.81R` |

Read: the exit manager can improve a bad baseline, but the full 2026 streams are still negative before trade-quality filtering. The smaller 120-bar smoke run is too small to trust. The better immediate improvement is filtering/quality selection before entry, not promoting one of these exit artifacts.

## Next Step

Before promotion:

1. Generate 2021-2024 terminal-stage rows.
2. Train on 2021-2025.
3. Validate on 2026.
4. Also do rolling tests such as 2021-2023 -> 2024, 2021-2024 -> 2025, 2021-2025 -> 2026.

If the `CL, EMD, HO, NG, RB` quality-layer setup survives those tests, it becomes a serious candidate.
