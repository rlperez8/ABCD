use std::collections::{HashMap, HashSet, VecDeque};
use std::env;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use chrono::{Duration, NaiveDate, NaiveDateTime, Timelike};
use serde_json::Value;
use sqlx::{mysql::MySqlPool, Row};

#[derive(Debug)]
struct Args {
    train_run_id: Option<String>,
    reuse_router_run_id: Option<String>,
    test_year: i64,
    source_scope: String,
    source_timeframe: Option<String>,
    min_train_tests: i64,
    sister_window_minutes: i64,
    limit: i64,
    prop_filter: bool,
    trade_min_tests: i64,
    trade_min_win_rate: f64,
    trade_min_avg_r: f64,
    watchlist_min_tests: i64,
    watchlist_min_win_rate: f64,
    watchlist_min_avg_r: f64,
    symbol_filter: bool,
    symbol_min_tests: i64,
    symbol_min_win_rate: f64,
    symbol_min_avg_r: f64,
    one_trade_at_a_time: bool,
    one_trade_per_root_symbol: bool,
    one_trade_per_minute: bool,
    trade_cooldown_minutes: i64,
    daily_loss_lockout: bool,
    near_pass_protection: bool,
    near_pass_within_r: f64,
    near_pass_daily_loss_r: f64,
    loss_cluster_day_lockout: bool,
    loss_cluster_loss_count: i64,
    loss_cluster_window_minutes: i64,
    playbook_description: Option<String>,
    ignore_manual_family_skips: bool,
}

#[derive(Clone, sqlx::FromRow)]
struct PatternSetup {
    setup_id: String,
    pattern_id: Option<String>,
    pattern_group_id: String,
    event_id: Option<String>,
    event_rank: Option<i64>,
    event_sister_count: Option<i64>,
    symbol: String,
    root_symbol: Option<String>,
    source_table: Option<String>,
    source_timeframe: Option<String>,
    market: String,
    pattern_family_key: Option<String>,
    d_date: NaiveDateTime,
    d_confirm_date: NaiveDateTime,
    cd_price_length: f64,
    full_pattern_length: i64,
}

#[derive(Clone)]
struct PatternDecisionEvent {
    event_key: String,
    event_id: Option<String>,
    decision_date: NaiveDateTime,
    candidates: Vec<PatternSetup>,
}

#[derive(Clone, sqlx::FromRow)]
struct ForwardCandle {
    candle_date: NaiveDateTime,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
}

#[derive(Clone)]
struct TemplateSpec {
    template_uid: String,
    template_label: String,
    template_name: String,
    entry_offset: i64,
    direction_mode: String,
    risk_multiple: f64,
    target_r: f64,
    max_hold_multiple: i64,
}

#[derive(Clone)]
struct RouterChoice {
    family_key: String,
    harmonic_type: String,
    market: String,
    family_bin: String,
    family_size_bucket: String,
    family_time_bin: String,
    family_x_strictness: String,
    template: TemplateSpec,
    rank: i64,
    score: f64,
    train_eval_count: i64,
    train_pass_count: i64,
    train_fail_count: i64,
    train_no_entry_count: i64,
    train_avg_r: f64,
    route_status: String,
    status_reason: String,
}

struct TemplateFamilyPerf {
    family_key: String,
    harmonic_type: String,
    market: String,
    family_bin: String,
    family_size_bucket: String,
    family_time_bin: String,
    family_x_strictness: String,
    template_uid: String,
    eval_count: i64,
    pass_count: i64,
    fail_count: i64,
    no_entry_count: i64,
    avg_r: f64,
}

#[derive(Clone)]
struct SymbolGateChoice {
    root_symbol: String,
    eval_count: i64,
    pass_count: i64,
    fail_count: i64,
    no_entry_count: i64,
    avg_r: f64,
    sum_r: f64,
    route_status: String,
    status_reason: String,
}

struct TemplateEvaluation {
    outcome: String,
    exit_reason: String,
    result_r: Option<f64>,
    entry_date: Option<NaiveDateTime>,
    exit_date: Option<NaiveDateTime>,
    entry_price: Option<f64>,
    stop_price: Option<f64>,
    target_price: Option<f64>,
    exit_price: Option<f64>,
    risk_points: Option<f64>,
    trade_direction: Option<String>,
}

#[derive(Clone)]
struct PropReplayTrade {
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
    sim_plays_used: i64,
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

struct OpenPropCycle {
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

impl OpenPropCycle {
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

    fn apply_trade(&mut self, trade: &PropReplayTrade) {
        let date = trade.event_date.date();
        if date != self.current_date {
            self.current_date = date;
            self.daily_r = 0.0;
        }

        self.trades += 1;
        match trade.outcome.as_str() {
            "pass" => self.wins += 1,
            "fail" => self.losses += 1,
            "no_entry" => self.no_entries += 1,
            _ => {}
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

#[derive(Default)]
struct RouterSummary {
    patterns_scanned: i64,
    raw_patterns_loaded: i64,
    routed_patterns: i64,
    no_route_patterns: i64,
    skipped_non_trade_patterns: i64,
    skipped_symbol_patterns: i64,
    skipped_overlap_patterns: i64,
    win_count: i64,
    loss_count: i64,
    no_entry_count: i64,
    sum_r: f64,
    best_r: f64,
    worst_r: f64,
}

impl RouterSummary {
    fn record(&mut self, evaluation: &TemplateEvaluation) {
        let result_r = evaluation.result_r.unwrap_or(0.0);
        if self.routed_patterns == 0 {
            self.best_r = result_r;
            self.worst_r = result_r;
        } else {
            self.best_r = self.best_r.max(result_r);
            self.worst_r = self.worst_r.min(result_r);
        }
        self.routed_patterns += 1;
        self.sum_r += result_r;
        match evaluation.outcome.as_str() {
            "pass" => self.win_count += 1,
            "fail" => self.loss_count += 1,
            "no_entry" => self.no_entry_count += 1,
            _ => {}
        }
    }

    fn avg_r(&self) -> f64 {
        if self.routed_patterns > 0 {
            self.sum_r / self.routed_patterns as f64
        } else {
            0.0
        }
    }
}

fn usage() -> &'static str {
    "Usage: cargo run --bin run_entry_exit_playbook_sim -- --playbook-id PLAYBOOK_ID [--test-year 2026] [--source futures] [--source-timeframe 1m|5m] [--limit 0] [--sister-window-minutes 15]\nPlaybook execution rules are loaded from --playbook-id. Create playbooks with create_entry_exit_playbook."
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|items| items[0] == name)
        .map(|items| items[1].clone())
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| arg == name)
}

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let raw_args = env::args().skip(1).collect::<Vec<_>>();
    if raw_args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", usage());
        std::process::exit(0);
    }

    Ok(Args {
        train_run_id: arg_value(&raw_args, "--train-run-id")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        reuse_router_run_id: arg_value(&raw_args, "--reuse-router-run-id")
            .or_else(|| arg_value(&raw_args, "--playbook-id"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        test_year: arg_value(&raw_args, "--test-year")
            .or_else(|| arg_value(&raw_args, "--year"))
            .as_deref()
            .unwrap_or("2026")
            .parse()?,
        source_scope: arg_value(&raw_args, "--source").unwrap_or_else(|| "futures".to_string()),
        source_timeframe: arg_value(&raw_args, "--source-timeframe")
            .or_else(|| arg_value(&raw_args, "--timeframe"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("all")),
        min_train_tests: arg_value(&raw_args, "--min-train-tests")
            .as_deref()
            .unwrap_or("25")
            .parse()?,
        sister_window_minutes: arg_value(&raw_args, "--sister-window-minutes")
            .as_deref()
            .unwrap_or("15")
            .parse()?,
        limit: arg_value(&raw_args, "--limit")
            .as_deref()
            .unwrap_or("0")
            .parse()?,
        prop_filter: has_flag(&raw_args, "--prop-filter"),
        trade_min_tests: arg_value(&raw_args, "--trade-min-tests")
            .as_deref()
            .unwrap_or("50")
            .parse()?,
        trade_min_win_rate: arg_value(&raw_args, "--trade-min-win-rate")
            .as_deref()
            .unwrap_or("0.24")
            .parse()?,
        trade_min_avg_r: arg_value(&raw_args, "--trade-min-avg-r")
            .as_deref()
            .unwrap_or("0.20")
            .parse()?,
        watchlist_min_tests: arg_value(&raw_args, "--watchlist-min-tests")
            .as_deref()
            .unwrap_or("25")
            .parse()?,
        watchlist_min_win_rate: arg_value(&raw_args, "--watchlist-min-win-rate")
            .as_deref()
            .unwrap_or("0.21")
            .parse()?,
        watchlist_min_avg_r: arg_value(&raw_args, "--watchlist-min-avg-r")
            .as_deref()
            .unwrap_or("0.05")
            .parse()?,
        symbol_filter: has_flag(&raw_args, "--symbol-filter"),
        symbol_min_tests: arg_value(&raw_args, "--symbol-min-tests")
            .as_deref()
            .unwrap_or("1000")
            .parse()?,
        symbol_min_win_rate: arg_value(&raw_args, "--symbol-min-win-rate")
            .as_deref()
            .unwrap_or("0.23")
            .parse()?,
        symbol_min_avg_r: arg_value(&raw_args, "--symbol-min-avg-r")
            .as_deref()
            .unwrap_or("0.05")
            .parse()?,
        one_trade_at_a_time: has_flag(&raw_args, "--one-trade-at-a-time"),
        one_trade_per_root_symbol: has_flag(&raw_args, "--one-trade-per-root-symbol"),
        one_trade_per_minute: has_flag(&raw_args, "--one-trade-per-minute"),
        trade_cooldown_minutes: arg_value(&raw_args, "--trade-cooldown-minutes")
            .or_else(|| arg_value(&raw_args, "--min-trade-gap-minutes"))
            .as_deref()
            .unwrap_or("0")
            .parse::<i64>()?
            .max(0),
        daily_loss_lockout: has_flag(&raw_args, "--daily-loss-lockout"),
        near_pass_protection: has_flag(&raw_args, "--near-pass-protection"),
        near_pass_within_r: arg_value(&raw_args, "--near-pass-within-r")
            .as_deref()
            .unwrap_or("10")
            .parse::<f64>()?
            .max(0.0),
        near_pass_daily_loss_r: arg_value(&raw_args, "--near-pass-daily-loss-r")
            .as_deref()
            .unwrap_or("3")
            .parse::<f64>()?
            .max(0.0),
        loss_cluster_day_lockout: has_flag(&raw_args, "--loss-cluster-day-lockout"),
        loss_cluster_loss_count: arg_value(&raw_args, "--loss-cluster-loss-count")
            .as_deref()
            .unwrap_or("3")
            .parse::<i64>()?
            .max(1),
        loss_cluster_window_minutes: arg_value(&raw_args, "--loss-cluster-window-minutes")
            .as_deref()
            .unwrap_or("60")
            .parse::<i64>()?
            .max(1),
        playbook_description: None,
        ignore_manual_family_skips: has_flag(&raw_args, "--ignore-manual-family-skips"),
    })
}

fn router_run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let pid = std::process::id();
    format!("eefr-{millis}-{pid}")
}

async fn latest_train_run_id(pool: &MySqlPool) -> Result<String, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT run_id
        FROM entry_exit_template_creator_runs
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .fetch_one(pool)
    .await
}

async fn reused_router_train_run_id(
    pool: &MySqlPool,
    reuse_router_run_id: &str,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT train_run_id
        FROM entry_exit_template_family_router_runs
        WHERE router_run_id = ?
        LIMIT 1
        "#,
    )
    .bind(reuse_router_run_id)
    .fetch_one(pool)
    .await
}

struct PlaybookExecutionRules {
    min_train_tests: i64,
    prop_filter: bool,
    trade_min_tests: i64,
    trade_min_win_rate: f64,
    trade_min_avg_r: f64,
    watchlist_min_tests: i64,
    watchlist_min_win_rate: f64,
    watchlist_min_avg_r: f64,
    symbol_filter: bool,
    symbol_min_tests: i64,
    symbol_min_win_rate: f64,
    symbol_min_avg_r: f64,
    one_trade_at_a_time: bool,
    one_trade_per_root_symbol: bool,
    one_trade_per_minute: bool,
    trade_cooldown_minutes: i64,
    daily_loss_lockout: bool,
    near_pass_protection: bool,
    near_pass_within_r: f64,
    near_pass_daily_loss_r: f64,
    loss_cluster_day_lockout: bool,
    loss_cluster_loss_count: i64,
    loss_cluster_window_minutes: i64,
    playbook_description: Option<String>,
}

async fn load_playbook_execution_rules(
    pool: &MySqlPool,
    playbook_id: &str,
) -> Result<PlaybookExecutionRules, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT
            min_train_tests,
            CAST(prop_filter_enabled AS SIGNED) AS prop_filter_enabled,
            trade_min_tests,
            trade_min_win_rate,
            trade_min_avg_r,
            watchlist_min_tests,
            watchlist_min_win_rate,
            watchlist_min_avg_r,
            CAST(symbol_filter_enabled AS SIGNED) AS symbol_filter_enabled,
            symbol_min_tests,
            symbol_min_win_rate,
            symbol_min_avg_r,
            CAST(one_trade_at_a_time AS SIGNED) AS one_trade_at_a_time,
            CAST(one_trade_per_root_symbol AS SIGNED) AS one_trade_per_root_symbol,
            CAST(one_trade_per_minute AS SIGNED) AS one_trade_per_minute,
            trade_cooldown_minutes,
            CAST(daily_loss_lockout AS SIGNED) AS daily_loss_lockout,
            CAST(COALESCE(near_pass_protection, 0) AS SIGNED) AS near_pass_protection,
            COALESCE(near_pass_within_r, 0) AS near_pass_within_r,
            COALESCE(near_pass_daily_loss_r, 0) AS near_pass_daily_loss_r,
            CAST(COALESCE(loss_cluster_day_lockout, 0) AS SIGNED) AS loss_cluster_day_lockout,
            COALESCE(loss_cluster_loss_count, 0) AS loss_cluster_loss_count,
            COALESCE(loss_cluster_window_minutes, 0) AS loss_cluster_window_minutes,
            playbook_description
        FROM entry_exit_template_family_router_runs
        WHERE router_run_id = ?
        LIMIT 1
        "#,
    )
    .bind(playbook_id)
    .fetch_one(pool)
    .await?;

    Ok(PlaybookExecutionRules {
        min_train_tests: row.try_get("min_train_tests")?,
        prop_filter: row.try_get::<i64, _>("prop_filter_enabled")? != 0,
        trade_min_tests: row.try_get("trade_min_tests")?,
        trade_min_win_rate: row.try_get("trade_min_win_rate")?,
        trade_min_avg_r: row.try_get("trade_min_avg_r")?,
        watchlist_min_tests: row.try_get("watchlist_min_tests")?,
        watchlist_min_win_rate: row.try_get("watchlist_min_win_rate")?,
        watchlist_min_avg_r: row.try_get("watchlist_min_avg_r")?,
        symbol_filter: row.try_get::<i64, _>("symbol_filter_enabled")? != 0,
        symbol_min_tests: row.try_get("symbol_min_tests")?,
        symbol_min_win_rate: row.try_get("symbol_min_win_rate")?,
        symbol_min_avg_r: row.try_get("symbol_min_avg_r")?,
        one_trade_at_a_time: row.try_get::<i64, _>("one_trade_at_a_time")? != 0,
        one_trade_per_root_symbol: row.try_get::<i64, _>("one_trade_per_root_symbol")? != 0,
        one_trade_per_minute: row.try_get::<i64, _>("one_trade_per_minute")? != 0,
        trade_cooldown_minutes: row
            .try_get::<Option<i64>, _>("trade_cooldown_minutes")?
            .unwrap_or(0)
            .max(0),
        daily_loss_lockout: row.try_get::<i64, _>("daily_loss_lockout")? != 0,
        near_pass_protection: row.try_get::<i64, _>("near_pass_protection")? != 0,
        near_pass_within_r: row.try_get("near_pass_within_r")?,
        near_pass_daily_loss_r: row.try_get("near_pass_daily_loss_r")?,
        loss_cluster_day_lockout: row.try_get::<i64, _>("loss_cluster_day_lockout")? != 0,
        loss_cluster_loss_count: row
            .try_get::<Option<i64>, _>("loss_cluster_loss_count")?
            .unwrap_or(0)
            .max(0),
        loss_cluster_window_minutes: row
            .try_get::<Option<i64>, _>("loss_cluster_window_minutes")?
            .unwrap_or(0)
            .max(0),
        playbook_description: row
            .try_get::<Option<String>, _>("playbook_description")?
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    })
}

fn apply_playbook_execution_rules(args: &mut Args, rules: PlaybookExecutionRules) {
    args.min_train_tests = rules.min_train_tests;
    args.prop_filter = rules.prop_filter;
    args.trade_min_tests = rules.trade_min_tests;
    args.trade_min_win_rate = rules.trade_min_win_rate;
    args.trade_min_avg_r = rules.trade_min_avg_r;
    args.watchlist_min_tests = rules.watchlist_min_tests;
    args.watchlist_min_win_rate = rules.watchlist_min_win_rate;
    args.watchlist_min_avg_r = rules.watchlist_min_avg_r;
    args.symbol_filter = rules.symbol_filter;
    args.symbol_min_tests = rules.symbol_min_tests;
    args.symbol_min_win_rate = rules.symbol_min_win_rate;
    args.symbol_min_avg_r = rules.symbol_min_avg_r;
    args.one_trade_at_a_time = rules.one_trade_at_a_time;
    args.one_trade_per_root_symbol = rules.one_trade_per_root_symbol;
    args.one_trade_per_minute = rules.one_trade_per_minute;
    args.trade_cooldown_minutes = rules.trade_cooldown_minutes;
    args.daily_loss_lockout = rules.daily_loss_lockout;
    args.near_pass_protection = rules.near_pass_protection;
    args.near_pass_within_r = rules.near_pass_within_r;
    args.near_pass_daily_loss_r = rules.near_pass_daily_loss_r;
    args.loss_cluster_day_lockout = rules.loss_cluster_day_lockout;
    args.loss_cluster_loss_count = rules.loss_cluster_loss_count.max(1);
    args.loss_cluster_window_minutes = rules.loss_cluster_window_minutes.max(1);
    args.playbook_description = rules.playbook_description;
}

async fn family_condition_stat_count(
    pool: &MySqlPool,
    train_run_id: &str,
) -> Result<i64, sqlx::Error> {
    let table_exists: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM information_schema.TABLES
        WHERE table_schema = DATABASE()
          AND table_name = 'entry_exit_template_condition_stats'
        "#,
    )
    .fetch_one(pool)
    .await?;

    if table_exists == 0 {
        return Ok(0);
    }

    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM entry_exit_template_condition_stats
        WHERE run_id = ?
          AND condition_type = 'pattern_family_key'
        "#,
    )
    .bind(train_run_id)
    .fetch_one(pool)
    .await
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
        FROM information_schema.COLUMNS
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
            "ALTER TABLE {table} ADD COLUMN {column} {definition}"
        ))
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn ensure_entry_exit_template_result_index(
    pool: &MySqlPool,
    index_name: &str,
    columns: &str,
    reason: &str,
) -> Result<(), sqlx::Error> {
    let table_exists: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM information_schema.TABLES
        WHERE table_schema = DATABASE()
          AND table_name = 'entry_exit_template_results'
        "#,
    )
    .fetch_one(pool)
    .await?;

    if table_exists == 0 {
        return Ok(());
    }

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

async fn ensure_tables(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    ensure_entry_exit_template_result_index(
        pool,
        "idx_entry_exit_template_results_run_family_template",
        "run_id, pattern_family_key, template_uid, market, outcome, result_r",
        "family-template model selection",
    )
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_template_family_router_runs (
            router_run_id VARCHAR(64) NOT NULL PRIMARY KEY,
            train_run_id VARCHAR(64) NOT NULL,
            source_scope VARCHAR(32) NOT NULL,
            test_year BIGINT NOT NULL,
            min_train_tests BIGINT NOT NULL,
            sister_window_minutes BIGINT NOT NULL,
            families_selected BIGINT NOT NULL DEFAULT 0,
            patterns_scanned BIGINT NOT NULL DEFAULT 0,
            routed_patterns BIGINT NOT NULL DEFAULT 0,
            no_route_patterns BIGINT NOT NULL DEFAULT 0,
            skipped_non_trade_patterns BIGINT NOT NULL DEFAULT 0,
            skipped_symbol_patterns BIGINT NOT NULL DEFAULT 0,
            trade_choices BIGINT NOT NULL DEFAULT 0,
            watchlist_choices BIGINT NOT NULL DEFAULT 0,
            skip_choices BIGINT NOT NULL DEFAULT 0,
            symbol_filter_enabled TINYINT NOT NULL DEFAULT 0,
            one_trade_at_a_time TINYINT NOT NULL DEFAULT 0,
            one_trade_per_root_symbol TINYINT NOT NULL DEFAULT 0,
            one_trade_per_minute TINYINT NOT NULL DEFAULT 0,
            trade_cooldown_minutes BIGINT NOT NULL DEFAULT 0,
            daily_loss_lockout TINYINT NOT NULL DEFAULT 0,
            near_pass_protection TINYINT NOT NULL DEFAULT 0,
            near_pass_within_r DOUBLE NOT NULL DEFAULT 0,
            near_pass_daily_loss_r DOUBLE NOT NULL DEFAULT 0,
            loss_cluster_day_lockout TINYINT NOT NULL DEFAULT 0,
            loss_cluster_loss_count BIGINT NOT NULL DEFAULT 0,
            loss_cluster_window_minutes BIGINT NOT NULL DEFAULT 0,
            playbook_description TEXT NULL,
            symbol_trade_roots BIGINT NOT NULL DEFAULT 0,
            symbol_skip_roots BIGINT NOT NULL DEFAULT 0,
            symbol_min_tests BIGINT NOT NULL DEFAULT 0,
            symbol_min_win_rate DOUBLE NOT NULL DEFAULT 0,
            symbol_min_avg_r DOUBLE NOT NULL DEFAULT 0,
            prop_filter_enabled TINYINT NOT NULL DEFAULT 0,
            trade_min_tests BIGINT NOT NULL DEFAULT 0,
            trade_min_win_rate DOUBLE NOT NULL DEFAULT 0,
            trade_min_avg_r DOUBLE NOT NULL DEFAULT 0,
            watchlist_min_tests BIGINT NOT NULL DEFAULT 0,
            watchlist_min_win_rate DOUBLE NOT NULL DEFAULT 0,
            watchlist_min_avg_r DOUBLE NOT NULL DEFAULT 0,
            win_count BIGINT NOT NULL DEFAULT 0,
            loss_count BIGINT NOT NULL DEFAULT 0,
            no_entry_count BIGINT NOT NULL DEFAULT 0,
            trade_win_rate DOUBLE NULL,
            avg_r DOUBLE NOT NULL DEFAULT 0,
            sum_r DOUBLE NOT NULL DEFAULT 0,
            best_r DOUBLE NOT NULL DEFAULT 0,
            worst_r DOUBLE NOT NULL DEFAULT 0,
            elapsed_ms BIGINT NOT NULL DEFAULT 0,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            INDEX idx_entry_exit_family_router_runs_train (train_run_id, test_year, created_at)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_template_family_router_choices (
            router_run_id VARCHAR(64) NOT NULL,
            train_run_id VARCHAR(64) NOT NULL,
            family_key VARCHAR(64) NOT NULL,
            template_uid VARCHAR(128) NOT NULL,
            template_label VARCHAR(16) NOT NULL,
            template_name VARCHAR(255) NOT NULL,
            template_rank BIGINT NOT NULL,
            score DOUBLE NOT NULL DEFAULT 0,
            train_eval_count BIGINT NOT NULL DEFAULT 0,
            train_pass_count BIGINT NOT NULL DEFAULT 0,
            train_fail_count BIGINT NOT NULL DEFAULT 0,
            train_no_entry_count BIGINT NOT NULL DEFAULT 0,
            train_avg_r DOUBLE NOT NULL DEFAULT 0,
            train_win_rate DOUBLE NOT NULL DEFAULT 0,
            train_fail_rate DOUBLE NOT NULL DEFAULT 0,
            route_status VARCHAR(16) NOT NULL DEFAULT 'TRADE',
            status_reason VARCHAR(255) NOT NULL DEFAULT '',
            harmonic_type VARCHAR(64) NOT NULL,
            market VARCHAR(16) NOT NULL,
            family_bin VARCHAR(32) NOT NULL,
            family_size_bucket VARCHAR(32) NOT NULL,
            family_time_bin VARCHAR(32) NOT NULL,
            family_x_strictness VARCHAR(32) NOT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (router_run_id, family_key, template_rank),
            INDEX idx_entry_exit_family_router_choice_template (router_run_id, template_uid)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_template_family_router_symbol_choices (
            router_run_id VARCHAR(64) NOT NULL,
            train_run_id VARCHAR(64) NOT NULL,
            root_symbol VARCHAR(32) NOT NULL,
            route_status VARCHAR(16) NOT NULL,
            status_reason VARCHAR(255) NOT NULL DEFAULT '',
            train_eval_count BIGINT NOT NULL DEFAULT 0,
            train_pass_count BIGINT NOT NULL DEFAULT 0,
            train_fail_count BIGINT NOT NULL DEFAULT 0,
            train_no_entry_count BIGINT NOT NULL DEFAULT 0,
            train_win_rate DOUBLE NOT NULL DEFAULT 0,
            train_fail_rate DOUBLE NOT NULL DEFAULT 0,
            train_avg_r DOUBLE NOT NULL DEFAULT 0,
            train_sum_r DOUBLE NOT NULL DEFAULT 0,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (router_run_id, root_symbol),
            INDEX idx_entry_exit_family_router_symbol_status (router_run_id, route_status)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_template_family_router_manual_family_skips (
            family_key VARCHAR(64) NOT NULL PRIMARY KEY,
            reason VARCHAR(255) NOT NULL DEFAULT '',
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;

    for (column, definition) in [
        (
            "skipped_non_trade_patterns",
            "BIGINT NOT NULL DEFAULT 0 AFTER no_route_patterns",
        ),
        (
            "skipped_symbol_patterns",
            "BIGINT NOT NULL DEFAULT 0 AFTER skipped_non_trade_patterns",
        ),
        ("source_timeframe", "VARCHAR(16) NULL AFTER source_scope"),
        (
            "skipped_overlap_patterns",
            "BIGINT NOT NULL DEFAULT 0 AFTER skipped_symbol_patterns",
        ),
        (
            "trade_choices",
            "BIGINT NOT NULL DEFAULT 0 AFTER skipped_overlap_patterns",
        ),
        (
            "watchlist_choices",
            "BIGINT NOT NULL DEFAULT 0 AFTER trade_choices",
        ),
        (
            "skip_choices",
            "BIGINT NOT NULL DEFAULT 0 AFTER watchlist_choices",
        ),
        (
            "prop_filter_enabled",
            "TINYINT NOT NULL DEFAULT 0 AFTER skip_choices",
        ),
        (
            "symbol_filter_enabled",
            "TINYINT NOT NULL DEFAULT 0 AFTER skip_choices",
        ),
        (
            "one_trade_at_a_time",
            "TINYINT NOT NULL DEFAULT 0 AFTER symbol_filter_enabled",
        ),
        (
            "one_trade_per_root_symbol",
            "TINYINT NOT NULL DEFAULT 0 AFTER one_trade_at_a_time",
        ),
        (
            "one_trade_per_minute",
            "TINYINT NOT NULL DEFAULT 0 AFTER one_trade_per_root_symbol",
        ),
        (
            "trade_cooldown_minutes",
            "BIGINT NOT NULL DEFAULT 0 AFTER one_trade_per_minute",
        ),
        (
            "daily_loss_lockout",
            "TINYINT NOT NULL DEFAULT 0 AFTER trade_cooldown_minutes",
        ),
        (
            "near_pass_protection",
            "TINYINT NOT NULL DEFAULT 0 AFTER daily_loss_lockout",
        ),
        (
            "near_pass_within_r",
            "DOUBLE NOT NULL DEFAULT 0 AFTER near_pass_protection",
        ),
        (
            "near_pass_daily_loss_r",
            "DOUBLE NOT NULL DEFAULT 0 AFTER near_pass_within_r",
        ),
        (
            "loss_cluster_day_lockout",
            "TINYINT NOT NULL DEFAULT 0 AFTER near_pass_daily_loss_r",
        ),
        (
            "loss_cluster_loss_count",
            "BIGINT NOT NULL DEFAULT 0 AFTER loss_cluster_day_lockout",
        ),
        (
            "loss_cluster_window_minutes",
            "BIGINT NOT NULL DEFAULT 0 AFTER loss_cluster_loss_count",
        ),
        (
            "playbook_description",
            "TEXT NULL AFTER loss_cluster_window_minutes",
        ),
        (
            "symbol_trade_roots",
            "BIGINT NOT NULL DEFAULT 0 AFTER playbook_description",
        ),
        (
            "symbol_skip_roots",
            "BIGINT NOT NULL DEFAULT 0 AFTER symbol_trade_roots",
        ),
        (
            "symbol_min_tests",
            "BIGINT NOT NULL DEFAULT 0 AFTER symbol_skip_roots",
        ),
        (
            "symbol_min_win_rate",
            "DOUBLE NOT NULL DEFAULT 0 AFTER symbol_min_tests",
        ),
        (
            "symbol_min_avg_r",
            "DOUBLE NOT NULL DEFAULT 0 AFTER symbol_min_win_rate",
        ),
        (
            "trade_min_tests",
            "BIGINT NOT NULL DEFAULT 0 AFTER prop_filter_enabled",
        ),
        (
            "trade_min_win_rate",
            "DOUBLE NOT NULL DEFAULT 0 AFTER trade_min_tests",
        ),
        (
            "trade_min_avg_r",
            "DOUBLE NOT NULL DEFAULT 0 AFTER trade_min_win_rate",
        ),
        (
            "watchlist_min_tests",
            "BIGINT NOT NULL DEFAULT 0 AFTER trade_min_avg_r",
        ),
        (
            "watchlist_min_win_rate",
            "DOUBLE NOT NULL DEFAULT 0 AFTER watchlist_min_tests",
        ),
        (
            "watchlist_min_avg_r",
            "DOUBLE NOT NULL DEFAULT 0 AFTER watchlist_min_win_rate",
        ),
    ] {
        ensure_column(
            pool,
            "entry_exit_template_family_router_runs",
            column,
            definition,
        )
        .await?;
    }

    for (column, definition) in [
        (
            "train_win_rate",
            "DOUBLE NOT NULL DEFAULT 0 AFTER train_avg_r",
        ),
        (
            "train_fail_rate",
            "DOUBLE NOT NULL DEFAULT 0 AFTER train_win_rate",
        ),
        (
            "route_status",
            "VARCHAR(16) NOT NULL DEFAULT 'TRADE' AFTER train_fail_rate",
        ),
        (
            "status_reason",
            "VARCHAR(255) NOT NULL DEFAULT '' AFTER route_status",
        ),
    ] {
        ensure_column(
            pool,
            "entry_exit_template_family_router_choices",
            column,
            definition,
        )
        .await?;
    }

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_template_family_router_results (
            id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
            router_run_id VARCHAR(64) NOT NULL,
            setup_id VARCHAR(64) NOT NULL,
            pattern_id VARCHAR(64) NULL,
            pattern_group_id VARCHAR(128) NOT NULL,
            event_id VARCHAR(64) NULL,
            event_rank BIGINT NULL,
            event_sister_count BIGINT NOT NULL DEFAULT 1,
            event_decision_date DATETIME NULL,
            event_candidate_count BIGINT NOT NULL DEFAULT 1,
            event_live_candidate_count BIGINT NOT NULL DEFAULT 1,
            family_key VARCHAR(64) NOT NULL,
            template_uid VARCHAR(128) NOT NULL,
            template_label VARCHAR(16) NOT NULL,
            symbol VARCHAR(32) NOT NULL,
            market VARCHAR(16) NOT NULL,
            d_date DATETIME NOT NULL,
            d_confirm_date DATETIME NOT NULL,
            outcome VARCHAR(16) NOT NULL,
            exit_reason VARCHAR(32) NOT NULL,
            result_r DOUBLE NULL,
            entry_date DATETIME NULL,
            exit_date DATETIME NULL,
            entry_price DOUBLE NULL,
            stop_price DOUBLE NULL,
            target_price DOUBLE NULL,
            exit_price DOUBLE NULL,
            risk_points DOUBLE NULL,
            trade_direction VARCHAR(8) NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE KEY uniq_entry_exit_family_router_result (router_run_id, setup_id),
            INDEX idx_entry_exit_family_router_result_template (router_run_id, template_uid, outcome),
            INDEX idx_entry_exit_family_router_result_family (router_run_id, family_key, outcome)
        )
        "#,
    )
    .execute(pool)
    .await?;

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
        "entry_exit_playbook_sim_runs",
        "trade_win_rate",
        "DOUBLE NULL AFTER no_entry_count",
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
            sim_plays_used BIGINT NULL,
            cycles BIGINT NOT NULL DEFAULT 0,
            passed BIGINT NOT NULL DEFAULT 0,
            daily_fails BIGINT NOT NULL DEFAULT 0,
            drawdown_fails BIGINT NOT NULL DEFAULT 0,
            incomplete BIGINT NOT NULL DEFAULT 0,
            pass_rate DOUBLE NOT NULL DEFAULT 0,
            closed_pass_rate DOUBLE NOT NULL DEFAULT 0,
            max_drawdown_r DOUBLE NOT NULL DEFAULT 0,
            max_loss_streak BIGINT NOT NULL DEFAULT 0,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            INDEX idx_entry_exit_sim_prop_summary_build (build_id, created_at),
            INDEX idx_entry_exit_sim_prop_summary_playbook (playbook_id, created_at)
        )
        "#,
    )
    .execute(pool)
    .await?;

    ensure_column(
        pool,
        "entry_exit_playbook_sim_prop_summary",
        "sim_plays_used",
        "BIGINT NULL AFTER daily_loss_r_limit",
    )
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
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (sim_run_id, cycle_number),
            INDEX idx_entry_exit_sim_prop_cycles_outcome (sim_run_id, outcome),
            INDEX idx_entry_exit_sim_prop_cycles_playbook (playbook_id, start_date)
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

    for (column, definition) in [
        ("event_id", "VARCHAR(64) NULL AFTER pattern_group_id"),
        ("event_rank", "BIGINT NULL AFTER event_id"),
        (
            "event_sister_count",
            "BIGINT NOT NULL DEFAULT 1 AFTER event_rank",
        ),
        (
            "event_decision_date",
            "DATETIME NULL AFTER event_sister_count",
        ),
        (
            "event_candidate_count",
            "BIGINT NOT NULL DEFAULT 1 AFTER event_decision_date",
        ),
        (
            "event_live_candidate_count",
            "BIGINT NOT NULL DEFAULT 1 AFTER event_candidate_count",
        ),
    ] {
        ensure_column(
            pool,
            "entry_exit_template_family_router_results",
            column,
            definition,
        )
        .await?;
    }

    Ok(())
}

fn parse_entry_offset(rule_json: &str) -> i64 {
    serde_json::from_str::<Value>(rule_json)
        .ok()
        .and_then(|value| {
            value
                .pointer("/entry/offset_from_confirmation")
                .and_then(Value::as_i64)
        })
        .unwrap_or(1)
        .max(1)
}

fn template_rule_label(template: &TemplateSpec) -> String {
    let direction = if template.direction_mode == "inverse_pattern" {
        "INV"
    } else {
        "PAT"
    };
    format!(
        "{direction} C+{} {:.3}CD {:.0}R",
        template.entry_offset, template.risk_multiple, template.target_r
    )
}

fn rate(count: i64, total: i64) -> f64 {
    if total > 0 {
        count as f64 / total as f64
    } else {
        0.0
    }
}

fn classify_choice(
    eval_count: i64,
    pass_count: i64,
    fail_count: i64,
    avg_r: f64,
    args: &Args,
) -> (String, String) {
    if !args.prop_filter {
        return ("TRADE".to_string(), "prop filter disabled".to_string());
    }

    let win_rate = rate(pass_count, eval_count);
    let fail_rate = rate(fail_count, eval_count);
    if eval_count >= args.trade_min_tests
        && win_rate >= args.trade_min_win_rate
        && avg_r >= args.trade_min_avg_r
    {
        return (
            "TRADE".to_string(),
            format!(
                "train {} tests / {:.1}% WR / {:.3}R avg / {:.1}% fail",
                eval_count,
                win_rate * 100.0,
                avg_r,
                fail_rate * 100.0
            ),
        );
    }

    if eval_count >= args.watchlist_min_tests
        && win_rate >= args.watchlist_min_win_rate
        && avg_r >= args.watchlist_min_avg_r
    {
        return (
            "WATCHLIST".to_string(),
            format!(
                "below trade bar: {} tests / {:.1}% WR / {:.3}R avg",
                eval_count,
                win_rate * 100.0,
                avg_r
            ),
        );
    }

    (
        "SKIP".to_string(),
        format!(
            "weak training: {} tests / {:.1}% WR / {:.3}R avg",
            eval_count,
            win_rate * 100.0,
            avg_r
        ),
    )
}

fn route_status_priority(status: &str) -> i64 {
    match status {
        "TRADE" => 3,
        "WATCHLIST" => 2,
        _ => 1,
    }
}

fn route_status_counts(choices: &HashMap<String, RouterChoice>) -> (i64, i64, i64) {
    let mut trade = 0;
    let mut watchlist = 0;
    let mut skip = 0;
    for choice in choices.values() {
        match choice.route_status.as_str() {
            "TRADE" => trade += 1,
            "WATCHLIST" => watchlist += 1,
            _ => skip += 1,
        }
    }
    (trade, watchlist, skip)
}

async fn load_manual_family_skips(
    pool: &MySqlPool,
) -> Result<HashMap<String, String>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT family_key, reason
        FROM entry_exit_template_family_router_manual_family_skips
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut skips = HashMap::new();
    for row in rows {
        let family_key: String = row.try_get("family_key")?;
        let reason: String = row.try_get("reason")?;
        let family_key = family_key.trim().to_string();
        if !family_key.is_empty() {
            skips.insert(family_key, reason);
        }
    }

    Ok(skips)
}

fn apply_manual_family_skips(
    choices: &mut HashMap<String, RouterChoice>,
    manual_skips: &HashMap<String, String>,
) -> usize {
    let mut applied = 0;
    for (family_key, reason) in manual_skips {
        let Some(choice) = choices.get_mut(family_key) else {
            continue;
        };

        choice.route_status = "SKIP".to_string();
        choice.status_reason = if reason.trim().is_empty() {
            "manual family skip".to_string()
        } else {
            format!("manual family skip: {}", reason.trim())
        };
        applied += 1;
    }

    applied
}

fn symbol_gate_counts(choices: &HashMap<String, SymbolGateChoice>) -> (i64, i64) {
    let mut trade = 0;
    let mut skip = 0;
    for choice in choices.values() {
        if choice.route_status == "TRADE" {
            trade += 1;
        } else {
            skip += 1;
        }
    }
    (trade, skip)
}

fn classify_symbol_gate(
    eval_count: i64,
    pass_count: i64,
    fail_count: i64,
    avg_r: f64,
    args: &Args,
) -> (String, String) {
    let win_rate = rate(pass_count, eval_count);
    let fail_rate = rate(fail_count, eval_count);
    if eval_count >= args.symbol_min_tests
        && win_rate >= args.symbol_min_win_rate
        && avg_r >= args.symbol_min_avg_r
    {
        return (
            "TRADE".to_string(),
            format!(
                "symbol train {} tests / {:.1}% WR / {:.3}R avg / {:.1}% fail",
                eval_count,
                win_rate * 100.0,
                avg_r,
                fail_rate * 100.0
            ),
        );
    }

    (
        "SKIP".to_string(),
        format!(
            "weak symbol training: {} tests / {:.1}% WR / {:.3}R avg",
            eval_count,
            win_rate * 100.0,
            avg_r
        ),
    )
}

async fn load_templates(
    pool: &MySqlPool,
    train_run_id: &str,
) -> Result<HashMap<String, TemplateSpec>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            t.template_uid,
            t.template_name,
            t.direction_mode,
            t.risk_multiple,
            t.target_r,
            CAST(t.max_hold_multiple AS SIGNED) AS max_hold_multiple,
            CAST(t.rule_json AS CHAR) AS rule_json,
            COALESCE(s.eval_count, 0) AS eval_count,
            COALESCE(s.pass_count, 0) AS pass_count,
            COALESCE(s.avg_r, 0) AS avg_r
        FROM entry_exit_templates t
        LEFT JOIN entry_exit_template_ui_stats s
          ON s.run_id = t.origin_run_id
         AND s.template_uid = t.template_uid
        WHERE t.origin_run_id = ?
        ORDER BY pass_count DESC, avg_r DESC, eval_count DESC, t.created_at ASC
        "#,
    )
    .bind(train_run_id)
    .fetch_all(pool)
    .await?;

    let mut templates = HashMap::new();
    for (index, row) in rows.into_iter().enumerate() {
        let template_uid: String = row.try_get("template_uid")?;
        let rule_json: String = row.try_get("rule_json")?;
        let mut template = TemplateSpec {
            template_uid: template_uid.clone(),
            template_label: format!("T{:02}", index + 1),
            template_name: row.try_get("template_name")?,
            entry_offset: parse_entry_offset(&rule_json),
            direction_mode: row.try_get("direction_mode")?,
            risk_multiple: row.try_get("risk_multiple")?,
            target_r: row.try_get("target_r")?,
            max_hold_multiple: row.try_get("max_hold_multiple")?,
        };
        template.template_name = template_rule_label(&template);
        templates.insert(template_uid, template);
    }

    Ok(templates)
}

fn template_score(eval_count: i64, pass_count: i64, fail_count: i64, avg_r: f64) -> f64 {
    let eval = eval_count.max(1) as f64;
    let win_rate = pass_count as f64 / eval;
    let fail_rate = fail_count as f64 / eval;
    avg_r * 100.0 + win_rate * 50.0 + eval.ln() * 5.0 - fail_rate * 25.0
}

async fn build_router_choices(
    pool: &MySqlPool,
    train_run_id: &str,
    templates: &HashMap<String, TemplateSpec>,
    args: &Args,
) -> Result<HashMap<String, RouterChoice>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            agg.template_uid,
            agg.family_key,
            COALESCE(meta.harmonic_type, 'Unknown') AS harmonic_type,
            COALESCE(meta.market, 'Unknown') AS market,
            COALESCE(meta.family_bin, 'Unknown') AS family_bin,
            COALESCE(meta.family_size_bucket, 'Unknown') AS family_size_bucket,
            COALESCE(meta.family_time_bin, 'Unknown') AS family_time_bin,
            COALESCE(meta.family_x_strictness, 'Unknown') AS family_x_strictness,
            agg.eval_count,
            agg.pass_count,
            agg.fail_count,
            agg.no_entry_count,
            agg.avg_r
        FROM (
            SELECT
                s.template_uid,
                s.condition_value AS family_key,
                CAST(s.eval_count AS SIGNED) AS eval_count,
                CAST(s.pass_count AS SIGNED) AS pass_count,
                CAST(s.fail_count AS SIGNED) AS fail_count,
                CAST(s.no_entry_count AS SIGNED) AS no_entry_count,
                s.avg_r
            FROM entry_exit_template_condition_stats s
            WHERE s.run_id = ?
              AND s.condition_type = 'pattern_family_key'
              AND COALESCE(NULLIF(s.condition_value, ''), '') <> ''
              AND s.condition_value <> 'Unknown'
              AND s.eval_count >= ?
              AND s.pass_count > 0
              AND s.avg_r > 0
        ) agg
        LEFT JOIN (
            SELECT
                pattern_family_key AS family_key,
                COALESCE(MAX(NULLIF(pattern_family_harmonic_type, '')), MAX(NULLIF(harmonic_type, '')), 'Unknown') AS harmonic_type,
                COALESCE(MAX(NULLIF(market, '')), 'Unknown') AS market,
                COALESCE(MAX(NULLIF(pattern_family_bin, '')), 'Unknown') AS family_bin,
                COALESCE(MAX(NULLIF(pattern_family_size_bucket, '')), 'Unknown') AS family_size_bucket,
                COALESCE(MAX(NULLIF(pattern_family_time_bin, '')), 'Unknown') AS family_time_bin,
                COALESCE(MAX(NULLIF(pattern_family_x_strictness, '')), 'Unknown') AS family_x_strictness
            FROM pattern_setups
            WHERE pattern_family_key IS NOT NULL
              AND pattern_family_key <> ''
            GROUP BY pattern_family_key
        ) meta
          ON meta.family_key = agg.family_key
        "#,
    )
    .bind(train_run_id)
    .bind(args.min_train_tests)
    .fetch_all(pool)
    .await?;

    let mut best = HashMap::new();
    for row in rows {
        let perf = TemplateFamilyPerf {
            family_key: row.try_get("family_key")?,
            harmonic_type: row.try_get("harmonic_type")?,
            market: row.try_get("market")?,
            family_bin: row.try_get("family_bin")?,
            family_size_bucket: row.try_get("family_size_bucket")?,
            family_time_bin: row.try_get("family_time_bin")?,
            family_x_strictness: row.try_get("family_x_strictness")?,
            template_uid: row.try_get("template_uid")?,
            eval_count: row.try_get("eval_count")?,
            pass_count: row.try_get("pass_count")?,
            fail_count: row.try_get("fail_count")?,
            no_entry_count: row.try_get("no_entry_count")?,
            avg_r: row.try_get("avg_r")?,
        };
        let Some(template) = templates.get(&perf.template_uid) else {
            continue;
        };
        let score = template_score(
            perf.eval_count,
            perf.pass_count,
            perf.fail_count,
            perf.avg_r,
        );
        let (route_status, status_reason) = classify_choice(
            perf.eval_count,
            perf.pass_count,
            perf.fail_count,
            perf.avg_r,
            args,
        );
        let choice = RouterChoice {
            family_key: perf.family_key.clone(),
            harmonic_type: perf.harmonic_type,
            market: perf.market,
            family_bin: perf.family_bin,
            family_size_bucket: perf.family_size_bucket,
            family_time_bin: perf.family_time_bin,
            family_x_strictness: perf.family_x_strictness,
            template: template.clone(),
            rank: 1,
            score,
            train_eval_count: perf.eval_count,
            train_pass_count: perf.pass_count,
            train_fail_count: perf.fail_count,
            train_no_entry_count: perf.no_entry_count,
            train_avg_r: perf.avg_r,
            route_status,
            status_reason,
        };

        best.entry(choice.family_key.clone())
            .and_modify(|current: &mut RouterChoice| {
                let choice_priority = route_status_priority(&choice.route_status);
                let current_priority = route_status_priority(&current.route_status);
                if choice_priority > current_priority
                    || (choice_priority == current_priority
                        && (choice.score > current.score
                            || (choice.score == current.score
                                && choice.train_eval_count > current.train_eval_count)))
                {
                    *current = choice.clone();
                }
            })
            .or_insert(choice);
    }

    Ok(best)
}

async fn build_symbol_gate_choices(
    pool: &MySqlPool,
    train_run_id: &str,
    choices: &HashMap<String, RouterChoice>,
    args: &Args,
) -> Result<HashMap<String, SymbolGateChoice>, sqlx::Error> {
    if !args.symbol_filter {
        return Ok(HashMap::new());
    }

    let template_uids = choices
        .values()
        .filter(|choice| choice.route_status == "TRADE")
        .map(|choice| choice.template.template_uid.clone())
        .collect::<HashSet<_>>();
    if template_uids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut rows_by_root: HashMap<String, SymbolGateChoice> = HashMap::new();
    for template_uid in template_uids {
        let rows = sqlx::query(
            r#"
            SELECT
                condition_value AS root_symbol,
                CAST(eval_count AS SIGNED) AS eval_count,
                CAST(pass_count AS SIGNED) AS pass_count,
                CAST(fail_count AS SIGNED) AS fail_count,
                CAST(no_entry_count AS SIGNED) AS no_entry_count,
                avg_r,
                sum_r
            FROM entry_exit_template_condition_stats
            WHERE run_id = ?
              AND template_uid = ?
              AND condition_type = 'root_symbol'
            "#,
        )
        .bind(train_run_id)
        .bind(&template_uid)
        .fetch_all(pool)
        .await?;

        for row in rows {
            let root_symbol = normalize_root_symbol(&row.try_get::<String, _>("root_symbol")?);
            if root_symbol.is_empty() || root_symbol == "UNKNOWN" || root_symbol == "N/A" {
                continue;
            }

            let eval_count: i64 = row.try_get("eval_count")?;
            let pass_count: i64 = row.try_get("pass_count")?;
            let fail_count: i64 = row.try_get("fail_count")?;
            let no_entry_count: i64 = row.try_get("no_entry_count")?;
            let sum_r: f64 = row.try_get("sum_r")?;

            rows_by_root
                .entry(root_symbol.clone())
                .and_modify(|choice| {
                    choice.eval_count += eval_count;
                    choice.pass_count += pass_count;
                    choice.fail_count += fail_count;
                    choice.no_entry_count += no_entry_count;
                    choice.sum_r += sum_r;
                    choice.avg_r = if choice.eval_count > 0 {
                        choice.sum_r / choice.eval_count as f64
                    } else {
                        0.0
                    };
                })
                .or_insert(SymbolGateChoice {
                    root_symbol,
                    eval_count,
                    pass_count,
                    fail_count,
                    no_entry_count,
                    avg_r: if eval_count > 0 {
                        sum_r / eval_count as f64
                    } else {
                        0.0
                    },
                    sum_r,
                    route_status: String::new(),
                    status_reason: String::new(),
                });
        }
    }

    for choice in rows_by_root.values_mut() {
        let (route_status, status_reason) = classify_symbol_gate(
            choice.eval_count,
            choice.pass_count,
            choice.fail_count,
            choice.avg_r,
            args,
        );
        choice.route_status = route_status;
        choice.status_reason = status_reason;
    }

    Ok(rows_by_root)
}

async fn load_reused_router_choices(
    pool: &MySqlPool,
    reuse_router_run_id: &str,
) -> Result<HashMap<String, RouterChoice>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            c.family_key,
            c.harmonic_type,
            c.market,
            c.family_bin,
            c.family_size_bucket,
            c.family_time_bin,
            c.family_x_strictness,
            c.template_uid,
            c.template_label,
            c.template_name,
            c.template_rank,
            c.score,
            c.train_eval_count,
            c.train_pass_count,
            c.train_fail_count,
            c.train_no_entry_count,
            c.train_avg_r,
            c.route_status,
            c.status_reason,
            t.direction_mode,
            t.risk_multiple,
            t.target_r,
            CAST(t.max_hold_multiple AS SIGNED) AS max_hold_multiple,
            CAST(t.rule_json AS CHAR) AS rule_json
        FROM entry_exit_template_family_router_choices c
        JOIN entry_exit_templates t
          ON t.template_uid = c.template_uid
        WHERE c.router_run_id = ?
        "#,
    )
    .bind(reuse_router_run_id)
    .fetch_all(pool)
    .await?;

    let mut choices = HashMap::new();
    for row in rows {
        let template_uid: String = row.try_get("template_uid")?;
        let rule_json: String = row.try_get("rule_json")?;
        let family_key: String = row.try_get("family_key")?;
        choices.insert(
            family_key.clone(),
            RouterChoice {
                family_key,
                harmonic_type: row.try_get("harmonic_type")?,
                market: row.try_get("market")?,
                family_bin: row.try_get("family_bin")?,
                family_size_bucket: row.try_get("family_size_bucket")?,
                family_time_bin: row.try_get("family_time_bin")?,
                family_x_strictness: row.try_get("family_x_strictness")?,
                template: TemplateSpec {
                    template_uid,
                    template_label: row.try_get("template_label")?,
                    template_name: row.try_get("template_name")?,
                    entry_offset: parse_entry_offset(&rule_json),
                    direction_mode: row.try_get("direction_mode")?,
                    risk_multiple: row.try_get("risk_multiple")?,
                    target_r: row.try_get("target_r")?,
                    max_hold_multiple: row.try_get("max_hold_multiple")?,
                },
                rank: row.try_get("template_rank")?,
                score: row.try_get("score")?,
                train_eval_count: row.try_get("train_eval_count")?,
                train_pass_count: row.try_get("train_pass_count")?,
                train_fail_count: row.try_get("train_fail_count")?,
                train_no_entry_count: row.try_get("train_no_entry_count")?,
                train_avg_r: row.try_get("train_avg_r")?,
                route_status: row.try_get("route_status")?,
                status_reason: row.try_get("status_reason")?,
            },
        );
    }

    Ok(choices)
}

async fn load_reused_symbol_gate_choices(
    pool: &MySqlPool,
    reuse_router_run_id: &str,
) -> Result<HashMap<String, SymbolGateChoice>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            root_symbol,
            route_status,
            status_reason,
            train_eval_count,
            train_pass_count,
            train_fail_count,
            train_no_entry_count,
            train_avg_r,
            train_sum_r
        FROM entry_exit_template_family_router_symbol_choices
        WHERE router_run_id = ?
        "#,
    )
    .bind(reuse_router_run_id)
    .fetch_all(pool)
    .await?;

    let mut choices = HashMap::new();
    for row in rows {
        let root_symbol = normalize_root_symbol(&row.try_get::<String, _>("root_symbol")?);
        choices.insert(
            root_symbol.clone(),
            SymbolGateChoice {
                root_symbol,
                eval_count: row.try_get("train_eval_count")?,
                pass_count: row.try_get("train_pass_count")?,
                fail_count: row.try_get("train_fail_count")?,
                no_entry_count: row.try_get("train_no_entry_count")?,
                avg_r: row.try_get("train_avg_r")?,
                sum_r: row.try_get("train_sum_r")?,
                route_status: row.try_get("route_status")?,
                status_reason: row.try_get("status_reason")?,
            },
        );
    }

    Ok(choices)
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

fn setup_gate_root_symbol(setup: &PatternSetup) -> String {
    let root = setup
        .root_symbol
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| root_symbol_from_contract(&setup.symbol));
    normalize_root_symbol(&root)
}

fn is_better_event_candidate(
    candidate_setup: &PatternSetup,
    candidate_choice: &RouterChoice,
    selected_setup: &PatternSetup,
    selected_choice: &RouterChoice,
) -> bool {
    if candidate_choice.score != selected_choice.score {
        return candidate_choice.score > selected_choice.score;
    }
    if candidate_choice.train_avg_r != selected_choice.train_avg_r {
        return candidate_choice.train_avg_r > selected_choice.train_avg_r;
    }
    if candidate_choice.train_eval_count != selected_choice.train_eval_count {
        return candidate_choice.train_eval_count > selected_choice.train_eval_count;
    }

    let candidate_rank = candidate_setup.event_rank.unwrap_or(i64::MAX);
    let selected_rank = selected_setup.event_rank.unwrap_or(i64::MAX);
    if candidate_rank != selected_rank {
        return candidate_rank < selected_rank;
    }

    candidate_setup.setup_id < selected_setup.setup_id
}

fn pattern_event_key(setup: &PatternSetup) -> String {
    setup
        .event_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("setup:{}", setup.setup_id))
}

fn build_twin_decision_events(
    patterns: Vec<PatternSetup>,
    limit: Option<usize>,
) -> Vec<PatternDecisionEvent> {
    let mut group_indexes: HashMap<String, usize> = HashMap::new();
    let mut events: Vec<PatternDecisionEvent> = Vec::new();

    for setup in patterns {
        let event_key = pattern_event_key(&setup);
        if let Some(index) = group_indexes.get(&event_key).copied() {
            let event = &mut events[index];
            event.decision_date = event.decision_date.min(setup.d_confirm_date);
            event.candidates.push(setup);
        } else {
            group_indexes.insert(event_key.clone(), events.len());
            events.push(PatternDecisionEvent {
                event_key,
                event_id: setup
                    .event_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string),
                decision_date: setup.d_confirm_date,
                candidates: vec![setup],
            });
        }
    }

    for event in &mut events {
        event.candidates.sort_by(|left, right| {
            left.d_confirm_date
                .cmp(&right.d_confirm_date)
                .then(
                    left.event_rank
                        .unwrap_or(i64::MAX)
                        .cmp(&right.event_rank.unwrap_or(i64::MAX)),
                )
                .then(left.setup_id.cmp(&right.setup_id))
        });
    }

    events.sort_by(|left, right| {
        left.decision_date
            .cmp(&right.decision_date)
            .then(left.event_key.cmp(&right.event_key))
    });

    if let Some(limit) = limit {
        events.truncate(limit);
    }

    events
}

async fn fetch_test_patterns(
    pool: &MySqlPool,
    args: &Args,
) -> Result<Vec<PatternSetup>, sqlx::Error> {
    let source_filter = match args.source_scope.as_str() {
        "futures" => "AND COALESCE(ps.source_table, '') LIKE 'futures_contract_%_candles'",
        "daily" => {
            "AND COALESCE(ps.source_table, '') = 'candles' AND COALESCE(ps.source_timeframe, '') = 'daily'"
        }
        _ => "",
    };
    let timeframe_filter = if args.source_timeframe.is_some() {
        "AND COALESCE(ps.source_timeframe, '') = ?"
    } else {
        ""
    };
    let raw_candidate_limit = if args.limit > 0 {
        Some(args.limit.saturating_mul(10).max(args.limit).min(100_000))
    } else {
        None
    };
    let limit_clause = if raw_candidate_limit.is_some() {
        "LIMIT ?"
    } else {
        ""
    };
    let sql = format!(
        r#"
        SELECT
            ps.setup_id,
            ps.pattern_id,
            ps.pattern_group_id,
            ps.event_id,
            CAST(ps.event_rank AS SIGNED) AS event_rank,
            CAST(ps.event_sister_count AS SIGNED) AS event_sister_count,
            ps.symbol,
            ps.root_symbol,
            ps.source_table,
            ps.source_timeframe,
            ps.market,
            ps.pattern_family_key,
            ps.d_date,
            CAST(COALESCE(ps.d_confirm_date, ps.d_date) AS DATETIME) AS d_confirm_date,
            ps.cd_price_length,
            CAST(ps.full_pattern_length AS SIGNED) AS full_pattern_length
        FROM pattern_setups ps
        WHERE ps.d_date IS NOT NULL
          AND ps.full_pattern_length > 0
          AND ABS(ps.cd_price_length) > 0
          AND YEAR(COALESCE(ps.d_confirm_date, ps.d_date)) = ?
          {source_filter}
          {timeframe_filter}
        ORDER BY COALESCE(ps.d_confirm_date, ps.d_date) ASC, ps.setup_id ASC
        {limit_clause}
        "#,
        source_filter = source_filter,
        timeframe_filter = timeframe_filter,
        limit_clause = limit_clause,
    );

    let mut query = sqlx::query_as::<_, PatternSetup>(&sql).bind(args.test_year);
    if let Some(source_timeframe) = args.source_timeframe.as_deref() {
        query = query.bind(source_timeframe);
    }
    if let Some(raw_candidate_limit) = raw_candidate_limit {
        query = query.bind(raw_candidate_limit);
    }
    query.fetch_all(pool).await
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
    setup: &PatternSetup,
) -> Result<Vec<ForwardCandle>, sqlx::Error> {
    let max_forward_bars = setup.full_pattern_length.saturating_mul(5).max(1);
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
        return sqlx::query_as::<_, ForwardCandle>(
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

    sqlx::query_as::<_, ForwardCandle>(&sql)
        .bind(&setup.symbol)
        .bind(setup.d_confirm_date)
        .bind(max_forward_bars.saturating_add(1))
        .fetch_all(pool)
        .await
}

fn setup_direction(setup: &PatternSetup) -> f64 {
    if setup.market.eq_ignore_ascii_case("Bearish") {
        -1.0
    } else {
        1.0
    }
}

fn forward_start_index(setup: &PatternSetup, candles: &[ForwardCandle]) -> usize {
    candles
        .first()
        .filter(|candle| candle.candle_date <= setup.d_confirm_date)
        .map(|_| 1)
        .unwrap_or(0)
}

fn direction_for_mode(setup: &PatternSetup, direction_mode: &str) -> f64 {
    let base = setup_direction(setup);
    if direction_mode == "inverse_pattern" {
        -base
    } else {
        base
    }
}

fn no_entry(reason: &str) -> TemplateEvaluation {
    TemplateEvaluation {
        outcome: "no_entry".to_string(),
        exit_reason: reason.to_string(),
        result_r: None,
        entry_date: None,
        exit_date: None,
        entry_price: None,
        stop_price: None,
        target_price: None,
        exit_price: None,
        risk_points: None,
        trade_direction: None,
    }
}

fn evaluate_template(
    template: &TemplateSpec,
    setup: &PatternSetup,
    candles: &[ForwardCandle],
) -> TemplateEvaluation {
    if candles.is_empty() {
        return no_entry("no_candles");
    }

    let start_index = forward_start_index(setup, candles);
    let entry_offset = template.entry_offset.max(1) as usize;
    let entry_index = start_index.saturating_add(entry_offset - 1);
    let Some(entry_candle) = candles.get(entry_index) else {
        return no_entry("entry_offset_missing");
    };

    let cd_length = setup.cd_price_length.abs();
    let risk_points = cd_length * template.risk_multiple;
    if !risk_points.is_finite() || risk_points <= 0.0 {
        return no_entry("invalid_risk");
    }

    let direction = direction_for_mode(setup, &template.direction_mode);
    let entry_price = entry_candle.open;
    let stop_price = entry_price - direction * risk_points;
    let target_price = entry_price + direction * risk_points * template.target_r;
    let max_hold_bars = setup
        .full_pattern_length
        .saturating_mul(template.max_hold_multiple.max(1))
        .max(1) as usize;
    let end_index = candles.len().min(entry_index.saturating_add(max_hold_bars));
    if end_index <= entry_index {
        return no_entry("hold_window_missing");
    }

    for (offset, candle) in candles[entry_index..end_index].iter().enumerate() {
        let candle_index = entry_index + offset;
        let stop_hit = (direction > 0.0 && candle.low <= stop_price)
            || (direction < 0.0 && candle.high >= stop_price);
        if stop_hit {
            return TemplateEvaluation {
                outcome: "fail".to_string(),
                exit_reason: "stop".to_string(),
                result_r: Some(-1.0),
                entry_date: Some(entry_candle.candle_date),
                exit_date: Some(candles[candle_index].candle_date),
                entry_price: Some(entry_price),
                stop_price: Some(stop_price),
                target_price: Some(target_price),
                exit_price: Some(stop_price),
                risk_points: Some(risk_points),
                trade_direction: Some(if direction > 0.0 { "long" } else { "short" }.to_string()),
            };
        }

        let target_hit = (direction > 0.0 && candle.high >= target_price)
            || (direction < 0.0 && candle.low <= target_price);
        if target_hit {
            return TemplateEvaluation {
                outcome: "pass".to_string(),
                exit_reason: "target".to_string(),
                result_r: Some(template.target_r),
                entry_date: Some(entry_candle.candle_date),
                exit_date: Some(candles[candle_index].candle_date),
                entry_price: Some(entry_price),
                stop_price: Some(stop_price),
                target_price: Some(target_price),
                exit_price: Some(target_price),
                risk_points: Some(risk_points),
                trade_direction: Some(if direction > 0.0 { "long" } else { "short" }.to_string()),
            };
        }
    }

    let exit_index = end_index - 1;
    let exit_price = candles[exit_index].close;
    let result_r = ((exit_price - entry_price) * direction) / risk_points;
    TemplateEvaluation {
        outcome: if result_r > 0.0 { "pass" } else { "fail" }.to_string(),
        exit_reason: "time".to_string(),
        result_r: Some(result_r),
        entry_date: Some(entry_candle.candle_date),
        exit_date: Some(candles[exit_index].candle_date),
        entry_price: Some(entry_price),
        stop_price: Some(stop_price),
        target_price: Some(target_price),
        exit_price: Some(exit_price),
        risk_points: Some(risk_points),
        trade_direction: Some(if direction > 0.0 { "long" } else { "short" }.to_string()),
    }
}

async fn insert_choices(
    pool: &MySqlPool,
    router_run_id: &str,
    train_run_id: &str,
    choices: &HashMap<String, RouterChoice>,
) -> Result<(), sqlx::Error> {
    for choice in choices.values() {
        sqlx::query(
            r#"
            INSERT INTO entry_exit_template_family_router_choices (
                router_run_id,
                train_run_id,
                family_key,
                template_uid,
                template_label,
                template_name,
                template_rank,
                score,
                train_eval_count,
                train_pass_count,
                train_fail_count,
                train_no_entry_count,
                train_avg_r,
                train_win_rate,
                train_fail_rate,
                route_status,
                status_reason,
                harmonic_type,
                market,
                family_bin,
                family_size_bucket,
                family_time_bin,
                family_x_strictness
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(router_run_id)
        .bind(train_run_id)
        .bind(&choice.family_key)
        .bind(&choice.template.template_uid)
        .bind(&choice.template.template_label)
        .bind(&choice.template.template_name)
        .bind(choice.rank)
        .bind(choice.score)
        .bind(choice.train_eval_count)
        .bind(choice.train_pass_count)
        .bind(choice.train_fail_count)
        .bind(choice.train_no_entry_count)
        .bind(choice.train_avg_r)
        .bind(rate(choice.train_pass_count, choice.train_eval_count))
        .bind(rate(choice.train_fail_count, choice.train_eval_count))
        .bind(&choice.route_status)
        .bind(&choice.status_reason)
        .bind(&choice.harmonic_type)
        .bind(&choice.market)
        .bind(&choice.family_bin)
        .bind(&choice.family_size_bucket)
        .bind(&choice.family_time_bin)
        .bind(&choice.family_x_strictness)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn insert_symbol_gate_choices(
    pool: &MySqlPool,
    router_run_id: &str,
    train_run_id: &str,
    choices: &HashMap<String, SymbolGateChoice>,
) -> Result<(), sqlx::Error> {
    for choice in choices.values() {
        sqlx::query(
            r#"
            INSERT INTO entry_exit_template_family_router_symbol_choices (
                router_run_id,
                train_run_id,
                root_symbol,
                route_status,
                status_reason,
                train_eval_count,
                train_pass_count,
                train_fail_count,
                train_no_entry_count,
                train_win_rate,
                train_fail_rate,
                train_avg_r,
                train_sum_r
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(router_run_id)
        .bind(train_run_id)
        .bind(&choice.root_symbol)
        .bind(&choice.route_status)
        .bind(&choice.status_reason)
        .bind(choice.eval_count)
        .bind(choice.pass_count)
        .bind(choice.fail_count)
        .bind(choice.no_entry_count)
        .bind(rate(choice.pass_count, choice.eval_count))
        .bind(rate(choice.fail_count, choice.eval_count))
        .bind(choice.avg_r)
        .bind(choice.sum_r)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn insert_result(
    pool: &MySqlPool,
    router_run_id: &str,
    event: &PatternDecisionEvent,
    live_candidate_count: i64,
    setup: &PatternSetup,
    choice: &RouterChoice,
    evaluation: &TemplateEvaluation,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO entry_exit_template_family_router_results (
            router_run_id,
            setup_id,
            pattern_id,
            pattern_group_id,
            event_id,
            event_rank,
            event_sister_count,
            event_decision_date,
            event_candidate_count,
            event_live_candidate_count,
            family_key,
            template_uid,
            template_label,
            symbol,
            market,
            d_date,
            d_confirm_date,
            outcome,
            exit_reason,
            result_r,
            entry_date,
            exit_date,
            entry_price,
            stop_price,
            target_price,
            exit_price,
            risk_points,
            trade_direction
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(router_run_id)
    .bind(&setup.setup_id)
    .bind(&setup.pattern_id)
    .bind(&setup.pattern_group_id)
    .bind(&event.event_id)
    .bind(setup.event_rank)
    .bind(setup.event_sister_count.unwrap_or(1))
    .bind(event.decision_date)
    .bind(event.candidates.len() as i64)
    .bind(live_candidate_count)
    .bind(&choice.family_key)
    .bind(&choice.template.template_uid)
    .bind(&choice.template.template_label)
    .bind(&setup.symbol)
    .bind(&setup.market)
    .bind(setup.d_date)
    .bind(setup.d_confirm_date)
    .bind(&evaluation.outcome)
    .bind(&evaluation.exit_reason)
    .bind(evaluation.result_r)
    .bind(evaluation.entry_date)
    .bind(evaluation.exit_date)
    .bind(evaluation.entry_price)
    .bind(evaluation.stop_price)
    .bind(evaluation.target_price)
    .bind(evaluation.exit_price)
    .bind(evaluation.risk_points)
    .bind(&evaluation.trade_direction)
    .execute(pool)
    .await?;

    Ok(())
}

async fn insert_run_summary(
    pool: &MySqlPool,
    router_run_id: &str,
    train_run_id: &str,
    args: &Args,
    families_selected: i64,
    trade_choices: i64,
    watchlist_choices: i64,
    skip_choices: i64,
    symbol_trade_roots: i64,
    symbol_skip_roots: i64,
    summary: &RouterSummary,
    elapsed_ms: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO entry_exit_template_family_router_runs (
            router_run_id,
            train_run_id,
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
            elapsed_ms
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(router_run_id)
    .bind(train_run_id)
    .bind(&args.source_scope)
    .bind(args.source_timeframe.as_deref())
    .bind(args.test_year)
    .bind(args.min_train_tests)
    .bind(args.sister_window_minutes)
    .bind(families_selected)
    .bind(summary.patterns_scanned)
    .bind(summary.routed_patterns)
    .bind(summary.no_route_patterns)
    .bind(summary.skipped_non_trade_patterns)
    .bind(summary.skipped_symbol_patterns)
    .bind(summary.skipped_overlap_patterns)
    .bind(trade_choices)
    .bind(watchlist_choices)
    .bind(skip_choices)
    .bind(if args.symbol_filter { 1 } else { 0 })
    .bind(if args.one_trade_at_a_time { 1 } else { 0 })
    .bind(if args.one_trade_per_root_symbol { 1 } else { 0 })
    .bind(if args.one_trade_per_minute { 1 } else { 0 })
    .bind(args.trade_cooldown_minutes)
    .bind(if args.daily_loss_lockout { 1 } else { 0 })
    .bind(if args.near_pass_protection { 1 } else { 0 })
    .bind(args.near_pass_within_r)
    .bind(args.near_pass_daily_loss_r)
    .bind(if args.loss_cluster_day_lockout { 1 } else { 0 })
    .bind(args.loss_cluster_loss_count)
    .bind(args.loss_cluster_window_minutes)
    .bind(args.playbook_description.as_deref())
    .bind(symbol_trade_roots)
    .bind(symbol_skip_roots)
    .bind(args.symbol_min_tests)
    .bind(args.symbol_min_win_rate)
    .bind(args.symbol_min_avg_r)
    .bind(if args.prop_filter { 1 } else { 0 })
    .bind(args.trade_min_tests)
    .bind(args.trade_min_win_rate)
    .bind(args.trade_min_avg_r)
    .bind(args.watchlist_min_tests)
    .bind(args.watchlist_min_win_rate)
    .bind(args.watchlist_min_avg_r)
    .bind(summary.win_count)
    .bind(summary.loss_count)
    .bind(summary.no_entry_count)
    .bind(summary.avg_r())
    .bind(summary.sum_r)
    .bind(summary.best_r)
    .bind(summary.worst_r)
    .bind(elapsed_ms)
    .execute(pool)
    .await?;

    Ok(())
}

async fn insert_sim_run_summary(
    pool: &MySqlPool,
    sim_run_id: &str,
    playbook_id: Option<&str>,
    build_id: &str,
    args: &Args,
    families_selected: i64,
    trade_choices: i64,
    watchlist_choices: i64,
    skip_choices: i64,
    manual_family_bans_applied: i64,
    symbol_trade_roots: i64,
    symbol_skip_roots: i64,
    summary: &RouterSummary,
    elapsed_ms: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO entry_exit_playbook_sim_runs SET
            sim_run_id = ?,
            playbook_id = ?,
            build_id = ?,
            source_scope = ?,
            source_timeframe = ?,
            test_year = ?,
            min_train_tests = ?,
            sister_window_minutes = ?,
            families_selected = ?,
            patterns_scanned = ?,
            routed_patterns = ?,
            no_route_patterns = ?,
            skipped_non_trade_patterns = ?,
            skipped_symbol_patterns = ?,
            skipped_overlap_patterns = ?,
            trade_choices = ?,
            watchlist_choices = ?,
            skip_choices = ?,
            manual_family_bans_applied = ?,
            symbol_filter_enabled = ?,
            one_trade_at_a_time = ?,
            one_trade_per_root_symbol = ?,
            one_trade_per_minute = ?,
            trade_cooldown_minutes = ?,
            daily_loss_lockout = ?,
            near_pass_protection = ?,
            near_pass_within_r = ?,
            near_pass_daily_loss_r = ?,
            loss_cluster_day_lockout = ?,
            loss_cluster_loss_count = ?,
            loss_cluster_window_minutes = ?,
            playbook_description = ?,
            symbol_trade_roots = ?,
            symbol_skip_roots = ?,
            symbol_min_tests = ?,
            symbol_min_win_rate = ?,
            symbol_min_avg_r = ?,
            prop_filter_enabled = ?,
            trade_min_tests = ?,
            trade_min_win_rate = ?,
            trade_min_avg_r = ?,
            watchlist_min_tests = ?,
            watchlist_min_win_rate = ?,
            watchlist_min_avg_r = ?,
            win_count = ?,
            loss_count = ?,
            no_entry_count = ?,
            avg_r = ?,
            sum_r = ?,
            best_r = ?,
            worst_r = ?,
            elapsed_ms = ?
        ON DUPLICATE KEY UPDATE
            playbook_id = VALUES(playbook_id),
            build_id = VALUES(build_id),
            source_scope = VALUES(source_scope),
            source_timeframe = VALUES(source_timeframe),
            test_year = VALUES(test_year),
            min_train_tests = VALUES(min_train_tests),
            sister_window_minutes = VALUES(sister_window_minutes),
            families_selected = VALUES(families_selected),
            patterns_scanned = VALUES(patterns_scanned),
            routed_patterns = VALUES(routed_patterns),
            no_route_patterns = VALUES(no_route_patterns),
            skipped_non_trade_patterns = VALUES(skipped_non_trade_patterns),
            skipped_symbol_patterns = VALUES(skipped_symbol_patterns),
            skipped_overlap_patterns = VALUES(skipped_overlap_patterns),
            trade_choices = VALUES(trade_choices),
            watchlist_choices = VALUES(watchlist_choices),
            skip_choices = VALUES(skip_choices),
            manual_family_bans_applied = VALUES(manual_family_bans_applied),
            symbol_filter_enabled = VALUES(symbol_filter_enabled),
            one_trade_at_a_time = VALUES(one_trade_at_a_time),
            one_trade_per_root_symbol = VALUES(one_trade_per_root_symbol),
            one_trade_per_minute = VALUES(one_trade_per_minute),
            trade_cooldown_minutes = VALUES(trade_cooldown_minutes),
            daily_loss_lockout = VALUES(daily_loss_lockout),
            near_pass_protection = VALUES(near_pass_protection),
            near_pass_within_r = VALUES(near_pass_within_r),
            near_pass_daily_loss_r = VALUES(near_pass_daily_loss_r),
            loss_cluster_day_lockout = VALUES(loss_cluster_day_lockout),
            loss_cluster_loss_count = VALUES(loss_cluster_loss_count),
            loss_cluster_window_minutes = VALUES(loss_cluster_window_minutes),
            playbook_description = VALUES(playbook_description),
            symbol_trade_roots = VALUES(symbol_trade_roots),
            symbol_skip_roots = VALUES(symbol_skip_roots),
            symbol_min_tests = VALUES(symbol_min_tests),
            symbol_min_win_rate = VALUES(symbol_min_win_rate),
            symbol_min_avg_r = VALUES(symbol_min_avg_r),
            prop_filter_enabled = VALUES(prop_filter_enabled),
            trade_min_tests = VALUES(trade_min_tests),
            trade_min_win_rate = VALUES(trade_min_win_rate),
            trade_min_avg_r = VALUES(trade_min_avg_r),
            watchlist_min_tests = VALUES(watchlist_min_tests),
            watchlist_min_win_rate = VALUES(watchlist_min_win_rate),
            watchlist_min_avg_r = VALUES(watchlist_min_avg_r),
            win_count = VALUES(win_count),
            loss_count = VALUES(loss_count),
            no_entry_count = VALUES(no_entry_count),
            avg_r = VALUES(avg_r),
            sum_r = VALUES(sum_r),
            best_r = VALUES(best_r),
            worst_r = VALUES(worst_r),
            elapsed_ms = VALUES(elapsed_ms)
        "#,
    )
    .bind(sim_run_id)
    .bind(playbook_id)
    .bind(build_id)
    .bind(&args.source_scope)
    .bind(args.source_timeframe.as_deref())
    .bind(args.test_year)
    .bind(args.min_train_tests)
    .bind(args.sister_window_minutes)
    .bind(families_selected)
    .bind(summary.patterns_scanned)
    .bind(summary.routed_patterns)
    .bind(summary.no_route_patterns)
    .bind(summary.skipped_non_trade_patterns)
    .bind(summary.skipped_symbol_patterns)
    .bind(summary.skipped_overlap_patterns)
    .bind(trade_choices)
    .bind(watchlist_choices)
    .bind(skip_choices)
    .bind(manual_family_bans_applied)
    .bind(if args.symbol_filter { 1 } else { 0 })
    .bind(if args.one_trade_at_a_time { 1 } else { 0 })
    .bind(if args.one_trade_per_root_symbol { 1 } else { 0 })
    .bind(if args.one_trade_per_minute { 1 } else { 0 })
    .bind(args.trade_cooldown_minutes)
    .bind(if args.daily_loss_lockout { 1 } else { 0 })
    .bind(if args.near_pass_protection { 1 } else { 0 })
    .bind(args.near_pass_within_r)
    .bind(args.near_pass_daily_loss_r)
    .bind(if args.loss_cluster_day_lockout { 1 } else { 0 })
    .bind(args.loss_cluster_loss_count)
    .bind(args.loss_cluster_window_minutes)
    .bind(args.playbook_description.as_deref())
    .bind(symbol_trade_roots)
    .bind(symbol_skip_roots)
    .bind(args.symbol_min_tests)
    .bind(args.symbol_min_win_rate)
    .bind(args.symbol_min_avg_r)
    .bind(if args.prop_filter { 1 } else { 0 })
    .bind(args.trade_min_tests)
    .bind(args.trade_min_win_rate)
    .bind(args.trade_min_avg_r)
    .bind(args.watchlist_min_tests)
    .bind(args.watchlist_min_win_rate)
    .bind(args.watchlist_min_avg_r)
    .bind(summary.win_count)
    .bind(summary.loss_count)
    .bind(summary.no_entry_count)
    .bind(summary.avg_r())
    .bind(summary.sum_r)
    .bind(summary.best_r)
    .bind(summary.worst_r)
    .bind(elapsed_ms)
    .execute(pool)
    .await?;

    Ok(())
}

async fn load_prop_replay_trades(
    pool: &MySqlPool,
    sim_run_id: &str,
) -> Result<Vec<PropReplayTrade>, sqlx::Error> {
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
            Ok(PropReplayTrade {
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

fn close_prop_cycle(
    summary: &mut PropSummary,
    cycles: &mut Vec<PropCycleRow>,
    cycle: &OpenPropCycle,
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

fn compute_prop_summary_from_trades(
    trades: &[PropReplayTrade],
    daily_loss_lockout: bool,
) -> (PropSummary, Vec<PropCycleRow>) {
    const PROFIT_TARGET_R: f64 = 30.0;
    const MAX_DRAWDOWN_R: f64 = 20.0;
    const DAILY_LOSS_R: f64 = 10.0;

    let mut summary = PropSummary::default();
    let mut cycles = Vec::new();
    let mut active_cycle: Option<OpenPropCycle> = None;
    let mut current_loss_streak = 0_i64;
    let mut next_cycle_number = 1_i64;
    let mut template_uids_used: HashSet<String> = HashSet::new();

    for trade in trades {
        if !trade.template_uid.trim().is_empty() {
            template_uids_used.insert(trade.template_uid.clone());
        }

        let date = trade.event_date.date();
        let cycle = active_cycle.get_or_insert_with(|| {
            let cycle = OpenPropCycle::new(next_cycle_number, date);
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
                close_prop_cycle(&mut summary, &mut cycles, &finished_cycle, outcome);
            }
        }
    }

    if let Some(finished_cycle) = active_cycle.take() {
        close_prop_cycle(
            &mut summary,
            &mut cycles,
            &finished_cycle,
            "open_incomplete",
        );
    }

    let closed_cycles = summary.passed + summary.daily_fails + summary.drawdown_fails;
    summary.sim_plays_used = template_uids_used.len() as i64;
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

fn compute_equity_points_from_trades(
    trades: &[PropReplayTrade],
    daily_loss_lockout: bool,
) -> Vec<EquityPointRow> {
    const PROFIT_TARGET_R: f64 = 30.0;
    const MAX_DRAWDOWN_R: f64 = 20.0;
    const DAILY_LOSS_R: f64 = 10.0;

    let mut points = Vec::with_capacity(trades.len());
    let mut cumulative_r = 0.0_f64;
    let mut active_cycle: Option<OpenPropCycle> = None;
    let mut next_cycle_number = 1_i64;

    for (index, trade) in trades.iter().enumerate() {
        let date = trade.event_date.date();
        let cycle = active_cycle.get_or_insert_with(|| {
            let cycle = OpenPropCycle::new(next_cycle_number, date);
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

fn compute_daily_r_from_trades(trades: &[PropReplayTrade]) -> Vec<DailyRRow> {
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

fn compute_hourly_performance_from_trades(
    trades: &[PropReplayTrade],
    daily_rows: &[DailyRRow],
) -> Vec<HourlyPerformanceRow> {
    let daily_loss_dates: HashSet<NaiveDate> = daily_rows
        .iter()
        .filter(|row| row.hit_daily_loss)
        .map(|row| row.trade_date)
        .collect();
    let mut by_hour: HashMap<i64, HourlyPerformanceRow> = HashMap::new();

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

fn compute_trade_cadence_from_trades(trades: &[PropReplayTrade]) -> TradeCadenceRow {
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

fn compute_loss_clustering_from_trades(
    trades: &[PropReplayTrade],
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
    trade: &PropReplayTrade,
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

fn compute_contribution_rows_from_trades(
    trades: &[PropReplayTrade],
    daily_rows: &[DailyRRow],
    key_for_trade: impl Fn(&PropReplayTrade) -> String,
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

fn compute_streaks_from_trades(trades: &[PropReplayTrade]) -> Vec<StreakRow> {
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
            sim_plays_used,
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
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE
            playbook_id = VALUES(playbook_id),
            build_id = VALUES(build_id),
            profit_target_r = VALUES(profit_target_r),
            max_drawdown_r_limit = VALUES(max_drawdown_r_limit),
            daily_loss_r_limit = VALUES(daily_loss_r_limit),
            sim_plays_used = VALUES(sim_plays_used),
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
    .bind(summary.sim_plays_used)
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
    let mut args = parse_args()?;
    let started = Instant::now();
    let pool = MySqlPool::connect(&database_url_from_env()?).await?;
    ensure_tables(&pool).await?;

    if args.reuse_router_run_id.is_none() {
        return Err(
            "Simulation now requires --playbook-id PLAYBOOK_ID. Create the playbook first with create_entry_exit_playbook."
                .into(),
        );
    }

    if let Some(playbook_id) = args.reuse_router_run_id.clone() {
        let playbook_rules = load_playbook_execution_rules(&pool, &playbook_id).await?;
        apply_playbook_execution_rules(&mut args, playbook_rules);
    }

    let train_run_id = match args.reuse_router_run_id.as_deref() {
        Some(reuse_router_run_id) => reused_router_train_run_id(&pool, reuse_router_run_id).await?,
        None => match args.train_run_id.clone() {
            Some(run_id) => run_id,
            None => latest_train_run_id(&pool).await?,
        },
    };
    let router_run_id = router_run_id();

    println!("Building family selection model");
    println!("model_test_id={router_run_id}");
    println!(
        "train_run_id={train_run_id} test_year={} source={} timeframe={} min_train_tests={}",
        args.test_year,
        args.source_scope,
        args.source_timeframe.as_deref().unwrap_or("all"),
        args.min_train_tests
    );
    if let Some(reuse_router_run_id) = args.reuse_router_run_id.as_deref() {
        println!("reuse_model_test_id={reuse_router_run_id}");
    }
    if args.prop_filter {
        println!(
            "prop_filter=on trade_bar={} tests / {:.1}% WR / {:.3}R avg watchlist_bar={} tests / {:.1}% WR / {:.3}R avg",
            args.trade_min_tests,
            args.trade_min_win_rate * 100.0,
            args.trade_min_avg_r,
            args.watchlist_min_tests,
            args.watchlist_min_win_rate * 100.0,
            args.watchlist_min_avg_r
        );
    }
    if args.symbol_filter {
        println!(
            "symbol_filter=on bar={} tests / {:.1}% WR / {:.3}R avg",
            args.symbol_min_tests,
            args.symbol_min_win_rate * 100.0,
            args.symbol_min_avg_r
        );
    }
    if args.one_trade_at_a_time {
        println!("one_trade_at_a_time=on");
    }
    if args.one_trade_per_root_symbol {
        println!("one_trade_per_root_symbol=on");
    }
    if args.one_trade_per_minute {
        println!("one_trade_per_minute=on");
    }
    if args.trade_cooldown_minutes > 0 {
        println!("trade_cooldown_minutes={}", args.trade_cooldown_minutes);
    }
    if args.daily_loss_lockout {
        println!("daily_loss_lockout=on");
    }
    if args.near_pass_protection {
        println!(
            "near_pass_protection=on within={:.2}R daily_loss={:.2}R",
            args.near_pass_within_r, args.near_pass_daily_loss_r
        );
    }
    if args.loss_cluster_day_lockout {
        println!(
            "loss_cluster_day_lockout=on losses={} window_minutes={}",
            args.loss_cluster_loss_count, args.loss_cluster_window_minutes
        );
    }
    if args.loss_cluster_day_lockout {
        println!(
            "loss_cluster_day_lockout=on losses={} window_minutes={}",
            args.loss_cluster_loss_count, args.loss_cluster_window_minutes
        );
    }

    let (mut choices, template_count) = match args.reuse_router_run_id.as_deref() {
        Some(reuse_router_run_id) => {
            let choices = load_reused_router_choices(&pool, reuse_router_run_id).await?;
            let template_count = choices
                .values()
                .map(|choice| choice.template.template_uid.clone())
                .collect::<HashSet<_>>()
                .len();
            (choices, template_count)
        }
        None => {
            let templates = load_templates(&pool, &train_run_id).await?;
            let template_count = templates.len();
            let family_stats = family_condition_stat_count(&pool, &train_run_id).await?;
            if family_stats == 0 {
                return Err(format!(
                    "Build {train_run_id} has no pattern_family_key condition stats. Rerun create_entry_exit_templates with the fixed builder so the playbook can select family -> entry/exit mappings."
                )
                .into());
            }
            println!("family_condition_stats={family_stats}");
            let choices = build_router_choices(&pool, &train_run_id, &templates, &args).await?;
            if choices.is_empty() {
                return Err(format!(
                    "No family -> entry/exit mappings were selected from build {train_run_id}. Try lower thresholds or inspect the pattern_family_key condition stats."
                )
                .into());
            }
            (choices, template_count)
        }
    };
    if !args.ignore_manual_family_skips {
        let manual_family_skips = load_manual_family_skips(&pool).await?;
        let applied_manual_family_skips =
            apply_manual_family_skips(&mut choices, &manual_family_skips);
        if !manual_family_skips.is_empty() {
            let mut skipped_keys = manual_family_skips.keys().cloned().collect::<Vec<_>>();
            skipped_keys.sort();
            println!(
                "manual_family_skips={} applied={} keys={}",
                manual_family_skips.len(),
                applied_manual_family_skips,
                skipped_keys.join(",")
            );
        }
    }
    let (trade_choices, watchlist_choices, skip_choices) = route_status_counts(&choices);
    insert_choices(&pool, &router_run_id, &train_run_id, &choices).await?;
    let symbol_choices = match args.reuse_router_run_id.as_deref() {
        Some(reuse_router_run_id) if args.symbol_filter => {
            load_reused_symbol_gate_choices(&pool, reuse_router_run_id).await?
        }
        _ => build_symbol_gate_choices(&pool, &train_run_id, &choices, &args).await?,
    };
    let (symbol_trade_roots, symbol_skip_roots) = symbol_gate_counts(&symbol_choices);
    insert_symbol_gate_choices(&pool, &router_run_id, &train_run_id, &symbol_choices).await?;
    println!(
        "selected {} family -> template mappings from {} templates (trade={} watchlist={} skip={})",
        choices.len(),
        template_count,
        trade_choices,
        watchlist_choices,
        skip_choices
    );
    if args.symbol_filter {
        let mut skipped_roots = symbol_choices
            .values()
            .filter(|choice| choice.route_status != "TRADE")
            .map(|choice| {
                format!(
                    "{}({} tests/{:.1}%/{:.3}R)",
                    choice.root_symbol,
                    choice.eval_count,
                    rate(choice.pass_count, choice.eval_count) * 100.0,
                    choice.avg_r
                )
            })
            .collect::<Vec<_>>();
        skipped_roots.sort();
        println!(
            "symbol filter roots trade={} skip={} skipped={}",
            symbol_trade_roots,
            symbol_skip_roots,
            skipped_roots.join(", ")
        );
    }

    let patterns = fetch_test_patterns(&pool, &args).await?;
    let raw_patterns_loaded = patterns.len() as i64;
    let event_limit = if args.limit > 0 {
        Some(args.limit as usize)
    } else {
        None
    };
    let events = build_twin_decision_events(patterns, event_limit);
    println!(
        "loaded {} patterns -> {} twin-aware decision events for {}",
        raw_patterns_loaded,
        events.len(),
        args.test_year
    );

    let mut summary = RouterSummary::default();
    summary.raw_patterns_loaded = raw_patterns_loaded;
    summary.patterns_scanned = events.len() as i64;
    let mut active_trade_exit_date: Option<NaiveDateTime> = None;
    let mut active_trade_exit_by_root: HashMap<String, NaiveDateTime> = HashMap::new();
    let mut last_accepted_trade_entry_date: Option<NaiveDateTime> = None;
    let mut daily_loss_lockout_dates: HashSet<NaiveDate> = HashSet::new();
    let mut lockout_cycle_equity_r = 0.0_f64;
    let mut lockout_cycle_peak_r = 0.0_f64;
    let mut lockout_daily_r = 0.0_f64;
    let mut lockout_current_date: Option<NaiveDate> = None;
    let mut near_pass_protection_active = false;
    let mut loss_cluster_lockout_dates: HashSet<NaiveDate> = HashSet::new();
    let mut loss_cluster_current_date: Option<NaiveDate> = None;
    let mut loss_cluster_losses: VecDeque<NaiveDateTime> = VecDeque::new();
    for (index, event) in events.iter().enumerate() {
        let live_candidates = event
            .candidates
            .iter()
            .filter(|setup| setup.d_confirm_date <= event.decision_date)
            .collect::<Vec<_>>();
        let live_candidate_count = live_candidates.len() as i64;
        if live_candidates.is_empty() {
            summary.no_route_patterns += 1;
            continue;
        }

        let mut saw_non_trade = false;
        let mut saw_symbol_blocked = false;
        let mut selected: Option<(&PatternSetup, &RouterChoice)> = None;

        for setup in live_candidates {
            let Some(family_key) = setup.pattern_family_key.as_deref() else {
                continue;
            };
            let Some(choice) = choices.get(family_key) else {
                continue;
            };
            if choice.route_status != "TRADE" {
                saw_non_trade = true;
                continue;
            }
            if args.symbol_filter {
                let root_symbol = setup_gate_root_symbol(setup);
                match symbol_choices.get(&root_symbol) {
                    Some(symbol_choice) if symbol_choice.route_status == "TRADE" => {}
                    _ => {
                        saw_symbol_blocked = true;
                        continue;
                    }
                }
            }

            let replace_selected = selected
                .map(|(selected_setup, selected_choice)| {
                    is_better_event_candidate(setup, choice, selected_setup, selected_choice)
                })
                .unwrap_or(true);
            if replace_selected {
                selected = Some((setup, choice));
            }
        }

        let Some((setup, choice)) = selected else {
            if saw_symbol_blocked {
                summary.skipped_symbol_patterns += 1;
            } else if saw_non_trade {
                summary.skipped_non_trade_patterns += 1;
            } else {
                summary.no_route_patterns += 1;
            }
            continue;
        };

        let candles = fetch_forward_candles(&pool, setup).await?;
        let evaluation = evaluate_template(&choice.template, setup, &candles);
        let setup_root_symbol = setup_gate_root_symbol(setup);
        if args.one_trade_at_a_time {
            if let (Some(entry_date), Some(active_exit_date)) =
                (evaluation.entry_date, active_trade_exit_date)
            {
                if entry_date < active_exit_date {
                    summary.skipped_overlap_patterns += 1;
                    continue;
                }
            }
        }
        if args.one_trade_per_root_symbol {
            if let (Some(entry_date), Some(active_exit_date)) = (
                evaluation.entry_date,
                active_trade_exit_by_root.get(&setup_root_symbol).copied(),
            ) {
                if entry_date < active_exit_date {
                    summary.skipped_overlap_patterns += 1;
                    continue;
                }
            }
        }
        if args.trade_cooldown_minutes > 0 {
            if let (Some(entry_date), Some(last_entry_date)) =
                (evaluation.entry_date, last_accepted_trade_entry_date)
            {
                if entry_date < last_entry_date + Duration::minutes(args.trade_cooldown_minutes) {
                    summary.skipped_overlap_patterns += 1;
                    continue;
                }
            }
        }
        if args.daily_loss_lockout || args.near_pass_protection {
            if let Some(entry_date) = evaluation.entry_date {
                if daily_loss_lockout_dates.contains(&entry_date.date()) {
                    summary.skipped_overlap_patterns += 1;
                    continue;
                }
            }
        }
        if args.loss_cluster_day_lockout {
            if let Some(entry_date) = evaluation.entry_date {
                if loss_cluster_lockout_dates.contains(&entry_date.date()) {
                    summary.skipped_overlap_patterns += 1;
                    continue;
                }
            }
        }
        summary.record(&evaluation);
        insert_result(
            &pool,
            &router_run_id,
            event,
            live_candidate_count,
            setup,
            choice,
            &evaluation,
        )
        .await?;
        if args.one_trade_at_a_time {
            if let (Some(_entry_date), Some(exit_date)) =
                (evaluation.entry_date, evaluation.exit_date)
            {
                active_trade_exit_date = Some(match active_trade_exit_date {
                    Some(current_exit_date) => current_exit_date.max(exit_date),
                    None => exit_date,
                });
            }
        }
        if args.one_trade_per_root_symbol {
            if let (Some(_entry_date), Some(exit_date)) =
                (evaluation.entry_date, evaluation.exit_date)
            {
                active_trade_exit_by_root
                    .entry(setup_root_symbol)
                    .and_modify(|current_exit_date| {
                        *current_exit_date = (*current_exit_date).max(exit_date);
                    })
                    .or_insert(exit_date);
            }
        }
        if args.trade_cooldown_minutes > 0 && matches!(evaluation.outcome.as_str(), "pass" | "fail")
        {
            if let Some(entry_date) = evaluation.entry_date {
                last_accepted_trade_entry_date = Some(entry_date);
            }
        }
        if (args.daily_loss_lockout || args.near_pass_protection)
            && matches!(evaluation.outcome.as_str(), "pass" | "fail")
        {
            if let Some(entry_date) = evaluation.entry_date {
                let trade_date = entry_date.date();
                if lockout_current_date != Some(trade_date) {
                    lockout_current_date = Some(trade_date);
                    lockout_daily_r = 0.0;
                }

                let result_r = evaluation.result_r.unwrap_or(0.0);
                lockout_cycle_equity_r += result_r;
                lockout_daily_r += result_r;
                lockout_cycle_peak_r = lockout_cycle_peak_r.max(lockout_cycle_equity_r);
                let lockout_cycle_drawdown_r = lockout_cycle_peak_r - lockout_cycle_equity_r;
                let near_pass_trigger_r = (30.0 - args.near_pass_within_r).max(0.0);
                if args.near_pass_protection && lockout_cycle_peak_r >= near_pass_trigger_r {
                    near_pass_protection_active = true;
                }

                let daily_lockout_limit_r = if args.near_pass_protection
                    && near_pass_protection_active
                    && args.near_pass_daily_loss_r > 0.0
                {
                    Some(args.near_pass_daily_loss_r)
                } else if args.daily_loss_lockout {
                    Some(10.0)
                } else {
                    None
                };

                if daily_lockout_limit_r
                    .map(|limit_r| lockout_daily_r <= -limit_r)
                    .unwrap_or(false)
                {
                    daily_loss_lockout_dates.insert(trade_date);
                }
                if lockout_cycle_drawdown_r >= 20.0 || lockout_cycle_equity_r >= 30.0 {
                    lockout_cycle_equity_r = 0.0;
                    lockout_cycle_peak_r = 0.0;
                    lockout_daily_r = 0.0;
                    lockout_current_date = None;
                    near_pass_protection_active = false;
                }
            }
        }
        if args.loss_cluster_day_lockout && evaluation.outcome == "fail" {
            if let Some(entry_date) = evaluation.entry_date {
                let trade_date = entry_date.date();
                if loss_cluster_current_date != Some(trade_date) {
                    loss_cluster_current_date = Some(trade_date);
                    loss_cluster_losses.clear();
                }

                loss_cluster_losses.push_back(entry_date);
                let cutoff = entry_date - Duration::minutes(args.loss_cluster_window_minutes);
                while loss_cluster_losses
                    .front()
                    .map(|loss_date| loss_date.date() != trade_date || *loss_date < cutoff)
                    .unwrap_or(false)
                {
                    loss_cluster_losses.pop_front();
                }

                if loss_cluster_losses.len() as i64 >= args.loss_cluster_loss_count {
                    loss_cluster_lockout_dates.insert(trade_date);
                }
            }
        }

        if (index + 1) % 500 == 0 {
            println!(
                "processed {}/{} routed={} wins={} losses={} no_entry={} overlap_skip={} avg_r={:.3}",
                index + 1,
                events.len(),
                summary.routed_patterns,
                summary.win_count,
                summary.loss_count,
                summary.no_entry_count,
                summary.skipped_overlap_patterns,
                summary.avg_r()
            );
        }
    }

    let elapsed_ms = started.elapsed().as_millis() as i64;
    let manual_family_bans_applied = choices
        .values()
        .filter(|choice| {
            choice.route_status == "SKIP"
                && choice.status_reason.to_ascii_lowercase().contains("manual")
        })
        .count() as i64;
    insert_run_summary(
        &pool,
        &router_run_id,
        &train_run_id,
        &args,
        choices.len() as i64,
        trade_choices,
        watchlist_choices,
        skip_choices,
        symbol_trade_roots,
        symbol_skip_roots,
        &summary,
        elapsed_ms,
    )
    .await?;

    println!("saving sim run summary...");
    insert_sim_run_summary(
        &pool,
        &router_run_id,
        args.reuse_router_run_id.as_deref(),
        &train_run_id,
        &args,
        choices.len() as i64,
        trade_choices,
        watchlist_choices,
        skip_choices,
        manual_family_bans_applied,
        symbol_trade_roots,
        symbol_skip_roots,
        &summary,
        elapsed_ms,
    )
    .await?;
    println!("saved sim run summary");

    println!("loading replay trades...");
    let prop_trades = load_prop_replay_trades(&pool, &router_run_id).await?;
    let (prop_summary, prop_cycles) =
        compute_prop_summary_from_trades(&prop_trades, args.daily_loss_lockout);
    let equity_points = compute_equity_points_from_trades(&prop_trades, args.daily_loss_lockout);
    let daily_r_rows = compute_daily_r_from_trades(&prop_trades);
    let hourly_rows = compute_hourly_performance_from_trades(&prop_trades, &daily_r_rows);
    let trade_cadence = compute_trade_cadence_from_trades(&prop_trades);
    let symbol_contribution_rows =
        compute_contribution_rows_from_trades(&prop_trades, &daily_r_rows, |trade| {
            trade.root_symbol.clone()
        });
    let family_contribution_rows =
        compute_contribution_rows_from_trades(&prop_trades, &daily_r_rows, |trade| {
            trade.family_key.clone()
        });
    let streaks = compute_streaks_from_trades(&prop_trades);
    let (loss_summary, loss_gap_buckets, loss_windows) =
        compute_loss_clustering_from_trades(&prop_trades, &streaks);
    println!("storing prop summary...");
    store_prop_summary(
        &pool,
        &router_run_id,
        args.reuse_router_run_id.as_deref(),
        &train_run_id,
        &prop_summary,
        &prop_cycles,
    )
    .await?;
    println!("storing equity points...");
    store_equity_points(
        &pool,
        &router_run_id,
        args.reuse_router_run_id.as_deref(),
        &train_run_id,
        &equity_points,
    )
    .await?;
    println!("storing daily R...");
    store_daily_r_rows(
        &pool,
        &router_run_id,
        args.reuse_router_run_id.as_deref(),
        &train_run_id,
        &daily_r_rows,
    )
    .await?;
    println!("storing hourly performance...");
    store_hourly_performance_rows(
        &pool,
        &router_run_id,
        args.reuse_router_run_id.as_deref(),
        &train_run_id,
        &hourly_rows,
    )
    .await?;
    println!("storing trade cadence...");
    store_trade_cadence(
        &pool,
        &router_run_id,
        args.reuse_router_run_id.as_deref(),
        &train_run_id,
        &trade_cadence,
    )
    .await?;
    println!("storing loss clustering...");
    store_loss_clustering(
        &pool,
        &router_run_id,
        args.reuse_router_run_id.as_deref(),
        &train_run_id,
        &loss_summary,
        &loss_gap_buckets,
        &loss_windows,
    )
    .await?;
    println!("storing symbol contribution...");
    store_symbol_contribution_rows(
        &pool,
        &router_run_id,
        args.reuse_router_run_id.as_deref(),
        &train_run_id,
        &symbol_contribution_rows,
    )
    .await?;
    println!("storing family contribution...");
    store_family_contribution_rows(
        &pool,
        &router_run_id,
        args.reuse_router_run_id.as_deref(),
        &train_run_id,
        &family_contribution_rows,
    )
    .await?;
    println!("storing streaks...");
    store_streaks(
        &pool,
        &router_run_id,
        args.reuse_router_run_id.as_deref(),
        &train_run_id,
        &streaks,
    )
    .await?;

    let win_rate = if summary.routed_patterns > 0 {
        summary.win_count as f64 / summary.routed_patterns as f64 * 100.0
    } else {
        0.0
    };
    println!();
    println!("Family selection model {} result", args.test_year);
    println!("model_test_id={router_run_id}");
    println!("families_selected={}", choices.len());
    println!("trade_choices={trade_choices}");
    println!("watchlist_choices={watchlist_choices}");
    println!("skip_choices={skip_choices}");
    println!("symbol_trade_roots={symbol_trade_roots}");
    println!("symbol_skip_roots={symbol_skip_roots}");
    println!("patterns_scanned={}", summary.patterns_scanned);
    println!("routed_patterns={}", summary.routed_patterns);
    println!("no_route_patterns={}", summary.no_route_patterns);
    println!(
        "skipped_non_trade_patterns={}",
        summary.skipped_non_trade_patterns
    );
    println!(
        "skipped_symbol_patterns={}",
        summary.skipped_symbol_patterns
    );
    println!(
        "skipped_overlap_patterns={}",
        summary.skipped_overlap_patterns
    );
    println!(
        "one_trade_at_a_time={}",
        if args.one_trade_at_a_time {
            "on"
        } else {
            "off"
        }
    );
    println!(
        "one_trade_per_root_symbol={}",
        if args.one_trade_per_root_symbol {
            "on"
        } else {
            "off"
        }
    );
    println!(
        "one_trade_per_minute={}",
        if args.one_trade_per_minute {
            "on"
        } else {
            "off"
        }
    );
    println!("trade_cooldown_minutes={}", args.trade_cooldown_minutes);
    println!(
        "daily_loss_lockout={}",
        if args.daily_loss_lockout { "on" } else { "off" }
    );
    println!("wins={}", summary.win_count);
    println!("losses={}", summary.loss_count);
    println!("no_entry={}", summary.no_entry_count);
    println!("win_rate={win_rate:.2}%");
    println!("avg_r={:.4}R", summary.avg_r());
    println!("sum_r={:.2}R", summary.sum_r);
    println!("best_r={:.2}R", summary.best_r);
    println!("worst_r={:.2}R", summary.worst_r);
    println!(
        "prop_cycles={} passed={} daily_fails={} drawdown_fails={} incomplete={} prop_pass_rate={:.2}% closed_prop_pass_rate={:.2}%",
        prop_summary.cycles,
        prop_summary.passed,
        prop_summary.daily_fails,
        prop_summary.drawdown_fails,
        prop_summary.incomplete,
        prop_summary.pass_rate,
        prop_summary.closed_pass_rate
    );
    println!("elapsed={:.2}s", elapsed_ms as f64 / 1000.0);

    Ok(())
}
