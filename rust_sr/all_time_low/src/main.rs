use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::error::Error;
// use csv::Writer;


use mysql::{PooledConn, Pool};
use mysql::*;
use mysql::prelude::*;
use chrono::NaiveDate;

#[derive(Debug, Clone, Serialize)]  // <- directly above the struct
pub struct Candle {
    pub symbol: String,
    pub date: NaiveDate,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

pub struct Database {
    pub conn: PooledConn,
}
impl Database {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let url = "mysql://rperezkc:Nar8uto!@localhost:3306/abcd";
        let pool = Pool::new(url)?;
        let conn = pool.get_conn()?;
        Ok(Self { conn })
    }

    pub fn get_distinct_symbols(&mut self) -> mysql::Result<Vec<String>> {
        let query = r#"
            SELECT DISTINCT symbol
            FROM abcd.candles
            ORDER BY symbol
        "#;

        let symbols: Vec<String> = self.conn.query_map(query, |symbol: String| symbol)?;
        Ok(symbols)
    }

    pub fn get_stored_candles(&mut self, symbol: &str) -> mysql::Result<Vec<Candle>> {

            
        let query = r#"
            SELECT symbol,
                   DATE_FORMAT(date, '%Y-%m-%d') AS date_str,
                   open, high, low, close, volume
            FROM abcd.candles
            WHERE symbol = :symbol
            ORDER BY date DESC
        "#;

        let params = params! { "symbol" => symbol };

        let mut candles: Vec<Candle> = self.conn.exec_map(
            query,
            params,
            |(symbol, date_str, open, high, low, close, volume): (String, String, f64, f64, f64, f64, u64)| {
                let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                    .expect("Failed to parse date");
                Candle {
                    symbol,
                    date,
                    open,
                    high,
                    low,
                    close,
                    volume,
                }
            },
        )?;

        candles.reverse();

        Ok(candles)
    }
}


//
// -------------------- SCANNER LOGIC BELOW ----------------------
//

/// Get average open-to-close body size
fn average_body_size(candles: &[Candle]) -> f64 {
    let total: f64 = candles
        .iter()
        .map(|c| (c.open - c.close).abs())
        .sum();

    total / candles.len() as f64
}

/// Determine if the last candle meets your pattern
fn matches_pattern(candles: &[Candle], x_avg_distance: f64) -> bool {
    // Need at least 3 candles to compare last vs previous 2
    if candles.len() < 10 {
        return false;
    }

    let last = candles.last().unwrap();
    let prev1 = &candles[candles.len() - 2];
    let prev2 = &candles[candles.len() - 3];

    // 1. Last candle must be red
    if last.close >= last.open {
        return false;
    }

    // Body sizes
    let body_last = (last.open - last.close).abs();
    let body_prev1 = (prev1.open - prev1.close).abs();
    let body_prev2 = (prev2.open - prev2.close).abs();

    // 2. Last body > average body size
    let avg = average_body_size(candles);
    if body_last < avg {
        return false;
    }

    // 3. Last body must be > previous 2 bodies
    if body_last <= body_prev1 || body_last <= body_prev2 {
        return false;
    }

    // 4. Near all-time low (within X average candle sizes)
    let all_time_low = candles.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
    let distance = last.low - all_time_low;

    distance <= avg * x_avg_distance
}

/// Main scanning function
fn main() -> Result<(), Box<dyn Error>> {
    let mut db = Database::new()?;
    let symbols = db.get_distinct_symbols()?;

    // Setting: within 2 average candle bodies of all-time low
    let x_avg_distance = 2.0;

    println!("Scanning {} symbols...\n", symbols.len());

    // for symbol in &symbols {
    for symbol in ["XBP".to_string()].iter() {
        
        let candles = db.get_stored_candles(symbol)?;
        if candles.is_empty() {
            println!("{} has no candles.", symbol);
            continue;
        }

        let first = candles.first().unwrap();
        let last = candles.last().unwrap();

        // Find lowest candle
        let lowest = candles
            .iter()
            .min_by(|a, b| a.low.partial_cmp(&b.low).unwrap())
            .unwrap();

        // Last 3 candles (safely)
        let len = candles.len();
        let last3 = &candles[len.saturating_sub(3)..];

        println!("Symbol: {}\n", symbol);

        // First candle ever
        println!("FIRST candle:  {:?}", first);

        // All-time low candle
        println!("LOWEST candle: {:?}", lowest);

        // Most recent 3 candles
        println!("LAST 3 candles:");
        for c in last3 {
            println!("{:?}", c);
        }

        println!("----------------------------------\n");


        // if matches_pattern(&candles, x_avg_distance) {
        //     println!("MATCH: {}", symbol);
        // }
    }

    println!("\nScan complete.");

    Ok(())
}