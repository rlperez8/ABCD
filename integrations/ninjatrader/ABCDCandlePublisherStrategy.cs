// NinjaTrader 8 Strategy source.
// Copy this file into Documents\NinjaTrader 8\bin\Custom\Strategies, then
// compile from the NinjaScript Editor. Data-only: this strategy never submits orders.

#region Using declarations
using System;
using System.Collections.Generic;
using System.ComponentModel.DataAnnotations;
using System.Globalization;
using System.IO;
using System.Net;
using System.Text;
using System.Threading;
using NinjaTrader.Data;
using NinjaTrader.NinjaScript;
#endregion

namespace NinjaTrader.NinjaScript.Strategies
{
    public class ABCDCandlePublisherStrategy : Strategy
    {
        private const string BridgeVersion = "0.2.0";
        private const string BridgeBaseUrl = "http://127.0.0.1:8080";
        private const string ClientId = "nt8-abcd-candle-publisher";

        private int publishedBars;
        private int publishedSnapshots;
        private int publishedHeartbeats;
        private DateTime lastClosedBarTime = DateTime.MinValue;
        private DateTime lastSnapshotPublishUtc = DateTime.MinValue;
        private DateTime lastKnownBarTime = DateTime.MinValue;
        private DateTime lastKnownHeartbeatBarTime = DateTime.MinValue;
        private double lastKnownPrice = double.NaN;
        private string activeInstrumentName;
        private string activeRootSymbol;
        private string activeTimeframe;
        private string activeBarsPeriodType;
        private int activeBarsPeriodValue;
        private double activeTickSize;
        private double activePointValue;
        private System.Threading.Timer heartbeatTimer;
        private readonly object heartbeatLock = new object();
        private readonly object pendingClosedBarsLock = new object();
        private readonly object pendingTimerClosedBarLock = new object();
        private readonly List<string> pendingClosedBarPayloads = new List<string>();
        private string pendingTimerClosedBarPayload;
        private DateTime pendingTimerClosedBarTime = DateTime.MinValue;
        private DateTime pendingTimerClosedBarUtc = DateTime.MinValue;
        private double pendingTimerClosedBarClose = double.NaN;
        private bool pendingTimerClosedBarPublished;
        private int isFlushingPendingBars;

        [NinjaScriptProperty]
        [Display(Name = "Publish historical bars", Order = 1, GroupName = "Publisher")]
        public bool PublishHistoricalBars { get; set; }

        [NinjaScriptProperty]
        [Range(1, 500)]
        [Display(Name = "Log every bars", Order = 2, GroupName = "Publisher")]
        public int LogEveryBars { get; set; }

        [NinjaScriptProperty]
        [Display(Name = "Publish live snapshots", Order = 3, GroupName = "Publisher")]
        public bool PublishLiveSnapshots { get; set; }

        [NinjaScriptProperty]
        [Range(250, 10000)]
        [Display(Name = "Snapshot milliseconds", Order = 4, GroupName = "Publisher")]
        public int LiveSnapshotMs { get; set; }

        [NinjaScriptProperty]
        [Display(Name = "Publish heartbeat", Order = 5, GroupName = "Publisher")]
        public bool PublishHeartbeat { get; set; }

        [NinjaScriptProperty]
        [Range(2, 120)]
        [Display(Name = "Heartbeat seconds", Order = 6, GroupName = "Publisher")]
        public int HeartbeatSeconds { get; set; }

        protected override void OnStateChange()
        {
            if (State == State.SetDefaults)
            {
                Name = "ABCDCandlePublisherStrategy";
                Description = "Posts closed NinjaTrader candles to the local ABCD server for model/trend scanning. Data-only; no orders.";
                Calculate = Calculate.OnEachTick;
                IsExitOnSessionCloseStrategy = false;
                IncludeCommission = false;
                BarsRequiredToTrade = 2;

                PublishHistoricalBars = true;
                LogEveryBars = 10;
                PublishLiveSnapshots = true;
                LiveSnapshotMs = 1000;
                PublishHeartbeat = true;
                HeartbeatSeconds = 5;
            }
            else if (State == State.Realtime)
            {
                RefreshInstrumentMetadata();
                StartHeartbeatTimer();
                Log("ABCD Candle Publisher live on " + Instrument.FullName
                    + " bars=" + BarsPeriod.Value.ToString(CultureInfo.InvariantCulture)
                    + " " + BarsPeriod.BarsPeriodType
                    + " closed_endpoint=" + BridgeBaseUrl + "/ninjatrader/candles"
                    + " snapshot_endpoint=" + BridgeBaseUrl + "/ninjatrader/live-bar-snapshot"
                    + " heartbeat_endpoint=" + BridgeBaseUrl + "/ninjatrader/heartbeat");
            }
            else if (State == State.Terminated)
            {
                StopHeartbeatTimer();
                Log("ABCD Candle Publisher terminated on "
                    + (Instrument != null ? Instrument.FullName : "unknown instrument")
                    + " after " + publishedBars.ToString(CultureInfo.InvariantCulture)
                    + " published closed bar(s) and "
                    + publishedSnapshots.ToString(CultureInfo.InvariantCulture)
                    + " live snapshot(s), "
                    + publishedHeartbeats.ToString(CultureInfo.InvariantCulture)
                    + " heartbeat(s).");
            }
        }

        protected override void OnBarUpdate()
        {
            if (CurrentBar < BarsRequiredToTrade)
                return;

            if (!PublishHistoricalBars && State != State.Realtime)
                return;

            UpdateHeartbeatState(0);

            if (State != State.Realtime)
            {
                PublishClosedBar(0);
                return;
            }

            PublishLiveSnapshot();

            if (IsFirstTickOfBar && CurrentBar > BarsRequiredToTrade)
                PublishClosedBar(1);
        }

        private void PublishClosedBar(int barsAgo)
        {
            if (barsAgo < 0 || barsAgo > CurrentBar)
                return;

            DateTime barTime = Time[barsAgo];
            if (barTime == lastClosedBarTime)
                return;

            lastClosedBarTime = barTime;
            double closeValue = Close[barsAgo];
            string json = BuildBarJson(barsAgo, true, closeValue);
            ThreadPool.QueueUserWorkItem(_ =>
            {
                try
                {
                    PostJson(BridgeBaseUrl + "/ninjatrader/candles", json);
                    publishedBars++;
                    if (publishedBars == 1 || publishedBars % Math.Max(1, LogEveryBars) == 0)
                    {
                        Log("ABCD Candle Publisher posted "
                            + publishedBars.ToString(CultureInfo.InvariantCulture)
                            + " closed bar(s), last=" + barTime.ToString("yyyy-MM-dd HH:mm:ss", CultureInfo.InvariantCulture)
                            + " close=" + closeValue.ToString(CultureInfo.InvariantCulture));
                    }
                }
                catch (Exception ex)
                {
                    EnqueuePendingClosedBar(json);
                    Log("ABCD Candle Publisher closed bar post failed: " + ex.Message);
                }
            });
        }

        private void PublishLiveSnapshot()
        {
            if (!PublishLiveSnapshots || !ShouldPublishLiveSnapshot())
                return;

            string json = BuildBarJson(0, false, Close[0]);
            string closedJson = BuildBarJson(0, true, Close[0]);
            DateTime barTime = Time[0];
            double lastPrice = Close[0];
            StoreTimerClosedBarCandidate(closedJson, barTime, lastPrice);

            ThreadPool.QueueUserWorkItem(_ =>
            {
                try
                {
                    PostJson(BridgeBaseUrl + "/ninjatrader/live-bar-snapshot", json);
                    publishedSnapshots++;
                    if (publishedSnapshots == 1 || publishedSnapshots % Math.Max(1, LogEveryBars) == 0)
                    {
                        Log("ABCD Candle Publisher posted "
                            + publishedSnapshots.ToString(CultureInfo.InvariantCulture)
                            + " live snapshot(s), bar=" + barTime.ToString("yyyy-MM-dd HH:mm:ss", CultureInfo.InvariantCulture)
                            + " last=" + lastPrice.ToString(CultureInfo.InvariantCulture));
                    }
                }
                catch (Exception ex)
                {
                    Log("ABCD Candle Publisher live snapshot post failed: " + ex.Message);
                }
            });
        }

        private bool ShouldPublishLiveSnapshot()
        {
            DateTime nowUtc = DateTime.UtcNow;
            if ((nowUtc - lastSnapshotPublishUtc).TotalMilliseconds < Math.Max(250, LiveSnapshotMs))
                return false;

            lastSnapshotPublishUtc = nowUtc;
            return true;
        }

        private void RefreshInstrumentMetadata()
        {
            activeInstrumentName = Instrument != null ? Instrument.FullName : null;
            activeRootSymbol = Instrument != null && Instrument.MasterInstrument != null
                ? Instrument.MasterInstrument.Name
                : null;
            activeTimeframe = BarsPeriod != null
                ? BarsPeriod.Value.ToString(CultureInfo.InvariantCulture) + "m"
                : null;
            activeBarsPeriodType = BarsPeriod != null ? BarsPeriod.BarsPeriodType.ToString() : null;
            activeBarsPeriodValue = BarsPeriod != null ? BarsPeriod.Value : 0;
            activeTickSize = TickSize > 0 ? TickSize : 0;
            activePointValue = Instrument != null && Instrument.MasterInstrument != null
                ? Instrument.MasterInstrument.PointValue
                : 0;
        }

        private void UpdateHeartbeatState(int barsAgo)
        {
            if (barsAgo < 0 || barsAgo > CurrentBar)
                return;

            lock (heartbeatLock)
            {
                lastKnownPrice = Close[barsAgo];
                lastKnownBarTime = Time[barsAgo];
                lastKnownHeartbeatBarTime = Time[0];
            }
        }

        private void StartHeartbeatTimer()
        {
            if (!PublishHeartbeat || heartbeatTimer != null)
                return;

            int intervalMs = Math.Max(2, HeartbeatSeconds) * 1000;
            heartbeatTimer = new System.Threading.Timer(_ => PublishHeartbeatNow(), null, 1000, intervalMs);
        }

        private void StopHeartbeatTimer()
        {
            System.Threading.Timer timer = heartbeatTimer;
            heartbeatTimer = null;
            if (timer != null)
                timer.Dispose();
        }

        private void PublishHeartbeatNow()
        {
            if (!PublishHeartbeat)
                return;

            ThreadPool.QueueUserWorkItem(_ =>
            {
                try
                {
                    FlushPendingClosedBars();
                    PublishTimerClosedBarIfDue();
                    string json = BuildHeartbeatJson();
                    PostJson(BridgeBaseUrl + "/ninjatrader/heartbeat", json);
                    publishedHeartbeats++;
                    if (publishedHeartbeats == 1 || publishedHeartbeats % Math.Max(1, LogEveryBars) == 0)
                    {
                        Log("ABCD Candle Publisher heartbeat "
                            + publishedHeartbeats.ToString(CultureInfo.InvariantCulture)
                            + " posted for " + (activeInstrumentName ?? "unknown instrument"));
                    }
                }
                catch (Exception ex)
                {
                    Log("ABCD Candle Publisher heartbeat post failed: " + ex.Message);
                }
            });
        }

        private void StoreTimerClosedBarCandidate(string closedJson, DateTime barTime, double closeValue)
        {
            if (string.IsNullOrWhiteSpace(closedJson) || barTime == DateTime.MinValue)
                return;

            DateTime barTimeUtc = ToUtcDateTime(barTime);
            lock (pendingTimerClosedBarLock)
            {
                if (barTime != pendingTimerClosedBarTime)
                {
                    pendingTimerClosedBarPublished = false;
                }

                pendingTimerClosedBarPayload = closedJson;
                pendingTimerClosedBarTime = barTime;
                pendingTimerClosedBarUtc = barTimeUtc;
                pendingTimerClosedBarClose = closeValue;
            }
        }

        private void PublishTimerClosedBarIfDue()
        {
            string json;
            DateTime barTime;
            double closeValue;

            lock (pendingTimerClosedBarLock)
            {
                if (pendingTimerClosedBarPublished
                    || string.IsNullOrWhiteSpace(pendingTimerClosedBarPayload)
                    || pendingTimerClosedBarUtc == DateTime.MinValue
                    || DateTime.UtcNow < pendingTimerClosedBarUtc.AddSeconds(2))
                {
                    return;
                }

                json = pendingTimerClosedBarPayload;
                barTime = pendingTimerClosedBarTime;
                closeValue = pendingTimerClosedBarClose;
                pendingTimerClosedBarPublished = true;
                lastClosedBarTime = barTime;
            }

            try
            {
                PostJson(BridgeBaseUrl + "/ninjatrader/candles", json);
                publishedBars++;
                Log("ABCD Candle Publisher timer-closed stale live bar, last="
                    + barTime.ToString("yyyy-MM-dd HH:mm:ss", CultureInfo.InvariantCulture)
                    + " close=" + closeValue.ToString(CultureInfo.InvariantCulture));
            }
            catch (Exception ex)
            {
                EnqueuePendingClosedBar(json);
                Log("ABCD Candle Publisher timer-closed bar post failed: " + ex.Message);
            }
        }

        private string BuildHeartbeatJson()
        {
            DateTime knownBarTime;
            DateTime knownHeartbeatBarTime;
            double knownPrice;

            lock (heartbeatLock)
            {
                knownBarTime = lastKnownBarTime;
                knownHeartbeatBarTime = lastKnownHeartbeatBarTime;
                knownPrice = lastKnownPrice;
            }

            StringBuilder json = new StringBuilder();
            json.Append("{");
            AppendString(json, "source", "ninjatrader");
            AppendString(json, "bridge_version", BridgeVersion);
            AppendString(json, "client_id", ClientId);
            AppendString(json, "instrument", activeInstrumentName);
            AppendString(json, "root_symbol", activeRootSymbol);
            AppendString(json, "timeframe", activeTimeframe);
            AppendString(json, "bars_period_type", activeBarsPeriodType);
            AppendNumber(json, "bars_period_value", activeBarsPeriodValue);
            AppendString(json, "heartbeat_time_utc", DateTime.UtcNow.ToString("yyyy-MM-dd HH:mm:ss.fff", CultureInfo.InvariantCulture));
            AppendDateTime(json, "last_candle_time", lastClosedBarTime);
            AppendUtcDateTime(json, "last_candle_time_utc", lastClosedBarTime);
            AppendUtcDateTime(json, "last_snapshot_time_utc", lastSnapshotPublishUtc);
            AppendDateTime(json, "last_known_bar_time", knownHeartbeatBarTime);
            AppendNullableNumber(json, "last_price", knownPrice);
            AppendNumber(json, "tick_size", activeTickSize);
            AppendNumber(json, "point_value", activePointValue);
            AppendBool(json, "is_realtime", State == State.Realtime);
            AppendString(json, "connection_status", State == State.Realtime ? "realtime" : State.ToString());

            if (json[json.Length - 1] == ',')
                json.Length -= 1;

            json.Append("}");
            return json.ToString();
        }

        private void EnqueuePendingClosedBar(string json)
        {
            if (string.IsNullOrWhiteSpace(json))
                return;

            lock (pendingClosedBarsLock)
            {
                pendingClosedBarPayloads.Add(json);
                while (pendingClosedBarPayloads.Count > 500)
                    pendingClosedBarPayloads.RemoveAt(0);
            }
        }

        private void FlushPendingClosedBars()
        {
            if (Interlocked.Exchange(ref isFlushingPendingBars, 1) == 1)
                return;

            try
            {
                while (true)
                {
                    string json = null;
                    lock (pendingClosedBarsLock)
                    {
                        if (pendingClosedBarPayloads.Count > 0)
                            json = pendingClosedBarPayloads[0];
                    }

                    if (json == null)
                        return;

                    try
                    {
                        PostJson(BridgeBaseUrl + "/ninjatrader/candles", json);
                        lock (pendingClosedBarsLock)
                        {
                            if (pendingClosedBarPayloads.Count > 0 && pendingClosedBarPayloads[0] == json)
                                pendingClosedBarPayloads.RemoveAt(0);
                        }
                    }
                    catch (Exception ex)
                    {
                        Log("ABCD Candle Publisher pending closed bar retry failed: " + ex.Message);
                        return;
                    }
                }
            }
            finally
            {
                Interlocked.Exchange(ref isFlushingPendingBars, 0);
            }
        }

        private string BuildBarJson(int barsAgo, bool isClosed, double lastPrice)
        {
            string root = Instrument != null && Instrument.MasterInstrument != null
                ? Instrument.MasterInstrument.Name
                : null;
            double pointValue = Instrument != null && Instrument.MasterInstrument != null
                ? Instrument.MasterInstrument.PointValue
                : 0;
            double tickSize = TickSize > 0 ? TickSize : 0;
            string timeframe = BarsPeriod.Value.ToString(CultureInfo.InvariantCulture) + "m";
            DateTime barTime = Time[barsAgo];

            StringBuilder json = new StringBuilder();
            json.Append("{");
            AppendString(json, "source", "ninjatrader");
            AppendString(json, "bridge_version", BridgeVersion);
            AppendString(json, "client_id", ClientId);
            AppendString(json, "instrument", Instrument != null ? Instrument.FullName : null);
            AppendString(json, "root_symbol", root);
            AppendString(json, "timeframe", timeframe);
            AppendString(json, "bars_period_type", BarsPeriod.BarsPeriodType.ToString());
            AppendNumber(json, "bars_period_value", BarsPeriod.Value);
            AppendString(json, "candle_time", barTime.ToString("yyyy-MM-dd HH:mm:ss.fff", CultureInfo.InvariantCulture));
            AppendString(json, "candle_time_utc", ToUtcText(barTime));
            AppendString(json, "bucket_time_utc", ToUtcText(GetBucketTime(barTime)));
            AppendString(json, "snapshot_time_utc", DateTime.UtcNow.ToString("yyyy-MM-dd HH:mm:ss.fff", CultureInfo.InvariantCulture));
            AppendNumber(json, "open", Open[barsAgo]);
            AppendNumber(json, "high", High[barsAgo]);
            AppendNumber(json, "low", Low[barsAgo]);
            AppendNumber(json, "close", Close[barsAgo]);
            AppendNumber(json, "last_price", lastPrice);
            AppendNumber(json, "volume", Volume[barsAgo]);
            AppendNumber(json, "tick_size", tickSize);
            AppendNumber(json, "point_value", pointValue);
            AppendBool(json, "is_realtime", State == State.Realtime);
            AppendBool(json, "is_closed", isClosed);

            if (json[json.Length - 1] == ',')
                json.Length -= 1;

            json.Append("}");
            return json.ToString();
        }

        private DateTime GetBucketTime(DateTime barTime)
        {
            if (BarsPeriod != null && BarsPeriod.BarsPeriodType == BarsPeriodType.Minute)
                return barTime.AddMinutes(-Math.Max(1, BarsPeriod.Value));

            return barTime;
        }

        private static string ToUtcText(DateTime value)
        {
            return ToUtcDateTime(value).ToString("yyyy-MM-dd HH:mm:ss.fff", CultureInfo.InvariantCulture);
        }

        private static DateTime ToUtcDateTime(DateTime value)
        {
            DateTime localValue = value.Kind == DateTimeKind.Unspecified
                ? DateTime.SpecifyKind(value, DateTimeKind.Local)
                : value;

            return localValue.ToUniversalTime();
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

        private static void AppendString(StringBuilder json, string key, string value)
        {
            json.Append('"').Append(Escape(key)).Append("\":");
            if (value == null)
                json.Append("null,");
            else
                json.Append('"').Append(Escape(value)).Append("\",");
        }

        private static void AppendNumber(StringBuilder json, string key, double value)
        {
            json.Append('"').Append(Escape(key)).Append("\":");
            json.Append(value.ToString(CultureInfo.InvariantCulture)).Append(',');
        }

        private static void AppendNullableNumber(StringBuilder json, string key, double value)
        {
            json.Append('"').Append(Escape(key)).Append("\":");
            if (double.IsNaN(value) || double.IsInfinity(value))
                json.Append("null,");
            else
                json.Append(value.ToString(CultureInfo.InvariantCulture)).Append(',');
        }

        private static void AppendDateTime(StringBuilder json, string key, DateTime value)
        {
            if (value == DateTime.MinValue)
                AppendString(json, key, null);
            else
                AppendString(json, key, value.ToString("yyyy-MM-dd HH:mm:ss.fff", CultureInfo.InvariantCulture));
        }

        private static void AppendUtcDateTime(StringBuilder json, string key, DateTime value)
        {
            if (value == DateTime.MinValue)
                AppendString(json, key, null);
            else
                AppendString(json, key, ToUtcText(value));
        }

        private static void AppendBool(StringBuilder json, string key, bool value)
        {
            json.Append('"').Append(Escape(key)).Append("\":");
            json.Append(value ? "true," : "false,");
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
    }
}
