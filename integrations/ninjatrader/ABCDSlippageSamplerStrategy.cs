// NinjaTrader 8 Strategy source.
// Copy this file into Documents\NinjaTrader 8\bin\Custom\Strategies, then
// compile from the NinjaScript Editor. Run on a SIM account only.

#region Using declarations
using System;
using System.ComponentModel.DataAnnotations;
using System.Globalization;
using NinjaTrader.Cbi;
using NinjaTrader.Data;
using NinjaTrader.NinjaScript;
#endregion

namespace NinjaTrader.NinjaScript.Strategies
{
    public class ABCDSlippageSamplerStrategy : Strategy
    {
        private const string SamplerRunId = "slippage-sampler";
        private const string SamplerTemplate = "trigger-distance-market";
        private const string ExitSignalName = "ABCD_SLIP_EXIT";

        private double anchorPrice;
        private double lastPrice;
        private DateTime nextAllowedSampleUtc = DateTime.MinValue;
        private DateTime activeSampleStartedUtc = DateTime.MinValue;
        private int samplesSubmitted;
        private bool entryInFlight;
        private bool exitSubmitted;
        private bool limitLogged;
        private string activeEntrySignal;
        private string activeEntrySide;

        [NinjaScriptProperty]
        [Range(1, 10000)]
        [Display(Name = "Max samples", Order = 1, GroupName = "Sampler")]
        public int MaxSamples { get; set; }

        [NinjaScriptProperty]
        [Range(1, 100)]
        [Display(Name = "Trigger ticks", Order = 2, GroupName = "Sampler")]
        public int TriggerTicks { get; set; }

        [NinjaScriptProperty]
        [Range(1, 300)]
        [Display(Name = "Cooldown seconds", Order = 3, GroupName = "Sampler")]
        public int CooldownSeconds { get; set; }

        [NinjaScriptProperty]
        [Range(1, 100)]
        [Display(Name = "Order quantity", Order = 4, GroupName = "Sampler")]
        public int OrderQuantity { get; set; }

        [NinjaScriptProperty]
        [Display(Name = "Enable buy samples", Order = 5, GroupName = "Sampler")]
        public bool EnableBuySamples { get; set; }

        [NinjaScriptProperty]
        [Display(Name = "Enable sell samples", Order = 6, GroupName = "Sampler")]
        public bool EnableSellSamples { get; set; }

        protected override void OnStateChange()
        {
            if (State == State.SetDefaults)
            {
                Name = "ABCDSlippageSamplerStrategy";
                Description = "Samples simulated market-order slippage after price moves a small trigger distance up or down.";
                Calculate = Calculate.OnEachTick;
                EntriesPerDirection = 1;
                EntryHandling = EntryHandling.AllEntries;
                IsExitOnSessionCloseStrategy = true;
                ExitOnSessionCloseSeconds = 30;
                IncludeCommission = true;
                BarsRequiredToTrade = 1;
                StartBehavior = StartBehavior.WaitUntilFlat;
                RealtimeErrorHandling = RealtimeErrorHandling.StopCancelClose;

                MaxSamples = 100;
                TriggerTicks = 2;
                CooldownSeconds = 5;
                OrderQuantity = 1;
                EnableBuySamples = true;
                EnableSellSamples = true;
            }
            else if (State == State.Realtime)
            {
                anchorPrice = 0;
                Log("ABCD Slippage Sampler live on " + Instrument.FullName
                    + " max=" + MaxSamples.ToString(CultureInfo.InvariantCulture)
                    + " triggerTicks=" + TriggerTicks.ToString(CultureInfo.InvariantCulture)
                    + " qty=" + OrderQuantity.ToString(CultureInfo.InvariantCulture));
            }
            else if (State == State.Terminated)
            {
                Log("ABCD Slippage Sampler terminated after "
                    + samplesSubmitted.ToString(CultureInfo.InvariantCulture)
                    + " submitted sample(s).");
            }
        }

        protected override void OnMarketData(MarketDataEventArgs marketDataUpdate)
        {
            if (marketDataUpdate == null || marketDataUpdate.MarketDataType != MarketDataType.Last)
                return;

            lastPrice = marketDataUpdate.Price;
            TrySample();
        }

        protected override void OnBarUpdate()
        {
            if (State != State.Realtime || CurrentBar < BarsRequiredToTrade)
                return;

            if (lastPrice <= 0)
                lastPrice = Close[0];

            TrySample();
        }

        protected override void OnExecutionUpdate(
            Execution execution,
            string executionId,
            double price,
            int quantity,
            MarketPosition marketPosition,
            string orderId,
            DateTime time)
        {
            if (execution == null || execution.Order == null)
                return;

            string orderName = execution.Order.Name ?? "";

            if (!string.IsNullOrWhiteSpace(activeEntrySignal)
                && string.Equals(orderName, activeEntrySignal, StringComparison.Ordinal)
                && !exitSubmitted)
            {
                exitSubmitted = true;
                if (string.Equals(activeEntrySide, "SHORT", StringComparison.OrdinalIgnoreCase))
                    ExitShort(Math.Max(1, quantity), ExitSignalName, activeEntrySignal);
                else
                    ExitLong(Math.Max(1, quantity), ExitSignalName, activeEntrySignal);

                return;
            }

            if (string.Equals(orderName, ExitSignalName, StringComparison.Ordinal))
            {
                entryInFlight = false;
                exitSubmitted = false;
                activeEntrySignal = null;
                activeEntrySide = null;
                nextAllowedSampleUtc = DateTime.UtcNow.AddSeconds(Math.Max(1, CooldownSeconds));
                ResetSampler(lastPrice > 0 ? lastPrice : price);
            }
        }

        protected override void OnOrderUpdate(
            Order order,
            double limitPrice,
            double stopPrice,
            int quantity,
            int filled,
            double averageFillPrice,
            OrderState orderState,
            DateTime time,
            ErrorCode error,
            string nativeError)
        {
            if (order == null || string.IsNullOrWhiteSpace(order.Name))
                return;

            bool isSamplerOrder = order.Name.StartsWith("ABCD", StringComparison.OrdinalIgnoreCase)
                || string.Equals(order.Name, ExitSignalName, StringComparison.Ordinal);
            if (!isSamplerOrder)
                return;

            Log("ABCD Slippage Sampler order "
                + order.Name
                + " state=" + orderState
                + " filled=" + filled.ToString(CultureInfo.InvariantCulture)
                + "/" + quantity.ToString(CultureInfo.InvariantCulture)
                + " avg=" + averageFillPrice.ToString(CultureInfo.InvariantCulture)
                + " error=" + error
                + (string.IsNullOrWhiteSpace(nativeError) ? "" : " native=" + nativeError));
        }

        private void TrySample()
        {
            if (State != State.Realtime || lastPrice <= 0)
                return;

            if (samplesSubmitted >= MaxSamples)
            {
                if (!limitLogged)
                {
                    limitLogged = true;
                    Log("ABCD Slippage Sampler reached max samples: " + MaxSamples.ToString(CultureInfo.InvariantCulture));
                }
                return;
            }

            if (entryInFlight)
            {
                RecoverIfStale();
                return;
            }

            if (Position.MarketPosition != MarketPosition.Flat)
                return;

            if (DateTime.UtcNow < nextAllowedSampleUtc)
                return;

            if (anchorPrice <= 0)
                ResetSampler(lastPrice);

            double distance = Math.Max(1, TriggerTicks) * TickSize;
            double upTrigger = RoundToTick(anchorPrice + distance);
            double downTrigger = RoundToTick(anchorPrice - distance);

            if (EnableBuySamples && lastPrice >= upTrigger)
            {
                SubmitSample("LONG", "UP_TRIGGER_BUY", upTrigger);
                return;
            }

            if (EnableSellSamples && lastPrice <= downTrigger)
                SubmitSample("SHORT", "DOWN_TRIGGER_SELL", downTrigger);
        }

        private void SubmitSample(string side, string setup, double expectedPrice)
        {
            samplesSubmitted++;
            entryInFlight = true;
            exitSubmitted = false;
            activeSampleStartedUtc = DateTime.UtcNow;
            activeEntrySide = side;
            activeEntrySignal = BuildOrderTag(side, setup, expectedPrice, samplesSubmitted);

            Log("ABCD Slippage Sampler sample "
                + samplesSubmitted.ToString(CultureInfo.InvariantCulture)
                + "/" + MaxSamples.ToString(CultureInfo.InvariantCulture)
                + " " + setup
                + " expected=" + expectedPrice.ToString(CultureInfo.InvariantCulture)
                + " last=" + lastPrice.ToString(CultureInfo.InvariantCulture));

            if (string.Equals(side, "SHORT", StringComparison.OrdinalIgnoreCase))
                EnterShort(Math.Max(1, OrderQuantity), activeEntrySignal);
            else
                EnterLong(Math.Max(1, OrderQuantity), activeEntrySignal);
        }

        private void RecoverIfStale()
        {
            if ((DateTime.UtcNow - activeSampleStartedUtc).TotalSeconds < 30)
                return;

            if (Position.MarketPosition == MarketPosition.Flat)
            {
                entryInFlight = false;
                exitSubmitted = false;
                activeEntrySignal = null;
                activeEntrySide = null;
                nextAllowedSampleUtc = DateTime.UtcNow.AddSeconds(Math.Max(1, CooldownSeconds));
                ResetSampler(lastPrice);
                return;
            }

            if (!exitSubmitted && !string.IsNullOrWhiteSpace(activeEntrySignal))
            {
                exitSubmitted = true;
                if (Position.MarketPosition == MarketPosition.Short)
                    ExitShort(Math.Max(1, OrderQuantity), ExitSignalName, activeEntrySignal);
                else if (Position.MarketPosition == MarketPosition.Long)
                    ExitLong(Math.Max(1, OrderQuantity), ExitSignalName, activeEntrySignal);
            }
        }

        private void ResetSampler(double price)
        {
            anchorPrice = RoundToTick(price);
        }

        private double RoundToTick(double price)
        {
            double tickSize = TickSize > 0 ? TickSize : 0.25;
            return Math.Round(price / tickSize, MidpointRounding.AwayFromZero) * tickSize;
        }

        private string BuildOrderTag(string side, string setup, double expectedPrice, int sampleNumber)
        {
            string sideCode = string.Equals(side, "SHORT", StringComparison.OrdinalIgnoreCase) ? "S" : "L";
            string setupCode = string.Equals(setup, "DOWN_TRIGGER_SELL", StringComparison.OrdinalIgnoreCase) ? "D" : "U";

            return "ABCD"
                + "|i=" + sampleNumber.ToString("0000", CultureInfo.InvariantCulture)
                + "|r=SS"
                + "|u=" + setupCode
                + "|t=TM"
                + "|a=" + sideCode
                + "|p=" + expectedPrice.ToString(CultureInfo.InvariantCulture);
        }

        private static string CleanTag(string value)
        {
            if (string.IsNullOrWhiteSpace(value))
                return "";
            return value.Replace("|", "_").Replace("=", "_").Trim();
        }

        private static void Log(string message)
        {
            NinjaTrader.Code.Output.Process(message, PrintTo.OutputTab1);
        }
    }
}
