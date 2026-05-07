use actix_cors::Cors;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
mod pattern;
use crate::pattern::Pattern;
use actix_web::middleware::Logger;
mod models;
use crate::models::candles::Candle;
use actix_web::dev::Service;
use actix_web::route;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use sqlx::{MySqlPool, Row};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;

const CANDLE_STORAGE_TABLES: [&str; 9] = [
    "futures_contract_1m_candles",
    "futures_contract_3m_candles",
    "futures_contract_5m_candles",
    "futures_contract_15m_candles",
    "futures_contract_30m_candles",
    "futures_contract_1h_candles",
    "futures_contract_4h_candles",
    "futures_contract_12h_candles",
    "futures_contract_1d_candles",
];
const ENGINE_STORAGE_TABLES: [&str; 2] = ["pattern_setups", "pattern_outcomes_prop"];
const ROLLUP_STORAGE_TABLES: [&str; 4] = [
    "prop_strategy_family_summary",
    "prop_strategy_family_yearly",
    "prop_strategy_contract_week_summary",
    "prop_strategy_family_weekly_cadence",
];

#[derive(Serialize)]
struct PatternSummariesResponse {
    patterns: Vec<PatternSummary>,
    total_count: i64,
    has_more: bool,
    earliest_entry_date: Option<NaiveDateTime>,
    latest_entry_date: Option<NaiveDateTime>,
    entry_dates: Vec<NaiveDate>,
}

#[derive(Debug, serde::Deserialize)]
struct FilterParams {
    pub bin: Option<String>,
    pub harmonic_type: Option<String>,
    pub size_bucket: Option<String>,
    pub reversal_type: Option<String>,
    pub market: Option<String>,
    pub trade_result: Option<i32>,
    pub recent_days: Option<i64>,
    pub max_days_open: Option<i64>,
    pub prop_outcome_mode: Option<String>,
    pub include_count: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_direction: Option<String>,
    pub min_closed_trades: Option<i64>,
    pub min_expectancy: Option<f64>,
    pub max_down_years: Option<i64>,
    pub min_worst_year_expectancy: Option<f64>,
    pub min_score: Option<f64>,
    pub strategy_markets: Option<Vec<String>>,
    pub strategy_harmonic_types: Option<Vec<String>>,
    pub strategy_bins: Option<Vec<String>>,
    pub strategy_reversal_types: Option<Vec<String>>,
    pub strategy_size_buckets: Option<Vec<String>>,
    pub strategy_time_bins: Option<Vec<String>>,
    pub strategy_x_strictness: Option<Vec<String>>,
    pub strategy_three_month_trends: Option<Vec<String>>,
    pub strategy_six_month_trends: Option<Vec<String>>,
    pub strategy_twelve_month_trends: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
struct SetupComparisonParams {
    pub prop_strategy_id: Option<String>,
    pub harmonic_type: Option<String>,
    pub market: Option<String>,
    pub bin: Option<String>,
    pub reversal_type: Option<String>,
    pub size_bucket: Option<String>,
    pub time_bin: Option<String>,
    pub three_month_trend: Option<String>,
    pub six_month_trend: Option<String>,
    pub twelve_month_trend: Option<String>,
    pub include_examples: Option<bool>,
}

#[derive(Debug, serde::Deserialize)]
struct StrategyCandidateParams {
    pub min_closed_trades: Option<i64>,
    pub min_expectancy: Option<f64>,
    pub max_down_years: Option<i64>,
    pub min_worst_year_expectancy: Option<f64>,
    pub min_score: Option<f64>,
    pub include_count: Option<bool>,
    pub limit: Option<i64>,
    pub prop_outcome_mode: Option<String>,
    pub sort_by: Option<String>,
    pub sort_direction: Option<String>,
    pub strategy_markets: Option<Vec<String>>,
    pub strategy_harmonic_types: Option<Vec<String>>,
    pub strategy_bins: Option<Vec<String>>,
    pub strategy_reversal_types: Option<Vec<String>>,
    pub strategy_size_buckets: Option<Vec<String>>,
    pub strategy_time_bins: Option<Vec<String>>,
    pub strategy_x_strictness: Option<Vec<String>>,
    pub strategy_three_month_trends: Option<Vec<String>>,
    pub strategy_six_month_trends: Option<Vec<String>>,
    pub strategy_twelve_month_trends: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
struct StrategyTradesParams {
    pub prop_strategy_id: Option<String>,
    pub include_count: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub first_start_date: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct StrategyContractWeekParams {
    pub prop_strategy_id: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, serde::Deserialize)]
struct CandleStorageParams {}

#[derive(Debug, Deserialize)]
struct AdminActionParams {
    action: String,
    root_symbol: Option<String>,
    contract_symbol: Option<String>,
    source_timeframe: Option<String>,
    scan_concurrency: Option<i64>,
    default_fit_only: Option<bool>,
    skip_processed_symbols: Option<bool>,
    defer_rebuild_indexes: Option<bool>,
    skip_prop_family_summaries: Option<bool>,
    confirm_text: Option<String>,
}

#[derive(Serialize)]
struct AdminActionStartResponse {
    run_id: i64,
    status: String,
    action: String,
    command_text: String,
}

#[derive(Serialize)]
struct AdminStatusResponse {
    abcd_dir: Option<String>,
    operations: Vec<AdminOperationSnapshot>,
    table_snapshots: Vec<AdminTableSnapshot>,
    engine_phases: Vec<AdminEnginePhaseSnapshot>,
    cache_states: Vec<AdminCacheStateSnapshot>,
}

#[derive(Serialize)]
struct AdminOperationSnapshot {
    id: i64,
    action: String,
    status: String,
    root_symbol: Option<String>,
    contract_symbol: Option<String>,
    source_timeframe: Option<String>,
    command_text: Option<String>,
    started_at: Option<String>,
    finished_at: Option<String>,
    duration_ms: Option<i64>,
    exit_code: Option<i64>,
    output_tail: Option<String>,
    error_message: Option<String>,
}

#[derive(Serialize)]
struct AdminTableSnapshot {
    table_name: String,
    exact_rows: i64,
    total_bytes: u64,
    refreshed_at: Option<String>,
}

#[derive(Serialize)]
struct AdminEnginePhaseSnapshot {
    run_id: String,
    symbol: Option<String>,
    phase: String,
    row_count: Option<i64>,
    duration_ms: i64,
    note: Option<String>,
    created_at: Option<String>,
}

#[derive(Serialize)]
struct AdminCacheStateSnapshot {
    cache_name: String,
    is_ready: bool,
    updated_at: Option<String>,
    note: Option<String>,
}

#[derive(Clone)]
struct AdminCommandSpec {
    program: String,
    args: Vec<String>,
    envs: Vec<(String, String)>,
    env_removes: Vec<String>,
    cwd: PathBuf,
    command_text: String,
}

#[derive(Serialize)]
struct CandleStorageResponse {
    disk_path: Option<String>,
    disk_total_bytes: Option<u64>,
    disk_free_bytes: Option<u64>,
    disk_used_bytes: Option<u64>,
    disk_free_percent: Option<f64>,
    tables: Vec<CandleTableStorage>,
    total_rows: i64,
    total_bytes: u64,
    bytes_per_row: f64,
    futures_roots: Vec<FuturesRootCandleStorage>,
    engine_tables: Vec<CandleTableStorage>,
    engine_total_rows: i64,
    engine_total_bytes: u64,
    engine_bytes_per_row: f64,
    rollup_tables: Vec<CandleTableStorage>,
    rollup_total_rows: i64,
    rollup_total_bytes: u64,
    rollup_bytes_per_row: f64,
    setup_tables: Vec<CandleTableStorage>,
    setup_total_rows: i64,
    setup_total_bytes: u64,
    setup_bytes_per_row: f64,
    setup_roots: Vec<PatternSetupRootStorage>,
    setup_markets: Vec<PatternSetupMarketStorage>,
    setup_contracts: Vec<PatternSetupContractStorage>,
    setup_patterns: Vec<PatternSetupTimeframePatternStorage>,
}

#[derive(Clone, Serialize)]
struct CandleTableStorage {
    table_name: String,
    exact_rows: i64,
    data_bytes: u64,
    index_bytes: u64,
    total_bytes: u64,
    bytes_per_row: f64,
}

#[derive(Serialize)]
struct FuturesRootCandleStorage {
    table_name: String,
    root_symbol: String,
    contract_count: i64,
    candle_count: i64,
    first_ts: Option<String>,
    last_ts: Option<String>,
    estimated_bytes: u64,
}

#[derive(Serialize)]
struct PatternSetupRootStorage {
    table_name: String,
    root_symbol: String,
    contract_count: i64,
    setup_count: i64,
    first_d_date: Option<String>,
    last_d_date: Option<String>,
    estimated_bytes: u64,
}

#[derive(Serialize)]
struct PatternSetupMarketStorage {
    table_name: String,
    market: String,
    harmonic_type: String,
    setup_count: i64,
    estimated_bytes: u64,
}

#[derive(Serialize)]
struct PatternSetupContractStorage {
    table_name: String,
    root_symbol: String,
    contract_symbol: String,
    source_timeframe: String,
    setup_count: i64,
    first_d_date: Option<String>,
    last_d_date: Option<String>,
    estimated_bytes: u64,
}

#[derive(Serialize)]
struct PatternSetupTimeframePatternStorage {
    table_name: String,
    root_symbol: String,
    contract_symbol: String,
    source_timeframe: String,
    market: String,
    harmonic_type: String,
    setup_count: i64,
    first_d_date: Option<String>,
    last_d_date: Option<String>,
    estimated_bytes: u64,
}

#[derive(Clone)]
struct DiskStorageInfo {
    path: String,
    total_bytes: u64,
    free_bytes: u64,
    used_bytes: u64,
    free_percent: f64,
}

#[derive(Debug, serde::Deserialize)]
struct SimulatorReplayParams {
    pub prop_strategy_id: Option<String>,
    pub first_start_date: Option<String>,
    pub tests_to_chain: Option<i64>,
    pub contracts: Option<i64>,
    pub starting_balance: Option<f64>,
    pub profit_target: Option<f64>,
    pub max_drawdown: Option<f64>,
    pub daily_loss_limit: Option<f64>,
    pub drawdown_model: Option<String>,
    pub one_trade_at_a_time: Option<bool>,
}

#[derive(Clone, sqlx::FromRow, serde::Serialize)]
struct SimulatorReplaySourceTrade {
    symbol: String,
    pattern_id: Option<String>,
    pattern_group_id: String,
    d_date: NaiveDateTime,
    d_confirm_date: Option<NaiveDateTime>,
    reversal_detect_date: Option<NaiveDateTime>,
    entry_date: NaiveDateTime,
    target_date: Option<NaiveDateTime>,
    trade_enter_price: f64,
    trade_risk_exit_price: f64,
    trade_reward_exit_price: f64,
    trade_lowest_price: Option<f64>,
    trade_highest_price: Option<f64>,
    trade_adverse_price: Option<f64>,
    trade_favorable_price: Option<f64>,
    max_adverse_points: Option<f64>,
    max_favorable_points: Option<f64>,
    trade_result: i64,
}

#[derive(serde::Serialize)]
struct SimulatorReplayTradeEvent {
    test_index: i64,
    trade_index: i64,
    symbol: String,
    pattern_id: Option<String>,
    pattern_group_id: String,
    entry_date: NaiveDateTime,
    target_date: Option<NaiveDateTime>,
    trade_result: i64,
    pnl: f64,
    closed_pnl: f64,
    point_value: f64,
    balance_before: f64,
    balance: f64,
    closed_balance: f64,
    intratrade_low_balance: f64,
    intratrade_high_balance: f64,
    intratrade_adverse_pnl: f64,
    intratrade_favorable_pnl: f64,
    drawdown: f64,
    trade_lowest_price: Option<f64>,
    trade_highest_price: Option<f64>,
    trade_adverse_price: Option<f64>,
    trade_favorable_price: Option<f64>,
    max_adverse_points: f64,
    max_favorable_points: f64,
    failed_intratrade_drawdown: bool,
    failure_reason: Option<String>,
    skipped_for_overlap: bool,
}

#[derive(serde::Serialize)]
struct SimulatorReplayTestResult {
    test_index: i64,
    status: String,
    start_date: NaiveDateTime,
    end_date: Option<NaiveDateTime>,
    starting_balance: f64,
    ending_balance: f64,
    peak_balance: f64,
    max_drawdown: f64,
    trade_count: i64,
    skipped_overlap_count: i64,
}

#[derive(serde::Serialize)]
struct SimulatorReplayResponse {
    family_key: String,
    tests: Vec<SimulatorReplayTestResult>,
    trades: Vec<SimulatorReplayTradeEvent>,
    eligible_trade_count: i64,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct PatternDetailParams {
    pub pattern_id: Option<String>,
    pub pattern_group_id: Option<String>,
    pub prop_outcome_mode: Option<String>,
    pub d_date: Option<NaiveDateTime>,
    pub market: Option<String>,
    pub harmonic_type: Option<String>,
    pub size_bucket: Option<String>,
    pub balance_bucket: Option<String>,
    pub trade_enter_price: Option<Decimal>,
    pub trade_risk_exit_price: Option<Decimal>,
    pub trade_reward_exit_price: Option<Decimal>,
    pub x_length: Option<i64>,
    pub a_length: Option<i64>,
    pub b_length: Option<i64>,
    pub c_length: Option<i64>,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
struct PatternSummary {
    pub symbol: String,
    pub d_date: NaiveDateTime,
    pub d_confirm_date: Option<NaiveDateTime>,
    pub reversal_detect_date: Option<NaiveDateTime>,
    pub entry_date: Option<NaiveDateTime>,
    pub target_date: Option<NaiveDateTime>,
    pub target_open: Option<Decimal>,
    pub target_high: Option<Decimal>,
    pub target_low: Option<Decimal>,
    pub target_close: Option<Decimal>,
    pub trade_enter_price: Decimal,
    pub trade_risk_exit_price: Decimal,
    pub trade_reward_exit_price: Decimal,
    pub trade_result: i64,
    pub market: String,
    pub pattern_id: Option<String>,
    pub pattern_group_id: String,
    pub prop_strategy_id: Option<String>,
    pub harmonic_type: Option<String>,
    pub reversal_type: Option<String>,
    pub size_bucket: Option<String>,
    pub balance_bucket: Option<String>,
    pub time_bin: Option<String>,
    pub x_strictness: Option<String>,
    pub three_month_trend: Option<String>,
    pub six_month_trend: Option<String>,
    pub twelve_month_trend: Option<String>,
    pub time_accuracy: Option<f64>,
    pub x_length: Option<i64>,
    pub a_length: Option<i64>,
    pub b_length: Option<i64>,
    pub c_length: Option<i64>,
    pub d_length: Option<i64>,
    pub full_pattern_length: Option<i64>,
    pub bat_accuracy: Option<Decimal>,
    pub butterfly_accuracy: Option<Decimal>,
    pub gartley_accuracy: Option<Decimal>,
    pub crab_accuracy: Option<Decimal>,
    pub shark_accuracy: Option<Decimal>,
}

#[derive(Clone, sqlx::FromRow, serde::Serialize)]
struct SetupComparisonSummary {
    total_count: i64,
    closed_count: i64,
    open_count: i64,
    win_count: i64,
    loss_count: i64,
    expectancy: f64,
    avg_return: f64,
    win_rate: f64,
    avg_win: f64,
    avg_loss: f64,
    avg_trade_length: f64,
    avg_ab_xa: f64,
    avg_bc_ab: f64,
    avg_cd_bc: f64,
    avg_cd_xa: f64,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct SetupComparisonExample {
    symbol: String,
    d_date: NaiveDateTime,
    trade_result: i64,
    trade_pnl: f64,
    trade_length: f64,
    trade_enter_price: f64,
}

#[derive(Clone, sqlx::FromRow, serde::Serialize)]
struct SetupComparisonYearlyPoint {
    year: i64,
    total_count: i64,
    closed_count: i64,
    open_count: i64,
    win_count: i64,
    loss_count: i64,
    expectancy: f64,
    avg_return: f64,
    win_rate: f64,
}

#[derive(serde::Serialize)]
struct SetupComparisonResponse {
    prop_strategy_id: Option<String>,
    family_key: Option<String>,
    family_name: Option<String>,
    family_level: Option<i64>,
    included_dimensions: Option<String>,
    outcome_model: Option<String>,
    harmonic_type: Option<String>,
    market: Option<String>,
    bin: Option<String>,
    reversal_type: Option<String>,
    size_bucket: Option<String>,
    time_bin: Option<String>,
    x_strictness: Option<String>,
    three_month_trend: Option<String>,
    six_month_trend: Option<String>,
    twelve_month_trend: Option<String>,
    summary: SetupComparisonSummary,
    yearly_performance: Vec<SetupComparisonYearlyPoint>,
    recent_examples: Vec<SetupComparisonExample>,
}

fn empty_setup_comparison_summary() -> SetupComparisonSummary {
    SetupComparisonSummary {
        total_count: 0,
        closed_count: 0,
        open_count: 0,
        win_count: 0,
        loss_count: 0,
        expectancy: 0.0,
        avg_return: 0.0,
        win_rate: 0.0,
        avg_win: 0.0,
        avg_loss: 0.0,
        avg_trade_length: 0.0,
        avg_ab_xa: 0.0,
        avg_bc_ab: 0.0,
        avg_cd_bc: 0.0,
        avg_cd_xa: 0.0,
    }
}

fn empty_setup_comparison_response(params: &SetupComparisonParams) -> SetupComparisonResponse {
    SetupComparisonResponse {
        prop_strategy_id: params.prop_strategy_id.clone(),
        family_key: params.prop_strategy_id.clone(),
        family_name: None,
        family_level: None,
        included_dimensions: None,
        outcome_model: None,
        harmonic_type: params.harmonic_type.clone(),
        market: params.market.clone(),
        bin: params.bin.clone(),
        reversal_type: params.reversal_type.clone(),
        size_bucket: params.size_bucket.clone(),
        time_bin: params.time_bin.clone(),
        x_strictness: None,
        three_month_trend: params.three_month_trend.clone(),
        six_month_trend: params.six_month_trend.clone(),
        twelve_month_trend: params.twelve_month_trend.clone(),
        summary: empty_setup_comparison_summary(),
        yearly_performance: Vec::new(),
        recent_examples: Vec::new(),
    }
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct StrategyCandidateSummary {
    prop_strategy_id: Option<String>,
    family_key: Option<String>,
    family_name: Option<String>,
    family_level: Option<i64>,
    included_dimensions: Option<String>,
    outcome_model: Option<String>,
    market: String,
    harmonic_type: String,
    bin: String,
    reversal_type: String,
    size_bucket: String,
    time_bin: String,
    x_strictness: Option<String>,
    three_month_trend: String,
    six_month_trend: String,
    twelve_month_trend: String,
    worst_year_expectancy: f64,
    down_years: i64,
    total_count: i64,
    closed_count: i64,
    open_count: i64,
    win_count: i64,
    loss_count: i64,
    expectancy: f64,
    avg_return: f64,
    win_rate: f64,
    closed_rate: f64,
    avg_win: f64,
    avg_loss: f64,
    avg_target_range: Option<f64>,
    score: Option<f64>,
    total_calendar_weeks: i64,
    active_weeks: i64,
    zero_setup_weeks: i64,
    zero_setup_week_rate: f64,
    total_setups: i64,
    avg_setups_per_week: f64,
    max_setups_per_week: i64,
}

#[derive(Serialize)]
struct StrategyCandidatesResponse {
    strategies: Vec<StrategyCandidateSummary>,
    total_count: i64,
    has_more: bool,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct StrategyContractWeekSummary {
    family_key: String,
    symbol: String,
    contract_week_index: i64,
    total_count: i64,
    closed_count: i64,
    open_count: i64,
    win_count: i64,
    loss_count: i64,
    expectancy: f64,
    avg_return: f64,
    win_rate: f64,
}

#[derive(Clone, sqlx::FromRow)]
struct PropStrategyFamilyFilter {
    family_key: String,
    family_name: String,
    family_level: i64,
    included_dimensions: String,
    outcome_model: String,
    market: String,
    harmonic_type: String,
    bin: String,
    reversal_type: String,
    size_bucket: String,
    time_bin: String,
    x_strictness: Option<String>,
    three_month_trend: String,
    six_month_trend: String,
    twelve_month_trend: String,
}

#[derive(Deserialize, Debug)]
pub struct CandleParams {
    symbol: String,
    start_date: Option<String>,
    end_date: Option<String>,
}

fn accuracy_column_for_harmonic_type(harmonic_type: &str) -> Option<&'static str> {
    match harmonic_type {
        "Bat" | "AlternateBat" => Some("bat_accuracy"),
        "Butterfly" => Some("butterfly_accuracy"),
        "Gartley" => Some("gartley_accuracy"),
        "Crab" | "DeepCrab" => Some("crab_accuracy"),
        "Shark" => Some("shark_accuracy"),
        _ => None,
    }
}

fn dominant_harmonic_predicate(harmonic_type: &str) -> Option<&'static str> {
    match harmonic_type {
        "Bat" | "AlternateBat" | "Butterfly" | "Gartley" | "Crab" | "DeepCrab" | "Shark" => {
            Some("1=1")
        }
        _ => None,
    }
}

fn normalize_market_filter(market: Option<&str>) -> Option<&str> {
    match market {
        Some("Bullish") => Some("Bullish"),
        Some("Bearish") => Some("Bearish"),
        _ => None,
    }
}

fn normalize_trade_result_filter(trade_result: Option<i32>) -> Option<i32> {
    match trade_result {
        Some(0) | Some(1) | Some(2) => trade_result,
        _ => None,
    }
}

fn is_supported_bin_label(bin: &str) -> bool {
    matches!(
        bin,
        "0-10"
            | "10-20"
            | "20-30"
            | "30-40"
            | "40-50"
            | "50-60"
            | "60-70"
            | "70-80"
            | "80-90"
            | "90-100"
    )
}

fn is_supported_size_bucket(bucket: &str) -> bool {
    matches!(bucket, "Micro" | "Small" | "Normal" | "Large" | "Massive")
}

fn is_supported_reversal_type(reversal_type: &str) -> bool {
    matches!(
        reversal_type,
        "BullishKeyReversal"
            | "BearishKeyReversal"
            | "BullishEngulfing"
            | "BearishEngulfing"
            | "BullishOutsideReversal"
            | "BearishOutsideReversal"
            | "MorningStar"
            | "EveningStar"
            | "ThreeWhiteSoldiers"
            | "ThreeBlackCrows"
            | "Hammer"
            | "ShootingStar"
            | "None"
    )
}

fn is_supported_trend_bucket(bucket: &str) -> bool {
    matches!(bucket, "Bullish" | "Bearish" | "Unknown")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PropOutcomeMode {
    D,
    Reversal,
}

fn normalize_prop_outcome_mode(value: Option<&str>) -> PropOutcomeMode {
    match value.map(|item| item.trim().to_ascii_lowercase()) {
        Some(mode) if mode == "reversal" => PropOutcomeMode::Reversal,
        _ => PropOutcomeMode::D,
    }
}

fn prop_outcome_model_label(mode: PropOutcomeMode) -> &'static str {
    match mode {
        PropOutcomeMode::D => "D",
        PropOutcomeMode::Reversal => "DReversal",
    }
}

fn prop_family_sort_column(
    sort_by: Option<&str>,
    alias: &str,
    cadence_alias: Option<&str>,
) -> String {
    if let Some(cadence_alias) = cadence_alias {
        let cadence_column = match sort_by {
            Some("activeWeeks") => Some("active_weeks"),
            Some("zeroWeekRate") => Some("zero_setup_week_rate"),
            Some("avgSetupsPerWeek") => Some("avg_setups_per_week"),
            Some("maxSetupsPerWeek") => Some("max_setups_per_week"),
            _ => None,
        };

        if let Some(column) = cadence_column {
            return format!("{cadence_alias}.{column}");
        }
    }

    let column = match sort_by {
        Some("route") => "outcome_model",
        Some("family") => "family_name",
        Some("market") => "market",
        Some("pattern") => "harmonic_type",
        Some("bin") => "bin",
        Some("reversal") => "reversal_type",
        Some("size") => "size_bucket",
        Some("time") => "time_bin",
        Some("xMode") => "x_strictness",
        Some("trend3m") => "three_month_trend",
        Some("trend6m") => "six_month_trend",
        Some("trend12m") => "twelve_month_trend",
        Some("worstYear") => "worst_year_expectancy",
        Some("downYears") => "down_years",
        Some("expectancy") => "expectancy",
        Some("score") => "score",
        Some("winRate") => "win_rate",
        Some("avgReturn") => "avg_return",
        Some("closed") => "closed_count",
        _ => "score",
    };

    format!("{alias}.{column}")
}

async fn table_exists(pool: &MySqlPool, table_name: &str) -> Result<bool, sqlx::Error> {
    let count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM INFORMATION_SCHEMA.TABLES
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_NAME = ?
        "#,
    )
    .bind(table_name)
    .fetch_one(pool)
    .await?;

    Ok(count > 0)
}

fn read_i64_or_zero(row: &sqlx::mysql::MySqlRow, column: &str) -> i64 {
    row.try_get::<i64, _>(column)
        .or_else(|_| row.try_get::<u64, _>(column).map(|value| value as i64))
        .or_else(|_| {
            row.try_get::<Decimal, _>(column)
                .map(|value| value.to_i64().unwrap_or(0))
        })
        .unwrap_or(0)
}

async fn fetch_cached_storage_table_group(
    pool: &MySqlPool,
    table_names: &[&str],
) -> Result<Vec<CandleTableStorage>, sqlx::Error> {
    if !table_exists(pool, "storage_table_summary").await? {
        return Ok(Vec::new());
    }

    let mut tables = Vec::new();
    for table_name in table_names {
        let row = sqlx::query(
            r#"
            SELECT
                table_name,
                exact_rows,
                data_bytes,
                index_bytes,
                total_bytes,
                bytes_per_row
            FROM storage_table_summary
            WHERE table_name = ?
            "#,
        )
        .bind(table_name)
        .fetch_optional(pool)
        .await?;

        if let Some(row) = row {
            tables.push(CandleTableStorage {
                table_name: row.try_get("table_name").unwrap_or_default(),
                exact_rows: read_i64_or_zero(&row, "exact_rows"),
                data_bytes: read_i64_or_zero(&row, "data_bytes").max(0) as u64,
                index_bytes: read_i64_or_zero(&row, "index_bytes").max(0) as u64,
                total_bytes: read_i64_or_zero(&row, "total_bytes").max(0) as u64,
                bytes_per_row: row.try_get("bytes_per_row").unwrap_or(0.0),
            });
        }
    }

    Ok(tables)
}

async fn fetch_cached_futures_root_storage(
    pool: &MySqlPool,
) -> Result<Vec<FuturesRootCandleStorage>, sqlx::Error> {
    if !table_exists(pool, "storage_futures_root_summary").await? {
        return Ok(Vec::new());
    }

    let rows = sqlx::query(
        r#"
        SELECT
            table_name,
            root_symbol,
            contract_count,
            candle_count,
            first_ts,
            last_ts,
            estimated_bytes
        FROM storage_futures_root_summary
        ORDER BY candle_count DESC, root_symbol ASC, table_name ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| FuturesRootCandleStorage {
            table_name: row.try_get("table_name").unwrap_or_default(),
            root_symbol: row.try_get("root_symbol").unwrap_or_default(),
            contract_count: read_i64_or_zero(&row, "contract_count"),
            candle_count: read_i64_or_zero(&row, "candle_count"),
            first_ts: row.try_get("first_ts").ok(),
            last_ts: row.try_get("last_ts").ok(),
            estimated_bytes: read_i64_or_zero(&row, "estimated_bytes").max(0) as u64,
        })
        .collect())
}

async fn fetch_cached_pattern_setup_root_storage(
    pool: &MySqlPool,
) -> Result<Vec<PatternSetupRootStorage>, sqlx::Error> {
    if !table_exists(pool, "storage_pattern_setup_root_summary").await? {
        return Ok(Vec::new());
    }

    let rows = sqlx::query(
        r#"
        SELECT
            table_name,
            root_symbol,
            contract_count,
            setup_count,
            first_d_date,
            last_d_date,
            estimated_bytes
        FROM storage_pattern_setup_root_summary
        ORDER BY setup_count DESC, root_symbol ASC, table_name ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| PatternSetupRootStorage {
            table_name: row.try_get("table_name").unwrap_or_default(),
            root_symbol: row.try_get("root_symbol").unwrap_or_default(),
            contract_count: read_i64_or_zero(&row, "contract_count"),
            setup_count: read_i64_or_zero(&row, "setup_count"),
            first_d_date: row.try_get("first_d_date").ok(),
            last_d_date: row.try_get("last_d_date").ok(),
            estimated_bytes: read_i64_or_zero(&row, "estimated_bytes").max(0) as u64,
        })
        .collect())
}

async fn fetch_cached_pattern_setup_market_storage(
    pool: &MySqlPool,
) -> Result<Vec<PatternSetupMarketStorage>, sqlx::Error> {
    if !table_exists(pool, "storage_pattern_setup_market_summary").await? {
        return Ok(Vec::new());
    }

    let rows = sqlx::query(
        r#"
        SELECT
            table_name,
            market,
            harmonic_type,
            setup_count,
            estimated_bytes
        FROM storage_pattern_setup_market_summary
        ORDER BY setup_count DESC, market ASC, harmonic_type ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| PatternSetupMarketStorage {
            table_name: row.try_get("table_name").unwrap_or_default(),
            market: row.try_get("market").unwrap_or_default(),
            harmonic_type: row.try_get("harmonic_type").unwrap_or_default(),
            setup_count: read_i64_or_zero(&row, "setup_count"),
            estimated_bytes: read_i64_or_zero(&row, "estimated_bytes").max(0) as u64,
        })
        .collect())
}

async fn fetch_cached_pattern_setup_contract_storage(
    pool: &MySqlPool,
) -> Result<Vec<PatternSetupContractStorage>, sqlx::Error> {
    if !table_exists(pool, "storage_pattern_setup_contract_summary").await? {
        return Ok(Vec::new());
    }

    let rows = sqlx::query(
        r#"
        SELECT
            table_name,
            root_symbol,
            contract_symbol,
            source_timeframe,
            setup_count,
            first_d_date,
            last_d_date,
            estimated_bytes
        FROM storage_pattern_setup_contract_summary
        ORDER BY setup_count DESC, root_symbol ASC, contract_symbol ASC, source_timeframe ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| PatternSetupContractStorage {
            table_name: row.try_get("table_name").unwrap_or_default(),
            root_symbol: row.try_get("root_symbol").unwrap_or_default(),
            contract_symbol: row.try_get("contract_symbol").unwrap_or_default(),
            source_timeframe: row.try_get("source_timeframe").unwrap_or_default(),
            setup_count: read_i64_or_zero(&row, "setup_count"),
            first_d_date: row.try_get("first_d_date").ok(),
            last_d_date: row.try_get("last_d_date").ok(),
            estimated_bytes: read_i64_or_zero(&row, "estimated_bytes").max(0) as u64,
        })
        .collect())
}

async fn fetch_cached_pattern_setup_timeframe_pattern_storage(
    pool: &MySqlPool,
) -> Result<Vec<PatternSetupTimeframePatternStorage>, sqlx::Error> {
    if !table_exists(pool, "storage_pattern_setup_timeframe_pattern_summary").await? {
        return Ok(Vec::new());
    }

    let rows = sqlx::query(
        r#"
        SELECT
            table_name,
            root_symbol,
            contract_symbol,
            source_timeframe,
            market,
            harmonic_type,
            setup_count,
            first_d_date,
            last_d_date,
            estimated_bytes
        FROM storage_pattern_setup_timeframe_pattern_summary
        ORDER BY setup_count DESC, root_symbol ASC, source_timeframe ASC, market ASC, harmonic_type ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| PatternSetupTimeframePatternStorage {
            table_name: row.try_get("table_name").unwrap_or_default(),
            root_symbol: row.try_get("root_symbol").unwrap_or_default(),
            contract_symbol: row.try_get("contract_symbol").unwrap_or_default(),
            source_timeframe: row.try_get("source_timeframe").unwrap_or_default(),
            market: row.try_get("market").unwrap_or_default(),
            harmonic_type: row.try_get("harmonic_type").unwrap_or_default(),
            setup_count: read_i64_or_zero(&row, "setup_count"),
            first_d_date: row.try_get("first_d_date").ok(),
            last_d_date: row.try_get("last_d_date").ok(),
            estimated_bytes: read_i64_or_zero(&row, "estimated_bytes").max(0) as u64,
        })
        .collect())
}

fn summarize_storage_tables(tables: &[CandleTableStorage]) -> (i64, u64, f64) {
    let total_rows = tables.iter().map(|table| table.exact_rows).sum::<i64>();
    let total_bytes = tables.iter().map(|table| table.total_bytes).sum::<u64>();
    let bytes_per_row = if total_rows > 0 {
        total_bytes as f64 / total_rows as f64
    } else {
        0.0
    };

    (total_rows, total_bytes, bytes_per_row)
}

#[cfg(target_os = "windows")]
fn disk_space_bytes(path: &Path) -> Option<(u64, u64)> {
    use std::os::windows::ffi::OsStrExt;

    extern "system" {
        fn GetDiskFreeSpaceExW(
            lpDirectoryName: *const u16,
            lpFreeBytesAvailableToCaller: *mut u64,
            lpTotalNumberOfBytes: *mut u64,
            lpTotalNumberOfFreeBytes: *mut u64,
        ) -> i32;
    }

    let wide_path = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut available_bytes = 0_u64;
    let mut total_bytes = 0_u64;
    let mut total_free_bytes = 0_u64;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide_path.as_ptr(),
            &mut available_bytes,
            &mut total_bytes,
            &mut total_free_bytes,
        )
    };

    if ok == 0 {
        None
    } else {
        Some((total_bytes, available_bytes))
    }
}

#[cfg(not(target_os = "windows"))]
fn disk_space_bytes(_path: &Path) -> Option<(u64, u64)> {
    None
}

fn build_disk_storage_info(path: PathBuf) -> Option<DiskStorageInfo> {
    let resolved_path = if path.exists() {
        path
    } else {
        std::env::current_dir().ok()?
    };
    let (total_bytes, free_bytes) = disk_space_bytes(&resolved_path)?;
    let used_bytes = total_bytes.saturating_sub(free_bytes);
    let free_percent = if total_bytes > 0 {
        (free_bytes as f64 / total_bytes as f64) * 100.0
    } else {
        0.0
    };

    Some(DiskStorageInfo {
        path: resolved_path.display().to_string(),
        total_bytes,
        free_bytes,
        used_bytes,
        free_percent,
    })
}

async fn fetch_database_disk_storage(pool: &MySqlPool) -> Option<DiskStorageInfo> {
    let datadir = sqlx::query("SELECT @@datadir AS datadir")
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|row| row.try_get::<String, _>("datadir").ok())
        .filter(|value| !value.trim().is_empty());

    let path = datadir
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())?;
    build_disk_storage_info(path)
}

async fn build_candle_storage_response(
    pool: &MySqlPool,
    _params: &CandleStorageParams,
) -> Result<CandleStorageResponse, sqlx::Error> {
    let tables = fetch_cached_storage_table_group(pool, &CANDLE_STORAGE_TABLES).await?;
    let engine_tables = fetch_cached_storage_table_group(pool, &ENGINE_STORAGE_TABLES).await?;
    let rollup_tables = fetch_cached_storage_table_group(pool, &ROLLUP_STORAGE_TABLES).await?;
    let setup_tables = engine_tables
        .iter()
        .filter(|table| table.table_name == "pattern_setups")
        .cloned()
        .collect::<Vec<_>>();

    let (total_rows, total_bytes, bytes_per_row) = summarize_storage_tables(&tables);
    let (engine_total_rows, engine_total_bytes, engine_bytes_per_row) =
        summarize_storage_tables(&engine_tables);
    let (rollup_total_rows, rollup_total_bytes, rollup_bytes_per_row) =
        summarize_storage_tables(&rollup_tables);
    let (setup_total_rows, setup_total_bytes, setup_bytes_per_row) =
        summarize_storage_tables(&setup_tables);

    let futures_roots = fetch_cached_futures_root_storage(pool).await?;
    let setup_roots = fetch_cached_pattern_setup_root_storage(pool).await?;
    let setup_markets = fetch_cached_pattern_setup_market_storage(pool).await?;
    let setup_contracts = fetch_cached_pattern_setup_contract_storage(pool).await?;
    let setup_patterns = fetch_cached_pattern_setup_timeframe_pattern_storage(pool).await?;
    let disk_storage = fetch_database_disk_storage(pool).await;

    Ok(CandleStorageResponse {
        disk_path: disk_storage.as_ref().map(|item| item.path.clone()),
        disk_total_bytes: disk_storage.as_ref().map(|item| item.total_bytes),
        disk_free_bytes: disk_storage.as_ref().map(|item| item.free_bytes),
        disk_used_bytes: disk_storage.as_ref().map(|item| item.used_bytes),
        disk_free_percent: disk_storage.as_ref().map(|item| item.free_percent),
        tables,
        total_rows,
        total_bytes,
        bytes_per_row,
        futures_roots,
        engine_tables,
        engine_total_rows,
        engine_total_bytes,
        engine_bytes_per_row,
        rollup_tables,
        rollup_total_rows,
        rollup_total_bytes,
        rollup_bytes_per_row,
        setup_tables,
        setup_total_rows,
        setup_total_bytes,
        setup_bytes_per_row,
        setup_roots,
        setup_markets,
        setup_contracts,
        setup_patterns,
    })
}

async fn ensure_admin_operation_table(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS admin_operation_runs (
            id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
            action VARCHAR(64) NOT NULL,
            status VARCHAR(32) NOT NULL DEFAULT 'queued',
            root_symbol VARCHAR(32) NULL,
            contract_symbol VARCHAR(64) NULL,
            source_timeframe VARCHAR(16) NULL,
            command_text TEXT NULL,
            started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            finished_at DATETIME NULL,
            duration_ms BIGINT NULL,
            exit_code INT NULL,
            output_tail MEDIUMTEXT NULL,
            error_message TEXT NULL,
            INDEX idx_admin_operation_status (status, started_at),
            INDEX idx_admin_operation_started (started_at)
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

fn trim_admin_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
}

fn normalize_admin_symbol(value: Option<&str>) -> Option<String> {
    trim_admin_text(value).map(|item| item.to_ascii_uppercase())
}

fn normalize_admin_timeframe(value: Option<&str>) -> Result<String, String> {
    let timeframe = trim_admin_text(value).unwrap_or_else(|| "1m".to_string());
    let allowed = ["1m", "3m", "5m", "15m", "30m", "1h", "4h", "12h", "1d"];

    if allowed.contains(&timeframe.as_str()) {
        Ok(timeframe)
    } else {
        Err(format!("Unsupported timeframe: {timeframe}"))
    }
}

fn bool_env(value: Option<bool>, default_value: bool) -> String {
    if value.unwrap_or(default_value) {
        "1".to_string()
    } else {
        "0".to_string()
    }
}

fn resolve_abcd_dir() -> Result<PathBuf, String> {
    if let Ok(value) = std::env::var("ABCD_ADMIN_ABCD_DIR") {
        let path = PathBuf::from(value);
        if path.join("Cargo.toml").exists() {
            return Ok(path);
        }
    }

    let current_dir = std::env::current_dir().map_err(|error| error.to_string())?;
    let candidates = [
        current_dir.join("rust_sr").join("abcd"),
        current_dir.join("..").join("abcd"),
        current_dir.join("..").join("..").join("rust_sr").join("abcd"),
        current_dir.clone(),
    ];

    for candidate in candidates {
        if candidate.join("Cargo.toml").exists() {
            return Ok(candidate);
        }
    }

    Err("Could not locate rust_sr/abcd. Set ABCD_ADMIN_ABCD_DIR.".to_string())
}

fn cargo_admin_command(
    abcd_dir: PathBuf,
    bin_name: &str,
    action_label: &str,
    envs: Vec<(String, String)>,
    env_removes: Vec<String>,
) -> AdminCommandSpec {
    let args = vec![
        "run".to_string(),
        "--bin".to_string(),
        bin_name.to_string(),
    ];
    let command_text = format!(
        "{action_label}: cargo {}",
        args.iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" ")
    );

    AdminCommandSpec {
        program: "cargo".to_string(),
        args,
        envs,
        env_removes,
        cwd: abcd_dir,
        command_text,
    }
}

fn build_admin_command(params: &AdminActionParams) -> Result<AdminCommandSpec, String> {
    let action = params.action.trim();
    let abcd_dir = resolve_abcd_dir()?;
    let clean_env = vec![
        "ABCD_ENGINE_WORKSTATION".to_string(),
        "ABCD_BENCHMARK_ONLY".to_string(),
        "ABCD_SCAN_ONLY".to_string(),
        "ABCD_EXPORT_CSV".to_string(),
        "ABCD_PHASE_TIMINGS".to_string(),
        "ABCD_PRINT_PHASE_TIMINGS".to_string(),
    ];

    match action {
        "rebuild_indexes" => Ok(cargo_admin_command(
            abcd_dir,
            "rebuild_engine_indexes",
            "Rebuild indexes",
            Vec::new(),
            clean_env,
        )),
        "refresh_rollups" => Ok(cargo_admin_command(
            abcd_dir,
            "refresh_prop_strategy_family_summaries",
            "Refresh rollups",
            Vec::new(),
            clean_env,
        )),
        "refresh_storage" => Ok(cargo_admin_command(
            abcd_dir,
            "refresh_storage_summary",
            "Refresh storage",
            Vec::new(),
            clean_env,
        )),
        "clear_engine" => {
            if params.confirm_text.as_deref() != Some("CLEAR ENGINE") {
                return Err("Type CLEAR ENGINE before clearing generated engine tables.".to_string());
            }

            Ok(cargo_admin_command(
                abcd_dir,
                "clear_engine_tables",
                "Clear engine",
                Vec::new(),
                clean_env,
            ))
        }
        "run_engine_scan" => {
            let timeframe = normalize_admin_timeframe(params.source_timeframe.as_deref())?;
            let root_symbol = normalize_admin_symbol(params.root_symbol.as_deref());
            let contract_symbol = normalize_admin_symbol(params.contract_symbol.as_deref());
            let scan_concurrency = params.scan_concurrency.unwrap_or(1).clamp(1, 16);
            let mut envs = vec![
                ("ABCD_CANDLE_SOURCE".to_string(), "futures_contracts".to_string()),
                ("ABCD_FUTURES_TIMEFRAME".to_string(), timeframe.clone()),
                (
                    "ABCD_SCAN_CONCURRENCY".to_string(),
                    scan_concurrency.to_string(),
                ),
                ("ABCD_WRITE_BATCH_SIZE".to_string(), "1".to_string()),
                (
                    "ABCD_SKIP_PROCESSED_SYMBOLS".to_string(),
                    bool_env(params.skip_processed_symbols, true),
                ),
                (
                    "ABCD_SKIP_PROP_FAMILY_SUMMARIES".to_string(),
                    bool_env(params.skip_prop_family_summaries, true),
                ),
                (
                    "ABCD_DEFER_REBUILD_INDEXES".to_string(),
                    bool_env(params.defer_rebuild_indexes, true),
                ),
                (
                    "ABCD_DEFAULT_FIT_ONLY".to_string(),
                    bool_env(params.default_fit_only, true),
                ),
                ("ABCD_PROGRESS_EVERY".to_string(), "10000".to_string()),
            ];

            let mut env_removes = clean_env;
            env_removes.push("ABCD_FUTURES_ROOT".to_string());
            env_removes.push("ABCD_FUTURES_CONTRACT_SYMBOL".to_string());

            if let Some(root_symbol) = root_symbol.as_ref() {
                envs.push(("ABCD_FUTURES_ROOT".to_string(), root_symbol.clone()));
            }
            if let Some(contract_symbol) = contract_symbol.as_ref() {
                envs.push((
                    "ABCD_FUTURES_CONTRACT_SYMBOL".to_string(),
                    contract_symbol.clone(),
                ));
            }

            let mut spec = cargo_admin_command(
                abcd_dir,
                "abcd",
                "Run engine scan",
                envs,
                env_removes,
            );
            spec.command_text = format!(
                "Run engine scan: {} {} {}",
                root_symbol.unwrap_or_else(|| "ALL_ROOTS".to_string()),
                contract_symbol.unwrap_or_else(|| "ALL_CONTRACTS".to_string()),
                timeframe
            );

            Ok(spec)
        }
        _ => Err(format!("Unknown admin action: {action}")),
    }
}

async fn insert_admin_operation(
    pool: &MySqlPool,
    params: &AdminActionParams,
    command_text: &str,
) -> Result<i64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        INSERT INTO admin_operation_runs (
            action, status, root_symbol, contract_symbol, source_timeframe,
            command_text, started_at
        )
        VALUES (?, 'running', ?, ?, ?, ?, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(params.action.trim())
    .bind(normalize_admin_symbol(params.root_symbol.as_deref()))
    .bind(normalize_admin_symbol(params.contract_symbol.as_deref()))
    .bind(trim_admin_text(params.source_timeframe.as_deref()))
    .bind(command_text)
    .execute(pool)
    .await?;

    Ok(result.last_insert_id() as i64)
}

fn append_output_tail(tail: &mut String, text: &str) {
    const MAX_OUTPUT_TAIL_BYTES: usize = 24_000;
    tail.push_str(text);
    while tail.len() > MAX_OUTPUT_TAIL_BYTES {
        if let Some(index) = tail.find('\n') {
            tail.drain(..=index);
        } else {
            tail.clear();
            break;
        }
    }
}

async fn collect_stream_tail<R>(stream: R, label: &'static str) -> String
where
    R: AsyncRead + Unpin,
{
    let mut reader = BufReader::new(stream).lines();
    let mut tail = String::new();

    loop {
        match reader.next_line().await {
            Ok(Some(line)) => {
                append_output_tail(&mut tail, &format!("[{label}] {line}\n"));
            }
            Ok(None) => break,
            Err(error) => {
                append_output_tail(&mut tail, &format!("[{label}] stream error: {error}\n"));
                break;
            }
        }
    }

    tail
}

async fn finish_admin_operation(
    pool: &MySqlPool,
    run_id: i64,
    status: &str,
    duration_ms: i64,
    exit_code: Option<i32>,
    output_tail: String,
    error_message: Option<String>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE admin_operation_runs
        SET
            status = ?,
            finished_at = CURRENT_TIMESTAMP,
            duration_ms = ?,
            exit_code = ?,
            output_tail = ?,
            error_message = ?
        WHERE id = ?
        "#,
    )
    .bind(status)
    .bind(duration_ms)
    .bind(exit_code)
    .bind(output_tail)
    .bind(error_message)
    .bind(run_id)
    .execute(pool)
    .await?;

    Ok(())
}

async fn run_admin_command_background(pool: MySqlPool, run_id: i64, spec: AdminCommandSpec) {
    let started_at = Instant::now();
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for env_name in &spec.env_removes {
        command.env_remove(env_name);
    }
    for (env_name, env_value) in &spec.envs {
        command.env(env_name, env_value);
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let _ = finish_admin_operation(
                &pool,
                run_id,
                "failed",
                started_at.elapsed().as_millis() as i64,
                None,
                String::new(),
                Some(format!("Failed to start command: {error}")),
            )
            .await;
            return;
        }
    };

    let stdout_task = child
        .stdout
        .take()
        .map(|stream| tokio::spawn(collect_stream_tail(stream, "out")));
    let stderr_task = child
        .stderr
        .take()
        .map(|stream| tokio::spawn(collect_stream_tail(stream, "err")));

    let status_result = child.wait().await;
    let mut output_tail = String::new();

    if let Some(task) = stdout_task {
        if let Ok(tail) = task.await {
            append_output_tail(&mut output_tail, &tail);
        }
    }
    if let Some(task) = stderr_task {
        if let Ok(tail) = task.await {
            append_output_tail(&mut output_tail, &tail);
        }
    }

    let duration_ms = started_at.elapsed().as_millis() as i64;
    let (status, exit_code, error_message) = match status_result {
        Ok(exit_status) if exit_status.success() => ("completed", exit_status.code(), None),
        Ok(exit_status) => (
            "failed",
            exit_status.code(),
            Some(format!("Command exited with status {exit_status}")),
        ),
        Err(error) => ("failed", None, Some(format!("Command wait failed: {error}"))),
    };

    if let Err(error) = finish_admin_operation(
        &pool,
        run_id,
        status,
        duration_ms,
        exit_code,
        output_tail,
        error_message,
    )
    .await
    {
        eprintln!("Admin operation update failed: {:?}", error);
    }
}

async fn fetch_admin_operations(
    pool: &MySqlPool,
) -> Result<Vec<AdminOperationSnapshot>, sqlx::Error> {
    if !table_exists(pool, "admin_operation_runs").await? {
        return Ok(Vec::new());
    }

    let rows = sqlx::query(
        r#"
        SELECT
            id,
            action,
            status,
            root_symbol,
            contract_symbol,
            source_timeframe,
            command_text,
            DATE_FORMAT(started_at, '%Y-%m-%d %H:%i:%s') AS started_at,
            DATE_FORMAT(finished_at, '%Y-%m-%d %H:%i:%s') AS finished_at,
            duration_ms,
            exit_code,
            output_tail,
            error_message
        FROM admin_operation_runs
        ORDER BY id DESC
        LIMIT 25
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| AdminOperationSnapshot {
            id: read_i64_or_zero(&row, "id"),
            action: row.try_get("action").unwrap_or_default(),
            status: row.try_get("status").unwrap_or_default(),
            root_symbol: row.try_get("root_symbol").ok(),
            contract_symbol: row.try_get("contract_symbol").ok(),
            source_timeframe: row.try_get("source_timeframe").ok(),
            command_text: row.try_get("command_text").ok(),
            started_at: row.try_get("started_at").ok(),
            finished_at: row.try_get("finished_at").ok(),
            duration_ms: row.try_get("duration_ms").ok(),
            exit_code: row
                .try_get::<Option<i32>, _>("exit_code")
                .ok()
                .flatten()
                .map(|value| value as i64),
            output_tail: row.try_get("output_tail").ok(),
            error_message: row.try_get("error_message").ok(),
        })
        .collect())
}

async fn fetch_admin_table_snapshots(
    pool: &MySqlPool,
) -> Result<Vec<AdminTableSnapshot>, sqlx::Error> {
    if !table_exists(pool, "storage_table_summary").await? {
        return Ok(Vec::new());
    }

    let rows = sqlx::query(
        r#"
        SELECT
            table_name,
            exact_rows,
            total_bytes,
            DATE_FORMAT(refreshed_at, '%Y-%m-%d %H:%i:%s') AS refreshed_at
        FROM storage_table_summary
        WHERE table_name IN (
            'pattern_setups',
            'pattern_outcomes_prop',
            'prop_strategy_family_summary',
            'prop_strategy_family_yearly',
            'prop_strategy_contract_week_summary',
            'prop_strategy_family_weekly_cadence',
            'futures_contract_1m_candles'
        )
        ORDER BY FIELD(
            table_name,
            'pattern_setups',
            'pattern_outcomes_prop',
            'prop_strategy_family_summary',
            'prop_strategy_family_yearly',
            'prop_strategy_contract_week_summary',
            'prop_strategy_family_weekly_cadence',
            'futures_contract_1m_candles'
        )
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| AdminTableSnapshot {
            table_name: row.try_get("table_name").unwrap_or_default(),
            exact_rows: read_i64_or_zero(&row, "exact_rows"),
            total_bytes: read_i64_or_zero(&row, "total_bytes").max(0) as u64,
            refreshed_at: row.try_get("refreshed_at").ok(),
        })
        .collect())
}

async fn fetch_admin_engine_phases(
    pool: &MySqlPool,
) -> Result<Vec<AdminEnginePhaseSnapshot>, sqlx::Error> {
    if !table_exists(pool, "engine_phase_timings").await? {
        return Ok(Vec::new());
    }

    let rows = sqlx::query(
        r#"
        SELECT
            run_id,
            symbol,
            phase,
            row_count,
            duration_ms,
            note,
            DATE_FORMAT(created_at, '%Y-%m-%d %H:%i:%s') AS created_at
        FROM engine_phase_timings
        ORDER BY id DESC
        LIMIT 30
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| AdminEnginePhaseSnapshot {
            run_id: row.try_get("run_id").unwrap_or_default(),
            symbol: row.try_get("symbol").ok(),
            phase: row.try_get("phase").unwrap_or_default(),
            row_count: row.try_get("row_count").ok(),
            duration_ms: read_i64_or_zero(&row, "duration_ms"),
            note: row.try_get("note").ok(),
            created_at: row.try_get("created_at").ok(),
        })
        .collect())
}

async fn fetch_admin_cache_states(
    pool: &MySqlPool,
) -> Result<Vec<AdminCacheStateSnapshot>, sqlx::Error> {
    if !table_exists(pool, "dashboard_cache_state").await? {
        return Ok(Vec::new());
    }

    let rows = sqlx::query(
        r#"
        SELECT
            cache_name,
            is_ready,
            DATE_FORMAT(updated_at, '%Y-%m-%d %H:%i:%s') AS updated_at,
            note
        FROM dashboard_cache_state
        ORDER BY updated_at DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| AdminCacheStateSnapshot {
            cache_name: row.try_get("cache_name").unwrap_or_default(),
            is_ready: row
                .try_get::<bool, _>("is_ready")
                .unwrap_or_else(|_| read_i64_or_zero(&row, "is_ready") != 0),
            updated_at: row.try_get("updated_at").ok(),
            note: row.try_get("note").ok(),
        })
        .collect())
}

async fn build_admin_status_response(
    pool: &MySqlPool,
) -> Result<AdminStatusResponse, sqlx::Error> {
    ensure_admin_operation_table(pool).await?;

    Ok(AdminStatusResponse {
        abcd_dir: resolve_abcd_dir()
            .ok()
            .map(|path| path.display().to_string()),
        operations: fetch_admin_operations(pool).await?,
        table_snapshots: fetch_admin_table_snapshots(pool).await?,
        engine_phases: fetch_admin_engine_phases(pool).await?,
        cache_states: fetch_admin_cache_states(pool).await?,
    })
}

fn cadence_select_fields(has_cadence: bool) -> &'static str {
    if has_cadence {
        r#"
            CAST(COALESCE(c.total_calendar_weeks, 0) AS SIGNED) AS total_calendar_weeks,
            CAST(COALESCE(c.active_weeks, 0) AS SIGNED) AS active_weeks,
            CAST(COALESCE(c.zero_setup_weeks, 0) AS SIGNED) AS zero_setup_weeks,
            CAST(COALESCE(c.zero_setup_week_rate, 0.0) AS DOUBLE) AS zero_setup_week_rate,
            CAST(COALESCE(c.total_setups, 0) AS SIGNED) AS total_setups,
            CAST(COALESCE(c.avg_setups_per_week, 0.0) AS DOUBLE) AS avg_setups_per_week,
            CAST(COALESCE(c.max_setups_per_week, 0) AS SIGNED) AS max_setups_per_week
        "#
    } else {
        r#"
            CAST(0 AS SIGNED) AS total_calendar_weeks,
            CAST(0 AS SIGNED) AS active_weeks,
            CAST(0 AS SIGNED) AS zero_setup_weeks,
            CAST(0.0 AS DOUBLE) AS zero_setup_week_rate,
            CAST(0 AS SIGNED) AS total_setups,
            CAST(0.0 AS DOUBLE) AS avg_setups_per_week,
            CAST(0 AS SIGNED) AS max_setups_per_week
        "#
    }
}

fn cadence_join_clause(has_cadence: bool) -> &'static str {
    if has_cadence {
        "LEFT JOIN prop_strategy_family_weekly_cadence c ON c.family_key = s.family_key"
    } else {
        ""
    }
}

fn prop_family_sort_direction(sort_direction: Option<&str>) -> &'static str {
    match sort_direction.map(|item| item.trim().to_ascii_lowercase()) {
        Some(direction) if direction == "asc" => "ASC",
        _ => "DESC",
    }
}

fn append_in_filter(sql: &mut String, alias: &str, column: &str, values: &Option<Vec<String>>) {
    let Some(values) = values.as_ref().filter(|items| !items.is_empty()) else {
        return;
    };

    let placeholders = std::iter::repeat("?")
        .take(values.len())
        .collect::<Vec<_>>()
        .join(", ");
    sql.push_str(&format!(" AND {alias}.{column} IN ({placeholders})"));
}

fn append_prop_family_option_filters(sql: &mut String, alias: &str, params: &FilterParams) {
    append_in_filter(sql, alias, "market", &params.strategy_markets);
    append_in_filter(sql, alias, "harmonic_type", &params.strategy_harmonic_types);
    append_in_filter(sql, alias, "bin", &params.strategy_bins);
    append_in_filter(sql, alias, "reversal_type", &params.strategy_reversal_types);
    append_in_filter(sql, alias, "size_bucket", &params.strategy_size_buckets);
    append_in_filter(sql, alias, "time_bin", &params.strategy_time_bins);
    append_in_filter(sql, alias, "x_strictness", &params.strategy_x_strictness);
    append_in_filter(
        sql,
        alias,
        "three_month_trend",
        &params.strategy_three_month_trends,
    );
    append_in_filter(
        sql,
        alias,
        "six_month_trend",
        &params.strategy_six_month_trends,
    );
    append_in_filter(
        sql,
        alias,
        "twelve_month_trend",
        &params.strategy_twelve_month_trends,
    );
}

fn append_prop_family_candidate_option_filters(
    sql: &mut String,
    alias: &str,
    params: &StrategyCandidateParams,
) {
    append_in_filter(sql, alias, "market", &params.strategy_markets);
    append_in_filter(sql, alias, "harmonic_type", &params.strategy_harmonic_types);
    append_in_filter(sql, alias, "bin", &params.strategy_bins);
    append_in_filter(sql, alias, "reversal_type", &params.strategy_reversal_types);
    append_in_filter(sql, alias, "size_bucket", &params.strategy_size_buckets);
    append_in_filter(sql, alias, "time_bin", &params.strategy_time_bins);
    append_in_filter(sql, alias, "x_strictness", &params.strategy_x_strictness);
    append_in_filter(
        sql,
        alias,
        "three_month_trend",
        &params.strategy_three_month_trends,
    );
    append_in_filter(
        sql,
        alias,
        "six_month_trend",
        &params.strategy_six_month_trends,
    );
    append_in_filter(
        sql,
        alias,
        "twelve_month_trend",
        &params.strategy_twelve_month_trends,
    );
}

fn is_missing_table_error(error: &sqlx::Error) -> bool {
    matches!(
        error,
        sqlx::Error::Database(db_error) if db_error.code().as_deref() == Some("1146")
    )
}

async fn is_rollup_cache_ready(pool: &MySqlPool, cache_name: &str) -> Result<bool, sqlx::Error> {
    let ready = match sqlx::query_scalar::<_, i64>(
        r#"
        SELECT CASE WHEN is_ready THEN 1 ELSE 0 END
        FROM dashboard_cache_state
        WHERE cache_name = ?
        LIMIT 1
        "#,
    )
    .bind(cache_name)
    .fetch_optional(pool)
    .await
    {
        Ok(ready) => ready,
        Err(error) if is_missing_table_error(&error) => return Ok(false),
        Err(error) => return Err(error),
    };

    Ok(matches!(ready, Some(1)))
}

async fn fetch_prop_strategy_family_filter(
    pool: &MySqlPool,
    family_key: &str,
) -> Result<Option<PropStrategyFamilyFilter>, sqlx::Error> {
    let sql = r#"
        SELECT
            family_key,
            family_name,
            CAST(family_level AS SIGNED) AS family_level,
            included_dimensions,
            outcome_model,
            market,
            harmonic_type,
            bin,
            reversal_type,
            size_bucket,
            time_bin,
            x_strictness,
            three_month_trend,
            six_month_trend,
            twelve_month_trend
        FROM prop_strategy_family_summary
        WHERE family_key = ?
        LIMIT 1
    "#;

    match sqlx::query_as::<_, PropStrategyFamilyFilter>(sql)
        .bind(family_key)
        .fetch_optional(pool)
        .await
    {
        Ok(row) => Ok(row),
        Err(error) if is_missing_table_error(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

async fn try_fetch_setup_comparison_from_prop_strategy_rollup(
    pool: &MySqlPool,
    params: &SetupComparisonParams,
) -> Result<Option<(SetupComparisonSummary, Vec<SetupComparisonYearlyPoint>)>, sqlx::Error> {
    if let Some(prop_strategy_id) = params.prop_strategy_id.as_deref() {
        if prop_strategy_id.trim().is_empty() {
            return Ok(None);
        }

        let summary_sql = r#"
            SELECT
                CAST(total_count AS SIGNED) AS total_count,
                CAST(closed_count AS SIGNED) AS closed_count,
                CAST(open_count AS SIGNED) AS open_count,
                CAST(win_count AS SIGNED) AS win_count,
                CAST(loss_count AS SIGNED) AS loss_count,
                CAST(expectancy AS DOUBLE) AS expectancy,
                CAST(avg_return AS DOUBLE) AS avg_return,
                CAST(win_rate AS DOUBLE) AS win_rate,
                CAST(avg_win AS DOUBLE) AS avg_win,
                CAST(avg_loss AS DOUBLE) AS avg_loss,
                CAST(0.0 AS DOUBLE) AS avg_trade_length,
                CAST(0.0 AS DOUBLE) AS avg_ab_xa,
                CAST(0.0 AS DOUBLE) AS avg_bc_ab,
                CAST(0.0 AS DOUBLE) AS avg_cd_bc,
                CAST(0.0 AS DOUBLE) AS avg_cd_xa
            FROM prop_strategy_family_summary
            WHERE family_key = ?
            LIMIT 1
        "#;

        let yearly_sql = r#"
            SELECT
                CAST(trade_year AS SIGNED) AS year,
                CAST(COALESCE(SUM(total_count), 0) AS SIGNED) AS total_count,
                CAST(COALESCE(SUM(closed_count), 0) AS SIGNED) AS closed_count,
                CAST(COALESCE(SUM(open_count), 0) AS SIGNED) AS open_count,
                CAST(COALESCE(SUM(win_count), 0) AS SIGNED) AS win_count,
                CAST(COALESCE(SUM(loss_count), 0) AS SIGNED) AS loss_count,
                CAST(CASE
                    WHEN SUM(expectancy_count) > 0 THEN SUM(expectancy_sum) / SUM(expectancy_count)
                    ELSE 0.0
                END AS DOUBLE) AS expectancy,
                CAST(CASE
                    WHEN SUM(return_count) > 0 THEN SUM(return_sum) / SUM(return_count)
                    ELSE 0.0
                END AS DOUBLE) AS avg_return,
                CAST(CASE
                    WHEN SUM(closed_count) > 0 THEN SUM(win_count) / SUM(closed_count)
                    ELSE 0.0
                END AS DOUBLE) AS win_rate
            FROM prop_strategy_family_yearly
            WHERE family_key = ?
            GROUP BY trade_year
            ORDER BY trade_year ASC
        "#;

        let summary = match sqlx::query_as::<_, SetupComparisonSummary>(summary_sql)
            .bind(prop_strategy_id)
            .fetch_optional(pool)
            .await
        {
            Ok(Some(row)) => row,
            Ok(None) => return Ok(None),
            Err(error) if is_missing_table_error(&error) => return Ok(None),
            Err(error) => return Err(error),
        };

        if summary.total_count == 0 {
            return Ok(None);
        }

        let yearly_performance = match sqlx::query_as::<_, SetupComparisonYearlyPoint>(yearly_sql)
            .bind(prop_strategy_id)
            .fetch_all(pool)
            .await
        {
            Ok(rows) => rows,
            Err(error) if is_missing_table_error(&error) => return Ok(None),
            Err(error) => return Err(error),
        };

        return Ok(Some((summary, yearly_performance)));
    }

    let Some(market) = params.market.as_deref() else {
        return Ok(None);
    };
    let Some(harmonic_type) = params.harmonic_type.as_deref() else {
        return Ok(None);
    };
    let Some(bin) = params.bin.as_deref() else {
        return Ok(None);
    };

    if normalize_market_filter(Some(market)).is_none() {
        return Ok(None);
    }
    if accuracy_column_for_harmonic_type(harmonic_type).is_none()
        || dominant_harmonic_predicate(harmonic_type).is_none()
    {
        return Ok(None);
    }

    if !is_supported_bin_label(bin) {
        return Ok(None);
    }
    if let Some(size_bucket) = params.size_bucket.as_deref() {
        if !is_supported_size_bucket(size_bucket) {
            return Ok(None);
        }
    }
    if let Some(reversal_type) = params.reversal_type.as_deref() {
        if !is_supported_reversal_type(reversal_type) {
            return Ok(None);
        }
    }
    if let Some(three_month_trend) = params.three_month_trend.as_deref() {
        if !is_supported_trend_bucket(three_month_trend) {
            return Ok(None);
        }
    }
    if let Some(six_month_trend) = params.six_month_trend.as_deref() {
        if !is_supported_trend_bucket(six_month_trend) {
            return Ok(None);
        }
    }
    if let Some(twelve_month_trend) = params.twelve_month_trend.as_deref() {
        if !is_supported_trend_bucket(twelve_month_trend) {
            return Ok(None);
        }
    }

    let where_filters = String::from(
        r#"
        WHERE market = ?
          AND harmonic_type = ?
          AND bin = ?
          AND reversal_type = ?
          AND size_bucket = ?
          AND time_bin = ?
          AND three_month_trend = ?
          AND six_month_trend = ?
          AND twelve_month_trend = ?
        "#,
    );

    let summary_sql = format!(
        r#"
        SELECT
            CAST(total_count AS SIGNED) AS total_count,
            CAST(closed_count AS SIGNED) AS closed_count,
            CAST(open_count AS SIGNED) AS open_count,
            CAST(win_count AS SIGNED) AS win_count,
            CAST(loss_count AS SIGNED) AS loss_count,
            CAST(expectancy AS DOUBLE) AS expectancy,
            CAST(avg_return AS DOUBLE) AS avg_return,
            CAST(win_rate AS DOUBLE) AS win_rate,
            CAST(avg_win AS DOUBLE) AS avg_win,
            CAST(avg_loss AS DOUBLE) AS avg_loss,
            CAST(avg_trade_length AS DOUBLE) AS avg_trade_length,
            CAST(avg_ab_xa AS DOUBLE) AS avg_ab_xa,
            CAST(avg_bc_ab AS DOUBLE) AS avg_bc_ab,
            CAST(avg_cd_bc AS DOUBLE) AS avg_cd_bc,
            CAST(avg_cd_xa AS DOUBLE) AS avg_cd_xa
        FROM prop_strategy_summary
        {where_filters}
        "#,
    );

    let yearly_sql = format!(
        r#"
        SELECT
            CAST(trade_year AS SIGNED) AS year,
            CAST(COALESCE(SUM(total_count), 0) AS SIGNED) AS total_count,
            CAST(COALESCE(SUM(closed_count), 0) AS SIGNED) AS closed_count,
            CAST(COALESCE(SUM(open_count), 0) AS SIGNED) AS open_count,
            CAST(COALESCE(SUM(win_count), 0) AS SIGNED) AS win_count,
            CAST(COALESCE(SUM(loss_count), 0) AS SIGNED) AS loss_count,
            CAST(CASE
                WHEN SUM(expectancy_count) > 0 THEN SUM(expectancy_sum) / SUM(expectancy_count)
                ELSE 0.0
            END AS DOUBLE) AS expectancy,
            CAST(CASE
                WHEN SUM(return_count) > 0 THEN SUM(return_sum) / SUM(return_count)
                ELSE 0.0
            END AS DOUBLE) AS avg_return,
            CAST(CASE
                WHEN SUM(closed_count) > 0 THEN SUM(win_count) / SUM(closed_count)
                ELSE 0.0
            END AS DOUBLE) AS win_rate
        FROM prop_strategy_yearly
        {where_filters}
        GROUP BY trade_year
        ORDER BY trade_year ASC
        "#,
    );

    let Some(reversal_type) = params.reversal_type.as_deref() else {
        return Ok(None);
    };
    let Some(size_bucket) = params.size_bucket.as_deref() else {
        return Ok(None);
    };
    let Some(time_bin) = params.time_bin.as_deref() else {
        return Ok(None);
    };
    let Some(three_month_trend) = params.three_month_trend.as_deref() else {
        return Ok(None);
    };
    let Some(six_month_trend) = params.six_month_trend.as_deref() else {
        return Ok(None);
    };
    let Some(twelve_month_trend) = params.twelve_month_trend.as_deref() else {
        return Ok(None);
    };

    let summary = match sqlx::query_as::<_, SetupComparisonSummary>(&summary_sql)
        .bind(market)
        .bind(harmonic_type)
        .bind(bin)
        .bind(reversal_type)
        .bind(size_bucket)
        .bind(time_bin)
        .bind(three_month_trend)
        .bind(six_month_trend)
        .bind(twelve_month_trend)
        .fetch_one(pool)
        .await
    {
        Ok(row) => row,
        Err(error) if is_missing_table_error(&error) => return Ok(None),
        Err(error) => return Err(error),
    };

    if summary.total_count == 0 {
        return Ok(None);
    }

    let yearly_performance = match sqlx::query_as::<_, SetupComparisonYearlyPoint>(&yearly_sql)
        .bind(market)
        .bind(harmonic_type)
        .bind(bin)
        .bind(reversal_type)
        .bind(size_bucket)
        .bind(time_bin)
        .bind(three_month_trend)
        .bind(six_month_trend)
        .bind(twelve_month_trend)
        .fetch_all(pool)
        .await
    {
        Ok(rows) => rows,
        Err(error) if is_missing_table_error(&error) => return Ok(None),
        Err(error) => return Err(error),
    };

    Ok(Some((summary, yearly_performance)))
}

#[route("/strategy-trades", method = "GET", method = "POST")]
async fn fetch_strategy_trades(
    pool: web::Data<MySqlPool>,
    params: web::Json<StrategyTradesParams>,
) -> impl Responder {
    let Some(prop_strategy_id) = params
        .prop_strategy_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return HttpResponse::BadRequest().body("Missing prop strategy id");
    };

    let limit = params.limit.unwrap_or(50).clamp(1, 500);
    let offset = params.offset.unwrap_or(0).max(0);
    let include_count = params.include_count.unwrap_or(false);
    let first_start_date = parse_simulator_start_date(params.first_start_date.as_deref());
    let family = match fetch_prop_strategy_family_filter(pool.get_ref(), prop_strategy_id).await {
        Ok(Some(family)) => family,
        Ok(None) => return HttpResponse::NotFound().body("Prop strategy family not found"),
        Err(error) => {
            eprintln!("Prop strategy family lookup DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let route_x_strictness_expr = x_strictness_expr("p.x_bars_left", "p.x_length");
    let family_base_where_clause = format!(
        r#"
            WHERE p.outcome_model = ?
              AND p.market = ?
              AND p.harmonic_type = ?
              AND p.bin = ?
              AND COALESCE(NULLIF(p.reversal_type, ''), 'None') = ?
              AND p.size_bucket = ?
              AND p.time_bin = ?
              AND {route_x_strictness_expr} = ?
              AND p.three_month_trend = ?
              AND p.six_month_trend = ?
              AND p.twelve_month_trend = ?
            "#,
        route_x_strictness_expr = route_x_strictness_expr,
    );
    let family_where_clause = format!(
        r#"
            {family_base_where_clause}
              AND (? IS NULL OR COALESCE(p.entry_date, p.reversal_detect_date, p.d_confirm_date, p.d_date) >= ?)
        "#,
        family_base_where_clause = family_base_where_clause,
    );
    let range_sql = format!(
        r#"
            SELECT
                MIN(COALESCE(p.entry_date, p.reversal_detect_date, p.d_confirm_date, p.d_date)) AS earliest_entry_date,
                MAX(COALESCE(p.entry_date, p.reversal_detect_date, p.d_confirm_date, p.d_date)) AS latest_entry_date
            FROM pattern_outcomes_prop p
            {family_base_where_clause}
        "#,
        family_base_where_clause = family_base_where_clause,
    );
    let entry_dates_sql = format!(
        r#"
            SELECT DISTINCT DATE(COALESCE(p.entry_date, p.reversal_detect_date, p.d_confirm_date, p.d_date)) AS entry_date
            FROM pattern_outcomes_prop p
            {family_base_where_clause}
              AND COALESCE(p.entry_date, p.reversal_detect_date, p.d_confirm_date, p.d_date) IS NOT NULL
            ORDER BY entry_date ASC
        "#,
        family_base_where_clause = family_base_where_clause,
    );
    let count_sql = format!(
        r#"
            SELECT COUNT(*)
            FROM pattern_outcomes_prop p
            {family_where_clause}
        "#
    );

    let route_time_accuracy_expr = bin_midpoint_expr("p.time_bin");
    let route_score_projection = route_harmonic_score_projection("p.harmonic_type", "p.bin");
    let data_sql = format!(
        r#"
            SELECT
                p.symbol,
                p.d_date,
                p.d_confirm_date,
                p.reversal_detect_date,
                p.entry_date,
                p.target_date,
                CAST(p.target_open AS DECIMAL(12,2)) AS target_open,
                CAST(p.target_high AS DECIMAL(12,2)) AS target_high,
                CAST(p.target_low AS DECIMAL(12,2)) AS target_low,
                CAST(p.target_close AS DECIMAL(12,2)) AS target_close,
                CAST(p.trade_enter_price AS DECIMAL(12,2)) AS trade_enter_price,
                CAST(p.trade_risk_exit_price AS DECIMAL(12,2)) AS trade_risk_exit_price,
                CAST(p.trade_reward_exit_price AS DECIMAL(12,2)) AS trade_reward_exit_price,
                CAST(p.prop_result AS SIGNED) AS trade_result,
                p.market,
                p.pattern_id,
                p.pattern_group_id,
                ? AS prop_strategy_id,
                p.harmonic_type,
                COALESCE(NULLIF(p.reversal_type, ''), 'None') AS reversal_type,
                p.size_bucket,
                NULL AS balance_bucket,
                p.time_bin,
                {route_x_strictness_expr_for_select} AS x_strictness,
                p.three_month_trend,
                p.six_month_trend,
                p.twelve_month_trend,
                CAST({route_time_accuracy_expr} AS DOUBLE) AS time_accuracy,
                CAST(p.x_length AS SIGNED) AS x_length,
                CAST(p.a_length AS SIGNED) AS a_length,
                CAST(p.b_length AS SIGNED) AS b_length,
                CAST(p.c_length AS SIGNED) AS c_length,
                CAST(p.d_length AS SIGNED) AS d_length,
                CAST(p.full_pattern_length AS SIGNED) AS full_pattern_length,
                {route_score_projection}
            FROM pattern_outcomes_prop p
            {family_where_clause}
            ORDER BY COALESCE(p.entry_date, p.reversal_detect_date, p.d_confirm_date, p.d_date) ASC,
                     p.trade_enter_price ASC
            LIMIT ? OFFSET ?
            "#,
        route_time_accuracy_expr = route_time_accuracy_expr,
        route_score_projection = route_score_projection,
        route_x_strictness_expr_for_select = x_strictness_expr("p.x_bars_left", "p.x_length"),
        family_where_clause = family_where_clause,
    );

    let (earliest_entry_date, latest_entry_date) = match sqlx::query_as::<
        _,
        (Option<NaiveDateTime>, Option<NaiveDateTime>),
    >(&range_sql)
    .bind(&family.outcome_model)
    .bind(&family.market)
    .bind(&family.harmonic_type)
    .bind(&family.bin)
    .bind(&family.reversal_type)
    .bind(&family.size_bucket)
    .bind(&family.time_bin)
    .bind(family.x_strictness.as_deref().unwrap_or("Loose"))
    .bind(&family.three_month_trend)
    .bind(&family.six_month_trend)
    .bind(&family.twelve_month_trend)
    .fetch_one(pool.get_ref())
    .await
    {
        Ok(range) => range,
        Err(error) => {
            eprintln!("Prop strategy trades range DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let entry_dates = match sqlx::query_scalar::<_, NaiveDate>(&entry_dates_sql)
        .bind(&family.outcome_model)
        .bind(&family.market)
        .bind(&family.harmonic_type)
        .bind(&family.bin)
        .bind(&family.reversal_type)
        .bind(&family.size_bucket)
        .bind(&family.time_bin)
        .bind(family.x_strictness.as_deref().unwrap_or("Loose"))
        .bind(&family.three_month_trend)
        .bind(&family.six_month_trend)
        .bind(&family.twelve_month_trend)
        .fetch_all(pool.get_ref())
        .await
    {
        Ok(dates) => dates,
        Err(error) => {
            eprintln!("Prop strategy trades date list DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let total_count = if include_count {
        match sqlx::query_scalar::<_, i64>(&count_sql)
            .bind(&family.outcome_model)
            .bind(&family.market)
            .bind(&family.harmonic_type)
            .bind(&family.bin)
            .bind(&family.reversal_type)
            .bind(&family.size_bucket)
            .bind(&family.time_bin)
            .bind(family.x_strictness.as_deref().unwrap_or("Loose"))
            .bind(&family.three_month_trend)
            .bind(&family.six_month_trend)
            .bind(&family.twelve_month_trend)
            .bind(first_start_date)
            .bind(first_start_date)
            .fetch_one(pool.get_ref())
            .await
        {
            Ok(count) => count,
            Err(error) => {
                eprintln!("Prop strategy trades count DB error: {:?}", error);
                return HttpResponse::InternalServerError().finish();
            }
        }
    } else {
        -1
    };

    let fetch_limit = if include_count { limit } else { limit + 1 };

    let mut patterns = match sqlx::query_as::<_, PatternSummary>(&data_sql)
        .bind(prop_strategy_id)
        .bind(&family.outcome_model)
        .bind(&family.market)
        .bind(&family.harmonic_type)
        .bind(&family.bin)
        .bind(&family.reversal_type)
        .bind(&family.size_bucket)
        .bind(&family.time_bin)
        .bind(family.x_strictness.as_deref().unwrap_or("Loose"))
        .bind(&family.three_month_trend)
        .bind(&family.six_month_trend)
        .bind(&family.twelve_month_trend)
        .bind(first_start_date)
        .bind(first_start_date)
        .bind(fetch_limit)
        .bind(offset)
        .fetch_all(pool.get_ref())
        .await
    {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("Prop strategy trades data DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let has_more = if include_count {
        offset + limit < total_count
    } else if patterns.len() as i64 > limit {
        patterns.truncate(limit as usize);
        true
    } else {
        false
    };

    return HttpResponse::Ok().json(PatternSummariesResponse {
        patterns,
        total_count,
        has_more,
        earliest_entry_date,
        latest_entry_date,
        entry_dates,
    });
}

fn parse_simulator_start_date(value: Option<&str>) -> Option<NaiveDateTime> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }

    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
        .ok()
        .or_else(|| {
            NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .ok()
                .and_then(|date| date.and_hms_opt(0, 0, 0))
        })
}

fn futures_root(symbol: &str) -> String {
    let uppercase = symbol.trim().to_uppercase();
    for root in [
        "MES", "MNQ", "MYM", "M2K", "MCL", "MGC", "SIL", "RTY", "ES", "NQ", "YM", "CL", "NG", "GC",
        "SI", "6E", "6J", "ZN", "ZB",
    ] {
        if uppercase.starts_with(root) {
            return root.to_string();
        }
    }

    uppercase
        .chars()
        .take_while(|ch| ch.is_ascii_alphabetic())
        .collect::<String>()
}

fn futures_point_value(symbol: &str) -> f64 {
    match futures_root(symbol).as_str() {
        "ES" => 50.0,
        "MES" => 5.0,
        "NQ" => 20.0,
        "MNQ" => 2.0,
        "YM" => 5.0,
        "MYM" => 0.5,
        "RTY" => 50.0,
        "M2K" => 5.0,
        "CL" => 1000.0,
        "MCL" => 100.0,
        "NG" => 10000.0,
        "GC" => 100.0,
        "MGC" => 10.0,
        "SI" => 5000.0,
        "SIL" => 1000.0,
        "6E" => 125000.0,
        "6J" => 12500000.0,
        "ZN" => 1000.0,
        "ZB" => 1000.0,
        _ => 1.0,
    }
}

fn simulator_trade_pnl(trade: &SimulatorReplaySourceTrade, contracts: i64) -> f64 {
    let point_move = match trade.trade_result {
        1 => (trade.trade_reward_exit_price - trade.trade_enter_price).abs(),
        2 => -(trade.trade_risk_exit_price - trade.trade_enter_price).abs(),
        _ => 0.0,
    };

    point_move * contracts as f64 * futures_point_value(&trade.symbol)
}

fn simulator_trade_excursion(
    trade: &SimulatorReplaySourceTrade,
    contracts: i64,
    point_value: f64,
) -> (f64, f64, f64, f64) {
    let fallback_adverse_points = if trade.trade_result == 2 {
        (trade.trade_risk_exit_price - trade.trade_enter_price).abs()
    } else {
        0.0
    };
    let fallback_favorable_points = if trade.trade_result == 1 {
        (trade.trade_reward_exit_price - trade.trade_enter_price).abs()
    } else {
        0.0
    };
    let max_adverse_points = trade
        .max_adverse_points
        .unwrap_or(fallback_adverse_points)
        .max(0.0);
    let max_favorable_points = trade
        .max_favorable_points
        .unwrap_or(fallback_favorable_points)
        .max(0.0);
    let multiplier = contracts as f64 * point_value;

    (
        max_adverse_points,
        max_favorable_points,
        -max_adverse_points * multiplier,
        max_favorable_points * multiplier,
    )
}

const SIMULATOR_DRAWDOWN_MODEL_INTRADAY: &str = "intraday";
const SIMULATOR_DRAWDOWN_MODEL_EOD: &str = "eod";

fn normalize_simulator_drawdown_model(value: Option<&str>) -> &'static str {
    if value
        .map(|item| item.trim().eq_ignore_ascii_case(SIMULATOR_DRAWDOWN_MODEL_EOD))
        .unwrap_or(false)
    {
        SIMULATOR_DRAWDOWN_MODEL_EOD
    } else {
        SIMULATOR_DRAWDOWN_MODEL_INTRADAY
    }
}

fn simulator_intratrade_peak_balance(
    drawdown_model: &str,
    pre_trade_peak_balance: f64,
    intratrade_high_balance: f64,
) -> f64 {
    if drawdown_model == SIMULATOR_DRAWDOWN_MODEL_INTRADAY {
        pre_trade_peak_balance.max(intratrade_high_balance)
    } else {
        pre_trade_peak_balance
    }
}

fn simulator_intratrade_failure_reason(
    drawdown_model: &str,
    max_drawdown: f64,
    intratrade_low_balance: f64,
    trailing_floor: f64,
    daily_floor: Option<f64>,
) -> Option<String> {
    if drawdown_model == SIMULATOR_DRAWDOWN_MODEL_INTRADAY
        && max_drawdown > 0.0
        && intratrade_low_balance <= trailing_floor
    {
        Some(String::from("Intratrade trailing drawdown breached"))
    } else if daily_floor
        .map(|floor| intratrade_low_balance <= floor)
        .unwrap_or(false)
    {
        Some(String::from("Intratrade daily loss limit breached"))
    } else {
        None
    }
}

fn simulator_next_peak_balance(
    drawdown_model: &str,
    failed_intratrade_drawdown: bool,
    previous_peak_balance: f64,
    intratrade_high_balance: f64,
    balance: f64,
) -> f64 {
    if failed_intratrade_drawdown {
        previous_peak_balance
    } else if drawdown_model == SIMULATOR_DRAWDOWN_MODEL_INTRADAY {
        previous_peak_balance
            .max(intratrade_high_balance)
            .max(balance)
    } else {
        previous_peak_balance.max(balance)
    }
}

#[cfg(test)]
mod simulator_tests {
    use super::*;

    #[test]
    fn intraday_drawdown_trails_against_in_trade_high_water_mark() {
        let pre_trade_peak = 50_000.0;
        let intratrade_high = 52_500.0;
        let intratrade_low = 50_300.0;
        let max_drawdown = 2_000.0;
        let intratrade_peak = simulator_intratrade_peak_balance(
            SIMULATOR_DRAWDOWN_MODEL_INTRADAY,
            pre_trade_peak,
            intratrade_high,
        );
        let trailing_floor = intratrade_peak - max_drawdown;

        assert_eq!(intratrade_peak, intratrade_high);
        assert_eq!(trailing_floor, 50_500.0);
        assert_eq!(
            simulator_intratrade_failure_reason(
                SIMULATOR_DRAWDOWN_MODEL_INTRADAY,
                max_drawdown,
                intratrade_low,
                trailing_floor,
                None,
            ),
            Some(String::from("Intratrade trailing drawdown breached"))
        );
    }

    #[test]
    fn eod_drawdown_does_not_fail_on_intratrade_high_water_mark() {
        let pre_trade_peak = 50_000.0;
        let intratrade_high = 52_500.0;
        let intratrade_low = 50_300.0;
        let max_drawdown = 2_000.0;
        let intratrade_peak = simulator_intratrade_peak_balance(
            SIMULATOR_DRAWDOWN_MODEL_EOD,
            pre_trade_peak,
            intratrade_high,
        );
        let trailing_floor = intratrade_peak - max_drawdown;

        assert_eq!(intratrade_peak, pre_trade_peak);
        assert_eq!(
            simulator_intratrade_failure_reason(
                SIMULATOR_DRAWDOWN_MODEL_EOD,
                max_drawdown,
                intratrade_low,
                trailing_floor,
                None,
            ),
            None
        );
    }

    #[test]
    fn failed_intraday_trade_does_not_advance_peak_after_breach() {
        let previous_peak = 50_000.0;
        let intratrade_high = 52_500.0;
        let stopped_balance = 49_900.0;

        assert_eq!(
            simulator_next_peak_balance(
                SIMULATOR_DRAWDOWN_MODEL_INTRADAY,
                true,
                previous_peak,
                intratrade_high,
                stopped_balance,
            ),
            previous_peak
        );
    }
}

#[route("/simulator/family-replay", method = "GET", method = "POST")]
async fn fetch_simulator_family_replay(
    pool: web::Data<MySqlPool>,
    params: web::Json<SimulatorReplayParams>,
) -> impl Responder {
    let Some(prop_strategy_id) = params
        .prop_strategy_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return HttpResponse::BadRequest().body("Simulator replay requires family key");
    };

    let Some(first_start_date) = parse_simulator_start_date(params.first_start_date.as_deref())
    else {
        return HttpResponse::BadRequest().body("Simulator replay requires first start date");
    };

    let tests_to_chain = params.tests_to_chain.unwrap_or(1).clamp(1, 250);
    let contracts = params.contracts.unwrap_or(1).clamp(1, 500);
    let starting_balance = params.starting_balance.unwrap_or(50_000.0).max(0.0);
    let profit_target = params.profit_target.unwrap_or(3_000.0).max(0.0);
    let max_drawdown = params.max_drawdown.unwrap_or(2_000.0).max(0.0);
    let daily_loss_limit = params.daily_loss_limit.filter(|value| *value > 0.0);
    let drawdown_model = normalize_simulator_drawdown_model(params.drawdown_model.as_deref());
    let one_trade_at_a_time = params.one_trade_at_a_time.unwrap_or(true);

    let family = match fetch_prop_strategy_family_filter(pool.get_ref(), prop_strategy_id).await {
        Ok(Some(family)) => family,
        Ok(None) => return HttpResponse::NotFound().body("Prop strategy family not found"),
        Err(error) => {
            eprintln!("Simulator family lookup DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let route_x_strictness_expr = x_strictness_expr("p.x_bars_left", "p.x_length");
    let sql = format!(
        r#"
            SELECT
                p.symbol,
                p.pattern_id,
                p.pattern_group_id,
                p.d_date,
                p.d_confirm_date,
                p.reversal_detect_date,
                p.entry_date,
                p.target_date,
                CAST(p.trade_enter_price AS DOUBLE) AS trade_enter_price,
                CAST(p.trade_risk_exit_price AS DOUBLE) AS trade_risk_exit_price,
                CAST(p.trade_reward_exit_price AS DOUBLE) AS trade_reward_exit_price,
                CAST(p.trade_lowest_price AS DOUBLE) AS trade_lowest_price,
                CAST(p.trade_highest_price AS DOUBLE) AS trade_highest_price,
                CAST(p.trade_adverse_price AS DOUBLE) AS trade_adverse_price,
                CAST(p.trade_favorable_price AS DOUBLE) AS trade_favorable_price,
                CAST(p.max_adverse_points AS DOUBLE) AS max_adverse_points,
                CAST(p.max_favorable_points AS DOUBLE) AS max_favorable_points,
                CAST(p.prop_result AS SIGNED) AS trade_result
            FROM pattern_outcomes_prop p
            WHERE p.outcome_model = ?
              AND p.market = ?
              AND p.harmonic_type = ?
              AND p.bin = ?
              AND COALESCE(NULLIF(p.reversal_type, ''), 'None') = ?
              AND p.size_bucket = ?
              AND p.time_bin = ?
              AND {route_x_strictness_expr} = ?
              AND p.three_month_trend = ?
              AND p.six_month_trend = ?
              AND p.twelve_month_trend = ?
              AND p.target_date IS NOT NULL
              AND p.entry_date >= ?
            ORDER BY p.entry_date ASC,
                     p.target_date ASC,
                     p.trade_enter_price ASC
            LIMIT 50000
        "#,
        route_x_strictness_expr = route_x_strictness_expr,
    );

    let rows = match sqlx::query_as::<_, SimulatorReplaySourceTrade>(&sql)
        .bind(&family.outcome_model)
        .bind(&family.market)
        .bind(&family.harmonic_type)
        .bind(&family.bin)
        .bind(&family.reversal_type)
        .bind(&family.size_bucket)
        .bind(&family.time_bin)
        .bind(family.x_strictness.as_deref().unwrap_or("Loose"))
        .bind(&family.three_month_trend)
        .bind(&family.six_month_trend)
        .bind(&family.twelve_month_trend)
        .bind(first_start_date)
        .fetch_all(pool.get_ref())
        .await
    {
        Ok(rows) => rows,
        Err(error) if is_missing_table_error(&error) => Vec::new(),
        Err(error) => {
            eprintln!("Simulator replay DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let mut tests = Vec::new();
    let mut events = Vec::new();
    let mut cursor = 0usize;
    let mut next_start_date = first_start_date;

    for test_index in 1..=tests_to_chain {
        while cursor < rows.len() && rows[cursor].entry_date < next_start_date {
            cursor += 1;
        }

        let test_start_date = rows
            .get(cursor)
            .map(|trade| trade.entry_date)
            .unwrap_or(next_start_date);
        let mut balance = starting_balance;
        let mut peak_balance = starting_balance;
        let mut max_drawdown_seen = 0.0f64;
        let mut trade_count = 0i64;
        let mut skipped_overlap_count = 0i64;
        let mut status = String::from("waiting");
        let mut end_date = None;
        let mut busy_until: Option<NaiveDateTime> = None;
        let mut day = test_start_date.date();
        let mut day_start_balance = starting_balance;

        while cursor < rows.len() {
            let trade = &rows[cursor];
            cursor += 1;

            if trade.entry_date.date() != day {
                day = trade.entry_date.date();
                day_start_balance = balance;
            }

            if one_trade_at_a_time
                && busy_until
                    .map(|date| trade.entry_date < date)
                    .unwrap_or(false)
            {
                skipped_overlap_count += 1;
                events.push(SimulatorReplayTradeEvent {
                    test_index,
                    trade_index: trade_count + skipped_overlap_count,
                    symbol: trade.symbol.clone(),
                    pattern_id: trade.pattern_id.clone(),
                    pattern_group_id: trade.pattern_group_id.clone(),
                    entry_date: trade.entry_date,
                    target_date: trade.target_date,
                    trade_result: trade.trade_result,
                    pnl: 0.0,
                    closed_pnl: 0.0,
                    point_value: futures_point_value(&trade.symbol),
                    balance_before: balance,
                    balance,
                    closed_balance: balance,
                    intratrade_low_balance: balance,
                    intratrade_high_balance: balance,
                    intratrade_adverse_pnl: 0.0,
                    intratrade_favorable_pnl: 0.0,
                    drawdown: balance - peak_balance,
                    trade_lowest_price: trade.trade_lowest_price,
                    trade_highest_price: trade.trade_highest_price,
                    trade_adverse_price: trade.trade_adverse_price,
                    trade_favorable_price: trade.trade_favorable_price,
                    max_adverse_points: trade.max_adverse_points.unwrap_or(0.0).max(0.0),
                    max_favorable_points: trade.max_favorable_points.unwrap_or(0.0).max(0.0),
                    failed_intratrade_drawdown: false,
                    failure_reason: Some(String::from("Skipped while prior trade was open")),
                    skipped_for_overlap: true,
                });
                continue;
            }

            let point_value = futures_point_value(&trade.symbol);
            let balance_before = balance;
            let closed_pnl = simulator_trade_pnl(trade, contracts);
            let closed_balance = balance_before + closed_pnl;
            let (
                max_adverse_points,
                max_favorable_points,
                intratrade_adverse_pnl,
                intratrade_favorable_pnl,
            ) = simulator_trade_excursion(trade, contracts, point_value);
            let intratrade_low_balance = balance_before + intratrade_adverse_pnl;
            let intratrade_high_balance = balance_before + intratrade_favorable_pnl;
            let pre_close_peak_balance = peak_balance;
            let intratrade_peak_balance = simulator_intratrade_peak_balance(
                drawdown_model,
                pre_close_peak_balance,
                intratrade_high_balance,
            );
            let intratrade_trailing_floor = intratrade_peak_balance - max_drawdown;
            let daily_floor = daily_loss_limit.map(|limit| day_start_balance - limit);

            let intratrade_failure_reason = simulator_intratrade_failure_reason(
                drawdown_model,
                max_drawdown,
                intratrade_low_balance,
                intratrade_trailing_floor,
                daily_floor,
            );

            let failed_intratrade_drawdown = intratrade_failure_reason.is_some();
            let pnl = if failed_intratrade_drawdown {
                intratrade_adverse_pnl
            } else {
                closed_pnl
            };
            balance = if failed_intratrade_drawdown {
                intratrade_low_balance
            } else {
                closed_balance
            };
            peak_balance = simulator_next_peak_balance(
                drawdown_model,
                failed_intratrade_drawdown,
                pre_close_peak_balance,
                intratrade_high_balance,
                balance,
            );
            let drawdown = balance - peak_balance;
            max_drawdown_seen = if drawdown_model == SIMULATOR_DRAWDOWN_MODEL_INTRADAY {
                max_drawdown_seen
                    .min(intratrade_low_balance - intratrade_peak_balance)
                    .min(drawdown)
            } else {
                max_drawdown_seen.min(drawdown)
            };
            trade_count += 1;
            busy_until = trade.target_date;
            end_date = trade.target_date.or(Some(trade.entry_date));

            events.push(SimulatorReplayTradeEvent {
                test_index,
                trade_index: trade_count,
                symbol: trade.symbol.clone(),
                pattern_id: trade.pattern_id.clone(),
                pattern_group_id: trade.pattern_group_id.clone(),
                entry_date: trade.entry_date,
                target_date: trade.target_date,
                trade_result: trade.trade_result,
                pnl,
                closed_pnl,
                point_value,
                balance_before,
                balance,
                closed_balance,
                intratrade_low_balance,
                intratrade_high_balance,
                intratrade_adverse_pnl,
                intratrade_favorable_pnl,
                drawdown,
                trade_lowest_price: trade.trade_lowest_price,
                trade_highest_price: trade.trade_highest_price,
                trade_adverse_price: trade.trade_adverse_price,
                trade_favorable_price: trade.trade_favorable_price,
                max_adverse_points,
                max_favorable_points,
                failed_intratrade_drawdown,
                failure_reason: intratrade_failure_reason.clone(),
                skipped_for_overlap: false,
            });

            let trailing_floor = peak_balance - max_drawdown;
            if failed_intratrade_drawdown
                || balance <= trailing_floor
                || daily_floor.map(|floor| balance <= floor).unwrap_or(false)
            {
                status = String::from("failed");
                break;
            }
            if balance >= starting_balance + profit_target {
                status = String::from("passed");
                break;
            }
        }

        tests.push(SimulatorReplayTestResult {
            test_index,
            status: status.clone(),
            start_date: test_start_date,
            end_date,
            starting_balance,
            ending_balance: balance,
            peak_balance,
            max_drawdown: max_drawdown_seen,
            trade_count,
            skipped_overlap_count,
        });

        if status == "waiting" {
            break;
        }

        next_start_date = end_date.unwrap_or(test_start_date);
    }

    HttpResponse::Ok().json(SimulatorReplayResponse {
        family_key: prop_strategy_id.to_string(),
        tests,
        trades: events,
        eligible_trade_count: rows.len() as i64,
    })
}

#[route("/current-open-setups", method = "GET", method = "POST")]
async fn fetch_current_open_setups(
    pool: web::Data<MySqlPool>,
    params: web::Json<FilterParams>,
) -> impl Responder {
    if !matches!(
        normalize_trade_result_filter(params.trade_result),
        None | Some(0)
    ) {
        return HttpResponse::BadRequest().body("Current open setups only support open trades");
    }

    let prop_outcome_mode = normalize_prop_outcome_mode(params.prop_outcome_mode.as_deref());
    let (
        where_clause,
        route_bin_filter,
        market_filter,
        harmonic_type_filter,
        reversal_type_filter,
        harmonic_scope,
        all_pattern_bin_scope,
        recent_days,
        max_days_open,
    ) = match build_current_open_setups_where_clause(&params, prop_outcome_mode) {
        Ok(result) => result,
        Err(response) => return response,
    };

    let limit = params.limit.unwrap_or(1000).clamp(1, 5000);
    let offset = params.offset.unwrap_or(0).max(0);
    let include_count = params.include_count.unwrap_or(false);
    let size_bucket_filter = params.size_bucket.clone();

    let outcome_model = prop_outcome_model_label(prop_outcome_mode);
    let route_time_accuracy_expr = bin_midpoint_expr("p.time_bin");
    let route_score_projection = route_harmonic_score_projection("p.harmonic_type", "p.bin");
    let prop_current_source_sql = format!(
        r#"
        SELECT
            p.symbol,
            p.d_date,
            p.d_confirm_date,
            p.reversal_detect_date,
            p.target_date,
            CAST(p.target_open AS DECIMAL(12,2)) AS target_open,
            CAST(p.target_high AS DECIMAL(12,2)) AS target_high,
            CAST(p.target_low AS DECIMAL(12,2)) AS target_low,
            CAST(p.target_close AS DECIMAL(12,2)) AS target_close,
            CAST(p.trade_enter_price AS DECIMAL(12,2)) AS trade_enter_price,
            CAST(p.trade_risk_exit_price AS DECIMAL(12,2)) AS trade_risk_exit_price,
            CAST(p.trade_reward_exit_price AS DECIMAL(12,2)) AS trade_reward_exit_price,
            p.market,
            p.pattern_id,
            p.pattern_group_id,
            p.prop_strategy_id,
            p.outcome_model,
            p.harmonic_type,
            p.bin,
            COALESCE(NULLIF(p.reversal_type, ''), 'None') AS reversal_type,
            p.size_bucket,
            p.time_bin,
            p.x_bars_left,
            p.x_length,
            p.a_length,
            p.b_length,
            p.c_length,
            p.d_length,
            p.full_pattern_length,
            {route_score_projection},
            CAST({route_time_accuracy_expr} AS DOUBLE) AS time_accuracy,
            p.three_month_trend,
            p.six_month_trend,
            p.twelve_month_trend,
            p.target_ready,
            p.prop_result
        FROM pattern_outcomes_prop p
        WHERE p.outcome_model = '{outcome_model}'
            AND p.prop_result = 0
            AND p.target_ready = FALSE
    "#,
        route_time_accuracy_expr = route_time_accuracy_expr,
        route_score_projection = route_score_projection,
        outcome_model = outcome_model,
    );

    let count_sql = format!(
        r#"
        SELECT COUNT(*)
        FROM (
            {prop_current_source_sql}
        ) current_rows
        {where_clause}
        "#,
        prop_current_source_sql = prop_current_source_sql,
    );

    let data_sql = format!(
        r#"
        SELECT
            symbol,
            d_date,
            d_confirm_date,
            reversal_detect_date,
            target_date,
            target_open,
            target_high,
            target_low,
            target_close,
            trade_enter_price,
            trade_risk_exit_price,
            trade_reward_exit_price,
            CAST(prop_result AS SIGNED) AS trade_result,
            market,
            pattern_id,
            pattern_group_id,
            prop_strategy_id,
            harmonic_type,
            COALESCE(NULLIF(reversal_type, ''), 'None') AS reversal_type,
            size_bucket,
            NULL AS balance_bucket,
            time_bin,
            {x_strictness_expr} AS x_strictness,
            three_month_trend,
            six_month_trend,
            twelve_month_trend,
            CAST(time_accuracy AS DOUBLE) AS time_accuracy,
            CAST(x_length AS SIGNED) AS x_length,
            CAST(a_length AS SIGNED) AS a_length,
            CAST(b_length AS SIGNED) AS b_length,
            CAST(c_length AS SIGNED) AS c_length,
            CAST(d_length AS SIGNED) AS d_length,
            CAST(full_pattern_length AS SIGNED) AS full_pattern_length,
            CAST(bat_accuracy AS DECIMAL(12,2)) AS bat_accuracy,
            CAST(butterfly_accuracy AS DECIMAL(12,2)) AS butterfly_accuracy,
            CAST(gartley_accuracy AS DECIMAL(12,2)) AS gartley_accuracy,
            CAST(crab_accuracy AS DECIMAL(12,2)) AS crab_accuracy,
            CAST(shark_accuracy AS DECIMAL(12,2)) AS shark_accuracy
        FROM (
            {prop_current_source_sql}
        ) current_rows
        {where_clause}
        ORDER BY COALESCE(d_confirm_date, d_date) DESC, d_date DESC, symbol ASC, time_accuracy DESC
        LIMIT ? OFFSET ?
        "#,
        prop_current_source_sql = prop_current_source_sql,
        x_strictness_expr = x_strictness_expr("x_bars_left", "x_length"),
    );

    let total_count = if include_count {
        let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
        if let Some((_, _, min, max)) = harmonic_scope {
            count_query = count_query.bind(min).bind(max);
        } else if let Some((min, max)) = all_pattern_bin_scope {
            count_query = count_query.bind(min).bind(max);
        }
        if let Some(bin) = route_bin_filter {
            count_query = count_query.bind(bin);
        }
        if let Some(market) = market_filter {
            count_query = count_query.bind(market);
        }
        if let Some(harmonic_type) = harmonic_type_filter {
            count_query = count_query.bind(harmonic_type);
        }
        if let Some(reversal_type) = reversal_type_filter {
            count_query = count_query.bind(reversal_type);
        }
        if let Some(size_bucket) = size_bucket_filter.as_deref() {
            count_query = count_query.bind(size_bucket);
        }
        if let Some(days) = recent_days {
            count_query = count_query.bind(days);
        }
        if let Some(days_open) = max_days_open {
            count_query = count_query.bind(days_open);
        }

        match count_query.fetch_one(pool.get_ref()).await {
            Ok(count) => count,
            Err(error) => {
                eprintln!("Current open setups count DB error: {:?}", error);
                return HttpResponse::InternalServerError().finish();
            }
        }
    } else {
        -1
    };

    let fetch_limit = if include_count { limit } else { limit + 1 };

    let mut data_query = sqlx::query_as::<_, PatternSummary>(&data_sql);
    if let Some((_, _, min, max)) = harmonic_scope {
        data_query = data_query.bind(min).bind(max);
    } else if let Some((min, max)) = all_pattern_bin_scope {
        data_query = data_query.bind(min).bind(max);
    }
    if let Some(bin) = route_bin_filter {
        data_query = data_query.bind(bin);
    }
    if let Some(market) = market_filter {
        data_query = data_query.bind(market);
    }
    if let Some(harmonic_type) = harmonic_type_filter {
        data_query = data_query.bind(harmonic_type);
    }
    if let Some(reversal_type) = reversal_type_filter {
        data_query = data_query.bind(reversal_type);
    }
    if let Some(size_bucket) = size_bucket_filter.as_deref() {
        data_query = data_query.bind(size_bucket);
    }
    if let Some(days) = recent_days {
        data_query = data_query.bind(days);
    }
    if let Some(days_open) = max_days_open {
        data_query = data_query.bind(days_open);
    }

    let mut patterns = match data_query
        .bind(fetch_limit)
        .bind(offset)
        .fetch_all(pool.get_ref())
        .await
    {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("Current open setups data DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let has_more = if include_count {
        offset + limit < total_count
    } else if patterns.len() as i64 > limit {
        patterns.truncate(limit as usize);
        true
    } else {
        false
    };

    HttpResponse::Ok().json(PatternSummariesResponse {
        patterns,
        total_count,
        has_more,
        earliest_entry_date: None,
        latest_entry_date: None,
        entry_dates: Vec::new(),
    })
}

#[route("/current-setup-strategies", method = "GET", method = "POST")]
async fn fetch_current_setup_strategies(
    pool: web::Data<MySqlPool>,
    params: web::Json<FilterParams>,
) -> impl Responder {
    let prop_outcome_mode = normalize_prop_outcome_mode(params.prop_outcome_mode.as_deref());
    match is_rollup_cache_ready(pool.get_ref(), "prop_strategy_family_rollups").await {
        Ok(true) => {}
        Ok(false) => {
            return HttpResponse::Ok().json(Vec::<StrategyCandidateSummary>::new());
        }
        Err(error) => {
            eprintln!(
                "Current setup strategies cache readiness error: {:?}",
                error
            );
            return HttpResponse::InternalServerError().finish();
        }
    }

    let (
        _where_clause,
        route_bin_filter,
        market_filter,
        harmonic_type_filter,
        reversal_type_filter,
        _harmonic_scope,
        _all_pattern_bin_scope,
        _recent_days,
        _max_days_open,
    ) = match build_current_open_setups_where_clause(&params, prop_outcome_mode) {
        Ok(result) => result,
        Err(response) => return response,
    };
    let size_bucket_filter = params.size_bucket.clone();
    let limit = params.limit.unwrap_or(500).clamp(1, 1_000);
    let outcome_model = prop_outcome_model_label(prop_outcome_mode);
    let mut summary_filters = String::new();
    if route_bin_filter.is_some() {
        summary_filters.push_str(" AND s.bin = ?");
    }
    if market_filter.is_some() {
        summary_filters.push_str(" AND s.market = ?");
    }
    if harmonic_type_filter.is_some() {
        summary_filters.push_str(" AND s.harmonic_type = ?");
    }
    if reversal_type_filter.is_some() {
        summary_filters.push_str(" AND s.reversal_type = ?");
    }
    if size_bucket_filter.is_some() {
        summary_filters.push_str(" AND s.size_bucket = ?");
    }
    if params.min_closed_trades.is_some() {
        summary_filters.push_str(" AND s.closed_count >= ?");
    }
    if params.min_expectancy.is_some() {
        summary_filters.push_str(" AND s.expectancy > ?");
    }
    if params.max_down_years.is_some() {
        summary_filters.push_str(" AND s.down_years <= ?");
    }
    if params.min_worst_year_expectancy.is_some() {
        summary_filters.push_str(" AND s.worst_year_expectancy > ?");
    }
    if params.min_score.is_some() {
        summary_filters.push_str(" AND s.score > ?");
    }
    append_prop_family_option_filters(&mut summary_filters, "s", &params);
    let has_cadence = table_exists(pool.get_ref(), "prop_strategy_family_weekly_cadence")
        .await
        .unwrap_or(false);
    let cadence_select = cadence_select_fields(has_cadence);
    let cadence_join = cadence_join_clause(has_cadence);
    let sort_column =
        prop_family_sort_column(params.sort_by.as_deref(), "s", has_cadence.then_some("c"));
    let sort_direction = prop_family_sort_direction(params.sort_direction.as_deref());

    let sql = format!(
        r#"
        SELECT
            s.family_key AS prop_strategy_id,
            s.family_key,
            s.family_name,
            CAST(s.family_level AS SIGNED) AS family_level,
            s.included_dimensions,
            s.outcome_model,
            s.market,
            s.harmonic_type,
            s.bin,
            s.reversal_type,
            s.size_bucket,
            s.time_bin,
            s.x_strictness,
            s.three_month_trend,
            s.six_month_trend,
            s.twelve_month_trend,
            CAST(s.worst_year_expectancy AS DOUBLE) AS worst_year_expectancy,
            CAST(s.down_years AS SIGNED) AS down_years,
            CAST(s.total_count AS SIGNED) AS total_count,
            CAST(s.closed_count AS SIGNED) AS closed_count,
            CAST(s.open_count AS SIGNED) AS open_count,
            CAST(s.win_count AS SIGNED) AS win_count,
            CAST(s.loss_count AS SIGNED) AS loss_count,
            CAST(s.expectancy AS DOUBLE) AS expectancy,
            CAST(s.avg_return AS DOUBLE) AS avg_return,
            CAST(s.win_rate AS DOUBLE) AS win_rate,
            CAST(s.closed_rate AS DOUBLE) AS closed_rate,
            CAST(s.avg_win AS DOUBLE) AS avg_win,
            CAST(s.avg_loss AS DOUBLE) AS avg_loss,
            CAST(s.avg_target_range AS DOUBLE) AS avg_target_range,
            CAST(s.score AS DOUBLE) AS score,
            {cadence_select}
        FROM prop_strategy_family_summary s
        {cadence_join}
        WHERE s.outcome_model = ?
          {summary_filters}
        ORDER BY
            {sort_column} {sort_direction},
            s.score DESC,
            s.expectancy DESC,
            s.closed_count DESC
        LIMIT ?
        "#,
        summary_filters = summary_filters,
        cadence_select = cadence_select,
        cadence_join = cadence_join,
        sort_column = sort_column,
        sort_direction = sort_direction,
    );

    let mut query = sqlx::query_as::<_, StrategyCandidateSummary>(&sql).bind(outcome_model);
    if let Some(bin) = route_bin_filter {
        query = query.bind(bin);
    }
    if let Some(market) = market_filter {
        query = query.bind(market);
    }
    if let Some(harmonic_type) = harmonic_type_filter {
        query = query.bind(harmonic_type);
    }
    if let Some(reversal_type) = reversal_type_filter {
        query = query.bind(reversal_type);
    }
    if let Some(size_bucket) = size_bucket_filter.as_deref() {
        query = query.bind(size_bucket);
    }
    if let Some(min_closed_trades) = params.min_closed_trades {
        query = query.bind(min_closed_trades);
    }
    if let Some(min_expectancy) = params.min_expectancy {
        query = query.bind(min_expectancy);
    }
    if let Some(max_down_years) = params.max_down_years {
        query = query.bind(max_down_years);
    }
    if let Some(min_worst_year_expectancy) = params.min_worst_year_expectancy {
        query = query.bind(min_worst_year_expectancy);
    }
    if let Some(min_score) = params.min_score {
        query = query.bind(min_score);
    }
    for values in [
        params.strategy_markets.as_ref(),
        params.strategy_harmonic_types.as_ref(),
        params.strategy_bins.as_ref(),
        params.strategy_reversal_types.as_ref(),
        params.strategy_size_buckets.as_ref(),
        params.strategy_time_bins.as_ref(),
        params.strategy_x_strictness.as_ref(),
        params.strategy_three_month_trends.as_ref(),
        params.strategy_six_month_trends.as_ref(),
        params.strategy_twelve_month_trends.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        for value in values {
            query = query.bind(value);
        }
    }
    query = query.bind(limit);

    match query.fetch_all(pool.get_ref()).await {
        Ok(rows) => HttpResponse::Ok().json(rows),
        Err(error) => {
            eprintln!("Current setup strategies DB error: {:?}", error);
            HttpResponse::InternalServerError().finish()
        }
    }
}

fn structure_sum_abc_expr() -> &'static str {
    "CAST(COALESCE(a_length, 0) + COALESCE(b_length, 0) + COALESCE(c_length, 0) AS DOUBLE)"
}

fn structure_pair_ratio_expr(left: &str, right: &str) -> String {
    format!(
        "CASE
            WHEN COALESCE({left}, 0) <= 0 OR COALESCE({right}, 0) <= 0 THEN NULL
            ELSE GREATEST(CAST({left} AS DOUBLE), CAST({right} AS DOUBLE))
                / LEAST(CAST({left} AS DOUBLE), CAST({right} AS DOUBLE))
        END",
        left = left,
        right = right,
    )
}

fn structure_balance_ratio_expr() -> String {
    let x_vs_pattern = format!(
        "CASE
            WHEN COALESCE(x_length, 0) <= 0 OR {sum_abc} <= 0 THEN NULL
            ELSE CAST(x_length AS DOUBLE) / {sum_abc}
        END",
        sum_abc = structure_sum_abc_expr(),
    );
    let a_vs_x = structure_pair_ratio_expr("a_length", "x_length");
    let b_vs_a = structure_pair_ratio_expr("b_length", "a_length");
    let c_vs_b = structure_pair_ratio_expr("c_length", "b_length");

    format!(
        "CASE
            WHEN {x_vs_pattern} IS NULL
                AND {a_vs_x} IS NULL
                AND {b_vs_a} IS NULL
                AND {c_vs_b} IS NULL THEN NULL
            ELSE GREATEST(
                COALESCE({x_vs_pattern}, 1.0),
                COALESCE({a_vs_x}, 1.0),
                COALESCE({b_vs_a}, 1.0),
                COALESCE({c_vs_b}, 1.0)
            )
        END",
        x_vs_pattern = x_vs_pattern,
        a_vs_x = a_vs_x,
        b_vs_a = b_vs_a,
        c_vs_b = c_vs_b,
    )
}

fn structure_balance_bucket_expr(balance_ratio_expr: &str) -> String {
    format!(
        "CASE
            WHEN {ratio} IS NULL THEN 'Unknown'
            WHEN {ratio} <= 1.25 THEN 'Tight'
            WHEN {ratio} <= 1.5 THEN 'Balanced'
            WHEN {ratio} <= 2.0 THEN 'Stretched'
            ELSE 'Distorted'
        END",
        ratio = balance_ratio_expr,
    )
}

fn x_strictness_expr(x_bars_left_expr: &str, x_length_expr: &str) -> String {
    format!(
        "CASE
            WHEN COALESCE({x_length_expr}, 0) <= 0 THEN 'Loose'
            WHEN COALESCE({x_bars_left_expr}, 0) >= COALESCE({x_length_expr}, 0) THEN 'Strict'
            WHEN COALESCE({x_bars_left_expr}, 0) * 2 >= COALESCE({x_length_expr}, 0) THEN 'Normal'
            ELSE 'Loose'
        END",
        x_bars_left_expr = x_bars_left_expr,
        x_length_expr = x_length_expr,
    )
}

fn bin_midpoint_expr(bin_expr: &str) -> String {
    format!(
        "CASE
            WHEN {bin_expr} = '0-10' THEN 5.0
            WHEN {bin_expr} = '10-20' THEN 15.0
            WHEN {bin_expr} = '20-30' THEN 25.0
            WHEN {bin_expr} = '30-40' THEN 35.0
            WHEN {bin_expr} = '40-50' THEN 45.0
            WHEN {bin_expr} = '50-60' THEN 55.0
            WHEN {bin_expr} = '60-70' THEN 65.0
            WHEN {bin_expr} = '70-80' THEN 75.0
            WHEN {bin_expr} = '80-90' THEN 85.0
            WHEN {bin_expr} = '90-100' THEN 95.0
            ELSE NULL
        END",
        bin_expr = bin_expr,
    )
}

fn route_harmonic_score_expr(
    harmonic_type_expr: &str,
    bin_expr: &str,
    harmonic_type: &str,
) -> String {
    let midpoint_expr = bin_midpoint_expr(bin_expr);
    let predicate = match harmonic_type {
        "Bat" => format!("{harmonic_type_expr} IN ('Bat', 'AlternateBat')"),
        "Crab" => format!("{harmonic_type_expr} IN ('Crab', 'DeepCrab')"),
        _ => format!("{harmonic_type_expr} = '{harmonic_type}'"),
    };
    format!(
        "CAST(CASE WHEN {predicate} THEN {midpoint_expr} ELSE NULL END AS DECIMAL(12,2))",
        predicate = predicate,
        midpoint_expr = midpoint_expr,
    )
}

fn route_harmonic_score_projection(harmonic_type_expr: &str, bin_expr: &str) -> String {
    format!(
        "{bat_expr} AS bat_accuracy,
            {butterfly_expr} AS butterfly_accuracy,
            {gartley_expr} AS gartley_accuracy,
            {crab_expr} AS crab_accuracy,
            {shark_expr} AS shark_accuracy",
        bat_expr = route_harmonic_score_expr(harmonic_type_expr, bin_expr, "Bat"),
        butterfly_expr = route_harmonic_score_expr(harmonic_type_expr, bin_expr, "Butterfly"),
        gartley_expr = route_harmonic_score_expr(harmonic_type_expr, bin_expr, "Gartley"),
        crab_expr = route_harmonic_score_expr(harmonic_type_expr, bin_expr, "Crab"),
        shark_expr = route_harmonic_score_expr(harmonic_type_expr, bin_expr, "Shark"),
    )
}

fn build_current_open_setups_where_clause(
    params: &FilterParams,
    _prop_outcome_mode: PropOutcomeMode,
) -> Result<
    (
        String,
        Option<&str>,
        Option<&str>,
        Option<&str>,
        Option<&str>,
        Option<(&'static str, &'static str, f64, f64)>,
        Option<(f64, f64)>,
        Option<i64>,
        Option<i64>,
    ),
    HttpResponse,
> {
    let market_filter = normalize_market_filter(params.market.as_deref());
    let harmonic_type_filter = params.harmonic_type.as_deref();
    let reversal_type_filter = params.reversal_type.as_deref();
    let size_bucket_filter = params.size_bucket.as_deref();
    let bin_filter = params.bin.as_deref();
    let route_bin_filter = bin_filter;
    let recent_days = params.recent_days.map(|days| days.max(1).min(30));
    let max_days_open = params.max_days_open.map(|days| days.max(0).min(3650));

    let harmonic_scope = None;
    let all_pattern_bin_scope = None;

    if let Some(harmonic_type) = harmonic_type_filter {
        if accuracy_column_for_harmonic_type(harmonic_type).is_none() {
            return Err(HttpResponse::BadRequest().body("Unsupported harmonic type"));
        }
    }

    if let Some(bin) = route_bin_filter {
        if !is_supported_bin_label(bin) {
            return Err(HttpResponse::BadRequest().body("Unsupported bin"));
        }
    }

    if let Some(reversal_type) = reversal_type_filter {
        if !is_supported_reversal_type(reversal_type) {
            return Err(HttpResponse::BadRequest().body("Unsupported reversal type"));
        }
    }

    if let Some(size_bucket) = size_bucket_filter {
        if !is_supported_size_bucket(size_bucket) {
            return Err(HttpResponse::BadRequest().body("Unsupported size bucket"));
        }
    }

    let mut where_clause = String::from(
        r#"
        WHERE 1 = 1
        "#,
    );

    if let Some((column, dominant_predicate, _, _)) = harmonic_scope {
        where_clause.push_str(" AND ");
        where_clause.push_str(column);
        where_clause.push_str(" > ? AND ");
        where_clause.push_str(column);
        where_clause.push_str(" <= ? AND (");
        where_clause.push_str(dominant_predicate);
        where_clause.push(')');
    } else if all_pattern_bin_scope.is_some() {
        where_clause.push_str(
            " AND GREATEST(
                COALESCE(bat_accuracy, -1),
                COALESCE(butterfly_accuracy, -1),
                COALESCE(gartley_accuracy, -1),
                COALESCE(crab_accuracy, -1),
                COALESCE(shark_accuracy, -1)
            ) > ? AND GREATEST(
                COALESCE(bat_accuracy, -1),
                COALESCE(butterfly_accuracy, -1),
                COALESCE(gartley_accuracy, -1),
                COALESCE(crab_accuracy, -1),
                COALESCE(shark_accuracy, -1)
            ) <= ?",
        );
    }

    if route_bin_filter.is_some() {
        where_clause.push_str(" AND bin = ?");
    }
    if market_filter.is_some() {
        where_clause.push_str(" AND market = ?");
    }
    if harmonic_type_filter.is_some() {
        where_clause.push_str(" AND harmonic_type = ?");
    }
    if reversal_type_filter.is_some() {
        where_clause.push_str(" AND reversal_type = ?");
    }
    if size_bucket_filter.is_some() {
        where_clause.push_str(" AND size_bucket = ?");
    }
    if recent_days.is_some() {
        where_clause.push_str(
            " AND COALESCE(d_confirm_date, d_date) >= DATE_SUB(CURDATE(), INTERVAL ? DAY)",
        );
    }
    if max_days_open.is_some() {
        where_clause.push_str(" AND DATEDIFF(CURDATE(), COALESCE(d_confirm_date, d_date)) <= ?");
    }

    Ok((
        where_clause,
        route_bin_filter,
        market_filter,
        harmonic_type_filter,
        reversal_type_filter,
        harmonic_scope,
        all_pattern_bin_scope,
        recent_days,
        max_days_open,
    ))
}

#[route("/storage/candle-summary", method = "GET", method = "POST")]
async fn fetch_candle_storage_summary(
    pool: web::Data<MySqlPool>,
    params: web::Json<CandleStorageParams>,
) -> impl Responder {
    match build_candle_storage_response(pool.get_ref(), &params).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(error) => {
            eprintln!("Candle storage summary DB error: {:?}", error);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[route("/admin/status", method = "GET", method = "POST")]
async fn fetch_admin_status(pool: web::Data<MySqlPool>) -> impl Responder {
    match build_admin_status_response(pool.get_ref()).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(error) => {
            eprintln!("Admin status DB error: {:?}", error);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[route("/admin/run-action", method = "POST")]
async fn run_admin_action(
    pool: web::Data<MySqlPool>,
    params: web::Json<AdminActionParams>,
) -> impl Responder {
    if let Err(error) = ensure_admin_operation_table(pool.get_ref()).await {
        eprintln!("Admin operation table error: {:?}", error);
        return HttpResponse::InternalServerError().finish();
    }

    let spec = match build_admin_command(&params) {
        Ok(spec) => spec,
        Err(message) => return HttpResponse::BadRequest().body(message),
    };

    let run_id = match insert_admin_operation(pool.get_ref(), &params, &spec.command_text).await {
        Ok(run_id) => run_id,
        Err(error) => {
            eprintln!("Admin operation insert error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let response = AdminActionStartResponse {
        run_id,
        status: "running".to_string(),
        action: params.action.trim().to_string(),
        command_text: spec.command_text.clone(),
    };

    let background_pool = pool.get_ref().clone();
    tokio::spawn(async move {
        run_admin_command_background(background_pool, run_id, spec).await;
    });

    HttpResponse::Ok().json(response)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("🔹 Starting server...");

    // --- Connect to local DB ---
    let database_url = "mysql://rperezkc:Nar8uto!@localhost:3306/abcd";

    let pool = match MySqlPool::connect(database_url).await {
        Ok(pool) => {
            println!("✅ Connected to local DB");
            pool
        }
        Err(e) => {
            eprintln!("❌ Failed to connect to DB: {:?}", e);
            panic!();
        }
    };
    let pool = web::Data::new(pool);

    // --- Start server ---
    HttpServer::new(move || {
        App::new()
            .app_data(pool.clone())
            .wrap(
                Cors::default()
                    .allowed_origin("http://localhost:3000")
                    .allowed_methods(vec!["GET", "POST"])
                    .allowed_headers(vec![actix_web::http::header::CONTENT_TYPE])
                    .max_age(3600),
            )
            .service(fetch_candles)
            .service(fetch_strategy_trades)
            .service(fetch_simulator_family_replay)
            .service(fetch_current_open_setups)
            .service(fetch_current_setup_strategies)
            .service(fetch_pattern_detail)
            .service(fetch_candle_storage_summary)
            .service(fetch_admin_status)
            .service(run_admin_action)
            .service(fetch_setup_comparison)
            .service(fetch_strategy_candidates)
            .service(fetch_strategy_contract_weeks)
            .wrap(Logger::default()) // built-in Actix logs
            .wrap_fn(|req, srv| {
                // <-- ADD THIS
                println!("🔔 Incoming request: {} {}", req.method(), req.path());
                let fut = srv.call(req);
                async move { fut.await }
            })
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

#[route("/pattern-detail", method = "GET", method = "POST")]
async fn fetch_pattern_detail(
    pool: web::Data<MySqlPool>,
    params: web::Json<PatternDetailParams>,
) -> impl Responder {
    match fetch_pattern_detail_from_prop_outcomes(pool.get_ref(), &params).await {
        Ok(Some(pattern)) => HttpResponse::Ok().json(pattern),
        Ok(None) => HttpResponse::NotFound().body("Pattern not found in pattern_outcomes_prop"),
        Err(error) => {
            eprintln!("Pattern detail prop-outcome lookup failed: {:?}", error);
            HttpResponse::InternalServerError().finish()
        }
    }
}

fn pattern_detail_candle_window_limit(params: &PatternDetailParams) -> i64 {
    let fallback_limit = 120_000;
    let Some(total_setup_length) = [
        params.x_length,
        params.a_length,
        params.b_length,
        params.c_length,
    ]
    .into_iter()
    .try_fold(0_i64, |sum, value| value.map(|length| sum + length)) else {
        return fallback_limit;
    };

    (total_setup_length + 32).clamp(16, fallback_limit)
}

async fn fetch_pattern_detail_from_prop_outcomes(
    pool: &MySqlPool,
    params: &PatternDetailParams,
) -> Result<Option<Pattern>, sqlx::Error> {
    let has_pattern_id = params
        .pattern_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();

    let route_score_projection = route_harmonic_score_projection("p.harmonic_type", "p.bin");
    let route_time_accuracy_expr = bin_midpoint_expr("p.time_bin");
    let balance_ratio_expr = structure_balance_ratio_expr()
        .replace("x_length", "p.x_length")
        .replace("a_length", "p.a_length")
        .replace("b_length", "p.b_length")
        .replace("c_length", "p.c_length");
    let balance_bucket_expr = structure_balance_bucket_expr(&balance_ratio_expr);

    let mut outcome_where = if has_pattern_id {
        String::from("WHERE p.pattern_id = ?")
    } else {
        String::from("WHERE p.pattern_group_id = ?")
    };
    let should_apply_exact_geometry_filters = !has_pattern_id;
    let requested_outcome_model = params
        .prop_outcome_mode
        .as_deref()
        .map(|mode| prop_outcome_model_label(normalize_prop_outcome_mode(Some(mode))));

    if requested_outcome_model.is_some() {
        outcome_where.push_str(" AND p.outcome_model = ?");
    }

    if !has_pattern_id && params.d_date.is_some() {
        outcome_where.push_str(" AND p.d_date = ?");
    }
    if !has_pattern_id && params.market.is_some() {
        outcome_where.push_str(" AND p.market = ?");
    }
    if !has_pattern_id && params.harmonic_type.is_some() {
        outcome_where.push_str(" AND p.harmonic_type = ?");
    }
    if !has_pattern_id && params.size_bucket.is_some() {
        outcome_where.push_str(" AND p.size_bucket = ?");
    }
    if !has_pattern_id && params.balance_bucket.is_some() {
        outcome_where.push_str(&format!(" AND {balance_bucket_expr} = ?"));
    }
    if should_apply_exact_geometry_filters && params.trade_enter_price.is_some() {
        outcome_where.push_str(" AND p.trade_enter_price = ?");
    }
    if should_apply_exact_geometry_filters && params.trade_risk_exit_price.is_some() {
        outcome_where.push_str(" AND p.trade_risk_exit_price = ?");
    }
    if should_apply_exact_geometry_filters && params.trade_reward_exit_price.is_some() {
        outcome_where.push_str(" AND p.trade_reward_exit_price = ?");
    }
    if should_apply_exact_geometry_filters && params.x_length.is_some() {
        outcome_where.push_str(" AND p.x_length = ?");
    }
    if should_apply_exact_geometry_filters && params.a_length.is_some() {
        outcome_where.push_str(" AND p.a_length = ?");
    }
    if should_apply_exact_geometry_filters && params.b_length.is_some() {
        outcome_where.push_str(" AND p.b_length = ?");
    }
    if should_apply_exact_geometry_filters && params.c_length.is_some() {
        outcome_where.push_str(" AND p.c_length = ?");
    }

    let candle_window_limit = pattern_detail_candle_window_limit(params);
    let sql = format!(
        r#"
        WITH selected_outcome AS (
            SELECT p.*
            FROM pattern_outcomes_prop p
            {outcome_where}
            ORDER BY COALESCE(p.reversal_detect_date, p.d_confirm_date, p.d_date) DESC,
                     p.trade_enter_price ASC
            LIMIT 1
        ),
        selected_source AS (
            SELECT
                p.*,
                EXISTS (
                    SELECT 1
                    FROM candles d_check
                    WHERE d_check.symbol = p.symbol
                      AND d_check.date = p.d_date
                    LIMIT 1
                ) AS use_daily_candles
            FROM selected_outcome p
        ),
        selected_candles AS (
            SELECT *
            FROM (
                SELECT
                    candle.symbol,
                    CAST(candle.date AS DATETIME) AS date,
                    CAST(candle.open AS DECIMAL(18,6)) AS open,
                    CAST(candle.high AS DECIMAL(18,6)) AS high,
                    CAST(candle.low AS DECIMAL(18,6)) AS low,
                    CAST(candle.close AS DECIMAL(18,6)) AS close,
                    candle.volume,
                    candle.three_month,
                    candle.six_month,
                    candle.twelve_month
                FROM candles candle
                INNER JOIN selected_source p
                    ON p.use_daily_candles
                   AND p.symbol = candle.symbol
                   AND candle.date <= p.d_date
                ORDER BY candle.date DESC
                LIMIT ?
            ) daily_candles
            UNION ALL
            SELECT
                *
            FROM (
                SELECT
                    candle.symbol,
                    candle.ts_utc AS date,
                    CAST(candle.open AS DECIMAL(18,6)) AS open,
                    CAST(candle.high AS DECIMAL(18,6)) AS high,
                    CAST(candle.low AS DECIMAL(18,6)) AS low,
                    CAST(candle.close AS DECIMAL(18,6)) AS close,
                    candle.volume,
                    CAST(NULL AS SIGNED) AS three_month,
                    CAST(NULL AS SIGNED) AS six_month,
                    CAST(NULL AS SIGNED) AS twelve_month
                FROM futures_contract_1m_candles candle
                INNER JOIN selected_source p
                    ON NOT p.use_daily_candles
                   AND p.symbol = candle.symbol
                   AND candle.ts_utc <= p.d_date
                ORDER BY candle.ts_utc DESC
                LIMIT ?
            ) futures_candles
        ),
        ranked_candles AS (
            SELECT
                c.symbol,
                c.date,
                c.open,
                c.high,
                c.low,
                c.close,
                c.volume,
                c.three_month,
                c.six_month,
                c.twelve_month,
                ROW_NUMBER() OVER (PARTITION BY c.symbol ORDER BY c.date) AS rn
            FROM selected_candles c
        ),
        indexed_outcome AS (
            SELECT
                p.*,
                CAST(d.rn AS SIGNED) AS d_rn,
                CAST(d.rn AS SIGNED) - CAST(p.c_length AS SIGNED) AS c_rn,
                CAST(d.rn AS SIGNED) - CAST(p.c_length AS SIGNED) - CAST(p.b_length AS SIGNED) AS b_rn,
                CAST(d.rn AS SIGNED) - CAST(p.c_length AS SIGNED) - CAST(p.b_length AS SIGNED) - CAST(p.a_length AS SIGNED) AS a_rn,
                CAST(d.rn AS SIGNED) - CAST(p.c_length AS SIGNED) - CAST(p.b_length AS SIGNED) - CAST(p.a_length AS SIGNED) - CAST(p.x_length AS SIGNED) AS x_rn
            FROM selected_outcome p
            INNER JOIN ranked_candles d
                ON d.symbol = p.symbol
               AND d.date = p.d_date
        ),
        shape AS (
            SELECT
                p.*,
                x.date AS x_date,
                x.open AS x_open,
                x.high AS x_high,
                x.low AS x_low,
                x.close AS x_close,
                CASE WHEN p.market = 'Bearish' THEN x.high ELSE x.low END AS x_min_max,
                a.date AS a_date,
                a.open AS a_open,
                a.high AS a_high,
                a.low AS a_low,
                a.close AS a_close,
                CASE WHEN p.market = 'Bearish' THEN a.low ELSE a.high END AS a_min_max,
                b.date AS b_date,
                b.open AS b_open,
                b.high AS b_high,
                b.low AS b_low,
                b.close AS b_close,
                CASE WHEN p.market = 'Bearish' THEN b.high ELSE b.low END AS b_min_max,
                c.date AS c_date,
                c.open AS c_open,
                c.high AS c_high,
                c.low AS c_low,
                c.close AS c_close,
                CASE WHEN p.market = 'Bearish' THEN c.low ELSE c.high END AS c_min_max,
                d.open AS d_open,
                d.high AS d_high,
                d.low AS d_low,
                d.close AS d_close,
                CASE WHEN p.market = 'Bearish' THEN d.high ELSE d.low END AS d_min_max
            FROM indexed_outcome p
            INNER JOIN ranked_candles x
                ON x.symbol = p.symbol
               AND x.rn = p.x_rn
            INNER JOIN ranked_candles a
                ON a.symbol = p.symbol
               AND a.rn = p.a_rn
            INNER JOIN ranked_candles b
                ON b.symbol = p.symbol
               AND b.rn = p.b_rn
            INNER JOIN ranked_candles c
                ON c.symbol = p.symbol
               AND c.rn = p.c_rn
            INNER JOIN ranked_candles d
                ON d.symbol = p.symbol
               AND d.rn = p.d_rn
        )
        SELECT
            p.symbol,
            p.pattern_id,
            p.x_date,
            CAST(p.x_open AS DECIMAL(18,6)) AS x_open,
            CAST(p.x_high AS DECIMAL(18,6)) AS x_high,
            CAST(p.x_low AS DECIMAL(18,6)) AS x_low,
            CAST(p.x_close AS DECIMAL(18,6)) AS x_close,
            CAST(p.x_length AS DECIMAL(18,0)) AS x_length,
            CAST(p.x_min_max AS DECIMAL(18,6)) AS x_min_max,
            p.a_date,
            CAST(p.a_open AS DECIMAL(18,6)) AS a_open,
            CAST(p.a_high AS DECIMAL(18,6)) AS a_high,
            CAST(p.a_low AS DECIMAL(18,6)) AS a_low,
            CAST(p.a_close AS DECIMAL(18,6)) AS a_close,
            CAST(p.a_length AS DECIMAL(18,0)) AS a_length,
            CAST(p.a_min_max AS DECIMAL(18,6)) AS a_min_max,
            p.b_date,
            CAST(p.b_open AS DECIMAL(18,6)) AS b_open,
            CAST(p.b_high AS DECIMAL(18,6)) AS b_high,
            CAST(p.b_low AS DECIMAL(18,6)) AS b_low,
            CAST(p.b_close AS DECIMAL(18,6)) AS b_close,
            CAST(p.b_length AS DECIMAL(18,0)) AS b_length,
            CAST(p.b_min_max AS DECIMAL(18,6)) AS b_min_max,
            p.c_date,
            CAST(p.c_open AS DECIMAL(18,6)) AS c_open,
            CAST(p.c_high AS DECIMAL(18,6)) AS c_high,
            CAST(p.c_low AS DECIMAL(18,6)) AS c_low,
            CAST(p.c_close AS DECIMAL(18,6)) AS c_close,
            CAST(p.c_length AS DECIMAL(18,0)) AS c_length,
            CAST(p.c_min_max AS DECIMAL(18,6)) AS c_min_max,
            p.d_date,
            CAST(p.d_open AS DECIMAL(18,6)) AS d_open,
            CAST(p.d_high AS DECIMAL(18,6)) AS d_high,
            CAST(p.d_low AS DECIMAL(18,6)) AS d_low,
            CAST(p.d_close AS DECIMAL(18,6)) AS d_close,
            CAST(p.d_length AS DECIMAL(18,0)) AS d_length,
            CAST(p.full_pattern_length AS SIGNED) AS full_pattern_length,
            CAST(p.d_min_max AS DECIMAL(18,6)) AS d_min_max,
            CASE WHEN p.target_date IS NULL THEN TRUE ELSE FALSE END AS trade_open,
            p.entry_date,
            p.d_confirm_date,
            p.reversal_detect_date,
            p.target_date,
            CAST(p.target_open AS DECIMAL(18,6)) AS target_open,
            CAST(p.target_high AS DECIMAL(18,6)) AS target_high,
            CAST(p.target_low AS DECIMAL(18,6)) AS target_low,
            CAST(p.target_close AS DECIMAL(18,6)) AS target_close,
            CAST(p.target_volume AS SIGNED) AS target_volume,
            p.target_is_green,
            CAST(p.target_close_vs_open_pct AS DECIMAL(18,6)) AS target_close_vs_open_pct,
            CAST(p.target_high_vs_open_pct AS DECIMAL(18,6)) AS target_high_vs_open_pct,
            CAST(p.target_low_vs_open_pct AS DECIMAL(18,6)) AS target_low_vs_open_pct,
            CAST(p.target_range_pct AS DECIMAL(18,6)) AS target_range_pct,
            p.target_breaks_entry_high,
            p.target_breaks_entry_low,
            CAST(p.trade_risk_exit_price AS DECIMAL(18,6)) AS trade_risk_exit_price,
            CAST(p.trade_reward_exit_price AS DECIMAL(18,6)) AS trade_reward_exit_price,
            CAST(p.trade_enter_price AS DECIMAL(18,6)) AS trade_enter_price,
            CAST(COALESCE(p.target_close, p.trade_enter_price) AS DECIMAL(18,6)) AS trade_current_price,
            CAST(COALESCE(DATEDIFF(COALESCE(p.target_date, CURDATE()), p.entry_date), 0) AS DECIMAL(18,0)) AS trade_length,
            CAST(COALESCE(p.target_close_vs_open_pct, 0.0) AS DECIMAL(18,6)) AS trade_pnl,
            CAST(p.prop_result AS SIGNED) AS trade_result,
            COALESCE(p.target_date, p.reversal_detect_date, p.d_confirm_date, p.d_date) AS trade_date,
            CAST(COALESCE(ABS(p.b_min_max - p.a_min_max) / NULLIF(ABS(p.a_min_max - p.x_min_max), 0) * 100.0, 0.0) AS DECIMAL(12,4)) AS trade_ab_price_retracement,
            CAST(COALESCE(ABS(p.c_min_max - p.b_min_max) / NULLIF(ABS(p.b_min_max - p.a_min_max), 0) * 100.0, 0.0) AS DECIMAL(12,4)) AS trade_bc_price_retracement,
            CAST(COALESCE(ABS(p.d_min_max - p.c_min_max) / NULLIF(ABS(p.c_min_max - p.b_min_max), 0) * 100.0, 0.0) AS DECIMAL(12,4)) AS trade_cd_bc_price_retracement,
            CAST((p.d_min_max - p.c_min_max) AS DECIMAL(18,6)) AS trade_cd_price_retracement,
            CAST(COALESCE(ABS(p.d_min_max - p.c_min_max) / NULLIF(ABS(p.a_min_max - p.x_min_max), 0) * 100.0, 0.0) AS DECIMAL(12,4)) AS trade_cd_xa_price_retracement,
            CAST(COALESCE(p.b_length / NULLIF(p.a_length, 0) * 100.0, 0.0) AS DOUBLE) AS trade_ab_bar_retracement,
            CAST(COALESCE(p.c_length / NULLIF(p.b_length, 0) * 100.0, 0.0) AS DECIMAL(18,6)) AS trade_bc_bar_retracement,
            CAST(COALESCE(p.d_length / NULLIF(p.c_length, 0) * 100.0, 0.0) AS DECIMAL(18,6)) AS trade_cd_bar_retracement,
            CAST(COALESCE(p.d_length / NULLIF(p.c_length, 0) * 100.0, 0.0) AS DOUBLE) AS trade_cd_bc_bar_retracement,
            CAST(COALESCE(p.d_length / NULLIF(p.x_length, 0) * 100.0, 0.0) AS DOUBLE) AS trade_cd_xa_bar_retracement,
            CAST(0.0 AS DECIMAL(18,6)) AS trade_snr,
            CAST(YEAR(COALESCE(p.target_date, p.reversal_detect_date, p.d_confirm_date, p.d_date)) AS SIGNED) AS trade_year,
            CAST(MONTH(COALESCE(p.target_date, p.reversal_detect_date, p.d_confirm_date, p.d_date)) AS SIGNED) AS trade_month,
            CAST(DAY(COALESCE(p.target_date, p.reversal_detect_date, p.d_confirm_date, p.d_date)) AS SIGNED) AS trade_day,
            COALESCE(NULLIF(p.reversal_type, ''), 'None') AS reversal_type,
            COALESCE(NULLIF(p.reversal_type, ''), 'None') = 'BullishKeyReversal' AS bullish_key_reversal,
            COALESCE(NULLIF(p.reversal_type, ''), 'None') = 'BearishKeyReversal' AS bearish_key_reversal,
            COALESCE(NULLIF(p.reversal_type, ''), 'None') = 'BullishEngulfing' AS bullish_engulfing,
            COALESCE(NULLIF(p.reversal_type, ''), 'None') = 'BearishEngulfing' AS bearish_engulfing,
            COALESCE(NULLIF(p.reversal_type, ''), 'None') = 'BullishOutsideReversal' AS bullish_outside_reversal,
            COALESCE(NULLIF(p.reversal_type, ''), 'None') = 'BearishOutsideReversal' AS bearish_outside_reversal,
            COALESCE(NULLIF(p.reversal_type, ''), 'None') = 'Hammer' AS hammer,
            COALESCE(NULLIF(p.reversal_type, ''), 'None') = 'ShootingStar' AS shooting_star,
            COALESCE(NULLIF(p.reversal_type, ''), 'None') = 'MorningStar' AS morning_star,
            COALESCE(NULLIF(p.reversal_type, ''), 'None') = 'EveningStar' AS evening_star,
            COALESCE(NULLIF(p.reversal_type, ''), 'None') = 'ThreeWhiteSoldiers' AS three_white_soldiers,
            COALESCE(NULLIF(p.reversal_type, ''), 'None') = 'ThreeBlackCrows' AS three_black_crows,
            p.market,
            CASE p.three_month_trend
                WHEN 'Bullish' THEN TRUE
                WHEN 'Bearish' THEN FALSE
                ELSE NULL
            END AS three_month,
            CASE p.six_month_trend
                WHEN 'Bullish' THEN TRUE
                WHEN 'Bearish' THEN FALSE
                ELSE NULL
            END AS six_month,
            CASE p.twelve_month_trend
                WHEN 'Bullish' THEN TRUE
                WHEN 'Bearish' THEN FALSE
                ELSE NULL
            END AS twelve_month,
            p.pattern_group_id,
            p.harmonic_type,
            {route_score_projection},
            CAST({route_time_accuracy_expr} AS DOUBLE) AS time_accuracy
        FROM shape p
        "#
    );

    let mut query = if let Some(pattern_id) = params
        .pattern_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        sqlx::query_as::<_, Pattern>(&sql).bind(pattern_id)
    } else if let Some(pattern_group_id) = params
        .pattern_group_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        sqlx::query_as::<_, Pattern>(&sql).bind(pattern_group_id)
    } else {
        return Ok(None);
    };

    if let Some(outcome_model) = requested_outcome_model {
        query = query.bind(outcome_model);
    }

    if !has_pattern_id && params.d_date.is_some() {
        let Some(d_date) = params.d_date else {
            unreachable!();
        };
        query = query.bind(d_date);
    }
    if !has_pattern_id && params.market.is_some() {
        let Some(market) = params.market.as_deref() else {
            unreachable!();
        };
        query = query.bind(market);
    }
    if !has_pattern_id && params.harmonic_type.is_some() {
        let Some(harmonic_type) = params.harmonic_type.as_deref() else {
            unreachable!();
        };
        query = query.bind(harmonic_type);
    }
    if !has_pattern_id && params.size_bucket.is_some() {
        let Some(size_bucket) = params.size_bucket.as_deref() else {
            unreachable!();
        };
        query = query.bind(size_bucket);
    }
    if !has_pattern_id && params.balance_bucket.is_some() {
        let Some(balance_bucket) = params.balance_bucket.as_deref() else {
            unreachable!();
        };
        query = query.bind(balance_bucket);
    }
    if should_apply_exact_geometry_filters {
        if let Some(trade_enter_price) = params.trade_enter_price {
            query = query.bind(trade_enter_price);
        }
        if let Some(trade_risk_exit_price) = params.trade_risk_exit_price {
            query = query.bind(trade_risk_exit_price);
        }
        if let Some(trade_reward_exit_price) = params.trade_reward_exit_price {
            query = query.bind(trade_reward_exit_price);
        }
        if let Some(x_length) = params.x_length {
            query = query.bind(x_length);
        }
        if let Some(a_length) = params.a_length {
            query = query.bind(a_length);
        }
        if let Some(b_length) = params.b_length {
            query = query.bind(b_length);
        }
        if let Some(c_length) = params.c_length {
            query = query.bind(c_length);
        }
    }

    query = query.bind(candle_window_limit).bind(candle_window_limit);

    match query.fetch_optional(pool).await {
        Ok(row) => Ok(row),
        Err(error) if is_missing_table_error(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

async fn fetch_setup_decision_response(
    pool: &MySqlPool,
    params: &SetupComparisonParams,
) -> HttpResponse {
    let Some(prop_strategy_id) = params
        .prop_strategy_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return HttpResponse::BadRequest().body("Setup comparison requires family key");
    };

    match is_rollup_cache_ready(pool, "prop_strategy_family_rollups").await {
        Ok(true) => {}
        Ok(false) => return HttpResponse::Ok().json(empty_setup_comparison_response(params)),
        Err(error) => {
            eprintln!("Setup comparison cache readiness error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    }

    let family = match fetch_prop_strategy_family_filter(pool, prop_strategy_id).await {
        Ok(Some(family)) => family,
        Ok(None) => return HttpResponse::NotFound().body("Prop strategy family not found"),
        Err(error) => {
            eprintln!("Prop strategy family lookup error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let Some((summary, yearly_performance)) =
        (match try_fetch_setup_comparison_from_prop_strategy_rollup(pool, params).await {
            Ok(result) => result,
            Err(error) => {
                eprintln!("Setup comparison family query error: {:?}", error);
                return HttpResponse::InternalServerError().finish();
            }
        })
    else {
        return HttpResponse::NotFound().body("Strategy family not found in cache");
    };

    let recent_examples = if params.include_examples.unwrap_or(true) {
        let example_x_strictness_expr = x_strictness_expr("p.x_bars_left", "p.x_length");
        let examples_sql = format!(
            r#"
                SELECT
                    p.symbol,
                    p.d_date,
                    CAST(p.prop_result AS SIGNED) AS trade_result,
                    CAST(
                        COALESCE(
                            CASE
                                WHEN p.market = 'Bearish' THEN -p.target_close_vs_open_pct
                                ELSE p.target_close_vs_open_pct
                            END,
                            0.0
                        ) AS DOUBLE
                    ) AS trade_pnl,
                    CAST(COALESCE(p.target_range_pct, 0.0) AS DOUBLE) AS trade_length,
                    CAST(p.trade_enter_price AS DOUBLE) AS trade_enter_price
                FROM pattern_outcomes_prop p
                WHERE p.target_date IS NOT NULL
                  AND p.outcome_model = ?
                  AND p.market = ?
                  AND p.harmonic_type = ?
                  AND p.bin = ?
                  AND COALESCE(NULLIF(p.reversal_type, ''), 'None') = ?
                  AND p.size_bucket = ?
                  AND p.time_bin = ?
                  AND {example_x_strictness_expr} = ?
                  AND p.three_month_trend = ?
                  AND p.six_month_trend = ?
                  AND p.twelve_month_trend = ?
                ORDER BY COALESCE(p.reversal_detect_date, p.d_confirm_date, p.d_date) DESC,
                         p.trade_enter_price ASC
                LIMIT 6
            "#,
            example_x_strictness_expr = example_x_strictness_expr,
        );

        match sqlx::query_as::<_, SetupComparisonExample>(&examples_sql)
            .bind(&family.outcome_model)
            .bind(&family.market)
            .bind(&family.harmonic_type)
            .bind(&family.bin)
            .bind(&family.reversal_type)
            .bind(&family.size_bucket)
            .bind(&family.time_bin)
            .bind(family.x_strictness.as_deref().unwrap_or("Loose"))
            .bind(&family.three_month_trend)
            .bind(&family.six_month_trend)
            .bind(&family.twelve_month_trend)
            .fetch_all(pool)
            .await
        {
            Ok(rows) => rows,
            Err(error) => {
                eprintln!("Prop setup comparison examples error: {:?}", error);
                return HttpResponse::InternalServerError().finish();
            }
        }
    } else {
        Vec::new()
    };

    HttpResponse::Ok().json(SetupComparisonResponse {
        prop_strategy_id: Some(prop_strategy_id.to_string()),
        family_key: Some(family.family_key),
        family_name: Some(family.family_name),
        family_level: Some(family.family_level),
        included_dimensions: Some(family.included_dimensions),
        outcome_model: Some(family.outcome_model),
        harmonic_type: Some(family.harmonic_type),
        market: Some(family.market),
        bin: Some(family.bin),
        reversal_type: Some(family.reversal_type),
        size_bucket: Some(family.size_bucket),
        time_bin: Some(family.time_bin),
        x_strictness: family.x_strictness,
        three_month_trend: Some(family.three_month_trend),
        six_month_trend: Some(family.six_month_trend),
        twelve_month_trend: Some(family.twelve_month_trend),
        summary,
        yearly_performance,
        recent_examples,
    })
}
#[route("/setup-comparison", method = "GET", method = "POST")]
async fn fetch_setup_comparison(
    pool: web::Data<MySqlPool>,
    params: web::Json<SetupComparisonParams>,
) -> impl Responder {
    fetch_setup_decision_response(pool.get_ref(), &params).await
}

#[route("/strategy-candidates", method = "GET", method = "POST")]
async fn fetch_strategy_candidates(
    pool: web::Data<MySqlPool>,
    params: web::Json<StrategyCandidateParams>,
) -> impl Responder {
    let prop_outcome_mode = normalize_prop_outcome_mode(params.prop_outcome_mode.as_deref());
    let min_closed_trades = params.min_closed_trades.unwrap_or(0).max(0);
    let limit = params.limit.unwrap_or(250).clamp(1, 1_000);
    let include_count = params.include_count.unwrap_or(false);

    match is_rollup_cache_ready(pool.get_ref(), "prop_strategy_family_rollups").await {
        Ok(true) => {}
        Ok(false) => {
            if include_count {
                return HttpResponse::Ok().json(StrategyCandidatesResponse {
                    strategies: Vec::new(),
                    total_count: 0,
                    has_more: false,
                });
            }

            return HttpResponse::Ok().json(Vec::<StrategyCandidateSummary>::new());
        }
        Err(error) => {
            eprintln!("Strategy candidates cache readiness error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    }

    let mut summary_filters = String::new();
    if params.min_expectancy.is_some() {
        summary_filters.push_str(" AND s.expectancy > ?");
    }
    if params.max_down_years.is_some() {
        summary_filters.push_str(" AND s.down_years <= ?");
    }
    if params.min_worst_year_expectancy.is_some() {
        summary_filters.push_str(" AND s.worst_year_expectancy > ?");
    }
    if params.min_score.is_some() {
        summary_filters.push_str(" AND s.score > ?");
    }
    append_prop_family_candidate_option_filters(&mut summary_filters, "s", &params);
    let has_cadence = table_exists(pool.get_ref(), "prop_strategy_family_weekly_cadence")
        .await
        .unwrap_or(false);
    let cadence_select = cadence_select_fields(has_cadence);
    let cadence_join = cadence_join_clause(has_cadence);
    let sort_column =
        prop_family_sort_column(params.sort_by.as_deref(), "s", has_cadence.then_some("c"));
    let sort_direction = prop_family_sort_direction(params.sort_direction.as_deref());
    let count_sql = format!(
        r#"
        SELECT COUNT(*)
        FROM prop_strategy_family_summary s
        WHERE s.closed_count >= ?
          AND s.outcome_model = ?
          {summary_filters}
        "#,
        summary_filters = summary_filters,
    );
    let sql = format!(
        r#"
        SELECT
            s.family_key AS prop_strategy_id,
            s.family_key,
            s.family_name,
            CAST(s.family_level AS SIGNED) AS family_level,
            s.included_dimensions,
            s.outcome_model,
            s.market,
            s.harmonic_type,
            s.bin,
            s.reversal_type,
            s.size_bucket,
            s.time_bin,
            s.x_strictness,
            s.three_month_trend,
            s.six_month_trend,
            s.twelve_month_trend,
            CAST(s.worst_year_expectancy AS DOUBLE) AS worst_year_expectancy,
            CAST(s.down_years AS SIGNED) AS down_years,
            CAST(s.total_count AS SIGNED) AS total_count,
            CAST(s.closed_count AS SIGNED) AS closed_count,
            CAST(s.open_count AS SIGNED) AS open_count,
            CAST(s.win_count AS SIGNED) AS win_count,
            CAST(s.loss_count AS SIGNED) AS loss_count,
            CAST(s.expectancy AS DOUBLE) AS expectancy,
            CAST(s.avg_return AS DOUBLE) AS avg_return,
            CAST(s.win_rate AS DOUBLE) AS win_rate,
            CAST(s.closed_rate AS DOUBLE) AS closed_rate,
            CAST(s.avg_win AS DOUBLE) AS avg_win,
            CAST(s.avg_loss AS DOUBLE) AS avg_loss,
            CAST(s.avg_target_range AS DOUBLE) AS avg_target_range,
            CAST(s.score AS DOUBLE) AS score,
            {cadence_select}
        FROM prop_strategy_family_summary s
        {cadence_join}
        WHERE s.closed_count >= ?
          AND s.outcome_model = ?
          {summary_filters}
        ORDER BY {sort_column} {sort_direction}, s.score DESC, s.expectancy DESC, s.closed_count DESC
        LIMIT ?
        "#,
        summary_filters = summary_filters,
        cadence_select = cadence_select,
        cadence_join = cadence_join,
        sort_column = sort_column,
        sort_direction = sort_direction,
    );

    let total_count = if include_count {
        let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql)
            .bind(min_closed_trades)
            .bind(prop_outcome_model_label(prop_outcome_mode));

        if let Some(min_expectancy) = params.min_expectancy {
            count_query = count_query.bind(min_expectancy);
        }
        if let Some(max_down_years) = params.max_down_years {
            count_query = count_query.bind(max_down_years);
        }
        if let Some(min_worst_year_expectancy) = params.min_worst_year_expectancy {
            count_query = count_query.bind(min_worst_year_expectancy);
        }
        if let Some(min_score) = params.min_score {
            count_query = count_query.bind(min_score);
        }
        for values in [
            params.strategy_markets.as_ref(),
            params.strategy_harmonic_types.as_ref(),
            params.strategy_bins.as_ref(),
            params.strategy_reversal_types.as_ref(),
            params.strategy_size_buckets.as_ref(),
            params.strategy_time_bins.as_ref(),
            params.strategy_x_strictness.as_ref(),
            params.strategy_three_month_trends.as_ref(),
            params.strategy_six_month_trends.as_ref(),
            params.strategy_twelve_month_trends.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            for value in values {
                count_query = count_query.bind(value);
            }
        }

        match count_query.fetch_one(pool.get_ref()).await {
            Ok(count) => count,
            Err(error) if is_missing_table_error(&error) => 0,
            Err(error) => {
                eprintln!("Strategy candidate count DB error: {:?}", error);
                return HttpResponse::InternalServerError().finish();
            }
        }
    } else {
        -1
    };

    let mut query = sqlx::query_as::<_, StrategyCandidateSummary>(&sql)
        .bind(min_closed_trades)
        .bind(prop_outcome_model_label(prop_outcome_mode));

    if let Some(min_expectancy) = params.min_expectancy {
        query = query.bind(min_expectancy);
    }
    if let Some(max_down_years) = params.max_down_years {
        query = query.bind(max_down_years);
    }
    if let Some(min_worst_year_expectancy) = params.min_worst_year_expectancy {
        query = query.bind(min_worst_year_expectancy);
    }
    if let Some(min_score) = params.min_score {
        query = query.bind(min_score);
    }
    for values in [
        params.strategy_markets.as_ref(),
        params.strategy_harmonic_types.as_ref(),
        params.strategy_bins.as_ref(),
        params.strategy_reversal_types.as_ref(),
        params.strategy_size_buckets.as_ref(),
        params.strategy_time_bins.as_ref(),
        params.strategy_x_strictness.as_ref(),
        params.strategy_three_month_trends.as_ref(),
        params.strategy_six_month_trends.as_ref(),
        params.strategy_twelve_month_trends.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        for value in values {
            query = query.bind(value);
        }
    }

    return match query.bind(limit).fetch_all(pool.get_ref()).await {
        Ok(rows) => {
            if include_count {
                let row_count = rows.len() as i64;
                HttpResponse::Ok().json(StrategyCandidatesResponse {
                    strategies: rows,
                    total_count,
                    has_more: row_count < total_count,
                })
            } else {
                HttpResponse::Ok().json(rows)
            }
        }
        Err(error) if is_missing_table_error(&error) => {
            if include_count {
                HttpResponse::Ok().json(StrategyCandidatesResponse {
                    strategies: Vec::new(),
                    total_count: 0,
                    has_more: false,
                })
            } else {
                HttpResponse::Ok().json(Vec::<StrategyCandidateSummary>::new())
            }
        }
        Err(error) => {
            eprintln!("Strategy candidate DB error: {:?}", error);
            HttpResponse::InternalServerError().finish()
        }
    };
}

#[route("/strategy-contract-weeks", method = "GET", method = "POST")]
async fn fetch_strategy_contract_weeks(
    pool: web::Data<MySqlPool>,
    params: web::Json<StrategyContractWeekParams>,
) -> impl Responder {
    let Some(family_key) = params.prop_strategy_id.as_deref() else {
        return HttpResponse::Ok().json(Vec::<StrategyContractWeekSummary>::new());
    };
    let limit = params.limit.unwrap_or(2_000).clamp(1, 10_000);

    match table_exists(pool.get_ref(), "prop_strategy_contract_week_summary").await {
        Ok(true) => {}
        Ok(false) => return HttpResponse::Ok().json(Vec::<StrategyContractWeekSummary>::new()),
        Err(error) => {
            eprintln!("Contract week table check error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    }

    let rows = sqlx::query_as::<_, StrategyContractWeekSummary>(
        r#"
        SELECT
            family_key,
            symbol,
            CAST(contract_week_index AS SIGNED) AS contract_week_index,
            CAST(total_count AS SIGNED) AS total_count,
            CAST(closed_count AS SIGNED) AS closed_count,
            CAST(open_count AS SIGNED) AS open_count,
            CAST(win_count AS SIGNED) AS win_count,
            CAST(loss_count AS SIGNED) AS loss_count,
            CAST(expectancy AS DOUBLE) AS expectancy,
            CAST(avg_return AS DOUBLE) AS avg_return,
            CAST(win_rate AS DOUBLE) AS win_rate
        FROM prop_strategy_contract_week_summary
        WHERE family_key = ?
        ORDER BY symbol ASC, contract_week_index ASC
        LIMIT ?
        "#,
    )
    .bind(family_key)
    .bind(limit)
    .fetch_all(pool.get_ref())
    .await;

    match rows {
        Ok(rows) => HttpResponse::Ok().json(rows),
        Err(error) if is_missing_table_error(&error) => {
            HttpResponse::Ok().json(Vec::<StrategyContractWeekSummary>::new())
        }
        Err(error) => {
            eprintln!("Strategy contract week DB error: {:?}", error);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[route("/candles", method = "GET", method = "POST")]
async fn fetch_candles(
    pool: web::Data<MySqlPool>,
    params: web::Json<CandleParams>,
) -> impl Responder {
    // println!("🔔 Handler called: fetch_candles");
    // println!("{:?}", params);

    let mut daily_date_filters = String::new();
    let mut futures_date_filters = String::new();
    if params.start_date.is_some() {
        daily_date_filters.push_str(" AND date >= ?");
        futures_date_filters.push_str(" AND ts_utc >= ?");
    }
    if params.end_date.is_some() {
        daily_date_filters.push_str(" AND date <= ?");
        futures_date_filters.push_str(" AND ts_utc <= ?");
    }

    let sql = format!(
        r#"
        SELECT symbol, date, open, high, low, close, volume, three_month, six_month, twelve_month
        FROM (
            SELECT
                symbol,
                CAST(date AS DATETIME) AS date,
                open,
                high,
                low,
                close,
                volume,
                three_month,
                six_month,
                twelve_month
            FROM candles
            WHERE symbol = ?
              {daily_date_filters}
            UNION ALL
            SELECT
                symbol,
                ts_utc AS date,
                open,
                high,
                low,
                close,
                volume,
                CAST(NULL AS SIGNED) AS three_month,
                CAST(NULL AS SIGNED) AS six_month,
                CAST(NULL AS SIGNED) AS twelve_month
            FROM futures_contract_1m_candles
            WHERE symbol = ?
              {futures_date_filters}
        ) all_candles
        ORDER BY date
        "#,
        daily_date_filters = daily_date_filters,
        futures_date_filters = futures_date_filters
    );

    let mut query = sqlx::query_as::<_, Candle>(&sql).bind(params.symbol.clone());
    if let Some(start_date) = params.start_date.as_deref() {
        query = query.bind(start_date);
    }
    if let Some(end_date) = params.end_date.as_deref() {
        query = query.bind(end_date);
    }
    query = query.bind(params.symbol.clone());
    if let Some(start_date) = params.start_date.as_deref() {
        query = query.bind(start_date);
    }
    if let Some(end_date) = params.end_date.as_deref() {
        query = query.bind(end_date);
    }

    let candles: Vec<Candle> = match query.fetch_all(pool.get_ref()).await {
        Ok(c) => {
            println!("✅ Successfully fetched {} candles", c.len());
            c
        }
        Err(e) => {
            eprintln!("❌ Failed to fetch candles: {}", e);
            return HttpResponse::InternalServerError().body("Failed to fetch candles");
        }
    };

    HttpResponse::Ok().json(candles)
}
