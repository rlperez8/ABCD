use crate::models::candle::Candle;

#[derive(Debug)]
pub struct SrLine {
    pub price: f64,
    pub score: f64,
}

impl SrLine {
    pub fn new(price: f64, score: f64) -> Self {
        SrLine { price, score }
    }

    pub fn create_support_resistance(&self, candles: &[Candle]) -> Vec<SrLine> {
        if candles.is_empty() {
            return vec![];
        }

        let decay_per_tick = 0.01;
        let range_pct = 0.05;
        let reaction_tolerance = 0.01;

        // === FIND RANGE ===
        let min_price = candles
            .iter()
            .flat_map(|c| [c.open, c.high, c.low, c.close])
            .fold(f64::INFINITY, |a, b| a.min(b));

        let max_price = candles
            .iter()
            .flat_map(|c| [c.open, c.high, c.low, c.close])
            .fold(f64::NEG_INFINITY, |a, b| a.max(b));

        let price_range = max_price - min_price;
        if !price_range.is_finite() || price_range <= 0.0 {
            return vec![SrLine {
                price: self.truncate_to_2_decimals(min_price),
                score: candles.len() as f64,
            }];
        }

        // === INITIALIZE TICKS ===
        let tick_interval = (price_range * 0.001).max(0.01);
        let tick_count = ((price_range / tick_interval).ceil() as usize).saturating_add(1);
        let mut ticks = Vec::with_capacity(tick_count);
        let mut current = min_price;
        while current <= max_price {
            ticks.push(current);
            current += tick_interval;
        }

        // --- Initialize scores ---
        let mut scores = vec![0.0; ticks.len()];

        // --- Reaction-only scoring ---
        // Update only the ticks near each candle low instead of scanning
        // every tick against every candle.
        for candle in candles {
            let lower_tick =
                ((candle.low - reaction_tolerance - min_price) / tick_interval).floor() as isize;
            let upper_tick =
                ((candle.low + reaction_tolerance - min_price) / tick_interval).ceil() as isize;

            let lower_tick = lower_tick.max(0) as usize;
            let upper_tick = upper_tick.min((ticks.len() as isize) - 1).max(0) as usize;

            for i in lower_tick..=upper_tick {
                let tick = ticks[i];
                let tick_dist = (tick - candle.low).abs() / tick_interval;
                scores[i] += (1.0 - tick_dist * decay_per_tick).max(0.0);
            }
        }

        // --- Pick top SR lines with ±range_pct removal ---
        let mut sr_lines = vec![];
        let mut remaining: Vec<(f64, f64)> =
            ticks.iter().copied().zip(scores.iter().copied()).collect();

        for _ in 0..1 {
            if remaining.is_empty() {
                break;
            }

            let (_, &(price, score)) = remaining
                .iter()
                .enumerate()
                .max_by(|a, b| a.1 .1.partial_cmp(&b.1 .1).unwrap())
                .unwrap();

            sr_lines.push(SrLine {
                price: self.truncate_to_2_decimals(price),
                score,
            });

            let lower = price * (1.0 - range_pct);
            let upper = price * (1.0 + range_pct);
            remaining.retain(|&(p, _)| p < lower || p > upper);
        }

        sr_lines
    }

    pub fn truncate_to_2_decimals(&self, value: f64) -> f64 {
        (value * 100.0).trunc() / 100.0
    }
}
