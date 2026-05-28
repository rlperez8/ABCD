use std::collections::BTreeMap;
use std::env;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use chrono::NaiveDateTime;
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::{MySql, QueryBuilder};

const INSERT_BATCH_SIZE: usize = 1_000;

#[derive(Debug)]
struct Args {
    source_scope: String,
    source_timeframe: Option<String>,
    start_year: i32,
    end_year: i32,
    limit: i64,
    max_forward_multiple: i64,
    xa_multiple: f64,
    direction_mode: String,
}

#[derive(Clone, sqlx::FromRow)]
struct PatternSetup {
    setup_id: String,
    pattern_id: Option<String>,
    pattern_group_id: String,
    event_id: Option<String>,
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
    xa_price_length: f64,
    full_pattern_length: i64,
}

#[derive(Clone, sqlx::FromRow)]
struct ForwardCandle {
    candle_date: NaiveDateTime,
    open: f64,
    high: f64,
    low: f64,
}

struct OutcomeRow {
    setup: PatternSetup,
    outcome: String,
    entry_date: Option<NaiveDateTime>,
    entry_price: Option<f64>,
    hit_date: Option<NaiveDateTime>,
    bars_to_hit: Option<i64>,
    minutes_to_hit: Option<i64>,
    xa_distance: f64,
    reversal_target_price: f64,
    continuation_target_price: f64,
    max_reversal_excursion: f64,
    max_continuation_excursion: f64,
    candles_scanned: i64,
}

fn usage() -> &'static str {
    "Usage: cargo run --bin build_pattern_xa_outcomes -- [--source futures|daily|all] [--timeframe 1m|5m|daily] [--year YYYY | --start-year YYYY --end-year YYYY] [--limit N] [--max-forward-multiple N] [--xa-multiple 1.0] [--direction-mode inverse_pattern|with_pattern]\nLabels each pattern as a C+1 open XA trade: entry is the first open after confirmation, TP is 1x XA in the selected direction from entry, SL is 1x XA against it. Use --max-forward-multiple 0 for an unlimited forward scan."
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|items| items[0] == name)
        .map(|items| items[1].trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_source(value: Option<String>) -> String {
    match value
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("futures") => "futures".to_string(),
        Some("daily") => "daily".to_string(),
        _ => "all".to_string(),
    }
}

fn normalize_timeframe(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_ascii_lowercase())
        .filter(|item| !item.is_empty() && item != "all")
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let raw_args = env::args().skip(1).collect::<Vec<_>>();
    if raw_args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", usage());
        std::process::exit(0);
    }

    let single_year = arg_value(&raw_args, "--year")
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(2026);
    let start_year = arg_value(&raw_args, "--start-year")
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(single_year);
    let end_year = arg_value(&raw_args, "--end-year")
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(start_year);

    Ok(Args {
        source_scope: normalize_source(arg_value(&raw_args, "--source")),
        source_timeframe: normalize_timeframe(arg_value(&raw_args, "--timeframe")),
        start_year: start_year.min(end_year),
        end_year: start_year.max(end_year),
        limit: arg_value(&raw_args, "--limit")
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0)
            .max(0),
        max_forward_multiple: arg_value(&raw_args, "--max-forward-multiple")
            .or_else(|| arg_value(&raw_args, "--max-hold-multiple"))
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(5)
            .clamp(0, 100),
        xa_multiple: arg_value(&raw_args, "--xa-multiple")
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(1.0),
        direction_mode: arg_value(&raw_args, "--direction-mode")
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| value == "with_pattern" || value == "inverse_pattern")
            .unwrap_or_else(|| "inverse_pattern".to_string()),
    })
}

fn outcome_mode(args: &Args) -> String {
    format!("c1_open_xa_trade_{}", args.direction_mode)
}

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

fn new_run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let pid = std::process::id();
    format!("pxao-{millis}-{pid}")
}

fn year_label(args: &Args) -> String {
    if args.start_year == args.end_year {
        args.start_year.to_string()
    } else {
        format!("{}-{}", args.start_year, args.end_year)
    }
}

async fn ensure_tables(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS pattern_xa_outcome_runs (
            run_id VARCHAR(64) NOT NULL PRIMARY KEY,
            source_scope VARCHAR(32) NOT NULL,
            source_timeframe VARCHAR(32) NULL,
            scan_year_start INT NOT NULL,
            scan_year_end INT NOT NULL,
            scan_year_label VARCHAR(32) NOT NULL,
            requested_limit BIGINT NOT NULL DEFAULT 0,
            max_forward_multiple BIGINT NOT NULL DEFAULT 5,
            xa_multiple DOUBLE NOT NULL DEFAULT 1,
            outcome_mode VARCHAR(64) NOT NULL DEFAULT 'c1_open_xa_trade',
            patterns_scanned BIGINT NOT NULL DEFAULT 0,
            reversal_count BIGINT NOT NULL DEFAULT 0,
            continuation_count BIGINT NOT NULL DEFAULT 0,
            ambiguous_count BIGINT NOT NULL DEFAULT 0,
            none_count BIGINT NOT NULL DEFAULT 0,
            elapsed_ms BIGINT NOT NULL DEFAULT 0,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            INDEX idx_pattern_xa_outcome_runs_created (created_at)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS pattern_xa_outcomes (
            run_id VARCHAR(64) NOT NULL,
            setup_id VARCHAR(64) NOT NULL,
            pattern_id VARCHAR(128) NULL,
            pattern_group_id VARCHAR(128) NOT NULL,
            event_id VARCHAR(128) NULL,
            symbol VARCHAR(32) NOT NULL,
            root_symbol VARCHAR(32) NULL,
            contract_symbol VARCHAR(32) NULL,
            source_table VARCHAR(128) NULL,
            source_timeframe VARCHAR(32) NULL,
            market VARCHAR(16) NOT NULL,
            pattern_family_key VARCHAR(64) NULL,
            d_date DATETIME NOT NULL,
            d_confirm_date DATETIME NOT NULL,
            d_price DOUBLE NOT NULL,
            entry_date DATETIME NULL,
            entry_price DOUBLE NULL,
            outcome_mode VARCHAR(64) NOT NULL DEFAULT 'c1_open_xa_trade',
            xa_distance DOUBLE NOT NULL,
            xa_multiple DOUBLE NOT NULL DEFAULT 1,
            reversal_target_price DOUBLE NOT NULL,
            continuation_target_price DOUBLE NOT NULL,
            outcome VARCHAR(32) NOT NULL,
            hit_date DATETIME NULL,
            bars_to_hit BIGINT NULL,
            minutes_to_hit BIGINT NULL,
            max_reversal_excursion DOUBLE NOT NULL DEFAULT 0,
            max_continuation_excursion DOUBLE NOT NULL DEFAULT 0,
            candles_scanned BIGINT NOT NULL DEFAULT 0,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (run_id, setup_id),
            INDEX idx_pattern_xa_outcomes_outcome (run_id, outcome),
            INDEX idx_pattern_xa_outcomes_family (run_id, pattern_family_key, outcome),
            INDEX idx_pattern_xa_outcomes_symbol (run_id, root_symbol, outcome),
            INDEX idx_pattern_xa_outcomes_confirm (run_id, d_confirm_date)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS pattern_xa_outcome_families (
            run_id VARCHAR(64) NOT NULL,
            pattern_family_key VARCHAR(128) NOT NULL,
            harmonic_type VARCHAR(32) NOT NULL DEFAULT 'Unknown',
            family_bin VARCHAR(32) NOT NULL DEFAULT 'Unknown',
            family_size_bucket VARCHAR(32) NOT NULL DEFAULT 'Unknown',
            family_time_bin VARCHAR(32) NOT NULL DEFAULT 'Unknown',
            family_x_strictness VARCHAR(32) NOT NULL DEFAULT 'Unknown',
            total_count BIGINT NOT NULL DEFAULT 0,
            reversal_count BIGINT NOT NULL DEFAULT 0,
            continuation_count BIGINT NOT NULL DEFAULT 0,
            ambiguous_count BIGINT NOT NULL DEFAULT 0,
            none_count BIGINT NOT NULL DEFAULT 0,
            reversal_rate DOUBLE NOT NULL DEFAULT 0,
            continuation_rate DOUBLE NOT NULL DEFAULT 0,
            avg_bars_to_hit DOUBLE NULL,
            avg_minutes_to_hit DOUBLE NULL,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (run_id, pattern_family_key),
            INDEX idx_pattern_xa_outcome_families_total (run_id, total_count),
            INDEX idx_pattern_xa_outcome_families_reversal (run_id, reversal_rate)
        )
        "#,
    )
    .execute(pool)
    .await?;

    ensure_column(
        pool,
        "pattern_xa_outcome_runs",
        "outcome_mode",
        "VARCHAR(64) NOT NULL DEFAULT 'c1_open_xa_trade'",
    )
    .await?;
    ensure_column(pool, "pattern_xa_outcomes", "entry_date", "DATETIME NULL").await?;
    ensure_column(pool, "pattern_xa_outcomes", "entry_price", "DOUBLE NULL").await?;
    ensure_column(
        pool,
        "pattern_xa_outcomes",
        "outcome_mode",
        "VARCHAR(64) NOT NULL DEFAULT 'c1_open_xa_trade'",
    )
    .await?;

    ensure_column(
        pool,
        "pattern_xa_outcome_families",
        "harmonic_type",
        "VARCHAR(32) NOT NULL DEFAULT 'Unknown'",
    )
    .await?;
    ensure_column(
        pool,
        "pattern_xa_outcome_families",
        "family_bin",
        "VARCHAR(32) NOT NULL DEFAULT 'Unknown'",
    )
    .await?;
    ensure_column(
        pool,
        "pattern_xa_outcome_families",
        "family_size_bucket",
        "VARCHAR(32) NOT NULL DEFAULT 'Unknown'",
    )
    .await?;
    ensure_column(
        pool,
        "pattern_xa_outcome_families",
        "family_time_bin",
        "VARCHAR(32) NOT NULL DEFAULT 'Unknown'",
    )
    .await?;
    ensure_column(
        pool,
        "pattern_xa_outcome_families",
        "family_x_strictness",
        "VARCHAR(32) NOT NULL DEFAULT 'Unknown'",
    )
    .await?;

    Ok(())
}

async fn ensure_column(
    pool: &MySqlPool,
    table_name: &str,
    column_name: &str,
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
    .bind(table_name)
    .bind(column_name)
    .fetch_one(pool)
    .await?
        > 0;

    if !exists {
        let statement = format!("ALTER TABLE {table_name} ADD COLUMN {column_name} {definition}");
        sqlx::query(&statement).execute(pool).await?;
    }

    Ok(())
}

async fn fetch_patterns(pool: &MySqlPool, args: &Args) -> Result<Vec<PatternSetup>, sqlx::Error> {
    let source_filter = match args.source_scope.as_str() {
        "futures" => "AND COALESCE(ps.source_table, '') LIKE 'futures_contract_%_candles'",
        "daily" => "AND COALESCE(ps.source_table, '') = 'candles' AND COALESCE(ps.source_timeframe, '') = 'daily'",
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
    let limit_clause = if args.limit > 0 { "LIMIT ?" } else { "" };
    let sql = format!(
        r#"
        SELECT
            ps.setup_id,
            ps.pattern_id,
            ps.pattern_group_id,
            ps.event_id,
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
            COALESCE(ps.xa_price_length, ABS(ps.a_min_max - ps.x_min_max), 0) AS xa_price_length,
            CAST(ps.full_pattern_length AS SIGNED) AS full_pattern_length
        FROM pattern_setups ps
        WHERE ps.d_date IS NOT NULL
          AND ps.full_pattern_length > 0
          AND COALESCE(ps.xa_price_length, ABS(ps.a_min_max - ps.x_min_max), 0) > 0
          {source_filter}
          {year_filter}
          {timeframe_filter}
        ORDER BY COALESCE(ps.d_confirm_date, ps.d_date) ASC, ps.setup_id ASC
        {limit_clause}
        "#
    );

    let mut query = sqlx::query_as::<_, PatternSetup>(&sql);
    if args.start_year == args.end_year {
        query = query.bind(args.start_year);
    } else {
        query = query.bind(args.start_year).bind(args.end_year);
    }
    if let Some(timeframe) = &args.source_timeframe {
        query = query.bind(timeframe);
    }
    if args.limit > 0 {
        query = query.bind(args.limit);
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
    max_forward_multiple: i64,
) -> Result<Vec<ForwardCandle>, sqlx::Error> {
    let max_forward_bars = if max_forward_multiple > 0 {
        Some(
            setup
                .full_pattern_length
                .saturating_mul(max_forward_multiple)
                .max(1),
        )
    } else {
        None
    };
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
        if let Some(limit) = max_forward_bars {
            return sqlx::query_as::<_, ForwardCandle>(
                r#"
            SELECT
                CAST(date AS DATETIME) AS candle_date,
                CAST(open AS DOUBLE) AS open,
                CAST(high AS DOUBLE) AS high,
                CAST(low AS DOUBLE) AS low
            FROM candles
            WHERE symbol = ?
              AND date >= ?
            ORDER BY date ASC
            LIMIT ?
            "#,
            )
            .bind(&setup.symbol)
            .bind(setup.d_confirm_date.date())
            .bind(limit.saturating_add(1))
            .fetch_all(pool)
            .await;
        }

        return sqlx::query_as::<_, ForwardCandle>(
            r#"
            SELECT
                CAST(date AS DATETIME) AS candle_date,
                CAST(open AS DOUBLE) AS open,
                CAST(high AS DOUBLE) AS high,
                CAST(low AS DOUBLE) AS low
            FROM candles
            WHERE symbol = ?
              AND date >= ?
            ORDER BY date ASC
            "#,
        )
        .bind(&setup.symbol)
        .bind(setup.d_confirm_date.date())
        .fetch_all(pool)
        .await;
    }

    let candle_table = futures_candle_table(setup.source_table.as_deref());
    if let Some(limit) = max_forward_bars {
        let sql = format!(
            r#"
        SELECT
            ts_utc AS candle_date,
            CAST(open AS DOUBLE) AS open,
            CAST(high AS DOUBLE) AS high,
            CAST(low AS DOUBLE) AS low
        FROM {candle_table}
        WHERE symbol = ?
          AND ts_utc >= ?
        ORDER BY ts_utc ASC
        LIMIT ?
        "#
        );

        return sqlx::query_as::<_, ForwardCandle>(&sql)
            .bind(&setup.symbol)
            .bind(setup.d_confirm_date)
            .bind(limit.saturating_add(1))
            .fetch_all(pool)
            .await;
    }

    let sql = format!(
        r#"
        SELECT
            ts_utc AS candle_date,
            CAST(open AS DOUBLE) AS open,
            CAST(high AS DOUBLE) AS high,
            CAST(low AS DOUBLE) AS low
        FROM {candle_table}
        WHERE symbol = ?
          AND ts_utc >= ?
        ORDER BY ts_utc ASC
        "#
    );
    sqlx::query_as::<_, ForwardCandle>(&sql)
        .bind(&setup.symbol)
        .bind(setup.d_confirm_date)
        .fetch_all(pool)
        .await
}

async fn fetch_unlimited_forward_candles_from(
    pool: &MySqlPool,
    setup: &PatternSetup,
    from_date: NaiveDateTime,
) -> Result<Vec<ForwardCandle>, sqlx::Error> {
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
                CAST(low AS DOUBLE) AS low
            FROM candles
            WHERE symbol = ?
              AND date >= ?
            ORDER BY date ASC
            "#,
        )
        .bind(&setup.symbol)
        .bind(from_date.date())
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
            CAST(low AS DOUBLE) AS low
        FROM {candle_table}
        WHERE symbol = ?
          AND ts_utc >= ?
        ORDER BY ts_utc ASC
        "#
    );
    sqlx::query_as::<_, ForwardCandle>(&sql)
        .bind(&setup.symbol)
        .bind(from_date)
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

fn trade_direction(setup: &PatternSetup, args: &Args) -> f64 {
    let base = setup_direction(setup);
    if args.direction_mode == "inverse_pattern" {
        -base
    } else {
        base
    }
}

fn forward_start_index(setup: &PatternSetup, candles: &[ForwardCandle]) -> usize {
    candles
        .iter()
        .position(|candle| candle.candle_date >= setup.d_confirm_date)
        .map(|index| {
            if candles[index].candle_date <= setup.d_confirm_date {
                index.saturating_add(1)
            } else {
                index
            }
        })
        .unwrap_or(candles.len())
}

fn candle_group_key(setup: &PatternSetup) -> String {
    format!(
        "{}|{}",
        setup.source_table.as_deref().unwrap_or(""),
        setup.symbol
    )
}

fn evaluate_outcome(setup: &PatternSetup, candles: &[ForwardCandle], args: &Args) -> OutcomeRow {
    let direction = trade_direction(setup, args);
    let xa_distance = setup.xa_price_length.abs() * args.xa_multiple;
    let entry_index = forward_start_index(setup, candles);
    let Some(entry_candle) = candles.get(entry_index) else {
        return OutcomeRow {
            setup: setup.clone(),
            outcome: "none".to_string(),
            entry_date: None,
            entry_price: None,
            hit_date: None,
            bars_to_hit: None,
            minutes_to_hit: None,
            xa_distance,
            reversal_target_price: setup.d_price + direction * xa_distance,
            continuation_target_price: setup.d_price - direction * xa_distance,
            max_reversal_excursion: 0.0,
            max_continuation_excursion: 0.0,
            candles_scanned: 0,
        };
    };
    let entry_date = entry_candle.candle_date;
    let entry_price = entry_candle.open;
    let reversal_target_price = entry_price + direction * xa_distance;
    let continuation_target_price = entry_price - direction * xa_distance;
    let mut max_reversal_excursion = 0.0f64;
    let mut max_continuation_excursion = 0.0f64;

    for (relative_index, candle) in candles.iter().enumerate().skip(entry_index) {
        let reversal_excursion = if direction > 0.0 {
            candle.high - entry_price
        } else {
            entry_price - candle.low
        }
        .max(0.0);
        let continuation_excursion = if direction > 0.0 {
            entry_price - candle.low
        } else {
            candle.high - entry_price
        }
        .max(0.0);
        max_reversal_excursion = max_reversal_excursion.max(reversal_excursion);
        max_continuation_excursion = max_continuation_excursion.max(continuation_excursion);

        let reversal_hit = if direction > 0.0 {
            candle.high >= reversal_target_price
        } else {
            candle.low <= reversal_target_price
        };
        let continuation_hit = if direction > 0.0 {
            candle.low <= continuation_target_price
        } else {
            candle.high >= continuation_target_price
        };

        if reversal_hit || continuation_hit {
            let outcome = if continuation_hit {
                "continuation_xa"
            } else {
                "reversal_xa"
            }
            .to_string();
            let bars_to_hit = relative_index.saturating_sub(entry_index).saturating_add(1) as i64;
            let minutes_to_hit = (candle.candle_date - entry_date).num_minutes();
            return OutcomeRow {
                setup: setup.clone(),
                outcome,
                entry_date: Some(entry_date),
                entry_price: Some(entry_price),
                hit_date: Some(candle.candle_date),
                bars_to_hit: Some(bars_to_hit),
                minutes_to_hit: Some(minutes_to_hit),
                xa_distance,
                reversal_target_price,
                continuation_target_price,
                max_reversal_excursion,
                max_continuation_excursion,
                candles_scanned: candles.len().saturating_sub(entry_index) as i64,
            };
        }
    }

    OutcomeRow {
        setup: setup.clone(),
        outcome: "none".to_string(),
        entry_date: Some(entry_date),
        entry_price: Some(entry_price),
        hit_date: None,
        bars_to_hit: None,
        minutes_to_hit: None,
        xa_distance,
        reversal_target_price,
        continuation_target_price,
        max_reversal_excursion,
        max_continuation_excursion,
        candles_scanned: candles.len().saturating_sub(entry_index) as i64,
    }
}

async fn flush_rows(
    pool: &MySqlPool,
    run_id: &str,
    xa_multiple: f64,
    outcome_mode_label: &str,
    rows: &mut Vec<OutcomeRow>,
) -> Result<(), sqlx::Error> {
    if rows.is_empty() {
        return Ok(());
    }

    let mut query_builder = QueryBuilder::<MySql>::new(
        r#"
        INSERT INTO pattern_xa_outcomes (
            run_id,
            setup_id,
            pattern_id,
            pattern_group_id,
            event_id,
            symbol,
            root_symbol,
            contract_symbol,
            source_table,
            source_timeframe,
            market,
            pattern_family_key,
            d_date,
            d_confirm_date,
            d_price,
            entry_date,
            entry_price,
            outcome_mode,
            xa_distance,
            xa_multiple,
            reversal_target_price,
            continuation_target_price,
            outcome,
            hit_date,
            bars_to_hit,
            minutes_to_hit,
            max_reversal_excursion,
            max_continuation_excursion,
            candles_scanned
        )
        "#,
    );

    query_builder.push_values(rows.iter(), |mut row_builder, row| {
        row_builder
            .push_bind(run_id)
            .push_bind(&row.setup.setup_id)
            .push_bind(&row.setup.pattern_id)
            .push_bind(&row.setup.pattern_group_id)
            .push_bind(&row.setup.event_id)
            .push_bind(&row.setup.symbol)
            .push_bind(&row.setup.root_symbol)
            .push_bind(&row.setup.contract_symbol)
            .push_bind(&row.setup.source_table)
            .push_bind(&row.setup.source_timeframe)
            .push_bind(&row.setup.market)
            .push_bind(&row.setup.pattern_family_key)
            .push_bind(row.setup.d_date)
            .push_bind(row.setup.d_confirm_date)
            .push_bind(row.setup.d_price)
            .push_bind(row.entry_date)
            .push_bind(row.entry_price)
            .push_bind(outcome_mode_label)
            .push_bind(row.xa_distance)
            .push_bind(xa_multiple)
            .push_bind(row.reversal_target_price)
            .push_bind(row.continuation_target_price)
            .push_bind(&row.outcome)
            .push_bind(row.hit_date)
            .push_bind(row.bars_to_hit)
            .push_bind(row.minutes_to_hit)
            .push_bind(row.max_reversal_excursion)
            .push_bind(row.max_continuation_excursion)
            .push_bind(row.candles_scanned);
    });

    query_builder.push(
        r#"
        ON DUPLICATE KEY UPDATE
            entry_date = VALUES(entry_date),
            entry_price = VALUES(entry_price),
            outcome_mode = VALUES(outcome_mode),
            reversal_target_price = VALUES(reversal_target_price),
            continuation_target_price = VALUES(continuation_target_price),
            outcome = VALUES(outcome),
            hit_date = VALUES(hit_date),
            bars_to_hit = VALUES(bars_to_hit),
            minutes_to_hit = VALUES(minutes_to_hit),
            max_reversal_excursion = VALUES(max_reversal_excursion),
            max_continuation_excursion = VALUES(max_continuation_excursion),
            candles_scanned = VALUES(candles_scanned)
        "#,
    );

    query_builder.build().execute(pool).await?;
    rows.clear();
    Ok(())
}

async fn refresh_family_rows(pool: &MySqlPool, run_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM pattern_xa_outcome_families WHERE run_id = ?")
        .bind(run_id)
        .execute(pool)
        .await?;

    sqlx::query(
        r#"
        INSERT INTO pattern_xa_outcome_families (
            run_id,
            pattern_family_key,
            harmonic_type,
            family_bin,
            family_size_bucket,
            family_time_bin,
            family_x_strictness,
            total_count,
            reversal_count,
            continuation_count,
            ambiguous_count,
            none_count,
            reversal_rate,
            continuation_rate,
            avg_bars_to_hit,
            avg_minutes_to_hit
        )
        SELECT
            family.run_id,
            family.pattern_family_key,
            COALESCE(meta.harmonic_type, 'Unknown') AS harmonic_type,
            COALESCE(meta.family_bin, 'Unknown') AS family_bin,
            COALESCE(meta.family_size_bucket, 'Unknown') AS family_size_bucket,
            COALESCE(meta.family_time_bin, 'Unknown') AS family_time_bin,
            COALESCE(meta.family_x_strictness, 'Unknown') AS family_x_strictness,
            family.total_count,
            family.reversal_count,
            family.continuation_count,
            family.ambiguous_count,
            family.none_count,
            family.reversal_rate,
            family.continuation_rate,
            family.avg_bars_to_hit,
            family.avg_minutes_to_hit
        FROM (
            SELECT
                run_id,
                COALESCE(pattern_family_key, 'Unknown') AS pattern_family_key,
                COUNT(*) AS total_count,
                SUM(CASE WHEN outcome = 'reversal_xa' THEN 1 ELSE 0 END) AS reversal_count,
                SUM(CASE WHEN outcome = 'continuation_xa' THEN 1 ELSE 0 END) AS continuation_count,
                SUM(CASE WHEN outcome = 'ambiguous' THEN 1 ELSE 0 END) AS ambiguous_count,
                SUM(CASE WHEN outcome = 'none' THEN 1 ELSE 0 END) AS none_count,
                SUM(CASE WHEN outcome = 'reversal_xa' THEN 1 ELSE 0 END) / COUNT(*) AS reversal_rate,
                SUM(CASE WHEN outcome = 'continuation_xa' THEN 1 ELSE 0 END) / COUNT(*) AS continuation_rate,
                AVG(bars_to_hit) AS avg_bars_to_hit,
                AVG(minutes_to_hit) AS avg_minutes_to_hit
            FROM pattern_xa_outcomes
            WHERE run_id = ?
            GROUP BY run_id, COALESCE(pattern_family_key, 'Unknown')
        ) family
        LEFT JOIN (
            SELECT
                pattern_family_key,
                COALESCE(MAX(NULLIF(pattern_family_harmonic_type, '')), MAX(NULLIF(harmonic_type, '')), 'Unknown') AS harmonic_type,
                COALESCE(MAX(NULLIF(pattern_family_bin, '')), 'Unknown') AS family_bin,
                COALESCE(MAX(NULLIF(pattern_family_size_bucket, '')), 'Unknown') AS family_size_bucket,
                COALESCE(MAX(NULLIF(pattern_family_time_bin, '')), 'Unknown') AS family_time_bin,
                COALESCE(MAX(NULLIF(pattern_family_x_strictness, '')), 'Unknown') AS family_x_strictness
            FROM pattern_setups
            WHERE pattern_family_key IS NOT NULL
              AND pattern_family_key <> ''
            GROUP BY pattern_family_key
        ) meta
          ON meta.pattern_family_key = family.pattern_family_key
        "#,
    )
    .bind(run_id)
    .execute(pool)
    .await?;

    Ok(())
}

async fn insert_run_start(pool: &MySqlPool, run_id: &str, args: &Args) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO pattern_xa_outcome_runs (
            run_id,
            source_scope,
            source_timeframe,
            scan_year_start,
            scan_year_end,
            scan_year_label,
            requested_limit,
            max_forward_multiple,
            xa_multiple,
            outcome_mode
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(run_id)
    .bind(&args.source_scope)
    .bind(&args.source_timeframe)
    .bind(args.start_year)
    .bind(args.end_year)
    .bind(year_label(args))
    .bind(args.limit)
    .bind(args.max_forward_multiple)
    .bind(args.xa_multiple)
    .bind(outcome_mode(args))
    .execute(pool)
    .await?;

    Ok(())
}

async fn update_run_finish(
    pool: &MySqlPool,
    run_id: &str,
    patterns_scanned: i64,
    reversal_count: i64,
    continuation_count: i64,
    ambiguous_count: i64,
    none_count: i64,
    elapsed_ms: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE pattern_xa_outcome_runs
        SET patterns_scanned = ?,
            reversal_count = ?,
            continuation_count = ?,
            ambiguous_count = ?,
            none_count = ?,
            elapsed_ms = ?
        WHERE run_id = ?
        "#,
    )
    .bind(patterns_scanned)
    .bind(reversal_count)
    .bind(continuation_count)
    .bind(ambiguous_count)
    .bind(none_count)
    .bind(elapsed_ms)
    .bind(run_id)
    .execute(pool)
    .await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let args = parse_args()?;
    let run_id = new_run_id();
    let started_at = Instant::now();
    let pool = MySqlPoolOptions::new()
        .max_connections(4)
        .connect(&database_url_from_env()?)
        .await?;

    ensure_tables(&pool).await?;
    insert_run_start(&pool, &run_id, &args).await?;
    let patterns = fetch_patterns(&pool, &args).await?;
    let outcome_mode_label = outcome_mode(&args);
    println!(
        "Pattern XA outcome run {run_id}: loaded {} patterns, source={}, timeframe={}, years={}, outcome_mode={}, xa_multiple={}, max_forward_multiple={}",
        patterns.len(),
        args.source_scope,
        args.source_timeframe.as_deref().unwrap_or("all"),
        year_label(&args),
        outcome_mode_label,
        args.xa_multiple,
        args.max_forward_multiple
    );

    let mut pending = Vec::with_capacity(INSERT_BATCH_SIZE);
    let mut reversal_count = 0_i64;
    let mut continuation_count = 0_i64;
    let mut ambiguous_count = 0_i64;
    let mut none_count = 0_i64;

    let mut processed_count = 0_usize;
    if args.max_forward_multiple == 0 {
        let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (index, setup) in patterns.iter().enumerate() {
            groups
                .entry(candle_group_key(setup))
                .or_default()
                .push(index);
        }
        println!(
            "Unlimited scan mode: grouped patterns into {} symbol/table candle loads",
            groups.len()
        );

        for pattern_indexes in groups.values() {
            let Some(first_index) = pattern_indexes.first() else {
                continue;
            };
            let first_setup = &patterns[*first_index];
            let min_confirm_date = pattern_indexes
                .iter()
                .map(|index| patterns[*index].d_confirm_date)
                .min()
                .unwrap_or(first_setup.d_confirm_date);
            let candles =
                fetch_unlimited_forward_candles_from(&pool, first_setup, min_confirm_date).await?;

            for pattern_index in pattern_indexes {
                let setup = &patterns[*pattern_index];
                let outcome = evaluate_outcome(setup, &candles, &args);
                match outcome.outcome.as_str() {
                    "reversal_xa" => reversal_count += 1,
                    "continuation_xa" => continuation_count += 1,
                    "ambiguous" => ambiguous_count += 1,
                    _ => none_count += 1,
                }
                pending.push(outcome);
                processed_count += 1;

                if pending.len() >= INSERT_BATCH_SIZE {
                    flush_rows(
                        &pool,
                        &run_id,
                        args.xa_multiple,
                        &outcome_mode_label,
                        &mut pending,
                    )
                    .await?;
                }
                if processed_count % 1_000 == 0 || processed_count == patterns.len() {
                    println!(
                        "Processed {}/{} patterns: reversal={}, continuation={}, ambiguous={}, none={}",
                        processed_count,
                        patterns.len(),
                        reversal_count,
                        continuation_count,
                        ambiguous_count,
                        none_count
                    );
                }
            }
        }
    } else {
        for setup in patterns.iter() {
            let candles = fetch_forward_candles(&pool, setup, args.max_forward_multiple).await?;
            let outcome = evaluate_outcome(setup, &candles, &args);
            match outcome.outcome.as_str() {
                "reversal_xa" => reversal_count += 1,
                "continuation_xa" => continuation_count += 1,
                "ambiguous" => ambiguous_count += 1,
                _ => none_count += 1,
            }
            pending.push(outcome);
            processed_count += 1;

            if pending.len() >= INSERT_BATCH_SIZE {
                flush_rows(
                    &pool,
                    &run_id,
                    args.xa_multiple,
                    &outcome_mode_label,
                    &mut pending,
                )
                .await?;
            }
            if processed_count % 1_000 == 0 || processed_count == patterns.len() {
                println!(
                    "Processed {}/{} patterns: reversal={}, continuation={}, ambiguous={}, none={}",
                    processed_count,
                    patterns.len(),
                    reversal_count,
                    continuation_count,
                    ambiguous_count,
                    none_count
                );
            }
        }
    }
    flush_rows(
        &pool,
        &run_id,
        args.xa_multiple,
        &outcome_mode_label,
        &mut pending,
    )
    .await?;
    refresh_family_rows(&pool, &run_id).await?;

    let elapsed_ms = started_at.elapsed().as_millis() as i64;
    update_run_finish(
        &pool,
        &run_id,
        patterns.len() as i64,
        reversal_count,
        continuation_count,
        ambiguous_count,
        none_count,
        elapsed_ms,
    )
    .await?;

    let total = patterns.len().max(1) as f64;
    println!("\nPattern XA outcome layer complete");
    println!("run_id={run_id}");
    println!("patterns_scanned={}", patterns.len());
    println!(
        "reversal_xa={} ({:.2}%)",
        reversal_count,
        reversal_count as f64 / total * 100.0
    );
    println!(
        "continuation_xa={} ({:.2}%)",
        continuation_count,
        continuation_count as f64 / total * 100.0
    );
    println!(
        "ambiguous={} ({:.2}%)",
        ambiguous_count,
        ambiguous_count as f64 / total * 100.0
    );
    println!(
        "none={} ({:.2}%)",
        none_count,
        none_count as f64 / total * 100.0
    );
    println!("elapsed={:.2}s", elapsed_ms as f64 / 1000.0);

    Ok(())
}
