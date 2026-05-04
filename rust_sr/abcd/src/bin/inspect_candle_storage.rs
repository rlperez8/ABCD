use std::env;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use sqlx::mysql::MySqlPool;
use sqlx::Row;

const CANDLE_TABLES: [&str; 2] = ["futures_contract_1m_candles", "futures_contract_3m_candles"];

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

fn format_bytes(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit_index = 0usize;

    while value >= 1024.0 && unit_index < units.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{bytes} {}", units[unit_index])
    } else {
        format!("{value:.2} {}", units[unit_index])
    }
}

fn format_number(value: i64) -> String {
    let digits = value.abs().to_string();
    let mut formatted = String::new();

    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(ch);
    }

    let mut formatted: String = formatted.chars().rev().collect();
    if value < 0 {
        formatted.insert(0, '-');
    }
    formatted
}

fn read_i64_or_zero(row: &sqlx::mysql::MySqlRow, column: &str) -> i64 {
    row.try_get::<i64, _>(column)
        .or_else(|_| row.try_get::<u64, _>(column).map(|value| value as i64))
        .or_else(|_| {
            row.try_get::<Decimal, _>(column)
                .map(|value| value.to_i64().unwrap_or(0))
        })
        .unwrap_or(0)
}

async fn table_rows(pool: &MySqlPool, table_name: &str) -> i64 {
    let count_sql = format!("SELECT COUNT(*) AS count FROM {table_name}");
    sqlx::query_scalar::<_, i64>(&count_sql)
        .fetch_one(pool)
        .await
        .unwrap_or(0)
}

async fn table_storage_bytes(
    pool: &MySqlPool,
    table_name: &str,
) -> Result<(u64, u64), Box<dyn std::error::Error>> {
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
    .fetch_optional(pool)
    .await?;

    let data_bytes = stats
        .as_ref()
        .map(|row| read_i64_or_zero(row, "data_length").max(0) as u64)
        .unwrap_or(0);
    let index_bytes = stats
        .as_ref()
        .map(|row| read_i64_or_zero(row, "index_length").max(0) as u64)
        .unwrap_or(0);

    Ok((data_bytes, index_bytes))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let pool = MySqlPool::connect(&database_url_from_env()?).await?;

    println!("FUTURES CANDLE TABLE STORAGE");
    println!("table\texact_rows\tdata_size\tindex_size\ttotal_size\tbytes_per_row");

    let mut total_rows = 0_i64;
    let mut total_bytes = 0_u64;
    for table_name in CANDLE_TABLES {
        let exact_rows = table_rows(&pool, table_name).await;
        let (data_bytes, index_bytes) = table_storage_bytes(&pool, table_name).await?;
        let table_bytes = data_bytes + index_bytes;
        let bytes_per_row = if exact_rows > 0 {
            table_bytes as f64 / exact_rows as f64
        } else {
            0.0
        };

        total_rows += exact_rows;
        total_bytes += table_bytes;

        println!(
            "{table_name}\t{}\t{}\t{}\t{}\t{bytes_per_row:.2}",
            format_number(exact_rows),
            format_bytes(data_bytes),
            format_bytes(index_bytes),
            format_bytes(table_bytes)
        );
    }

    println!(
        "TOTAL\t{}\t\t\t{}\t{:.2}",
        format_number(total_rows),
        format_bytes(total_bytes),
        if total_rows > 0 {
            total_bytes as f64 / total_rows as f64
        } else {
            0.0
        }
    );

    println!();
    println!("FUTURES CANDLES BY ROOT SYMBOL");
    println!("table\troot_symbol\tcontracts\tcandles\tfirst_ts\tlast_ts\test_size");

    for table_name in ["futures_contract_1m_candles", "futures_contract_3m_candles"] {
        let (data_bytes, index_bytes) = table_storage_bytes(&pool, table_name).await?;
        let table_bytes = data_bytes + index_bytes;
        let table_rows = table_rows(&pool, table_name).await;
        let bytes_per_row = if table_rows > 0 {
            table_bytes as f64 / table_rows as f64
        } else {
            0.0
        };

        let sql = format!(
            r#"
            SELECT
                root_symbol,
                COUNT(DISTINCT symbol) AS contract_count,
                COUNT(*) AS candle_count,
                CAST(MIN(ts_utc) AS CHAR) AS first_ts,
                CAST(MAX(ts_utc) AS CHAR) AS last_ts
            FROM {table_name}
            GROUP BY root_symbol
            ORDER BY candle_count DESC, root_symbol ASC
            "#
        );

        let rows = sqlx::query(&sql).fetch_all(&pool).await?;
        for row in rows {
            let root_symbol: String = row.try_get("root_symbol")?;
            let contract_count: i64 = row.try_get("contract_count")?;
            let candle_count: i64 = row.try_get("candle_count")?;
            let first_ts: Option<String> = row.try_get("first_ts").ok();
            let last_ts: Option<String> = row.try_get("last_ts").ok();
            let estimated_bytes = (candle_count as f64 * bytes_per_row).round().max(0.0) as u64;

            println!(
                "{table_name}\t{root_symbol}\t{}\t{}\t{}\t{}\t{}",
                format_number(contract_count),
                format_number(candle_count),
                first_ts.unwrap_or_else(|| "-".to_string()),
                last_ts.unwrap_or_else(|| "-".to_string()),
                format_bytes(estimated_bytes)
            );
        }
    }

    Ok(())
}
