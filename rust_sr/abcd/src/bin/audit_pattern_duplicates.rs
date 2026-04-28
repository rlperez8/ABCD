use std::env;

use chrono::NaiveDate;
use sqlx::mysql::MySqlPool;

#[path = "../models/mod.rs"]
mod models;

use models::database::Database;

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

#[derive(sqlx::FromRow)]
struct IdentityCoverageRow {
    total_rows: i64,
    distinct_pattern_ids: i64,
    missing_pattern_ids: i64,
}

#[derive(sqlx::FromRow)]
struct DuplicateSummaryRow {
    duplicate_groups: i64,
    duplicate_rows: i64,
}

#[derive(sqlx::FromRow)]
struct DuplicatePatternRow {
    pattern_id: String,
    row_count: i64,
    strategy_count: i64,
    symbols: Option<String>,
    first_d_date: Option<NaiveDate>,
    last_d_date: Option<NaiveDate>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url = database_url_from_env()?;
    let pool = MySqlPool::connect(&database_url).await?;
    let db = Database { pool: pool.clone() };

    db.backfill_xabcd_pattern_ids().await?;

    let coverage = sqlx::query_as::<_, IdentityCoverageRow>(
        r#"
        SELECT
            CAST(COUNT(*) AS SIGNED) AS total_rows,
            CAST(COUNT(DISTINCT pattern_id) AS SIGNED) AS distinct_pattern_ids,
            CAST(COALESCE(SUM(CASE WHEN pattern_id IS NULL OR pattern_id = '' THEN 1 ELSE 0 END), 0) AS SIGNED) AS missing_pattern_ids
        FROM xabcd_patterns
        "#,
    )
    .fetch_one(&pool)
    .await?;

    let duplicate_summary = sqlx::query_as::<_, DuplicateSummaryRow>(
        r#"
        SELECT
            CAST(COUNT(*) AS SIGNED) AS duplicate_groups,
            CAST(COALESCE(SUM(row_count), 0) AS SIGNED) AS duplicate_rows
        FROM (
            SELECT COUNT(*) AS row_count
            FROM xabcd_patterns
            WHERE pattern_id IS NOT NULL
              AND pattern_id <> ''
            GROUP BY pattern_id
            HAVING COUNT(*) > 1
        ) duplicate_groups
        "#,
    )
    .fetch_one(&pool)
    .await?;

    let duplicates = sqlx::query_as::<_, DuplicatePatternRow>(
        r#"
        SELECT
            pattern_id,
            COUNT(*) AS row_count,
            COUNT(DISTINCT COALESCE(prop_strategy_id, '')) AS strategy_count,
            GROUP_CONCAT(DISTINCT symbol ORDER BY symbol SEPARATOR ',') AS symbols,
            MIN(d_date) AS first_d_date,
            MAX(d_date) AS last_d_date
        FROM xabcd_patterns
        WHERE pattern_id IS NOT NULL
          AND pattern_id <> ''
        GROUP BY pattern_id
        HAVING COUNT(*) > 1
        ORDER BY row_count DESC, pattern_id ASC
        LIMIT 20
        "#,
    )
    .fetch_all(&pool)
    .await?;

    println!("total_rows={}", coverage.total_rows);
    println!("distinct_pattern_ids={}", coverage.distinct_pattern_ids);
    println!("missing_pattern_ids={}", coverage.missing_pattern_ids);
    println!("duplicate_groups={}", duplicate_summary.duplicate_groups);
    println!("duplicate_rows={}", duplicate_summary.duplicate_rows);

    if duplicates.is_empty() {
        println!("top_duplicate_groups=none");
    } else {
        println!("top_duplicate_groups:");
        for duplicate in duplicates {
            println!(
                "pattern_id={} rows={} strategy_count={} symbols={} first_d_date={:?} last_d_date={:?}",
                duplicate.pattern_id,
                duplicate.row_count,
                duplicate.strategy_count,
                duplicate.symbols.unwrap_or_else(|| "n/a".to_string()),
                duplicate.first_d_date,
                duplicate.last_d_date,
            );
        }
    }

    Ok(())
}
