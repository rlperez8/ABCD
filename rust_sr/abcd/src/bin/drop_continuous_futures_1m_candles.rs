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

    if env::var("ABCD_CONFIRM_DROP_CONTINUOUS").ok().as_deref() != Some("1") {
        println!("Set ABCD_CONFIRM_DROP_CONTINUOUS=1 to delete continuous futures rows.");
        println!("This targets symbols containing '.c.' in futures_contract_1m_candles.");
        return Ok(());
    }

    let pool = MySqlPool::connect(&database_url_from_env()?).await?;
    let before: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM futures_contract_1m_candles
        WHERE symbol LIKE '%.c.%'
        "#,
    )
    .fetch_one(&pool)
    .await?;

    let result = sqlx::query(
        r#"
        DELETE FROM futures_contract_1m_candles
        WHERE symbol LIKE '%.c.%'
        "#,
    )
    .execute(&pool)
    .await?;

    println!(
        "continuous futures rows before={}, deleted={}",
        before,
        result.rows_affected()
    );

    Ok(())
}
