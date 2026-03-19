// use crate::models::abcd_type::HarmonicType;

use serde::Serialize;

use crate::models::PatternXABCD;
use crate::models::HarmonicType;

#[derive(Default, Debug, Clone, Serialize)]
pub struct PatternAccuracy {
    pub ab_xa: f64,
    pub bc_ab: f64,
    pub cd_bc: f64,
    pub cd_xa: f64,
    pub pattern_accuracy: f64,
    pub harmonic_type: String,
}
#[derive(Default, Debug, Clone, Serialize)]
pub struct Accuracies {
    pub bat: PatternAccuracy,
    pub butterfly: PatternAccuracy,
    pub gartley: PatternAccuracy,
    pub crab: PatternAccuracy,
    pub shark: PatternAccuracy,
}

impl Accuracies {

    pub fn new() -> Self {
        Accuracies {
            bat: PatternAccuracy::default(),
            butterfly: PatternAccuracy::default(),
            gartley: PatternAccuracy::default(),
            crab: PatternAccuracy::default(),
            shark: PatternAccuracy::default(),
        }
    }

    pub fn leg_accuracy(current_leg: f64, target_leg: f64) -> f64 {
        // Assume current_leg is in "percent" form (like 42 for 0.42)
        let current_leg_fraction = current_leg / 100.0;
        let accuracy = 100.0 * (1.0 - (current_leg_fraction - target_leg).abs() / target_leg);
        accuracy.clamp(0.0, 100.0)
    }

    pub fn get_accuracy(&self, mut xabcd_patterns: Vec<PatternXABCD>) -> Vec<PatternXABCD>  {
   
        // let mut all_accuracies: Vec<Accuracies> = Vec::new();

        for pattern in xabcd_patterns.iter_mut() {
            let mut accuracies = Accuracies::new();

            for harmonic_type in [HarmonicType::Bat, HarmonicType::Butterfly, HarmonicType::Gartley, HarmonicType::Crab, HarmonicType::Shark] {

                match harmonic_type {
                    HarmonicType::Bat => {
                        let mut bat: PatternAccuracy = PatternAccuracy::default(); 
                        bat.ab_xa = Self::leg_accuracy(pattern.trade.ab_price_retracement, 0.382);
                        bat.bc_ab = Self::leg_accuracy(pattern.trade.bc_price_retracement, 0.382);
                        bat.cd_bc = Self::leg_accuracy(pattern.trade.cd_bc_price_retracement, 2.618);
                        bat.cd_xa = Self::leg_accuracy(pattern.trade.cd_xa_price_retracement, 1.618); 
                        bat.pattern_accuracy = (bat.ab_xa + bat.bc_ab + bat.cd_bc + bat.cd_xa) / 4.0;
                        bat.harmonic_type = "Bat".to_string();
                        accuracies.bat = bat;

                        // println!("Bat AB/XA: {:.2}, Accuracy: {:.2}, Target: {:.2}", 
                        //     pattern.trade.ab_price_retracement / 100.0, 
                        //     accuracies.bat.ab_xa,
                        //     0.382
                        // );
                 
                     },
                    HarmonicType::Butterfly => {
                        let mut butterfly: PatternAccuracy = PatternAccuracy::default(); 
                        butterfly.ab_xa = Self::leg_accuracy(pattern.trade.ab_price_retracement, 0.786);
                        butterfly.bc_ab = Self::leg_accuracy(pattern.trade.bc_price_retracement, 0.382);
                        butterfly.cd_bc = Self::leg_accuracy(pattern.trade.cd_bc_price_retracement, 1.27);
                        butterfly.cd_xa = Self::leg_accuracy(pattern.trade.cd_xa_price_retracement, 1.618); 
                        butterfly.pattern_accuracy = (butterfly.ab_xa + butterfly.bc_ab + butterfly.cd_bc + butterfly.cd_xa) / 4.0;
                        butterfly.harmonic_type = "Butterfly".to_string();
                        accuracies.butterfly = butterfly
                    },
                    HarmonicType::Gartley => {
                        let mut gartley: PatternAccuracy = PatternAccuracy::default(); 
                        gartley.ab_xa = Self::leg_accuracy(pattern.trade.ab_price_retracement, 0.618);
                        gartley.bc_ab = Self::leg_accuracy(pattern.trade.bc_price_retracement, 0.382);
                        gartley.cd_bc = Self::leg_accuracy(pattern.trade.cd_bc_price_retracement, 1.27);
                        gartley.cd_xa = Self::leg_accuracy(pattern.trade.cd_xa_price_retracement, 0.786); 
                        gartley.pattern_accuracy = (gartley.ab_xa + gartley.bc_ab + gartley.cd_bc + gartley.cd_xa) / 4.0;   
                        gartley.harmonic_type = "Gartley".to_string();  
                        accuracies.gartley = gartley
                    },
                    HarmonicType::Crab => {
                        let mut crab: PatternAccuracy = PatternAccuracy::default(); 
                        crab.ab_xa = Self::leg_accuracy(pattern.trade.ab_price_retracement, 0.382);
                        crab.bc_ab = Self::leg_accuracy(pattern.trade.bc_price_retracement, 0.382);
                        crab.cd_bc = Self::leg_accuracy(pattern.trade.cd_bc_price_retracement, 3.618);
                        crab.cd_xa = Self::leg_accuracy(pattern.trade.cd_xa_price_retracement, 2.618); 
                        crab.pattern_accuracy = (crab.ab_xa + crab.bc_ab + crab.cd_bc + crab.cd_xa) / 4.0;
                        crab.harmonic_type = "Crab".to_string();    
                        accuracies.crab = crab
                    },
                    HarmonicType::Shark => {
                        let mut shark: PatternAccuracy = PatternAccuracy::default(); 
                        shark.ab_xa = Self::leg_accuracy(pattern.trade.ab_price_retracement, 0.886);
                        shark.bc_ab = Self::leg_accuracy(pattern.trade.bc_price_retracement, 0.382);
                        shark.cd_bc = Self::leg_accuracy(pattern.trade.cd_bc_price_retracement, 1.13);
                        shark.cd_xa = Self::leg_accuracy(pattern.trade.cd_xa_price_retracement, 1.618); 
                        shark.pattern_accuracy = (shark.ab_xa + shark.bc_ab + shark.cd_bc + shark.cd_xa) / 4.0;
                        shark.harmonic_type = "Shark".to_string();
                        accuracies.shark = shark
                    },
             
                    _ => {},
                }
                
                
            }
            // println!("{:?}", accuracies);

            pattern.accuracies = accuracies.clone();
            // all_accuracies.push(accuracies);
        }
           
        xabcd_patterns


    }

}