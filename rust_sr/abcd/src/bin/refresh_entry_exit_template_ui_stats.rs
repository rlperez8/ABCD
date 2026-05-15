use std::env;

use sqlx::{MySqlPool, Row};

fn usage() -> &'static str {
    "Usage: cargo run --bin refresh_entry_exit_template_ui_stats -- [--run-id RUN_ID]\nRebuilds the small Entry/Exit template UI stats table from raw template results."
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|items| items[0] == name)
        .map(|items| items[1].clone())
}

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

async fn latest_run_id(pool: &MySqlPool) -> Result<Option<String>, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT run_id
        FROM entry_exit_template_creator_runs
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| row.get("run_id")))
}

async fn ensure_entry_exit_template_ui_stats_table(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_template_ui_stats (
            run_id VARCHAR(64) NOT NULL,
            template_uid VARCHAR(128) NOT NULL,
            eval_count BIGINT NOT NULL DEFAULT 0,
            pass_count BIGINT NOT NULL DEFAULT 0,
            fail_count BIGINT NOT NULL DEFAULT 0,
            no_entry_count BIGINT NOT NULL DEFAULT 0,
            avg_r DOUBLE NOT NULL DEFAULT 0,
            sum_r DOUBLE NOT NULL DEFAULT 0,
            best_r DOUBLE NOT NULL DEFAULT 0,
            worst_r DOUBLE NOT NULL DEFAULT 0,
            bullish_eval_count BIGINT NOT NULL DEFAULT 0,
            bullish_pass_count BIGINT NOT NULL DEFAULT 0,
            bullish_fail_count BIGINT NOT NULL DEFAULT 0,
            bullish_no_entry_count BIGINT NOT NULL DEFAULT 0,
            bullish_win_rate DOUBLE NOT NULL DEFAULT 0,
            bullish_avg_r DOUBLE NOT NULL DEFAULT 0,
            bearish_eval_count BIGINT NOT NULL DEFAULT 0,
            bearish_pass_count BIGINT NOT NULL DEFAULT 0,
            bearish_fail_count BIGINT NOT NULL DEFAULT 0,
            bearish_no_entry_count BIGINT NOT NULL DEFAULT 0,
            bearish_win_rate DOUBLE NOT NULL DEFAULT 0,
            bearish_avg_r DOUBLE NOT NULL DEFAULT 0,
            market_edge_label VARCHAR(16) NOT NULL DEFAULT 'Flat',
            market_edge_score DOUBLE NOT NULL DEFAULT 0,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (run_id, template_uid),
            INDEX idx_entry_exit_template_ui_stats_run_rank (run_id, pass_count, avg_r, eval_count)
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn refresh_entry_exit_template_ui_stats(
    pool: &MySqlPool,
    run_id: &str,
) -> Result<u64, sqlx::Error> {
    ensure_entry_exit_template_ui_stats_table(pool).await?;

    sqlx::query("DELETE FROM entry_exit_template_ui_stats WHERE run_id = ?")
        .bind(run_id)
        .execute(pool)
        .await?;

    let result = sqlx::query(
        r#"
        INSERT INTO entry_exit_template_ui_stats (
            run_id,
            template_uid,
            eval_count,
            pass_count,
            fail_count,
            no_entry_count,
            avg_r,
            sum_r,
            best_r,
            worst_r,
            bullish_eval_count,
            bullish_pass_count,
            bullish_fail_count,
            bullish_no_entry_count,
            bullish_win_rate,
            bullish_avg_r,
            bearish_eval_count,
            bearish_pass_count,
            bearish_fail_count,
            bearish_no_entry_count,
            bearish_win_rate,
            bearish_avg_r,
            market_edge_label,
            market_edge_score
        )
        SELECT
            t.origin_run_id AS run_id,
            t.template_uid,
            COALESCE(r.eval_count, 0) AS eval_count,
            COALESCE(r.pass_count, 0) AS pass_count,
            COALESCE(r.fail_count, 0) AS fail_count,
            COALESCE(r.no_entry_count, 0) AS no_entry_count,
            COALESCE(r.avg_r, 0) AS avg_r,
            COALESCE(r.sum_r, 0) AS sum_r,
            COALESCE(r.best_r, 0) AS best_r,
            COALESCE(r.worst_r, 0) AS worst_r,
            COALESCE(r.bullish_eval_count, 0) AS bullish_eval_count,
            COALESCE(r.bullish_pass_count, 0) AS bullish_pass_count,
            COALESCE(r.bullish_fail_count, 0) AS bullish_fail_count,
            COALESCE(r.bullish_no_entry_count, 0) AS bullish_no_entry_count,
            COALESCE(r.bullish_win_rate, 0) AS bullish_win_rate,
            COALESCE(r.bullish_avg_r, 0) AS bullish_avg_r,
            COALESCE(r.bearish_eval_count, 0) AS bearish_eval_count,
            COALESCE(r.bearish_pass_count, 0) AS bearish_pass_count,
            COALESCE(r.bearish_fail_count, 0) AS bearish_fail_count,
            COALESCE(r.bearish_no_entry_count, 0) AS bearish_no_entry_count,
            COALESCE(r.bearish_win_rate, 0) AS bearish_win_rate,
            COALESCE(r.bearish_avg_r, 0) AS bearish_avg_r,
            CASE
                WHEN ABS(COALESCE(r.bullish_avg_r, 0) - COALESCE(r.bearish_avg_r, 0)) < 0.05
                  THEN 'Flat'
                WHEN COALESCE(r.bullish_avg_r, 0) > COALESCE(r.bearish_avg_r, 0)
                  THEN 'Bullish'
                ELSE 'Bearish'
            END AS market_edge_label,
            ABS(COALESCE(r.bullish_avg_r, 0) - COALESCE(r.bearish_avg_r, 0)) AS market_edge_score
        FROM entry_exit_templates t
        LEFT JOIN (
            SELECT
                template_uid,
                CAST(COUNT(*) AS SIGNED) AS eval_count,
                CAST(SUM(CASE WHEN outcome = 'pass' THEN 1 ELSE 0 END) AS SIGNED) AS pass_count,
                CAST(SUM(CASE WHEN outcome = 'fail' THEN 1 ELSE 0 END) AS SIGNED) AS fail_count,
                CAST(SUM(CASE WHEN outcome = 'no_entry' THEN 1 ELSE 0 END) AS SIGNED) AS no_entry_count,
                COALESCE(AVG(COALESCE(result_r, 0)), 0) AS avg_r,
                COALESCE(SUM(COALESCE(result_r, 0)), 0) AS sum_r,
                COALESCE(MAX(COALESCE(result_r, 0)), 0) AS best_r,
                COALESCE(MIN(COALESCE(result_r, 0)), 0) AS worst_r,
                CAST(SUM(CASE WHEN market = 'Bullish' THEN 1 ELSE 0 END) AS SIGNED) AS bullish_eval_count,
                CAST(SUM(CASE WHEN market = 'Bullish' AND outcome = 'pass' THEN 1 ELSE 0 END) AS SIGNED) AS bullish_pass_count,
                CAST(SUM(CASE WHEN market = 'Bullish' AND outcome = 'fail' THEN 1 ELSE 0 END) AS SIGNED) AS bullish_fail_count,
                CAST(SUM(CASE WHEN market = 'Bullish' AND outcome = 'no_entry' THEN 1 ELSE 0 END) AS SIGNED) AS bullish_no_entry_count,
                CASE
                    WHEN SUM(CASE WHEN market = 'Bullish' THEN 1 ELSE 0 END) > 0
                      THEN SUM(CASE WHEN market = 'Bullish' AND outcome = 'pass' THEN 1 ELSE 0 END)
                           / SUM(CASE WHEN market = 'Bullish' THEN 1 ELSE 0 END)
                    ELSE 0
                END AS bullish_win_rate,
                COALESCE(AVG(CASE WHEN market = 'Bullish' THEN COALESCE(result_r, 0) ELSE NULL END), 0) AS bullish_avg_r,
                CAST(SUM(CASE WHEN market = 'Bearish' THEN 1 ELSE 0 END) AS SIGNED) AS bearish_eval_count,
                CAST(SUM(CASE WHEN market = 'Bearish' AND outcome = 'pass' THEN 1 ELSE 0 END) AS SIGNED) AS bearish_pass_count,
                CAST(SUM(CASE WHEN market = 'Bearish' AND outcome = 'fail' THEN 1 ELSE 0 END) AS SIGNED) AS bearish_fail_count,
                CAST(SUM(CASE WHEN market = 'Bearish' AND outcome = 'no_entry' THEN 1 ELSE 0 END) AS SIGNED) AS bearish_no_entry_count,
                CASE
                    WHEN SUM(CASE WHEN market = 'Bearish' THEN 1 ELSE 0 END) > 0
                      THEN SUM(CASE WHEN market = 'Bearish' AND outcome = 'pass' THEN 1 ELSE 0 END)
                           / SUM(CASE WHEN market = 'Bearish' THEN 1 ELSE 0 END)
                    ELSE 0
                END AS bearish_win_rate,
                COALESCE(AVG(CASE WHEN market = 'Bearish' THEN COALESCE(result_r, 0) ELSE NULL END), 0) AS bearish_avg_r
            FROM entry_exit_template_results
            WHERE run_id = ?
            GROUP BY template_uid
        ) r ON r.template_uid = t.template_uid
        WHERE t.origin_run_id = ?
        "#,
    )
    .bind(run_id)
    .bind(run_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let raw_args = env::args().skip(1).collect::<Vec<_>>();
    if raw_args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", usage());
        return Ok(());
    }

    let pool = MySqlPool::connect(&database_url_from_env()?).await?;
    let run_id = match arg_value(&raw_args, "--run-id")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        Some(run_id) => run_id,
        None => latest_run_id(&pool)
            .await?
            .ok_or("No entry_exit_template_creator_runs rows found")?,
    };

    println!("Refreshing Entry/Exit template UI stats for run {run_id}...");
    let rows = refresh_entry_exit_template_ui_stats(&pool, &run_id).await?;
    println!("Refreshed {rows} template UI stat row(s).");

    Ok(())
}
