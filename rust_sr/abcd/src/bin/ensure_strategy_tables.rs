use std::env;

use abcd::models::database::Database;
use sqlx::mysql::MySqlPool;

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .or_else(|_| env::var("MYSQL_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL, DATABASE_URL, or MYSQL_URL"
                .into()
        })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url = database_url_from_env()?;
    let pool = MySqlPool::connect(&database_url).await?;
    let db = Database { pool };

    db.ensure_swing_strategy_yearly_table().await?;
    db.ensure_swing_strategy_summary_table().await?;
    db.ensure_prop_strategy_family_yearly_table().await?;
    db.ensure_prop_strategy_family_summary_table().await?;

    println!("Ensured strategy tables:");
    println!("  swing_strategy_summary");
    println!("  swing_strategy_yearly");
    println!("  prop_strategy_family_summary");
    println!("  prop_strategy_family_yearly");

    Ok(())
}
