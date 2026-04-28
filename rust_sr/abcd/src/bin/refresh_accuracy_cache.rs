#[path = "../models/mod.rs"]
mod models;

use models::database::Database;
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

    db.refresh_accuracy_bin_cache().await?;
    println!("Refreshed accuracy_bin_cache successfully");

    Ok(())
}
