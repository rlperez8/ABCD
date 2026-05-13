use std::collections::HashMap;

use chrono::NaiveDateTime;

use crate::models::{Candle, Market, PatternXABCD};

#[derive(Debug, Clone, Copy)]
pub struct ForwardObservationConfig {
    pub window_multiple: i64,
    pub min_bars: i64,
    pub max_bars: Option<i64>,
}

impl ForwardObservationConfig {
    pub fn sanitized(self) -> Self {
        let window_multiple = self.window_multiple.max(1);
        let min_bars = self.min_bars.max(1);
        let max_bars = self.max_bars.filter(|value| *value > 0);

        Self {
            window_multiple,
            min_bars,
            max_bars,
        }
    }
}

impl Default for ForwardObservationConfig {
    fn default() -> Self {
        Self {
            window_multiple: 5,
            min_bars: 1,
            max_bars: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PatternForwardObservation {
    pub observation_id: String,
    pub outcome_row_id: String,
    pub setup_id: String,
    pub prop_strategy_id: Option<String>,
    pub outcome_model: String,
    pub has_reversal: bool,
    pub reversal_type: String,
    pub reversal_detect_date: Option<NaiveDateTime>,
    pub reversal_bars_after_d: Option<i64>,
    pub pattern_id: Option<String>,
    pub pattern_group_id: String,
    pub x_bars_left: i64,
    pub x_strictness: String,
    pub symbol: String,
    pub root_symbol: Option<String>,
    pub contract_symbol: Option<String>,
    pub source_table: String,
    pub source_timeframe: String,
    pub d_date: NaiveDateTime,
    pub d_confirm_date: NaiveDateTime,
    pub entry_date: NaiveDateTime,
    pub contract_week_index: Option<i64>,
    pub contract_days_from_start: Option<i64>,
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
    pub formation_length: i64,
    pub window_multiple: i64,
    pub window_min_bars: i64,
    pub window_max_bars: Option<i64>,
    pub bars_requested: i64,
    pub bars_observed: i64,
    pub window_complete: bool,
    pub observation_start_date: NaiveDateTime,
    pub observation_end_date: NaiveDateTime,
    pub reference_price: f64,
    pub risk_points: f64,
    pub reward_points: f64,
    pub mfe_points: f64,
    pub mae_points: f64,
    pub mfe_r: Option<f64>,
    pub mae_r: Option<f64>,
    pub mfe_bar: i64,
    pub mae_bar: i64,
    pub mfe_price: f64,
    pub mae_price: f64,
    pub end_close_price: f64,
    pub end_close_return_points: f64,
    pub end_close_return_pct: Option<f64>,
    pub end_close_return_r: Option<f64>,
    pub close_return_1x_points: Option<f64>,
    pub close_return_1x_r: Option<f64>,
    pub close_return_2x_points: Option<f64>,
    pub close_return_2x_r: Option<f64>,
    pub close_return_3x_points: Option<f64>,
    pub close_return_3x_r: Option<f64>,
    pub close_return_5x_points: Option<f64>,
    pub close_return_5x_r: Option<f64>,
    pub hit_pos_0_5r_bar: Option<i64>,
    pub hit_pos_1_0r_bar: Option<i64>,
    pub hit_pos_1_5r_bar: Option<i64>,
    pub hit_pos_2_0r_bar: Option<i64>,
    pub hit_neg_0_5r_bar: Option<i64>,
    pub hit_neg_1_0r_bar: Option<i64>,
    pub hit_pos_1r_before_neg_1r: Option<bool>,
}

struct ObservationSeed {
    outcome_row_id: String,
    setup_id: String,
    prop_strategy_id: Option<String>,
    outcome_model: String,
    has_reversal: bool,
    reversal_type: String,
    reversal_detect_date: Option<NaiveDateTime>,
    reversal_bars_after_d: Option<i64>,
    pattern_id: Option<String>,
    pattern_group_id: String,
    x_bars_left: i64,
    symbol: String,
    root_symbol: Option<String>,
    contract_symbol: Option<String>,
    source_table: String,
    source_timeframe: String,
    d_date: NaiveDateTime,
    d_confirm_date: NaiveDateTime,
    entry_date: NaiveDateTime,
    contract_week_index: Option<i64>,
    contract_days_from_start: Option<i64>,
    market: Market,
    harmonic_type: String,
    bin: String,
    size_bucket: String,
    time_bin: String,
    three_month_trend: String,
    six_month_trend: String,
    twelve_month_trend: String,
    x_length: i64,
    a_length: i64,
    b_length: i64,
    c_length: i64,
    reference_price: f64,
    risk_points: f64,
    reward_points: f64,
}

pub fn build_forward_observations(
    patterns: &[PatternXABCD],
    candles: &[Candle],
    config: ForwardObservationConfig,
) -> Vec<PatternForwardObservation> {
    if candles.is_empty() {
        return Vec::new();
    }

    let config = config.sanitized();
    let entry_index_by_date: HashMap<NaiveDateTime, usize> = candles
        .iter()
        .enumerate()
        .map(|(index, candle)| (candle.date, index))
        .collect();
    let mut rows = Vec::new();

    for pattern in patterns {
        if pattern.pattern_id.is_empty() {
            continue;
        }

        let Some(entry_index) = pattern
            .entry_index
            .or_else(|| entry_index_by_date.get(&pattern.d_confirm_date).copied())
            .or_else(|| entry_index_by_date.get(&pattern.trade.entry_date).copied())
        else {
            continue;
        };
        let Some(anchor_candle) = candles.get(entry_index) else {
            continue;
        };

        let setup_id = pattern.pattern_id.clone();
        let outcome_row_id = format!("{:x}", md5::compute(format!("{setup_id}|D")));
        let lens = pattern.dominant_harmonic_lens();
        let reference_price = anchor_candle.open;
        let risk_points = neutral_risk_points(pattern, reference_price, anchor_candle);
        let seed = ObservationSeed {
            outcome_row_id,
            setup_id: setup_id.clone(),
            prop_strategy_id: non_empty_string(pattern.prop_strategy_id.clone()),
            outcome_model: "D".to_string(),
            has_reversal: false,
            reversal_type: format!("{:?}", pattern.trade.reversal_type),
            reversal_detect_date: None,
            reversal_bars_after_d: None,
            pattern_id: Some(setup_id),
            pattern_group_id: format!("{}{}", pattern.symbol, pattern.a.date),
            x_bars_left: pattern.x_bars_left,
            symbol: pattern.symbol.to_string(),
            root_symbol: pattern.root_symbol.as_deref().map(str::to_string),
            contract_symbol: pattern.contract_symbol.as_deref().map(str::to_string),
            source_table: pattern.source_table.to_string(),
            source_timeframe: pattern.source_timeframe.to_string(),
            d_date: pattern.d.date,
            d_confirm_date: pattern.d_confirm_date,
            entry_date: pattern.trade.entry_date,
            contract_week_index: pattern.contract_week_index,
            contract_days_from_start: pattern.contract_days_from_start,
            market: pattern.market,
            harmonic_type: lens.harmonic_type.to_string(),
            bin: lens.bin.to_string(),
            size_bucket: size_bucket(
                pattern.x.length + pattern.a.length + pattern.b.length + pattern.c.length,
            ),
            time_bin: lens.time_bin.to_string(),
            three_month_trend: trend_bucket(pattern.three_month),
            six_month_trend: trend_bucket(pattern.six_month),
            twelve_month_trend: trend_bucket(pattern.twelve_month),
            x_length: pattern.x.length,
            a_length: pattern.a.length,
            b_length: pattern.b.length,
            c_length: pattern.c.length,
            reference_price,
            risk_points,
            reward_points: risk_points,
        };

        if let Some(row) = build_observation_row(seed, candles, entry_index, config) {
            rows.push(row);
        }
    }

    rows
}

fn build_observation_row(
    seed: ObservationSeed,
    candles: &[Candle],
    entry_index: usize,
    config: ForwardObservationConfig,
) -> Option<PatternForwardObservation> {
    let start_candle = candles.get(entry_index)?;
    let formation_length = (seed.x_length + seed.a_length + seed.b_length + seed.c_length).max(1);
    let mut bars_requested = formation_length
        .saturating_mul(config.window_multiple)
        .max(config.min_bars);
    if let Some(max_bars) = config.max_bars {
        bars_requested = bars_requested.min(max_bars);
    }
    bars_requested = bars_requested.max(1);

    let requested_end_index = entry_index.saturating_add(bars_requested as usize - 1);
    let end_index = requested_end_index.min(candles.len().saturating_sub(1));
    if end_index < entry_index {
        return None;
    }

    let risk_points = seed.risk_points.abs();
    let reward_points = seed.reward_points.abs();
    let risk_r = (risk_points > f64::EPSILON).then_some(risk_points);

    let mut mfe_points = 0.0;
    let mut mae_points = 0.0;
    let mut mfe_bar = 1i64;
    let mut mae_bar = 1i64;
    let mut mfe_price = seed.reference_price;
    let mut mae_price = seed.reference_price;
    let mut hit_pos_0_5r_bar = None;
    let mut hit_pos_1_0r_bar = None;
    let mut hit_pos_1_5r_bar = None;
    let mut hit_pos_2_0r_bar = None;
    let mut hit_neg_0_5r_bar = None;
    let mut hit_neg_1_0r_bar = None;

    for (offset, candle) in candles[entry_index..=end_index].iter().enumerate() {
        let bar = offset as i64 + 1;
        let (favorable_points, favorable_price, adverse_points, adverse_price) =
            candle_excursion(seed.market, seed.reference_price, candle);

        if favorable_points > mfe_points {
            mfe_points = favorable_points;
            mfe_bar = bar;
            mfe_price = favorable_price;
        }
        if adverse_points > mae_points {
            mae_points = adverse_points;
            mae_bar = bar;
            mae_price = adverse_price;
        }

        if let Some(risk_points) = risk_r {
            set_first_hit(
                &mut hit_pos_0_5r_bar,
                favorable_points,
                risk_points * 0.5,
                bar,
            );
            set_first_hit(&mut hit_pos_1_0r_bar, favorable_points, risk_points, bar);
            set_first_hit(
                &mut hit_pos_1_5r_bar,
                favorable_points,
                risk_points * 1.5,
                bar,
            );
            set_first_hit(
                &mut hit_pos_2_0r_bar,
                favorable_points,
                risk_points * 2.0,
                bar,
            );
            set_first_hit(
                &mut hit_neg_0_5r_bar,
                adverse_points,
                risk_points * 0.5,
                bar,
            );
            set_first_hit(&mut hit_neg_1_0r_bar, adverse_points, risk_points, bar);
        }
    }

    let end_candle = &candles[end_index];
    let bars_observed = end_index.saturating_sub(entry_index) as i64 + 1;
    let end_close_return_points =
        close_return_points(seed.market, seed.reference_price, end_candle.close);
    let (close_return_1x_points, close_return_1x_r) = close_return_at_horizon(
        candles,
        entry_index,
        requested_end_index,
        formation_length,
        seed.market,
        seed.reference_price,
        risk_r,
    );
    let (close_return_2x_points, close_return_2x_r) = close_return_at_horizon(
        candles,
        entry_index,
        requested_end_index,
        formation_length.saturating_mul(2),
        seed.market,
        seed.reference_price,
        risk_r,
    );
    let (close_return_3x_points, close_return_3x_r) = close_return_at_horizon(
        candles,
        entry_index,
        requested_end_index,
        formation_length.saturating_mul(3),
        seed.market,
        seed.reference_price,
        risk_r,
    );
    let (close_return_5x_points, close_return_5x_r) = close_return_at_horizon(
        candles,
        entry_index,
        requested_end_index,
        formation_length.saturating_mul(5),
        seed.market,
        seed.reference_price,
        risk_r,
    );
    let window_max_bars = config.max_bars;
    let observation_id = format!(
        "{:x}",
        md5::compute(
            format!(
                "{}|{}|{}|{}|{}",
                seed.outcome_row_id,
                config.window_multiple,
                config.min_bars,
                window_max_bars
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                seed.entry_date
            )
            .as_bytes(),
        )
    );

    Some(PatternForwardObservation {
        observation_id,
        outcome_row_id: seed.outcome_row_id,
        setup_id: seed.setup_id,
        prop_strategy_id: seed.prop_strategy_id,
        outcome_model: seed.outcome_model,
        has_reversal: seed.has_reversal,
        reversal_type: seed.reversal_type,
        reversal_detect_date: seed.reversal_detect_date,
        reversal_bars_after_d: seed.reversal_bars_after_d,
        pattern_id: seed.pattern_id,
        pattern_group_id: seed.pattern_group_id,
        x_bars_left: seed.x_bars_left,
        x_strictness: x_strictness(seed.x_bars_left, seed.x_length),
        symbol: seed.symbol,
        root_symbol: seed.root_symbol,
        contract_symbol: seed.contract_symbol,
        source_table: seed.source_table,
        source_timeframe: seed.source_timeframe,
        d_date: seed.d_date,
        d_confirm_date: seed.d_confirm_date,
        entry_date: seed.entry_date,
        contract_week_index: seed.contract_week_index,
        contract_days_from_start: seed.contract_days_from_start,
        market: market_label(seed.market).to_string(),
        harmonic_type: seed.harmonic_type,
        bin: seed.bin,
        size_bucket: seed.size_bucket,
        time_bin: seed.time_bin,
        three_month_trend: seed.three_month_trend,
        six_month_trend: seed.six_month_trend,
        twelve_month_trend: seed.twelve_month_trend,
        x_length: seed.x_length,
        a_length: seed.a_length,
        b_length: seed.b_length,
        c_length: seed.c_length,
        formation_length,
        window_multiple: config.window_multiple,
        window_min_bars: config.min_bars,
        window_max_bars,
        bars_requested,
        bars_observed,
        window_complete: requested_end_index < candles.len(),
        observation_start_date: start_candle.date,
        observation_end_date: end_candle.date,
        reference_price: seed.reference_price,
        risk_points,
        reward_points,
        mfe_points,
        mae_points,
        mfe_r: risk_r.map(|risk_points| mfe_points / risk_points),
        mae_r: risk_r.map(|risk_points| mae_points / risk_points),
        mfe_bar,
        mae_bar,
        mfe_price,
        mae_price,
        end_close_price: end_candle.close,
        end_close_return_points,
        end_close_return_pct: pct_return(seed.reference_price, end_close_return_points),
        end_close_return_r: risk_r.map(|risk_points| end_close_return_points / risk_points),
        close_return_1x_points,
        close_return_1x_r,
        close_return_2x_points,
        close_return_2x_r,
        close_return_3x_points,
        close_return_3x_r,
        close_return_5x_points,
        close_return_5x_r,
        hit_pos_0_5r_bar,
        hit_pos_1_0r_bar,
        hit_pos_1_5r_bar,
        hit_pos_2_0r_bar,
        hit_neg_0_5r_bar,
        hit_neg_1_0r_bar,
        hit_pos_1r_before_neg_1r: first_pos_before_neg(hit_pos_1_0r_bar, hit_neg_1_0r_bar),
    })
}

fn candle_excursion(market: Market, reference_price: f64, candle: &Candle) -> (f64, f64, f64, f64) {
    match market {
        Market::Bullish => (
            (candle.high - reference_price).max(0.0),
            candle.high,
            (reference_price - candle.low).max(0.0),
            candle.low,
        ),
        Market::Bearish => (
            (reference_price - candle.low).max(0.0),
            candle.low,
            (candle.high - reference_price).max(0.0),
            candle.high,
        ),
    }
}

fn close_return_points(market: Market, reference_price: f64, close: f64) -> f64 {
    match market {
        Market::Bullish => close - reference_price,
        Market::Bearish => reference_price - close,
    }
}

fn close_return_at_horizon(
    candles: &[Candle],
    entry_index: usize,
    requested_end_index: usize,
    horizon_bars: i64,
    market: Market,
    reference_price: f64,
    risk_points: Option<f64>,
) -> (Option<f64>, Option<f64>) {
    if horizon_bars <= 0 {
        return (None, None);
    }

    let Some(horizon_index) = entry_index.checked_add(horizon_bars as usize - 1) else {
        return (None, None);
    };
    if horizon_index > requested_end_index {
        return (None, None);
    }

    let Some(candle) = candles.get(horizon_index) else {
        return (None, None);
    };

    let points = close_return_points(market, reference_price, candle.close);
    (Some(points), risk_points.map(|risk| points / risk))
}

fn set_first_hit(slot: &mut Option<i64>, value: f64, threshold: f64, bar: i64) {
    if slot.is_none() && value >= threshold {
        *slot = Some(bar);
    }
}

fn first_pos_before_neg(pos_bar: Option<i64>, neg_bar: Option<i64>) -> Option<bool> {
    match (pos_bar, neg_bar) {
        (Some(pos), Some(neg)) => Some(pos <= neg),
        (Some(_), None) => Some(true),
        (None, Some(_)) => Some(false),
        (None, None) => None,
    }
}

fn pct_return(reference_price: f64, points: f64) -> Option<f64> {
    (reference_price.abs() > f64::EPSILON).then_some((points / reference_price.abs()) * 100.0)
}

fn neutral_risk_points(
    pattern: &PatternXABCD,
    reference_price: f64,
    anchor_candle: &Candle,
) -> f64 {
    let candidates = [
        (pattern.c.min_max - pattern.d.min_max).abs(),
        (pattern.d.high - pattern.d.low).abs(),
        (anchor_candle.high - anchor_candle.low).abs(),
        reference_price.abs() * 0.001,
        1.0,
    ];
    candidates
        .into_iter()
        .find(|value| value.is_finite() && *value > f64::EPSILON)
        .unwrap_or(1.0)
}

fn market_label(market: Market) -> &'static str {
    match market {
        Market::Bullish => "Bullish",
        Market::Bearish => "Bearish",
    }
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

fn x_strictness(x_bars_left: i64, x_length: i64) -> String {
    let x_length = x_length.max(0);
    let normal_threshold = ((x_length as f64) / 2.0).ceil() as i64;

    if x_bars_left >= x_length {
        "Strict".to_string()
    } else if x_bars_left >= normal_threshold {
        "Normal".to_string()
    } else {
        "Loose".to_string()
    }
}

fn non_empty_string(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}
