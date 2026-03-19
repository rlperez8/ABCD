use serde::Serialize;
use crate::models::accuracy::Accuracies;
use crate::models::pivot::Pivot;
use crate::models::market::Market;
use crate::models::harmonic_types::HarmonicType;
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
    pub abcd_type: HarmonicType,
    pub trade: Trade,
    pub three_month: Option<bool>,
    pub six_month: Option<bool>,
    pub twelve_month: Option<bool>,
    pub pattern_group_id: String,
    pub accuracies: Accuracies

}

