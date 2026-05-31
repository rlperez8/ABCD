use std::env;

use chrono::NaiveDateTime;
use sqlx::mysql::MySqlPool;

const DEFAULT_SYMBOL_LIMIT: i64 = i64::MAX;
const SOURCE_TABLE: &str = "futures_contract_1m_candles";

#[derive(Clone, Copy)]
struct TimeframeSpec {
    label: &'static str,
    table_name: &'static str,
    index_prefix: &'static str,
    minutes: i64,
}

const TIMEFRAMES: [TimeframeSpec; 19] = [
    TimeframeSpec {
        label: "2m",
        table_name: "futures_contract_2m_candles",
        index_prefix: "futures_2m",
        minutes: 2,
    },
    TimeframeSpec {
        label: "3m",
        table_name: "futures_contract_3m_candles",
        index_prefix: "futures_3m",
        minutes: 3,
    },
    TimeframeSpec {
        label: "4m",
        table_name: "futures_contract_4m_candles",
        index_prefix: "futures_4m",
        minutes: 4,
    },
    TimeframeSpec {
        label: "5m",
        table_name: "futures_contract_5m_candles",
        index_prefix: "futures_5m",
        minutes: 5,
    },
    TimeframeSpec {
        label: "6m",
        table_name: "futures_contract_6m_candles",
        index_prefix: "futures_6m",
        minutes: 6,
    },
    TimeframeSpec {
        label: "7m",
        table_name: "futures_contract_7m_candles",
        index_prefix: "futures_7m",
        minutes: 7,
    },
    TimeframeSpec {
        label: "8m",
        table_name: "futures_contract_8m_candles",
        index_prefix: "futures_8m",
        minutes: 8,
    },
    TimeframeSpec {
        label: "9m",
        table_name: "futures_contract_9m_candles",
        index_prefix: "futures_9m",
        minutes: 9,
    },
    TimeframeSpec {
        label: "10m",
        table_name: "futures_contract_10m_candles",
        index_prefix: "futures_10m",
        minutes: 10,
    },
    TimeframeSpec {
        label: "11m",
        table_name: "futures_contract_11m_candles",
        index_prefix: "futures_11m",
        minutes: 11,
    },
    TimeframeSpec {
        label: "12m",
        table_name: "futures_contract_12m_candles",
        index_prefix: "futures_12m",
        minutes: 12,
    },
    TimeframeSpec {
        label: "13m",
        table_name: "futures_contract_13m_candles",
        index_prefix: "futures_13m",
        minutes: 13,
    },
    TimeframeSpec {
        label: "14m",
        table_name: "futures_contract_14m_candles",
        index_prefix: "futures_14m",
        minutes: 14,
    },
    TimeframeSpec {
        label: "15m",
        table_name: "futures_contract_15m_candles",
        index_prefix: "futures_15m",
        minutes: 15,
    },
    TimeframeSpec {
        label: "30m",
        table_name: "futures_contract_30m_candles",
        index_prefix: "futures_30m",
        minutes: 30,
    },
    TimeframeSpec {
        label: "1h",
        table_name: "futures_contract_1h_candles",
        index_prefix: "futures_1h",
        minutes: 60,
    },
    TimeframeSpec {
        label: "4h",
        table_name: "futures_contract_4h_candles",
        index_prefix: "futures_4h",
        minutes: 240,
    },
    TimeframeSpec {
        label: "12h",
        table_name: "futures_contract_12h_candles",
        index_prefix: "futures_12h",
        minutes: 720,
    },
    TimeframeSpec {
        label: "1d",
        table_name: "futures_contract_1d_candles",
        index_prefix: "futures_1d",
        minutes: 1440,
    },
];

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn env_i64(name: &str, default: i64) -> Result<i64, Box<dyn std::error::Error>> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(value.trim().parse::<i64>()?),
        _ => Ok(default),
    }
}

fn env_datetime(name: &str) -> Result<Option<NaiveDateTime>, Box<dyn std::error::Error>> {
    let Ok(value) = env::var(name) else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    Ok(Some(NaiveDateTime::parse_from_str(
        trimmed,
        "%Y-%m-%d %H:%M:%S",
    )?))
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
    let normalized = normalize_timeframe(value);
    match normalized.as_str() {
        "2" | "2m" => Some(TIMEFRAMES[0]),
        "3" | "3m" => Some(TIMEFRAMES[1]),
        "4" | "4m" => Some(TIMEFRAMES[2]),
        "5" | "5m" => Some(TIMEFRAMES[3]),
        "6" | "6m" => Some(TIMEFRAMES[4]),
        "7" | "7m" => Some(TIMEFRAMES[5]),
        "8" | "8m" => Some(TIMEFRAMES[6]),
        "9" | "9m" => Some(TIMEFRAMES[7]),
        "10" | "10m" => Some(TIMEFRAMES[8]),
        "11" | "11m" => Some(TIMEFRAMES[9]),
        "12" | "12m" => Some(TIMEFRAMES[10]),
        "13" | "13m" => Some(TIMEFRAMES[11]),
        "14" | "14m" => Some(TIMEFRAMES[12]),
        "15" | "15m" => Some(TIMEFRAMES[13]),
        "30" | "30m" => Some(TIMEFRAMES[14]),
        "60" | "60m" | "1h" => Some(TIMEFRAMES[15]),
        "240" | "240m" | "4h" => Some(TIMEFRAMES[16]),
        "720" | "720m" | "12h" => Some(TIMEFRAMES[17]),
        "1440" | "1440m" | "24h" | "1d" | "d" | "day" | "daily" => Some(TIMEFRAMES[18]),
        _ => None,
    }
}

fn requested_timeframes() -> Result<Vec<TimeframeSpec>, Box<dyn std::error::Error>> {
    let raw = env::var("ABCD_TIMEFRAMES")
        .or_else(|_| env::var("ABCD_TIMEFRAME"))
        .or_else(|_| env::var("ABCD_FUTURES_TIMEFRAME"))
        .unwrap_or_else(|_| "5m,15m,30m,1h,4h,12h,1d".to_string());

    let mut specs = Vec::new();
    for item in raw.split(',') {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(spec) = parse_timeframe(trimmed) else {
            return Err(format!("Unsupported timeframe: {trimmed}").into());
        };
        if !specs
            .iter()
            .any(|existing: &TimeframeSpec| existing.label == spec.label)
        {
            specs.push(spec);
        }
    }

    if specs.is_empty() {
        return Err("No timeframes requested".into());
    }

    Ok(specs)
}

async fn ensure_target_table(pool: &MySqlPool, spec: TimeframeSpec) -> Result<(), sqlx::Error> {
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {} (
            root_symbol VARCHAR(16) NULL,
            symbol VARCHAR(32) NOT NULL,
            ts_utc DATETIME NOT NULL,
            open DECIMAL(18,8) NOT NULL,
            high DECIMAL(18,8) NOT NULL,
            low DECIMAL(18,8) NOT NULL,
            close DECIMAL(18,8) NOT NULL,
            volume BIGINT NOT NULL DEFAULT 0,
            source_count INT NOT NULL DEFAULT 0,
            source_first_ts DATETIME NULL,
            source_last_ts DATETIME NULL,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (symbol, ts_utc),
            INDEX idx_{}_root_ts (root_symbol, ts_utc),
            INDEX idx_{}_ts (ts_utc)
        )
        "#,
        spec.table_name, spec.index_prefix, spec.index_prefix
    ))
    .execute(pool)
    .await?;

    Ok(())
}

async fn selected_symbols(
    pool: &MySqlPool,
    root_symbol: Option<&str>,
    symbol: Option<&str>,
    limit: i64,
) -> Result<Vec<String>, sqlx::Error> {
    let mut sql = format!("SELECT DISTINCT symbol FROM {SOURCE_TABLE}");
    let mut where_parts = Vec::new();

    if root_symbol.is_some() {
        where_parts.push("root_symbol = ?");
    }
    if symbol.is_some() {
        where_parts.push("symbol = ?");
    }

    if !where_parts.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&where_parts.join(" AND "));
    }

    sql.push_str(" ORDER BY symbol");
    if limit != DEFAULT_SYMBOL_LIMIT {
        sql.push_str(" LIMIT ?");
    }

    let mut query = sqlx::query_scalar::<_, String>(&sql);
    if let Some(root_symbol) = root_symbol {
        query = query.bind(root_symbol);
    }
    if let Some(symbol) = symbol {
        query = query.bind(symbol);
    }
    if limit != DEFAULT_SYMBOL_LIMIT {
        query = query.bind(limit.max(1));
    }

    query.fetch_all(pool).await
}

async fn rebuild_symbol(
    pool: &MySqlPool,
    spec: TimeframeSpec,
    symbol: &str,
    start_ts: Option<NaiveDateTime>,
    end_ts: Option<NaiveDateTime>,
) -> Result<u64, sqlx::Error> {
    let mut sql = format!(
        r#"
        INSERT INTO {} (
            root_symbol,
            symbol,
            ts_utc,
            open,
            high,
            low,
            close,
            volume,
            source_count,
            source_first_ts,
            source_last_ts
        )
        SELECT
            agg.root_symbol,
            agg.symbol,
            agg.bucket_ts AS ts_utc,
            CAST(MAX(open_source.open) AS DECIMAL(18,8)) AS open,
            agg.high,
            agg.low,
            CAST(MAX(close_source.close) AS DECIMAL(18,8)) AS close,
            agg.volume,
            agg.source_count,
            agg.source_first_ts,
            agg.source_last_ts
        FROM (
            SELECT
                MAX(root_symbol) AS root_symbol,
                symbol,
                TIMESTAMPADD(
                    MINUTE,
                    FLOOR(TIMESTAMPDIFF(MINUTE, '1970-01-01 00:00:00', ts_utc) / {}) * {},
                    '1970-01-01 00:00:00'
                ) AS bucket_ts,
                MAX(high) AS high,
                MIN(low) AS low,
                COALESCE(SUM(volume), 0) AS volume,
                COUNT(*) AS source_count,
                MIN(ts_utc) AS source_first_ts,
                MAX(ts_utc) AS source_last_ts
            FROM {SOURCE_TABLE}
            WHERE symbol = ?
        "#,
        spec.table_name, spec.minutes, spec.minutes
    );

    if start_ts.is_some() {
        sql.push_str(" AND ts_utc >= ?");
    }
    if end_ts.is_some() {
        sql.push_str(" AND ts_utc < ?");
    }

    sql.push_str(&format!(
        r#"
            GROUP BY symbol, bucket_ts
        ) agg
        INNER JOIN {SOURCE_TABLE} open_source
            ON open_source.symbol = agg.symbol
           AND open_source.ts_utc = agg.source_first_ts
        INNER JOIN {SOURCE_TABLE} close_source
            ON close_source.symbol = agg.symbol
           AND close_source.ts_utc = agg.source_last_ts
        GROUP BY
            agg.root_symbol,
            agg.symbol,
            agg.bucket_ts,
            agg.high,
            agg.low,
            agg.volume,
            agg.source_count,
            agg.source_first_ts,
            agg.source_last_ts
        ON DUPLICATE KEY UPDATE
            root_symbol = VALUES(root_symbol),
            open = VALUES(open),
            high = VALUES(high),
            low = VALUES(low),
            close = VALUES(close),
            volume = VALUES(volume),
            source_count = VALUES(source_count),
            source_first_ts = VALUES(source_first_ts),
            source_last_ts = VALUES(source_last_ts),
            updated_at = CURRENT_TIMESTAMP
        "#
    ));

    let mut query = sqlx::query(&sql).bind(symbol);
    if let Some(start_ts) = start_ts {
        query = query.bind(start_ts);
    }
    if let Some(end_ts) = end_ts {
        query = query.bind(end_ts);
    }

    let result = query.execute(pool).await?;
    Ok(result.rows_affected())
}

async fn clear_symbol(
    pool: &MySqlPool,
    spec: TimeframeSpec,
    symbol: &str,
    start_ts: Option<NaiveDateTime>,
    end_ts: Option<NaiveDateTime>,
) -> Result<u64, sqlx::Error> {
    let mut sql = format!("DELETE FROM {} WHERE symbol = ?", spec.table_name);
    if start_ts.is_some() {
        sql.push_str(" AND ts_utc >= ?");
    }
    if end_ts.is_some() {
        sql.push_str(" AND ts_utc < ?");
    }

    let mut query = sqlx::query(&sql).bind(symbol);
    if let Some(start_ts) = start_ts {
        query = query.bind(start_ts);
    }
    if let Some(end_ts) = end_ts {
        query = query.bind(end_ts);
    }

    let result = query.execute(pool).await?;
    Ok(result.rows_affected())
}

async fn build_timeframe(
    pool: &MySqlPool,
    spec: TimeframeSpec,
    symbols: &[String],
    root_symbol: Option<&str>,
    symbol_filter: Option<&str>,
    start_ts: Option<NaiveDateTime>,
    end_ts: Option<NaiveDateTime>,
    rebuild: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    ensure_target_table(pool, spec).await?;

    println!(
        "Building {} from {SOURCE_TABLE}: timeframe={} minutes={} symbols={} root={} symbol={} start={} end={} rebuild={}",
        spec.table_name,
        spec.label,
        spec.minutes,
        symbols.len(),
        root_symbol.unwrap_or("ALL"),
        symbol_filter.unwrap_or("ALL"),
        start_ts
            .map(|value| value.to_string())
            .unwrap_or_else(|| "BEGINNING".to_string()),
        end_ts
            .map(|value| value.to_string())
            .unwrap_or_else(|| "OPEN".to_string()),
        rebuild
    );

    let mut total_affected = 0_u64;
    let mut total_deleted = 0_u64;

    for (index, symbol) in symbols.iter().enumerate() {
        let deleted = if rebuild {
            clear_symbol(pool, spec, symbol, start_ts, end_ts).await?
        } else {
            0
        };
        total_deleted += deleted;

        let affected = rebuild_symbol(pool, spec, symbol, start_ts, end_ts).await?;
        total_affected += affected;

        println!(
            "[{}/{}] {} {} affected={} deleted={}",
            index + 1,
            symbols.len(),
            spec.label,
            symbol,
            affected,
            deleted
        );
    }

    println!(
        "Done. table={} timeframe={} symbols={} affected={} deleted={}",
        spec.table_name,
        spec.label,
        symbols.len(),
        total_affected,
        total_deleted
    );

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url = database_url_from_env()?;
    let pool = MySqlPool::connect(&database_url).await?;
    let root_symbol = env::var("ABCD_FUTURES_ROOT").ok();
    let symbol = env::var("ABCD_SYMBOL").ok();
    let symbol_limit = env_i64("ABCD_SYMBOL_LIMIT", DEFAULT_SYMBOL_LIMIT)?;
    let start_ts = env_datetime("ABCD_START_TS")?;
    let end_ts = env_datetime("ABCD_END_TS")?;
    let rebuild =
        env_flag("ABCD_REBUILD_TIMEFRAME_CANDLES") || env_flag("ABCD_REBUILD_AGGREGATE_CANDLES");
    let timeframes = requested_timeframes()?;

    let symbols = selected_symbols(
        &pool,
        root_symbol.as_deref(),
        symbol.as_deref(),
        symbol_limit,
    )
    .await?;

    for spec in timeframes {
        build_timeframe(
            &pool,
            spec,
            &symbols,
            root_symbol.as_deref(),
            symbol.as_deref(),
            start_ts,
            end_ts,
            rebuild,
        )
        .await?;
    }

    Ok(())
}
