use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::FromRow;
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Candle {
    pub symbol: String,
    pub date: NaiveDateTime,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
    pub three_month: Option<bool>,
    pub six_month: Option<bool>,
    pub twelve_month: Option<bool>,
}
