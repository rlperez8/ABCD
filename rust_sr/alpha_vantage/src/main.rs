use reqwest;
use csv;
use std::error::Error;
use std::env;
use chrono::NaiveDate;
use alpha_vantage::models::listing_status::ListingStatus;
use alpha_vantage::models::alpha_vantage::{AlphaVantage, AlphaVantageError};
use alpha_vantage::models::candle::Candle;

const DEFAULT_MIN_HISTORY_DAYS: usize = 365;

fn required_env(name: &str) -> Result<String, Box<dyn Error>> {
    env::var(name).map_err(|_| format!("Missing required environment variable: {}", name).into())
}

fn env_flag(name: &str) -> bool {
    match env::var(name) {
        Ok(value) => matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"),
        Err(_) => false,
    }
}

fn env_usize(name: &str) -> Result<Option<usize>, Box<dyn Error>> {
    match env::var(name) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }

            let parsed = trimmed.parse::<usize>()?;
            Ok(Some(parsed))
        }
        Err(_) => Ok(None),
    }
}

fn env_f64(name: &str) -> Result<Option<f64>, Box<dyn Error>> {
    match env::var(name) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }

            let parsed = trimmed.parse::<f64>()?;
            Ok(Some(parsed))
        }
        Err(_) => Ok(None),
    }
}

fn env_string(name: &str) -> Option<String> {
    match env::var(name) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Err(_) => None,
    }
}

async fn column_exists(
    pool: &sqlx::MySqlPool,
    table_name: &str,
    column_name: &str,
) -> Result<bool, sqlx::Error> {
    let exists = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM information_schema.columns
        WHERE table_schema = DATABASE()
          AND table_name = ?
          AND column_name = ?
        "#,
    )
    .bind(table_name)
    .bind(column_name)
    .fetch_one(pool)
    .await?;

    Ok(exists > 0)
}

async fn ensure_listing_status_schema(pool: &sqlx::MySqlPool) -> Result<(), sqlx::Error> {
    let required_columns = [
        (
            "ingest_note",
            "ALTER TABLE listing_status ADD COLUMN ingest_note TEXT NULL AFTER bugged",
        ),
        (
            "market_category",
            "ALTER TABLE listing_status ADD COLUMN market_category VARCHAR(64) NULL AFTER status",
        ),
        (
            "market_detail",
            "ALTER TABLE listing_status ADD COLUMN market_detail VARCHAR(255) NULL AFTER market_category",
        ),
        (
            "history_days",
            "ALTER TABLE listing_status ADD COLUMN history_days BIGINT NULL AFTER average_volume_30d",
        ),
        (
            "last_candle_date",
            "ALTER TABLE listing_status ADD COLUMN last_candle_date DATE NULL AFTER history_days",
        ),
        (
            "last_ingested_at",
            "ALTER TABLE listing_status ADD COLUMN last_ingested_at DATETIME NULL AFTER last_candle_date",
        ),
    ];

    for (column_name, alter_sql) in required_columns {
        if !column_exists(pool, "listing_status", column_name).await? {
            sqlx::query(alter_sql).execute(pool).await?;
        }
    }

    Ok(())
}

fn build_market_metadata(exchange: &str, asset_type: &str, symbol: &str) -> (String, String) {
    let exchange_trimmed = exchange.trim();
    let asset_type_trimmed = asset_type.trim();
    let symbol_trimmed = symbol.trim();
    let exchange_upper = exchange_trimmed.to_ascii_uppercase();
    let asset_type_upper = asset_type_trimmed.to_ascii_uppercase();

    let market_category = if asset_type_upper.contains("CRYPTO") || exchange_upper.contains("CRYPTO")
    {
        "Crypto".to_string()
    } else if asset_type_upper.contains("ETF") {
        "ETF".to_string()
    } else if exchange_upper.contains("NASDAQ") {
        "NASDAQ".to_string()
    } else if exchange_upper.contains("NYSE ARCA") {
        "NYSE Arca".to_string()
    } else if exchange_upper.contains("NYSE AMERICAN") || exchange_upper.contains("AMEX") {
        "NYSE American".to_string()
    } else if exchange_upper.contains("NYSE") {
        "NYSE".to_string()
    } else if exchange_upper.contains("OTC") {
        "OTC".to_string()
    } else if exchange_upper.contains("CBOE") {
        "CBOE".to_string()
    } else if exchange_upper.contains("BATS") {
        "BATS".to_string()
    } else if !asset_type_trimmed.is_empty() {
        asset_type_trimmed.to_string()
    } else if !exchange_trimmed.is_empty() {
        exchange_trimmed.to_string()
    } else {
        "Unknown".to_string()
    };

    let market_detail = format!(
        "asset_type={} | exchange={} | symbol={}",
        if asset_type_trimmed.is_empty() {
            "<unknown>"
        } else {
            asset_type_trimmed
        },
        if exchange_trimmed.is_empty() {
            "<unknown>"
        } else {
            exchange_trimmed
        },
        if symbol_trimmed.is_empty() {
            "<unknown>"
        } else {
            symbol_trimmed
        }
    );

    (market_category, market_detail)
}

fn history_span_days(candles: &[Candle]) -> Option<i64> {
    let oldest = candles.first().and_then(|candle| candle.date);
    let newest = candles.last().and_then(|candle| candle.date);

    match (oldest, newest) {
        (Some(oldest), Some(newest)) => Some((newest - oldest).num_days()),
        _ => None,
    }
}

fn recent_average_volume_from_candles(candles: &[Candle], lookback: usize) -> Option<f64> {
    let mut total_volume = 0f64;
    let mut count = 0usize;
    let start_index = candles.len().saturating_sub(lookback);

    for candle in candles.iter().skip(start_index) {
        if let Ok(volume) = candle.volume.trim().parse::<f64>() {
            total_volume += volume;
            count += 1;
        }
    }

    if count == 0 {
        None
    } else {
        Some(total_volume / count as f64)
    }
}

fn skip_reason_for_symbol(symbol: &str) -> Option<String> {
    if symbol.contains(':') {
        return Some("Skipped unsupported colon-formatted symbol from listing feed.".to_string());
    }

    if symbol.contains('/') {
        return Some("Skipped unsupported slash-formatted symbol from listing feed.".to_string());
    }

    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CandleIngestionMode {
    Full,
    Compact,
}

impl CandleIngestionMode {
    fn from_env() -> Self {
        match env::var("ALPHA_VANTAGE_INGEST_MODE") {
            Ok(value) if value.trim().eq_ignore_ascii_case("full") => Self::Full,
            _ => Self::Compact,
        }
    }

    fn output_size(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Compact => "compact",
        }
    }
}

pub async fn update_listing_ingest_state(
    pool: &sqlx::MySqlPool,
    symbol: &str,
    bugged: Option<bool>,
    average_volume_30d: Option<f64>,
    ingest_note: Option<&str>,
    history_days: Option<i64>,
    last_candle_date: Option<NaiveDate>,
) -> Result<(), sqlx::Error> {
    let last_candle_date = last_candle_date.map(|value| value.format("%Y-%m-%d").to_string());

    let query = r#"
        UPDATE listing_status
        SET bugged = ?,
            average_volume_30d = ?,
            ingest_note = ?,
            history_days = ?,
            last_candle_date = ?,
            last_ingested_at = NOW()
        WHERE symbol = ?
    "#;

    sqlx::query(query)
        .bind(bugged)
        .bind(average_volume_30d)
        .bind(ingest_note)
        .bind(history_days)
        .bind(last_candle_date)
        .bind(symbol)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn mark_as_bugged(
    pool: &sqlx::MySqlPool,
    symbol: &str,
    note: &str,
    history_days: Option<i64>,
    last_candle_date: Option<NaiveDate>,
) -> Result<(), sqlx::Error> {
    update_listing_ingest_state(
        pool,
        symbol,
        Some(true),
        None,
        Some(note),
        history_days,
        last_candle_date,
    )
    .await
}

pub async fn mark_as_not_bugged(
    pool: &sqlx::MySqlPool,
    symbol: &str,
    average_volume_30d: Option<f64>,
    note: &str,
    history_days: Option<i64>,
    last_candle_date: Option<NaiveDate>,
) -> Result<(), sqlx::Error> {
    update_listing_ingest_state(
        pool,
        symbol,
        Some(false),
        average_volume_30d,
        Some(note),
        history_days,
        last_candle_date,
    )
    .await
}

pub async fn above_500_volume(pool: &sqlx::MySqlPool) -> Result<Vec<String>, sqlx::Error> {

  

    let rows = sqlx::query_scalar::<_, String>(
        r#"
        SELECT symbol
        from listing_status
        WHERE average_volume_30d > 500000
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)

}

pub async fn get_symbols_missing_avg(pool: &sqlx::MySqlPool) -> Result<Vec<String>, sqlx::Error> {

    let rows = sqlx::query_scalar::<_, String>(
        r#"
        SELECT symbol
        FROM listing_status
        WHERE average_volume_30d IS NULL
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn update_listing_status(pool: &sqlx::MySqlPool,symbol: &str, average_volume_30d: f64,) -> Result<(), sqlx::Error> {

    let query = r#"
        UPDATE listing_status
        SET average_volume_30d = ?
        WHERE symbol = ?
    "#;

    sqlx::query(query)
        .bind(average_volume_30d)
        .bind(symbol)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn clear_average_volume(pool: &sqlx::MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE listing_status
        SET average_volume_30d = NULL
        "#
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn clear_listing_backfill_state(pool: &sqlx::MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE listing_status
        SET average_volume_30d = NULL,
            bugged = NULL,
            ingest_note = NULL,
            history_days = NULL,
            last_candle_date = NULL,
            last_ingested_at = NULL
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn fetch_listing_status() -> Result<Vec<ListingStatus>, Box<dyn Error>> {
    let api_key = required_env("ALPHA_VANTAGE_API_KEY")?;
    let url = format!(
        "https://www.alphavantage.co/query?function=LISTING_STATUS&apikey={}",
        api_key
    );

    // download CSV text
    let response = reqwest::get(url).await?.text().await?;

    // CSV reader
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)   // skip first row automatically
        .from_reader(response.as_bytes());

    let mut results = Vec::new();

    for record in rdr.deserialize() {
        let row: ListingStatus = record?;
        results.push(row);
    }

    Ok(results)
}

pub async fn clear_candles(pool: &sqlx::MySqlPool) -> Result<(), sqlx::Error> {
    if let Err(truncate_error) = sqlx::query("TRUNCATE TABLE candles").execute(pool).await {
        eprintln!(
            "TRUNCATE TABLE candles failed ({}). Falling back to DELETE FROM candles.",
            truncate_error
        );
        sqlx::query("DELETE FROM candles").execute(pool).await?;
    }

    Ok(())
}

pub async fn delete_symbol_candles(pool: &sqlx::MySqlPool, symbol: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM candles
        WHERE symbol = ?
        "#
    )
    .bind(symbol)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn symbol_candle_count(pool: &sqlx::MySqlPool, symbol: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM candles
        WHERE symbol = ?
        "#
    )
    .bind(symbol)
    .fetch_one(pool)
    .await
}

pub async fn latest_symbol_candle_date(
    pool: &sqlx::MySqlPool,
    symbol: &str,
) -> Result<Option<NaiveDate>, sqlx::Error> {
    let latest_date = sqlx::query_scalar::<_, Option<String>>(
        r#"
        SELECT DATE_FORMAT(MAX(date), '%Y-%m-%d')
        FROM candles
        WHERE symbol = ?
        "#
    )
    .bind(symbol)
    .fetch_one(pool)
    .await?;

    latest_date
        .map(|value| NaiveDate::parse_from_str(&value, "%Y-%m-%d"))
        .transpose()
        .map_err(|error| sqlx::Error::Protocol(format!("Invalid candle date {error}").into()))
}

pub async fn symbol_candle_history_days(
    pool: &sqlx::MySqlPool,
    symbol: &str,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar::<_, Option<i64>>(
        r#"
        SELECT DATEDIFF(MAX(date), MIN(date))
        FROM candles
        WHERE symbol = ?
        "#,
    )
    .bind(symbol)
    .fetch_one(pool)
    .await
}

pub async fn clear_listing_status(pool: &sqlx::MySqlPool) -> Result<(), sqlx::Error> {
    if let Err(truncate_error) = sqlx::query("TRUNCATE TABLE listing_status").execute(pool).await {
        eprintln!(
            "TRUNCATE TABLE listing_status failed ({}). Falling back to DELETE FROM listing_status.",
            truncate_error
        );
        sqlx::query("DELETE FROM listing_status").execute(pool).await?;
    }

    Ok(())
}

pub async fn insert_listing_statuses(
    pool: &sqlx::MySqlPool,
    rows: &[ListingStatus],
) -> Result<(), sqlx::Error> {
    let query = r#"
        INSERT INTO listing_status (
            symbol, name, exchange, assetType, ipoDate, delistingDate, status, market_category, market_detail
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE
            name = VALUES(name),
            exchange = VALUES(exchange),
            assetType = VALUES(assetType),
            ipoDate = VALUES(ipoDate),
            delistingDate = VALUES(delistingDate),
            status = VALUES(status),
            market_category = VALUES(market_category),
            market_detail = VALUES(market_detail)
    "#;

    for row in rows {
        let (market_category, market_detail) =
            build_market_metadata(&row.exchange, &row.asset_type, &row.symbol);

        sqlx::query(query)
            .bind(row.symbol.trim())
            .bind(row.name.trim())
            .bind(row.exchange.trim())
            .bind(row.asset_type.trim())
            .bind(row.ipo_date.trim())
            .bind(row.delisting_date.trim())
            .bind(row.status.trim())
            .bind(market_category)
            .bind(market_detail)
            .execute(pool)
            .await?;
    }

    Ok(())
}

pub async fn sync_listing_status(
    pool: &sqlx::MySqlPool,
    reset_listing_status: bool,
) -> Result<Vec<ListingStatus>, Box<dyn Error>> {
    let listing_status = fetch_listing_status().await?;

    if reset_listing_status {
        println!("Resetting listing_status table before reload.");
        clear_listing_status(pool).await?;
    }

    insert_listing_statuses(pool, &listing_status).await?;
    println!(
        "Inserted or updated {} listing_status rows.",
        listing_status.len()
    );

    Ok(listing_status)
}

pub async fn get_active_symbols_from_db(
    pool: &sqlx::MySqlPool,
    include_bugged: bool,
) -> Result<Vec<String>, sqlx::Error> {
    let sql = if include_bugged {
        r#"
        SELECT symbol
        FROM listing_status
        WHERE status = 'Active'
        "#
    } else {
        r#"
        SELECT symbol
        FROM listing_status
        WHERE status = 'Active'
          AND (bugged IS NULL OR bugged = false)
        "#
    };

    let mut symbols = sqlx::query_scalar::<_, String>(sql)
        .fetch_all(pool)
        .await?;

    symbols.sort();
    symbols.dedup();

    Ok(symbols)
}

pub async fn get_recent_average_volume(
    pool: &sqlx::MySqlPool,
    symbol: &str,
    lookback_candles: i64,
) -> Result<Option<f64>, sqlx::Error> {
    sqlx::query_scalar::<_, Option<f64>>(
        r#"
        SELECT CAST(AVG(recent.volume) AS DOUBLE)
        FROM (
            SELECT volume
            FROM candles
            WHERE symbol = ?
            ORDER BY date DESC
            LIMIT ?
        ) AS recent
        "#
    )
    .bind(symbol)
    .bind(lookback_candles)
    .fetch_one(pool)
    .await
}

pub async fn refresh_recent_average_volume(
    pool: &sqlx::MySqlPool,
    lookback_candles: usize,
    min_average_volume: Option<f64>,
) -> Result<(), Box<dyn Error>> {
    let symbols = get_active_symbols_from_db(pool, false).await?;
    let lookback_candles = lookback_candles as i64;
    let mut updated_symbols = 0usize;
    let mut missing_symbols = 0usize;
    let mut above_threshold = 0usize;

    clear_average_volume(pool).await?;

    for symbol in symbols {
        let average_volume_30d = get_recent_average_volume(pool, &symbol, lookback_candles).await?;

        match average_volume_30d {
            Some(value) => {
                update_listing_status(pool, &symbol, value).await?;
                updated_symbols += 1;

                if min_average_volume.is_some_and(|threshold| value >= threshold) {
                    above_threshold += 1;
                }
            }
            None => {
                missing_symbols += 1;
            }
        }
    }

    println!(
        "Recent average volume refresh complete. Updated: {}. Missing candles: {}. Above threshold: {}.",
        updated_symbols,
        missing_symbols,
        above_threshold
    );

    Ok(())
}

fn has_sufficient_history(candles: &[Candle], min_history_days: usize) -> bool {
    history_span_days(candles).is_some_and(|span_days| span_days >= min_history_days as i64)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let db_url = required_env("ALPHA_VANTAGE_DATABASE_URL")?;
    let api_key = required_env("ALPHA_VANTAGE_API_KEY")?;
    let pool = sqlx::MySqlPool::connect(&db_url).await?;
    let alpha = AlphaVantage { api_key };
    let reset_listing_status = env_flag("ALPHA_VANTAGE_RESET_LISTING_STATUS");
    let sync_listing_status_only = env_flag("ALPHA_VANTAGE_SYNC_LISTING_STATUS_ONLY");
    let reset_candles = env_flag("ALPHA_VANTAGE_RESET_CANDLES");
    let refresh_existing_candles = env_flag("ALPHA_VANTAGE_REFRESH_EXISTING_CANDLES");
    let refresh_average_volume_only = env_flag("ALPHA_VANTAGE_REFRESH_AVG_VOLUME_ONLY");
    let symbol_offset = env_usize("ALPHA_VANTAGE_SYMBOL_OFFSET")?.unwrap_or(0);
    let symbol_limit = env_usize("ALPHA_VANTAGE_SYMBOL_LIMIT")?;
    let min_history_days =
        env_usize("ALPHA_VANTAGE_MIN_HISTORY_DAYS")?.unwrap_or(DEFAULT_MIN_HISTORY_DAYS);
    let average_volume_lookback =
        env_usize("ALPHA_VANTAGE_AVG_VOLUME_LOOKBACK")?.unwrap_or(30);
    let min_average_volume = env_f64("ALPHA_VANTAGE_MIN_AVG_VOLUME")?;
    let start_symbol = env_string("ALPHA_VANTAGE_START_SYMBOL");
    let ingestion_mode = CandleIngestionMode::from_env();
    let reevaluate_bugged = reset_candles || env_flag("ALPHA_VANTAGE_REEVALUATE_BUGGED");

    ensure_listing_status_schema(&pool).await?;

    if reset_listing_status || sync_listing_status_only {
        sync_listing_status(&pool, reset_listing_status).await?;
    }

    if sync_listing_status_only {
        println!("Listing status sync completed. Exiting without candle backfill.");
        return Ok(());
    }

    if refresh_average_volume_only {
        refresh_recent_average_volume(&pool, average_volume_lookback, min_average_volume).await?;
        return Ok(());
    }

    if reset_candles {
        println!("Resetting candles table before backfill.");
        clear_candles(&pool).await?;
        clear_listing_backfill_state(&pool).await?;
    }

    let listing = get_active_symbols_from_db(&pool, reevaluate_bugged).await?;
    let total_symbols = listing.len();
    let mut start = symbol_offset.min(total_symbols);

    if let Some(symbol) = start_symbol.as_deref() {
        if let Some(position) = listing.iter().position(|item| item.as_str() >= symbol) {
            start = position;
        } else {
            start = total_symbols;
        }
    }

    let end = match symbol_limit {
        Some(limit) => start.saturating_add(limit).min(total_symbols),
        None => total_symbols,
    };
    let selected_symbols = &listing[start..end];

    println!(
        "Loaded {} active symbols from DB. Processing batch [{}..{}) with {} symbols. Minimum history: {} days. Start symbol: {}. Reevaluate bugged: {}. Ingestion mode: {:?}.",
        total_symbols,
        start,
        end,
        selected_symbols.len(),
        min_history_days,
        start_symbol.as_deref().unwrap_or("<none>"),
        reevaluate_bugged,
        ingestion_mode
    );
    
    let mut inserted_symbols = 0usize;
    let mut bugged_symbols = 0usize;
    let mut skipped_symbols = 0usize;
    let mut already_loaded_symbols = 0usize;

    for (index, item) in selected_symbols.iter().enumerate() {
        println!(
            "[batch {}/{} | global {}/{}] Symbol {}",
            index + 1,
            selected_symbols.len(),
            start + index + 1,
            total_symbols,
            item
        );

        if let Some(skip_reason) = skip_reason_for_symbol(item) {
            eprintln!("{} {}", item, skip_reason);
            delete_symbol_candles(&pool, item).await?;
            mark_as_bugged(&pool, item, &skip_reason, None, None).await?;
            bugged_symbols += 1;
            continue;
        }

        let existing_candles = symbol_candle_count(&pool, item).await?;
        let is_restart_symbol = start_symbol.as_deref() == Some(item.as_str());

        if is_restart_symbol && existing_candles > 0 {
            println!(
                "Restart symbol {} already has {} candles. Clearing and reloading it.",
                item,
                existing_candles
            );
            delete_symbol_candles(&pool, item).await?;
        } else if existing_candles > 0 && !refresh_existing_candles {
            println!(
                "Skipping {} because it already has {} candles in the database.",
                item,
                existing_candles
            );
            let last_candle_date = latest_symbol_candle_date(&pool, item).await?;
            let history_days = symbol_candle_history_days(&pool, item).await?;
            let average_volume_30d =
                get_recent_average_volume(&pool, item, average_volume_lookback as i64).await?;
            mark_as_not_bugged(
                &pool,
                item,
                average_volume_30d,
                "Skipped because candles are already loaded.",
                history_days,
                last_candle_date,
            )
            .await?;
            already_loaded_symbols += 1;
            continue;
        } else if existing_candles > 0 {
            println!(
                "Refreshing {} with {} existing candles. Missing new dates will be appended.",
                item,
                existing_candles
            );
        }

        let latest_stored_date = if existing_candles > 0 {
            latest_symbol_candle_date(&pool, item).await?
        } else {
            None
        };

        let requested_mode = if existing_candles > 0 && ingestion_mode == CandleIngestionMode::Compact {
            CandleIngestionMode::Compact
        } else {
            CandleIngestionMode::Full
        };

        // FETCH SINGLE SYMBOL CANDLES
        let candles = match alpha
            .load_single_symbol_candle_data(requested_mode.output_size(), item)
            .await
        {
            Ok(c) => c,
            Err(AlphaVantageError::RateLimited(message)) => {
                eprintln!("Stopping backfill after rate limit for {}: {}", item, message);
                break;
            }
            Err(AlphaVantageError::Api(message)) => {
                eprintln!("Permanent API error for {}: {}", item, message);
                delete_symbol_candles(&pool, item).await?;
                mark_as_bugged(
                    &pool,
                    item,
                    &format!("Permanent API error: {}", message),
                    None,
                    None,
                )
                .await?;
                bugged_symbols += 1;
                continue;
            }
            Err(AlphaVantageError::UnexpectedResponse(message)) => {
                eprintln!("Unsupported or empty data for {}: {}", item, message);
                delete_symbol_candles(&pool, item).await?;
                mark_as_bugged(
                    &pool,
                    item,
                    &format!("Unexpected Alpha Vantage response: {}", message),
                    None,
                    None,
                )
                .await?;
                bugged_symbols += 1;
                continue;
            }
            Err(e) => {
                eprintln!("Temporary or unexpected API error for {}: {}", item, e);
                update_listing_ingest_state(
                    &pool,
                    item,
                    None,
                    None,
                    Some(&format!("Temporary ingest error: {}", e)),
                    None,
                    None,
                )
                .await?;
                skipped_symbols += 1;
                continue;
            }
        };

        let new_candles: Vec<Candle> = if requested_mode == CandleIngestionMode::Compact {
            match latest_stored_date {
                Some(latest_date) => candles
                    .into_iter()
                    .filter(|candle| candle.date.is_some_and(|date| date > latest_date))
                    .collect(),
                None => candles,
            }
        } else {
            candles
        };

        // CHECK IF CANDLES
        if new_candles.is_empty() {
            if requested_mode == CandleIngestionMode::Compact && existing_candles > 0 {
                println!(
                    "No newer candles for {} beyond {:?}.",
                    item,
                    latest_stored_date
                );
                let history_days = symbol_candle_history_days(&pool, item).await?;
                let average_volume_30d =
                    get_recent_average_volume(&pool, item, average_volume_lookback as i64).await?;
                mark_as_not_bugged(
                    &pool,
                    item,
                    average_volume_30d,
                    "No newer candles were available; existing history kept.",
                    history_days,
                    latest_stored_date,
                )
                .await?;
                inserted_symbols += 1;
                continue;
            }

            eprintln!("No candles returned for {}", item);
            delete_symbol_candles(&pool, item).await?;
            mark_as_bugged(
                &pool,
                item,
                "No daily candles returned from Alpha Vantage.",
                None,
                None,
            )
            .await?;
            bugged_symbols += 1;
            continue;
        }

        let candle_history_days = history_span_days(&new_candles);

        if existing_candles == 0 && !has_sufficient_history(&new_candles, min_history_days) {
            eprintln!(
                "Insufficient history for {}. Found {} candles, which is less than the required {} days of history.",
                item,
                new_candles.len(),
                min_history_days
            );
            delete_symbol_candles(&pool, item).await?;
            mark_as_bugged(
                &pool,
                item,
                &format!(
                    "Insufficient history: {} candles spanning {} days; requires at least {} days.",
                    new_candles.len(),
                    candle_history_days.unwrap_or(0),
                    min_history_days
                ),
                candle_history_days,
                new_candles.last().and_then(|candle| candle.date),
            )
            .await?;
            bugged_symbols += 1;
            continue;
        }

        // let cutoff = NaiveDate::from_ymd_opt(2026, 1, 16).unwrap();

        // let index = candles
        //     .iter()
        //     .position(|c| c.date.map_or(false, |d| d > cutoff))
        //     .unwrap_or(candles.len());

        // let candles_after = &candles[index..];


        // let candles = alpha.load_single_symbol_candle_data("full", item.symbol.as_str()).await?;
        // if let Some(last_candle) = candles.last() {
        //     insert_candles(&pool, std::slice::from_ref(last_candle)).await?;
        //     println!("Inserted last candle: {:?}", last_candle);
        // }
        // let last_n = candles.len().saturating_sub(3);
        // let last_three = &candles[last_n..];

        let inserted_count = insert_candles(&pool, &new_candles).await?;
        let average_volume_30d =
            recent_average_volume_from_candles(&new_candles, average_volume_lookback);
        mark_as_not_bugged(
            &pool,
            item,
            average_volume_30d,
            &format!(
                "Loaded {} candles with {:?} mode. Inserted {} new rows.",
                new_candles.len(),
                requested_mode,
                inserted_count
            ),
            candle_history_days,
            new_candles.last().and_then(|candle| candle.date),
        )
        .await?;
        inserted_symbols += 1;
        println!(
            "Fetched {} candles with {:?} mode. Inserted {} new candles.",
            new_candles.len(),
            requested_mode,
            inserted_count
        );
        // println!("Inserted candles: {:?}", candles_after);




        // // ---- Calculate average volume ----
        // let mut total_volume: u64 = 0;
        // let mut count: usize = 0;
        

        // for c in &candles {
        //     if let Ok(v) = c.volume.parse::<u64>() {
        //         total_volume += v;
        //         count += 1;
        //     }
        // }

        // let average_volume = if count > 0 {
        //     total_volume as f64 / count as f64
        // } else {
        //     0.0
        // };

        // if let Err(e) = update_listing_status(&pool, item, average_volume).await {
        //     eprintln!("Error updating {}: {}", item, e);

        //     // Mark this row as bugged
        //     mark_as_bugged(&pool, item).await?;

        //     // Continue to next row
        //     continue;
        // }

    
                

    }

    println!(
        "Batch finished. Inserted symbols: {}. Marked bugged: {}. Temporary skips: {}. Already loaded skips: {}.",
        inserted_symbols,
        bugged_symbols,
        skipped_symbols,
        already_loaded_symbols
    );

    refresh_recent_average_volume(&pool, average_volume_lookback, min_average_volume).await?;


    Ok(())
}


pub async fn insert_candles(
    pool: &sqlx::MySqlPool,
    candles: &[Candle],
) -> Result<usize, sqlx::Error> {

    let query = r#"
        INSERT IGNORE INTO candles (
            symbol, date, open, high, low, close, volume
        ) VALUES (?, ?, ?, ?, ?, ?,?)
    "#;

    let mut inserted_rows = 0usize;

    for c in candles {
        let date_str = c.date.map(|d| d.format("%Y-%m-%d").to_string());

        let symbol = c.symbol.as_deref();

        let result = sqlx::query(query)
            .bind(symbol)
            .bind(date_str)
            .bind(&c.open)
            .bind(&c.high)
            .bind(&c.low)
            .bind(&c.close)
            .bind(&c.volume)
            .execute(pool)
            .await?;

        inserted_rows += result.rows_affected() as usize;
    }

    Ok(inserted_rows)
}
