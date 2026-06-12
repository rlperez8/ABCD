# 2026-06-06 Stage 3 Candle-By-Candle Policy

## Question

Can Stage 3 manage a trade entirely candle by candle, with no fixed strategy stop and no fixed take-profit?

## Run

`rust_sr/abcd/scripts/ai_stage3_candle_policy_research.py`

Output:

`rust_sr/abcd/model_registry/aicw-stage3-candle-policy-v1-2m-tr2025-v2026`

## Design

- Entry source: Stage 2 confirmed trend-start entries from the prior target/bracket run.
- Roots: `CL, EMD, HO, NG, RB`
- Filter: Stage 2 score `>= 0.910`, risk `72-144` ticks.
- Entry: next candle open after Stage 2 confirmation.
- No fixed take-profit.
- No fixed strategy stop.
- The initial swing/ATR distance is only the R unit for scoring/sizing.
- Every closed candle becomes a policy decision: hold or exit on next open.
- Disaster stop was disabled for this research pass: `0.0R`.

Rows:

- Train entries: 619
- Threshold entries: 293
- Valid entries: 414
- Train decision rows: 109,411
- Threshold decision rows: 51,473
- Valid decision rows: 73,567

## Best 2026 Valid Result

Best policy threshold selected from the 2025 threshold slice:

- Exit threshold: `0.870629`
- Trades: 414
- Sum R: `-5.14R`
- Max DD: `112.42R`
- Avg R: `-0.012R`
- Win rate: `47.10%`
- Max concurrent: 8

Same selected rows using the existing terminal hard-stop/time-exit comparison:

- Sum R: `70.53R`
- Max DD: `31.23R`
- Avg R: `0.170R`
- Win rate: `25.60%`
- Max concurrent: 6

Best policy exit reasons:

- Model exit: 400
- Time exit: 14

## No-Stop Time-Exit Baseline

If the no-fixed-stop version simply holds every trade to the max window:

- Trades: 414
- Sum R: `-17.75R`
- Max DD: `161.93R`
- Avg R: `-0.043R`

## Verdict

Reject this first pure candle-policy version.

The result is important: removing the fixed hard stop entirely was harmful. The model exited too often and still produced larger drawdown than the terminal hard-stop comparison. The no-stop time-exit baseline was also bad, which means the protective stop was doing real work.

## Next

The better design is not fixed stop versus no stop.

The next version should be:

`dynamic protective stop updated every candle + model hold/exit decision`

In other words:

- The stop is not fixed from entry.
- The model can tighten, loosen within limits, or exit.
- There is still a broker-side disaster stop for live safety.
- The strategy stop should be a dynamic candle-by-candle risk line, not absent.
