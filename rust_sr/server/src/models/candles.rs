use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::FromRow;
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Candle {
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
