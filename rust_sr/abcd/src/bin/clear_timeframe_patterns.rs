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

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|window| window[0] == name)
        .map(|window| window[1].clone())
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| arg == name)
}

fn usage() -> &'static str {
    "Usage: cargo run --bin clear_timeframe_patterns -- --timeframe 5m --source-table futures_contract_5m_candles --apply"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let args: Vec<String> = env::args().collect();
    let Some(timeframe) = arg_value(&args, "--timeframe") else {
        return Err(usage().into());
    };
    let source_table = arg_value(&args, "--source-table")
        .unwrap_or_else(|| format!("futures_contract_{}_candles", timeframe));
    let apply = has_flag(&args, "--apply");

    let pool = MySqlPool::connect(&database_url_from_env()?).await?;

    let before = sqlx::query(
        r#"
        SELECT COUNT(*) AS count
        FROM pattern_setups
        WHERE source_timeframe = ?
          AND source_table = ?
        "#,
    )
    .bind(&timeframe)
    .bind(&source_table)
    .fetch_one(&pool)
    .await?
    .try_get::<i64, _>("count")?;

    println!("{source_table} / {timeframe}: {before} pattern_setups rows matched");

    if !apply {
        println!("dry run only; pass --apply to delete");
        return Ok(());
    }

    let result = sqlx::query(
        r#"
        DELETE FROM pattern_setups
        WHERE source_timeframe = ?
          AND source_table = ?
        "#,
    )
    .bind(&timeframe)
    .bind(&source_table)
    .execute(&pool)
    .await?;

    println!("deleted {} pattern_setups rows", result.rows_affected());

    let after = sqlx::query(
        r#"
        SELECT COUNT(*) AS count
        FROM pattern_setups
        WHERE source_timeframe = ?
          AND source_table = ?
        "#,
    )
    .bind(&timeframe)
    .bind(&source_table)
    .fetch_one(&pool)
    .await?
    .try_get::<i64, _>("count")?;

    println!("{source_table} / {timeframe}: {after} rows remain");

    Ok(())
}
