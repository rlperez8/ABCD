use reqwest;
use csv;
use std::error::Error;
use std::env;
use alpha_vantage::models::listing_status::ListingStatus;
use alpha_vantage::models::alpha_vantage::{AlphaVantage, AlphaVantageError};
use alpha_vantage::models::candle::Candle;

const DEFAULT_MIN_HISTORY_DAYS: usize = 365 * 3;

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

pub async fn set_bugged_status(
    pool: &sqlx::MySqlPool,
    symbol: &str,
    bugged: bool,
) -> Result<(), sqlx::Error> {

    let query = r#"
        UPDATE listing_status
        SET bugged = ?
        WHERE symbol = ?
    "#;

    sqlx::query(query)
        .bind(bugged)
        .bind(symbol)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn mark_as_bugged(pool: &sqlx::MySqlPool,symbol: &str,) -> Result<(), sqlx::Error> {
    set_bugged_status(pool, symbol, true).await
}

pub async fn mark_as_not_bugged(pool: &sqlx::MySqlPool,symbol: &str,) -> Result<(), sqlx::Error> {
    set_bugged_status(pool, symbol, false).await
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
            symbol, name, exchange, assetType, ipoDate, delistingDate, status
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE
            name = VALUES(name),
            exchange = VALUES(exchange),
            assetType = VALUES(assetType),
            ipoDate = VALUES(ipoDate),
            delistingDate = VALUES(delistingDate),
            status = VALUES(status)
    "#;

    for row in rows {
        sqlx::query(query)
            .bind(row.symbol.trim())
            .bind(row.name.trim())
            .bind(row.exchange.trim())
            .bind(row.asset_type.trim())
            .bind(row.ipo_date.trim())
            .bind(row.delisting_date.trim())
            .bind(row.status.trim())
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

pub async fn get_active_symbols_from_db(pool: &sqlx::MySqlPool) -> Result<Vec<String>, sqlx::Error> {
    let mut symbols = sqlx::query_scalar::<_, String>(
        r#"
        SELECT symbol
        FROM listing_status
        WHERE status = 'Active'
          AND (bugged IS NULL OR bugged = false)
        "#
    )
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
    let symbols = get_active_symbols_from_db(pool).await?;
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
    let oldest = candles.first().and_then(|candle| candle.date);
    let newest = candles.last().and_then(|candle| candle.date);

    match (oldest, newest) {
        (Some(oldest), Some(newest)) => {
            let span_days = (newest - oldest).num_days();
            span_days >= min_history_days as i64
        }
        _ => false,
    }
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
    let refresh_average_volume_only = env_flag("ALPHA_VANTAGE_REFRESH_AVG_VOLUME_ONLY");
    let symbol_offset = env_usize("ALPHA_VANTAGE_SYMBOL_OFFSET")?.unwrap_or(0);
    let symbol_limit = env_usize("ALPHA_VANTAGE_SYMBOL_LIMIT")?;
    let min_history_days =
        env_usize("ALPHA_VANTAGE_MIN_HISTORY_DAYS")?.unwrap_or(DEFAULT_MIN_HISTORY_DAYS);
    let average_volume_lookback =
        env_usize("ALPHA_VANTAGE_AVG_VOLUME_LOOKBACK")?.unwrap_or(30);
    let min_average_volume = env_f64("ALPHA_VANTAGE_MIN_AVG_VOLUME")?;
    let start_symbol = env_string("ALPHA_VANTAGE_START_SYMBOL");

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
    }

    let listing = get_active_symbols_from_db(&pool).await?;
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
        "Loaded {} active non-bugged symbols from DB. Processing batch [{}..{}) with {} symbols. Minimum history: {} days. Start symbol: {}.",
        total_symbols,
        start,
        end,
        selected_symbols.len(),
        min_history_days,
        start_symbol.as_deref().unwrap_or("<none>")
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

        let existing_candles = symbol_candle_count(&pool, item).await?;
        let is_restart_symbol = start_symbol.as_deref() == Some(item.as_str());

        if is_restart_symbol && existing_candles > 0 {
            println!(
                "Restart symbol {} already has {} candles. Clearing and reloading it.",
                item,
                existing_candles
            );
            delete_symbol_candles(&pool, item).await?;
        } else if existing_candles > 0 {
            println!(
                "Skipping {} because it already has {} candles in the database.",
                item,
                existing_candles
            );
            already_loaded_symbols += 1;
            continue;
        }

        // FETCH SINGLE SYMBOL CANDLES
        let candles = match alpha.load_single_symbol_candle_data("full", item).await {
            Ok(c) => c,
            Err(AlphaVantageError::RateLimited(message)) => {
                eprintln!("Stopping backfill after rate limit for {}: {}", item, message);
                break;
            }
            Err(AlphaVantageError::Api(message)) => {
                eprintln!("Permanent API error for {}: {}", item, message);
                delete_symbol_candles(&pool, item).await?;
                mark_as_bugged(&pool, item).await?;
                bugged_symbols += 1;
                continue;
            }
            Err(e) => {
                eprintln!("Temporary or unexpected API error for {}: {}", item, e);
                skipped_symbols += 1;
                continue;
            }
        };

        // CHECK IF CANDLES
        if candles.is_empty() {
            eprintln!("No candles returned for {}", item);
            delete_symbol_candles(&pool, item).await?;
            mark_as_bugged(&pool, item).await?;
            bugged_symbols += 1;
            continue;
        }

        if !has_sufficient_history(&candles, min_history_days) {
            eprintln!(
                "Insufficient history for {}. Found {} candles, which is less than the required {} days of history.",
                item,
                candles.len(),
                min_history_days
            );
            delete_symbol_candles(&pool, item).await?;
            mark_as_bugged(&pool, item).await?;
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

        insert_candles(&pool, &candles).await?;
        mark_as_not_bugged(&pool, item).await?;
        inserted_symbols += 1;
        println!("Inserted {} candles", candles.len());
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


pub async fn insert_candles(pool: &sqlx::MySqlPool,candles: &[Candle],) -> Result<(), sqlx::Error> {

    let query = r#"
        INSERT IGNORE INTO candles (
            symbol, date, open, high, low, close, volume
        ) VALUES (?, ?, ?, ?, ?, ?,?)
    "#;

    for c in candles {
        let date_str = c.date.map(|d| d.format("%Y-%m-%d").to_string());

        let symbol = c.symbol.as_deref();

        sqlx::query(query)
            .bind(symbol)
            .bind(date_str)
            .bind(&c.open)
            .bind(&c.high)
            .bind(&c.low)
            .bind(&c.close)
            .bind(&c.volume)
            .execute(pool)
            .await?;
    }

    Ok(())
}
