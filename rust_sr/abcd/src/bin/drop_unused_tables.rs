use std::env;

use sqlx::mysql::MySqlPool;

const UNUSED_TABLES: [&str; 9] = [
    "abcds_sheet_df",
    "failed_pattern_ab",
    "listing_status_backup_20260412_230714",
    "pattern_a",
    "pattern_ab",
    "pattern_abc",
    "pattern_abcd",
    "pattern_abcd_rust",
    "simple_open_trades_14d",
];

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

async fn table_exists(pool: &MySqlPool, table: &str) -> Result<bool, sqlx::Error> {
    let exists: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM INFORMATION_SCHEMA.TABLES
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_NAME = ?
        "#,
    )
    .bind(table)
    .fetch_one(pool)
    .await?;

    Ok(exists > 0)
}

async fn table_count(pool: &MySqlPool, table: &str) -> Result<i64, sqlx::Error> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    sqlx::query_scalar::<_, i64>(&sql).fetch_one(pool).await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url = database_url_from_env()?;
    let pool = MySqlPool::connect(&database_url).await?;

    println!("Dropping unused tables:");
    for table in UNUSED_TABLES {
        if table_exists(&pool, table).await? {
            let count = table_count(&pool, table).await?;
            println!("dropping {table} ({count} rows)");
            let sql = format!("DROP TABLE IF EXISTS {table}");
            sqlx::query(&sql).execute(&pool).await?;
        } else {
            println!("skipping {table}: missing");
        }
    }

    println!("Unused table cleanup complete.");
    Ok(())
}
