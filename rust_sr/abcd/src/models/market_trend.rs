use chrono::{Duration, NaiveDateTime, Timelike};

pub const MARKET_TREND_EMA_PERIOD: usize = 21;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketTrendTimeframe {
    FiveMinute,
    FifteenMinute,
    OneHour,
}

impl MarketTrendTimeframe {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FiveMinute => "5m",
            Self::FifteenMinute => "15m",
            Self::OneHour => "1h",
        }
    }

    pub fn source_table(self) -> &'static str {
        match self {
            Self::FiveMinute => "futures_contract_5m_candles",
            Self::FifteenMinute => "futures_contract_15m_candles",
            Self::OneHour => "futures_contract_1h_candles",
        }
    }

    pub fn candle_minutes(self) -> i64 {
        match self {
            Self::FiveMinute => 5,
            Self::FifteenMinute => 15,
            Self::OneHour => 60,
        }
    }

    pub fn core_set() -> [Self; 3] {
        [Self::FiveMinute, Self::FifteenMinute, Self::OneHour]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketTrendLabel {
    Bullish,
    Bearish,
    Neutral,
}

impl MarketTrendLabel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bullish => "bullish",
            Self::Bearish => "bearish",
            Self::Neutral => "neutral",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MarketTrendInputCandle {
    pub ts_utc: NaiveDateTime,
    pub close: f64,
}

#[derive(Debug, Clone)]
pub struct MarketTrendPoint {
    pub ts_utc: NaiveDateTime,
    pub ema_21: Option<f64>,
    pub ema_21_slope: Option<f64>,
    pub close_to_ema_pct: Option<f64>,
    pub ema_slope_pct: Option<f64>,
    pub strength_pct: Option<f64>,
    pub label: Option<MarketTrendLabel>,
}

pub fn classify_ema21_trend(close: f64, ema_21: f64, ema_21_slope: f64) -> MarketTrendLabel {
    if close > ema_21 && ema_21_slope > 0.0 {
        MarketTrendLabel::Bullish
    } else if close < ema_21 && ema_21_slope < 0.0 {
        MarketTrendLabel::Bearish
    } else {
        MarketTrendLabel::Neutral
    }
}

pub fn calculate_ema21_trends(candles: &[MarketTrendInputCandle]) -> Vec<MarketTrendPoint> {
    calculate_ema_trends(candles, MARKET_TREND_EMA_PERIOD)
}

pub fn last_closed_candle_start_at_or_before(
    entry_at: NaiveDateTime,
    timeframe: MarketTrendTimeframe,
) -> NaiveDateTime {
    let close_cutoff = entry_at - Duration::minutes(timeframe.candle_minutes());
    floor_to_timeframe_start(close_cutoff, timeframe.candle_minutes())
}

fn floor_to_timeframe_start(value: NaiveDateTime, timeframe_minutes: i64) -> NaiveDateTime {
    let bucket_seconds = timeframe_minutes.max(1) * 60;
    let seconds_from_midnight = value.num_seconds_from_midnight() as i64;
    let floored_seconds = seconds_from_midnight / bucket_seconds * bucket_seconds;
    let seconds_to_subtract = seconds_from_midnight - floored_seconds;

    value
        - Duration::seconds(seconds_to_subtract)
        - Duration::nanoseconds(value.nanosecond() as i64)
}

fn calculate_ema_trends(
    candles: &[MarketTrendInputCandle],
    period: usize,
) -> Vec<MarketTrendPoint> {
    if period == 0 {
        return Vec::new();
    }

    let mut points = Vec::with_capacity(candles.len());
    let multiplier = 2.0 / (period as f64 + 1.0);
    let mut close_sum = 0.0;
    let mut previous_ema: Option<f64> = None;

    for (index, candle) in candles.iter().enumerate() {
        close_sum += candle.close;

        let mut ema_21 = None;
        let mut ema_21_slope = None;
        let mut close_to_ema_pct = None;
        let mut ema_slope_pct = None;
        let mut strength_pct = None;
        let mut label = None;

        if index + 1 == period {
            let seeded_ema = close_sum / period as f64;
            previous_ema = Some(seeded_ema);
            ema_21 = Some(seeded_ema);
        } else if index + 1 > period {
            if let Some(prior_ema) = previous_ema {
                let current_ema = (candle.close - prior_ema) * multiplier + prior_ema;
                let slope = current_ema - prior_ema;

                ema_21 = Some(current_ema);
                ema_21_slope = Some(slope);

                if current_ema != 0.0 {
                    let distance_pct = ((candle.close - current_ema) / current_ema) * 100.0;
                    close_to_ema_pct = Some(distance_pct);
                }

                if prior_ema != 0.0 {
                    let slope_pct = (slope / prior_ema) * 100.0;
                    ema_slope_pct = Some(slope_pct);
                }

                if let (Some(distance_pct), Some(slope_pct)) = (close_to_ema_pct, ema_slope_pct) {
                    strength_pct = Some(distance_pct.abs() + slope_pct.abs());
                }

                label = Some(classify_ema21_trend(candle.close, current_ema, slope));
                previous_ema = Some(current_ema);
            }
        }

        points.push(MarketTrendPoint {
            ts_utc: candle.ts_utc,
            ema_21,
            ema_21_slope,
            close_to_ema_pct,
            ema_slope_pct,
            strength_pct,
            label,
        });
    }

    points
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, NaiveDate};

    fn candle_at(index: i64, close: f64) -> MarketTrendInputCandle {
        MarketTrendInputCandle {
            ts_utc: NaiveDate::from_ymd_opt(2026, 1, 1)
                .unwrap()
                .and_hms_opt(9, 30, 0)
                .unwrap()
                + Duration::minutes(index * 5),
            close,
        }
    }

    #[test]
    fn waits_for_ema_seed_and_prior_ema_before_labeling() {
        let candles: Vec<_> = (0..MARKET_TREND_EMA_PERIOD)
            .map(|index| candle_at(index as i64, 100.0))
            .collect();

        let points = calculate_ema21_trends(&candles);

        assert!(points[..MARKET_TREND_EMA_PERIOD - 1]
            .iter()
            .all(|point| point.ema_21.is_none() && point.label.is_none()));
        assert!(points[MARKET_TREND_EMA_PERIOD - 1].ema_21.is_some());
        assert!(points[MARKET_TREND_EMA_PERIOD - 1].label.is_none());
    }

    #[test]
    fn rising_close_above_rising_ema_is_bullish() {
        let candles: Vec<_> = (0..30)
            .map(|index| candle_at(index, 100.0 + index as f64))
            .collect();

        let last = calculate_ema21_trends(&candles).pop().unwrap();

        assert_eq!(last.label, Some(MarketTrendLabel::Bullish));
        assert!(last.close_to_ema_pct.unwrap() > 0.0);
        assert!(last.ema_slope_pct.unwrap() > 0.0);
        assert!(last.strength_pct.unwrap() > 0.0);
    }

    #[test]
    fn falling_close_below_falling_ema_is_bearish() {
        let candles: Vec<_> = (0..30)
            .map(|index| candle_at(index, 130.0 - index as f64))
            .collect();

        let last = calculate_ema21_trends(&candles).pop().unwrap();

        assert_eq!(last.label, Some(MarketTrendLabel::Bearish));
        assert!(last.close_to_ema_pct.unwrap() < 0.0);
        assert!(last.ema_slope_pct.unwrap() < 0.0);
    }

    #[test]
    fn mixed_close_and_slope_is_neutral() {
        assert_eq!(
            classify_ema21_trend(101.0, 100.0, -0.1),
            MarketTrendLabel::Neutral
        );
        assert_eq!(
            classify_ema21_trend(99.0, 100.0, 0.1),
            MarketTrendLabel::Neutral
        );
    }

    #[test]
    fn finds_last_fully_closed_candle_start_without_lookahead() {
        let entry_at = NaiveDate::from_ymd_opt(2026, 1, 1)
            .unwrap()
            .and_hms_opt(10, 8, 0)
            .unwrap();

        let last_closed =
            last_closed_candle_start_at_or_before(entry_at, MarketTrendTimeframe::FiveMinute);

        assert_eq!(
            last_closed,
            NaiveDate::from_ymd_opt(2026, 1, 1)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap()
        );
    }

    #[test]
    fn exact_candle_close_can_use_that_closed_candle() {
        let entry_at = NaiveDate::from_ymd_opt(2026, 1, 1)
            .unwrap()
            .and_hms_opt(10, 5, 0)
            .unwrap();

        let last_closed =
            last_closed_candle_start_at_or_before(entry_at, MarketTrendTimeframe::FiveMinute);

        assert_eq!(
            last_closed,
            NaiveDate::from_ymd_opt(2026, 1, 1)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap()
        );
    }

    #[test]
    fn one_hour_candle_uses_prior_closed_hour() {
        let entry_at = NaiveDate::from_ymd_opt(2026, 1, 1)
            .unwrap()
            .and_hms_opt(10, 30, 0)
            .unwrap();

        let last_closed =
            last_closed_candle_start_at_or_before(entry_at, MarketTrendTimeframe::OneHour);

        assert_eq!(
            last_closed,
            NaiveDate::from_ymd_opt(2026, 1, 1)
                .unwrap()
                .and_hms_opt(9, 0, 0)
                .unwrap()
        );
    }
}
