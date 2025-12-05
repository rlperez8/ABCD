use serde::Serialize;
use crate::models::pivot::Pivot;
use crate::models::market::Market;
use crate::models::abcd_type::ABCDType;
use crate::models::trade::Trade;

#[derive(Debug, Clone, Serialize)]
pub struct PatternXABCD {
    pub symbol: String,
    pub x: Pivot,
    pub a: Pivot,
    pub b: Pivot,
    pub c: Pivot,
    pub d: Pivot,
    pub market: Market,
    pub abcd_type: ABCDType,
    pub trade: Trade,
}

