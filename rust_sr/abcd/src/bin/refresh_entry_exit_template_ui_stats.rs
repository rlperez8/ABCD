use std::{collections::HashMap, env};

use chrono::{Datelike, NaiveDateTime, Timelike};
use futures_util::TryStreamExt;
use sqlx::{MySql, MySqlPool, QueryBuilder, Row};

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

async fn ensure_entry_exit_template_condition_stats_table(
    pool: &MySqlPool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_template_condition_stats (
            run_id VARCHAR(64) NOT NULL,
            template_uid VARCHAR(128) NOT NULL,
            condition_type VARCHAR(64) NOT NULL,
            condition_value VARCHAR(128) NOT NULL,
            eval_count BIGINT NOT NULL DEFAULT 0,
            pass_count BIGINT NOT NULL DEFAULT 0,
            fail_count BIGINT NOT NULL DEFAULT 0,
            no_entry_count BIGINT NOT NULL DEFAULT 0,
            avg_r DOUBLE NOT NULL DEFAULT 0,
            sum_r DOUBLE NOT NULL DEFAULT 0,
            best_r DOUBLE NOT NULL DEFAULT 0,
            worst_r DOUBLE NOT NULL DEFAULT 0,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (run_id, template_uid, condition_type, condition_value),
            INDEX idx_entry_exit_condition_lookup (run_id, template_uid, condition_type, pass_count, avg_r),
            INDEX idx_entry_exit_condition_value (run_id, condition_type, condition_value, pass_count)
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn ensure_entry_exit_template_result_indexes(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    ensure_entry_exit_template_result_index(
        pool,
        "idx_entry_exit_template_results_run_template_stats",
        "run_id, template_uid, outcome, result_r",
        "full-run Entry/Exit template stats",
    )
    .await?;
    ensure_entry_exit_template_result_index(
        pool,
        "idx_entry_exit_template_results_run_setup",
        "run_id, setup_id, template_uid",
        "stored Entry/Exit condition breakdowns",
    )
    .await?;

    Ok(())
}

async fn ensure_entry_exit_template_result_index(
    pool: &MySqlPool,
    index_name: &str,
    columns: &str,
    reason: &str,
) -> Result<(), sqlx::Error> {
    let exists: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM information_schema.statistics
        WHERE table_schema = DATABASE()
          AND table_name = 'entry_exit_template_results'
          AND index_name = ?
        "#,
    )
    .bind(index_name)
    .fetch_one(pool)
    .await?;

    if exists == 0 {
        println!("Adding index {index_name} for {reason}...");
        sqlx::query(&format!(
            "ALTER TABLE entry_exit_template_results ADD INDEX {index_name} ({columns})"
        ))
        .execute(pool)
        .await?;
    }

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

#[derive(Clone, Debug)]
struct ConditionAggregate {
    eval_count: i64,
    pass_count: i64,
    fail_count: i64,
    no_entry_count: i64,
    sum_r: f64,
    best_r: f64,
    worst_r: f64,
}

impl Default for ConditionAggregate {
    fn default() -> Self {
        Self {
            eval_count: 0,
            pass_count: 0,
            fail_count: 0,
            no_entry_count: 0,
            sum_r: 0.0,
            best_r: 0.0,
            worst_r: 0.0,
        }
    }
}

impl ConditionAggregate {
    fn record(&mut self, outcome: &str, result_r: Option<f64>) {
        let result_r = result_r.unwrap_or(0.0);
        if self.eval_count == 0 {
            self.best_r = result_r;
            self.worst_r = result_r;
        } else {
            self.best_r = self.best_r.max(result_r);
            self.worst_r = self.worst_r.min(result_r);
        }

        self.eval_count += 1;
        self.sum_r += result_r;
        match outcome {
            "pass" => self.pass_count += 1,
            "fail" => self.fail_count += 1,
            "no_entry" => self.no_entry_count += 1,
            _ => {}
        }
    }

    fn avg_r(&self) -> f64 {
        if self.eval_count > 0 {
            self.sum_r / self.eval_count as f64
        } else {
            0.0
        }
    }
}

fn clean_condition_value(value: Option<String>) -> String {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn first_clean_condition_value(primary: Option<String>, fallback: Option<String>) -> String {
    let primary = clean_condition_value(primary);
    if primary == "Unknown" {
        clean_condition_value(fallback)
    } else {
        primary
    }
}

fn trend_condition_value(value: Option<i64>) -> String {
    match value {
        Some(value) if value != 0 => "Bullish".to_string(),
        Some(_) => "Bearish".to_string(),
        None => "Unknown".to_string(),
    }
}

fn confirm_year_value(value: Option<NaiveDateTime>) -> String {
    value
        .map(|value| value.year().to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn confirm_quarter_value(value: Option<NaiveDateTime>) -> String {
    value
        .map(|value| format!("{}-Q{}", value.year(), ((value.month() - 1) / 3) + 1))
        .unwrap_or_else(|| "Unknown".to_string())
}

fn confirm_session_value(value: Option<NaiveDateTime>) -> String {
    let Some(value) = value else {
        return "Unknown".to_string();
    };

    match value.hour() {
        0..=5 => "Overnight",
        6..=8 => "Pre-Market",
        9..=15 => "Regular",
        16..=20 => "After-Hours",
        _ => "Late",
    }
    .to_string()
}

fn pattern_length_bucket(value: Option<i64>) -> String {
    match value {
        Some(value) if value <= 20 => "0-20 bars",
        Some(value) if value <= 50 => "21-50 bars",
        Some(value) if value <= 100 => "51-100 bars",
        Some(value) if value <= 200 => "101-200 bars",
        Some(_) => "200+ bars",
        None => "Unknown",
    }
    .to_string()
}

fn record_condition(
    aggregates: &mut HashMap<(String, String, String), ConditionAggregate>,
    template_uid: &str,
    condition_type: &str,
    condition_value: String,
    outcome: &str,
    result_r: Option<f64>,
) {
    aggregates
        .entry((
            template_uid.to_string(),
            condition_type.to_string(),
            condition_value,
        ))
        .or_default()
        .record(outcome, result_r);
}

async fn refresh_entry_exit_template_condition_stats(
    pool: &MySqlPool,
    run_id: &str,
) -> Result<u64, sqlx::Error> {
    ensure_entry_exit_template_condition_stats_table(pool).await?;

    sqlx::query("DELETE FROM entry_exit_template_condition_stats WHERE run_id = ?")
        .bind(run_id)
        .execute(pool)
        .await?;

    let mut aggregates: HashMap<(String, String, String), ConditionAggregate> = HashMap::new();
    let mut rows = sqlx::query(
        r#"
        SELECT
            r.template_uid,
            r.outcome,
            r.result_r,
            r.market,
            r.symbol,
            r.trade_direction,
            r.exit_reason,
            r.d_confirm_date,
            ps.root_symbol,
            ps.source_timeframe,
            ps.pattern_family_harmonic_type,
            ps.harmonic_type,
            ps.pattern_family_bin,
            ps.pattern_family_size_bucket,
            ps.pattern_family_time_bin,
            ps.pattern_family_x_strictness,
            CAST(ps.three_month AS SIGNED) AS three_month,
            CAST(ps.six_month AS SIGNED) AS six_month,
            CAST(ps.twelve_month AS SIGNED) AS twelve_month,
            ps.full_pattern_length
        FROM entry_exit_template_results r FORCE INDEX (idx_entry_exit_template_results_run_setup)
        LEFT JOIN pattern_setups ps
          ON ps.setup_id = r.setup_id
        WHERE r.run_id = ?
        "#,
    )
    .bind(run_id)
    .fetch(pool);

    while let Some(row) = rows.try_next().await? {
        let template_uid: String = row.try_get("template_uid")?;
        let outcome: String = row.try_get("outcome")?;
        let result_r: Option<f64> = row.try_get("result_r")?;
        let symbol: Option<String> = row.try_get("symbol").ok();
        let d_confirm_date: Option<NaiveDateTime> = row.try_get("d_confirm_date").ok();

        record_condition(
            &mut aggregates,
            &template_uid,
            "market",
            clean_condition_value(row.try_get("market").ok()),
            &outcome,
            result_r,
        );
        record_condition(
            &mut aggregates,
            &template_uid,
            "symbol",
            clean_condition_value(symbol.clone()),
            &outcome,
            result_r,
        );
        record_condition(
            &mut aggregates,
            &template_uid,
            "root_symbol",
            first_clean_condition_value(row.try_get("root_symbol").ok(), symbol),
            &outcome,
            result_r,
        );
        record_condition(
            &mut aggregates,
            &template_uid,
            "source_timeframe",
            clean_condition_value(row.try_get("source_timeframe").ok()),
            &outcome,
            result_r,
        );
        record_condition(
            &mut aggregates,
            &template_uid,
            "harmonic_type",
            first_clean_condition_value(
                row.try_get("pattern_family_harmonic_type").ok(),
                row.try_get("harmonic_type").ok(),
            ),
            &outcome,
            result_r,
        );
        record_condition(
            &mut aggregates,
            &template_uid,
            "family_bin",
            clean_condition_value(row.try_get("pattern_family_bin").ok()),
            &outcome,
            result_r,
        );
        record_condition(
            &mut aggregates,
            &template_uid,
            "family_size_bucket",
            clean_condition_value(row.try_get("pattern_family_size_bucket").ok()),
            &outcome,
            result_r,
        );
        record_condition(
            &mut aggregates,
            &template_uid,
            "family_time_bin",
            clean_condition_value(row.try_get("pattern_family_time_bin").ok()),
            &outcome,
            result_r,
        );
        record_condition(
            &mut aggregates,
            &template_uid,
            "family_x_strictness",
            clean_condition_value(row.try_get("pattern_family_x_strictness").ok()),
            &outcome,
            result_r,
        );
        record_condition(
            &mut aggregates,
            &template_uid,
            "trend_3m",
            trend_condition_value(row.try_get("three_month").ok()),
            &outcome,
            result_r,
        );
        record_condition(
            &mut aggregates,
            &template_uid,
            "trend_6m",
            trend_condition_value(row.try_get("six_month").ok()),
            &outcome,
            result_r,
        );
        record_condition(
            &mut aggregates,
            &template_uid,
            "trend_12m",
            trend_condition_value(row.try_get("twelve_month").ok()),
            &outcome,
            result_r,
        );
        record_condition(
            &mut aggregates,
            &template_uid,
            "trade_direction",
            clean_condition_value(row.try_get("trade_direction").ok()),
            &outcome,
            result_r,
        );
        record_condition(
            &mut aggregates,
            &template_uid,
            "exit_reason",
            clean_condition_value(row.try_get("exit_reason").ok()),
            &outcome,
            result_r,
        );
        record_condition(
            &mut aggregates,
            &template_uid,
            "confirm_year",
            confirm_year_value(d_confirm_date),
            &outcome,
            result_r,
        );
        record_condition(
            &mut aggregates,
            &template_uid,
            "confirm_quarter",
            confirm_quarter_value(d_confirm_date),
            &outcome,
            result_r,
        );
        record_condition(
            &mut aggregates,
            &template_uid,
            "confirm_session",
            confirm_session_value(d_confirm_date),
            &outcome,
            result_r,
        );
        record_condition(
            &mut aggregates,
            &template_uid,
            "pattern_length_bucket",
            pattern_length_bucket(row.try_get("full_pattern_length").ok()),
            &outcome,
            result_r,
        );
    }

    let mut condition_rows = aggregates.into_iter().collect::<Vec<_>>();
    condition_rows.sort_by(|left, right| left.0.cmp(&right.0));

    let mut total_rows = 0;
    for chunk in condition_rows.chunks(500) {
        let mut query_builder = QueryBuilder::<MySql>::new(
            r#"
            INSERT INTO entry_exit_template_condition_stats (
                run_id,
                template_uid,
                condition_type,
                condition_value,
                eval_count,
                pass_count,
                fail_count,
                no_entry_count,
                avg_r,
                sum_r,
                best_r,
                worst_r
            )
            "#,
        );

        query_builder.push_values(
            chunk,
            |mut row_builder, ((template_uid, condition_type, condition_value), aggregate)| {
                row_builder
                    .push_bind(run_id)
                    .push_bind(template_uid)
                    .push_bind(condition_type)
                    .push_bind(condition_value)
                    .push_bind(aggregate.eval_count)
                    .push_bind(aggregate.pass_count)
                    .push_bind(aggregate.fail_count)
                    .push_bind(aggregate.no_entry_count)
                    .push_bind(aggregate.avg_r())
                    .push_bind(aggregate.sum_r)
                    .push_bind(aggregate.best_r)
                    .push_bind(aggregate.worst_r);
            },
        );

        total_rows += query_builder.build().execute(pool).await?.rows_affected();
    }

    Ok(total_rows)
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
    ensure_entry_exit_template_result_indexes(&pool).await?;
    let rows = refresh_entry_exit_template_ui_stats(&pool, &run_id).await?;
    println!("Refreshed {rows} template UI stat row(s).");
    let condition_rows = refresh_entry_exit_template_condition_stats(&pool, &run_id).await?;
    println!("Refreshed {condition_rows} template condition stat row(s).");

    Ok(())
}
