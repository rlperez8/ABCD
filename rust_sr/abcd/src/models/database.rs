use mysql::{PooledConn, Pool};
use mysql::*;
use mysql::prelude::*;
use crate::models::candle::Candle; 
use chrono::NaiveDate;

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
            LIMIT 90
    
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