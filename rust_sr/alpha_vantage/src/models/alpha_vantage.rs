use crate::models::candle::Candle;
use serde_json::Value;
use chrono::NaiveDate;
use std::collections::HashMap;

pub struct AlphaVantage {
    pub api_key: String,
}

impl AlphaVantage {
    
    pub async fn load_single_symbol_candle_data(&self,output_size: &str,ticker: &str,) -> Result<Vec<Candle>, Box<dyn std::error::Error>> {
        let timeframe = "TIME_SERIES_DAILY";
        let url = format!(
            "https://www.alphavantage.co/query?function={}&symbol={}&outputsize={}&apikey={}",
            timeframe, ticker, output_size, self.api_key
        );

        // Fetch JSON
        let r = reqwest::get(&url).await?;
        let data: Value = r.json().await?;

        // Error handling
        if data.get("Error Message").is_some() || data.get("Time Series (Daily)").is_none() {
            println!("=================");
            println!("Error {}", ticker);
            return Ok(vec![]);
        }

        // Parse time series
        let ts = data["Time Series (Daily)"].as_object().unwrap();
        let mut candles: Vec<Candle> = Vec::new();

        for (date_str, values) in ts.iter() {
            let mut candle: Candle = serde_json::from_value(values.clone())?;
            candle.date = Some(NaiveDate::parse_from_str(date_str, "%Y-%m-%d")?);
            candle.symbol = Some(ticker.to_string());
            candles.push(candle);
        }

        // Optional: sort by date ascending
        candles.sort_by_key(|c| c.date);

        Ok(candles)
    }
}
