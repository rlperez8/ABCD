// NinjaTrader 8 AddOn source.
// Copy this file into Documents\NinjaTrader 8\bin\Custom\AddOns, then compile
// from the NinjaScript Editor. Edit AccountName before using.

#region Using declarations
using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Net;
using System.Text;
using System.Threading;
using NinjaTrader.Cbi;
using NinjaTrader.NinjaScript;
#endregion

namespace NinjaTrader.NinjaScript.AddOns
{
    public class ABCDSlippageBridgeAddOn : AddOnBase
    {
        private const string BridgeUrl = "http://127.0.0.1:8080/ninjatrader/executions";
        private const string BridgeVersion = "0.1.0";

        // Exact NinjaTrader account display name to monitor.
        private const string AccountName = "DEMO5859105";

        private Account account;

        protected override void OnStateChange()
        {
            if (State == State.SetDefaults)
            {
                Name = "ABCDSlippageBridgeAddOn";
            }
            else if (State == State.Active)
            {
                Subscribe();
            }
            else if (State == State.Terminated)
            {
                Unsubscribe();
            }
        }

        private void Subscribe()
        {
            Unsubscribe();

            account = Account.All.FirstOrDefault(item => item.Name == AccountName);
            if (account == null)
            {
                Log("ABCD Slippage Bridge: account not found: " + AccountName);
                return;
            }

            account.ExecutionUpdate += OnExecutionUpdate;
            Log("ABCD Slippage Bridge: subscribed to " + account.Name);
        }

        private void Unsubscribe()
        {
            if (account != null)
            {
                account.ExecutionUpdate -= OnExecutionUpdate;
                Log("ABCD Slippage Bridge: unsubscribed from " + account.Name);
                account = null;
            }
        }

        private void OnExecutionUpdate(object sender, ExecutionEventArgs e)
        {
            try
            {
                if (e == null || e.Execution == null)
                    return;

                string json = BuildExecutionJson(e);
                string orderName = e.Execution.Order != null ? e.Execution.Order.Name : null;
                if (!string.IsNullOrWhiteSpace(orderName) && orderName.StartsWith("ABCD", StringComparison.OrdinalIgnoreCase))
                {
                    Log("ABCD Slippage Bridge execution captured "
                        + e.Execution.Instrument.FullName
                        + " price=" + e.Price.ToString(CultureInfo.InvariantCulture)
                        + " order=" + orderName);
                }
                ThreadPool.QueueUserWorkItem(_ => PostJson(json, orderName));
            }
            catch (Exception ex)
            {
                Log("ABCD Slippage Bridge execution error: " + ex.Message);
            }
        }

        private string BuildExecutionJson(ExecutionEventArgs e)
        {
            Execution execution = e.Execution;
            Order order = execution.Order;
            Instrument instrument = execution.Instrument;
            MasterInstrument master = instrument != null ? instrument.MasterInstrument : null;
            Dictionary<string, string> expected = ParseExpectedTags(order != null ? order.Name : null);

            StringBuilder json = new StringBuilder();
            json.Append("{");
            AppendString(json, "source", "ninjatrader");
            AppendString(json, "bridge_version", BridgeVersion);
            AppendString(json, "account_name", account != null ? account.Name : AccountName);
            AppendString(json, "connection_name", null);
            AppendString(json, "strategy_name", null);
            AppendString(json, "instrument", instrument != null ? instrument.FullName : null);
            AppendString(json, "root_symbol", master != null ? master.Name : null);
            AppendString(json, "execution_id", execution.ExecutionId);
            AppendString(json, "execution_time", execution.Time.ToString("yyyy-MM-dd HH:mm:ss.fff", CultureInfo.InvariantCulture));
            AppendString(json, "order_id", order != null ? order.OrderId : execution.OrderId);
            AppendString(json, "order_name", order != null ? order.Name : null);
            AppendString(json, "order_action", order != null ? order.OrderAction.ToString() : null);
            AppendString(json, "order_type", order != null ? order.OrderType.ToString() : null);
            AppendString(json, "order_state", order != null ? order.OrderState.ToString() : null);
            AppendString(json, "market_position", execution.MarketPosition.ToString());
            AppendNumber(json, "quantity", e.Quantity);
            AppendNumber(json, "price", e.Price);
            AppendNumber(json, "commission", execution.Commission);
            AppendNumber(json, "tick_size", master != null ? master.TickSize : 0);
            string expectedSide = ExpandSide(GetTag(expected, "side", "direction", "a"));
            AppendString(json, "expected_ai_run_id", ExpandAiRunId(GetTag(expected, "ai", "run", "ai_run", "r")));
            AppendString(json, "expected_setup_id", ExpandSetup(GetTag(expected, "setup", "setup_id", "u"), expectedSide));
            AppendString(json, "expected_template_uid", ExpandTemplate(GetTag(expected, "tpl", "template", "template_uid", "t")));
            AppendString(json, "expected_side", expectedSide);
            AppendNumberOrNull(json, "expected_price", GetTag(expected, "expected", "expected_price", "price", "p"));
            AppendString(json, "expected_time", GetTag(expected, "time", "expected_time"));

            if (json[json.Length - 1] == ',')
                json.Length -= 1;

            json.Append("}");
            return json.ToString();
        }

        private static Dictionary<string, string> ParseExpectedTags(string orderName)
        {
            Dictionary<string, string> tags = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            if (string.IsNullOrWhiteSpace(orderName))
                return tags;

            char[] separators = new[] { '|', ';' };
            foreach (string token in orderName.Split(separators, StringSplitOptions.RemoveEmptyEntries))
            {
                int index = token.IndexOf('=');
                if (index <= 0 || index >= token.Length - 1)
                    continue;

                string key = token.Substring(0, index).Trim();
                string value = token.Substring(index + 1).Trim();
                if (key.Length > 0 && value.Length > 0)
                    tags[key] = value;
            }

            return tags;
        }

        private static string GetTag(Dictionary<string, string> tags, params string[] keys)
        {
            foreach (string key in keys)
            {
                if (tags.TryGetValue(key, out string value))
                    return value;
            }
            return null;
        }

        private static string ExpandAiRunId(string value)
        {
            if (string.IsNullOrWhiteSpace(value))
                return null;

            string clean = value.Trim();
            if (string.Equals(clean, "SS", StringComparison.OrdinalIgnoreCase))
                return "slippage-sampler";

            return clean;
        }

        private static string ExpandSetup(string value, string side)
        {
            if (string.IsNullOrWhiteSpace(value))
                return null;

            string clean = value.Trim();
            if (string.Equals(clean, "U", StringComparison.OrdinalIgnoreCase))
                return "UP_TRIGGER_BUY";
            if (string.Equals(clean, "D", StringComparison.OrdinalIgnoreCase))
                return "DOWN_TRIGGER_SELL";
            if (string.Equals(clean, "T", StringComparison.OrdinalIgnoreCase))
                return string.Equals(side, "SHORT", StringComparison.OrdinalIgnoreCase) ? "DOWN_TRIGGER_SELL" : "UP_TRIGGER_BUY";

            return clean;
        }

        private static string ExpandTemplate(string value)
        {
            if (string.IsNullOrWhiteSpace(value))
                return null;

            string clean = value.Trim();
            if (string.Equals(clean, "TM", StringComparison.OrdinalIgnoreCase))
                return "trigger-distance-market";

            return clean;
        }

        private static string ExpandSide(string value)
        {
            if (string.IsNullOrWhiteSpace(value))
                return null;

            string clean = value.Trim();
            if (string.Equals(clean, "L", StringComparison.OrdinalIgnoreCase)
                || string.Equals(clean, "B", StringComparison.OrdinalIgnoreCase)
                || string.Equals(clean, "BUY", StringComparison.OrdinalIgnoreCase))
                return "LONG";

            if (string.Equals(clean, "S", StringComparison.OrdinalIgnoreCase)
                || string.Equals(clean, "SELL", StringComparison.OrdinalIgnoreCase)
                || string.Equals(clean, "SHORT", StringComparison.OrdinalIgnoreCase))
                return "SHORT";

            return clean;
        }

        private static void PostJson(string json, string orderName)
        {
            try
            {
                byte[] body = Encoding.UTF8.GetBytes(json);
                HttpWebRequest request = (HttpWebRequest)WebRequest.Create(BridgeUrl);
                request.Method = "POST";
                request.ContentType = "application/json";
                request.ContentLength = body.Length;
                request.Timeout = 3000;

                using (Stream requestStream = request.GetRequestStream())
                    requestStream.Write(body, 0, body.Length);

                using (HttpWebResponse response = (HttpWebResponse)request.GetResponse())
                {
                    // Opening the response is enough; the server stores the fill.
                }

                if (!string.IsNullOrWhiteSpace(orderName) && orderName.StartsWith("ABCD", StringComparison.OrdinalIgnoreCase))
                    Log("ABCD Slippage Bridge posted execution order=" + orderName);
            }
            catch (Exception ex)
            {
                // Do not let a local bridge outage block NinjaTrader execution handling.
                if (!string.IsNullOrWhiteSpace(orderName) && orderName.StartsWith("ABCD", StringComparison.OrdinalIgnoreCase))
                    Log("ABCD Slippage Bridge post failed: " + ex.Message);
            }
        }

        private static void Log(string message)
        {
            NinjaTrader.Code.Output.Process(message, PrintTo.OutputTab1);
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

        private static void AppendNumber(StringBuilder json, string key, long value)
        {
            json.Append('"').Append(Escape(key)).Append("\":");
            json.Append(value.ToString(CultureInfo.InvariantCulture)).Append(',');
        }

        private static void AppendNumberOrNull(StringBuilder json, string key, string value)
        {
            json.Append('"').Append(Escape(key)).Append("\":");
            if (double.TryParse(value, NumberStyles.Any, CultureInfo.InvariantCulture, out double parsed))
                json.Append(parsed.ToString(CultureInfo.InvariantCulture)).Append(',');
            else
                json.Append("null,");
        }

        private static string Escape(string value)
        {
            if (value == null)
                return string.Empty;

            return value
                .Replace("\\", "\\\\")
                .Replace("\"", "\\\"")
                .Replace("\r", "\\r")
                .Replace("\n", "\\n")
                .Replace("\t", "\\t");
        }
    }
}
