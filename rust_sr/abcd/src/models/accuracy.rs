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

    fn set_harmonic_accuracy(&mut self, harmonic_type: HarmonicType, accuracy: PatternAccuracy) {
        match harmonic_type {
            HarmonicType::Bat => self.bat = accuracy,
            HarmonicType::AlternateBat => self.alternate_bat = accuracy,
            HarmonicType::Butterfly => self.butterfly = accuracy,
            HarmonicType::Gartley => self.gartley = accuracy,
            HarmonicType::Crab => self.crab = accuracy,
            HarmonicType::DeepCrab => self.deep_crab = accuracy,
            HarmonicType::Shark => self.shark = accuracy,
            _ => {}
        }
    }

    fn harmonic_accuracy(
        pattern: &PatternXABCD,
        harmonic_type: HarmonicType,
        targets: (f64, f64, f64, f64),
    ) -> (PatternAccuracy, TimeAccuracy) {
        let (ab_xa_target, bc_ab_target, cd_bc_target, cd_xa_target) = targets;

        let mut price_accuracy = PatternAccuracy::default();
        price_accuracy.ab_xa = Self::leg_accuracy(pattern.trade.ab_price_retracement, ab_xa_target);
        price_accuracy.bc_ab = Self::leg_accuracy(pattern.trade.bc_price_retracement, bc_ab_target);
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

        let mut time_accuracy = TimeAccuracy::default();
        time_accuracy.ab_xa = Self::leg_accuracy(pattern.trade.ab_bar_retracement, ab_xa_target);
        time_accuracy.bc_ab = Self::leg_accuracy(pattern.trade.bc_bar_retracement, bc_ab_target);
        time_accuracy.cd_bc = Self::leg_accuracy(pattern.trade.cd_bc_bar_retracement, cd_bc_target);
        time_accuracy.cd_xa = Self::leg_accuracy(pattern.trade.cd_xa_bar_retracement, cd_xa_target);
        time_accuracy.time_accuracy =
            (time_accuracy.ab_xa + time_accuracy.bc_ab + time_accuracy.cd_bc + time_accuracy.cd_xa)
                / 4.0;

        (price_accuracy, time_accuracy)
    }

    fn better_dominant(
        current: Option<(HarmonicType, PatternAccuracy, TimeAccuracy)>,
        candidate: (HarmonicType, PatternAccuracy, TimeAccuracy),
    ) -> Option<(HarmonicType, PatternAccuracy, TimeAccuracy)> {
        let Some(best) = current else {
            return Some(candidate);
        };

        let price_is_better = candidate.1.pattern_accuracy > best.1.pattern_accuracy;
        let price_tie_with_better_time =
            (candidate.1.pattern_accuracy - best.1.pattern_accuracy).abs() <= f64::EPSILON
                && candidate.2.time_accuracy > best.2.time_accuracy;

        if price_is_better || price_tie_with_better_time {
            Some(candidate)
        } else {
            Some(best)
        }
    }

    pub fn get_accuracy(&self, mut xabcd_patterns: Vec<PatternXABCD>) -> Vec<PatternXABCD> {
        for pattern in xabcd_patterns.iter_mut() {
            let mut dominant = None;

            for harmonic_type in [
                HarmonicType::Bat,
                HarmonicType::AlternateBat,
                HarmonicType::Butterfly,
                HarmonicType::Gartley,
                HarmonicType::Crab,
                HarmonicType::DeepCrab,
                HarmonicType::Shark,
            ] {
                let Some(targets) = Self::harmonic_targets(harmonic_type) else {
                    continue;
                };

                let (price_accuracy, time_accuracy) =
                    Self::harmonic_accuracy(pattern, harmonic_type, targets);
                dominant =
                    Self::better_dominant(dominant, (harmonic_type, price_accuracy, time_accuracy));
            }

            let mut accuracies = Accuracies::new();
            let mut time_accuracies = TimeAccuracies::new();
            if let Some((harmonic_type, price_accuracy, time_accuracy)) = dominant {
                accuracies.set_harmonic_accuracy(harmonic_type, price_accuracy);
                time_accuracies.set_harmonic_accuracy(harmonic_type, time_accuracy);
            }
            pattern.accuracies = accuracies;
            pattern.time_accuracies = time_accuracies;
            pattern.refresh_pattern_id();
            pattern.refresh_prop_strategy_id();
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

    fn set_harmonic_accuracy(&mut self, harmonic_type: HarmonicType, accuracy: TimeAccuracy) {
        match harmonic_type {
            HarmonicType::Bat => self.bat = accuracy,
            HarmonicType::AlternateBat => self.alternate_bat = accuracy,
            HarmonicType::Butterfly => self.butterfly = accuracy,
            HarmonicType::Gartley => self.gartley = accuracy,
            HarmonicType::Crab => self.crab = accuracy,
            HarmonicType::DeepCrab => self.deep_crab = accuracy,
            HarmonicType::Shark => self.shark = accuracy,
            _ => {}
        }
    }
}
