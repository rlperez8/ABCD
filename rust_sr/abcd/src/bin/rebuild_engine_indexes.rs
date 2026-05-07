use std::env;
use std::time::Instant;

use abcd::models::database::Database;
use sqlx::mysql::MySqlPool;

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

    let started = Instant::now();
    let pool = MySqlPool::connect(&database_url_from_env()?).await?;
    let db = Database { pool };

    println!("Rebuilding generated engine secondary indexes...");
    db.recreate_rebuild_secondary_indexes().await?;
    println!(
        "Engine secondary indexes rebuilt in {:.1}s",
        started.elapsed().as_secs_f64()
    );

    Ok(())
}
