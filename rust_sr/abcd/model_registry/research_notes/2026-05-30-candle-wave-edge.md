# 2026-05-30 Candle Wave Edge Research

Protected baseline:

- Run: `aicw-mtf-eg1-xtight-t014-rd3-en4-v1-2m-2026`
- Trades: 402
- Wins / losses: 173 / 229
- Win rate: 43.03%
- Sum R: 222.24R
- Avg R: 0.553R
- Max drawdown: 5.02R

Best research candidates found in this pass:

| Candidate | Change | Trades | Win Rate | Sum R | Avg R | Max DD | Notes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| `aicw-research-t01425-lb25allm3-v1-2m-2026` | Threshold `0.014 -> 0.01425` plus all-system daily loss brake: after 3 trades, stop for day if day <= `-2.5R` | 397 | 43.32% | 230.05R | 0.579R | 5.02R | Best robust candidate. It also improved a fair 2025 walk-forward split. |
| `aicw-research-t014-rd3en5x06-v1-2m-2026` | Energy cap `4/day -> 5/day`, but 5th energy trade needs predicted R `>= 0.06` | 410 | 43.41% | 235.15R | 0.574R | 5.02R | Best candidate from this pass. It keeps the useful extra energy slot while filtering the weakest added trades. |
| `aicw-research-t0141-rd3en4-v1-2m-2026` | Threshold `0.014 -> 0.0141`, same caps | 400 | 43.25% | 230.77R | 0.577R | 5.02R | Cleaner but most of the gain is one trade swap caused by the threshold freeing an energy slot. |
| `aicw-research-t014-rd3en5-v1-2m-2026` | Energy cap `4/day -> 5/day`, same threshold | 417 | 42.69% | 232.26R | 0.557R | 5.02R | Best total R found without increasing drawdown. This suggests the current model may be slightly over-throttled on energy trades. |

Rejected / not-yet-keeper checks:

- Capped-R auto threshold: `aicw-research-auto-cap5-en5-v1-2m-2026`
  - 136 trades, 43.38% wins, 166.11R sum, 1.221R avg, 6.29R max DD.
  - Good average, but lower total return and worse drawdown.
- Higher minimum risk ticks:
  - Filtering above 12 ticks removed too many important winners and cut total R sharply.
- Higher MTF entry alignment:
  - Requiring 2+ aligned timeframes improved average slightly but removed too much total edge.
- Root removal:
  - NQ/QM/PL were weak in 2026, but they were not weak in 2025, so removing them would look like post-test overfitting.
- Energy extra-slot rule:
  - `aicw-research-t014-rd3en5x06-v1-2m-2026` is the best 2026-only result, but on the 2024-train / 2025-valid walk-forward it slipped from 99.06R to 97.00R and drawdown rose from 21.70R to 23.60R. Do not promote without more evidence.

Slippage stress from stored 3+3 tick model returns:

| Run | Stored 3+3 | 5+5 Stress | 7+7 Stress | 10+10 Stress |
| --- | ---: | ---: | ---: | ---: |
| Protected | 222.24R | 203.41R | 184.57R | 156.31R |
| Robust threshold/loss brake | 230.05R | 211.07R | 192.09R | 163.61R |
| Threshold 0.0141 | 230.77R | 211.69R | 192.62R | 164.01R |
| Energy cap 5 | 232.26R | 212.54R | 192.83R | 163.25R |
| Energy cap 5 with strong-only 5th slot | 235.15R | 215.73R | 196.32R | 167.19R |

Walk-forward robustness check:

| Split | Baseline | Robust threshold/loss brake |
| --- | ---: | ---: |
| Train 2024, validate 2025 | 99.06R, 21.70R DD | 106.35R, 19.32R DD |
| Train 2024-2025, validate 2026 | 222.24R, 5.02R DD | 230.05R, 5.02R DD |

Additional confidence checks:

- Random-seed stability:
  - Same robust rules on 2026 with seeds 11, 37, 101, 151 stayed profitable but fell to 169.8R, 198.4R, 192.8R, and 180.1R with worse drawdowns.
  - Same robust rules on 2025 with seeds 11, 37, 101, 151 produced 84.0R, 69.4R, 102.1R, and 107.2R.
  - Conclusion: the robust rule is not dead, but the 230.05R seed-73 result is seed-favorable.
- Five-seed average ensemble:
  - Did not improve robustness. Best balanced threshold did not beat the simple baseline cleanly.
- Deterministic CatBoost (`bootstrap_type=No`, `random_strength=0`):
  - 2025 improved to 113.1R with 12.2R DD at threshold 0.01425.
  - 2026 fell to 202.7R with 6.4R DD, below the protected model.
  - Conclusion: deterministic training is safer-looking on 2025 but not good enough on 2026.
- Liquidity/outlier check:
  - The huge SIJ6 winner entered 2026-04-12 22:22 and exited 2026-04-15 15:56 for 110.25R.
  - It had only 31 same-contract 2m bars in the prior 7 days and 0 bars in the prior 24 hours. Between entry and exit it had only 23 2m candles over roughly 3 days.
  - Removing SI entirely still leaves the robust candidate better than baseline on both 2025 and 2026, but 2026 robust drops from 230.05R to 98.92R.
  - Equal pre-entry liquidity filters still leave robust better than baseline, but absolute R drops sharply.
- Materialized 24h pre-entry liquidity gate:
  - Added scanner flags: `--liquidity-lookback-hours`, `--min-liquidity-bars`, `--min-liquidity-volume`, and `--liquidity-gate-train`.
  - Default behavior is selection/validation gating only. Training rows are only gated when `--liquidity-gate-train` is passed.
  - Practical gate tested: prior 24h same-contract candles `>= 1` and volume `>= 1`.
  - This removes the suspicious SIJ6 mega-winner from 2026.
  - Matching gated baseline vs robust:
    - 2025 baseline: 95.41R, 26.08R DD.
    - 2025 robust: 101.56R, 24.84R DD.
    - 2026 baseline: 116.49R, 5.12R DD.
    - 2026 robust: 124.30R, 5.06R DD.
  - Train-gating the liquidity rows was tested and rejected for now: it made the CatBoost run unstable and weaker on 2026.
- Daily boundary sensitivity:
  - UTC day is the current implementation and gives balanced 2025/2026 behavior.
  - Central/account-day and futures-session day variants change results materially, so the day definition must be decided before live promotion.

Full-history 2m rebuild and training:

- Built full available `2m` candles from `1m` data for 2021-2026.
  - 2021 is partial: Apr 25-Dec 31.
  - 2022-2025 are full available years.
  - 2026 covers Jan 1-Apr 23.
- Context candle tables needed by the model (`5m`, `15m`, `1h`, `4h`) already had full available coverage.
- Rebuilt candle-wave candidates with formula `candle_wave_2m_mtf_ema8_21_breakout_trail_market_slip_v2_eg1_xtight1`.
  - 2021: 84,817 candidates.
  - 2022: 125,574 candidates.
  - 2023: 124,862 candidates.
  - 2024: 124,286 candidates.
  - 2025: 120,152 candidates.
  - 2026: 36,875 candidates.

Full-history results:

| Run | Train | Test | Trades | Win Rate | Sum R | Avg R | Max DD | Notes |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| `aicw-fullhist-t014-base-v1-2m-2026` | 2021-2025 | 2026 | 635 | 39.84% | 180.56R | 0.284R | 23.81R | Strong total R, but drawdown is much worse than the protected model. |
| `aicw-fullhist-liq24b1v1-t01425-lb25allm3-v1-2m-2026` | 2021-2025 | 2026 | 608 | 39.97% | 74.48R | 0.122R | 25.70R | Liquidity/loss-brake version rejected on full history. |
| `aicw-fullhist-wf25-t014-base-v1-2m-2025` | 2021-2024 | 2025 | 2,070 | 36.96% | 170.95R | 0.083R | 16.00R | Prior-year walk-forward held up in total R. |
| `aicw-fullhist-wf25-liq24b1v1-t01425-lb25allm3-v1-2m-2025` | 2021-2024 | 2025 | 1,961 | 36.82% | 145.74R | 0.074R | 14.82R | Slightly lower drawdown, but gave up return. |

Full-history read:

- Adding 2021-2023 did not beat the protected model on 2026.
- The full-history baseline did validate directionally on 2025 and 2026, so it is not dead.
- It is not the current main candidate because 2026 max drawdown expanded from roughly 5R to roughly 24R while total R fell below the protected model.
- The full-history candidate is useful as a broader training reference, but it needs drawdown control before promotion.

Exit-model phase one:

- Added standalone script `rust_sr/abcd/scripts/ai_candle_wave_exit_model.py`.
- First model run: `aicw-exit-early-v1-2m-2026`.
- Source model: `aicw-mtf-eg1-xtight-t014-rd3-en4-v1-2m-2026`.
- Phase-one behavior:
  - It is an early-exit overlay only.
  - It can exit before the source rule-based exit.
  - It cannot hold longer than the source exit yet.
  - It does not change entries.
- Training/calibration/test split:
  - Train selected-like 2024 trades.
  - Calibrate early-exit threshold on selected-like 2025 trades.
  - Test on stored source-model 2026 selected trades.
- Data:
  - Train trades: 3,980.
  - Train decision rows: 33,955.
  - Threshold trades: 4,327.
  - Threshold decision rows: 36,286.
  - Valid trades: 402.
- Result:
  - Calibration selected the no-early-exit threshold because every early-exit threshold was worse on 2025.
  - 2026 overlay result stayed identical to source: 222.24R, 5.02R DD, 0 early exits.
- Oracle check on the 2026 source trades:
  - Perfect hindsight early exits could move 222.24R to 355.12R on the same 402 trades.
  - Improvable trades by hindsight: 325 / 402.
  - This means there is theoretical exit edge, but the first model/features did not learn it reliably enough.
- Recommendation:
  - Do not promote phase-one early-exit overlay.
  - Next exit research should move beyond "exit before source" and train a dynamic hold/exit policy with richer path/context features.

Exit-model phase two:

- Extended `rust_sr/abcd/scripts/ai_candle_wave_exit_model.py` with `--mode dynamic`.
- Dynamic mode target:
  - For each post-entry candle, train the model to estimate whether continuing from here has more future value than exiting next bar.
  - It can exit before or after the source strategy exit.
  - It keeps the original hard stop as the safety stop.
- Added risk-aware threshold calibration:
  - `--max-threshold-dd-r`
  - `--threshold-dd-penalty`
- Dynamic 240-bar / 4-step run:
  - Run: `aicw-exit-dyn240s4-v1-2m-2026`.
  - Train decision rows: 105,649.
  - Threshold decision rows: 108,520.
  - Calibration chased 2025 upside: 600.98R with 73.55R DD.
  - 2026 failed validation: 158.06R, 28.76R DD vs source 222.24R, 5.02R DD.
  - Rejected.
- Dynamic 240-bar / 4-step risk-aware run:
  - Run: `aicw-exit-dyn240s4-dd30p2-v1-2m-2026`.
  - With 30R threshold-year DD cap and 2x DD penalty, calibration disabled the overlay.
  - 2026 stayed identical to source: 222.24R, 5.02R DD.
- Dynamic 60-bar / 2-step risk-aware run:
  - Run: `aicw-exit-dyn60s2-dd30p2-v1-2m-2026`.
  - Train decision rows: 72,751.
  - Threshold decision rows: 76,737.
  - Calibration disabled the overlay.
  - 2026 stayed identical to source: 222.24R, 5.02R DD.
- Dynamic 60-bar uncapped run:
  - Started, but MySQL shut down during threshold decision-row generation.
  - The run did not materialize.
- Dynamic 60-bar uncapped rerun:
  - Run: `aicw-exit-dyn60s2-uncapped-v1-2m-2026`.
  - Calibration disabled the overlay.
  - 2026 stayed identical to source: 222.24R, 5.02R DD.

Exit-model phase three:

- Added source-anchored dynamic mode via `--dynamic-no-signal-exit source`.
  - The old dynamic behavior forced every no-signal trade to the dynamic terminal exit.
  - Source-anchored mode keeps the source/champion exit unless the exit model fires.
  - Added live-safe source-exit-state features: whether the source exit signal has appeared, bars since that signal, source-exit result once known, and current result versus that source exit.
- Source-anchored 2026 sweep against protected source:

| Run | Max Bars / Step | Trades | Source Sum / DD | Exit Sum / DD | Model Exits | Early / Late | Read |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| `aicw-exit-dyn30s2-srcfb-v1-2m-2026` | 30 / 2 | 402 | 222.24R / 5.02R | 225.68R / 5.21R | 103 | 12 / 91 | Too short; tiny gain. |
| `aicw-exit-dyn60s2-srcfallback-v1-2m-2026` | 60 / 2 | 402 | 222.24R / 5.02R | 261.13R / 5.17R | 93 | 6 / 87 | First useful result. |
| `aicw-exit-dyn90s2-srcfb-v1-2m-2026` | 90 / 2 | 402 | 222.24R / 5.02R | 281.59R / 5.82R | 84 | 3 / 81 | Better return, more DD. |
| `aicw-exit-dyn120s2-srcfb-v1-2m-2026` | 120 / 2 | 402 | 222.24R / 5.02R | 299.52R / 5.05R | 64 | 2 / 61 | Best balance so far. |
| `aicw-exit-dyn180s2-srcfb-v1-2m-2026` | 180 / 2 | 402 | 222.24R / 5.02R | 339.85R / 5.58R | 90 | 7 / 83 | Best return so far. |
| `aicw-exit-dyn240s4-srcfb-v1-2m-2026` | 240 / 4 | 402 | 222.24R / 5.02R | 335.99R / 5.11R | 85 | 4 / 80 | Good, but did not beat 180 return. |

- Source-anchored walk-forward checks against 2025 source run:

| Run | Max Bars / Step | Trades | Source Sum / DD | Exit Sum / DD | Model Exits | Early / Late | Read |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| `aicw-exit-dyn60s2-srcfallback-wf25-v1-2m-2025` | 60 / 2 | 2,068 | 99.06R / 21.70R | 308.31R / 12.68R | 585 | 33 / 548 | Validated directionally. |
| `aicw-exit-dyn120s2-srcfb-wf25-v1-2m-2025` | 120 / 2 | 2,068 | 99.06R / 21.70R | 474.09R / 10.10R | 481 | 35 / 438 | Strongest balance on 2025. |
| `aicw-exit-dyn180s2-srcfb-wf25-v1-2m-2025` | 180 / 2 | 2,068 | 99.06R / 21.70R | 614.53R / 11.77R | 639 | 72 / 550 | Strongest return on 2025. |

- Phase-three read:
  - Source-anchoring appears to be the missing safety rail for dynamic exits.
  - The useful edge is mostly "hold past the old trailing/source exit when the model says the wave still has value," not early panic exits.
  - 120 bars is the cleanest balance so far; 180 bars has the highest return but a slightly larger 2026 drawdown.
  - The 2026 improvement is not concentrated in the suspicious SI mega-winner. On the 60-bar run, 54 trades improved, 38 worsened, and 310 stayed unchanged; gains were mostly spread through EMD, RB, HO, and NQ.
  - Do not promote yet without auditing 120-bar and 180-bar trade rows visually and deciding whether the longer post-source holds are realistic live behavior.

Phase-two read:

- The dynamic hold/exit idea has real upside in hindsight, but the first learned policy is not stable enough.
- Raw R calibration badly overfits the threshold year.
- Risk-aware calibration correctly refuses to change the current source model.
- Keep current #1 unchanged.
- Next exit work should focus on better state labels and validation, not simply longer hold windows.

Important caution:

- The run is still heavily helped by a very large SI winner.
- Energy cap 5 remains positive after removing the single largest trade, but drops from 232.26R to 122.01R.
- The strong-only 5th energy slot remains positive after removing the single largest trade, but drops from 235.15R to 124.90R.
- The edge is promising, but the next review should inspect whether the SI mega-winner is a realistic fill/hold and whether energy cap 5 creates too many low-quality extra trades.

Temporary recommendation:

- Keep the protected model pinned for UI/live review.
- Treat `aicw-liq24b1v1-2026-robust-v1-2m-2026` as the leading safer research candidate when sparse contracts are excluded at selection time.
- Treat `aicw-research-t01425-lb25allm3-v1-2m-2026` as the ungated high-upside research candidate, not a live-ready main model.
- Treat `aicw-research-t014-rd3en5x06-v1-2m-2026` as the best 2026-only upside candidate, not yet robust enough to promote.
- Before promotion, add or decide a pre-entry liquidity gate and manually audit sparse metal-contract winners.
- Treat `aicw-research-t014-rd3en5-v1-2m-2026` as the simpler alternate if we do not want a conditional extra-slot rule.
- Treat `aicw-research-t0141-rd3en4-v1-2m-2026` as the conservative alternate.
- Treat `aicw-exit-dyn180s2-srcfb-v1-2m-2026` as the promoted main exit-overlay candidate for now after reviewing the 180-only changes: +41.22R improved, -5.33R worsened, +35.89R net.
- Keep `aicw-exit-dyn120s2-srcfb-v1-2m-2026` as the cleaner balance fallback if 180-bar visual audit shows too many late trend-death exits.
- Do not replace the pinned source model with an exit overlay until the changed trades are visually audited in the UI/canvas.
