# 2026-05-30 Candle Wave Edge Research

Data universe TODO:

- Keep the model/training universe aligned to roots that are actually tradable in the user's NinjaTrader account.
- Verify currently loaded but questionable roots before live-facing training: `NKD`, `PA`, `PL`.
- Priority NinjaTrader-tradable roots to add to 1m candle history: `MES`, `MNQ`, `M2K`, `MYM`, `MCL`, `MGC`, `MHG`, `QO`, `QI`, `ZF`, `ZT`, `UB`, `M6A`, `M6B`, `M6E`, `MCD`/NinjaTrader `MICD`, `6M`.
- After loading new 1m roots, rebuild derived timeframe candles before using them in multi-timeframe models.
- Full-history validation path: rebuild derived candles and candidate rows for all tradable roots across `2020-2026`, run frozen/walk-forward tests on truly unseen years/symbols, then train a final all-years production candidate only if the walk-forward holds up. Preferred splits: train `2020-2022` -> test `2023`, train `2020-2023` -> test `2024`, train `2020-2024` -> test `2025`, train `2020-2025` -> test `2026`.

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

Live-valid source-manager correction:

- The source-fallback 180-bar exit overlay is not live-valid as a standalone policy.
  - The old source-fallback run could hold past the source exit, then still fall back to the earlier source exit if the AI never fired.
  - That fallback cannot happen live after choosing to hold, because the source exit has already been passed.
- Added `aicw-src-manager-180-v1-2m-2026`:
  - A manager model that only decides at the live source-exit moment.
  - Action is either `EXIT_SOURCE` or `HOLD_FOR_AI`.
  - If it holds, the old 180-bar AI path must exit live by model exit, hard stop, or window terminal. No source fallback is allowed.
- First full live-valid manager result with saved metadata threshold:
  - 2025: 4,327 trades, 373.02R, 66.33R DD, 41.62% win.
  - 2026: 4,378 trades, 136.54R, 118.45R DD, 39.38% win.
- Threshold sweep read:
  - A conservative 2025-balanced manager threshold near -0.2798 gives 2025 363.06R / 47.27R DD and 2026 87.73R / 80.17R DD.
  - A more aggressive manager threshold near -0.4994 gives 2025 316.04R / 53.52R DD and 2026 178.10R / 101.17R DD using the original AI exit threshold.
- AI exit-threshold sweep:
  - With manager threshold -0.4994, changing the live AI exit threshold to 0.0 improved the full policy.
  - 2025: 335.55R, 51.74R DD, 41.71% win.
  - 2026: 203.94R, 97.32R DD, 39.74% win.
- Saved candidate:
  - Run: `aicw-src-manager-180-livefix-v1-2m-2026`.
  - Uses the same manager weights as `aicw-src-manager-180-v1-2m-2026`.
  - Records `manager_threshold = -0.4994` and `policy_exit_threshold = 0.0` in metadata.
  - Reproduces from metadata with `eval_source_exit_manager_full_policy.py`.
- Current read:
  - This is the leading live-valid exit-manager candidate.
  - It is much more realistic than the old source-fallback 180 result, but not as strong as the invalid fallback run.
  - Remaining risk is concentrated in trades where neither the source exit nor AI exit fires before the terminal window, plus over-held manager `HOLD_FOR_AI` selections.
  - Next research should focus on live-valid no-source/window risk handling and hold-manager calibration before promotion.

Live-valid root-gated candidate:

- Root gate basis:
  - Use 2025 livefix-policy results only.
  - Exclude roots with at least 100 trades and negative total R in 2025.
  - Excluded roots: `CL`, `GF`, `NKD`, `RTY`, `YM`.
- Saved candidate:
  - Run: `aicw-src-manager-180-livefix-rootgate25-v1-2m-2026`.
  - Same manager weights and exit policy as `aicw-src-manager-180-livefix-v1-2m-2026`.
  - Adds `policy_exclude_roots = ["CL", "GF", "NKD", "RTY", "YM"]`.
- Validation:
  - 2025: 1,698 trades, 551.17R, 15.02R DD, 45.35% win.
  - 2026: 2,929 trades, 279.92R, 61.51R DD, 40.25% win.
- Current read:
  - This is the best live-valid policy shape found so far.
  - It is a root gate, not a new learned model, so it should be treated as a live candidate with an explicit calibration rule rather than hidden model magic.
  - Before promotion, verify excluded roots against any account/tradability constraints and decide whether excluding `GF` is acceptable since it was only barely negative in 2025 but negative again in 2026.
- Threshold sensitivity after root gate:
  - Current threshold around -0.50 is near the 2026 top-sum threshold: 281.45R / 61.51R DD at -0.5000.
  - 2025 top-sum threshold around -0.5957 gives 622.59R / 16.78R DD on 2025, but drops 2026 to about 263.55R / 73.24R DD.
  - Lower-DD thresholds around -0.20 to -0.30 reduce 2026 DD to roughly 40R-47R but cut 2026 return to roughly 150R-211R.
- Root sensitivity:
  - Excluding `RTY`, `CL`, `NKD`, `YM` but keeping `GF`: 2026 261.78R / 60.13R DD.
  - Excluding `RTY`, `CL`, `YM`, `GF` but keeping `NKD`: 2026 289.47R / 67.50R DD.
  - Excluding all five: 2026 279.92R / 61.51R DD.
  - High-confidence roots only, using 2025 roots with at least 25 trades and positive 2025 R (`EMD`, `RB`, `NQ`, `HO`, `GC`, `HG`, `ZS`, `SI`): 2026 227.01R / 13.88R DD on 569 trades. This is interesting but too narrow to promote yet.

Root-gated retrain check:

- Run: `aicw-src-manager-180-livefix-roottrain25-v1-2m-2026`.
- Same excluded roots and AI exit threshold, but the manager itself was retrained on the gated universe.
- Result:
  - 2025 full policy: 613.18R, 29.63R DD.
  - 2026 full policy: 161.37R, 102.51R DD.
  - 2026 threshold sweep best by sum: 184.39R, 97.49R DD.
- Read:
  - Rejected for now.
  - It over-holds on 2026 and does not beat the filtered original manager.
  - Keep `aicw-src-manager-180-livefix-rootgate25-v1-2m-2026` as the leading candidate.

Oracle perfect trend teacher:

- Added DB-backed oracle builder:
  - Script: `rust_sr/abcd/scripts/build_oracle_perfect_trend_trades.py`.
  - Tables: `ai_oracle_trend_runs`, `ai_oracle_trend_trades`.
- First full oracle run:
  - Run: `oracle-perfect-trends-v1-2m-2024_2026`.
  - Timeframe: 2m.
  - Years: 2024, 2025, 2026.
  - Roots: all loaded 2m candle roots.
  - Rows: 235,027 oracle winning trend trades.
  - Verified non-winners: 0.
- Oracle definition:
  - Hindsight pivot-low to pivot-high segments become LONG oracle trades.
  - Hindsight pivot-high to pivot-low segments become SHORT oracle trades.
  - Minimum result after entry/exit slippage: 2.0R.
  - Maximum adverse movement: 0.75R.
  - Minimum path efficiency: 0.18.
  - Risk is ATR-based with a 12-tick minimum.
  - These are teacher rows only, not live-valid entries.
- Summary:
  - 2024: 75,352 rows, 327,503.14R, 4.35R average.
  - 2025: 84,863 rows, 376,618.77R, 4.44R average.
  - 2026: 74,812 rows, 339,929.89R, 4.54R average.
- Next use:
  - Train entry models to identify candles near oracle starts.
  - Train hold models to identify whether current candle is inside an oracle trend segment.
  - Train exit models to identify candles near oracle ends.
  - Compare current CatBoost/livefix trades against oracle segments to measure early/late entries and exits.

Oracle trend-start detector:

- Goal:
  - Entry research only.
  - Teach a model to identify candles near the start of hindsight-perfect oracle trend segments.
  - No trade, stop, target, or exit is created by this model.
- Added scripts:
  - `rust_sr/abcd/scripts/ai_oracle_trend_start_model.py`.
  - `rust_sr/abcd/scripts/eval_oracle_trend_start_full_grid.py`.
- Saved model:
  - Run: `aicw-oracle-start-v1-2m-tr2024-v2026`.
  - Train year: 2024.
  - Threshold year: 2025.
  - Validation year: 2026.
  - Timeframe: 2m.
  - Features are live-safe candle/indicator features from the signal candle and prior candles.
- Balanced sampled validation read:
  - Selected threshold from 2025: 0.3849878749.
  - 2026 balanced sample AUC: 0.7815.
  - 2026 balanced sample AP: 0.7603.
  - 2026 balanced sample precision: 60.96%.
  - 2026 balanced sample recall: 92.95%.
  - Caution: this is a balanced positive/negative sample, not production base rate.
- Full 2026 candle-grid evaluation:
  - Evaluated 4,527,878 candle/direction candidates.
  - Oracle starts: 74,812.
  - Cooldown: 8 bars.
  - Match window: +/- 4 bars.
  - Threshold 0.385: 246,952 fired events, 28.85% matched picks, 95.22% oracle recall, median abs distance 2 bars.
  - Threshold 0.600: 186,585 fired events, 35.15% matched picks, 87.67% oracle recall, median abs distance 2 bars.
  - Threshold 0.800: 95,176 fired events, 46.81% matched picks, 59.55% oracle recall, median abs distance 1 bar.
  - Threshold 0.900: 30,617 fired events, 57.78% matched picks, 23.64% oracle recall, median abs distance 1 bar.
- Current read:
  - The model is learning the visual/statistical shape of trend starts.
  - The low threshold is useful as a broad scanner but fires too often for a trade trigger.
  - The high threshold is much cleaner and usually late by about one 2m bar on matched picks.
  - Next step should tune a firing policy, not just a raw score threshold: threshold crossing, cooldown, local score peak, and market/session/root gates.

Oracle visual trend-start detector:

- Goal:
  - Give the image/CNN lane the same oracle teacher as the numeric CatBoost trend-start detector.
  - Model predicts whether a chart image ending at the current closed candle looks like an oracle trend start.
  - This is still entry-signal research only; no trade, stop, target, or exit is created.
- Added script:
  - `rust_sr/abcd/scripts/ai_oracle_trend_start_visual_cnn.py`.
- Saved full run:
  - Run: `aicw-oracle-start-visual-cnn-v1-2m-tr2024-v2026`.
  - Train year: 2024.
  - Threshold year: 2025.
  - Validation year: 2026.
  - Sample size: 40,000 rows per split, balanced 50/50 oracle-start positives and hard-away negatives.
  - Image: 48x48, 64-bar lookback ending at the signal candle.
- 2026 balanced sample:
  - Visual CNN threshold: 0.3907007181.
  - Visual CNN: 28,262 picks, 66.45% precision, 93.90% recall, AUC 0.8275, AP 0.7919.
  - Numeric CatBoost at threshold 0.3849878749: 30,570 picks, 60.97% precision, 93.19% recall, AUC 0.7812, AP 0.7582.
  - Both agree: 25,391 picks, 69.69% precision, 88.48% recall.
  - Agreement share of visual picks: 89.84%.
  - Agreement share of CatBoost picks: 83.06%.
  - Pick Jaccard: 75.93%.
- Read:
  - The image model is learning useful independent structure from the oracle starts.
  - Consensus improves precision versus either model alone while preserving most recall on the balanced sample.
  - The rows where only one model fires are much lower precision, especially CatBoost-only picks.
  - This supports using image+numeric agreement as the next entry gate.
- Caution:
  - These are balanced sampled metrics, not full candle-grid base-rate metrics.
  - Next step should score image consensus on the full-grid CatBoost-fired events, especially stricter thresholds around 0.8 and 0.9, before calling it a live entry signal.

Four-model oracle-start agreement:

- Goal:
  - Add XGBoost and LightGBM as independent tabular models.
  - Compare all four models on identical oracle-start sampled rows:
    - Numeric CatBoost.
    - Visual CNN.
    - XGBoost.
    - LightGBM.
- Added script:
  - `rust_sr/abcd/scripts/ai_oracle_start_four_model_agreement.py`.
- Saved run:
  - Run: `aicw-oracle-start-fourmodel-v1-v2026`.
  - Source rows: `aicw-oracle-start-visual-cnn-v1-2m-tr2024-v2026`.
  - Train split: 40,000 rows from 2024.
  - Threshold split: 40,000 rows from 2025.
  - Validation split: 40,000 rows from 2026.
  - XGBoost and LightGBM were trained on the same tabular features used by CatBoost.
- 2026 balanced sample single models:
  - CatBoost: 31,037 picks, 60.50% precision, 93.88% recall.
  - Visual CNN: 28,262 picks, 66.45% precision, 93.90% recall.
  - XGBoost: 30,304 picks, 61.24% precision, 92.79% recall.
  - LightGBM: 29,237 picks, 62.34% precision, 91.14% recall.
- 2026 balanced sample agreement:
  - CatBoost + Visual CNN: 25,658 picks, 69.44% precision, 89.08% recall.
  - Visual CNN + LightGBM: 24,694 picks, 70.35% precision, 86.87% recall.
  - Image + tabular majority: 25,271 picks, 69.84% precision, 88.25% recall.
  - Any 3 of 4: 29,319 picks, 62.93% precision, 92.25% recall.
  - All 4: 23,959 picks, 71.03% precision, 85.09% recall.
- Read:
  - Visual CNN remains the best single model on this sampled task.
  - XGBoost and LightGBM are useful challengers but do not beat the image model alone.
  - Requiring all four to agree gives the cleanest precision found so far while keeping a surprisingly high amount of recall on the balanced sample.
  - Visual CNN plus LightGBM is also interesting: nearly all-four precision with slightly better recall.
- Caution:
  - This is still sampled oracle data, not a full candle-grid live scan.
  - The next meaningful test is full-grid scoring of candidate fires, then applying the image/XGB/LGBM agreement filters to measure real base-rate precision and signal count.

Four-model full-grid oracle-start check:

- Added script:
  - `rust_sr/abcd/scripts/eval_oracle_start_four_model_full_grid.py`.
- Purpose:
  - Stream the 2026 candle grid like a live scanner.
  - Score CatBoost, XGBoost, and LightGBM on every candidate candle/direction.
  - Score the visual CNN only after a tabular gate, because image scoring every candle is too expensive.
  - Measure fired signal events against oracle starts.
- Full run:
  - Source model run: `aicw-oracle-start-fourmodel-v1-v2026`.
  - Year: 2026.
  - Roots: all loaded 2m symbols.
  - Thresholds: CatBoost 0.8, XGBoost 0.8, LightGBM 0.8, Visual CNN 0.8.
  - Image gate: tabular all 3.
  - Cooldown: 8 bars.
  - Match window: +/- 4 bars.
  - Total candidate candle/directions: 4,527,038.
  - Image rows scored: 141,020.
- 2026 full-grid event results:
  - CatBoost: 95,162 events, 44,548 matched, 46.81% match rate, 59.55% oracle recall, median distance 1 bar.
  - XGBoost: 75,504 events, 37,198 matched, 49.27% match rate, 49.72% oracle recall, median distance 2 bars.
  - LightGBM: 94,467 events, 44,136 matched, 46.72% match rate, 59.00% oracle recall, median distance 2 bars.
  - Tabular majority 2 of 3: 85,027 events, 41,261 matched, 48.53% match rate, 55.15% oracle recall.
  - Tabular all 3: 63,623 events, 32,623 matched, 51.28% match rate, 43.61% oracle recall.
  - All 4 agree: 38,825 events, 21,497 matched, 55.37% match rate, 28.73% oracle recall, median distance 1 bar.
- Read:
  - All-four agreement improves real full-grid signal cleanliness versus CatBoost alone by about 8.6 percentage points.
  - The lift is real but not enormous; it trades a lot of recall for cleaner entries.
  - The visual model helps most as a confirmation gate after tabular agreement.
  - Hard all-four agreement is probably too rigid as final live logic, but it is a strong feature for a future manager/meta-model.
  - The next best route is a manager model trained on the four scores, score spreads, and agreement counts instead of a permanent all-4-must-agree rule.

Oracle-start manager model:

- Goal:
  - Train a meta-model on oracle labels that sees all four specialist scores:
    - CatBoost score.
    - XGBoost score.
    - LightGBM score.
    - Visual CNN score.
  - Also feed it score spreads, agreement counts, and market context so it can learn when specialist disagreement matters.
- Added script:
  - `rust_sr/abcd/scripts/ai_oracle_start_manager_model.py`.
- Saved model:
  - Run: `aicw-oracle-start-manager-v1-v2026`.
  - Model type: CatBoost manager.
  - Source rows: `aicw-oracle-start-fourmodel-v1-v2026`.
  - Train split: 40,000 sampled rows from 2024.
  - Threshold split: 40,000 sampled rows from 2025.
  - Validation split: 40,000 sampled rows from 2026.
  - Selected manager threshold: 0.2731136387.
- Sampled 2026 result:
  - Manager: 26,613 picks, 66.91% precision, 89.03% recall.
  - All 4 agreement baseline: 23,959 picks, 71.03% precision, 85.09% recall.
  - Read: manager selected for recall, not maximum precision. It did not beat all-four agreement on precision in the sampled split.
- Full-grid manager check:
  - Added optional manager scoring to `eval_oracle_start_four_model_full_grid.py`.
  - Full run used the same strict 0.8 tabular-all image gate as the all-four full-grid check.
  - Manager default threshold:
    - 60,994 events.
    - 31,152 matched.
    - 51.07% match rate.
    - 41.64% oracle recall.
    - Median distance: 1 bar.
  - All-four agreement baseline from the same run:
    - 38,825 events.
    - 21,497 matched.
    - 55.37% match rate.
    - 28.73% oracle recall.
    - Median distance: 1 bar.
  - Read:
    - The current manager expands recall but is not cleaner than all-four agreement.
    - It is not yet a promoted entry selector.
    - It is still useful because it proves the manager architecture works and exposes the tradeoff: more starts found, lower cleanliness.
    - Next manager version should train/evaluate directly on full-grid-like negatives, not only balanced sampled rows, if we want it to improve real base-rate precision.

Stage 2 confirmation / fakeout filter:

- Goal:
  - Stop trying to enter the exact first candle of every trend.
  - Let Stage 1 cast a wide net for possible trend starts.
  - Wait a fixed number of candles, then let Stage 2 decide whether the move is actually confirming or should be canceled as a fakeout.
- Added script:
  - `rust_sr/abcd/scripts/ai_oracle_start_stage2_confirmation.py`.
- Saved proof-of-concept:
  - Run: `aicw-oracle-start-stage2-confirm-v1-catboost-w8-v2026`.
  - Stage 1 rule: CatBoost wide signal.
  - Stage 1 threshold: 0.385.
  - Confirmation wait: 8 bars on 2m candles.
  - Model type: CatBoost.
  - Train split: 40,000 sampled rows from 2024.
  - Threshold split: 40,000 sampled rows from 2025.
  - Validation split: 40,000 sampled rows from 2026.
- Stage 2 features:
  - Four specialist model scores and agreement features.
  - Original market/context features.
  - Post-signal confirmation behavior after 8 candles:
    - net close move in ATR.
    - max favorable move.
    - max adverse move.
    - favorable minus adverse.
    - body alignment share.
    - close-step alignment share.
    - second-half continuation vs first-half.
    - final candle body/wick behavior.
- 2026 balanced sample result:
  - Stage 1 alone:
    - 30,569 picks.
    - 18,638 true picks.
    - 60.97% precision.
    - 93.19% recall.
  - Stage 1 then Stage 2:
    - 20,451 picks.
    - 17,363 true picks.
    - 84.90% precision.
    - 86.82% recall.
  - Stage 2 over all sampled rows:
    - 21,804 picks.
    - 18,290 true picks.
    - 83.88% precision.
    - 91.45% recall.
- Read:
  - This is the most promising direction so far for reducing fake trend starts.
  - Waiting 8 bars gives the model enough live-safe evidence to separate real trend formation from fakeouts.
  - It sacrifices perfect bottom/top entry, but keeps most of the oracle opportunities while greatly improving sample precision.
- Caution:
  - This is still sampled/balanced oracle data.
  - A full-grid/live-style Stage 2 run is required before promotion.
  - The current Stage 2 model enters 8 bars later, so the next evaluation must measure remaining trend R after the confirmation entry, not only oracle-start match precision.

Adaptive Stage 2 confirmation:

- Goal:
  - Replace the fixed "wait 8 bars, then decide" idea with a rolling confirmation model.
  - After Stage 1 fires, score candle offset 1, 2, 3, ... up to a max cap.
  - Confirm on the first offset above threshold; reject if no offset confirms before the cap.
- Added script:
  - `rust_sr/abcd/scripts/ai_oracle_start_stage2_adaptive_confirmation.py`.
- Smoke run:
  - Run: `aicw-oracle-start-stage2-adaptive-smoke-v1-catboost-m12-v2026`.
  - Stage 1 rule: CatBoost wide signal.
  - Stage 1 threshold: 0.385.
  - Max confirmation cap: 12 bars on 2m candles.
  - Train/threshold/valid: 6,000 sampled rows per split.
- 2026 smoke result at selected threshold:
  - Stage 1 alone:
    - 4,580 events.
    - 2,800 true oracle-start events.
    - 61.14% precision.
    - 93.33% total oracle recall.
  - Adaptive Stage 2:
    - 3,165 confirmed events.
    - 2,506 true confirmed events.
    - 659 false confirmed events.
    - 79.18% precision.
    - 89.50% recall within Stage 1 positives.
    - 83.53% total oracle recall.
    - Average true confirmation offset: 1.78 bars.
    - Median true confirmation offset: 1 bar.
- Strict threshold examples on 2026 smoke:
  - Threshold 0.90: 2,120 confirms, 86.89% precision, 65.79% Stage 1 recall, average true offset 2.48 bars.
  - Threshold 0.95: 1,722 confirms, 89.31% precision, 54.93% Stage 1 recall, average true offset 2.82 bars.
  - Threshold 0.98: 1,252 confirms, 92.49% precision, 41.36% Stage 1 recall, average true offset 3.54 bars.
- Read:
  - Adaptive confirmation works as a tunable dial.
  - Loose thresholds confirm quickly and keep many oracle starts.
  - Strict thresholds wait longer and become much cleaner.
  - Compared with the fixed 8-bar proof, adaptive Stage 2 is faster but currently less clean at the selected F1-style threshold.
- Caution:
  - This is a smoke-sized balanced sample.
  - Next step is a larger/full split and then a full-grid/live-style run that measures remaining trend R after the confirmation candle.

Adaptive Stage 2 proof-target tuning:

- Changed the adaptive Stage 2 target so each offset row answers a sharper live question:
  - Is this offset proof-ready now?
  - Did it show at least small follow-through?
  - Is there still enough oracle trend left after this confirmation candle?
- Added nearest-oracle detail matching:
  - Stage 1 positives are not always exactly on the oracle entry candle.
  - The adaptive target now matches oracle details within a few bars instead of requiring exact timestamp equality.
- Current proof label parameters:
  - Min close follow-through: 0.05 ATR.
  - Min favorable probe: 0.15 ATR.
  - Max adverse probe: 1.5 ATR.
  - Min remaining trend after confirmation: 0.25R.
  - Min remaining bars: 1.
  - Max confirmation cap: 16 bars on 2m.
- Stage 1 tuning smoke results:
  - CatBoost 0.385:
    - 2026 precision: 79.85%.
    - Target recall: 69.50%.
    - Total oracle recall: 63.53%.
  - CatBoost 0.50:
    - 2026 precision: 78.65%.
    - Target recall: 81.14%.
    - Total oracle recall: 68.17%.
  - CatBoost 0.55:
    - 2026 precision: 78.61%.
    - Target recall: 88.69%.
    - Total oracle recall: 69.93%.
  - CatBoost 0.60:
    - 2026 precision: 79.46%.
    - Target recall: 84.28%.
    - Total oracle recall: 61.03%.
  - All-four agreement:
    - 2026 precision: 79.41%.
    - Target recall: 84.50%.
    - Total oracle recall: 69.97%.
  - Image plus tabular majority:
    - 2026 precision: 79.08%.
    - Target recall: 80.57%.
    - Total oracle recall: 69.50%.
- Medium candidate:
  - Run: `aicw-oracle-start-stage2-adaptive--catboost-m16-cf15104944`.
  - Stage 1: CatBoost threshold 0.55.
  - Sample: 12,000 rows per split before adaptive expansion.
  - 2026 balanced threshold:
    - 7,116 Stage 1 events.
    - 4,508 target-positive events.
    - 5,114 confirmed events.
    - 4,020 true confirmed events.
    - 1,094 false confirmed events.
    - 78.61% proof precision.
    - 89.17% target recall.
    - 83.56% proof F1.
    - 82.44% oracle precision.
    - 70.27% total oracle recall.
    - Average true confirmation offset: 1.76 bars.
  - Strict threshold variant:
    - Run: `aicw-oracle-start-stage2-adaptive--catboost-m16-4c40addd4a`.
    - Same model setup, selected for at least 82% precision on 2025.
    - 2026 strict result:
      - 3,469 confirmed events.
      - 2,843 true confirmed events.
      - 626 false confirmed events.
      - 81.95% proof precision.
      - 63.07% target recall.
      - 71.28% proof F1.
      - 84.35% oracle precision.
      - 48.77% total oracle recall.
      - Average true confirmation offset: 1.46 bars.
- Read:
  - CatBoost Stage 1 at 0.55 is the best current balance in sampled testing.
  - The strict threshold is useful if cleanliness matters more than opportunity count.
  - Next step should be a larger/full adaptive run for the 0.55 candidate, then a full-grid/live-style evaluator that measures remaining R after confirmation.

Stage 2 100% precision tuning pass:

- Goal:
  - Start from the user's desired target: Stage 2 should only say yes when it is 100% confident.
  - Work backward from perfect precision and measure how much opportunity is sacrificed.
- Expanded threshold search:
  - Added high-tail threshold candidates to `ai_oracle_start_stage2_adaptive_confirmation.py`.
  - This lets the selector search the extreme score region instead of stopping at the 99th percentile.
- 100% target run:
  - Run: `aicw-oracle-start-stage2-adaptive--catboost-m16-d519584aee`.
  - Stage 1: CatBoost 0.55.
  - Stage 2: adaptive proof target, medium sample.
  - Threshold selected on 2025 for 100% precision: 0.986520.
  - 2025 result:
    - 15 confirmed.
    - 15 true.
    - 0 false.
    - 100.00% precision.
    - 0.25% total oracle recall.
  - 2026 result:
    - 8 confirmed.
    - 7 true.
    - 1 false.
    - 87.50% precision.
    - 0.12% total oracle recall.
  - Read:
    - Pure 100% by threshold is too tiny and does not generalize.
    - It is not a usable model; it is just an over-strict filter.
- Confirmation gate sweep:
  - Added script:
    - `rust_sr/abcd/scripts/analyze_stage2_confirmation_gates.py`.
  - Saved expanded rows run:
    - `aicw-oracle-start-stage2-adaptive--catboost-m16-7f7651dfe7`.
  - Simple gates searched:
    - Stage 2 score.
    - Min close follow-through.
    - Min favorable probe.
    - Max adverse probe.
    - Confirmation efficiency.
    - Close-step alignment.
  - Best high-tail 2025 pocket:
    - 24 confirmed.
    - 24 true.
    - 0 false.
    - 100.00% precision.
  - Same gate on 2026:
    - 16 confirmed.
    - 13 true.
    - 3 false.
    - 81.25% precision.
  - Read:
    - Simple candle gates also fail to produce stable 100%.
    - The false positives can still look clean by these basic follow-through metrics.
- Stage 2B rejector:
  - Added script:
    - `rust_sr/abcd/scripts/ai_stage2_confirmation_rejector.py`.
  - Purpose:
    - Adaptive Stage 2 confirms first.
    - Stage 2B learns only on confirmed candidates and tries to reject false confirmations.
  - 100% target run:
    - Run: `aicw-stage2-confirm-rejector-p100-v1-7f7651dfe7-v2026`.
    - 2025: 5 confirmed, 5 true, 0 false.
    - 2026: 0 confirmed.
    - Read: perfect but unusable.
  - Practical thresholds from rejector sweep:
    - 2026 at high threshold 0.990575:
      - 82 confirmed.
      - 70 true.
      - 12 false.
      - 85.37% precision.
      - 1.55% target recall.
    - 2026 at threshold 0.727489:
      - 3,124 confirmed.
      - 2,579 true.
      - 545 false.
      - 82.55% precision.
      - 57.21% target recall.
    - 2026 at threshold 0.225485:
      - 4,838 confirmed.
      - 3,867 true.
      - 971 false.
      - 79.93% precision.
      - 85.78% target recall.
  - Read:
    - Stage 2B improves the precision/recall tradeoff a little, but it does not solve 100%.
    - The best useful band is still around low/mid 80s precision.
- Extra confirmation-shape features:
  - Added to adaptive Stage 2:
    - Favorable/adverse ratio.
    - Close-to-favorable ratio.
    - Giveback from favorable move.
    - Best-close giveback.
    - Net cleanliness.
    - Close draw-up efficiency.
  - Smoke run:
    - `aicw-oracle-start-stage2-adaptive--catboost-m16-9ce84c6a33`.
  - 2026 result:
    - 78.88% precision.
    - 86.15% target recall.
    - 67.50% total oracle recall.
  - Read:
    - Shape features improved training but did not materially improve 2026.
    - This points toward overfitting if we keep only tightening Stage 2 with the same data.
- Current conclusion:
  - Stable 100% precision is not available from the current Stage 1/Stage 2 features on this sampled validation setup.
  - The practical ceiling so far is roughly:
    - 80-85% precision if we keep useful recall.
    - 90%+ precision only with very small trade counts.
    - 100% only as a tiny, brittle pocket that fails to generalize.
  - Next real improvement likely needs new information, not just tighter thresholds:
    - Full-grid negatives.
    - More market context/higher timeframe context.
    - Regime filters.
    - Better oracle labels that distinguish "tradable trend start" from "technically oracle nearby."

Stage 1 live-grid pilot:

- Added script:
  - `rust_sr/abcd/scripts/ai_oracle_start_stage1_live_grid_model.py`.
- Purpose:
  - Build Stage 1 rows by walking the candle grid directly.
  - Use the oracle only as the historical label/grade.
  - Train on sampled negatives from the full grid.
  - Validate on the full live-style grid with cooldown and oracle matching.
- First attempted root:
  - `MNQ 2m` was rejected for 2024 because the current oracle run has no MNQ labels in 2024.
  - Switched pilot to `NQ 2m`, which has labels in 2024, 2025, and 2026.
- Baseline pilot:
  - Run: `aicw-oracle-start-livegrid-pilot-v1-2m-NQ-tr2024-v2026`.
  - Label: positive pre 2 bars through post 2 bars.
  - Negative ratio: 8.
  - Train: 2024.
  - Threshold: 2025.
  - Validate: 2026.
  - 2026 selected:
    - Threshold: 0.45.
    - Picked events: 13,380.
    - Matched picks: 5,338.
    - Pick match rate: 39.90%.
    - Oracle recall: 91.66%.
    - Picks per oracle: 2.30.
- Label/negative variants:
  - Run: `aicw-oracle-start-livegrid-pre0neg2-v1-2m-NQ-tr2024-v2026`.
    - Label: oracle start through +2 bars.
    - Negative ratio: 2.
    - 2026 selected:
      - Threshold: 0.50838274.
      - Picked events: 12,008.
      - Matched picks: 5,287.
      - Pick match rate: 44.03%.
      - Oracle recall: 90.78%.
      - Picks per oracle: 2.06.
  - Run: `aicw-oracle-start-livegrid-pre0neg8-v1-2m-NQ-tr2024-v2026`.
    - Label: oracle start through +2 bars.
    - Negative ratio: 8.
    - 2026 selected:
      - Threshold: 0.51415716.
      - Picked events: 11,915.
      - Matched picks: 5,273.
      - Pick match rate: 44.26%.
      - Oracle recall: 90.54%.
      - Picks per oracle: 2.05.
- Old sampled Stage 1 comparison on the same `NQ 2m / 2026` full grid:
  - Model: `aicw-oracle-start-v1-2m-tr2024-v2026`.
  - At threshold 0.50:
    - Picked events: 11,684.
    - Matched picks: 5,250.
    - Pick match rate: 44.93%.
    - Oracle recall: 90.14%.
  - At threshold 0.45:
    - Picked events: 12,523.
    - Matched picks: 5,374.
    - Pick match rate: 42.91%.
    - Oracle recall: 92.27%.
- Read:
  - Live-grid training is now implemented and auditable.
  - The first broad early-positive label was worse.
  - The old label shape, oracle start through +2 bars, is better.
  - Negative ratio 8 is slightly cleaner than 2, but the difference is small.
  - The old sampled all-root model still slightly edges the one-root live-grid pilot on NQ.
  - Do not promote live-grid Stage 1 yet.
  - Next best step is phased all-root live-grid training with the pre0/post2 label and neg8 negatives, then compare again on 2026.

Stage 1 live-grid all-timeframe specialist fire pass:

- Added/updated scripts:
  - `rust_sr/abcd/scripts/eval_oracle_trend_start_full_grid.py`.
    - Now supports live-grid Stage 1 model files.
    - Uses metadata root filters by default, preventing a specialist from scoring unrelated roots.
  - `rust_sr/abcd/scripts/run_oracle_start_stage1_live_grid_fire_batch.py`.
    - Runs every trained live-grid Stage 1 specialist over the 2026 full candle grid.
    - Stores per-model pick CSV and summary JSON in each model registry folder.
- Batch:
  - Run prefix: `aicw-oracle-start-livegrid-alltf-v1`.
  - Year scored: 2026.
  - Models scored: 192.
  - Failures: 0.
  - Total fired events: 661,305.
  - Mean pick match rate across specialists: 74.4%.
  - Mean oracle recall across specialists: 63.4%.
  - Batch summary:
    - `rust_sr/abcd/logs/stage1_livegrid_fires/aicw-oracle-start-livegrid-alltf-v1-2026-fire-batch-summary.json`.
- Timeframe read:
  - 1m:
    - 110,942 fires.
    - Average match rate 27.1%.
    - Average recall 71.4%.
    - Very useful as a wide/noisy radar and fakeout source for Stage 2.
  - 2m:
    - 92,916 fires.
    - Average match rate 36.3%.
    - Average recall 85.0%.
  - 3m:
    - 77,885 fires.
    - Average match rate 45.7%.
    - Average recall 91.2%.
  - 4m:
    - 60,074 fires.
    - Average match rate 56.3%.
    - Average recall 86.0%.
  - 5m-9m:
    - Cleaner middle band.
    - Average match rates rise from 71.2% to 84.9%.
    - Average recall falls from 76.4% to 62.3%.
  - 10m-15m:
    - Clean but sparser.
    - Average match rates 86.8%-91.1%.
    - Average recall 58.4%-44.7%.
  - 30m:
    - Very clean and sparse.
    - 10,042 fires.
    - Average match rate 95.6%.
    - Average recall 25.2%.
- Read:
  - The materialized Stage 1 fires are the right candidate universe for Stage 2.
  - Stage 2 should learn from these fires, not only from perfect oracle starts.
  - The 1m/2m specialists provide broad candidate discovery and many fakeouts.
  - The mid/high-minute specialists provide cleaner confirmation signals for managers.

Per-timeframe oracle batch:

- Added script:
  - `rust_sr/abcd/scripts/run_oracle_per_timeframe_batch.py`.
- Purpose:
  - Build a separate hindsight oracle run for each specialist timeframe.
  - These are intended to become the target labels for true timeframe-specific Stage 1 specialists.
- Batch:
  - Run prefix: `oracle-perfect-trends-tfspec-v1`.
  - Years: 2024, 2025, 2026.
  - Roots: `ES,RTY,YM,EMD,CL,RB,HO,NG,GF,LE,NKD,NQ`.
  - Timeframes: `1m,2m,3m,4m,5m,6m,7m,8m,9m,10m,11m,12m,13m,14m,15m,30m`.
  - Result: 16/16 oracle runs completed.
  - Batch summary:
    - `rust_sr/abcd/logs/oracle_per_timeframe/oracle-perfect-trends-tfspec-v1-batch-summary.json`.
- Oracle rows by timeframe:
  - 1m: 289,688 oracle trends, avg 4.31R.
  - 2m: 190,772 oracle trends, avg 4.44R.
  - 3m: 145,099 oracle trends, avg 4.52R.
  - 4m: 118,380 oracle trends, avg 4.58R.
  - 5m: 99,822 oracle trends, avg 4.63R.
  - 6m: 86,410 oracle trends, avg 4.70R.
  - 7m: 76,795 oracle trends, avg 4.76R.
  - 8m: 68,476 oracle trends, avg 4.82R.
  - 9m: 61,861 oracle trends, avg 4.87R.
  - 10m: 56,166 oracle trends, avg 4.93R.
  - 11m: 51,971 oracle trends, avg 4.97R.
  - 12m: 47,837 oracle trends, avg 5.03R.
  - 13m: 44,725 oracle trends, avg 5.08R.
  - 14m: 41,853 oracle trends, avg 5.12R.
  - 15m: 38,782 oracle trends, avg 5.18R.
  - 30m: 19,207 oracle trends, avg 5.81R.
- Read:
  - The oracle count naturally falls as timeframe increases.
  - The average oracle R rises as timeframe increases.
  - This gives each timeframe a proper target instead of forcing every specialist to chase the old shared 2m oracle.
  - Next step is to retrain Stage 1 live-grid specialists using each timeframe's matching `oracle-perfect-trends-tfspec-v1-{tf}-2024_2026` run.
