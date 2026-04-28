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

    let database_url = database_url_from_env()?;
    let pool = MySqlPool::connect(&database_url).await?;

    let rows = sqlx::query(
        r#"
        SELECT
            table_name,
            table_rows
        FROM INFORMATION_SCHEMA.TABLES
        WHERE TABLE_SCHEMA = DATABASE()
        ORDER BY table_name
        "#,
    )
    .fetch_all(&pool)
    .await?;

    for row in rows {
        let table_name: String = row.try_get(0)?;
        let estimated_rows: Option<u64> = row.try_get(1).ok();
        let count_sql = format!("SELECT COUNT(*) AS count FROM {table_name}");
        let exact_count = sqlx::query_scalar::<_, i64>(&count_sql)
            .fetch_one(&pool)
            .await
            .ok();

        println!(
            "{}\t{}",
            table_name,
            exact_count
                .map(|count| count.to_string())
                .or_else(|| estimated_rows.map(|count| format!("~{count}")))
                .unwrap_or_else(|| "unknown".to_string())
        );
    }

    Ok(())
}
