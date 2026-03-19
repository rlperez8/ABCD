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
                            

        // === INITIALIZE TICKS ===
        let tick_interval = (price_range * 0.001).max(0.01);
        let mut ticks = vec![];
        let mut current = min_price;
        while current <= max_price {
            ticks.push(current);
            current += tick_interval;
        }

        // --- Initialize scores ---
        let mut scores = vec![0.0; ticks.len()];

        // --- Reaction-only scoring ---
        for (i, &tick) in ticks.iter().enumerate() {
            for candle in candles {
                if candle.low >= tick - reaction_tolerance && candle.low <= tick + reaction_tolerance {
                    let tick_dist = (tick - candle.low).abs() / tick_interval;
                    scores[i] += (1.0 - tick_dist * decay_per_tick).max(0.0);
                }
            }
        }

        // --- Pick top SR lines with ±range_pct removal ---
        let mut sr_lines = vec![];
        let mut remaining: Vec<(f64, f64)> = ticks.iter().copied().zip(scores.iter().copied()).collect();

        for _ in 0..1 {
            if remaining.is_empty() { break; }

            let (top_idx, &(price, score)) = remaining.iter().enumerate()
                .max_by(|a, b| a.1.1.partial_cmp(&b.1.1).unwrap())
                .unwrap();

            sr_lines.push(SrLine {
                price:  self.truncate_to_2_decimals(price),
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