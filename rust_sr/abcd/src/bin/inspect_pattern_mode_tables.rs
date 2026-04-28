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

async fn fetch_count(pool: &MySqlPool, table: &str) -> Result<i64, sqlx::Error> {
    let sql = format!("SELECT COUNT(*) AS count FROM {}", table);
    let count = sqlx::query_scalar::<_, i64>(&sql).fetch_one(pool).await?;
    Ok(count)
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url = database_url_from_env()?;
    let pool = MySqlPool::connect(&database_url).await?;

    for table in [
        "xabcd_patterns",
        "pattern_setups",
        "pattern_harmonic_scores",
        "pattern_outcomes_swing",
        "pattern_outcomes_prop",
    ] {
        if table_exists(&pool, table).await? {
            let count = fetch_count(&pool, table).await?;
            println!("{}={}", table, count);
        } else {
            println!("{}=MISSING", table);
        }
    }

    println!("processlist:");
    let process_rows = sqlx::query("SHOW FULL PROCESSLIST")
        .fetch_all(&pool)
        .await?;
    for row in process_rows {
        let id: u64 = row.try_get("Id").unwrap_or_default();
        let user: String = row.try_get("User").unwrap_or_default();
        let db: Option<String> = row.try_get("db").ok();
        let command: String = row.try_get("Command").unwrap_or_default();
        let time: i64 = row.try_get("Time").unwrap_or_default();
        let state: Option<String> = row.try_get("State").ok();
        let info: Option<String> = row.try_get("Info").ok();
        println!(
            "id={} user={} db={} command={} time={} state={} info={}",
            id,
            user,
            db.unwrap_or_else(|| "-".to_string()),
            command,
            time,
            state.unwrap_or_else(|| "-".to_string()),
            info.unwrap_or_else(|| "-".to_string())
        );
    }

    Ok(())
}
