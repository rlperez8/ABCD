use std::env;
use std::time::Duration;

use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::MySqlConnection;
use tokio::time::sleep;

const ENTRY_EXIT_TEMPLATE_TABLES: [&str; 5] = [
    "entry_exit_template_condition_stats",
    "entry_exit_template_ui_stats",
    "entry_exit_template_results",
    "entry_exit_templates",
    "entry_exit_template_creator_runs",
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

fn is_lock_timeout(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(db_error) => {
            db_error.code().as_deref() == Some("HY000") && db_error.message().contains("1205")
        }
        _ => false,
    }
}

async fn clear_table_with_retry(
    conn: &mut MySqlConnection,
    table: &str,
) -> Result<(), sqlx::Error> {
    let sql = format!("TRUNCATE TABLE {table}");

    for attempt in 1..=3 {
        match sqlx::query(&sql).execute(&mut *conn).await {
            Ok(_) => {
                println!("truncated {table}");
                return Ok(());
            }
            Err(error) if is_lock_timeout(&error) && attempt < 3 => {
                println!(
                    "lock timeout truncating {table}; retrying attempt {}",
                    attempt + 1
                );
                sleep(Duration::from_secs(2 * attempt)).await;
            }
            Err(error) => return Err(error),
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(&database_url_from_env()?)
        .await?;

    let mut existing_tables = Vec::new();
    println!("Clearing Entry/Exit template creator tables:");
    for table in ENTRY_EXIT_TEMPLATE_TABLES {
        if table_exists(&pool, table).await? {
            println!("before {table}={}", table_count(&pool, table).await?);
            existing_tables.push(table);
        } else {
            println!("before {table}=MISSING");
        }
    }

    let mut conn = pool.acquire().await?;
    sqlx::query("SET FOREIGN_KEY_CHECKS = 0")
        .execute(&mut *conn)
        .await?;

    let mut clear_result = Ok(());
    for table in &existing_tables {
        if let Err(error) = clear_table_with_retry(&mut conn, table).await {
            clear_result = Err(error);
            break;
        }
    }

    sqlx::query("SET FOREIGN_KEY_CHECKS = 1")
        .execute(&mut *conn)
        .await?;

    drop(conn);
    clear_result?;

    println!("Counts after clear:");
    for table in existing_tables {
        println!("after {table}={}", table_count(&pool, table).await?);
    }

    Ok(())
}
