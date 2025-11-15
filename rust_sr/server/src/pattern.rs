use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[derive(Debug, Deserialize, Serialize, Clone)]


pub struct Pattern {
    pub symbol: String,
    pub a_date: String,
    pub a_open: f64,
    pub a_high: f64,
    pub a_low: f64,
    pub a_close: f64,
    pub a_length: f64,
    pub a_min_max: f64,
    pub b_date: String,
    pub b_open: f64,
    pub b_high: f64,
    pub b_low: f64,
    pub b_close: f64,
    pub b_length: f64,
    pub b_min_max: f64,
    pub c_date: String,
    pub c_open: f64,
    pub c_high: f64,
    pub c_low: f64,
    pub c_close: f64,
    pub c_length: f64,
    pub c_min_max: f64,
    pub d_date: String,
    pub d_open: f64,
    pub d_high: f64,
    pub d_low: f64,
    pub d_close: f64,
    pub d_length: f64,
    pub d_min_max: f64,
    pub trade_open: String,
    pub trade_risk_exit_price: f64,
    pub trade_reward_exit_price: f64,
    pub trade_enter_price: f64,
    pub trade_current_price: f64,
    pub trade_length: f64,
    pub trade_pnl: f64,
    pub trade_result: String,
    pub trade_date: String,
    pub trade_bc_price_retracement: f64,
    pub trade_cd_price_retracement: f64,
    pub trade_snr: f64,
    pub trade_year: f64,
    pub trade_month: f64,
    pub trade_day: f64,
}

impl Pattern {

    pub fn count_closed(patterns: &[Pattern]) -> usize {
        patterns
            .iter()
            .filter(|p| p.trade_open == "false")
            .count()
    }

    pub fn count_open(patterns: &[Pattern]) -> usize {
        patterns
            .iter()
            .filter(|p| p.trade_open == "true")
            .count()
    }

    pub fn count_wins(patterns: &[Pattern]) -> usize {
        patterns
            .iter()
            .filter(|p| p.trade_result == "true")
            .count()
    }

    pub fn count_lost(patterns: &[Pattern]) -> usize {
        patterns
            .iter()
            .filter(|p| p.trade_result == "false")
            .count()
    }

    pub fn group_by_month(patterns: &[Pattern]) -> HashMap<u32, Vec<Pattern>> {
        let mut grouped = HashMap::new();
        for p in patterns {
            grouped.entry(p.trade_month as u32).or_insert_with(Vec::new).push(p.clone());
        }
        grouped
    }



}