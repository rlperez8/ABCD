use std::collections::{HashMap, HashSet};
use std::env;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{mysql::MySqlPool, Row};

#[derive(Debug)]
struct Args {
    train_run_id: Option<String>,
    reuse_router_run_id: Option<String>,
    test_year: i64,
    source_scope: String,
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

#[derive(Default)]
struct RouterSummary {
    patterns_scanned: i64,
    routed_patterns: i64,
    no_route_patterns: i64,
    skipped_non_trade_patterns: i64,
    skipped_symbol_patterns: i64,
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
    "Usage: cargo run --bin run_entry_exit_family_router -- [--train-run-id RUN_ID | --reuse-router-run-id ROUTER_ID] [--test-year 2026] [--source futures] [--min-train-tests 25] [--limit 0] [--sister-window-minutes 15] [--prop-filter] [--symbol-filter] [--trade-min-tests 50] [--trade-min-win-rate 0.24] [--trade-min-avg-r 0.20] [--symbol-min-tests 1000] [--symbol-min-win-rate 0.23] [--symbol-min-avg-r 0.05]"
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
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        test_year: arg_value(&raw_args, "--test-year")
            .or_else(|| arg_value(&raw_args, "--year"))
            .as_deref()
            .unwrap_or("2026")
            .parse()?,
        source_scope: arg_value(&raw_args, "--source").unwrap_or_else(|| "futures".to_string()),
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

async fn ensure_tables(pool: &MySqlPool) -> Result<(), sqlx::Error> {
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

    for (column, definition) in [
        (
            "skipped_non_trade_patterns",
            "BIGINT NOT NULL DEFAULT 0 AFTER no_route_patterns",
        ),
        (
            "skipped_symbol_patterns",
            "BIGINT NOT NULL DEFAULT 0 AFTER skipped_non_trade_patterns",
        ),
        (
            "trade_choices",
            "BIGINT NOT NULL DEFAULT 0 AFTER skipped_symbol_patterns",
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
            "symbol_trade_roots",
            "BIGINT NOT NULL DEFAULT 0 AFTER symbol_filter_enabled",
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
            r.template_uid,
            COALESCE(NULLIF(r.pattern_family_key, ''), NULLIF(ps.pattern_family_key, ''), 'Unknown') AS family_key,
            COALESCE(NULLIF(ps.pattern_family_harmonic_type, ''), NULLIF(ps.harmonic_type, ''), 'Unknown') AS harmonic_type,
            COALESCE(NULLIF(ps.market, ''), NULLIF(r.market, ''), 'Unknown') AS market,
            COALESCE(NULLIF(ps.pattern_family_bin, ''), 'Unknown') AS family_bin,
            COALESCE(NULLIF(ps.pattern_family_size_bucket, ''), 'Unknown') AS family_size_bucket,
            COALESCE(NULLIF(ps.pattern_family_time_bin, ''), 'Unknown') AS family_time_bin,
            COALESCE(NULLIF(ps.pattern_family_x_strictness, ''), 'Unknown') AS family_x_strictness,
            CAST(COUNT(*) AS SIGNED) AS eval_count,
            CAST(SUM(CASE WHEN r.outcome = 'pass' THEN 1 ELSE 0 END) AS SIGNED) AS pass_count,
            CAST(SUM(CASE WHEN r.outcome = 'fail' THEN 1 ELSE 0 END) AS SIGNED) AS fail_count,
            CAST(SUM(CASE WHEN r.outcome = 'no_entry' THEN 1 ELSE 0 END) AS SIGNED) AS no_entry_count,
            COALESCE(AVG(COALESCE(r.result_r, 0)), 0) AS avg_r
        FROM entry_exit_template_results r
        LEFT JOIN pattern_setups ps
          ON ps.setup_id = r.setup_id
        WHERE r.run_id = ?
          AND COALESCE(NULLIF(r.pattern_family_key, ''), NULLIF(ps.pattern_family_key, ''), '') <> ''
        GROUP BY
            r.template_uid,
            COALESCE(NULLIF(r.pattern_family_key, ''), NULLIF(ps.pattern_family_key, ''), 'Unknown'),
            COALESCE(NULLIF(ps.pattern_family_harmonic_type, ''), NULLIF(ps.harmonic_type, ''), 'Unknown'),
            COALESCE(NULLIF(ps.market, ''), NULLIF(r.market, ''), 'Unknown'),
            COALESCE(NULLIF(ps.pattern_family_bin, ''), 'Unknown'),
            COALESCE(NULLIF(ps.pattern_family_size_bucket, ''), 'Unknown'),
            COALESCE(NULLIF(ps.pattern_family_time_bin, ''), 'Unknown'),
            COALESCE(NULLIF(ps.pattern_family_x_strictness, ''), 'Unknown')
        HAVING eval_count >= ?
           AND pass_count > 0
           AND avg_r > 0
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

fn setup_root_symbol(setup: &PatternSetup) -> &str {
    setup
        .root_symbol
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&setup.symbol)
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
          AND YEAR(COALESCE(ps.d_confirm_date, ps.d_date)) = ?
          {source_filter}
        ORDER BY COALESCE(ps.d_confirm_date, ps.d_date) ASC, ps.setup_id ASC
        {limit_clause}
        "#,
        source_filter = source_filter,
        limit_clause = limit_clause,
    );

    let mut query = sqlx::query_as::<_, PatternSetup>(&sql).bind(args.test_year);
    if let Some(raw_candidate_limit) = raw_candidate_limit {
        query = query.bind(raw_candidate_limit);
    }
    let rows = query.fetch_all(pool).await?;
    Ok(dedupe_sister_patterns(
        rows,
        (args.limit > 0).then_some(args.limit as usize),
        args.sister_window_minutes,
    ))
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
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(router_run_id)
    .bind(&setup.setup_id)
    .bind(&setup.pattern_id)
    .bind(&setup.pattern_group_id)
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
            test_year,
            min_train_tests,
            sister_window_minutes,
            families_selected,
            patterns_scanned,
            routed_patterns,
            no_route_patterns,
            skipped_non_trade_patterns,
            skipped_symbol_patterns,
            trade_choices,
            watchlist_choices,
            skip_choices,
            symbol_filter_enabled,
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
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(router_run_id)
    .bind(train_run_id)
    .bind(&args.source_scope)
    .bind(args.test_year)
    .bind(args.min_train_tests)
    .bind(args.sister_window_minutes)
    .bind(families_selected)
    .bind(summary.patterns_scanned)
    .bind(summary.routed_patterns)
    .bind(summary.no_route_patterns)
    .bind(summary.skipped_non_trade_patterns)
    .bind(summary.skipped_symbol_patterns)
    .bind(trade_choices)
    .bind(watchlist_choices)
    .bind(skip_choices)
    .bind(if args.symbol_filter { 1 } else { 0 })
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let args = parse_args()?;
    let started = Instant::now();
    let pool = MySqlPool::connect(&database_url_from_env()?).await?;
    ensure_tables(&pool).await?;

    let train_run_id = match args.reuse_router_run_id.as_deref() {
        Some(reuse_router_run_id) => reused_router_train_run_id(&pool, reuse_router_run_id).await?,
        None => match args.train_run_id.clone() {
            Some(run_id) => run_id,
            None => latest_train_run_id(&pool).await?,
        },
    };
    let router_run_id = router_run_id();

    println!("Building family router");
    println!("router_run_id={router_run_id}");
    println!(
        "train_run_id={train_run_id} test_year={} source={} min_train_tests={}",
        args.test_year, args.source_scope, args.min_train_tests
    );
    if let Some(reuse_router_run_id) = args.reuse_router_run_id.as_deref() {
        println!("reuse_router_run_id={reuse_router_run_id}");
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

    let (choices, template_count) = match args.reuse_router_run_id.as_deref() {
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
            let choices = build_router_choices(&pool, &train_run_id, &templates, &args).await?;
            (choices, template_count)
        }
    };
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
            "symbol gate roots trade={} skip={} skipped={}",
            symbol_trade_roots,
            symbol_skip_roots,
            skipped_roots.join(", ")
        );
    }

    let patterns = fetch_test_patterns(&pool, &args).await?;
    println!(
        "loaded {} deduped {} patterns",
        patterns.len(),
        args.test_year
    );

    let mut summary = RouterSummary::default();
    summary.patterns_scanned = patterns.len() as i64;
    for (index, setup) in patterns.iter().enumerate() {
        let Some(family_key) = setup.pattern_family_key.as_deref() else {
            summary.no_route_patterns += 1;
            continue;
        };
        let Some(choice) = choices.get(family_key) else {
            summary.no_route_patterns += 1;
            continue;
        };
        if choice.route_status != "TRADE" {
            summary.skipped_non_trade_patterns += 1;
            continue;
        }
        if args.symbol_filter {
            let root_symbol = setup_gate_root_symbol(setup);
            match symbol_choices.get(&root_symbol) {
                Some(symbol_choice) if symbol_choice.route_status == "TRADE" => {}
                _ => {
                    summary.skipped_symbol_patterns += 1;
                    continue;
                }
            }
        }

        let candles = fetch_forward_candles(&pool, setup).await?;
        let evaluation = evaluate_template(&choice.template, setup, &candles);
        summary.record(&evaluation);
        insert_result(&pool, &router_run_id, setup, choice, &evaluation).await?;

        if (index + 1) % 500 == 0 {
            println!(
                "processed {}/{} routed={} wins={} losses={} no_entry={} avg_r={:.3}",
                index + 1,
                patterns.len(),
                summary.routed_patterns,
                summary.win_count,
                summary.loss_count,
                summary.no_entry_count,
                summary.avg_r()
            );
        }
    }

    let elapsed_ms = started.elapsed().as_millis() as i64;
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

    let win_rate = if summary.routed_patterns > 0 {
        summary.win_count as f64 / summary.routed_patterns as f64 * 100.0
    } else {
        0.0
    };
    println!();
    println!("Family router {} result", args.test_year);
    println!("router_run_id={router_run_id}");
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
    println!("wins={}", summary.win_count);
    println!("losses={}", summary.loss_count);
    println!("no_entry={}", summary.no_entry_count);
    println!("win_rate={win_rate:.2}%");
    println!("avg_r={:.4}R", summary.avg_r());
    println!("sum_r={:.2}R", summary.sum_r);
    println!("best_r={:.2}R", summary.best_r);
    println!("worst_r={:.2}R", summary.worst_r);
    println!("elapsed={:.2}s", elapsed_ms as f64 / 1000.0);

    Ok(())
}
