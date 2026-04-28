use std::env;

use sqlx::mysql::MySqlPool;

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .or_else(|_| env::var("MYSQL_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL, DATABASE_URL, or MYSQL_URL"
                .into()
        })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url = database_url_from_env()?;
    let pool = MySqlPool::connect(&database_url).await?;

    let legacy_tables = [
        "strategy_cohort_summary_cache",
        "strategy_yearly_rollup",
        "strategy_trade_summary_cache",
        "current_open_setups_cache",
        "prop_strategy_cohort_summary_cache",
        "prop_strategy_yearly_rollup",
        "prop_reversal_strategy_cohort_summary_cache",
        "prop_reversal_strategy_yearly_rollup",
        "pattern_outcomes_prop_reversal",
        "prop_strategy_summary",
        "prop_strategy_yearly",
        "prop_reversal_strategy_summary",
        "prop_reversal_strategy_yearly",
    ];

    for table in legacy_tables {
        sqlx::query(&format!("DROP TABLE IF EXISTS {table}"))
            .execute(&pool)
            .await?;
        println!("Dropped legacy table if present: {table}");
    }

    Ok(())
}
