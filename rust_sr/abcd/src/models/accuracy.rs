use serde::Serialize;

use crate::models::HarmonicType;
use crate::models::PatternXABCD;

#[derive(Default, Debug, Clone, Copy, Serialize)]
pub struct PatternAccuracy {
    pub ab_xa: f64,
    pub bc_ab: f64,
    pub cd_bc: f64,
    pub cd_xa: f64,
    pub pattern_accuracy: f64,
    pub harmonic_type: HarmonicType,
}

#[derive(Default, Debug, Clone, Copy, Serialize)]
pub struct TimeAccuracy {
    pub ab_xa: f64,
    pub bc_ab: f64,
    pub cd_bc: f64,
    pub cd_xa: f64,
    pub time_accuracy: f64,
}

#[derive(Default, Debug, Clone, Copy, Serialize)]
pub struct TimeAccuracies {
    pub bat: TimeAccuracy,
    pub alternate_bat: TimeAccuracy,
    pub butterfly: TimeAccuracy,
    pub gartley: TimeAccuracy,
    pub crab: TimeAccuracy,
    pub deep_crab: TimeAccuracy,
    pub shark: TimeAccuracy,
}

#[derive(Default, Debug, Clone, Copy, Serialize)]
pub struct Accuracies {
    pub bat: PatternAccuracy,
    pub alternate_bat: PatternAccuracy,
    pub butterfly: PatternAccuracy,
    pub gartley: PatternAccuracy,
    pub crab: PatternAccuracy,
    pub deep_crab: PatternAccuracy,
    pub shark: PatternAccuracy,
}

impl Accuracies {
    pub fn new() -> Self {
        Accuracies {
            bat: PatternAccuracy::default(),
            alternate_bat: PatternAccuracy::default(),
            butterfly: PatternAccuracy::default(),
            gartley: PatternAccuracy::default(),
            crab: PatternAccuracy::default(),
            deep_crab: PatternAccuracy::default(),
            shark: PatternAccuracy::default(),
        }
    }

    pub fn leg_accuracy(current_leg: f64, target_leg: f64) -> f64 {
        // Assume current_leg is in "percent" form (like 42 for 0.42)
        let current_leg_fraction = current_leg / 100.0;
        let accuracy = 100.0 * (1.0 - (current_leg_fraction - target_leg).abs() / target_leg);
        accuracy.clamp(0.0, 100.0)
    }

    fn harmonic_targets(harmonic_type: HarmonicType) -> Option<(f64, f64, f64, f64)> {
        match harmonic_type {
            HarmonicType::Bat => Some((0.382, 0.382, 2.618, 1.618)),
            HarmonicType::AlternateBat => Some((0.382, 0.382, 2.0, 1.13)),
            HarmonicType::Butterfly => Some((0.786, 0.382, 1.27, 1.618)),
            HarmonicType::Gartley => Some((0.618, 0.382, 1.27, 0.786)),
            HarmonicType::Crab => Some((0.382, 0.382, 3.618, 2.618)),
            HarmonicType::DeepCrab => Some((0.886, 0.382, 2.618, 1.618)),
            HarmonicType::Shark => Some((0.886, 0.382, 1.13, 1.618)),
            _ => None,
        }
    }

    fn harmonic_time_accuracy(pattern: &PatternXABCD, harmonic_type: HarmonicType) -> TimeAccuracy {
        let Some((ab_xa_target, bc_ab_target, cd_bc_target, cd_xa_target)) =
            Self::harmonic_targets(harmonic_type)
        else {
            return TimeAccuracy::default();
        };

        let mut time_accuracy = TimeAccuracy::default();
        time_accuracy.ab_xa = Self::leg_accuracy(pattern.trade.ab_bar_retracement, ab_xa_target);
        time_accuracy.bc_ab = Self::leg_accuracy(pattern.trade.bc_bar_retracement, bc_ab_target);
        time_accuracy.cd_bc = Self::leg_accuracy(pattern.trade.cd_bc_bar_retracement, cd_bc_target);
        time_accuracy.cd_xa = Self::leg_accuracy(pattern.trade.cd_xa_bar_retracement, cd_xa_target);
        time_accuracy.time_accuracy =
            (time_accuracy.ab_xa + time_accuracy.bc_ab + time_accuracy.cd_bc + time_accuracy.cd_xa)
                / 4.0;
        time_accuracy
    }

    pub fn get_accuracy(&self, mut xabcd_patterns: Vec<PatternXABCD>) -> Vec<PatternXABCD> {
        // let mut all_accuracies: Vec<Accuracies> = Vec::new();

        for pattern in xabcd_patterns.iter_mut() {
            let mut accuracies = Accuracies::new();
            let mut time_accuracies = TimeAccuracies::new();

            for harmonic_type in [
                HarmonicType::Bat,
                HarmonicType::AlternateBat,
                HarmonicType::Butterfly,
                HarmonicType::Gartley,
                HarmonicType::Crab,
                HarmonicType::DeepCrab,
                HarmonicType::Shark,
            ] {
                let Some((ab_xa_target, bc_ab_target, cd_bc_target, cd_xa_target)) =
                    Self::harmonic_targets(harmonic_type)
                else {
                    continue;
                };

                let mut price_accuracy = PatternAccuracy::default();
                let time_accuracy = Self::harmonic_time_accuracy(pattern, harmonic_type);
                price_accuracy.ab_xa =
                    Self::leg_accuracy(pattern.trade.ab_price_retracement, ab_xa_target);
                price_accuracy.bc_ab =
                    Self::leg_accuracy(pattern.trade.bc_price_retracement, bc_ab_target);
                price_accuracy.cd_bc =
                    Self::leg_accuracy(pattern.trade.cd_bc_price_retracement, cd_bc_target);
                price_accuracy.cd_xa =
                    Self::leg_accuracy(pattern.trade.cd_xa_price_retracement, cd_xa_target);
                price_accuracy.pattern_accuracy = (price_accuracy.ab_xa
                    + price_accuracy.bc_ab
                    + price_accuracy.cd_bc
                    + price_accuracy.cd_xa)
                    / 4.0;
                price_accuracy.harmonic_type = harmonic_type;

                match harmonic_type {
                    HarmonicType::Bat => {
                        accuracies.bat = price_accuracy;
                        time_accuracies.bat = time_accuracy;
                    }
                    HarmonicType::AlternateBat => {
                        accuracies.alternate_bat = price_accuracy;
                        time_accuracies.alternate_bat = time_accuracy;
                    }
                    HarmonicType::Butterfly => {
                        accuracies.butterfly = price_accuracy;
                        time_accuracies.butterfly = time_accuracy;
                    }
                    HarmonicType::Gartley => {
                        accuracies.gartley = price_accuracy;
                        time_accuracies.gartley = time_accuracy;
                    }
                    HarmonicType::Crab => {
                        accuracies.crab = price_accuracy;
                        time_accuracies.crab = time_accuracy;
                    }
                    HarmonicType::DeepCrab => {
                        accuracies.deep_crab = price_accuracy;
                        time_accuracies.deep_crab = time_accuracy;
                    }
                    HarmonicType::Shark => {
                        accuracies.shark = price_accuracy;
                        time_accuracies.shark = time_accuracy;
                    }
                    _ => {}
                }
            }
            // println!("{:?}", accuracies);

            pattern.accuracies = accuracies;
            pattern.time_accuracies = time_accuracies;
            pattern.refresh_pattern_id();
            pattern.refresh_prop_strategy_id();
            // all_accuracies.push(accuracies);
        }

        xabcd_patterns
    }
}

impl TimeAccuracies {
    pub fn new() -> Self {
        Self {
            bat: TimeAccuracy::default(),
            alternate_bat: TimeAccuracy::default(),
            butterfly: TimeAccuracy::default(),
            gartley: TimeAccuracy::default(),
            crab: TimeAccuracy::default(),
            deep_crab: TimeAccuracy::default(),
            shark: TimeAccuracy::default(),
        }
    }

    pub fn compatibility_scalar(&self) -> f64 {
        [
            self.bat.time_accuracy,
            self.alternate_bat.time_accuracy,
            self.butterfly.time_accuracy,
            self.gartley.time_accuracy,
            self.crab.time_accuracy,
            self.deep_crab.time_accuracy,
            self.shark.time_accuracy,
        ]
        .into_iter()
        .fold(0.0, f64::max)
    }
}
