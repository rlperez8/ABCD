use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, env};

use chrono::{Datelike, NaiveDateTime, Timelike};
use futures_util::TryStreamExt;
use serde::Serialize;
use sqlx::{MySql, MySqlPool, QueryBuilder, Row};

const RESULT_BATCH_SIZE: usize = 1_000;

#[derive(Debug)]
struct Args {
    source_scope: String,
    period_year: i64,
    start_year: i64,
    end_year: i64,
    limit: i64,
    sister_window_minutes: i64,
    family_key: Option<String>,
    symbol: Option<String>,
}

#[derive(Clone, sqlx::FromRow)]
struct PatternSetup {
    setup_id: String,
    pattern_id: Option<String>,
    pattern_group_id: String,
    symbol: String,
    root_symbol: Option<String>,
    source_table: Option<String>,
    source_timeframe: Option<String>,
    market: String,
    pattern_family_key: Option<String>,
    x_date: NaiveDateTime,
    x_high: f64,
    x_low: f64,
    a_date: NaiveDateTime,
    a_high: f64,
    a_low: f64,
    d_date: NaiveDateTime,
    d_high: f64,
    d_low: f64,
    d_confirm_date: NaiveDateTime,
    cd_price_length: f64,
    full_pattern_length: i64,
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
    pattern_family_key: Option<String>,
    symbol: String,
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
            pattern_family_key: setup.pattern_family_key.clone(),
            symbol: setup.symbol.clone(),
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

fn usage() -> &'static str {
    "Usage: cargo run --bin create_entry_exit_templates -- [--source futures|daily|all] [--year YYYY | --start-year YYYY --end-year YYYY] [--limit N] [--family FAMILY_KEY] [--symbol SYMBOL] [--sister-window-minutes N]\nUse --limit 0 to scan every deduped event in the selected period. Creates templates chronologically from deduped pattern events. It does not read entry_exit_tests or the Phase 1 optimizer routes."
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

    let source_scope = normalize_source_scope(arg_value(&raw_args, "--source"));
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
    let sister_window_minutes = arg_value(&raw_args, "--sister-window-minutes")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(15)
        .clamp(1, 240);
    let family_key = arg_value(&raw_args, "--family")
        .or_else(|| arg_value(&raw_args, "--family-key"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let symbol = arg_value(&raw_args, "--symbol")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    Ok(Args {
        source_scope,
        period_year,
        start_year,
        end_year,
        limit,
        sister_window_minutes,
        family_key,
        symbol,
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

    fn dedupe_limit(&self) -> Option<usize> {
        if self.limit > 0 {
            Some(self.limit as usize)
        } else {
            None
        }
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
    ensure_entry_exit_template_condition_stats_table(pool).await?;
    ensure_entry_exit_template_indexes(pool).await?;

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

async fn fetch_patterns(pool: &MySqlPool, args: &Args) -> Result<Vec<PatternSetup>, sqlx::Error> {
    let raw_candidate_limit = if args.limit > 0 {
        Some(args.limit.saturating_mul(10).max(args.limit).min(100_000))
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
            ps.symbol,
            ps.root_symbol,
            ps.source_table,
            ps.source_timeframe,
            ps.market,
            ps.pattern_family_key,
            ps.x_date,
            ps.x_high,
            ps.x_low,
            ps.a_date,
            ps.a_high,
            ps.a_low,
            ps.d_date,
            ps.d_high,
            ps.d_low,
            CAST(COALESCE(ps.d_confirm_date, ps.d_date) AS DATETIME) AS d_confirm_date,
            ps.cd_price_length,
            CAST(ps.full_pattern_length AS SIGNED) AS full_pattern_length
        FROM pattern_setups ps
        WHERE ps.d_date IS NOT NULL
          AND ps.full_pattern_length > 0
          AND ABS(ps.cd_price_length) > 0
          {source_filter}
          {year_filter}
          {family_filter}
          {symbol_filter}
        ORDER BY COALESCE(ps.d_confirm_date, ps.d_date) ASC, ps.setup_id ASC
        {limit_clause}
        "#,
        source_filter = source_filter,
        year_filter = year_filter,
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
    if let Some(family_key) = &args.family_key {
        query = query.bind(family_key);
    }
    if let Some(symbol) = &args.symbol {
        query = query.bind(symbol).bind(symbol);
    }

    if let Some(raw_candidate_limit) = raw_candidate_limit {
        query = query.bind(raw_candidate_limit);
    }
    let candidates = query.fetch_all(pool).await?;
    let before_count = candidates.len();
    let patterns =
        dedupe_sister_patterns(candidates, args.dedupe_limit(), args.sister_window_minutes);
    println!(
        "Pattern event filter: fetched {} raw candidates, kept {} distinct events, skipped {} sister pattern(s). Sister window={}m.",
        before_count,
        patterns.len(),
        before_count.saturating_sub(patterns.len()),
        args.sister_window_minutes
    );
    Ok(patterns)
}

fn setup_root_symbol(setup: &PatternSetup) -> &str {
    setup
        .root_symbol
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&setup.symbol)
}

fn minutes_between(left: NaiveDateTime, right: NaiveDateTime) -> i64 {
    left.signed_duration_since(right).num_minutes().abs()
}

fn midpoint(high: f64, low: f64) -> f64 {
    (high + low) / 2.0
}

fn similar_price(left: f64, right: f64, tolerance: f64) -> bool {
    (left - right).abs() <= tolerance
}

fn anchor_time_window(
    left: &PatternSetup,
    right: &PatternSetup,
    sister_window_minutes: i64,
) -> i64 {
    let pattern_minutes = left
        .full_pattern_length
        .max(right.full_pattern_length)
        .saturating_mul(2)
        .clamp(sister_window_minutes, 240);
    pattern_minutes.max(sister_window_minutes)
}

fn is_sister_pattern(
    kept: &PatternSetup,
    candidate: &PatternSetup,
    sister_window_minutes: i64,
) -> bool {
    if setup_root_symbol(kept) != setup_root_symbol(candidate) {
        return false;
    }
    if !kept.market.eq_ignore_ascii_case(&candidate.market) {
        return false;
    }
    if minutes_between(kept.d_confirm_date, candidate.d_confirm_date) > sister_window_minutes {
        return false;
    }

    let anchor_window = anchor_time_window(kept, candidate, sister_window_minutes);
    let anchor_time_matches = [
        minutes_between(kept.x_date, candidate.x_date) <= anchor_window,
        minutes_between(kept.a_date, candidate.a_date) <= anchor_window,
        minutes_between(kept.d_date, candidate.d_date) <= sister_window_minutes,
    ]
    .into_iter()
    .filter(|matched| *matched)
    .count();
    if anchor_time_matches < 2 {
        return false;
    }

    let cd_scale = kept
        .cd_price_length
        .abs()
        .max(candidate.cd_price_length.abs())
        .max(0.000001);
    let price_tolerance = cd_scale * 0.15;
    let anchor_price_matches = [
        similar_price(
            midpoint(kept.x_high, kept.x_low),
            midpoint(candidate.x_high, candidate.x_low),
            price_tolerance,
        ),
        similar_price(
            midpoint(kept.a_high, kept.a_low),
            midpoint(candidate.a_high, candidate.a_low),
            price_tolerance,
        ),
        similar_price(
            midpoint(kept.d_high, kept.d_low),
            midpoint(candidate.d_high, candidate.d_low),
            price_tolerance,
        ),
    ]
    .into_iter()
    .filter(|matched| *matched)
    .count();

    anchor_price_matches >= 2
}

fn dedupe_sister_patterns(
    candidates: Vec<PatternSetup>,
    limit: Option<usize>,
    sister_window_minutes: i64,
) -> Vec<PatternSetup> {
    let mut kept: Vec<PatternSetup> =
        Vec::with_capacity(limit.unwrap_or(candidates.len()).min(candidates.len()));

    for candidate in candidates {
        let mut is_sister = false;
        for existing in kept.iter().rev() {
            if minutes_between(existing.d_confirm_date, candidate.d_confirm_date)
                > sister_window_minutes
            {
                break;
            }
            if is_sister_pattern(existing, &candidate, sister_window_minutes) {
                is_sister = true;
                break;
            }
        }
        if is_sister {
            continue;
        }

        kept.push(candidate);
        if limit.is_some_and(|limit| kept.len() >= limit) {
            break;
        }
    }

    kept
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
    template: &GeneratedTemplate,
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
            kind: "confirmation_plus_n_open".to_string(),
            offset_from_confirmation: template.entry_offset,
            price: "open".to_string(),
        },
        stop: StopRule {
            kind: "cd_fraction_from_entry".to_string(),
            basis: template.risk_basis.clone(),
            multiple: template.risk_multiple,
            description:
                "Risk is calculated as ABS(CD price length) multiplied by this template multiple."
                    .to_string(),
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
) -> Option<GeneratedTemplate> {
    let start_index = forward_start_index(setup, candles);
    if candles.len() <= start_index {
        return None;
    }

    let cd_length = setup.cd_price_length.abs();
    if !cd_length.is_finite() || cd_length <= 0.0 {
        return None;
    }

    let max_hold_bars = setup.full_pattern_length.saturating_mul(5).max(1) as usize;
    let max_entry_offset = candles
        .len()
        .saturating_sub(start_index)
        .min(max_hold_bars)
        .min(24);
    let target_rs = [4.0, 3.0, 2.0, 1.5, 1.0, 0.75, 0.5];
    let risk_buffer = (cd_length * 0.01).max(0.01);
    let mut best: Option<(GeneratedTemplate, TemplateEvaluation, f64)> = None;

    for entry_offset in 1..=max_entry_offset {
        let entry_index = start_index + entry_offset - 1;
        let Some(entry_candle) = candles.get(entry_index) else {
            continue;
        };
        let end_index = candles.len().min(entry_index.saturating_add(max_hold_bars));
        if end_index <= entry_index {
            continue;
        }

        for direction_mode in ["pattern", "inverse_pattern"] {
            let direction = direction_for_mode(setup, direction_mode);
            let entry_price = entry_candle.open;
            let mut max_adverse = 0.0f64;

            for candle in &candles[entry_index..end_index] {
                let (adverse, favorable) = candidate_price_stats(direction, entry_price, candle);
                max_adverse = max_adverse.max(adverse);

                for target_r in target_rs {
                    if favorable <= 0.0 {
                        continue;
                    }
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
                    let risk_multiple = risk_points / cd_length;
                    if !(0.005..=5.0).contains(&risk_multiple) {
                        continue;
                    }

                    let mut template = GeneratedTemplate {
                        template_uid: template_uid(run_id, sequence),
                        template_name: String::new(),
                        rule_json: "{}".to_string(),
                        entry_offset: entry_offset as i64,
                        direction_mode: direction_mode.to_string(),
                        risk_basis: "cd_price_length".to_string(),
                        risk_multiple,
                        target_r,
                        max_hold_multiple: 5,
                    };
                    template.template_name = format!(
                        "{} + confirmation+{} open + CD {:.3} risk + {:.2}R target",
                        if direction_mode == "inverse_pattern" {
                            "Inverse pattern"
                        } else {
                            "Pattern direction"
                        },
                        template.entry_offset,
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
    .bind("confirmation_plus_n_open")
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

async fn flush_result_batch(
    pool: &MySqlPool,
    run_id: &str,
    results: &mut Vec<StoredTemplateResult>,
) -> Result<(), sqlx::Error> {
    if results.is_empty() {
        return Ok(());
    }

    let mut query_builder = QueryBuilder::<MySql>::new(
        r#"
        INSERT INTO entry_exit_template_results (
            run_id,
            template_uid,
            setup_id,
            pattern_id,
            pattern_group_id,
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
    );

    query_builder.push_values(results.iter(), |mut row, result| {
        row.push_bind(run_id)
            .push_bind(&result.template_uid)
            .push_bind(&result.setup_id)
            .push_bind(&result.pattern_id)
            .push_bind(&result.pattern_group_id)
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

async fn print_template_leaderboard(pool: &MySqlPool, run_id: &str) -> Result<(), sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            t.template_uid,
            t.template_name,
            t.created_from_symbol,
            COALESCE(t.created_from_family_key, 'N/A') AS created_from_family_key,
            COALESCE(r.eval_count, 0) AS eval_count,
            COALESCE(r.pass_count, 0) AS pass_count,
            COALESCE(r.fail_count, 0) AS fail_count,
            COALESCE(r.no_entry_count, 0) AS no_entry_count,
            COALESCE(r.avg_r, 0) AS avg_r
        FROM entry_exit_templates t
        LEFT JOIN (
            SELECT
                template_uid,
                CAST(COUNT(*) AS SIGNED) AS eval_count,
                CAST(SUM(CASE WHEN outcome = 'pass' THEN 1 ELSE 0 END) AS SIGNED) AS pass_count,
                CAST(SUM(CASE WHEN outcome = 'fail' THEN 1 ELSE 0 END) AS SIGNED) AS fail_count,
                CAST(SUM(CASE WHEN outcome = 'no_entry' THEN 1 ELSE 0 END) AS SIGNED) AS no_entry_count,
                AVG(COALESCE(result_r, 0)) AS avg_r
            FROM entry_exit_template_results
            WHERE run_id = ?
            GROUP BY template_uid
        ) r ON r.template_uid = t.template_uid
        WHERE t.origin_run_id = ?
        ORDER BY pass_count DESC, avg_r DESC, eval_count DESC
        LIMIT 12
        "#,
    )
    .bind(run_id)
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
    if patterns.is_empty() {
        println!(
            "No patterns found for source={} year={} family={:?} symbol={:?}",
            args.source_scope,
            args.year_label(),
            args.family_key,
            args.symbol
        );
        return Ok(());
    }

    let run_id = creator_run_id();
    let started = Instant::now();
    let mut templates: Vec<GeneratedTemplate> = Vec::new();
    let mut existing_template_passes = 0_i64;
    let mut failed_to_create = 0_i64;
    let mut result_rows = 0_i64;
    let mut pending_results: Vec<StoredTemplateResult> = Vec::with_capacity(RESULT_BATCH_SIZE);

    println!(
        "Entry/Exit template creator run {run_id}: {} patterns, source={}, year={}",
        patterns.len(),
        args.source_scope,
        args.year_label()
    );
    println!(
        "Pattern loop: evaluate existing generated templates first; create one only if none pass."
    );

    for (pattern_index, setup) in patterns.iter().enumerate() {
        let candles = match fetch_forward_candles(&pool, setup).await {
            Ok(candles) => candles,
            Err(error) => {
                eprintln!(
                    "Candle fetch failed for setup {} ({}): {:?}",
                    setup.setup_id, setup.symbol, error
                );
                Vec::new()
            }
        };

        let evaluation_order = pattern_index as i64 + 1;
        let mut any_existing_passed = false;
        for template in &templates {
            let evaluation = evaluate_template(template, setup, &candles);
            if evaluation.outcome == "pass" {
                any_existing_passed = true;
                existing_template_passes += 1;
            }
            pending_results.push(StoredTemplateResult::from_evaluation(
                template,
                setup,
                evaluation_order,
                false,
                &evaluation,
            ));
            result_rows += 1;
            if pending_results.len() >= RESULT_BATCH_SIZE {
                flush_result_batch(&pool, &run_id, &mut pending_results).await?;
            }
        }

        if !any_existing_passed {
            let next_sequence = templates.len() + 1;
            match synthesize_template(&run_id, next_sequence, setup, &candles) {
                Some(template) => {
                    let evaluation = evaluate_template(&template, setup, &candles);
                    insert_template(&pool, &template, setup, &evaluation, &run_id).await?;
                    pending_results.push(StoredTemplateResult::from_evaluation(
                        &template,
                        setup,
                        evaluation_order,
                        true,
                        &evaluation,
                    ));
                    result_rows += 1;
                    if pending_results.len() >= RESULT_BATCH_SIZE {
                        flush_result_batch(&pool, &run_id, &mut pending_results).await?;
                    }
                    println!(
                        "Created {} from pattern {}/{}: {} {} {} family={} result={:.2}R",
                        template.template_uid,
                        pattern_index + 1,
                        patterns.len(),
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
                        "No passing template could be synthesized for pattern {}/{}: {} {} {} family={}",
                        pattern_index + 1,
                        patterns.len(),
                        setup.symbol,
                        setup.market,
                        setup.d_confirm_date,
                        setup.pattern_family_key.as_deref().unwrap_or("N/A")
                    );
                }
            }
        }

        if (pattern_index + 1) % 25 == 0 || pattern_index + 1 == patterns.len() {
            println!(
                "Processed {}/{} patterns, generated templates={}, existing template passes={}",
                pattern_index + 1,
                patterns.len(),
                templates.len(),
                existing_template_passes
            );
        }
    }

    flush_result_batch(&pool, &run_id, &mut pending_results).await?;

    let elapsed_ms = started.elapsed().as_millis() as i64;
    insert_run_summary(
        &pool,
        &run_id,
        &args,
        patterns.len() as i64,
        templates.len() as i64,
        existing_template_passes,
        failed_to_create,
        result_rows,
        elapsed_ms,
    )
    .await?;

    let ui_stats_rows = refresh_entry_exit_template_ui_stats(&pool, &run_id).await?;
    let condition_stats_rows = refresh_entry_exit_template_condition_stats(&pool, &run_id).await?;

    println!();
    println!(
        "Stored creator run {run_id}: scanned={}, templates_created={}, existing_template_passes={}, failed_to_create={}, result_rows={}, ui_stats_rows={}, condition_stats_rows={}, elapsed={:.2}s",
        patterns.len(),
        templates.len(),
        existing_template_passes,
        failed_to_create,
        result_rows,
        ui_stats_rows,
        condition_stats_rows,
        elapsed_ms as f64 / 1000.0
    );
    print_template_leaderboard(&pool, &run_id).await?;

    Ok(())
}
