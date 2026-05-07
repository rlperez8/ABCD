use std::collections::BTreeMap;
use std::env;
use std::time::Instant;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use sqlx::mysql::{MySqlPool, MySqlRow};
use sqlx::Row;

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

const ENGINE_STORAGE_TABLES: [&str; 2] = ["pattern_setups", "pattern_outcomes_prop"];
const ROLLUP_STORAGE_TABLES: [&str; 4] = [
    "prop_strategy_family_summary",
    "prop_strategy_family_yearly",
    "prop_strategy_contract_week_summary",
    "prop_strategy_family_weekly_cadence",
];

#[derive(Clone)]
struct TableSummary {
    table_name: String,
    category: &'static str,
    exact_rows: i64,
    data_bytes: u64,
    index_bytes: u64,
    total_bytes: u64,
    bytes_per_row: f64,
}

#[derive(Default)]
struct SetupRootAggregate {
    contract_count: i64,
    setup_count: i64,
    first_d_date: Option<String>,
    last_d_date: Option<String>,
}

#[derive(Default)]
struct SetupTimeframePatternAggregate {
    setup_count: i64,
    first_d_date: Option<String>,
    last_d_date: Option<String>,
}

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

fn read_i64_or_zero(row: &MySqlRow, column: &str) -> i64 {
    row.try_get::<i64, _>(column)
        .or_else(|_| row.try_get::<u64, _>(column).map(|value| value as i64))
        .or_else(|_| {
            row.try_get::<Decimal, _>(column)
                .map(|value| value.to_i64().unwrap_or(0))
        })
        .unwrap_or(0)
}

fn futures_storage_root(symbol: &str) -> String {
    let uppercase = symbol.trim().to_uppercase();
    let chars = uppercase.chars().collect::<Vec<_>>();
    let month_codes = ['F', 'G', 'H', 'J', 'K', 'M', 'N', 'Q', 'U', 'V', 'X', 'Z'];

    if chars.len() >= 3
        && chars[chars.len() - 2].is_ascii_digit()
        && chars[chars.len() - 1].is_ascii_digit()
        && month_codes.contains(&chars[chars.len() - 3])
    {
        return chars[..chars.len() - 3].iter().collect();
    }

    if chars.len() >= 2
        && chars[chars.len() - 1].is_ascii_digit()
        && month_codes.contains(&chars[chars.len() - 2])
    {
        return chars[..chars.len() - 2].iter().collect();
    }

    if uppercase.len() >= 2 && uppercase.starts_with('6') {
        return uppercase.chars().take(2).collect();
    }

    let root = uppercase
        .chars()
        .take_while(|ch| ch.is_ascii_alphabetic())
        .collect::<String>();

    if root.is_empty() {
        uppercase
    } else {
        root
    }
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

async fn table_column_exists(
    pool: &MySqlPool,
    table_name: &str,
    column_name: &str,
) -> Result<bool, sqlx::Error> {
    let count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM INFORMATION_SCHEMA.COLUMNS
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_NAME = ?
          AND COLUMN_NAME = ?
        "#,
    )
    .bind(table_name)
    .bind(column_name)
    .fetch_one(pool)
    .await?;

    Ok(count > 0)
}

async fn ensure_summary_tables(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS storage_table_summary (
            table_name VARCHAR(128) NOT NULL,
            category VARCHAR(32) NOT NULL,
            exact_rows BIGINT NOT NULL DEFAULT 0,
            data_bytes BIGINT UNSIGNED NOT NULL DEFAULT 0,
            index_bytes BIGINT UNSIGNED NOT NULL DEFAULT 0,
            total_bytes BIGINT UNSIGNED NOT NULL DEFAULT 0,
            bytes_per_row DOUBLE NOT NULL DEFAULT 0,
            refreshed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (table_name),
            INDEX idx_storage_table_summary_category (category)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS storage_futures_root_summary (
            table_name VARCHAR(128) NOT NULL,
            root_symbol VARCHAR(32) NOT NULL,
            contract_count BIGINT NOT NULL DEFAULT 0,
            candle_count BIGINT NOT NULL DEFAULT 0,
            first_ts VARCHAR(32) NULL,
            last_ts VARCHAR(32) NULL,
            estimated_bytes BIGINT UNSIGNED NOT NULL DEFAULT 0,
            refreshed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (table_name, root_symbol),
            INDEX idx_storage_futures_root_rows (candle_count)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS storage_pattern_setup_root_summary (
            table_name VARCHAR(128) NOT NULL,
            root_symbol VARCHAR(32) NOT NULL,
            contract_count BIGINT NOT NULL DEFAULT 0,
            setup_count BIGINT NOT NULL DEFAULT 0,
            first_d_date VARCHAR(32) NULL,
            last_d_date VARCHAR(32) NULL,
            estimated_bytes BIGINT UNSIGNED NOT NULL DEFAULT 0,
            refreshed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (table_name, root_symbol),
            INDEX idx_storage_setup_root_rows (setup_count)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS storage_pattern_setup_market_summary (
            table_name VARCHAR(128) NOT NULL,
            market VARCHAR(32) NOT NULL,
            harmonic_type VARCHAR(32) NOT NULL,
            setup_count BIGINT NOT NULL DEFAULT 0,
            estimated_bytes BIGINT UNSIGNED NOT NULL DEFAULT 0,
            refreshed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (table_name, market, harmonic_type),
            INDEX idx_storage_setup_market_rows (setup_count)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS storage_pattern_setup_contract_summary (
            table_name VARCHAR(128) NOT NULL,
            root_symbol VARCHAR(32) NOT NULL,
            contract_symbol VARCHAR(64) NOT NULL,
            source_timeframe VARCHAR(16) NOT NULL,
            setup_count BIGINT NOT NULL DEFAULT 0,
            first_d_date VARCHAR(32) NULL,
            last_d_date VARCHAR(32) NULL,
            estimated_bytes BIGINT UNSIGNED NOT NULL DEFAULT 0,
            refreshed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (table_name, root_symbol, contract_symbol, source_timeframe),
            INDEX idx_storage_setup_contract_rows (setup_count),
            INDEX idx_storage_setup_contract_timeframe (source_timeframe, setup_count)
        )
        "#,
    )
    .execute(pool)
    .await?;

    if table_exists(pool, "storage_pattern_setup_timeframe_pattern_summary").await?
        && !table_column_exists(
            pool,
            "storage_pattern_setup_timeframe_pattern_summary",
            "contract_symbol",
        )
        .await?
    {
        sqlx::query("DROP TABLE IF EXISTS storage_pattern_setup_timeframe_pattern_summary")
            .execute(pool)
            .await?;
    }

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS storage_pattern_setup_timeframe_pattern_summary (
            table_name VARCHAR(128) NOT NULL,
            root_symbol VARCHAR(32) NOT NULL,
            contract_symbol VARCHAR(64) NOT NULL,
            source_timeframe VARCHAR(16) NOT NULL,
            market VARCHAR(32) NOT NULL,
            harmonic_type VARCHAR(32) NOT NULL,
            setup_count BIGINT NOT NULL DEFAULT 0,
            first_d_date VARCHAR(32) NULL,
            last_d_date VARCHAR(32) NULL,
            estimated_bytes BIGINT UNSIGNED NOT NULL DEFAULT 0,
            refreshed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (table_name, root_symbol, contract_symbol, source_timeframe, market, harmonic_type),
            INDEX idx_storage_setup_tf_pattern_rows (setup_count),
            INDEX idx_storage_setup_tf_pattern_symbol (root_symbol, contract_symbol, source_timeframe, setup_count)
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn fetch_table_summary(
    pool: &MySqlPool,
    table_name: &str,
    category: &'static str,
) -> Result<Option<TableSummary>, sqlx::Error> {
    if !table_exists(pool, table_name).await? {
        return Ok(None);
    }

    let exact_rows_sql = format!("SELECT COUNT(*) AS exact_rows FROM {table_name}");
    let exact_rows = sqlx::query_scalar::<_, i64>(&exact_rows_sql)
        .fetch_one(pool)
        .await?;

    let stats = sqlx::query(
        r#"
        SELECT
            CAST(COALESCE(data_length, 0) AS SIGNED) AS data_length,
            CAST(COALESCE(index_length, 0) AS SIGNED) AS index_length
        FROM INFORMATION_SCHEMA.TABLES
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_NAME = ?
        "#,
    )
    .bind(table_name)
    .fetch_one(pool)
    .await?;

    let data_bytes = read_i64_or_zero(&stats, "data_length").max(0) as u64;
    let index_bytes = read_i64_or_zero(&stats, "index_length").max(0) as u64;
    let total_bytes = data_bytes + index_bytes;
    let bytes_per_row = if exact_rows > 0 {
        total_bytes as f64 / exact_rows as f64
    } else {
        0.0
    };

    Ok(Some(TableSummary {
        table_name: table_name.to_string(),
        category,
        exact_rows,
        data_bytes,
        index_bytes,
        total_bytes,
        bytes_per_row,
    }))
}

async fn save_table_summary(pool: &MySqlPool, summary: &TableSummary) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO storage_table_summary (
            table_name, category, exact_rows, data_bytes, index_bytes,
            total_bytes, bytes_per_row, refreshed_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
        ON DUPLICATE KEY UPDATE
            category = VALUES(category),
            exact_rows = VALUES(exact_rows),
            data_bytes = VALUES(data_bytes),
            index_bytes = VALUES(index_bytes),
            total_bytes = VALUES(total_bytes),
            bytes_per_row = VALUES(bytes_per_row),
            refreshed_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(&summary.table_name)
    .bind(summary.category)
    .bind(summary.exact_rows)
    .bind(summary.data_bytes as i64)
    .bind(summary.index_bytes as i64)
    .bind(summary.total_bytes as i64)
    .bind(summary.bytes_per_row)
    .execute(pool)
    .await?;

    Ok(())
}

async fn refresh_table_summaries(
    pool: &MySqlPool,
) -> Result<BTreeMap<String, TableSummary>, sqlx::Error> {
    let mut summaries = BTreeMap::new();
    let groups = [
        ("raw_candles", CANDLE_STORAGE_TABLES.as_slice()),
        ("engine_output", ENGINE_STORAGE_TABLES.as_slice()),
        ("rollups", ROLLUP_STORAGE_TABLES.as_slice()),
    ];

    for (category, tables) in groups {
        for table_name in tables {
            let started_at = Instant::now();
            match fetch_table_summary(pool, table_name, category).await? {
                Some(summary) => {
                    save_table_summary(pool, &summary).await?;
                    println!(
                        "saved table summary: {} rows={} size={} bytes ({:.1}s)",
                        summary.table_name,
                        summary.exact_rows,
                        summary.total_bytes,
                        started_at.elapsed().as_secs_f64()
                    );
                    summaries.insert(summary.table_name.clone(), summary);
                }
                None => {
                    println!("skipped missing table: {table_name}");
                }
            }
        }
    }

    Ok(summaries)
}

async fn refresh_futures_root_summary(
    pool: &MySqlPool,
    summary: &TableSummary,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM storage_futures_root_summary WHERE table_name = ?")
        .bind(&summary.table_name)
        .execute(pool)
        .await?;

    let sql = format!(
        r#"
        SELECT
            root_symbol,
            COUNT(DISTINCT symbol) AS contract_count,
            COUNT(*) AS candle_count,
            CAST(MIN(ts_utc) AS CHAR) AS first_ts,
            CAST(MAX(ts_utc) AS CHAR) AS last_ts
        FROM {}
        GROUP BY root_symbol
        ORDER BY candle_count DESC, root_symbol ASC
        "#,
        summary.table_name
    );

    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    for row in rows {
        let candle_count = read_i64_or_zero(&row, "candle_count");
        let estimated_bytes = (candle_count as f64 * summary.bytes_per_row)
            .round()
            .max(0.0) as i64;

        sqlx::query(
            r#"
            INSERT INTO storage_futures_root_summary (
                table_name, root_symbol, contract_count, candle_count,
                first_ts, last_ts, estimated_bytes, refreshed_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(&summary.table_name)
        .bind(row.try_get::<String, _>("root_symbol").unwrap_or_default())
        .bind(read_i64_or_zero(&row, "contract_count"))
        .bind(candle_count)
        .bind(row.try_get::<Option<String>, _>("first_ts").ok().flatten())
        .bind(row.try_get::<Option<String>, _>("last_ts").ok().flatten())
        .bind(estimated_bytes)
        .execute(pool)
        .await?;
    }

    println!("saved futures root summary: {}", summary.table_name);
    Ok(())
}

async fn refresh_pattern_setup_root_summary(
    pool: &MySqlPool,
    summary: &TableSummary,
) -> Result<(), sqlx::Error> {
    if !table_exists(pool, &summary.table_name).await? {
        return Ok(());
    }

    sqlx::query("DELETE FROM storage_pattern_setup_root_summary WHERE table_name = ?")
        .bind(&summary.table_name)
        .execute(pool)
        .await?;

    let has_root_symbol = table_column_exists(pool, &summary.table_name, "root_symbol").await?;
    let has_contract_symbol =
        table_column_exists(pool, &summary.table_name, "contract_symbol").await?;
    let root_projection = if has_root_symbol {
        "COALESCE(NULLIF(root_symbol, ''), '')"
    } else {
        "''"
    };
    let contract_projection = if has_contract_symbol {
        "COALESCE(NULLIF(contract_symbol, ''), symbol)"
    } else {
        "symbol"
    };
    let sql = format!(
        r#"
        SELECT
            {root_projection} AS stored_root_symbol,
            {contract_projection} AS contract_symbol,
            COUNT(*) AS setup_count,
            CAST(MIN(d_date) AS CHAR) AS first_d_date,
            CAST(MAX(d_date) AS CHAR) AS last_d_date
        FROM {}
        GROUP BY {root_projection}, {contract_projection}
        ORDER BY setup_count DESC, contract_symbol ASC
        "#,
        summary.table_name,
        root_projection = root_projection,
        contract_projection = contract_projection,
    );

    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    let mut by_root = BTreeMap::<String, SetupRootAggregate>::new();
    for row in rows {
        let contract_symbol = row
            .try_get::<String, _>("contract_symbol")
            .unwrap_or_default();
        let stored_root_symbol = row
            .try_get::<String, _>("stored_root_symbol")
            .unwrap_or_default();
        let root_symbol = if stored_root_symbol.trim().is_empty() {
            futures_storage_root(&contract_symbol)
        } else {
            stored_root_symbol
        };
        let setup_count = read_i64_or_zero(&row, "setup_count");
        let first_d_date = row
            .try_get::<Option<String>, _>("first_d_date")
            .ok()
            .flatten();
        let last_d_date = row
            .try_get::<Option<String>, _>("last_d_date")
            .ok()
            .flatten();
        let aggregate = by_root.entry(root_symbol).or_default();

        aggregate.contract_count += 1;
        aggregate.setup_count += setup_count;
        if let Some(first_d_date) = first_d_date {
            if aggregate
                .first_d_date
                .as_ref()
                .map(|current| first_d_date < *current)
                .unwrap_or(true)
            {
                aggregate.first_d_date = Some(first_d_date);
            }
        }
        if let Some(last_d_date) = last_d_date {
            if aggregate
                .last_d_date
                .as_ref()
                .map(|current| last_d_date > *current)
                .unwrap_or(true)
            {
                aggregate.last_d_date = Some(last_d_date);
            }
        }
    }

    for (root_symbol, aggregate) in by_root {
        let estimated_bytes = (aggregate.setup_count as f64 * summary.bytes_per_row)
            .round()
            .max(0.0) as i64;

        sqlx::query(
            r#"
            INSERT INTO storage_pattern_setup_root_summary (
                table_name, root_symbol, contract_count, setup_count,
                first_d_date, last_d_date, estimated_bytes, refreshed_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(&summary.table_name)
        .bind(root_symbol)
        .bind(aggregate.contract_count)
        .bind(aggregate.setup_count)
        .bind(aggregate.first_d_date)
        .bind(aggregate.last_d_date)
        .bind(estimated_bytes)
        .execute(pool)
        .await?;
    }

    println!("saved setup root summary: {}", summary.table_name);
    Ok(())
}

async fn refresh_pattern_setup_market_summary(
    pool: &MySqlPool,
    summary: &TableSummary,
) -> Result<(), sqlx::Error> {
    if !table_exists(pool, &summary.table_name).await? {
        return Ok(());
    }

    sqlx::query("DELETE FROM storage_pattern_setup_market_summary WHERE table_name = ?")
        .bind(&summary.table_name)
        .execute(pool)
        .await?;

    let sql = format!(
        r#"
        SELECT
            market,
            harmonic_type,
            COUNT(*) AS setup_count
        FROM {}
        GROUP BY market, harmonic_type
        ORDER BY setup_count DESC, market ASC, harmonic_type ASC
        "#,
        summary.table_name
    );

    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    for row in rows {
        let setup_count = read_i64_or_zero(&row, "setup_count");
        let estimated_bytes = (setup_count as f64 * summary.bytes_per_row)
            .round()
            .max(0.0) as i64;

        sqlx::query(
            r#"
            INSERT INTO storage_pattern_setup_market_summary (
                table_name, market, harmonic_type, setup_count,
                estimated_bytes, refreshed_at
            )
            VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(&summary.table_name)
        .bind(row.try_get::<String, _>("market").unwrap_or_default())
        .bind(
            row.try_get::<String, _>("harmonic_type")
                .unwrap_or_default(),
        )
        .bind(setup_count)
        .bind(estimated_bytes)
        .execute(pool)
        .await?;
    }

    println!("saved setup market summary: {}", summary.table_name);
    Ok(())
}

async fn refresh_pattern_setup_contract_summary(
    pool: &MySqlPool,
    summary: &TableSummary,
) -> Result<(), sqlx::Error> {
    if !table_exists(pool, &summary.table_name).await? {
        return Ok(());
    }

    sqlx::query("DELETE FROM storage_pattern_setup_contract_summary WHERE table_name = ?")
        .bind(&summary.table_name)
        .execute(pool)
        .await?;

    let has_root_symbol = table_column_exists(pool, &summary.table_name, "root_symbol").await?;
    let has_contract_symbol =
        table_column_exists(pool, &summary.table_name, "contract_symbol").await?;
    let has_source_timeframe =
        table_column_exists(pool, &summary.table_name, "source_timeframe").await?;
    let root_projection = if has_root_symbol {
        "COALESCE(NULLIF(root_symbol, ''), '')"
    } else {
        "''"
    };
    let contract_projection = if has_contract_symbol {
        "COALESCE(NULLIF(contract_symbol, ''), symbol)"
    } else {
        "symbol"
    };
    let timeframe_projection = if has_source_timeframe {
        "COALESCE(NULLIF(source_timeframe, ''), 'unknown')"
    } else {
        "'unknown'"
    };
    let sql = format!(
        r#"
        SELECT
            {root_projection} AS stored_root_symbol,
            {contract_projection} AS contract_symbol,
            {timeframe_projection} AS source_timeframe,
            COUNT(*) AS setup_count,
            CAST(MIN(d_date) AS CHAR) AS first_d_date,
            CAST(MAX(d_date) AS CHAR) AS last_d_date
        FROM {}
        GROUP BY {root_projection}, {contract_projection}, {timeframe_projection}
        ORDER BY setup_count DESC, contract_symbol ASC, source_timeframe ASC
        "#,
        summary.table_name,
        root_projection = root_projection,
        contract_projection = contract_projection,
        timeframe_projection = timeframe_projection,
    );

    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    for row in rows {
        let contract_symbol = row
            .try_get::<String, _>("contract_symbol")
            .unwrap_or_default();
        let stored_root_symbol = row
            .try_get::<String, _>("stored_root_symbol")
            .unwrap_or_default();
        let root_symbol = if stored_root_symbol.trim().is_empty() {
            futures_storage_root(&contract_symbol)
        } else {
            stored_root_symbol
        };
        let setup_count = read_i64_or_zero(&row, "setup_count");
        let estimated_bytes = (setup_count as f64 * summary.bytes_per_row)
            .round()
            .max(0.0) as i64;

        sqlx::query(
            r#"
            INSERT INTO storage_pattern_setup_contract_summary (
                table_name, root_symbol, contract_symbol, source_timeframe,
                setup_count, first_d_date, last_d_date, estimated_bytes, refreshed_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(&summary.table_name)
        .bind(root_symbol)
        .bind(contract_symbol)
        .bind(
            row.try_get::<String, _>("source_timeframe")
                .unwrap_or_else(|_| "unknown".to_string()),
        )
        .bind(setup_count)
        .bind(
            row.try_get::<Option<String>, _>("first_d_date")
                .ok()
                .flatten(),
        )
        .bind(
            row.try_get::<Option<String>, _>("last_d_date")
                .ok()
                .flatten(),
        )
        .bind(estimated_bytes)
        .execute(pool)
        .await?;
    }

    println!("saved setup contract summary: {}", summary.table_name);
    Ok(())
}

async fn refresh_pattern_setup_timeframe_pattern_summary(
    pool: &MySqlPool,
    summary: &TableSummary,
) -> Result<(), sqlx::Error> {
    if !table_exists(pool, &summary.table_name).await? {
        return Ok(());
    }

    sqlx::query(
        "DELETE FROM storage_pattern_setup_timeframe_pattern_summary WHERE table_name = ?",
    )
    .bind(&summary.table_name)
    .execute(pool)
    .await?;

    let has_root_symbol = table_column_exists(pool, &summary.table_name, "root_symbol").await?;
    let has_contract_symbol =
        table_column_exists(pool, &summary.table_name, "contract_symbol").await?;
    let has_source_timeframe =
        table_column_exists(pool, &summary.table_name, "source_timeframe").await?;
    let root_projection = if has_root_symbol {
        "COALESCE(NULLIF(root_symbol, ''), '')"
    } else {
        "''"
    };
    let contract_projection = if has_contract_symbol {
        "COALESCE(NULLIF(contract_symbol, ''), symbol)"
    } else {
        "symbol"
    };
    let timeframe_projection = if has_source_timeframe {
        "COALESCE(NULLIF(source_timeframe, ''), 'unknown')"
    } else {
        "'unknown'"
    };
    let sql = format!(
        r#"
        SELECT
            {root_projection} AS stored_root_symbol,
            {contract_projection} AS contract_symbol,
            {timeframe_projection} AS source_timeframe,
            COALESCE(NULLIF(market, ''), 'Unknown') AS market,
            COALESCE(NULLIF(harmonic_type, ''), 'Unknown') AS harmonic_type,
            COUNT(*) AS setup_count,
            CAST(MIN(d_date) AS CHAR) AS first_d_date,
            CAST(MAX(d_date) AS CHAR) AS last_d_date
        FROM {}
        GROUP BY
            {root_projection},
            {contract_projection},
            {timeframe_projection},
            COALESCE(NULLIF(market, ''), 'Unknown'),
            COALESCE(NULLIF(harmonic_type, ''), 'Unknown')
        ORDER BY setup_count DESC, source_timeframe ASC, market ASC, harmonic_type ASC
        "#,
        summary.table_name,
        root_projection = root_projection,
        contract_projection = contract_projection,
        timeframe_projection = timeframe_projection,
    );

    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    let mut by_pattern =
        BTreeMap::<(String, String, String, String, String), SetupTimeframePatternAggregate>::new();

    for row in rows {
        let contract_symbol = row
            .try_get::<String, _>("contract_symbol")
            .unwrap_or_default();
        let stored_root_symbol = row
            .try_get::<String, _>("stored_root_symbol")
            .unwrap_or_default();
        let root_symbol = if stored_root_symbol.trim().is_empty() {
            futures_storage_root(&contract_symbol)
        } else {
            stored_root_symbol
        };
        let source_timeframe = row
            .try_get::<String, _>("source_timeframe")
            .unwrap_or_else(|_| "unknown".to_string());
        let market = row.try_get::<String, _>("market").unwrap_or_default();
        let harmonic_type = row
            .try_get::<String, _>("harmonic_type")
            .unwrap_or_default();
        let setup_count = read_i64_or_zero(&row, "setup_count");
        let first_d_date = row
            .try_get::<Option<String>, _>("first_d_date")
            .ok()
            .flatten();
        let last_d_date = row
            .try_get::<Option<String>, _>("last_d_date")
            .ok()
            .flatten();

        let aggregate = by_pattern
            .entry((
                root_symbol,
                contract_symbol,
                source_timeframe,
                market,
                harmonic_type,
            ))
            .or_default();
        aggregate.setup_count += setup_count;
        if let Some(first_d_date) = first_d_date {
            if aggregate
                .first_d_date
                .as_ref()
                .map(|current| first_d_date < *current)
                .unwrap_or(true)
            {
                aggregate.first_d_date = Some(first_d_date);
            }
        }
        if let Some(last_d_date) = last_d_date {
            if aggregate
                .last_d_date
                .as_ref()
                .map(|current| last_d_date > *current)
                .unwrap_or(true)
            {
                aggregate.last_d_date = Some(last_d_date);
            }
        }
    }

    for (
        (root_symbol, contract_symbol, source_timeframe, market, harmonic_type),
        aggregate,
    ) in by_pattern
    {
        let estimated_bytes = (aggregate.setup_count as f64 * summary.bytes_per_row)
            .round()
            .max(0.0) as i64;

        sqlx::query(
            r#"
            INSERT INTO storage_pattern_setup_timeframe_pattern_summary (
                table_name, root_symbol, contract_symbol, source_timeframe,
                market, harmonic_type, setup_count, first_d_date, last_d_date,
                estimated_bytes, refreshed_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(&summary.table_name)
        .bind(root_symbol)
        .bind(contract_symbol)
        .bind(source_timeframe)
        .bind(market)
        .bind(harmonic_type)
        .bind(aggregate.setup_count)
        .bind(aggregate.first_d_date)
        .bind(aggregate.last_d_date)
        .bind(estimated_bytes)
        .execute(pool)
        .await?;
    }

    println!(
        "saved setup timeframe pattern summary: {}",
        summary.table_name
    );
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let started_at = Instant::now();
    let pool = MySqlPool::connect(&database_url_from_env()?).await?;
    ensure_summary_tables(&pool).await?;

    let summaries = refresh_table_summaries(&pool).await?;

    for table_name in CANDLE_STORAGE_TABLES {
        if let Some(summary) = summaries.get(table_name) {
            let started_at = Instant::now();
            refresh_futures_root_summary(&pool, summary).await?;
            println!(
                "futures breakdown finished: {} ({:.1}s)",
                table_name,
                started_at.elapsed().as_secs_f64()
            );
        }
    }

    if let Some(summary) = summaries.get("pattern_setups") {
        let started_at = Instant::now();
        refresh_pattern_setup_root_summary(&pool, summary).await?;
        println!(
            "setup root breakdown finished ({:.1}s)",
            started_at.elapsed().as_secs_f64()
        );

        let started_at = Instant::now();
        refresh_pattern_setup_market_summary(&pool, summary).await?;
        println!(
            "setup market breakdown finished ({:.1}s)",
            started_at.elapsed().as_secs_f64()
        );

        let started_at = Instant::now();
        refresh_pattern_setup_contract_summary(&pool, summary).await?;
        println!(
            "setup contract breakdown finished ({:.1}s)",
            started_at.elapsed().as_secs_f64()
        );

        let started_at = Instant::now();
        refresh_pattern_setup_timeframe_pattern_summary(&pool, summary).await?;
        println!(
            "setup timeframe pattern breakdown finished ({:.1}s)",
            started_at.elapsed().as_secs_f64()
        );
    }

    println!(
        "storage summary refresh complete ({:.1}s)",
        started_at.elapsed().as_secs_f64()
    );

    Ok(())
}
