use std::collections::{HashMap, HashSet};
use std::env;

use chrono::{NaiveDate, NaiveDateTime, Timelike};
use sqlx::{mysql::MySqlPool, Row};

#[derive(Debug)]
struct Args {
    sim_run_id: String,
    playbook_id: Option<String>,
}

#[derive(Clone)]
struct ReplayTrade {
    event_date: NaiveDateTime,
    symbol: String,
    root_symbol: String,
    family_key: String,
    template_uid: String,
    outcome: String,
    result_r: f64,
}

#[derive(Default)]
struct PropSummary {
    cycles: i64,
    passed: i64,
    daily_fails: i64,
    drawdown_fails: i64,
    incomplete: i64,
    pass_rate: f64,
    closed_pass_rate: f64,
    max_drawdown_r: f64,
    max_loss_streak: i64,
}

struct PropCycleRow {
    cycle_number: i64,
    start_date: NaiveDate,
    end_date: NaiveDate,
    outcome: String,
    trades: i64,
    wins: i64,
    losses: i64,
    no_entries: i64,
    sum_r: f64,
    max_drawdown_r: f64,
    worst_day_r: f64,
}

struct EquityPointRow {
    point_index: i64,
    event_date: NaiveDateTime,
    outcome: String,
    result_r: f64,
    cumulative_r: f64,
    drawdown_r: f64,
    daily_r: f64,
}

struct DailyRRow {
    trade_date: NaiveDate,
    total_r: f64,
    trades: i64,
    wins: i64,
    losses: i64,
    no_entries: i64,
    best_trade_r: f64,
    worst_trade_r: f64,
    worst_intraday_r: f64,
    hit_daily_loss: bool,
}

#[derive(Default)]
struct TradeCadenceRow {
    first_trade_at: Option<NaiveDateTime>,
    last_trade_at: Option<NaiveDateTime>,
    trades: i64,
    trade_days: i64,
    gap_count: i64,
    avg_gap_minutes: f64,
    median_gap_minutes: f64,
    min_gap_minutes: f64,
    max_gap_minutes: f64,
    avg_trades_per_day: f64,
    max_trades_per_day: i64,
    max_trades_per_hour: i64,
    max_trades_5m_window: i64,
    max_trades_15m_window: i64,
    gap_0_1m: i64,
    gap_1_5m: i64,
    gap_5_15m: i64,
    gap_15_30m: i64,
    gap_30_60m: i64,
    gap_over_60m: i64,
}

#[derive(Default)]
struct LossClusterSummaryRow {
    losses: i64,
    loss_gap_count: i64,
    avg_loss_gap_minutes: f64,
    median_loss_gap_minutes: f64,
    min_loss_gap_minutes: f64,
    max_loss_gap_minutes: f64,
    clustered_60m_loss_pairs: i64,
    same_day_loss_pairs: i64,
    clustered_60m_rate: f64,
    loss_days: i64,
    loss_days_5_plus: i64,
    worst_loss_day: Option<NaiveDate>,
    worst_loss_day_losses: i64,
    worst_loss_day_r: f64,
    worst_loss_hour_date: Option<NaiveDate>,
    worst_loss_hour: Option<i64>,
    worst_loss_hour_losses: i64,
    worst_loss_hour_r: f64,
    max_loss_streak: i64,
}

struct LossGapBucketRow {
    bucket_key: String,
    bucket_label: String,
    sort_order: i64,
    gap_count: i64,
    gap_percent: f64,
}

struct LossWindowRow {
    trade_date: NaiveDate,
    entry_hour: i64,
    trades: i64,
    wins: i64,
    losses: i64,
    no_entries: i64,
    total_r: f64,
    loss_rate: f64,
    top_root_symbol: String,
    top_family_key: String,
    top_template_uid: String,
}

struct LossWindowAccumulator {
    trade_date: NaiveDate,
    entry_hour: i64,
    trades: i64,
    wins: i64,
    losses: i64,
    no_entries: i64,
    total_r: f64,
    root_counts: HashMap<String, i64>,
    family_counts: HashMap<String, i64>,
    template_counts: HashMap<String, i64>,
}

#[derive(Default)]
struct HourlyPerformanceRow {
    entry_hour: i64,
    trades: i64,
    wins: i64,
    losses: i64,
    no_entries: i64,
    sum_r: f64,
    best_r: f64,
    worst_r: f64,
    daily_loss_day_trades: i64,
    daily_loss_days: HashSet<NaiveDate>,
}

#[derive(Default)]
struct ContributionRow {
    key: String,
    trades: i64,
    wins: i64,
    losses: i64,
    no_entries: i64,
    sum_r: f64,
    best_r: f64,
    worst_r: f64,
    daily_loss_day_trades: i64,
    daily_loss_days: HashSet<NaiveDate>,
    root_symbols: HashSet<String>,
    contract_symbols: HashSet<String>,
    family_keys: HashSet<String>,
    template_uids: HashSet<String>,
}

struct StreakRow {
    streak_number: i64,
    streak_type: String,
    start_index: i64,
    end_index: i64,
    start_date: NaiveDateTime,
    end_date: NaiveDateTime,
    streak_length: i64,
    sum_r: f64,
}

struct OpenCycle {
    cycle_number: i64,
    start_date: NaiveDate,
    current_date: NaiveDate,
    trades: i64,
    wins: i64,
    losses: i64,
    no_entries: i64,
    equity_r: f64,
    peak_r: f64,
    max_drawdown_r: f64,
    daily_r: f64,
    worst_day_r: f64,
}

fn root_symbol_from_contract(symbol: &str) -> String {
    let chars = symbol.chars().collect::<Vec<_>>();
    for index in 0..chars.len().saturating_sub(1) {
        let is_contract_month = matches!(
            chars[index],
            'F' | 'G' | 'H' | 'J' | 'K' | 'M' | 'N' | 'Q' | 'U' | 'V' | 'X' | 'Z'
        );
        if is_contract_month && chars[index + 1].is_ascii_digit() && index > 0 {
            return chars[..index].iter().collect::<String>();
        }
    }
    symbol
        .trim_end_matches(|ch: char| ch.is_ascii_digit())
        .to_string()
}

fn normalize_root_symbol(value: &str) -> String {
    value.trim().to_uppercase()
}

impl OpenCycle {
    fn new(cycle_number: i64, date: NaiveDate) -> Self {
        Self {
            cycle_number,
            start_date: date,
            current_date: date,
            trades: 0,
            wins: 0,
            losses: 0,
            no_entries: 0,
            equity_r: 0.0,
            peak_r: 0.0,
            max_drawdown_r: 0.0,
            daily_r: 0.0,
            worst_day_r: 0.0,
        }
    }

    fn apply_trade(&mut self, trade: &ReplayTrade) {
        let date = trade.event_date.date();
        if self.current_date != date {
            self.current_date = date;
            self.daily_r = 0.0;
        }

        self.trades += 1;
        if trade.outcome == "pass" {
            self.wins += 1;
        } else if trade.outcome == "fail" {
            self.losses += 1;
        } else {
            self.no_entries += 1;
        }

        self.equity_r += trade.result_r;
        self.daily_r += trade.result_r;
        self.peak_r = self.peak_r.max(self.equity_r);
        self.max_drawdown_r = self.max_drawdown_r.max(self.peak_r - self.equity_r);
        self.worst_day_r = self.worst_day_r.min(self.daily_r);
    }

    fn finish(&self, outcome: &str) -> PropCycleRow {
        PropCycleRow {
            cycle_number: self.cycle_number,
            start_date: self.start_date,
            end_date: self.current_date,
            outcome: outcome.to_string(),
            trades: self.trades,
            wins: self.wins,
            losses: self.losses,
            no_entries: self.no_entries,
            sum_r: self.equity_r,
            max_drawdown_r: self.max_drawdown_r,
            worst_day_r: self.worst_day_r,
        }
    }
}

fn usage() -> &'static str {
    "Usage: cargo run --bin store_entry_exit_playbook_sim_prop -- --sim-run-id SIM_RUN_ID [--playbook-id PLAYBOOK_ID]\n\nStores prop-firm replay summary/cycles for an existing entry/exit playbook simulation run."
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|items| items[0] == name)
        .map(|items| items[1].clone())
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let raw_args = env::args().skip(1).collect::<Vec<_>>();
    if raw_args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", usage());
        std::process::exit(0);
    }

    let sim_run_id = arg_value(&raw_args, "--sim-run-id")
        .or_else(|| arg_value(&raw_args, "--router-run-id"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or("Missing required --sim-run-id")?;

    let playbook_id = arg_value(&raw_args, "--playbook-id")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    Ok(Args {
        sim_run_id,
        playbook_id,
    })
}

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

async fn ensure_column(
    pool: &MySqlPool,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), sqlx::Error> {
    let exists = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM INFORMATION_SCHEMA.COLUMNS
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_NAME = ?
          AND COLUMN_NAME = ?
        "#,
    )
    .bind(table)
    .bind(column)
    .fetch_one(pool)
    .await?;

    if exists == 0 {
        sqlx::query(&format!(
            "ALTER TABLE `{table}` ADD COLUMN `{column}` {definition}"
        ))
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn ensure_tables(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_playbook_sim_runs (
            sim_run_id VARCHAR(64) NOT NULL PRIMARY KEY,
            playbook_id VARCHAR(64) NULL,
            build_id VARCHAR(64) NOT NULL,
            source_scope VARCHAR(32) NOT NULL,
            source_timeframe VARCHAR(16) NULL,
            test_year BIGINT NOT NULL,
            min_train_tests BIGINT NOT NULL DEFAULT 0,
            sister_window_minutes BIGINT NOT NULL DEFAULT 0,
            families_selected BIGINT NOT NULL DEFAULT 0,
            patterns_scanned BIGINT NOT NULL DEFAULT 0,
            routed_patterns BIGINT NOT NULL DEFAULT 0,
            no_route_patterns BIGINT NOT NULL DEFAULT 0,
            skipped_non_trade_patterns BIGINT NOT NULL DEFAULT 0,
            skipped_symbol_patterns BIGINT NOT NULL DEFAULT 0,
            skipped_overlap_patterns BIGINT NOT NULL DEFAULT 0,
            trade_choices BIGINT NOT NULL DEFAULT 0,
            watchlist_choices BIGINT NOT NULL DEFAULT 0,
            skip_choices BIGINT NOT NULL DEFAULT 0,
            manual_family_bans_applied BIGINT NOT NULL DEFAULT 0,
            symbol_filter_enabled TINYINT(1) NOT NULL DEFAULT 0,
            one_trade_at_a_time TINYINT(1) NOT NULL DEFAULT 0,
            one_trade_per_root_symbol TINYINT(1) NOT NULL DEFAULT 0,
            one_trade_per_minute TINYINT(1) NOT NULL DEFAULT 0,
            trade_cooldown_minutes BIGINT NOT NULL DEFAULT 0,
            daily_loss_lockout TINYINT(1) NOT NULL DEFAULT 0,
            near_pass_protection TINYINT(1) NOT NULL DEFAULT 0,
            near_pass_within_r DOUBLE NOT NULL DEFAULT 0,
            near_pass_daily_loss_r DOUBLE NOT NULL DEFAULT 0,
            loss_cluster_day_lockout TINYINT(1) NOT NULL DEFAULT 0,
            loss_cluster_loss_count BIGINT NOT NULL DEFAULT 0,
            loss_cluster_window_minutes BIGINT NOT NULL DEFAULT 0,
            playbook_description TEXT NULL,
            symbol_trade_roots BIGINT NOT NULL DEFAULT 0,
            symbol_skip_roots BIGINT NOT NULL DEFAULT 0,
            symbol_min_tests BIGINT NOT NULL DEFAULT 0,
            symbol_min_win_rate DOUBLE NOT NULL DEFAULT 0,
            symbol_min_avg_r DOUBLE NOT NULL DEFAULT 0,
            prop_filter_enabled TINYINT(1) NOT NULL DEFAULT 0,
            trade_min_tests BIGINT NOT NULL DEFAULT 0,
            trade_min_win_rate DOUBLE NOT NULL DEFAULT 0,
            trade_min_avg_r DOUBLE NOT NULL DEFAULT 0,
            watchlist_min_tests BIGINT NOT NULL DEFAULT 0,
            watchlist_min_win_rate DOUBLE NOT NULL DEFAULT 0,
            watchlist_min_avg_r DOUBLE NOT NULL DEFAULT 0,
            win_count BIGINT NOT NULL DEFAULT 0,
            loss_count BIGINT NOT NULL DEFAULT 0,
            no_entry_count BIGINT NOT NULL DEFAULT 0,
            avg_r DOUBLE NOT NULL DEFAULT 0,
            sum_r DOUBLE NOT NULL DEFAULT 0,
            best_r DOUBLE NOT NULL DEFAULT 0,
            worst_r DOUBLE NOT NULL DEFAULT 0,
            elapsed_ms BIGINT NOT NULL DEFAULT 0,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            INDEX idx_entry_exit_playbook_sim_runs_build (build_id, test_year, created_at),
            INDEX idx_entry_exit_playbook_sim_runs_playbook (playbook_id, test_year, created_at)
        )
        "#,
    )
    .execute(pool)
    .await?;

    ensure_column(
        pool,
        "entry_exit_playbook_sim_runs",
        "one_trade_per_minute",
        "TINYINT(1) NOT NULL DEFAULT 0 AFTER one_trade_per_root_symbol",
    )
    .await?;
    ensure_column(
        pool,
        "entry_exit_playbook_sim_runs",
        "trade_cooldown_minutes",
        "BIGINT NOT NULL DEFAULT 0 AFTER one_trade_per_minute",
    )
    .await?;
    ensure_column(
        pool,
        "entry_exit_playbook_sim_runs",
        "daily_loss_lockout",
        "TINYINT(1) NOT NULL DEFAULT 0 AFTER trade_cooldown_minutes",
    )
    .await?;
    ensure_column(
        pool,
        "entry_exit_playbook_sim_runs",
        "near_pass_protection",
        "TINYINT(1) NOT NULL DEFAULT 0 AFTER daily_loss_lockout",
    )
    .await?;
    ensure_column(
        pool,
        "entry_exit_playbook_sim_runs",
        "near_pass_within_r",
        "DOUBLE NOT NULL DEFAULT 0 AFTER near_pass_protection",
    )
    .await?;
    ensure_column(
        pool,
        "entry_exit_playbook_sim_runs",
        "near_pass_daily_loss_r",
        "DOUBLE NOT NULL DEFAULT 0 AFTER near_pass_within_r",
    )
    .await?;
    ensure_column(
        pool,
        "entry_exit_playbook_sim_runs",
        "loss_cluster_day_lockout",
        "TINYINT(1) NOT NULL DEFAULT 0 AFTER near_pass_daily_loss_r",
    )
    .await?;
    ensure_column(
        pool,
        "entry_exit_playbook_sim_runs",
        "loss_cluster_loss_count",
        "BIGINT NOT NULL DEFAULT 0 AFTER loss_cluster_day_lockout",
    )
    .await?;
    ensure_column(
        pool,
        "entry_exit_playbook_sim_runs",
        "loss_cluster_window_minutes",
        "BIGINT NOT NULL DEFAULT 0 AFTER loss_cluster_loss_count",
    )
    .await?;
    ensure_column(
        pool,
        "entry_exit_playbook_sim_runs",
        "playbook_description",
        "TEXT NULL AFTER loss_cluster_window_minutes",
    )
    .await?;
    ensure_column(
        pool,
        "entry_exit_template_family_router_runs",
        "trade_cooldown_minutes",
        "BIGINT NOT NULL DEFAULT 0 AFTER one_trade_per_minute",
    )
    .await?;
    ensure_column(
        pool,
        "entry_exit_template_family_router_runs",
        "daily_loss_lockout",
        "TINYINT NOT NULL DEFAULT 0 AFTER trade_cooldown_minutes",
    )
    .await?;
    ensure_column(
        pool,
        "entry_exit_template_family_router_runs",
        "near_pass_protection",
        "TINYINT NOT NULL DEFAULT 0 AFTER daily_loss_lockout",
    )
    .await?;
    ensure_column(
        pool,
        "entry_exit_template_family_router_runs",
        "near_pass_within_r",
        "DOUBLE NOT NULL DEFAULT 0 AFTER near_pass_protection",
    )
    .await?;
    ensure_column(
        pool,
        "entry_exit_template_family_router_runs",
        "near_pass_daily_loss_r",
        "DOUBLE NOT NULL DEFAULT 0 AFTER near_pass_within_r",
    )
    .await?;
    ensure_column(
        pool,
        "entry_exit_template_family_router_runs",
        "loss_cluster_day_lockout",
        "TINYINT NOT NULL DEFAULT 0 AFTER near_pass_daily_loss_r",
    )
    .await?;
    ensure_column(
        pool,
        "entry_exit_template_family_router_runs",
        "loss_cluster_loss_count",
        "BIGINT NOT NULL DEFAULT 0 AFTER loss_cluster_day_lockout",
    )
    .await?;
    ensure_column(
        pool,
        "entry_exit_template_family_router_runs",
        "loss_cluster_window_minutes",
        "BIGINT NOT NULL DEFAULT 0 AFTER loss_cluster_loss_count",
    )
    .await?;
    ensure_column(
        pool,
        "entry_exit_template_family_router_runs",
        "playbook_description",
        "TEXT NULL AFTER loss_cluster_window_minutes",
    )
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_playbook_sim_prop_summary (
            sim_run_id VARCHAR(64) NOT NULL PRIMARY KEY,
            playbook_id VARCHAR(64) NULL,
            build_id VARCHAR(64) NOT NULL,
            profit_target_r DOUBLE NOT NULL DEFAULT 30,
            max_drawdown_r_limit DOUBLE NOT NULL DEFAULT 20,
            daily_loss_r_limit DOUBLE NOT NULL DEFAULT 10,
            cycles BIGINT NOT NULL DEFAULT 0,
            passed BIGINT NOT NULL DEFAULT 0,
            daily_fails BIGINT NOT NULL DEFAULT 0,
            drawdown_fails BIGINT NOT NULL DEFAULT 0,
            incomplete BIGINT NOT NULL DEFAULT 0,
            pass_rate DOUBLE NOT NULL DEFAULT 0,
            closed_pass_rate DOUBLE NOT NULL DEFAULT 0,
            max_drawdown_r DOUBLE NOT NULL DEFAULT 0,
            max_loss_streak BIGINT NOT NULL DEFAULT 0,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            INDEX idx_eepb_sim_prop_summary_playbook (playbook_id),
            INDEX idx_eepb_sim_prop_summary_build (build_id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_playbook_sim_prop_cycles (
            sim_run_id VARCHAR(64) NOT NULL,
            cycle_number BIGINT NOT NULL,
            playbook_id VARCHAR(64) NULL,
            build_id VARCHAR(64) NOT NULL,
            outcome VARCHAR(32) NOT NULL,
            start_date DATE NOT NULL,
            end_date DATE NOT NULL,
            trades BIGINT NOT NULL DEFAULT 0,
            wins BIGINT NOT NULL DEFAULT 0,
            losses BIGINT NOT NULL DEFAULT 0,
            no_entries BIGINT NOT NULL DEFAULT 0,
            sum_r DOUBLE NOT NULL DEFAULT 0,
            max_drawdown_r DOUBLE NOT NULL DEFAULT 0,
            worst_day_r DOUBLE NOT NULL DEFAULT 0,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (sim_run_id, cycle_number),
            INDEX idx_eepb_sim_prop_cycles_playbook (playbook_id),
            INDEX idx_eepb_sim_prop_cycles_build (build_id),
            INDEX idx_eepb_sim_prop_cycles_outcome (outcome)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_playbook_sim_equity_points (
            sim_run_id VARCHAR(64) NOT NULL,
            point_index BIGINT NOT NULL,
            playbook_id VARCHAR(64) NULL,
            build_id VARCHAR(64) NOT NULL,
            event_date DATETIME NOT NULL,
            outcome VARCHAR(16) NOT NULL,
            result_r DOUBLE NOT NULL DEFAULT 0,
            cumulative_r DOUBLE NOT NULL DEFAULT 0,
            drawdown_r DOUBLE NOT NULL DEFAULT 0,
            daily_r DOUBLE NOT NULL DEFAULT 0,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (sim_run_id, point_index),
            INDEX idx_entry_exit_sim_equity_playbook (playbook_id, point_index),
            INDEX idx_entry_exit_sim_equity_build (build_id, event_date)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_playbook_sim_daily_r (
            sim_run_id VARCHAR(64) NOT NULL,
            trade_date DATE NOT NULL,
            playbook_id VARCHAR(64) NULL,
            build_id VARCHAR(64) NOT NULL,
            total_r DOUBLE NOT NULL DEFAULT 0,
            trades BIGINT NOT NULL DEFAULT 0,
            wins BIGINT NOT NULL DEFAULT 0,
            losses BIGINT NOT NULL DEFAULT 0,
            no_entries BIGINT NOT NULL DEFAULT 0,
            best_trade_r DOUBLE NOT NULL DEFAULT 0,
            worst_trade_r DOUBLE NOT NULL DEFAULT 0,
            worst_intraday_r DOUBLE NOT NULL DEFAULT 0,
            hit_daily_loss TINYINT(1) NOT NULL DEFAULT 0,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (sim_run_id, trade_date),
            INDEX idx_entry_exit_sim_daily_r_playbook (playbook_id, trade_date),
            INDEX idx_entry_exit_sim_daily_r_build (build_id, trade_date),
            INDEX idx_entry_exit_sim_daily_r_loss (sim_run_id, hit_daily_loss, worst_intraday_r)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_playbook_sim_hourly (
            sim_run_id VARCHAR(64) NOT NULL,
            entry_hour BIGINT NOT NULL,
            playbook_id VARCHAR(64) NULL,
            build_id VARCHAR(64) NOT NULL,
            trades BIGINT NOT NULL DEFAULT 0,
            wins BIGINT NOT NULL DEFAULT 0,
            losses BIGINT NOT NULL DEFAULT 0,
            no_entries BIGINT NOT NULL DEFAULT 0,
            win_rate DOUBLE NOT NULL DEFAULT 0,
            avg_r DOUBLE NOT NULL DEFAULT 0,
            sum_r DOUBLE NOT NULL DEFAULT 0,
            best_r DOUBLE NOT NULL DEFAULT 0,
            worst_r DOUBLE NOT NULL DEFAULT 0,
            daily_loss_day_trades BIGINT NOT NULL DEFAULT 0,
            daily_loss_day_count BIGINT NOT NULL DEFAULT 0,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (sim_run_id, entry_hour),
            INDEX idx_entry_exit_sim_hourly_playbook (playbook_id, entry_hour),
            INDEX idx_entry_exit_sim_hourly_build (build_id, entry_hour),
            INDEX idx_entry_exit_sim_hourly_loss (sim_run_id, daily_loss_day_trades)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_playbook_sim_trade_cadence (
            sim_run_id VARCHAR(64) NOT NULL PRIMARY KEY,
            playbook_id VARCHAR(64) NULL,
            build_id VARCHAR(64) NOT NULL,
            first_trade_at DATETIME NULL,
            last_trade_at DATETIME NULL,
            trades BIGINT NOT NULL DEFAULT 0,
            trade_days BIGINT NOT NULL DEFAULT 0,
            gap_count BIGINT NOT NULL DEFAULT 0,
            avg_gap_minutes DOUBLE NOT NULL DEFAULT 0,
            median_gap_minutes DOUBLE NOT NULL DEFAULT 0,
            min_gap_minutes DOUBLE NOT NULL DEFAULT 0,
            max_gap_minutes DOUBLE NOT NULL DEFAULT 0,
            avg_trades_per_day DOUBLE NOT NULL DEFAULT 0,
            max_trades_per_day BIGINT NOT NULL DEFAULT 0,
            max_trades_per_hour BIGINT NOT NULL DEFAULT 0,
            max_trades_5m_window BIGINT NOT NULL DEFAULT 0,
            max_trades_15m_window BIGINT NOT NULL DEFAULT 0,
            gap_0_1m BIGINT NOT NULL DEFAULT 0,
            gap_1_5m BIGINT NOT NULL DEFAULT 0,
            gap_5_15m BIGINT NOT NULL DEFAULT 0,
            gap_15_30m BIGINT NOT NULL DEFAULT 0,
            gap_30_60m BIGINT NOT NULL DEFAULT 0,
            gap_over_60m BIGINT NOT NULL DEFAULT 0,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            INDEX idx_entry_exit_sim_trade_cadence_playbook (playbook_id, median_gap_minutes),
            INDEX idx_entry_exit_sim_trade_cadence_build (build_id, median_gap_minutes)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_playbook_sim_loss_summary (
            sim_run_id VARCHAR(64) NOT NULL PRIMARY KEY,
            playbook_id VARCHAR(64) NULL,
            build_id VARCHAR(64) NOT NULL,
            losses BIGINT NOT NULL DEFAULT 0,
            loss_gap_count BIGINT NOT NULL DEFAULT 0,
            avg_loss_gap_minutes DOUBLE NOT NULL DEFAULT 0,
            median_loss_gap_minutes DOUBLE NOT NULL DEFAULT 0,
            min_loss_gap_minutes DOUBLE NOT NULL DEFAULT 0,
            max_loss_gap_minutes DOUBLE NOT NULL DEFAULT 0,
            clustered_60m_loss_pairs BIGINT NOT NULL DEFAULT 0,
            same_day_loss_pairs BIGINT NOT NULL DEFAULT 0,
            clustered_60m_rate DOUBLE NOT NULL DEFAULT 0,
            loss_days BIGINT NOT NULL DEFAULT 0,
            loss_days_5_plus BIGINT NOT NULL DEFAULT 0,
            worst_loss_day DATE NULL,
            worst_loss_day_losses BIGINT NOT NULL DEFAULT 0,
            worst_loss_day_r DOUBLE NOT NULL DEFAULT 0,
            worst_loss_hour_date DATE NULL,
            worst_loss_hour BIGINT NULL,
            worst_loss_hour_losses BIGINT NOT NULL DEFAULT 0,
            worst_loss_hour_r DOUBLE NOT NULL DEFAULT 0,
            max_loss_streak BIGINT NOT NULL DEFAULT 0,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            INDEX idx_entry_exit_sim_loss_summary_playbook (playbook_id, clustered_60m_rate),
            INDEX idx_entry_exit_sim_loss_summary_build (build_id, clustered_60m_rate)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_playbook_sim_loss_gap_buckets (
            sim_run_id VARCHAR(64) NOT NULL,
            bucket_key VARCHAR(32) NOT NULL,
            playbook_id VARCHAR(64) NULL,
            build_id VARCHAR(64) NOT NULL,
            bucket_label VARCHAR(32) NOT NULL,
            sort_order BIGINT NOT NULL DEFAULT 0,
            gap_count BIGINT NOT NULL DEFAULT 0,
            gap_percent DOUBLE NOT NULL DEFAULT 0,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (sim_run_id, bucket_key),
            INDEX idx_entry_exit_sim_loss_gap_playbook (playbook_id, sort_order),
            INDEX idx_entry_exit_sim_loss_gap_build (build_id, sort_order)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_playbook_sim_loss_windows (
            sim_run_id VARCHAR(64) NOT NULL,
            trade_date DATE NOT NULL,
            entry_hour BIGINT NOT NULL,
            playbook_id VARCHAR(64) NULL,
            build_id VARCHAR(64) NOT NULL,
            trades BIGINT NOT NULL DEFAULT 0,
            wins BIGINT NOT NULL DEFAULT 0,
            losses BIGINT NOT NULL DEFAULT 0,
            no_entries BIGINT NOT NULL DEFAULT 0,
            total_r DOUBLE NOT NULL DEFAULT 0,
            loss_rate DOUBLE NOT NULL DEFAULT 0,
            top_root_symbol VARCHAR(32) NOT NULL DEFAULT '',
            top_family_key VARCHAR(64) NOT NULL DEFAULT '',
            top_template_uid VARCHAR(128) NOT NULL DEFAULT '',
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (sim_run_id, trade_date, entry_hour),
            INDEX idx_entry_exit_sim_loss_windows_playbook (playbook_id, losses),
            INDEX idx_entry_exit_sim_loss_windows_build (build_id, losses)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_playbook_sim_symbol_contribution (
            sim_run_id VARCHAR(64) NOT NULL,
            root_symbol VARCHAR(32) NOT NULL,
            playbook_id VARCHAR(64) NULL,
            build_id VARCHAR(64) NOT NULL,
            trades BIGINT NOT NULL DEFAULT 0,
            wins BIGINT NOT NULL DEFAULT 0,
            losses BIGINT NOT NULL DEFAULT 0,
            no_entries BIGINT NOT NULL DEFAULT 0,
            win_rate DOUBLE NOT NULL DEFAULT 0,
            avg_r DOUBLE NOT NULL DEFAULT 0,
            sum_r DOUBLE NOT NULL DEFAULT 0,
            best_r DOUBLE NOT NULL DEFAULT 0,
            worst_r DOUBLE NOT NULL DEFAULT 0,
            daily_loss_day_trades BIGINT NOT NULL DEFAULT 0,
            daily_loss_day_count BIGINT NOT NULL DEFAULT 0,
            family_count BIGINT NOT NULL DEFAULT 0,
            template_count BIGINT NOT NULL DEFAULT 0,
            contract_count BIGINT NOT NULL DEFAULT 0,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (sim_run_id, root_symbol),
            INDEX idx_entry_exit_sim_symbol_contribution_playbook (playbook_id, sum_r),
            INDEX idx_entry_exit_sim_symbol_contribution_build (build_id, sum_r)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_playbook_sim_family_contribution (
            sim_run_id VARCHAR(64) NOT NULL,
            family_key VARCHAR(64) NOT NULL,
            playbook_id VARCHAR(64) NULL,
            build_id VARCHAR(64) NOT NULL,
            trades BIGINT NOT NULL DEFAULT 0,
            wins BIGINT NOT NULL DEFAULT 0,
            losses BIGINT NOT NULL DEFAULT 0,
            no_entries BIGINT NOT NULL DEFAULT 0,
            win_rate DOUBLE NOT NULL DEFAULT 0,
            avg_r DOUBLE NOT NULL DEFAULT 0,
            sum_r DOUBLE NOT NULL DEFAULT 0,
            best_r DOUBLE NOT NULL DEFAULT 0,
            worst_r DOUBLE NOT NULL DEFAULT 0,
            daily_loss_day_trades BIGINT NOT NULL DEFAULT 0,
            daily_loss_day_count BIGINT NOT NULL DEFAULT 0,
            symbol_count BIGINT NOT NULL DEFAULT 0,
            template_count BIGINT NOT NULL DEFAULT 0,
            contract_count BIGINT NOT NULL DEFAULT 0,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (sim_run_id, family_key),
            INDEX idx_entry_exit_sim_family_contribution_playbook (playbook_id, sum_r),
            INDEX idx_entry_exit_sim_family_contribution_build (build_id, sum_r)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_playbook_sim_streaks (
            sim_run_id VARCHAR(64) NOT NULL,
            streak_number BIGINT NOT NULL,
            playbook_id VARCHAR(64) NULL,
            build_id VARCHAR(64) NOT NULL,
            streak_type VARCHAR(8) NOT NULL,
            start_index BIGINT NOT NULL,
            end_index BIGINT NOT NULL,
            start_date DATETIME NOT NULL,
            end_date DATETIME NOT NULL,
            streak_length BIGINT NOT NULL DEFAULT 0,
            sum_r DOUBLE NOT NULL DEFAULT 0,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (sim_run_id, streak_number),
            INDEX idx_entry_exit_sim_streaks_type (sim_run_id, streak_type, streak_length),
            INDEX idx_entry_exit_sim_streaks_playbook (playbook_id, streak_type, streak_length)
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn store_sim_run_summary(
    pool: &MySqlPool,
    sim_run_id: &str,
    playbook_id: Option<&str>,
) -> Result<u64, sqlx::Error> {
    sqlx::query("DELETE FROM entry_exit_playbook_sim_runs WHERE sim_run_id = ?")
        .bind(sim_run_id)
        .execute(pool)
        .await?;

    let result = sqlx::query(
        r#"
        INSERT INTO entry_exit_playbook_sim_runs (
            sim_run_id,
            playbook_id,
            build_id,
            source_scope,
            source_timeframe,
            test_year,
            min_train_tests,
            sister_window_minutes,
            families_selected,
            patterns_scanned,
            routed_patterns,
            no_route_patterns,
            skipped_non_trade_patterns,
            skipped_symbol_patterns,
            skipped_overlap_patterns,
            trade_choices,
            watchlist_choices,
            skip_choices,
            manual_family_bans_applied,
            symbol_filter_enabled,
            one_trade_at_a_time,
            one_trade_per_root_symbol,
            one_trade_per_minute,
            trade_cooldown_minutes,
            daily_loss_lockout,
            near_pass_protection,
            near_pass_within_r,
            near_pass_daily_loss_r,
            loss_cluster_day_lockout,
            loss_cluster_loss_count,
            loss_cluster_window_minutes,
            playbook_description,
            symbol_trade_roots,
            symbol_skip_roots,
            symbol_min_tests,
            symbol_min_win_rate,
            symbol_min_avg_r,
            prop_filter_enabled,
            trade_min_tests,
            trade_min_win_rate,
            trade_min_avg_r,
            watchlist_min_tests,
            watchlist_min_win_rate,
            watchlist_min_avg_r,
            win_count,
            loss_count,
            no_entry_count,
            avg_r,
            sum_r,
            best_r,
            worst_r,
            elapsed_ms,
            created_at
        )
        SELECT
            runs.router_run_id,
            ?,
            runs.train_run_id,
            runs.source_scope,
            NULLIF(runs.source_timeframe, ''),
            runs.test_year,
            runs.min_train_tests,
            runs.sister_window_minutes,
            runs.families_selected,
            runs.patterns_scanned,
            runs.routed_patterns,
            runs.no_route_patterns,
            runs.skipped_non_trade_patterns,
            runs.skipped_symbol_patterns,
            runs.skipped_overlap_patterns,
            runs.trade_choices,
            runs.watchlist_choices,
            runs.skip_choices,
            (
                SELECT CAST(COUNT(*) AS SIGNED)
                FROM entry_exit_template_family_router_choices choices
                WHERE choices.router_run_id = runs.router_run_id
                  AND choices.route_status = 'SKIP'
                  AND LOWER(COALESCE(choices.status_reason, '')) LIKE '%manual%'
            ),
            runs.symbol_filter_enabled,
            runs.one_trade_at_a_time,
            runs.one_trade_per_root_symbol,
            runs.one_trade_per_minute,
            COALESCE(runs.trade_cooldown_minutes, 0),
            COALESCE(runs.daily_loss_lockout, 0),
            COALESCE(runs.near_pass_protection, 0),
            COALESCE(runs.near_pass_within_r, 0),
            COALESCE(runs.near_pass_daily_loss_r, 0),
            COALESCE(runs.loss_cluster_day_lockout, 0),
            COALESCE(runs.loss_cluster_loss_count, 0),
            COALESCE(runs.loss_cluster_window_minutes, 0),
            runs.playbook_description,
            runs.symbol_trade_roots,
            runs.symbol_skip_roots,
            runs.symbol_min_tests,
            runs.symbol_min_win_rate,
            runs.symbol_min_avg_r,
            runs.prop_filter_enabled,
            runs.trade_min_tests,
            runs.trade_min_win_rate,
            runs.trade_min_avg_r,
            runs.watchlist_min_tests,
            runs.watchlist_min_win_rate,
            runs.watchlist_min_avg_r,
            runs.win_count,
            runs.loss_count,
            runs.no_entry_count,
            runs.avg_r,
            runs.sum_r,
            runs.best_r,
            runs.worst_r,
            runs.elapsed_ms,
            runs.created_at
        FROM entry_exit_template_family_router_runs runs
        WHERE runs.router_run_id = ?
        "#,
    )
    .bind(playbook_id)
    .bind(sim_run_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

async fn load_build_id(pool: &MySqlPool, sim_run_id: &str) -> Result<String, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT train_run_id
        FROM entry_exit_template_family_router_runs
        WHERE router_run_id = ?
        LIMIT 1
        "#,
    )
    .bind(sim_run_id)
    .fetch_one(pool)
    .await
}

async fn load_daily_loss_lockout(pool: &MySqlPool, sim_run_id: &str) -> Result<bool, sqlx::Error> {
    let value = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT CAST(COALESCE(daily_loss_lockout, 0) AS SIGNED)
        FROM entry_exit_playbook_sim_runs
        WHERE sim_run_id = ?
        LIMIT 1
        "#,
    )
    .bind(sim_run_id)
    .fetch_one(pool)
    .await?;
    Ok(value != 0)
}

async fn load_trades(pool: &MySqlPool, sim_run_id: &str) -> Result<Vec<ReplayTrade>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            COALESCE(entry_date, d_confirm_date, d_date) AS event_date,
            symbol,
            family_key,
            template_uid,
            outcome,
            COALESCE(result_r, 0) AS result_r
        FROM entry_exit_template_family_router_results
        WHERE router_run_id = ?
        ORDER BY COALESCE(entry_date, d_confirm_date, d_date) ASC, id ASC
        "#,
    )
    .bind(sim_run_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let symbol: String = row.try_get("symbol")?;
            Ok(ReplayTrade {
                event_date: row.try_get("event_date")?,
                root_symbol: normalize_root_symbol(&root_symbol_from_contract(&symbol)),
                symbol,
                family_key: row.try_get("family_key")?,
                template_uid: row.try_get("template_uid")?,
                outcome: row.try_get("outcome")?,
                result_r: row.try_get("result_r")?,
            })
        })
        .collect()
}

fn close_cycle(
    summary: &mut PropSummary,
    cycles: &mut Vec<PropCycleRow>,
    cycle: &OpenCycle,
    outcome: &str,
) {
    summary.cycles += 1;
    summary.max_drawdown_r = summary.max_drawdown_r.max(cycle.max_drawdown_r);
    match outcome {
        "passed_profit_target" => summary.passed += 1,
        "failed_daily_loss" => summary.daily_fails += 1,
        "failed_drawdown" => summary.drawdown_fails += 1,
        _ => summary.incomplete += 1,
    }
    cycles.push(cycle.finish(outcome));
}

fn compute_prop_summary(
    trades: &[ReplayTrade],
    daily_loss_lockout: bool,
) -> (PropSummary, Vec<PropCycleRow>) {
    const PROFIT_TARGET_R: f64 = 30.0;
    const MAX_DRAWDOWN_R: f64 = 20.0;
    const DAILY_LOSS_R: f64 = 10.0;

    let mut summary = PropSummary::default();
    let mut cycles = Vec::new();
    let mut active_cycle: Option<OpenCycle> = None;
    let mut current_loss_streak = 0_i64;
    let mut next_cycle_number = 1_i64;

    for trade in trades {
        let date = trade.event_date.date();
        let cycle = active_cycle.get_or_insert_with(|| {
            let cycle = OpenCycle::new(next_cycle_number, date);
            next_cycle_number += 1;
            cycle
        });
        cycle.apply_trade(trade);

        if trade.outcome == "pass" {
            current_loss_streak = 0;
        } else if trade.outcome == "fail" {
            current_loss_streak += 1;
            summary.max_loss_streak = summary.max_loss_streak.max(current_loss_streak);
        }

        let close_cycle_as = if !daily_loss_lockout && cycle.daily_r <= -DAILY_LOSS_R {
            Some("failed_daily_loss")
        } else if cycle.max_drawdown_r >= MAX_DRAWDOWN_R {
            Some("failed_drawdown")
        } else if cycle.equity_r >= PROFIT_TARGET_R {
            Some("passed_profit_target")
        } else {
            None
        };

        if let Some(outcome) = close_cycle_as {
            if let Some(finished_cycle) = active_cycle.take() {
                close_cycle(&mut summary, &mut cycles, &finished_cycle, outcome);
            }
        }
    }

    if let Some(finished_cycle) = active_cycle.take() {
        close_cycle(
            &mut summary,
            &mut cycles,
            &finished_cycle,
            "open_incomplete",
        );
    }

    let closed_cycles = summary.passed + summary.daily_fails + summary.drawdown_fails;
    summary.pass_rate = if summary.cycles > 0 {
        summary.passed as f64 / summary.cycles as f64 * 100.0
    } else {
        0.0
    };
    summary.closed_pass_rate = if closed_cycles > 0 {
        summary.passed as f64 / closed_cycles as f64 * 100.0
    } else {
        0.0
    };

    (summary, cycles)
}

fn compute_equity_points(trades: &[ReplayTrade], daily_loss_lockout: bool) -> Vec<EquityPointRow> {
    const PROFIT_TARGET_R: f64 = 30.0;
    const MAX_DRAWDOWN_R: f64 = 20.0;
    const DAILY_LOSS_R: f64 = 10.0;

    let mut points = Vec::with_capacity(trades.len());
    let mut cumulative_r = 0.0_f64;
    let mut active_cycle: Option<OpenCycle> = None;
    let mut next_cycle_number = 1_i64;

    for (index, trade) in trades.iter().enumerate() {
        let date = trade.event_date.date();
        let cycle = active_cycle.get_or_insert_with(|| {
            let cycle = OpenCycle::new(next_cycle_number, date);
            next_cycle_number += 1;
            cycle
        });
        cycle.apply_trade(trade);
        cumulative_r += trade.result_r;
        let cycle_drawdown_r = cycle.peak_r - cycle.equity_r;
        let cycle_daily_r = cycle.daily_r;
        points.push(EquityPointRow {
            point_index: index as i64 + 1,
            event_date: trade.event_date,
            outcome: trade.outcome.clone(),
            result_r: trade.result_r,
            cumulative_r,
            drawdown_r: cycle_drawdown_r,
            daily_r: cycle_daily_r,
        });

        let should_close_cycle = (!daily_loss_lockout && cycle.daily_r <= -DAILY_LOSS_R)
            || cycle.max_drawdown_r >= MAX_DRAWDOWN_R
            || cycle.equity_r >= PROFIT_TARGET_R;
        if should_close_cycle {
            active_cycle.take();
        }
    }

    points
}

fn finish_daily_r_row(
    rows: &mut Vec<DailyRRow>,
    trade_date: Option<NaiveDate>,
    total_r: f64,
    trades: i64,
    wins: i64,
    losses: i64,
    no_entries: i64,
    best_trade_r: Option<f64>,
    worst_trade_r: Option<f64>,
    worst_intraday_r: f64,
) {
    let Some(trade_date) = trade_date else {
        return;
    };

    rows.push(DailyRRow {
        trade_date,
        total_r,
        trades,
        wins,
        losses,
        no_entries,
        best_trade_r: best_trade_r.unwrap_or(0.0),
        worst_trade_r: worst_trade_r.unwrap_or(0.0),
        worst_intraday_r,
        hit_daily_loss: worst_intraday_r <= -10.0,
    });
}

fn compute_daily_r(trades: &[ReplayTrade]) -> Vec<DailyRRow> {
    let mut rows = Vec::new();
    let mut current_date: Option<NaiveDate> = None;
    let mut total_r = 0.0_f64;
    let mut intraday_r = 0.0_f64;
    let mut worst_intraday_r = 0.0_f64;
    let mut trades_count = 0_i64;
    let mut wins = 0_i64;
    let mut losses = 0_i64;
    let mut no_entries = 0_i64;
    let mut best_trade_r: Option<f64> = None;
    let mut worst_trade_r: Option<f64> = None;

    for trade in trades {
        let trade_date = trade.event_date.date();
        if current_date != Some(trade_date) {
            finish_daily_r_row(
                &mut rows,
                current_date,
                total_r,
                trades_count,
                wins,
                losses,
                no_entries,
                best_trade_r,
                worst_trade_r,
                worst_intraday_r,
            );

            current_date = Some(trade_date);
            total_r = 0.0;
            intraday_r = 0.0;
            worst_intraday_r = 0.0;
            trades_count = 0;
            wins = 0;
            losses = 0;
            no_entries = 0;
            best_trade_r = None;
            worst_trade_r = None;
        }

        match trade.outcome.as_str() {
            "pass" => {
                trades_count += 1;
                wins += 1;
                best_trade_r =
                    Some(best_trade_r.map_or(trade.result_r, |value| value.max(trade.result_r)));
                worst_trade_r =
                    Some(worst_trade_r.map_or(trade.result_r, |value| value.min(trade.result_r)));
            }
            "fail" => {
                trades_count += 1;
                losses += 1;
                best_trade_r =
                    Some(best_trade_r.map_or(trade.result_r, |value| value.max(trade.result_r)));
                worst_trade_r =
                    Some(worst_trade_r.map_or(trade.result_r, |value| value.min(trade.result_r)));
            }
            "no_entry" => {
                no_entries += 1;
            }
            _ => {}
        }

        total_r += trade.result_r;
        intraday_r += trade.result_r;
        worst_intraday_r = worst_intraday_r.min(intraday_r);
    }

    finish_daily_r_row(
        &mut rows,
        current_date,
        total_r,
        trades_count,
        wins,
        losses,
        no_entries,
        best_trade_r,
        worst_trade_r,
        worst_intraday_r,
    );

    rows
}

fn compute_hourly_performance(
    trades: &[ReplayTrade],
    daily_rows: &[DailyRRow],
) -> Vec<HourlyPerformanceRow> {
    let daily_loss_dates: std::collections::HashSet<NaiveDate> = daily_rows
        .iter()
        .filter(|row| row.hit_daily_loss)
        .map(|row| row.trade_date)
        .collect();
    let mut by_hour: std::collections::HashMap<i64, HourlyPerformanceRow> =
        std::collections::HashMap::new();

    for trade in trades {
        let entry_hour = trade.event_date.hour() as i64;
        let trade_date = trade.event_date.date();
        let row = by_hour
            .entry(entry_hour)
            .or_insert_with(|| HourlyPerformanceRow {
                entry_hour,
                ..HourlyPerformanceRow::default()
            });

        match trade.outcome.as_str() {
            "pass" => {
                row.trades += 1;
                row.wins += 1;
                row.sum_r += trade.result_r;
                if row.trades == 1 {
                    row.best_r = trade.result_r;
                    row.worst_r = trade.result_r;
                } else {
                    row.best_r = row.best_r.max(trade.result_r);
                    row.worst_r = row.worst_r.min(trade.result_r);
                }
                if daily_loss_dates.contains(&trade_date) {
                    row.daily_loss_day_trades += 1;
                    row.daily_loss_days.insert(trade_date);
                }
            }
            "fail" => {
                row.trades += 1;
                row.losses += 1;
                row.sum_r += trade.result_r;
                if row.trades == 1 {
                    row.best_r = trade.result_r;
                    row.worst_r = trade.result_r;
                } else {
                    row.best_r = row.best_r.max(trade.result_r);
                    row.worst_r = row.worst_r.min(trade.result_r);
                }
                if daily_loss_dates.contains(&trade_date) {
                    row.daily_loss_day_trades += 1;
                    row.daily_loss_days.insert(trade_date);
                }
            }
            "no_entry" => {
                row.no_entries += 1;
            }
            _ => {}
        }
    }

    let mut rows = by_hour.into_values().collect::<Vec<_>>();
    rows.sort_by_key(|row| row.entry_hour);
    rows
}

fn max_trades_in_window(times: &[NaiveDateTime], window_minutes: i64) -> i64 {
    if times.is_empty() {
        return 0;
    }

    let mut start = 0_usize;
    let mut max_count = 1_i64;
    for end in 0..times.len() {
        while start < end
            && times[end].signed_duration_since(times[start]).num_seconds() > window_minutes * 60
        {
            start += 1;
        }
        max_count = max_count.max((end - start + 1) as i64);
    }

    max_count
}

fn median(values: &mut [f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let midpoint = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[midpoint - 1] + values[midpoint]) / 2.0
    } else {
        values[midpoint]
    }
}

fn compute_trade_cadence(trades: &[ReplayTrade]) -> TradeCadenceRow {
    let mut times = trades
        .iter()
        .filter(|trade| matches!(trade.outcome.as_str(), "pass" | "fail"))
        .map(|trade| trade.event_date)
        .collect::<Vec<_>>();
    times.sort();

    let mut cadence = TradeCadenceRow {
        trades: times.len() as i64,
        first_trade_at: times.first().copied(),
        last_trade_at: times.last().copied(),
        ..TradeCadenceRow::default()
    };

    if times.is_empty() {
        return cadence;
    }

    let mut trades_by_day: HashMap<NaiveDate, i64> = HashMap::new();
    let mut trades_by_hour: HashMap<(NaiveDate, u32), i64> = HashMap::new();
    for trade_time in &times {
        *trades_by_day.entry(trade_time.date()).or_default() += 1;
        *trades_by_hour
            .entry((trade_time.date(), trade_time.hour()))
            .or_default() += 1;
    }
    cadence.trade_days = trades_by_day.len() as i64;
    cadence.avg_trades_per_day = if cadence.trade_days > 0 {
        cadence.trades as f64 / cadence.trade_days as f64
    } else {
        0.0
    };
    cadence.max_trades_per_day = trades_by_day.values().copied().max().unwrap_or(0);
    cadence.max_trades_per_hour = trades_by_hour.values().copied().max().unwrap_or(0);
    cadence.max_trades_5m_window = max_trades_in_window(&times, 5);
    cadence.max_trades_15m_window = max_trades_in_window(&times, 15);

    let gaps = times
        .windows(2)
        .map(|window| {
            window[1]
                .signed_duration_since(window[0])
                .num_seconds()
                .max(0) as f64
                / 60.0
        })
        .collect::<Vec<_>>();
    cadence.gap_count = gaps.len() as i64;
    if gaps.is_empty() {
        return cadence;
    }

    cadence.avg_gap_minutes = gaps.iter().sum::<f64>() / gaps.len() as f64;
    let mut gaps_for_median = gaps.clone();
    cadence.median_gap_minutes = median(&mut gaps_for_median);
    cadence.min_gap_minutes = gaps.iter().copied().fold(f64::INFINITY, f64::min);
    cadence.max_gap_minutes = gaps.iter().copied().fold(0.0, f64::max);
    for gap in gaps {
        if gap <= 1.0 {
            cadence.gap_0_1m += 1;
        } else if gap <= 5.0 {
            cadence.gap_1_5m += 1;
        } else if gap <= 15.0 {
            cadence.gap_5_15m += 1;
        } else if gap <= 30.0 {
            cadence.gap_15_30m += 1;
        } else if gap <= 60.0 {
            cadence.gap_30_60m += 1;
        } else {
            cadence.gap_over_60m += 1;
        }
    }

    cadence
}

fn loss_gap_bucket(previous: NaiveDateTime, current: NaiveDateTime, gap_minutes: f64) -> (&'static str, &'static str, i64) {
    if previous.date() != current.date() {
        ("next_day_plus", "Next day+", 7)
    } else if gap_minutes <= 5.0 {
        ("0_5m", "0-5m", 1)
    } else if gap_minutes <= 15.0 {
        ("5_15m", "5-15m", 2)
    } else if gap_minutes <= 30.0 {
        ("15_30m", "15-30m", 3)
    } else if gap_minutes <= 60.0 {
        ("30_60m", "30-60m", 4)
    } else if gap_minutes <= 240.0 {
        ("1_4h", "1-4h", 5)
    } else {
        ("same_day_4h_plus", "4h+ same day", 6)
    }
}

fn increment_count(counts: &mut HashMap<String, i64>, key: &str) {
    if key.trim().is_empty() {
        return;
    }
    *counts.entry(key.to_string()).or_default() += 1;
}

fn most_common_key(counts: &HashMap<String, i64>) -> String {
    counts
        .iter()
        .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
        .map(|(key, _)| key.clone())
        .unwrap_or_default()
}

fn compute_loss_clustering(
    trades: &[ReplayTrade],
    streaks: &[StreakRow],
) -> (LossClusterSummaryRow, Vec<LossGapBucketRow>, Vec<LossWindowRow>) {
    let loss_trades = trades
        .iter()
        .filter(|trade| trade.outcome == "fail")
        .collect::<Vec<_>>();
    let mut summary = LossClusterSummaryRow {
        losses: loss_trades.len() as i64,
        max_loss_streak: streaks
            .iter()
            .filter(|streak| streak.streak_type == "loss")
            .map(|streak| streak.streak_length)
            .max()
            .unwrap_or(0),
        ..LossClusterSummaryRow::default()
    };

    let mut bucket_counts: HashMap<&'static str, (&'static str, i64, i64)> = [
        ("0_5m", ("0-5m", 1, 0)),
        ("5_15m", ("5-15m", 2, 0)),
        ("15_30m", ("15-30m", 3, 0)),
        ("30_60m", ("30-60m", 4, 0)),
        ("1_4h", ("1-4h", 5, 0)),
        ("same_day_4h_plus", ("4h+ same day", 6, 0)),
        ("next_day_plus", ("Next day+", 7, 0)),
    ]
    .into_iter()
    .collect();

    let mut gaps = Vec::new();
    for window in loss_trades.windows(2) {
        let previous = window[0].event_date;
        let current = window[1].event_date;
        let gap_minutes = current
            .signed_duration_since(previous)
            .num_seconds()
            .max(0) as f64
            / 60.0;
        gaps.push(gap_minutes);
        if gap_minutes <= 60.0 {
            summary.clustered_60m_loss_pairs += 1;
        }
        if previous.date() == current.date() {
            summary.same_day_loss_pairs += 1;
        }
        let (key, label, sort_order) = loss_gap_bucket(previous, current, gap_minutes);
        let entry = bucket_counts.entry(key).or_insert((label, sort_order, 0));
        entry.2 += 1;
    }

    summary.loss_gap_count = gaps.len() as i64;
    if !gaps.is_empty() {
        summary.avg_loss_gap_minutes = gaps.iter().sum::<f64>() / gaps.len() as f64;
        let mut median_gaps = gaps.clone();
        summary.median_loss_gap_minutes = median(&mut median_gaps);
        summary.min_loss_gap_minutes = gaps.iter().copied().fold(f64::INFINITY, f64::min);
        summary.max_loss_gap_minutes = gaps.iter().copied().fold(0.0, f64::max);
        summary.clustered_60m_rate =
            summary.clustered_60m_loss_pairs as f64 / summary.loss_gap_count as f64 * 100.0;
    }

    let mut day_losses: HashMap<NaiveDate, i64> = HashMap::new();
    let mut day_r: HashMap<NaiveDate, f64> = HashMap::new();
    let mut windows: HashMap<(NaiveDate, i64), LossWindowAccumulator> = HashMap::new();

    for trade in trades {
        let trade_date = trade.event_date.date();
        let entry_hour = trade.event_date.hour() as i64;
        *day_r.entry(trade_date).or_default() += trade.result_r;
        if trade.outcome == "fail" {
            *day_losses.entry(trade_date).or_default() += 1;
        }

        let window = windows
            .entry((trade_date, entry_hour))
            .or_insert_with(|| LossWindowAccumulator {
                trade_date,
                entry_hour,
                trades: 0,
                wins: 0,
                losses: 0,
                no_entries: 0,
                total_r: 0.0,
                root_counts: HashMap::new(),
                family_counts: HashMap::new(),
                template_counts: HashMap::new(),
            });

        match trade.outcome.as_str() {
            "pass" => {
                window.trades += 1;
                window.wins += 1;
                window.total_r += trade.result_r;
            }
            "fail" => {
                window.trades += 1;
                window.losses += 1;
                window.total_r += trade.result_r;
                increment_count(&mut window.root_counts, &trade.root_symbol);
                increment_count(&mut window.family_counts, &trade.family_key);
                increment_count(&mut window.template_counts, &trade.template_uid);
            }
            "no_entry" => {
                window.no_entries += 1;
            }
            _ => {}
        }
    }

    summary.loss_days = day_losses.len() as i64;
    summary.loss_days_5_plus = day_losses.values().filter(|losses| **losses >= 5).count() as i64;
    if let Some((day, losses)) = day_losses.iter().max_by(|left, right| {
        left.1
            .cmp(right.1)
            .then_with(|| {
                day_r
                    .get(right.0)
                    .copied()
                    .unwrap_or(0.0)
                    .partial_cmp(&day_r.get(left.0).copied().unwrap_or(0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }) {
        summary.worst_loss_day = Some(*day);
        summary.worst_loss_day_losses = *losses;
        summary.worst_loss_day_r = day_r.get(day).copied().unwrap_or(0.0);
    }

    let mut window_rows = windows
        .into_values()
        .filter(|window| window.losses > 0)
        .map(|window| {
            let loss_rate = if window.trades > 0 {
                window.losses as f64 / window.trades as f64 * 100.0
            } else {
                0.0
            };
            LossWindowRow {
                trade_date: window.trade_date,
                entry_hour: window.entry_hour,
                trades: window.trades,
                wins: window.wins,
                losses: window.losses,
                no_entries: window.no_entries,
                total_r: window.total_r,
                loss_rate,
                top_root_symbol: most_common_key(&window.root_counts),
                top_family_key: most_common_key(&window.family_counts),
                top_template_uid: most_common_key(&window.template_counts),
            }
        })
        .collect::<Vec<_>>();
    window_rows.sort_by(|left, right| {
        right
            .losses
            .cmp(&left.losses)
            .then_with(|| {
                left.total_r
                    .partial_cmp(&right.total_r)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| left.trade_date.cmp(&right.trade_date))
            .then_with(|| left.entry_hour.cmp(&right.entry_hour))
    });

    if let Some(worst_hour) = window_rows.first() {
        summary.worst_loss_hour_date = Some(worst_hour.trade_date);
        summary.worst_loss_hour = Some(worst_hour.entry_hour);
        summary.worst_loss_hour_losses = worst_hour.losses;
        summary.worst_loss_hour_r = worst_hour.total_r;
    }

    let mut buckets = bucket_counts
        .into_iter()
        .map(|(key, (label, sort_order, gap_count))| LossGapBucketRow {
            bucket_key: key.to_string(),
            bucket_label: label.to_string(),
            sort_order,
            gap_count,
            gap_percent: if summary.loss_gap_count > 0 {
                gap_count as f64 / summary.loss_gap_count as f64 * 100.0
            } else {
                0.0
            },
        })
        .collect::<Vec<_>>();
    buckets.sort_by_key(|bucket| bucket.sort_order);

    (summary, buckets, window_rows)
}

fn apply_trade_to_contribution_row(
    row: &mut ContributionRow,
    trade: &ReplayTrade,
    daily_loss_dates: &HashSet<NaiveDate>,
) {
    row.root_symbols.insert(trade.root_symbol.clone());
    row.contract_symbols.insert(trade.symbol.clone());
    row.family_keys.insert(trade.family_key.clone());
    row.template_uids.insert(trade.template_uid.clone());

    match trade.outcome.as_str() {
        "pass" => {
            row.trades += 1;
            row.wins += 1;
        }
        "fail" => {
            row.trades += 1;
            row.losses += 1;
        }
        "no_entry" => {
            row.no_entries += 1;
        }
        _ => {}
    }

    if matches!(trade.outcome.as_str(), "pass" | "fail") {
        row.sum_r += trade.result_r;
        if row.trades == 1 {
            row.best_r = trade.result_r;
            row.worst_r = trade.result_r;
        } else {
            row.best_r = row.best_r.max(trade.result_r);
            row.worst_r = row.worst_r.min(trade.result_r);
        }

        let trade_date = trade.event_date.date();
        if daily_loss_dates.contains(&trade_date) {
            row.daily_loss_day_trades += 1;
            row.daily_loss_days.insert(trade_date);
        }
    }
}

fn compute_contribution_rows(
    trades: &[ReplayTrade],
    daily_rows: &[DailyRRow],
    key_for_trade: impl Fn(&ReplayTrade) -> String,
) -> Vec<ContributionRow> {
    let daily_loss_dates: HashSet<NaiveDate> = daily_rows
        .iter()
        .filter(|row| row.hit_daily_loss)
        .map(|row| row.trade_date)
        .collect();
    let mut by_key: HashMap<String, ContributionRow> = HashMap::new();

    for trade in trades {
        let key = key_for_trade(trade);
        if key.trim().is_empty() {
            continue;
        }
        let row = by_key
            .entry(key.clone())
            .or_insert_with(|| ContributionRow {
                key,
                ..ContributionRow::default()
            });
        apply_trade_to_contribution_row(row, trade, &daily_loss_dates);
    }

    let mut rows = by_key.into_values().collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .sum_r
            .partial_cmp(&left.sum_r)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.trades.cmp(&left.trades))
            .then_with(|| left.key.cmp(&right.key))
    });
    rows
}

fn finish_streak(
    streaks: &mut Vec<StreakRow>,
    streak_type: &str,
    start_index: i64,
    end_index: i64,
    start_date: NaiveDateTime,
    end_date: NaiveDateTime,
    streak_length: i64,
    sum_r: f64,
) {
    streaks.push(StreakRow {
        streak_number: streaks.len() as i64 + 1,
        streak_type: streak_type.to_string(),
        start_index,
        end_index,
        start_date,
        end_date,
        streak_length,
        sum_r,
    });
}

fn compute_streaks(trades: &[ReplayTrade]) -> Vec<StreakRow> {
    let mut streaks = Vec::new();
    let mut current_type: Option<String> = None;
    let mut start_index = 0_i64;
    let mut end_index = 0_i64;
    let mut start_date: Option<NaiveDateTime> = None;
    let mut end_date: Option<NaiveDateTime> = None;
    let mut streak_length = 0_i64;
    let mut sum_r = 0.0_f64;

    for (index, trade) in trades.iter().enumerate() {
        let next_type = match trade.outcome.as_str() {
            "pass" => "win",
            "fail" => "loss",
            _ => continue,
        };
        let point_index = index as i64 + 1;

        if current_type.as_deref() != Some(next_type) {
            if let (Some(streak_type), Some(streak_start_date), Some(streak_end_date)) =
                (current_type.as_deref(), start_date, end_date)
            {
                finish_streak(
                    &mut streaks,
                    streak_type,
                    start_index,
                    end_index,
                    streak_start_date,
                    streak_end_date,
                    streak_length,
                    sum_r,
                );
            }

            current_type = Some(next_type.to_string());
            start_index = point_index;
            start_date = Some(trade.event_date);
            streak_length = 0;
            sum_r = 0.0;
        }

        end_index = point_index;
        end_date = Some(trade.event_date);
        streak_length += 1;
        sum_r += trade.result_r;
    }

    if let (Some(streak_type), Some(streak_start_date), Some(streak_end_date)) =
        (current_type.as_deref(), start_date, end_date)
    {
        finish_streak(
            &mut streaks,
            streak_type,
            start_index,
            end_index,
            streak_start_date,
            streak_end_date,
            streak_length,
            sum_r,
        );
    }

    streaks
}

async fn store_prop_summary(
    pool: &MySqlPool,
    sim_run_id: &str,
    playbook_id: Option<&str>,
    build_id: &str,
    summary: &PropSummary,
    cycles: &[PropCycleRow],
) -> Result<(), sqlx::Error> {
    const PROFIT_TARGET_R: f64 = 30.0;
    const MAX_DRAWDOWN_R: f64 = 20.0;
    const DAILY_LOSS_R: f64 = 10.0;

    sqlx::query("DELETE FROM entry_exit_playbook_sim_prop_cycles WHERE sim_run_id = ?")
        .bind(sim_run_id)
        .execute(pool)
        .await?;

    sqlx::query(
        r#"
        INSERT INTO entry_exit_playbook_sim_prop_summary (
            sim_run_id,
            playbook_id,
            build_id,
            profit_target_r,
            max_drawdown_r_limit,
            daily_loss_r_limit,
            cycles,
            passed,
            daily_fails,
            drawdown_fails,
            incomplete,
            pass_rate,
            closed_pass_rate,
            max_drawdown_r,
            max_loss_streak
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE
            playbook_id = VALUES(playbook_id),
            build_id = VALUES(build_id),
            profit_target_r = VALUES(profit_target_r),
            max_drawdown_r_limit = VALUES(max_drawdown_r_limit),
            daily_loss_r_limit = VALUES(daily_loss_r_limit),
            cycles = VALUES(cycles),
            passed = VALUES(passed),
            daily_fails = VALUES(daily_fails),
            drawdown_fails = VALUES(drawdown_fails),
            incomplete = VALUES(incomplete),
            pass_rate = VALUES(pass_rate),
            closed_pass_rate = VALUES(closed_pass_rate),
            max_drawdown_r = VALUES(max_drawdown_r),
            max_loss_streak = VALUES(max_loss_streak)
        "#,
    )
    .bind(sim_run_id)
    .bind(playbook_id)
    .bind(build_id)
    .bind(PROFIT_TARGET_R)
    .bind(MAX_DRAWDOWN_R)
    .bind(DAILY_LOSS_R)
    .bind(summary.cycles)
    .bind(summary.passed)
    .bind(summary.daily_fails)
    .bind(summary.drawdown_fails)
    .bind(summary.incomplete)
    .bind(summary.pass_rate)
    .bind(summary.closed_pass_rate)
    .bind(summary.max_drawdown_r)
    .bind(summary.max_loss_streak)
    .execute(pool)
    .await?;

    for cycle in cycles {
        sqlx::query(
            r#"
            INSERT INTO entry_exit_playbook_sim_prop_cycles (
                sim_run_id,
                cycle_number,
                playbook_id,
                build_id,
                outcome,
                start_date,
                end_date,
                trades,
                wins,
                losses,
                no_entries,
                sum_r,
                max_drawdown_r,
                worst_day_r
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(sim_run_id)
        .bind(cycle.cycle_number)
        .bind(playbook_id)
        .bind(build_id)
        .bind(&cycle.outcome)
        .bind(cycle.start_date)
        .bind(cycle.end_date)
        .bind(cycle.trades)
        .bind(cycle.wins)
        .bind(cycle.losses)
        .bind(cycle.no_entries)
        .bind(cycle.sum_r)
        .bind(cycle.max_drawdown_r)
        .bind(cycle.worst_day_r)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn store_equity_points(
    pool: &MySqlPool,
    sim_run_id: &str,
    playbook_id: Option<&str>,
    build_id: &str,
    points: &[EquityPointRow],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM entry_exit_playbook_sim_equity_points WHERE sim_run_id = ?")
        .bind(sim_run_id)
        .execute(pool)
        .await?;

    for point in points {
        sqlx::query(
            r#"
            INSERT INTO entry_exit_playbook_sim_equity_points (
                sim_run_id,
                point_index,
                playbook_id,
                build_id,
                event_date,
                outcome,
                result_r,
                cumulative_r,
                drawdown_r,
                daily_r
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(sim_run_id)
        .bind(point.point_index)
        .bind(playbook_id)
        .bind(build_id)
        .bind(point.event_date)
        .bind(&point.outcome)
        .bind(point.result_r)
        .bind(point.cumulative_r)
        .bind(point.drawdown_r)
        .bind(point.daily_r)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn store_daily_r_rows(
    pool: &MySqlPool,
    sim_run_id: &str,
    playbook_id: Option<&str>,
    build_id: &str,
    rows: &[DailyRRow],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM entry_exit_playbook_sim_daily_r WHERE sim_run_id = ?")
        .bind(sim_run_id)
        .execute(pool)
        .await?;

    for row in rows {
        sqlx::query(
            r#"
            INSERT INTO entry_exit_playbook_sim_daily_r (
                sim_run_id,
                trade_date,
                playbook_id,
                build_id,
                total_r,
                trades,
                wins,
                losses,
                no_entries,
                best_trade_r,
                worst_trade_r,
                worst_intraday_r,
                hit_daily_loss
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(sim_run_id)
        .bind(row.trade_date)
        .bind(playbook_id)
        .bind(build_id)
        .bind(row.total_r)
        .bind(row.trades)
        .bind(row.wins)
        .bind(row.losses)
        .bind(row.no_entries)
        .bind(row.best_trade_r)
        .bind(row.worst_trade_r)
        .bind(row.worst_intraday_r)
        .bind(row.hit_daily_loss)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn store_hourly_performance_rows(
    pool: &MySqlPool,
    sim_run_id: &str,
    playbook_id: Option<&str>,
    build_id: &str,
    rows: &[HourlyPerformanceRow],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM entry_exit_playbook_sim_hourly WHERE sim_run_id = ?")
        .bind(sim_run_id)
        .execute(pool)
        .await?;

    for row in rows {
        let win_rate = if row.trades > 0 {
            row.wins as f64 / row.trades as f64 * 100.0
        } else {
            0.0
        };
        let avg_r = if row.trades > 0 {
            row.sum_r / row.trades as f64
        } else {
            0.0
        };

        sqlx::query(
            r#"
            INSERT INTO entry_exit_playbook_sim_hourly (
                sim_run_id,
                entry_hour,
                playbook_id,
                build_id,
                trades,
                wins,
                losses,
                no_entries,
                win_rate,
                avg_r,
                sum_r,
                best_r,
                worst_r,
                daily_loss_day_trades,
                daily_loss_day_count
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(sim_run_id)
        .bind(row.entry_hour)
        .bind(playbook_id)
        .bind(build_id)
        .bind(row.trades)
        .bind(row.wins)
        .bind(row.losses)
        .bind(row.no_entries)
        .bind(win_rate)
        .bind(avg_r)
        .bind(row.sum_r)
        .bind(row.best_r)
        .bind(row.worst_r)
        .bind(row.daily_loss_day_trades)
        .bind(row.daily_loss_days.len() as i64)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn store_trade_cadence(
    pool: &MySqlPool,
    sim_run_id: &str,
    playbook_id: Option<&str>,
    build_id: &str,
    cadence: &TradeCadenceRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO entry_exit_playbook_sim_trade_cadence (
            sim_run_id,
            playbook_id,
            build_id,
            first_trade_at,
            last_trade_at,
            trades,
            trade_days,
            gap_count,
            avg_gap_minutes,
            median_gap_minutes,
            min_gap_minutes,
            max_gap_minutes,
            avg_trades_per_day,
            max_trades_per_day,
            max_trades_per_hour,
            max_trades_5m_window,
            max_trades_15m_window,
            gap_0_1m,
            gap_1_5m,
            gap_5_15m,
            gap_15_30m,
            gap_30_60m,
            gap_over_60m
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE
            playbook_id = VALUES(playbook_id),
            build_id = VALUES(build_id),
            first_trade_at = VALUES(first_trade_at),
            last_trade_at = VALUES(last_trade_at),
            trades = VALUES(trades),
            trade_days = VALUES(trade_days),
            gap_count = VALUES(gap_count),
            avg_gap_minutes = VALUES(avg_gap_minutes),
            median_gap_minutes = VALUES(median_gap_minutes),
            min_gap_minutes = VALUES(min_gap_minutes),
            max_gap_minutes = VALUES(max_gap_minutes),
            avg_trades_per_day = VALUES(avg_trades_per_day),
            max_trades_per_day = VALUES(max_trades_per_day),
            max_trades_per_hour = VALUES(max_trades_per_hour),
            max_trades_5m_window = VALUES(max_trades_5m_window),
            max_trades_15m_window = VALUES(max_trades_15m_window),
            gap_0_1m = VALUES(gap_0_1m),
            gap_1_5m = VALUES(gap_1_5m),
            gap_5_15m = VALUES(gap_5_15m),
            gap_15_30m = VALUES(gap_15_30m),
            gap_30_60m = VALUES(gap_30_60m),
            gap_over_60m = VALUES(gap_over_60m)
        "#,
    )
    .bind(sim_run_id)
    .bind(playbook_id)
    .bind(build_id)
    .bind(cadence.first_trade_at)
    .bind(cadence.last_trade_at)
    .bind(cadence.trades)
    .bind(cadence.trade_days)
    .bind(cadence.gap_count)
    .bind(cadence.avg_gap_minutes)
    .bind(cadence.median_gap_minutes)
    .bind(cadence.min_gap_minutes)
    .bind(cadence.max_gap_minutes)
    .bind(cadence.avg_trades_per_day)
    .bind(cadence.max_trades_per_day)
    .bind(cadence.max_trades_per_hour)
    .bind(cadence.max_trades_5m_window)
    .bind(cadence.max_trades_15m_window)
    .bind(cadence.gap_0_1m)
    .bind(cadence.gap_1_5m)
    .bind(cadence.gap_5_15m)
    .bind(cadence.gap_15_30m)
    .bind(cadence.gap_30_60m)
    .bind(cadence.gap_over_60m)
    .execute(pool)
    .await?;

    Ok(())
}

async fn store_loss_clustering(
    pool: &MySqlPool,
    sim_run_id: &str,
    playbook_id: Option<&str>,
    build_id: &str,
    summary: &LossClusterSummaryRow,
    buckets: &[LossGapBucketRow],
    windows: &[LossWindowRow],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM entry_exit_playbook_sim_loss_gap_buckets WHERE sim_run_id = ?")
        .bind(sim_run_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM entry_exit_playbook_sim_loss_windows WHERE sim_run_id = ?")
        .bind(sim_run_id)
        .execute(pool)
        .await?;

    sqlx::query(
        r#"
        INSERT INTO entry_exit_playbook_sim_loss_summary (
            sim_run_id,
            playbook_id,
            build_id,
            losses,
            loss_gap_count,
            avg_loss_gap_minutes,
            median_loss_gap_minutes,
            min_loss_gap_minutes,
            max_loss_gap_minutes,
            clustered_60m_loss_pairs,
            same_day_loss_pairs,
            clustered_60m_rate,
            loss_days,
            loss_days_5_plus,
            worst_loss_day,
            worst_loss_day_losses,
            worst_loss_day_r,
            worst_loss_hour_date,
            worst_loss_hour,
            worst_loss_hour_losses,
            worst_loss_hour_r,
            max_loss_streak
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE
            playbook_id = VALUES(playbook_id),
            build_id = VALUES(build_id),
            losses = VALUES(losses),
            loss_gap_count = VALUES(loss_gap_count),
            avg_loss_gap_minutes = VALUES(avg_loss_gap_minutes),
            median_loss_gap_minutes = VALUES(median_loss_gap_minutes),
            min_loss_gap_minutes = VALUES(min_loss_gap_minutes),
            max_loss_gap_minutes = VALUES(max_loss_gap_minutes),
            clustered_60m_loss_pairs = VALUES(clustered_60m_loss_pairs),
            same_day_loss_pairs = VALUES(same_day_loss_pairs),
            clustered_60m_rate = VALUES(clustered_60m_rate),
            loss_days = VALUES(loss_days),
            loss_days_5_plus = VALUES(loss_days_5_plus),
            worst_loss_day = VALUES(worst_loss_day),
            worst_loss_day_losses = VALUES(worst_loss_day_losses),
            worst_loss_day_r = VALUES(worst_loss_day_r),
            worst_loss_hour_date = VALUES(worst_loss_hour_date),
            worst_loss_hour = VALUES(worst_loss_hour),
            worst_loss_hour_losses = VALUES(worst_loss_hour_losses),
            worst_loss_hour_r = VALUES(worst_loss_hour_r),
            max_loss_streak = VALUES(max_loss_streak)
        "#,
    )
    .bind(sim_run_id)
    .bind(playbook_id)
    .bind(build_id)
    .bind(summary.losses)
    .bind(summary.loss_gap_count)
    .bind(summary.avg_loss_gap_minutes)
    .bind(summary.median_loss_gap_minutes)
    .bind(summary.min_loss_gap_minutes)
    .bind(summary.max_loss_gap_minutes)
    .bind(summary.clustered_60m_loss_pairs)
    .bind(summary.same_day_loss_pairs)
    .bind(summary.clustered_60m_rate)
    .bind(summary.loss_days)
    .bind(summary.loss_days_5_plus)
    .bind(summary.worst_loss_day)
    .bind(summary.worst_loss_day_losses)
    .bind(summary.worst_loss_day_r)
    .bind(summary.worst_loss_hour_date)
    .bind(summary.worst_loss_hour)
    .bind(summary.worst_loss_hour_losses)
    .bind(summary.worst_loss_hour_r)
    .bind(summary.max_loss_streak)
    .execute(pool)
    .await?;

    for bucket in buckets {
        sqlx::query(
            r#"
            INSERT INTO entry_exit_playbook_sim_loss_gap_buckets (
                sim_run_id,
                bucket_key,
                playbook_id,
                build_id,
                bucket_label,
                sort_order,
                gap_count,
                gap_percent
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(sim_run_id)
        .bind(&bucket.bucket_key)
        .bind(playbook_id)
        .bind(build_id)
        .bind(&bucket.bucket_label)
        .bind(bucket.sort_order)
        .bind(bucket.gap_count)
        .bind(bucket.gap_percent)
        .execute(pool)
        .await?;
    }

    for window in windows.iter().take(200) {
        sqlx::query(
            r#"
            INSERT INTO entry_exit_playbook_sim_loss_windows (
                sim_run_id,
                trade_date,
                entry_hour,
                playbook_id,
                build_id,
                trades,
                wins,
                losses,
                no_entries,
                total_r,
                loss_rate,
                top_root_symbol,
                top_family_key,
                top_template_uid
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(sim_run_id)
        .bind(window.trade_date)
        .bind(window.entry_hour)
        .bind(playbook_id)
        .bind(build_id)
        .bind(window.trades)
        .bind(window.wins)
        .bind(window.losses)
        .bind(window.no_entries)
        .bind(window.total_r)
        .bind(window.loss_rate)
        .bind(&window.top_root_symbol)
        .bind(&window.top_family_key)
        .bind(&window.top_template_uid)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn store_symbol_contribution_rows(
    pool: &MySqlPool,
    sim_run_id: &str,
    playbook_id: Option<&str>,
    build_id: &str,
    rows: &[ContributionRow],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM entry_exit_playbook_sim_symbol_contribution WHERE sim_run_id = ?")
        .bind(sim_run_id)
        .execute(pool)
        .await?;

    for row in rows {
        let win_rate = if row.trades > 0 {
            row.wins as f64 / row.trades as f64 * 100.0
        } else {
            0.0
        };
        let avg_r = if row.trades > 0 {
            row.sum_r / row.trades as f64
        } else {
            0.0
        };

        sqlx::query(
            r#"
            INSERT INTO entry_exit_playbook_sim_symbol_contribution (
                sim_run_id,
                root_symbol,
                playbook_id,
                build_id,
                trades,
                wins,
                losses,
                no_entries,
                win_rate,
                avg_r,
                sum_r,
                best_r,
                worst_r,
                daily_loss_day_trades,
                daily_loss_day_count,
                family_count,
                template_count,
                contract_count
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(sim_run_id)
        .bind(&row.key)
        .bind(playbook_id)
        .bind(build_id)
        .bind(row.trades)
        .bind(row.wins)
        .bind(row.losses)
        .bind(row.no_entries)
        .bind(win_rate)
        .bind(avg_r)
        .bind(row.sum_r)
        .bind(row.best_r)
        .bind(row.worst_r)
        .bind(row.daily_loss_day_trades)
        .bind(row.daily_loss_days.len() as i64)
        .bind(row.family_keys.len() as i64)
        .bind(row.template_uids.len() as i64)
        .bind(row.contract_symbols.len() as i64)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn store_family_contribution_rows(
    pool: &MySqlPool,
    sim_run_id: &str,
    playbook_id: Option<&str>,
    build_id: &str,
    rows: &[ContributionRow],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM entry_exit_playbook_sim_family_contribution WHERE sim_run_id = ?")
        .bind(sim_run_id)
        .execute(pool)
        .await?;

    for row in rows {
        let win_rate = if row.trades > 0 {
            row.wins as f64 / row.trades as f64 * 100.0
        } else {
            0.0
        };
        let avg_r = if row.trades > 0 {
            row.sum_r / row.trades as f64
        } else {
            0.0
        };

        sqlx::query(
            r#"
            INSERT INTO entry_exit_playbook_sim_family_contribution (
                sim_run_id,
                family_key,
                playbook_id,
                build_id,
                trades,
                wins,
                losses,
                no_entries,
                win_rate,
                avg_r,
                sum_r,
                best_r,
                worst_r,
                daily_loss_day_trades,
                daily_loss_day_count,
                symbol_count,
                template_count,
                contract_count
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(sim_run_id)
        .bind(&row.key)
        .bind(playbook_id)
        .bind(build_id)
        .bind(row.trades)
        .bind(row.wins)
        .bind(row.losses)
        .bind(row.no_entries)
        .bind(win_rate)
        .bind(avg_r)
        .bind(row.sum_r)
        .bind(row.best_r)
        .bind(row.worst_r)
        .bind(row.daily_loss_day_trades)
        .bind(row.daily_loss_days.len() as i64)
        .bind(row.root_symbols.len() as i64)
        .bind(row.template_uids.len() as i64)
        .bind(row.contract_symbols.len() as i64)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn store_streaks(
    pool: &MySqlPool,
    sim_run_id: &str,
    playbook_id: Option<&str>,
    build_id: &str,
    streaks: &[StreakRow],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM entry_exit_playbook_sim_streaks WHERE sim_run_id = ?")
        .bind(sim_run_id)
        .execute(pool)
        .await?;

    for streak in streaks {
        sqlx::query(
            r#"
            INSERT INTO entry_exit_playbook_sim_streaks (
                sim_run_id,
                streak_number,
                playbook_id,
                build_id,
                streak_type,
                start_index,
                end_index,
                start_date,
                end_date,
                streak_length,
                sum_r
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(sim_run_id)
        .bind(streak.streak_number)
        .bind(playbook_id)
        .bind(build_id)
        .bind(&streak.streak_type)
        .bind(streak.start_index)
        .bind(streak.end_index)
        .bind(streak.start_date)
        .bind(streak.end_date)
        .bind(streak.streak_length)
        .bind(streak.sum_r)
        .execute(pool)
        .await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let args = parse_args()?;
    let pool = MySqlPool::connect(&database_url_from_env()?).await?;
    ensure_tables(&pool).await?;

    let build_id = load_build_id(&pool, &args.sim_run_id).await?;
    let sim_summary_rows =
        store_sim_run_summary(&pool, &args.sim_run_id, args.playbook_id.as_deref()).await?;
    let daily_loss_lockout = load_daily_loss_lockout(&pool, &args.sim_run_id).await?;
    let trades = load_trades(&pool, &args.sim_run_id).await?;
    if trades.is_empty() {
        return Err(format!("Simulation {} has no result rows.", args.sim_run_id).into());
    }

    let (summary, cycles) = compute_prop_summary(&trades, daily_loss_lockout);
    let equity_points = compute_equity_points(&trades, daily_loss_lockout);
    let daily_r_rows = compute_daily_r(&trades);
    let hourly_rows = compute_hourly_performance(&trades, &daily_r_rows);
    let trade_cadence = compute_trade_cadence(&trades);
    let symbol_contribution_rows =
        compute_contribution_rows(&trades, &daily_r_rows, |trade| trade.root_symbol.clone());
    let family_contribution_rows =
        compute_contribution_rows(&trades, &daily_r_rows, |trade| trade.family_key.clone());
    let streaks = compute_streaks(&trades);
    let (loss_summary, loss_gap_buckets, loss_windows) =
        compute_loss_clustering(&trades, &streaks);
    store_prop_summary(
        &pool,
        &args.sim_run_id,
        args.playbook_id.as_deref(),
        &build_id,
        &summary,
        &cycles,
    )
    .await?;
    store_equity_points(
        &pool,
        &args.sim_run_id,
        args.playbook_id.as_deref(),
        &build_id,
        &equity_points,
    )
    .await?;
    store_daily_r_rows(
        &pool,
        &args.sim_run_id,
        args.playbook_id.as_deref(),
        &build_id,
        &daily_r_rows,
    )
    .await?;
    store_hourly_performance_rows(
        &pool,
        &args.sim_run_id,
        args.playbook_id.as_deref(),
        &build_id,
        &hourly_rows,
    )
    .await?;
    store_trade_cadence(
        &pool,
        &args.sim_run_id,
        args.playbook_id.as_deref(),
        &build_id,
        &trade_cadence,
    )
    .await?;
    store_loss_clustering(
        &pool,
        &args.sim_run_id,
        args.playbook_id.as_deref(),
        &build_id,
        &loss_summary,
        &loss_gap_buckets,
        &loss_windows,
    )
    .await?;
    store_symbol_contribution_rows(
        &pool,
        &args.sim_run_id,
        args.playbook_id.as_deref(),
        &build_id,
        &symbol_contribution_rows,
    )
    .await?;
    store_family_contribution_rows(
        &pool,
        &args.sim_run_id,
        args.playbook_id.as_deref(),
        &build_id,
        &family_contribution_rows,
    )
    .await?;
    store_streaks(
        &pool,
        &args.sim_run_id,
        args.playbook_id.as_deref(),
        &build_id,
        &streaks,
    )
    .await?;

    println!("Stored sim prop data");
    println!("sim_run_id={}", args.sim_run_id);
    println!("build_id={build_id}");
    if let Some(playbook_id) = args.playbook_id {
        println!("playbook_id={playbook_id}");
    }
    println!("sim_summary_rows={sim_summary_rows}");
    println!(
        "daily_loss_lockout={}",
        if daily_loss_lockout { "on" } else { "off" }
    );
    println!("trades={}", trades.len());
    println!("equity_points={}", equity_points.len());
    println!("daily_r_days={}", daily_r_rows.len());
    println!("hourly_rows={}", hourly_rows.len());
    println!("trade_cadence_trades={}", trade_cadence.trades);
    println!("losses={}", loss_summary.losses);
    println!("loss_gap_buckets={}", loss_gap_buckets.len());
    println!("loss_windows={}", loss_windows.len().min(200));
    println!(
        "symbol_contribution_rows={}",
        symbol_contribution_rows.len()
    );
    println!(
        "family_contribution_rows={}",
        family_contribution_rows.len()
    );
    println!("streaks={}", streaks.len());
    println!("cycles={}", summary.cycles);
    println!("passed={}", summary.passed);
    println!("daily_fails={}", summary.daily_fails);
    println!("drawdown_fails={}", summary.drawdown_fails);
    println!("incomplete={}", summary.incomplete);
    println!("pass_rate={:.2}%", summary.pass_rate);
    println!("closed_pass_rate={:.2}%", summary.closed_pass_rate);

    Ok(())
}
