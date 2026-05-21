use std::env;

use sqlx::mysql::MySqlPool;

fn usage() -> &'static str {
    "Usage: cargo run --bin kill_mysql_thread -- THREAD_ID"
}

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

    let thread_id = env::args()
        .nth(1)
        .ok_or_else(|| usage().to_string())?
        .parse::<u64>()?;

    let pool = MySqlPool::connect(&database_url_from_env()?).await?;
    sqlx::query(&format!("KILL {thread_id}"))
        .execute(&pool)
        .await?;
    println!("Killed MySQL thread {thread_id}");

    Ok(())
}
