use std::env;

use sqlx::mysql::MySqlPool;

fn usage() -> &'static str {
    "Usage: cargo run --bin inspect_entry_exit_run_counts -- --run-id RUN_ID"
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

async fn count_for_run(pool: &MySqlPool, table: &str, column: &str, run_id: &str) -> Option<i64> {
    if !table_exists(pool, table).await.ok()? {
        return None;
    }
    let sql = format!("SELECT COUNT(*) FROM {table} WHERE {column} = ?");
    sqlx::query_scalar::<_, i64>(&sql)
        .bind(run_id)
        .fetch_one(pool)
        .await
        .ok()
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let raw_args = env::args().skip(1).collect::<Vec<_>>();
    let Some(run_id) = arg_value(&raw_args, "--run-id") else {
        return Err(usage().into());
    };

    let pool = MySqlPool::connect(&database_url_from_env()?).await?;
    let creator_count = count_for_run(&pool, "entry_exit_template_creator_runs", "run_id", &run_id)
        .await
        .unwrap_or(0);
    let template_count = count_for_run(&pool, "entry_exit_templates", "origin_run_id", &run_id)
        .await
        .unwrap_or(0);
    let ui_stats_count = count_for_run(&pool, "entry_exit_template_ui_stats", "run_id", &run_id)
        .await
        .unwrap_or(0);
    let build_summary_count = count_for_run(
        &pool,
        "entry_exit_template_build_summary_ui",
        "run_id",
        &run_id,
    )
    .await
    .unwrap_or(0);
    let build_coverage_count = count_for_run(
        &pool,
        "entry_exit_template_build_coverage_rows_ui",
        "run_id",
        &run_id,
    )
    .await
    .unwrap_or(0);
    let condition_stats_count = count_for_run(
        &pool,
        "entry_exit_template_condition_stats",
        "run_id",
        &run_id,
    )
    .await
    .unwrap_or(0);
    let family_condition_stats_count = if table_exists(&pool, "entry_exit_template_condition_stats")
        .await
        .unwrap_or(false)
    {
        sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM entry_exit_template_condition_stats
            WHERE run_id = ?
              AND condition_type = 'pattern_family_key'
            "#,
        )
        .bind(&run_id)
        .fetch_one(&pool)
        .await
        .unwrap_or(0)
    } else {
        0
    };
    let raw_result_table = result_table_name_for_run(&run_id);
    let raw_result_table_exists = table_exists(&pool, &raw_result_table)
        .await
        .unwrap_or(false);

    println!("run_id={run_id}");
    println!("creator_rows={creator_count}");
    println!("templates={template_count}");
    println!("ui_stats={ui_stats_count}");
    println!("build_summary_rows={build_summary_count}");
    println!("build_coverage_rows={build_coverage_count}");
    println!("condition_stats_rows={condition_stats_count}");
    println!("family_condition_stats_rows={family_condition_stats_count}");
    println!("raw_result_table={raw_result_table}");
    println!("raw_result_table_exists={raw_result_table_exists}");

    Ok(())
}
