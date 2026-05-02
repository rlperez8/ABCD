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

## Safety

- Commit every recovery phase.
- Avoid deleting recovered files unless they are clearly generated artifacts.
- Keep the old damaged folder untouched unless the user explicitly asks otherwise.
