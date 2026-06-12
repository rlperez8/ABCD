# 2026-06-06 Stage 2 Target Forecast + Stage 3 Bracket

## Question

Can Stage 2 predict a realistic target price, then hand that target to Stage 3 so the live trade has both:

- fixed hard stop
- fixed take-profit
- early live-only fade exit

## Run

`rust_sr/abcd/scripts/ai_stage2_target_stage3_bracket_research.py`

Output:

`rust_sr/abcd/model_registry/aicw-stage2-target-stage3-bracket-v1-2m-tr2025-v2026`

## Design

- Stage 2 confirmed event enters on the next 2m candle open.
- Initial stop is the existing swing/ATR hard stop.
- A CatBoost regressor predicts maximum favorable excursion from live entry features.
- The predicted MFE is converted into a bracket take-profit target.
- Stage 3 exits by target, hard stop, max window, momentum fade, or giveback fade.
- No old harmonic source exit or future field is used as a feature.
- Slippage assumption: 3 ticks entry + 3 ticks exit.

## Data

- Train target model: 2025 Stage 2 train confirmations.
- Select target settings: 2025 threshold confirmations.
- Validate: 2026 confirmations.

Rows built:

- Train: 37,778
- Threshold: 16,111
- Valid: 24,081

## Best 2026 Valid Result

Selected threshold rule:

- Roots: `CL, EMD, HO, NG, RB`
- Stage 2 score: `>= 0.910`
- Risk: `72-144` ticks
- Predicted MFE: `>= 1.25R`
- Target factor: `0.75`
- Target range: `1.0R` to `1.5R`

2026 bracket result:

- Trades: 331
- Sum R: 5.09R
- Max DD: 17.85R
- Avg R: 0.015R
- Win rate: 39.58%
- Max concurrent: 4

Same selected rows with terminal hard-stop/time-exit comparison:

- Sum R: 94.39R
- Max DD: 35.25R
- Avg R: 0.285R
- Win rate: 27.19%
- Max concurrent: 6

Exit reasons for the best bracket result:

- Momentum fade: 188
- Take profit: 95
- Hard stop: 26
- Giveback fade: 22

## Verdict

Reject this first implementation.

The bracket/fade design cut drawdown, but it cut profit much more. The early fade and tight target converted too many trend runners into small exits. This proves the concern was real, but the first fix is not better than the current terminal/quality path.

## Next

Keep the target-forecast concept, but do not promote this fixed TP/fade version.

Better next versions:

- Train a target distribution instead of one conservative MFE estimate.
- Let Stage 3 pick from target tiers only when expected R/DD improves.
- Add a smarter hold/exit manager that can avoid capping strong trends.
- Optimize replay speed before sweeping many target/fade variants.
