use abcd::models::database::Database;
use sqlx::mysql::MySqlPool;

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    std::env::var("ABCD_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url = database_url_from_env()?;
    let pool = MySqlPool::connect(&database_url).await?;
    let db = Database { pool };

    db.ensure_candle_trend_columns().await?;

    let symbols = db.get_distinct_symbols().await?;
    let total_symbols = symbols.len();

    println!("Refreshing candle trends for {} symbols", total_symbols);

    for (index, symbol) in symbols.iter().enumerate() {
        db.get_stored_candles_with_trend_persist(symbol, true)
            .await?;

        if (index + 1) % 100 == 0 || index + 1 == total_symbols {
            println!(
                "Refreshed candle trends for {}/{} symbols",
                index + 1,
                total_symbols
            );
        }
    }

    println!("Candle trend refresh complete");

    Ok(())
}
