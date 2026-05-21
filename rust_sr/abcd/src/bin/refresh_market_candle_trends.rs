use std::env;

use abcd::models::market_trend::{
    calculate_ema21_trends, MarketTrendInputCandle, MarketTrendLabel, MarketTrendPoint,
    MarketTrendTimeframe,
};
use chrono::NaiveDateTime;
use sqlx::{mysql::MySqlPool, MySql, QueryBuilder, Row};

const TREND_TABLE: &str = "entry_exit_market_candle_trends";
const UPSERT_CHUNK_SIZE: usize = 1_000;
const DEFAULT_SYMBOL_LIMIT: i64 = i64::MAX;

#[derive(Debug, Clone, Copy)]
struct TimeframeSpec {
    timeframe: MarketTrendTimeframe,
    label: &'static str,
    source_table: &'static str,
}

const TIMEFRAMES: [TimeframeSpec; 3] = [
    TimeframeSpec {
        timeframe: MarketTrendTimeframe::FiveMinute,
        label: "5m",
        source_table: "futures_contract_5m_candles",
    },
    TimeframeSpec {
        timeframe: MarketTrendTimeframe::FifteenMinute,
        label: "15m",
        source_table: "futures_contract_15m_candles",
    },
    TimeframeSpec {
        timeframe: MarketTrendTimeframe::OneHour,
        label: "1h",
        source_table: "futures_contract_1h_candles",
    },
];

#[derive(Debug)]
struct Args {
    timeframes: Vec<TimeframeSpec>,
    symbol: Option<String>,
    root_symbol: Option<String>,
    symbol_limit: i64,
    summary_only: bool,
}

#[derive(Clone, sqlx::FromRow)]
struct CandleRow {
    root_symbol: Option<String>,
    symbol: String,
    ts_utc: NaiveDateTime,
    close: f64,
}

struct TrendRow {
    root_symbol: Option<String>,
    symbol: String,
    timeframe: &'static str,
    source_table: &'static str,
    candle_ts_utc: NaiveDateTime,
    close: f64,
    ema_21: Option<f64>,
    ema_21_slope: Option<f64>,
    close_to_ema_pct: Option<f64>,
    ema_slope_pct: Option<f64>,
    strength_pct: Option<f64>,
    trend_label: Option<&'static str>,
}

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

fn arg_value(raw_args: &[String], name: &str) -> Option<String> {
    raw_args
        .windows(2)
        .find(|window| window[0] == name)
        .map(|window| window[1].clone())
        .or_else(|| {
            raw_args
                .iter()
                .find_map(|arg| arg.strip_prefix(&format!("{name}=")).map(str::to_string))
        })
}

fn normalize_timeframe(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '_', '-'], "")
        .replace("minutes", "m")
        .replace("minute", "m")
        .replace("mins", "m")
        .replace("min", "m")
        .replace("hours", "h")
        .replace("hour", "h")
        .replace("hrs", "h")
        .replace("hr", "h")
}

fn parse_timeframe(value: &str) -> Option<TimeframeSpec> {
    match normalize_timeframe(value).as_str() {
        "5" | "5m" => Some(TIMEFRAMES[0]),
        "15" | "15m" => Some(TIMEFRAMES[1]),
        "60" | "60m" | "1h" => Some(TIMEFRAMES[2]),
        _ => None,
    }
}

fn requested_timeframes(
    raw_args: &[String],
) -> Result<Vec<TimeframeSpec>, Box<dyn std::error::Error>> {
    let raw = arg_value(raw_args, "--timeframes")
        .or_else(|| arg_value(raw_args, "--timeframe"))
        .or_else(|| env::var("ABCD_MARKET_TREND_TIMEFRAMES").ok())
        .or_else(|| env::var("ABCD_MARKET_TREND_TIMEFRAME").ok())
        .unwrap_or_else(|| "5m,15m,1h".to_string());

    let mut specs: Vec<TimeframeSpec> = Vec::new();
    for item in raw.split(',') {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Some(spec) = parse_timeframe(trimmed) else {
            return Err(format!("Unsupported market trend timeframe: {trimmed}").into());
        };

        if !specs.iter().any(|existing| existing.label == spec.label) {
            specs.push(spec);
        }
    }

    if specs.is_empty() {
        return Err("No market trend timeframes requested".into());
    }

    Ok(specs)
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let raw_args: Vec<String> = env::args().collect();
    let symbol_limit = arg_value(&raw_args, "--symbol-limit")
        .or_else(|| env::var("ABCD_MARKET_TREND_SYMBOL_LIMIT").ok())
        .map(|value| value.trim().parse::<i64>())
        .transpose()?
        .unwrap_or(DEFAULT_SYMBOL_LIMIT);

    Ok(Args {
        timeframes: requested_timeframes(&raw_args)?,
        symbol: arg_value(&raw_args, "--symbol")
            .or_else(|| env::var("ABCD_MARKET_TREND_SYMBOL").ok()),
        root_symbol: arg_value(&raw_args, "--root-symbol")
            .or_else(|| env::var("ABCD_MARKET_TREND_ROOT_SYMBOL").ok()),
        symbol_limit,
        summary_only: raw_args.iter().any(|arg| arg == "--summary-only"),
    })
}

async fn ensure_trend_table(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {TREND_TABLE} (
            root_symbol VARCHAR(16) NULL,
            symbol VARCHAR(32) NOT NULL,
            timeframe VARCHAR(16) NOT NULL,
            source_table VARCHAR(64) NOT NULL,
            candle_ts_utc DATETIME NOT NULL,
            close DOUBLE NOT NULL,
            ema_21 DOUBLE NULL,
            ema_21_slope DOUBLE NULL,
            close_to_ema_pct DOUBLE NULL,
            ema_slope_pct DOUBLE NULL,
            strength_pct DOUBLE NULL,
            trend_label VARCHAR(16) NULL,
            formula VARCHAR(64) NOT NULL DEFAULT 'ema21_close_vs_slope',
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (symbol, timeframe, candle_ts_utc),
            INDEX idx_market_trends_root_tf_ts (root_symbol, timeframe, candle_ts_utc),
            INDEX idx_market_trends_tf_label (timeframe, trend_label),
            INDEX idx_market_trends_ts (candle_ts_utc)
        )
        "#
    ))
    .execute(pool)
    .await?;

    Ok(())
}

async fn selected_symbols(
    pool: &MySqlPool,
    spec: TimeframeSpec,
    args: &Args,
) -> Result<Vec<String>, sqlx::Error> {
    let mut sql = format!("SELECT DISTINCT symbol FROM {}", spec.source_table);
    let mut where_parts = Vec::new();

    if args.symbol.is_some() {
        where_parts.push("symbol = ?");
    }
    if args.root_symbol.is_some() {
        where_parts.push("root_symbol = ?");
    }

    if !where_parts.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&where_parts.join(" AND "));
    }

    sql.push_str(" ORDER BY symbol");
    if args.symbol_limit != DEFAULT_SYMBOL_LIMIT {
        sql.push_str(" LIMIT ?");
    }

    let mut query = sqlx::query_scalar::<_, String>(&sql);
    if let Some(symbol) = args.symbol.as_deref() {
        query = query.bind(symbol);
    }
    if let Some(root_symbol) = args.root_symbol.as_deref() {
        query = query.bind(root_symbol);
    }
    if args.symbol_limit != DEFAULT_SYMBOL_LIMIT {
        query = query.bind(args.symbol_limit.max(0));
    }

    query.fetch_all(pool).await
}

async fn candles_for_symbol(
    pool: &MySqlPool,
    spec: TimeframeSpec,
    symbol: &str,
) -> Result<Vec<CandleRow>, sqlx::Error> {
    sqlx::query_as::<_, CandleRow>(&format!(
        r#"
        SELECT
            root_symbol,
            symbol,
            ts_utc,
            CAST(close AS DOUBLE) AS close
        FROM {}
        WHERE symbol = ?
        ORDER BY ts_utc
        "#,
        spec.source_table
    ))
    .bind(symbol)
    .fetch_all(pool)
    .await
}

fn trend_label(value: Option<MarketTrendLabel>) -> Option<&'static str> {
    value.map(MarketTrendLabel::as_str)
}

fn build_trend_rows(candles: &[CandleRow], spec: TimeframeSpec) -> Vec<TrendRow> {
    let input: Vec<_> = candles
        .iter()
        .map(|candle| MarketTrendInputCandle {
            ts_utc: candle.ts_utc,
            close: candle.close,
        })
        .collect();
    let trend_points = calculate_ema21_trends(&input);

    candles
        .iter()
        .zip(trend_points.iter())
        .map(|(candle, point)| trend_row(candle, point, spec))
        .collect()
}

fn trend_row(candle: &CandleRow, point: &MarketTrendPoint, spec: TimeframeSpec) -> TrendRow {
    TrendRow {
        root_symbol: candle.root_symbol.clone(),
        symbol: candle.symbol.clone(),
        timeframe: spec.label,
        source_table: spec.source_table,
        candle_ts_utc: point.ts_utc,
        close: candle.close,
        ema_21: point.ema_21,
        ema_21_slope: point.ema_21_slope,
        close_to_ema_pct: point.close_to_ema_pct,
        ema_slope_pct: point.ema_slope_pct,
        strength_pct: point.strength_pct,
        trend_label: trend_label(point.label),
    }
}

async fn upsert_trend_rows(pool: &MySqlPool, rows: &[TrendRow]) -> Result<u64, sqlx::Error> {
    if rows.is_empty() {
        return Ok(0);
    }

    let mut written = 0_u64;

    for chunk in rows.chunks(UPSERT_CHUNK_SIZE) {
        let mut builder = QueryBuilder::<MySql>::new(format!(
            r#"
            INSERT INTO {TREND_TABLE} (
                root_symbol,
                symbol,
                timeframe,
                source_table,
                candle_ts_utc,
                close,
                ema_21,
                ema_21_slope,
                close_to_ema_pct,
                ema_slope_pct,
                strength_pct,
                trend_label
            )
            "#
        ));

        builder.push_values(chunk, |mut row, item| {
            row.push_bind(&item.root_symbol)
                .push_bind(&item.symbol)
                .push_bind(item.timeframe)
                .push_bind(item.source_table)
                .push_bind(item.candle_ts_utc)
                .push_bind(item.close)
                .push_bind(item.ema_21)
                .push_bind(item.ema_21_slope)
                .push_bind(item.close_to_ema_pct)
                .push_bind(item.ema_slope_pct)
                .push_bind(item.strength_pct)
                .push_bind(item.trend_label);
        });

        builder.push(
            r#"
            ON DUPLICATE KEY UPDATE
                root_symbol = VALUES(root_symbol),
                source_table = VALUES(source_table),
                close = VALUES(close),
                ema_21 = VALUES(ema_21),
                ema_21_slope = VALUES(ema_21_slope),
                close_to_ema_pct = VALUES(close_to_ema_pct),
                ema_slope_pct = VALUES(ema_slope_pct),
                strength_pct = VALUES(strength_pct),
                trend_label = VALUES(trend_label),
                formula = 'ema21_close_vs_slope',
                updated_at = CURRENT_TIMESTAMP
            "#,
        );

        let result = builder.build().execute(pool).await?;
        written += result.rows_affected();
    }

    Ok(written)
}

async fn refresh_timeframe(
    pool: &MySqlPool,
    spec: TimeframeSpec,
    args: &Args,
) -> Result<i64, Box<dyn std::error::Error>> {
    let symbols = selected_symbols(pool, spec, args).await?;
    let total_symbols = symbols.len();
    let mut total_rows = 0_i64;

    println!(
        "Refreshing {} trends from {} for {} symbols ({} minute candles)",
        spec.label,
        spec.source_table,
        total_symbols,
        spec.timeframe.candle_minutes()
    );

    for (index, symbol) in symbols.iter().enumerate() {
        let candles = candles_for_symbol(pool, spec, symbol).await?;
        let rows = build_trend_rows(&candles, spec);
        upsert_trend_rows(pool, &rows).await?;
        total_rows += rows.len() as i64;

        if (index + 1) % 100 == 0 || index + 1 == total_symbols {
            println!(
                "{}: refreshed {}/{} symbols, {} trend rows",
                spec.label,
                index + 1,
                total_symbols,
                total_rows
            );
        }
    }

    Ok(total_rows)
}

async fn print_summary(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    let rows = sqlx::query(&format!(
        r#"
        SELECT
            timeframe,
            COUNT(*) AS row_count,
            COUNT(trend_label) AS labeled_count,
            CAST(SUM(CASE WHEN trend_label = 'bullish' THEN 1 ELSE 0 END) AS SIGNED) AS bullish_count,
            CAST(SUM(CASE WHEN trend_label = 'bearish' THEN 1 ELSE 0 END) AS SIGNED) AS bearish_count,
            CAST(SUM(CASE WHEN trend_label = 'neutral' THEN 1 ELSE 0 END) AS SIGNED) AS neutral_count,
            COUNT(DISTINCT symbol) AS symbol_count,
            MIN(candle_ts_utc) AS first_candle_at,
            MAX(candle_ts_utc) AS last_candle_at
        FROM {TREND_TABLE}
        GROUP BY timeframe
        ORDER BY FIELD(timeframe, '5m', '15m', '1h'), timeframe
        "#
    ))
    .fetch_all(pool)
    .await?;

    println!("timeframe\trows\tlabeled\tbullish\tbearish\tneutral\tsymbols\tfirst\tlast");
    for row in rows {
        let timeframe: String = row.try_get("timeframe")?;
        let row_count: i64 = row.try_get("row_count")?;
        let labeled_count: i64 = row.try_get("labeled_count")?;
        let bullish_count: Option<i64> = row.try_get("bullish_count").ok();
        let bearish_count: Option<i64> = row.try_get("bearish_count").ok();
        let neutral_count: Option<i64> = row.try_get("neutral_count").ok();
        let symbol_count: i64 = row.try_get("symbol_count")?;
        let first_candle_at: Option<NaiveDateTime> = row.try_get("first_candle_at").ok();
        let last_candle_at: Option<NaiveDateTime> = row.try_get("last_candle_at").ok();

        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            timeframe,
            row_count,
            labeled_count,
            bullish_count.unwrap_or(0),
            bearish_count.unwrap_or(0),
            neutral_count.unwrap_or(0),
            symbol_count,
            first_candle_at
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            last_candle_at
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string())
        );
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let args = parse_args()?;
    let pool = MySqlPool::connect(&database_url_from_env()?).await?;

    ensure_trend_table(&pool).await?;

    if args.summary_only {
        print_summary(&pool).await?;
        return Ok(());
    }

    let mut total_rows = 0_i64;
    for spec in &args.timeframes {
        total_rows += refresh_timeframe(&pool, *spec, &args).await?;
    }

    println!(
        "Market candle trend refresh complete: {} rows scanned into {}",
        total_rows, TREND_TABLE
    );

    Ok(())
}
