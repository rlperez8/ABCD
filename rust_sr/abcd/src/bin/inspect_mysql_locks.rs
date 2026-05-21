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

    println!("Active MySQL processes touching Entry/Exit tables:");
    let processes = sqlx::query(
        r#"
        SELECT
            ID,
            USER,
            HOST,
            DB,
            COMMAND,
            TIME,
            STATE,
            INFO
        FROM INFORMATION_SCHEMA.PROCESSLIST
        WHERE INFO LIKE '%entry_exit_template%'
           OR STATE LIKE '%lock%'
           OR COMMAND <> 'Sleep'
        ORDER BY TIME DESC
        LIMIT 20
        "#,
    )
    .fetch_all(&pool)
    .await?;

    for row in processes {
        let id: u64 = row.try_get("ID")?;
        let user: Option<String> = row.try_get("USER").ok();
        let host: Option<String> = row.try_get("HOST").ok();
        let db: Option<String> = row.try_get("DB").ok();
        let command: Option<String> = row.try_get("COMMAND").ok();
        let time: Option<i64> = row.try_get("TIME").ok();
        let state: Option<String> = row.try_get("STATE").ok();
        let info: Option<String> = row.try_get("INFO").ok();
        println!(
            "id={id} user={} host={} db={} command={} time={} state={} info={}",
            user.unwrap_or_else(|| "n/a".to_string()),
            host.unwrap_or_else(|| "n/a".to_string()),
            db.unwrap_or_else(|| "n/a".to_string()),
            command.unwrap_or_else(|| "n/a".to_string()),
            time.unwrap_or(0),
            state.unwrap_or_else(|| "n/a".to_string()),
            info.map(|value| value.chars().take(240).collect::<String>())
                .unwrap_or_else(|| "n/a".to_string())
        );
    }

    println!();
    println!("Active InnoDB transactions:");
    let trx_rows = sqlx::query(
        r#"
        SELECT
            trx_id,
            trx_state,
            trx_started,
            trx_wait_started,
            trx_mysql_thread_id,
            trx_query,
            trx_rows_locked,
            trx_rows_modified
        FROM INFORMATION_SCHEMA.INNODB_TRX
        ORDER BY trx_started ASC
        LIMIT 20
        "#,
    )
    .fetch_all(&pool)
    .await?;

    if trx_rows.is_empty() {
        println!("none");
    }

    for row in trx_rows {
        let trx_id: Option<String> = row.try_get("trx_id").ok();
        let trx_state: Option<String> = row.try_get("trx_state").ok();
        let trx_started: Option<String> = row.try_get("trx_started").ok();
        let trx_wait_started: Option<String> = row.try_get("trx_wait_started").ok();
        let trx_mysql_thread_id: Option<u64> = row.try_get("trx_mysql_thread_id").ok();
        let trx_query: Option<String> = row.try_get("trx_query").ok();
        let trx_rows_locked: Option<i64> = row.try_get("trx_rows_locked").ok();
        let trx_rows_modified: Option<i64> = row.try_get("trx_rows_modified").ok();
        println!(
            "trx={} state={} started={} wait_started={} thread={} rows_locked={} rows_modified={} query={}",
            trx_id.unwrap_or_else(|| "n/a".to_string()),
            trx_state.unwrap_or_else(|| "n/a".to_string()),
            trx_started.unwrap_or_else(|| "n/a".to_string()),
            trx_wait_started.unwrap_or_else(|| "n/a".to_string()),
            trx_mysql_thread_id.unwrap_or(0),
            trx_rows_locked.unwrap_or(0),
            trx_rows_modified.unwrap_or(0),
            trx_query
                .map(|value| value.chars().take(240).collect::<String>())
                .unwrap_or_else(|| "n/a".to_string())
        );
    }

    Ok(())
}
