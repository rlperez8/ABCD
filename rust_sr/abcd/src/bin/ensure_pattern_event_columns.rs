use std::env;

use sqlx::mysql::MySqlPool;

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

async fn table_exists(pool: &MySqlPool, table: &str) -> Result<bool, sqlx::Error> {
    let count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM information_schema.TABLES
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_NAME = ?
        "#,
    )
    .bind(table)
    .fetch_one(pool)
    .await?;

    Ok(count > 0)
}

async fn column_exists(pool: &MySqlPool, table: &str, column: &str) -> Result<bool, sqlx::Error> {
    let count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM information_schema.COLUMNS
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_NAME = ?
          AND COLUMN_NAME = ?
        "#,
    )
    .bind(table)
    .bind(column)
    .fetch_one(pool)
    .await?;

    Ok(count > 0)
}

async fn index_exists(pool: &MySqlPool, table: &str, index: &str) -> Result<bool, sqlx::Error> {
    let count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM information_schema.STATISTICS
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_NAME = ?
          AND INDEX_NAME = ?
        "#,
    )
    .bind(table)
    .bind(index)
    .fetch_one(pool)
    .await?;

    Ok(count > 0)
}

async fn ensure_column(
    pool: &MySqlPool,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), sqlx::Error> {
    if column_exists(pool, table, column).await? {
        println!("{table}.{column} exists");
        return Ok(());
    }

    let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {definition}");
    sqlx::query(&sql).execute(pool).await?;
    println!("{table}.{column} created");
    Ok(())
}

async fn ensure_index(
    pool: &MySqlPool,
    table: &str,
    index: &str,
    definition: &str,
) -> Result<(), sqlx::Error> {
    if index_exists(pool, table, index).await? {
        println!("{table}.{index} exists");
        return Ok(());
    }

    let sql = format!("ALTER TABLE {table} ADD INDEX {index} {definition}");
    sqlx::query(&sql).execute(pool).await?;
    println!("{table}.{index} created");
    Ok(())
}

async fn ensure_pattern_event_columns(pool: &MySqlPool, table: &str) -> Result<(), sqlx::Error> {
    if !table_exists(pool, table).await? {
        println!("{table} missing, skipped");
        return Ok(());
    }

    ensure_column(pool, table, "event_id", "VARCHAR(64) NULL").await?;
    ensure_column(pool, table, "event_rank", "INT NULL").await?;
    ensure_column(
        pool,
        table,
        "is_event_primary",
        "TINYINT NOT NULL DEFAULT 0",
    )
    .await?;
    ensure_column(pool, table, "event_sister_count", "INT NOT NULL DEFAULT 1").await?;
    ensure_column(pool, table, "event_similarity_score", "DOUBLE NULL").await?;

    ensure_index(
        pool,
        table,
        "idx_pattern_setups_event",
        "(event_id, event_rank)",
    )
    .await?;
    ensure_index(
        pool,
        table,
        "idx_pattern_setups_event_scan",
        "(symbol, market, d_confirm_date)",
    )
    .await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url = database_url_from_env()?;
    let pool = MySqlPool::connect(&database_url).await?;

    ensure_pattern_event_columns(&pool, "pattern_setups").await?;
    ensure_pattern_event_columns(&pool, "pattern_setups_build").await?;

    Ok(())
}
