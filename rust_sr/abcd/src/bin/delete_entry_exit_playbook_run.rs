use std::env;

use sqlx::mysql::MySqlPool;

fn usage() -> &'static str {
    "Usage: cargo run --bin delete_entry_exit_playbook_run -- --run-id ROUTER_RUN_ID"
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

async fn table_exists(pool: &MySqlPool, table: &str) -> Result<bool, sqlx::Error> {
    let exists: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM information_schema.TABLES
        WHERE table_schema = DATABASE()
          AND table_name = ?
        "#,
    )
    .bind(table)
    .fetch_one(pool)
    .await?;

    Ok(exists > 0)
}

async fn delete_from_table(
    pool: &MySqlPool,
    table: &str,
    run_id: &str,
) -> Result<u64, sqlx::Error> {
    if !table_exists(pool, table).await? {
        return Ok(0);
    }
    let sql = format!("DELETE FROM {table} WHERE router_run_id = ?");
    Ok(sqlx::query(&sql)
        .bind(run_id)
        .execute(pool)
        .await?
        .rows_affected())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let raw_args = env::args().skip(1).collect::<Vec<_>>();
    let Some(run_id) = arg_value(&raw_args, "--run-id") else {
        return Err(usage().into());
    };

    let pool = MySqlPool::connect(&database_url_from_env()?).await?;
    let tables = [
        "entry_exit_template_family_router_results",
        "entry_exit_template_family_router_symbol_choices",
        "entry_exit_template_family_router_choices",
        "entry_exit_template_family_router_runs",
    ];

    let mut total = 0_u64;
    for table in tables {
        let deleted = delete_from_table(&pool, table, &run_id).await?;
        total += deleted;
        println!("{table}={deleted}");
    }
    println!("deleted_total={total}");

    Ok(())
}
