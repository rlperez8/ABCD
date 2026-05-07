use std::env;

use sqlx::{mysql::MySqlPool, Row};

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

fn futures_candle_table(timeframe: &str) -> &'static str {
    match timeframe
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '_', '-'], "")
        .replace("minutes", "m")
        .replace("minute", "m")
        .replace("mins", "m")
        .replace("min", "m")
        .replace("hours", "h")
        .replace("hour", "h")
        .replace("hrs", "h")
        .replace("hr", "h")
        .as_str()
    {
        "3" | "3m" => "futures_contract_3m_candles",
        "5" | "5m" => "futures_contract_5m_candles",
        "15" | "15m" => "futures_contract_15m_candles",
        "30" | "30m" => "futures_contract_30m_candles",
        "60" | "60m" | "1h" => "futures_contract_1h_candles",
        "240" | "240m" | "4h" => "futures_contract_4h_candles",
        "720" | "720m" | "12h" => "futures_contract_12h_candles",
        "1440" | "1440m" | "24h" | "1d" | "d" | "day" | "daily" => "futures_contract_1d_candles",
        _ => "futures_contract_1m_candles",
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url = database_url_from_env()?;
    let root = env::var("ABCD_FUTURES_ROOT").unwrap_or_else(|_| "ES".to_string());
    let timeframe = env::var("ABCD_FUTURES_TIMEFRAME").unwrap_or_else(|_| "1m".to_string());
    let source_table = futures_candle_table(&timeframe);
    let pool = MySqlPool::connect(&database_url).await?;

    let sql = format!(
        r#"
        SELECT
            c.symbol,
            c.candle_count,
            CAST(c.first_ts AS CHAR) AS first_ts,
            CAST(c.last_ts AS CHAR) AS last_ts,
            COALESCE(s.setup_count, 0) AS setup_count,
            COALESCE(o.outcome_count, 0) AS outcome_count,
            COALESCE(o.closed_count, 0) AS closed_count
        FROM (
            SELECT
                symbol,
                COUNT(*) AS candle_count,
                MIN(ts_utc) AS first_ts,
                MAX(ts_utc) AS last_ts
            FROM {source_table}
            WHERE root_symbol = ?
            GROUP BY symbol
        ) c
        LEFT JOIN (
            SELECT contract_symbol, COUNT(*) AS setup_count
            FROM pattern_setups
            WHERE root_symbol = ? AND source_timeframe = ?
            GROUP BY contract_symbol
        ) s ON s.contract_symbol = c.symbol
        LEFT JOIN (
            SELECT
                contract_symbol,
                COUNT(*) AS outcome_count,
                SUM(CASE WHEN target_ready THEN 1 ELSE 0 END) AS closed_count
            FROM pattern_outcomes_prop
            WHERE root_symbol = ? AND source_timeframe = ?
            GROUP BY contract_symbol
        ) o ON o.contract_symbol = c.symbol
        ORDER BY c.symbol
        "#
    );

    let rows = sqlx::query(&sql)
        .bind(&root)
        .bind(&root)
        .bind(&timeframe)
        .bind(&root)
        .bind(&timeframe)
        .fetch_all(&pool)
        .await?;

    println!(
        "Engine resume check for root {root}, timeframe {timeframe}, source table {source_table}"
    );
    println!("offset\tsymbol\tcandles\tsetups\toutcomes\tclosed\tfirst\tlast");

    let mut first_missing_offset: Option<usize> = None;
    for (offset, row) in rows.iter().enumerate() {
        let symbol: String = row.try_get("symbol").unwrap_or_default();
        let candle_count: i64 = row.try_get("candle_count").unwrap_or(0);
        let setup_count: i64 = row.try_get("setup_count").unwrap_or(0);
        let outcome_count: i64 = row.try_get("outcome_count").unwrap_or(0);
        let closed_count: i64 = row.try_get("closed_count").unwrap_or(0);
        let first_ts: String = row.try_get("first_ts").unwrap_or_default();
        let last_ts: String = row.try_get("last_ts").unwrap_or_default();

        if first_missing_offset.is_none() && setup_count == 0 && outcome_count == 0 {
            first_missing_offset = Some(offset);
        }

        println!(
            "{offset}\t{symbol}\t{candle_count}\t{setup_count}\t{outcome_count}\t{closed_count}\t{first_ts}\t{last_ts}"
        );
    }

    match first_missing_offset {
        Some(offset) => println!("first missing engine output offset: {offset}"),
        None => println!("no missing engine output offsets found"),
    }

    Ok(())
}
