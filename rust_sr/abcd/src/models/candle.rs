use chrono::NaiveDate;
use serde::Serialize;
// use sqlx::FromRow;
use rust_decimal::Decimal;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CandleDecimal {
    pub symbol: String,
    pub date: NaiveDate,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: i64,
    pub three_month: Option<bool>,
    pub six_month: Option<bool>,
    pub twelve_month: Option<bool>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Candle {
    pub symbol: String,
    pub date: NaiveDate,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
    pub three_month: Option<bool>,
    pub six_month: Option<bool>,
    pub twelve_month: Option<bool>,
}
