use reqwest;
use csv;
use std::error::Error;
use alpha_vantage::models::listing_status::ListingStatus;
use alpha_vantage::models::alpha_vantage::AlphaVantage;
use alpha_vantage::models::candle::Candle;

pub async fn mark_as_bugged(pool: &sqlx::MySqlPool,symbol: &str,) -> Result<(), sqlx::Error> {

    let query = r#"
        UPDATE listing_status
        SET bugged = true
        WHERE symbol = ?
    "#;

    sqlx::query(query)
        .bind(symbol)
        .execute(pool)
        .await?;

    Ok(())
}
pub async fn above_500_volume(pool: &sqlx::MySqlPool) -> Result<Vec<String>, sqlx::Error> {

  

    let rows = sqlx::query_scalar::<_, String>(
        r#"
        select *
        from listing_status
        WHERE average_volume > 500000
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
        WHERE average_volume IS NULL
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
pub async fn update_listing_status(pool: &sqlx::MySqlPool,symbol: &str, average_volume: f64,) -> Result<(), sqlx::Error> {

    let query = r#"
        UPDATE listing_status
        SET average_volume = ?
        WHERE symbol = ?
    "#;

    sqlx::query(query)
        .bind(average_volume)
        .bind(symbol)
        .execute(pool)
        .await?;

    Ok(())
}
pub async fn get_listing_status() -> Result<Vec<ListingStatus>, Box<dyn Error>> {
    let url = "https://www.alphavantage.co/query?function=LISTING_STATUS&apikey=ZA9N4R1HE9ARIJ0S";

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {


    let db_url = "mysql://rperezkc:Nar8uto!@localhost:3306/abcd";
    let pool = sqlx::MySqlPool::connect(db_url).await?;

    let listing = above_500_volume(&pool).await?;
    
    
    for item in &listing{

        let alpha = AlphaVantage {api_key: "ZA9N4R1HE9ARIJ0S".to_string()};

        let candles = match alpha.load_single_symbol_candle_data("full", item).await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("API error for {}: {}", item, e);
                mark_as_bugged(&pool, item).await?;
                continue;
            }
        };
        if candles.is_empty() {
            eprintln!("No candles returned for {}", item);
            mark_as_bugged(&pool, item).await?;
            continue;
        }



        // let candles = alpha.load_single_symbol_candle_data("full", item.symbol.as_str()).await?;
        if let Some(last_candle) = candles.last() {
            insert_candles(&pool, std::slice::from_ref(last_candle)).await?;
            println!("Inserted last candle: {:?}", last_candle);
        }
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

