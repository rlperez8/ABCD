use std::collections::{HashMap, HashSet};
use std::env;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use sqlx::{mysql::MySqlPool, Row};

#[derive(Debug)]
struct Args {
    build_id: Option<String>,
    source_scope: String,
    source_timeframe: Option<String>,
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
    ignore_manual_family_skips: bool,
}

#[derive(Clone)]
struct TemplateSpec {
    template_uid: String,
    template_label: String,
    template_name: String,
    entry_kind: String,
    entry_offset: i64,
    direction_mode: String,
    risk_basis: String,
    risk_multiple: f64,
    target_r: f64,
}

#[derive(Clone)]
struct PlaybookChoice {
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

fn usage() -> &'static str {
    "Usage: cargo run --bin create_entry_exit_playbook -- --build-id BUILD_ID [--source futures] [--source-timeframe 1m|5m] [--min-train-tests 25] [--prop-filter] [--symbol-filter] [--trade-min-tests 50] [--trade-min-win-rate 0.24] [--trade-min-avg-r 0.20] [--symbol-min-tests 1000] [--symbol-min-win-rate 0.23] [--symbol-min-avg-r 0.05] [--one-trade-at-a-time] [--one-trade-per-root-symbol] [--trade-cooldown-minutes 5] [--daily-loss-lockout] [--near-pass-protection] [--near-pass-within-r 10] [--near-pass-daily-loss-r 3] [--loss-cluster-day-lockout] [--loss-cluster-loss-count 3] [--loss-cluster-window-minutes 60] [--ignore-manual-family-skips]"
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|items| items[0] == name)
        .map(|items| items[1].trim().to_string())
        .filter(|value| !value.is_empty())
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| arg == name)
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let raw_args = env::args().skip(1).collect::<Vec<_>>();
    if raw_args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", usage());
        std::process::exit(0);
    }

    Ok(Args {
        build_id: arg_value(&raw_args, "--build-id")
            .or_else(|| arg_value(&raw_args, "--train-run-id"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        source_scope: arg_value(&raw_args, "--source").unwrap_or_else(|| "futures".to_string()),
        source_timeframe: arg_value(&raw_args, "--source-timeframe")
            .or_else(|| arg_value(&raw_args, "--timeframe"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("all")),
        min_train_tests: arg_value(&raw_args, "--min-train-tests")
            .as_deref()
            .unwrap_or("25")
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
        ignore_manual_family_skips: has_flag(&raw_args, "--ignore-manual-family-skips"),
    })
}

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

fn playbook_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let pid = std::process::id();
    format!("eepb-{millis}-{pid}")
}

fn template_risk_label(template: &TemplateSpec) -> &'static str {
    if template.risk_basis == "xa_price_length" {
        "XA"
    } else if template.risk_basis == "pullback_retest_extreme" {
        "PB"
    } else {
        "CD"
    }
}

fn build_playbook_description(
    build_id: &str,
    args: &Args,
    families_selected: i64,
    trade_choices: i64,
    watchlist_choices: i64,
    skip_choices: i64,
    symbol_trade_roots: i64,
    symbol_skip_roots: i64,
) -> String {
    let mut parts = Vec::new();
    parts.push(format!(
        "Built from entry/exit build {build_id} on {} {} data. The playbook keeps the reusable Entry/Exit Templates from the build and assigns the best qualifying template to each harmonic family.",
        args.source_scope,
        args.source_timeframe.as_deref().unwrap_or("all-timeframe")
    ));
    parts.push(format!(
        "Family selection produced {families_selected} family rows: {trade_choices} trade, {watchlist_choices} watchlist, and {skip_choices} skip."
    ));

    if args.prop_filter {
        parts.push(format!(
            "Prop filter is on. A family/template can trade when it has at least {} training tests, {:.1}% win rate, and {:.3}R average. Watchlist rows use at least {} tests, {:.1}% win rate, and {:.3}R average.",
            args.trade_min_tests,
            args.trade_min_win_rate * 100.0,
            args.trade_min_avg_r,
            args.watchlist_min_tests,
            args.watchlist_min_win_rate * 100.0,
            args.watchlist_min_avg_r
        ));
    } else {
        parts.push("Prop filter is off, so family/template rows are selected without the prop trade/watchlist thresholds.".to_string());
    }

    if args.symbol_filter {
        parts.push(format!(
            "Symbol gate is on. Root symbols need at least {} tests, {:.1}% win rate, and {:.3}R average to be tradable. This playbook allows {symbol_trade_roots} roots and skips {symbol_skip_roots}.",
            args.symbol_min_tests,
            args.symbol_min_win_rate * 100.0,
            args.symbol_min_avg_r
        ));
    } else {
        parts.push("Symbol gate is off, so tradable families can fire on any root symbol in the tested source.".to_string());
    }

    let mut execution_rules = Vec::new();
    if args.one_trade_at_a_time {
        execution_rules
            .push("only one active trade can be open across the whole account".to_string());
    } else if args.one_trade_per_root_symbol {
        execution_rules.push("only one active trade can be open per root symbol".to_string());
    }
    if args.one_trade_per_minute {
        execution_rules.push("only one entry is allowed per minute".to_string());
    }
    if args.trade_cooldown_minutes > 0 {
        execution_rules.push(format!(
            "after an accepted win/loss trade, the next accepted entry must be at least {} rolling minutes later",
            args.trade_cooldown_minutes
        ));
    }
    if args.daily_loss_lockout {
        execution_rules.push("if the day reaches -10R, the playbook stops taking trades for the rest of that trade date instead of marking the prop cycle as a daily-loss failure".to_string());
    }
    if args.near_pass_protection {
        execution_rules.push(format!(
            "near-pass protection turns on once the cycle is within {:.0}R of the +30R target, then tightens the daily lockout to -{:.0}R",
            args.near_pass_within_r, args.near_pass_daily_loss_r
        ));
    }
    if args.loss_cluster_day_lockout {
        execution_rules.push(format!(
            "loss cluster guard stops trading for the rest of the date after {} losses occur inside a rolling {} minute window",
            args.loss_cluster_loss_count, args.loss_cluster_window_minutes
        ));
    }

    if execution_rules.is_empty() {
        parts.push("Execution has no overlap, cooldown, daily lockout, near-pass, or loss-cluster guard enabled.".to_string());
    } else {
        parts.push(format!("Execution rules: {}.", execution_rules.join("; ")));
    }

    parts.push("This playbook does not change the underlying entry/exit templates; it changes which family/template plays are allowed and how the replay gates control trade timing and prop-firm risk.".to_string());
    parts.join("\n\n")
}

async fn latest_build_id(pool: &MySqlPool) -> Result<String, sqlx::Error> {
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
            source_timeframe VARCHAR(16) NULL,
            test_year BIGINT NOT NULL,
            min_train_tests BIGINT NOT NULL,
            sister_window_minutes BIGINT NOT NULL,
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
        ("source_timeframe", "VARCHAR(16) NULL AFTER source_scope"),
        (
            "skipped_overlap_patterns",
            "BIGINT NOT NULL DEFAULT 0 AFTER skipped_symbol_patterns",
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
            "prop_filter_enabled",
            "TINYINT NOT NULL DEFAULT 0 AFTER symbol_min_avg_r",
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

    Ok(())
}

async fn family_condition_stat_count(pool: &MySqlPool, build_id: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM entry_exit_template_condition_stats
        WHERE run_id = ?
          AND condition_type = 'pattern_family_key'
        "#,
    )
    .bind(build_id)
    .fetch_one(pool)
    .await
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
    if template.entry_kind == "pullback_retest_d" {
        return format!("{direction} PB-D {:.0}R", template.target_r);
    }
    format!(
        "{direction} C+{} {:.3}{} {:.0}R",
        template.entry_offset,
        template.risk_multiple,
        template_risk_label(template),
        template.target_r
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
                "train {} tests | {:.1}% WR | {:.3}R avg | {:.1}% fail",
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
                "below trade bar: {} tests | {:.1}% WR | {:.3}R avg",
                eval_count,
                win_rate * 100.0,
                avg_r
            ),
        );
    }

    (
        "SKIP".to_string(),
        format!(
            "weak training: {} tests | {:.1}% WR | {:.3}R avg",
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

fn route_status_counts(choices: &HashMap<String, PlaybookChoice>) -> (i64, i64, i64) {
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
    choices: &mut HashMap<String, PlaybookChoice>,
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

async fn load_templates(
    pool: &MySqlPool,
    build_id: &str,
) -> Result<HashMap<String, TemplateSpec>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            t.template_uid,
            t.template_name,
            t.entry_kind,
            t.direction_mode,
            t.risk_basis,
            t.risk_multiple,
            t.target_r,
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
    .bind(build_id)
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
            entry_kind: row.try_get("entry_kind")?,
            entry_offset: parse_entry_offset(&rule_json),
            direction_mode: row.try_get("direction_mode")?,
            risk_basis: row.try_get("risk_basis")?,
            risk_multiple: row.try_get("risk_multiple")?,
            target_r: row.try_get("target_r")?,
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

async fn build_playbook_choices(
    pool: &MySqlPool,
    build_id: &str,
    templates: &HashMap<String, TemplateSpec>,
    args: &Args,
) -> Result<HashMap<String, PlaybookChoice>, sqlx::Error> {
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
    .bind(build_id)
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
        let choice = PlaybookChoice {
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
            .and_modify(|current: &mut PlaybookChoice| {
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

fn normalize_root_symbol(value: &str) -> String {
    value.trim().to_uppercase()
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
                "symbol train {} tests | {:.1}% WR | {:.3}R avg | {:.1}% fail",
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
            "weak symbol training: {} tests | {:.1}% WR | {:.3}R avg",
            eval_count,
            win_rate * 100.0,
            avg_r
        ),
    )
}

async fn build_symbol_gate_choices(
    pool: &MySqlPool,
    build_id: &str,
    choices: &HashMap<String, PlaybookChoice>,
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
        .bind(build_id)
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

async fn insert_choices(
    pool: &MySqlPool,
    playbook_id: &str,
    build_id: &str,
    choices: &HashMap<String, PlaybookChoice>,
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
        .bind(playbook_id)
        .bind(build_id)
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
    playbook_id: &str,
    build_id: &str,
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
        .bind(playbook_id)
        .bind(build_id)
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

async fn insert_playbook_summary(
    pool: &MySqlPool,
    playbook_id: &str,
    build_id: &str,
    args: &Args,
    families_selected: i64,
    trade_choices: i64,
    watchlist_choices: i64,
    skip_choices: i64,
    symbol_trade_roots: i64,
    symbol_skip_roots: i64,
    elapsed_ms: i64,
) -> Result<(), sqlx::Error> {
    let playbook_description = build_playbook_description(
        build_id,
        args,
        families_selected,
        trade_choices,
        watchlist_choices,
        skip_choices,
        symbol_trade_roots,
        symbol_skip_roots,
    );

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
        VALUES (?, ?, ?, ?, 0, ?, 0, ?, 0, 0, 0, 0, 0, 0, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 0, 0, 0, 0, 0, 0, ?)
        "#,
    )
    .bind(playbook_id)
    .bind(build_id)
    .bind(&args.source_scope)
    .bind(args.source_timeframe.as_deref())
    .bind(args.min_train_tests)
    .bind(families_selected)
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
    .bind(playbook_description)
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

    let build_id = match args.build_id.clone() {
        Some(build_id) => build_id,
        None => latest_build_id(&pool).await?,
    };

    println!("Creating entry/exit playbook");
    println!(
        "build_id={build_id} source={} timeframe={} min_train_tests={}",
        args.source_scope,
        args.source_timeframe.as_deref().unwrap_or("all"),
        args.min_train_tests
    );
    if args.prop_filter {
        println!(
            "prop_filter=on trade_bar={} tests | {:.1}% WR | {:.3}R avg watchlist_bar={} tests | {:.1}% WR | {:.3}R avg",
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
            "symbol_filter=on bar={} tests | {:.1}% WR | {:.3}R avg",
            args.symbol_min_tests,
            args.symbol_min_win_rate * 100.0,
            args.symbol_min_avg_r
        );
    }
    if args.one_trade_at_a_time {
        println!("execution_rule=one_trade_at_a_time");
    }
    if args.one_trade_per_root_symbol {
        println!("execution_rule=one_trade_per_root_symbol");
    }
    if args.one_trade_per_minute {
        println!("execution_rule=one_trade_per_minute");
    }
    if args.trade_cooldown_minutes > 0 {
        println!(
            "execution_rule=trade_cooldown_minutes:{}",
            args.trade_cooldown_minutes
        );
    }
    if args.daily_loss_lockout {
        println!("execution_rule=daily_loss_lockout");
    }
    if args.near_pass_protection {
        println!(
            "execution_rule=near_pass_protection within={:.2}R daily_loss={:.2}R",
            args.near_pass_within_r, args.near_pass_daily_loss_r
        );
    }
    if args.loss_cluster_day_lockout {
        println!(
            "execution_rule=loss_cluster_day_lockout losses={} window_minutes={}",
            args.loss_cluster_loss_count, args.loss_cluster_window_minutes
        );
    }

    let templates = load_templates(&pool, &build_id).await?;
    let template_count = templates.len();
    if template_count == 0 {
        return Err(format!("Build {build_id} has no entry/exit templates.").into());
    }

    let family_stats = family_condition_stat_count(&pool, &build_id).await?;
    if family_stats == 0 {
        return Err(format!(
            "Build {build_id} has no pattern_family_key condition stats. Rerun create_entry_exit_templates with the fixed builder before creating a playbook."
        )
        .into());
    }
    println!("family_condition_stats={family_stats}");

    let mut choices = build_playbook_choices(&pool, &build_id, &templates, &args).await?;
    if choices.is_empty() {
        return Err(format!(
            "No family -> entry/exit mappings were selected from build {build_id}. Try lower thresholds or inspect the family condition stats."
        )
        .into());
    }

    let playbook_id = playbook_id();
    println!("playbook_id={playbook_id}");

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
    let symbol_choices = build_symbol_gate_choices(&pool, &build_id, &choices, &args).await?;
    let (symbol_trade_roots, symbol_skip_roots) = symbol_gate_counts(&symbol_choices);

    insert_choices(&pool, &playbook_id, &build_id, &choices).await?;
    insert_symbol_gate_choices(&pool, &playbook_id, &build_id, &symbol_choices).await?;
    let elapsed_ms = started.elapsed().as_millis() as i64;
    insert_playbook_summary(
        &pool,
        &playbook_id,
        &build_id,
        &args,
        choices.len() as i64,
        trade_choices,
        watchlist_choices,
        skip_choices,
        symbol_trade_roots,
        symbol_skip_roots,
        elapsed_ms,
    )
    .await?;

    println!();
    println!("Entry/Exit playbook created");
    println!("playbook_id={playbook_id}");
    println!("build_id={build_id}");
    println!("templates_available={template_count}");
    println!("families_selected={}", choices.len());
    println!("trade_choices={trade_choices}");
    println!("watchlist_choices={watchlist_choices}");
    println!("skip_choices={skip_choices}");
    println!("symbol_trade_roots={symbol_trade_roots}");
    println!("symbol_skip_roots={symbol_skip_roots}");
    println!("elapsed={:.2}s", elapsed_ms as f64 / 1000.0);

    Ok(())
}
