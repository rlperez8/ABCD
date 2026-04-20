use std::env;

use sqlx::mysql::MySqlPool;
use sqlx::Row;

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| "Missing ABCD_DATABASE_URL or DATABASE_URL".into())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url = database_url_from_env()?;
    let pool = MySqlPool::connect(&database_url).await?;

    let rows = sqlx::query(
        r#"
        SELECT
            run_id,
            symbol,
            phase,
            row_count,
            duration_ms,
            ROUND(duration_ms / 1000, 2) AS seconds,
            note,
            DATE_FORMAT(created_at, '%Y-%m-%d %H:%i:%s') AS created_at
        FROM engine_phase_timings
        WHERE run_id = (
            SELECT run_id
            FROM engine_phase_timings
            ORDER BY created_at DESC
            LIMIT 1
        )
        ORDER BY id
        "#,
    )
    .fetch_all(&pool)
    .await?;

    for row in rows {
        let run_id: String = row.try_get("run_id")?;
        let symbol: Option<String> = row.try_get("symbol")?;
        let phase: String = row.try_get("phase")?;
        let row_count: Option<i64> = row.try_get("row_count")?;
        let duration_ms: i64 = row.try_get("duration_ms")?;
        let seconds: Option<f64> = row.try_get("seconds").ok();
        let note: Option<String> = row.try_get("note")?;
        let created_at: String = row.try_get("created_at")?;

        println!(
            "{}\t{}\t{}\t{}\t{}\t{:.2}\t{}\t{}",
            run_id,
            symbol.unwrap_or_default(),
            phase,
            row_count.map(|count| count.to_string()).unwrap_or_default(),
            duration_ms,
            seconds.unwrap_or(duration_ms as f64 / 1000.0),
            note.unwrap_or_default(),
            created_at
        );
    }

    Ok(())
}
