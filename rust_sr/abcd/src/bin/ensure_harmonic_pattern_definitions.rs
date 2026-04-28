use std::env;

use abcd::models::database::Database;
use sqlx::mysql::MySqlPool;

#[derive(sqlx::FromRow)]
struct HarmonicPatternDefinitionRow {
    harmonic_type: String,
    display_name: String,
    scanner_family: String,
    is_enabled: bool,
}

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
    let db = Database { pool };

    db.ensure_harmonic_pattern_definitions_table().await?;

    let rows = sqlx::query_as::<_, HarmonicPatternDefinitionRow>(
        r#"
        SELECT harmonic_type, display_name, scanner_family, is_enabled
        FROM harmonic_pattern_definitions
        ORDER BY
            CASE scanner_family
                WHEN 'xabcd' THEN 1
                WHEN 'abcd' THEN 2
                WHEN 'cypher' THEN 3
                WHEN 'five_zero' THEN 4
                WHEN 'three_drives' THEN 5
                ELSE 99
            END,
            is_enabled DESC,
            display_name
        "#,
    )
    .fetch_all(&db.pool)
    .await?;

    println!("Ensured harmonic_pattern_definitions:");
    for row in rows {
        let status = if row.is_enabled {
            "enabled"
        } else {
            "disabled"
        };
        println!(
            "  {:<16} {:<18} {:<12} {}",
            row.harmonic_type, row.display_name, row.scanner_family, status
        );
    }

    Ok(())
}
