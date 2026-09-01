// NinjaTrader 8 Strategy source.
// Copy this file into Documents\NinjaTrader 8\bin\Custom\Strategies, then
// compile from the NinjaScript Editor. Run it on the chart/instrument you want
// to test and choose account DEMO5859105 in the strategy settings.

#region Using declarations
using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Net;
using System.Text;
using System.Threading;
using NinjaTrader.Cbi;
using NinjaTrader.Data;
using NinjaTrader.NinjaScript;
#endregion

namespace NinjaTrader.NinjaScript.Strategies
{
    public class ABCDSignalQueueStrategy : Strategy
    {
        private const string BridgeBaseUrl = "http://127.0.0.1:8080";
        private const string AccountName = "DEMO5859105";
        private const string ClientId = "nt8-abcd-signal-queue";
        private const int MaxHoldBars = 180;

        private readonly object signalLock = new object();
        private List<SignalRow> signals = new List<SignalRow>();
        private DateTime lastPollUtc = DateTime.MinValue;
        private DateTime lastStatusPostUtc = DateTime.MinValue;
        private DateTime lastHeartbeatUtc = DateTime.MinValue;
        private double lastPrice;
        private bool polling;
        private readonly HashSet<string> submittedSignals = new HashSet<string>();
        private readonly HashSet<string> quickExitSignals = new HashSet<string>();
        private string activeSignalUid;
        private string activeEntryOrderTag;
        private bool timeExitSubmitted;
        private Timer queueTimer;
        private DateTime lastTimerErrorUtc = DateTime.MinValue;
        private DateTime activeEntryTime = DateTime.MinValue;
        private DateTime lastOpenSignalRecoveryUtc = DateTime.MinValue;

        protected override void OnStateChange()
        {
            if (State == State.SetDefaults)
            {
                Name = "ABCDSignalQueueStrategy";
                Description = "Polls ABCD local signal queue and submits simulated market orders when entry price is touched.";
                Calculate = Calculate.OnEachTick;
                EntriesPerDirection = 1;
                EntryHandling = EntryHandling.AllEntries;
                IsExitOnSessionCloseStrategy = false;
                IncludeCommission = true;
                BarsRequiredToTrade = 1;
            }
            else if (State == State.Configure)
            {
                Log("ABCD Signal Queue configured for account " + AccountName);
            }
            else if (State == State.Realtime)
            {
                Log("ABCD Signal Queue live on " + Instrument.FullName + " account " + AccountName);
                StartQueueTimer();
            }
            else if (State == State.Terminated)
            {
                StopQueueTimer();
                Log("ABCD Signal Queue terminated on " + (Instrument != null ? Instrument.FullName : "unknown instrument"));
            }
        }

        protected override void OnMarketData(MarketDataEventArgs marketDataUpdate)
        {
            if (marketDataUpdate == null || marketDataUpdate.MarketDataType != MarketDataType.Last)
                return;

            lastPrice = marketDataUpdate.Price;
            ProcessQueue();
        }

        protected override void OnBarUpdate()
        {
            if (CurrentBar < BarsRequiredToTrade)
                return;

            ProcessQueue();
            ManageTimeExit();
        }

        private void StartQueueTimer()
        {
            StopQueueTimer();
            queueTimer = new Timer(_ =>
            {
                try
                {
                    TriggerCustomEvent(o => ProcessQueue(), null);
                }
                catch (Exception ex)
                {
                    if ((DateTime.UtcNow - lastTimerErrorUtc).TotalSeconds >= 15)
                    {
                        lastTimerErrorUtc = DateTime.UtcNow;
                        Log("ABCD Signal Queue timer failed: " + ex.Message);
                    }
                }
            }, null, 250, 1000);
            Log("ABCD Signal Queue timer polling enabled every 1s.");
        }

        private void StopQueueTimer()
        {
            Timer timer = queueTimer;
            queueTimer = null;
            if (timer != null)
                timer.Dispose();
        }

        private void ProcessQueue()
        {
            if (CurrentBar >= 0 && Close[0] > 0 && !double.IsNaN(Close[0]))
                lastPrice = Close[0];

            PollIfNeeded();
            TrySubmitSignals();
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

            string signalUid = ExtractTag(execution.Order.Name, "sig");
            if (!string.IsNullOrWhiteSpace(signalUid))
            {
                activeSignalUid = signalUid;
                activeEntryOrderTag = execution.Order.Name;
                activeEntryTime = time;
                timeExitSubmitted = false;
                PostStatus(signalUid, "triggered", orderId, price, "execution received");

                if (quickExitSignals.Contains(signalUid))
                {
                    if (marketPosition == MarketPosition.Long)
                        ExitLong("ABCD_QEXIT", execution.Order.Name);
                    else if (marketPosition == MarketPosition.Short)
                        ExitShort("ABCD_QEXIT", execution.Order.Name);
                }
            }
            else if (!string.IsNullOrWhiteSpace(activeSignalUid))
            {
                PostStatus(activeSignalUid, "completed", orderId, null, "exit execution received");
                quickExitSignals.Remove(activeSignalUid);
                activeSignalUid = null;
                activeEntryOrderTag = null;
                activeEntryTime = DateTime.MinValue;
                timeExitSubmitted = false;
            }
        }

        private void ManageTimeExit()
        {
            if (State != State.Realtime)
                return;

            if (MaxHoldBars <= 0 || timeExitSubmitted)
                return;

            if (Position.MarketPosition == MarketPosition.Flat)
                return;

            SignalRow recoveredSignal = null;
            if (string.IsNullOrWhiteSpace(activeSignalUid))
            {
                recoveredSignal = RecoverOpenSignalForPosition();
                if (recoveredSignal == null)
                    return;

                activeSignalUid = recoveredSignal.signal_uid;
                activeEntryOrderTag = BuildOrderTag(recoveredSignal);
                activeEntryTime = BestSignalTime(recoveredSignal);
                timeExitSubmitted = false;
                Log("ABCD Signal Queue recovered open signal " + activeSignalUid + " for time-exit management.");
            }

            int barsSinceEntry = -1;
            if (!string.IsNullOrWhiteSpace(activeEntryOrderTag))
                barsSinceEntry = BarsSinceEntryExecution(0, activeEntryOrderTag, 0);

            if (barsSinceEntry < 0 && activeEntryTime != DateTime.MinValue)
            {
                double minutes = Math.Max(0, (Time[0] - activeEntryTime).TotalMinutes);
                double barMinutes = BarsPeriod != null && BarsPeriod.BarsPeriodType == BarsPeriodType.Minute
                    ? Math.Max(1, BarsPeriod.Value)
                    : 2;
                barsSinceEntry = (int)Math.Floor(minutes / barMinutes);
            }

            if (barsSinceEntry < MaxHoldBars)
                return;

            timeExitSubmitted = true;
            bool hasTrackedEntry = !string.IsNullOrWhiteSpace(activeEntryOrderTag)
                && BarsSinceEntryExecution(0, activeEntryOrderTag, 0) >= 0;

            if (Position.MarketPosition == MarketPosition.Long)
            {
                if (hasTrackedEntry)
                    ExitLong("ABCD_TIME_EXIT", activeEntryOrderTag);
                else
                    ExitLong("ABCD_TIME_EXIT");
            }
            else if (Position.MarketPosition == MarketPosition.Short)
            {
                if (hasTrackedEntry)
                    ExitShort("ABCD_TIME_EXIT", activeEntryOrderTag);
                else
                    ExitShort("ABCD_TIME_EXIT");
            }

            Log("ABCD Signal Queue time exit submitted after " + barsSinceEntry + " bars for " + activeSignalUid);
        }

        private SignalRow RecoverOpenSignalForPosition()
        {
            if ((DateTime.UtcNow - lastOpenSignalRecoveryUtc).TotalSeconds < 10)
                return null;
            lastOpenSignalRecoveryUtc = DateTime.UtcNow;

            try
            {
                string body = "{"
                    + JsonPair("account_name", AccountName)
                    + JsonPair("instrument", Instrument.FullName)
                    + "\"include_cancelled\":true,"
                    + "\"limit\":20"
                    + "}";
                string response = PostJson(BridgeBaseUrl + "/ninjatrader/signals/history", body);
                foreach (SignalRow signal in ParseSignals(response))
                {
                    if (signal == null)
                        continue;
                    if (!string.Equals(signal.status, "triggered", StringComparison.OrdinalIgnoreCase))
                        continue;
                    if (!string.Equals(signal.instrument, Instrument.FullName, StringComparison.OrdinalIgnoreCase))
                        continue;
                    if (Position.MarketPosition == MarketPosition.Long
                        && string.Equals(signal.side, "LONG", StringComparison.OrdinalIgnoreCase))
                        return signal;
                    if (Position.MarketPosition == MarketPosition.Short
                        && string.Equals(signal.side, "SHORT", StringComparison.OrdinalIgnoreCase))
                        return signal;
                }
            }
            catch (Exception ex)
            {
                Log("ABCD Signal Queue open signal recovery failed: " + ex.Message);
            }

            return null;
        }

        private static DateTime BestSignalTime(SignalRow signal)
        {
            if (signal == null)
                return DateTime.MinValue;

            DateTime parsed;
            if (TryParseBridgeDateTime(signal.entry_execution_time, out parsed))
                return parsed;
            if (TryParseBridgeDateTime(signal.triggered_at, out parsed))
                return parsed;
            if (TryParseBridgeDateTime(signal.expected_time, out parsed))
                return parsed;
            return DateTime.MinValue;
        }

        private void PollIfNeeded()
        {
            if (polling)
                return;

            if ((DateTime.UtcNow - lastPollUtc).TotalMilliseconds < 750)
                return;

            if ((DateTime.UtcNow - lastHeartbeatUtc).TotalSeconds >= 15)
            {
                lastHeartbeatUtc = DateTime.UtcNow;
                Log("ABCD Signal Queue polling " + Instrument.FullName + " last=" + lastPrice.ToString(CultureInfo.InvariantCulture));
            }

            polling = true;
            lastPollUtc = DateTime.UtcNow;
            ThreadPool.QueueUserWorkItem(_ =>
            {
                try
                {
                    List<SignalRow> nextSignals = FetchSignals();
                    lock (signalLock)
                    {
                        signals = nextSignals;
                    }
                    if (nextSignals.Count > 0)
                        Log("ABCD Signal Queue loaded " + nextSignals.Count + " pending signal(s).");
                }
                catch (Exception ex)
                {
                    Log("ABCD Signal Queue poll failed: " + ex.Message);
                }
                finally
                {
                    polling = false;
                }
            });
        }

        private List<SignalRow> FetchSignals()
        {
            string body = "{"
                + JsonPair("account_name", AccountName)
                + JsonPair("instrument", Instrument.FullName)
                + JsonPair("client_id", ClientId)
                + "\"limit\":10"
                + "}";

            string response = PostJson(BridgeBaseUrl + "/ninjatrader/signals/pending", body);
            return ParseSignals(response);
        }

        private void TrySubmitSignals()
        {
            List<SignalRow> currentSignals;
            lock (signalLock)
            {
                currentSignals = new List<SignalRow>(signals);
            }

            foreach (SignalRow signal in currentSignals)
            {
                if (signal == null || string.IsNullOrWhiteSpace(signal.signal_uid))
                    continue;

                if (Position.MarketPosition != MarketPosition.Flat)
                    continue;

                if (submittedSignals.Contains(signal.signal_uid))
                    continue;

                if (!string.Equals(signal.instrument, Instrument.FullName, StringComparison.OrdinalIgnoreCase))
                    continue;

                if (!ShouldTrigger(signal))
                    continue;

                string orderTag = BuildOrderTag(signal);
                submittedSignals.Add(signal.signal_uid);
                activeSignalUid = signal.signal_uid;
                activeEntryOrderTag = orderTag;
                timeExitSubmitted = false;
                if (IsQuickExit(signal))
                    quickExitSignals.Add(signal.signal_uid);
                PostStatus(signal.signal_uid, "claimed", null, lastPrice, "price touched");

                if (signal.stop_price > 0)
                    SetStopLoss(orderTag, CalculationMode.Price, signal.stop_price, false);

                if (signal.target_price > 0)
                    SetProfitTarget(orderTag, CalculationMode.Price, signal.target_price);

                if (string.Equals(signal.side, "SHORT", StringComparison.OrdinalIgnoreCase))
                    EnterShort(Math.Max(1, signal.quantity), orderTag);
                else
                    EnterLong(Math.Max(1, signal.quantity), orderTag);
            }
        }

        private bool ShouldTrigger(SignalRow signal)
        {
            if (lastPrice <= 0 || signal.expected_price <= 0)
                return false;

            if (IsMarketNow(signal))
                return true;

            if (string.Equals(signal.side, "SHORT", StringComparison.OrdinalIgnoreCase))
                return lastPrice >= signal.expected_price;

            return lastPrice <= signal.expected_price;
        }

        private static bool IsMarketNow(SignalRow signal)
        {
            if (signal == null || string.IsNullOrWhiteSpace(signal.notes))
                return false;
            string text = signal.notes.ToLowerInvariant();
            return text.Contains("market_now") || text.Contains("market-now");
        }

        private static bool IsQuickExit(SignalRow signal)
        {
            if (signal == null || string.IsNullOrWhiteSpace(signal.notes))
                return false;
            string text = signal.notes.ToLowerInvariant();
            return text.Contains("quick_exit") || text.Contains("quick-exit") || text.Contains("round_trip_test");
        }

        private string BuildOrderTag(SignalRow signal)
        {
            string side = string.IsNullOrWhiteSpace(signal.side) ? "LONG" : signal.side.ToUpperInvariant();
            string action = side.StartsWith("S", StringComparison.OrdinalIgnoreCase) ? "S" : "L";
            string signalUid = CleanTag(signal.signal_uid);
            if (signalUid.Length > 18)
                signalUid = signalUid.Substring(0, 18);
            return "ABCD"
                + "|sig=" + signalUid
                + "|a=" + CleanTag(action)
                + "|p=" + signal.expected_price.ToString(CultureInfo.InvariantCulture);
        }

        private void PostStatus(string signalUid, string status, string orderId, double? triggerPrice, string message)
        {
            lastStatusPostUtc = DateTime.UtcNow;
            string body = "{"
                + JsonPair("signal_uid", signalUid)
                + JsonPair("status", status)
                + JsonPair("client_id", ClientId)
                + JsonPair("order_id", orderId)
                + (triggerPrice.HasValue ? "\"actual_trigger_price\":" + triggerPrice.Value.ToString(CultureInfo.InvariantCulture) + "," : "")
                + JsonPair("status_message", message).TrimEnd(',')
                + "}";

            ThreadPool.QueueUserWorkItem(_ =>
            {
                try
                {
                    PostJson(BridgeBaseUrl + "/ninjatrader/signals/status", body);
                }
                catch (Exception ex)
                {
                    Log("ABCD Signal Queue status failed: " + ex.Message);
                }
            });
        }

        private static List<SignalRow> ParseSignals(string json)
        {
            List<SignalRow> rows = new List<SignalRow>();
            if (string.IsNullOrWhiteSpace(json))
                return rows;

            int index = 0;
            while (true)
            {
                int signalIndex = json.IndexOf("\"signal_uid\"", index, StringComparison.OrdinalIgnoreCase);
                if (signalIndex < 0)
                    break;

                int objectStart = json.LastIndexOf('{', signalIndex);
                int objectEnd = json.IndexOf('}', signalIndex);
                if (objectStart < 0 || objectEnd < 0 || objectEnd <= objectStart)
                    break;

                string item = json.Substring(objectStart, objectEnd - objectStart + 1);
                rows.Add(new SignalRow
                {
                    signal_uid = JsonString(item, "signal_uid"),
                    status = JsonString(item, "status"),
                    account_name = JsonString(item, "account_name"),
                    instrument = JsonString(item, "instrument"),
                    side = JsonString(item, "side"),
                    quantity = Math.Max(1, JsonInt(item, "quantity")),
                    expected_price = JsonDouble(item, "expected_price"),
                    expected_time = JsonString(item, "expected_time"),
                    stop_price = JsonDouble(item, "stop_price"),
                    target_price = JsonDouble(item, "target_price"),
                    tick_size = JsonDouble(item, "tick_size"),
                    expected_ai_run_id = JsonString(item, "expected_ai_run_id"),
                    expected_setup_id = JsonString(item, "expected_setup_id"),
                    expected_template_uid = JsonString(item, "expected_template_uid"),
                    entry_execution_time = JsonString(item, "entry_execution_time"),
                    triggered_at = JsonString(item, "triggered_at"),
                    notes = JsonString(item, "notes")
                });

                index = objectEnd + 1;
            }

            return rows;
        }

        private static string PostJson(string url, string body)
        {
            byte[] payload = Encoding.UTF8.GetBytes(body);
            HttpWebRequest request = (HttpWebRequest)WebRequest.Create(url);
            request.Method = "POST";
            request.ContentType = "application/json";
            request.ContentLength = payload.Length;
            request.Timeout = 3000;

            using (Stream requestStream = request.GetRequestStream())
                requestStream.Write(payload, 0, payload.Length);

            using (HttpWebResponse response = (HttpWebResponse)request.GetResponse())
            using (Stream responseStream = response.GetResponseStream())
            using (StreamReader reader = new StreamReader(responseStream))
                return reader.ReadToEnd();
        }

        private static string JsonPair(string key, string value)
        {
            return "\"" + Escape(key) + "\":" + (value == null ? "null" : "\"" + Escape(value) + "\"") + ",";
        }

        private static string JsonString(string json, string key)
        {
            string marker = "\"" + key + "\":";
            int index = json.IndexOf(marker, StringComparison.OrdinalIgnoreCase);
            if (index < 0)
                return null;

            index += marker.Length;
            while (index < json.Length && char.IsWhiteSpace(json[index]))
                index++;

            if (index >= json.Length || json[index] != '"')
                return null;

            index++;
            StringBuilder value = new StringBuilder();
            while (index < json.Length)
            {
                char ch = json[index++];
                if (ch == '"')
                    break;
                if (ch == '\\' && index < json.Length)
                    ch = json[index++];
                value.Append(ch);
            }
            return value.ToString();
        }

        private static double JsonDouble(string json, string key)
        {
            string value = JsonNumberToken(json, key);
            return double.TryParse(value, NumberStyles.Any, CultureInfo.InvariantCulture, out double parsed) ? parsed : 0;
        }

        private static int JsonInt(string json, string key)
        {
            string value = JsonNumberToken(json, key);
            return int.TryParse(value, NumberStyles.Any, CultureInfo.InvariantCulture, out int parsed) ? parsed : 0;
        }

        private static bool TryParseBridgeDateTime(string value, out DateTime parsed)
        {
            parsed = DateTime.MinValue;
            if (string.IsNullOrWhiteSpace(value))
                return false;

            value = value.Trim().TrimEnd('Z');
            string[] formats = new[]
            {
                "yyyy-MM-dd'T'HH:mm:ss.ffffff",
                "yyyy-MM-dd'T'HH:mm:ss",
                "yyyy-MM-dd HH:mm:ss.ffffff",
                "yyyy-MM-dd HH:mm:ss"
            };
            return DateTime.TryParseExact(
                value,
                formats,
                CultureInfo.InvariantCulture,
                DateTimeStyles.AssumeLocal,
                out parsed
            ) || DateTime.TryParse(value, CultureInfo.InvariantCulture, DateTimeStyles.AssumeLocal, out parsed);
        }

        private static string JsonNumberToken(string json, string key)
        {
            string marker = "\"" + key + "\":";
            int index = json.IndexOf(marker, StringComparison.OrdinalIgnoreCase);
            if (index < 0)
                return null;

            index += marker.Length;
            while (index < json.Length && char.IsWhiteSpace(json[index]))
                index++;

            int start = index;
            while (index < json.Length && "-+.0123456789".IndexOf(json[index]) >= 0)
                index++;

            return json.Substring(start, index - start);
        }

        private static string ExtractTag(string text, string key)
        {
            if (string.IsNullOrWhiteSpace(text))
                return null;

            foreach (string token in text.Split('|'))
            {
                int index = token.IndexOf('=');
                if (index <= 0)
                    continue;

                if (string.Equals(token.Substring(0, index).Trim(), key, StringComparison.OrdinalIgnoreCase))
                    return token.Substring(index + 1).Trim();
            }

            return null;
        }

        private static string CleanTag(string value)
        {
            if (string.IsNullOrWhiteSpace(value))
                return "";
            return value.Replace("|", "_").Replace("=", "_").Trim();
        }

        private static string Escape(string value)
        {
            if (value == null)
                return "";

            return value
                .Replace("\\", "\\\\")
                .Replace("\"", "\\\"")
                .Replace("\r", "\\r")
                .Replace("\n", "\\n")
                .Replace("\t", "\\t");
        }

        private static void Log(string message)
        {
            NinjaTrader.Code.Output.Process(message, PrintTo.OutputTab1);
        }

        private class SignalRow
        {
            public string signal_uid;
            public string status;
            public string account_name;
            public string instrument;
            public string side;
            public int quantity;
            public double expected_price;
            public string expected_time;
            public double stop_price;
            public double target_price;
            public double tick_size;
            public string expected_ai_run_id;
            public string expected_setup_id;
            public string expected_template_uid;
            public string entry_execution_time;
            public string triggered_at;
            public string notes;
        }
    }
}
