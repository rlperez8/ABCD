# Experiment Template

Use this template for every meaningful test, even if the test is small or fails.

## Experiment ID

`EXP-YYYYMMDD-###`

## Question

What exact question are we trying to answer?

## Setup Type

Use one:

- `old_harmonic_pattern`
- `trend_detect`
- `execution_filter`
- `market_data`
- `infrastructure`
- `mixed`
- `unknown`

## Hypothesis

What do we expect to happen, and why?

## Setup

- Data years:
- Timeframe:
- Symbols / roots:
- Setup type:
- Strategy filters:
- Risk / sizing rules:
- Entry rule:
- Exit rule:
- Slippage assumption:
- Live compatibility:
- Model / script:
- Run id / output folder:
- Known exclusions:

## Command

```powershell

```

## Metrics To Compare

- Trades:
- Sum R:
- Max drawdown R:
- Average R:
- Win rate:
- PnL / return if account-based:
- Max concurrent trades:
- Prop pass/fail counts if relevant:

## Result

What happened?

## Verdict

Use one:

- `PROMOTE`
- `PROMISING`
- `NEEDS_VALIDATION`
- `REJECT`
- `INVALID_BUG`
- `DATA_ONLY`

## Why

Short explanation of what this test taught us.

## Next Step

What should be run or checked next?
