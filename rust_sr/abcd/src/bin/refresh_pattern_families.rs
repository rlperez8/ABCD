use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use abcd::models::database::Database;
use sqlx::mysql::MySqlPool;

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

fn refresh_run_id() -> String {
    let started_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();

    format!(
        "pattern-family-refresh-{}-{}",
        started_at_ms,
        std::process::id()
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url = database_url_from_env()?;
    let pool = MySqlPool::connect(&database_url).await?;
    let db = Database { pool };
    let run_id = refresh_run_id();

    db.ensure_engine_phase_timings_table().await?;
    println!("Pattern family refresh run id: {run_id}");
    db.refresh_pattern_family_source_summary(Some(&run_id))
        .await?;
    println!("Pattern family universe refreshed");

    Ok(())
}
