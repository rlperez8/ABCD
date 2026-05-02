# Recovery Rebuild Checklist

This file is the project recovery source of truth after the May 2, 2026 local cleanup loss.
The working recovery repo is `abcd_local_v3_github_shallow_20260502_165339` on branch `recovery/rebuild-local-work`.

## Already Recovered

- Restored the repo from GitHub and created the recovery branch.
- Restored `.env` for `rust_sr/abcd` from VS Code local history.
- Reinstalled frontend dependencies with `npm ci`.
- Rebuilt futures-engine local work:
  - `ABCD_CANDLE_SOURCE=futures_contracts`
  - `ABCD_FUTURES_ROOT`
  - 1-minute futures candle timestamps
  - delayed D-confirmed entry on the next candle
  - C target with mirrored stop from actual entry
  - pre-entry C breach invalidation
  - contract week/day fields
  - `ABCD_SKIP_PROP_FAMILY_SUMMARIES`
  - legacy `xabcd_patterns` removed from runtime paths
- Verified `cargo check --bin abcd` and server `cargo check`.

## Phase 1 - UI Recovery

- Restore the left-side Pattern Identity menu.
- Keep Pattern Identity feature groups closed by default.
- Rename identity labels:
  - Pattern -> Dominant Harmonic
  - Bin -> Price Ratio Accuracy
  - Time -> Time Ratio Accuracy
- Add the simulator tab beside Canvas and Graphs.
- Let the simulator show the selected family ID and allow manual override.
- Add fixed-width simulator controls for first start date, tests to chain, contracts, account size, and one-trade-at-a-time.
- Add Apex rule reference UI and a replay/results shell.

## Phase 2 - Simulator API

- Add backend endpoint for running a historical family replay.
- Input: family id, account profile, start date, chain count, contracts, one-trade-at-a-time flag.
- Query patterns sorted by confirmed entry date, not raw D date.
- Trade only the selected family when family-only mode is active.
- Respect confirmed pivot entry timing.

## Phase 3 - Rule Engine

- Track account balance, realized PnL, open PnL, peak balance, drawdown, daily loss, and max contracts.
- Stop each test as soon as it passes or fails.
- Chain the next test from the previous completion date.
- Persist each test and its trades separately.
- Color passed tests greenish and failed tests reddish.

## Phase 4 - Replay And Results

- Results graph toggles between per-trade PnL and total PnL climb.
- Drawdown is visible with PnL.
- Replay page shows a family pattern timeline/calendar.
- Timeline marks all family patterns, trades taken, how long each trade lasted, and how many eligible patterns occurred while already in a trade.

## Phase 5 - Rule Audit

- Keep a simulator rules page for Apex settings.
- Re-check Apex official pages before relying on rule values for real decisions.
- Add source links and last-verified date to the UI.

## Imported Old Chat 1 - Needs Rebuild

This batch was compared against the recovered branch on May 2, 2026. Most of it is not currently present in the recovered code.

### Project Direction

- Move primary research direction from stocks/daily candles toward futures/prop testing with fewer symbols and lower timeframes.
- Start futures testing around 15m, with one additional timeframe possible later.
- Keep family as pattern/setup identity only.
- Treat session, trend, timeframe, volatility, and similar context as later layers/filters.
- Remove 3M/6M/12M trend from family identity and rollups.
- Keep baseline target/exit rule as D entry to exact C pivot price.

### Engine And Rollup Rebuild

- Remove target-candle assignment from the main scan loop.
- Keep normal swing D-leg logic, not target-candle logic.
- Classify completed D reversal after D is formed.
- Prop outcomes should use completed trade result, not target candle color.
- Target fields should represent swing outcome:
  - `target_open = entry`
  - `target_close = exit`
  - `target_high/target_low = max/min of entry and exit`
- Change family rollup key version from `prop-family-v3` to `prop-family-v4`.
- Set family trend columns to `All`.
- Remove trend from `canonical_prop_strategy_key`.
- Remove trend from DReversal route strategy id.
- Confirm `clear_engine_tables` only clears generated engine/rollup tables and preserves candles.

### Server Rebuild

- Remove trend filters from `/strategy-candidates` and `/current-setup-strategies`.
- Stop requiring trend dimensions in family trade/detail queries.
- Keep `/strategy-trades` resolving family keys into family dimensions.
- Fix `/pattern-detail` so it accepts `prop_strategy_id`, resolves family key to actual dimensions, and avoids exact rounded price/length filters when `pattern_id` is present.

### Client Rebuild

- Remove 3M/6M/12M strategy filter state and UI groups.
- Remove client-side trend matching for family rows.
- Stop sending trend fields to family trade/comparison calls.
- Stop sending rounded trade price/length filters to `/pattern-detail`.
- Remove trend from strategy IDs.
- Remove trend columns from the family table.
- Remove trend cards from selected family profile.
- Export `rankStrategies` from `StrategyLeaderboardCard`.
- Make Leaders graph use the table-sorted family array instead of independently sorting.
- Add `leaderChartStartIndex` so Leaders graph follows the family table visible range.
- Add family-table scroll tracking and show rank windows like `Family table ranks 25-36 of 180`.

### Chart Rebuild

- Restore D leg on canvas.
- Make D/trade phase visually distinct.
- Re-add R:R zones as a toggle.
- R:R target should use exact C pivot/target price.
- Rename/rework target-candle UI concept as swing exit.
- Canvas should use `trade_date` / `target_date` and exact `exit_price` / `trade_current_price`.

### Later TODOs From This Batch

- Add futures symbols/contracts.
- Add session layer per family and overall session stats across families.
- Add trend as a context layer, not family dimension.
- Consider multi-timeframe confirmation later.
- Revisit D detection improvements:
  - 5-candle pivot instead of 3
  - break buffer beyond B
  - deepest valid D before reversal
  - pattern-ratio-based D zone
- Decide whether Leaders chart scroll-follow should become smooth scroll, paging, or a locked visible range.
- Consider a small mini-scroll/slider for Leaders chart.
- Confirm yearly win-rate chart clamps/labels 0-100%.
- Re-run engine after futures/timeframe changes.

## Safety

- Commit every recovery phase.
- Avoid deleting recovered files unless they are clearly generated artifacts.
- Keep the old damaged folder untouched unless the user explicitly asks otherwise.
