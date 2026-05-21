use std::env;

use sqlx::{mysql::MySqlPool, Row};

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let pool = MySqlPool::connect(&database_url_from_env()?).await?;

    let template_runs = sqlx::query(
        r#"
        SELECT
            origin_run_id,
            CAST(COUNT(*) AS SIGNED) AS template_count,
            MIN(created_at) AS first_template_at,
            MAX(created_at) AS last_template_at
        FROM entry_exit_templates
        GROUP BY origin_run_id
        ORDER BY MAX(created_at) DESC
        LIMIT 3
        "#,
    )
    .fetch_all(&pool)
    .await?;

    for run in template_runs {
        let run_id: String = run.try_get("origin_run_id")?;
        let template_count: i64 = run.try_get("template_count")?;
        let first_template_at: Option<String> = run.try_get("first_template_at").ok();
        let last_template_at: Option<String> = run.try_get("last_template_at").ok();
        let result_table_name = result_table_name_for_run(&run_id);

        if !table_exists(&pool, &result_table_name).await? {
            println!("run_id={run_id}");
            println!("templates={template_count} result_table={result_table_name} missing");
            println!();
            continue;
        }

        let result = sqlx::query(&format!(
            r#"
            SELECT
                CAST(COUNT(*) AS SIGNED) AS result_rows,
                CAST(COUNT(DISTINCT template_uid) AS SIGNED) AS result_templates,
                CAST(MAX(evaluation_order) AS SIGNED) AS max_evaluation_order,
                MIN(created_at) AS first_result_at,
                MAX(created_at) AS last_result_at
            FROM {} FORCE INDEX (idx_entry_exit_template_results_run)
            WHERE run_id = ?
            "#,
            quoted_identifier(&result_table_name)
        ))
        .bind(&run_id)
        .fetch_one(&pool)
        .await?;

        let summary = sqlx::query(
            r#"
            SELECT
                scanned_patterns,
                templates_created,
                result_rows,
                elapsed_ms,
                created_at
            FROM entry_exit_template_creator_runs
            WHERE run_id = ?
            LIMIT 1
            "#,
        )
        .bind(&run_id)
        .fetch_optional(&pool)
        .await?;

        println!("run_id={run_id}");
        println!(
            "templates={template_count} first_template_at={} last_template_at={}",
            first_template_at.unwrap_or_else(|| "n/a".to_string()),
            last_template_at.unwrap_or_else(|| "n/a".to_string())
        );
        println!(
            "result_rows={} result_templates={} max_event_order={} first_result_at={} last_result_at={}",
            result.try_get::<i64, _>("result_rows").unwrap_or(0),
            result.try_get::<i64, _>("result_templates").unwrap_or(0),
            result.try_get::<Option<i64>, _>("max_evaluation_order").ok().flatten().unwrap_or(0),
            result
                .try_get::<Option<String>, _>("first_result_at")
                .ok()
                .flatten()
                .unwrap_or_else(|| "n/a".to_string()),
            result
                .try_get::<Option<String>, _>("last_result_at")
                .ok()
                .flatten()
                .unwrap_or_else(|| "n/a".to_string())
        );

        if let Some(summary) = summary {
            println!(
                "summary=scanned {} templates {} results {} elapsed {:.1}s stored_at={}",
                summary.try_get::<i64, _>("scanned_patterns").unwrap_or(0),
                summary.try_get::<i64, _>("templates_created").unwrap_or(0),
                summary.try_get::<i64, _>("result_rows").unwrap_or(0),
                summary.try_get::<i64, _>("elapsed_ms").unwrap_or(0) as f64 / 1000.0,
                summary
                    .try_get::<Option<String>, _>("created_at")
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "n/a".to_string())
            );
        } else {
            println!("summary=in progress or not written yet");
        }
        println!();
    }

    Ok(())
}
