use abcd::models::database::Database;
use sqlx::MySqlPool;
use std::env;

fn database_url() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

fn env_f64(name: &str, default: f64) -> Result<f64, Box<dyn std::error::Error>> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(value.parse::<f64>()?),
        _ => Ok(default),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = database_url()?;
    let minimum_average_volume = env_f64("ABCD_MIN_AVG_VOLUME", 500_000.0)?;

    let pool = MySqlPool::connect(&database_url).await?;
    let db = Database { pool };

    let symbols = db
        .get_symbols_above_average_volume(minimum_average_volume)
        .await?;

    println!(
        "eligible_symbols={} minimum_average_volume={}",
        symbols.len(),
        minimum_average_volume
    );

    Ok(())
}
