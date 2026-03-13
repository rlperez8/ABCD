use crate::models::abcd_type::ABCDType;

#[derive(Default)]
pub struct PatternAccuracy {
    pub ab_xa: f64,
    pub bc_ab: f64,
    pub cd_bc: f64,
    pub cd_ab: f64,
    pub cd_xa: f64,
    pub pattern_accuracy: f64,
}

pub struct Accuracies {
    pub bat: PatternAccuracy,
    pub butterfly: PatternAccuracy,
    pub gartley: PatternAccuracy,
    pub crab: PatternAccuracy,
    pub shark: PatternAccuracy,
}