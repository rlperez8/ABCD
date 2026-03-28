use std::cmp;
use std::env;
use std::sync::Arc;
use std::time::Instant;

mod models;

use crate::models::accuracy::Accuracies;
use crate::models::database::Database;
use crate::models::*;
use sqlx::mysql::MySqlPool;
use tokio::task::JoinSet;

pub fn truncate_to_2_decimals(value: f64) -> f64 {
    (value * 100.0).trunc() / 100.0
}

fn check_for_pivot(current: &Candle, prev1: &Candle, prev2: &Candle, pivot_type: PivotType) -> bool {
    match pivot_type {
        PivotType::High => prev1.high > current.high && prev1.high > prev2.high,
        PivotType::Low => prev1.low < current.low && prev1.low < prev2.low,
    }
}

fn required_env(name: &str) -> Result<String, Box<dyn std::error::Error>> {
    env::var(name).map_err(|_| format!("Missing required environment variable: {}", name).into())
}

fn env_f64(name: &str, default: f64) -> Result<f64, Box<dyn std::error::Error>> {
    match env::var(name) {
        Ok(value) => Ok(value.trim().parse::<f64>()?),
        Err(_) => Ok(default),
    }
}

fn env_usize(name: &str, default: usize) -> Result<usize, Box<dyn std::error::Error>> {
    match env::var(name) {
        Ok(value) => Ok(value.trim().parse::<usize>()?),
        Err(_) => Ok(default),
    }
}

fn env_flag(name: &str) -> bool {
    match env::var(name) {
        Ok(value) => matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"),
        Err(_) => false,
    }
}

struct SymbolScanResult {
    symbol: String,
    patterns: Vec<PatternXABCD>,
    bearish_count: usize,
    bullish_count: usize,
}

fn scan_symbol(symbol: String, candles: Vec<Candle>) -> SymbolScanResult {
    let symbol = Arc::<str>::from(symbol);

    if candles.len() < 3 {
        return SymbolScanResult {
            symbol: symbol.to_string(),
            patterns: Vec::new(),
            bearish_count: 0,
            bullish_count: 0,
        };
    }

    let sr = SrLine::new(0.0, 0.0);
    let support_level = sr
        .create_support_resistance(&candles)
        .first()
        .map(|line| line.price)
        .unwrap_or(0.0);

    let mut pattern_x: Vec<PatternX> = Vec::new();
    let mut pattern_xa: Vec<PatternXA> = Vec::new();
    let mut pattern_xab: Vec<PatternXAB> = Vec::new();
    let mut pattern_xabc: Vec<PatternXABC> = Vec::new();
    let mut pattern_xabc_holder: Vec<PatternXABC> = Vec::new();
    let mut pattern_xabcd: Vec<PatternXABCD> = Vec::new();

    for window in candles.windows(3) {
        let prev2 = &window[0];
        let prev1 = &window[1];
        let current = &window[2];

        if check_for_pivot(current, prev1, prev2, PivotType::Low) {
            let x = Pivot::new(prev1, PivotType::Low, 0, prev1.low, 0.0);
            pattern_x.push(PatternX { x });
        } else if check_for_pivot(current, prev1, prev2, PivotType::High) {
            let x = Pivot::new(prev1, PivotType::High, 0, prev1.low, 0.0);
            pattern_x.push(PatternX { x });
        }

        for pattern in &mut pattern_x {
            pattern.x.length += 1;

            match pattern.x.type_ {
                PivotType::Low => pattern.x.min_max = pattern.x.min_max.max(prev1.high),
                PivotType::High => pattern.x.min_max = pattern.x.min_max.min(prev1.low),
            }

            let conditions = match pattern.x.type_ {
                PivotType::Low => prev1.high > pattern.x.low && prev1.high >= pattern.x.min_max,
                PivotType::High => prev1.low < pattern.x.high && prev1.low <= pattern.x.min_max,
            };

            let pivot_check = match pattern.x.type_ {
                PivotType::Low => check_for_pivot(current, prev1, prev2, PivotType::High),
                PivotType::High => check_for_pivot(current, prev1, prev2, PivotType::Low),
            };

            let new_a_type = match pattern.x.type_ {
                PivotType::Low => PivotType::High,
                PivotType::High => PivotType::Low,
            };

            if conditions && pivot_check {
                let leg_price_length = match new_a_type {
                    PivotType::Low => pattern.x.low - prev1.low,
                    PivotType::High => prev1.high - pattern.x.low,
                };

                let new_a = Pivot::new(prev1, new_a_type, 0, prev1.low, leg_price_length);

                pattern_xa.push(PatternXA {
                    x: pattern.x,
                    a: new_a,
                });
            }
        }

        for pattern in &mut pattern_xa {
            pattern.a.length += 1;

            match pattern.a.type_ {
                PivotType::Low => pattern.a.min_max = pattern.a.min_max.max(prev1.high),
                PivotType::High => pattern.a.min_max = pattern.a.min_max.min(prev1.low),
            }

            let conditions = match pattern.a.type_ {
                PivotType::Low => prev1.high > pattern.a.high && prev1.high >= pattern.a.min_max,
                PivotType::High => prev1.low < pattern.a.low && prev1.low <= pattern.a.min_max,
            };

            let pivot_check = match pattern.a.type_ {
                PivotType::Low => check_for_pivot(current, prev1, prev2, PivotType::High),
                PivotType::High => check_for_pivot(current, prev1, prev2, PivotType::Low),
            };

            let new_b_type = match pattern.a.type_ {
                PivotType::Low => PivotType::High,
                PivotType::High => PivotType::Low,
            };

            if conditions && pivot_check {
                let leg_price_length = match new_b_type {
                    PivotType::Low => pattern.a.high - prev1.low,
                    PivotType::High => prev1.high - pattern.a.low,
                };

                let new_b = Pivot::new(prev1, new_b_type, 0, prev1.low, leg_price_length);

                pattern_xab.push(PatternXAB {
                    x: pattern.x,
                    a: pattern.a,
                    b: new_b,
                });
            }
        }

        for pattern in &mut pattern_xab {
            pattern.b.length += 1;

            match pattern.b.type_ {
                PivotType::Low => pattern.b.min_max = pattern.b.min_max.max(prev1.high),
                PivotType::High => pattern.b.min_max = pattern.b.min_max.min(prev1.low),
            }

            let conditions = match pattern.b.type_ {
                PivotType::Low => {
                    prev1.high >= pattern.b.min_max
                        && prev1.high > pattern.b.low
                        && prev1.high < pattern.a.high
                }
                PivotType::High => {
                    prev1.low <= pattern.b.min_max
                        && prev1.low < pattern.b.high
                        && prev1.low > pattern.a.low
                }
            };

            let pivot_check = match pattern.b.type_ {
                PivotType::Low => check_for_pivot(current, prev1, prev2, PivotType::High),
                PivotType::High => check_for_pivot(current, prev1, prev2, PivotType::Low),
            };

            let new_c_type = match pattern.b.type_ {
                PivotType::Low => PivotType::High,
                PivotType::High => PivotType::Low,
            };

            if conditions && pivot_check {
                let leg_price_length = match new_c_type {
                    PivotType::Low => pattern.b.low - prev1.low,
                    PivotType::High => prev1.high - pattern.b.low,
                };

                let new_c = Pivot::new(prev1, new_c_type, 0, prev1.low, leg_price_length);

                pattern_xabc_holder.push(PatternXABC {
                    x: pattern.x,
                    a: pattern.a,
                    b: pattern.b,
                    c: new_c,
                });
            }
        }

        for pattern in &mut pattern_xabc {
            pattern.c.length += 1;

            match pattern.c.type_ {
                PivotType::Low => pattern.c.min_max = pattern.c.min_max.max(prev1.high),
                PivotType::High => pattern.c.min_max = pattern.c.min_max.min(prev1.low),
            }

            let conditions = match pattern.c.type_ {
                PivotType::Low => {
                    prev1.high >= pattern.c.min_max
                        && prev1.high > pattern.b.high
                        && prev1.high < pattern.x.high
                }
                PivotType::High => {
                    prev1.low <= pattern.c.min_max
                        && prev1.low < pattern.b.low
                        && prev1.low > pattern.x.low
                }
            };

            let pivot_check = match pattern.c.type_ {
                PivotType::Low => check_for_pivot(current, prev1, prev2, PivotType::High),
                PivotType::High => check_for_pivot(current, prev1, prev2, PivotType::Low),
            };

            let three_month = match pattern.c.type_ {
                PivotType::Low => prev1.high == support_level,
                PivotType::High => prev1.low == support_level,
            };

            if conditions && pivot_check {
                let new_d_type = match pattern.c.type_ {
                    PivotType::Low => PivotType::High,
                    PivotType::High => PivotType::Low,
                };

                let market = match pattern.c.type_ {
                    PivotType::Low => Market::Bearish,
                    PivotType::High => Market::Bullish,
                };

                let leg_price_length = match new_d_type {
                    PivotType::Low => pattern.c.high - prev1.low,
                    PivotType::High => prev1.high - pattern.c.low,
                };

                let new_d = Pivot::new(prev1, new_d_type, 0, prev1.low, leg_price_length);

                let trade = Trade::new(
                    market,
                    &pattern.x,
                    &pattern.a,
                    &pattern.b,
                    &pattern.c,
                    prev1,
                    support_level,
                    ReversalType::None,
                );

                pattern_xabcd.push(PatternXABCD {
                    symbol: Arc::clone(&symbol),
                    x: pattern.x,
                    a: pattern.a,
                    b: pattern.b,
                    c: pattern.c,
                    d: new_d,
                    market,
                    abcd_type: HarmonicType::None,
                    trade,
                    three_month: Some(three_month),
                    six_month: Some(false),
                    twelve_month: Some(false),
                    accuracies: Accuracies::new(),
                });
            }
        }

        for pattern in &mut pattern_xabcd {
            if pattern.trade.open {
                pattern.d.length += 1;

                let pnl = match pattern.market {
                    Market::Bullish => current.close - pattern.trade.enter_price,
                    Market::Bearish => pattern.trade.enter_price - current.close,
                };

                pattern.trade.pnl = pnl;
                pattern.trade.date = current.date;
                pattern.trade.current_price = truncate_to_2_decimals(current.close);

                match pattern.market {
                    Market::Bullish => {
                        if current.high >= pattern.trade.reward_exit_price
                            || current.close >= pattern.trade.reward_exit_price
                            || current.open >= pattern.trade.reward_exit_price
                            || current.low >= pattern.trade.reward_exit_price
                        {
                            pattern.trade.open = false;
                            pattern.trade.current_price = pattern.trade.reward_exit_price;
                            pattern.trade.result = 1;
                            pattern.trade.length = pattern.a.length
                                + pattern.b.length
                                + pattern.c.length
                                + pattern.d.length;
                        }

                        if pattern.trade.open
                            && (current.low <= pattern.trade.risk_exit_price
                                || current.close <= pattern.trade.risk_exit_price
                                || current.open <= pattern.trade.risk_exit_price
                                || current.high <= pattern.trade.risk_exit_price)
                        {
                            pattern.trade.open = false;
                            pattern.trade.current_price = pattern.trade.risk_exit_price;
                            pattern.trade.result = 2;
                            pattern.trade.length = pattern.a.length
                                + pattern.b.length
                                + pattern.c.length
                                + pattern.d.length;
                        }
                    }
                    Market::Bearish => {
                        if current.low <= pattern.trade.reward_exit_price
                            || current.close <= pattern.trade.reward_exit_price
                            || current.open <= pattern.trade.reward_exit_price
                            || current.high <= pattern.trade.reward_exit_price
                        {
                            pattern.trade.open = false;
                            pattern.trade.current_price = pattern.trade.reward_exit_price;
                            pattern.trade.result = 1;
                            pattern.trade.length = pattern.a.length
                                + pattern.b.length
                                + pattern.c.length
                                + pattern.d.length;
                        }

                        if pattern.trade.open
                            && (current.high >= pattern.trade.risk_exit_price
                                || current.close >= pattern.trade.risk_exit_price
                                || current.open >= pattern.trade.risk_exit_price
                                || current.low >= pattern.trade.risk_exit_price)
                        {
                            pattern.trade.open = false;
                            pattern.trade.current_price = pattern.trade.risk_exit_price;
                            pattern.trade.result = 2;
                            pattern.trade.length = pattern.a.length
                                + pattern.b.length
                                + pattern.c.length
                                + pattern.d.length;
                        }
                    }
                }
            }
        }

        pattern_xabc.extend(pattern_xabc_holder.drain(..));

        pattern_x.retain(|p| match p.x.type_ {
            PivotType::High => current.high <= p.x.high,
            PivotType::Low => current.low >= p.x.low,
        });
        pattern_xa.retain(|p| match p.a.type_ {
            PivotType::High => current.high <= p.a.high && current.low >= p.x.low,
            PivotType::Low => current.low >= p.a.low && current.high <= p.x.high,
        });
        pattern_xab.retain(|p| match p.b.type_ {
            PivotType::High => {
                current.high <= p.b.high && current.low >= p.a.low && current.high <= p.x.high
            }
            PivotType::Low => {
                current.low >= p.b.low && current.high <= p.a.high && current.low >= p.x.low
            }
        });
        pattern_xabc.retain(|p| match p.c.type_ {
            PivotType::High => current.high <= p.c.high,
            PivotType::Low => current.low >= p.c.low,
        });
    }

    let bearish_count = pattern_xabcd
        .iter()
        .filter(|p| p.market == Market::Bearish)
        .count();
    let bullish_count = pattern_xabcd
        .iter()
        .filter(|p| p.market == Market::Bullish)
        .count();

    SymbolScanResult {
        symbol: symbol.to_string(),
        patterns: pattern_xabcd,
        bearish_count,
        bullish_count,
    }
}

fn spawn_symbol_scan(
    tasks: &mut JoinSet<Result<SymbolScanResult, String>>,
    pool: MySqlPool,
    symbol: String,
) {
    tasks.spawn(async move {
        let db = Database { pool };
        let candles = db
            .get_stored_candles(&symbol)
            .await
            .map_err(|error| format!("{}: failed to load candles: {}", symbol, error))?;

        let symbol_for_error = symbol.clone();
        tokio::task::spawn_blocking(move || scan_symbol(symbol, candles))
            .await
            .map_err(|error| format!("{}: scanner worker failed: {}", symbol_for_error, error))
    });
}

async fn flush_pending_patterns(
    db: &Database,
    pending_patterns: &mut Vec<PatternXABCD>,
) -> Result<(), Box<dyn std::error::Error>> {
    if pending_patterns.is_empty() {
        return Ok(());
    }

    db.insert_scatter_plot(pending_patterns).await?;
    db.insert_patterns(pending_patterns).await?;
    pending_patterns.clear();

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();

    dotenvy::dotenv().ok();

    let database_url = required_env("ABCD_DATABASE_URL")?;
    let minimum_average_volume = env_f64("ABCD_MIN_AVG_VOLUME", 500000.0)?;
    let reset_outputs = env_flag("ABCD_RESET_OUTPUTS");
    let scan_concurrency = cmp::max(1, env_usize("ABCD_SCAN_CONCURRENCY", 4)?);
    let write_batch_size = cmp::max(1, env_usize("ABCD_WRITE_BATCH_SIZE", 1000)?);

    let pool = match MySqlPool::connect(&database_url).await {
        Ok(pool) => {
            println!("Connected to local DB");
            pool
        }
        Err(error) => {
            eprintln!("Failed to connect to DB: {:?}", error);
            return Err(error.into());
        }
    };

    let db = Database {
        pool: pool.clone(),
    };

    if reset_outputs {
        db.clear_generated_outputs().await?;
        println!("Cleared xabcd output tables before run");
    }

    let symbols = db
        .get_symbols_above_average_volume(minimum_average_volume)
        .await?;

    println!(
        "Running xabcd scan on {} symbols with recent average volume >= {}",
        symbols.len(),
        minimum_average_volume
    );
    println!(
        "Using scan concurrency {} and write batch size {}",
        scan_concurrency,
        write_batch_size
    );

    let total_symbols = symbols.len();
    let mut completed_symbols = 0usize;
    let mut failed_symbols = 0usize;
    let mut total_patterns_written = 0usize;
    let mut total_bearish_patterns = 0usize;
    let mut total_bullish_patterns = 0usize;
    let mut pending_patterns: Vec<PatternXABCD> = Vec::new();

    let mut tasks: JoinSet<Result<SymbolScanResult, String>> = JoinSet::new();
    let mut symbol_iter = symbols.into_iter();

    for _ in 0..scan_concurrency {
        if let Some(symbol) = symbol_iter.next() {
            spawn_symbol_scan(&mut tasks, pool.clone(), symbol);
        }
    }

    while let Some(task_result) = tasks.join_next().await {
        if let Some(symbol) = symbol_iter.next() {
            spawn_symbol_scan(&mut tasks, pool.clone(), symbol);
        }

        completed_symbols += 1;

        let result = match task_result {
            Ok(Ok(result)) => result,
            Ok(Err(message)) => {
                failed_symbols += 1;
                eprintln!(
                    "Skipping symbol after worker error ({}/{}): {}",
                    completed_symbols,
                    total_symbols,
                    message
                );
                continue;
            }
            Err(error) => {
                failed_symbols += 1;
                eprintln!(
                    "Worker join failed ({}/{}): {}",
                    completed_symbols,
                    total_symbols,
                    error
                );
                continue;
            }
        };

        let mut patterns = Accuracies::new().get_accuracy(result.patterns);
        total_patterns_written += patterns.len();
        total_bearish_patterns += result.bearish_count;
        total_bullish_patterns += result.bullish_count;

        println!(
            "Symbol: {}, XABCD total: {}, Bear: {}, Bull: {} ({}/{})",
            result.symbol,
            patterns.len(),
            result.bearish_count,
            result.bullish_count,
            completed_symbols,
            total_symbols
        );

        pending_patterns.append(&mut patterns);

        if pending_patterns.len() >= write_batch_size {
            let flush_size = pending_patterns.len();
            flush_pending_patterns(&db, &mut pending_patterns).await?;
            println!("Flushed {} patterns to DB", flush_size);
        }
    }

    flush_pending_patterns(&db, &mut pending_patterns).await?;

    println!(
        "XABCD total: {}, Bear: {}, Bull: {}, Failed symbols: {}",
        total_patterns_written,
        total_bearish_patterns,
        total_bullish_patterns,
        failed_symbols
    );

    let duration = start.elapsed();
    println!("Execution time: {:?}", duration);

    Ok(())
}
