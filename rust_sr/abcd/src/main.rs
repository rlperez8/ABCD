use std::time::Instant;

use chrono::NaiveDate;

use chrono::Datelike;

mod models; 
use crate::models::database::Database;

use crate::models::*;


#[derive(Clone, Debug)]
struct SRLine {
    price: f64,
    score: f64,
}

pub fn create_support_resistance(candles: &[Candle]) -> Vec<SRLine> {
        let decay_per_tick = 0.01;       
        let range_pct = 0.05;            
        let reaction_tolerance = 0.01;   

        // === FIND RANGE ===
        let min_price = candles.iter()
            .flat_map(|c| vec![c.open, c.high, c.low, c.close])
            .fold(f64::INFINITY, |a, b| a.min(b));
        let max_price = candles.iter()
            .flat_map(|c| vec![c.open, c.high, c.low, c.close])
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

            sr_lines.push(SRLine {
                price:  truncate_to_2_decimals(price),
                score,
            });

            // let lower = price * (1.0 - range_pct);
            // let upper = price * (1.0 + range_pct);
            // remaining.retain(|&(p, _)| p < lower || p > upper);
        }

        sr_lines
    }



    fn is_morning_star(c1: &Candle, c2: &Candle, c3: &Candle) -> bool {
    let body1 = (c1.close - c1.open).abs();
    let body2 = (c2.close - c2.open).abs();
    let body3 = (c3.close - c3.open).abs();

    let range1 = c1.high - c1.low;
    let range2 = c2.high - c2.low;
    let range3 = c3.high - c3.low;

    let c1_bearish = c1.close < c1.open;
    let c1_strong = body1 >= 0.5 * range1;

    let c2_small_body = body2 <= 0.3 * range2;

    let c3_bullish = c3.close > c3.open;
    let c3_strong = body3 >= 0.5 * range3;

    let midpoint_c1 = (c1.open + c1.close) / 2.0;
    let c3_closes_into_c1 = c3.close >= midpoint_c1;

    c1_bearish && c1_strong && c2_small_body && c3_bullish && c3_strong && c3_closes_into_c1
}
fn is_evening_star(c1: &Candle, c2: &Candle, c3: &Candle) -> bool {
    let body1 = (c1.close - c1.open).abs();
    let body2 = (c2.close - c2.open).abs();
    let body3 = (c3.close - c3.open).abs();

    let range1 = c1.high - c1.low;
    let range2 = c2.high - c2.low;
    let range3 = c3.high - c3.low;

    let c1_bullish = c1.close > c1.open;
    let c1_strong = body1 >= 0.5 * range1;

    let c2_small_body = body2 <= 0.3 * range2;

    let c3_bearish = c3.close < c3.open;
    let c3_strong = body3 >= 0.5 * range3;

    let midpoint_c1 = (c1.open + c1.close) / 2.0;
    let c3_closes_into_c1 = c3.close <= midpoint_c1;

    c1_bullish && c1_strong && c2_small_body && c3_bearish && c3_strong && c3_closes_into_c1
}
pub fn truncate_to_2_decimals(value: f64) -> f64 {
    (value * 100.0).trunc() / 100.0
}


// === Pivot check function ===
fn check_for_pivot(current: &Candle, prev1: &Candle, prev2: &Candle, pivot_type: PivotType) -> bool {
    match pivot_type {
        PivotType::High => prev1.high > current.high && prev1.high > prev2.high,
        PivotType::Low => prev1.low < current.low && prev1.low < prev2.low,
    }
}
fn detect_pattern(c1: &Candle, c2: &Candle, c3: &Candle) -> ReversalType {
                            
    if is_morning_star(c1, c2, c3) {
        ReversalType::MorningStar
    } else if is_evening_star(c1, c2, c3) {
        ReversalType::EveningStar
    } else {
        ReversalType::None
    }
}
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();

    let mut all_patterns: Vec<PatternXABCD> = Vec::new();
    
    // === Connect ===  
    let mut db = Database::new()?;
    let symbols = db.get_distinct_symbols()?;

    // === Symbols ===
    for symbol in &symbols {
    // for symbol in symbols.iter().take(1) {
    // for symbol in ["ACB".to_string()].iter() {

        // === Candles ===
        let mut candles = db.get_stored_candles(&symbol)?;
     
        //  // === Calculate Heatmap / SR Zones ===
        let support_and_resistance = create_support_resistance(&candles);
        println!("{:?}", support_and_resistance);

        // === Pattern storage ===
        let mut pattern_x: Vec<PatternX> = Vec::new();
        let mut pattern_xa: Vec<PatternXA> = Vec::new();

        let mut pattern_xab: Vec<PatternXAB> = Vec::new();
        let mut pattern_xab_holder: Vec<PatternXABC> = Vec::new();

        let mut pattern_xabc: Vec<PatternXABC> = Vec::new();
        let mut pattern_xabc_holder: Vec<PatternXABC> = Vec::new();

        let mut pattern_xabcd: Vec<PatternXABCD> = Vec::new();

        // === Detect pivots ===
        for window in candles.windows(3) {
            let prev2 = &window[0];
            let prev1 = &window[1];
            let current = &window[2];

            // === X ===
            if check_for_pivot(current, prev1, prev2, PivotType::Low) {
                let x = Pivot::new(prev1, PivotType::Low, 0, prev1.low);
                pattern_x.push(PatternX { x });
            }
            else if check_for_pivot(current, prev1, prev2, PivotType::High) {

                let x = Pivot::new(
                    prev1, 
                    PivotType::High, 
                    0, 
                    prev1.low
                );

                pattern_x.push(PatternX { x });
            }

            // === XA === 
            for pattern in &mut pattern_x {
                pattern.x.length += 1;
                
                // === MIN MAX ===
                match pattern.x.type_ {
                    PivotType::Low => pattern.x.min_max = pattern.x.min_max.max(prev1.high),
                    PivotType::High => pattern.x.min_max = pattern.x.min_max.min(prev1.low),
                }

                //  === CONDITIONS === 
                let conditions = match pattern.x.type_ {
                    PivotType::Low => prev1.high > pattern.x.low && prev1.high >= pattern.x.min_max,
                    PivotType::High => prev1.low < pattern.x.high && prev1.low <= pattern.x.min_max,
                };

                // === PIVOT CHECK ===
                let pivot_check = match pattern.x.type_ {
                    PivotType::Low => check_for_pivot(current, prev1, prev2, PivotType::High),
                    PivotType::High => check_for_pivot(current, prev1, prev2, PivotType::Low),
                };

                // === PIVOT TYPE ====
                let new_a_type = match pattern.x.type_ {
                    PivotType::Low => PivotType::High,
                    PivotType::High => PivotType::Low,
                };

                if conditions && pivot_check {

                     let new_a = Pivot::new(prev1, new_a_type, 0, prev1.low);

                     pattern_xa.push(PatternXA { 
                        x: pattern.x.clone(), 
                        a: new_a, 
                        
                    });

                }

            }

            // === XAB === 
            for pattern in &mut pattern_xa {
                pattern.a.length += 1;

                // === MIN MAX ===
                match pattern.a.type_ {
                    PivotType::Low => pattern.a.min_max = pattern.a.min_max.max(prev1.high),
                    PivotType::High => pattern.a.min_max = pattern.a.min_max.min(prev1.low),
                }

                //  === CONDITIONS === 
                let conditions = match pattern.a.type_ {
                    PivotType::Low => prev1.high > pattern.a.high && prev1.high >= pattern.a.min_max,
                    PivotType::High => prev1.low < pattern.a.low && prev1.low <= pattern.a.min_max,
                };

                // === PIVOT CHECK ===
                let pivot_check = match pattern.a.type_ {
                    PivotType::Low => check_for_pivot(current, prev1, prev2, PivotType::High),
                    PivotType::High => check_for_pivot(current, prev1, prev2, PivotType::Low),
                };

                // === PIVOT TYPE ====
                let new_b_type = match pattern.a.type_ {
                    PivotType::Low => PivotType::High,
                    PivotType::High => PivotType::Low,
                };

                if conditions && pivot_check {

                     let new_b = Pivot::new(prev1, new_b_type, 0, prev1.low);

                     pattern_xab.push(PatternXAB { 
                        x: pattern.x.clone(), 
                        a: pattern.a.clone(), 
                        b: new_b
                    });

                }
            }
            
            // === XABC ===
            for pattern in &mut pattern_xab {
                pattern.b.length += 1;
                
                //  === MIN MAX ===
                match pattern.b.type_ {
                    PivotType::Low => pattern.b.min_max = pattern.b.min_max.max(prev1.high),
                    PivotType::High => pattern.b.min_max = pattern.b.min_max.min(prev1.low),
                }

                //  === CONDITIONS === 
                let conditions = match pattern.b.type_ {
                    PivotType::Low => prev1.high >= pattern.b.min_max && prev1.high > pattern.b.low && prev1.high < pattern.a.high,
                    PivotType::High => prev1.low <= pattern.b.min_max && prev1.low < pattern.b.high && prev1.low > pattern.a.low,
                };

                // === PIVOT CHECK ===
                let pivot_check = match pattern.b.type_ {
                    PivotType::Low => check_for_pivot(current, prev1, prev2, PivotType::High),
                    PivotType::High => check_for_pivot(current, prev1, prev2, PivotType::Low),
                };

                // === PIVOT TYPE ====
                let new_c_type = match pattern.b.type_ {
                    PivotType::Low => PivotType::High,
                    PivotType::High => PivotType::Low,
                };

                if conditions && pivot_check {
                 
                    let new_c = Pivot::new(prev1, new_c_type, 0, prev1.low);

                    pattern_xabc_holder.push(PatternXABC { 
                        x: pattern.x.clone(),
                        a: pattern.a.clone(),
                        b: pattern.b.clone(),
                        c: new_c,
                    });
                }
            }

            // === XABCD ===
            for pattern in &mut pattern_xabc {

                pattern.c.length += 1;
                
                //  === MIN MAX ===
                match pattern.c.type_ {
                    PivotType::Low => pattern.c.min_max = pattern.c.min_max.max(prev1.high),
                    PivotType::High => pattern.c.min_max = pattern.c.min_max.min(prev1.low),
                }
                
                //  === CONDITIONS === 
                let conditions = match pattern.c.type_ {
                    PivotType::Low => prev1.high >= pattern.c.min_max && prev1.high > pattern.b.high && prev1.high < pattern.x.high,
                    PivotType::High => prev1.low <= pattern.c.min_max && prev1.low < pattern.b.low && prev1.low > pattern.x.low,
                };

                // === PIVOT CHECK ===
                let pivot_check = match pattern.c.type_ {
                    PivotType::Low => check_for_pivot(current, prev1, prev2, PivotType::High),
                    PivotType::High => check_for_pivot(current, prev1, prev2, PivotType::Low),
                };

                // === SUPPORT & RESISTANCE ===
                let level = support_and_resistance[0].price;
           
                let three_month = match pattern.c.type_ {
                    PivotType::Low => {
                        let touched = prev1.high == level;
                        touched
                    }
                    PivotType::High => {
                        let touched = prev1.low == level;
                        touched
                    }
                };

                if conditions && pivot_check {

                    // === PIVOT TYPE ====
                    let new_d_type = match pattern.c.type_ {
                        PivotType::Low => PivotType::High,
                        PivotType::High => PivotType::Low,
                    };

                    // === TRADE MARKET ===
                    let market = match pattern.c.type_ {
                        PivotType::Low => Market::Bearish,
                        PivotType::High => Market::Bullish
                    };
                    
                    let new_d = Pivot::new(prev1, new_d_type, 0, prev1.low);

                    let reversal = detect_pattern(prev2, prev1, current);  
                    
                    let trade = Trade::new(
                        symbol, 
                        market,
                        &pattern.x,
                        &pattern.a,
                        &pattern.b,
                        &pattern.c,
                        &prev1, 
                        support_and_resistance[0].price,
                        ReversalType::None,
                    );
          
                    pattern_xabcd.push(PatternXABCD {
                        symbol: symbol.clone(),
                        x: pattern.x.clone(),
                        a: pattern.a.clone(),
                        b: pattern.b.clone(),
                        c: pattern.c.clone(),
                        d: new_d, 
                        market,
                        abcd_type: ABCDType::Standard,
                        trade,
                        three_month: Some(three_month),
                        six_month: Some(false),
                        twelve_month: Some(false)
                    });
                
                        
                }
            }

            // === TRADE ===
            for pattern in &mut pattern_xabcd {

                if pattern.trade.open {

                    // increment D-leg length
                    pattern.d.length += 1;

                    // PnL based on market direction
                    let pnl = match pattern.market {
                        Market::Bullish => current.close - pattern.trade.enter_price,
                        Market::Bearish => pattern.trade.enter_price - current.close,
                    };
                    
                    pattern.trade.pnl = pnl;
                    pattern.trade.date = current.date.to_string();
                    pattern.trade.current_price = truncate_to_2_decimals(current.close);

                    match pattern.market {

                        // ======================
                        //       BULLISH EXIT
                        // ======================
                        Market::Bullish => {

                            // --- TAKE PROFIT ---
                            if current.high >= pattern.trade.reward_exit_price
                                || current.close >= pattern.trade.reward_exit_price
                                || current.open >= pattern.trade.reward_exit_price
                                || current.low >= pattern.trade.reward_exit_price
                            {
                                pattern.trade.open = false;
                                pattern.trade.current_price = pattern.trade.reward_exit_price;
                                pattern.trade.result = Some(true);
                                pattern.trade.length =
                                    pattern.a.length + pattern.b.length + pattern.c.length + pattern.d.length;
                            }

                            // --- STOP LOSS ---
                            if pattern.trade.open && (
                                current.low <= pattern.trade.risk_exit_price
                                || current.close <= pattern.trade.risk_exit_price
                                || current.open <= pattern.trade.risk_exit_price
                                || current.high <= pattern.trade.risk_exit_price
                            ) {
                                pattern.trade.open = false;
                                pattern.trade.current_price = pattern.trade.risk_exit_price;
                                pattern.trade.result = Some(false);
                                pattern.trade.length =
                                    pattern.a.length + pattern.b.length + pattern.c.length + pattern.d.length;
                            }
                        }

                        // ======================
                        //       BEARISH EXIT
                        // ======================
                        Market::Bearish => {

                            // --- TAKE PROFIT ---
                            if current.low <= pattern.trade.reward_exit_price
                                || current.close <= pattern.trade.reward_exit_price
                                || current.open <= pattern.trade.reward_exit_price
                                || current.high <= pattern.trade.reward_exit_price
                            {
                                pattern.trade.open = false;
                                pattern.trade.current_price = pattern.trade.reward_exit_price;
                                pattern.trade.result = Some(true);
                                pattern.trade.length =
                                    pattern.a.length + pattern.b.length + pattern.c.length + pattern.d.length;
                            }

                            // --- STOP LOSS ---
                            if pattern.trade.open && (
                                current.high >= pattern.trade.risk_exit_price
                                || current.close >= pattern.trade.risk_exit_price
                                || current.open >= pattern.trade.risk_exit_price
                                || current.low >= pattern.trade.risk_exit_price
                            ) {
                                pattern.trade.open = false;
                                pattern.trade.current_price = pattern.trade.risk_exit_price;
                                pattern.trade.result = Some(false);
                                pattern.trade.length =
                                    pattern.a.length + pattern.b.length + pattern.c.length + pattern.d.length;
                            }
                        }
                    }
                }
            }   

            // === LOAD === 
            pattern_xabc.extend(pattern_xabc_holder.drain(..));

            // === Clean ===
            pattern_x.retain(|p| {
                match p.x.type_ {
                    PivotType::High => current.high <= p.x.high,  
                    PivotType::Low => current.low >= p.x.low,                   
                }
            });
            pattern_xa.retain(|p| {
                match p.a.type_ {
                    PivotType::High => current.high <= p.a.high && current.low >= p.x.low,  
                    PivotType::Low => current.low >= p.a.low && current.high <= p.x.high,                   
                }
            });
            pattern_xab.retain(|p| match p.b.type_ {
                PivotType::High => current.high <= p.b.high && current.low >= p.a.low && current.high <= p.x.high,
                PivotType::Low  => current.low >= p.b.low && current.high <= p.a.high && current.low >= p.x.low,
            });
            pattern_xabc.retain(|p| match p.c.type_ {
                PivotType::High => current.high <= p.c.high,
                PivotType::Low  => current.low >= p.c.low,
            });
        }

        all_patterns.extend(pattern_xabcd.clone());
        
        let a_bull = pattern_xabcd
            .iter()
            .filter(|p| p.a.type_ == PivotType::High)
            .count();

        let a_bear = pattern_xabcd
            .iter()
            .filter(|p| p.a.type_ == PivotType::Low)
            .count();

        println!(
            "Symbol: {}, XABCD total: {}, Bear: {}, Bull: {}",
            symbol,
            pattern_xabcd.len(),
            a_bear,
            a_bull
        );

    }

    XABCD_CSV::write_patterns_to_csv(&all_patterns, "../../patterns_all.csv")?;

    let abcd_bear = all_patterns
            .iter()
            .filter(|p| p.market == market::Market::Bearish)
            .count();

    let abcd_bull = all_patterns
        .iter()
        .filter(|p| p.market ==  market::Market::Bullish)
        .count();
    println!(
            "XABCD total: {}, Bear: {}, Bull: {}",
     
            all_patterns.len(),
            abcd_bear,
            abcd_bull
    );


    let duration = start.elapsed();
    println!("Execution time: {:?}",  duration);

    Ok(())
}
