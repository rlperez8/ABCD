# Experiment Log

This is the project lab notebook.

The goal is to track what was tested, why it was tested, what happened, and whether it should be promoted, rejected, or retested. This file is intentionally plain and readable. The companion CSV is:

`rust_sr/abcd/model_registry/research_notes/experiment_ledger.csv`

## Rules Going Forward

Every meaningful test should answer:

- Was this the old harmonic pattern setup, the trend-detect setup, or infrastructure/execution work?
- What question was tested?
- What exact data was used?
- What changed from the prior run?
- What filters were active? For example symbols/roots, hours, score thresholds, risk ticks, max-open, daily stop, liquidity exclusions, or direction limits.
- What result metrics were recorded? At minimum trades, sum R, max DD R, average R, win rate, and account PnL/return when account-based.
- What metric improved or got worse?
- Was the test live-compatible?
- Was it promoted, rejected, or left as promising?
- Where is the output folder?

Verdicts:

- `PROMOTE`: strong enough to become the current main candidate.
- `PROMISING`: worth more testing, not main yet.
- `NEEDS_VALIDATION`: interesting, but missing years or walk-forward checks.
- `REJECT`: tested and not worth continuing right now.
- `INVALID_BUG`: result was polluted by a bug, leakage, non-live logic, or bad assumptions.
- `DATA_ONLY`: infrastructure/data work, not a strategy result.

Setup types:

- `old_harmonic_pattern`: original ABCD/harmonic pattern build/playbook/simulation path.
- `trend_detect`: newer trend-start / candle-wave / market-wave / Stage 1-3 AI path.
- `execution_filter`: slippage, liquidity, or tradability filters.
- `market_data`: candles, trend tables, derived timeframes, or source data work.
- `infrastructure`: UI, DB structure, runners, logging, or experiment tooling.
- `mixed`: intentionally combines old pattern and newer trend/AI layers.
- `unknown`: not enough evidence yet.

## Core CSV Fields

The CSV ledger is the sortable source for comparing tests. These fields should be filled whenever the result has them:

- `setup_type`: old harmonic pattern, trend detect, execution filter, market data, infrastructure, mixed, or unknown.
- `data_years`, `timeframe`, `symbols_roots`: the exact market slice tested.
- `strategy_filters`: score thresholds, symbol filters, daily stops, max-open, hours, risk windows, top-N rules, liquidity exclusions, or other knobs.
- `risk_sizing`, `entry_rule`, `exit_rule`, `slippage`: the actual trade mechanics.
- `trades`, `sum_r`, `max_dd_r`, `avg_r`, `win_rate`: the main strategy scoreboard.
- `pnl`, `return_pct`, `max_concurrent`: account-style and live-capacity scoreboard.
- `live_compatible`: whether the test could be run candle-by-candle without future data or non-live fallback logic.

## Current Main Scientific Takeaway

The system can find good trades, but raw signal streams are still too noisy. Account governors such as max-open and daily-loss stops improve results, but the cleaner long-term path is:

`trend detection -> trade-quality selector -> conflict ranking -> account governor -> live timing proof`

The best current improvement path is the terminal trade-quality layer using Stage 2 features. It showed a strong 2025-to-2026 result, but reverse validation was weaker, so it is not promoted yet.

## Backfill Status

This log is backfilled from artifacts currently on disk plus recent research notes. It is not yet a perfect record of every early UI/playbook action. The old build/playbook/simulation DB runs should be extracted later into this same format.

## Major Backfilled Experiments

| ID | Date | Area | Setup Type | Question | Key Setup | Result | Verdict | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| EXP-LEGACY-001 | Pre-ledger | Build / Playbook / Prop Sim | old_harmonic_pattern | Could the original pattern/playbook workflow produce useful prop-firm style results? | P4 era build/playbook/sim testing UI, prop summaries, raw rows, DB-backed sections | Useful enough to continue, but early records are scattered and need DB backfill | NEEDS_VALIDATION | Legacy DB/UI artifacts |
| EXP-LEGACY-002 | Pre-ledger | UI / DB provenance | infrastructure | Should simulation testing be DB-backed instead of UI-computed? | Split Playbook Test Overview, Prop Simulation Overview, prop cycles, workload, trend sections | Direction accepted: UI should display stored DB results, runner computes and stores after run | DATA_ONLY | UI and runner changes |
| EXP-LEGACY-003 | Pre-ledger | Trends | market_data | Add higher timeframe trend context without rerunning pattern scanner | 5m, 15m, 1h trend tables joined to sim trades | Useful as separate market-data layer, not family/pattern feature | DATA_ONLY | Trend table/UI work |
| EXP-20260529-001 | 2026-05-29 | Market wave / AI trades | trend_detect | Can market-wave meta models pick profitable 2026 trades after slippage? | `aimv-market-wave-*`, `aimv-mwmeta-*` series | Some promising runs, but tiny entry/exit distances and slippage realism became a major problem | NEEDS_VALIDATION | `rust_sr/abcd/model_registry/aimv-*` |
| EXP-20260529-002 | 2026-05-29 | Slippage / execution quality | execution_filter | Do tiny trades survive realistic slippage? | Execution-quality floor, 3+3 tick stress, NinjaTrader sampler later | Tiny target/stop trades were not realistic enough; led to minimum move/risk filtering | PROMISING | `aimv-mwmeta-v3-v7-twin-xq*`, Ninja logs |
| EXP-20260530-001 | 2026-05-30 | Candle wave baseline | trend_detect | What is the protected 2m candle-wave baseline? | `aicw-mtf-eg1-xtight-t014-rd3-en4-v1-2m-2026` | 402 trades, 222.24R, 0.553R avg, 5.02R DD | PROMISING | `2026-05-30-candle-wave-edge.md` |
| EXP-20260530-002 | 2026-05-30 | Candle wave tuning | trend_detect | Does threshold/loss brake improve baseline? | `aicw-research-t01425-lb25allm3-v1-2m-2026` | 397 trades, 230.05R, 5.02R DD; 2025 WF improved to 106.35R / 19.32R DD | PROMISING | `2026-05-30-candle-wave-edge.md` |
| EXP-20260530-003 | 2026-05-30 | Energy cap | trend_detect | Does a stronger 5th energy slot help? | `aicw-research-t014-rd3en5x06-v1-2m-2026` | 235.15R in 2026, but 2025 WF slipped from 99.06R to 97.00R and DD rose | NEEDS_VALIDATION | `2026-05-30-candle-wave-edge.md` |
| EXP-20260530-004 | 2026-05-30 | Liquidity / outlier audit | execution_filter | Was the huge SIJ6 winner tradable/reliable? | Liquidity audit around SIJ6 110.25R winner | Removing SI dropped robust 2026 from 230.05R to 98.92R; edge still existed but much smaller | NEEDS_VALIDATION | `2026-05-30-candle-wave-edge.md` |
| EXP-20260531-001 | 2026-05-31 | Data | market_data | Add Databento candles and tradable roots | 1m candle updates, priority root adds, 2020 add-on | More history and roots available; derived timeframes still need consistency checks before full validation | DATA_ONLY | Databento load logs |
| EXP-20260601-001 | 2026-06-01 | Oracle trend starts | trend_detect | Can oracle-labeled trend starts train Stage 1/Stage 2? | CatBoost, LightGBM, XGBoost, stage 1/2 oracle experiments | Architecture formed, but early attempts needed better caching and validation design | PROMISING | `aicw-oracle-start-*`, `aicw-stage2-*` |
| EXP-20260603-001 | 2026-06-03 | CNN | trend_detect | Should CNN be used as a major live scanner? | CNN/tower experiments around NQ/all timeframes | CNN looked interesting but too slow/scaling-risky as a first scanner | REJECT | CNN model folders and later note |
| EXP-20260604-001 | 2026-06-04 | Model hierarchy | trend_detect | Compare specialist/manager levels and Cat/Light/XGB blocks | L1/L2/L3/L4/L5 manager experiments | Complexity grew quickly; 2m single-timeframe path became preferred for speed and clarity | NEEDS_VALIDATION | `aicw-oracle-start-level*`, `aicw-os-l2-*` |
| EXP-20260605-001 | 2026-06-05 | Mainline stage models | trend_detect | Create 2m all-root Cat/Light/XGB plus Level 2 manager | `aicw-os-cat/light/xgb-l1-*`, `aicw-os-l2-three-l1-*` | Became current Stage 1/Level 2 source for later Stage 2/3 terminal research | PROMISING | `aicw-os-l2-three-l1-2m-p1p1n6m3-v1-ALL-2m-tr2025-v2026` |
| EXP-20260606-001 | 2026-06-06 | Terminal hold | trend_detect | Are raw Stage 2 terminal candidates good enough by themselves? | `aicw-os-stage3-terminal-both-*` | Raw streams were negative overall: 2026 terminal hold `-767.58R`, 2025 `-256.50R` | REJECT | Stage3 terminal metadata |
| EXP-20260606-002 | 2026-06-06 | Live-paper baseline | trend_detect | Can the live-style loop replay 2026 with account rules? | `prototype-live-stage2-terminal-runner-2026-full` and baseline copy | Corrected estimate around ending `$1,943.49`, +94.35%, DD about `$421.18`, 342 closed trades | NEEDS_VALIDATION | `prototype-live-stage2-terminal-runner-2026-full*` |
| EXP-20260606-003 | 2026-06-06 | Symbol filter | trend_detect | Do root groups help under fixed global rules? | `score >= 0.93`, risk 48-192, max open 3, daily loss cap | Best clean group `EMD,HO,NG,NQ,RTY,YM`: 2025 +$1,228.26, 2026 +$781.02 | NEEDS_VALIDATION | `2026-06-06-symbol-group-filters-and-cnn-layer.md` |
| EXP-20260606-004 | 2026-06-06 | Profit push | trend_detect | Can symbol groups push profit with manageable DD? | `score >= 0.915`, risk 60-240, max open 2, stop after 1 loss | Best balanced push `CL,EMD,ES,HO,NQ,YM`: 2025 +$806.90, 2026 +$843.76 | NEEDS_VALIDATION | `2026-06-06-profit-push-symbol-groups.md` |
| EXP-20260606-005 | 2026-06-06 | Pure R no-cap | trend_detect | What if every qualifying trade is counted with no capital/max-open bottleneck? | No account cap, no daily stop, overlap allowed | Better pure-R group `CL,EMD,HO,NG,RB`: 2025 73.60R / 19.67R DD, 2026 70.53R / 31.23R DD | PROMISING | `2026-06-06-profit-push-symbol-groups.md` |
| EXP-20260606-006 | 2026-06-06 | Trade-quality V1 | trend_detect | Can a thin live-compatible quality selector improve raw trades? | Terminal rows only: root/symbol/direction/hour/score/risk | Mixed transfer; 2025->2026 positive, reverse weak/tiny | REJECT | `aicw-terminal-trade-quality-v1-2025-2026` |
| EXP-20260606-007 | 2026-06-06 | Trade-quality V2 | trend_detect | Does adding Stage 2 entry-time features improve selection? | Stage 2 scores, Cat/Light/XGB scores, L1 agreement, confirmation offset | 2025->2026 improved to 117.51R / 19.39R DD; reverse 2026->2025 weak at 11.15R / 21.30R DD | PROMISING | `2026-06-06-terminal-trade-quality-layer.md` |
| EXP-20260606-008 | 2026-06-06 | Exit manager check | trend_detect | Are existing Stage 3 exit managers the breakthrough? | Existing `aicw-os-stage3-trend-manager-*` artifacts | Some improved bad baseline, but full 2026 streams still negative; not the current path | REJECT | `2026-06-06-terminal-trade-quality-layer.md` |
| EXP-20260606-009 | 2026-06-06 | Stage 2 target + Stage 3 bracket | trend_detect | Can Stage 2 predict target and Stage 3 trade a fixed TP bracket? | Predicted MFE target, swing/ATR hard stop, early momentum/giveback fade | Best valid bracket was 5.09R / 17.85R DD versus same-row terminal 94.39R / 35.25R DD; too conservative | REJECT | `2026-06-06-stage2-target-stage3-bracket.md` |
| EXP-20260606-010 | 2026-06-06 | Stage 3 candle policy | trend_detect | Can Stage 3 manage each trade candle by candle with no fixed stop or TP? | Model hold/exit every candle, no strategy stop, no fixed TP, disaster stop disabled | Best valid policy was -5.14R / 112.42R DD versus same-row terminal 70.53R / 31.23R DD; no-stop was unsafe | REJECT | `2026-06-06-stage3-candle-policy.md` |

## Current Open Questions

1. Generate 2021-2024 terminal-stage rows so symbol groups and quality selector can be tested across more years.
2. Decide whether `CL, EMD, HO, NG, RB` remains the clean pure-R group after full-history validation.
3. Train quality selector on 2021-2025 and test on 2026.
4. Add a standard summary exporter so every run writes one small `experiment_summary.json`.
5. Backfill early build/playbook/simulation DB runs into this ledger.
