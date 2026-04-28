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

    let (prop_rows, d_rows, dreversal_rows): (i64, Option<i64>, Option<i64>) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) AS prop_rows,
            CAST(SUM(CASE WHEN outcome_model = 'D' THEN 1 ELSE 0 END) AS SIGNED) AS d_rows,
            CAST(SUM(CASE WHEN outcome_model = 'DReversal' THEN 1 ELSE 0 END) AS SIGNED) AS dreversal_rows
        FROM pattern_outcomes_prop
        "#,
    )
    .fetch_one(&pool)
    .await?;

    let (summary_rows, template_count): (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) AS summary_rows,
            COUNT(DISTINCT family_name) AS template_count
        FROM prop_strategy_family_summary
        "#,
    )
    .fetch_one(&pool)
    .await?;

    let (yearly_rows, yearly_template_count, yearly_variations): (i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) AS yearly_rows,
            COUNT(DISTINCT family_name) AS yearly_template_count,
            COUNT(DISTINCT family_key) AS yearly_variations
        FROM prop_strategy_family_yearly
        "#,
    )
    .fetch_one(&pool)
    .await?;

    println!("pattern_outcomes_prop rows: {prop_rows}");
    println!("  D rows: {}", d_rows.unwrap_or(0));
    println!("  DReversal rows: {}", dreversal_rows.unwrap_or(0));
    println!("prop_strategy_family_summary rows: {summary_rows}");
    println!("summary templates represented: {template_count}");
    println!("prop_strategy_family_yearly rows: {yearly_rows}");
    println!("yearly templates represented: {yearly_template_count}");
    println!("yearly distinct variations: {yearly_variations}");
    println!();
    println!("family_name\tvariation_rows\ttotal_source_rows\tclosed_source_rows\tmax_source_rows");

    let breakdown: Vec<(String, i64, Option<i64>, Option<i64>, Option<i64>)> = sqlx::query_as(
        r#"
        SELECT
            family_name,
            COUNT(*) AS variation_rows,
            CAST(SUM(total_count) AS SIGNED) AS total_source_rows,
            CAST(SUM(closed_count) AS SIGNED) AS closed_source_rows,
            CAST(MAX(total_count) AS SIGNED) AS max_source_rows
        FROM prop_strategy_family_summary
        GROUP BY family_name
        ORDER BY MIN(family_level), family_name
        "#,
    )
    .fetch_all(&pool)
    .await?;

    for (family_name, variation_rows, total_source_rows, closed_source_rows, max_source_rows) in
        breakdown
    {
        println!(
            "{family_name}\t{variation_rows}\t{}\t{}\t{}",
            total_source_rows.unwrap_or(0),
            closed_source_rows.unwrap_or(0),
            max_source_rows.unwrap_or(0)
        );
    }

    println!();
    println!(
        "yearly_family_name\tdistinct_variations\tyearly_rows\ttotal_source_rows\tclosed_source_rows\tmax_source_rows"
    );

    let yearly_breakdown: Vec<(String, i64, i64, Option<i64>, Option<i64>, Option<i64>)> =
        sqlx::query_as(
            r#"
            SELECT
                family_name,
                COUNT(DISTINCT family_key) AS distinct_variations,
                COUNT(*) AS yearly_rows,
                CAST(SUM(total_count) AS SIGNED) AS total_source_rows,
                CAST(SUM(closed_count) AS SIGNED) AS closed_source_rows,
                CAST(MAX(total_count) AS SIGNED) AS max_source_rows
            FROM prop_strategy_family_yearly
            GROUP BY family_name
            ORDER BY MIN(family_level), family_name
            "#,
        )
        .fetch_all(&pool)
        .await?;

    for (
        family_name,
        distinct_variations,
        yearly_rows,
        total_source_rows,
        closed_source_rows,
        max_source_rows,
    ) in yearly_breakdown
    {
        println!(
            "{family_name}\t{distinct_variations}\t{yearly_rows}\t{}\t{}\t{}",
            total_source_rows.unwrap_or(0),
            closed_source_rows.unwrap_or(0),
            max_source_rows.unwrap_or(0)
        );
    }

    Ok(())
}
