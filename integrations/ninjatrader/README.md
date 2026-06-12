# NinjaTrader Slippage Bridge

This bridge is for measuring real execution slippage before the ABCD app is
allowed to place trades.

## What It Does

1. NinjaTrader listens for account execution updates.
2. Each fill is posted to the local ABCD server:

   `POST http://127.0.0.1:8080/ninjatrader/executions`

3. The server stores fills in:

   `ninjatrader_execution_fills`

4. Slippage summaries can be queried from:

   `POST http://127.0.0.1:8080/ninjatrader/slippage`

## Install

1. Start the ABCD Rust server.
1. Copy `ABCDSlippageBridgeAddOn.cs` into:

   `Documents\NinjaTrader 8\bin\Custom\AddOns`

1. Copy `ABCDSignalQueueStrategy.cs` into:

   `Documents\NinjaTrader 8\bin\Custom\Strategies`

1. Copy `ABCDSlippageSamplerStrategy.cs` into:

   `Documents\NinjaTrader 8\bin\Custom\Strategies`

1. Copy `ABCDCandlePublisherStrategy.cs` into:

   `Documents\NinjaTrader 8\bin\Custom\Strategies`

1. Open NinjaTrader.
1. Open the NinjaScript Editor and compile.
1. Edit `AccountName` in the AddOn/Strategy sources if you want an account
   other than `DEMO5859105`.

## Candle Publisher Flow

Run `ABCDCandlePublisherStrategy` on an HO or MHO chart when you want live
closed bars stored locally for model/trend scanning without placing orders.

Default publisher settings:

- `Publish historical bars`: false
- `Log every bars`: 10
- Endpoint: `POST http://127.0.0.1:8080/ninjatrader/candles`

The server stores bars in:

`ninjatrader_live_candles`

For the current model path, start with an HO chart on the same timeframe as the
model, then later map any accepted signal to MHO execution. MHO can also be
published directly, but our historical MHO feed is sparse, so HO is the cleaner
signal source.

## Signal Queue Test Flow

1. Run the `ABCDSignalQueueStrategy` on the chart/instrument you want to test.
2. Make sure the strategy is enabled on account `DEMO5859105`.
3. Queue a signal in the ABCD server:

   `POST http://127.0.0.1:8080/ninjatrader/signals`

4. The strategy polls:

   `POST http://127.0.0.1:8080/ninjatrader/signals/pending`

5. When the last price touches the expected entry:

   - Long triggers when `last <= expected_price`.
   - Short triggers when `last >= expected_price`.

6. NinjaTrader places a market order and tags it with the expected price.
7. The AddOn records the fill through `/ninjatrader/executions`.
8. Slippage can be queried through `/ninjatrader/slippage`.

## Slippage Sampler Flow

Run `ABCDSlippageSamplerStrategy` on a SIM chart when you want repeated fill
samples without using an ABCD trade signal.

Default sampler settings:

- `Max samples`: 100
- `Trigger ticks`: 2
- `Cooldown seconds`: 5
- `Order quantity`: 1
- `Enable buy samples`: true
- `Enable sell samples`: true

The sampler anchors at the latest live price. If price moves up by the trigger
distance, it submits a tagged buy market order. If price moves down by the
trigger distance, it submits a tagged sell/short market order. After the entry
fill, it immediately sends a market exit and waits for the cooldown before
sampling again.

Only the entry orders are tagged with expected price data. That keeps buy and
sell slippage samples clean in `ninjatrader_execution_fills`, while the exit
fills are still stored as normal execution records.

## Expected Price Tags

The bridge can record actual fills without expected prices. To compute slippage,
the order name should include expected trade tags.

Supported order name format:

```text
ABCD|ai=aimv-1779936115494-235564|setup=SETUP_ID|tpl=TEMPLATE_UID|side=LONG|expected=123.45|time=2026-01-02 09:31:00.000
```

Useful tags:

- `ai`, `run`, or `ai_run`
- `setup` or `setup_id`
- `tpl`, `template`, or `template_uid`
- `side` or `direction`
- `expected`, `expected_price`, or `price`
- `time` or `expected_time`

## Slippage Meaning

The server stores both:

- `raw_slippage_points`: actual fill price minus expected price.
- `adverse_slippage_points`: positive means worse fill, negative means price
  improvement. For buy fills, higher actual price is worse. For sell fills,
  lower actual price is worse.

`adverse_slippage_ticks` divides adverse slippage by the instrument tick size.
