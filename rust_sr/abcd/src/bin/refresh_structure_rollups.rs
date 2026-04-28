use std::env;

use abcd::models::database::Database;
use sqlx::mysql::MySqlPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .or_else(|_| env::var("MYSQL_URL"))
        .map_err(|_| "DATABASE_URL or MYSQL_URL must be set")?;

    let pool = MySqlPool::connect(&database_url).await?;
    let db = Database { pool };

    db.refresh_structure_rollups().await?;
    println!("Refreshed structure rollups successfully");

    Ok(())
}
