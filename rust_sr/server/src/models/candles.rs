use serde::Serialize;
use chrono::NaiveDate;  
use sqlx::FromRow;
use rust_decimal::Decimal;
#[derive(Debug, Clone, Serialize, FromRow)]  
pub struct Candle {
    pub symbol: String,
    pub date: NaiveDate,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: i64,
}