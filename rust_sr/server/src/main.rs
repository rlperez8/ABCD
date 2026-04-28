use actix_cors::Cors;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
mod peformance;
mod utils;
use crate::peformance::{MonthlySummary, YearlySummary};
mod pattern;
use crate::pattern::Pattern;
use actix_web::middleware::Logger;
mod models;
use crate::candles::Candle;
use crate::models::*;
use actix_web::dev::Service;
use actix_web::route;
use rust_decimal::Decimal;
use sqlx::MySqlPool;

#[derive(Serialize)]
struct PatternsResponse {
    patterns: Vec<Pattern>,
    total_count: i64,
    has_more: bool,
    monthly_stats: Vec<MonthlySummary>,
    yearly_summary: Vec<YearlySummary>,
}

#[derive(Serialize)]
struct PatternSummariesResponse {
    patterns: Vec<PatternSummary>,
    total_count: i64,
    has_more: bool,
}

#[derive(Debug, serde::Deserialize)]
struct FilterParams {
    pub bin: Option<String>,
    pub harmonic_type: Option<String>,
    pub size_bucket: Option<String>,
    pub balance_bucket: Option<String>,
    pub reversal_type: Option<String>,
    pub reversal_signal: Option<String>,
    pub market: Option<String>,
    pub trade_result: Option<i32>,
    pub retracement: Option<RetracementFilter>,
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

#[derive(Debug, Clone, serde::Deserialize)]
struct RetracementFilter {
    pub ab_xa_gr: f64,
    pub ab_xa_lt: f64,
    pub bc_ab_gr: f64,
    pub bc_ab_lt: f64,
    pub cd_bc_gr: f64,
    pub cd_bc_lt: f64,
    pub cd_xa_gr: f64,
    pub cd_xa_lt: f64,
}

#[derive(Debug, serde::Deserialize)]
struct AccuracyParams {
    pub market: Option<String>,
    pub trade_result: Option<i32>,
    pub retracement: Option<RetracementFilter>,
    pub harmonic_type: Option<String>,
    pub size_bucket: Option<String>,
    pub balance_bucket: Option<String>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct PatternStructureSummary {
    total_count: i64,
    closed_count: i64,
    open_count: i64,
    avg_total_bars: f64,
    avg_balance_ratio: f64,
    distorted_count: i64,
    distorted_rate: f64,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct PatternStructureBucket {
    bucket: String,
    bucket_order: i64,
    total_count: i64,
    closed_count: i64,
    expectancy: f64,
    win_rate: f64,
    avg_total_bars: f64,
    avg_balance_ratio: f64,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct PatternStructureGridCell {
    size_bucket: String,
    size_order: i64,
    balance_bucket: String,
    balance_order: i64,
    total_count: i64,
    closed_count: i64,
    expectancy: f64,
    win_rate: f64,
}

#[derive(serde::Serialize)]
struct PatternStructureResponse {
    summary: PatternStructureSummary,
    size_buckets: Vec<PatternStructureBucket>,
    balance_buckets: Vec<PatternStructureBucket>,
    size_balance_grid: Vec<PatternStructureGridCell>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct PatternReversalSummary {
    total_count: i64,
    closed_count: i64,
    confirmed_count: i64,
    confirmation_rate: f64,
    avg_return: f64,
    win_rate: f64,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct PatternReversalTypeBucket {
    signal_key: String,
    reversal_type: String,
    bucket_order: i64,
    total_count: i64,
    closed_count: i64,
    expectancy: f64,
    win_rate: f64,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct PatternReversalSignalBucket {
    signal_key: String,
    signal_label: String,
    signal_order: i64,
    total_count: i64,
    closed_count: i64,
    expectancy: f64,
    win_rate: f64,
}

#[derive(serde::Serialize)]
struct PatternReversalResponse {
    summary: PatternReversalSummary,
    reversal_types: Vec<PatternReversalTypeBucket>,
    signal_buckets: Vec<PatternReversalSignalBucket>,
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
    pub prop_mode: Option<bool>,
    pub prop_outcome_mode: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct StrategyCandidateParams {
    pub min_closed_trades: Option<i64>,
    pub min_expectancy: Option<f64>,
    pub max_down_years: Option<i64>,
    pub min_worst_year_expectancy: Option<f64>,
    pub min_score: Option<f64>,
    pub limit: Option<i64>,
    pub prop_mode: Option<bool>,
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
    pub harmonic_type: Option<String>,
    pub bin: Option<String>,
    pub market: Option<String>,
    pub reversal_type: Option<String>,
    pub size_bucket: Option<String>,
    pub time_bin: Option<String>,
    pub three_month_trend: Option<String>,
    pub six_month_trend: Option<String>,
    pub twelve_month_trend: Option<String>,
    pub include_count: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub prop_mode: Option<bool>,
    pub prop_outcome_mode: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct PatternDetailParams {
    pub symbol: Option<String>,
    pub pattern_id: Option<String>,
    pub pattern_group_id: String,
    pub x_date: Option<NaiveDate>,
    pub d_date: Option<NaiveDate>,
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
    pub prop_outcome_mode: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
struct PatternSummary {
    pub symbol: String,
    pub d_date: NaiveDate,
    pub d_confirm_date: Option<NaiveDate>,
    pub reversal_detect_date: Option<NaiveDate>,
    pub target_date: Option<NaiveDate>,
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
    d_date: NaiveDate,
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
pub struct Params {
    symbol: String,
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

fn parse_bin_range(bin: Option<&str>) -> Option<(f64, f64)> {
    bin.and_then(|value| value.split_once('-'))
        .map(|(min, max)| {
            (
                min.parse::<f64>().unwrap_or(0.0),
                max.parse::<f64>().unwrap_or(100.0),
            )
        })
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

fn is_supported_balance_bucket(bucket: &str) -> bool {
    matches!(
        bucket,
        "Unknown" | "Tight" | "Balanced" | "Stretched" | "Distorted"
    )
}

fn is_supported_reversal_signal(signal: &str) -> bool {
    matches!(
        signal,
        "bullish_key_reversal"
            | "bearish_key_reversal"
            | "bullish_engulfing"
            | "bearish_engulfing"
            | "bullish_outside_reversal"
            | "bearish_outside_reversal"
            | "hammer"
            | "shooting_star"
            | "morning_star"
            | "evening_star"
            | "three_white_soldiers"
            | "three_black_crows"
    )
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

fn prop_family_sort_column(sort_by: Option<&str>, alias: &str) -> String {
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

async fn try_fetch_setup_comparison_from_strategy_rollup(
    pool: &MySqlPool,
    params: &SetupComparisonParams,
) -> Result<Option<(SetupComparisonSummary, Vec<SetupComparisonYearlyPoint>)>, sqlx::Error> {
    if !is_rollup_cache_ready(pool, "structure_rollups").await?
        || !is_rollup_cache_ready(pool, "strategy_live_caches").await?
    {
        return Ok(None);
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

    if !is_supported_bin_label(bin) {
        return Ok(None);
    }

    if let Some(time_bin) = params.time_bin.as_deref() {
        if !is_supported_bin_label(time_bin) {
            return Ok(None);
        }
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
        FROM strategy_cohort_summary_cache
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
        FROM strategy_yearly_rollup
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

    let summary_query = sqlx::query_as::<_, SetupComparisonSummary>(&summary_sql)
        .bind(market)
        .bind(harmonic_type)
        .bind(bin)
        .bind(reversal_type)
        .bind(size_bucket)
        .bind(time_bin)
        .bind(three_month_trend)
        .bind(six_month_trend)
        .bind(twelve_month_trend);

    let yearly_query = sqlx::query_as::<_, SetupComparisonYearlyPoint>(&yearly_sql)
        .bind(market)
        .bind(harmonic_type)
        .bind(bin)
        .bind(reversal_type)
        .bind(size_bucket)
        .bind(time_bin)
        .bind(three_month_trend)
        .bind(six_month_trend)
        .bind(twelve_month_trend);

    let summary = match summary_query.fetch_one(pool).await {
        Ok(row) => row,
        Err(error) if is_missing_table_error(&error) => return Ok(None),
        Err(error) => return Err(error),
    };

    if summary.total_count == 0 {
        return Ok(None);
    }

    let yearly_performance = match yearly_query.fetch_all(pool).await {
        Ok(rows) => rows,
        Err(error) if is_missing_table_error(&error) => return Ok(None),
        Err(error) => return Err(error),
    };

    Ok(Some((summary, yearly_performance)))
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

async fn try_fetch_setup_comparison_from_prop_reversal_rollup(
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
                0.0 AS avg_trade_length,
                0.0 AS avg_ab_xa,
                0.0 AS avg_bc_ab,
                0.0 AS avg_cd_bc,
                0.0 AS avg_cd_xa
            FROM prop_reversal_strategy_summary
            WHERE prop_strategy_id = ?
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
            FROM prop_reversal_strategy_yearly
            WHERE prop_strategy_id = ?
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
            0.0 AS avg_trade_length,
            0.0 AS avg_ab_xa,
            0.0 AS avg_bc_ab,
            0.0 AS avg_cd_bc,
            0.0 AS avg_cd_xa
        FROM prop_reversal_strategy_summary
        WHERE market = ?
          AND harmonic_type = ?
          AND bin = ?
          AND reversal_type = ?
          AND size_bucket = ?
          AND time_bin = ?
          AND three_month_trend = ?
          AND six_month_trend = ?
          AND twelve_month_trend = ?
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
        FROM prop_reversal_strategy_yearly
        WHERE market = ?
          AND harmonic_type = ?
          AND bin = ?
          AND reversal_type = ?
          AND size_bucket = ?
          AND time_bin = ?
          AND three_month_trend = ?
          AND six_month_trend = ?
          AND twelve_month_trend = ?
        GROUP BY trade_year
        ORDER BY trade_year ASC
    "#;

    let summary = match sqlx::query_as::<_, SetupComparisonSummary>(summary_sql)
        .bind(market)
        .bind(harmonic_type)
        .bind(bin)
        .bind(reversal_type)
        .bind(size_bucket)
        .bind(time_bin)
        .bind(three_month_trend)
        .bind(six_month_trend)
        .bind(twelve_month_trend)
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
    let prop_mode = params.prop_mode.unwrap_or(false);

    if prop_mode {
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
        let family = match fetch_prop_strategy_family_filter(pool.get_ref(), prop_strategy_id).await
        {
            Ok(Some(family)) => family,
            Ok(None) => return HttpResponse::NotFound().body("Prop strategy family not found"),
            Err(error) => {
                eprintln!("Prop strategy family lookup DB error: {:?}", error);
                return HttpResponse::InternalServerError().finish();
            }
        };

        let route_x_strictness_expr = x_strictness_expr("p.x_bars_left", "p.x_length");
        let family_where_clause = format!(
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
            ORDER BY COALESCE(p.reversal_detect_date, p.d_confirm_date, p.d_date) DESC,
                     p.trade_enter_price ASC
            LIMIT ? OFFSET ?
            "#,
            route_time_accuracy_expr = route_time_accuracy_expr,
            route_score_projection = route_score_projection,
            route_x_strictness_expr_for_select = x_strictness_expr("p.x_bars_left", "p.x_length"),
            family_where_clause = family_where_clause,
        );

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
        });
    }

    let Some(harmonic_type) = params.harmonic_type.as_deref() else {
        return HttpResponse::BadRequest().body("Missing harmonic type");
    };
    let Some(market) = params.market.as_deref() else {
        return HttpResponse::BadRequest().body("Missing market");
    };
    let Some(bin) = params.bin.as_deref() else {
        return HttpResponse::BadRequest().body("Missing bin");
    };
    let Some(reversal_type) = params.reversal_type.as_deref() else {
        return HttpResponse::BadRequest().body("Missing reversal type");
    };
    let Some(size_bucket) = params.size_bucket.as_deref() else {
        return HttpResponse::BadRequest().body("Missing size bucket");
    };
    let Some(time_bin) = params.time_bin.as_deref() else {
        return HttpResponse::BadRequest().body("Missing time bin");
    };
    let Some(three_month_trend) = params.three_month_trend.as_deref() else {
        return HttpResponse::BadRequest().body("Missing 3M trend");
    };
    let Some(six_month_trend) = params.six_month_trend.as_deref() else {
        return HttpResponse::BadRequest().body("Missing 6M trend");
    };
    let Some(twelve_month_trend) = params.twelve_month_trend.as_deref() else {
        return HttpResponse::BadRequest().body("Missing 12M trend");
    };

    if accuracy_column_for_harmonic_type(harmonic_type).is_none()
        || dominant_harmonic_predicate(harmonic_type).is_none()
    {
        return HttpResponse::BadRequest().body("Unsupported harmonic type");
    }

    if normalize_market_filter(Some(market)).is_none() {
        return HttpResponse::BadRequest().body("Unsupported market");
    }

    if !is_supported_bin_label(bin) {
        return HttpResponse::BadRequest().body("Unsupported bin");
    }

    if !is_supported_bin_label(time_bin) {
        return HttpResponse::BadRequest().body("Unsupported time bin");
    }
    if !is_supported_reversal_type(reversal_type) {
        return HttpResponse::BadRequest().body("Unsupported reversal type");
    }
    if !is_supported_size_bucket(size_bucket) {
        return HttpResponse::BadRequest().body("Unsupported size bucket");
    }
    if !is_supported_trend_bucket(three_month_trend)
        || !is_supported_trend_bucket(six_month_trend)
        || !is_supported_trend_bucket(twelve_month_trend)
    {
        return HttpResponse::BadRequest().body("Unsupported trend bucket");
    }

    let limit = params.limit.unwrap_or(50).clamp(1, 500);
    let offset = params.offset.unwrap_or(0).max(0);
    let include_count = params.include_count.unwrap_or(false);

    let where_clause = r#"
        WHERE market = ?
          AND harmonic_type = ?
          AND bin = ?
          AND reversal_type = ?
          AND size_bucket = ?
          AND time_bin = ?
          AND three_month_trend = ?
          AND six_month_trend = ?
          AND twelve_month_trend = ?
        "#;

    let count_sql = format!(
        r#"
        SELECT COUNT(*)
        FROM strategy_trade_summary_cache
        {where_clause}
        "#,
    );

    let data_sql = format!(
        r#"
        SELECT
            symbol,
            d_date,
            d_confirm_date,
            NULL AS reversal_detect_date,
            NULL AS target_date,
            NULL AS target_open,
            NULL AS target_high,
            NULL AS target_low,
            NULL AS target_close,
            trade_enter_price,
            trade_risk_exit_price,
            trade_reward_exit_price,
            trade_result,
            market,
            NULL AS pattern_id,
            pattern_group_id,
            NULL AS prop_strategy_id,
            harmonic_type,
            reversal_type,
            size_bucket,
            NULL AS balance_bucket,
            time_bin,
            NULL AS x_strictness,
            three_month_trend,
            six_month_trend,
            twelve_month_trend,
            CAST(time_accuracy AS DOUBLE) AS time_accuracy,
            x_length,
            a_length,
            b_length,
            c_length,
            d_length,
            full_pattern_length,
            CAST(bat_accuracy AS DECIMAL(12,2)) AS bat_accuracy,
            CAST(butterfly_accuracy AS DECIMAL(12,2)) AS butterfly_accuracy,
            CAST(gartley_accuracy AS DECIMAL(12,2)) AS gartley_accuracy,
            CAST(crab_accuracy AS DECIMAL(12,2)) AS crab_accuracy,
            CAST(shark_accuracy AS DECIMAL(12,2)) AS shark_accuracy
        FROM strategy_trade_summary_cache
        {where_clause}
        ORDER BY d_date DESC, trade_enter_price ASC
        LIMIT ? OFFSET ?
        "#,
    );

    let total_count = if include_count {
        match sqlx::query_scalar::<_, i64>(&count_sql)
            .bind(market)
            .bind(harmonic_type)
            .bind(bin)
            .bind(reversal_type)
            .bind(size_bucket)
            .bind(time_bin)
            .bind(three_month_trend)
            .bind(six_month_trend)
            .bind(twelve_month_trend)
            .fetch_one(pool.get_ref())
            .await
        {
            Ok(count) => count,
            Err(error) => {
                eprintln!("Strategy trades count DB error: {:?}", error);
                return HttpResponse::InternalServerError().finish();
            }
        }
    } else {
        -1
    };

    let fetch_limit = if include_count { limit } else { limit + 1 };

    let mut patterns = match sqlx::query_as::<_, PatternSummary>(&data_sql)
        .bind(market)
        .bind(harmonic_type)
        .bind(bin)
        .bind(reversal_type)
        .bind(size_bucket)
        .bind(time_bin)
        .bind(three_month_trend)
        .bind(six_month_trend)
        .bind(twelve_month_trend)
        .bind(fetch_limit)
        .bind(offset)
        .fetch_all(pool.get_ref())
        .await
    {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("Strategy trades data DB error: {:?}", error);
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
            p.target_ready
        FROM pattern_outcomes_prop p
        WHERE p.outcome_model = '{outcome_model}'
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
            0 AS trade_result,
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
    let sort_column = prop_family_sort_column(params.sort_by.as_deref(), "s");
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
            CAST(s.score AS DOUBLE) AS score
        FROM prop_strategy_family_summary s
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

fn structure_total_bars_expr() -> &'static str {
    "CAST(COALESCE(x_length, 0) + COALESCE(a_length, 0) + COALESCE(b_length, 0) + COALESCE(c_length, 0) AS DOUBLE)"
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

fn structure_size_bucket_expr(total_bars_expr: &str) -> String {
    format!(
        "CASE
            WHEN {total} <= 20 THEN 'Micro'
            WHEN {total} <= 60 THEN 'Small'
            WHEN {total} <= 180 THEN 'Normal'
            WHEN {total} <= 365 THEN 'Large'
            ELSE 'Massive'
        END",
        total = total_bars_expr,
    )
}

fn structure_size_bucket_order_expr(total_bars_expr: &str) -> String {
    format!(
        "CASE
            WHEN {total} <= 20 THEN 1
            WHEN {total} <= 60 THEN 2
            WHEN {total} <= 180 THEN 3
            WHEN {total} <= 365 THEN 4
            ELSE 5
        END",
        total = total_bars_expr,
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

fn structure_balance_bucket_order_expr(balance_ratio_expr: &str) -> String {
    format!(
        "CASE
            WHEN {ratio} IS NULL THEN 0
            WHEN {ratio} <= 1.75 THEN 1
            WHEN {ratio} <= 2.5 THEN 2
            WHEN {ratio} <= 4.0 THEN 3
            ELSE 4
        END",
        ratio = balance_ratio_expr,
    )
}

fn leg_accuracy_sql_expr(current_expr: &str, target_leg: f64) -> String {
    format!(
        "CASE
            WHEN COALESCE({current_expr}, 0) <= 0 THEN 0.0
            ELSE LEAST(
                GREATEST(
                    100.0 * (1.0 - ABS((CAST({current_expr} AS DOUBLE) / 100.0) - {target_leg}) / {target_leg}),
                    0.0
                ),
                100.0
            )
        END",
        current_expr = current_expr,
        target_leg = target_leg,
    )
}

fn dominant_time_accuracy_expr() -> String {
    let build_pattern_expr = |ab_xa: f64, bc_ab: f64, cd_bc: f64, cd_xa: f64| {
        format!(
            "(
                {ab_xa_expr}
                + {bc_ab_expr}
                + {cd_bc_expr}
                + {cd_xa_expr}
            ) / 4.0",
            ab_xa_expr = leg_accuracy_sql_expr("trade_ab_bar_retracement", ab_xa),
            bc_ab_expr = leg_accuracy_sql_expr("trade_bc_bar_retracement", bc_ab),
            cd_bc_expr = leg_accuracy_sql_expr("trade_cd_bc_bar_retracement", cd_bc),
            cd_xa_expr = leg_accuracy_sql_expr("trade_cd_xa_bar_retracement", cd_xa),
        )
    };

    format!(
        "CASE
            WHEN harmonic_type = 'Bat' THEN {bat_expr}
            WHEN harmonic_type = 'Butterfly' THEN {butterfly_expr}
            WHEN harmonic_type = 'Gartley' THEN {gartley_expr}
            WHEN harmonic_type = 'Crab' THEN {crab_expr}
            WHEN harmonic_type = 'Shark' THEN {shark_expr}
            ELSE NULL
        END",
        bat_expr = build_pattern_expr(0.382, 0.382, 2.618, 1.618),
        butterfly_expr = build_pattern_expr(0.786, 0.382, 1.27, 1.618),
        gartley_expr = build_pattern_expr(0.618, 0.382, 1.27, 0.786),
        crab_expr = build_pattern_expr(0.382, 0.382, 3.618, 2.618),
        shark_expr = build_pattern_expr(0.886, 0.382, 1.13, 1.618),
    )
}

fn time_bin_expr(time_accuracy_expr: &str) -> String {
    format!(
        "CASE
            WHEN {time_accuracy_expr} IS NULL THEN NULL
            WHEN {time_accuracy_expr} <= 10 THEN '0-10'
            WHEN {time_accuracy_expr} <= 20 THEN '10-20'
            WHEN {time_accuracy_expr} <= 30 THEN '20-30'
            WHEN {time_accuracy_expr} <= 40 THEN '30-40'
            WHEN {time_accuracy_expr} <= 50 THEN '40-50'
            WHEN {time_accuracy_expr} <= 60 THEN '50-60'
            WHEN {time_accuracy_expr} <= 70 THEN '60-70'
            WHEN {time_accuracy_expr} <= 80 THEN '70-80'
            WHEN {time_accuracy_expr} <= 90 THEN '80-90'
            ELSE '90-100'
        END",
        time_accuracy_expr = time_accuracy_expr,
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

fn dominant_harmonic_bin_expr() -> String {
    r#"
    CASE
        WHEN GREATEST(
            COALESCE(CAST(bat_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(butterfly_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(gartley_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(crab_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(shark_accuracy AS DOUBLE), 0.0)
        ) <= 10 THEN '0-10'
        WHEN GREATEST(
            COALESCE(CAST(bat_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(butterfly_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(gartley_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(crab_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(shark_accuracy AS DOUBLE), 0.0)
        ) <= 20 THEN '10-20'
        WHEN GREATEST(
            COALESCE(CAST(bat_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(butterfly_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(gartley_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(crab_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(shark_accuracy AS DOUBLE), 0.0)
        ) <= 30 THEN '20-30'
        WHEN GREATEST(
            COALESCE(CAST(bat_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(butterfly_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(gartley_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(crab_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(shark_accuracy AS DOUBLE), 0.0)
        ) <= 40 THEN '30-40'
        WHEN GREATEST(
            COALESCE(CAST(bat_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(butterfly_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(gartley_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(crab_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(shark_accuracy AS DOUBLE), 0.0)
        ) <= 50 THEN '40-50'
        WHEN GREATEST(
            COALESCE(CAST(bat_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(butterfly_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(gartley_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(crab_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(shark_accuracy AS DOUBLE), 0.0)
        ) <= 60 THEN '50-60'
        WHEN GREATEST(
            COALESCE(CAST(bat_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(butterfly_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(gartley_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(crab_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(shark_accuracy AS DOUBLE), 0.0)
        ) <= 70 THEN '60-70'
        WHEN GREATEST(
            COALESCE(CAST(bat_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(butterfly_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(gartley_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(crab_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(shark_accuracy AS DOUBLE), 0.0)
        ) <= 80 THEN '70-80'
        WHEN GREATEST(
            COALESCE(CAST(bat_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(butterfly_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(gartley_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(crab_accuracy AS DOUBLE), 0.0),
            COALESCE(CAST(shark_accuracy AS DOUBLE), 0.0)
        ) <= 90 THEN '80-90'
        ELSE '90-100'
    END
    "#
    .to_string()
}

fn trend_bucket_expr(trend_column: &str) -> String {
    format!(
        "CASE
            WHEN {trend_column} IS TRUE THEN 'Bullish'
            WHEN {trend_column} IS FALSE THEN 'Bearish'
            ELSE 'Unknown'
        END"
    )
}

async fn try_fetch_pattern_structure_from_rollup(
    pool: &MySqlPool,
    params: &FilterParams,
) -> Result<Option<PatternStructureResponse>, sqlx::Error> {
    if !is_rollup_cache_ready(pool, "structure_rollups").await? {
        return Ok(None);
    }

    if params.retracement.is_some() || params.recent_days.is_some() {
        return Ok(None);
    }

    if let Some(harmonic_type) = params.harmonic_type.as_deref() {
        if accuracy_column_for_harmonic_type(harmonic_type).is_none() {
            return Ok(None);
        }
    }

    if let Some(bin) = params.bin.as_deref() {
        if !is_supported_bin_label(bin) {
            return Ok(None);
        }
    }

    let market_filter = normalize_market_filter(params.market.as_deref());
    let trade_result_filter = normalize_trade_result_filter(params.trade_result);
    let harmonic_type_filter = params.harmonic_type.as_deref();
    let bin_filter = params.bin.as_deref();

    let mut where_clause = String::from("WHERE 1 = 1");

    if market_filter.is_some() {
        where_clause.push_str(" AND market = ?");
    }

    if trade_result_filter.is_some() {
        where_clause.push_str(" AND trade_result_value = ?");
    }

    if harmonic_type_filter.is_some() {
        where_clause.push_str(" AND harmonic_type = ?");
    }

    if bin_filter.is_some() {
        where_clause.push_str(" AND bin = ?");
    }

    let summary_sql = format!(
        r#"
        SELECT
            CAST(COALESCE(SUM(total_count), 0) AS SIGNED) AS total_count,
            CAST(COALESCE(SUM(closed_count), 0) AS SIGNED) AS closed_count,
            CAST(COALESCE(SUM(open_count), 0) AS SIGNED) AS open_count,
            COALESCE(
                CAST(COALESCE(SUM(total_bars_sum), 0) / NULLIF(COALESCE(SUM(total_bars_count), 0), 0) AS DOUBLE),
                0.0
            ) AS avg_total_bars,
            COALESCE(
                CAST(COALESCE(SUM(balance_ratio_sum), 0) / NULLIF(COALESCE(SUM(balance_ratio_count), 0), 0) AS DOUBLE),
                0.0
            ) AS avg_balance_ratio,
            CAST(COALESCE(SUM(distorted_count), 0) AS SIGNED) AS distorted_count,
            COALESCE(
                CAST(COALESCE(SUM(distorted_count), 0) / NULLIF(COALESCE(SUM(total_count), 0), 0) AS DOUBLE),
                0.0
            ) AS distorted_rate
        FROM pattern_structure_rollup
        {where_clause}
        "#
    );

    let size_buckets_sql = format!(
        r#"
        SELECT
            size_bucket AS bucket,
            size_order AS bucket_order,
            CAST(COALESCE(SUM(total_count), 0) AS SIGNED) AS total_count,
            CAST(COALESCE(SUM(closed_count), 0) AS SIGNED) AS closed_count,
            COALESCE(
                CAST(COALESCE(SUM(expectancy_sum), 0) / NULLIF(COALESCE(SUM(expectancy_count), 0), 0) AS DOUBLE),
                0.0
            ) AS expectancy,
            COALESCE(
                CAST(COALESCE(SUM(win_count), 0) / NULLIF(COALESCE(SUM(closed_count), 0), 0) AS DOUBLE),
                0.0
            ) AS win_rate,
            COALESCE(
                CAST(COALESCE(SUM(total_bars_sum), 0) / NULLIF(COALESCE(SUM(total_bars_count), 0), 0) AS DOUBLE),
                0.0
            ) AS avg_total_bars,
            COALESCE(
                CAST(COALESCE(SUM(balance_ratio_sum), 0) / NULLIF(COALESCE(SUM(balance_ratio_count), 0), 0) AS DOUBLE),
                0.0
            ) AS avg_balance_ratio
        FROM pattern_structure_rollup
        {where_clause}
        GROUP BY size_bucket, size_order
        ORDER BY size_order ASC
        "#
    );

    let balance_buckets_sql = format!(
        r#"
        SELECT
            balance_bucket AS bucket,
            balance_order AS bucket_order,
            CAST(COALESCE(SUM(total_count), 0) AS SIGNED) AS total_count,
            CAST(COALESCE(SUM(closed_count), 0) AS SIGNED) AS closed_count,
            COALESCE(
                CAST(COALESCE(SUM(expectancy_sum), 0) / NULLIF(COALESCE(SUM(expectancy_count), 0), 0) AS DOUBLE),
                0.0
            ) AS expectancy,
            COALESCE(
                CAST(COALESCE(SUM(win_count), 0) / NULLIF(COALESCE(SUM(closed_count), 0), 0) AS DOUBLE),
                0.0
            ) AS win_rate,
            COALESCE(
                CAST(COALESCE(SUM(total_bars_sum), 0) / NULLIF(COALESCE(SUM(total_bars_count), 0), 0) AS DOUBLE),
                0.0
            ) AS avg_total_bars,
            COALESCE(
                CAST(COALESCE(SUM(balance_ratio_sum), 0) / NULLIF(COALESCE(SUM(balance_ratio_count), 0), 0) AS DOUBLE),
                0.0
            ) AS avg_balance_ratio
        FROM pattern_structure_rollup
        {where_clause}
        GROUP BY balance_bucket, balance_order
        ORDER BY balance_order ASC
        "#
    );

    let size_balance_grid_sql = format!(
        r#"
        SELECT
            size_bucket,
            size_order,
            balance_bucket,
            balance_order,
            CAST(COALESCE(SUM(total_count), 0) AS SIGNED) AS total_count,
            CAST(COALESCE(SUM(closed_count), 0) AS SIGNED) AS closed_count,
            COALESCE(
                CAST(COALESCE(SUM(expectancy_sum), 0) / NULLIF(COALESCE(SUM(expectancy_count), 0), 0) AS DOUBLE),
                0.0
            ) AS expectancy,
            COALESCE(
                CAST(COALESCE(SUM(win_count), 0) / NULLIF(COALESCE(SUM(closed_count), 0), 0) AS DOUBLE),
                0.0
            ) AS win_rate
        FROM pattern_structure_rollup
        {where_clause}
        GROUP BY size_bucket, size_order, balance_bucket, balance_order
        ORDER BY size_order ASC, balance_order ASC
        "#
    );

    let mut summary_query = sqlx::query_as::<_, PatternStructureSummary>(&summary_sql);
    let mut size_buckets_query = sqlx::query_as::<_, PatternStructureBucket>(&size_buckets_sql);
    let mut balance_buckets_query =
        sqlx::query_as::<_, PatternStructureBucket>(&balance_buckets_sql);
    let mut size_balance_grid_query =
        sqlx::query_as::<_, PatternStructureGridCell>(&size_balance_grid_sql);

    if let Some(market) = market_filter {
        summary_query = summary_query.bind(market);
        size_buckets_query = size_buckets_query.bind(market);
        balance_buckets_query = balance_buckets_query.bind(market);
        size_balance_grid_query = size_balance_grid_query.bind(market);
    }

    if let Some(trade_result) = trade_result_filter {
        summary_query = summary_query.bind(trade_result);
        size_buckets_query = size_buckets_query.bind(trade_result);
        balance_buckets_query = balance_buckets_query.bind(trade_result);
        size_balance_grid_query = size_balance_grid_query.bind(trade_result);
    }

    if let Some(harmonic_type) = harmonic_type_filter {
        summary_query = summary_query.bind(harmonic_type);
        size_buckets_query = size_buckets_query.bind(harmonic_type);
        balance_buckets_query = balance_buckets_query.bind(harmonic_type);
        size_balance_grid_query = size_balance_grid_query.bind(harmonic_type);
    }

    if let Some(bin) = bin_filter {
        summary_query = summary_query.bind(bin);
        size_buckets_query = size_buckets_query.bind(bin);
        balance_buckets_query = balance_buckets_query.bind(bin);
        size_balance_grid_query = size_balance_grid_query.bind(bin);
    }

    let summary = summary_query.fetch_one(pool).await?;

    if summary.total_count == 0 {
        return Ok(Some(PatternStructureResponse {
            summary,
            size_buckets: Vec::new(),
            balance_buckets: Vec::new(),
            size_balance_grid: Vec::new(),
        }));
    }

    let size_buckets = size_buckets_query.fetch_all(pool).await?;
    let balance_buckets = balance_buckets_query.fetch_all(pool).await?;
    let size_balance_grid = size_balance_grid_query.fetch_all(pool).await?;

    Ok(Some(PatternStructureResponse {
        summary,
        size_buckets,
        balance_buckets,
        size_balance_grid,
    }))
}

async fn try_fetch_accuracy_from_structure_rollup(
    pool: &MySqlPool,
    params: &AccuracyParams,
) -> Result<Option<Vec<AccuracyBin>>, sqlx::Error> {
    if !is_rollup_cache_ready(pool, "structure_rollups").await? {
        return Ok(None);
    }

    let market_filter = normalize_market_filter(params.market.as_deref());
    let trade_result_filter = normalize_trade_result_filter(params.trade_result);
    let harmonic_type_filter = params.harmonic_type.as_deref();
    let size_bucket_filter = params.size_bucket.as_deref();
    let balance_bucket_filter = params.balance_bucket.as_deref();

    if size_bucket_filter.is_none() && balance_bucket_filter.is_none() {
        return Ok(None);
    }

    if let Some(size_bucket) = size_bucket_filter {
        if !is_supported_size_bucket(size_bucket) {
            return Ok(None);
        }
    }

    if let Some(balance_bucket) = balance_bucket_filter {
        if !is_supported_balance_bucket(balance_bucket) {
            return Ok(None);
        }
    }

    let source_scope = "all_patterns";

    let types_sql = if let Some(harmonic_type) = harmonic_type_filter {
        format!("SELECT '{}' AS harmonic_type", harmonic_type)
    } else {
        r#"
        SELECT 'Bat' AS harmonic_type UNION ALL
        SELECT 'Butterfly' UNION ALL
        SELECT 'Gartley' UNION ALL
        SELECT 'Crab' UNION ALL
        SELECT 'Shark'
        "#
        .to_string()
    };

    let mut rollup_filters = String::new();
    if market_filter.is_some() {
        rollup_filters.push_str(" AND market = ? ");
    }
    if trade_result_filter.is_some() {
        rollup_filters.push_str(" AND trade_result_value = ? ");
    }
    if harmonic_type_filter.is_some() {
        rollup_filters.push_str(" AND harmonic_type = ? ");
    }
    if size_bucket_filter.is_some() {
        rollup_filters.push_str(" AND size_bucket = ? ");
    }
    if balance_bucket_filter.is_some() {
        rollup_filters.push_str(" AND balance_bucket = ? ");
    }

    let rollup_sql = format!(
        r#"
        WITH bins AS (
            SELECT 1 AS bin_order, '0-10' AS bin UNION ALL
            SELECT 2, '10-20' UNION ALL
            SELECT 3, '20-30' UNION ALL
            SELECT 4, '30-40' UNION ALL
            SELECT 5, '40-50' UNION ALL
            SELECT 6, '50-60' UNION ALL
            SELECT 7, '60-70' UNION ALL
            SELECT 8, '70-80' UNION ALL
            SELECT 9, '80-90' UNION ALL
            SELECT 10, '90-100'
        ),
        types AS (
            {types_sql}
        ),
        agg AS (
            SELECT
                harmonic_type,
                bin,
                CAST(SUM(closed_count) AS SIGNED) AS count,
                CAST(SUM(total_count) AS SIGNED) AS total_count,
                CAST(SUM(open_count) AS SIGNED) AS open_count,
                CAST(SUM(win_count) AS SIGNED) AS win_count,
                CAST(SUM(loss_count) AS SIGNED) AS loss_count,
                CAST(CASE
                    WHEN SUM(return_count) > 0 THEN SUM(return_sum) / SUM(return_count)
                    ELSE 0.0
                END AS DOUBLE) AS avg_return,
                CAST(CASE
                    WHEN SUM(expectancy_count) > 0 THEN SUM(expectancy_sum) / SUM(expectancy_count)
                    ELSE 0.0
                END AS DOUBLE) AS expectancy,
                CAST(CASE
                    WHEN SUM(closed_count) > 0 THEN SUM(win_count) / SUM(closed_count)
                    ELSE 0.0
                END AS DOUBLE) AS win_rate,
                CAST(CASE
                    WHEN SUM(total_count) > 0 THEN SUM(closed_count) / SUM(total_count)
                    ELSE 0.0
                END AS DOUBLE) AS closed_rate,
                CAST(CASE
                    WHEN SUM(win_return_count) > 0 THEN SUM(win_return_sum) / SUM(win_return_count)
                    ELSE 0.0
                END AS DOUBLE) AS avg_win,
                CAST(CASE
                    WHEN SUM(loss_return_count) > 0 THEN SUM(loss_return_sum) / SUM(loss_return_count)
                    ELSE 0.0
                END AS DOUBLE) AS avg_loss
            FROM accuracy_structure_rollup
            WHERE source_scope = ?
            {rollup_filters}
            GROUP BY harmonic_type, bin
        )
        SELECT
            t.harmonic_type,
            b.bin,
            CAST(COALESCE(a.count, 0) AS SIGNED) AS count,
            CAST(COALESCE(a.total_count, 0) AS SIGNED) AS total_count,
            CAST(COALESCE(a.open_count, 0) AS SIGNED) AS open_count,
            CAST(COALESCE(a.win_count, 0) AS SIGNED) AS win_count,
            CAST(COALESCE(a.loss_count, 0) AS SIGNED) AS loss_count,
            COALESCE(a.avg_return, 0.0) AS avg_return,
            COALESCE(a.expectancy, 0.0) AS expectancy,
            COALESCE(a.win_rate, 0.0) AS win_rate,
            COALESCE(a.closed_rate, 0.0) AS closed_rate,
            COALESCE(a.avg_win, 0.0) AS avg_win,
            COALESCE(a.avg_loss, 0.0) AS avg_loss
        FROM types t
        CROSS JOIN bins b
        LEFT JOIN agg a
            ON a.harmonic_type = t.harmonic_type
           AND a.bin = b.bin
        ORDER BY t.harmonic_type, b.bin_order
        "#,
    );

    let mut rollup_query = sqlx::query_as::<_, AccuracyBin>(&rollup_sql).bind(source_scope);

    if let Some(market) = market_filter {
        rollup_query = rollup_query.bind(market);
    }

    if let Some(trade_result) = trade_result_filter {
        rollup_query = rollup_query.bind(trade_result);
    }

    if let Some(harmonic_type) = harmonic_type_filter {
        rollup_query = rollup_query.bind(harmonic_type);
    }

    if let Some(size_bucket) = size_bucket_filter {
        rollup_query = rollup_query.bind(size_bucket);
    }

    if let Some(balance_bucket) = balance_bucket_filter {
        rollup_query = rollup_query.bind(balance_bucket);
    }

    let rows = rollup_query.fetch_all(pool).await?;

    if rows.is_empty() {
        return Ok(None);
    }

    Ok(Some(rows))
}

fn build_patterns_where_clause(
    params: &FilterParams,
) -> Result<
    (
        String,
        Option<(&'static str, &'static str, f64, f64)>,
        Option<(f64, f64)>,
        Option<&str>,
        Option<i32>,
        Option<i64>,
    ),
    HttpResponse,
> {
    let market_filter = normalize_market_filter(params.market.as_deref());
    let trade_result_filter = normalize_trade_result_filter(params.trade_result);
    let harmonic_type_filter = params.harmonic_type.as_deref();
    let size_bucket_filter = params.size_bucket.as_deref();
    let balance_bucket_filter = params.balance_bucket.as_deref();
    let reversal_type_filter = params.reversal_type.as_deref();
    let bin_filter = params.bin.as_deref();
    let recent_days = params.recent_days.map(|days| days.max(1).min(30));

    if let Some(size_bucket) = size_bucket_filter {
        if !is_supported_size_bucket(size_bucket) {
            return Err(HttpResponse::BadRequest().body("Unsupported size bucket"));
        }
    }

    if let Some(balance_bucket) = balance_bucket_filter {
        if !is_supported_balance_bucket(balance_bucket) {
            return Err(HttpResponse::BadRequest().body("Unsupported balance bucket"));
        }
    }

    if let Some(reversal_type) = reversal_type_filter {
        if !is_supported_reversal_type(reversal_type) {
            return Err(HttpResponse::BadRequest().body("Unsupported reversal type"));
        }
    }

    let harmonic_scope = if let Some(harmonic_type) = harmonic_type_filter {
        let Some(column) = accuracy_column_for_harmonic_type(harmonic_type) else {
            return Err(HttpResponse::BadRequest().body("Unsupported harmonic type"));
        };
        let Some(dominant_predicate) = dominant_harmonic_predicate(harmonic_type) else {
            return Err(HttpResponse::BadRequest().body("Unsupported harmonic type"));
        };
        let (min, max) = parse_bin_range(bin_filter).unwrap_or((0.0, 100.0));

        Some((column, dominant_predicate, min, max))
    } else {
        None
    };

    let all_pattern_bin_scope = if harmonic_type_filter.is_none() {
        parse_bin_range(bin_filter)
    } else {
        None
    };

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

    if market_filter.is_some() {
        where_clause.push_str(" AND market = ?");
    }

    if trade_result_filter.is_some() {
        where_clause.push_str(" AND trade_result = ?");
    }

    if recent_days.is_some() {
        where_clause.push_str(" AND d_date >= DATE_SUB(CURDATE(), INTERVAL ? DAY)");
    }

    if size_bucket_filter.is_some() {
        let total_bars_expr = structure_total_bars_expr();
        let size_bucket_expr = structure_size_bucket_expr(total_bars_expr);
        where_clause.push_str(&format!(" AND {size_bucket_expr} = ? "));
    }

    if balance_bucket_filter.is_some() {
        let balance_ratio_expr = structure_balance_ratio_expr();
        let balance_bucket_expr = structure_balance_bucket_expr(&balance_ratio_expr);
        where_clause.push_str(&format!(" AND {balance_bucket_expr} = ? "));
    }

    if reversal_type_filter.is_some() {
        where_clause.push_str(" AND COALESCE(NULLIF(reversal_type, ''), 'None') = ?");
    }

    if params.retracement.is_some() {
        where_clause.push_str(
            r#"
            AND trade_ab_price_retracement >= ?
            AND trade_ab_price_retracement <= ?
            AND trade_bc_price_retracement >= ?
            AND trade_bc_price_retracement <= ?
            AND trade_cd_bc_price_retracement >= ?
            AND trade_cd_bc_price_retracement <= ?
            AND trade_cd_xa_price_retracement >= ?
            AND trade_cd_xa_price_retracement <= ?
            "#,
        );
    }

    Ok((
        where_clause,
        harmonic_scope,
        all_pattern_bin_scope,
        market_filter,
        trade_result_filter,
        recent_days,
    ))
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
        WHERE target_ready = TRUE
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

fn build_strategy_live_cache_where_clause(
    params: &FilterParams,
) -> Result<
    (
        String,
        Option<&str>,
        Option<&str>,
        Option<&str>,
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
    let bin_range = parse_bin_range(params.bin.as_deref());
    let recent_days = params.recent_days.map(|days| days.max(1).min(30));
    let max_days_open = params.max_days_open.map(|days| days.max(0).min(3650));

    if let Some(harmonic_type) = harmonic_type_filter {
        if accuracy_column_for_harmonic_type(harmonic_type).is_none() {
            return Err(HttpResponse::BadRequest().body("Unsupported harmonic type"));
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
        WHERE trade_result = 0
        "#,
    );

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
    if bin_range.is_some() {
        where_clause.push_str(" AND bin = ?");
    }
    if recent_days.is_some() {
        where_clause.push_str(" AND d_date >= DATE_SUB(CURDATE(), INTERVAL ? DAY)");
    }
    if max_days_open.is_some() {
        where_clause.push_str(" AND d_length <= ?");
    }

    Ok((
        where_clause,
        market_filter,
        harmonic_type_filter,
        reversal_type_filter,
        bin_range,
        recent_days,
        max_days_open,
    ))
}

// --- Main ---
fn is_duplicate_or_missing_index_error(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(db_error) => {
            let message = db_error.message();
            message.contains("Duplicate key name")
                || message.contains("already exists")
                || message.contains("doesn't exist")
                || message.contains("Unknown table")
                || message.contains("Unknown column")
        }
        _ => false,
    }
}

async fn create_index_if_missing(pool: &MySqlPool, sql: &str) -> Result<(), sqlx::Error> {
    if let Err(error) = sqlx::query(sql).execute(pool).await {
        if !is_duplicate_or_missing_index_error(&error) {
            return Err(error);
        }
    }

    Ok(())
}

async fn ensure_canvas_load_indexes(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    for sql in [
        "CREATE INDEX idx_candles_symbol_date ON candles (symbol, date)",
        "CREATE INDEX idx_pattern_outcomes_prop_pattern_id ON pattern_outcomes_prop (pattern_id, d_date)",
        "CREATE INDEX idx_pattern_outcomes_prop_group_detail ON pattern_outcomes_prop (pattern_group_id, d_date, market, harmonic_type, size_bucket)",
        "CREATE INDEX idx_pattern_outcomes_prop_symbol_d_date ON pattern_outcomes_prop (symbol, d_date)",
        "CREATE INDEX idx_xabcd_pattern_group_d_date ON xabcd_patterns (pattern_group_id, d_date)",
    ] {
        create_index_if_missing(pool, sql).await?;
    }

    Ok(())
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
    if let Err(error) = ensure_canvas_load_indexes(&pool).await {
        eprintln!("Canvas load index check failed: {:?}", error);
    }
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
            .service(fetch_accuracy)
            .service(fetch_candles)
            .service(fetch_patterns)
            .service(fetch_pattern_summaries)
            .service(fetch_reversal_pattern_summaries)
            .service(fetch_strategy_trades)
            .service(fetch_current_open_setups)
            .service(fetch_current_setup_strategies)
            .service(fetch_pattern_detail)
            .service(fetch_pattern_structure)
            .service(fetch_pattern_reversal)
            .service(fetch_setup_comparison)
            .service(fetch_setup_decision)
            .service(fetch_strategy_candidates)
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

#[route("/patterns", method = "GET", method = "POST")]
async fn fetch_patterns(
    pool: web::Data<MySqlPool>,
    params: web::Json<FilterParams>,
) -> impl Responder {
    println!("🔔 Handler called: fetch_patterns");
    println!("📥 Received filter: {:?}", params);

    let limit = params.limit.unwrap_or(500).clamp(1, 2_000);
    let offset = params.offset.unwrap_or(0).max(0);
    let (
        where_clause,
        harmonic_scope,
        all_pattern_bin_scope,
        market_filter,
        trade_result_filter,
        recent_days,
    ) = match build_patterns_where_clause(&params) {
        Ok(parts) => parts,
        Err(response) => return response,
    };

    let include_count = params.include_count.unwrap_or(true);
    let count_query_sql = format!(
        r#"
        SELECT COUNT(*)
        FROM xabcd_patterns
        {where_clause}
        "#
    );
    let order_clause = if recent_days.is_some() {
        "ORDER BY d_date DESC, trade_enter_price ASC"
    } else {
        ""
    };

    let data_query_sql = format!(
        r#"
        SELECT *
        FROM xabcd_patterns
        {where_clause}
        {order_clause}
        LIMIT ? OFFSET ?
        "#
    );

    let mut pattern_query = sqlx::query_as::<_, Pattern>(&data_query_sql);

    if let Some((_, _, min, max)) = harmonic_scope {
        pattern_query = pattern_query.bind(min).bind(max);
    } else if let Some((min, max)) = all_pattern_bin_scope {
        pattern_query = pattern_query.bind(min).bind(max);
    }

    if let Some(market) = market_filter {
        pattern_query = pattern_query.bind(market);
    }

    if let Some(trade_result) = trade_result_filter {
        pattern_query = pattern_query.bind(trade_result);
    }

    if let Some(days) = recent_days {
        pattern_query = pattern_query.bind(days);
    }

    if let Some(size_bucket) = params.size_bucket.as_deref() {
        pattern_query = pattern_query.bind(size_bucket);
    }

    if let Some(balance_bucket) = params.balance_bucket.as_deref() {
        pattern_query = pattern_query.bind(balance_bucket);
    }

    if let Some(retracement) = &params.retracement {
        pattern_query = pattern_query
            .bind(retracement.ab_xa_gr)
            .bind(retracement.ab_xa_lt)
            .bind(retracement.bc_ab_gr)
            .bind(retracement.bc_ab_lt)
            .bind(retracement.cd_bc_gr)
            .bind(retracement.cd_bc_lt)
            .bind(retracement.cd_xa_gr)
            .bind(retracement.cd_xa_lt);
    }

    let total_count = if include_count {
        let mut count_query = sqlx::query_scalar::<_, i64>(&count_query_sql);

        if let Some((_, _, min, max)) = harmonic_scope {
            count_query = count_query.bind(min).bind(max);
        } else if let Some((min, max)) = all_pattern_bin_scope {
            count_query = count_query.bind(min).bind(max);
        }

        if let Some(market) = market_filter {
            count_query = count_query.bind(market);
        }

        if let Some(trade_result) = trade_result_filter {
            count_query = count_query.bind(trade_result);
        }

        if let Some(days) = recent_days {
            count_query = count_query.bind(days);
        }

        if let Some(size_bucket) = params.size_bucket.as_deref() {
            count_query = count_query.bind(size_bucket);
        }

        if let Some(balance_bucket) = params.balance_bucket.as_deref() {
            count_query = count_query.bind(balance_bucket);
        }

        if let Some(retracement) = &params.retracement {
            count_query = count_query
                .bind(retracement.ab_xa_gr)
                .bind(retracement.ab_xa_lt)
                .bind(retracement.bc_ab_gr)
                .bind(retracement.bc_ab_lt)
                .bind(retracement.cd_bc_gr)
                .bind(retracement.cd_bc_lt)
                .bind(retracement.cd_xa_gr)
                .bind(retracement.cd_xa_lt);
        }

        match count_query.fetch_one(pool.get_ref()).await {
            Ok(count) => count,
            Err(e) => {
                eprintln!("Count DB error: {:?}", e);
                return HttpResponse::InternalServerError().finish();
            }
        }
    } else {
        -1
    };

    let fetch_limit = if include_count { limit } else { limit + 1 };
    pattern_query = pattern_query.bind(fetch_limit).bind(offset);

    let mut patterns: Vec<Pattern> = match pattern_query.fetch_all(pool.get_ref()).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("DB error: {:?}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    println!("📊 Found {} patterns", patterns.len());

    // === SUMMARY ===
    let has_more = if include_count {
        offset + limit < total_count
    } else if patterns.len() as i64 > limit {
        patterns.truncate(limit as usize);
        true
    } else {
        false
    };

    let grouped_by_month = MonthlySummary::group_patterns_by_month(&patterns);
    let monthly_stats = MonthlySummary::from_grouped_patterns(&grouped_by_month);

    let grouped_by_year = YearlySummary::group_patterns_by_year(&patterns);
    let yearly_summary = YearlySummary::get_yearly_summary(&grouped_by_year);

    let response = PatternsResponse {
        patterns,
        total_count,
        has_more,
        monthly_stats,
        yearly_summary,
    };

    HttpResponse::Ok().json(response)
}

#[route("/pattern-structure", method = "GET", method = "POST")]
async fn fetch_pattern_structure(
    pool: web::Data<MySqlPool>,
    params: web::Json<FilterParams>,
) -> impl Responder {
    println!("🔔 Handler called: fetch_pattern_structure");
    println!("📥 Received filter: {:?}", params);

    match try_fetch_pattern_structure_from_rollup(pool.get_ref(), &params).await {
        Ok(Some(response)) => return HttpResponse::Ok().json(response),
        Ok(None) => {}
        Err(error) => {
            eprintln!(
                "Pattern structure rollup lookup failed, checking raw path: {:?}",
                error
            );
        }
    }

    let (
        where_clause,
        harmonic_scope,
        all_pattern_bin_scope,
        market_filter,
        trade_result_filter,
        recent_days,
    ) = match build_patterns_where_clause(&params) {
        Ok(parts) => parts,
        Err(response) => return response,
    };

    let total_bars_expr = structure_total_bars_expr();
    let balance_ratio_expr = structure_balance_ratio_expr();
    let size_bucket_expr = structure_size_bucket_expr(total_bars_expr);
    let size_bucket_order_expr = structure_size_bucket_order_expr(total_bars_expr);
    let balance_bucket_expr = structure_balance_bucket_expr(&balance_ratio_expr);
    let balance_bucket_order_expr = structure_balance_bucket_order_expr(&balance_ratio_expr);

    let filtered_patterns_cte = format!(
        r#"
        WITH filtered_patterns AS (
            SELECT
                x_length,
                a_length,
                b_length,
                c_length,
                trade_result,
                CAST(trade_pnl AS DOUBLE) AS trade_pnl
            FROM xabcd_patterns
            {where_clause}
        )
        "#
    );

    let summary_sql = format!(
        r#"
        {filtered_patterns_cte}
        SELECT
            CAST(COUNT(*) AS SIGNED) AS total_count,
            CAST(SUM(CASE WHEN trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
            CAST(SUM(CASE WHEN trade_result NOT IN (1, 2) OR trade_result IS NULL THEN 1 ELSE 0 END) AS SIGNED) AS open_count,
            COALESCE(CAST(AVG({total_bars_expr}) AS DOUBLE), 0.0) AS avg_total_bars,
            COALESCE(CAST(AVG({balance_ratio_expr}) AS DOUBLE), 0.0) AS avg_balance_ratio,
            CAST(SUM(CASE WHEN {balance_bucket_expr} = 'Distorted' THEN 1 ELSE 0 END) AS SIGNED) AS distorted_count,
            COALESCE(
                CAST(AVG(CASE WHEN {balance_bucket_expr} = 'Distorted' THEN 1.0 ELSE 0.0 END) AS DOUBLE),
                0.0
            ) AS distorted_rate
        FROM filtered_patterns
        "#
    );

    let size_buckets_sql = format!(
        r#"
        {filtered_patterns_cte}
        SELECT
            {size_bucket_expr} AS bucket,
            {size_bucket_order_expr} AS bucket_order,
            CAST(COUNT(*) AS SIGNED) AS total_count,
            CAST(SUM(CASE WHEN trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
            COALESCE(CAST(AVG(CASE WHEN trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0) AS expectancy,
            COALESCE(
                CAST(
                    AVG(
                        CASE
                            WHEN trade_result = 1 THEN 1.0
                            WHEN trade_result = 2 THEN 0.0
                            ELSE NULL
                        END
                    ) AS DOUBLE
                ),
                0.0
            ) AS win_rate,
            COALESCE(CAST(AVG({total_bars_expr}) AS DOUBLE), 0.0) AS avg_total_bars,
            COALESCE(CAST(AVG({balance_ratio_expr}) AS DOUBLE), 0.0) AS avg_balance_ratio
        FROM filtered_patterns
        GROUP BY bucket, bucket_order
        ORDER BY bucket_order ASC
        "#
    );

    let balance_buckets_sql = format!(
        r#"
        {filtered_patterns_cte}
        SELECT
            {balance_bucket_expr} AS bucket,
            {balance_bucket_order_expr} AS bucket_order,
            CAST(COUNT(*) AS SIGNED) AS total_count,
            CAST(SUM(CASE WHEN trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
            COALESCE(CAST(AVG(CASE WHEN trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0) AS expectancy,
            COALESCE(
                CAST(
                    AVG(
                        CASE
                            WHEN trade_result = 1 THEN 1.0
                            WHEN trade_result = 2 THEN 0.0
                            ELSE NULL
                        END
                    ) AS DOUBLE
                ),
                0.0
            ) AS win_rate,
            COALESCE(CAST(AVG({total_bars_expr}) AS DOUBLE), 0.0) AS avg_total_bars,
            COALESCE(CAST(AVG({balance_ratio_expr}) AS DOUBLE), 0.0) AS avg_balance_ratio
        FROM filtered_patterns
        GROUP BY bucket, bucket_order
        ORDER BY bucket_order ASC
        "#
    );

    let size_balance_grid_sql = format!(
        r#"
        {filtered_patterns_cte}
        SELECT
            {size_bucket_expr} AS size_bucket,
            {size_bucket_order_expr} AS size_order,
            {balance_bucket_expr} AS balance_bucket,
            {balance_bucket_order_expr} AS balance_order,
            CAST(COUNT(*) AS SIGNED) AS total_count,
            CAST(SUM(CASE WHEN trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
            COALESCE(CAST(AVG(CASE WHEN trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0) AS expectancy,
            COALESCE(
                CAST(
                    AVG(
                        CASE
                            WHEN trade_result = 1 THEN 1.0
                            WHEN trade_result = 2 THEN 0.0
                            ELSE NULL
                        END
                    ) AS DOUBLE
                ),
                0.0
            ) AS win_rate
        FROM filtered_patterns
        GROUP BY size_bucket, size_order, balance_bucket, balance_order
        ORDER BY size_order ASC, balance_order ASC
        "#
    );

    let mut summary_query = sqlx::query_as::<_, PatternStructureSummary>(&summary_sql);
    let mut size_buckets_query = sqlx::query_as::<_, PatternStructureBucket>(&size_buckets_sql);
    let mut balance_buckets_query =
        sqlx::query_as::<_, PatternStructureBucket>(&balance_buckets_sql);
    let mut size_balance_grid_query =
        sqlx::query_as::<_, PatternStructureGridCell>(&size_balance_grid_sql);

    if let Some((_, _, min, max)) = harmonic_scope {
        summary_query = summary_query.bind(min).bind(max);
        size_buckets_query = size_buckets_query.bind(min).bind(max);
        balance_buckets_query = balance_buckets_query.bind(min).bind(max);
        size_balance_grid_query = size_balance_grid_query.bind(min).bind(max);
    } else if let Some((min, max)) = all_pattern_bin_scope {
        summary_query = summary_query.bind(min).bind(max);
        size_buckets_query = size_buckets_query.bind(min).bind(max);
        balance_buckets_query = balance_buckets_query.bind(min).bind(max);
        size_balance_grid_query = size_balance_grid_query.bind(min).bind(max);
    }

    if let Some(market) = market_filter {
        summary_query = summary_query.bind(market);
        size_buckets_query = size_buckets_query.bind(market);
        balance_buckets_query = balance_buckets_query.bind(market);
        size_balance_grid_query = size_balance_grid_query.bind(market);
    }

    if let Some(trade_result) = trade_result_filter {
        summary_query = summary_query.bind(trade_result);
        size_buckets_query = size_buckets_query.bind(trade_result);
        balance_buckets_query = balance_buckets_query.bind(trade_result);
        size_balance_grid_query = size_balance_grid_query.bind(trade_result);
    }

    if let Some(days) = recent_days {
        summary_query = summary_query.bind(days);
        size_buckets_query = size_buckets_query.bind(days);
        balance_buckets_query = balance_buckets_query.bind(days);
        size_balance_grid_query = size_balance_grid_query.bind(days);
    }

    if let Some(retracement) = &params.retracement {
        summary_query = summary_query
            .bind(retracement.ab_xa_gr)
            .bind(retracement.ab_xa_lt)
            .bind(retracement.bc_ab_gr)
            .bind(retracement.bc_ab_lt)
            .bind(retracement.cd_bc_gr)
            .bind(retracement.cd_bc_lt)
            .bind(retracement.cd_xa_gr)
            .bind(retracement.cd_xa_lt);

        size_buckets_query = size_buckets_query
            .bind(retracement.ab_xa_gr)
            .bind(retracement.ab_xa_lt)
            .bind(retracement.bc_ab_gr)
            .bind(retracement.bc_ab_lt)
            .bind(retracement.cd_bc_gr)
            .bind(retracement.cd_bc_lt)
            .bind(retracement.cd_xa_gr)
            .bind(retracement.cd_xa_lt);

        balance_buckets_query = balance_buckets_query
            .bind(retracement.ab_xa_gr)
            .bind(retracement.ab_xa_lt)
            .bind(retracement.bc_ab_gr)
            .bind(retracement.bc_ab_lt)
            .bind(retracement.cd_bc_gr)
            .bind(retracement.cd_bc_lt)
            .bind(retracement.cd_xa_gr)
            .bind(retracement.cd_xa_lt);

        size_balance_grid_query = size_balance_grid_query
            .bind(retracement.ab_xa_gr)
            .bind(retracement.ab_xa_lt)
            .bind(retracement.bc_ab_gr)
            .bind(retracement.bc_ab_lt)
            .bind(retracement.cd_bc_gr)
            .bind(retracement.cd_bc_lt)
            .bind(retracement.cd_xa_gr)
            .bind(retracement.cd_xa_lt);
    }

    let summary = match summary_query.fetch_one(pool.get_ref()).await {
        Ok(row) => row,
        Err(error) => {
            eprintln!("Pattern structure summary DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let size_buckets = match size_buckets_query.fetch_all(pool.get_ref()).await {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("Pattern structure size bucket DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let balance_buckets = match balance_buckets_query.fetch_all(pool.get_ref()).await {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("Pattern structure balance bucket DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let size_balance_grid = match size_balance_grid_query.fetch_all(pool.get_ref()).await {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("Pattern structure grid DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().json(PatternStructureResponse {
        summary,
        size_buckets,
        balance_buckets,
        size_balance_grid,
    })
}

#[route("/pattern-reversal", method = "GET", method = "POST")]
async fn fetch_pattern_reversal(
    pool: web::Data<MySqlPool>,
    params: web::Json<FilterParams>,
) -> impl Responder {
    println!("🔔 Handler called: fetch_pattern_reversal");
    println!("📥 Received filter: {:?}", params);

    let (
        where_clause,
        harmonic_scope,
        all_pattern_bin_scope,
        market_filter,
        trade_result_filter,
        recent_days,
    ) = match build_patterns_where_clause(&params) {
        Ok(parts) => parts,
        Err(response) => return response,
    };

    let filtered_patterns_cte = format!(
        r#"
        WITH filtered_patterns AS (
            SELECT
                COALESCE(NULLIF(sw.reversal_type, ''), 'None') AS reversal_type,
                sw.bullish_key_reversal,
                sw.bearish_key_reversal,
                sw.bullish_engulfing,
                sw.bearish_engulfing,
                sw.bullish_outside_reversal,
                sw.bearish_outside_reversal,
                sw.hammer,
                sw.shooting_star,
                sw.morning_star,
                sw.evening_star,
                sw.three_white_soldiers,
                sw.three_black_crows,
                sw.trade_result,
                CAST(sw.trade_pnl AS DOUBLE) AS trade_pnl,
                CAST(sw.trade_ab_price_retracement AS DOUBLE) AS trade_ab_price_retracement,
                CAST(sw.trade_bc_price_retracement AS DOUBLE) AS trade_bc_price_retracement,
                CAST(sw.trade_cd_bc_price_retracement AS DOUBLE) AS trade_cd_bc_price_retracement,
                CAST(sw.trade_cd_xa_price_retracement AS DOUBLE) AS trade_cd_xa_price_retracement,
                s.market,
                s.d_date,
                s.x_length,
                s.a_length,
                s.b_length,
                s.c_length,
                s.bat_accuracy,
                s.butterfly_accuracy,
                s.gartley_accuracy,
                s.crab_accuracy,
                s.shark_accuracy,
                s.harmonic_type,
                s.time_accuracy
            FROM pattern_setups s
            INNER JOIN pattern_outcomes_swing sw
                ON sw.setup_id = s.setup_id
            {where_clause}
        )
        "#
    );

    let summary_sql = format!(
        r#"
        {filtered_patterns_cte}
        SELECT
            CAST(COUNT(*) AS SIGNED) AS total_count,
            CAST(SUM(CASE WHEN trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
            CAST(
                SUM(
                    CASE
                        WHEN bullish_key_reversal = TRUE
                          OR bearish_key_reversal = TRUE
                          OR bullish_engulfing = TRUE
                          OR bearish_engulfing = TRUE
                          OR bullish_outside_reversal = TRUE
                          OR bearish_outside_reversal = TRUE
                          OR hammer = TRUE
                          OR shooting_star = TRUE
                          OR morning_star = TRUE
                          OR evening_star = TRUE
                          OR three_white_soldiers = TRUE
                          OR three_black_crows = TRUE
                        THEN 1
                        ELSE 0
                    END
                ) AS SIGNED
            ) AS confirmed_count,
            COALESCE(
                CAST(
                    AVG(
                        CASE
                            WHEN bullish_key_reversal = TRUE
                              OR bearish_key_reversal = TRUE
                              OR bullish_engulfing = TRUE
                              OR bearish_engulfing = TRUE
                              OR bullish_outside_reversal = TRUE
                              OR bearish_outside_reversal = TRUE
                              OR hammer = TRUE
                              OR shooting_star = TRUE
                              OR morning_star = TRUE
                              OR evening_star = TRUE
                              OR three_white_soldiers = TRUE
                              OR three_black_crows = TRUE
                            THEN 1.0
                            ELSE 0.0
                        END
                    ) AS DOUBLE
                ),
                0.0
            ) AS confirmation_rate,
            COALESCE(CAST(AVG(CASE WHEN trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0) AS avg_return,
            COALESCE(
                CAST(
                    AVG(
                        CASE
                            WHEN trade_result = 1 THEN 1.0
                            WHEN trade_result = 2 THEN 0.0
                            ELSE NULL
                        END
                    ) AS DOUBLE
                ),
                0.0
            ) AS win_rate
        FROM filtered_patterns
        "#
    );

    let reversal_types_sql = format!(
        r#"
        {filtered_patterns_cte}
        SELECT
            signal_key,
            reversal_type,
            bucket_order,
            total_count,
            closed_count,
            expectancy,
            win_rate
        FROM (
            SELECT
                'bullish_key_reversal' AS signal_key,
                'BullishKeyReversal' AS reversal_type,
                1 AS bucket_order,
                CAST(SUM(CASE WHEN bullish_key_reversal = TRUE THEN 1 ELSE 0 END) AS SIGNED) AS total_count,
                CAST(SUM(CASE WHEN bullish_key_reversal = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
                COALESCE(CAST(AVG(CASE WHEN bullish_key_reversal = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0) AS expectancy,
                COALESCE(CAST(AVG(CASE WHEN bullish_key_reversal = TRUE AND trade_result = 1 THEN 1.0 WHEN bullish_key_reversal = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0) AS win_rate
            FROM filtered_patterns

            UNION ALL

            SELECT
                'bearish_key_reversal',
                'BearishKeyReversal',
                2,
                CAST(SUM(CASE WHEN bearish_key_reversal = TRUE THEN 1 ELSE 0 END) AS SIGNED),
                CAST(SUM(CASE WHEN bearish_key_reversal = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED),
                COALESCE(CAST(AVG(CASE WHEN bearish_key_reversal = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0),
                COALESCE(CAST(AVG(CASE WHEN bearish_key_reversal = TRUE AND trade_result = 1 THEN 1.0 WHEN bearish_key_reversal = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0)
            FROM filtered_patterns

            UNION ALL

            SELECT
                'bullish_outside_reversal',
                'BullishOutsideReversal',
                3,
                CAST(SUM(CASE WHEN bullish_outside_reversal = TRUE THEN 1 ELSE 0 END) AS SIGNED),
                CAST(SUM(CASE WHEN bullish_outside_reversal = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED),
                COALESCE(CAST(AVG(CASE WHEN bullish_outside_reversal = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0),
                COALESCE(CAST(AVG(CASE WHEN bullish_outside_reversal = TRUE AND trade_result = 1 THEN 1.0 WHEN bullish_outside_reversal = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0)
            FROM filtered_patterns

            UNION ALL

            SELECT
                'bearish_outside_reversal',
                'BearishOutsideReversal',
                4,
                CAST(SUM(CASE WHEN bearish_outside_reversal = TRUE THEN 1 ELSE 0 END) AS SIGNED),
                CAST(SUM(CASE WHEN bearish_outside_reversal = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED),
                COALESCE(CAST(AVG(CASE WHEN bearish_outside_reversal = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0),
                COALESCE(CAST(AVG(CASE WHEN bearish_outside_reversal = TRUE AND trade_result = 1 THEN 1.0 WHEN bearish_outside_reversal = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0)
            FROM filtered_patterns

            UNION ALL

            SELECT
                'hammer',
                'Hammer',
                5,
                CAST(SUM(CASE WHEN hammer = TRUE THEN 1 ELSE 0 END) AS SIGNED),
                CAST(SUM(CASE WHEN hammer = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED),
                COALESCE(CAST(AVG(CASE WHEN hammer = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0),
                COALESCE(CAST(AVG(CASE WHEN hammer = TRUE AND trade_result = 1 THEN 1.0 WHEN hammer = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0)
            FROM filtered_patterns

            UNION ALL

            SELECT
                'shooting_star',
                'ShootingStar',
                6,
                CAST(SUM(CASE WHEN shooting_star = TRUE THEN 1 ELSE 0 END) AS SIGNED),
                CAST(SUM(CASE WHEN shooting_star = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED),
                COALESCE(CAST(AVG(CASE WHEN shooting_star = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0),
                COALESCE(CAST(AVG(CASE WHEN shooting_star = TRUE AND trade_result = 1 THEN 1.0 WHEN shooting_star = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0)
            FROM filtered_patterns

            UNION ALL

            SELECT
                'bullish_engulfing',
                'BullishEngulfing',
                7,
                CAST(SUM(CASE WHEN bullish_engulfing = TRUE THEN 1 ELSE 0 END) AS SIGNED),
                CAST(SUM(CASE WHEN bullish_engulfing = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED),
                COALESCE(CAST(AVG(CASE WHEN bullish_engulfing = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0),
                COALESCE(CAST(AVG(CASE WHEN bullish_engulfing = TRUE AND trade_result = 1 THEN 1.0 WHEN bullish_engulfing = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0)
            FROM filtered_patterns

            UNION ALL

            SELECT
                'bearish_engulfing',
                'BearishEngulfing',
                8,
                CAST(SUM(CASE WHEN bearish_engulfing = TRUE THEN 1 ELSE 0 END) AS SIGNED),
                CAST(SUM(CASE WHEN bearish_engulfing = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED),
                COALESCE(CAST(AVG(CASE WHEN bearish_engulfing = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0),
                COALESCE(CAST(AVG(CASE WHEN bearish_engulfing = TRUE AND trade_result = 1 THEN 1.0 WHEN bearish_engulfing = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0)
            FROM filtered_patterns

            UNION ALL

            SELECT
                'evening_star',
                'EveningStar',
                10,
                CAST(SUM(CASE WHEN evening_star = TRUE THEN 1 ELSE 0 END) AS SIGNED),
                CAST(SUM(CASE WHEN evening_star = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED),
                COALESCE(CAST(AVG(CASE WHEN evening_star = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0),
                COALESCE(CAST(AVG(CASE WHEN evening_star = TRUE AND trade_result = 1 THEN 1.0 WHEN evening_star = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0)
            FROM filtered_patterns

            UNION ALL

            SELECT
                'three_white_soldiers',
                'ThreeWhiteSoldiers',
                11,
                CAST(SUM(CASE WHEN three_white_soldiers = TRUE THEN 1 ELSE 0 END) AS SIGNED),
                CAST(SUM(CASE WHEN three_white_soldiers = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED),
                COALESCE(CAST(AVG(CASE WHEN three_white_soldiers = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0),
                COALESCE(CAST(AVG(CASE WHEN three_white_soldiers = TRUE AND trade_result = 1 THEN 1.0 WHEN three_white_soldiers = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0)
            FROM filtered_patterns

            UNION ALL

            SELECT
                'three_black_crows',
                'ThreeBlackCrows',
                12,
                CAST(SUM(CASE WHEN three_black_crows = TRUE THEN 1 ELSE 0 END) AS SIGNED),
                CAST(SUM(CASE WHEN three_black_crows = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED),
                COALESCE(CAST(AVG(CASE WHEN three_black_crows = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0),
                COALESCE(CAST(AVG(CASE WHEN three_black_crows = TRUE AND trade_result = 1 THEN 1.0 WHEN three_black_crows = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0)
            FROM filtered_patterns

            UNION ALL

            SELECT
                'morning_star',
                'MorningStar',
                9,
                CAST(SUM(CASE WHEN morning_star = TRUE THEN 1 ELSE 0 END) AS SIGNED),
                CAST(SUM(CASE WHEN morning_star = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED),
                COALESCE(CAST(AVG(CASE WHEN morning_star = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0),
                COALESCE(CAST(AVG(CASE WHEN morning_star = TRUE AND trade_result = 1 THEN 1.0 WHEN morning_star = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0)
            FROM filtered_patterns
        ) reversal_bucket_rollup
        WHERE total_count > 0
        ORDER BY bucket_order ASC, total_count DESC
        "#
    );

    let signal_buckets_sql = format!(
        r#"
        {filtered_patterns_cte}
        SELECT
            signal_key,
            signal_label,
            signal_order,
            total_count,
            closed_count,
            expectancy,
            win_rate
        FROM (
            SELECT
                'bullish_key_reversal' AS signal_key,
                'Bullish Key Reversal' AS signal_label,
                1 AS signal_order,
                CAST(SUM(CASE WHEN bullish_key_reversal = TRUE THEN 1 ELSE 0 END) AS SIGNED) AS total_count,
                CAST(SUM(CASE WHEN bullish_key_reversal = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
                COALESCE(CAST(AVG(CASE WHEN bullish_key_reversal = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0) AS expectancy,
                COALESCE(CAST(AVG(CASE WHEN bullish_key_reversal = TRUE AND trade_result = 1 THEN 1.0 WHEN bullish_key_reversal = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0) AS win_rate
            FROM filtered_patterns

            UNION ALL

            SELECT
                'bearish_key_reversal' AS signal_key,
                'Bearish Key Reversal' AS signal_label,
                2 AS signal_order,
                CAST(SUM(CASE WHEN bearish_key_reversal = TRUE THEN 1 ELSE 0 END) AS SIGNED) AS total_count,
                CAST(SUM(CASE WHEN bearish_key_reversal = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
                COALESCE(CAST(AVG(CASE WHEN bearish_key_reversal = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0) AS expectancy,
                COALESCE(CAST(AVG(CASE WHEN bearish_key_reversal = TRUE AND trade_result = 1 THEN 1.0 WHEN bearish_key_reversal = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0) AS win_rate
            FROM filtered_patterns

            UNION ALL

            SELECT
                'bullish_engulfing' AS signal_key,
                'Bullish Engulfing' AS signal_label,
                3 AS signal_order,
                CAST(SUM(CASE WHEN bullish_engulfing = TRUE THEN 1 ELSE 0 END) AS SIGNED) AS total_count,
                CAST(SUM(CASE WHEN bullish_engulfing = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
                COALESCE(CAST(AVG(CASE WHEN bullish_engulfing = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0) AS expectancy,
                COALESCE(CAST(AVG(CASE WHEN bullish_engulfing = TRUE AND trade_result = 1 THEN 1.0 WHEN bullish_engulfing = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0) AS win_rate
            FROM filtered_patterns

            UNION ALL

            SELECT
                'bearish_engulfing' AS signal_key,
                'Bearish Engulfing' AS signal_label,
                4 AS signal_order,
                CAST(SUM(CASE WHEN bearish_engulfing = TRUE THEN 1 ELSE 0 END) AS SIGNED) AS total_count,
                CAST(SUM(CASE WHEN bearish_engulfing = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
                COALESCE(CAST(AVG(CASE WHEN bearish_engulfing = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0) AS expectancy,
                COALESCE(CAST(AVG(CASE WHEN bearish_engulfing = TRUE AND trade_result = 1 THEN 1.0 WHEN bearish_engulfing = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0) AS win_rate
            FROM filtered_patterns

            UNION ALL

            SELECT
                'bullish_outside_reversal' AS signal_key,
                'Bullish Outside Reversal' AS signal_label,
                5 AS signal_order,
                CAST(SUM(CASE WHEN bullish_outside_reversal = TRUE THEN 1 ELSE 0 END) AS SIGNED) AS total_count,
                CAST(SUM(CASE WHEN bullish_outside_reversal = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
                COALESCE(CAST(AVG(CASE WHEN bullish_outside_reversal = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0) AS expectancy,
                COALESCE(CAST(AVG(CASE WHEN bullish_outside_reversal = TRUE AND trade_result = 1 THEN 1.0 WHEN bullish_outside_reversal = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0) AS win_rate
            FROM filtered_patterns

            UNION ALL

            SELECT
                'bearish_outside_reversal' AS signal_key,
                'Bearish Outside Reversal' AS signal_label,
                6 AS signal_order,
                CAST(SUM(CASE WHEN bearish_outside_reversal = TRUE THEN 1 ELSE 0 END) AS SIGNED) AS total_count,
                CAST(SUM(CASE WHEN bearish_outside_reversal = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
                COALESCE(CAST(AVG(CASE WHEN bearish_outside_reversal = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0) AS expectancy,
                COALESCE(CAST(AVG(CASE WHEN bearish_outside_reversal = TRUE AND trade_result = 1 THEN 1.0 WHEN bearish_outside_reversal = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0) AS win_rate
            FROM filtered_patterns

            UNION ALL

            SELECT
                'hammer' AS signal_key,
                'Hammer' AS signal_label,
                7 AS signal_order,
                CAST(SUM(CASE WHEN hammer = TRUE THEN 1 ELSE 0 END) AS SIGNED) AS total_count,
                CAST(SUM(CASE WHEN hammer = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
                COALESCE(CAST(AVG(CASE WHEN hammer = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0) AS expectancy,
                COALESCE(CAST(AVG(CASE WHEN hammer = TRUE AND trade_result = 1 THEN 1.0 WHEN hammer = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0) AS win_rate
            FROM filtered_patterns

            UNION ALL

            SELECT
                'shooting_star' AS signal_key,
                'Shooting Star' AS signal_label,
                8 AS signal_order,
                CAST(SUM(CASE WHEN shooting_star = TRUE THEN 1 ELSE 0 END) AS SIGNED) AS total_count,
                CAST(SUM(CASE WHEN shooting_star = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
                COALESCE(CAST(AVG(CASE WHEN shooting_star = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0) AS expectancy,
                COALESCE(CAST(AVG(CASE WHEN shooting_star = TRUE AND trade_result = 1 THEN 1.0 WHEN shooting_star = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0) AS win_rate
            FROM filtered_patterns

            UNION ALL

            SELECT
                'evening_star' AS signal_key,
                'Evening Star' AS signal_label,
                10 AS signal_order,
                CAST(SUM(CASE WHEN evening_star = TRUE THEN 1 ELSE 0 END) AS SIGNED) AS total_count,
                CAST(SUM(CASE WHEN evening_star = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
                COALESCE(CAST(AVG(CASE WHEN evening_star = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0) AS expectancy,
                COALESCE(CAST(AVG(CASE WHEN evening_star = TRUE AND trade_result = 1 THEN 1.0 WHEN evening_star = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0) AS win_rate
            FROM filtered_patterns

            UNION ALL

            SELECT
                'three_white_soldiers' AS signal_key,
                'Three White Soldiers' AS signal_label,
                11 AS signal_order,
                CAST(SUM(CASE WHEN three_white_soldiers = TRUE THEN 1 ELSE 0 END) AS SIGNED) AS total_count,
                CAST(SUM(CASE WHEN three_white_soldiers = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
                COALESCE(CAST(AVG(CASE WHEN three_white_soldiers = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0) AS expectancy,
                COALESCE(CAST(AVG(CASE WHEN three_white_soldiers = TRUE AND trade_result = 1 THEN 1.0 WHEN three_white_soldiers = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0) AS win_rate
            FROM filtered_patterns

            UNION ALL

            SELECT
                'three_black_crows' AS signal_key,
                'Three Black Crows' AS signal_label,
                12 AS signal_order,
                CAST(SUM(CASE WHEN three_black_crows = TRUE THEN 1 ELSE 0 END) AS SIGNED) AS total_count,
                CAST(SUM(CASE WHEN three_black_crows = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
                COALESCE(CAST(AVG(CASE WHEN three_black_crows = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0) AS expectancy,
                COALESCE(CAST(AVG(CASE WHEN three_black_crows = TRUE AND trade_result = 1 THEN 1.0 WHEN three_black_crows = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0) AS win_rate
            FROM filtered_patterns

            UNION ALL

            SELECT
                'morning_star' AS signal_key,
                'Morning Star' AS signal_label,
                9 AS signal_order,
                CAST(SUM(CASE WHEN morning_star = TRUE THEN 1 ELSE 0 END) AS SIGNED) AS total_count,
                CAST(SUM(CASE WHEN morning_star = TRUE AND trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
                COALESCE(CAST(AVG(CASE WHEN morning_star = TRUE AND trade_result IN (1, 2) THEN trade_pnl ELSE NULL END) AS DOUBLE), 0.0) AS expectancy,
                COALESCE(CAST(AVG(CASE WHEN morning_star = TRUE AND trade_result = 1 THEN 1.0 WHEN morning_star = TRUE AND trade_result = 2 THEN 0.0 ELSE NULL END) AS DOUBLE), 0.0) AS win_rate
            FROM filtered_patterns
        ) signal_rollup
        ORDER BY signal_order ASC
        "#
    );

    let mut summary_query = sqlx::query_as::<_, PatternReversalSummary>(&summary_sql);
    let mut reversal_types_query =
        sqlx::query_as::<_, PatternReversalTypeBucket>(&reversal_types_sql);
    let mut signal_buckets_query =
        sqlx::query_as::<_, PatternReversalSignalBucket>(&signal_buckets_sql);

    if let Some((_, _, min, max)) = harmonic_scope {
        summary_query = summary_query.bind(min).bind(max);
        reversal_types_query = reversal_types_query.bind(min).bind(max);
        signal_buckets_query = signal_buckets_query.bind(min).bind(max);
    } else if let Some((min, max)) = all_pattern_bin_scope {
        summary_query = summary_query.bind(min).bind(max);
        reversal_types_query = reversal_types_query.bind(min).bind(max);
        signal_buckets_query = signal_buckets_query.bind(min).bind(max);
    }

    if let Some(market) = market_filter {
        summary_query = summary_query.bind(market);
        reversal_types_query = reversal_types_query.bind(market);
        signal_buckets_query = signal_buckets_query.bind(market);
    }

    if let Some(trade_result) = trade_result_filter {
        summary_query = summary_query.bind(trade_result);
        reversal_types_query = reversal_types_query.bind(trade_result);
        signal_buckets_query = signal_buckets_query.bind(trade_result);
    }

    if let Some(days) = recent_days {
        summary_query = summary_query.bind(days);
        reversal_types_query = reversal_types_query.bind(days);
        signal_buckets_query = signal_buckets_query.bind(days);
    }

    if let Some(size_bucket) = params.size_bucket.as_deref() {
        summary_query = summary_query.bind(size_bucket);
        reversal_types_query = reversal_types_query.bind(size_bucket);
        signal_buckets_query = signal_buckets_query.bind(size_bucket);
    }

    if let Some(balance_bucket) = params.balance_bucket.as_deref() {
        summary_query = summary_query.bind(balance_bucket);
        reversal_types_query = reversal_types_query.bind(balance_bucket);
        signal_buckets_query = signal_buckets_query.bind(balance_bucket);
    }

    if let Some(retracement) = &params.retracement {
        summary_query = summary_query
            .bind(retracement.ab_xa_gr)
            .bind(retracement.ab_xa_lt)
            .bind(retracement.bc_ab_gr)
            .bind(retracement.bc_ab_lt)
            .bind(retracement.cd_bc_gr)
            .bind(retracement.cd_bc_lt)
            .bind(retracement.cd_xa_gr)
            .bind(retracement.cd_xa_lt);

        reversal_types_query = reversal_types_query
            .bind(retracement.ab_xa_gr)
            .bind(retracement.ab_xa_lt)
            .bind(retracement.bc_ab_gr)
            .bind(retracement.bc_ab_lt)
            .bind(retracement.cd_bc_gr)
            .bind(retracement.cd_bc_lt)
            .bind(retracement.cd_xa_gr)
            .bind(retracement.cd_xa_lt);

        signal_buckets_query = signal_buckets_query
            .bind(retracement.ab_xa_gr)
            .bind(retracement.ab_xa_lt)
            .bind(retracement.bc_ab_gr)
            .bind(retracement.bc_ab_lt)
            .bind(retracement.cd_bc_gr)
            .bind(retracement.cd_bc_lt)
            .bind(retracement.cd_xa_gr)
            .bind(retracement.cd_xa_lt);
    }

    let summary = match summary_query.fetch_one(pool.get_ref()).await {
        Ok(row) => row,
        Err(error) => {
            eprintln!("Pattern reversal summary DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let reversal_types = match reversal_types_query.fetch_all(pool.get_ref()).await {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("Pattern reversal types DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let signal_buckets = match signal_buckets_query.fetch_all(pool.get_ref()).await {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("Pattern reversal signal DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().json(PatternReversalResponse {
        summary,
        reversal_types,
        signal_buckets,
    })
}

#[route("/pattern-summaries", method = "GET", method = "POST")]
async fn fetch_pattern_summaries(
    pool: web::Data<MySqlPool>,
    params: web::Json<FilterParams>,
) -> impl Responder {
    println!("🔔 Handler called: fetch_pattern_summaries");
    println!("📥 Received filter: {:?}", params);

    let limit = params.limit.unwrap_or(250).clamp(1, 1_000);
    let offset = params.offset.unwrap_or(0).max(0);

    let (
        where_clause,
        harmonic_scope,
        all_pattern_bin_scope,
        market_filter,
        trade_result_filter,
        recent_days,
    ) = match build_patterns_where_clause(&params) {
        Ok(parts) => parts,
        Err(response) => return response,
    };

    let include_count = params.include_count.unwrap_or(false);
    let count_query_sql = format!(
        r#"
        SELECT COUNT(*)
        FROM xabcd_patterns
        {where_clause}
        "#
    );

    let order_clause = if recent_days.is_some() {
        "ORDER BY d_date DESC, trade_enter_price ASC"
    } else {
        ""
    };

    let balance_bucket_expr = structure_balance_bucket_expr(&structure_balance_ratio_expr());
    let dominant_time_accuracy_expr = dominant_time_accuracy_expr();
    let dominant_time_bin_expr = time_bin_expr(&dominant_time_accuracy_expr);
    let three_month_trend_expr = trend_bucket_expr("three_month");
    let six_month_trend_expr = trend_bucket_expr("six_month");
    let twelve_month_trend_expr = trend_bucket_expr("twelve_month");
    let data_query_sql = format!(
        r#"
        SELECT
            symbol,
            d_date,
            NULL AS d_confirm_date,
            NULL AS reversal_detect_date,
            target_date,
            target_open,
            target_high,
            target_low,
            target_close,
            trade_enter_price,
            trade_risk_exit_price,
            trade_reward_exit_price,
            trade_result,
            market,
            pattern_id,
            pattern_group_id,
            prop_strategy_id,
            harmonic_type,
            COALESCE(NULLIF(reversal_type, ''), 'None') AS reversal_type,
            CASE
                WHEN (COALESCE(x_length, 0) + COALESCE(a_length, 0) + COALESCE(b_length, 0) + COALESCE(c_length, 0)) <= 20 THEN 'Micro'
                WHEN (COALESCE(x_length, 0) + COALESCE(a_length, 0) + COALESCE(b_length, 0) + COALESCE(c_length, 0)) <= 60 THEN 'Small'
                WHEN (COALESCE(x_length, 0) + COALESCE(a_length, 0) + COALESCE(b_length, 0) + COALESCE(c_length, 0)) <= 180 THEN 'Normal'
                WHEN (COALESCE(x_length, 0) + COALESCE(a_length, 0) + COALESCE(b_length, 0) + COALESCE(c_length, 0)) <= 365 THEN 'Large'
                ELSE 'Massive'
            END AS size_bucket,
            {balance_bucket_expr} AS balance_bucket,
            {dominant_time_bin_expr} AS time_bin,
            {x_strictness_expr} AS x_strictness,
            {dominant_time_accuracy_expr} AS time_accuracy,
            {three_month_trend_expr} AS three_month_trend,
            {six_month_trend_expr} AS six_month_trend,
            {twelve_month_trend_expr} AS twelve_month_trend,
            CAST(x_length AS SIGNED) AS x_length,
            CAST(a_length AS SIGNED) AS a_length,
            CAST(b_length AS SIGNED) AS b_length,
            CAST(c_length AS SIGNED) AS c_length,
            CAST(d_length AS SIGNED) AS d_length,
            CAST(full_pattern_length AS SIGNED) AS full_pattern_length,
            bat_accuracy,
            butterfly_accuracy,
            gartley_accuracy,
            crab_accuracy,
            shark_accuracy
        FROM xabcd_patterns
        {where_clause}
        {order_clause}
        LIMIT ? OFFSET ?
        "#,
        balance_bucket_expr = balance_bucket_expr,
        dominant_time_bin_expr = dominant_time_bin_expr,
        x_strictness_expr = x_strictness_expr("x_bars_left", "x_length"),
        dominant_time_accuracy_expr = dominant_time_accuracy_expr,
        three_month_trend_expr = three_month_trend_expr,
        six_month_trend_expr = six_month_trend_expr,
        twelve_month_trend_expr = twelve_month_trend_expr,
    );

    let mut pattern_query = sqlx::query_as::<_, PatternSummary>(&data_query_sql);

    if let Some((_, _, min, max)) = harmonic_scope {
        pattern_query = pattern_query.bind(min).bind(max);
    } else if let Some((min, max)) = all_pattern_bin_scope {
        pattern_query = pattern_query.bind(min).bind(max);
    }

    if let Some(market) = market_filter {
        pattern_query = pattern_query.bind(market);
    }

    if let Some(trade_result) = trade_result_filter {
        pattern_query = pattern_query.bind(trade_result);
    }

    if let Some(days) = recent_days {
        pattern_query = pattern_query.bind(days);
    }

    if let Some(size_bucket) = params.size_bucket.as_deref() {
        pattern_query = pattern_query.bind(size_bucket);
    }

    if let Some(balance_bucket) = params.balance_bucket.as_deref() {
        pattern_query = pattern_query.bind(balance_bucket);
    }

    if let Some(reversal_type) = params.reversal_type.as_deref() {
        pattern_query = pattern_query.bind(reversal_type);
    }

    if let Some(retracement) = &params.retracement {
        pattern_query = pattern_query
            .bind(retracement.ab_xa_gr)
            .bind(retracement.ab_xa_lt)
            .bind(retracement.bc_ab_gr)
            .bind(retracement.bc_ab_lt)
            .bind(retracement.cd_bc_gr)
            .bind(retracement.cd_bc_lt)
            .bind(retracement.cd_xa_gr)
            .bind(retracement.cd_xa_lt);
    }

    let total_count = if include_count {
        let mut count_query = sqlx::query_scalar::<_, i64>(&count_query_sql);

        if let Some((_, _, min, max)) = harmonic_scope {
            count_query = count_query.bind(min).bind(max);
        } else if let Some((min, max)) = all_pattern_bin_scope {
            count_query = count_query.bind(min).bind(max);
        }

        if let Some(market) = market_filter {
            count_query = count_query.bind(market);
        }

        if let Some(trade_result) = trade_result_filter {
            count_query = count_query.bind(trade_result);
        }

        if let Some(days) = recent_days {
            count_query = count_query.bind(days);
        }

        if let Some(size_bucket) = params.size_bucket.as_deref() {
            count_query = count_query.bind(size_bucket);
        }

        if let Some(balance_bucket) = params.balance_bucket.as_deref() {
            count_query = count_query.bind(balance_bucket);
        }

        if let Some(reversal_type) = params.reversal_type.as_deref() {
            count_query = count_query.bind(reversal_type);
        }

        if let Some(retracement) = &params.retracement {
            count_query = count_query
                .bind(retracement.ab_xa_gr)
                .bind(retracement.ab_xa_lt)
                .bind(retracement.bc_ab_gr)
                .bind(retracement.bc_ab_lt)
                .bind(retracement.cd_bc_gr)
                .bind(retracement.cd_bc_lt)
                .bind(retracement.cd_xa_gr)
                .bind(retracement.cd_xa_lt);
        }

        match count_query.fetch_one(pool.get_ref()).await {
            Ok(count) => count,
            Err(error) => {
                eprintln!("Summary count DB error: {:?}", error);
                return HttpResponse::InternalServerError().finish();
            }
        }
    } else {
        -1
    };

    let fetch_limit = if include_count { limit } else { limit + 1 };

    pattern_query = pattern_query.bind(fetch_limit).bind(offset);

    let mut patterns = match pattern_query.fetch_all(pool.get_ref()).await {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("Summary DB error: {:?}", error);
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
    })
}

#[route("/reversal-pattern-summaries", method = "GET", method = "POST")]
async fn fetch_reversal_pattern_summaries(
    pool: web::Data<MySqlPool>,
    params: web::Json<FilterParams>,
) -> impl Responder {
    println!("🔔 Handler called: fetch_reversal_pattern_summaries");
    println!("📥 Received filter: {:?}", params);

    let limit = params.limit.unwrap_or(250).clamp(1, 1_000);
    let offset = params.offset.unwrap_or(0).max(0);

    if let Some(signal) = params.reversal_signal.as_deref() {
        if !is_supported_reversal_signal(signal) {
            return HttpResponse::BadRequest().body("Unsupported reversal signal");
        }
    }

    let (
        mut where_clause,
        harmonic_scope,
        all_pattern_bin_scope,
        market_filter,
        trade_result_filter,
        recent_days,
    ) = match build_patterns_where_clause(&params) {
        Ok(parts) => parts,
        Err(response) => return response,
    };

    if let Some(signal) = params.reversal_signal.as_deref() {
        where_clause.push_str(&format!(" AND {signal} = TRUE"));
    }

    let include_count = params.include_count.unwrap_or(false);
    let count_query_sql = format!(
        r#"
        SELECT COUNT(*)
        FROM xabcd_patterns
        {where_clause}
        "#
    );

    let order_clause = if recent_days.is_some() {
        "ORDER BY d_date DESC, trade_enter_price ASC"
    } else {
        "ORDER BY d_date DESC, trade_enter_price ASC"
    };

    let balance_bucket_expr = structure_balance_bucket_expr(&structure_balance_ratio_expr());
    let dominant_time_accuracy_expr = dominant_time_accuracy_expr();
    let dominant_time_bin_expr = time_bin_expr(&dominant_time_accuracy_expr);
    let three_month_trend_expr = trend_bucket_expr("three_month");
    let six_month_trend_expr = trend_bucket_expr("six_month");
    let twelve_month_trend_expr = trend_bucket_expr("twelve_month");
    let data_query_sql = format!(
        r#"
        SELECT
            symbol,
            d_date,
            trade_enter_price,
            trade_risk_exit_price,
            trade_reward_exit_price,
            trade_result,
            market,
            pattern_id,
            pattern_group_id,
            prop_strategy_id,
            harmonic_type,
            COALESCE(NULLIF(reversal_type, ''), 'None') AS reversal_type,
            CASE
                WHEN (COALESCE(x_length, 0) + COALESCE(a_length, 0) + COALESCE(b_length, 0) + COALESCE(c_length, 0)) <= 20 THEN 'Micro'
                WHEN (COALESCE(x_length, 0) + COALESCE(a_length, 0) + COALESCE(b_length, 0) + COALESCE(c_length, 0)) <= 60 THEN 'Small'
                WHEN (COALESCE(x_length, 0) + COALESCE(a_length, 0) + COALESCE(b_length, 0) + COALESCE(c_length, 0)) <= 180 THEN 'Normal'
                WHEN (COALESCE(x_length, 0) + COALESCE(a_length, 0) + COALESCE(b_length, 0) + COALESCE(c_length, 0)) <= 365 THEN 'Large'
                ELSE 'Massive'
            END AS size_bucket,
            {balance_bucket_expr} AS balance_bucket,
            {dominant_time_bin_expr} AS time_bin,
            {x_strictness_expr} AS x_strictness,
            {dominant_time_accuracy_expr} AS time_accuracy,
            {three_month_trend_expr} AS three_month_trend,
            {six_month_trend_expr} AS six_month_trend,
            {twelve_month_trend_expr} AS twelve_month_trend,
            CAST(x_length AS SIGNED) AS x_length,
            CAST(a_length AS SIGNED) AS a_length,
            CAST(b_length AS SIGNED) AS b_length,
            CAST(c_length AS SIGNED) AS c_length,
            CAST(d_length AS SIGNED) AS d_length,
            CAST(full_pattern_length AS SIGNED) AS full_pattern_length,
            bat_accuracy,
            butterfly_accuracy,
            gartley_accuracy,
            crab_accuracy,
            shark_accuracy
        FROM xabcd_patterns
        {where_clause}
        {order_clause}
        LIMIT ? OFFSET ?
        "#,
        balance_bucket_expr = balance_bucket_expr,
        dominant_time_bin_expr = dominant_time_bin_expr,
        x_strictness_expr = x_strictness_expr("x_bars_left", "x_length"),
        dominant_time_accuracy_expr = dominant_time_accuracy_expr,
        three_month_trend_expr = three_month_trend_expr,
        six_month_trend_expr = six_month_trend_expr,
        twelve_month_trend_expr = twelve_month_trend_expr,
    );

    let mut pattern_query = sqlx::query_as::<_, PatternSummary>(&data_query_sql);

    if let Some((_, _, min, max)) = harmonic_scope {
        pattern_query = pattern_query.bind(min).bind(max);
    } else if let Some((min, max)) = all_pattern_bin_scope {
        pattern_query = pattern_query.bind(min).bind(max);
    }

    if let Some(market) = market_filter {
        pattern_query = pattern_query.bind(market);
    }

    if let Some(trade_result) = trade_result_filter {
        pattern_query = pattern_query.bind(trade_result);
    }

    if let Some(days) = recent_days {
        pattern_query = pattern_query.bind(days);
    }

    if let Some(size_bucket) = params.size_bucket.as_deref() {
        pattern_query = pattern_query.bind(size_bucket);
    }

    if let Some(balance_bucket) = params.balance_bucket.as_deref() {
        pattern_query = pattern_query.bind(balance_bucket);
    }

    if let Some(retracement) = &params.retracement {
        pattern_query = pattern_query
            .bind(retracement.ab_xa_gr)
            .bind(retracement.ab_xa_lt)
            .bind(retracement.bc_ab_gr)
            .bind(retracement.bc_ab_lt)
            .bind(retracement.cd_bc_gr)
            .bind(retracement.cd_bc_lt)
            .bind(retracement.cd_xa_gr)
            .bind(retracement.cd_xa_lt);
    }

    let total_count = if include_count {
        let mut count_query = sqlx::query_scalar::<_, i64>(&count_query_sql);

        if let Some((_, _, min, max)) = harmonic_scope {
            count_query = count_query.bind(min).bind(max);
        } else if let Some((min, max)) = all_pattern_bin_scope {
            count_query = count_query.bind(min).bind(max);
        }

        if let Some(market) = market_filter {
            count_query = count_query.bind(market);
        }

        if let Some(trade_result) = trade_result_filter {
            count_query = count_query.bind(trade_result);
        }

        if let Some(days) = recent_days {
            count_query = count_query.bind(days);
        }

        if let Some(size_bucket) = params.size_bucket.as_deref() {
            count_query = count_query.bind(size_bucket);
        }

        if let Some(balance_bucket) = params.balance_bucket.as_deref() {
            count_query = count_query.bind(balance_bucket);
        }

        if let Some(retracement) = &params.retracement {
            count_query = count_query
                .bind(retracement.ab_xa_gr)
                .bind(retracement.ab_xa_lt)
                .bind(retracement.bc_ab_gr)
                .bind(retracement.bc_ab_lt)
                .bind(retracement.cd_bc_gr)
                .bind(retracement.cd_bc_lt)
                .bind(retracement.cd_xa_gr)
                .bind(retracement.cd_xa_lt);
        }

        match count_query.fetch_one(pool.get_ref()).await {
            Ok(count) => count,
            Err(error) => {
                eprintln!("Reversal summary count DB error: {:?}", error);
                return HttpResponse::InternalServerError().finish();
            }
        }
    } else {
        -1
    };

    let fetch_limit = if include_count { limit } else { limit + 1 };
    pattern_query = pattern_query.bind(fetch_limit).bind(offset);

    let mut patterns = match pattern_query.fetch_all(pool.get_ref()).await {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("Reversal summary data DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let has_more = if !include_count && patterns.len() as i64 > limit {
        patterns.truncate(limit as usize);
        true
    } else if include_count {
        offset + limit < total_count
    } else {
        false
    };

    HttpResponse::Ok().json(PatternSummariesResponse {
        patterns,
        total_count,
        has_more,
    })
}

#[route("/pattern-detail", method = "GET", method = "POST")]
async fn fetch_pattern_detail(
    pool: web::Data<MySqlPool>,
    params: web::Json<PatternDetailParams>,
) -> impl Responder {
    let prefer_prop_outcomes = params
        .prop_outcome_mode
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();

    if prefer_prop_outcomes {
        match fetch_pattern_detail_from_prop_outcomes(pool.get_ref(), &params).await {
            Ok(Some(pattern)) => return HttpResponse::Ok().json(pattern),
            Ok(None) => {}
            Err(error) => {
                eprintln!(
                    "Pattern detail prop-outcome reconstruction failed, checking pattern tables: {:?}",
                    error
                );
            }
        }
    }

    match fetch_pattern_detail_from_pattern_tables(pool.get_ref(), &params).await {
        Ok(Some(pattern)) => return HttpResponse::Ok().json(pattern),
        Ok(None) => {}
        Err(error) => {
            eprintln!(
                "Pattern detail pattern-table lookup failed, checking legacy mirror: {:?}",
                error
            );
        }
    }

    if !prefer_prop_outcomes {
        match fetch_pattern_detail_from_prop_outcomes(pool.get_ref(), &params).await {
            Ok(Some(pattern)) => return HttpResponse::Ok().json(pattern),
            Ok(None) => {}
            Err(error) => {
                eprintln!(
                    "Pattern detail prop-outcome reconstruction failed, checking legacy mirror: {:?}",
                    error
                );
            }
        }
    }

    let balance_bucket_expr = structure_balance_bucket_expr(&structure_balance_ratio_expr());
    let has_pattern_id = params
        .pattern_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();
    let mut sql = if has_pattern_id {
        String::from(
            r#"
            SELECT *
            FROM xabcd_patterns
            WHERE pattern_id = ?
            "#,
        )
    } else {
        String::from(
            r#"
            SELECT *
            FROM xabcd_patterns
            WHERE pattern_group_id = ?
            "#,
        )
    };

    if params.d_date.is_some() {
        sql.push_str(" AND d_date = ?");
    }

    if params.x_date.is_some() {
        sql.push_str(" AND x_date = ?");
    }

    if params.market.is_some() {
        sql.push_str(" AND market = ?");
    }

    if params.harmonic_type.is_some() {
        sql.push_str(" AND harmonic_type = ?");
    }

    if params.size_bucket.is_some() {
        sql.push_str(
            " AND CASE
                WHEN (COALESCE(x_length, 0) + COALESCE(a_length, 0) + COALESCE(b_length, 0) + COALESCE(c_length, 0)) <= 20 THEN 'Micro'
                WHEN (COALESCE(x_length, 0) + COALESCE(a_length, 0) + COALESCE(b_length, 0) + COALESCE(c_length, 0)) <= 60 THEN 'Small'
                WHEN (COALESCE(x_length, 0) + COALESCE(a_length, 0) + COALESCE(b_length, 0) + COALESCE(c_length, 0)) <= 180 THEN 'Normal'
                WHEN (COALESCE(x_length, 0) + COALESCE(a_length, 0) + COALESCE(b_length, 0) + COALESCE(c_length, 0)) <= 365 THEN 'Large'
                ELSE 'Massive'
            END = ?",
        );
    }

    if params.balance_bucket.is_some() {
        sql.push_str(&format!(" AND {balance_bucket_expr} = ?"));
    }

    if params.trade_enter_price.is_some() {
        sql.push_str(" AND trade_enter_price = ?");
    }

    if params.trade_risk_exit_price.is_some() {
        sql.push_str(" AND trade_risk_exit_price = ?");
    }

    if params.trade_reward_exit_price.is_some() {
        sql.push_str(" AND trade_reward_exit_price = ?");
    }

    if params.x_length.is_some() {
        sql.push_str(" AND x_length = ?");
    }

    if params.a_length.is_some() {
        sql.push_str(" AND a_length = ?");
    }

    if params.b_length.is_some() {
        sql.push_str(" AND b_length = ?");
    }

    if params.c_length.is_some() {
        sql.push_str(" AND c_length = ?");
    }

    sql.push_str(" LIMIT 1");

    let mut query = if let Some(pattern_id) = params
        .pattern_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        sqlx::query_as::<_, Pattern>(&sql).bind(pattern_id)
    } else {
        sqlx::query_as::<_, Pattern>(&sql).bind(params.pattern_group_id.clone())
    };

    if let Some(d_date) = params.d_date {
        query = query.bind(d_date);
    }

    if let Some(x_date) = params.x_date {
        query = query.bind(x_date);
    }

    if let Some(market) = params.market.as_deref() {
        query = query.bind(market);
    }

    if let Some(harmonic_type) = params.harmonic_type.as_deref() {
        query = query.bind(harmonic_type);
    }

    if let Some(size_bucket) = params.size_bucket.as_deref() {
        query = query.bind(size_bucket);
    }

    if let Some(balance_bucket) = params.balance_bucket.as_deref() {
        query = query.bind(balance_bucket);
    }

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

    let pattern = match query.fetch_optional(pool.get_ref()).await {
        Ok(Some(pattern)) => pattern,
        Ok(None) => {
            let mut fallback_sql = if has_pattern_id {
                String::from(
                    r#"
                    SELECT *
                    FROM xabcd_patterns
                    WHERE pattern_id = ?
                    "#,
                )
            } else {
                String::from(
                    r#"
                    SELECT *
                    FROM xabcd_patterns
                    WHERE pattern_group_id = ?
                    "#,
                )
            };

            if params.d_date.is_some() {
                fallback_sql.push_str(" AND d_date = ?");
            }

            if params.x_date.is_some() {
                fallback_sql.push_str(" AND x_date = ?");
            }

            if params.market.is_some() {
                fallback_sql.push_str(" AND market = ?");
            }

            if params.harmonic_type.is_some() {
                fallback_sql.push_str(" AND harmonic_type = ?");
            }

            fallback_sql.push_str(" ORDER BY d_date DESC, trade_enter_price ASC LIMIT 1");

            let mut fallback_query = if let Some(pattern_id) = params
                .pattern_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                sqlx::query_as::<_, Pattern>(&fallback_sql).bind(pattern_id)
            } else {
                sqlx::query_as::<_, Pattern>(&fallback_sql).bind(params.pattern_group_id.clone())
            };

            if let Some(d_date) = params.d_date {
                fallback_query = fallback_query.bind(d_date);
            }

            if let Some(x_date) = params.x_date {
                fallback_query = fallback_query.bind(x_date);
            }

            if let Some(market) = params.market.as_deref() {
                fallback_query = fallback_query.bind(market);
            }

            if let Some(harmonic_type) = params.harmonic_type.as_deref() {
                fallback_query = fallback_query.bind(harmonic_type);
            }

            match fallback_query.fetch_optional(pool.get_ref()).await {
                Ok(Some(pattern)) => {
                    eprintln!(
                        "Pattern detail strict lookup missed, fallback resolved row for group={} d_date={:?}",
                        params.pattern_group_id, params.d_date
                    );
                    pattern
                }
                Ok(None) => {
                    let Some(symbol) = params.symbol.as_deref() else {
                        return HttpResponse::NotFound().body("Pattern not found");
                    };

                    let Some(d_date) = params.d_date else {
                        return HttpResponse::NotFound().body("Pattern not found");
                    };

                    let mut symbol_sql = String::from(
                        r#"
                        SELECT *
                        FROM xabcd_patterns
                        WHERE symbol = ?
                          AND d_date = ?
                        "#,
                    );

                    if params.trade_enter_price.is_some() {
                        symbol_sql.push_str(
                            " ORDER BY ABS(CAST(trade_enter_price AS DOUBLE) - CAST(? AS DOUBLE)) ASC, trade_enter_price ASC LIMIT 1",
                        );
                    } else {
                        symbol_sql.push_str(" ORDER BY trade_enter_price ASC LIMIT 1");
                    }

                    let mut symbol_query = sqlx::query_as::<_, Pattern>(&symbol_sql)
                        .bind(symbol)
                        .bind(d_date);

                    if let Some(trade_enter_price) = params.trade_enter_price {
                        symbol_query = symbol_query.bind(trade_enter_price);
                    }

                    match symbol_query.fetch_optional(pool.get_ref()).await {
                        Ok(Some(pattern)) => {
                            eprintln!(
                                "Pattern detail group lookup missed, symbol fallback resolved row for symbol={} d_date={}",
                                symbol, d_date
                            );
                            pattern
                        }
                        Ok(None) => return HttpResponse::NotFound().body("Pattern not found"),
                        Err(error) => {
                            eprintln!("Pattern detail symbol fallback DB error: {:?}", error);
                            return HttpResponse::InternalServerError().finish();
                        }
                    }
                }
                Err(error) => {
                    eprintln!("Pattern detail fallback DB error: {:?}", error);
                    return HttpResponse::InternalServerError().finish();
                }
            }
        }
        Err(error) => {
            eprintln!("Pattern detail DB error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().json(pattern)
}

async fn fetch_pattern_detail_from_pattern_tables(
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
    let ab_xa_expr =
        "CAST(COALESCE(ABS(s.ab_price_length) / NULLIF(ABS(s.xa_price_length), 0) * 100.0, 0.0) AS DECIMAL(12,4))";
    let bc_ab_expr =
        "CAST(COALESCE(ABS(s.bc_price_length) / NULLIF(ABS(s.ab_price_length), 0) * 100.0, 0.0) AS DECIMAL(12,4))";
    let cd_bc_expr =
        "CAST(COALESCE(ABS(s.cd_price_length) / NULLIF(ABS(s.bc_price_length), 0) * 100.0, 0.0) AS DECIMAL(12,4))";
    let cd_xa_expr =
        "CAST(COALESCE(ABS(s.cd_price_length) / NULLIF(ABS(s.xa_price_length), 0) * 100.0, 0.0) AS DECIMAL(12,4))";
    let trade_date_expr =
        "COALESCE(p.target_date, p.reversal_detect_date, p.d_confirm_date, s.d_date)";
    let entry_date_expr = "COALESCE(p.reversal_detect_date, p.d_confirm_date, s.d_date)";

    let mut sql = if has_pattern_id {
        String::from(
            r#"
            SELECT
                s.symbol,
                s.pattern_id,
                s.x_date,
                CAST(s.x_open AS DECIMAL(18,6)) AS x_open,
                CAST(s.x_high AS DECIMAL(18,6)) AS x_high,
                CAST(s.x_low AS DECIMAL(18,6)) AS x_low,
                CAST(s.x_close AS DECIMAL(18,6)) AS x_close,
                CAST(s.x_length AS DECIMAL(18,0)) AS x_length,
                CAST(s.x_min_max AS DECIMAL(18,6)) AS x_min_max,
                s.a_date,
                CAST(s.a_open AS DECIMAL(18,6)) AS a_open,
                CAST(s.a_high AS DECIMAL(18,6)) AS a_high,
                CAST(s.a_low AS DECIMAL(18,6)) AS a_low,
                CAST(s.a_close AS DECIMAL(18,6)) AS a_close,
                CAST(s.a_length AS DECIMAL(18,0)) AS a_length,
                CAST(s.a_min_max AS DECIMAL(18,6)) AS a_min_max,
                s.b_date,
                CAST(s.b_open AS DECIMAL(18,6)) AS b_open,
                CAST(s.b_high AS DECIMAL(18,6)) AS b_high,
                CAST(s.b_low AS DECIMAL(18,6)) AS b_low,
                CAST(s.b_close AS DECIMAL(18,6)) AS b_close,
                CAST(s.b_length AS DECIMAL(18,0)) AS b_length,
                CAST(s.b_min_max AS DECIMAL(18,6)) AS b_min_max,
                s.c_date,
                CAST(s.c_open AS DECIMAL(18,6)) AS c_open,
                CAST(s.c_high AS DECIMAL(18,6)) AS c_high,
                CAST(s.c_low AS DECIMAL(18,6)) AS c_low,
                CAST(s.c_close AS DECIMAL(18,6)) AS c_close,
                CAST(s.c_length AS DECIMAL(18,0)) AS c_length,
                CAST(s.c_min_max AS DECIMAL(18,6)) AS c_min_max,
                s.d_date,
                CAST(s.d_open AS DECIMAL(18,6)) AS d_open,
                CAST(s.d_high AS DECIMAL(18,6)) AS d_high,
                CAST(s.d_low AS DECIMAL(18,6)) AS d_low,
                CAST(s.d_close AS DECIMAL(18,6)) AS d_close,
                CAST(s.d_length AS DECIMAL(18,0)) AS d_length,
                CAST(s.full_pattern_length AS SIGNED) AS full_pattern_length,
                CAST(s.d_min_max AS DECIMAL(18,6)) AS d_min_max,
                CASE WHEN p.target_date IS NULL THEN TRUE ELSE FALSE END AS trade_open,
                CAST(p.trade_risk_exit_price AS DECIMAL(18,6)) AS trade_risk_exit_price,
                CAST(p.trade_reward_exit_price AS DECIMAL(18,6)) AS trade_reward_exit_price,
                CAST(p.trade_enter_price AS DECIMAL(18,6)) AS trade_enter_price,
                CAST(COALESCE(p.target_close, p.trade_enter_price) AS DECIMAL(18,6)) AS trade_current_price,
                CAST(COALESCE(DATEDIFF(COALESCE(p.target_date, CURDATE()), {entry_date_expr}), 0) AS DECIMAL(18,0)) AS trade_length,
                CAST(COALESCE(p.target_close_vs_open_pct, 0.0) AS DECIMAL(18,6)) AS trade_pnl,
                CAST(p.prop_result AS SIGNED) AS trade_result,
                {trade_date_expr} AS trade_date,
                {ab_xa_expr} AS trade_ab_price_retracement,
                {bc_ab_expr} AS trade_bc_price_retracement,
                {cd_bc_expr} AS trade_cd_bc_price_retracement,
                CAST(s.cd_price_length AS DECIMAL(18,6)) AS trade_cd_price_retracement,
                {cd_xa_expr} AS trade_cd_xa_price_retracement,
                CAST(COALESCE(s.b_length / NULLIF(s.a_length, 0) * 100.0, 0.0) AS DOUBLE) AS trade_ab_bar_retracement,
                CAST(COALESCE(s.c_length / NULLIF(s.b_length, 0) * 100.0, 0.0) AS DECIMAL(18,6)) AS trade_bc_bar_retracement,
                CAST(COALESCE(s.d_length / NULLIF(s.c_length, 0) * 100.0, 0.0) AS DECIMAL(18,6)) AS trade_cd_bar_retracement,
                CAST(COALESCE(s.d_length / NULLIF(s.c_length, 0) * 100.0, 0.0) AS DOUBLE) AS trade_cd_bc_bar_retracement,
                CAST(COALESCE(s.d_length / NULLIF(s.x_length, 0) * 100.0, 0.0) AS DOUBLE) AS trade_cd_xa_bar_retracement,
                CAST(0.0 AS DECIMAL(18,6)) AS trade_snr,
                CAST(YEAR({trade_date_expr}) AS SIGNED) AS trade_year,
                CAST(MONTH({trade_date_expr}) AS SIGNED) AS trade_month,
                CAST(DAY({trade_date_expr}) AS SIGNED) AS trade_day,
                COALESCE(NULLIF(p.reversal_type, ''), 'None') AS reversal_type,
                s.bullish_key_reversal,
                s.bearish_key_reversal,
                s.bullish_engulfing,
                s.bearish_engulfing,
                s.bullish_outside_reversal,
                s.bearish_outside_reversal,
                s.hammer,
                s.shooting_star,
                s.morning_star,
                s.evening_star,
                s.three_white_soldiers,
                s.three_black_crows,
                s.market,
                s.three_month,
                s.six_month,
                s.twelve_month,
                s.pattern_group_id,
                p.harmonic_type,
                {route_score_projection},
                CAST({route_time_accuracy_expr} AS DOUBLE) AS time_accuracy
            FROM pattern_setups s
            INNER JOIN pattern_outcomes_prop p
                ON p.setup_id = s.setup_id
            WHERE COALESCE(NULLIF(p.pattern_id, ''), s.pattern_id) = ?
            "#,
        )
    } else {
        String::from(
            r#"
            SELECT
                s.symbol,
                s.pattern_id,
                s.x_date,
                CAST(s.x_open AS DECIMAL(18,6)) AS x_open,
                CAST(s.x_high AS DECIMAL(18,6)) AS x_high,
                CAST(s.x_low AS DECIMAL(18,6)) AS x_low,
                CAST(s.x_close AS DECIMAL(18,6)) AS x_close,
                CAST(s.x_length AS DECIMAL(18,0)) AS x_length,
                CAST(s.x_min_max AS DECIMAL(18,6)) AS x_min_max,
                s.a_date,
                CAST(s.a_open AS DECIMAL(18,6)) AS a_open,
                CAST(s.a_high AS DECIMAL(18,6)) AS a_high,
                CAST(s.a_low AS DECIMAL(18,6)) AS a_low,
                CAST(s.a_close AS DECIMAL(18,6)) AS a_close,
                CAST(s.a_length AS DECIMAL(18,0)) AS a_length,
                CAST(s.a_min_max AS DECIMAL(18,6)) AS a_min_max,
                s.b_date,
                CAST(s.b_open AS DECIMAL(18,6)) AS b_open,
                CAST(s.b_high AS DECIMAL(18,6)) AS b_high,
                CAST(s.b_low AS DECIMAL(18,6)) AS b_low,
                CAST(s.b_close AS DECIMAL(18,6)) AS b_close,
                CAST(s.b_length AS DECIMAL(18,0)) AS b_length,
                CAST(s.b_min_max AS DECIMAL(18,6)) AS b_min_max,
                s.c_date,
                CAST(s.c_open AS DECIMAL(18,6)) AS c_open,
                CAST(s.c_high AS DECIMAL(18,6)) AS c_high,
                CAST(s.c_low AS DECIMAL(18,6)) AS c_low,
                CAST(s.c_close AS DECIMAL(18,6)) AS c_close,
                CAST(s.c_length AS DECIMAL(18,0)) AS c_length,
                CAST(s.c_min_max AS DECIMAL(18,6)) AS c_min_max,
                s.d_date,
                CAST(s.d_open AS DECIMAL(18,6)) AS d_open,
                CAST(s.d_high AS DECIMAL(18,6)) AS d_high,
                CAST(s.d_low AS DECIMAL(18,6)) AS d_low,
                CAST(s.d_close AS DECIMAL(18,6)) AS d_close,
                CAST(s.d_length AS DECIMAL(18,0)) AS d_length,
                CAST(s.full_pattern_length AS SIGNED) AS full_pattern_length,
                CAST(s.d_min_max AS DECIMAL(18,6)) AS d_min_max,
                CASE WHEN p.target_date IS NULL THEN TRUE ELSE FALSE END AS trade_open,
                CAST(p.trade_risk_exit_price AS DECIMAL(18,6)) AS trade_risk_exit_price,
                CAST(p.trade_reward_exit_price AS DECIMAL(18,6)) AS trade_reward_exit_price,
                CAST(p.trade_enter_price AS DECIMAL(18,6)) AS trade_enter_price,
                CAST(COALESCE(p.target_close, p.trade_enter_price) AS DECIMAL(18,6)) AS trade_current_price,
                CAST(COALESCE(DATEDIFF(COALESCE(p.target_date, CURDATE()), {entry_date_expr}), 0) AS DECIMAL(18,0)) AS trade_length,
                CAST(COALESCE(p.target_close_vs_open_pct, 0.0) AS DECIMAL(18,6)) AS trade_pnl,
                CAST(p.prop_result AS SIGNED) AS trade_result,
                {trade_date_expr} AS trade_date,
                {ab_xa_expr} AS trade_ab_price_retracement,
                {bc_ab_expr} AS trade_bc_price_retracement,
                {cd_bc_expr} AS trade_cd_bc_price_retracement,
                CAST(s.cd_price_length AS DECIMAL(18,6)) AS trade_cd_price_retracement,
                {cd_xa_expr} AS trade_cd_xa_price_retracement,
                CAST(COALESCE(s.b_length / NULLIF(s.a_length, 0) * 100.0, 0.0) AS DOUBLE) AS trade_ab_bar_retracement,
                CAST(COALESCE(s.c_length / NULLIF(s.b_length, 0) * 100.0, 0.0) AS DECIMAL(18,6)) AS trade_bc_bar_retracement,
                CAST(COALESCE(s.d_length / NULLIF(s.c_length, 0) * 100.0, 0.0) AS DECIMAL(18,6)) AS trade_cd_bar_retracement,
                CAST(COALESCE(s.d_length / NULLIF(s.c_length, 0) * 100.0, 0.0) AS DOUBLE) AS trade_cd_bc_bar_retracement,
                CAST(COALESCE(s.d_length / NULLIF(s.x_length, 0) * 100.0, 0.0) AS DOUBLE) AS trade_cd_xa_bar_retracement,
                CAST(0.0 AS DECIMAL(18,6)) AS trade_snr,
                CAST(YEAR({trade_date_expr}) AS SIGNED) AS trade_year,
                CAST(MONTH({trade_date_expr}) AS SIGNED) AS trade_month,
                CAST(DAY({trade_date_expr}) AS SIGNED) AS trade_day,
                COALESCE(NULLIF(p.reversal_type, ''), 'None') AS reversal_type,
                s.bullish_key_reversal,
                s.bearish_key_reversal,
                s.bullish_engulfing,
                s.bearish_engulfing,
                s.bullish_outside_reversal,
                s.bearish_outside_reversal,
                s.hammer,
                s.shooting_star,
                s.morning_star,
                s.evening_star,
                s.three_white_soldiers,
                s.three_black_crows,
                s.market,
                s.three_month,
                s.six_month,
                s.twelve_month,
                s.pattern_group_id,
                p.harmonic_type,
                {route_score_projection},
                CAST({route_time_accuracy_expr} AS DOUBLE) AS time_accuracy
            FROM pattern_setups s
            INNER JOIN pattern_outcomes_prop p
                ON p.setup_id = s.setup_id
            WHERE s.pattern_group_id = ?
            "#,
        )
    };

    sql = sql
        .replace("{trade_date_expr}", trade_date_expr)
        .replace("{entry_date_expr}", entry_date_expr)
        .replace("{ab_xa_expr}", ab_xa_expr)
        .replace("{bc_ab_expr}", bc_ab_expr)
        .replace("{cd_bc_expr}", cd_bc_expr)
        .replace("{cd_xa_expr}", cd_xa_expr)
        .replace("{route_score_projection}", &route_score_projection)
        .replace("{route_time_accuracy_expr}", &route_time_accuracy_expr);

    if params.d_date.is_some() {
        sql.push_str(" AND s.d_date = ?");
    }
    if params.x_date.is_some() {
        sql.push_str(" AND s.x_date = ?");
    }
    if params.market.is_some() {
        sql.push_str(" AND s.market = ?");
    }
    if params.harmonic_type.is_some() {
        sql.push_str(" AND p.harmonic_type = ?");
    }
    if params.size_bucket.is_some() {
        sql.push_str(" AND p.size_bucket = ?");
    }
    if params.trade_enter_price.is_some() {
        sql.push_str(" AND p.trade_enter_price = ?");
    }
    if params.trade_risk_exit_price.is_some() {
        sql.push_str(" AND p.trade_risk_exit_price = ?");
    }
    if params.trade_reward_exit_price.is_some() {
        sql.push_str(" AND p.trade_reward_exit_price = ?");
    }
    if params.x_length.is_some() {
        sql.push_str(" AND s.x_length = ?");
    }
    if params.a_length.is_some() {
        sql.push_str(" AND s.a_length = ?");
    }
    if params.b_length.is_some() {
        sql.push_str(" AND s.b_length = ?");
    }
    if params.c_length.is_some() {
        sql.push_str(" AND s.c_length = ?");
    }

    sql.push_str(" ORDER BY COALESCE(p.reversal_detect_date, p.d_confirm_date, p.d_date) DESC, p.trade_enter_price ASC LIMIT 1");

    let mut query = if let Some(pattern_id) = params
        .pattern_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        sqlx::query_as::<_, Pattern>(&sql).bind(pattern_id)
    } else {
        sqlx::query_as::<_, Pattern>(&sql).bind(params.pattern_group_id.clone())
    };

    if let Some(d_date) = params.d_date {
        query = query.bind(d_date);
    }
    if let Some(x_date) = params.x_date {
        query = query.bind(x_date);
    }
    if let Some(market) = params.market.as_deref() {
        query = query.bind(market);
    }
    if let Some(harmonic_type) = params.harmonic_type.as_deref() {
        query = query.bind(harmonic_type);
    }
    if let Some(size_bucket) = params.size_bucket.as_deref() {
        query = query.bind(size_bucket);
    }
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

    match query.fetch_optional(pool).await {
        Ok(Some(row)) => Ok(Some(row)),
        Ok(None) if has_pattern_id => {
            let mut group_params = params.clone();
            group_params.pattern_id = None;
            Box::pin(fetch_pattern_detail_from_pattern_tables(
                pool,
                &group_params,
            ))
            .await
        }
        Ok(None) => Ok(None),
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

    if params.d_date.is_some() {
        outcome_where.push_str(" AND p.d_date = ?");
    }
    if params.market.is_some() {
        outcome_where.push_str(" AND p.market = ?");
    }
    if params.harmonic_type.is_some() {
        outcome_where.push_str(" AND p.harmonic_type = ?");
    }
    if params.size_bucket.is_some() {
        outcome_where.push_str(" AND p.size_bucket = ?");
    }
    if params.balance_bucket.is_some() {
        outcome_where.push_str(&format!(" AND {balance_bucket_expr} = ?"));
    }
    if params.trade_enter_price.is_some() {
        outcome_where.push_str(" AND p.trade_enter_price = ?");
    }
    if params.trade_risk_exit_price.is_some() {
        outcome_where.push_str(" AND p.trade_risk_exit_price = ?");
    }
    if params.trade_reward_exit_price.is_some() {
        outcome_where.push_str(" AND p.trade_reward_exit_price = ?");
    }
    if params.x_length.is_some() {
        outcome_where.push_str(" AND p.x_length = ?");
    }
    if params.a_length.is_some() {
        outcome_where.push_str(" AND p.a_length = ?");
    }
    if params.b_length.is_some() {
        outcome_where.push_str(" AND p.b_length = ?");
    }
    if params.c_length.is_some() {
        outcome_where.push_str(" AND p.c_length = ?");
    }

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
            FROM candles c
            INNER JOIN selected_outcome p
                ON p.symbol = c.symbol
               AND c.date <= p.d_date
        ),
        indexed_outcome AS (
            SELECT
                p.*,
                d.rn AS d_rn,
                d.rn - p.c_length AS c_rn,
                d.rn - p.c_length - p.b_length AS b_rn,
                d.rn - p.c_length - p.b_length - p.a_length AS a_rn,
                d.rn - p.c_length - p.b_length - p.a_length - p.x_length AS x_rn
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
    } else {
        sqlx::query_as::<_, Pattern>(&sql).bind(params.pattern_group_id.clone())
    };

    if let Some(d_date) = params.d_date {
        query = query.bind(d_date);
    }
    if let Some(market) = params.market.as_deref() {
        query = query.bind(market);
    }
    if let Some(harmonic_type) = params.harmonic_type.as_deref() {
        query = query.bind(harmonic_type);
    }
    if let Some(size_bucket) = params.size_bucket.as_deref() {
        query = query.bind(size_bucket);
    }
    if let Some(balance_bucket) = params.balance_bucket.as_deref() {
        query = query.bind(balance_bucket);
    }
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
    let prop_mode = params.prop_mode.unwrap_or(false);
    let prop_strategy_id = params
        .prop_strategy_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if !(prop_mode && prop_strategy_id.is_some()) {
        let Some(harmonic_type) = params.harmonic_type.as_deref() else {
            return HttpResponse::BadRequest().body("Setup decision requires harmonic type");
        };
        let Some(market) = params.market.as_deref() else {
            return HttpResponse::BadRequest().body("Setup decision requires market");
        };
        let Some(bin) = params.bin.as_deref() else {
            return HttpResponse::BadRequest().body("Setup decision requires bin");
        };
        let Some(reversal_type) = params.reversal_type.as_deref() else {
            return HttpResponse::BadRequest().body("Setup decision requires reversal type");
        };
        let Some(size_bucket) = params.size_bucket.as_deref() else {
            return HttpResponse::BadRequest().body("Setup decision requires size bucket");
        };
        let Some(time_bin) = params.time_bin.as_deref() else {
            return HttpResponse::BadRequest().body("Setup decision requires time bin");
        };
        let Some(three_month_trend) = params.three_month_trend.as_deref() else {
            return HttpResponse::BadRequest().body("Setup decision requires 3M trend");
        };
        let Some(six_month_trend) = params.six_month_trend.as_deref() else {
            return HttpResponse::BadRequest().body("Setup decision requires 6M trend");
        };
        let Some(twelve_month_trend) = params.twelve_month_trend.as_deref() else {
            return HttpResponse::BadRequest().body("Setup decision requires 12M trend");
        };

        if accuracy_column_for_harmonic_type(harmonic_type).is_none()
            || dominant_harmonic_predicate(harmonic_type).is_none()
        {
            return HttpResponse::BadRequest().body("Unsupported harmonic type");
        }
        if normalize_market_filter(Some(market)).is_none() {
            return HttpResponse::BadRequest().body("Unsupported market");
        }
        if !is_supported_bin_label(bin) {
            return HttpResponse::BadRequest().body("Unsupported bin");
        }
        if !is_supported_reversal_type(reversal_type) {
            return HttpResponse::BadRequest().body("Unsupported reversal type");
        }
        if !is_supported_size_bucket(size_bucket) {
            return HttpResponse::BadRequest().body("Unsupported size bucket");
        }
        if !is_supported_bin_label(time_bin) {
            return HttpResponse::BadRequest().body("Unsupported time bin");
        }
        if !is_supported_trend_bucket(three_month_trend)
            || !is_supported_trend_bucket(six_month_trend)
            || !is_supported_trend_bucket(twelve_month_trend)
        {
            return HttpResponse::BadRequest().body("Unsupported trend bucket");
        }
    }

    match is_rollup_cache_ready(
        pool,
        if prop_mode {
            "prop_strategy_family_rollups"
        } else {
            "strategy_live_caches"
        },
    )
    .await
    {
        Ok(true) => {}
        Ok(false) => {
            if prop_mode {
                return HttpResponse::Ok().json(empty_setup_comparison_response(params));
            }
        }
        Err(error) => {
            eprintln!("Setup decision cache readiness error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    }

    let include_examples = params.include_examples.unwrap_or(true);
    let Some((summary, yearly_performance)) = (match if prop_mode {
        try_fetch_setup_comparison_from_prop_strategy_rollup(pool, params).await
    } else {
        try_fetch_setup_comparison_from_strategy_rollup(pool, params).await
    } {
        Ok(result) => result,
        Err(error) => {
            eprintln!("Setup decision cache query error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    }) else {
        return HttpResponse::NotFound().body("Strategy cohort not found in cache");
    };

    let recent_examples = if include_examples {
        let rows = if prop_mode {
            let Some(prop_strategy_id) = prop_strategy_id else {
                return HttpResponse::BadRequest().body("Setup decision requires family key");
            };
            let family = match fetch_prop_strategy_family_filter(pool, prop_strategy_id).await {
                Ok(Some(family)) => family,
                Ok(None) => return HttpResponse::NotFound().body("Prop strategy family not found"),
                Err(error) => {
                    eprintln!("Prop strategy family examples lookup error: {:?}", error);
                    return HttpResponse::InternalServerError().finish();
                }
            };
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
                    eprintln!("Prop setup decision examples error: {:?}", error);
                    return HttpResponse::InternalServerError().finish();
                }
            }
        } else {
            let examples_sql = r#"
                SELECT
                    symbol,
                    d_date,
                    trade_result,
                    0.0 AS trade_pnl,
                    0.0 AS trade_length,
                    CAST(trade_enter_price AS DOUBLE) AS trade_enter_price
                FROM strategy_trade_summary_cache
                WHERE market = ?
                  AND harmonic_type = ?
                  AND bin = ?
                  AND reversal_type = ?
                  AND size_bucket = ?
                  AND time_bin = ?
                  AND three_month_trend = ?
                  AND six_month_trend = ?
                  AND twelve_month_trend = ?
                  AND trade_result IN (1, 2)
                ORDER BY d_date DESC, trade_enter_price ASC
                LIMIT 6
            "#;

            let Some(reversal_type) = params.reversal_type.as_deref() else {
                return HttpResponse::BadRequest().body("Setup decision requires reversal type");
            };
            let Some(size_bucket) = params.size_bucket.as_deref() else {
                return HttpResponse::BadRequest().body("Setup decision requires size bucket");
            };
            let Some(time_bin) = params.time_bin.as_deref() else {
                return HttpResponse::BadRequest().body("Setup decision requires time bin");
            };
            let Some(three_month_trend) = params.three_month_trend.as_deref() else {
                return HttpResponse::BadRequest().body("Setup decision requires 3M trend");
            };
            let Some(six_month_trend) = params.six_month_trend.as_deref() else {
                return HttpResponse::BadRequest().body("Setup decision requires 6M trend");
            };
            let Some(twelve_month_trend) = params.twelve_month_trend.as_deref() else {
                return HttpResponse::BadRequest().body("Setup decision requires 12M trend");
            };

            match sqlx::query_as::<_, SetupComparisonExample>(examples_sql)
                .bind(params.market.as_deref())
                .bind(params.harmonic_type.as_deref())
                .bind(params.bin.as_deref())
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
                Err(error) => {
                    eprintln!("Setup decision examples error: {:?}", error);
                    return HttpResponse::InternalServerError().finish();
                }
            }
        };

        rows
    } else {
        Vec::new()
    };

    HttpResponse::Ok().json(SetupComparisonResponse {
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

#[route("/setup-decision", method = "GET", method = "POST")]
async fn fetch_setup_decision(
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
    let prop_mode = params.prop_mode.unwrap_or(false);
    let prop_outcome_mode = normalize_prop_outcome_mode(params.prop_outcome_mode.as_deref());
    let min_closed_trades = params.min_closed_trades.unwrap_or(0).max(0);
    let limit = params.limit.unwrap_or(250).clamp(1, 1_000);

    match is_rollup_cache_ready(
        pool.get_ref(),
        if prop_mode {
            "prop_strategy_family_rollups"
        } else {
            "strategy_live_caches"
        },
    )
    .await
    {
        Ok(true) => {}
        Ok(false) => {
            if prop_mode {
                return HttpResponse::Ok().json(Vec::<StrategyCandidateSummary>::new());
            }
        }
        Err(error) => {
            eprintln!("Strategy candidates cache readiness error: {:?}", error);
            return HttpResponse::InternalServerError().finish();
        }
    }

    if prop_mode {
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
        let sort_column = prop_family_sort_column(params.sort_by.as_deref(), "s");
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
            CAST(s.score AS DOUBLE) AS score
        FROM prop_strategy_family_summary s
        WHERE s.closed_count >= ?
          AND s.outcome_model = ?
          {summary_filters}
        ORDER BY {sort_column} {sort_direction}, s.score DESC, s.expectancy DESC, s.closed_count DESC
        LIMIT ?
        "#,
            summary_filters = summary_filters,
            sort_column = sort_column,
            sort_direction = sort_direction,
        );

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
            Ok(rows) => HttpResponse::Ok().json(rows),
            Err(error) if is_missing_table_error(&error) => {
                HttpResponse::Ok().json(Vec::<StrategyCandidateSummary>::new())
            }
            Err(error) => {
                eprintln!("Strategy candidate DB error: {:?}", error);
                HttpResponse::InternalServerError().finish()
            }
        };
    } else {
        let sql = r#"
        SELECT
            NULL AS prop_strategy_id,
            NULL AS family_key,
            NULL AS family_name,
            NULL AS family_level,
            NULL AS included_dimensions,
            NULL AS outcome_model,
            market,
            harmonic_type,
            bin,
            reversal_type,
            size_bucket,
            time_bin,
            NULL AS x_strictness,
            three_month_trend,
            six_month_trend,
            twelve_month_trend,
            CAST(worst_year_expectancy AS DOUBLE) AS worst_year_expectancy,
            CAST(down_years AS SIGNED) AS down_years,
            CAST(total_count AS SIGNED) AS total_count,
            CAST(closed_count AS SIGNED) AS closed_count,
            CAST(open_count AS SIGNED) AS open_count,
            CAST(win_count AS SIGNED) AS win_count,
            CAST(loss_count AS SIGNED) AS loss_count,
            CAST(expectancy AS DOUBLE) AS expectancy,
            CAST(avg_return AS DOUBLE) AS avg_return,
            CAST(win_rate AS DOUBLE) AS win_rate,
            CAST(closed_rate AS DOUBLE) AS closed_rate,
            CAST(avg_win AS DOUBLE) AS avg_win,
            CAST(avg_loss AS DOUBLE) AS avg_loss,
            NULL AS avg_target_range,
            NULL AS score
        FROM strategy_cohort_summary_cache
        WHERE closed_count >= ?
        ORDER BY expectancy DESC, closed_count DESC
        LIMIT ?
        "#;

        match sqlx::query_as::<_, StrategyCandidateSummary>(sql)
            .bind(min_closed_trades)
            .bind(limit)
            .fetch_all(pool.get_ref())
            .await
        {
            Ok(rows) => HttpResponse::Ok().json(rows),
            Err(error) if is_missing_table_error(&error) => {
                HttpResponse::Ok().json(Vec::<StrategyCandidateSummary>::new())
            }
            Err(error) => {
                eprintln!("Strategy candidate DB error: {:?}", error);
                HttpResponse::InternalServerError().finish()
            }
        }
    }
}

#[route("/candles", method = "GET", method = "POST")]
async fn fetch_candles(pool: web::Data<MySqlPool>, params: web::Json<Params>) -> impl Responder {
    // println!("🔔 Handler called: fetch_candles");
    // println!("{:?}", params);

    let candles: Vec<Candle> = match sqlx::query_as::<_, Candle>(
        "SELECT symbol, date, open, high, low, close, volume, three_month, six_month, twelve_month
        FROM candles
        WHERE symbol =  ?
        ORDER BY date",
    )
    .bind(params.symbol.clone())
    .fetch_all(pool.get_ref())
    .await
    {
        Ok(c) => {
            println!("✅ Successfully fetched {} candles", c.len());
            c
        }
        Err(e) => {
            eprintln!("❌ Failed to fetch candles: {}", e);
            return HttpResponse::InternalServerError().body("Failed to fetch candles");
        }
    };

    // let candles = match pool.get_stored_candles(&params.symbol) {
    //     Ok(c) => c,
    //     Err(e) => {
    //         println!("Failed to fetch candles: {}", e);
    //         return HttpResponse::InternalServerError().body("Failed to fetch candles");
    //     }
    // };

    // println!("{:?}", candles);
    HttpResponse::Ok().json(candles)
}

// #[route("/candles", method = "GET", method = "POST")]
// async fn fetch_accuracy(pool: web::Data<MySqlPool>,params: web::Json<Params>) -> impl Responder {
//     println!("🔥 HIT");
//     HttpResponse::Ok().body("works")
// }

#[derive(sqlx::FromRow, serde::Serialize)]
struct AccuracyBin {
    harmonic_type: String,
    bin: String,
    count: i64,
    total_count: i64,
    open_count: i64,
    win_count: i64,
    loss_count: i64,
    avg_return: f64,
    expectancy: f64,
    win_rate: f64,
    closed_rate: f64,
    avg_win: f64,
    avg_loss: f64,
}

#[route("accuracy", method = "GET", method = "POST")]
async fn fetch_accuracy(
    pool: web::Data<MySqlPool>,
    params: web::Json<AccuracyParams>,
) -> impl Responder {
    println!("🔔 Handler called: fetch_accuracy");
    if params.retracement.is_none() {
        match try_fetch_accuracy_from_structure_rollup(pool.get_ref(), &params).await {
            Ok(Some(rows)) => {
                println!(
                    "Fetched {} aggregated bins from accuracy_structure_rollup",
                    rows.len()
                );
                return HttpResponse::Ok().json(rows);
            }
            Ok(None) => {}
            Err(error) => {
                eprintln!(
                    "accuracy_structure_rollup lookup failed, checking other paths: {:?}",
                    error
                );
            }
        }
    }

    let market_filter = normalize_market_filter(params.market.as_deref());
    let trade_result_filter = normalize_trade_result_filter(params.trade_result);
    let harmonic_type_filter = params.harmonic_type.as_deref();
    if params.retracement.is_none() {
        let rollup_ready = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT CASE WHEN is_ready THEN 1 ELSE 0 END
            FROM dashboard_cache_state
            WHERE cache_name = 'accuracy_bin_rollup'
            LIMIT 1
            "#,
        )
        .fetch_optional(pool.get_ref())
        .await;

        if matches!(rollup_ready, Ok(Some(1))) {
            let source_scope = "all_patterns";

            let types_sql = if let Some(harmonic_type) = harmonic_type_filter {
                format!("SELECT '{}' AS harmonic_type", harmonic_type)
            } else {
                r#"
                SELECT 'Bat' AS harmonic_type UNION ALL
                SELECT 'Butterfly' UNION ALL
                SELECT 'Gartley' UNION ALL
                SELECT 'Crab' UNION ALL
                SELECT 'Shark'
                "#
                .to_string()
            };

            let mut rollup_filters = String::new();
            if market_filter.is_some() {
                rollup_filters.push_str(" AND market = ? ");
            }
            if trade_result_filter.is_some() {
                rollup_filters.push_str(" AND trade_result_value = ? ");
            }
            if harmonic_type_filter.is_some() {
                rollup_filters.push_str(" AND harmonic_type = ? ");
            }

            let rollup_sql = format!(
                r#"
                WITH bins AS (
                    SELECT 1 AS bin_order, '0-10' AS bin UNION ALL
                    SELECT 2, '10-20' UNION ALL
                    SELECT 3, '20-30' UNION ALL
                    SELECT 4, '30-40' UNION ALL
                    SELECT 5, '40-50' UNION ALL
                    SELECT 6, '50-60' UNION ALL
                    SELECT 7, '60-70' UNION ALL
                    SELECT 8, '70-80' UNION ALL
                    SELECT 9, '80-90' UNION ALL
                    SELECT 10, '90-100'
                ),
                types AS (
                    {types_sql}
                ),
                agg AS (
                    SELECT
                        harmonic_type,
                        bin,
                        CAST(SUM(closed_count) AS SIGNED) AS count,
                        CAST(SUM(total_count) AS SIGNED) AS total_count,
                        CAST(SUM(open_count) AS SIGNED) AS open_count,
                        CAST(SUM(win_count) AS SIGNED) AS win_count,
                        CAST(SUM(loss_count) AS SIGNED) AS loss_count,
                        CAST(CASE
                            WHEN SUM(return_count) > 0 THEN SUM(return_sum) / SUM(return_count)
                            ELSE 0.0
                        END AS DOUBLE) AS avg_return,
                        CAST(CASE
                            WHEN SUM(expectancy_count) > 0 THEN SUM(expectancy_sum) / SUM(expectancy_count)
                            ELSE 0.0
                        END AS DOUBLE) AS expectancy,
                        CAST(CASE
                            WHEN SUM(closed_count) > 0 THEN SUM(win_count) / SUM(closed_count)
                            ELSE 0.0
                        END AS DOUBLE) AS win_rate,
                        CAST(CASE
                            WHEN SUM(total_count) > 0 THEN SUM(closed_count) / SUM(total_count)
                            ELSE 0.0
                        END AS DOUBLE) AS closed_rate,
                        CAST(CASE
                            WHEN SUM(win_return_count) > 0 THEN SUM(win_return_sum) / SUM(win_return_count)
                            ELSE 0.0
                        END AS DOUBLE) AS avg_win,
                        CAST(CASE
                            WHEN SUM(loss_return_count) > 0 THEN SUM(loss_return_sum) / SUM(loss_return_count)
                            ELSE 0.0
                        END AS DOUBLE) AS avg_loss
                    FROM accuracy_bin_rollup
                    WHERE source_scope = ?
                    {rollup_filters}
                    GROUP BY harmonic_type, bin
                )
                SELECT
                    t.harmonic_type,
                    b.bin,
                    CAST(COALESCE(a.count, 0) AS SIGNED) AS count,
                    CAST(COALESCE(a.total_count, 0) AS SIGNED) AS total_count,
                    CAST(COALESCE(a.open_count, 0) AS SIGNED) AS open_count,
                    CAST(COALESCE(a.win_count, 0) AS SIGNED) AS win_count,
                    CAST(COALESCE(a.loss_count, 0) AS SIGNED) AS loss_count,
                    COALESCE(a.avg_return, 0.0) AS avg_return,
                    COALESCE(a.expectancy, 0.0) AS expectancy,
                    COALESCE(a.win_rate, 0.0) AS win_rate,
                    COALESCE(a.closed_rate, 0.0) AS closed_rate,
                    COALESCE(a.avg_win, 0.0) AS avg_win,
                    COALESCE(a.avg_loss, 0.0) AS avg_loss
                FROM types t
                CROSS JOIN bins b
                LEFT JOIN agg a
                    ON a.harmonic_type = t.harmonic_type
                   AND a.bin = b.bin
                ORDER BY t.harmonic_type, b.bin_order
                "#,
            );

            let mut rollup_query = sqlx::query_as::<_, AccuracyBin>(&rollup_sql).bind(source_scope);

            if let Some(market) = market_filter {
                rollup_query = rollup_query.bind(market);
            }

            if let Some(trade_result) = trade_result_filter {
                rollup_query = rollup_query.bind(trade_result);
            }

            if let Some(harmonic_type) = harmonic_type_filter {
                rollup_query = rollup_query.bind(harmonic_type);
            }

            match rollup_query.fetch_all(pool.get_ref()).await {
                Ok(rows) if !rows.is_empty() => {
                    println!(
                        "Fetched {} aggregated bins from accuracy_bin_rollup",
                        rows.len()
                    );
                    return HttpResponse::Ok().json(rows);
                }
                Ok(_) => {
                    println!(
                        "accuracy_bin_rollup returned no rows, checking older cache/live paths"
                    );
                }
                Err(error) => {
                    eprintln!(
                        "accuracy_bin_rollup lookup failed, checking older cache/live paths: {:?}",
                        error
                    );
                }
            }
        } else if let Err(error) = rollup_ready {
            eprintln!(
                "dashboard_cache_state lookup failed, checking older cache/live paths: {:?}",
                error
            );
        }

        let source_scope = "all_patterns";
        let market_scope = market_filter.unwrap_or("All");
        let trade_result_scope = trade_result_filter.unwrap_or(-1);

        let (cache_sql, bind_harmonic_type) = if harmonic_type_filter.is_some() {
            (
                r#"
                SELECT
                    harmonic_type,
                    bin,
                    count,
                    total_count,
                    open_count,
                    win_count,
                    loss_count,
                    avg_return,
                    expectancy,
                    win_rate,
                    closed_rate,
                    avg_win,
                    avg_loss
                FROM accuracy_bin_cache
                WHERE source_scope = ?
                  AND market_scope = ?
                  AND trade_result_scope = ?
                  AND harmonic_type = ?
                ORDER BY FIELD(bin, '0-10', '10-20', '20-30', '30-40', '40-50', '50-60', '60-70', '70-80', '80-90', '90-100')
                "#,
                true,
            )
        } else {
            (
                r#"
                SELECT
                    harmonic_type,
                    bin,
                    count,
                    total_count,
                    open_count,
                    win_count,
                    loss_count,
                    avg_return,
                    expectancy,
                    win_rate,
                    closed_rate,
                    avg_win,
                    avg_loss
                FROM accuracy_bin_cache
                WHERE source_scope = ?
                  AND market_scope = ?
                  AND trade_result_scope = ?
                ORDER BY harmonic_type, FIELD(bin, '0-10', '10-20', '20-30', '30-40', '40-50', '50-60', '60-70', '70-80', '80-90', '90-100')
                "#,
                false,
            )
        };

        let mut cache_query = sqlx::query_as::<_, AccuracyBin>(cache_sql)
            .bind(source_scope)
            .bind(market_scope)
            .bind(trade_result_scope);

        if bind_harmonic_type {
            cache_query = cache_query.bind(harmonic_type_filter.unwrap());
        }

        match cache_query.fetch_all(pool.get_ref()).await {
            Ok(rows) if !rows.is_empty() => {
                println!("âœ… Fetched {} cached aggregated bins", rows.len());
                return HttpResponse::Ok().json(rows);
            }
            Ok(_) => {
                println!(
                    "âš ï¸ accuracy_bin_cache returned no rows, falling back to live aggregation"
                );
            }
            Err(error) => {
                eprintln!(
                    "âš ï¸ accuracy_bin_cache lookup failed, falling back: {:?}",
                    error
                );
            }
        }
    }

    let mut filters_sql = String::new();
    let source_sql = if let Some(harmonic_type) = harmonic_type_filter {
        let Some(column) = accuracy_column_for_harmonic_type(harmonic_type) else {
            return HttpResponse::BadRequest().body("Unsupported harmonic type");
        };
        let Some(dominant_predicate) = dominant_harmonic_predicate(harmonic_type) else {
            return HttpResponse::BadRequest().body("Unsupported harmonic type");
        };

        format!(
            r#"
        types AS (
            SELECT '{harmonic_type}' AS harmonic_type
        ),
        source AS (
            SELECT
                '{harmonic_type}' AS harmonic_type,
                {column} AS accuracy,
                trade_pnl AS return_pct,
                trade_result,
                market,
                trade_ab_price_retracement,
                trade_bc_price_retracement,
                trade_cd_bc_price_retracement,
                trade_cd_xa_price_retracement
            FROM xabcd_patterns
            WHERE {column} IS NOT NULL
            AND ({dominant_predicate})
        ),
            "#
        )
    } else {
        r#"
        types AS (
            SELECT 'Bat' AS harmonic_type UNION ALL
            SELECT 'Butterfly' UNION ALL
            SELECT 'Gartley' UNION ALL
            SELECT 'Crab' UNION ALL
            SELECT 'Shark'
        ),
        source AS (
            SELECT
                'Bat' AS harmonic_type,
                bat_accuracy AS accuracy,
                trade_pnl AS return_pct,
                trade_result,
                market,
                trade_ab_price_retracement,
                trade_bc_price_retracement,
                trade_cd_bc_price_retracement,
                trade_cd_xa_price_retracement
            FROM xabcd_patterns
            WHERE bat_accuracy IS NOT NULL

            UNION ALL

            SELECT
                'Butterfly' AS harmonic_type,
                butterfly_accuracy AS accuracy,
                trade_pnl AS return_pct,
                trade_result,
                market,
                trade_ab_price_retracement,
                trade_bc_price_retracement,
                trade_cd_bc_price_retracement,
                trade_cd_xa_price_retracement
            FROM xabcd_patterns
            WHERE butterfly_accuracy IS NOT NULL

            UNION ALL

            SELECT
                'Gartley' AS harmonic_type,
                gartley_accuracy AS accuracy,
                trade_pnl AS return_pct,
                trade_result,
                market,
                trade_ab_price_retracement,
                trade_bc_price_retracement,
                trade_cd_bc_price_retracement,
                trade_cd_xa_price_retracement
            FROM xabcd_patterns
            WHERE gartley_accuracy IS NOT NULL

            UNION ALL

            SELECT
                'Crab' AS harmonic_type,
                crab_accuracy AS accuracy,
                trade_pnl AS return_pct,
                trade_result,
                market,
                trade_ab_price_retracement,
                trade_bc_price_retracement,
                trade_cd_bc_price_retracement,
                trade_cd_xa_price_retracement
            FROM xabcd_patterns
            WHERE crab_accuracy IS NOT NULL

            UNION ALL

            SELECT
                'Shark' AS harmonic_type,
                shark_accuracy AS accuracy,
                trade_pnl AS return_pct,
                trade_result,
                market,
                trade_ab_price_retracement,
                trade_bc_price_retracement,
                trade_cd_bc_price_retracement,
                trade_cd_xa_price_retracement
            FROM xabcd_patterns
            WHERE shark_accuracy IS NOT NULL
        ),
        "#
        .to_string()
    };

    if market_filter.is_some() {
        filters_sql.push_str(" AND market = ?");
    }

    if trade_result_filter.is_some() {
        filters_sql.push_str(" AND trade_result = ?");
    }

    if params.retracement.is_some() {
        filters_sql.push_str(
            r#"
            AND trade_ab_price_retracement >= ?
            AND trade_ab_price_retracement <= ?
            AND trade_bc_price_retracement >= ?
            AND trade_bc_price_retracement <= ?
            AND trade_cd_bc_price_retracement >= ?
            AND trade_cd_bc_price_retracement <= ?
            AND trade_cd_xa_price_retracement >= ?
            AND trade_cd_xa_price_retracement <= ?
            "#,
        );
    }

    let sql = format!(
        r#"
        WITH bins AS (
            SELECT 1 AS bin_order, '0-10' AS bin UNION ALL
            SELECT 2, '10-20' UNION ALL
            SELECT 3, '20-30' UNION ALL
            SELECT 4, '30-40' UNION ALL
            SELECT 5, '40-50' UNION ALL
            SELECT 6, '50-60' UNION ALL
            SELECT 7, '60-70' UNION ALL
            SELECT 8, '70-80' UNION ALL
            SELECT 9, '80-90' UNION ALL
            SELECT 10, '90-100'
        ),
        {source_sql}
        filtered_source AS (
            SELECT *
            FROM source
            WHERE 1 = 1
            {filters_sql}
        ),
        agg AS (
            SELECT
                harmonic_type,
                CASE
                    WHEN accuracy <= 10 THEN '0-10'
                    WHEN accuracy <= 20 THEN '10-20'
                    WHEN accuracy <= 30 THEN '20-30'
                    WHEN accuracy <= 40 THEN '30-40'
                    WHEN accuracy <= 50 THEN '40-50'
                    WHEN accuracy <= 60 THEN '50-60'
                    WHEN accuracy <= 70 THEN '60-70'
                    WHEN accuracy <= 80 THEN '70-80'
                    WHEN accuracy <= 90 THEN '80-90'
                    ELSE '90-100'
                END AS bin,
                COUNT(*) AS total_count,
                SUM(CASE WHEN trade_result IN (1, 2) THEN 1 ELSE 0 END) AS closed_count,
                SUM(CASE WHEN trade_result NOT IN (1, 2) OR trade_result IS NULL THEN 1 ELSE 0 END) AS open_count,
                SUM(CASE WHEN trade_result = 1 THEN 1 ELSE 0 END) AS win_count,
                SUM(CASE WHEN trade_result = 2 THEN 1 ELSE 0 END) AS loss_count,
                CAST(AVG(return_pct) AS DOUBLE) AS avg_return,
                CAST(AVG(CASE WHEN trade_result IN (1, 2) THEN return_pct ELSE NULL END) AS DOUBLE) AS expectancy,
                CAST(
                    AVG(
                        CASE
                            WHEN trade_result = 1 THEN 1.0
                            WHEN trade_result = 2 THEN 0.0
                            ELSE NULL
                        END
                    ) AS DOUBLE
                ) AS win_rate,
                CAST(
                    AVG(
                        CASE
                            WHEN trade_result = 1 THEN return_pct
                            ELSE NULL
                        END
                    ) AS DOUBLE
                ) AS avg_win,
                CAST(
                    AVG(
                        CASE
                            WHEN trade_result = 2 THEN return_pct
                            ELSE NULL
                        END
                    ) AS DOUBLE
                ) AS avg_loss,
                CAST(
                    SUM(CASE WHEN trade_result IN (1, 2) THEN 1 ELSE 0 END) / NULLIF(COUNT(*), 0)
                    AS DOUBLE
                ) AS closed_rate
            FROM filtered_source
            WHERE accuracy IS NOT NULL
            GROUP BY harmonic_type, bin
        )

        SELECT
            t.harmonic_type,
            b.bin,
            CAST(COALESCE(a.closed_count, 0) AS SIGNED) AS count,
            CAST(COALESCE(a.total_count, 0) AS SIGNED) AS total_count,
            CAST(COALESCE(a.open_count, 0) AS SIGNED) AS open_count,
            CAST(COALESCE(a.win_count, 0) AS SIGNED) AS win_count,
            CAST(COALESCE(a.loss_count, 0) AS SIGNED) AS loss_count,
            COALESCE(a.avg_return, 0.0) AS avg_return,
            COALESCE(a.expectancy, 0.0) AS expectancy,
            COALESCE(a.win_rate, 0.0) AS win_rate,
            COALESCE(a.closed_rate, 0.0) AS closed_rate,
            COALESCE(a.avg_win, 0.0) AS avg_win,
            COALESCE(a.avg_loss, 0.0) AS avg_loss
        FROM types t
        CROSS JOIN bins b
        LEFT JOIN agg a
            ON a.harmonic_type = t.harmonic_type
            AND a.bin = b.bin
        ORDER BY t.harmonic_type, b.bin_order;
    "#
    );
    // println!("SQL:\n{}", sql);

    // SQLx automatically maps each row into AccuracyBin
    let mut accuracy_query = sqlx::query_as::<_, AccuracyBin>(&sql);

    if let Some(market) = market_filter {
        accuracy_query = accuracy_query.bind(market);
    }

    if let Some(trade_result) = trade_result_filter {
        accuracy_query = accuracy_query.bind(trade_result);
    }

    if let Some(retracement) = &params.retracement {
        accuracy_query = accuracy_query
            .bind(retracement.ab_xa_gr)
            .bind(retracement.ab_xa_lt)
            .bind(retracement.bc_ab_gr)
            .bind(retracement.bc_ab_lt)
            .bind(retracement.cd_bc_gr)
            .bind(retracement.cd_bc_lt)
            .bind(retracement.cd_xa_gr)
            .bind(retracement.cd_xa_lt);
    }

    let rows: Vec<AccuracyBin> = match accuracy_query.fetch_all(pool.get_ref()).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("❌ DB error: {:?}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    println!("✅ Fetched {} aggregated bins", rows.len());

    HttpResponse::Ok().json(rows)
}
