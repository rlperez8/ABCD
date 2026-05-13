use std::env;

use sqlx::mysql::MySqlPool;

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

fn leg_accuracy_expr(current_expr: &str, target: f64) -> String {
    format!(
        "CASE
            WHEN COALESCE({current_expr}, 0.0) <= 0 THEN 0.0
            ELSE LEAST(
                GREATEST(
                    100.0 * (1.0 - ABS((CAST({current_expr} AS DOUBLE) / 100.0) - {target}) / {target}),
                    0.0
                ),
                100.0
            )
        END"
    )
}

fn ratio_expr(numerator: &str, denominator: &str) -> String {
    format!(
        "CASE
            WHEN COALESCE({denominator}, 0.0) > 0
            THEN (CAST({numerator} AS DOUBLE) / CAST({denominator} AS DOUBLE)) * 100.0
            ELSE 0.0
        END"
    )
}

fn score_select(harmonic_type: &str, ab_xa: f64, bc_ab: f64, cd_bc: f64, cd_xa: f64) -> String {
    let ab_xa_price = ratio_expr("ab_price_length", "xa_price_length");
    let bc_ab_price = ratio_expr("bc_price_length", "ab_price_length");
    let cd_bc_price = ratio_expr("cd_price_length", "bc_price_length");
    let d_completion_price = ratio_expr("ABS(a_min_max - d_min_max)", "xa_price_length");
    let ab_xa_time = ratio_expr("a_length", "x_length");
    let bc_ab_time = ratio_expr("b_length", "a_length");
    let cd_bc_time = ratio_expr("c_length", "b_length");
    let cd_xa_time = ratio_expr("c_length", "x_length");

    format!(
        r#"
        SELECT
            setup_id,
            '{harmonic_type}' AS harmonic_type,
            (
                {price_ab_xa} + {price_bc_ab} + {price_cd_bc} + {price_cd_xa}
            ) / 4.0 AS price_accuracy,
            (
                {time_ab_xa} + {time_bc_ab} + {time_cd_bc} + {time_cd_xa}
            ) / 4.0 AS time_accuracy
        FROM pattern_setups
        "#,
        price_ab_xa = leg_accuracy_expr(&ab_xa_price, ab_xa),
        price_bc_ab = leg_accuracy_expr(&bc_ab_price, bc_ab),
        price_cd_bc = leg_accuracy_expr(&cd_bc_price, cd_bc),
        price_cd_xa = leg_accuracy_expr(&d_completion_price, cd_xa),
        time_ab_xa = leg_accuracy_expr(&ab_xa_time, ab_xa),
        time_bc_ab = leg_accuracy_expr(&bc_ab_time, bc_ab),
        time_cd_bc = leg_accuracy_expr(&cd_bc_time, cd_bc),
        time_cd_xa = leg_accuracy_expr(&cd_xa_time, cd_xa),
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let pool = MySqlPool::connect(&database_url_from_env()?).await?;

    println!(
        "Refreshing pattern_harmonic_scores from pattern_setups with AD/XA completion scoring..."
    );
    sqlx::query("TRUNCATE TABLE pattern_harmonic_scores")
        .execute(&pool)
        .await?;

    let score_selects = [
        score_select("Bat", 0.500, 0.382, 1.618, 0.886),
        score_select("AlternateBat", 0.382, 0.382, 2.0, 1.13),
        score_select("Butterfly", 0.786, 0.382, 1.618, 1.272),
        score_select("Gartley", 0.618, 0.382, 1.272, 0.786),
        score_select("Crab", 0.382, 0.382, 2.618, 1.618),
        score_select("DeepCrab", 0.886, 0.382, 2.618, 1.618),
        score_select("Shark", 0.500, 1.13, 1.618, 0.886),
    ]
    .join("\nUNION ALL\n");

    let sql = format!(
        r#"
        INSERT INTO pattern_harmonic_scores (
            setup_id,
            harmonic_type,
            price_accuracy,
            time_accuracy
        )
        {score_selects}
        "#
    );

    let rows = sqlx::query(&sql).execute(&pool).await?.rows_affected();
    println!("pattern_harmonic_scores rows written: {rows}");

    Ok(())
}
