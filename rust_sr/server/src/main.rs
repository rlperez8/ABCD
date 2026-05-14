use actix_cors::Cors;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use chrono::{Datelike, NaiveDate, NaiveDateTime};
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
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

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
const ENGINE_STORAGE_TABLES: [&str; 3] = [
    "pattern_setups",
    "pattern_outcomes_prop",
    "pattern_forward_observations",
];
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
struct PatternDiscoveryParams {
    pub prop_strategy_id: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, serde::Deserialize)]
struct PatternDiscoverySaveLogicParams {
    pub prop_strategy_id: Option<String>,
    pub title: Option<String>,
    pub logic_text: Option<String>,
    pub rule_json: Option<String>,
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
    #[serde(alias = "confirmText")]
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
    engine_progress: Option<AdminEngineProgressSnapshot>,
    cache_states: Vec<AdminCacheStateSnapshot>,
    entry_exit_tests: Vec<AdminEntryExitTestSnapshot>,
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
struct AdminEngineProgressSnapshot {
    run_id: String,
    total_symbols: i64,
    queued_symbols: i64,
    completed_symbols: i64,
    percent_complete: f64,
    elapsed_ms: i64,
    estimated_total_ms: Option<i64>,
    estimated_remaining_ms: Option<i64>,
    latest_symbol: Option<String>,
    latest_phase: String,
    updated_at: Option<String>,
}

#[derive(Serialize)]
struct AdminCacheStateSnapshot {
    cache_name: String,
    is_ready: bool,
    updated_at: Option<String>,
    note: Option<String>,
}

#[derive(Serialize)]
struct AdminEntryExitTestSnapshot {
    test_id: String,
    test_name: String,
    entry_mode: String,
    stop_mode: String,
    target_r: f64,
    max_hold_multiple: i64,
    is_enabled: bool,
    notes: Option<String>,
    updated_at: Option<String>,
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
    pub use_candidate_logic: Option<bool>,
}

#[derive(Debug, serde::Deserialize)]
struct Phase1SimulatorReplayParams {
    pub family_key: Option<String>,
    pub run_id: Option<String>,
    pub route_id: Option<String>,
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

#[derive(Debug, Clone, serde::Deserialize)]
struct Phase1PatternRouteReplayParams {
    pub family_key: Option<String>,
    pub run_id: Option<String>,
    pub route_id: Option<String>,
    pub pattern_id: Option<String>,
    pub pattern_group_id: Option<String>,
    pub d_date: Option<NaiveDateTime>,
    pub contracts: Option<i64>,
}

#[derive(Debug, Clone)]
struct CandidateLogicReplayFilter {
    title: String,
    value: String,
    label: String,
}

#[derive(Clone, sqlx::FromRow, serde::Serialize)]
struct SimulatorReplaySourceTrade {
    trade_id: Option<i64>,
    trade_uid: Option<String>,
    trade_direction: Option<String>,
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
    exit_price: Option<f64>,
    result_r: Option<f64>,
    risk_points: Option<f64>,
    trade_lowest_price: Option<f64>,
    trade_highest_price: Option<f64>,
    trade_adverse_price: Option<f64>,
    trade_favorable_price: Option<f64>,
    max_adverse_points: Option<f64>,
    max_favorable_points: Option<f64>,
    trade_result: i64,
    exit_reason: Option<String>,
}

#[derive(serde::Serialize)]
struct SimulatorReplayTradeEvent {
    trade_id: Option<i64>,
    trade_uid: Option<String>,
    trade_direction: Option<String>,
    test_index: i64,
    trade_index: i64,
    symbol: String,
    pattern_id: Option<String>,
    pattern_group_id: String,
    entry_date: NaiveDateTime,
    target_date: Option<NaiveDateTime>,
    trade_result: i64,
    exit_reason: Option<String>,
    trade_enter_price: f64,
    trade_risk_exit_price: f64,
    trade_reward_exit_price: f64,
    exit_price: Option<f64>,
    result_r: Option<f64>,
    risk_points: Option<f64>,
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
    candidate_logic_applied: bool,
    candidate_logic_filters: Vec<String>,
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

#[derive(Clone, sqlx::FromRow, serde::Serialize)]
struct PatternFamilySummary {
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
    x_strictness: String,
    three_month_trend: String,
    six_month_trend: String,
    twelve_month_trend: String,
    setup_count: i64,
    symbol_count: i64,
    first_d_date: Option<NaiveDate>,
    last_d_date: Option<NaiveDate>,
}

#[derive(Deserialize, Debug)]
struct PatternFamilyParams {
    limit: Option<i64>,
    min_setup_count: Option<i64>,
    year: Option<i64>,
    source_scope: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Phase1ResultsParams {
    family_key: Option<String>,
    source_scope: Option<String>,
    year: Option<i64>,
    limit: Option<i64>,
}

#[derive(Deserialize, Debug)]
struct Phase1FamilyPatternsParams {
    family_key: Option<String>,
    source_scope: Option<String>,
    year: Option<i64>,
    limit: Option<i64>,
    offset: Option<i64>,
    include_count: Option<bool>,
}

#[derive(Deserialize, Debug)]
struct Phase1LeaderboardParams {
    source_scope: Option<String>,
    year: Option<i64>,
    limit: Option<i64>,
    min_trade_count: Option<i64>,
    min_setup_count: Option<i64>,
    best_per_family: Option<bool>,
    route_id: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Phase1SupplyParams {
    source_scope: Option<String>,
    year: Option<i64>,
    limit: Option<i64>,
}

#[derive(Deserialize, Debug)]
struct Phase1YearlyParams {
    family_key: Option<String>,
    run_id: Option<String>,
    route_id: Option<String>,
    cache_only: Option<bool>,
}

#[derive(sqlx::FromRow, Serialize)]
struct Phase1StrategyResult {
    run_id: String,
    family_key: String,
    source_scope: String,
    period_year: i64,
    route_id: String,
    route_label: String,
    result_rank: i64,
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
    created_at: Option<NaiveDateTime>,
}

#[derive(sqlx::FromRow, Serialize)]
struct Phase1LeaderboardResult {
    run_id: String,
    family_key: String,
    source_scope: String,
    period_year: i64,
    route_id: String,
    route_label: String,
    result_rank: i64,
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
    created_at: Option<NaiveDateTime>,
    harmonic_type: String,
    bin: String,
    size_bucket: String,
    time_bin: String,
    x_strictness: String,
}

#[derive(sqlx::FromRow, Serialize)]
struct Phase1SymbolSupplyRow {
    symbol: String,
    setup_count: i64,
    pattern_count: i64,
    family_count: i64,
    contract_count: i64,
    first_d_date: Option<NaiveDate>,
    last_d_date: Option<NaiveDate>,
}

#[derive(sqlx::FromRow, Serialize)]
struct Phase1FamilySupplyRow {
    family_key: String,
    harmonic_type: String,
    bin: String,
    size_bucket: String,
    time_bin: String,
    x_strictness: String,
    setup_count: i64,
    pattern_count: i64,
    symbol_count: i64,
    first_d_date: Option<NaiveDate>,
    last_d_date: Option<NaiveDate>,
}

#[derive(Serialize)]
struct Phase1SupplyResponse {
    source_scope: String,
    period_year: i64,
    symbols: Vec<Phase1SymbolSupplyRow>,
    families: Vec<Phase1FamilySupplyRow>,
}

#[derive(sqlx::FromRow)]
struct Phase1YearlyRouteContext {
    run_id: String,
    family_key: String,
    source_scope: String,
    period_year: i64,
    route_id: String,
    route_label: String,
    result_rank: i64,
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
    created_at: Option<NaiveDateTime>,
    max_forward_bars: i64,
    harmonic_type: String,
    bin: String,
    size_bucket: String,
    time_bin: String,
    x_strictness: String,
}

#[derive(Clone, sqlx::FromRow)]
struct Phase1ReplaySetup {
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
struct Phase1ReplayCandle {
    candle_date: NaiveDateTime,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
}

struct Phase1ReplayTradeDetail {
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
    trade_result: i64,
    exit_reason: String,
}

struct Phase1EntryDecision {
    index: usize,
    price: f64,
    direction: f64,
}

#[derive(sqlx::FromRow, Serialize)]
struct Phase1YearlyBreakdownRow {
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

#[derive(Serialize)]
struct Phase1YearlyBreakdownResponse {
    route: Phase1LeaderboardResult,
    years: Vec<Phase1YearlyBreakdownRow>,
    cached: bool,
}

#[derive(Default)]
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum PatternFamilySourceScope {
    All,
    Futures,
    Daily,
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

#[derive(Clone, serde::Serialize)]
struct PatternDiscoveryFamily {
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

impl From<&PropStrategyFamilyFilter> for PatternDiscoveryFamily {
    fn from(family: &PropStrategyFamilyFilter) -> Self {
        Self {
            family_key: family.family_key.clone(),
            family_name: family.family_name.clone(),
            family_level: family.family_level,
            included_dimensions: family.included_dimensions.clone(),
            outcome_model: family.outcome_model.clone(),
            market: family.market.clone(),
            harmonic_type: family.harmonic_type.clone(),
            bin: family.bin.clone(),
            reversal_type: family.reversal_type.clone(),
            size_bucket: family.size_bucket.clone(),
            time_bin: family.time_bin.clone(),
            x_strictness: family.x_strictness.clone(),
            three_month_trend: family.three_month_trend.clone(),
            six_month_trend: family.six_month_trend.clone(),
            twelve_month_trend: family.twelve_month_trend.clone(),
        }
    }
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct PatternDiscoverySummary {
    observations: i64,
    complete_windows: i64,
    avg_bars_observed: f64,
    avg_mfe_r: f64,
    avg_mae_r: f64,
    avg_end_return_r: f64,
    avg_return_1x_r: f64,
    avg_return_2x_r: f64,
    avg_return_3x_r: f64,
    avg_return_5x_r: f64,
    hit_pos_0_5r_rate: f64,
    hit_pos_1_0r_rate: f64,
    hit_pos_1_5r_rate: f64,
    hit_pos_2_0r_rate: f64,
    hit_neg_0_5r_rate: f64,
    hit_neg_1_0r_rate: f64,
    pos_1r_before_neg_1r_rate: f64,
    avg_pos_1r_bar: f64,
    avg_neg_1r_bar: f64,
}

fn empty_pattern_discovery_summary() -> PatternDiscoverySummary {
    PatternDiscoverySummary {
        observations: 0,
        complete_windows: 0,
        avg_bars_observed: 0.0,
        avg_mfe_r: 0.0,
        avg_mae_r: 0.0,
        avg_end_return_r: 0.0,
        avg_return_1x_r: 0.0,
        avg_return_2x_r: 0.0,
        avg_return_3x_r: 0.0,
        avg_return_5x_r: 0.0,
        hit_pos_0_5r_rate: 0.0,
        hit_pos_1_0r_rate: 0.0,
        hit_pos_1_5r_rate: 0.0,
        hit_pos_2_0r_rate: 0.0,
        hit_neg_0_5r_rate: 0.0,
        hit_neg_1_0r_rate: 0.0,
        pos_1r_before_neg_1r_rate: 0.0,
        avg_pos_1r_bar: 0.0,
        avg_neg_1r_bar: 0.0,
    }
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct PatternDiscoveryBreakdownRow {
    value: String,
    observations: i64,
    avg_mfe_r: f64,
    avg_mae_r: f64,
    avg_return_3x_r: f64,
    avg_return_5x_r: f64,
    hit_pos_1_0r_rate: f64,
    hit_neg_1_0r_rate: f64,
    pos_1r_before_neg_1r_rate: f64,
}

#[derive(serde::Serialize)]
struct PatternDiscoveryBreakdown {
    title: String,
    rows: Vec<PatternDiscoveryBreakdownRow>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct PatternDiscoveryObservationExample {
    observation_id: String,
    symbol: String,
    entry_date: NaiveDateTime,
    observation_end_date: NaiveDateTime,
    reference_price: f64,
    mfe_r: Option<f64>,
    mae_r: Option<f64>,
    end_close_return_r: Option<f64>,
    close_return_3x_r: Option<f64>,
    close_return_5x_r: Option<f64>,
    hit_pos_1_0r_bar: Option<i64>,
    hit_neg_1_0r_bar: Option<i64>,
    hit_pos_1r_before_neg_1r: Option<bool>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct PatternDiscoverySavedLogic {
    id: i64,
    family_key: String,
    title: String,
    logic_text: String,
    rule_json: Option<String>,
    status: String,
    created_at: Option<NaiveDateTime>,
    updated_at: Option<NaiveDateTime>,
}

#[derive(serde::Serialize)]
struct PatternDiscoveryResponse {
    family: PatternDiscoveryFamily,
    summary: PatternDiscoverySummary,
    breakdowns: Vec<PatternDiscoveryBreakdown>,
    examples: Vec<PatternDiscoveryObservationExample>,
    saved_logic: Vec<PatternDiscoverySavedLogic>,
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

fn normalize_pattern_family_source_scope(value: Option<&str>) -> PatternFamilySourceScope {
    match value.map(|item| item.trim().to_ascii_lowercase()) {
        Some(scope) if scope == "futures" || scope == "futures_1m" => {
            PatternFamilySourceScope::Futures
        }
        Some(scope) if scope == "daily" || scope == "candles_daily" => {
            PatternFamilySourceScope::Daily
        }
        _ => PatternFamilySourceScope::All,
    }
}

fn pattern_family_source_scope_label(scope: PatternFamilySourceScope) -> &'static str {
    match scope {
        PatternFamilySourceScope::All => "all",
        PatternFamilySourceScope::Futures => "futures",
        PatternFamilySourceScope::Daily => "daily",
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
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from));
    let mut candidates = vec![
        current_dir.join("rust_sr").join("abcd"),
        current_dir.join("..").join("abcd"),
        current_dir
            .join("..")
            .join("..")
            .join("rust_sr")
            .join("abcd"),
        current_dir.clone(),
    ];

    if let Some(exe_dir) = exe_dir {
        candidates.extend([
            exe_dir.join("..").join("..").join("..").join("abcd"),
            exe_dir
                .join("..")
                .join("..")
                .join("..")
                .join("rust_sr")
                .join("abcd"),
            exe_dir.join("..").join(".."),
        ]);
    }

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
    cargo_admin_command_with_args(
        abcd_dir,
        bin_name,
        action_label,
        Vec::new(),
        envs,
        env_removes,
    )
}

fn cargo_admin_command_with_args(
    abcd_dir: PathBuf,
    bin_name: &str,
    action_label: &str,
    bin_args: Vec<String>,
    envs: Vec<(String, String)>,
    env_removes: Vec<String>,
) -> AdminCommandSpec {
    let mut args = vec!["run".to_string(), "--bin".to_string(), bin_name.to_string()];
    if !bin_args.is_empty() {
        args.push("--".to_string());
        args.extend(bin_args);
    }
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
                return Err(
                    "Type CLEAR ENGINE before clearing generated engine tables.".to_string()
                );
            }

            Ok(cargo_admin_command(
                abcd_dir,
                "clear_engine_tables",
                "Clear engine",
                Vec::new(),
                clean_env,
            ))
        }
        "clear_phase1_routes" => {
            if params.confirm_text.as_deref() != Some("CLEAR PHASE1 ROUTES") {
                return Err(
                    "Type CLEAR PHASE1 ROUTES before clearing Phase 1 route tables.".to_string(),
                );
            }

            Ok(cargo_admin_command(
                abcd_dir,
                "clear_phase1_routes",
                "Clear Phase 1 routes",
                Vec::new(),
                clean_env,
            ))
        }
        "run_entry_exit_tests" => Ok(cargo_admin_command_with_args(
            abcd_dir,
            "run_phase1_optimizer",
            "Run Entry / Exit tests",
            vec![
                "--all-families".to_string(),
                "--source".to_string(),
                "futures".to_string(),
                "--year".to_string(),
                "0".to_string(),
                "--max-setups".to_string(),
                "1000".to_string(),
            ],
            Vec::new(),
            clean_env,
        )),
        "run_engine_scan" => {
            let timeframe = normalize_admin_timeframe(params.source_timeframe.as_deref())?;
            let root_symbol = normalize_admin_symbol(params.root_symbol.as_deref());
            let contract_symbol = normalize_admin_symbol(params.contract_symbol.as_deref());
            let scan_concurrency = params.scan_concurrency.unwrap_or(1).clamp(1, 16);
            let mut envs = vec![
                (
                    "ABCD_CANDLE_SOURCE".to_string(),
                    "futures_contracts".to_string(),
                ),
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

            let mut spec =
                cargo_admin_command(abcd_dir, "abcd", "Run engine scan", envs, env_removes);
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

async fn update_admin_operation_output_tail(
    pool: &MySqlPool,
    run_id: i64,
    output_tail: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE admin_operation_runs
        SET output_tail = ?
        WHERE id = ?
        "#,
    )
    .bind(output_tail)
    .bind(run_id)
    .execute(pool)
    .await?;

    Ok(())
}

async fn collect_stream_tail<R>(
    stream: R,
    label: &'static str,
    pool: MySqlPool,
    run_id: i64,
    shared_tail: Arc<Mutex<String>>,
) where
    R: AsyncRead + Unpin,
{
    let mut reader = BufReader::new(stream).lines();
    let mut last_flush = Instant::now();

    loop {
        match reader.next_line().await {
            Ok(Some(line)) => {
                let mut flush_tail = None;
                {
                    let mut tail = shared_tail.lock().await;
                    append_output_tail(&mut tail, &format!("[{label}] {line}\n"));
                    if last_flush.elapsed().as_millis() >= 750 {
                        flush_tail = Some(tail.clone());
                        last_flush = Instant::now();
                    }
                }
                if let Some(tail) = flush_tail {
                    let _ = update_admin_operation_output_tail(&pool, run_id, &tail).await;
                }
            }
            Ok(None) => break,
            Err(error) => {
                let mut tail = shared_tail.lock().await;
                append_output_tail(&mut tail, &format!("[{label}] stream error: {error}\n"));
                break;
            }
        }
    }

    let tail = shared_tail.lock().await.clone();
    let _ = update_admin_operation_output_tail(&pool, run_id, &tail).await;
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

    let shared_tail = Arc::new(Mutex::new(String::new()));
    let stdout_task = child.stdout.take().map(|stream| {
        tokio::spawn(collect_stream_tail(
            stream,
            "out",
            pool.clone(),
            run_id,
            shared_tail.clone(),
        ))
    });
    let stderr_task = child.stderr.take().map(|stream| {
        tokio::spawn(collect_stream_tail(
            stream,
            "err",
            pool.clone(),
            run_id,
            shared_tail.clone(),
        ))
    });

    let status_result = child.wait().await;

    if let Some(task) = stdout_task {
        let _ = task.await;
    }
    if let Some(task) = stderr_task {
        let _ = task.await;
    }
    let output_tail = shared_tail.lock().await.clone();

    let duration_ms = started_at.elapsed().as_millis() as i64;
    let (status, exit_code, error_message) = match status_result {
        Ok(exit_status) if exit_status.success() => ("completed", exit_status.code(), None),
        Ok(exit_status) => (
            "failed",
            exit_status.code(),
            Some(format!("Command exited with status {exit_status}")),
        ),
        Err(error) => (
            "failed",
            None,
            Some(format!("Command wait failed: {error}")),
        ),
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

async fn fetch_admin_engine_progress(
    pool: &MySqlPool,
) -> Result<Option<AdminEngineProgressSnapshot>, sqlx::Error> {
    if !table_exists(pool, "engine_phase_timings").await? {
        return Ok(None);
    }

    let Some(run_id) = sqlx::query_scalar::<_, Option<String>>(
        r#"
        SELECT run_id
        FROM engine_phase_timings
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .fetch_one(pool)
    .await?
    else {
        return Ok(None);
    };

    let aggregate = sqlx::query(
        r#"
        SELECT
            COALESCE(MAX(CASE WHEN phase = 'select_symbols' THEN row_count END), 0) AS total_symbols,
            COALESCE(SUM(CASE
                WHEN phase IN ('queue_initial_symbols', 'queue_next_symbol')
                THEN COALESCE(row_count, 0)
                ELSE 0
            END), 0) AS queued_symbols,
            COALESCE(SUM(CASE WHEN phase = 'await_symbol_result' THEN 1 ELSE 0 END), 0) AS completed_symbols,
            CAST(COALESCE(AVG(CASE
                WHEN phase = 'await_symbol_result' AND duration_ms > 0
                THEN duration_ms
                ELSE NULL
            END), 0) AS DOUBLE) AS avg_symbol_duration_ms,
            TIMESTAMPDIFF(MICROSECOND, MIN(created_at), NOW(6)) / 1000 AS elapsed_ms
        FROM engine_phase_timings
        WHERE run_id = ?
        "#,
    )
    .bind(&run_id)
    .fetch_one(pool)
    .await?;

    let latest = sqlx::query(
        r#"
        SELECT
            symbol,
            phase,
            DATE_FORMAT(created_at, '%Y-%m-%d %H:%i:%s') AS updated_at
        FROM engine_phase_timings
        WHERE run_id = ?
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .bind(&run_id)
    .fetch_one(pool)
    .await?;

    let total_symbols = read_i64_or_zero(&aggregate, "total_symbols");
    let queued_symbols = read_i64_or_zero(&aggregate, "queued_symbols");
    let completed_symbols = read_i64_or_zero(&aggregate, "completed_symbols");
    let avg_symbol_duration_ms = aggregate
        .try_get::<f64, _>("avg_symbol_duration_ms")
        .unwrap_or(0.0);
    let elapsed_ms = read_i64_or_zero(&aggregate, "elapsed_ms").max(0);
    let percent_complete = if total_symbols > 0 {
        ((completed_symbols as f64 / total_symbols as f64) * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let remaining_symbols = total_symbols.saturating_sub(completed_symbols);
    let estimated_remaining_ms = if avg_symbol_duration_ms > 0.0 && remaining_symbols > 0 {
        Some((avg_symbol_duration_ms * remaining_symbols as f64).round() as i64)
    } else {
        None
    };
    let estimated_total_ms = estimated_remaining_ms.map(|remaining_ms| elapsed_ms + remaining_ms);

    Ok(Some(AdminEngineProgressSnapshot {
        run_id,
        total_symbols,
        queued_symbols,
        completed_symbols,
        percent_complete,
        elapsed_ms,
        estimated_total_ms,
        estimated_remaining_ms,
        latest_symbol: latest.try_get("symbol").ok(),
        latest_phase: latest.try_get("phase").unwrap_or_default(),
        updated_at: latest.try_get("updated_at").ok(),
    }))
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

async fn fetch_admin_entry_exit_tests(
    pool: &MySqlPool,
) -> Result<Vec<AdminEntryExitTestSnapshot>, sqlx::Error> {
    ensure_admin_entry_exit_tests_table(pool).await?;
    if !table_exists(pool, "entry_exit_tests").await? {
        return Ok(Vec::new());
    }

    let rows = sqlx::query(
        r#"
        SELECT
            test_id,
            test_name,
            entry_mode,
            stop_mode,
            target_r,
            CAST(max_hold_multiple AS SIGNED) AS max_hold_multiple,
            is_enabled,
            notes,
            updated_at
        FROM entry_exit_tests
        ORDER BY is_enabled DESC, updated_at DESC, test_name ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| AdminEntryExitTestSnapshot {
            test_id: row.try_get("test_id").unwrap_or_default(),
            test_name: row.try_get("test_name").unwrap_or_default(),
            entry_mode: row.try_get("entry_mode").unwrap_or_default(),
            stop_mode: row.try_get("stop_mode").unwrap_or_default(),
            target_r: row.try_get("target_r").unwrap_or_default(),
            max_hold_multiple: read_i64_or_zero(&row, "max_hold_multiple"),
            is_enabled: row
                .try_get::<bool, _>("is_enabled")
                .unwrap_or_else(|_| read_i64_or_zero(&row, "is_enabled") != 0),
            notes: row.try_get("notes").ok(),
            updated_at: row.try_get("updated_at").ok(),
        })
        .collect())
}

async fn ensure_admin_entry_exit_tests_table(pool: &MySqlPool) -> Result<(), sqlx::Error> {
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

async fn build_admin_status_response(pool: &MySqlPool) -> Result<AdminStatusResponse, sqlx::Error> {
    ensure_admin_operation_table(pool).await?;

    Ok(AdminStatusResponse {
        abcd_dir: resolve_abcd_dir()
            .ok()
            .map(|path| path.display().to_string()),
        operations: fetch_admin_operations(pool).await?,
        table_snapshots: fetch_admin_table_snapshots(pool).await?,
        engine_phases: fetch_admin_engine_phases(pool).await?,
        engine_progress: fetch_admin_engine_progress(pool).await?,
        cache_states: fetch_admin_cache_states(pool).await?,
        entry_exit_tests: fetch_admin_entry_exit_tests(pool).await?,
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
    match error {
        sqlx::Error::Database(db_error) => {
            matches!(db_error.code().as_deref(), Some("1146") | Some("42S02"))
                || db_error.message().contains("doesn't exist")
                || db_error.message().contains("does not exist")
                || db_error.message().contains("Unknown table")
        }
        _ => false,
    }
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

async fn ensure_pattern_discovery_logic_table(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS pattern_discovery_logic (
            id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
            family_key VARCHAR(64) NOT NULL,
            title VARCHAR(128) NOT NULL,
            logic_text TEXT NOT NULL,
            rule_json LONGTEXT NULL,
            status VARCHAR(32) NOT NULL DEFAULT 'candidate',
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            INDEX idx_pattern_discovery_logic_family (family_key, created_at)
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn fetch_pattern_discovery_saved_logic(
    pool: &MySqlPool,
    family_key: &str,
) -> Result<Vec<PatternDiscoverySavedLogic>, sqlx::Error> {
    ensure_pattern_discovery_logic_table(pool).await?;

    sqlx::query_as::<_, PatternDiscoverySavedLogic>(
        r#"
        SELECT
            CAST(id AS SIGNED) AS id,
            family_key,
            title,
            logic_text,
            rule_json,
            status,
            created_at,
            updated_at
        FROM pattern_discovery_logic
        WHERE family_key = ?
        ORDER BY created_at DESC, id DESC
        LIMIT 20
        "#,
    )
    .bind(family_key)
    .fetch_all(pool)
    .await
}

fn parse_candidate_logic_replay_filters(logic_text: &str) -> Vec<CandidateLogicReplayFilter> {
    logic_text
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("avoid_when ")?;
            let (title, value_with_note) = rest.split_once(" = ")?;
            let value = value_with_note
                .split_once(" (")
                .map(|(value, _)| value)
                .unwrap_or(value_with_note)
                .trim();
            let title = title.trim();

            if value.is_empty() || candidate_logic_filter_condition(title).is_none() {
                return None;
            }

            Some(CandidateLogicReplayFilter {
                title: title.to_string(),
                value: value.to_string(),
                label: format!("Avoid {} = {}", title, value),
            })
        })
        .collect()
}

fn candidate_logic_filter_condition(title: &str) -> Option<&'static str> {
    match title {
        "Roots" => Some("COALESCE(NULLIF(p.root_symbol, ''), SUBSTRING(p.symbol, 1, 3)) <> ?"),
        "Entry Hour" => Some("CAST(HOUR(p.entry_date) AS CHAR) <> ?"),
        "Formation Length" => Some(
            "CASE WHEN (p.x_length + p.a_length + p.b_length + p.c_length) <= 20 THEN '<=20' WHEN (p.x_length + p.a_length + p.b_length + p.c_length) <= 40 THEN '21-40' WHEN (p.x_length + p.a_length + p.b_length + p.c_length) <= 80 THEN '41-80' WHEN (p.x_length + p.a_length + p.b_length + p.c_length) <= 160 THEN '81-160' ELSE '161+' END <> ?",
        ),
        "Contract Week" => Some("COALESCE(CAST(p.contract_week_index AS CHAR), 'Unknown') <> ?"),
        _ => None,
    }
}

fn pattern_discovery_family_where_clause() -> String {
    r#"
        WHERE p.outcome_model = ?
          AND p.market = ?
          AND p.harmonic_type = ?
          AND p.bin = ?
          AND COALESCE(NULLIF(p.reversal_type, ''), 'None') = ?
          AND p.size_bucket = ?
          AND p.time_bin = ?
          AND p.x_strictness = ?
          AND p.three_month_trend = ?
          AND p.six_month_trend = ?
          AND p.twelve_month_trend = ?
        "#
    .to_string()
}

fn bind_pattern_discovery_family_as<'q, T>(
    query: sqlx::query::QueryAs<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>,
    family: &'q PropStrategyFamilyFilter,
) -> sqlx::query::QueryAs<'q, sqlx::MySql, T, sqlx::mysql::MySqlArguments>
where
    T: Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow>,
{
    query
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
}

async fn fetch_pattern_discovery_summary(
    pool: &MySqlPool,
    family: &PropStrategyFamilyFilter,
) -> Result<PatternDiscoverySummary, sqlx::Error> {
    let sql = format!(
        r#"
        SELECT
            CAST(COUNT(*) AS SIGNED) AS observations,
            CAST(SUM(CASE WHEN p.window_complete THEN 1 ELSE 0 END) AS SIGNED) AS complete_windows,
            CAST(COALESCE(AVG(p.bars_observed), 0.0) AS DOUBLE) AS avg_bars_observed,
            CAST(COALESCE(AVG(p.mfe_r), 0.0) AS DOUBLE) AS avg_mfe_r,
            CAST(COALESCE(AVG(p.mae_r), 0.0) AS DOUBLE) AS avg_mae_r,
            CAST(COALESCE(AVG(p.end_close_return_r), 0.0) AS DOUBLE) AS avg_end_return_r,
            CAST(COALESCE(AVG(p.close_return_1x_r), 0.0) AS DOUBLE) AS avg_return_1x_r,
            CAST(COALESCE(AVG(p.close_return_2x_r), 0.0) AS DOUBLE) AS avg_return_2x_r,
            CAST(COALESCE(AVG(p.close_return_3x_r), 0.0) AS DOUBLE) AS avg_return_3x_r,
            CAST(COALESCE(AVG(p.close_return_5x_r), 0.0) AS DOUBLE) AS avg_return_5x_r,
            CAST(COALESCE(AVG(CASE WHEN p.hit_pos_0_5r_bar IS NULL THEN 0 ELSE 1 END), 0.0) AS DOUBLE) AS hit_pos_0_5r_rate,
            CAST(COALESCE(AVG(CASE WHEN p.hit_pos_1_0r_bar IS NULL THEN 0 ELSE 1 END), 0.0) AS DOUBLE) AS hit_pos_1_0r_rate,
            CAST(COALESCE(AVG(CASE WHEN p.hit_pos_1_5r_bar IS NULL THEN 0 ELSE 1 END), 0.0) AS DOUBLE) AS hit_pos_1_5r_rate,
            CAST(COALESCE(AVG(CASE WHEN p.hit_pos_2_0r_bar IS NULL THEN 0 ELSE 1 END), 0.0) AS DOUBLE) AS hit_pos_2_0r_rate,
            CAST(COALESCE(AVG(CASE WHEN p.hit_neg_0_5r_bar IS NULL THEN 0 ELSE 1 END), 0.0) AS DOUBLE) AS hit_neg_0_5r_rate,
            CAST(COALESCE(AVG(CASE WHEN p.hit_neg_1_0r_bar IS NULL THEN 0 ELSE 1 END), 0.0) AS DOUBLE) AS hit_neg_1_0r_rate,
            CAST(COALESCE(AVG(CASE WHEN p.hit_pos_1r_before_neg_1r THEN 1 ELSE 0 END), 0.0) AS DOUBLE) AS pos_1r_before_neg_1r_rate,
            CAST(COALESCE(AVG(p.hit_pos_1_0r_bar), 0.0) AS DOUBLE) AS avg_pos_1r_bar,
            CAST(COALESCE(AVG(p.hit_neg_1_0r_bar), 0.0) AS DOUBLE) AS avg_neg_1r_bar
        FROM pattern_forward_observations p
        {where_clause}
        "#,
        where_clause = pattern_discovery_family_where_clause(),
    );

    bind_pattern_discovery_family_as(sqlx::query_as::<_, PatternDiscoverySummary>(&sql), family)
        .fetch_one(pool)
        .await
}

async fn fetch_pattern_discovery_breakdown(
    pool: &MySqlPool,
    family: &PropStrategyFamilyFilter,
    title: &str,
    value_expr: &str,
    min_rows: i64,
    limit: i64,
) -> Result<PatternDiscoveryBreakdown, sqlx::Error> {
    let sql = format!(
        r#"
        SELECT
            {value_expr} AS value,
            CAST(COUNT(*) AS SIGNED) AS observations,
            CAST(COALESCE(AVG(p.mfe_r), 0.0) AS DOUBLE) AS avg_mfe_r,
            CAST(COALESCE(AVG(p.mae_r), 0.0) AS DOUBLE) AS avg_mae_r,
            CAST(COALESCE(AVG(p.close_return_3x_r), 0.0) AS DOUBLE) AS avg_return_3x_r,
            CAST(COALESCE(AVG(p.close_return_5x_r), 0.0) AS DOUBLE) AS avg_return_5x_r,
            CAST(COALESCE(AVG(CASE WHEN p.hit_pos_1_0r_bar IS NULL THEN 0 ELSE 1 END), 0.0) AS DOUBLE) AS hit_pos_1_0r_rate,
            CAST(COALESCE(AVG(CASE WHEN p.hit_neg_1_0r_bar IS NULL THEN 0 ELSE 1 END), 0.0) AS DOUBLE) AS hit_neg_1_0r_rate,
            CAST(COALESCE(AVG(CASE WHEN p.hit_pos_1r_before_neg_1r THEN 1 ELSE 0 END), 0.0) AS DOUBLE) AS pos_1r_before_neg_1r_rate
        FROM pattern_forward_observations p
        {where_clause}
        GROUP BY value
        HAVING observations >= ?
        ORDER BY pos_1r_before_neg_1r_rate DESC, observations DESC
        LIMIT ?
        "#,
        where_clause = pattern_discovery_family_where_clause(),
    );

    let rows = bind_pattern_discovery_family_as(
        sqlx::query_as::<_, PatternDiscoveryBreakdownRow>(&sql),
        family,
    )
    .bind(min_rows)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(PatternDiscoveryBreakdown {
        title: title.to_string(),
        rows,
    })
}

async fn fetch_pattern_discovery_examples(
    pool: &MySqlPool,
    family: &PropStrategyFamilyFilter,
    limit: i64,
) -> Result<Vec<PatternDiscoveryObservationExample>, sqlx::Error> {
    let sql = format!(
        r#"
        SELECT
            p.observation_id,
            p.symbol,
            p.entry_date,
            p.observation_end_date,
            CAST(p.reference_price AS DOUBLE) AS reference_price,
            CAST(p.mfe_r AS DOUBLE) AS mfe_r,
            CAST(p.mae_r AS DOUBLE) AS mae_r,
            CAST(p.end_close_return_r AS DOUBLE) AS end_close_return_r,
            CAST(p.close_return_3x_r AS DOUBLE) AS close_return_3x_r,
            CAST(p.close_return_5x_r AS DOUBLE) AS close_return_5x_r,
            CAST(p.hit_pos_1_0r_bar AS SIGNED) AS hit_pos_1_0r_bar,
            CAST(p.hit_neg_1_0r_bar AS SIGNED) AS hit_neg_1_0r_bar,
            p.hit_pos_1r_before_neg_1r
        FROM pattern_forward_observations p
        {where_clause}
        ORDER BY p.entry_date DESC, p.symbol ASC
        LIMIT ?
        "#,
        where_clause = pattern_discovery_family_where_clause(),
    );

    bind_pattern_discovery_family_as(
        sqlx::query_as::<_, PatternDiscoveryObservationExample>(&sql),
        family,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
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

    let (earliest_entry_date, latest_entry_date) =
        match sqlx::query_as::<_, (Option<NaiveDateTime>, Option<NaiveDateTime>)>(&range_sql)
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

#[route("/pattern-discovery/family", method = "GET", method = "POST")]
async fn fetch_pattern_discovery_family(
    pool: web::Data<MySqlPool>,
    params: web::Json<PatternDiscoveryParams>,
) -> impl Responder {
    let Some(prop_strategy_id) = params
        .prop_strategy_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return HttpResponse::BadRequest().body("Missing family key");
    };

    let family = match fetch_prop_strategy_family_filter(pool.get_ref(), prop_strategy_id).await {
        Ok(Some(family)) => family,
        Ok(None) => return HttpResponse::NotFound().body("Prop strategy family not found"),
        Err(error) => {
            eprintln!("Pattern discovery family lookup DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let mut has_forward_observations_table = true;
    let summary = match fetch_pattern_discovery_summary(pool.get_ref(), &family).await {
        Ok(summary) => summary,
        Err(error) if is_missing_table_error(&error) => {
            has_forward_observations_table = false;
            empty_pattern_discovery_summary()
        }
        Err(error) => {
            eprintln!("Pattern discovery summary DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let min_rows = (summary.observations / 20).clamp(5, 50);
    let breakdown_specs = [
        (
            "Roots",
            "COALESCE(NULLIF(p.root_symbol, ''), SUBSTRING(p.symbol, 1, 3))",
        ),
        (
            "Entry Hour",
            "CAST(HOUR(p.entry_date) AS CHAR)",
        ),
        (
            "Formation Length",
            "CASE WHEN p.formation_length <= 20 THEN '<=20' WHEN p.formation_length <= 40 THEN '21-40' WHEN p.formation_length <= 80 THEN '41-80' WHEN p.formation_length <= 160 THEN '81-160' ELSE '161+' END",
        ),
        (
            "Contract Week",
            "COALESCE(CAST(p.contract_week_index AS CHAR), 'Unknown')",
        ),
        (
            "Window Complete",
            "CASE WHEN p.window_complete THEN 'Complete' ELSE 'Partial' END",
        ),
    ];
    let mut breakdowns = Vec::new();
    if has_forward_observations_table && summary.observations > 0 {
        for (title, value_expr) in breakdown_specs {
            match fetch_pattern_discovery_breakdown(
                pool.get_ref(),
                &family,
                title,
                value_expr,
                min_rows,
                10,
            )
            .await
            {
                Ok(breakdown) => breakdowns.push(breakdown),
                Err(error) if is_missing_table_error(&error) => {
                    has_forward_observations_table = false;
                    break;
                }
                Err(error) => {
                    eprintln!("Pattern discovery breakdown DB error: {:?}", error);
                    return HttpResponse::InternalServerError().finish();
                }
            }
        }
    }

    let examples = if has_forward_observations_table && summary.observations > 0 {
        match fetch_pattern_discovery_examples(
            pool.get_ref(),
            &family,
            params.limit.unwrap_or(40).clamp(1, 200),
        )
        .await
        {
            Ok(examples) => examples,
            Err(error) if is_missing_table_error(&error) => Vec::new(),
            Err(error) => {
                eprintln!("Pattern discovery examples DB error: {:?}", error);
                return HttpResponse::InternalServerError().finish();
            }
        }
    } else {
        Vec::new()
    };

    let saved_logic =
        match fetch_pattern_discovery_saved_logic(pool.get_ref(), &family.family_key).await {
            Ok(rows) => rows,
            Err(error) => {
                eprintln!("Pattern discovery saved logic DB error: {:?}", error);
                return HttpResponse::InternalServerError().finish();
            }
        };

    HttpResponse::Ok().json(PatternDiscoveryResponse {
        family: PatternDiscoveryFamily::from(&family),
        summary,
        breakdowns,
        examples,
        saved_logic,
    })
}

#[route("/pattern-discovery/save-logic", method = "POST")]
async fn save_pattern_discovery_logic(
    pool: web::Data<MySqlPool>,
    params: web::Json<PatternDiscoverySaveLogicParams>,
) -> impl Responder {
    let Some(prop_strategy_id) = params
        .prop_strategy_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return HttpResponse::BadRequest().body("Missing family key");
    };
    let Some(logic_text) = params
        .logic_text
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return HttpResponse::BadRequest().body("Missing logic text");
    };

    let family = match fetch_prop_strategy_family_filter(pool.get_ref(), prop_strategy_id).await {
        Ok(Some(family)) => family,
        Ok(None) => return HttpResponse::NotFound().body("Prop strategy family not found"),
        Err(error) => {
            eprintln!("Pattern discovery save family lookup DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    if let Err(error) = ensure_pattern_discovery_logic_table(pool.get_ref()).await {
        eprintln!("Pattern discovery save table DB error: {:?}", error);
        return HttpResponse::InternalServerError().finish();
    }

    let title = params
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Candidate logic");
    let rule_json = params
        .rule_json
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Err(error) = sqlx::query(
        r#"
        INSERT INTO pattern_discovery_logic (
            family_key,
            title,
            logic_text,
            rule_json,
            status
        )
        VALUES (?, ?, ?, ?, 'candidate')
        "#,
    )
    .bind(&family.family_key)
    .bind(title)
    .bind(logic_text)
    .bind(rule_json)
    .execute(pool.get_ref())
    .await
    {
        eprintln!("Pattern discovery save logic DB error: {:?}", error);
        return HttpResponse::InternalServerError().finish();
    }

    match fetch_pattern_discovery_saved_logic(pool.get_ref(), &family.family_key).await {
        Ok(rows) => HttpResponse::Ok().json(rows),
        Err(error) => {
            eprintln!(
                "Pattern discovery saved logic refresh DB error: {:?}",
                error
            );
            HttpResponse::InternalServerError().finish()
        }
    }
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
    if let (Some(result_r), Some(risk_points)) = (trade.result_r, trade.risk_points) {
        return result_r
            * risk_points.abs()
            * contracts as f64
            * futures_point_value(&trade.symbol);
    }

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
        .map(|item| {
            item.trim()
                .eq_ignore_ascii_case(SIMULATOR_DRAWDOWN_MODEL_EOD)
        })
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

fn build_simulator_replay_response(
    family_key: &str,
    rows: &[SimulatorReplaySourceTrade],
    first_start_date: NaiveDateTime,
    tests_to_chain: i64,
    contracts: i64,
    starting_balance: f64,
    profit_target: f64,
    max_drawdown: f64,
    daily_loss_limit: Option<f64>,
    drawdown_model: &str,
    one_trade_at_a_time: bool,
    candidate_logic_applied: bool,
    candidate_logic_filters: Vec<String>,
) -> SimulatorReplayResponse {
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
                    trade_id: trade.trade_id,
                    trade_uid: trade.trade_uid.clone(),
                    trade_direction: trade.trade_direction.clone(),
                    test_index,
                    trade_index: trade_count + skipped_overlap_count,
                    symbol: trade.symbol.clone(),
                    pattern_id: trade.pattern_id.clone(),
                    pattern_group_id: trade.pattern_group_id.clone(),
                    entry_date: trade.entry_date,
                    target_date: trade.target_date,
                    trade_result: trade.trade_result,
                    exit_reason: trade.exit_reason.clone(),
                    trade_enter_price: trade.trade_enter_price,
                    trade_risk_exit_price: trade.trade_risk_exit_price,
                    trade_reward_exit_price: trade.trade_reward_exit_price,
                    exit_price: trade.exit_price,
                    result_r: trade.result_r,
                    risk_points: trade.risk_points,
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
                trade_id: trade.trade_id,
                trade_uid: trade.trade_uid.clone(),
                trade_direction: trade.trade_direction.clone(),
                test_index,
                trade_index: trade_count,
                symbol: trade.symbol.clone(),
                pattern_id: trade.pattern_id.clone(),
                pattern_group_id: trade.pattern_group_id.clone(),
                entry_date: trade.entry_date,
                target_date: trade.target_date,
                trade_result: trade.trade_result,
                exit_reason: trade.exit_reason.clone(),
                trade_enter_price: trade.trade_enter_price,
                trade_risk_exit_price: trade.trade_risk_exit_price,
                trade_reward_exit_price: trade.trade_reward_exit_price,
                exit_price: trade.exit_price,
                result_r: trade.result_r,
                risk_points: trade.risk_points,
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

    SimulatorReplayResponse {
        family_key: family_key.to_string(),
        tests,
        trades: events,
        eligible_trade_count: rows.len() as i64,
        candidate_logic_applied,
        candidate_logic_filters,
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
    let use_candidate_logic = params.use_candidate_logic.unwrap_or(false);

    let family = match fetch_prop_strategy_family_filter(pool.get_ref(), prop_strategy_id).await {
        Ok(Some(family)) => family,
        Ok(None) => return HttpResponse::NotFound().body("Prop strategy family not found"),
        Err(error) => {
            eprintln!("Simulator family lookup DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let candidate_logic_filters = if use_candidate_logic {
        match fetch_pattern_discovery_saved_logic(pool.get_ref(), &family.family_key).await {
            Ok(saved_logic) => saved_logic
                .first()
                .map(|logic| parse_candidate_logic_replay_filters(&logic.logic_text))
                .unwrap_or_default(),
            Err(error) => {
                eprintln!("Simulator candidate logic DB error: {:?}", error);
                return HttpResponse::InternalServerError().finish();
            }
        }
    } else {
        Vec::new()
    };
    let candidate_logic_filter_sql = candidate_logic_filters
        .iter()
        .filter_map(|filter| candidate_logic_filter_condition(&filter.title))
        .map(|condition| format!("              AND ({condition})\n"))
        .collect::<String>();
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
                CAST(COALESCE(
                    p.target_close,
                    CASE
                        WHEN p.prop_result = 1 THEN p.trade_reward_exit_price
                        WHEN p.prop_result = 2 THEN p.trade_risk_exit_price
                        ELSE p.trade_enter_price
                    END
                ) AS DOUBLE) AS exit_price,
                CAST(NULL AS DOUBLE) AS result_r,
                CAST(NULL AS DOUBLE) AS risk_points,
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
{candidate_logic_filter_sql}              AND p.target_date IS NOT NULL
              AND p.entry_date >= ?
            ORDER BY p.entry_date ASC,
                     p.target_date ASC,
                     p.trade_enter_price ASC
            LIMIT 50000
        "#,
        route_x_strictness_expr = route_x_strictness_expr,
        candidate_logic_filter_sql = candidate_logic_filter_sql,
    );

    let mut query = sqlx::query_as::<_, SimulatorReplaySourceTrade>(&sql)
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
        .bind(&family.twelve_month_trend);
    for filter in &candidate_logic_filters {
        query = query.bind(&filter.value);
    }
    let rows = match query.bind(first_start_date).fetch_all(pool.get_ref()).await {
        Ok(rows) => rows,
        Err(error) if is_missing_table_error(&error) => Vec::new(),
        Err(error) => {
            eprintln!("Simulator replay DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().json(build_simulator_replay_response(
        prop_strategy_id,
        &rows,
        first_start_date,
        tests_to_chain,
        contracts,
        starting_balance,
        profit_target,
        max_drawdown,
        daily_loss_limit,
        drawdown_model,
        one_trade_at_a_time,
        use_candidate_logic && !candidate_logic_filters.is_empty(),
        candidate_logic_filters
            .iter()
            .map(|filter| filter.label.clone())
            .collect(),
    ))
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
            .service(fetch_pattern_discovery_family)
            .service(save_pattern_discovery_logic)
            .service(fetch_simulator_family_replay)
            .service(fetch_simulator_phase1_route_replay)
            .service(fetch_simulator_phase1_pattern_route_replay)
            .service(fetch_current_open_setups)
            .service(fetch_current_setup_strategies)
            .service(fetch_pattern_detail)
            .service(fetch_candle_storage_summary)
            .service(fetch_admin_status)
            .service(run_admin_action)
            .service(fetch_setup_comparison)
            .service(fetch_strategy_candidates)
            .service(fetch_pattern_families)
            .service(fetch_phase1_results)
            .service(fetch_phase1_family_patterns)
            .service(fetch_phase1_leaderboard)
            .service(fetch_phase1_supply)
            .service(fetch_phase1_yearly_breakdown)
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
    if params
        .prop_outcome_mode
        .as_deref()
        .map(|mode| mode.trim().eq_ignore_ascii_case("phase1-family"))
        .unwrap_or(false)
    {
        return match fetch_pattern_detail_from_pattern_setups(pool.get_ref(), &params).await {
            Ok(Some(pattern)) => HttpResponse::Ok().json(pattern),
            Ok(None) => HttpResponse::NotFound().body("Pattern not found in pattern_setups"),
            Err(error) => {
                eprintln!("Pattern setup detail lookup failed: {:?}", error);
                HttpResponse::InternalServerError().finish()
            }
        };
    }

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

async fn fetch_pattern_detail_from_pattern_setups(
    pool: &MySqlPool,
    params: &PatternDetailParams,
) -> Result<Option<Pattern>, sqlx::Error> {
    let has_pattern_id = params
        .pattern_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();
    let setup_where = if has_pattern_id {
        "WHERE ps.pattern_id = ?"
    } else {
        "WHERE ps.pattern_group_id = ?"
    };
    let sql = format!(
        r#"
        WITH selected_setup AS (
            SELECT ps.*
            FROM pattern_setups ps
            {setup_where}
            ORDER BY ps.d_date DESC, ps.setup_id ASC
            LIMIT 1
        ),
        best_harmonic AS (
            SELECT
                hs.setup_id,
                SUBSTRING_INDEX(
                    GROUP_CONCAT(
                        hs.harmonic_type
                        ORDER BY
                            COALESCE(hs.price_accuracy, 0.0) DESC,
                            COALESCE(hs.time_accuracy, 0.0) DESC,
                            hs.harmonic_type ASC
                        SEPARATOR '|'
                    ),
                    '|',
                    1
                ) AS harmonic_type,
                CAST(SUBSTRING_INDEX(
                    GROUP_CONCAT(
                        COALESCE(CAST(hs.price_accuracy AS CHAR), '0')
                        ORDER BY
                            COALESCE(hs.price_accuracy, 0.0) DESC,
                            COALESCE(hs.time_accuracy, 0.0) DESC,
                            hs.harmonic_type ASC
                        SEPARATOR '|'
                    ),
                    '|',
                    1
                ) AS DOUBLE) AS price_accuracy,
                CAST(SUBSTRING_INDEX(
                    GROUP_CONCAT(
                        COALESCE(CAST(hs.time_accuracy AS CHAR), '0')
                        ORDER BY
                            COALESCE(hs.price_accuracy, 0.0) DESC,
                            COALESCE(hs.time_accuracy, 0.0) DESC,
                            hs.harmonic_type ASC
                        SEPARATOR '|'
                    ),
                    '|',
                    1
                ) AS DOUBLE) AS time_accuracy
            FROM pattern_harmonic_scores hs
            INNER JOIN selected_setup ps
              ON ps.setup_id = hs.setup_id
            GROUP BY hs.setup_id
        )
        SELECT
            ps.symbol,
            ps.pattern_id,
            CAST(ps.x_date AS DATETIME) AS x_date,
            CAST(ps.x_open AS DECIMAL(18,6)) AS x_open,
            CAST(ps.x_high AS DECIMAL(18,6)) AS x_high,
            CAST(ps.x_low AS DECIMAL(18,6)) AS x_low,
            CAST(ps.x_close AS DECIMAL(18,6)) AS x_close,
            CAST(ps.x_length AS DECIMAL(18,0)) AS x_length,
            CAST(ps.x_min_max AS DECIMAL(18,6)) AS x_min_max,
            CAST(ps.a_date AS DATETIME) AS a_date,
            CAST(ps.a_open AS DECIMAL(18,6)) AS a_open,
            CAST(ps.a_high AS DECIMAL(18,6)) AS a_high,
            CAST(ps.a_low AS DECIMAL(18,6)) AS a_low,
            CAST(ps.a_close AS DECIMAL(18,6)) AS a_close,
            CAST(ps.a_length AS DECIMAL(18,0)) AS a_length,
            CAST(ps.a_min_max AS DECIMAL(18,6)) AS a_min_max,
            CAST(ps.b_date AS DATETIME) AS b_date,
            CAST(ps.b_open AS DECIMAL(18,6)) AS b_open,
            CAST(ps.b_high AS DECIMAL(18,6)) AS b_high,
            CAST(ps.b_low AS DECIMAL(18,6)) AS b_low,
            CAST(ps.b_close AS DECIMAL(18,6)) AS b_close,
            CAST(ps.b_length AS DECIMAL(18,0)) AS b_length,
            CAST(ps.b_min_max AS DECIMAL(18,6)) AS b_min_max,
            CAST(ps.c_date AS DATETIME) AS c_date,
            CAST(ps.c_open AS DECIMAL(18,6)) AS c_open,
            CAST(ps.c_high AS DECIMAL(18,6)) AS c_high,
            CAST(ps.c_low AS DECIMAL(18,6)) AS c_low,
            CAST(ps.c_close AS DECIMAL(18,6)) AS c_close,
            CAST(ps.c_length AS DECIMAL(18,0)) AS c_length,
            CAST(ps.c_min_max AS DECIMAL(18,6)) AS c_min_max,
            CAST(ps.d_date AS DATETIME) AS d_date,
            CAST(ps.d_open AS DECIMAL(18,6)) AS d_open,
            CAST(ps.d_high AS DECIMAL(18,6)) AS d_high,
            CAST(ps.d_low AS DECIMAL(18,6)) AS d_low,
            CAST(ps.d_close AS DECIMAL(18,6)) AS d_close,
            CAST(ps.d_length AS DECIMAL(18,0)) AS d_length,
            CAST(ps.full_pattern_length AS SIGNED) AS full_pattern_length,
            CAST(ps.d_min_max AS DECIMAL(18,6)) AS d_min_max,
            TRUE AS trade_open,
            CAST(COALESCE(ps.d_confirm_date, ps.d_date) AS DATETIME) AS entry_date,
            CAST(COALESCE(ps.d_confirm_date, ps.d_date) AS DATETIME) AS d_confirm_date,
            CAST(NULL AS DATETIME) AS reversal_detect_date,
            CAST(NULL AS DATETIME) AS target_date,
            CAST(NULL AS DECIMAL(18,6)) AS target_open,
            CAST(NULL AS DECIMAL(18,6)) AS target_high,
            CAST(NULL AS DECIMAL(18,6)) AS target_low,
            CAST(NULL AS DECIMAL(18,6)) AS target_close,
            CAST(NULL AS SIGNED) AS target_volume,
            CAST(NULL AS SIGNED) AS target_is_green,
            CAST(NULL AS DECIMAL(18,6)) AS target_close_vs_open_pct,
            CAST(NULL AS DECIMAL(18,6)) AS target_high_vs_open_pct,
            CAST(NULL AS DECIMAL(18,6)) AS target_low_vs_open_pct,
            CAST(NULL AS DECIMAL(18,6)) AS target_range_pct,
            CAST(NULL AS SIGNED) AS target_breaks_entry_high,
            CAST(NULL AS SIGNED) AS target_breaks_entry_low,
            CAST(CASE WHEN ps.market = 'Bullish' THEN ps.d_low ELSE ps.d_high END AS DECIMAL(18,6)) AS trade_risk_exit_price,
            CAST(ps.d_close AS DECIMAL(18,6)) AS trade_reward_exit_price,
            CAST(ps.d_close AS DECIMAL(18,6)) AS trade_enter_price,
            CAST(ps.d_close AS DECIMAL(18,6)) AS trade_current_price,
            CAST(0 AS DECIMAL(18,0)) AS trade_length,
            CAST(0 AS DECIMAL(18,6)) AS trade_pnl,
            CAST(0 AS SIGNED) AS trade_result,
            CAST(COALESCE(ps.d_confirm_date, ps.d_date) AS DATETIME) AS trade_date,
            CAST(COALESCE(ABS(ps.b_min_max - ps.a_min_max) / NULLIF(ABS(ps.a_min_max - ps.x_min_max), 0) * 100.0, 0.0) AS DECIMAL(12,4)) AS trade_ab_price_retracement,
            CAST(COALESCE(ABS(ps.c_min_max - ps.b_min_max) / NULLIF(ABS(ps.b_min_max - ps.a_min_max), 0) * 100.0, 0.0) AS DECIMAL(12,4)) AS trade_bc_price_retracement,
            CAST(COALESCE(ABS(ps.d_min_max - ps.c_min_max) / NULLIF(ABS(ps.c_min_max - ps.b_min_max), 0) * 100.0, 0.0) AS DECIMAL(12,4)) AS trade_cd_bc_price_retracement,
            CAST((ps.d_min_max - ps.c_min_max) AS DECIMAL(18,6)) AS trade_cd_price_retracement,
            CAST(COALESCE(ABS(ps.d_min_max - ps.c_min_max) / NULLIF(ABS(ps.a_min_max - ps.x_min_max), 0) * 100.0, 0.0) AS DECIMAL(12,4)) AS trade_cd_xa_price_retracement,
            CAST(COALESCE(ps.b_length / NULLIF(ps.a_length, 0) * 100.0, 0.0) AS DOUBLE) AS trade_ab_bar_retracement,
            CAST(COALESCE(ps.c_length / NULLIF(ps.b_length, 0) * 100.0, 0.0) AS DECIMAL(18,6)) AS trade_bc_bar_retracement,
            CAST(COALESCE(ps.d_length / NULLIF(ps.c_length, 0) * 100.0, 0.0) AS DECIMAL(18,6)) AS trade_cd_bar_retracement,
            CAST(COALESCE(ps.d_length / NULLIF(ps.c_length, 0) * 100.0, 0.0) AS DOUBLE) AS trade_cd_bc_bar_retracement,
            CAST(COALESCE(ps.d_length / NULLIF(ps.x_length, 0) * 100.0, 0.0) AS DOUBLE) AS trade_cd_xa_bar_retracement,
            CAST(0.0 AS DECIMAL(18,6)) AS trade_snr,
            CAST(YEAR(COALESCE(ps.d_confirm_date, ps.d_date)) AS SIGNED) AS trade_year,
            CAST(MONTH(COALESCE(ps.d_confirm_date, ps.d_date)) AS SIGNED) AS trade_month,
            CAST(DAY(COALESCE(ps.d_confirm_date, ps.d_date)) AS SIGNED) AS trade_day,
            'None' AS reversal_type,
            ps.bullish_key_reversal,
            ps.bearish_key_reversal,
            ps.bullish_engulfing,
            ps.bearish_engulfing,
            ps.bullish_outside_reversal,
            ps.bearish_outside_reversal,
            ps.hammer,
            ps.shooting_star,
            ps.morning_star,
            ps.evening_star,
            ps.three_white_soldiers,
            ps.three_black_crows,
            ps.market,
            ps.three_month,
            ps.six_month,
            ps.twelve_month,
            ps.pattern_group_id,
            COALESCE(best_harmonic.harmonic_type, ps.harmonic_type) AS harmonic_type,
            CAST(CASE WHEN COALESCE(best_harmonic.harmonic_type, ps.harmonic_type) IN ('Bat', 'AlternateBat') THEN COALESCE(best_harmonic.price_accuracy, 0.0) ELSE NULL END AS DECIMAL(12,2)) AS bat_accuracy,
            CAST(CASE WHEN COALESCE(best_harmonic.harmonic_type, ps.harmonic_type) = 'Butterfly' THEN COALESCE(best_harmonic.price_accuracy, 0.0) ELSE NULL END AS DECIMAL(12,2)) AS butterfly_accuracy,
            CAST(CASE WHEN COALESCE(best_harmonic.harmonic_type, ps.harmonic_type) = 'Gartley' THEN COALESCE(best_harmonic.price_accuracy, 0.0) ELSE NULL END AS DECIMAL(12,2)) AS gartley_accuracy,
            CAST(CASE WHEN COALESCE(best_harmonic.harmonic_type, ps.harmonic_type) IN ('Crab', 'DeepCrab') THEN COALESCE(best_harmonic.price_accuracy, 0.0) ELSE NULL END AS DECIMAL(12,2)) AS crab_accuracy,
            CAST(CASE WHEN COALESCE(best_harmonic.harmonic_type, ps.harmonic_type) = 'Shark' THEN COALESCE(best_harmonic.price_accuracy, 0.0) ELSE NULL END AS DECIMAL(12,2)) AS shark_accuracy,
            CAST(best_harmonic.time_accuracy AS DOUBLE) AS time_accuracy
        FROM selected_setup ps
        LEFT JOIN best_harmonic
          ON best_harmonic.setup_id = ps.setup_id
        "#,
        setup_where = setup_where,
    );

    let query = if let Some(pattern_id) = params
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

    match query.fetch_optional(pool).await {
        Ok(row) => Ok(row),
        Err(error) if is_missing_table_error(&error) => Ok(None),
        Err(error) => Err(error),
    }
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

#[route("/pattern-families", method = "GET", method = "POST")]
async fn fetch_pattern_families(
    pool: web::Data<MySqlPool>,
    params: web::Json<PatternFamilyParams>,
) -> impl Responder {
    let limit = params.limit.unwrap_or(1_000).clamp(1, 10_000);
    let min_setup_count = params.min_setup_count.unwrap_or(1).max(0);
    let year = params.year.filter(|year| (1900..=2200).contains(year));
    let source_scope = normalize_pattern_family_source_scope(params.source_scope.as_deref());

    if year.is_some() || source_scope != PatternFamilySourceScope::All {
        let rows = sqlx::query_as::<_, PatternFamilySummary>(
            r#"
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
                twelve_month_trend,
                CAST(setup_count AS SIGNED) AS setup_count,
                CAST(symbol_count AS SIGNED) AS symbol_count,
                first_d_date,
                last_d_date
            FROM pattern_family_source_summary
            WHERE source_scope = ?
              AND period_year = ?
              AND setup_count >= ?
            ORDER BY setup_count DESC, symbol_count DESC, family_key ASC
            LIMIT ?
            "#,
        )
        .bind(pattern_family_source_scope_label(source_scope))
        .bind(year.unwrap_or(0))
        .bind(min_setup_count)
        .bind(limit)
        .fetch_all(pool.get_ref())
        .await;

        return match rows {
            Ok(rows) => HttpResponse::Ok().json(rows),
            Err(error) if is_missing_table_error(&error) => {
                HttpResponse::Ok().json(Vec::<PatternFamilySummary>::new())
            }
            Err(error) => {
                eprintln!("Pattern families yearly DB error: {:?}", error);
                HttpResponse::InternalServerError().finish()
            }
        };
    }

    let rows = sqlx::query_as::<_, PatternFamilySummary>(
        r#"
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
            twelve_month_trend,
            CAST(setup_count AS SIGNED) AS setup_count,
            CAST(symbol_count AS SIGNED) AS symbol_count,
            first_d_date,
            last_d_date
        FROM pattern_family_summary
        WHERE setup_count >= ?
        ORDER BY setup_count DESC, symbol_count DESC, family_key ASC
        LIMIT ?
        "#,
    )
    .bind(min_setup_count)
    .bind(limit)
    .fetch_all(pool.get_ref())
    .await;

    match rows {
        Ok(rows) => HttpResponse::Ok().json(rows),
        Err(error) if is_missing_table_error(&error) => {
            HttpResponse::Ok().json(Vec::<PatternFamilySummary>::new())
        }
        Err(error) => {
            eprintln!("Pattern families DB error: {:?}", error);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[route("/phase1/results", method = "GET", method = "POST")]
async fn fetch_phase1_results(
    pool: web::Data<MySqlPool>,
    params: web::Json<Phase1ResultsParams>,
) -> impl Responder {
    let Some(family_key) = params
        .family_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return HttpResponse::Ok().json(Vec::<Phase1StrategyResult>::new());
    };
    let source_scope = pattern_family_source_scope_label(normalize_pattern_family_source_scope(
        params.source_scope.as_deref(),
    ));
    let period_year = params
        .year
        .filter(|year| (1900..=2200).contains(year))
        .unwrap_or(0);
    let limit = params.limit.unwrap_or(250).clamp(1, 2_000);

    let latest_run_id = match sqlx::query_scalar::<_, Option<String>>(
        r#"
        SELECT run_id
        FROM phase1_strategy_runs
        WHERE family_key = ?
          AND source_scope = ?
          AND period_year = ?
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .bind(family_key)
    .bind(source_scope)
    .bind(period_year)
    .fetch_one(pool.get_ref())
    .await
    {
        Ok(run_id) => run_id,
        Err(error) if is_missing_table_error(&error) => None,
        Err(error) => {
            eprintln!("Phase 1 latest run DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let Some(run_id) = latest_run_id else {
        return HttpResponse::Ok().json(Vec::<Phase1StrategyResult>::new());
    };

    let rows = sqlx::query_as::<_, Phase1StrategyResult>(
        r#"
        SELECT
            run_id,
            family_key,
            source_scope,
            CAST(period_year AS SIGNED) AS period_year,
            route_id,
            route_label,
            CAST(result_rank AS SIGNED) AS result_rank,
            entry_mode,
            stop_mode,
            CAST(target_r AS DOUBLE) AS target_r,
            CAST(max_hold_multiple AS SIGNED) AS max_hold_multiple,
            CAST(setup_count AS SIGNED) AS setup_count,
            CAST(trade_count AS SIGNED) AS trade_count,
            CAST(no_entry_count AS SIGNED) AS no_entry_count,
            CAST(win_count AS SIGNED) AS win_count,
            CAST(loss_count AS SIGNED) AS loss_count,
            CAST(win_rate AS DOUBLE) AS win_rate,
            CAST(avg_r AS DOUBLE) AS avg_r,
            CAST(profit_factor AS DOUBLE) AS profit_factor,
            CAST(max_drawdown_r AS DOUBLE) AS max_drawdown_r,
            CAST(worst_year_avg_r AS DOUBLE) AS worst_year_avg_r,
            CAST(score AS DOUBLE) AS score,
            created_at
        FROM phase1_strategy_results
        WHERE run_id = ?
        ORDER BY result_rank ASC
        LIMIT ?
        "#,
    )
    .bind(run_id)
    .bind(limit)
    .fetch_all(pool.get_ref())
    .await;

    match rows {
        Ok(rows) => HttpResponse::Ok().json(rows),
        Err(error) if is_missing_table_error(&error) => {
            HttpResponse::Ok().json(Vec::<Phase1StrategyResult>::new())
        }
        Err(error) => {
            eprintln!("Phase 1 results DB error: {:?}", error);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[route("/phase1/family-patterns", method = "GET", method = "POST")]
async fn fetch_phase1_family_patterns(
    pool: web::Data<MySqlPool>,
    params: web::Json<Phase1FamilyPatternsParams>,
) -> impl Responder {
    let Some(family_key) = params
        .family_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return HttpResponse::Ok().json(PatternSummariesResponse {
            patterns: Vec::new(),
            total_count: 0,
            has_more: false,
            earliest_entry_date: None,
            latest_entry_date: None,
            entry_dates: Vec::new(),
        });
    };

    let source_scope = normalize_pattern_family_source_scope(params.source_scope.as_deref());
    let source_scope_label = pattern_family_source_scope_label(source_scope);
    let period_year = params
        .year
        .filter(|year| (1900..=2200).contains(year))
        .unwrap_or(0);
    let limit = params.limit.unwrap_or(500).clamp(1, 5_000);
    let offset = params.offset.unwrap_or(0).max(0);
    let include_count = params.include_count.unwrap_or(true);

    let family_from_source = if period_year > 0 || source_scope != PatternFamilySourceScope::All {
        sqlx::query_as::<_, PatternFamilySummary>(
            r#"
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
                twelve_month_trend,
                CAST(setup_count AS SIGNED) AS setup_count,
                CAST(symbol_count AS SIGNED) AS symbol_count,
                first_d_date,
                last_d_date
            FROM pattern_family_source_summary
            WHERE family_key = ?
              AND source_scope = ?
              AND period_year = ?
            LIMIT 1
            "#,
        )
        .bind(family_key)
        .bind(source_scope_label)
        .bind(period_year)
        .fetch_optional(pool.get_ref())
        .await
    } else {
        Ok(None)
    };

    let family = match family_from_source {
        Ok(Some(family)) => Some(family),
        Ok(None) if period_year == 0 => match sqlx::query_as::<_, PatternFamilySummary>(
            r#"
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
                twelve_month_trend,
                CAST(setup_count AS SIGNED) AS setup_count,
                CAST(symbol_count AS SIGNED) AS symbol_count,
                first_d_date,
                last_d_date
            FROM pattern_family_summary
            WHERE family_key = ?
            LIMIT 1
            "#,
        )
        .bind(family_key)
        .fetch_optional(pool.get_ref())
        .await
        {
            Ok(family) => family,
            Err(error) if is_missing_table_error(&error) => None,
            Err(error) => {
                eprintln!("Phase 1 family lookup DB error: {:?}", error);
                return HttpResponse::InternalServerError().finish();
            }
        },
        Ok(None) => None,
        Err(error) if is_missing_table_error(&error) => None,
        Err(error) => {
            eprintln!("Phase 1 source family lookup DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let Some(family) = family else {
        return HttpResponse::Ok().json(PatternSummariesResponse {
            patterns: Vec::new(),
            total_count: 0,
            has_more: false,
            earliest_entry_date: None,
            latest_entry_date: None,
            entry_dates: Vec::new(),
        });
    };

    let source_filter = match source_scope {
        PatternFamilySourceScope::Futures => {
            "AND COALESCE(ps.source_table, '') LIKE 'futures_contract_%_candles'"
        }
        PatternFamilySourceScope::Daily => {
            "AND COALESCE(ps.source_table, '') = 'candles' AND COALESCE(ps.source_timeframe, '') = 'daily'"
        }
        PatternFamilySourceScope::All => "",
    };
    let year_filter = if period_year > 0 {
        "AND YEAR(ps.d_date) = ?"
    } else {
        ""
    };
    let sql = format!(
        r#"
        SELECT
            ps.symbol,
            CAST(ps.d_date AS DATETIME) AS d_date,
            CAST(COALESCE(ps.d_confirm_date, ps.d_date) AS DATETIME) AS d_confirm_date,
            CAST(NULL AS DATETIME) AS reversal_detect_date,
            CAST(COALESCE(ps.d_confirm_date, ps.d_date) AS DATETIME) AS entry_date,
            CAST(NULL AS DATETIME) AS target_date,
            CAST(NULL AS DECIMAL(18,6)) AS target_open,
            CAST(NULL AS DECIMAL(18,6)) AS target_high,
            CAST(NULL AS DECIMAL(18,6)) AS target_low,
            CAST(NULL AS DECIMAL(18,6)) AS target_close,
            CAST(COALESCE(ps.d_close, 0) AS DECIMAL(18,6)) AS trade_enter_price,
            CAST(CASE WHEN ps.market = 'Bullish' THEN COALESCE(ps.d_low, ps.d_close, 0) ELSE COALESCE(ps.d_high, ps.d_close, 0) END AS DECIMAL(18,6)) AS trade_risk_exit_price,
            CAST(COALESCE(ps.d_close, 0) AS DECIMAL(18,6)) AS trade_reward_exit_price,
            CAST(0 AS SIGNED) AS trade_result,
            ps.market,
            ps.pattern_id,
            ps.pattern_group_id,
            ps.pattern_family_key AS prop_strategy_id,
            ps.pattern_family_harmonic_type AS harmonic_type,
            'None' AS reversal_type,
            ps.pattern_family_size_bucket AS size_bucket,
            CAST(NULL AS CHAR) AS balance_bucket,
            ps.pattern_family_time_bin AS time_bin,
            ps.pattern_family_x_strictness AS x_strictness,
            CAST(NULL AS CHAR) AS three_month_trend,
            CAST(NULL AS CHAR) AS six_month_trend,
            CAST(NULL AS CHAR) AS twelve_month_trend,
            CAST(NULL AS DOUBLE) AS time_accuracy,
            CAST(ps.x_length AS SIGNED) AS x_length,
            CAST(ps.a_length AS SIGNED) AS a_length,
            CAST(ps.b_length AS SIGNED) AS b_length,
            CAST(ps.c_length AS SIGNED) AS c_length,
            CAST(ps.d_length AS SIGNED) AS d_length,
            CAST(ps.full_pattern_length AS SIGNED) AS full_pattern_length,
            CAST(NULL AS DECIMAL(18,6)) AS bat_accuracy,
            CAST(NULL AS DECIMAL(18,6)) AS butterfly_accuracy,
            CAST(NULL AS DECIMAL(18,6)) AS gartley_accuracy,
            CAST(NULL AS DECIMAL(18,6)) AS crab_accuracy,
            CAST(NULL AS DECIMAL(18,6)) AS shark_accuracy
        FROM pattern_setups ps
        WHERE ps.pattern_family_key = ?
          AND ps.d_date IS NOT NULL
          {source_filter}
          {year_filter}
        ORDER BY ps.d_date ASC, ps.setup_id ASC
        LIMIT ? OFFSET ?
        "#,
        source_filter = source_filter,
        year_filter = year_filter,
    );

    let mut query = sqlx::query_as::<_, PatternSummary>(&sql).bind(family_key);
    if period_year > 0 {
        query = query.bind(period_year);
    }
    query = query.bind(limit).bind(offset);

    let patterns = match query.fetch_all(pool.get_ref()).await {
        Ok(patterns) => patterns,
        Err(error) if is_missing_table_error(&error) => Vec::new(),
        Err(error) => {
            eprintln!("Phase 1 family patterns DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let total_count = if include_count {
        family.setup_count
    } else {
        -1
    };
    let has_more = if total_count >= 0 {
        offset + (patterns.len() as i64) < total_count
    } else {
        patterns.len() as i64 >= limit
    };
    let earliest_entry_date = family
        .first_d_date
        .and_then(|date| date.and_hms_opt(0, 0, 0));
    let latest_entry_date = family
        .last_d_date
        .and_then(|date| date.and_hms_opt(0, 0, 0));
    let mut seen_dates = HashSet::new();
    let entry_dates = patterns
        .iter()
        .filter_map(|pattern| pattern.entry_date.map(|date| date.date()))
        .filter(|date| seen_dates.insert(*date))
        .collect::<Vec<_>>();

    HttpResponse::Ok().json(PatternSummariesResponse {
        patterns,
        total_count,
        has_more,
        earliest_entry_date,
        latest_entry_date,
        entry_dates,
    })
}

#[route("/phase1/leaderboard", method = "GET", method = "POST")]
async fn fetch_phase1_leaderboard(
    pool: web::Data<MySqlPool>,
    params: web::Json<Phase1LeaderboardParams>,
) -> impl Responder {
    let source_scope = pattern_family_source_scope_label(normalize_pattern_family_source_scope(
        params.source_scope.as_deref(),
    ));
    let period_year = params
        .year
        .filter(|year| (1900..=2200).contains(year))
        .unwrap_or(0);
    let limit = params.limit.unwrap_or(500).clamp(1, 2_000);
    let min_trade_count = params.min_trade_count.unwrap_or(100).clamp(0, 1_000_000);
    let min_setup_count = params.min_setup_count.unwrap_or(1).clamp(0, 1_000_000);
    let best_per_family = params.best_per_family.unwrap_or(false);
    let route_id = params
        .route_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let latest_runs = sqlx::query(
        r#"
        SELECT
            r.family_key,
            r.run_id
        FROM phase1_strategy_runs r
        INNER JOIN (
            SELECT family_key, MAX(id) AS latest_id
            FROM phase1_strategy_runs
            WHERE source_scope = ?
              AND period_year = ?
            GROUP BY family_key
        ) latest_runs
          ON latest_runs.latest_id = r.id
        "#,
    )
    .bind(source_scope)
    .bind(period_year)
    .fetch_all(pool.get_ref())
    .await;

    let latest_by_family = match latest_runs {
        Ok(rows) => rows
            .into_iter()
            .filter_map(|row| {
                let family_key = row.try_get::<String, _>("family_key").ok()?;
                let run_id = row.try_get::<String, _>("run_id").ok()?;
                Some((family_key, run_id))
            })
            .collect::<HashMap<_, _>>(),
        Err(error) if is_missing_table_error(&error) => {
            return HttpResponse::Ok().json(Vec::<Phase1LeaderboardResult>::new());
        }
        Err(error) => {
            eprintln!("Phase 1 latest leaderboard runs DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    if latest_by_family.is_empty() {
        return HttpResponse::Ok().json(Vec::<Phase1LeaderboardResult>::new());
    }

    let candidate_limit = if best_per_family {
        limit.saturating_mul(3).clamp(limit, 5_000)
    } else {
        limit.saturating_mul(3).clamp(limit, 10_000)
    };

    let sql = if best_per_family {
        r#"
        SELECT
            ranked.run_id,
            ranked.family_key,
            ranked.source_scope,
            ranked.period_year,
            ranked.route_id,
            ranked.route_label,
            ranked.result_rank,
            ranked.entry_mode,
            ranked.stop_mode,
            ranked.target_r,
            ranked.max_hold_multiple,
            ranked.setup_count,
            ranked.trade_count,
            ranked.no_entry_count,
            ranked.win_count,
            ranked.loss_count,
            ranked.win_rate,
            ranked.avg_r,
            ranked.profit_factor,
            ranked.max_drawdown_r,
            ranked.worst_year_avg_r,
            ranked.score,
            ranked.created_at,
            ranked.harmonic_type,
            ranked.bin,
            ranked.size_bucket,
            ranked.time_bin,
            ranked.x_strictness
        FROM (
            SELECT
                rr.run_id,
                rr.family_key,
                rr.source_scope,
                CAST(rr.period_year AS SIGNED) AS period_year,
                rr.route_id,
                rr.route_label,
                CAST(rr.result_rank AS SIGNED) AS result_rank,
                rr.entry_mode,
                rr.stop_mode,
                CAST(rr.target_r AS DOUBLE) AS target_r,
                CAST(rr.max_hold_multiple AS SIGNED) AS max_hold_multiple,
                CAST(rr.setup_count AS SIGNED) AS setup_count,
                CAST(rr.trade_count AS SIGNED) AS trade_count,
                CAST(rr.no_entry_count AS SIGNED) AS no_entry_count,
                CAST(rr.win_count AS SIGNED) AS win_count,
                CAST(rr.loss_count AS SIGNED) AS loss_count,
                CAST(rr.win_rate AS DOUBLE) AS win_rate,
                CAST(rr.avg_r AS DOUBLE) AS avg_r,
                CAST(rr.profit_factor AS DOUBLE) AS profit_factor,
                CAST(rr.max_drawdown_r AS DOUBLE) AS max_drawdown_r,
                CAST(rr.worst_year_avg_r AS DOUBLE) AS worst_year_avg_r,
                CAST(rr.score AS DOUBLE) AS score,
                rr.created_at,
                r.harmonic_type,
                r.bin,
                r.size_bucket,
                r.time_bin,
                r.x_strictness,
                ROW_NUMBER() OVER (
                    PARTITION BY rr.family_key
                    ORDER BY rr.score DESC, rr.avg_r DESC, rr.trade_count DESC, rr.result_rank ASC
                ) AS family_route_rank
            FROM phase1_strategy_results rr FORCE INDEX (idx_phase1_results_all_leaderboard)
            INNER JOIN phase1_strategy_runs r
              ON r.run_id = rr.run_id
            INNER JOIN (
                SELECT family_key, MAX(id) AS latest_id
                FROM phase1_strategy_runs
                WHERE source_scope = ?
                  AND period_year = ?
                GROUP BY family_key
            ) latest_runs
              ON latest_runs.latest_id = r.id
            WHERE rr.source_scope = ?
              AND rr.period_year = ?
              AND rr.trade_count >= ?
              AND rr.setup_count >= ?
              AND (? IS NULL OR rr.route_id = ?)
        ) ranked
        WHERE ranked.family_route_rank = 1
        ORDER BY ranked.score DESC, ranked.avg_r DESC, ranked.trade_count DESC, ranked.result_rank ASC
        LIMIT ?
        "#
    } else {
        r#"
        SELECT
            rr.run_id,
            rr.family_key,
            rr.source_scope,
            CAST(rr.period_year AS SIGNED) AS period_year,
            rr.route_id,
            rr.route_label,
            CAST(rr.result_rank AS SIGNED) AS result_rank,
            rr.entry_mode,
            rr.stop_mode,
            CAST(rr.target_r AS DOUBLE) AS target_r,
            CAST(rr.max_hold_multiple AS SIGNED) AS max_hold_multiple,
            CAST(rr.setup_count AS SIGNED) AS setup_count,
            CAST(rr.trade_count AS SIGNED) AS trade_count,
            CAST(rr.no_entry_count AS SIGNED) AS no_entry_count,
            CAST(rr.win_count AS SIGNED) AS win_count,
            CAST(rr.loss_count AS SIGNED) AS loss_count,
            CAST(rr.win_rate AS DOUBLE) AS win_rate,
            CAST(rr.avg_r AS DOUBLE) AS avg_r,
            CAST(rr.profit_factor AS DOUBLE) AS profit_factor,
            CAST(rr.max_drawdown_r AS DOUBLE) AS max_drawdown_r,
            CAST(rr.worst_year_avg_r AS DOUBLE) AS worst_year_avg_r,
            CAST(rr.score AS DOUBLE) AS score,
            rr.created_at,
            r.harmonic_type,
            r.bin,
            r.size_bucket,
            r.time_bin,
            r.x_strictness
        FROM phase1_strategy_results rr FORCE INDEX (idx_phase1_results_all_leaderboard)
        INNER JOIN phase1_strategy_runs r
          ON r.run_id = rr.run_id
        INNER JOIN (
            SELECT family_key, MAX(id) AS latest_id
            FROM phase1_strategy_runs
            WHERE source_scope = ?
              AND period_year = ?
            GROUP BY family_key
        ) latest_runs
          ON latest_runs.latest_id = r.id
        WHERE rr.source_scope = ?
          AND rr.period_year = ?
          AND rr.trade_count >= ?
          AND rr.setup_count >= ?
          AND (? IS NULL OR rr.route_id = ?)
        ORDER BY rr.score DESC, rr.avg_r DESC, rr.trade_count DESC, rr.result_rank ASC
        LIMIT ?
        "#
    };

    let mut query = sqlx::query_as::<_, Phase1LeaderboardResult>(sql)
        .bind(source_scope)
        .bind(period_year)
        .bind(source_scope)
        .bind(period_year)
        .bind(min_trade_count)
        .bind(min_setup_count)
        .bind(route_id)
        .bind(route_id);

    query = query.bind(candidate_limit);

    let rows = query.fetch_all(pool.get_ref()).await;

    match rows {
        Ok(candidate_rows) => {
            let rows = candidate_rows
                .into_iter()
                .filter(|row| latest_by_family.get(&row.family_key) == Some(&row.run_id))
                .take(limit as usize)
                .collect::<Vec<_>>();

            HttpResponse::Ok().json(rows)
        }
        Err(error) if is_missing_table_error(&error) => {
            HttpResponse::Ok().json(Vec::<Phase1LeaderboardResult>::new())
        }
        Err(error) => {
            eprintln!("Phase 1 leaderboard DB error: {:?}", error);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[route("/phase1/supply", method = "GET", method = "POST")]
async fn fetch_phase1_supply(
    pool: web::Data<MySqlPool>,
    params: web::Json<Phase1SupplyParams>,
) -> impl Responder {
    let source_scope = normalize_pattern_family_source_scope(params.source_scope.as_deref());
    let source_scope_label = pattern_family_source_scope_label(source_scope).to_string();
    let period_year = params
        .year
        .filter(|year| (1900..=2200).contains(year))
        .unwrap_or(0);
    let limit = params.limit.unwrap_or(100).clamp(1, 1_000);
    let source_filter = match source_scope {
        PatternFamilySourceScope::Futures => {
            "AND COALESCE(ps.source_table, '') LIKE 'futures_contract_%_candles'"
        }
        PatternFamilySourceScope::Daily => {
            "AND COALESCE(ps.source_table, '') = 'candles' AND COALESCE(ps.source_timeframe, '') = 'daily'"
        }
        PatternFamilySourceScope::All => "",
    };
    let year_filter = if period_year > 0 {
        "AND YEAR(ps.d_date) = ?"
    } else {
        ""
    };

    let symbol_sql = format!(
        r#"
        SELECT
            COALESCE(NULLIF(ps.root_symbol, ''), ps.symbol, 'N/A') AS symbol,
            CAST(COUNT(*) AS SIGNED) AS setup_count,
            CAST(COUNT(DISTINCT COALESCE(NULLIF(ps.pattern_group_id, ''), ps.setup_id)) AS SIGNED) AS pattern_count,
            CAST(COUNT(DISTINCT ps.pattern_family_key) AS SIGNED) AS family_count,
            CAST(COUNT(DISTINCT COALESCE(NULLIF(ps.contract_symbol, ''), ps.symbol, 'N/A')) AS SIGNED) AS contract_count,
            MIN(DATE(ps.d_date)) AS first_d_date,
            MAX(DATE(ps.d_date)) AS last_d_date
        FROM pattern_setups ps
        WHERE ps.d_date IS NOT NULL
          {source_filter}
          {year_filter}
        GROUP BY COALESCE(NULLIF(ps.root_symbol, ''), ps.symbol, 'N/A')
        ORDER BY setup_count DESC, family_count DESC, pattern_count DESC, symbol ASC
        LIMIT ?
        "#,
        source_filter = source_filter,
        year_filter = year_filter,
    );

    let family_sql = format!(
        r#"
        SELECT
            ps.pattern_family_key AS family_key,
            COALESCE(ps.pattern_family_harmonic_type, 'N/A') AS harmonic_type,
            COALESCE(ps.pattern_family_bin, 'N/A') AS bin,
            COALESCE(ps.pattern_family_size_bucket, 'N/A') AS size_bucket,
            COALESCE(ps.pattern_family_time_bin, 'N/A') AS time_bin,
            COALESCE(ps.pattern_family_x_strictness, 'N/A') AS x_strictness,
            CAST(COUNT(*) AS SIGNED) AS setup_count,
            CAST(COUNT(DISTINCT COALESCE(NULLIF(ps.pattern_group_id, ''), ps.setup_id)) AS SIGNED) AS pattern_count,
            CAST(COUNT(DISTINCT COALESCE(NULLIF(ps.root_symbol, ''), ps.symbol, 'N/A')) AS SIGNED) AS symbol_count,
            MIN(DATE(ps.d_date)) AS first_d_date,
            MAX(DATE(ps.d_date)) AS last_d_date
        FROM pattern_setups ps
        WHERE ps.d_date IS NOT NULL
          AND ps.pattern_family_key IS NOT NULL
          AND ps.pattern_family_key <> ''
          {source_filter}
          {year_filter}
        GROUP BY
            ps.pattern_family_key,
            COALESCE(ps.pattern_family_harmonic_type, 'N/A'),
            COALESCE(ps.pattern_family_bin, 'N/A'),
            COALESCE(ps.pattern_family_size_bucket, 'N/A'),
            COALESCE(ps.pattern_family_time_bin, 'N/A'),
            COALESCE(ps.pattern_family_x_strictness, 'N/A')
        ORDER BY setup_count DESC, symbol_count DESC, pattern_count DESC, family_key ASC
        LIMIT ?
        "#,
        source_filter = source_filter,
        year_filter = year_filter,
    );

    let mut symbol_query = sqlx::query_as::<_, Phase1SymbolSupplyRow>(&symbol_sql);
    if period_year > 0 {
        symbol_query = symbol_query.bind(period_year);
    }
    symbol_query = symbol_query.bind(limit);

    let mut family_query = sqlx::query_as::<_, Phase1FamilySupplyRow>(&family_sql);
    if period_year > 0 {
        family_query = family_query.bind(period_year);
    }
    family_query = family_query.bind(limit);

    let symbols = match symbol_query.fetch_all(pool.get_ref()).await {
        Ok(rows) => rows,
        Err(error) if is_missing_table_error(&error) => Vec::new(),
        Err(error) => {
            eprintln!("Phase 1 supply symbol DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };
    let families = match family_query.fetch_all(pool.get_ref()).await {
        Ok(rows) => rows,
        Err(error) if is_missing_table_error(&error) => Vec::new(),
        Err(error) => {
            eprintln!("Phase 1 supply family DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().json(Phase1SupplyResponse {
        source_scope: source_scope_label,
        period_year,
        symbols,
        families,
    })
}

fn phase1_futures_candle_table(source_table: Option<&str>) -> &'static str {
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

async fn fetch_phase1_replay_setups(
    pool: &MySqlPool,
    context: &Phase1YearlyRouteContext,
) -> Result<Vec<Phase1ReplaySetup>, sqlx::Error> {
    let source_filter = match context.source_scope.as_str() {
        "futures" => "AND COALESCE(ps.source_table, '') LIKE 'futures_contract_%_candles'",
        "daily" => {
            "AND COALESCE(ps.source_table, '') = 'candles' AND COALESCE(ps.source_timeframe, '') = 'daily'"
        }
        _ => "",
    };
    let year_filter = if context.period_year > 0 {
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
            CAST(COALESCE(ps.d_confirm_date, ps.d_date) AS DATETIME) AS d_confirm_date,
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

    let mut query = sqlx::query_as::<_, Phase1ReplaySetup>(&sql).bind(&context.family_key);
    if context.period_year > 0 {
        query = query.bind(context.period_year);
    }

    query.bind(context.setup_count.max(1)).fetch_all(pool).await
}

async fn fetch_phase1_replay_setup_for_pattern(
    pool: &MySqlPool,
    context: &Phase1YearlyRouteContext,
    pattern_id: Option<&str>,
    pattern_group_id: Option<&str>,
    d_date: Option<NaiveDateTime>,
) -> Result<Option<Phase1ReplaySetup>, sqlx::Error> {
    let pattern_id = pattern_id.map(str::trim).filter(|value| !value.is_empty());
    let pattern_group_id = pattern_group_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if pattern_id.is_none() && pattern_group_id.is_none() {
        return Ok(None);
    }

    let source_filter = match context.source_scope.as_str() {
        "futures" => "AND COALESCE(ps.source_table, '') LIKE 'futures_contract_%_candles'",
        "daily" => {
            "AND COALESCE(ps.source_table, '') = 'candles' AND COALESCE(ps.source_timeframe, '') = 'daily'"
        }
        _ => "",
    };
    let year_filter = if context.period_year > 0 {
        "AND YEAR(ps.d_date) = ?"
    } else {
        ""
    };
    let d_date_filter = if d_date.is_some() {
        "AND ps.d_date = ?"
    } else {
        ""
    };
    let mut identifier_filters = Vec::new();
    if pattern_id.is_some() {
        identifier_filters.push("ps.pattern_id = ?");
    }
    if pattern_group_id.is_some() {
        identifier_filters.push("ps.pattern_group_id = ?");
    }

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
            CAST(COALESCE(ps.d_confirm_date, ps.d_date) AS DATETIME) AS d_confirm_date,
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
          AND ({identifier_filter})
          {source_filter}
          {year_filter}
          {d_date_filter}
        ORDER BY ps.d_date ASC, ps.setup_id ASC
        LIMIT 1
        "#,
        identifier_filter = identifier_filters.join(" OR "),
        source_filter = source_filter,
        year_filter = year_filter,
        d_date_filter = d_date_filter,
    );

    let mut query = sqlx::query_as::<_, Phase1ReplaySetup>(&sql).bind(&context.family_key);
    if let Some(pattern_id) = pattern_id {
        query = query.bind(pattern_id);
    }
    if let Some(pattern_group_id) = pattern_group_id {
        query = query.bind(pattern_group_id);
    }
    if context.period_year > 0 {
        query = query.bind(context.period_year);
    }
    if let Some(d_date) = d_date {
        query = query.bind(d_date);
    }

    query.fetch_optional(pool).await
}

async fn fetch_phase1_forward_candles(
    pool: &MySqlPool,
    setup: &Phase1ReplaySetup,
    max_forward_bars: i64,
) -> Result<Vec<Phase1ReplayCandle>, sqlx::Error> {
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
        return sqlx::query_as::<_, Phase1ReplayCandle>(
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

    let candle_table = phase1_futures_candle_table(setup.source_table.as_deref());
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

    sqlx::query_as::<_, Phase1ReplayCandle>(&sql)
        .bind(&setup.symbol)
        .bind(setup.d_confirm_date)
        .bind(max_forward_bars.saturating_add(1))
        .fetch_all(pool)
        .await
}

fn phase1_direction(setup: &Phase1ReplaySetup) -> f64 {
    if setup.market.eq_ignore_ascii_case("Bearish") {
        -1.0
    } else {
        1.0
    }
}

fn phase1_break_entry(direction: f64, level: f64, candle: &Phase1ReplayCandle) -> Option<f64> {
    if direction > 0.0 && candle.high >= level {
        Some(level)
    } else if direction < 0.0 && candle.low <= level {
        Some(level)
    } else {
        None
    }
}

fn phase1_forward_start_index(setup: &Phase1ReplaySetup, candles: &[Phase1ReplayCandle]) -> usize {
    candles
        .first()
        .filter(|candle| candle.candle_date <= setup.d_confirm_date)
        .map(|_| 1)
        .unwrap_or(0)
}

fn phase1_find_entry(
    context: &Phase1YearlyRouteContext,
    setup: &Phase1ReplaySetup,
    candles: &[Phase1ReplayCandle],
) -> Option<Phase1EntryDecision> {
    let setup_direction = phase1_direction(setup);
    let start_index = phase1_forward_start_index(setup, candles);
    let entry_scan_limit = candles.len().min(start_index.saturating_add(80));

    match context.entry_mode.as_str() {
        "next_open" => candles.get(start_index).map(|candle| Phase1EntryDecision {
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
                    Some(Phase1EntryDecision {
                        index: start_index,
                        price: decision.open,
                        direction: 1.0,
                    })
                } else if decision.close < setup.d_high {
                    Some(Phase1EntryDecision {
                        index: start_index,
                        price: decision.open,
                        direction: -1.0,
                    })
                } else {
                    None
                }
            } else if decision.close < confirmation.open {
                Some(Phase1EntryDecision {
                    index: start_index,
                    price: decision.open,
                    direction: -1.0,
                })
            } else if decision.close > setup.d_low {
                Some(Phase1EntryDecision {
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
                (signal && entry_valid).then_some(Phase1EntryDecision {
                    index: start_index + 1,
                    price: confirmation_plus_two.open,
                    direction: 1.0,
                })
            } else {
                let signal = confirmation_plus_one.close < confirmation_body_low;
                let entry_valid = confirmation_plus_two.open < confirmation_body_low
                    && confirmation_plus_two.open < plus_one_body_high;
                (signal && entry_valid).then_some(Phase1EntryDecision {
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
                .take(entry_scan_limit.saturating_sub(start_index))
                .enumerate()
                .find_map(|(index, candle)| {
                    phase1_break_entry(setup_direction, level, candle).map(|entry| {
                        Phase1EntryDecision {
                            index: start_index + index,
                            price: entry,
                            direction: setup_direction,
                        }
                    })
                })
        }
        "d_close_confirm" => candles
            .iter()
            .skip(start_index)
            .take(entry_scan_limit.saturating_sub(start_index))
            .enumerate()
            .find_map(|(index, candle)| {
                let confirms = (setup_direction > 0.0 && candle.close > setup.d_close)
                    || (setup_direction < 0.0 && candle.close < setup.d_close);
                confirms.then_some(Phase1EntryDecision {
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
                .take(entry_scan_limit.saturating_sub(start_index))
                .enumerate()
                .find_map(|(index, candle)| {
                    phase1_break_entry(setup_direction, level, candle).map(|entry| {
                        Phase1EntryDecision {
                            index: start_index + index,
                            price: entry,
                            direction: setup_direction,
                        }
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
                .take(entry_scan_limit.saturating_sub(start_index))
                .enumerate()
                .find_map(|(index, candle)| {
                    phase1_break_entry(setup_direction, level, candle).map(|entry| {
                        Phase1EntryDecision {
                            index: start_index + index,
                            price: entry,
                            direction: setup_direction,
                        }
                    })
                })
        }
        _ => None,
    }
}

fn phase1_stop_price(
    context: &Phase1YearlyRouteContext,
    setup: &Phase1ReplaySetup,
    entry_price: f64,
    trade_direction: f64,
) -> f64 {
    match context.stop_mode.as_str() {
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

fn phase1_replay_route(
    context: &Phase1YearlyRouteContext,
    setup: &Phase1ReplaySetup,
    candles: &[Phase1ReplayCandle],
) -> Option<f64> {
    phase1_replay_route_detail(context, setup, candles).map(|trade| trade.result_r)
}

fn phase1_replay_route_detail(
    context: &Phase1YearlyRouteContext,
    setup: &Phase1ReplaySetup,
    candles: &[Phase1ReplayCandle],
) -> Option<Phase1ReplayTradeDetail> {
    let entry = phase1_find_entry(context, setup, candles)?;
    let direction = entry.direction;
    let entry_index = entry.index;
    let entry_price = entry.price;
    let stop_price = phase1_stop_price(context, setup, entry_price, direction);
    let risk = (entry_price - stop_price) * direction;
    if !risk.is_finite() || risk <= 0.0 {
        return None;
    }

    let target_price = entry_price + direction * risk * context.target_r;
    let max_hold_bars = setup
        .full_pattern_length
        .saturating_mul(context.max_hold_multiple.max(1))
        .max(1) as usize;
    let end_index = candles.len().min(entry_index.saturating_add(max_hold_bars));
    if end_index <= entry_index {
        return None;
    }

    let mut lowest_price = f64::INFINITY;
    let mut highest_price = f64::NEG_INFINITY;
    let mut max_adverse_points = 0.0f64;
    let mut max_favorable_points = 0.0f64;
    let mut adverse_price = entry_price;
    let mut favorable_price = entry_price;

    for (offset, candle) in candles[entry_index..end_index].iter().enumerate() {
        lowest_price = lowest_price.min(candle.low);
        highest_price = highest_price.max(candle.high);

        let (adverse_points, candidate_adverse_price) = if direction > 0.0 {
            ((entry_price - candle.low).max(0.0), candle.low)
        } else {
            ((candle.high - entry_price).max(0.0), candle.high)
        };
        if adverse_points > max_adverse_points {
            max_adverse_points = adverse_points;
            adverse_price = candidate_adverse_price;
        }

        let (favorable_points, candidate_favorable_price) = if direction > 0.0 {
            ((candle.high - entry_price).max(0.0), candle.high)
        } else {
            ((entry_price - candle.low).max(0.0), candle.low)
        };
        if favorable_points > max_favorable_points {
            max_favorable_points = favorable_points;
            favorable_price = candidate_favorable_price;
        }

        let candle_index = entry_index + offset;
        let stop_hit = (direction > 0.0 && candle.low <= stop_price)
            || (direction < 0.0 && candle.high >= stop_price);
        if stop_hit {
            return Some(Phase1ReplayTradeDetail {
                result_r: -1.0,
                entry_date: candles[entry_index].candle_date,
                exit_date: candles[candle_index].candle_date,
                exit_price: stop_price,
                entry_price,
                stop_price,
                target_price,
                lowest_price,
                highest_price,
                adverse_price: stop_price,
                favorable_price,
                max_adverse_points: risk.abs().max(max_adverse_points),
                max_favorable_points,
                risk_points: risk.abs(),
                trade_result: 2,
                exit_reason: "stop".to_string(),
            });
        }

        let target_hit = (direction > 0.0 && candle.high >= target_price)
            || (direction < 0.0 && candle.low <= target_price);
        if target_hit {
            return Some(Phase1ReplayTradeDetail {
                result_r: context.target_r,
                entry_date: candles[entry_index].candle_date,
                exit_date: candles[candle_index].candle_date,
                exit_price: target_price,
                entry_price,
                stop_price,
                target_price,
                lowest_price,
                highest_price,
                adverse_price,
                favorable_price: target_price,
                max_adverse_points,
                max_favorable_points: (risk.abs() * context.target_r).max(max_favorable_points),
                risk_points: risk.abs(),
                trade_result: 1,
                exit_reason: "target".to_string(),
            });
        }
    }

    let exit_index = end_index - 1;
    let exit_close = candles[exit_index].close;
    let result_r = ((exit_close - entry_price) * direction) / risk;
    Some(Phase1ReplayTradeDetail {
        result_r,
        entry_date: candles[entry_index].candle_date,
        exit_date: candles[exit_index].candle_date,
        exit_price: exit_close,
        entry_price,
        stop_price,
        target_price,
        lowest_price,
        highest_price,
        adverse_price,
        favorable_price,
        max_adverse_points,
        max_favorable_points,
        risk_points: risk.abs(),
        trade_result: if result_r > 0.0 { 1 } else { 2 },
        exit_reason: "time".to_string(),
    })
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

    fn into_row(self, year: i32) -> Phase1YearlyBreakdownRow {
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

        Phase1YearlyBreakdownRow {
            year,
            setup_count: self.setup_count,
            trade_count: self.trade_count,
            no_entry_count: self.no_entry_count,
            win_count: self.win_count,
            loss_count: self.loss_count,
            win_rate,
            avg_r,
            sum_r: self.sum_r,
            profit_factor,
            max_drawdown_r: self.max_drawdown_r,
        }
    }
}

async fn ensure_phase1_yearly_cache_table(pool: &MySqlPool) -> Result<(), sqlx::Error> {
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

    Ok(())
}

async fn fetch_cached_phase1_yearly_rows(
    pool: &MySqlPool,
    run_id: &str,
    route_id: &str,
) -> Result<Vec<Phase1YearlyBreakdownRow>, sqlx::Error> {
    sqlx::query_as::<_, Phase1YearlyBreakdownRow>(
        r#"
        SELECT
            CAST(trade_year AS SIGNED) AS year,
            CAST(setup_count AS SIGNED) AS setup_count,
            CAST(trade_count AS SIGNED) AS trade_count,
            CAST(no_entry_count AS SIGNED) AS no_entry_count,
            CAST(win_count AS SIGNED) AS win_count,
            CAST(loss_count AS SIGNED) AS loss_count,
            CAST(win_rate AS DOUBLE) AS win_rate,
            CAST(avg_r AS DOUBLE) AS avg_r,
            CAST(sum_r AS DOUBLE) AS sum_r,
            CAST(profit_factor AS DOUBLE) AS profit_factor,
            CAST(max_drawdown_r AS DOUBLE) AS max_drawdown_r
        FROM phase1_strategy_yearly_results
        WHERE run_id = ?
          AND route_id = ?
        ORDER BY trade_year ASC
        "#,
    )
    .bind(run_id)
    .bind(route_id)
    .fetch_all(pool)
    .await
}

async fn save_phase1_yearly_rows(
    pool: &MySqlPool,
    context: &Phase1YearlyRouteContext,
    years: &[Phase1YearlyBreakdownRow],
) -> Result<(), sqlx::Error> {
    for row in years {
        sqlx::query(
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
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE
                setup_count = VALUES(setup_count),
                trade_count = VALUES(trade_count),
                no_entry_count = VALUES(no_entry_count),
                win_count = VALUES(win_count),
                loss_count = VALUES(loss_count),
                win_rate = VALUES(win_rate),
                avg_r = VALUES(avg_r),
                sum_r = VALUES(sum_r),
                profit_factor = VALUES(profit_factor),
                max_drawdown_r = VALUES(max_drawdown_r)
            "#,
        )
        .bind(&context.run_id)
        .bind(&context.family_key)
        .bind(&context.source_scope)
        .bind(context.period_year)
        .bind(&context.route_id)
        .bind(row.year)
        .bind(row.setup_count)
        .bind(row.trade_count)
        .bind(row.no_entry_count)
        .bind(row.win_count)
        .bind(row.loss_count)
        .bind(row.win_rate)
        .bind(row.avg_r)
        .bind(row.sum_r)
        .bind(row.profit_factor)
        .bind(row.max_drawdown_r)
        .execute(pool)
        .await?;
    }

    Ok(())
}

fn phase1_context_route(context: &Phase1YearlyRouteContext) -> Phase1LeaderboardResult {
    Phase1LeaderboardResult {
        run_id: context.run_id.clone(),
        family_key: context.family_key.clone(),
        source_scope: context.source_scope.clone(),
        period_year: context.period_year,
        route_id: context.route_id.clone(),
        route_label: context.route_label.clone(),
        result_rank: context.result_rank,
        entry_mode: context.entry_mode.clone(),
        stop_mode: context.stop_mode.clone(),
        target_r: context.target_r,
        max_hold_multiple: context.max_hold_multiple,
        setup_count: context.setup_count,
        trade_count: context.trade_count,
        no_entry_count: context.no_entry_count,
        win_count: context.win_count,
        loss_count: context.loss_count,
        win_rate: context.win_rate,
        avg_r: context.avg_r,
        profit_factor: context.profit_factor,
        max_drawdown_r: context.max_drawdown_r,
        worst_year_avg_r: context.worst_year_avg_r,
        score: context.score,
        created_at: context.created_at,
        harmonic_type: context.harmonic_type.clone(),
        bin: context.bin.clone(),
        size_bucket: context.size_bucket.clone(),
        time_bin: context.time_bin.clone(),
        x_strictness: context.x_strictness.clone(),
    }
}

async fn fetch_phase1_route_context(
    pool: &MySqlPool,
    family_key: &str,
    run_id: &str,
    route_id: &str,
) -> Result<Option<Phase1YearlyRouteContext>, sqlx::Error> {
    sqlx::query_as::<_, Phase1YearlyRouteContext>(
        r#"
        SELECT
            rr.run_id,
            rr.family_key,
            rr.source_scope,
            CAST(rr.period_year AS SIGNED) AS period_year,
            rr.route_id,
            rr.route_label,
            CAST(rr.result_rank AS SIGNED) AS result_rank,
            rr.entry_mode,
            rr.stop_mode,
            CAST(rr.target_r AS DOUBLE) AS target_r,
            CAST(rr.max_hold_multiple AS SIGNED) AS max_hold_multiple,
            CAST(rr.setup_count AS SIGNED) AS setup_count,
            CAST(rr.trade_count AS SIGNED) AS trade_count,
            CAST(rr.no_entry_count AS SIGNED) AS no_entry_count,
            CAST(rr.win_count AS SIGNED) AS win_count,
            CAST(rr.loss_count AS SIGNED) AS loss_count,
            CAST(rr.win_rate AS DOUBLE) AS win_rate,
            CAST(rr.avg_r AS DOUBLE) AS avg_r,
            CAST(rr.profit_factor AS DOUBLE) AS profit_factor,
            CAST(rr.max_drawdown_r AS DOUBLE) AS max_drawdown_r,
            CAST(rr.worst_year_avg_r AS DOUBLE) AS worst_year_avg_r,
            CAST(rr.score AS DOUBLE) AS score,
            rr.created_at,
            CAST(r.max_forward_bars AS SIGNED) AS max_forward_bars,
            r.harmonic_type,
            r.bin,
            r.size_bucket,
            r.time_bin,
            r.x_strictness
        FROM phase1_strategy_results rr
        INNER JOIN phase1_strategy_runs r
          ON r.run_id = rr.run_id
        WHERE rr.family_key = ?
          AND rr.run_id = ?
          AND rr.route_id = ?
        LIMIT 1
        "#,
    )
    .bind(family_key)
    .bind(run_id)
    .bind(route_id)
    .fetch_optional(pool)
    .await
}

async fn fetch_stored_phase1_route_trades(
    pool: &MySqlPool,
    family_key: &str,
    run_id: &str,
    route_id: &str,
) -> Result<Vec<SimulatorReplaySourceTrade>, sqlx::Error> {
    sqlx::query_as::<_, SimulatorReplaySourceTrade>(
        r#"
        SELECT
            CAST(id AS SIGNED) AS trade_id,
            trade_uid,
            trade_direction,
            symbol,
            pattern_id,
            pattern_group_id,
            d_date,
            d_confirm_date,
            CAST(NULL AS DATETIME) AS reversal_detect_date,
            entry_date,
            exit_date AS target_date,
            CAST(entry_price AS DOUBLE) AS trade_enter_price,
            CAST(stop_price AS DOUBLE) AS trade_risk_exit_price,
            CAST(target_price AS DOUBLE) AS trade_reward_exit_price,
            CAST(exit_price AS DOUBLE) AS exit_price,
            CAST(result_r AS DOUBLE) AS result_r,
            CAST(risk_points AS DOUBLE) AS risk_points,
            CAST(lowest_price AS DOUBLE) AS trade_lowest_price,
            CAST(highest_price AS DOUBLE) AS trade_highest_price,
            CAST(adverse_price AS DOUBLE) AS trade_adverse_price,
            CAST(favorable_price AS DOUBLE) AS trade_favorable_price,
            CAST(max_adverse_points AS DOUBLE) AS max_adverse_points,
            CAST(max_favorable_points AS DOUBLE) AS max_favorable_points,
            CAST(trade_result AS SIGNED) AS trade_result,
            exit_reason
        FROM phase1_strategy_trades
        WHERE family_key = ?
          AND run_id = ?
          AND route_id = ?
        ORDER BY entry_date ASC, id ASC
        "#,
    )
    .bind(family_key)
    .bind(run_id)
    .bind(route_id)
    .fetch_all(pool)
    .await
}

async fn fetch_stored_phase1_pattern_route_trade(
    pool: &MySqlPool,
    family_key: &str,
    run_id: &str,
    route_id: &str,
    pattern_id: Option<&str>,
    pattern_group_id: Option<&str>,
) -> Result<Vec<SimulatorReplaySourceTrade>, sqlx::Error> {
    let pattern_id = pattern_id.map(str::trim).filter(|value| !value.is_empty());
    let pattern_group_id = pattern_group_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if pattern_id.is_none() && pattern_group_id.is_none() {
        return Ok(Vec::new());
    }

    let mut filters = Vec::new();
    if pattern_id.is_some() {
        filters.push("pattern_id = ?");
    }
    if pattern_group_id.is_some() {
        filters.push("pattern_group_id = ?");
    }

    let sql = format!(
        r#"
        SELECT
            CAST(id AS SIGNED) AS trade_id,
            trade_uid,
            trade_direction,
            symbol,
            pattern_id,
            pattern_group_id,
            d_date,
            d_confirm_date,
            CAST(NULL AS DATETIME) AS reversal_detect_date,
            entry_date,
            exit_date AS target_date,
            CAST(entry_price AS DOUBLE) AS trade_enter_price,
            CAST(stop_price AS DOUBLE) AS trade_risk_exit_price,
            CAST(target_price AS DOUBLE) AS trade_reward_exit_price,
            CAST(exit_price AS DOUBLE) AS exit_price,
            CAST(result_r AS DOUBLE) AS result_r,
            CAST(risk_points AS DOUBLE) AS risk_points,
            CAST(lowest_price AS DOUBLE) AS trade_lowest_price,
            CAST(highest_price AS DOUBLE) AS trade_highest_price,
            CAST(adverse_price AS DOUBLE) AS trade_adverse_price,
            CAST(favorable_price AS DOUBLE) AS trade_favorable_price,
            CAST(max_adverse_points AS DOUBLE) AS max_adverse_points,
            CAST(max_favorable_points AS DOUBLE) AS max_favorable_points,
            CAST(trade_result AS SIGNED) AS trade_result,
            exit_reason
        FROM phase1_strategy_trades
        WHERE family_key = ?
          AND run_id = ?
          AND route_id = ?
          AND ({})
        ORDER BY entry_date ASC, id ASC
        LIMIT 1
        "#,
        filters.join(" OR "),
    );

    let mut query = sqlx::query_as::<_, SimulatorReplaySourceTrade>(&sql)
        .bind(family_key)
        .bind(run_id)
        .bind(route_id);
    if let Some(pattern_id) = pattern_id {
        query = query.bind(pattern_id);
    }
    if let Some(pattern_group_id) = pattern_group_id {
        query = query.bind(pattern_group_id);
    }

    query.fetch_all(pool).await
}

#[route("/simulator/phase1-route-replay", method = "GET", method = "POST")]
async fn fetch_simulator_phase1_route_replay(
    pool: web::Data<MySqlPool>,
    params: web::Json<Phase1SimulatorReplayParams>,
) -> impl Responder {
    let Some(family_key) = params
        .family_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return HttpResponse::BadRequest().body("Phase 1 replay requires family_key");
    };
    let Some(run_id) = params
        .run_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return HttpResponse::BadRequest().body("Phase 1 replay requires run_id");
    };
    let Some(route_id) = params
        .route_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return HttpResponse::BadRequest().body("Phase 1 replay requires route_id");
    };
    let Some(first_start_date) = parse_simulator_start_date(params.first_start_date.as_deref())
    else {
        return HttpResponse::BadRequest().body("Phase 1 replay requires first start date");
    };

    let tests_to_chain = params.tests_to_chain.unwrap_or(1).clamp(1, 250);
    let contracts = params.contracts.unwrap_or(1).clamp(1, 500);
    let starting_balance = params.starting_balance.unwrap_or(50_000.0).max(0.0);
    let profit_target = params.profit_target.unwrap_or(3_000.0).max(0.0);
    let max_drawdown = params.max_drawdown.unwrap_or(2_000.0).max(0.0);
    let daily_loss_limit = params.daily_loss_limit.filter(|value| *value > 0.0);
    let drawdown_model = normalize_simulator_drawdown_model(params.drawdown_model.as_deref());
    let one_trade_at_a_time = params.one_trade_at_a_time.unwrap_or(true);

    let context =
        match fetch_phase1_route_context(pool.get_ref(), family_key, run_id, route_id).await {
            Ok(Some(context)) => context,
            Ok(None) => return HttpResponse::NotFound().body("Phase 1 route not found"),
            Err(error) if is_missing_table_error(&error) => {
                return HttpResponse::Ok().json(SimulatorReplayResponse {
                    family_key: family_key.to_string(),
                    tests: Vec::new(),
                    trades: Vec::new(),
                    eligible_trade_count: 0,
                    candidate_logic_applied: false,
                    candidate_logic_filters: Vec::new(),
                });
            }
            Err(error) => {
                eprintln!("Phase 1 simulator route DB error: {:?}", error);
                return HttpResponse::InternalServerError().finish();
            }
        };

    match fetch_stored_phase1_route_trades(pool.get_ref(), family_key, run_id, route_id).await {
        Ok(rows) if !rows.is_empty() => {
            return HttpResponse::Ok().json(build_simulator_replay_response(
                family_key,
                &rows,
                first_start_date,
                tests_to_chain,
                contracts,
                starting_balance,
                profit_target,
                max_drawdown,
                daily_loss_limit,
                drawdown_model,
                one_trade_at_a_time,
                false,
                vec![context.route_label],
            ));
        }
        Ok(_) => {}
        Err(error) if is_missing_table_error(&error) => {}
        Err(error) => {
            eprintln!("Phase 1 stored trade DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    }

    let setups = match fetch_phase1_replay_setups(pool.get_ref(), &context).await {
        Ok(setups) => setups,
        Err(error) if is_missing_table_error(&error) => Vec::new(),
        Err(error) => {
            eprintln!("Phase 1 simulator setup DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let mut rows = Vec::new();
    for setup in &setups {
        let setup_forward_bars = setup.full_pattern_length.saturating_mul(5).max(1);
        let candles = fetch_phase1_forward_candles(pool.get_ref(), setup, setup_forward_bars)
            .await
            .unwrap_or_else(|error| {
                eprintln!(
                    "Phase 1 simulator candle fetch failed for setup {} ({}): {:?}",
                    setup.setup_id, setup.symbol, error
                );
                Vec::new()
            });

        let Some(detail) = phase1_replay_route_detail(&context, setup, &candles) else {
            continue;
        };
        if !detail.result_r.is_finite() {
            continue;
        }

        rows.push(SimulatorReplaySourceTrade {
            trade_id: None,
            trade_uid: None,
            trade_direction: None,
            symbol: setup.symbol.clone(),
            pattern_id: setup
                .pattern_id
                .clone()
                .or_else(|| Some(setup.setup_id.clone())),
            pattern_group_id: setup.pattern_group_id.clone(),
            d_date: setup.d_date,
            d_confirm_date: Some(setup.d_confirm_date),
            reversal_detect_date: None,
            entry_date: detail.entry_date,
            target_date: Some(detail.exit_date),
            trade_enter_price: detail.entry_price,
            trade_risk_exit_price: detail.stop_price,
            trade_reward_exit_price: detail.target_price,
            exit_price: Some(detail.exit_price),
            result_r: Some(detail.result_r),
            risk_points: Some(detail.risk_points),
            trade_lowest_price: Some(detail.lowest_price),
            trade_highest_price: Some(detail.highest_price),
            trade_adverse_price: Some(detail.adverse_price),
            trade_favorable_price: Some(detail.favorable_price),
            max_adverse_points: Some(detail.max_adverse_points),
            max_favorable_points: Some(detail.max_favorable_points),
            trade_result: detail.trade_result,
            exit_reason: Some(detail.exit_reason),
        });
    }

    rows.sort_by(|left, right| {
        left.entry_date
            .cmp(&right.entry_date)
            .then_with(|| left.target_date.cmp(&right.target_date))
            .then_with(|| left.symbol.cmp(&right.symbol))
    });

    HttpResponse::Ok().json(build_simulator_replay_response(
        family_key,
        &rows,
        first_start_date,
        tests_to_chain,
        contracts,
        starting_balance,
        profit_target,
        max_drawdown,
        daily_loss_limit,
        drawdown_model,
        one_trade_at_a_time,
        false,
        vec![context.route_label],
    ))
}

#[route(
    "/simulator/phase1-pattern-route-replay",
    method = "GET",
    method = "POST"
)]
async fn fetch_simulator_phase1_pattern_route_replay(
    pool: web::Data<MySqlPool>,
    params: web::Json<Phase1PatternRouteReplayParams>,
) -> impl Responder {
    let Some(family_key) = params
        .family_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return HttpResponse::BadRequest().body("Phase 1 pattern replay requires family_key");
    };
    let Some(run_id) = params
        .run_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return HttpResponse::BadRequest().body("Phase 1 pattern replay requires run_id");
    };
    let Some(route_id) = params
        .route_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return HttpResponse::BadRequest().body("Phase 1 pattern replay requires route_id");
    };

    let context =
        match fetch_phase1_route_context(pool.get_ref(), family_key, run_id, route_id).await {
            Ok(Some(context)) => context,
            Ok(None) => return HttpResponse::NotFound().body("Phase 1 route not found"),
            Err(error) if is_missing_table_error(&error) => {
                return HttpResponse::Ok().json(SimulatorReplayResponse {
                    family_key: family_key.to_string(),
                    tests: Vec::new(),
                    trades: Vec::new(),
                    eligible_trade_count: 0,
                    candidate_logic_applied: false,
                    candidate_logic_filters: Vec::new(),
                });
            }
            Err(error) => {
                eprintln!("Phase 1 pattern route DB error: {:?}", error);
                return HttpResponse::InternalServerError().finish();
            }
        };

    match fetch_stored_phase1_pattern_route_trade(
        pool.get_ref(),
        family_key,
        run_id,
        route_id,
        params.pattern_id.as_deref(),
        params.pattern_group_id.as_deref(),
    )
    .await
    {
        Ok(rows) if !rows.is_empty() => {
            let first_start_date = rows[0].entry_date;
            let contracts = params.contracts.unwrap_or(1).clamp(1, 500);
            return HttpResponse::Ok().json(build_simulator_replay_response(
                family_key,
                &rows,
                first_start_date,
                1,
                contracts,
                50_000.0,
                3_000.0,
                2_000.0,
                None,
                SIMULATOR_DRAWDOWN_MODEL_INTRADAY,
                false,
                false,
                vec![context.route_label],
            ));
        }
        Ok(_) => {}
        Err(error) if is_missing_table_error(&error) => {}
        Err(error) => {
            eprintln!("Phase 1 stored pattern trade DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    }

    let setup = match fetch_phase1_replay_setup_for_pattern(
        pool.get_ref(),
        &context,
        params.pattern_id.as_deref(),
        params.pattern_group_id.as_deref(),
        params.d_date,
    )
    .await
    {
        Ok(Some(setup)) => setup,
        Ok(None) => {
            return HttpResponse::Ok().json(SimulatorReplayResponse {
                family_key: family_key.to_string(),
                tests: Vec::new(),
                trades: Vec::new(),
                eligible_trade_count: 0,
                candidate_logic_applied: false,
                candidate_logic_filters: vec![String::from("Selected pattern was not found")],
            });
        }
        Err(error) if is_missing_table_error(&error) => {
            return HttpResponse::Ok().json(SimulatorReplayResponse {
                family_key: family_key.to_string(),
                tests: Vec::new(),
                trades: Vec::new(),
                eligible_trade_count: 0,
                candidate_logic_applied: false,
                candidate_logic_filters: Vec::new(),
            });
        }
        Err(error) => {
            eprintln!("Phase 1 selected pattern setup DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let setup_forward_bars = setup.full_pattern_length.saturating_mul(5).max(1);
    let candles =
        match fetch_phase1_forward_candles(pool.get_ref(), &setup, setup_forward_bars).await {
            Ok(candles) => candles,
            Err(error) => {
                eprintln!(
                    "Phase 1 selected pattern candle fetch failed for setup {} ({}): {:?}",
                    setup.setup_id, setup.symbol, error
                );
                Vec::new()
            }
        };

    let mut rows = Vec::new();
    if let Some(detail) = phase1_replay_route_detail(&context, &setup, &candles) {
        if detail.result_r.is_finite() {
            rows.push(SimulatorReplaySourceTrade {
                trade_id: None,
                trade_uid: None,
                trade_direction: None,
                symbol: setup.symbol.clone(),
                pattern_id: setup
                    .pattern_id
                    .clone()
                    .or_else(|| Some(setup.setup_id.clone())),
                pattern_group_id: setup.pattern_group_id.clone(),
                d_date: setup.d_date,
                d_confirm_date: Some(setup.d_confirm_date),
                reversal_detect_date: None,
                entry_date: detail.entry_date,
                target_date: Some(detail.exit_date),
                trade_enter_price: detail.entry_price,
                trade_risk_exit_price: detail.stop_price,
                trade_reward_exit_price: detail.target_price,
                exit_price: Some(detail.exit_price),
                result_r: Some(detail.result_r),
                risk_points: Some(detail.risk_points),
                trade_lowest_price: Some(detail.lowest_price),
                trade_highest_price: Some(detail.highest_price),
                trade_adverse_price: Some(detail.adverse_price),
                trade_favorable_price: Some(detail.favorable_price),
                max_adverse_points: Some(detail.max_adverse_points),
                max_favorable_points: Some(detail.max_favorable_points),
                trade_result: detail.trade_result,
                exit_reason: Some(detail.exit_reason),
            });
        }
    }

    let first_start_date = rows
        .first()
        .map(|trade| trade.entry_date)
        .unwrap_or(setup.d_confirm_date);
    let contracts = params.contracts.unwrap_or(1).clamp(1, 500);

    HttpResponse::Ok().json(build_simulator_replay_response(
        family_key,
        &rows,
        first_start_date,
        1,
        contracts,
        50_000.0,
        3_000.0,
        2_000.0,
        None,
        SIMULATOR_DRAWDOWN_MODEL_INTRADAY,
        false,
        false,
        vec![context.route_label],
    ))
}

#[route("/phase1/yearly-breakdown", method = "GET", method = "POST")]
async fn fetch_phase1_yearly_breakdown(
    pool: web::Data<MySqlPool>,
    params: web::Json<Phase1YearlyParams>,
) -> impl Responder {
    let Some(family_key) = params
        .family_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return HttpResponse::BadRequest().body("Missing family_key");
    };
    let Some(run_id) = params
        .run_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return HttpResponse::BadRequest().body("Missing run_id");
    };
    let Some(route_id) = params
        .route_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return HttpResponse::BadRequest().body("Missing route_id");
    };

    let context = match sqlx::query_as::<_, Phase1YearlyRouteContext>(
        r#"
        SELECT
            rr.run_id,
            rr.family_key,
            rr.source_scope,
            CAST(rr.period_year AS SIGNED) AS period_year,
            rr.route_id,
            rr.route_label,
            CAST(rr.result_rank AS SIGNED) AS result_rank,
            rr.entry_mode,
            rr.stop_mode,
            CAST(rr.target_r AS DOUBLE) AS target_r,
            CAST(rr.max_hold_multiple AS SIGNED) AS max_hold_multiple,
            CAST(rr.setup_count AS SIGNED) AS setup_count,
            CAST(rr.trade_count AS SIGNED) AS trade_count,
            CAST(rr.no_entry_count AS SIGNED) AS no_entry_count,
            CAST(rr.win_count AS SIGNED) AS win_count,
            CAST(rr.loss_count AS SIGNED) AS loss_count,
            CAST(rr.win_rate AS DOUBLE) AS win_rate,
            CAST(rr.avg_r AS DOUBLE) AS avg_r,
            CAST(rr.profit_factor AS DOUBLE) AS profit_factor,
            CAST(rr.max_drawdown_r AS DOUBLE) AS max_drawdown_r,
            CAST(rr.worst_year_avg_r AS DOUBLE) AS worst_year_avg_r,
            CAST(rr.score AS DOUBLE) AS score,
            rr.created_at,
            CAST(r.max_forward_bars AS SIGNED) AS max_forward_bars,
            r.harmonic_type,
            r.bin,
            r.size_bucket,
            r.time_bin,
            r.x_strictness
        FROM phase1_strategy_results rr
        INNER JOIN phase1_strategy_runs r
          ON r.run_id = rr.run_id
        WHERE rr.family_key = ?
          AND rr.run_id = ?
          AND rr.route_id = ?
        LIMIT 1
        "#,
    )
    .bind(family_key)
    .bind(run_id)
    .bind(route_id)
    .fetch_optional(pool.get_ref())
    .await
    {
        Ok(Some(context)) => context,
        Ok(None) => return HttpResponse::NotFound().body("Phase 1 route not found"),
        Err(error) if is_missing_table_error(&error) => {
            return HttpResponse::Ok().json(Phase1YearlyBreakdownResponse {
                route: Phase1LeaderboardResult {
                    run_id: run_id.to_string(),
                    family_key: family_key.to_string(),
                    source_scope: "futures".to_string(),
                    period_year: 0,
                    route_id: route_id.to_string(),
                    route_label: String::new(),
                    result_rank: 0,
                    entry_mode: String::new(),
                    stop_mode: String::new(),
                    target_r: 0.0,
                    max_hold_multiple: 0,
                    setup_count: 0,
                    trade_count: 0,
                    no_entry_count: 0,
                    win_count: 0,
                    loss_count: 0,
                    win_rate: 0.0,
                    avg_r: 0.0,
                    profit_factor: 0.0,
                    max_drawdown_r: 0.0,
                    worst_year_avg_r: 0.0,
                    score: 0.0,
                    created_at: None,
                    harmonic_type: String::new(),
                    bin: String::new(),
                    size_bucket: String::new(),
                    time_bin: String::new(),
                    x_strictness: String::new(),
                },
                years: Vec::new(),
                cached: false,
            });
        }
        Err(error) => {
            eprintln!("Phase 1 yearly route DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    if let Err(error) = ensure_phase1_yearly_cache_table(pool.get_ref()).await {
        eprintln!("Phase 1 yearly cache table DB error: {:?}", error);
        return HttpResponse::InternalServerError().finish();
    }

    match fetch_cached_phase1_yearly_rows(pool.get_ref(), &context.run_id, &context.route_id).await
    {
        Ok(rows) if !rows.is_empty() => {
            return HttpResponse::Ok().json(Phase1YearlyBreakdownResponse {
                route: phase1_context_route(&context),
                years: rows,
                cached: true,
            });
        }
        Ok(_) => {}
        Err(error) if is_missing_table_error(&error) => {}
        Err(error) => {
            eprintln!("Phase 1 yearly cache read DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    }

    if params.cache_only.unwrap_or(false) {
        return HttpResponse::Ok().json(Phase1YearlyBreakdownResponse {
            route: phase1_context_route(&context),
            years: Vec::new(),
            cached: false,
        });
    }

    let setups = match fetch_phase1_replay_setups(pool.get_ref(), &context).await {
        Ok(setups) => setups,
        Err(error) if is_missing_table_error(&error) => Vec::new(),
        Err(error) => {
            eprintln!("Phase 1 yearly setup DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let mut yearly: HashMap<i32, Phase1YearlyAccumulator> = HashMap::new();
    for setup in &setups {
        let year = setup.d_date.year();
        let accumulator = yearly.entry(year).or_default();
        let setup_forward_bars = setup.full_pattern_length.saturating_mul(5).max(1);
        let candles = fetch_phase1_forward_candles(pool.get_ref(), setup, setup_forward_bars)
            .await
            .unwrap_or_else(|error| {
                eprintln!(
                    "Phase 1 yearly candle fetch failed for setup {} ({}): {:?}",
                    setup.setup_id, setup.symbol, error
                );
                Vec::new()
            });

        if candles.is_empty() {
            accumulator.record_no_entry();
            continue;
        }

        match phase1_replay_route(&context, setup, &candles) {
            Some(result_r) if result_r.is_finite() => accumulator.record_trade(result_r),
            _ => accumulator.record_no_entry(),
        }
    }

    let mut years = yearly
        .into_iter()
        .map(|(year, accumulator)| accumulator.into_row(year))
        .collect::<Vec<_>>();
    years.sort_by_key(|row| row.year);

    if let Err(error) = save_phase1_yearly_rows(pool.get_ref(), &context, &years).await {
        eprintln!("Phase 1 yearly cache write DB error: {:?}", error);
    }

    HttpResponse::Ok().json(Phase1YearlyBreakdownResponse {
        route: phase1_context_route(&context),
        years,
        cached: false,
    })
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
