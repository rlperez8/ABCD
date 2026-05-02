use std::env;

use sqlx::mysql::MySqlPool;
use sqlx::Row;

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let root = env::var("ABCD_FUTURES_ROOT").ok();
    let limit = env::var("ABCD_SYMBOL_LIMIT")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(50)
        .clamp(1, 500);
    let pool = MySqlPool::connect(&database_url_from_env()?).await?;

    let mut sql = String::from(
        r#"
        SELECT
            root_symbol,
            symbol,
            COUNT(*) AS candle_count,
            MIN(ts_utc) AS first_ts,
            MAX(ts_utc) AS last_ts,
            SUM(volume) AS total_volume
        FROM futures_contract_1m_candles
        "#,
    );

    if root.is_some() {
        sql.push_str(" WHERE root_symbol = ?");
    }

    sql.push_str(
        r#"
        GROUP BY root_symbol, symbol
        ORDER BY root_symbol, symbol
        LIMIT ?
        "#,
    );

    let mut query = sqlx::query(&sql);
    if let Some(root) = root.as_deref() {
        query = query.bind(root);
    }
    query = query.bind(limit);

    let rows = query.fetch_all(&pool).await?;

    println!("root\tsymbol\tcandles\tfirst_ts\tlast_ts\tvolume");
    for row in rows {
        let root_symbol: Option<String> = row.try_get("root_symbol").ok();
        let symbol: String = row.try_get("symbol")?;
        let candle_count: i64 = row.try_get("candle_count")?;
        let first_ts: Option<chrono::NaiveDateTime> = row.try_get("first_ts").ok();
        let last_ts: Option<chrono::NaiveDateTime> = row.try_get("last_ts").ok();
        let total_volume: Option<rust_decimal::Decimal> = row.try_get("total_volume").ok();

        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            root_symbol.unwrap_or_else(|| "?".to_string()),
            symbol,
            candle_count,
            first_ts
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string()),
            last_ts
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string()),
            total_volume
                .map(|value| value.to_string())
                .unwrap_or_else(|| "0".to_string())
        );
    }

    Ok(())
}
