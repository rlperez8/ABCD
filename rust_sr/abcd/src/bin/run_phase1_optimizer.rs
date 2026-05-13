use std::collections::HashMap;
use std::env;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use chrono::{Datelike, NaiveDateTime};
use sqlx::{MySql, MySqlPool, QueryBuilder};

#[derive(Debug)]
struct Args {
    family_key: String,
    source_scope: String,
    period_year: i64,
    max_setups: i64,
    max_forward_bars: i64,
}

#[derive(Clone, sqlx::FromRow)]
struct PatternFamilySummary {
    family_key: String,
    harmonic_type: String,
    bin: String,
    size_bucket: String,
    time_bin: String,
    x_strictness: String,
    setup_count: i64,
}

#[derive(Clone, sqlx::FromRow)]
struct Phase1SetupSource {
    setup_id: String,
    symbol: String,
    source_table: Option<String>,
    source_timeframe: Option<String>,
    market: String,
    d_date: NaiveDateTime,
    x_high: f64,
    x_low: f64,
    b_high: f64,
    b_low: f64,
    c_high: f64,
    c_low: f64,
    d_high: f64,
    d_low: f64,
    d_close: f64,
    cd_price_length: f64,
    full_pattern_length: i64,
}

#[derive(Clone, sqlx::FromRow)]
struct Phase1ForwardCandle {
    open: f64,
    high: f64,
    low: f64,
    close: f64,
}

#[derive(Clone)]
struct Phase1RouteSpec {
    route_id: String,
    route_label: String,
    entry_mode: String,
    stop_mode: String,
    target_r: f64,
    max_hold_multiple: i64,
}

struct Phase1RouteAccumulator {
    spec: Phase1RouteSpec,
    setup_count: i64,
    trade_count: i64,
    no_entry_count: i64,
    win_count: i64,
    loss_count: i64,
    sum_r: f64,
    positive_r: f64,
    negative_r_abs: f64,
    cumulative_r: f64,
    peak_r: f64,
    max_drawdown_r: f64,
    yearly: HashMap<i32, Phase1YearlyAccumulator>,
}

#[derive(Clone, Default)]
struct Phase1YearlyAccumulator {
    setup_count: i64,
    trade_count: i64,
    no_entry_count: i64,
    win_count: i64,
    loss_count: i64,
    sum_r: f64,
    positive_r: f64,
    negative_r_abs: f64,
    cumulative_r: f64,
    peak_r: f64,
    max_drawdown_r: f64,
}

struct Phase1ResultRow {
    run_id: String,
    family_key: String,
    source_scope: String,
    period_year: i64,
    route_id: String,
    route_label: String,
    rank: i64,
    entry_mode: String,
    stop_mode: String,
    target_r: f64,
    max_hold_multiple: i64,
    setup_count: i64,
    trade_count: i64,
    no_entry_count: i64,
    win_count: i64,
    loss_count: i64,
    win_rate: f64,
    avg_r: f64,
    profit_factor: f64,
    max_drawdown_r: f64,
    worst_year_avg_r: f64,
    score: f64,
}

struct Phase1YearlyResultRow {
    run_id: String,
    family_key: String,
    source_scope: String,
    period_year: i64,
    route_id: String,
    year: i32,
    setup_count: i64,
    trade_count: i64,
    no_entry_count: i64,
    win_count: i64,
    loss_count: i64,
    win_rate: f64,
    avg_r: f64,
    sum_r: f64,
    profit_factor: f64,
    max_drawdown_r: f64,
}

impl Phase1YearlyAccumulator {
    fn record_no_entry(&mut self) {
        self.setup_count += 1;
        self.no_entry_count += 1;
    }

    fn record_trade(&mut self, result_r: f64) {
        self.setup_count += 1;
        self.trade_count += 1;
        self.sum_r += result_r;

        if result_r > 0.0 {
            self.win_count += 1;
            self.positive_r += result_r;
        } else {
            self.loss_count += 1;
            self.negative_r_abs += result_r.abs();
        }

        self.cumulative_r += result_r;
        self.peak_r = self.peak_r.max(self.cumulative_r);
        self.max_drawdown_r = self.max_drawdown_r.max(self.peak_r - self.cumulative_r);
    }

    fn avg_r(&self) -> f64 {
        if self.trade_count > 0 {
            self.sum_r / self.trade_count as f64
        } else {
            0.0
        }
    }

    fn into_row(
        self,
        run_id: &str,
        family_key: &str,
        source_scope: &str,
        period_year: i64,
        route_id: &str,
        year: i32,
    ) -> Phase1YearlyResultRow {
        let win_rate = if self.trade_count > 0 {
            (self.win_count as f64 / self.trade_count as f64) * 100.0
        } else {
            0.0
        };
        let profit_factor = if self.negative_r_abs > 0.0 {
            self.positive_r / self.negative_r_abs
        } else if self.positive_r > 0.0 {
            999.0
        } else {
            0.0
        };

        Phase1YearlyResultRow {
            run_id: run_id.to_string(),
            family_key: family_key.to_string(),
            source_scope: source_scope.to_string(),
            period_year,
            route_id: route_id.to_string(),
            year,
            setup_count: self.setup_count,
            trade_count: self.trade_count,
            no_entry_count: self.no_entry_count,
            win_count: self.win_count,
            loss_count: self.loss_count,
            win_rate,
            avg_r: self.avg_r(),
            sum_r: self.sum_r,
            profit_factor,
            max_drawdown_r: self.max_drawdown_r,
        }
    }
}

impl Phase1RouteAccumulator {
    fn new(spec: Phase1RouteSpec) -> Self {
        Self {
            spec,
            setup_count: 0,
            trade_count: 0,
            no_entry_count: 0,
            win_count: 0,
            loss_count: 0,
            sum_r: 0.0,
            positive_r: 0.0,
            negative_r_abs: 0.0,
            cumulative_r: 0.0,
            peak_r: 0.0,
            max_drawdown_r: 0.0,
            yearly: HashMap::new(),
        }
    }

    fn record_no_entry(&mut self, year: i32) {
        self.setup_count += 1;
        self.no_entry_count += 1;
        self.yearly.entry(year).or_default().record_no_entry();
    }

    fn record_trade(&mut self, year: i32, result_r: f64) {
        self.setup_count += 1;
        self.trade_count += 1;
        self.sum_r += result_r;

        if result_r > 0.0 {
            self.win_count += 1;
            self.positive_r += result_r;
        } else {
            self.loss_count += 1;
            self.negative_r_abs += result_r.abs();
        }

        self.cumulative_r += result_r;
        self.peak_r = self.peak_r.max(self.cumulative_r);
        self.max_drawdown_r = self.max_drawdown_r.max(self.peak_r - self.cumulative_r);

        self.yearly.entry(year).or_default().record_trade(result_r);
    }

    fn into_rows(
        self,
        run_id: &str,
        family_key: &str,
        source_scope: &str,
        period_year: i64,
    ) -> (Phase1ResultRow, Vec<Phase1YearlyResultRow>) {
        let avg_r = if self.trade_count > 0 {
            self.sum_r / self.trade_count as f64
        } else {
            0.0
        };
        let win_rate = if self.trade_count > 0 {
            (self.win_count as f64 / self.trade_count as f64) * 100.0
        } else {
            0.0
        };
        let profit_factor = if self.negative_r_abs > 0.0 {
            self.positive_r / self.negative_r_abs
        } else if self.positive_r > 0.0 {
            999.0
        } else {
            0.0
        };
        let worst_year_avg_r = self
            .yearly
            .values()
            .filter(|yearly| yearly.trade_count > 0)
            .map(Phase1YearlyAccumulator::avg_r)
            .min_by(|left, right| left.total_cmp(right))
            .unwrap_or(0.0);
        let trade_count_boost = (self.trade_count.max(1) as f64).ln();
        let score = (avg_r * 100.0)
            + (profit_factor.min(5.0) * 8.0)
            + (win_rate * 0.12)
            + (worst_year_avg_r * 35.0)
            + trade_count_boost
            - (self.max_drawdown_r * 1.5);

        let route_id = self.spec.route_id;
        let yearly_rows = self
            .yearly
            .into_iter()
            .map(|(year, yearly)| {
                yearly.into_row(
                    run_id,
                    family_key,
                    source_scope,
                    period_year,
                    &route_id,
                    year,
                )
            })
            .collect::<Vec<_>>();

        let result = Phase1ResultRow {
            run_id: run_id.to_string(),
            family_key: family_key.to_string(),
            source_scope: source_scope.to_string(),
            period_year,
            route_id,
            route_label: self.spec.route_label,
            rank: 0,
            entry_mode: self.spec.entry_mode,
            stop_mode: self.spec.stop_mode,
            target_r: self.spec.target_r,
            max_hold_multiple: self.spec.max_hold_multiple,
            setup_count: self.setup_count,
            trade_count: self.trade_count,
            no_entry_count: self.no_entry_count,
            win_count: self.win_count,
            loss_count: self.loss_count,
            win_rate,
            avg_r,
            profit_factor,
            max_drawdown_r: self.max_drawdown_r,
            worst_year_avg_r,
            score,
        };

        (result, yearly_rows)
    }
}

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

fn usage() -> &'static str {
    "Usage: cargo run --bin run_phase1_optimizer -- --family <family_key> [--source futures|daily|all] [--year YYYY] [--max-setups N] [--max-forward-bars N]\nNote: forward bars are dynamic per setup: min(full_pattern_length * 5, --max-forward-bars safety cap)."
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|items| items[0] == name)
        .map(|items| items[1].clone())
}

fn normalize_source_scope(value: Option<String>) -> String {
    match value.as_deref().map(str::trim).map(str::to_ascii_lowercase) {
        Some(value) if value == "futures" || value == "futures_1m" => "futures".to_string(),
        Some(value) if value == "daily" || value == "candles_daily" => "daily".to_string(),
        _ => "all".to_string(),
    }
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let raw_args = env::args().skip(1).collect::<Vec<_>>();
    if raw_args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", usage());
        std::process::exit(0);
    }

    let family_key = arg_value(&raw_args, "--family")
        .or_else(|| arg_value(&raw_args, "--family-key"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Missing --family argument.\n{}", usage()))?;
    let source_scope = normalize_source_scope(arg_value(&raw_args, "--source"));
    let period_year = arg_value(&raw_args, "--year")
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|year| (1900..=2200).contains(year))
        .unwrap_or(0);
    let max_setups = arg_value(&raw_args, "--max-setups")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(1_000)
        .clamp(25, 25_000);
    let max_forward_bars = arg_value(&raw_args, "--max-forward-bars")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(420)
        .clamp(40, 5_000);

    Ok(Args {
        family_key,
        source_scope,
        period_year,
        max_setups,
        max_forward_bars,
    })
}

fn run_id() -> String {
    let started_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("phase1-{started_at_ms}-{}", std::process::id())
}

async fn ensure_phase1_tables(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS phase1_strategy_runs (
            id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
            run_id VARCHAR(64) NOT NULL UNIQUE,
            family_key CHAR(16) NOT NULL,
            source_scope VARCHAR(16) NOT NULL,
            period_year INT NOT NULL,
            harmonic_type VARCHAR(24) NOT NULL,
            bin VARCHAR(16) NOT NULL,
            size_bucket VARCHAR(16) NOT NULL,
            time_bin VARCHAR(16) NOT NULL,
            x_strictness VARCHAR(16) NOT NULL,
            requested_setup_count BIGINT NOT NULL,
            tested_setup_count BIGINT NOT NULL,
            route_count BIGINT NOT NULL,
            max_forward_bars BIGINT NOT NULL,
            elapsed_ms BIGINT NOT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            INDEX idx_phase1_runs_family (family_key, source_scope, period_year, created_at),
            INDEX idx_phase1_runs_latest_family (family_key, source_scope, period_year, id),
            INDEX idx_phase1_runs_latest_scope (source_scope, period_year, family_key, id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS phase1_strategy_results (
            id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
            run_id VARCHAR(64) NOT NULL,
            family_key CHAR(16) NOT NULL,
            source_scope VARCHAR(16) NOT NULL,
            period_year INT NOT NULL,
            route_id VARCHAR(128) NOT NULL,
            route_label VARCHAR(255) NOT NULL,
            result_rank BIGINT NOT NULL,
            entry_mode VARCHAR(32) NOT NULL,
            stop_mode VARCHAR(32) NOT NULL,
            target_r DOUBLE NOT NULL,
            max_hold_multiple BIGINT NOT NULL,
            setup_count BIGINT NOT NULL,
            trade_count BIGINT NOT NULL,
            no_entry_count BIGINT NOT NULL,
            win_count BIGINT NOT NULL,
            loss_count BIGINT NOT NULL,
            win_rate DOUBLE NOT NULL,
            avg_r DOUBLE NOT NULL,
            profit_factor DOUBLE NOT NULL,
            max_drawdown_r DOUBLE NOT NULL,
            worst_year_avg_r DOUBLE NOT NULL,
            score DOUBLE NOT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE KEY uniq_phase1_result_route (run_id, route_id),
            INDEX idx_phase1_results_family_rank (family_key, source_scope, period_year, result_rank),
            INDEX idx_phase1_results_score (score),
            INDEX idx_phase1_results_best_leaderboard (source_scope, period_year, result_rank, score DESC, avg_r DESC, trade_count DESC),
            INDEX idx_phase1_results_all_leaderboard (source_scope, period_year, score DESC, avg_r DESC, trade_count DESC)
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        ALTER TABLE phase1_strategy_results
        ADD COLUMN max_hold_multiple BIGINT NOT NULL DEFAULT 0 AFTER target_r
        "#,
    )
    .execute(pool)
    .await
    .ok();
    sqlx::query(
        r#"
        ALTER TABLE phase1_strategy_results
        MODIFY COLUMN max_hold_bars BIGINT NOT NULL DEFAULT 0
        "#,
    )
    .execute(pool)
    .await
    .ok();

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS phase1_strategy_yearly_results (
            run_id VARCHAR(64) NOT NULL,
            family_key CHAR(16) NOT NULL,
            source_scope VARCHAR(16) NOT NULL,
            period_year INT NOT NULL,
            route_id VARCHAR(128) NOT NULL,
            trade_year INT NOT NULL,
            setup_count BIGINT NOT NULL,
            trade_count BIGINT NOT NULL,
            no_entry_count BIGINT NOT NULL,
            win_count BIGINT NOT NULL,
            loss_count BIGINT NOT NULL,
            win_rate DOUBLE NOT NULL,
            avg_r DOUBLE NOT NULL,
            sum_r DOUBLE NOT NULL,
            profit_factor DOUBLE NOT NULL,
            max_drawdown_r DOUBLE NOT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (run_id, route_id, trade_year),
            INDEX idx_phase1_yearly_family (family_key, source_scope, period_year, trade_year)
        )
        "#,
    )
    .execute(pool)
    .await?;

    ensure_phase1_leaderboard_indexes(pool).await;

    Ok(())
}

async fn ensure_phase1_leaderboard_indexes(pool: &MySqlPool) {
    let statements = [
        r#"
        ALTER TABLE phase1_strategy_runs
        ADD INDEX idx_phase1_runs_latest_family (family_key, source_scope, period_year, id)
        "#,
        r#"
        ALTER TABLE phase1_strategy_runs
        ADD INDEX idx_phase1_runs_latest_scope (source_scope, period_year, family_key, id)
        "#,
        r#"
        ALTER TABLE phase1_strategy_results
        ADD INDEX idx_phase1_results_best_leaderboard (source_scope, period_year, result_rank, score DESC, avg_r DESC, trade_count DESC)
        "#,
        r#"
        ALTER TABLE phase1_strategy_results
        ADD INDEX idx_phase1_results_all_leaderboard (source_scope, period_year, score DESC, avg_r DESC, trade_count DESC)
        "#,
    ];

    for statement in statements {
        sqlx::query(statement).execute(pool).await.ok();
    }
}

async fn fetch_family(
    pool: &MySqlPool,
    family_key: &str,
    source_scope: &str,
    period_year: i64,
) -> Result<Option<PatternFamilySummary>, sqlx::Error> {
    sqlx::query_as::<_, PatternFamilySummary>(
        r#"
        SELECT
            family_key,
            harmonic_type,
            bin,
            size_bucket,
            time_bin,
            x_strictness,
            CAST(setup_count AS SIGNED) AS setup_count
        FROM pattern_family_source_summary
        WHERE family_key = ?
          AND source_scope = ?
          AND period_year = ?
        LIMIT 1
        "#,
    )
    .bind(family_key)
    .bind(source_scope)
    .bind(period_year)
    .fetch_optional(pool)
    .await
}

fn accuracy_bin_expr(accuracy_expr: &str) -> String {
    format!(
        "CASE
            WHEN {accuracy_expr} <= 10 THEN '0-10'
            WHEN {accuracy_expr} <= 20 THEN '10-20'
            WHEN {accuracy_expr} <= 30 THEN '20-30'
            WHEN {accuracy_expr} <= 40 THEN '30-40'
            WHEN {accuracy_expr} <= 50 THEN '40-50'
            WHEN {accuracy_expr} <= 60 THEN '50-60'
            WHEN {accuracy_expr} <= 70 THEN '60-70'
            WHEN {accuracy_expr} <= 80 THEN '70-80'
            WHEN {accuracy_expr} <= 90 THEN '80-90'
            ELSE '90-100'
        END"
    )
}

fn size_bucket_expr(total_bars_expr: &str) -> String {
    format!(
        "CASE
            WHEN {total_bars_expr} <= 20 THEN 'Micro'
            WHEN {total_bars_expr} <= 60 THEN 'Small'
            WHEN {total_bars_expr} <= 180 THEN 'Normal'
            WHEN {total_bars_expr} <= 365 THEN 'Large'
            ELSE 'Massive'
        END"
    )
}

fn x_strictness_expr(alias: &str) -> String {
    format!(
        "CASE
        WHEN COALESCE({alias}.x_length, 0) <= 0 THEN 'Loose'
        WHEN COALESCE({alias}.x_bars_left, 0) >= COALESCE({alias}.x_length, 0) THEN 'Strict'
        WHEN COALESCE({alias}.x_bars_left, 0) * 2 >= COALESCE({alias}.x_length, 0) THEN 'Normal'
        ELSE 'Loose'
    END"
    )
}

fn setup_ratio_expr(numerator_expr: &str, denominator_expr: &str) -> String {
    format!(
        "CASE
            WHEN COALESCE({denominator_expr}, 0.0) > 0
            THEN (CAST({numerator_expr} AS DOUBLE) / CAST({denominator_expr} AS DOUBLE)) * 100.0
            ELSE 0.0
        END"
    )
}

fn leg_accuracy_expr(current_expr: &str, target: f64) -> String {
    format!(
        "CASE
            WHEN COALESCE({current_expr}, 0.0) <= 0 THEN 0.0
            ELSE LEAST(
                GREATEST(
                    100.0 * (1.0 - ABS((CAST({current_expr} AS DOUBLE) / 100.0) - {target}) / {target}),
                    0.0
                ),
                100.0
            )
        END"
    )
}

fn harmonic_score_select(
    harmonic_type: &str,
    ab_xa: f64,
    bc_ab: f64,
    cd_bc: f64,
    d_completion: f64,
) -> String {
    let ab_xa_price = setup_ratio_expr("ab_price_length", "xa_price_length");
    let bc_ab_price = setup_ratio_expr("bc_price_length", "ab_price_length");
    let cd_bc_price = setup_ratio_expr("cd_price_length", "bc_price_length");
    let d_completion_price = setup_ratio_expr("ABS(a_min_max - d_min_max)", "xa_price_length");
    let ab_xa_time = setup_ratio_expr("a_length", "x_length");
    let bc_ab_time = setup_ratio_expr("b_length", "a_length");
    let cd_bc_time = setup_ratio_expr("c_length", "b_length");
    let cd_xa_time = setup_ratio_expr("c_length", "x_length");

    format!(
        r#"
        SELECT
            setup_id,
            '{harmonic_type}' AS harmonic_type,
            (
                {price_ab_xa} + {price_bc_ab} + {price_cd_bc} + {price_d_completion}
            ) / 4.0 AS price_accuracy,
            (
                {time_ab_xa} + {time_bc_ab} + {time_cd_bc} + {time_cd_xa}
            ) / 4.0 AS time_accuracy
        FROM filtered_setups
        "#,
        price_ab_xa = leg_accuracy_expr(&ab_xa_price, ab_xa),
        price_bc_ab = leg_accuracy_expr(&bc_ab_price, bc_ab),
        price_cd_bc = leg_accuracy_expr(&cd_bc_price, cd_bc),
        price_d_completion = leg_accuracy_expr(&d_completion_price, d_completion),
        time_ab_xa = leg_accuracy_expr(&ab_xa_time, ab_xa),
        time_bc_ab = leg_accuracy_expr(&bc_ab_time, bc_ab),
        time_cd_bc = leg_accuracy_expr(&cd_bc_time, cd_bc),
        time_cd_xa = leg_accuracy_expr(&cd_xa_time, d_completion),
    )
}

fn harmonic_score_selects() -> String {
    [
        harmonic_score_select("Bat", 0.500, 0.382, 1.618, 0.886),
        harmonic_score_select("AlternateBat", 0.382, 0.382, 2.000, 1.130),
        harmonic_score_select("Butterfly", 0.786, 0.382, 1.618, 1.272),
        harmonic_score_select("Gartley", 0.618, 0.382, 1.272, 0.786),
        harmonic_score_select("Crab", 0.382, 0.382, 2.618, 1.618),
        harmonic_score_select("DeepCrab", 0.886, 0.382, 2.618, 1.618),
        harmonic_score_select("Shark", 0.500, 1.130, 1.618, 0.886),
    ]
    .join("\nUNION ALL\n")
}

async fn fetch_setups(
    pool: &MySqlPool,
    family: &PatternFamilySummary,
    source_scope: &str,
    period_year: i64,
    limit: i64,
) -> Result<Vec<Phase1SetupSource>, sqlx::Error> {
    let source_filter = match source_scope {
        "futures" => "AND COALESCE(ps.source_table, '') LIKE 'futures_contract_%_candles'",
        "daily" => {
            "AND COALESCE(ps.source_table, '') = 'candles' AND COALESCE(ps.source_timeframe, '') = 'daily'"
        }
        _ => "",
    };
    let year_filter = if period_year > 0 {
        "AND YEAR(ps.d_date) = ?"
    } else {
        ""
    };
    let bin_expr = accuracy_bin_expr("COALESCE(best_harmonic.price_accuracy, 0.0)");
    let time_bin_expr = accuracy_bin_expr("COALESCE(best_harmonic.time_accuracy, 0.0)");
    let size_bucket_expr = size_bucket_expr(
        "CAST(COALESCE(fs.x_length, 0) + COALESCE(fs.a_length, 0) + COALESCE(fs.b_length, 0) + COALESCE(fs.c_length, 0) AS DOUBLE)",
    );
    let x_strictness_expr = x_strictness_expr("fs");
    let score_selects = harmonic_score_selects();
    let sql = format!(
        r#"
        WITH filtered_setups AS (
            SELECT
                ps.setup_id,
                ps.symbol,
                ps.source_table,
                ps.source_timeframe,
                ps.market,
                ps.d_date,
                ps.x_high,
                ps.x_low,
                ps.b_high,
                ps.b_low,
                ps.c_high,
                ps.c_low,
                ps.d_high,
                ps.d_low,
                ps.d_close,
                ps.cd_price_length,
                ps.full_pattern_length,
                ps.xa_price_length,
                ps.ab_price_length,
                ps.bc_price_length,
                ps.a_min_max,
                ps.d_min_max,
                ps.x_bars_left,
                ps.x_length,
                ps.a_length,
                ps.b_length,
                ps.c_length
            FROM pattern_setups ps
            WHERE ps.d_date IS NOT NULL
              {source_filter}
              {year_filter}
        ),
        setup_scores AS (
            {score_selects}
        ),
        best_harmonic AS (
            SELECT
                setup_id,
                SUBSTRING_INDEX(
                    GROUP_CONCAT(
                        harmonic_type
                        ORDER BY
                            COALESCE(price_accuracy, 0.0) DESC,
                            COALESCE(time_accuracy, 0.0) DESC,
                            harmonic_type ASC
                        SEPARATOR '|'
                    ),
                    '|',
                    1
                ) AS harmonic_type,
                CAST(SUBSTRING_INDEX(
                    GROUP_CONCAT(
                        COALESCE(CAST(price_accuracy AS CHAR), '0')
                        ORDER BY
                            COALESCE(price_accuracy, 0.0) DESC,
                            COALESCE(time_accuracy, 0.0) DESC,
                            harmonic_type ASC
                        SEPARATOR '|'
                    ),
                    '|',
                    1
                ) AS DOUBLE) AS price_accuracy,
                CAST(SUBSTRING_INDEX(
                    GROUP_CONCAT(
                        COALESCE(CAST(time_accuracy AS CHAR), '0')
                        ORDER BY
                            COALESCE(price_accuracy, 0.0) DESC,
                            COALESCE(time_accuracy, 0.0) DESC,
                            harmonic_type ASC
                        SEPARATOR '|'
                    ),
                    '|',
                    1
                ) AS DOUBLE) AS time_accuracy
            FROM setup_scores
            GROUP BY setup_id
        ),
        pattern_rows AS (
            SELECT
                fs.*,
                COALESCE(best_harmonic.harmonic_type, 'Unknown') AS best_harmonic_type,
                {bin_expr} AS best_bin,
                {time_bin_expr} AS best_time_bin,
                {size_bucket_expr} AS best_size_bucket,
                {x_strictness_expr} AS best_x_strictness
            FROM filtered_setups fs
            LEFT JOIN best_harmonic
              ON best_harmonic.setup_id = fs.setup_id
        )
        SELECT
            setup_id,
            symbol,
            source_table,
            source_timeframe,
            market,
            d_date,
            x_high,
            x_low,
            b_high,
            b_low,
            c_high,
            c_low,
            d_high,
            d_low,
            d_close,
            cd_price_length,
            CAST(full_pattern_length AS SIGNED) AS full_pattern_length
        FROM pattern_rows
        WHERE best_harmonic_type = ?
          AND best_bin = ?
          AND best_size_bucket = ?
          AND best_time_bin = ?
          AND best_x_strictness = ?
        ORDER BY d_date ASC, setup_id ASC
        LIMIT ?
        "#,
        source_filter = source_filter,
        year_filter = year_filter,
        score_selects = score_selects,
        bin_expr = bin_expr,
        time_bin_expr = time_bin_expr,
        size_bucket_expr = size_bucket_expr,
        x_strictness_expr = x_strictness_expr,
    );

    let mut query = sqlx::query_as::<_, Phase1SetupSource>(&sql);
    if period_year > 0 {
        query = query.bind(period_year);
    }

    query
        .bind(&family.harmonic_type)
        .bind(&family.bin)
        .bind(&family.size_bucket)
        .bind(&family.time_bin)
        .bind(&family.x_strictness)
        .bind(limit)
        .fetch_all(pool)
        .await
}

fn futures_candle_table(source_table: Option<&str>) -> &'static str {
    match source_table {
        Some("futures_contract_3m_candles") => "futures_contract_3m_candles",
        Some("futures_contract_5m_candles") => "futures_contract_5m_candles",
        Some("futures_contract_15m_candles") => "futures_contract_15m_candles",
        Some("futures_contract_30m_candles") => "futures_contract_30m_candles",
        Some("futures_contract_1h_candles") => "futures_contract_1h_candles",
        Some("futures_contract_4h_candles") => "futures_contract_4h_candles",
        Some("futures_contract_12h_candles") => "futures_contract_12h_candles",
        Some("futures_contract_1d_candles") => "futures_contract_1d_candles",
        _ => "futures_contract_1m_candles",
    }
}

async fn fetch_forward_candles(
    pool: &MySqlPool,
    setup: &Phase1SetupSource,
    max_forward_bars: i64,
) -> Result<Vec<Phase1ForwardCandle>, sqlx::Error> {
    let use_daily = setup
        .source_table
        .as_deref()
        .map(|table| table == "candles")
        .unwrap_or(false)
        || setup
            .source_timeframe
            .as_deref()
            .map(|timeframe| timeframe == "daily")
            .unwrap_or(false);

    if use_daily {
        return sqlx::query_as::<_, Phase1ForwardCandle>(
            r#"
            SELECT
                CAST(open AS DOUBLE) AS open,
                CAST(high AS DOUBLE) AS high,
                CAST(low AS DOUBLE) AS low,
                CAST(close AS DOUBLE) AS close
            FROM candles
            WHERE symbol = ?
              AND date > ?
            ORDER BY date ASC
            LIMIT ?
            "#,
        )
        .bind(&setup.symbol)
        .bind(setup.d_date.date())
        .bind(max_forward_bars)
        .fetch_all(pool)
        .await;
    }

    let candle_table = futures_candle_table(setup.source_table.as_deref());
    let sql = format!(
        r#"
        SELECT
            CAST(open AS DOUBLE) AS open,
            CAST(high AS DOUBLE) AS high,
            CAST(low AS DOUBLE) AS low,
            CAST(close AS DOUBLE) AS close
        FROM {candle_table}
        WHERE symbol = ?
          AND ts_utc > ?
        ORDER BY ts_utc ASC
        LIMIT ?
        "#
    );

    sqlx::query_as::<_, Phase1ForwardCandle>(&sql)
        .bind(&setup.symbol)
        .bind(setup.d_date)
        .bind(max_forward_bars)
        .fetch_all(pool)
        .await
}

fn build_routes() -> Vec<Phase1RouteSpec> {
    let entries = [
        ("next_open", "Next open"),
        ("d_break", "Break D"),
        ("d_close_confirm", "Close confirms D"),
        ("c_break", "Break C"),
        ("b_break", "Break B"),
    ];
    let stops = [
        ("d_extreme", "D stop"),
        ("c_extreme", "C stop"),
        ("x_extreme", "X stop"),
        ("cd_025", "0.25 CD stop"),
        ("cd_050", "0.50 CD stop"),
        ("cd_075", "0.75 CD stop"),
        ("cd_100", "1.00 CD stop"),
        ("cd_150", "1.50 CD stop"),
    ];
    let targets = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0];
    let max_hold_multiples = [1, 2, 3, 5];
    let mut routes = Vec::new();

    for (entry_mode, entry_label) in entries {
        for (stop_mode, stop_label) in stops {
            for target_r in targets {
                for max_hold_multiple in max_hold_multiples {
                    let target_label = format!("{target_r:.2}R").replace(".00", "");
                    routes.push(Phase1RouteSpec {
                        route_id: format!(
                            "{entry_mode}__{stop_mode}__{}__{}x",
                            target_label.replace('.', "p"),
                            max_hold_multiple
                        ),
                        route_label: format!(
                            "{entry_label} + {stop_label} + {target_label} target + {max_hold_multiple}x pattern hold"
                        ),
                        entry_mode: entry_mode.to_string(),
                        stop_mode: stop_mode.to_string(),
                        target_r,
                        max_hold_multiple,
                    });
                }
            }
        }
    }

    routes
}

fn direction(setup: &Phase1SetupSource) -> f64 {
    if setup.market.eq_ignore_ascii_case("Bearish") {
        -1.0
    } else {
        1.0
    }
}

fn break_entry(direction: f64, level: f64, candle: &Phase1ForwardCandle) -> Option<f64> {
    if direction > 0.0 && candle.high >= level {
        Some(level)
    } else if direction < 0.0 && candle.low <= level {
        Some(level)
    } else {
        None
    }
}

fn find_entry(
    route: &Phase1RouteSpec,
    setup: &Phase1SetupSource,
    candles: &[Phase1ForwardCandle],
) -> Option<(usize, f64)> {
    let direction = direction(setup);
    let entry_scan_limit = candles.len().min(80);

    match route.entry_mode.as_str() {
        "next_open" => candles.first().map(|candle| (0, candle.open)),
        "d_break" => {
            let level = if direction > 0.0 {
                setup.d_high
            } else {
                setup.d_low
            };
            candles
                .iter()
                .take(entry_scan_limit)
                .enumerate()
                .find_map(|(index, candle)| {
                    break_entry(direction, level, candle).map(|entry| (index, entry))
                })
        }
        "d_close_confirm" => {
            candles
                .iter()
                .take(entry_scan_limit)
                .enumerate()
                .find_map(|(index, candle)| {
                    let confirms = (direction > 0.0 && candle.close > setup.d_close)
                        || (direction < 0.0 && candle.close < setup.d_close);
                    confirms.then_some((index, candle.close))
                })
        }
        "c_break" => {
            let level = if direction > 0.0 {
                setup.c_high
            } else {
                setup.c_low
            };
            candles
                .iter()
                .take(entry_scan_limit)
                .enumerate()
                .find_map(|(index, candle)| {
                    break_entry(direction, level, candle).map(|entry| (index, entry))
                })
        }
        "b_break" => {
            let level = if direction > 0.0 {
                setup.b_high
            } else {
                setup.b_low
            };
            candles
                .iter()
                .take(entry_scan_limit)
                .enumerate()
                .find_map(|(index, candle)| {
                    break_entry(direction, level, candle).map(|entry| (index, entry))
                })
        }
        _ => None,
    }
}

fn stop_price(route: &Phase1RouteSpec, setup: &Phase1SetupSource, entry_price: f64) -> f64 {
    let direction = direction(setup);
    match route.stop_mode.as_str() {
        "d_extreme" => {
            if direction > 0.0 {
                setup.d_low
            } else {
                setup.d_high
            }
        }
        "c_extreme" => {
            if direction > 0.0 {
                setup.c_low
            } else {
                setup.c_high
            }
        }
        "x_extreme" => {
            if direction > 0.0 {
                setup.x_low
            } else {
                setup.x_high
            }
        }
        "cd_025" => entry_price - direction * setup.cd_price_length.abs() * 0.25,
        "cd_050" => entry_price - direction * setup.cd_price_length.abs() * 0.50,
        "cd_075" => entry_price - direction * setup.cd_price_length.abs() * 0.75,
        "cd_100" => entry_price - direction * setup.cd_price_length.abs() * 1.00,
        "cd_150" => entry_price - direction * setup.cd_price_length.abs() * 1.50,
        _ => entry_price - direction * setup.cd_price_length.abs(),
    }
}

fn replay_route(
    route: &Phase1RouteSpec,
    setup: &Phase1SetupSource,
    candles: &[Phase1ForwardCandle],
) -> Option<f64> {
    let direction = direction(setup);
    let (entry_index, entry_price) = find_entry(route, setup, candles)?;
    let stop_price = stop_price(route, setup, entry_price);
    let risk = (entry_price - stop_price) * direction;
    if !risk.is_finite() || risk <= 0.0 {
        return None;
    }

    let target_price = entry_price + direction * risk * route.target_r;
    let max_hold_bars = setup
        .full_pattern_length
        .saturating_mul(route.max_hold_multiple.max(1))
        .max(1) as usize;
    let end_index = candles.len().min(entry_index.saturating_add(max_hold_bars));
    if end_index <= entry_index {
        return None;
    }

    for candle in &candles[entry_index..end_index] {
        let stop_hit = (direction > 0.0 && candle.low <= stop_price)
            || (direction < 0.0 && candle.high >= stop_price);
        if stop_hit {
            return Some(-1.0);
        }

        let target_hit = (direction > 0.0 && candle.high >= target_price)
            || (direction < 0.0 && candle.low <= target_price);
        if target_hit {
            return Some(route.target_r);
        }
    }

    let exit_close = candles[end_index - 1].close;
    Some(((exit_close - entry_price) * direction) / risk)
}

async fn insert_results(
    pool: &MySqlPool,
    run_id: &str,
    family: &PatternFamilySummary,
    args: &Args,
    tested_setup_count: i64,
    route_count: i64,
    elapsed_ms: i64,
    results: &[Phase1ResultRow],
    yearly_results: &[Phase1YearlyResultRow],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"
        INSERT INTO phase1_strategy_runs (
            run_id,
            family_key,
            source_scope,
            period_year,
            harmonic_type,
            bin,
            size_bucket,
            time_bin,
            x_strictness,
            requested_setup_count,
            tested_setup_count,
            route_count,
            max_forward_bars,
            elapsed_ms
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(run_id)
    .bind(&family.family_key)
    .bind(&args.source_scope)
    .bind(args.period_year)
    .bind(&family.harmonic_type)
    .bind(&family.bin)
    .bind(&family.size_bucket)
    .bind(&family.time_bin)
    .bind(&family.x_strictness)
    .bind(family.setup_count)
    .bind(tested_setup_count)
    .bind(route_count)
    .bind(args.max_forward_bars)
    .bind(elapsed_ms)
    .execute(&mut *tx)
    .await?;

    let mut builder = QueryBuilder::<MySql>::new(
        r#"
        INSERT INTO phase1_strategy_results (
            run_id,
            family_key,
            source_scope,
            period_year,
            route_id,
            route_label,
            result_rank,
            entry_mode,
            stop_mode,
            target_r,
            max_hold_multiple,
            setup_count,
            trade_count,
            no_entry_count,
            win_count,
            loss_count,
            win_rate,
            avg_r,
            profit_factor,
            max_drawdown_r,
            worst_year_avg_r,
            score
        )
        "#,
    );

    builder.push_values(results, |mut row, result| {
        row.push_bind(&result.run_id)
            .push_bind(&result.family_key)
            .push_bind(&result.source_scope)
            .push_bind(result.period_year)
            .push_bind(&result.route_id)
            .push_bind(&result.route_label)
            .push_bind(result.rank)
            .push_bind(&result.entry_mode)
            .push_bind(&result.stop_mode)
            .push_bind(result.target_r)
            .push_bind(result.max_hold_multiple)
            .push_bind(result.setup_count)
            .push_bind(result.trade_count)
            .push_bind(result.no_entry_count)
            .push_bind(result.win_count)
            .push_bind(result.loss_count)
            .push_bind(result.win_rate)
            .push_bind(result.avg_r)
            .push_bind(result.profit_factor)
            .push_bind(result.max_drawdown_r)
            .push_bind(result.worst_year_avg_r)
            .push_bind(result.score);
    });
    builder.build().execute(&mut *tx).await?;

    if !yearly_results.is_empty() {
        let mut yearly_builder = QueryBuilder::<MySql>::new(
            r#"
            INSERT INTO phase1_strategy_yearly_results (
                run_id,
                family_key,
                source_scope,
                period_year,
                route_id,
                trade_year,
                setup_count,
                trade_count,
                no_entry_count,
                win_count,
                loss_count,
                win_rate,
                avg_r,
                sum_r,
                profit_factor,
                max_drawdown_r
            )
            "#,
        );

        yearly_builder.push_values(yearly_results, |mut row, result| {
            row.push_bind(&result.run_id)
                .push_bind(&result.family_key)
                .push_bind(&result.source_scope)
                .push_bind(result.period_year)
                .push_bind(&result.route_id)
                .push_bind(result.year)
                .push_bind(result.setup_count)
                .push_bind(result.trade_count)
                .push_bind(result.no_entry_count)
                .push_bind(result.win_count)
                .push_bind(result.loss_count)
                .push_bind(result.win_rate)
                .push_bind(result.avg_r)
                .push_bind(result.sum_r)
                .push_bind(result.profit_factor)
                .push_bind(result.max_drawdown_r);
        });
        yearly_builder.build().execute(&mut *tx).await?;
    }

    tx.commit().await?;

    Ok(())
}

fn print_top_results(results: &[Phase1ResultRow]) {
    println!("Top Phase 1 routes:");
    for result in results.iter().take(10) {
        println!(
            "#{:<3} score={:>8.2} avg_r={:>7.3} trades={:<5} win={:>6.2}% pf={:>6.2} dd={:>7.2} {}",
            result.rank,
            result.score,
            result.avg_r,
            result.trade_count,
            result.win_rate,
            result.profit_factor,
            result.max_drawdown_r,
            result.route_label
        );
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let args = parse_args()?;
    let started = Instant::now();
    let run_id = run_id();
    let pool = MySqlPool::connect(&database_url_from_env()?).await?;
    ensure_phase1_tables(&pool).await?;

    let family = fetch_family(
        &pool,
        &args.family_key,
        &args.source_scope,
        args.period_year,
    )
    .await?
    .ok_or_else(|| {
        format!(
            "Family {} not found for source={} year={}",
            args.family_key, args.source_scope, args.period_year
        )
    })?;

    println!(
        "Phase 1 run {run_id}: family={} {} / {} / {} / {} / {}, source={}, year={}, family setups={}, max setups={}, forward bars={}",
        family.family_key,
        family.harmonic_type,
        family.bin,
        family.size_bucket,
        family.time_bin,
        family.x_strictness,
        args.source_scope,
        if args.period_year > 0 { args.period_year.to_string() } else { "all".to_string() },
        family.setup_count,
        args.max_setups,
        args.max_forward_bars
    );
    println!(
        "Forward window: min(full_pattern_length * 5, {} safety cap). Time exits: 1x, 2x, 3x, 5x pattern length.",
        args.max_forward_bars
    );

    let setups = fetch_setups(
        &pool,
        &family,
        &args.source_scope,
        args.period_year,
        args.max_setups,
    )
    .await?;
    if setups.is_empty() {
        println!("No setups matched this family.");
        return Ok(());
    }

    let routes = build_routes();
    println!(
        "Testing {} setups against {} entry/exit routes...",
        setups.len(),
        routes.len()
    );
    let mut accumulators = routes
        .into_iter()
        .map(Phase1RouteAccumulator::new)
        .collect::<Vec<_>>();

    for (index, setup) in setups.iter().enumerate() {
        let setup_forward_bars = setup
            .full_pattern_length
            .saturating_mul(5)
            .clamp(40, args.max_forward_bars);
        let candles = fetch_forward_candles(&pool, setup, setup_forward_bars)
            .await
            .unwrap_or_else(|error| {
                eprintln!(
                    "Candle fetch failed for setup {} ({}): {:?}",
                    setup.setup_id, setup.symbol, error
                );
                Vec::new()
            });
        let year = setup.d_date.year();

        for accumulator in &mut accumulators {
            if candles.is_empty() {
                accumulator.record_no_entry(year);
                continue;
            }

            match replay_route(&accumulator.spec, setup, &candles) {
                Some(result_r) if result_r.is_finite() => accumulator.record_trade(year, result_r),
                _ => accumulator.record_no_entry(year),
            }
        }

        if (index + 1) % 100 == 0 || index + 1 == setups.len() {
            println!("Processed {}/{} setups", index + 1, setups.len());
        }
    }

    let mut result_rows = accumulators
        .into_iter()
        .map(|accumulator| {
            accumulator.into_rows(
                &run_id,
                &family.family_key,
                &args.source_scope,
                args.period_year,
            )
        })
        .collect::<Vec<_>>();
    result_rows.sort_by(|left, right| {
        right
            .0
            .score
            .total_cmp(&left.0.score)
            .then_with(|| right.0.avg_r.total_cmp(&left.0.avg_r))
            .then_with(|| right.0.trade_count.cmp(&left.0.trade_count))
    });

    let mut yearly_results = Vec::new();
    let mut results = Vec::with_capacity(result_rows.len());
    for (index, (mut result, yearly_rows)) in result_rows.into_iter().enumerate() {
        result.rank = index as i64 + 1;
        if index < 50 {
            yearly_results.extend(yearly_rows);
        }
        results.push(result);
    }

    let elapsed_ms = started.elapsed().as_millis() as i64;
    insert_results(
        &pool,
        &run_id,
        &family,
        &args,
        setups.len() as i64,
        results.len() as i64,
        elapsed_ms,
        &results,
        &yearly_results,
    )
    .await?;

    println!(
        "Stored {} Phase 1 route results and {} yearly rows in {:.2}s",
        results.len(),
        yearly_results.len(),
        elapsed_ms as f64 / 1000.0
    );
    print_top_results(&results);

    Ok(())
}
