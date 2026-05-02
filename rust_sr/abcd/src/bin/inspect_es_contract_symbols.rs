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

    let root = env::var("ABCD_FUTURES_ROOT").unwrap_or_else(|_| "ES".to_string());
    let pool = MySqlPool::connect(&database_url_from_env()?).await?;
    let rows = sqlx::query(
        r#"
        SELECT
            symbol,
            COUNT(*) AS candle_count,
            MIN(ts_utc) AS first_ts,
            MAX(ts_utc) AS last_ts
        FROM futures_contract_1m_candles
        WHERE root_symbol = ?
        GROUP BY symbol
        ORDER BY symbol
        "#,
    )
    .bind(&root)
    .fetch_all(&pool)
    .await?;

    println!("Contracts for root {root}: {}", rows.len());
    println!("symbol\tcandles\tfirst_ts\tlast_ts");
    for row in rows {
        let symbol: String = row.try_get("symbol")?;
        let candle_count: i64 = row.try_get("candle_count")?;
        let first_ts: Option<chrono::NaiveDateTime> = row.try_get("first_ts").ok();
        let last_ts: Option<chrono::NaiveDateTime> = row.try_get("last_ts").ok();

        println!(
            "{}\t{}\t{}\t{}",
            symbol,
            candle_count,
            first_ts
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string()),
            last_ts
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string())
        );
    }

    Ok(())
}
