use std::env;

use chrono::NaiveDateTime;
use sqlx::mysql::MySqlPool;

const DEFAULT_SYMBOL_LIMIT: i64 = i64::MAX;
const SOURCE_TABLE: &str = "futures_contract_1m_candles";
const TARGET_TABLE: &str = "futures_contract_3m_candles";

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

async fn ensure_target_table(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {TARGET_TABLE} (
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
            INDEX idx_futures_3m_root_ts (root_symbol, ts_utc),
            INDEX idx_futures_3m_ts (ts_utc)
        )
        "#
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
    symbol: &str,
    start_ts: Option<NaiveDateTime>,
    end_ts: Option<NaiveDateTime>,
) -> Result<u64, sqlx::Error> {
    let mut sql = format!(
        r#"
        INSERT INTO {TARGET_TABLE} (
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
            MAX(bucketed.root_symbol) AS root_symbol,
            bucketed.symbol,
            bucketed.bucket_ts AS ts_utc,
            CAST(SUBSTRING_INDEX(GROUP_CONCAT(bucketed.open ORDER BY bucketed.ts_utc ASC), ',', 1) AS DECIMAL(18,8)) AS open,
            MAX(bucketed.high) AS high,
            MIN(bucketed.low) AS low,
            CAST(SUBSTRING_INDEX(GROUP_CONCAT(bucketed.close ORDER BY bucketed.ts_utc DESC), ',', 1) AS DECIMAL(18,8)) AS close,
            COALESCE(SUM(bucketed.volume), 0) AS volume,
            COUNT(*) AS source_count,
            MIN(bucketed.ts_utc) AS source_first_ts,
            MAX(bucketed.ts_utc) AS source_last_ts
        FROM (
            SELECT
                root_symbol,
                symbol,
                ts_utc,
                open,
                high,
                low,
                close,
                volume,
                TIMESTAMPADD(
                    MINUTE,
                    FLOOR(TIMESTAMPDIFF(MINUTE, '1970-01-01 00:00:00', ts_utc) / 3) * 3,
                    '1970-01-01 00:00:00'
                ) AS bucket_ts
            FROM {SOURCE_TABLE}
            WHERE symbol = ?
        "#
    );

    if start_ts.is_some() {
        sql.push_str(" AND ts_utc >= ?");
    }
    if end_ts.is_some() {
        sql.push_str(" AND ts_utc < ?");
    }

    sql.push_str(
        r#"
        ) bucketed
        GROUP BY bucketed.symbol, bucketed.bucket_ts
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
        "#,
    );

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
    symbol: &str,
    start_ts: Option<NaiveDateTime>,
    end_ts: Option<NaiveDateTime>,
) -> Result<u64, sqlx::Error> {
    let mut sql = format!("DELETE FROM {TARGET_TABLE} WHERE symbol = ?");
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
    let rebuild = env_flag("ABCD_REBUILD_3M_CANDLES");

    ensure_target_table(&pool).await?;

    let symbols = selected_symbols(
        &pool,
        root_symbol.as_deref(),
        symbol.as_deref(),
        symbol_limit,
    )
    .await?;

    println!(
        "Building {TARGET_TABLE} from {SOURCE_TABLE}: symbols={} root={} symbol={} start={} end={} rebuild={}",
        symbols.len(),
        root_symbol.as_deref().unwrap_or("ALL"),
        symbol.as_deref().unwrap_or("ALL"),
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
        if rebuild {
            let deleted = clear_symbol(&pool, symbol, start_ts, end_ts).await?;
            total_deleted += deleted;
        }

        let affected = rebuild_symbol(&pool, symbol, start_ts, end_ts).await?;
        total_affected += affected;

        println!(
            "[{}/{}] {} affected={} deleted={}",
            index + 1,
            symbols.len(),
            symbol,
            affected,
            if rebuild { total_deleted } else { 0 }
        );
    }

    println!(
        "Done. table={} symbols={} affected={} deleted={}",
        TARGET_TABLE,
        symbols.len(),
        total_affected,
        total_deleted
    );

    Ok(())
}
