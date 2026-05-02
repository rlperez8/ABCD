use chrono::NaiveDateTime;

use crate::models::{Candle, Market, PatternXABCD};

#[derive(Debug, Clone)]
pub struct PropReversalOutcome {
    pub reversal_row_id: String,
    pub setup_id: String,
    pub prop_strategy_id: String,
    pub pattern_id: Option<String>,
    pub pattern_group_id: String,
    pub x_bars_left: i64,
    pub symbol: String,
    pub d_date: NaiveDateTime,
    pub entry_date: NaiveDateTime,
    pub d_confirm_date: NaiveDateTime,
    pub contract_week_index: Option<i64>,
    pub contract_days_from_start: Option<i64>,
    pub reversal_type: String,
    pub reversal_detect_date: NaiveDateTime,
    pub reversal_bars_after_d: i64,
    pub market: String,
    pub harmonic_type: String,
    pub bin: String,
    pub size_bucket: String,
    pub time_bin: String,
    pub three_month_trend: String,
    pub six_month_trend: String,
    pub twelve_month_trend: String,
    pub x_length: i64,
    pub a_length: i64,
    pub b_length: i64,
    pub c_length: i64,
    pub d_length: i64,
    pub full_pattern_length: i64,
    pub trade_enter_price: f64,
    pub trade_risk_exit_price: f64,
    pub trade_reward_exit_price: f64,
    pub trade_result: i32,
    pub target_ready: bool,
    pub target_date: Option<NaiveDateTime>,
    pub target_open: Option<f64>,
    pub target_high: Option<f64>,
    pub target_low: Option<f64>,
    pub target_close: Option<f64>,
    pub target_volume: Option<i64>,
    pub target_is_green: Option<bool>,
    pub target_close_vs_open_pct: Option<f64>,
    pub target_high_vs_open_pct: Option<f64>,
    pub target_low_vs_open_pct: Option<f64>,
    pub target_range_pct: Option<f64>,
    pub target_breaks_reversal_high: Option<bool>,
    pub target_breaks_reversal_low: Option<bool>,
}

#[derive(Debug, Clone, Copy)]
struct ReversalCandidate {
    reversal_type: &'static str,
    completion_offset: usize,
}

fn pct_change(base: f64, value: f64) -> f64 {
    if base.abs() <= f64::EPSILON {
        0.0
    } else {
        ((value - base) / base) * 100.0
    }
}

fn range_pct(open: f64, high: f64, low: f64) -> f64 {
    if open.abs() <= f64::EPSILON {
        0.0
    } else {
        ((high - low) / open) * 100.0
    }
}

#[derive(Clone)]
struct SimulatedTargetTrade {
    entry_date: NaiveDateTime,
    enter_price: f64,
    risk_exit_price: f64,
    reward_exit_price: f64,
    result: i32,
    d_length: i64,
    target_candle: Option<Candle>,
}

fn simulate_c_target_trade(
    pattern: &PatternXABCD,
    candles: &[Candle],
    entry_index: usize,
) -> Option<SimulatedTargetTrade> {
    let entry_candle = candles.get(entry_index)?.clone();
    let (reward_exit_price, target_distance) = match pattern.market {
        Market::Bullish => (pattern.c.high, pattern.c.high - entry_candle.open),
        Market::Bearish => (pattern.c.low, entry_candle.open - pattern.c.low),
    };

    if target_distance <= f64::EPSILON {
        return Some(SimulatedTargetTrade {
            entry_date: entry_candle.date,
            enter_price: entry_candle.open,
            risk_exit_price: entry_candle.open,
            reward_exit_price,
            result: 0,
            d_length: entry_index.saturating_sub(pattern.reversal_context.d_index) as i64,
            target_candle: None,
        });
    }

    let risk_exit_price = match pattern.market {
        Market::Bullish => entry_candle.open - target_distance,
        Market::Bearish => entry_candle.open + target_distance,
    };
    let mut d_length = entry_index.saturating_sub(pattern.reversal_context.d_index) as i64;

    for (index, candle) in candles.iter().enumerate().skip(entry_index) {
        d_length = index.saturating_sub(pattern.reversal_context.d_index) as i64;
        match pattern.market {
            Market::Bullish => {
                if candle.high >= reward_exit_price
                    || candle.close >= reward_exit_price
                    || candle.open >= reward_exit_price
                    || candle.low >= reward_exit_price
                {
                    return Some(SimulatedTargetTrade {
                        entry_date: entry_candle.date,
                        enter_price: entry_candle.open,
                        risk_exit_price,
                        reward_exit_price,
                        result: 1,
                        d_length,
                        target_candle: Some(candle.clone()),
                    });
                }

                if candle.low <= risk_exit_price
                    || candle.close <= risk_exit_price
                    || candle.open <= risk_exit_price
                    || candle.high <= risk_exit_price
                {
                    return Some(SimulatedTargetTrade {
                        entry_date: entry_candle.date,
                        enter_price: entry_candle.open,
                        risk_exit_price,
                        reward_exit_price,
                        result: 2,
                        d_length,
                        target_candle: Some(candle.clone()),
                    });
                }
            }
            Market::Bearish => {
                if candle.low <= reward_exit_price
                    || candle.close <= reward_exit_price
                    || candle.open <= reward_exit_price
                    || candle.high <= reward_exit_price
                {
                    return Some(SimulatedTargetTrade {
                        entry_date: entry_candle.date,
                        enter_price: entry_candle.open,
                        risk_exit_price,
                        reward_exit_price,
                        result: 1,
                        d_length,
                        target_candle: Some(candle.clone()),
                    });
                }

                if candle.high >= risk_exit_price
                    || candle.close >= risk_exit_price
                    || candle.open >= risk_exit_price
                    || candle.low >= risk_exit_price
                {
                    return Some(SimulatedTargetTrade {
                        entry_date: entry_candle.date,
                        enter_price: entry_candle.open,
                        risk_exit_price,
                        reward_exit_price,
                        result: 2,
                        d_length,
                        target_candle: Some(candle.clone()),
                    });
                }
            }
        }
    }

    Some(SimulatedTargetTrade {
        entry_date: entry_candle.date,
        enter_price: entry_candle.open,
        risk_exit_price,
        reward_exit_price,
        result: 0,
        d_length,
        target_candle: None,
    })
}

fn trend_bucket(value: Option<bool>) -> String {
    match value {
        Some(true) => "Bullish".to_string(),
        Some(false) => "Bearish".to_string(),
        None => "Unknown".to_string(),
    }
}

fn size_bucket(total_bars: i64) -> String {
    if total_bars <= 20 {
        "Micro".to_string()
    } else if total_bars <= 60 {
        "Small".to_string()
    } else if total_bars <= 180 {
        "Normal".to_string()
    } else if total_bars <= 365 {
        "Large".to_string()
    } else {
        "Massive".to_string()
    }
}

fn build_prop_strategy_id(
    market: &str,
    harmonic_type: &str,
    bin: &str,
    reversal_type: &str,
    size_bucket: &str,
    time_bin: &str,
    three_month_trend: &str,
    six_month_trend: &str,
    twelve_month_trend: &str,
) -> String {
    let canonical_key = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}",
        market.to_lowercase(),
        harmonic_type.to_lowercase(),
        bin.to_lowercase(),
        reversal_type.to_lowercase(),
        size_bucket.to_lowercase(),
        time_bin.to_lowercase(),
        three_month_trend.to_lowercase(),
        six_month_trend.to_lowercase(),
        twelve_month_trend.to_lowercase(),
    );

    let hash = format!("{:x}", md5::compute(canonical_key.as_bytes()));
    hash[..16].to_string()
}

fn signal_candidates(pattern: &PatternXABCD) -> Vec<ReversalCandidate> {
    let mut candidates = Vec::new();

    if pattern.trade.bullish_key_reversal {
        candidates.push(ReversalCandidate {
            reversal_type: "BullishKeyReversal",
            completion_offset: 0,
        });
    }
    if pattern.trade.bearish_key_reversal {
        candidates.push(ReversalCandidate {
            reversal_type: "BearishKeyReversal",
            completion_offset: 0,
        });
    }
    if pattern.trade.bullish_engulfing {
        candidates.push(ReversalCandidate {
            reversal_type: "BullishEngulfing",
            completion_offset: 0,
        });
    }
    if pattern.trade.bearish_engulfing {
        candidates.push(ReversalCandidate {
            reversal_type: "BearishEngulfing",
            completion_offset: 0,
        });
    }
    if pattern.trade.bullish_outside_reversal {
        candidates.push(ReversalCandidate {
            reversal_type: "BullishOutsideReversal",
            completion_offset: 0,
        });
    }
    if pattern.trade.bearish_outside_reversal {
        candidates.push(ReversalCandidate {
            reversal_type: "BearishOutsideReversal",
            completion_offset: 0,
        });
    }
    if pattern.trade.hammer {
        candidates.push(ReversalCandidate {
            reversal_type: "Hammer",
            completion_offset: 0,
        });
    }
    if pattern.trade.morning_star {
        candidates.push(ReversalCandidate {
            reversal_type: "MorningStar",
            completion_offset: 1,
        });
    }
    if pattern.trade.shooting_star {
        candidates.push(ReversalCandidate {
            reversal_type: "ShootingStar",
            completion_offset: 1,
        });
    }
    if pattern.trade.evening_star {
        candidates.push(ReversalCandidate {
            reversal_type: "EveningStar",
            completion_offset: 1,
        });
    }
    if pattern.trade.three_white_soldiers {
        candidates.push(ReversalCandidate {
            reversal_type: "ThreeWhiteSoldiers",
            completion_offset: 2,
        });
    }
    if pattern.trade.three_black_crows {
        candidates.push(ReversalCandidate {
            reversal_type: "ThreeBlackCrows",
            completion_offset: 2,
        });
    }

    candidates
}

pub fn build_prop_reversal_outcomes(
    patterns: &[PatternXABCD],
    candles: &[Candle],
) -> Vec<PropReversalOutcome> {
    let mut rows = Vec::new();

    for pattern in patterns {
        let setup_id = if pattern.pattern_id.is_empty() {
            continue;
        } else {
            pattern.pattern_id.clone()
        };
        let market = format!("{:?}", pattern.market);
        let lens = pattern.dominant_harmonic_lens();
        let size_bucket =
            size_bucket(pattern.x.length + pattern.a.length + pattern.b.length + pattern.c.length);
        let three_month_trend = trend_bucket(pattern.three_month);
        let six_month_trend = trend_bucket(pattern.six_month);
        let twelve_month_trend = trend_bucket(pattern.twelve_month);
        let d_index = pattern.reversal_context.d_index;

        for candidate in signal_candidates(pattern) {
            let completion_index = d_index + candidate.completion_offset;
            let Some(reversal_candle) = candles.get(completion_index) else {
                continue;
            };

            let entry_index = std::cmp::max(d_index + 2, completion_index + 1);
            let Some(simulated_trade) = simulate_c_target_trade(pattern, candles, entry_index)
            else {
                continue;
            };
            let target_candle = simulated_trade.target_candle.as_ref();
            let target_ready = target_candle.is_some();
            let target_is_green = target_candle.map(|candle| candle.close > candle.open);
            let trade_enter_price = simulated_trade.enter_price;
            let trade_risk_exit_price = simulated_trade.risk_exit_price;
            let trade_reward_exit_price = simulated_trade.reward_exit_price;
            let trade_result = simulated_trade.result;
            let prop_strategy_id = build_prop_strategy_id(
                &market,
                lens.harmonic_type,
                lens.bin,
                candidate.reversal_type,
                &size_bucket,
                lens.time_bin,
                &three_month_trend,
                &six_month_trend,
                &twelve_month_trend,
            );
            let reversal_row_id = format!(
                "{:x}",
                md5::compute(
                    format!(
                        "{setup_id}|{}|{}",
                        candidate.reversal_type, reversal_candle.date
                    )
                    .as_bytes(),
                )
            );

            rows.push(PropReversalOutcome {
                reversal_row_id,
                setup_id: setup_id.clone(),
                prop_strategy_id,
                pattern_id: Some(pattern.pattern_id.clone()).filter(|value| !value.is_empty()),
                pattern_group_id: format!("{}{}", pattern.symbol, pattern.a.date),
                x_bars_left: pattern.x_bars_left,
                symbol: pattern.symbol.to_string(),
                d_date: pattern.d.date,
                entry_date: simulated_trade.entry_date,
                d_confirm_date: pattern.d_confirm_date,
                contract_week_index: pattern.contract_week_index,
                contract_days_from_start: pattern.contract_days_from_start,
                reversal_type: candidate.reversal_type.to_string(),
                reversal_detect_date: reversal_candle.date,
                reversal_bars_after_d: candidate.completion_offset as i64,
                market: market.clone(),
                harmonic_type: lens.harmonic_type.to_string(),
                bin: lens.bin.to_string(),
                size_bucket: size_bucket.clone(),
                time_bin: lens.time_bin.to_string(),
                three_month_trend: three_month_trend.clone(),
                six_month_trend: six_month_trend.clone(),
                twelve_month_trend: twelve_month_trend.clone(),
                x_length: pattern.x.length,
                a_length: pattern.a.length,
                b_length: pattern.b.length,
                c_length: pattern.c.length,
                d_length: simulated_trade.d_length,
                full_pattern_length: pattern.x.length
                    + pattern.a.length
                    + pattern.b.length
                    + pattern.c.length
                    + simulated_trade.d_length,
                trade_enter_price,
                trade_risk_exit_price,
                trade_reward_exit_price,
                trade_result,
                target_ready,
                target_date: target_candle.map(|candle| candle.date),
                target_open: target_candle.map(|candle| candle.open),
                target_high: target_candle.map(|candle| candle.high),
                target_low: target_candle.map(|candle| candle.low),
                target_close: target_candle.map(|candle| candle.close),
                target_volume: target_candle.map(|candle| candle.volume),
                target_is_green,
                target_close_vs_open_pct: target_candle
                    .map(|candle| pct_change(candle.open, candle.close)),
                target_high_vs_open_pct: target_candle
                    .map(|candle| pct_change(candle.open, candle.high)),
                target_low_vs_open_pct: target_candle
                    .map(|candle| pct_change(candle.open, candle.low)),
                target_range_pct: target_candle
                    .map(|candle| range_pct(candle.open, candle.high, candle.low)),
                target_breaks_reversal_high: target_candle
                    .and_then(|candle| candles.get(entry_index).map(|entry| candle.high > entry.high)),
                target_breaks_reversal_low: target_candle
                    .and_then(|candle| candles.get(entry_index).map(|entry| candle.low < entry.low)),
            });
        }
    }

    rows
}
