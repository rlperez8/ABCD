use std::env;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::Row;

#[derive(Debug)]
struct Args {
    source_xa_run_id: Option<String>,
    decision_threshold: f64,
    model_type: String,
    feature_set_version: String,
}

fn usage() -> &'static str {
    "Usage: cargo run --bin build_pattern_reversal_ai_scores -- [--xa-run-id RUN_ID] [--decision-threshold 0.60]\nCreates stored pattern reversal AI score rows from the XA outcome layer. Phase 1 uses a family-reversal-rate baseline model."
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|items| items[0] == name)
        .map(|items| items[1].trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let raw_args = env::args().skip(1).collect::<Vec<_>>();
    if raw_args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", usage());
        std::process::exit(0);
    }

    let decision_threshold = arg_value(&raw_args, "--decision-threshold")
        .or_else(|| arg_value(&raw_args, "--threshold"))
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite())
        .unwrap_or(0.60)
        .clamp(0.01, 0.99);

    Ok(Args {
        source_xa_run_id: arg_value(&raw_args, "--xa-run-id")
            .or_else(|| arg_value(&raw_args, "--source-xa-run-id")),
        decision_threshold,
        model_type: "family_reversal_rate_baseline".to_string(),
        feature_set_version: "phase1_family_features_v1".to_string(),
    })
}

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

fn new_ai_run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let pid = std::process::id();
    format!("prai-{millis}-{pid}")
}

async fn ensure_tables(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS pattern_reversal_ai_runs (
            ai_run_id VARCHAR(64) NOT NULL PRIMARY KEY,
            source_xa_run_id VARCHAR(64) NOT NULL,
            model_type VARCHAR(64) NOT NULL,
            feature_set_version VARCHAR(64) NOT NULL,
            decision_threshold DOUBLE NOT NULL,
            score_rows BIGINT NOT NULL DEFAULT 0,
            predicted_reversal_count BIGINT NOT NULL DEFAULT 0,
            predicted_reversal_actual_reversal_count BIGINT NOT NULL DEFAULT 0,
            baseline_reversal_rate DOUBLE NOT NULL DEFAULT 0,
            predicted_reversal_actual_rate DOUBLE NOT NULL DEFAULT 0,
            lift_vs_baseline DOUBLE NOT NULL DEFAULT 0,
            elapsed_ms BIGINT NOT NULL DEFAULT 0,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            INDEX idx_pattern_reversal_ai_runs_created (created_at),
            INDEX idx_pattern_reversal_ai_runs_source (source_xa_run_id, created_at)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS pattern_reversal_ai_scores (
            ai_run_id VARCHAR(64) NOT NULL,
            source_xa_run_id VARCHAR(64) NOT NULL,
            setup_id VARCHAR(64) NOT NULL,
            pattern_id VARCHAR(128) NULL,
            pattern_group_id VARCHAR(128) NOT NULL,
            symbol VARCHAR(32) NOT NULL,
            root_symbol VARCHAR(32) NULL,
            market VARCHAR(16) NOT NULL,
            pattern_family_key VARCHAR(64) NULL,
            d_confirm_date DATETIME NOT NULL,
            actual_outcome VARCHAR(32) NOT NULL,
            actual_reversed TINYINT(1) NOT NULL DEFAULT 0,
            predicted_reversal_probability DOUBLE NOT NULL DEFAULT 0,
            predicted_continuation_probability DOUBLE NOT NULL DEFAULT 0,
            ai_decision VARCHAR(32) NOT NULL,
            was_correct TINYINT(1) NOT NULL DEFAULT 0,
            confidence_bucket VARCHAR(16) NOT NULL,
            harmonic_type VARCHAR(32) NOT NULL DEFAULT 'Unknown',
            family_bin VARCHAR(32) NOT NULL DEFAULT 'Unknown',
            family_size_bucket VARCHAR(32) NOT NULL DEFAULT 'Unknown',
            family_time_bin VARCHAR(32) NOT NULL DEFAULT 'Unknown',
            family_x_strictness VARCHAR(32) NOT NULL DEFAULT 'Unknown',
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (ai_run_id, setup_id),
            INDEX idx_pattern_reversal_ai_scores_decision (ai_run_id, ai_decision, actual_reversed),
            INDEX idx_pattern_reversal_ai_scores_family (ai_run_id, pattern_family_key, predicted_reversal_probability),
            INDEX idx_pattern_reversal_ai_scores_symbol (ai_run_id, root_symbol, predicted_reversal_probability),
            INDEX idx_pattern_reversal_ai_scores_actual (ai_run_id, actual_outcome)
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn latest_xa_run_id(pool: &MySqlPool) -> Result<String, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT run_id
        FROM pattern_xa_outcome_runs
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .fetch_one(pool)
    .await
}

async fn insert_run_start(
    pool: &MySqlPool,
    ai_run_id: &str,
    source_xa_run_id: &str,
    args: &Args,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO pattern_reversal_ai_runs (
            ai_run_id,
            source_xa_run_id,
            model_type,
            feature_set_version,
            decision_threshold
        ) VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(ai_run_id)
    .bind(source_xa_run_id)
    .bind(&args.model_type)
    .bind(&args.feature_set_version)
    .bind(args.decision_threshold)
    .execute(pool)
    .await?;

    Ok(())
}

async fn insert_scores(
    pool: &MySqlPool,
    ai_run_id: &str,
    source_xa_run_id: &str,
    decision_threshold: f64,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        INSERT INTO pattern_reversal_ai_scores (
            ai_run_id,
            source_xa_run_id,
            setup_id,
            pattern_id,
            pattern_group_id,
            symbol,
            root_symbol,
            market,
            pattern_family_key,
            d_confirm_date,
            actual_outcome,
            actual_reversed,
            predicted_reversal_probability,
            predicted_continuation_probability,
            ai_decision,
            was_correct,
            confidence_bucket,
            harmonic_type,
            family_bin,
            family_size_bucket,
            family_time_bin,
            family_x_strictness
        )
        SELECT
            ? AS ai_run_id,
            o.run_id AS source_xa_run_id,
            o.setup_id,
            o.pattern_id,
            o.pattern_group_id,
            o.symbol,
            o.root_symbol,
            o.market,
            o.pattern_family_key,
            o.d_confirm_date,
            o.outcome AS actual_outcome,
            CASE WHEN o.outcome = 'reversal_xa' THEN 1 ELSE 0 END AS actual_reversed,
            COALESCE(f.reversal_rate, 0) AS predicted_reversal_probability,
            COALESCE(f.continuation_rate, 0) AS predicted_continuation_probability,
            CASE
                WHEN COALESCE(f.reversal_rate, 0) >= ? THEN 'TAKE_REVERSAL'
                ELSE 'SKIP_REVERSAL'
            END AS ai_decision,
            CASE
                WHEN COALESCE(f.reversal_rate, 0) >= ? AND o.outcome = 'reversal_xa' THEN 1
                WHEN COALESCE(f.reversal_rate, 0) < ? AND o.outcome <> 'reversal_xa' THEN 1
                ELSE 0
            END AS was_correct,
            CASE
                WHEN COALESCE(f.reversal_rate, 0) >= 0.70 THEN '70%+'
                WHEN COALESCE(f.reversal_rate, 0) >= 0.65 THEN '65-70%'
                WHEN COALESCE(f.reversal_rate, 0) >= 0.60 THEN '60-65%'
                WHEN COALESCE(f.reversal_rate, 0) >= 0.55 THEN '55-60%'
                WHEN COALESCE(f.reversal_rate, 0) >= 0.50 THEN '50-55%'
                ELSE '<50%'
            END AS confidence_bucket,
            COALESCE(f.harmonic_type, 'Unknown') AS harmonic_type,
            COALESCE(f.family_bin, 'Unknown') AS family_bin,
            COALESCE(f.family_size_bucket, 'Unknown') AS family_size_bucket,
            COALESCE(f.family_time_bin, 'Unknown') AS family_time_bin,
            COALESCE(f.family_x_strictness, 'Unknown') AS family_x_strictness
        FROM pattern_xa_outcomes o
        LEFT JOIN pattern_xa_outcome_families f
          ON f.run_id = o.run_id
         AND f.pattern_family_key = COALESCE(o.pattern_family_key, 'Unknown')
        WHERE o.run_id = ?
        "#,
    )
    .bind(ai_run_id)
    .bind(decision_threshold)
    .bind(decision_threshold)
    .bind(decision_threshold)
    .bind(source_xa_run_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

async fn update_run_finish(
    pool: &MySqlPool,
    ai_run_id: &str,
    elapsed_ms: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE pattern_reversal_ai_runs runs
        JOIN (
            SELECT
                ai_run_id,
                COUNT(*) AS score_rows,
                SUM(CASE WHEN ai_decision = 'TAKE_REVERSAL' THEN 1 ELSE 0 END) AS predicted_reversal_count,
                SUM(CASE WHEN ai_decision = 'TAKE_REVERSAL' AND actual_reversed = 1 THEN 1 ELSE 0 END) AS predicted_reversal_actual_reversal_count,
                AVG(actual_reversed) AS baseline_reversal_rate,
                CASE
                    WHEN SUM(CASE WHEN ai_decision = 'TAKE_REVERSAL' THEN 1 ELSE 0 END) > 0
                    THEN SUM(CASE WHEN ai_decision = 'TAKE_REVERSAL' AND actual_reversed = 1 THEN 1 ELSE 0 END)
                         / SUM(CASE WHEN ai_decision = 'TAKE_REVERSAL' THEN 1 ELSE 0 END)
                    ELSE 0
                END AS predicted_reversal_actual_rate
            FROM pattern_reversal_ai_scores
            WHERE ai_run_id = ?
            GROUP BY ai_run_id
        ) stats
          ON stats.ai_run_id = runs.ai_run_id
        SET
            runs.score_rows = stats.score_rows,
            runs.predicted_reversal_count = stats.predicted_reversal_count,
            runs.predicted_reversal_actual_reversal_count = stats.predicted_reversal_actual_reversal_count,
            runs.baseline_reversal_rate = stats.baseline_reversal_rate,
            runs.predicted_reversal_actual_rate = stats.predicted_reversal_actual_rate,
            runs.lift_vs_baseline = stats.predicted_reversal_actual_rate - stats.baseline_reversal_rate,
            runs.elapsed_ms = ?
        WHERE runs.ai_run_id = ?
        "#,
    )
    .bind(ai_run_id)
    .bind(elapsed_ms)
    .bind(ai_run_id)
    .execute(pool)
    .await?;

    Ok(())
}

async fn print_summary(pool: &MySqlPool, ai_run_id: &str) -> Result<(), sqlx::Error> {
    let run = sqlx::query(
        r#"
        SELECT
            ai_run_id,
            source_xa_run_id,
            score_rows,
            predicted_reversal_count,
            predicted_reversal_actual_reversal_count,
            baseline_reversal_rate,
            predicted_reversal_actual_rate,
            lift_vs_baseline,
            elapsed_ms
        FROM pattern_reversal_ai_runs
        WHERE ai_run_id = ?
        "#,
    )
    .bind(ai_run_id)
    .fetch_one(pool)
    .await?;

    println!("\nPattern reversal AI score run complete");
    println!("ai_run_id={}", run.try_get::<String, _>("ai_run_id")?);
    println!(
        "source_xa_run_id={}",
        run.try_get::<String, _>("source_xa_run_id")?
    );
    println!("score_rows={}", run.try_get::<i64, _>("score_rows")?);
    println!(
        "called_reversal={} actual_reversal={} actual_rate={:.2}% baseline={:.2}% lift={:.2}pp elapsed={:.2}s",
        run.try_get::<i64, _>("predicted_reversal_count")?,
        run.try_get::<i64, _>("predicted_reversal_actual_reversal_count")?,
        run.try_get::<f64, _>("predicted_reversal_actual_rate")? * 100.0,
        run.try_get::<f64, _>("baseline_reversal_rate")? * 100.0,
        run.try_get::<f64, _>("lift_vs_baseline")? * 100.0,
        run.try_get::<i64, _>("elapsed_ms")? as f64 / 1000.0,
    );

    let buckets = sqlx::query(
        r#"
        SELECT
            confidence_bucket,
            COUNT(*) AS rows_count,
            CAST(AVG(actual_reversed) AS DOUBLE) AS actual_reversal_rate,
            CAST(AVG(predicted_reversal_probability) AS DOUBLE) AS avg_predicted_reversal_probability
        FROM pattern_reversal_ai_scores
        WHERE ai_run_id = ?
        GROUP BY confidence_bucket
        ORDER BY MIN(predicted_reversal_probability) DESC
        "#,
    )
    .bind(ai_run_id)
    .fetch_all(pool)
    .await?;

    println!("bucket\trows\tactual_rev%\tavg_ai_rev%");
    for row in buckets {
        println!(
            "{}\t{}\t{:.2}\t{:.2}",
            row.try_get::<String, _>("confidence_bucket")?,
            row.try_get::<i64, _>("rows_count")?,
            row.try_get::<f64, _>("actual_reversal_rate")? * 100.0,
            row.try_get::<f64, _>("avg_predicted_reversal_probability")? * 100.0,
        );
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let args = parse_args()?;
    let pool = MySqlPoolOptions::new()
        .max_connections(8)
        .connect(&database_url_from_env()?)
        .await?;
    ensure_tables(&pool).await?;

    let source_xa_run_id = match args.source_xa_run_id.clone() {
        Some(run_id) => run_id,
        None => latest_xa_run_id(&pool).await?,
    };
    let ai_run_id = new_ai_run_id();
    let started = Instant::now();

    insert_run_start(&pool, &ai_run_id, &source_xa_run_id, &args).await?;
    let rows = insert_scores(
        &pool,
        &ai_run_id,
        &source_xa_run_id,
        args.decision_threshold,
    )
    .await?;
    update_run_finish(&pool, &ai_run_id, started.elapsed().as_millis() as i64).await?;

    println!(
        "Inserted {rows} pattern reversal AI score rows using threshold {:.2}",
        args.decision_threshold
    );
    print_summary(&pool, &ai_run_id).await?;

    Ok(())
}
