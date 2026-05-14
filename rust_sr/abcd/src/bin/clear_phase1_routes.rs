use std::env;
use std::time::Duration;

use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::MySqlConnection;
use tokio::time::sleep;

const PHASE1_ROUTE_TABLES: [&str; 3] = [
    "phase1_strategy_yearly_results",
    "phase1_strategy_results",
    "phase1_strategy_runs",
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

async fn ensure_dashboard_cache_state_table(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS dashboard_cache_state (
            cache_name VARCHAR(64) NOT NULL PRIMARY KEY,
            is_ready BOOLEAN NOT NULL DEFAULT FALSE,
            last_completed_year INT NULL,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            note TEXT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn mark_phase1_cache_cleared(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO dashboard_cache_state (
            cache_name,
            is_ready,
            last_completed_year,
            note
        )
        VALUES ('phase1_leaderboard', FALSE, NULL, 'phase1 routes cleared')
        ON DUPLICATE KEY UPDATE
            is_ready = VALUES(is_ready),
            last_completed_year = VALUES(last_completed_year),
            note = VALUES(note)
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
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

    let database_url = database_url_from_env()?;
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;

    let mut existing_tables = Vec::new();

    println!("Clearing Phase 1 route tables with TRUNCATE TABLE:");
    for table in PHASE1_ROUTE_TABLES {
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

    ensure_dashboard_cache_state_table(&pool).await?;
    mark_phase1_cache_cleared(&pool).await?;
    println!("marked phase1_leaderboard=cleared");

    println!("Counts after clear:");
    for table in existing_tables {
        println!("after {table}={}", table_count(&pool, table).await?);
    }

    Ok(())
}
