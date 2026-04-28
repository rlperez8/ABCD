use std::env;

use sqlx::mysql::MySqlPool;

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

    let high_bins = "('50-60', '60-70', '70-80', '80-90', '90-100')";

    let (outcome_rows, high_price_rows, high_time_rows, high_both_rows): (
        i64,
        Option<i64>,
        Option<i64>,
        Option<i64>,
    ) = sqlx::query_as(&format!(
        r#"
        SELECT
            COUNT(*) AS outcome_rows,
            CAST(SUM(CASE WHEN bin IN {high_bins} THEN 1 ELSE 0 END) AS SIGNED) AS high_price_rows,
            CAST(SUM(CASE WHEN time_bin IN {high_bins} THEN 1 ELSE 0 END) AS SIGNED) AS high_time_rows,
            CAST(SUM(CASE WHEN bin IN {high_bins} AND time_bin IN {high_bins} THEN 1 ELSE 0 END) AS SIGNED) AS high_both_rows
        FROM pattern_outcomes_prop
        "#
    ))
    .fetch_one(&pool)
    .await?;

    println!("pattern_outcomes_prop rows: {outcome_rows}");
    println!("price bin >= 50 rows: {}", high_price_rows.unwrap_or(0));
    println!("time bin >= 50 rows: {}", high_time_rows.unwrap_or(0));
    println!(
        "price + time bin >= 50 rows: {}",
        high_both_rows.unwrap_or(0)
    );
    println!();
    println!("bin\ttime_bin\toutcome_rows\tdistinct_patterns\tclosed_rows");

    let rows: Vec<(String, String, i64, i64, Option<i64>)> = sqlx::query_as(
        r#"
        SELECT
            bin,
            time_bin,
            COUNT(*) AS outcome_rows,
            COUNT(DISTINCT COALESCE(NULLIF(pattern_id, ''), setup_id)) AS distinct_patterns,
            CAST(SUM(CASE WHEN target_date IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS closed_rows
        FROM pattern_outcomes_prop
        GROUP BY bin, time_bin
        ORDER BY FIELD(bin, '0-10', '10-20', '20-30', '30-40', '40-50', '50-60', '60-70', '70-80', '80-90', '90-100'),
                 FIELD(time_bin, '0-10', '10-20', '20-30', '30-40', '40-50', '50-60', '60-70', '70-80', '80-90', '90-100')
        "#,
    )
    .fetch_all(&pool)
    .await?;

    for (bin, time_bin, count, distinct_patterns, closed_rows) in rows {
        println!(
            "{bin}\t{time_bin}\t{count}\t{distinct_patterns}\t{}",
            closed_rows.unwrap_or(0)
        );
    }

    println!();
    println!("family_name\tvisible_default_rows\tvisible_default_closed");

    let summary_rows: Vec<(String, Option<i64>, Option<i64>)> = sqlx::query_as(&format!(
        r#"
        SELECT
            family_name,
            CAST(SUM(total_count) AS SIGNED) AS visible_default_rows,
            CAST(SUM(closed_count) AS SIGNED) AS visible_default_closed
        FROM prop_strategy_family_summary
        WHERE bin IN {high_bins}
          AND time_bin IN {high_bins}
          AND size_bucket IN ('Micro', 'Small', 'Normal')
        GROUP BY family_name
        ORDER BY visible_default_rows DESC
        "#
    ))
    .fetch_all(&pool)
    .await?;

    if summary_rows.is_empty() {
        println!("no default-visible family summary rows");
    } else {
        for (family_name, total_count, closed_count) in summary_rows {
            println!(
                "{family_name}\t{}\t{}",
                total_count.unwrap_or(0),
                closed_count.unwrap_or(0)
            );
        }
    }

    let (default_family_rows, default_hard_filter_rows): (i64, Option<i64>) =
        sqlx::query_as(&format!(
            r#"
            SELECT
                COUNT(*) AS default_family_rows,
                CAST(SUM(CASE
                    WHEN closed_count >= 50
                     AND expectancy > 0
                     AND down_years <= 0
                     AND worst_year_expectancy > 0
                     AND COALESCE(score, 0) > 0
                    THEN 1 ELSE 0 END) AS SIGNED) AS default_hard_filter_rows
            FROM prop_strategy_family_summary
            WHERE bin IN {high_bins}
              AND time_bin IN {high_bins}
              AND size_bucket IN ('Micro', 'Small', 'Normal')
            "#
        ))
        .fetch_one(&pool)
        .await?;

    println!();
    println!("default-visible family rows: {default_family_rows}");
    println!(
        "default-visible rows passing current hard filters: {}",
        default_hard_filter_rows.unwrap_or(0)
    );

    Ok(())
}
