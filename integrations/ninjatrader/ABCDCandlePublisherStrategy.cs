// NinjaTrader 8 Strategy source.
// Copy this file into Documents\NinjaTrader 8\bin\Custom\Strategies, then
// compile from the NinjaScript Editor. Data-only: this strategy never submits orders.

#region Using declarations
using System;
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
        private DateTime lastClosedBarTime = DateTime.MinValue;
        private DateTime lastSnapshotPublishUtc = DateTime.MinValue;

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

                PublishHistoricalBars = false;
                LogEveryBars = 10;
                PublishLiveSnapshots = true;
                LiveSnapshotMs = 1000;
            }
            else if (State == State.Realtime)
            {
                Log("ABCD Candle Publisher live on " + Instrument.FullName
                    + " bars=" + BarsPeriod.Value.ToString(CultureInfo.InvariantCulture)
                    + " " + BarsPeriod.BarsPeriodType
                    + " closed_endpoint=" + BridgeBaseUrl + "/ninjatrader/candles"
                    + " snapshot_endpoint=" + BridgeBaseUrl + "/ninjatrader/live-bar-snapshot");
            }
            else if (State == State.Terminated)
            {
                Log("ABCD Candle Publisher terminated on "
                    + (Instrument != null ? Instrument.FullName : "unknown instrument")
                    + " after " + publishedBars.ToString(CultureInfo.InvariantCulture)
                    + " published closed bar(s) and "
                    + publishedSnapshots.ToString(CultureInfo.InvariantCulture)
                    + " live snapshot(s).");
            }
        }

        protected override void OnBarUpdate()
        {
            if (CurrentBar < BarsRequiredToTrade)
                return;

            if (!PublishHistoricalBars && State != State.Realtime)
                return;

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
                    Log("ABCD Candle Publisher closed bar post failed: " + ex.Message);
                }
            });
        }

        private void PublishLiveSnapshot()
        {
            if (!PublishLiveSnapshots || !ShouldPublishLiveSnapshot())
                return;

            string json = BuildBarJson(0, false, Close[0]);
            DateTime barTime = Time[0];
            double lastPrice = Close[0];

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
            DateTime localValue = value.Kind == DateTimeKind.Unspecified
                ? DateTime.SpecifyKind(value, DateTimeKind.Local)
                : value;

            return localValue.ToUniversalTime().ToString("yyyy-MM-dd HH:mm:ss.fff", CultureInfo.InvariantCulture);
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
