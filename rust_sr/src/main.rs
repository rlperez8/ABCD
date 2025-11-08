use mysql::*;
use mysql::prelude::*;
use chrono::NaiveDate;
use std::time::Instant;
use std::fs::File;
use csv::WriterBuilder;
use serde::Serialize;

#[derive(Debug, Clone)]
struct Candle {
    symbol: String,
    date: NaiveDate,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: u64,
}
#[derive(Debug, Clone, Serialize)]
struct Pivot {
    date: String,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    length: i64,
    min_max: f64,
}
impl Pivot {
    fn new(candle: &Candle, length: i64, min_max: f64) -> Self {
        Self {
            date: candle.date.to_string(),
            open: candle.open,
            high: candle.high,
            low: candle.low,
            close: candle.close,
            length,
            min_max,
        }
    }
}
#[derive(Debug, Clone, Serialize)]
struct Trade {
    open: bool,
    risk_exit_price: f64,
    reward_exit_price: f64,
    enter_price: f64,
    exit_price: Option<f64>,
    current_price: f64,
    length: i64,
    pnl: f64,
    result: Option<bool>,
}
#[derive(Debug, Clone)]
struct PatternA {
    a: Pivot,
}
#[derive(Debug, Clone)]
struct PatternAB {
    a: Pivot,
    b: Pivot,
}
#[derive(Debug, Clone)]
struct PatternABC {
    a: Pivot,
    b: Pivot,
    c: Pivot,
}
#[derive(Debug, Clone, Serialize)]
struct PatternABCD {
    symbol: String,
    a: Pivot,
    b: Pivot,
    c: Pivot,
    d: Pivot,
    trade: Trade,
}
enum PivotType {
    High,
    Low,
}
#[derive(Serialize)]
struct PatternABCDCsv {
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
    trade_exit_price: Option<f64>,
    trade_current_price: f64,
    trade_length: i64,
    trade_pnl: f64,
    trade_result: Option<bool>,
}
fn get_distinct_symbols(conn: &mut PooledConn) -> mysql::Result<Vec<String>> {
    let query = r#"
        SELECT DISTINCT symbol
        FROM abcd.candles
        ORDER BY symbol
    "#;

    let symbols: Vec<String> = conn.query_map(query, |symbol: String| symbol)?;
    Ok(symbols)
}
fn get_stored_candles(symbol: &str, conn: &mut PooledConn) -> mysql::Result<Vec<Candle>> {
    let query = r#"
        SELECT symbol,
               DATE_FORMAT(date, '%Y-%m-%d') AS date_str,
               open, high, low, close, volume
        FROM abcd.candles
        WHERE symbol = :symbol
        ORDER BY date DESC
        LIMIT 90
    "#;

    let params = params! { "symbol" => symbol };

    let candles: Vec<Candle> = conn.exec_map(
        query,
        params,
        |(symbol, date_str, open, high, low, close, volume): (String, String, f64, f64, f64, f64, u64)| {
            let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .expect("Failed to parse date");
            Candle { symbol, date, open, high, low, close, volume }
        },
    )?;

    Ok(candles)
}
fn check_for_pivot(current: &Candle, prev1: &Candle, prev2: &Candle, pivot_type: PivotType) -> bool {
    match pivot_type {
        PivotType::High => prev1.high > current.high && prev1.high > prev2.high,
        PivotType::Low => prev1.low < current.low && prev1.low < prev2.low,
    }
}
fn write_patterns_to_csv(patterns: &[PatternABCD]) -> std::result::Result<(), Box<dyn std::error::Error>> {
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
        trade_exit_price: p.trade.exit_price,
        trade_current_price: p.trade.current_price,
        trade_length: p.trade.length,
        trade_pnl: p.trade.pnl,
        trade_result: p.trade.result,
    }).collect();

    let file = File::create("patterns_all.csv")?;
    let mut writer = WriterBuilder::new().has_headers(true).from_writer(file);

    for p in csv_patterns {
        writer.serialize(p)?;
    }

    writer.flush()?;
    Ok(())
}
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();

    // Connect to database
    let url = "mysql://rperezkc:Nar8uto!@localhost:3306/abcd";
    let pool = Pool::new(url)?;
    let mut conn = pool.get_conn()?;

    // Prepare vector for all patterns across all symbols
    let mut all_patterns: Vec<PatternABCD> = Vec::new();

    // Fetch all candles
    let symbols = get_distinct_symbols(&mut conn)?;
    
    for symbol in symbols {
        let mut candles = get_stored_candles(&symbol, &mut conn)?;
        candles.reverse();

        let mut pattern_a: Vec<PatternA> = Vec::new();
        let mut pattern_ab: Vec<PatternAB> = Vec::new();
        let mut pattern_abc: Vec<PatternABC> = Vec::new();
        let mut pattern_abcd: Vec<PatternABCD> = Vec::new();

        for window in candles.windows(3) {
            let prev2 = &window[0];
            let prev1 = &window[1];
            let current = &window[2];

            // === A ===
            if check_for_pivot(current, prev1, prev2, PivotType::High) {
                let a = Pivot::new(prev1, 0, prev1.low);
                pattern_a.push(PatternA { a });
            }

            // === B ===
            for pattern in &mut pattern_a {
                pattern.a.length += 1;
                pattern.a.min_max = pattern.a.min_max.min(prev1.low);

                if prev1.low < pattern.a.low && prev1.low <= pattern.a.min_max {
                    if check_for_pivot(current, prev1, prev2, PivotType::Low) {
                        let b = Pivot::new(prev1, 0, prev1.high);
                        pattern_ab.push(PatternAB { a: pattern.a.clone(), b });
                    }
                }
            }

            // === C ===
            for pattern in &mut pattern_ab {
                pattern.a.length += 1;
                pattern.b.min_max = pattern.b.min_max.max(prev1.high);

                if prev1.high >= pattern.b.min_max {
                    if prev1.high > pattern.b.low && prev1.high < pattern.a.high {
                        if check_for_pivot(current, prev1, prev2, PivotType::High) {
                            let c = Pivot::new(prev1, 0, prev1.low);
                            pattern_abc.push(PatternABC { a: pattern.a.clone(), b: pattern.b.clone(), c });
                        }
                    }
                }
            }

            // === D ===
            for pattern in &mut pattern_abc {
                pattern.c.length += 1;
                pattern.c.min_max = pattern.c.min_max.min(prev1.low);

                if prev1.low <= pattern.c.min_max && prev1.low < pattern.b.low {
                    if check_for_pivot(current, prev1, prev2, PivotType::Low) {
                        let d = Pivot::new(prev1, 0, prev1.high);
                        let trade = Trade {
                            open: true,
                            risk_exit_price: pattern.b.high,
                            reward_exit_price: pattern.c.high,
                            enter_price: prev1.close,
                            exit_price: None,
                            current_price: prev1.close,
                            length: 0,
                            pnl: 0.0,
                            result: None,
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

            // === Exit ===
            for pattern in &mut pattern_abcd {
                pattern.trade.length += 1;
                let unrealized_pnl = prev1.close - pattern.trade.enter_price;
                pattern.trade.pnl = unrealized_pnl;

                if prev1.high > pattern.trade.reward_exit_price {
                    pattern.trade.open = false;
                    pattern.trade.exit_price = Some(prev1.high);
                    pattern.trade.result = Some(true);
                }

                if prev1.low < pattern.trade.risk_exit_price {
                    pattern.trade.open = false;
                    pattern.trade.exit_price = Some(prev1.low);
                    pattern.trade.result = Some(false);
                }
            }

            pattern_a.retain(|p| prev1.high <= p.a.high);
            pattern_ab.retain(|p| prev1.low >= p.b.low && prev1.high <= p.a.high);
            pattern_abc.retain(|p| prev1.high <= p.c.high);
        }

        println!("{} Detected Pivot A→B→C→D patterns: {}", symbol, pattern_abcd.len());
        all_patterns.extend(pattern_abcd);
    }

    // === Write all patterns to one big CSV ===
    write_patterns_to_csv(&all_patterns)?;

    let duration = start.elapsed();
    println!("✅ Patterns written to patterns_all.csv in {:.2?}", duration);

    Ok(())
}
