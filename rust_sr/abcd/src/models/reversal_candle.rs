use crate::models::candle::Candle;
use crate::models::market::Market;
use crate::models::reversal_type::ReversalType;

#[derive(Debug, Clone, Copy)]
pub struct ReversalCandleSignals {
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

impl ReversalCandleSignals {
    pub fn none() -> Self {
        Self {
            reversal_type: ReversalType::None,
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
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ReversalPatternContext {
    pub d_index: usize,
}

fn candle_body(candle: &Candle) -> f64 {
    (candle.close - candle.open).abs()
}

fn upper_shadow(candle: &Candle) -> f64 {
    candle.high - candle.open.max(candle.close)
}

fn lower_shadow(candle: &Candle) -> f64 {
    candle.open.min(candle.close) - candle.low
}

fn is_bullish(candle: &Candle) -> bool {
    candle.close > candle.open
}

fn is_bearish(candle: &Candle) -> bool {
    candle.close < candle.open
}

fn body_dominates_range(candle: &Candle, minimum_ratio: f64) -> bool {
    let range = (candle.high - candle.low).abs();
    range > 0.0 && candle_body(candle) >= range * minimum_ratio
}

fn set_primary_reversal_type(signals: &mut ReversalCandleSignals, reversal_type: ReversalType) {
    if matches!(signals.reversal_type, ReversalType::None) {
        signals.reversal_type = reversal_type;
    }
}

pub fn classify_closed_reversal_pattern(
    candles: &[Candle],
    context: &ReversalPatternContext,
    close_index: usize,
    market: Market,
) -> ReversalCandleSignals {
    if close_index >= candles.len() || context.d_index == 0 || close_index <= context.d_index {
        return ReversalCandleSignals::none();
    }

    let observed_candles = &candles[..=close_index];
    let Some(previous) = observed_candles.get(context.d_index.saturating_sub(1)) else {
        return ReversalCandleSignals::none();
    };
    let Some(d_candle) = observed_candles.get(context.d_index) else {
        return ReversalCandleSignals::none();
    };
    let Some(confirmation) = observed_candles.get(context.d_index + 1) else {
        return ReversalCandleSignals::none();
    };

    let previous_body = candle_body(previous);
    let d_body = candle_body(d_candle);
    let d_range = (d_candle.high - d_candle.low).abs();
    let confirmation_body = candle_body(confirmation);

    if previous_body <= 0.0 || d_range <= 0.0 || confirmation_body <= 0.0 {
        return ReversalCandleSignals::none();
    }

    let mut signals = ReversalCandleSignals::none();
    let previous_midpoint = (previous.open + previous.close) / 2.0;
    let previous_is_bearish = is_bearish(previous);
    let previous_is_bullish = is_bullish(previous);
    let d_is_bullish = is_bullish(d_candle);
    let d_is_bearish = is_bearish(d_candle);
    let d_is_small = d_body <= previous_body * 0.5 && d_body <= d_range * 0.35;
    let d_stalls_lower = d_candle.close <= previous.close || d_candle.low < previous.low;
    let confirmation_is_bullish = is_bullish(confirmation);
    let confirmation_reclaims_d =
        confirmation.close > d_candle.close && confirmation.high >= d_candle.high;
    let confirmation_closes_into_prior_body = confirmation.close >= previous_midpoint;

    if market == Market::Bullish {
        let body_engulfs_previous =
            d_candle.open <= previous.close && d_candle.close >= previous.open;
        let d_reclaims_previous_body = d_candle.close >= previous_midpoint;
        let d_body_is_meaningful = d_body >= previous_body * 1.05 && d_body >= d_range * 0.45;

        if previous_is_bearish
            && d_is_bullish
            && body_engulfs_previous
            && d_reclaims_previous_body
            && d_body_is_meaningful
        {
            set_primary_reversal_type(&mut signals, ReversalType::BullishEngulfing);
            signals.bullish_engulfing = true;
        }
    }

    if market == Market::Bearish {
        let body_engulfs_previous =
            d_candle.open >= previous.close && d_candle.close <= previous.open;
        let d_reclaims_previous_body = d_candle.close <= previous_midpoint;
        let d_body_is_meaningful = d_body >= previous_body * 1.05 && d_body >= d_range * 0.45;

        if previous_is_bullish
            && d_is_bearish
            && body_engulfs_previous
            && d_reclaims_previous_body
            && d_body_is_meaningful
        {
            set_primary_reversal_type(&mut signals, ReversalType::BearishEngulfing);
            signals.bearish_engulfing = true;
        }
    }

    if previous_is_bearish
        && d_is_small
        && d_stalls_lower
        && confirmation_is_bullish
        && confirmation_reclaims_d
        && confirmation_closes_into_prior_body
    {
        set_primary_reversal_type(&mut signals, ReversalType::MorningStar);
        signals.morning_star = true;
    }

    if market == Market::Bearish {
        let d_upper_shadow = upper_shadow(d_candle);
        let d_lower_shadow = lower_shadow(d_candle);
        let d_rejects_higher = d_candle.high > previous.high || d_candle.close >= previous.close;
        let d_has_long_upper_shadow =
            d_upper_shadow >= d_body * 2.0 && d_upper_shadow >= d_range * 0.45;
        let d_has_small_lower_shadow = d_lower_shadow <= d_body && d_lower_shadow <= d_range * 0.2;
        let confirmation_is_bearish = is_bearish(confirmation);
        let confirmation_falls_from_d = confirmation.close < d_candle.close
            && confirmation.close <= d_candle.open.min(d_candle.close);
        let confirmation_closes_into_prior_body = confirmation.close <= previous_midpoint;

        if previous_is_bullish
            && d_is_small
            && d_rejects_higher
            && d_has_long_upper_shadow
            && d_has_small_lower_shadow
            && confirmation_is_bearish
            && confirmation_falls_from_d
            && confirmation_closes_into_prior_body
        {
            set_primary_reversal_type(&mut signals, ReversalType::ShootingStar);
            signals.shooting_star = true;
        }
    }

    if market == Market::Bearish {
        let d_stalls_higher = d_candle.close >= previous.close || d_candle.high > previous.high;
        let confirmation_is_bearish = is_bearish(confirmation);
        let confirmation_breaks_d =
            confirmation.close < d_candle.close && confirmation.low <= d_candle.low;
        let confirmation_closes_into_prior_body = confirmation.close <= previous_midpoint;

        if previous_is_bullish
            && d_is_small
            && d_stalls_higher
            && confirmation_is_bearish
            && confirmation_breaks_d
            && confirmation_closes_into_prior_body
        {
            set_primary_reversal_type(&mut signals, ReversalType::EveningStar);
            signals.evening_star = true;
        }
    }

    if let Some(second_follow_through) = observed_candles.get(context.d_index + 2) {
        if market == Market::Bullish {
            let first_is_bullish = is_bullish(d_candle);
            let second_is_bullish = is_bullish(confirmation);
            let third_is_bullish = is_bullish(second_follow_through);
            let first_strong = body_dominates_range(d_candle, 0.45);
            let second_strong = body_dominates_range(confirmation, 0.45);
            let third_strong = body_dominates_range(second_follow_through, 0.45);
            let upper_shadows_small = upper_shadow(d_candle) <= candle_body(d_candle) * 0.6
                && upper_shadow(confirmation) <= candle_body(confirmation) * 0.6
                && upper_shadow(second_follow_through) <= candle_body(second_follow_through) * 0.6;
            let opens_step_up = confirmation.open >= d_candle.open.min(d_candle.close)
                && confirmation.open <= d_candle.close
                && second_follow_through.open >= confirmation.open.min(confirmation.close)
                && second_follow_through.open <= confirmation.close;
            let closes_step_up = d_candle.close < confirmation.close
                && confirmation.close < second_follow_through.close;
            let first_soldier_reclaims = d_candle.close >= previous_midpoint;

            if previous_is_bearish
                && first_is_bullish
                && second_is_bullish
                && third_is_bullish
                && first_strong
                && second_strong
                && third_strong
                && upper_shadows_small
                && opens_step_up
                && closes_step_up
                && first_soldier_reclaims
            {
                set_primary_reversal_type(&mut signals, ReversalType::ThreeWhiteSoldiers);
                signals.three_white_soldiers = true;
            }
        }

        if market == Market::Bearish {
            let first_is_bearish = is_bearish(d_candle);
            let second_is_bearish = is_bearish(confirmation);
            let third_is_bearish = is_bearish(second_follow_through);
            let first_strong = body_dominates_range(d_candle, 0.45);
            let second_strong = body_dominates_range(confirmation, 0.45);
            let third_strong = body_dominates_range(second_follow_through, 0.45);
            let lower_shadows_small = lower_shadow(d_candle) <= candle_body(d_candle) * 0.6
                && lower_shadow(confirmation) <= candle_body(confirmation) * 0.6
                && lower_shadow(second_follow_through) <= candle_body(second_follow_through) * 0.6;
            let opens_step_down = confirmation.open <= d_candle.open.max(d_candle.close)
                && confirmation.open >= d_candle.close
                && second_follow_through.open <= confirmation.open.max(confirmation.close)
                && second_follow_through.open >= confirmation.close;
            let closes_step_down = d_candle.close > confirmation.close
                && confirmation.close > second_follow_through.close;
            let first_crow_reclaims = d_candle.close <= previous_midpoint;

            if is_bullish(previous)
                && first_is_bearish
                && second_is_bearish
                && third_is_bearish
                && first_strong
                && second_strong
                && third_strong
                && lower_shadows_small
                && opens_step_down
                && closes_step_down
                && first_crow_reclaims
            {
                set_primary_reversal_type(&mut signals, ReversalType::ThreeBlackCrows);
                signals.three_black_crows = true;
            }
        }
    }

    signals
}
