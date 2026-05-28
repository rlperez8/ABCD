use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::{
    collections::{HashMap, HashSet},
    env,
};

use chrono::{Datelike, NaiveDateTime, Timelike};
use futures_util::TryStreamExt;
use serde::Serialize;
use sqlx::{MySql, MySqlPool, QueryBuilder, Row};

const RESULT_BATCH_SIZE: usize = 2_000;
const MIN_EXECUTION_RISK_TICKS: f64 = 4.0;
const PULLBACK_RETEST_WINDOW_BARS: usize = 24;
const PULLBACK_RETEST_TOLERANCE_CD_MULTIPLE: f64 = 0.25;

#[derive(Debug)]
struct Args {
    source_scope: String,
    source_timeframe: Option<String>,
    period_year: i64,
    start_year: i64,
    end_year: i64,
    limit: i64,
    event_result_policy: String,
    result_storage: String,
    risk_basis: String,
    risk_multiple: Option<f64>,
    target_r: Option<f64>,
    max_hold_multiple: i64,
    entry_kind: String,
    entry_offset: Option<i64>,
    direction_mode: Option<String>,
    skip_condition_stats: bool,
    family_key: Option<String>,
    symbol: Option<String>,
    template_source_run_id: Option<String>,
    output_run_id: Option<String>,
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
    contract_symbol: Option<String>,
    source_table: Option<String>,
    source_timeframe: Option<String>,
    market: String,
    pattern_family_key: Option<String>,
    d_date: NaiveDateTime,
    d_confirm_date: NaiveDateTime,
    d_price: f64,
    cd_price_length: f64,
    xa_price_length: f64,
    full_pattern_length: i64,
}

#[derive(Clone)]
struct PatternEvent {
    event_key: String,
    event_id: Option<String>,
    decision_date: NaiveDateTime,
    candidates: Vec<PatternSetup>,
}

#[derive(Default)]
struct BuildCoverageBucket {
    pattern_count: i64,
    contract_symbols: HashSet<String>,
    first_d_confirm_date: Option<NaiveDateTime>,
    last_d_confirm_date: Option<NaiveDateTime>,
}

struct StoredBuildCoverageRow {
    run_id: String,
    source_scope: String,
    root_symbol: String,
    exchange_name: String,
    source_timeframe: String,
    pattern_count: i64,
    contract_count: i64,
    first_d_confirm_date: Option<NaiveDateTime>,
    last_d_confirm_date: Option<NaiveDateTime>,
}

#[derive(Clone, sqlx::FromRow)]
struct ForwardCandle {
    candle_date: NaiveDateTime,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
}

#[derive(Clone, Serialize)]
struct TemplateRule {
    schema_version: i64,
    created_by: String,
    direction: DirectionRule,
    entry: EntryRule,
    stop: StopRule,
    target: TargetRule,
    hold: HoldRule,
    creator_evidence: CreatorEvidence,
}

#[derive(Clone, Serialize)]
struct DirectionRule {
    mode: String,
    description: String,
}

#[derive(Clone, Serialize)]
struct EntryRule {
    kind: String,
    offset_from_confirmation: i64,
    price: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pullback_window_bars: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pullback_tolerance_basis: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pullback_tolerance_multiple: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trigger: Option<String>,
}

#[derive(Clone, Serialize)]
struct StopRule {
    kind: String,
    basis: String,
    multiple: f64,
    description: String,
}

#[derive(Clone, Serialize)]
struct TargetRule {
    kind: String,
    r: f64,
}

#[derive(Clone, Serialize)]
struct HoldRule {
    kind: String,
    pattern_multiple: i64,
}

#[derive(Clone, Serialize)]
struct CreatorEvidence {
    setup_id: String,
    pattern_id: Option<String>,
    pattern_family_key: Option<String>,
    symbol: String,
    market: String,
    entry_date: Option<String>,
    exit_date: Option<String>,
    result_r: Option<f64>,
    risk_points: Option<f64>,
}

#[derive(Clone)]
struct GeneratedTemplate {
    template_uid: String,
    template_name: String,
    rule_json: String,
    entry_kind: String,
    entry_offset: i64,
    direction_mode: String,
    risk_basis: String,
    risk_multiple: f64,
    target_r: f64,
    max_hold_multiple: i64,
}

#[derive(Clone)]
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

struct StoredTemplateResult {
    template_uid: String,
    setup_id: String,
    pattern_id: Option<String>,
    pattern_group_id: String,
    event_id: Option<String>,
    event_rank: Option<i64>,
    event_sister_count: Option<i64>,
    event_decision_date: NaiveDateTime,
    event_candidate_count: i64,
    event_live_candidate_count: i64,
    pattern_family_key: Option<String>,
    symbol: String,
    root_symbol: String,
    exchange_name: String,
    source_timeframe: String,
    market: String,
    d_date: NaiveDateTime,
    d_confirm_date: NaiveDateTime,
    evaluation_order: i64,
    was_created_for_setup: bool,
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

impl StoredTemplateResult {
    fn from_evaluation(
        template: &GeneratedTemplate,
        event: &PatternEvent,
        live_candidate_count: i64,
        setup: &PatternSetup,
        evaluation_order: i64,
        was_created_for_setup: bool,
        evaluation: &TemplateEvaluation,
    ) -> Self {
        Self {
            template_uid: template.template_uid.clone(),
            setup_id: setup.setup_id.clone(),
            pattern_id: setup.pattern_id.clone(),
            pattern_group_id: setup.pattern_group_id.clone(),
            event_id: event.event_id.clone(),
            event_rank: setup.event_rank,
            event_sister_count: setup.event_sister_count,
            event_decision_date: event.decision_date,
            event_candidate_count: event.candidates.len() as i64,
            event_live_candidate_count: live_candidate_count,
            pattern_family_key: setup.pattern_family_key.clone(),
            symbol: setup.symbol.clone(),
            root_symbol: normalized_root_symbol(setup),
            exchange_name: exchange_for_root(&normalized_root_symbol(setup)).to_string(),
            source_timeframe: clean_optional_text(setup.source_timeframe.as_deref())
                .unwrap_or_else(|| "unknown".to_string()),
            market: setup.market.clone(),
            d_date: setup.d_date,
            d_confirm_date: setup.d_confirm_date,
            evaluation_order,
            was_created_for_setup,
            outcome: evaluation.outcome.clone(),
            exit_reason: evaluation.exit_reason.clone(),
            result_r: evaluation.result_r,
            entry_date: evaluation.entry_date,
            exit_date: evaluation.exit_date,
            entry_price: evaluation.entry_price,
            stop_price: evaluation.stop_price,
            target_price: evaluation.target_price,
            exit_price: evaluation.exit_price,
            risk_points: evaluation.risk_points,
            trade_direction: evaluation.trade_direction.clone(),
        }
    }
}

#[derive(Clone, Default)]
struct AggregateStats {
    eval_count: i64,
    pass_count: i64,
    fail_count: i64,
    no_entry_count: i64,
    sum_r: f64,
    best_r: f64,
    worst_r: f64,
}

impl AggregateStats {
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

    fn win_rate(&self) -> f64 {
        if self.eval_count > 0 {
            self.pass_count as f64 / self.eval_count as f64
        } else {
            0.0
        }
    }
}

#[derive(Default)]
struct SummaryCollector {
    template_stats: HashMap<String, AggregateStats>,
    market_stats: HashMap<(String, String), AggregateStats>,
    condition_stats: HashMap<(String, String, String), AggregateStats>,
}

impl SummaryCollector {
    fn record(&mut self, result: &StoredTemplateResult) {
        self.template_stats
            .entry(result.template_uid.clone())
            .or_default()
            .record(&result.outcome, result.result_r);
        self.market_stats
            .entry((
                result.template_uid.clone(),
                clean_condition_value(Some(result.market.clone())),
            ))
            .or_default()
            .record(&result.outcome, result.result_r);

        self.record_condition(
            &result.template_uid,
            "market",
            Some(result.market.as_str()),
            result,
        );
        self.record_condition(
            &result.template_uid,
            "pattern_family_key",
            result.pattern_family_key.as_deref(),
            result,
        );
        self.record_condition(
            &result.template_uid,
            "symbol",
            Some(result.symbol.as_str()),
            result,
        );
        self.record_condition(
            &result.template_uid,
            "root_symbol",
            Some(result.root_symbol.as_str()),
            result,
        );
        self.record_condition(
            &result.template_uid,
            "exchange",
            Some(result.exchange_name.as_str()),
            result,
        );
        self.record_condition(
            &result.template_uid,
            "source_timeframe",
            Some(result.source_timeframe.as_str()),
            result,
        );
        self.record_condition(
            &result.template_uid,
            "trade_direction",
            result.trade_direction.as_deref(),
            result,
        );
        self.record_condition(
            &result.template_uid,
            "exit_reason",
            Some(result.exit_reason.as_str()),
            result,
        );
        self.record_condition(
            &result.template_uid,
            "confirm_year",
            Some(&result.d_confirm_date.year().to_string()),
            result,
        );
        let confirm_session = confirm_session_value(Some(result.d_confirm_date));
        self.record_condition(
            &result.template_uid,
            "confirm_session",
            Some(confirm_session.as_str()),
            result,
        );
    }

    fn record_condition(
        &mut self,
        template_uid: &str,
        condition_type: &str,
        condition_value: Option<&str>,
        result: &StoredTemplateResult,
    ) {
        self.condition_stats
            .entry((
                template_uid.to_string(),
                condition_type.to_string(),
                clean_condition_value(condition_value.map(str::to_string)),
            ))
            .or_default()
            .record(&result.outcome, result.result_r);
    }
}

fn usage() -> &'static str {
    "Usage: cargo run --bin create_entry_exit_templates -- [--source futures|daily|all] [--timeframe 1m|5m|daily] [--year YYYY | --start-year YYYY --end-year YYYY] [--limit N] [--family FAMILY_KEY] [--symbol SYMBOL] [--event-result-policy first|best] [--result-storage summary-only|full] [--entry-kind confirmation|pullback-retest-d] [--entry-offset N] [--direction-mode pattern|inverse_pattern|reversal] [--risk-basis cd|xa] [--risk-multiple M] [--target-r R] [--max-hold-multiple N] [--template-source-run-id RUN_ID] [--output-run-id RUN_ID] [--skip-condition-stats]\nUse --limit 0 to scan every Twin/Event in the selected period. Default result storage is summary-only; use --result-storage full for raw audit rows. With --template-source-run-id it evaluates that run's existing templates over the selected period without creating new templates. It does not read entry_exit_tests or the Phase 1 optimizer routes."
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

fn normalize_timeframe(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .map(|value| match value.as_str() {
            "1" | "1min" | "1-min" | "1minute" | "1-minute" => "1m".to_string(),
            "5" | "5min" | "5-min" | "5minute" | "5-minute" => "5m".to_string(),
            "day" | "1d" => "daily".to_string(),
            _ => value,
        })
}

fn normalize_risk_basis(value: Option<String>) -> String {
    match value.as_deref().map(str::trim).map(str::to_ascii_lowercase) {
        Some(value) if value == "xa" || value == "xa_price_length" || value == "x_to_a" => {
            "xa_price_length".to_string()
        }
        _ => "cd_price_length".to_string(),
    }
}

fn normalize_entry_kind(value: Option<String>) -> String {
    match value.as_deref().map(str::trim).map(str::to_ascii_lowercase) {
        Some(value)
            if value == "pullback"
                || value == "pullback_retest"
                || value == "pullback-retest-d"
                || value == "pullback_retest_d"
                || value == "retest-d"
                || value == "retest_d" =>
        {
            "pullback_retest_d".to_string()
        }
        _ => "confirmation_plus_n_open".to_string(),
    }
}

fn normalize_direction_mode(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .and_then(|value| match value.as_str() {
            "pattern" | "pattern_direction" | "with_pattern" => Some("pattern".to_string()),
            "inverse" | "inverse_pattern" | "against_pattern" | "reversal" | "xa_reversal" => {
                Some("inverse_pattern".to_string())
            }
            _ => None,
        })
}

fn clean_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn normalized_root_symbol(setup: &PatternSetup) -> String {
    let root = clean_optional_text(setup.root_symbol.as_deref())
        .or_else(|| clean_optional_text(setup.contract_symbol.as_deref()))
        .or_else(|| clean_optional_text(Some(&setup.symbol)))
        .unwrap_or_else(|| "Unknown".to_string())
        .to_ascii_uppercase();
    let bytes = root.as_bytes();
    let is_treasury_contract = root.len() >= 4
        && (root.starts_with("ZB") || root.starts_with("ZN"))
        && matches!(
            bytes.get(2).copied(),
            Some(b'F' | b'G' | b'H' | b'J' | b'K' | b'M' | b'N' | b'Q' | b'U' | b'V' | b'X' | b'Z')
        )
        && bytes.get(3).is_some_and(u8::is_ascii_digit);

    if is_treasury_contract {
        root[..2].to_string()
    } else {
        root
    }
}

fn root_symbol_from_contract(symbol: &str) -> String {
    let uppercase = symbol.trim().to_uppercase();
    let chars = uppercase.chars().collect::<Vec<_>>();
    for index in 0..chars.len().saturating_sub(1) {
        let is_contract_month = matches!(
            chars[index],
            'F' | 'G' | 'H' | 'J' | 'K' | 'M' | 'N' | 'Q' | 'U' | 'V' | 'X' | 'Z'
        );
        if is_contract_month && chars[index + 1].is_ascii_digit() && index > 0 {
            return chars[..index].iter().collect::<String>();
        }
    }
    uppercase
}

fn tick_size_for_root(root_symbol: &str) -> f64 {
    match root_symbol {
        "ES" | "MES" | "NQ" | "MNQ" => 0.25,
        "RTY" | "M2K" | "EMD" => 0.10,
        "YM" | "MYM" => 1.0,
        "NKD" => 5.0,
        "CL" | "MCL" => 0.01,
        "QM" => 0.025,
        "NG" => 0.001,
        "QG" => 0.005,
        "GC" | "MGC" | "PA" | "PL" => 0.10,
        "SI" | "SIL" => 0.005,
        "HG" => 0.0005,
        "RB" | "HO" => 0.0001,
        "6A" | "6B" | "6C" | "6E" | "6M" | "6N" | "6S" => 0.00005,
        "6J" => 0.0000005,
        "GF" | "HE" | "LE" => 0.025,
        "ZC" | "ZW" | "ZS" | "KE" => 0.25,
        "ZL" => 0.01,
        "ZM" => 0.10,
        "ZB" | "UB" => 0.03125,
        "ZN" => 0.015625,
        "ZF" => 0.0078125,
        "ZT" => 0.00390625,
        _ => 0.01,
    }
}

fn tick_size_for_setup(setup: &PatternSetup) -> f64 {
    let root = clean_optional_text(setup.root_symbol.as_deref())
        .or_else(|| clean_optional_text(setup.contract_symbol.as_deref()))
        .or_else(|| clean_optional_text(Some(&setup.symbol)))
        .unwrap_or_else(|| setup.symbol.clone());
    tick_size_for_root(&root_symbol_from_contract(&root))
}

fn clean_tick_price(price: f64) -> f64 {
    (price * 1_000_000_000.0).round() / 1_000_000_000.0
}

fn round_to_tick(price: f64, tick_size: f64) -> f64 {
    if !price.is_finite() || !tick_size.is_finite() || tick_size <= 0.0 {
        return price;
    }
    clean_tick_price((price / tick_size).round() * tick_size)
}

fn floor_to_tick(price: f64, tick_size: f64) -> f64 {
    if !price.is_finite() || !tick_size.is_finite() || tick_size <= 0.0 {
        return price;
    }
    clean_tick_price((price / tick_size).floor() * tick_size)
}

fn ceil_to_tick(price: f64, tick_size: f64) -> f64 {
    if !price.is_finite() || !tick_size.is_finite() || tick_size <= 0.0 {
        return price;
    }
    clean_tick_price((price / tick_size).ceil() * tick_size)
}

fn exchange_for_root(root_symbol: &str) -> &'static str {
    match root_symbol {
        "6A" | "6B" | "6C" | "6E" | "6J" | "6M" | "6N" | "6S" | "BTC" | "EMD" | "ES" | "GF"
        | "HE" | "LE" | "M2K" | "MES" | "MNQ" | "NKD" | "NQ" | "RTY" => "CME",
        "KE" | "UB" | "YM" | "ZB" | "ZC" | "ZF" | "ZL" | "ZM" | "ZN" | "ZS" | "ZT" | "ZW" => "CBOT",
        "GC" | "HG" | "MGC" | "SI" => "COMEX",
        "CL" | "HO" | "MCL" | "NG" | "PA" | "PL" | "QG" | "QM" | "RB" => "NYMEX",
        _ => "Unknown",
    }
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let raw_args = env::args().skip(1).collect::<Vec<_>>();
    if raw_args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", usage());
        std::process::exit(0);
    }

    let source_scope = normalize_source_scope(arg_value(&raw_args, "--source"));
    let source_timeframe = normalize_timeframe(
        arg_value(&raw_args, "--timeframe").or_else(|| arg_value(&raw_args, "--tf")),
    );
    let single_year = arg_value(&raw_args, "--year")
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|year| (1900..=2200).contains(year));
    let parsed_start_year = arg_value(&raw_args, "--start-year")
        .or_else(|| arg_value(&raw_args, "--from-year"))
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|year| (1900..=2200).contains(year));
    let parsed_end_year = arg_value(&raw_args, "--end-year")
        .or_else(|| arg_value(&raw_args, "--to-year"))
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|year| (1900..=2200).contains(year));
    let has_year_range = parsed_start_year.is_some() || parsed_end_year.is_some();
    let (mut start_year, mut end_year) = if has_year_range {
        let fallback_year = single_year.unwrap_or(2025);
        (
            parsed_start_year.unwrap_or(fallback_year),
            parsed_end_year.unwrap_or(fallback_year),
        )
    } else {
        let year = single_year.unwrap_or(2025);
        (year, year)
    };
    if start_year > end_year {
        std::mem::swap(&mut start_year, &mut end_year);
    }
    let period_year = if start_year == end_year {
        start_year
    } else {
        0
    };
    let limit = arg_value(&raw_args, "--limit")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(100)
        .clamp(0, 1_000_000);
    let event_result_policy = arg_value(&raw_args, "--event-result-policy")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| value == "first" || value == "best")
        .unwrap_or_else(|| "first".to_string());
    let result_storage = arg_value(&raw_args, "--result-storage")
        .or_else(|| arg_value(&raw_args, "--storage"))
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| value == "summary-only" || value == "full")
        .unwrap_or_else(|| {
            if raw_args.iter().any(|arg| arg == "--store-result-rows") {
                "full".to_string()
            } else {
                "summary-only".to_string()
            }
        });
    let risk_basis = normalize_risk_basis(arg_value(&raw_args, "--risk-basis"));
    let risk_multiple = arg_value(&raw_args, "--risk-multiple")
        .or_else(|| arg_value(&raw_args, "--risk-mult"))
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0);
    let target_r = arg_value(&raw_args, "--target-r")
        .or_else(|| arg_value(&raw_args, "--target"))
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0);
    let max_hold_multiple = arg_value(&raw_args, "--max-hold-multiple")
        .or_else(|| arg_value(&raw_args, "--hold-multiple"))
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(5)
        .clamp(1, 50);
    let entry_kind = normalize_entry_kind(
        arg_value(&raw_args, "--entry-kind")
            .or_else(|| arg_value(&raw_args, "--entry"))
            .or_else(|| arg_value(&raw_args, "--entry-type")),
    );
    let entry_offset = arg_value(&raw_args, "--entry-offset")
        .or_else(|| arg_value(&raw_args, "--offset"))
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0);
    let direction_mode = normalize_direction_mode(
        arg_value(&raw_args, "--direction-mode")
            .or_else(|| arg_value(&raw_args, "--direction"))
            .or_else(|| arg_value(&raw_args, "--trade-direction-mode")),
    );
    let skip_condition_stats = raw_args.iter().any(|arg| arg == "--skip-condition-stats");
    let family_key = arg_value(&raw_args, "--family")
        .or_else(|| arg_value(&raw_args, "--family-key"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let symbol = arg_value(&raw_args, "--symbol")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let template_source_run_id = arg_value(&raw_args, "--template-source-run-id")
        .or_else(|| arg_value(&raw_args, "--existing-template-run-id"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let output_run_id = arg_value(&raw_args, "--output-run-id")
        .or_else(|| arg_value(&raw_args, "--append-to-run-id"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    Ok(Args {
        source_scope,
        source_timeframe,
        period_year,
        start_year,
        end_year,
        limit,
        event_result_policy,
        result_storage,
        risk_basis,
        risk_multiple,
        target_r,
        max_hold_multiple,
        entry_kind,
        entry_offset,
        direction_mode,
        skip_condition_stats,
        family_key,
        symbol,
        template_source_run_id,
        output_run_id,
    })
}

impl Args {
    fn year_label(&self) -> String {
        if self.start_year == self.end_year {
            self.start_year.to_string()
        } else {
            format!("{}-{}", self.start_year, self.end_year)
        }
    }

    fn event_limit(&self) -> Option<usize> {
        if self.limit > 0 {
            Some(self.limit as usize)
        } else {
            None
        }
    }

    fn stores_raw_results(&self) -> bool {
        self.result_storage == "full"
    }
}

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

fn creator_run_id() -> String {
    let started_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("eetc-{started_at_ms}-{}", std::process::id())
}

fn result_table_name_for_run(run_id: &str) -> String {
    let suffix = run_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("entry_exit_template_results_{suffix}")
}

fn quoted_identifier(identifier: &str) -> String {
    debug_assert!(identifier
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_'));
    format!("`{identifier}`")
}

async fn ensure_template_tables(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_template_creator_runs (
            run_id VARCHAR(64) NOT NULL PRIMARY KEY,
            source_scope VARCHAR(16) NOT NULL,
            period_year INT NOT NULL,
            requested_limit BIGINT NOT NULL,
            scanned_patterns BIGINT NOT NULL,
            templates_created BIGINT NOT NULL,
            existing_template_passes BIGINT NOT NULL,
            failed_to_create BIGINT NOT NULL,
            result_rows BIGINT NOT NULL,
            elapsed_ms BIGINT NOT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_templates (
            template_uid VARCHAR(128) NOT NULL PRIMARY KEY,
            origin_run_id VARCHAR(64) NOT NULL,
            template_name VARCHAR(255) NOT NULL,
            template_version INT NOT NULL DEFAULT 1,
            is_active BOOLEAN NOT NULL DEFAULT TRUE,
            entry_kind VARCHAR(64) NOT NULL,
            direction_mode VARCHAR(32) NOT NULL,
            risk_basis VARCHAR(32) NOT NULL,
            risk_multiple DOUBLE NOT NULL,
            target_r DOUBLE NOT NULL,
            max_hold_multiple BIGINT NOT NULL,
            rule_json JSON NOT NULL,
            created_from_setup_id VARCHAR(64) NOT NULL,
            created_from_pattern_id VARCHAR(64) NULL,
            created_from_family_key VARCHAR(64) NULL,
            created_from_symbol VARCHAR(32) NOT NULL,
            created_from_market VARCHAR(16) NOT NULL,
            created_from_d_confirm_date DATETIME NOT NULL,
            first_result_r DOUBLE NULL,
            notes TEXT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            INDEX idx_entry_exit_templates_origin (origin_run_id, created_at),
            INDEX idx_entry_exit_templates_lookup (is_active, direction_mode, target_r, risk_basis)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_template_results (
            id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
            run_id VARCHAR(64) NOT NULL,
            template_uid VARCHAR(128) NOT NULL,
            setup_id VARCHAR(64) NOT NULL,
            pattern_id VARCHAR(64) NULL,
            pattern_group_id VARCHAR(128) NOT NULL,
            event_id VARCHAR(64) NULL,
            event_rank BIGINT NULL,
            event_sister_count BIGINT NOT NULL DEFAULT 1,
            event_decision_date DATETIME NULL,
            event_candidate_count BIGINT NOT NULL DEFAULT 1,
            event_live_candidate_count BIGINT NOT NULL DEFAULT 1,
            pattern_family_key VARCHAR(64) NULL,
            symbol VARCHAR(32) NOT NULL,
            market VARCHAR(16) NOT NULL,
            d_date DATETIME NOT NULL,
            d_confirm_date DATETIME NOT NULL,
            evaluation_order BIGINT NOT NULL,
            was_created_for_setup BOOLEAN NOT NULL DEFAULT FALSE,
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
            UNIQUE KEY uniq_entry_exit_template_result (run_id, template_uid, setup_id),
            INDEX idx_entry_exit_template_results_run (run_id, evaluation_order),
            INDEX idx_entry_exit_template_results_template (template_uid, outcome),
            INDEX idx_entry_exit_template_results_run_template_stats (run_id, template_uid, outcome, result_r),
            INDEX idx_entry_exit_template_results_run_setup (run_id, setup_id, template_uid),
            INDEX idx_entry_exit_template_results_pattern (setup_id, pattern_id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    ensure_entry_exit_template_ui_stats_table(pool).await?;
    ensure_entry_exit_template_build_coverage_ui_table(pool).await?;
    ensure_entry_exit_template_build_coverage_rows_ui_table(pool).await?;
    ensure_entry_exit_template_build_summary_ui_table(pool).await?;
    ensure_entry_exit_template_condition_stats_table(pool).await?;
    ensure_entry_exit_template_result_event_columns(pool).await?;
    ensure_entry_exit_template_indexes(pool).await?;

    Ok(())
}

async fn ensure_entry_exit_template_result_event_columns(
    pool: &MySqlPool,
) -> Result<(), sqlx::Error> {
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
        ensure_entry_exit_template_result_column(pool, column, definition).await?;
    }

    Ok(())
}

async fn ensure_entry_exit_template_result_column(
    pool: &MySqlPool,
    column: &str,
    definition: &str,
) -> Result<(), sqlx::Error> {
    let exists: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM information_schema.columns
        WHERE table_schema = DATABASE()
          AND table_name = 'entry_exit_template_results'
          AND column_name = ?
        "#,
    )
    .bind(column)
    .fetch_one(pool)
    .await?;

    if exists == 0 {
        sqlx::query(&format!(
            "ALTER TABLE entry_exit_template_results ADD COLUMN {column} {definition}"
        ))
        .execute(pool)
        .await?;
    }

    Ok(())
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

async fn ensure_entry_exit_template_build_coverage_ui_table(
    pool: &MySqlPool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_template_build_coverage_ui (
            run_id VARCHAR(64) NOT NULL,
            source_scope VARCHAR(16) NOT NULL,
            root_symbol VARCHAR(32) NOT NULL,
            exchange_name VARCHAR(32) NOT NULL DEFAULT 'Unknown',
            source_timeframe VARCHAR(16) NOT NULL DEFAULT 'unknown',
            pattern_count BIGINT NOT NULL DEFAULT 0,
            contract_count BIGINT NOT NULL DEFAULT 0,
            first_d_confirm_date DATETIME NULL,
            last_d_confirm_date DATETIME NULL,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (run_id, root_symbol, source_timeframe),
            INDEX idx_entry_exit_build_coverage_run_exchange (run_id, exchange_name, pattern_count),
            INDEX idx_entry_exit_build_coverage_run_root (run_id, root_symbol)
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn ensure_entry_exit_template_build_coverage_rows_ui_table(
    pool: &MySqlPool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_template_build_coverage_rows_ui (
            test_id VARCHAR(64) NOT NULL,
            run_id VARCHAR(64) NOT NULL,
            source_scope VARCHAR(16) NOT NULL,
            exchange_name VARCHAR(32) NOT NULL DEFAULT 'Unknown',
            root_symbol VARCHAR(32) NOT NULL,
            source_timeframe VARCHAR(16) NOT NULL DEFAULT 'unknown',
            scanned_pattern_count BIGINT NOT NULL DEFAULT 0,
            universe_pattern_count BIGINT NOT NULL DEFAULT 0,
            contract_count BIGINT NOT NULL DEFAULT 0,
            status VARCHAR(16) NOT NULL DEFAULT 'Not scanned',
            sort_order BIGINT NOT NULL DEFAULT 0,
            first_d_confirm_date DATETIME NULL,
            last_d_confirm_date DATETIME NULL,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (test_id, root_symbol, source_timeframe),
            INDEX idx_entry_exit_build_coverage_rows_run_exchange (run_id, exchange_name, sort_order),
            INDEX idx_entry_exit_build_coverage_rows_status (run_id, status, scanned_pattern_count)
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn ensure_entry_exit_template_build_summary_ui_table(
    pool: &MySqlPool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_template_build_summary_ui (
            test_id VARCHAR(64) NOT NULL PRIMARY KEY,
            run_id VARCHAR(64) NOT NULL UNIQUE,
            build_label VARCHAR(32) NULL,
            source_scope VARCHAR(16) NOT NULL,
            source_timeframe VARCHAR(16) NOT NULL DEFAULT 'unknown',
            scan_year_start INT NULL,
            scan_year_end INT NULL,
            scan_year_label VARCHAR(32) NOT NULL DEFAULT 'All',
            patterns_scanned BIGINT NOT NULL DEFAULT 0,
            templates_created BIGINT NOT NULL DEFAULT 0,
            coverage_patterns BIGINT NOT NULL DEFAULT 0,
            root_count BIGINT NOT NULL DEFAULT 0,
            exchange_count BIGINT NOT NULL DEFAULT 0,
            requested_limit BIGINT NOT NULL DEFAULT 0,
            result_rows BIGINT NOT NULL DEFAULT 0,
            elapsed_ms BIGINT NOT NULL DEFAULT 0,
            created_at DATETIME NULL,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            INDEX idx_entry_exit_build_summary_run (run_id),
            INDEX idx_entry_exit_build_summary_created (created_at)
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

async fn ensure_entry_exit_template_indexes(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    ensure_entry_exit_template_index(
        pool,
        "idx_entry_exit_template_results_run_template_stats",
        "run_id, template_uid, outcome, result_r",
        "full-run Entry/Exit template stats",
    )
    .await?;
    ensure_entry_exit_template_index(
        pool,
        "idx_entry_exit_template_results_run_setup",
        "run_id, setup_id, template_uid",
        "stored Entry/Exit condition breakdowns",
    )
    .await?;
    ensure_entry_exit_template_index(
        pool,
        "idx_entry_exit_template_results_run_family_template",
        "run_id, pattern_family_key, template_uid, outcome, result_r",
        "Entry/Exit family playbook training",
    )
    .await?;

    Ok(())
}

async fn ensure_entry_exit_template_index(
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

async fn ensure_result_table_for_run(
    pool: &MySqlPool,
    table_name: &str,
) -> Result<(), sqlx::Error> {
    let quoted_table = quoted_identifier(table_name);
    sqlx::query(&format!(
        "CREATE TABLE IF NOT EXISTS {quoted_table} LIKE entry_exit_template_results"
    ))
    .execute(pool)
    .await?;

    Ok(())
}

async fn fetch_patterns(pool: &MySqlPool, args: &Args) -> Result<Vec<PatternSetup>, sqlx::Error> {
    let raw_candidate_limit = if args.limit > 0 {
        Some(args.limit.saturating_mul(20).max(args.limit).min(1_000_000))
    } else {
        None
    };
    let source_filter = match args.source_scope.as_str() {
        "futures" => "AND COALESCE(ps.source_table, '') LIKE 'futures_contract_%_candles'",
        "daily" => {
            "AND COALESCE(ps.source_table, '') = 'candles' AND COALESCE(ps.source_timeframe, '') = 'daily'"
        }
        _ => "",
    };
    let year_filter = if args.start_year == args.end_year {
        "AND YEAR(COALESCE(ps.d_confirm_date, ps.d_date)) = ?"
    } else {
        "AND YEAR(COALESCE(ps.d_confirm_date, ps.d_date)) BETWEEN ? AND ?"
    };
    let timeframe_filter = if args.source_timeframe.is_some() {
        "AND COALESCE(NULLIF(ps.source_timeframe, ''), 'unknown') = ?"
    } else {
        ""
    };
    let family_filter = if args.family_key.is_some() {
        "AND ps.pattern_family_key = ?"
    } else {
        ""
    };
    let symbol_filter = if args.symbol.is_some() {
        "AND (ps.symbol = ? OR ps.root_symbol = ?)"
    } else {
        ""
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
            ps.contract_symbol,
            ps.source_table,
            ps.source_timeframe,
            ps.market,
            ps.pattern_family_key,
            ps.d_date,
            CAST(COALESCE(ps.d_confirm_date, ps.d_date) AS DATETIME) AS d_confirm_date,
            ps.d_min_max AS d_price,
            ps.cd_price_length,
            COALESCE(ps.xa_price_length, ABS(ps.a_min_max - ps.x_min_max), 0) AS xa_price_length,
            CAST(ps.full_pattern_length AS SIGNED) AS full_pattern_length
        FROM pattern_setups ps
        WHERE ps.d_date IS NOT NULL
          AND ps.full_pattern_length > 0
          AND ABS(ps.cd_price_length) > 0
          AND COALESCE(ps.xa_price_length, ABS(ps.a_min_max - ps.x_min_max), 0) > 0
          {source_filter}
          {year_filter}
          {timeframe_filter}
          {family_filter}
          {symbol_filter}
        ORDER BY COALESCE(ps.d_confirm_date, ps.d_date) ASC, ps.setup_id ASC
        {limit_clause}
        "#,
        source_filter = source_filter,
        year_filter = year_filter,
        timeframe_filter = timeframe_filter,
        family_filter = family_filter,
        symbol_filter = symbol_filter,
        limit_clause = limit_clause,
    );

    let mut query = sqlx::query_as::<_, PatternSetup>(&sql);
    if args.start_year == args.end_year {
        query = query.bind(args.start_year);
    } else {
        query = query.bind(args.start_year).bind(args.end_year);
    }
    if let Some(source_timeframe) = &args.source_timeframe {
        query = query.bind(source_timeframe);
    }
    if let Some(family_key) = &args.family_key {
        query = query.bind(family_key);
    }
    if let Some(symbol) = &args.symbol {
        query = query.bind(symbol).bind(symbol);
    }

    if let Some(raw_candidate_limit) = raw_candidate_limit {
        query = query.bind(raw_candidate_limit);
    }
    query.fetch_all(pool).await
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

fn build_pattern_events(patterns: Vec<PatternSetup>, limit: Option<usize>) -> Vec<PatternEvent> {
    let mut group_indexes: HashMap<String, usize> = HashMap::new();
    let mut events: Vec<PatternEvent> = Vec::new();

    for setup in patterns {
        let event_key = pattern_event_key(&setup);
        if let Some(index) = group_indexes.get(&event_key).copied() {
            let event = &mut events[index];
            event.decision_date = event.decision_date.min(setup.d_confirm_date);
            event.candidates.push(setup);
        } else {
            group_indexes.insert(event_key.clone(), events.len());
            events.push(PatternEvent {
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
    max_hold_multiple: i64,
) -> Result<Vec<ForwardCandle>, sqlx::Error> {
    let max_forward_bars = setup
        .full_pattern_length
        .saturating_mul(max_hold_multiple.max(5))
        .max(1);
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

fn risk_basis_length(setup: &PatternSetup, risk_basis: &str) -> f64 {
    match risk_basis {
        "xa_price_length" => setup.xa_price_length.abs(),
        _ => setup.cd_price_length.abs(),
    }
}

fn risk_points_for_template(template: &GeneratedTemplate, setup: &PatternSetup) -> f64 {
    risk_basis_length(setup, &template.risk_basis) * template.risk_multiple
}

fn execution_prices(
    template: &GeneratedTemplate,
    setup: &PatternSetup,
    raw_entry_price: f64,
    direction: f64,
) -> Option<(f64, f64, f64, f64, f64)> {
    let raw_risk_points = risk_points_for_template(template, setup);
    if !raw_risk_points.is_finite() || raw_risk_points <= 0.0 {
        return None;
    }

    let tick_size = tick_size_for_setup(setup);
    let entry_price = round_to_tick(raw_entry_price, tick_size);
    let min_risk_points = MIN_EXECUTION_RISK_TICKS * tick_size;
    let tick_rounded_risk = clean_tick_price((raw_risk_points / tick_size).ceil() * tick_size);
    let risk_points = tick_rounded_risk.max(min_risk_points);
    let stop_price = if direction > 0.0 {
        floor_to_tick(entry_price - risk_points, tick_size)
    } else {
        ceil_to_tick(entry_price + risk_points, tick_size)
    };
    let adjusted_risk_points = clean_tick_price((entry_price - stop_price).abs());
    if !adjusted_risk_points.is_finite() || adjusted_risk_points <= 0.0 {
        return None;
    }

    let raw_target_price = entry_price + direction * adjusted_risk_points * template.target_r;
    let target_price = if direction > 0.0 {
        ceil_to_tick(raw_target_price, tick_size)
    } else {
        floor_to_tick(raw_target_price, tick_size)
    };
    let target_result_r = ((target_price - entry_price) * direction) / adjusted_risk_points;
    Some((
        entry_price,
        stop_price,
        target_price,
        adjusted_risk_points,
        target_result_r,
    ))
}

fn execution_prices_from_stop(
    template: &GeneratedTemplate,
    setup: &PatternSetup,
    raw_entry_price: f64,
    raw_stop_price: f64,
    direction: f64,
) -> Option<(f64, f64, f64, f64, f64)> {
    let tick_size = tick_size_for_setup(setup);
    let entry_price = round_to_tick(raw_entry_price, tick_size);
    let mut stop_price = if direction > 0.0 {
        floor_to_tick(raw_stop_price, tick_size)
    } else {
        ceil_to_tick(raw_stop_price, tick_size)
    };

    let min_risk_points = MIN_EXECUTION_RISK_TICKS * tick_size;
    let risk_points = clean_tick_price((entry_price - stop_price).abs());
    if !risk_points.is_finite() || risk_points < min_risk_points {
        stop_price = if direction > 0.0 {
            floor_to_tick(entry_price - min_risk_points, tick_size)
        } else {
            ceil_to_tick(entry_price + min_risk_points, tick_size)
        };
    }

    let adjusted_risk_points = clean_tick_price((entry_price - stop_price).abs());
    if !adjusted_risk_points.is_finite() || adjusted_risk_points <= 0.0 {
        return None;
    }

    let raw_target_price = entry_price + direction * adjusted_risk_points * template.target_r;
    let target_price = if direction > 0.0 {
        ceil_to_tick(raw_target_price, tick_size)
    } else {
        floor_to_tick(raw_target_price, tick_size)
    };
    let target_result_r = ((target_price - entry_price) * direction) / adjusted_risk_points;
    Some((
        entry_price,
        stop_price,
        target_price,
        adjusted_risk_points,
        target_result_r,
    ))
}

fn pullback_retest_entry(
    setup: &PatternSetup,
    candles: &[ForwardCandle],
    start_index: usize,
    direction: f64,
) -> Option<(usize, f64)> {
    let tick_size = tick_size_for_setup(setup);
    let tolerance = (setup.cd_price_length.abs() * PULLBACK_RETEST_TOLERANCE_CD_MULTIPLE)
        .max(MIN_EXECUTION_RISK_TICKS * tick_size);
    let zone_low = setup.d_price - tolerance;
    let zone_high = setup.d_price + tolerance;
    let search_end = candles
        .len()
        .min(start_index.saturating_add(PULLBACK_RETEST_WINDOW_BARS));

    for trigger_index in start_index..search_end {
        let Some(candle) = candles.get(trigger_index) else {
            continue;
        };
        let touches_zone = candle.low <= zone_high && candle.high >= zone_low;
        let rejects_zone = if direction > 0.0 {
            candle.close > setup.d_price
        } else {
            candle.close < setup.d_price
        };
        if touches_zone && rejects_zone {
            let entry_index = trigger_index.saturating_add(1);
            if entry_index >= candles.len() {
                return None;
            }
            let raw_stop_price = if direction > 0.0 {
                candle.low - tick_size
            } else {
                candle.high + tick_size
            };
            return Some((entry_index, raw_stop_price));
        }
    }

    None
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
    template: &GeneratedTemplate,
    setup: &PatternSetup,
    candles: &[ForwardCandle],
) -> TemplateEvaluation {
    if candles.is_empty() {
        return no_entry("no_candles");
    }

    let direction = direction_for_mode(setup, &template.direction_mode);
    let start_index = forward_start_index(setup, candles);
    let (entry_index, prices) = if template.entry_kind == "pullback_retest_d" {
        let Some((entry_index, raw_stop_price)) =
            pullback_retest_entry(setup, candles, start_index, direction)
        else {
            return no_entry("pullback_retest_missing");
        };
        let Some(entry_candle) = candles.get(entry_index) else {
            return no_entry("entry_offset_missing");
        };
        let Some(prices) = execution_prices_from_stop(
            template,
            setup,
            entry_candle.open,
            raw_stop_price,
            direction,
        ) else {
            return no_entry("invalid_risk");
        };
        (entry_index, prices)
    } else {
        let entry_offset = template.entry_offset.max(1) as usize;
        let entry_index = start_index.saturating_add(entry_offset - 1);
        let Some(entry_candle) = candles.get(entry_index) else {
            return no_entry("entry_offset_missing");
        };
        let Some(prices) = execution_prices(template, setup, entry_candle.open, direction) else {
            return no_entry("invalid_risk");
        };
        (entry_index, prices)
    };
    let Some(entry_candle) = candles.get(entry_index) else {
        return no_entry("entry_offset_missing");
    };
    let (entry_price, stop_price, target_price, risk_points, target_result_r) = prices;
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
                result_r: Some(target_result_r),
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

fn live_event_candidate_count(event: &PatternEvent) -> i64 {
    event
        .candidates
        .iter()
        .filter(|setup| setup.d_confirm_date <= event.decision_date)
        .count()
        .max(1) as i64
}

fn event_result_score(evaluation: &TemplateEvaluation) -> (f64, i64) {
    let result_r = evaluation.result_r.unwrap_or(0.0);
    let outcome_rank = match evaluation.outcome.as_str() {
        "pass" => 2,
        "no_entry" => 1,
        "fail" => 0,
        _ => 0,
    };
    (result_r, outcome_rank)
}

struct EventTemplateEvaluation {
    selected_result: StoredTemplateResult,
    passed_setup_ids: Vec<String>,
    any_passed: bool,
}

fn evaluate_template_on_event(
    template: &GeneratedTemplate,
    event: &PatternEvent,
    candle_cache: &HashMap<String, Vec<ForwardCandle>>,
    evaluation_order: i64,
    event_result_policy: &str,
    created_for_setup_id: Option<&str>,
) -> Option<EventTemplateEvaluation> {
    let live_candidate_count = live_event_candidate_count(event);
    let use_best = event_result_policy == "best";
    let mut selected_setup: Option<&PatternSetup> = None;
    let mut selected_evaluation: Option<TemplateEvaluation> = None;
    let mut passed_setup_ids = Vec::new();

    for setup in &event.candidates {
        let candles = candle_cache
            .get(&setup.setup_id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let evaluation = evaluate_template(template, setup, candles);
        if evaluation.outcome == "pass" {
            passed_setup_ids.push(setup.setup_id.clone());
        }

        let selectable = use_best || setup.d_confirm_date <= event.decision_date;
        if !selectable {
            continue;
        }

        let replace_selected = if use_best {
            selected_evaluation
                .as_ref()
                .map(|current| event_result_score(&evaluation) > event_result_score(current))
                .unwrap_or(true)
        } else {
            selected_setup.is_none()
        };

        if replace_selected {
            selected_setup = Some(setup);
            selected_evaluation = Some(evaluation);
        }
    }

    let setup = selected_setup.or_else(|| event.candidates.first())?;
    let evaluation = selected_evaluation.unwrap_or_else(|| {
        let candles = candle_cache
            .get(&setup.setup_id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        evaluate_template(template, setup, candles)
    });
    let was_created_for_setup = created_for_setup_id
        .map(|origin_setup_id| origin_setup_id == setup.setup_id)
        .unwrap_or(false);

    Some(EventTemplateEvaluation {
        selected_result: StoredTemplateResult::from_evaluation(
            template,
            event,
            live_candidate_count,
            setup,
            evaluation_order,
            was_created_for_setup,
            &evaluation,
        ),
        any_passed: !passed_setup_ids.is_empty(),
        passed_setup_ids,
    })
}

fn candidate_price_stats(direction: f64, entry_price: f64, candle: &ForwardCandle) -> (f64, f64) {
    if direction > 0.0 {
        (
            (entry_price - candle.low).max(0.0),
            (candle.high - entry_price).max(0.0),
        )
    } else {
        (
            (candle.high - entry_price).max(0.0),
            (entry_price - candle.low).max(0.0),
        )
    }
}

fn template_uid(run_id: &str, sequence: usize) -> String {
    format!("tpl-{}-{sequence:04}", run_id.trim_start_matches("eetc-"))
}

fn build_template_json(
    setup: &PatternSetup,
    template: &GeneratedTemplate,
    evaluation: &TemplateEvaluation,
) -> Result<String, serde_json::Error> {
    let rule = TemplateRule {
        schema_version: 1,
        created_by: "create_entry_exit_templates_v0".to_string(),
        direction: DirectionRule {
            mode: template.direction_mode.clone(),
            description: if template.direction_mode == "inverse_pattern" {
                "Trade opposite the detected pattern direction.".to_string()
            } else {
                "Trade with the detected pattern direction.".to_string()
            },
        },
        entry: EntryRule {
            kind: template.entry_kind.clone(),
            offset_from_confirmation: template.entry_offset,
            price: "open".to_string(),
            pullback_window_bars: if template.entry_kind == "pullback_retest_d" {
                Some(PULLBACK_RETEST_WINDOW_BARS as i64)
            } else {
                None
            },
            pullback_tolerance_basis: if template.entry_kind == "pullback_retest_d" {
                Some("cd_price_length".to_string())
            } else {
                None
            },
            pullback_tolerance_multiple: if template.entry_kind == "pullback_retest_d" {
                Some(PULLBACK_RETEST_TOLERANCE_CD_MULTIPLE)
            } else {
                None
            },
            trigger: if template.entry_kind == "pullback_retest_d" {
                Some("touch D zone, close back in trade direction, enter next open".to_string())
            } else {
                None
            },
        },
        stop: StopRule {
            kind: if template.entry_kind == "pullback_retest_d" {
                "beyond_pullback_extreme_min_4_ticks".to_string()
            } else if template.risk_basis == "xa_price_length" {
                "xa_distance_from_entry".to_string()
            } else {
                "cd_fraction_from_entry".to_string()
            },
            basis: template.risk_basis.clone(),
            multiple: template.risk_multiple,
            description: if template.entry_kind == "pullback_retest_d" {
                "Risk is calculated from entry to the pullback rejection candle extreme, with a minimum 4-tick stop distance."
                    .to_string()
            } else if template.risk_basis == "xa_price_length" {
                "Risk is calculated as ABS(A price - X price) multiplied by this template multiple."
                    .to_string()
            } else {
                "Risk is calculated as ABS(CD price length) multiplied by this template multiple."
                    .to_string()
            },
        },
        target: TargetRule {
            kind: "risk_multiple".to_string(),
            r: template.target_r,
        },
        hold: HoldRule {
            kind: "pattern_length_multiple".to_string(),
            pattern_multiple: template.max_hold_multiple,
        },
        creator_evidence: CreatorEvidence {
            setup_id: setup.setup_id.clone(),
            pattern_id: setup.pattern_id.clone(),
            pattern_family_key: setup.pattern_family_key.clone(),
            symbol: setup.symbol.clone(),
            market: setup.market.clone(),
            entry_date: evaluation.entry_date.map(|value| value.to_string()),
            exit_date: evaluation.exit_date.map(|value| value.to_string()),
            result_r: evaluation.result_r,
            risk_points: evaluation.risk_points,
        },
    };

    serde_json::to_string_pretty(&rule)
}

fn synthesize_template(
    run_id: &str,
    sequence: usize,
    setup: &PatternSetup,
    candles: &[ForwardCandle],
    args: &Args,
) -> Option<GeneratedTemplate> {
    let start_index = forward_start_index(setup, candles);
    if candles.len() <= start_index {
        return None;
    }

    let basis_length = risk_basis_length(setup, &args.risk_basis);
    if !basis_length.is_finite() || basis_length <= 0.0 {
        return None;
    }

    let max_hold_bars = setup
        .full_pattern_length
        .saturating_mul(args.max_hold_multiple.max(1))
        .max(1) as usize;
    let max_entry_offset = candles
        .len()
        .saturating_sub(start_index)
        .min(max_hold_bars)
        .min(24);
    let target_rs = args
        .target_r
        .map(|target_r| vec![target_r])
        .unwrap_or_else(|| vec![4.0, 3.0, 2.0, 1.5, 1.0, 0.75, 0.5]);
    let direction_modes = args
        .direction_mode
        .as_deref()
        .map(|direction_mode| vec![direction_mode])
        .unwrap_or_else(|| vec!["pattern", "inverse_pattern"]);
    let risk_buffer = (basis_length * 0.01).max(0.01);
    let mut best: Option<(GeneratedTemplate, TemplateEvaluation, f64)> = None;

    if args.entry_kind == "pullback_retest_d" {
        for direction_mode in direction_modes.iter().copied() {
            let direction = direction_for_mode(setup, direction_mode);
            let Some((entry_index, _)) =
                pullback_retest_entry(setup, candles, start_index, direction)
            else {
                continue;
            };
            let entry_offset = entry_index.saturating_sub(start_index).saturating_add(1);

            for target_r in target_rs.iter().copied() {
                let mut template = GeneratedTemplate {
                    template_uid: template_uid(run_id, sequence),
                    template_name: String::new(),
                    rule_json: "{}".to_string(),
                    entry_kind: args.entry_kind.clone(),
                    entry_offset: entry_offset as i64,
                    direction_mode: direction_mode.to_string(),
                    risk_basis: "pullback_retest_extreme".to_string(),
                    risk_multiple: 1.0,
                    target_r,
                    max_hold_multiple: args.max_hold_multiple,
                };

                let evaluation = evaluate_template(&template, setup, candles);
                if evaluation.outcome != "pass" {
                    continue;
                }
                let display_risk_multiple = evaluation
                    .risk_points
                    .map(|risk_points| risk_points / basis_length)
                    .filter(|value| value.is_finite() && *value > 0.0)
                    .unwrap_or(1.0);
                template.risk_multiple = display_risk_multiple;
                template.template_name = format!(
                    "{} + D pullback retest + swing stop + {:.2}R target",
                    if direction_mode == "inverse_pattern" {
                        "Inverse pattern"
                    } else {
                        "Pattern direction"
                    },
                    template.target_r
                );
                let score =
                    target_r * 100.0 - entry_offset as f64 * 3.0 - display_risk_multiple * 4.0
                        + if direction_mode == "pattern" {
                            5.0
                        } else {
                            0.0
                        };

                let replace = best
                    .as_ref()
                    .map(|(_, _, best_score)| score > *best_score)
                    .unwrap_or(true);
                if replace {
                    best = Some((template, evaluation, score));
                }
            }
        }

        let (mut template, evaluation, _) = best?;
        template.rule_json = build_template_json(setup, &template, &evaluation).ok()?;
        return Some(template);
    }

    let entry_offsets: Vec<usize> = args
        .entry_offset
        .map(|entry_offset| vec![entry_offset as usize])
        .unwrap_or_else(|| (1..=max_entry_offset).collect());

    for entry_offset in entry_offsets {
        if entry_offset == 0 || entry_offset > max_entry_offset {
            continue;
        }
        let entry_index = start_index + entry_offset - 1;
        let Some(entry_candle) = candles.get(entry_index) else {
            continue;
        };
        let end_index = candles.len().min(entry_index.saturating_add(max_hold_bars));
        if end_index <= entry_index {
            continue;
        }

        for direction_mode in direction_modes.iter().copied() {
            let direction = direction_for_mode(setup, direction_mode);
            let entry_price = entry_candle.open;
            let mut max_adverse = 0.0f64;

            for candle in &candles[entry_index..end_index] {
                let (adverse, favorable) = candidate_price_stats(direction, entry_price, candle);
                max_adverse = max_adverse.max(adverse);

                for target_r in target_rs.iter().copied() {
                    if favorable <= 0.0 {
                        continue;
                    }
                    let risk_multiple = if let Some(risk_multiple) = args.risk_multiple {
                        risk_multiple
                    } else if args.risk_basis == "xa_price_length" {
                        1.0
                    } else {
                        let min_risk = max_adverse + risk_buffer;
                        let max_risk = favorable / target_r;
                        if !min_risk.is_finite()
                            || !max_risk.is_finite()
                            || min_risk <= 0.0
                            || max_risk <= min_risk
                        {
                            continue;
                        }

                        let risk_points = (min_risk + max_risk) / 2.0;
                        risk_points / basis_length
                    };
                    if !(0.005..=5.0).contains(&risk_multiple) {
                        continue;
                    }

                    let mut template = GeneratedTemplate {
                        template_uid: template_uid(run_id, sequence),
                        template_name: String::new(),
                        rule_json: "{}".to_string(),
                        entry_kind: args.entry_kind.clone(),
                        entry_offset: entry_offset as i64,
                        direction_mode: direction_mode.to_string(),
                        risk_basis: args.risk_basis.clone(),
                        risk_multiple,
                        target_r,
                        max_hold_multiple: args.max_hold_multiple,
                    };
                    template.template_name = format!(
                        "{} + confirmation+{} open + {} {:.3} risk + {:.2}R target",
                        if direction_mode == "inverse_pattern" {
                            "Inverse pattern"
                        } else {
                            "Pattern direction"
                        },
                        template.entry_offset,
                        if template.risk_basis == "xa_price_length" {
                            "XA"
                        } else {
                            "CD"
                        },
                        template.risk_multiple,
                        template.target_r
                    );

                    let evaluation = evaluate_template(&template, setup, candles);
                    if evaluation.outcome != "pass" {
                        continue;
                    }
                    let score = target_r * 100.0 - entry_offset as f64 * 3.0 - risk_multiple * 4.0
                        + if direction_mode == "pattern" {
                            5.0
                        } else {
                            0.0
                        };

                    let replace = best
                        .as_ref()
                        .map(|(_, _, best_score)| score > *best_score)
                        .unwrap_or(true);
                    if replace {
                        best = Some((template, evaluation, score));
                    }
                }
            }
        }
    }

    let (mut template, evaluation, _) = best?;
    template.rule_json = build_template_json(setup, &template, &evaluation).ok()?;
    Some(template)
}

async fn insert_template(
    pool: &MySqlPool,
    template: &GeneratedTemplate,
    setup: &PatternSetup,
    evaluation: &TemplateEvaluation,
    run_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO entry_exit_templates (
            template_uid,
            origin_run_id,
            template_name,
            entry_kind,
            direction_mode,
            risk_basis,
            risk_multiple,
            target_r,
            max_hold_multiple,
            rule_json,
            created_from_setup_id,
            created_from_pattern_id,
            created_from_family_key,
            created_from_symbol,
            created_from_market,
            created_from_d_confirm_date,
            first_result_r,
            notes
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&template.template_uid)
    .bind(run_id)
    .bind(&template.template_name)
    .bind(&template.entry_kind)
    .bind(&template.direction_mode)
    .bind(&template.risk_basis)
    .bind(template.risk_multiple)
    .bind(template.target_r)
    .bind(template.max_hold_multiple)
    .bind(&template.rule_json)
    .bind(&setup.setup_id)
    .bind(&setup.pattern_id)
    .bind(&setup.pattern_family_key)
    .bind(&setup.symbol)
    .bind(&setup.market)
    .bind(setup.d_confirm_date)
    .bind(evaluation.result_r)
    .bind("Prototype-created from a chronological pattern scan. Promote only after broader validation.")
    .execute(pool)
    .await?;

    Ok(())
}

async fn load_templates_for_run(
    pool: &MySqlPool,
    source_run_id: &str,
) -> Result<Vec<GeneratedTemplate>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            template_uid,
            template_name,
            CAST(rule_json AS CHAR) AS rule_json,
            entry_kind,
            COALESCE(
                CAST(JSON_UNQUOTE(JSON_EXTRACT(rule_json, '$.entry.offset_from_confirmation')) AS SIGNED),
                1
            ) AS entry_offset,
            direction_mode,
            risk_basis,
            risk_multiple,
            target_r,
            CAST(max_hold_multiple AS SIGNED) AS max_hold_multiple
        FROM entry_exit_templates
        WHERE origin_run_id = ?
          AND is_active = TRUE
        ORDER BY created_at ASC, template_uid ASC
        "#,
    )
    .bind(source_run_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(GeneratedTemplate {
                template_uid: row.try_get("template_uid")?,
                template_name: row.try_get("template_name")?,
                rule_json: row.try_get("rule_json")?,
                entry_kind: row.try_get("entry_kind")?,
                entry_offset: row.try_get("entry_offset")?,
                direction_mode: row.try_get("direction_mode")?,
                risk_basis: row.try_get("risk_basis")?,
                risk_multiple: row.try_get("risk_multiple")?,
                target_r: row.try_get("target_r")?,
                max_hold_multiple: row.try_get("max_hold_multiple")?,
            })
        })
        .collect()
}

async fn flush_result_batch(
    pool: &MySqlPool,
    result_table_name: &str,
    run_id: &str,
    results: &mut Vec<StoredTemplateResult>,
) -> Result<(), sqlx::Error> {
    if results.is_empty() {
        return Ok(());
    }

    let mut query_builder = QueryBuilder::<MySql>::new(format!(
        r#"
        INSERT INTO {} (
            run_id,
            template_uid,
            setup_id,
            pattern_id,
            pattern_group_id,
            event_id,
            event_rank,
            event_sister_count,
            event_decision_date,
            event_candidate_count,
            event_live_candidate_count,
            pattern_family_key,
            symbol,
            market,
            d_date,
            d_confirm_date,
            evaluation_order,
            was_created_for_setup,
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
"#,
        quoted_identifier(result_table_name)
    ));

    query_builder.push_values(results.iter(), |mut row, result| {
        row.push_bind(run_id)
            .push_bind(&result.template_uid)
            .push_bind(&result.setup_id)
            .push_bind(&result.pattern_id)
            .push_bind(&result.pattern_group_id)
            .push_bind(&result.event_id)
            .push_bind(result.event_rank)
            .push_bind(result.event_sister_count.unwrap_or(1))
            .push_bind(result.event_decision_date)
            .push_bind(result.event_candidate_count)
            .push_bind(result.event_live_candidate_count)
            .push_bind(&result.pattern_family_key)
            .push_bind(&result.symbol)
            .push_bind(&result.market)
            .push_bind(result.d_date)
            .push_bind(result.d_confirm_date)
            .push_bind(result.evaluation_order)
            .push_bind(result.was_created_for_setup)
            .push_bind(&result.outcome)
            .push_bind(&result.exit_reason)
            .push_bind(result.result_r)
            .push_bind(result.entry_date)
            .push_bind(result.exit_date)
            .push_bind(result.entry_price)
            .push_bind(result.stop_price)
            .push_bind(result.target_price)
            .push_bind(result.exit_price)
            .push_bind(result.risk_points)
            .push_bind(&result.trade_direction);
    });

    query_builder.push(
        r#"
        ON DUPLICATE KEY UPDATE
            evaluation_order = VALUES(evaluation_order),
            was_created_for_setup = VALUES(was_created_for_setup),
            event_id = VALUES(event_id),
            event_rank = VALUES(event_rank),
            event_sister_count = VALUES(event_sister_count),
            event_decision_date = VALUES(event_decision_date),
            event_candidate_count = VALUES(event_candidate_count),
            event_live_candidate_count = VALUES(event_live_candidate_count),
            outcome = VALUES(outcome),
            exit_reason = VALUES(exit_reason),
            result_r = VALUES(result_r),
            entry_date = VALUES(entry_date),
            exit_date = VALUES(exit_date),
            entry_price = VALUES(entry_price),
            stop_price = VALUES(stop_price),
            target_price = VALUES(target_price),
            exit_price = VALUES(exit_price),
            risk_points = VALUES(risk_points),
            trade_direction = VALUES(trade_direction)
        "#,
    );

    query_builder.build().execute(pool).await?;
    results.clear();

    Ok(())
}

async fn insert_run_summary(
    pool: &MySqlPool,
    run_id: &str,
    args: &Args,
    scanned_patterns: i64,
    templates_created: i64,
    existing_template_passes: i64,
    failed_to_create: i64,
    result_rows: i64,
    elapsed_ms: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO entry_exit_template_creator_runs (
            run_id,
            source_scope,
            period_year,
            requested_limit,
            scanned_patterns,
            templates_created,
            existing_template_passes,
            failed_to_create,
            result_rows,
            elapsed_ms
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(run_id)
    .bind(&args.source_scope)
    .bind(args.period_year)
    .bind(args.limit)
    .bind(scanned_patterns)
    .bind(templates_created)
    .bind(existing_template_passes)
    .bind(failed_to_create)
    .bind(result_rows)
    .bind(elapsed_ms)
    .execute(pool)
    .await?;

    Ok(())
}

async fn refresh_entry_exit_template_ui_stats(
    pool: &MySqlPool,
    result_table_name: &str,
    run_id: &str,
) -> Result<u64, sqlx::Error> {
    ensure_entry_exit_template_ui_stats_table(pool).await?;

    sqlx::query("DELETE FROM entry_exit_template_ui_stats WHERE run_id = ?")
        .bind(run_id)
        .execute(pool)
        .await?;

    let result = sqlx::query(&format!(
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
            FROM {}
            WHERE run_id = ?
            GROUP BY template_uid
        ) r ON r.template_uid = t.template_uid
        WHERE t.origin_run_id = ?
        "#,
        quoted_identifier(result_table_name)
    ))
    .bind(run_id)
    .bind(run_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

async fn insert_entry_exit_template_ui_stats_from_summary(
    pool: &MySqlPool,
    run_id: &str,
    templates: &[GeneratedTemplate],
    summary: &SummaryCollector,
) -> Result<u64, sqlx::Error> {
    ensure_entry_exit_template_ui_stats_table(pool).await?;

    sqlx::query("DELETE FROM entry_exit_template_ui_stats WHERE run_id = ?")
        .bind(run_id)
        .execute(pool)
        .await?;

    if templates.is_empty() {
        return Ok(0);
    }

    let mut affected = 0_u64;
    for chunk in templates.chunks(500) {
        let mut query_builder = QueryBuilder::<MySql>::new(
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
            "#,
        );

        query_builder.push_values(chunk, |mut row, template| {
            let total = summary
                .template_stats
                .get(&template.template_uid)
                .cloned()
                .unwrap_or_default();
            let bullish = summary
                .market_stats
                .get(&(template.template_uid.clone(), "Bullish".to_string()))
                .cloned()
                .unwrap_or_default();
            let bearish = summary
                .market_stats
                .get(&(template.template_uid.clone(), "Bearish".to_string()))
                .cloned()
                .unwrap_or_default();
            let market_edge_score = (bullish.avg_r() - bearish.avg_r()).abs();
            let market_edge_label = if market_edge_score < 0.05 {
                "Flat"
            } else if bullish.avg_r() > bearish.avg_r() {
                "Bullish"
            } else {
                "Bearish"
            };

            row.push_bind(run_id)
                .push_bind(&template.template_uid)
                .push_bind(total.eval_count)
                .push_bind(total.pass_count)
                .push_bind(total.fail_count)
                .push_bind(total.no_entry_count)
                .push_bind(total.avg_r())
                .push_bind(total.sum_r)
                .push_bind(total.best_r)
                .push_bind(total.worst_r)
                .push_bind(bullish.eval_count)
                .push_bind(bullish.pass_count)
                .push_bind(bullish.fail_count)
                .push_bind(bullish.no_entry_count)
                .push_bind(bullish.win_rate())
                .push_bind(bullish.avg_r())
                .push_bind(bearish.eval_count)
                .push_bind(bearish.pass_count)
                .push_bind(bearish.fail_count)
                .push_bind(bearish.no_entry_count)
                .push_bind(bearish.win_rate())
                .push_bind(bearish.avg_r())
                .push_bind(market_edge_label)
                .push_bind(market_edge_score);
        });

        affected += query_builder.build().execute(pool).await?.rows_affected();
    }

    Ok(affected)
}

async fn insert_entry_exit_template_condition_stats_from_summary(
    pool: &MySqlPool,
    run_id: &str,
    summary: &SummaryCollector,
) -> Result<u64, sqlx::Error> {
    ensure_entry_exit_template_condition_stats_table(pool).await?;

    sqlx::query("DELETE FROM entry_exit_template_condition_stats WHERE run_id = ?")
        .bind(run_id)
        .execute(pool)
        .await?;

    let mut rows = summary.condition_stats.iter().collect::<Vec<_>>();
    rows.sort_by(|left, right| left.0.cmp(right.0));

    let mut affected = 0_u64;
    for chunk in rows.chunks(500) {
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
            |mut row, ((template_uid, condition_type, condition_value), aggregate)| {
                row.push_bind(run_id)
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

        affected += query_builder.build().execute(pool).await?.rows_affected();
    }

    Ok(affected)
}

fn first_date(left: Option<NaiveDateTime>, right: Option<NaiveDateTime>) -> Option<NaiveDateTime> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn last_date(left: Option<NaiveDateTime>, right: Option<NaiveDateTime>) -> Option<NaiveDateTime> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

async fn refresh_entry_exit_template_build_coverage_ui_from_events(
    pool: &MySqlPool,
    run_id: &str,
    args: &Args,
    events: &[PatternEvent],
) -> Result<u64, sqlx::Error> {
    ensure_entry_exit_template_build_coverage_ui_table(pool).await?;

    sqlx::query("DELETE FROM entry_exit_template_build_coverage_ui WHERE run_id = ?")
        .bind(run_id)
        .execute(pool)
        .await?;

    let mut buckets: HashMap<(String, String), BuildCoverageBucket> = HashMap::new();
    for event in events {
        let Some(setup) = event.candidates.first() else {
            continue;
        };
        let root_symbol = normalized_root_symbol(setup);
        let source_timeframe = clean_optional_text(setup.source_timeframe.as_deref())
            .or_else(|| args.source_timeframe.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let contract_symbol = clean_optional_text(setup.contract_symbol.as_deref())
            .or_else(|| clean_optional_text(Some(&setup.symbol)))
            .unwrap_or_else(|| "Unknown".to_string());
        let bucket = buckets
            .entry((root_symbol, source_timeframe))
            .or_insert_with(BuildCoverageBucket::default);
        bucket.pattern_count += 1;
        bucket.contract_symbols.insert(contract_symbol);
        bucket.first_d_confirm_date =
            first_date(bucket.first_d_confirm_date, Some(setup.d_confirm_date));
        bucket.last_d_confirm_date =
            last_date(bucket.last_d_confirm_date, Some(setup.d_confirm_date));
    }

    if buckets.is_empty() {
        return Ok(0);
    }

    let mut rows = buckets
        .into_iter()
        .map(
            |((root_symbol, source_timeframe), bucket)| StoredBuildCoverageRow {
                run_id: run_id.to_string(),
                source_scope: args.source_scope.clone(),
                exchange_name: exchange_for_root(&root_symbol).to_string(),
                root_symbol,
                source_timeframe,
                pattern_count: bucket.pattern_count,
                contract_count: bucket.contract_symbols.len() as i64,
                first_d_confirm_date: bucket.first_d_confirm_date,
                last_d_confirm_date: bucket.last_d_confirm_date,
            },
        )
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left.exchange_name
            .cmp(&right.exchange_name)
            .then_with(|| right.pattern_count.cmp(&left.pattern_count))
            .then_with(|| left.root_symbol.cmp(&right.root_symbol))
            .then_with(|| left.source_timeframe.cmp(&right.source_timeframe))
    });

    let mut query_builder = QueryBuilder::<MySql>::new(
        r#"
        INSERT INTO entry_exit_template_build_coverage_ui (
            run_id,
            source_scope,
            root_symbol,
            exchange_name,
            source_timeframe,
            pattern_count,
            contract_count,
            first_d_confirm_date,
            last_d_confirm_date
        )
        "#,
    );

    query_builder.push_values(&rows, |mut builder, row| {
        builder
            .push_bind(&row.run_id)
            .push_bind(&row.source_scope)
            .push_bind(&row.root_symbol)
            .push_bind(&row.exchange_name)
            .push_bind(&row.source_timeframe)
            .push_bind(row.pattern_count)
            .push_bind(row.contract_count)
            .push_bind(row.first_d_confirm_date)
            .push_bind(row.last_d_confirm_date);
    });

    Ok(query_builder.build().execute(pool).await?.rows_affected())
}

async fn refresh_entry_exit_template_build_coverage_rows_ui(
    pool: &MySqlPool,
    run_id: &str,
) -> Result<u64, sqlx::Error> {
    ensure_entry_exit_template_build_coverage_rows_ui_table(pool).await?;

    let mut affected = 0_u64;
    sqlx::query("DELETE FROM entry_exit_template_build_coverage_rows_ui WHERE run_id = ?")
        .bind(run_id)
        .execute(pool)
        .await?;

    let scanned_result = sqlx::query(
        r#"
        INSERT INTO entry_exit_template_build_coverage_rows_ui (
            test_id,
            run_id,
            source_scope,
            exchange_name,
            root_symbol,
            source_timeframe,
            scanned_pattern_count,
            universe_pattern_count,
            contract_count,
            status,
            sort_order,
            first_d_confirm_date,
            last_d_confirm_date
        )
        SELECT
            scanned.run_id AS test_id,
            scanned.run_id,
            scanned.source_scope,
            scanned.exchange_name,
            scanned.root_symbol,
            scanned.source_timeframe,
            scanned.scanned_pattern_count,
            0 AS universe_pattern_count,
            scanned.contract_count,
            'Scanned' AS status,
            ROW_NUMBER() OVER (
                ORDER BY
                    CASE scanned.exchange_name
                        WHEN 'CME' THEN 1
                        WHEN 'CBOT' THEN 2
                        WHEN 'NYMEX' THEN 3
                        WHEN 'COMEX' THEN 4
                        ELSE 9
                    END,
                    scanned.scanned_pattern_count DESC,
                    scanned.root_symbol ASC,
                    scanned.source_timeframe ASC
            ) AS sort_order,
            scanned.first_d_confirm_date,
            scanned.last_d_confirm_date
        FROM (
            SELECT
                c.run_id,
                c.source_scope,
                CASE
                    WHEN normalized.root_symbol IN ('6A', '6B', '6C', '6E', '6J', '6M', '6N', '6S', 'BTC', 'EMD', 'ES', 'GF', 'HE', 'LE', 'M2K', 'MES', 'MNQ', 'NKD', 'NQ', 'RTY') THEN 'CME'
                    WHEN normalized.root_symbol IN ('KE', 'UB', 'YM', 'ZB', 'ZC', 'ZF', 'ZL', 'ZM', 'ZN', 'ZS', 'ZT', 'ZW') THEN 'CBOT'
                    WHEN normalized.root_symbol IN ('GC', 'HG', 'MGC', 'SI') THEN 'COMEX'
                    WHEN normalized.root_symbol IN ('CL', 'HO', 'MCL', 'NG', 'PA', 'PL', 'QG', 'QM', 'RB') THEN 'NYMEX'
                    ELSE COALESCE(MAX(NULLIF(c.exchange_name, '')), 'Unknown')
                END AS exchange_name,
                normalized.root_symbol,
                c.source_timeframe,
                CAST(SUM(c.pattern_count) AS SIGNED) AS scanned_pattern_count,
                CAST(SUM(c.contract_count) AS SIGNED) AS contract_count,
                MIN(c.first_d_confirm_date) AS first_d_confirm_date,
                MAX(c.last_d_confirm_date) AS last_d_confirm_date
            FROM entry_exit_template_build_coverage_ui c
            JOIN (
                SELECT
                    run_id,
                    source_timeframe,
                    root_symbol AS original_root_symbol,
                    CASE
                        WHEN root_symbol REGEXP '^ZB[FGHJKMNQUVXZ][0-9]{1,2}$' THEN 'ZB'
                        WHEN root_symbol REGEXP '^ZN[FGHJKMNQUVXZ][0-9]{1,2}$' THEN 'ZN'
                        ELSE root_symbol
                    END AS root_symbol
                FROM entry_exit_template_build_coverage_ui
                WHERE run_id = ?
            ) normalized
              ON normalized.run_id = c.run_id
             AND normalized.source_timeframe = c.source_timeframe
             AND normalized.original_root_symbol = c.root_symbol
            WHERE c.run_id = ?
            GROUP BY c.run_id, c.source_scope, normalized.root_symbol, c.source_timeframe
        ) scanned
        "#,
    )
    .bind(run_id)
    .bind(run_id)
    .execute(pool)
    .await?;
    affected += scanned_result.rows_affected();

    let universe_result = sqlx::query(
        r#"
        INSERT INTO entry_exit_template_build_coverage_rows_ui (
            test_id,
            run_id,
            source_scope,
            exchange_name,
            root_symbol,
            source_timeframe,
            scanned_pattern_count,
            universe_pattern_count,
            contract_count,
            status,
            sort_order,
            first_d_confirm_date,
            last_d_confirm_date
        )
        SELECT
            ? AS test_id,
            ? AS run_id,
            universe.source_scope,
            universe.exchange_name,
            universe.root_symbol,
            universe.source_timeframe,
            0 AS scanned_pattern_count,
            universe.universe_pattern_count,
            universe.contract_count,
            'Not scanned' AS status,
            100000 AS sort_order,
            universe.first_d_confirm_date,
            universe.last_d_confirm_date
        FROM (
            SELECT
                run_meta.source_scope,
                CASE
                    WHEN normalized.root_symbol IN ('6A', '6B', '6C', '6E', '6J', '6M', '6N', '6S', 'BTC', 'EMD', 'ES', 'GF', 'HE', 'LE', 'M2K', 'MES', 'MNQ', 'NKD', 'NQ', 'RTY') THEN 'CME'
                    WHEN normalized.root_symbol IN ('KE', 'UB', 'YM', 'ZB', 'ZC', 'ZF', 'ZL', 'ZM', 'ZN', 'ZS', 'ZT', 'ZW') THEN 'CBOT'
                    WHEN normalized.root_symbol IN ('GC', 'HG', 'MGC', 'SI') THEN 'COMEX'
                    WHEN normalized.root_symbol IN ('CL', 'HO', 'MCL', 'NG', 'PA', 'PL', 'QG', 'QM', 'RB') THEN 'NYMEX'
                    ELSE 'Unknown'
                END AS exchange_name,
                normalized.root_symbol,
                normalized.source_timeframe,
                CAST(COUNT(DISTINCT normalized.setup_id) AS SIGNED) AS universe_pattern_count,
                CAST(COUNT(DISTINCT normalized.contract_symbol) AS SIGNED) AS contract_count,
                MIN(normalized.d_confirm_date) AS first_d_confirm_date,
                MAX(normalized.d_confirm_date) AS last_d_confirm_date
            FROM (
                SELECT
                    ps.setup_id,
                    CASE
                        WHEN COALESCE(NULLIF(ps.root_symbol, ''), NULLIF(ps.symbol, ''), 'Unknown') REGEXP '^ZB[FGHJKMNQUVXZ][0-9]{1,2}$' THEN 'ZB'
                        WHEN COALESCE(NULLIF(ps.root_symbol, ''), NULLIF(ps.symbol, ''), 'Unknown') REGEXP '^ZN[FGHJKMNQUVXZ][0-9]{1,2}$' THEN 'ZN'
                        ELSE COALESCE(NULLIF(ps.root_symbol, ''), NULLIF(ps.symbol, ''), 'Unknown')
                    END AS root_symbol,
                    COALESCE(NULLIF(ps.contract_symbol, ''), NULLIF(ps.symbol, ''), 'Unknown') AS contract_symbol,
                    COALESCE(NULLIF(ps.source_timeframe, ''), 'unknown') AS source_timeframe,
                    COALESCE(ps.d_confirm_date, ps.d_date) AS d_confirm_date,
                    ps.source_table
                FROM pattern_setups ps
                JOIN (
                    SELECT DISTINCT source_timeframe
                    FROM entry_exit_template_build_coverage_ui
                    WHERE run_id = ?
                ) build_timeframes
                  ON build_timeframes.source_timeframe = COALESCE(NULLIF(ps.source_timeframe, ''), 'unknown')
                WHERE ps.d_date IS NOT NULL
                  AND ps.full_pattern_length > 0
                  AND ABS(ps.cd_price_length) > 0
            ) normalized
            JOIN entry_exit_template_creator_runs run_meta
              ON run_meta.run_id = ?
            WHERE (
                (run_meta.source_scope = 'futures' AND COALESCE(normalized.source_table, '') LIKE 'futures_contract_%_candles')
                OR (run_meta.source_scope = 'daily' AND COALESCE(normalized.source_table, '') = 'candles' AND normalized.source_timeframe = 'daily')
                OR (run_meta.source_scope NOT IN ('futures', 'daily'))
            )
            GROUP BY run_meta.source_scope, normalized.root_symbol, normalized.source_timeframe
        ) universe
        ON DUPLICATE KEY UPDATE
            universe_pattern_count = VALUES(universe_pattern_count),
            contract_count = GREATEST(entry_exit_template_build_coverage_rows_ui.contract_count, VALUES(contract_count)),
            status = CASE
                WHEN scanned_pattern_count > 0 THEN 'Scanned'
                ELSE VALUES(status)
            END
        "#,
    )
    .bind(run_id)
    .bind(run_id)
    .bind(run_id)
    .bind(run_id)
    .execute(pool)
    .await?;
    affected += universe_result.rows_affected();

    let sort_result = sqlx::query(
        r#"
        UPDATE entry_exit_template_build_coverage_rows_ui rows_ui
        JOIN (
            SELECT
                test_id,
                root_symbol,
                source_timeframe,
                ROW_NUMBER() OVER (
                    ORDER BY
                        CASE exchange_name
                            WHEN 'CME' THEN 1
                            WHEN 'CBOT' THEN 2
                            WHEN 'NYMEX' THEN 3
                            WHEN 'COMEX' THEN 4
                            ELSE 9
                        END,
                        CASE status WHEN 'Scanned' THEN 0 ELSE 1 END,
                        scanned_pattern_count DESC,
                        root_symbol ASC,
                        source_timeframe ASC
                ) AS next_sort_order
            FROM entry_exit_template_build_coverage_rows_ui
            WHERE test_id = ?
        ) ranked
          ON ranked.test_id = rows_ui.test_id
         AND ranked.root_symbol = rows_ui.root_symbol
         AND ranked.source_timeframe = rows_ui.source_timeframe
        SET rows_ui.sort_order = ranked.next_sort_order
        WHERE rows_ui.test_id = ?
        "#,
    )
    .bind(run_id)
    .bind(run_id)
    .execute(pool)
    .await?;
    affected += sort_result.rows_affected();

    Ok(affected)
}

async fn refresh_entry_exit_template_build_summary_ui(
    pool: &MySqlPool,
    run_id: &str,
) -> Result<u64, sqlx::Error> {
    ensure_entry_exit_template_build_summary_ui_table(pool).await?;

    let result = sqlx::query(
        r#"
        INSERT INTO entry_exit_template_build_summary_ui (
            test_id,
            run_id,
            build_label,
            source_scope,
            source_timeframe,
            scan_year_start,
            scan_year_end,
            scan_year_label,
            patterns_scanned,
            templates_created,
            coverage_patterns,
            root_count,
            exchange_count,
            requested_limit,
            result_rows,
            elapsed_ms,
            created_at
        )
        SELECT
            r.run_id AS test_id,
            r.run_id,
            NULL AS build_label,
            r.source_scope,
            COALESCE(NULLIF(coverage.source_timeframe, ''), 'unknown') AS source_timeframe,
            years.scan_year_start,
            years.scan_year_end,
            CASE
                WHEN years.scan_year_start IS NULL AND r.period_year > 0 THEN CAST(r.period_year AS CHAR)
                WHEN years.scan_year_start IS NULL THEN 'All'
                WHEN years.scan_year_start = years.scan_year_end THEN CAST(years.scan_year_start AS CHAR)
                ELSE CONCAT(years.scan_year_start, '-', years.scan_year_end)
            END AS scan_year_label,
            r.scanned_patterns AS patterns_scanned,
            r.templates_created,
            COALESCE(coverage.coverage_patterns, 0) AS coverage_patterns,
            COALESCE(coverage.root_count, 0) AS root_count,
            COALESCE(coverage.exchange_count, 0) AS exchange_count,
            r.requested_limit,
            r.result_rows,
            r.elapsed_ms,
            r.created_at
        FROM entry_exit_template_creator_runs r
        LEFT JOIN (
            SELECT
                run_id,
                CAST(SUM(pattern_count) AS SIGNED) AS coverage_patterns,
                CAST(COUNT(DISTINCT root_symbol) AS SIGNED) AS root_count,
                CAST(COUNT(DISTINCT CASE WHEN pattern_count > 0 THEN exchange_name ELSE NULL END) AS SIGNED) AS exchange_count,
                CASE
                    WHEN COUNT(DISTINCT source_timeframe) = 1 THEN MIN(source_timeframe)
                    WHEN COUNT(DISTINCT source_timeframe) > 1 THEN 'mixed'
                    ELSE 'unknown'
                END AS source_timeframe
            FROM entry_exit_template_build_coverage_ui
            WHERE run_id = ?
            GROUP BY run_id
        ) coverage
          ON coverage.run_id = r.run_id
        LEFT JOIN (
            SELECT
                run_id,
                MIN(YEAR(first_d_confirm_date)) AS scan_year_start,
                MAX(YEAR(last_d_confirm_date)) AS scan_year_end
            FROM entry_exit_template_build_coverage_ui
            WHERE run_id = ?
            GROUP BY run_id
        ) years
          ON years.run_id = r.run_id
        WHERE r.run_id = ?
        ON DUPLICATE KEY UPDATE
            run_id = VALUES(run_id),
            build_label = COALESCE(VALUES(build_label), build_label),
            source_scope = VALUES(source_scope),
            source_timeframe = VALUES(source_timeframe),
            scan_year_start = VALUES(scan_year_start),
            scan_year_end = VALUES(scan_year_end),
            scan_year_label = VALUES(scan_year_label),
            patterns_scanned = VALUES(patterns_scanned),
            templates_created = VALUES(templates_created),
            coverage_patterns = VALUES(coverage_patterns),
            root_count = VALUES(root_count),
            exchange_count = VALUES(exchange_count),
            requested_limit = VALUES(requested_limit),
            result_rows = VALUES(result_rows),
            elapsed_ms = VALUES(elapsed_ms),
            created_at = VALUES(created_at)
        "#,
    )
    .bind(run_id)
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
    result_table_name: &str,
    run_id: &str,
) -> Result<u64, sqlx::Error> {
    ensure_entry_exit_template_condition_stats_table(pool).await?;

    sqlx::query("DELETE FROM entry_exit_template_condition_stats WHERE run_id = ?")
        .bind(run_id)
        .execute(pool)
        .await?;

    let mut aggregates: HashMap<(String, String, String), ConditionAggregate> = HashMap::new();
    let condition_sql = format!(
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
        FROM {} r FORCE INDEX (idx_entry_exit_template_results_run_setup)
        LEFT JOIN pattern_setups ps
          ON ps.setup_id = r.setup_id
        WHERE r.run_id = ?
        "#,
        quoted_identifier(result_table_name)
    );
    let mut rows = sqlx::query(&condition_sql).bind(run_id).fetch(pool);

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

async fn print_template_leaderboard(pool: &MySqlPool, run_id: &str) -> Result<(), sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            t.template_uid,
            t.template_name,
            t.created_from_symbol,
            COALESCE(t.created_from_family_key, 'N/A') AS created_from_family_key,
            COALESCE(s.eval_count, 0) AS eval_count,
            COALESCE(s.pass_count, 0) AS pass_count,
            COALESCE(s.fail_count, 0) AS fail_count,
            COALESCE(s.no_entry_count, 0) AS no_entry_count,
            COALESCE(s.avg_r, 0) AS avg_r
        FROM entry_exit_templates t
        LEFT JOIN entry_exit_template_ui_stats s
          ON s.run_id = t.origin_run_id
         AND s.template_uid = t.template_uid
        WHERE t.origin_run_id = ?
        ORDER BY pass_count DESC, avg_r DESC, eval_count DESC
        LIMIT 12
        "#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await?;

    println!();
    println!("Top generated templates for run {run_id}");
    println!("template\tpasses/fails/no-entry\tevals\tavg_r\tcreated_from");
    for row in rows {
        let template_uid: String = row.try_get("template_uid")?;
        let template_name: String = row.try_get("template_name")?;
        let symbol: String = row.try_get("created_from_symbol")?;
        let family: String = row.try_get("created_from_family_key")?;
        let eval_count: i64 = row.try_get("eval_count")?;
        let pass_count: i64 = row.try_get("pass_count")?;
        let fail_count: i64 = row.try_get("fail_count")?;
        let no_entry_count: i64 = row.try_get("no_entry_count")?;
        let avg_r: Option<f64> = row.try_get("avg_r")?;
        println!(
            "{}\t{}/{}/{}\t{}\t{:.3}\t{} / {}\t{}",
            template_uid,
            pass_count,
            fail_count,
            no_entry_count,
            eval_count,
            avg_r.unwrap_or(0.0),
            symbol,
            family,
            template_name
        );
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let args = parse_args()?;
    let pool = MySqlPool::connect(&database_url_from_env()?).await?;
    ensure_template_tables(&pool).await?;

    let patterns = fetch_patterns(&pool, &args).await?;
    let raw_patterns_loaded = patterns.len();
    let events = build_pattern_events(patterns, args.event_limit());
    if events.is_empty() {
        println!(
            "No patterns found for source={} year={} family={:?} symbol={:?}",
            args.source_scope,
            args.year_label(),
            args.family_key,
            args.symbol
        );
        return Ok(());
    }

    let store_raw_results = args.stores_raw_results();

    if let Some(template_source_run_id) = args.template_source_run_id.as_deref() {
        if !store_raw_results {
            return Err("--template-source-run-id requires --result-storage full so validation rows are stored".into());
        }

        let run_id = args
            .output_run_id
            .as_deref()
            .unwrap_or(template_source_run_id)
            .to_string();
        let result_table_name = result_table_name_for_run(&run_id);
        ensure_result_table_for_run(&pool, &result_table_name).await?;

        let templates = load_templates_for_run(&pool, template_source_run_id).await?;
        if templates.is_empty() {
            println!("No active templates found for source run {template_source_run_id}.");
            return Ok(());
        }

        let started = Instant::now();
        let mut pending_results: Vec<StoredTemplateResult> =
            Vec::with_capacity(RESULT_BATCH_SIZE);
        let mut summary = SummaryCollector::default();
        let mut result_rows = 0_i64;

        println!(
            "Entry/Exit existing-template evaluator run {run_id}: {} raw patterns -> {} Twin/Event decisions, templates_from={}, templates={}, source={}, timeframe={}, year={}, event_result_policy={}, result_storage={}",
            raw_patterns_loaded,
            events.len(),
            template_source_run_id,
            templates.len(),
            args.source_scope,
            args.source_timeframe.as_deref().unwrap_or("all"),
            args.year_label(),
            args.event_result_policy,
            args.result_storage
        );
        println!("Build test results table: {result_table_name}");
        println!("Template discovery is disabled in this mode; only existing templates are evaluated.");

        for (event_index, event) in events.iter().enumerate() {
            let mut candle_cache: HashMap<String, Vec<ForwardCandle>> = HashMap::new();
            for setup in &event.candidates {
                let candles = match fetch_forward_candles(&pool, setup, args.max_hold_multiple).await
                {
                    Ok(candles) => candles,
                    Err(error) => {
                        eprintln!(
                            "Existing-template candle fetch failed for setup {} ({}): {:?}",
                            setup.setup_id, setup.symbol, error
                        );
                        Vec::new()
                    }
                };
                candle_cache.insert(setup.setup_id.clone(), candles);
            }

            let evaluation_order = event_index as i64 + 1;
            for template in &templates {
                let Some(event_evaluation) = evaluate_template_on_event(
                    template,
                    event,
                    &candle_cache,
                    evaluation_order,
                    &args.event_result_policy,
                    None,
                ) else {
                    continue;
                };
                summary.record(&event_evaluation.selected_result);
                pending_results.push(event_evaluation.selected_result);
                result_rows += 1;

                if pending_results.len() >= RESULT_BATCH_SIZE {
                    flush_result_batch(&pool, &result_table_name, &run_id, &mut pending_results)
                        .await?;
                }
            }

            if (event_index + 1) % 500 == 0 || event_index + 1 == events.len() {
                println!(
                    "Existing-template coverage {}/{} decisions, templates={}, result_rows={}",
                    event_index + 1,
                    events.len(),
                    templates.len(),
                    result_rows
                );
            }
        }

        flush_result_batch(&pool, &result_table_name, &run_id, &mut pending_results).await?;

        let elapsed_ms = started.elapsed().as_millis() as i64;
        println!();
        println!(
            "Stored existing-template evaluation rows for {run_id}: scanned={}, templates_evaluated={}, result_rows={}, elapsed={:.2}s",
            events.len(),
            templates.len(),
            result_rows,
            elapsed_ms as f64 / 1000.0
        );

        return Ok(());
    }

    let run_id = creator_run_id();
    let result_table_name = result_table_name_for_run(&run_id);
    if store_raw_results {
        ensure_result_table_for_run(&pool, &result_table_name).await?;
    }
    let started = Instant::now();
    let mut templates: Vec<GeneratedTemplate> = Vec::new();
    let mut existing_template_passes = 0_i64;
    let mut failed_to_create = 0_i64;
    let mut pending_results: Vec<StoredTemplateResult> = Vec::with_capacity(RESULT_BATCH_SIZE);
    let mut tested_template_events: HashSet<(String, String)> = HashSet::new();
    let mut summary = SummaryCollector::default();

    println!(
        "Entry/Exit template creator run {run_id}: {} raw patterns -> {} Twin/Event decisions, source={}, timeframe={}, year={}, event_result_policy={}, result_storage={}, entry_kind={}, entry_offset={}, direction_mode={}, risk_basis={}, risk_multiple={}, target_r={}, max_hold_multiple={}",
        raw_patterns_loaded,
        events.len(),
        args.source_scope,
        args.source_timeframe.as_deref().unwrap_or("all"),
        args.year_label(),
        args.event_result_policy,
        args.result_storage,
        args.entry_kind,
        args.entry_offset
            .map(|value| value.to_string())
            .unwrap_or_else(|| "auto".to_string()),
        args.direction_mode.as_deref().unwrap_or("auto"),
        args.risk_basis,
        args.risk_multiple
            .map(|value| format!("{value:.3}"))
            .unwrap_or_else(|| "auto".to_string()),
        args.target_r
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| "auto".to_string()),
        args.max_hold_multiple
    );
    if store_raw_results {
        println!("Build test results table: {result_table_name}");
    } else {
        println!("Build test result rows: summary-only (raw rows are not stored)");
    }
    println!(
        "Pattern loop: raw twins may create templates; stored template stats are one selected result per Twin/Event."
    );

    for (event_index, event) in events.iter().enumerate() {
        let mut candle_cache: HashMap<String, Vec<ForwardCandle>> = HashMap::new();
        for setup in &event.candidates {
            let candles = match fetch_forward_candles(&pool, setup, args.max_hold_multiple).await {
                Ok(candles) => candles,
                Err(error) => {
                    eprintln!(
                        "Candle fetch failed for setup {} ({}): {:?}",
                        setup.setup_id, setup.symbol, error
                    );
                    Vec::new()
                }
            };
            candle_cache.insert(setup.setup_id.clone(), candles);
        }

        let evaluation_order = event_index as i64 + 1;
        let mut covered_setup_ids: HashSet<String> = HashSet::new();
        let template_count_at_event_start = templates.len();
        for template_index in 0..template_count_at_event_start {
            let template = &templates[template_index];
            let Some(event_evaluation) = evaluate_template_on_event(
                template,
                event,
                &candle_cache,
                evaluation_order,
                &args.event_result_policy,
                None,
            ) else {
                continue;
            };
            if event_evaluation.any_passed {
                existing_template_passes += 1;
                for setup_id in event_evaluation.passed_setup_ids {
                    covered_setup_ids.insert(setup_id);
                }
            }
            tested_template_events.insert((template.template_uid.clone(), event.event_key.clone()));
            summary.record(&event_evaluation.selected_result);
            if store_raw_results {
                pending_results.push(event_evaluation.selected_result);
            }
            if store_raw_results && pending_results.len() >= RESULT_BATCH_SIZE {
                flush_result_batch(&pool, &result_table_name, &run_id, &mut pending_results)
                    .await?;
            }
        }

        for setup in &event.candidates {
            if covered_setup_ids.contains(&setup.setup_id) {
                continue;
            }

            let candles = candle_cache
                .get(&setup.setup_id)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let next_sequence = templates.len() + 1;
            match synthesize_template(&run_id, next_sequence, setup, &candles, &args) {
                Some(template) => {
                    let evaluation = evaluate_template(&template, setup, &candles);
                    insert_template(&pool, &template, setup, &evaluation, &run_id).await?;
                    if let Some(event_evaluation) = evaluate_template_on_event(
                        &template,
                        event,
                        &candle_cache,
                        evaluation_order,
                        &args.event_result_policy,
                        Some(&setup.setup_id),
                    ) {
                        for setup_id in event_evaluation.passed_setup_ids {
                            covered_setup_ids.insert(setup_id);
                        }
                        tested_template_events
                            .insert((template.template_uid.clone(), event.event_key.clone()));
                        summary.record(&event_evaluation.selected_result);
                        if store_raw_results {
                            pending_results.push(event_evaluation.selected_result);
                        }
                        if store_raw_results && pending_results.len() >= RESULT_BATCH_SIZE {
                            flush_result_batch(
                                &pool,
                                &result_table_name,
                                &run_id,
                                &mut pending_results,
                            )
                            .await?;
                        }
                    }
                    println!(
                        "Created {} from event {}/{} candidate {}/{}: {} {} {} family={} result={:.2}R",
                        template.template_uid,
                        event_index + 1,
                        events.len(),
                        setup.event_rank.unwrap_or(1),
                        setup.event_sister_count.unwrap_or(event.candidates.len() as i64),
                        setup.symbol,
                        setup.market,
                        setup.d_confirm_date,
                        setup.pattern_family_key.as_deref().unwrap_or("N/A"),
                        evaluation.result_r.unwrap_or(0.0)
                    );
                    templates.push(template);
                }
                None => {
                    failed_to_create += 1;
                    println!(
                        "No passing template could be synthesized for event {}/{} candidate {}/{}: {} {} {} family={}",
                        event_index + 1,
                        events.len(),
                        setup.event_rank.unwrap_or(1),
                        setup.event_sister_count.unwrap_or(event.candidates.len() as i64),
                        setup.symbol,
                        setup.market,
                        setup.d_confirm_date,
                        setup.pattern_family_key.as_deref().unwrap_or("N/A")
                    );
                }
            }
        }

        if (event_index + 1) % 25 == 0 || event_index + 1 == events.len() {
            println!(
                "Processed {}/{} Twin/Event decisions, generated templates={}, existing template passes={}",
                event_index + 1,
                events.len(),
                templates.len(),
                existing_template_passes
            );
        }
    }

    if store_raw_results {
        flush_result_batch(&pool, &result_table_name, &run_id, &mut pending_results).await?;
    }

    println!();
    println!(
        "Template discovery complete: {} templates created. Filling missing template/event tests across {} decisions.",
        templates.len(),
        events.len()
    );
    let mut result_rows = tested_template_events.len() as i64;
    let mut missing_result_rows = 0_i64;

    for (event_index, event) in events.iter().enumerate() {
        let mut candle_cache: HashMap<String, Vec<ForwardCandle>> = HashMap::new();
        for setup in &event.candidates {
            let candles = match fetch_forward_candles(&pool, setup, args.max_hold_multiple).await {
                Ok(candles) => candles,
                Err(error) => {
                    eprintln!(
                        "Full coverage candle fetch failed for setup {} ({}): {:?}",
                        setup.setup_id, setup.symbol, error
                    );
                    Vec::new()
                }
            };
            candle_cache.insert(setup.setup_id.clone(), candles);
        }

        let evaluation_order = event_index as i64 + 1;
        for template in &templates {
            let result_key = (template.template_uid.clone(), event.event_key.clone());
            if tested_template_events.contains(&result_key) {
                continue;
            }

            let Some(event_evaluation) = evaluate_template_on_event(
                template,
                event,
                &candle_cache,
                evaluation_order,
                &args.event_result_policy,
                None,
            ) else {
                continue;
            };
            tested_template_events.insert(result_key);
            summary.record(&event_evaluation.selected_result);
            if store_raw_results {
                pending_results.push(event_evaluation.selected_result);
            }
            result_rows += 1;
            missing_result_rows += 1;
            if store_raw_results && pending_results.len() >= RESULT_BATCH_SIZE {
                flush_result_batch(&pool, &result_table_name, &run_id, &mut pending_results)
                    .await?;
            }
        }

        if (event_index + 1) % 500 == 0 || event_index + 1 == events.len() {
            println!(
                "Coverage fill {}/{} decisions, templates={}, missing_added={}, final_rows={}",
                event_index + 1,
                events.len(),
                templates.len(),
                missing_result_rows,
                result_rows
            );
        }
    }

    if store_raw_results {
        flush_result_batch(&pool, &result_table_name, &run_id, &mut pending_results).await?;
    }

    let elapsed_ms = started.elapsed().as_millis() as i64;
    insert_run_summary(
        &pool,
        &run_id,
        &args,
        events.len() as i64,
        templates.len() as i64,
        existing_template_passes,
        failed_to_create,
        result_rows,
        elapsed_ms,
    )
    .await?;

    let ui_stats_rows =
        insert_entry_exit_template_ui_stats_from_summary(&pool, &run_id, &templates, &summary)
            .await?;
    let condition_stats_rows = if args.skip_condition_stats {
        println!("Skipped Entry/Exit condition stats refresh (--skip-condition-stats).");
        0
    } else {
        insert_entry_exit_template_condition_stats_from_summary(&pool, &run_id, &summary).await?
    };
    let coverage_rows =
        refresh_entry_exit_template_build_coverage_ui_from_events(&pool, &run_id, &args, &events)
            .await?;
    let coverage_table_rows =
        refresh_entry_exit_template_build_coverage_rows_ui(&pool, &run_id).await?;
    let build_summary_rows = refresh_entry_exit_template_build_summary_ui(&pool, &run_id).await?;

    println!();
    println!(
        "Stored creator run {run_id}: scanned={}, templates_created={}, existing_template_passes={}, failed_to_create={}, result_rows={}, ui_stats_rows={}, coverage_rows={}, coverage_table_rows={}, build_summary_rows={}, condition_stats_rows={}, elapsed={:.2}s",
        events.len(),
        templates.len(),
        existing_template_passes,
        failed_to_create,
        result_rows,
        ui_stats_rows,
        coverage_rows,
        coverage_table_rows,
        build_summary_rows,
        condition_stats_rows,
        elapsed_ms as f64 / 1000.0
    );
    print_template_leaderboard(&pool, &run_id).await?;

    Ok(())
}
