use serde::Serialize;
use chrono::NaiveDate;  

#[derive(Debug, Clone, Serialize)]  
pub struct Candle {
    pub symbol: String,
    pub date: NaiveDate,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}