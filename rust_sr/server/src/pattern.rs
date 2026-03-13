
use mysql::{Pool, PooledConn, params};
use mysql::prelude::*; 
use std::error::Error;
use mysql::Row;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::NaiveDate;
use rust_decimal::Decimal;

#[derive(Debug, Deserialize, Serialize, Clone, sqlx::FromRow)]
pub struct Pattern {
    pub symbol: String,
    pub x_date: NaiveDate,
    pub x_open: Decimal,
    pub x_high: Decimal,
    pub x_low: Decimal,
    pub x_close: Decimal,
    pub x_length: Decimal,
    pub x_min_max: Decimal,
    pub a_date: NaiveDate,
    pub a_open: Decimal,
    pub a_high: Decimal,
    pub a_low: Decimal,
    pub a_close: Decimal,
    pub a_length: Decimal,
    pub a_min_max: Decimal,
    pub b_date: NaiveDate,
    pub b_open: Decimal,
    pub b_high: Decimal,
    pub b_low: Decimal,
    pub b_close: Decimal,
    pub b_length: Decimal,
    pub b_min_max: Decimal,
    pub c_date: NaiveDate,
    pub c_open: Decimal,
    pub c_high: Decimal,
    pub c_low: Decimal,
    pub c_close: Decimal,
    pub c_length: Decimal,
    pub c_min_max: Decimal,
    pub d_date: NaiveDate,
    pub d_open: Decimal,
    pub d_high: Decimal,
    pub d_low: Decimal,
    pub d_close: Decimal,
    pub d_length: Decimal,
    pub d_min_max: Decimal,
    pub trade_open: bool,
    pub trade_risk_exit_price: Decimal,
    pub trade_reward_exit_price: Decimal,
    pub trade_enter_price: Decimal,
    pub trade_current_price: Decimal,
    pub trade_length: Decimal,
    pub trade_pnl: Decimal,
    pub trade_result: i64,
    pub trade_date: NaiveDate,
    pub trade_ab_price_retracement: Decimal,
    pub trade_bc_price_retracement: Decimal,
    pub trade_cd_bc_price_retracement: Decimal,
    pub trade_cd_price_retracement: Decimal,
    pub trade_cd_xa_price_retracement: Decimal,
    pub trade_bc_bar_retracement: Decimal,
    pub trade_cd_bar_retracement: Decimal,
    pub trade_snr: Decimal,
    pub trade_year: i64,
    pub trade_month: i64,
    pub trade_day: i64,
    // pub reversalType: Option<String>,
    pub market: String,
    pub three_month: Decimal,
    pub six_month: Decimal,
    pub twelve_month: Decimal,
    pub pattern_group_id: String,
    pub harmonic_type: Option<String>

}

impl Pattern {

    pub fn count_closed(patterns: &[Pattern]) -> usize {
        patterns
            .iter()
            .filter(|p| p.trade_open == false)
            .count()
    }

    pub fn count_open(patterns: &[Pattern]) -> usize {
        patterns
            .iter()
            .filter(|p| p.trade_open == true)
            .count()
    }

    pub fn count_wins(patterns: &[Pattern]) -> usize {
        patterns
            .iter()
            .filter(|p| p.trade_result == 1)
            .count()
    }

    pub fn count_lost(patterns: &[Pattern]) -> usize {
        patterns
            .iter()
            .filter(|p| p.trade_result == 2)
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