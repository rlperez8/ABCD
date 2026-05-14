use std::collections::HashMap;
use std::env;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use chrono::{Datelike, NaiveDateTime};
use sqlx::{MySql, MySqlPool, QueryBuilder};

#[derive(Debug)]
struct Args {
    family_key: Option<String>,
    all_families: bool,
    source_scope: String,
    period_year: i64,
    max_setups: i64,
    max_forward_bars: i64,
    family_limit: i64,
    test_id: Option<String>,
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
    pattern_id: Option<String>,
    pattern_group_id: String,
    symbol: String,
    source_table: Option<String>,
    source_timeframe: Option<String>,
    market: String,
    d_date: NaiveDateTime,
    d_confirm_date: NaiveDateTime,
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
    candle_date: NaiveDateTime,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
}

#[derive(Clone, sqlx::FromRow)]
struct Phase1RouteSpec {
    route_id: String,
    route_label: String,
    entry_mode: String,
    stop_mode: String,
    target_r: f64,
    max_hold_multiple: i64,
}

struct EntryDecision {
    index: usize,
    price: f64,
    direction: f64,
}

struct Phase1TradeDetail {
    result_r: f64,
    entry_date: NaiveDateTime,
    exit_date: NaiveDateTime,
    exit_price: f64,
    entry_price: f64,
    stop_price: f64,
    target_price: f64,
    lowest_price: f64,
    highest_price: f64,
    adverse_price: f64,
    favorable_price: f64,
    max_adverse_points: f64,
    max_favorable_points: f64,
    risk_points: f64,
    trade_direction: f64,
    trade_result: i64,
    exit_reason: String,
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

struct Phase1TradeRow {
    trade_uid: String,
    run_id: String,
    family_key: String,
    source_scope: String,
    period_year: i64,
    route_id: String,
    route_label: String,
    entry_mode: String,
    stop_mode: String,
    target_r: f64,
    max_hold_multiple: i64,
    setup_id: String,
    pattern_id: Option<String>,
    pattern_group_id: String,
    symbol: String,
    market: String,
    d_date: NaiveDateTime,
    d_confirm_date: NaiveDateTime,
    entry_date: NaiveDateTime,
    exit_date: NaiveDateTime,
    entry_price: f64,
    stop_price: f64,
    target_price: f64,
    exit_price: f64,
    result_r: f64,
    risk_points: f64,
    trade_direction: String,
    trade_result: i64,
    exit_reason: String,
    lowest_price: f64,
    highest_price: f64,
    adverse_price: f64,
    favorable_price: f64,
    max_adverse_points: f64,
    max_favorable_points: f64,
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
    "Usage: cargo run --bin run_phase1_optimizer -- (--family <family_key> | --all-families) [--source futures|daily|all] [--year YYYY] [--test-id TEST] [--family-limit N] [--max-setups N]\nNote: forward bars are dynamic per setup: full_pattern_length * 5."
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

    let all_families = raw_args.iter().any(|arg| arg == "--all-families");
    let family_key = arg_value(&raw_args, "--family")
        .or_else(|| arg_value(&raw_args, "--family-key"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if !all_families && family_key.is_none() {
        return Err(format!("Missing --family argument.\n{}", usage()).into());
    }
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
        .unwrap_or(0)
        .max(0);
    let family_limit = arg_value(&raw_args, "--family-limit")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0)
        .clamp(0, 100_000);
    let test_id = arg_value(&raw_args, "--test-id")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    Ok(Args {
        family_key,
        all_families,
        source_scope,
        period_year,
        max_setups,
        max_forward_bars,
        family_limit,
        test_id,
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
            trade_uid VARCHAR(320) NOT NULL,
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

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS phase1_strategy_trades (
            id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
            run_id VARCHAR(64) NOT NULL,
            family_key CHAR(16) NOT NULL,
            source_scope VARCHAR(16) NOT NULL,
            period_year INT NOT NULL,
            route_id VARCHAR(128) NOT NULL,
            route_label VARCHAR(255) NOT NULL,
            entry_mode VARCHAR(64) NOT NULL,
            stop_mode VARCHAR(64) NOT NULL,
            target_r DOUBLE NOT NULL,
            max_hold_multiple BIGINT NOT NULL,
            setup_id VARCHAR(64) NOT NULL,
            pattern_id VARCHAR(64) NULL,
            pattern_group_id VARCHAR(128) NOT NULL,
            symbol VARCHAR(32) NOT NULL,
            market VARCHAR(16) NOT NULL,
            d_date DATETIME NOT NULL,
            d_confirm_date DATETIME NOT NULL,
            entry_date DATETIME NOT NULL,
            exit_date DATETIME NOT NULL,
            entry_price DOUBLE NOT NULL,
            stop_price DOUBLE NOT NULL,
            target_price DOUBLE NOT NULL,
            exit_price DOUBLE NOT NULL,
            result_r DOUBLE NOT NULL,
            risk_points DOUBLE NOT NULL,
            trade_direction VARCHAR(8) NOT NULL,
            trade_result BIGINT NOT NULL,
            exit_reason VARCHAR(16) NOT NULL,
            lowest_price DOUBLE NOT NULL,
            highest_price DOUBLE NOT NULL,
            adverse_price DOUBLE NOT NULL,
            favorable_price DOUBLE NOT NULL,
            max_adverse_points DOUBLE NOT NULL,
            max_favorable_points DOUBLE NOT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE KEY uniq_phase1_trade_uid (trade_uid),
            UNIQUE KEY uniq_phase1_trade (run_id, route_id, setup_id),
            INDEX idx_phase1_trades_route (family_key, run_id, route_id, entry_date),
            INDEX idx_phase1_trades_pattern (pattern_id, pattern_group_id),
            INDEX idx_phase1_trades_test (route_id, source_scope, period_year)
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        ALTER TABLE phase1_strategy_trades
        ADD COLUMN trade_uid VARCHAR(320) NULL AFTER id
        "#,
    )
    .execute(pool)
    .await
    .ok();
    sqlx::query(
        r#"
        UPDATE phase1_strategy_trades
        SET trade_uid = CONCAT(run_id, '::', route_id, '::', setup_id)
        WHERE trade_uid IS NULL OR trade_uid = ''
        "#,
    )
    .execute(pool)
    .await
    .ok();
    sqlx::query(
        r#"
        ALTER TABLE phase1_strategy_trades
        MODIFY COLUMN trade_uid VARCHAR(320) NOT NULL
        "#,
    )
    .execute(pool)
    .await
    .ok();
    sqlx::query(
        r#"
        ALTER TABLE phase1_strategy_trades
        ADD UNIQUE KEY uniq_phase1_trade_uid (trade_uid)
        "#,
    )
    .execute(pool)
    .await
    .ok();

    ensure_phase1_leaderboard_indexes(pool).await;
    ensure_entry_exit_tests(pool).await?;

    Ok(())
}

async fn ensure_entry_exit_tests(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_tests (
            test_id VARCHAR(128) NOT NULL PRIMARY KEY,
            test_name VARCHAR(255) NOT NULL,
            entry_mode VARCHAR(64) NOT NULL,
            stop_mode VARCHAR(64) NOT NULL,
            target_r DOUBLE NOT NULL,
            max_hold_multiple BIGINT NOT NULL,
            is_enabled BOOLEAN NOT NULL DEFAULT FALSE,
            notes TEXT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            INDEX idx_entry_exit_tests_enabled (is_enabled, updated_at)
        )
        "#,
    )
    .execute(pool)
    .await?;

    seed_entry_exit_tests(pool).await
}

async fn seed_entry_exit_tests(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    for route in default_entry_exit_tests(false) {
        sqlx::query(
            r#"
            INSERT IGNORE INTO entry_exit_tests (
                test_id, test_name, entry_mode, stop_mode, target_r, max_hold_multiple, is_enabled, notes
            )
            VALUES (?, ?, ?, ?, ?, ?, FALSE, ?)
            "#,
        )
        .bind(&route.route_id)
        .bind(&route.route_label)
        .bind(&route.entry_mode)
        .bind(&route.stop_mode)
        .bind(route.target_r)
        .bind(route.max_hold_multiple)
        .bind("Legacy Phase 1 route candidate. Enable it when you want to include it in the test batch.")
        .execute(pool)
        .await?;
    }

    sqlx::query(
        r#"
        INSERT INTO entry_exit_tests (
            test_id, test_name, entry_mode, stop_mode, target_r, max_hold_multiple, is_enabled, notes
        )
        VALUES (?, ?, ?, ?, ?, ?, TRUE, ?)
        ON DUPLICATE KEY UPDATE
            test_name = VALUES(test_name),
            entry_mode = VALUES(entry_mode),
            stop_mode = VALUES(stop_mode),
            target_r = VALUES(target_r),
            max_hold_multiple = VALUES(max_hold_multiple),
            notes = VALUES(notes)
        "#,
    )
    .bind("post_confirm_decision__c_extreme__4R__1x")
    .bind("Post-confirm decision + C stop + 4R target + 1x pattern hold")
    .bind("post_confirm_decision")
    .bind("c_extreme")
    .bind(4.0)
    .bind(1_i64)
    .bind("Decision candle test: bullish setup enters at the decision candle open, long if that candle closes above the confirmation open, short only if it closes back below D high. Bearish setup mirrors this: short below confirmation open, long only back above D low.")
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO entry_exit_tests (
            test_id, test_name, entry_mode, stop_mode, target_r, max_hold_multiple, is_enabled, notes
        )
        VALUES (?, ?, ?, ?, ?, ?, TRUE, ?)
        ON DUPLICATE KEY UPDATE
            test_name = VALUES(test_name),
            entry_mode = VALUES(entry_mode),
            stop_mode = VALUES(stop_mode),
            target_r = VALUES(target_r),
            max_hold_multiple = VALUES(max_hold_multiple),
            notes = VALUES(notes)
        "#,
    )
    .bind("confirm_p1_body_signal_p2_open__c_extreme__4R__1x")
    .bind("Confirmation +1 body signal + Confirmation +2 open + C stop + 4R target + 1x hold")
    .bind("confirm_p1_body_signal_p2_open")
    .bind("c_extreme")
    .bind(4.0)
    .bind(1_i64)
    .bind("Body signal test: bearish setup requires Confirmation +1 close below Confirmation body low, then enters short at Confirmation +2 open only if that open is below Confirmation body low and below Confirmation +1 body high. Bullish setup mirrors this above Confirmation body high and above Confirmation +1 body low.")
    .execute(pool)
    .await?;

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

async fn fetch_families(
    pool: &MySqlPool,
    source_scope: &str,
    period_year: i64,
    limit: i64,
) -> Result<Vec<PatternFamilySummary>, sqlx::Error> {
    let sql = if limit > 0 {
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
        WHERE source_scope = ?
          AND period_year = ?
        ORDER BY setup_count DESC, family_key ASC
        LIMIT ?
        "#
    } else {
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
        WHERE source_scope = ?
          AND period_year = ?
        ORDER BY setup_count DESC, family_key ASC
        "#
    };

    let mut query = sqlx::query_as::<_, PatternFamilySummary>(sql)
        .bind(source_scope)
        .bind(period_year);
    if limit > 0 {
        query = query.bind(limit);
    }

    query.fetch_all(pool).await
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
    let sql = format!(
        r#"
        SELECT
            ps.setup_id,
            ps.pattern_id,
            ps.pattern_group_id,
            ps.symbol,
            ps.source_table,
            ps.source_timeframe,
            ps.market,
            ps.d_date,
            COALESCE(ps.d_confirm_date, ps.d_date) AS d_confirm_date,
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
            CAST(ps.full_pattern_length AS SIGNED) AS full_pattern_length
        FROM pattern_setups ps
        WHERE ps.pattern_family_key = ?
          AND ps.d_date IS NOT NULL
          {source_filter}
          {year_filter}
        ORDER BY ps.d_date ASC, ps.setup_id ASC
        LIMIT ?
        "#,
        source_filter = source_filter,
        year_filter = year_filter,
    );

    let mut query = sqlx::query_as::<_, Phase1SetupSource>(&sql).bind(&family.family_key);
    if period_year > 0 {
        query = query.bind(period_year);
    }

    query.bind(limit).fetch_all(pool).await
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
                CAST(date AS DATETIME) AS candle_date,
                CAST(open AS DOUBLE) AS open,
                CAST(high AS DOUBLE) AS high,
                CAST(low AS DOUBLE) AS low,
                CAST(close AS DOUBLE) AS close
            FROM candles
            WHERE symbol = ?
              AND date >= ?
            ORDER BY date ASC
            LIMIT ?
            "#,
        )
        .bind(&setup.symbol)
        .bind(setup.d_confirm_date.date())
        .bind(max_forward_bars.saturating_add(1))
        .fetch_all(pool)
        .await;
    }

    let candle_table = futures_candle_table(setup.source_table.as_deref());
    let sql = format!(
        r#"
        SELECT
            ts_utc AS candle_date,
            CAST(open AS DOUBLE) AS open,
            CAST(high AS DOUBLE) AS high,
            CAST(low AS DOUBLE) AS low,
            CAST(close AS DOUBLE) AS close
        FROM {candle_table}
        WHERE symbol = ?
          AND ts_utc >= ?
        ORDER BY ts_utc ASC
        LIMIT ?
        "#
    );

    sqlx::query_as::<_, Phase1ForwardCandle>(&sql)
        .bind(&setup.symbol)
        .bind(setup.d_confirm_date)
        .bind(max_forward_bars.saturating_add(1))
        .fetch_all(pool)
        .await
}

fn default_entry_exit_tests(include_decision_test: bool) -> Vec<Phase1RouteSpec> {
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

    if include_decision_test {
        routes.push(Phase1RouteSpec {
            route_id: "post_confirm_decision__c_extreme__4R__1x".to_string(),
            route_label: "Post-confirm decision + C stop + 4R target + 1x pattern hold".to_string(),
            entry_mode: "post_confirm_decision".to_string(),
            stop_mode: "c_extreme".to_string(),
            target_r: 4.0,
            max_hold_multiple: 1,
        });
        routes.push(Phase1RouteSpec {
            route_id: "confirm_p1_body_signal_p2_open__c_extreme__4R__1x".to_string(),
            route_label:
                "Confirmation +1 body signal + Confirmation +2 open + C stop + 4R target + 1x hold"
                    .to_string(),
            entry_mode: "confirm_p1_body_signal_p2_open".to_string(),
            stop_mode: "c_extreme".to_string(),
            target_r: 4.0,
            max_hold_multiple: 1,
        });
    }

    routes
}

async fn fetch_entry_exit_tests(
    pool: &MySqlPool,
    test_id: Option<&str>,
) -> Result<Vec<Phase1RouteSpec>, sqlx::Error> {
    let rows = if let Some(test_id) = test_id {
        sqlx::query_as::<_, Phase1RouteSpec>(
            r#"
            SELECT
                test_id AS route_id,
                test_name AS route_label,
                entry_mode,
                stop_mode,
                target_r,
                CAST(max_hold_multiple AS SIGNED) AS max_hold_multiple
            FROM entry_exit_tests
            WHERE test_id = ?
            LIMIT 1
            "#,
        )
        .bind(test_id)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, Phase1RouteSpec>(
            r#"
            SELECT
                test_id AS route_id,
                test_name AS route_label,
                entry_mode,
                stop_mode,
                target_r,
                CAST(max_hold_multiple AS SIGNED) AS max_hold_multiple
            FROM entry_exit_tests
            WHERE is_enabled = TRUE
            ORDER BY updated_at DESC, test_name ASC
            "#,
        )
        .fetch_all(pool)
        .await?
    };

    if rows.is_empty() {
        Ok(default_entry_exit_tests(true)
            .into_iter()
            .filter(|route| route.entry_mode == "post_confirm_decision")
            .collect())
    } else {
        Ok(rows)
    }
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

fn forward_start_index(setup: &Phase1SetupSource, candles: &[Phase1ForwardCandle]) -> usize {
    candles
        .first()
        .filter(|candle| candle.candle_date <= setup.d_confirm_date)
        .map(|_| 1)
        .unwrap_or(0)
}

fn find_entry(
    route: &Phase1RouteSpec,
    setup: &Phase1SetupSource,
    candles: &[Phase1ForwardCandle],
) -> Option<EntryDecision> {
    let setup_direction = direction(setup);
    let start_index = forward_start_index(setup, candles);
    let entry_scan_limit = candles.len().min(start_index.saturating_add(80));

    match route.entry_mode.as_str() {
        "next_open" => candles.get(start_index).map(|candle| EntryDecision {
            index: start_index,
            price: candle.open,
            direction: setup_direction,
        }),
        "post_confirm_decision" => {
            if start_index == 0 || start_index >= candles.len() {
                return None;
            }

            let confirmation = &candles[start_index - 1];
            let decision = &candles[start_index];

            if setup_direction > 0.0 {
                if decision.close > confirmation.open {
                    Some(EntryDecision {
                        index: start_index,
                        price: decision.open,
                        direction: 1.0,
                    })
                } else if decision.close < setup.d_high {
                    Some(EntryDecision {
                        index: start_index,
                        price: decision.open,
                        direction: -1.0,
                    })
                } else {
                    None
                }
            } else if decision.close < confirmation.open {
                Some(EntryDecision {
                    index: start_index,
                    price: decision.open,
                    direction: -1.0,
                })
            } else if decision.close > setup.d_low {
                Some(EntryDecision {
                    index: start_index,
                    price: decision.open,
                    direction: 1.0,
                })
            } else {
                None
            }
        }
        "confirm_p1_body_signal_p2_open" => {
            if start_index == 0 || start_index + 1 >= candles.len() {
                return None;
            }

            let confirmation = &candles[start_index - 1];
            let confirmation_plus_one = &candles[start_index];
            let confirmation_plus_two = &candles[start_index + 1];

            let confirmation_body_low = confirmation.open.min(confirmation.close);
            let confirmation_body_high = confirmation.open.max(confirmation.close);
            let plus_one_body_low = confirmation_plus_one.open.min(confirmation_plus_one.close);
            let plus_one_body_high = confirmation_plus_one.open.max(confirmation_plus_one.close);

            if setup_direction > 0.0 {
                let signal = confirmation_plus_one.close > confirmation_body_high;
                let entry_valid = confirmation_plus_two.open > confirmation_body_high
                    && confirmation_plus_two.open > plus_one_body_low;
                (signal && entry_valid).then_some(EntryDecision {
                    index: start_index + 1,
                    price: confirmation_plus_two.open,
                    direction: 1.0,
                })
            } else {
                let signal = confirmation_plus_one.close < confirmation_body_low;
                let entry_valid = confirmation_plus_two.open < confirmation_body_low
                    && confirmation_plus_two.open < plus_one_body_high;
                (signal && entry_valid).then_some(EntryDecision {
                    index: start_index + 1,
                    price: confirmation_plus_two.open,
                    direction: -1.0,
                })
            }
        }
        "d_break" => {
            let level = if setup_direction > 0.0 {
                setup.d_high
            } else {
                setup.d_low
            };
            candles
                .iter()
                .skip(start_index)
                .take(entry_scan_limit)
                .enumerate()
                .find_map(|(index, candle)| {
                    break_entry(setup_direction, level, candle).map(|entry| EntryDecision {
                        index: start_index + index,
                        price: entry,
                        direction: setup_direction,
                    })
                })
        }
        "d_close_confirm" => candles
            .iter()
            .skip(start_index)
            .take(entry_scan_limit)
            .enumerate()
            .find_map(|(index, candle)| {
                let confirms = (setup_direction > 0.0 && candle.close > setup.d_close)
                    || (setup_direction < 0.0 && candle.close < setup.d_close);
                confirms.then_some(EntryDecision {
                    index: start_index + index,
                    price: candle.close,
                    direction: setup_direction,
                })
            }),
        "c_break" => {
            let level = if setup_direction > 0.0 {
                setup.c_high
            } else {
                setup.c_low
            };
            candles
                .iter()
                .skip(start_index)
                .take(entry_scan_limit)
                .enumerate()
                .find_map(|(index, candle)| {
                    break_entry(setup_direction, level, candle).map(|entry| EntryDecision {
                        index: start_index + index,
                        price: entry,
                        direction: setup_direction,
                    })
                })
        }
        "b_break" => {
            let level = if setup_direction > 0.0 {
                setup.b_high
            } else {
                setup.b_low
            };
            candles
                .iter()
                .skip(start_index)
                .take(entry_scan_limit)
                .enumerate()
                .find_map(|(index, candle)| {
                    break_entry(setup_direction, level, candle).map(|entry| EntryDecision {
                        index: start_index + index,
                        price: entry,
                        direction: setup_direction,
                    })
                })
        }
        _ => None,
    }
}

fn stop_price(
    route: &Phase1RouteSpec,
    setup: &Phase1SetupSource,
    entry_price: f64,
    trade_direction: f64,
) -> f64 {
    match route.stop_mode.as_str() {
        "d_extreme" => {
            if trade_direction > 0.0 {
                setup.d_low
            } else {
                setup.d_high
            }
        }
        "c_extreme" => {
            if trade_direction > 0.0 {
                setup.c_low
            } else {
                setup.c_high
            }
        }
        "x_extreme" => {
            if trade_direction > 0.0 {
                setup.x_low
            } else {
                setup.x_high
            }
        }
        "cd_025" => entry_price - trade_direction * setup.cd_price_length.abs() * 0.25,
        "cd_050" => entry_price - trade_direction * setup.cd_price_length.abs() * 0.50,
        "cd_075" => entry_price - trade_direction * setup.cd_price_length.abs() * 0.75,
        "cd_100" => entry_price - trade_direction * setup.cd_price_length.abs() * 1.00,
        "cd_150" => entry_price - trade_direction * setup.cd_price_length.abs() * 1.50,
        _ => entry_price - trade_direction * setup.cd_price_length.abs(),
    }
}

fn replay_route_detail(
    route: &Phase1RouteSpec,
    setup: &Phase1SetupSource,
    candles: &[Phase1ForwardCandle],
) -> Option<Phase1TradeDetail> {
    let entry = find_entry(route, setup, candles)?;
    let stop_price = stop_price(route, setup, entry.price, entry.direction);
    let risk = (entry.price - stop_price) * entry.direction;
    if !risk.is_finite() || risk <= 0.0 {
        return None;
    }

    let target_price = entry.price + entry.direction * risk * route.target_r;
    let max_hold_bars = setup
        .full_pattern_length
        .saturating_mul(route.max_hold_multiple.max(1))
        .max(1) as usize;
    let end_index = candles.len().min(entry.index.saturating_add(max_hold_bars));
    if end_index <= entry.index {
        return None;
    }

    let mut lowest_price = f64::INFINITY;
    let mut highest_price = f64::NEG_INFINITY;
    let mut max_adverse_points = 0.0f64;
    let mut max_favorable_points = 0.0f64;
    let mut adverse_price = entry.price;
    let mut favorable_price = entry.price;

    for (offset, candle) in candles[entry.index..end_index].iter().enumerate() {
        lowest_price = lowest_price.min(candle.low);
        highest_price = highest_price.max(candle.high);

        let (adverse_points, candidate_adverse_price) = if entry.direction > 0.0 {
            ((entry.price - candle.low).max(0.0), candle.low)
        } else {
            ((candle.high - entry.price).max(0.0), candle.high)
        };
        if adverse_points > max_adverse_points {
            max_adverse_points = adverse_points;
            adverse_price = candidate_adverse_price;
        }

        let (favorable_points, candidate_favorable_price) = if entry.direction > 0.0 {
            ((candle.high - entry.price).max(0.0), candle.high)
        } else {
            ((entry.price - candle.low).max(0.0), candle.low)
        };
        if favorable_points > max_favorable_points {
            max_favorable_points = favorable_points;
            favorable_price = candidate_favorable_price;
        }

        let candle_index = entry.index + offset;
        let stop_hit = (entry.direction > 0.0 && candle.low <= stop_price)
            || (entry.direction < 0.0 && candle.high >= stop_price);
        if stop_hit {
            return Some(Phase1TradeDetail {
                result_r: -1.0,
                entry_date: candles[entry.index].candle_date,
                exit_date: candles[candle_index].candle_date,
                exit_price: stop_price,
                entry_price: entry.price,
                stop_price,
                target_price,
                lowest_price,
                highest_price,
                adverse_price: stop_price,
                favorable_price,
                max_adverse_points: risk.abs().max(max_adverse_points),
                max_favorable_points,
                risk_points: risk.abs(),
                trade_direction: entry.direction,
                trade_result: 2,
                exit_reason: "stop".to_string(),
            });
        }

        let target_hit = (entry.direction > 0.0 && candle.high >= target_price)
            || (entry.direction < 0.0 && candle.low <= target_price);
        if target_hit {
            return Some(Phase1TradeDetail {
                result_r: route.target_r,
                entry_date: candles[entry.index].candle_date,
                exit_date: candles[candle_index].candle_date,
                exit_price: target_price,
                entry_price: entry.price,
                stop_price,
                target_price,
                lowest_price,
                highest_price,
                adverse_price,
                favorable_price: target_price,
                max_adverse_points,
                max_favorable_points: (risk.abs() * route.target_r).max(max_favorable_points),
                risk_points: risk.abs(),
                trade_direction: entry.direction,
                trade_result: 1,
                exit_reason: "target".to_string(),
            });
        }
    }

    let exit_index = end_index - 1;
    let exit_close = candles[exit_index].close;
    let result_r = ((exit_close - entry.price) * entry.direction) / risk;
    Some(Phase1TradeDetail {
        result_r,
        entry_date: candles[entry.index].candle_date,
        exit_date: candles[exit_index].candle_date,
        exit_price: exit_close,
        entry_price: entry.price,
        stop_price,
        target_price,
        lowest_price,
        highest_price,
        adverse_price,
        favorable_price,
        max_adverse_points,
        max_favorable_points,
        risk_points: risk.abs(),
        trade_direction: entry.direction,
        trade_result: if result_r > 0.0 { 1 } else { 2 },
        exit_reason: "time".to_string(),
    })
}

fn trade_row_from_detail(
    run_id: &str,
    family_key: &str,
    source_scope: &str,
    period_year: i64,
    route: &Phase1RouteSpec,
    setup: &Phase1SetupSource,
    detail: &Phase1TradeDetail,
) -> Phase1TradeRow {
    let trade_uid = format!("{run_id}::{}::{}", route.route_id, setup.setup_id);
    Phase1TradeRow {
        trade_uid,
        run_id: run_id.to_string(),
        family_key: family_key.to_string(),
        source_scope: source_scope.to_string(),
        period_year,
        route_id: route.route_id.clone(),
        route_label: route.route_label.clone(),
        entry_mode: route.entry_mode.clone(),
        stop_mode: route.stop_mode.clone(),
        target_r: route.target_r,
        max_hold_multiple: route.max_hold_multiple,
        setup_id: setup.setup_id.clone(),
        pattern_id: setup.pattern_id.clone(),
        pattern_group_id: setup.pattern_group_id.clone(),
        symbol: setup.symbol.clone(),
        market: setup.market.clone(),
        d_date: setup.d_date,
        d_confirm_date: setup.d_confirm_date,
        entry_date: detail.entry_date,
        exit_date: detail.exit_date,
        entry_price: detail.entry_price,
        stop_price: detail.stop_price,
        target_price: detail.target_price,
        exit_price: detail.exit_price,
        result_r: detail.result_r,
        risk_points: detail.risk_points,
        trade_direction: if detail.trade_direction > 0.0 {
            "long".to_string()
        } else {
            "short".to_string()
        },
        trade_result: detail.trade_result,
        exit_reason: detail.exit_reason.clone(),
        lowest_price: detail.lowest_price,
        highest_price: detail.highest_price,
        adverse_price: detail.adverse_price,
        favorable_price: detail.favorable_price,
        max_adverse_points: detail.max_adverse_points,
        max_favorable_points: detail.max_favorable_points,
    }
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
    trade_rows: &[Phase1TradeRow],
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

    if !trade_rows.is_empty() {
        let mut trade_builder = QueryBuilder::<MySql>::new(
            r#"
            INSERT INTO phase1_strategy_trades (
                trade_uid,
                run_id,
                family_key,
                source_scope,
                period_year,
                route_id,
                route_label,
                entry_mode,
                stop_mode,
                target_r,
                max_hold_multiple,
                setup_id,
                pattern_id,
                pattern_group_id,
                symbol,
                market,
                d_date,
                d_confirm_date,
                entry_date,
                exit_date,
                entry_price,
                stop_price,
                target_price,
                exit_price,
                result_r,
                risk_points,
                trade_direction,
                trade_result,
                exit_reason,
                lowest_price,
                highest_price,
                adverse_price,
                favorable_price,
                max_adverse_points,
                max_favorable_points
            )
            "#,
        );

        trade_builder.push_values(trade_rows, |mut row, trade| {
            row.push_bind(&trade.trade_uid)
                .push_bind(&trade.run_id)
                .push_bind(&trade.family_key)
                .push_bind(&trade.source_scope)
                .push_bind(trade.period_year)
                .push_bind(&trade.route_id)
                .push_bind(&trade.route_label)
                .push_bind(&trade.entry_mode)
                .push_bind(&trade.stop_mode)
                .push_bind(trade.target_r)
                .push_bind(trade.max_hold_multiple)
                .push_bind(&trade.setup_id)
                .push_bind(&trade.pattern_id)
                .push_bind(&trade.pattern_group_id)
                .push_bind(&trade.symbol)
                .push_bind(&trade.market)
                .push_bind(trade.d_date)
                .push_bind(trade.d_confirm_date)
                .push_bind(trade.entry_date)
                .push_bind(trade.exit_date)
                .push_bind(trade.entry_price)
                .push_bind(trade.stop_price)
                .push_bind(trade.target_price)
                .push_bind(trade.exit_price)
                .push_bind(trade.result_r)
                .push_bind(trade.risk_points)
                .push_bind(&trade.trade_direction)
                .push_bind(trade.trade_result)
                .push_bind(&trade.exit_reason)
                .push_bind(trade.lowest_price)
                .push_bind(trade.highest_price)
                .push_bind(trade.adverse_price)
                .push_bind(trade.favorable_price)
                .push_bind(trade.max_adverse_points)
                .push_bind(trade.max_favorable_points);
        });
        trade_builder.build().execute(&mut *tx).await?;
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

async fn run_family(
    pool: &MySqlPool,
    family: &PatternFamilySummary,
    args: &Args,
    routes: &[Phase1RouteSpec],
) -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let run_id = run_id();

    println!(
        "Phase 1 test run {run_id}: family={} {} / {} / {} / {} / {}, source={}, year={}, family setups={}, max setups={}, forward bars=5x pattern length, tests={}",
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
        routes.len()
    );

    let setups = fetch_setups(
        pool,
        family,
        &args.source_scope,
        args.period_year,
        args.max_setups,
    )
    .await?;
    if setups.is_empty() {
        println!("No setups matched family {}.", family.family_key);
        return Ok(());
    }

    let mut accumulators = routes
        .iter()
        .cloned()
        .map(Phase1RouteAccumulator::new)
        .collect::<Vec<_>>();
    let mut trade_rows = Vec::new();

    for (index, setup) in setups.iter().enumerate() {
        let setup_forward_bars = setup.full_pattern_length.saturating_mul(5).max(1);
        let candles = fetch_forward_candles(pool, setup, setup_forward_bars)
            .await
            .unwrap_or_else(|error| {
                eprintln!(
                    "Candle fetch failed for setup {} ({}): {:?}",
                    setup.setup_id, setup.symbol, error
                );
                Vec::new()
            });
        let year = setup.d_confirm_date.year();

        for accumulator in &mut accumulators {
            if candles.is_empty() {
                accumulator.record_no_entry(year);
                continue;
            }

            match replay_route_detail(&accumulator.spec, setup, &candles) {
                Some(detail) if detail.result_r.is_finite() => {
                    accumulator.record_trade(year, detail.result_r);
                    trade_rows.push(trade_row_from_detail(
                        &run_id,
                        &family.family_key,
                        &args.source_scope,
                        args.period_year,
                        &accumulator.spec,
                        setup,
                        &detail,
                    ));
                }
                _ => accumulator.record_no_entry(year),
            }
        }

        if (index + 1) % 100 == 0 || index + 1 == setups.len() {
            println!(
                "Family {} processed {}/{} setups",
                family.family_key,
                index + 1,
                setups.len()
            );
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
        pool,
        &run_id,
        family,
        args,
        setups.len() as i64,
        results.len() as i64,
        elapsed_ms,
        &results,
        &yearly_results,
        &trade_rows,
    )
    .await?;

    println!(
        "Stored {} Entry / Exit test results, {} yearly rows, and {} trade rows for family {} in {:.2}s",
        results.len(),
        yearly_results.len(),
        trade_rows.len(),
        family.family_key,
        elapsed_ms as f64 / 1000.0
    );
    print_top_results(&results);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let args = parse_args()?;
    let pool = MySqlPool::connect(&database_url_from_env()?).await?;
    ensure_phase1_tables(&pool).await?;
    let routes = fetch_entry_exit_tests(&pool, args.test_id.as_deref()).await?;
    println!(
        "Testing {} enabled Entry / Exit definition(s). Forward window: full_pattern_length * 5.",
        routes.len()
    );

    let families = if args.all_families {
        fetch_families(
            &pool,
            &args.source_scope,
            args.period_year,
            args.family_limit,
        )
        .await?
    } else {
        let family_key = args.family_key.as_deref().unwrap_or_default();
        vec![
            fetch_family(&pool, family_key, &args.source_scope, args.period_year)
                .await?
                .ok_or_else(|| {
                    format!(
                        "Family {} not found for source={} year={}",
                        family_key, args.source_scope, args.period_year
                    )
                })?,
        ]
    };

    println!(
        "Running Entry / Exit tests across {} family/families.",
        families.len()
    );
    for (index, family) in families.iter().enumerate() {
        println!("Family batch {}/{}", index + 1, families.len());
        run_family(&pool, family, &args, &routes).await?;
    }

    Ok(())
}
