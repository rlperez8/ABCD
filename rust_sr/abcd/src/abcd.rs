use tools::db_utils::{Database, Candle};
use serde::Serialize;
use csv::WriterBuilder;
use std::time::Instant;
use std::fs::File;
use chrono::Datelike;

pub fn truncate_to_2_decimals(value: f64) -> f64 {
    (value * 100.0).trunc() / 100.0
}

// === Enum ===

pub enum PivotType { High, Low }

// === Struct ===

#[derive(Debug, Clone, Serialize)]
pub struct Pivot {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub length: i64,
    pub min_max: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct Trade {
    pub open: bool,
    pub risk_exit_price: f64,
    pub reward_exit_price: f64,
    pub enter_price: f64,
    pub current_price: f64,
    pub length: i64,
    pub pnl: f64,
    pub result: Option<bool>,
    pub date: String,
    pub symbol: String,
    pub bc_price_retracement: f64,
    pub cd_price_retracement: f64,
    pub snr: f64,
    pub year: i64,
    pub month: i64,
    pub day: i64,

}
#[derive(Debug, Clone)]
pub struct PatternA { pub a: Pivot }

#[derive(Debug, Clone)]
pub struct PatternAB { pub a: Pivot, pub b: Pivot }

#[derive(Debug, Clone)]
pub struct PatternABC { pub a: Pivot, pub b: Pivot, pub c: Pivot }

#[derive(Debug, Clone, Serialize)]
pub struct PatternABCD {
    pub symbol: String,
    pub a: Pivot,
    pub b: Pivot,
    pub c: Pivot,
    pub d: Pivot,
    pub trade: Trade,
}
#[derive(Serialize)]
pub struct PatternABCDCsv {
    symbol: String,
    a_date: String,
    a_open: f64,
    a_high: f64,
    a_low: f64,
    a_close: f64,
    a_length: i64,
    a_min_max: f64,

    b_date: String,
    b_open: f64,
    b_high: f64,
    b_low: f64,
    b_close: f64,
    b_length: i64,
    b_min_max: f64,

    c_date: String,
    c_open: f64,
    c_high: f64,
    c_low: f64,
    c_close: f64,
    c_length: i64,
    c_min_max: f64,

    d_date: String,
    d_open: f64,
    d_high: f64,
    d_low: f64,
    d_close: f64,
    d_length: i64,
    d_min_max: f64,

    trade_open: bool,
    trade_risk_exit_price: f64,
    trade_reward_exit_price: f64,
    trade_enter_price: f64,
    trade_current_price: f64,
    trade_length: i64,
    trade_pnl: f64,
    trade_result: Option<bool>,
    trade_date: String,
    trade_symbol: String,
    trade_bc_price_retracement: f64,
    trade_cd_price_retracement: f64,
    trade_snr: f64,
    trade_year: i64,
    trade_month: i64,
    trade_day: i64,

}
pub struct ABCD;

#[derive(Debug, Clone)]
pub struct SRLine {
    pub price: f64,
    pub score: f64,
    pub lower: f64,
    pub upper: f64,
}
// === Impl === 

impl Pivot {
    pub fn new(candle: &Candle, length: i64, min_max: f64) -> Self {
        Self {
            date: candle.date.to_string(),
            open: truncate_to_2_decimals(candle.open),
            high: truncate_to_2_decimals(candle.high),
            low: truncate_to_2_decimals(candle.low),
            close: truncate_to_2_decimals(candle.close),
            length,
            min_max: truncate_to_2_decimals(min_max),
        }
    }
}
impl ABCD {

    fn check_for_pivot(current: &Candle, prev1: &Candle, prev2: &Candle, pivot_type: PivotType) -> bool {
        match pivot_type {
            PivotType::High => prev1.high > current.high && prev1.high > prev2.high,
            PivotType::Low => prev1.low < current.low && prev1.low < prev2.low,
        }
    }
    pub fn write_patterns_to_csv(patterns: &[PatternABCD], filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Map nested structs into flat CSV structs
        let csv_patterns: Vec<PatternABCDCsv> = patterns.iter().map(|p| PatternABCDCsv {
            symbol: p.symbol.clone(),
            a_date: p.a.date.clone(),
            a_open: p.a.open,
            a_high: p.a.high,
            a_low: p.a.low,
            a_close: p.a.close,
            a_length: p.a.length,
            a_min_max: p.a.min_max,
            b_date: p.b.date.clone(),
            b_open: p.b.open,
            b_high: p.b.high,
            b_low: p.b.low,
            b_close: p.b.close,
            b_length: p.b.length,
            b_min_max: p.b.min_max,
            c_date: p.c.date.clone(),
            c_open: p.c.open,
            c_high: p.c.high,
            c_low: p.c.low,
            c_close: p.c.close,
            c_length: p.c.length,
            c_min_max: p.c.min_max,
            d_date: p.d.date.clone(),
            d_open: p.d.open,
            d_high: p.d.high,
            d_low: p.d.low,
            d_close: p.d.close,
            d_length: p.d.length,
            d_min_max: p.d.min_max,
            trade_open: p.trade.open,
            trade_risk_exit_price: p.trade.risk_exit_price,
            trade_reward_exit_price: p.trade.reward_exit_price,
            trade_enter_price: p.trade.enter_price,
            trade_current_price: p.trade.current_price,
            trade_length: p.trade.length,
            trade_pnl: p.trade.pnl,
            trade_result: p.trade.result,
            trade_date: p.trade.date.clone(),
            trade_symbol: p.trade.symbol.clone(),
            trade_bc_price_retracement:  p.trade.bc_price_retracement,
            trade_cd_price_retracement: p.trade.cd_price_retracement,
            trade_snr: p.trade.snr,
            trade_year: p.trade.year,
            trade_month: p.trade.month,
            trade_day: p.trade.day,
        }).collect();

        let file = File::create(filename)?;
        let mut writer = WriterBuilder::new().has_headers(true).from_writer(file);

        for p in csv_patterns {
            writer.serialize(p)?;
        }

        writer.flush()?;
        Ok(())
    }
    pub fn detect_patterns(db: &mut Database) -> mysql::Result<Vec<PatternABCD>> {
        let mut all_patterns: Vec<PatternABCD> = Vec::new();
        
        // Symbols
        let symbols = db.get_distinct_symbols()?;

        for symbol in symbols {

            // Candles
            let mut candles = db.get_stored_candles(&symbol)?;
            candles.reverse();

            // S&R
            let support_and_resitance = ABCD::create_support_resistance(&candles);

            // Patterns
            let mut pattern_a: Vec<PatternA> = Vec::new();
            let mut pattern_ab: Vec<PatternAB> = Vec::new();
            let mut pattern_abc: Vec<PatternABC> = Vec::new();
            let mut pattern_abcd: Vec<PatternABCD> = Vec::new();

            for window in candles.windows(3) {
                let prev2 = &window[0];
                let prev1 = &window[1];
                let current = &window[2];

                // === Detect Pivot A ===
                if Self::check_for_pivot(current, prev1, prev2, PivotType::High) {
                    let a = Pivot::new(prev1, 0, prev1.low);
                    pattern_a.push(PatternA { a });
                }

                // === Detect Pivot B ===
                for pattern in &mut pattern_a {
                    pattern.a.length += 1;
                    pattern.a.min_max = pattern.a.min_max.min(prev1.low);

                    if prev1.low < pattern.a.low && prev1.low <= pattern.a.min_max {
                        if Self::check_for_pivot(current, prev1, prev2, PivotType::Low) {
                            let b = Pivot::new(prev1, 0, prev1.high);
                            pattern_ab.push(PatternAB { a: pattern.a.clone(), b });
                        }
                    }
                }

                // === Detect Pivot C ===
                for pattern in &mut pattern_ab {
                    pattern.b.length += 1;
                    pattern.b.min_max = pattern.b.min_max.max(prev1.high);

                    if prev1.high >= pattern.b.min_max {
                        if prev1.high > pattern.b.low && prev1.high < pattern.a.high {
                            if Self::check_for_pivot(current, prev1, prev2, PivotType::High) {
                                let c = Pivot::new(prev1, 0, prev1.low);
                                pattern_abc.push(PatternABC { a: pattern.a.clone(), b: pattern.b.clone(), c });
                            }
                        }
                    }
                }

                // === Detect Pivot D & trade ===
                for pattern in &mut pattern_abc {
                    pattern.c.length += 1;
                    pattern.c.min_max = pattern.c.min_max.min(prev1.low);

                    if prev1.low <= pattern.c.min_max && prev1.low < pattern.b.low {
                        if Self::check_for_pivot(current, prev1, prev2, PivotType::Low) {


                            let c_ = prev1.close == support_and_resitance[0].price;
                            let l_ = prev1.low == support_and_resitance[0].price;
                            let o_ = prev1.open == support_and_resitance[0].price;

                            let d_on_sr = c_ || l_|| o_;
                            if d_on_sr {

                                let d = Pivot::new(prev1, 0, prev1.high);

                                let take_profit_price = pattern.c.high;
                                let reward_pnl = pattern.c.high - prev1.close;
                                let stop_lost_price = pattern.b.low - reward_pnl;

                                let ab_price_length = pattern.a.high - pattern.b.low;
                                let bc_price_length = pattern.c.high - pattern.b.low;
                                let cd_price_length = pattern.c.high - prev1.low;

                                let bc_price_retracement = (bc_price_length.abs() / ab_price_length) * 100.0;
                                let cd_price_retracement = (cd_price_length.abs() / ab_price_length) * 100.0;
                                
                                let trade = Trade {
                                    open: true,
                                    risk_exit_price: truncate_to_2_decimals(stop_lost_price),
                                    reward_exit_price: truncate_to_2_decimals(take_profit_price),
                                    enter_price: truncate_to_2_decimals(prev1.close),
                                    current_price: truncate_to_2_decimals(prev1.close),
                                    length: 0,
                                    pnl: 0.0,
                                    result: None,
                                    date: prev1.date.to_string(), // optional: keep string if you want
                                    symbol: symbol.clone(),
                                    bc_price_retracement,
                                    cd_price_retracement,
                                    snr: support_and_resitance[0].price,
                                    year: prev1.date.year() as i64,
                                    month: prev1.date.month() as i64,
                                    day: prev1.date.day() as i64,
                                };

                                pattern_abcd.push(PatternABCD {
                                    symbol: symbol.clone(),
                                    a: pattern.a.clone(),
                                    b: pattern.b.clone(),
                                    c: pattern.c.clone(),
                                    d,
                                    trade,
                                });
                                                        
                            }
                        }
                    }
                }

                // === Exit logic for open trades ===
                for pattern in &mut pattern_abcd {
                    if pattern.trade.open {
                        pattern.d.length += 1;
                        let unrealized_pnl = current.close - pattern.trade.enter_price;
                        pattern.trade.pnl = truncate_to_2_decimals(unrealized_pnl);
                        pattern.trade.date = current.date.to_string();
                        pattern.trade.current_price = truncate_to_2_decimals(current.close);

                        // Check reward exit
                        if current.high > pattern.trade.reward_exit_price
                            || current.close > pattern.trade.reward_exit_price
                            || current.open > pattern.trade.reward_exit_price
                            || current.low > pattern.trade.reward_exit_price
                        {
                            pattern.trade.open = false;
                            pattern.trade.current_price = truncate_to_2_decimals(pattern.trade.reward_exit_price);
                            pattern.trade.result = Some(true);
                            pattern.trade.length = pattern.a.length + pattern.b.length + pattern.c.length + pattern.d.length;
                
                        }

                        // Check risk exit
                        if current.high < pattern.trade.risk_exit_price
                            || current.close < pattern.trade.risk_exit_price
                            || current.open < pattern.trade.risk_exit_price
                            || current.low < pattern.trade.risk_exit_price
                        {
                            pattern.trade.open = false;
                            pattern.trade.current_price = truncate_to_2_decimals(pattern.trade.risk_exit_price);
                            pattern.trade.result = Some(false);
                            pattern.trade.length = pattern.a.length + pattern.b.length + pattern.c.length + pattern.d.length;
                      
                        }
                    }
                }

                pattern_a.retain(|p| current.high <= p.a.high);
                pattern_ab.retain(|p| current.low >= p.b.low && current.high <= p.a.high);
                pattern_abc.retain(|p| current.high <= p.c.high);
            }

            println!("Symbol: {} Patterns: {} S&R: {}", symbol, pattern_abcd.len(), support_and_resitance[0].price);
            all_patterns.extend(pattern_abcd);
        }

        Ok(all_patterns)
    }
    pub fn create_support_resistance(candles: &[Candle]) -> Vec<SRLine> {
        let decay_per_tick = 0.01;       // points lost per tick away
        let range_pct = 0.05;            // ±5% range for top SR selection
        let reaction_tolerance = 0.01;   // how close candle must touch/reverse

        // --- Adaptive tick levels ---
        let min_price = candles.iter()
            .flat_map(|c| vec![c.open, c.high, c.low, c.close])
            .fold(f64::INFINITY, |a, b| a.min(b));
        let max_price = candles.iter()
            .flat_map(|c| vec![c.open, c.high, c.low, c.close])
            .fold(f64::NEG_INFINITY, |a, b| a.max(b));
        let price_range = max_price - min_price;

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
                    if candle.close > candle.low {
                        let tick_dist = (tick - candle.low).abs() / tick_interval;
                        scores[i] += (1.0 - tick_dist * decay_per_tick).max(0.0);
                    }
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

            sr_lines.push(SRLine {
                price:  truncate_to_2_decimals(price),
                score,
                lower:  truncate_to_2_decimals(price * (1.0 - range_pct)),
                upper: truncate_to_2_decimals(price * (1.0 + range_pct)), 
            });

            let lower = price * (1.0 - range_pct);
            let upper = price * (1.0 + range_pct);
            remaining.retain(|&(p, _)| p < lower || p > upper);
        }

        sr_lines
    }
}

