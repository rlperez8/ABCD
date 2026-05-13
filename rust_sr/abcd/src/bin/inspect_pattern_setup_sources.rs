use std::env;

use sqlx::{mysql::MySqlPool, Row};

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

    let pool = MySqlPool::connect(&database_url_from_env()?).await?;

    let total = sqlx::query(
        r#"
        SELECT
            CAST(COUNT(*) AS SIGNED) AS setup_count,
            DATE(MIN(d_date)) AS first_d_date,
            DATE(MAX(d_date)) AS last_d_date
        FROM pattern_setups
        "#,
    )
    .fetch_one(&pool)
    .await?;
    println!(
        "pattern_setups total={} first={} last={}",
        total.try_get::<i64, _>("setup_count")?,
        total
            .try_get::<Option<chrono::NaiveDate>, _>("first_d_date")?
            .map(|value| value.to_string())
            .unwrap_or_else(|| "N/A".to_string()),
        total
            .try_get::<Option<chrono::NaiveDate>, _>("last_d_date")?
            .map(|value| value.to_string())
            .unwrap_or_else(|| "N/A".to_string())
    );

    println!();
    println!("source_table\ttimeframe\tsetups\tsymbols\troots\tfirst_d\tlast_d");
    let by_source = sqlx::query(
        r#"
        SELECT
            COALESCE(NULLIF(source_table, ''), 'Unknown') AS source_table,
            COALESCE(NULLIF(source_timeframe, ''), 'Unknown') AS source_timeframe,
            CAST(COUNT(*) AS SIGNED) AS setup_count,
            CAST(COUNT(DISTINCT symbol) AS SIGNED) AS symbol_count,
            CAST(COUNT(DISTINCT COALESCE(root_symbol, '')) AS SIGNED) AS root_count,
            DATE(MIN(d_date)) AS first_d_date,
            DATE(MAX(d_date)) AS last_d_date
        FROM pattern_setups
        GROUP BY
            COALESCE(NULLIF(source_table, ''), 'Unknown'),
            COALESCE(NULLIF(source_timeframe, ''), 'Unknown')
        ORDER BY setup_count DESC
        "#,
    )
    .fetch_all(&pool)
    .await?;
    for row in by_source {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            row.try_get::<String, _>("source_table")?,
            row.try_get::<String, _>("source_timeframe")?,
            row.try_get::<i64, _>("setup_count")?,
            row.try_get::<i64, _>("symbol_count")?,
            row.try_get::<i64, _>("root_count")?,
            row.try_get::<Option<chrono::NaiveDate>, _>("first_d_date")?
                .map(|value| value.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            row.try_get::<Option<chrono::NaiveDate>, _>("last_d_date")?
                .map(|value| value.to_string())
                .unwrap_or_else(|| "N/A".to_string())
        );
    }

    println!();
    println!("pre_2021 roots");
    println!("root\tsource_table\ttimeframe\tsetups\tsymbols\tfirst_d\tlast_d");
    let pre_2021_roots = sqlx::query(
        r#"
        SELECT
            COALESCE(NULLIF(root_symbol, ''), 'Unknown') AS root_symbol,
            COALESCE(NULLIF(source_table, ''), 'Unknown') AS source_table,
            COALESCE(NULLIF(source_timeframe, ''), 'Unknown') AS source_timeframe,
            CAST(COUNT(*) AS SIGNED) AS setup_count,
            CAST(COUNT(DISTINCT symbol) AS SIGNED) AS symbol_count,
            DATE(MIN(d_date)) AS first_d_date,
            DATE(MAX(d_date)) AS last_d_date
        FROM pattern_setups
        WHERE d_date < '2021-01-01'
        GROUP BY
            COALESCE(NULLIF(root_symbol, ''), 'Unknown'),
            COALESCE(NULLIF(source_table, ''), 'Unknown'),
            COALESCE(NULLIF(source_timeframe, ''), 'Unknown')
        ORDER BY setup_count DESC
        LIMIT 40
        "#,
    )
    .fetch_all(&pool)
    .await?;
    for row in pre_2021_roots {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            row.try_get::<String, _>("root_symbol")?,
            row.try_get::<String, _>("source_table")?,
            row.try_get::<String, _>("source_timeframe")?,
            row.try_get::<i64, _>("setup_count")?,
            row.try_get::<i64, _>("symbol_count")?,
            row.try_get::<Option<chrono::NaiveDate>, _>("first_d_date")?
                .map(|value| value.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            row.try_get::<Option<chrono::NaiveDate>, _>("last_d_date")?
                .map(|value| value.to_string())
                .unwrap_or_else(|| "N/A".to_string())
        );
    }

    Ok(())
}
