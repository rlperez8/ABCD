use crate::models::candle::Candle;
use crate::models::pivot_type::PivotType;
use chrono::NaiveDate;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Pivot {
    pub type_: PivotType,
    pub date: NaiveDate,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub length: i64,
    pub min_max: f64,
    pub leg_price_length: f64,
}

impl Pivot {
    fn truncate_to_2_decimals(value: f64) -> f64 {
        (value * 100.0).trunc() / 100.0
    }

    // Constructor for Pivot
    pub fn new(
        candle: &Candle,
        type_: PivotType,
        length: i64,
        min_max: f64,
        leg_price_length: f64,
    ) -> Self {
        Self {
            type_,
            date: candle.date,
            open: Self::truncate_to_2_decimals(candle.open),
            high: Self::truncate_to_2_decimals(candle.high),
            low: Self::truncate_to_2_decimals(candle.low),
            close: Self::truncate_to_2_decimals(candle.close),
            length,
            min_max: Self::truncate_to_2_decimals(min_max),
            leg_price_length,
        }
    }
}
