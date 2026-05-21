use std::env;

use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use tokio::time::{sleep, Duration};

const DELETE_BATCH_SIZE: u64 = 25_000;

const RUN_SCOPED_TABLES: [&str; 7] = [
    "entry_exit_template_condition_stats",
    "entry_exit_template_ui_stats",
    "entry_exit_template_build_coverage_rows_ui",
    "entry_exit_template_build_coverage_ui",
    "entry_exit_template_build_summary_ui",
    "entry_exit_template_results",
    "entry_exit_templates",
];

fn usage() -> &'static str {
    "Usage: cargo run --bin delete_entry_exit_build_run -- --run-id RUN_ID\nDeletes one Entry/Exit build run and its stored UI/dashboard rows."
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|items| items[0] == name)
        .map(|items| items[1].trim().to_string())
        .filter(|value| !value.is_empty())
}

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

fn result_table_name_for_run(run_id: &str) -> String {
    let suffix = run_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("entry_exit_template_results_{suffix}")
}

fn quoted_identifier(identifier: &str) -> String {
    debug_assert!(identifier
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_'));
    format!("`{identifier}`")
}

async fn table_exists(pool: &MySqlPool, table: &str) -> Result<bool, sqlx::Error> {
    let exists: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM INFORMATION_SCHEMA.TABLES
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_NAME = ?
        "#,
    )
    .bind(table)
    .fetch_one(pool)
    .await?;

    Ok(exists > 0)
}

async fn count_for_run(pool: &MySqlPool, table: &str, column: &str, run_id: &str) -> i64 {
    let sql = format!("SELECT COUNT(*) FROM {table} WHERE {column} = ?");
    sqlx::query_scalar::<_, i64>(&sql)
        .bind(run_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0)
}

fn is_lock_timeout(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(db_error) => {
            db_error.message().contains("Lock wait timeout")
                || db_error.message().contains("1205")
                || db_error.code().as_deref() == Some("HY000")
        }
        _ => false,
    }
}

async fn delete_for_run(
    pool: &MySqlPool,
    table: &str,
    column: &str,
    run_id: &str,
) -> Result<u64, sqlx::Error> {
    let mut total_deleted = 0_u64;
    let mut lock_timeouts = 0_u64;
    loop {
        let sql = format!("DELETE FROM {table} WHERE {column} = ? LIMIT {DELETE_BATCH_SIZE}");
        let deleted = match sqlx::query(&sql).bind(run_id).execute(pool).await {
            Ok(result) => {
                lock_timeouts = 0;
                result.rows_affected()
            }
            Err(error) if is_lock_timeout(&error) && lock_timeouts < 20 => {
                lock_timeouts += 1;
                println!("{table}: lock timeout, retrying in {}s", lock_timeouts * 2);
                sleep(Duration::from_secs(lock_timeouts * 2)).await;
                continue;
            }
            Err(error) => return Err(error),
        };
        total_deleted += deleted;
        if deleted == 0 {
            break;
        }
        println!("{table}: deleted batch {deleted}, total {total_deleted}");
        if deleted < DELETE_BATCH_SIZE {
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }

    Ok(total_deleted)
}

async fn drop_table_if_exists(pool: &MySqlPool, table: &str) -> Result<(), sqlx::Error> {
    if table_exists(pool, table).await? {
        sqlx::query(&format!("DROP TABLE {}", quoted_identifier(table)))
            .execute(pool)
            .await?;
        println!("{table}: dropped");
    } else {
        println!("{table}: missing");
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let raw_args = env::args().skip(1).collect::<Vec<_>>();
    if raw_args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", usage());
        return Ok(());
    }

    let Some(run_id) = arg_value(&raw_args, "--run-id") else {
        return Err(usage().into());
    };
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(&database_url_from_env()?)
        .await?;

    println!("Deleting Entry/Exit build run {run_id}");
    let result_table_name = result_table_name_for_run(&run_id);
    drop_table_if_exists(&pool, &result_table_name).await?;

    for table in RUN_SCOPED_TABLES {
        if table == "entry_exit_template_results" {
            println!("{table}: shared schema table kept");
            continue;
        }
        if !table_exists(&pool, table).await? {
            println!("{table}: missing");
            continue;
        }
        let column = if table == "entry_exit_templates" {
            "origin_run_id"
        } else {
            "run_id"
        };
        let before = count_for_run(&pool, table, column, &run_id).await;
        if before == 0 {
            println!("{table}: 0 rows");
            continue;
        }
        let deleted = delete_for_run(&pool, table, column, &run_id).await?;
        println!("{table}: deleted {deleted} / before {before}");
    }

    if table_exists(&pool, "entry_exit_template_creator_runs").await? {
        let before =
            count_for_run(&pool, "entry_exit_template_creator_runs", "run_id", &run_id).await;
        let deleted =
            delete_for_run(&pool, "entry_exit_template_creator_runs", "run_id", &run_id).await?;
        println!("entry_exit_template_creator_runs: deleted {deleted} / before {before}");
    }

    Ok(())
}
