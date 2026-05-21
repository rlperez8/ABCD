use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::MySqlConnection;
use sqlx::Row;
use std::env;

const ENTRY_EXIT_TEMPLATE_TABLES: [&str; 8] = [
    "entry_exit_template_condition_stats",
    "entry_exit_template_ui_stats",
    "entry_exit_template_build_coverage_rows_ui",
    "entry_exit_template_build_coverage_ui",
    "entry_exit_template_build_summary_ui",
    "entry_exit_template_results",
    "entry_exit_templates",
    "entry_exit_template_creator_runs",
];

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
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

async fn clear_table_with_retry(
    conn: &mut MySqlConnection,
    table: &str,
) -> Result<(), sqlx::Error> {
    let sql = format!("TRUNCATE TABLE {table}");
    sqlx::query(&sql).execute(&mut *conn).await?;
    println!("truncated {table}");
    Ok(())
}

async fn dynamic_result_tables(pool: &MySqlPool) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT TABLE_NAME
        FROM INFORMATION_SCHEMA.TABLES
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_NAME LIKE 'entry_exit_template_results_eetc\\_%' ESCAPE '\\'
        ORDER BY TABLE_NAME
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| row.try_get::<String, _>("TABLE_NAME").ok())
        .collect())
}

async fn drop_dynamic_result_table(
    conn: &mut MySqlConnection,
    table: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(&format!("DROP TABLE {}", quoted_identifier(table)))
        .execute(&mut *conn)
        .await?;
    println!("dropped {table}");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(&database_url_from_env()?)
        .await?;

    let mut existing_tables = Vec::new();
    println!("Clearing Entry/Exit build tables without pre-counting:");
    for table in ENTRY_EXIT_TEMPLATE_TABLES {
        if table_exists(&pool, table).await? {
            println!("found {table}");
            existing_tables.push(table);
        } else {
            println!("missing {table}");
        }
    }
    let dynamic_tables = dynamic_result_tables(&pool).await?;

    let mut conn = pool.acquire().await?;
    sqlx::query("SET FOREIGN_KEY_CHECKS = 0")
        .execute(&mut *conn)
        .await?;

    for table in dynamic_tables {
        drop_dynamic_result_table(&mut conn, &table).await?;
    }

    let mut clear_result = Ok(());
    for table in &existing_tables {
        if let Err(error) = clear_table_with_retry(&mut conn, table).await {
            clear_result = Err(error);
            break;
        }
    }

    sqlx::query("SET FOREIGN_KEY_CHECKS = 1")
        .execute(&mut *conn)
        .await?;

    drop(conn);
    clear_result?;

    println!("Cleared tables:");
    for table in existing_tables {
        println!("{table}");
    }

    Ok(())
}
