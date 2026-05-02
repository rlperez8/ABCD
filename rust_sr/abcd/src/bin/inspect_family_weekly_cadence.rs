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

    let limit = env::var("ABCD_LIMIT")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(25)
        .clamp(1, 500);
    let pool = MySqlPool::connect(&database_url_from_env()?).await?;

    let rows = sqlx::query(
        r#"
        SELECT
            family_key,
            market,
            harmonic_type,
            bin,
            reversal_type,
            active_weeks,
            total_calendar_weeks,
            zero_setup_week_rate,
            avg_setups_per_week,
            max_setups_per_week,
            total_setups
        FROM prop_strategy_family_weekly_cadence
        ORDER BY active_weeks DESC, avg_setups_per_week DESC
        LIMIT ?
        "#,
    )
    .bind(limit)
    .fetch_all(&pool)
    .await?;

    println!("family_key\tmarket\tpattern\tbin\treversal\tactive/total\tzero_rate\tavg_wk\tmax_wk\tsetups");
    for row in rows {
        let family_key: String = row.try_get("family_key")?;
        let market: String = row.try_get("market")?;
        let harmonic_type: String = row.try_get("harmonic_type")?;
        let bin: String = row.try_get("bin")?;
        let reversal_type: String = row.try_get("reversal_type")?;
        let active_weeks: i64 = row.try_get("active_weeks")?;
        let total_calendar_weeks: i64 = row.try_get("total_calendar_weeks")?;
        let zero_setup_week_rate: f64 = row.try_get("zero_setup_week_rate")?;
        let avg_setups_per_week: f64 = row.try_get("avg_setups_per_week")?;
        let max_setups_per_week: i64 = row.try_get("max_setups_per_week")?;
        let total_setups: i64 = row.try_get("total_setups")?;

        println!(
            "{}\t{}\t{}\t{}\t{}\t{}/{}\t{:.2}%\t{:.2}\t{}\t{}",
            family_key,
            market,
            harmonic_type,
            bin,
            reversal_type,
            active_weeks,
            total_calendar_weeks,
            zero_setup_week_rate * 100.0,
            avg_setups_per_week,
            max_setups_per_week,
            total_setups
        );
    }

    Ok(())
}
