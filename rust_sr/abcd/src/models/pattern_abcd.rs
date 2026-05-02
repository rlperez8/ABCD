use crate::models::accuracy::{Accuracies, TimeAccuracies};
use crate::models::candle::Candle;
use crate::models::market::Market;
use crate::models::pivot::Pivot;
use crate::models::reversal_candle::ReversalPatternContext;
use crate::models::trade::Trade;
use chrono::NaiveDateTime;
use serde::Serialize;
use std::fmt::Write as _;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct TargetCandle {
    pub date: NaiveDateTime,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
    pub is_green: bool,
    pub close_vs_open_pct: f64,
    pub high_vs_open_pct: f64,
    pub low_vs_open_pct: f64,
    pub range_pct: f64,
    pub breaks_d_high: bool,
    pub breaks_d_low: bool,
}

impl TargetCandle {
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

    pub fn from_candle(candle: &Candle, entry: &Candle) -> Self {
        Self {
            date: candle.date,
            open: candle.open,
            high: candle.high,
            low: candle.low,
            close: candle.close,
            volume: candle.volume,
            is_green: candle.close > candle.open,
            close_vs_open_pct: Self::pct_change(candle.open, candle.close),
            high_vs_open_pct: Self::pct_change(candle.open, candle.high),
            low_vs_open_pct: Self::pct_change(candle.open, candle.low),
            range_pct: Self::range_pct(candle.open, candle.high, candle.low),
            breaks_d_high: candle.high > entry.high,
            breaks_d_low: candle.low < entry.low,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PatternXABCD {
    pub symbol: Arc<str>,
    pub pattern_id: String,
    pub x_bars_left: i64,
    #[serde(skip_serializing)]
    pub x_index: usize,
    pub x: Pivot,
    pub a: Pivot,
    pub b: Pivot,
    pub c: Pivot,
    pub d: Pivot,
    pub market: Market,
    pub trade: Trade,
    #[serde(skip_serializing)]
    pub entry_index: Option<usize>,
    #[serde(skip_serializing)]
    pub reversal_context: ReversalPatternContext,
    pub d_confirm_date: NaiveDateTime,
    pub target_candle: Option<TargetCandle>,
    pub contract_week_index: Option<i64>,
    pub contract_days_from_start: Option<i64>,
    pub three_month: Option<bool>,
    pub six_month: Option<bool>,
    pub twelve_month: Option<bool>,
    pub accuracies: Accuracies,
    pub time_accuracies: TimeAccuracies,
    pub prop_strategy_id: String,
}

#[derive(Debug, Clone, Copy)]
pub struct DominantHarmonicLens {
    pub harmonic_type: &'static str,
    pub bin: &'static str,
    pub time_bin: &'static str,
}

impl PatternXABCD {
    pub fn calculate_x_bars_left(&self) -> i64 {
        self.x_bars_left
    }

    pub fn x_strictness(&self) -> &'static str {
        let xa_length = self.x.length.max(0);
        let normal_threshold = ((xa_length as f64) / 2.0).ceil() as i64;

        if self.x_bars_left >= xa_length {
            "Strict"
        } else if self.x_bars_left >= normal_threshold {
            "Normal"
        } else {
            "Loose"
        }
    }

    fn format_pattern_price(value: f64) -> String {
        format!("{value:.2}")
    }

    fn bucket_label(value: f64) -> &'static str {
        if value <= 10.0 {
            "0-10"
        } else if value <= 20.0 {
            "10-20"
        } else if value <= 30.0 {
            "20-30"
        } else if value <= 40.0 {
            "30-40"
        } else if value <= 50.0 {
            "40-50"
        } else if value <= 60.0 {
            "50-60"
        } else if value <= 70.0 {
            "60-70"
        } else if value <= 80.0 {
            "70-80"
        } else if value <= 90.0 {
            "80-90"
        } else {
            "90-100"
        }
    }

    fn trend_label(value: Option<bool>) -> &'static str {
        match value {
            Some(true) => "Bullish",
            Some(false) => "Bearish",
            None => "Unknown",
        }
    }

    fn size_bucket(&self) -> &'static str {
        // Match the cache/server strategy grouping logic exactly: X + A + B + C only.
        let total_bars = self.x.length + self.a.length + self.b.length + self.c.length;

        if total_bars <= 20 {
            "Micro"
        } else if total_bars <= 60 {
            "Small"
        } else if total_bars <= 180 {
            "Normal"
        } else if total_bars <= 365 {
            "Large"
        } else {
            "Massive"
        }
    }

    fn rounded_metric(value: f64) -> f64 {
        (value * 100.0).round() / 100.0
    }

    fn canonical_pattern_key(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            format!("{:?}", self.market).to_lowercase(),
            self.symbol,
            self.x.date,
            Self::format_pattern_price(self.x.min_max),
            self.a.date,
            Self::format_pattern_price(self.a.min_max),
            self.b.date,
            Self::format_pattern_price(self.b.min_max),
            self.c.date,
            Self::format_pattern_price(self.c.min_max),
            self.d.date,
            Self::format_pattern_price(self.d.min_max),
        )
    }

    pub fn refresh_pattern_id(&mut self) {
        let canonical_key = self.canonical_pattern_key();
        let hash = md5::compute(canonical_key.as_bytes());
        self.pattern_id = format!("{hash:x}")[..24].to_string();
    }

    pub fn dominant_harmonic_lens(&self) -> DominantHarmonicLens {
        let candidates = [
            (
                "Bat",
                self.accuracies.bat.pattern_accuracy,
                self.time_accuracies.bat.time_accuracy,
            ),
            (
                "AlternateBat",
                self.accuracies.alternate_bat.pattern_accuracy,
                self.time_accuracies.alternate_bat.time_accuracy,
            ),
            (
                "Butterfly",
                self.accuracies.butterfly.pattern_accuracy,
                self.time_accuracies.butterfly.time_accuracy,
            ),
            (
                "Gartley",
                self.accuracies.gartley.pattern_accuracy,
                self.time_accuracies.gartley.time_accuracy,
            ),
            (
                "Crab",
                self.accuracies.crab.pattern_accuracy,
                self.time_accuracies.crab.time_accuracy,
            ),
            (
                "DeepCrab",
                self.accuracies.deep_crab.pattern_accuracy,
                self.time_accuracies.deep_crab.time_accuracy,
            ),
            (
                "Shark",
                self.accuracies.shark.pattern_accuracy,
                self.time_accuracies.shark.time_accuracy,
            ),
        ];

        let mut best = candidates[0];
        for candidate in candidates.iter().copied().skip(1) {
            let price_is_better = candidate.1 > best.1;
            let price_tie_with_better_time =
                (candidate.1 - best.1).abs() <= f64::EPSILON && candidate.2 > best.2;

            if price_is_better || price_tie_with_better_time {
                best = candidate;
            }
        }

        let (harmonic_type, accuracy, time_accuracy) = best;

        DominantHarmonicLens {
            harmonic_type,
            bin: Self::bucket_label(Self::rounded_metric(accuracy)),
            time_bin: Self::bucket_label(Self::rounded_metric(time_accuracy)),
        }
    }

    fn canonical_prop_strategy_key(&self) -> String {
        let lens = self.dominant_harmonic_lens();

        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}",
            format!("{:?}", self.market).to_lowercase(),
            lens.harmonic_type.to_lowercase(),
            lens.bin.to_lowercase(),
            format!("{:?}", self.trade.reversal_type).to_lowercase(),
            self.size_bucket().to_lowercase(),
            lens.time_bin.to_lowercase(),
            Self::trend_label(self.three_month).to_lowercase(),
            Self::trend_label(self.six_month).to_lowercase(),
            Self::trend_label(self.twelve_month).to_lowercase(),
        )
    }

    pub fn refresh_prop_strategy_id(&mut self) {
        let canonical_key = self.canonical_prop_strategy_key();
        let hash = blake3::hash(canonical_key.as_bytes());
        let mut strategy_id = String::with_capacity(16);

        for byte in &hash.as_bytes()[..8] {
            let _ = write!(&mut strategy_id, "{:02x}", byte);
        }

        self.prop_strategy_id = strategy_id;
    }
}
