use crate::models::candle::Candle;
use crate::models::market::Market;
use crate::models::pivot::Pivot;
use crate::models::reversal_type::ReversalType;
use chrono::Datelike;
use chrono::NaiveDateTime;
use serde::Serialize;
#[derive(Debug, Clone, Copy, Serialize)]

pub struct Trade {
    pub open: bool,
    pub risk_exit_price: f64,
    pub reward_exit_price: f64,
    pub enter_price: f64,
    pub current_price: f64,
    pub length: i64,
    pub pnl: f64,
    pub result: i32,
    pub date: NaiveDateTime,
    pub entry_date: NaiveDateTime,
    pub ab_price_retracement: f64,
    pub bc_price_retracement: f64,
    pub cd_bc_price_retracement: f64,
    pub cd_price_retracement: f64,
    pub cd_xa_price_retracement: f64,
    pub ab_bar_retracement: f64,
    pub bc_bar_retracement: f64,
    pub cd_bar_retracement: f64,
    pub cd_bc_bar_retracement: f64,
    pub cd_xa_bar_retracement: f64,
    pub snr: f64,
    pub year: i64,
    pub month: i64,
    pub day: i64,
    pub reversal_type: ReversalType,
    pub bullish_key_reversal: bool,
    pub bearish_key_reversal: bool,
    pub bullish_engulfing: bool,
    pub bearish_engulfing: bool,
    pub bullish_outside_reversal: bool,
    pub bearish_outside_reversal: bool,
    pub hammer: bool,
    pub shooting_star: bool,
    pub morning_star: bool,
    pub evening_star: bool,
    pub three_white_soldiers: bool,
    pub three_black_crows: bool,
}

pub fn truncate_to_2_decimals(value: f64) -> f64 {
    (value * 100.0).trunc() / 100.0
}

impl Trade {
    pub fn new(
        market: Market,
        candle_x: &Pivot,
        candle_a: &Pivot,
        candle_b: &Pivot,
        candle_c: &Pivot,
        candle_d: &Candle,
        entry_candle: Option<&Candle>,
        snr: f64,
        candle_reversal: ReversalType,
    ) -> Trade {
        let entry_date = entry_candle.map(|candle| candle.date).unwrap_or(candle_d.date);
        let enter_price = entry_candle
            .map(|candle| candle.open)
            .unwrap_or(candle_d.close);
        let pnl = 0.0; // starting PnL

        // Use the actual C level as the target, then mirror that distance from
        // the delayed live entry for the stop.
        let (reward_exit_price, target_distance) = match market {
            Market::Bearish => (candle_c.low, enter_price - candle_c.low),
            Market::Bullish => (candle_c.high, candle_c.high - enter_price),
        };
        let target_distance = target_distance.max(0.0);
        let risk_exit_price = match market {
            Market::Bearish => enter_price + target_distance,
            Market::Bullish => enter_price - target_distance,
        };
        let current_price = enter_price;

        // --- PRICE RETRACEMENT ---
        let (xa_price_length, ab_price_length, bc_price_length, cd_price_length) = match market {
            Market::Bearish => (
                (candle_x.high - candle_a.low).abs(),
                (candle_a.low - candle_b.high).abs(),
                (candle_c.low - candle_b.high).abs(),
                (candle_c.low - candle_d.high).abs(),
            ),
            Market::Bullish => (
                (candle_a.high - candle_x.low).abs(),
                (candle_a.high - candle_b.low).abs(),
                (candle_c.high - candle_b.low).abs(),
                (candle_c.high - candle_d.low).abs(),
            ),
        };

        // AB -> XA
        let ab_price_retracement = if xa_price_length != 0.0 {
            (ab_price_length / xa_price_length) * 100.0
        } else {
            0.0
        };

        // BC -> XA
        let bc_price_retracement = if ab_price_length != 0.0 {
            (bc_price_length / ab_price_length) * 100.0
        } else {
            0.0
        };

        // CD -> AB
        let cd_price_retracement = if ab_price_length != 0.0 {
            (cd_price_length / ab_price_length) * 100.0
        } else {
            0.0
        };

        // CD -> BC
        let cd_bc_price_retracement = if bc_price_length != 0.0 {
            (cd_price_length / bc_price_length) * 100.0
        } else {
            0.0
        };

        // CD -> XA
        let cd_xa_price_retracement = if xa_price_length != 0.0 {
            (cd_price_length / xa_price_length) * 100.0
        } else {
            0.0
        };

        let x_len = candle_x.length as f64;
        let a_len = candle_a.length as f64;
        let b_len = candle_b.length as f64;
        let c_len = candle_c.length as f64;

        // --- LENGTH RETRACEMENT ---

        let ab_bar_retracement = if x_len != 0.0 {
            (a_len / x_len) * 100.0
        } else {
            0.0
        };

        let bc_bar_retracement = if a_len != 0.0 {
            (b_len / a_len) * 100.0
        } else {
            0.0
        };

        let cd_bar_retracement = if a_len != 0.0 {
            (c_len / a_len) * 100.0
        } else {
            0.0
        };

        let cd_bc_bar_retracement = if b_len != 0.0 {
            (c_len / b_len) * 100.0
        } else {
            0.0
        };

        let cd_xa_bar_retracement = if x_len != 0.0 {
            (c_len / x_len) * 100.0
        } else {
            0.0
        };

        // --- Construct Trade ---
        Trade {
            open: target_distance > f64::EPSILON,
            enter_price,
            current_price,
            pnl,
            risk_exit_price,
            reward_exit_price,
            length: 0,
            result: 0,
            date: entry_date,
            entry_date,
            ab_price_retracement,
            bc_price_retracement,
            cd_bc_price_retracement,
            cd_price_retracement,
            cd_xa_price_retracement,
            snr,
            year: entry_date.year() as i64,
            month: entry_date.month() as i64,
            day: entry_date.day() as i64,
            reversal_type: candle_reversal,
            bullish_key_reversal: false,
            bearish_key_reversal: false,
            bullish_engulfing: false,
            bearish_engulfing: false,
            bullish_outside_reversal: false,
            bearish_outside_reversal: false,
            hammer: false,
            shooting_star: false,
            morning_star: false,
            evening_star: false,
            three_white_soldiers: false,
            three_black_crows: false,
            ab_bar_retracement,
            bc_bar_retracement,
            cd_bar_retracement,
            cd_bc_bar_retracement,
            cd_xa_bar_retracement,
        }
    }
}
