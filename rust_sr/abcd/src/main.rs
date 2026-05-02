use std::cmp;
use std::env;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use abcd::models::accuracy::Accuracies;
use abcd::models::database::{Database, OutputWriteOptions};
use abcd::models::*;
use sqlx::mysql::MySqlPool;
use tokio::task::JoinSet;

pub fn truncate_to_2_decimals(value: f64) -> f64 {
    (value * 100.0).trunc() / 100.0
}

fn check_for_pivot(
    current: &Candle,
    prev1: &Candle,
    prev2: &Candle,
    pivot_type: PivotType,
) -> bool {
    match pivot_type {
        PivotType::High => prev1.high > current.high && prev1.high > prev2.high,
        PivotType::Low => prev1.low < current.low && prev1.low < prev2.low,
    }
}

fn calculate_x_extreme_bars_left(candles: &[Candle], x_index: usize, x: &Pivot) -> i64 {
    candles[..x_index]
        .iter()
        .rev()
        .take_while(|candle| match x.type_ {
            PivotType::High => candle.high <= x.high,
            PivotType::Low => candle.low >= x.low,
        })
        .count() as i64
}

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
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

fn env_i64(name: &str, default: i64) -> Result<i64, Box<dyn std::error::Error>> {
    match env::var(name) {
        Ok(value) => Ok(value.trim().parse::<i64>()?),
        Err(_) => Ok(default),
    }
}

fn env_flag(name: &str) -> bool {
    match env::var(name) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        ),
        Err(_) => false,
    }
}

fn env_flag_or(name: &str, default: bool) -> bool {
    match env::var(name) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" => true,
            "0" | "false" | "no" => false,
            _ => default,
        },
        Err(_) => default,
    }
}

struct SymbolScanResult {
    symbol: String,
    candle_count: usize,
    patterns: Vec<PatternXABCD>,
    prop_reversal_outcomes: Vec<PropReversalOutcome>,
    bearish_count: usize,
    bullish_count: usize,
}

#[derive(Debug, Clone, Copy)]
enum CandleSource {
    EquityCandles,
    FuturesContracts,
}

impl CandleSource {
    fn from_env() -> Self {
        match env::var("ABCD_CANDLE_SOURCE")
            .unwrap_or_else(|_| "candles".to_string())
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "futures_contracts" | "futures" | "contracts" => Self::FuturesContracts,
            _ => Self::EquityCandles,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::EquityCandles => "candles",
            Self::FuturesContracts => "futures_contracts",
        }
    }
}

fn engine_run_id() -> String {
    let started_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();

    format!("engine-{}-{}", started_at_ms, std::process::id())
}

fn apply_closed_trade_reversal(pattern: &mut PatternXABCD, candles: &[Candle], close_index: usize) {
    let reversal_signals = classify_closed_reversal_pattern(
        candles,
        &pattern.reversal_context,
        close_index,
        pattern.market,
    );

    pattern.trade.reversal_type = reversal_signals.reversal_type;
    pattern.trade.bullish_key_reversal = reversal_signals.bullish_key_reversal;
    pattern.trade.bearish_key_reversal = reversal_signals.bearish_key_reversal;
    pattern.trade.bullish_engulfing = reversal_signals.bullish_engulfing;
    pattern.trade.bearish_engulfing = reversal_signals.bearish_engulfing;
    pattern.trade.bullish_outside_reversal = reversal_signals.bullish_outside_reversal;
    pattern.trade.bearish_outside_reversal = reversal_signals.bearish_outside_reversal;
    pattern.trade.hammer = reversal_signals.hammer;
    pattern.trade.shooting_star = reversal_signals.shooting_star;
    pattern.trade.morning_star = reversal_signals.morning_star;
    pattern.trade.evening_star = reversal_signals.evening_star;
    pattern.trade.three_white_soldiers = reversal_signals.three_white_soldiers;
    pattern.trade.three_black_crows = reversal_signals.three_black_crows;
}

fn contract_age_fields(candles: &[Candle], d_candle: &Candle) -> (Option<i64>, Option<i64>) {
    let Some(first_candle) = candles.first() else {
        return (None, None);
    };
    let days_from_start = d_candle
        .date
        .date()
        .signed_duration_since(first_candle.date.date())
        .num_days()
        .max(0);
    let week_index = (days_from_start / 7) + 1;
    (Some(week_index), Some(days_from_start))
}

fn scan_symbol(
    symbol: String,
    candles: Vec<Candle>,
    max_x_bars_left: Option<i64>,
) -> SymbolScanResult {
    let symbol = Arc::<str>::from(symbol);

    if candles.len() < 3 {
        return SymbolScanResult {
            symbol: symbol.to_string(),
            candle_count: candles.len(),
            patterns: Vec::new(),
            prop_reversal_outcomes: Vec::new(),
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
    let mut pattern_xabcd_holder: Vec<PatternXABCD> = Vec::new();

    for (window_idx, window) in candles.windows(3).enumerate() {
        let prev2 = &window[0];
        let prev1 = &window[1];
        let current = &window[2];
        let prev1_index = window_idx + 1;
        let current_index = window_idx + 2;

        if check_for_pivot(current, prev1, prev2, PivotType::Low) {
            let x = Pivot::new(prev1, PivotType::Low, 0, prev1.low, 0.0);
            pattern_x.push(PatternX {
                x,
                x_index: prev1_index,
            });
        } else if check_for_pivot(current, prev1, prev2, PivotType::High) {
            let x = Pivot::new(prev1, PivotType::High, 0, prev1.low, 0.0);
            pattern_x.push(PatternX {
                x,
                x_index: prev1_index,
            });
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
                let mut finalized_x = pattern.x;
                finalized_x.length = prev1_index.saturating_sub(pattern.x_index) as i64;

                pattern_xa.push(PatternXA {
                    x: finalized_x,
                    a: new_a,
                    x_index: pattern.x_index,
                    a_index: prev1_index,
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
                let mut finalized_a = pattern.a;
                finalized_a.length = prev1_index.saturating_sub(pattern.a_index) as i64;

                pattern_xab.push(PatternXAB {
                    x: pattern.x,
                    a: finalized_a,
                    b: new_b,
                    x_index: pattern.x_index,
                    a_index: pattern.a_index,
                    b_index: prev1_index,
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
                let mut finalized_b = pattern.b;
                finalized_b.length = prev1_index.saturating_sub(pattern.b_index) as i64;

                pattern_xabc_holder.push(PatternXABC {
                    x: pattern.x,
                    a: pattern.a,
                    b: finalized_b,
                    c: new_c,
                    x_index: pattern.x_index,
                    a_index: pattern.a_index,
                    b_index: pattern.b_index,
                    c_index: prev1_index,
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
                let mut finalized_c = pattern.c;
                finalized_c.length = prev1_index.saturating_sub(pattern.c_index) as i64;

                let entry_index = current_index + 1;
                let entry_candle = candles.get(entry_index);
                let (contract_week_index, contract_days_from_start) =
                    contract_age_fields(&candles, prev1);

                let trade = Trade::new(
                    market,
                    &pattern.x,
                    &pattern.a,
                    &pattern.b,
                    &finalized_c,
                    prev1,
                    entry_candle,
                    support_level,
                    ReversalType::None,
                );

                pattern_xabcd_holder.push(PatternXABCD {
                    symbol: Arc::clone(&symbol),
                    pattern_id: String::new(),
                    x_bars_left: 0,
                    x_index: pattern.x_index,
                    x: pattern.x,
                    a: pattern.a,
                    b: pattern.b,
                    c: finalized_c,
                    d: new_d,
                    market,
                    trade,
                    entry_index: entry_candle.map(|_| entry_index),
                    reversal_context: ReversalPatternContext {
                        d_index: prev1_index,
                    },
                    d_confirm_date: current.date,
                    target_candle: None,
                    contract_week_index,
                    contract_days_from_start,
                    three_month: prev1.three_month,
                    six_month: prev1.six_month,
                    twelve_month: prev1.twelve_month,
                    accuracies: Accuracies::new(),
                    time_accuracies: Default::default(),
                    prop_strategy_id: String::new(),
                });
            }
        }

        for pattern in &mut pattern_xabcd {
            if pattern.trade.open {
                let Some(entry_index) = pattern.entry_index else {
                    continue;
                };
                if current_index < entry_index {
                    continue;
                }
                let Some(entry_candle) = candles.get(entry_index) else {
                    continue;
                };

                pattern.d.length =
                    current_index.saturating_sub(pattern.reversal_context.d_index) as i64;
                pattern.trade.length = pattern.d.length;

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
                            pattern.trade.length = pattern.d.length;
                            pattern.target_candle =
                                Some(TargetCandle::from_candle(current, entry_candle));
                            apply_closed_trade_reversal(pattern, &candles, current_index);
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
                            pattern.trade.length = pattern.d.length;
                            pattern.target_candle =
                                Some(TargetCandle::from_candle(current, entry_candle));
                            apply_closed_trade_reversal(pattern, &candles, current_index);
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
                            pattern.trade.length = pattern.d.length;
                            pattern.target_candle =
                                Some(TargetCandle::from_candle(current, entry_candle));
                            apply_closed_trade_reversal(pattern, &candles, current_index);
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
                            pattern.trade.length = pattern.d.length;
                            pattern.target_candle =
                                Some(TargetCandle::from_candle(current, entry_candle));
                            apply_closed_trade_reversal(pattern, &candles, current_index);
                        }
                    }
                }
            }
        }

        pattern_xabcd.extend(pattern_xabcd_holder.drain(..));
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

    let mut patterns = Accuracies::new().get_accuracy(pattern_xabcd);
    for pattern in &mut patterns {
        pattern.x_bars_left = calculate_x_extreme_bars_left(&candles, pattern.x_index, &pattern.x);
    }
    if let Some(max_x_bars_left) = max_x_bars_left {
        patterns.retain(|pattern| pattern.x_bars_left <= max_x_bars_left);
    }
    let bearish_count = patterns
        .iter()
        .filter(|p| p.market == Market::Bearish)
        .count();
    let bullish_count = patterns
        .iter()
        .filter(|p| p.market == Market::Bullish)
        .count();
    let prop_reversal_outcomes = build_prop_reversal_outcomes(&patterns, &candles);

    SymbolScanResult {
        symbol: symbol.to_string(),
        candle_count: candles.len(),
        patterns,
        prop_reversal_outcomes,
        bearish_count,
        bullish_count,
    }
}

fn spawn_symbol_scan(
    tasks: &mut JoinSet<Result<SymbolScanResult, String>>,
    pool: MySqlPool,
    symbol: String,
    candle_source: CandleSource,
    max_x_bars_left: Option<i64>,
) {
    tasks.spawn(async move {
        let db = Database { pool };
        let candles = match candle_source {
            CandleSource::EquityCandles => db.get_stored_candles(&symbol).await,
            CandleSource::FuturesContracts => db.get_stored_futures_contract_candles(&symbol).await,
        }
        .map_err(|error| format!("{}: failed to load candles: {}", symbol, error))?;
        let candle_count = candles.len();

        let symbol_for_error = symbol.clone();
        let mut result =
            tokio::task::spawn_blocking(move || scan_symbol(symbol, candles, max_x_bars_left))
                .await
                .map_err(|error| {
                    format!("{}: scanner worker failed: {}", symbol_for_error, error)
                })?;
        result.candle_count = candle_count;
        Ok(result)
    });
}

async fn flush_pending_patterns(
    db: &Database,
    run_id: &str,
    pending_patterns: &mut Vec<PatternXABCD>,
    pending_prop_reversal_outcomes: &mut Vec<PropReversalOutcome>,
    fast_rebuild: bool,
    use_build_tables: bool,
    write_pattern_setups: bool,
    write_harmonic_scores: bool,
    target_ready_outcomes_only: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if pending_patterns.is_empty() {
        return Ok(());
    }

    if write_pattern_setups || write_harmonic_scores {
        db.insert_pattern_setups_with_timings(
            pending_patterns,
            Some(run_id),
            fast_rebuild,
            use_build_tables,
            write_pattern_setups,
            write_harmonic_scores,
            false,
            false,
        )
        .await?;
    } else {
        db.record_engine_phase_timing(
            run_id,
            None,
            "write_pattern_setups",
            Some(0),
            std::time::Duration::ZERO,
            Some("disabled; strategy run writes pattern_outcomes_prop directly"),
        )
        .await?;
    }

    let phase_started = Instant::now();
    let prop_outcome_rows_written = db
        .sync_prop_outcomes(
            pending_patterns,
            pending_prop_reversal_outcomes,
            use_build_tables,
            target_ready_outcomes_only,
        )
        .await?;
    db.record_engine_phase_timing(
        run_id,
        None,
        "write_prop_outcomes",
        Some(prop_outcome_rows_written),
        phase_started.elapsed(),
        Some(if target_ready_outcomes_only {
            "closed/target-ready outcome rows only"
        } else {
            "all outcome rows, including open/not-target-ready rows"
        }),
    )
    .await?;
    pending_patterns.clear();
    pending_prop_reversal_outcomes.clear();

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let run_id = engine_run_id();

    dotenvy::dotenv().ok();

    let database_url = database_url_from_env()?;
    let minimum_average_volume = env_f64("ABCD_MIN_AVG_VOLUME", 500000.0)?;
    let reset_outputs = env_flag("ABCD_RESET_OUTPUTS");
    let use_build_tables = env_flag("ABCD_USE_BUILD_TABLES");
    let fast_rebuild = use_build_tables || reset_outputs || env_flag("ABCD_FAST_REBUILD");
    let write_pattern_setups = env_flag_or("ABCD_WRITE_PATTERN_SETUPS", true);
    let write_harmonic_scores = env_flag("ABCD_WRITE_HARMONIC_SCORES");
    let write_prop_outcomes = true;
    let target_ready_outcomes_only = env_flag_or("ABCD_TARGET_READY_OUTCOMES_ONLY", true)
        && !env_flag("ABCD_WRITE_OPEN_PROP_OUTCOMES");
    let candle_source = CandleSource::from_env();
    let futures_root = env::var("ABCD_FUTURES_ROOT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let output_write_options = OutputWriteOptions {
        write_pattern_setups,
        write_harmonic_scores,
        write_swing_outcomes: false,
        write_prop_outcomes,
    };
    let refresh_prop_family_summaries =
        env_flag_or("ABCD_REFRESH_PROP_FAMILY_SUMMARIES", write_prop_outcomes)
            && !env_flag("ABCD_SKIP_PROP_FAMILY_SUMMARIES");
    let scan_concurrency = cmp::max(1, env_usize("ABCD_SCAN_CONCURRENCY", 4)?);
    let write_batch_size = cmp::max(1, env_usize("ABCD_WRITE_BATCH_SIZE", 1000)?);
    let symbol_offset = env_usize("ABCD_SYMBOL_OFFSET", 0)?;
    let symbol_limit = env_usize("ABCD_SYMBOL_LIMIT", 0)?;
    let max_x_bars_left = match env_i64("ABCD_MAX_X_BARS_LEFT", 0)? {
        value if value > 0 => Some(value),
        _ => None,
    };

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

    let db = Database { pool: pool.clone() };

    db.ensure_engine_phase_timings_table().await?;
    println!("Engine run id: {}", run_id);
    db.record_engine_phase_timing(
        &run_id,
        None,
        "bootstrap",
        None,
        start.elapsed(),
        Some("config, database connection, timing table setup"),
    )
    .await?;

    let phase_started = Instant::now();
    if matches!(candle_source, CandleSource::EquityCandles) {
        db.ensure_candle_trend_columns().await?;
    }
    db.ensure_pattern_mode_tables().await?;
    db.drop_legacy_xabcd_patterns_table().await?;
    db.record_engine_phase_timing(
        &run_id,
        None,
        "ensure_tables",
        None,
        phase_started.elapsed(),
        None,
    )
    .await?;

    if reset_outputs && !use_build_tables {
        let phase_started = Instant::now();
        db.clear_generated_outputs().await?;
        db.record_engine_phase_timing(
            &run_id,
            None,
            "reset_outputs",
            None,
            phase_started.elapsed(),
            None,
        )
        .await?;
        println!("Cleared xabcd output tables before run");
    }

    if use_build_tables {
        let phase_started = Instant::now();
        db.recreate_build_output_tables(output_write_options)
            .await?;
        db.record_engine_phase_timing(
            &run_id,
            None,
            "recreate_build_tables",
            None,
            phase_started.elapsed(),
            Some("fresh build tables created from current output schemas"),
        )
        .await?;
        println!("Created fresh build tables for output swap");
    } else if fast_rebuild && reset_outputs {
        let phase_started = Instant::now();
        db.recreate_fast_rebuild_output_tables().await?;
        db.record_engine_phase_timing(
            &run_id,
            None,
            "recreate_fast_rebuild_tables",
            None,
            phase_started.elapsed(),
            Some("append-friendly generated output table schema"),
        )
        .await?;
        println!("Recreated generated output tables with append-friendly schema");
    }

    if fast_rebuild {
        let phase_started = Instant::now();
        if use_build_tables {
            db.drop_build_rebuild_secondary_indexes(output_write_options)
                .await?;
        } else {
            db.drop_rebuild_secondary_indexes().await?;
        }
        db.record_engine_phase_timing(
            &run_id,
            None,
            "drop_rebuild_indexes",
            None,
            phase_started.elapsed(),
            Some(if use_build_tables {
                "secondary lookup indexes dropped on build tables only; primary keys kept"
            } else {
                "secondary lookup indexes only; primary keys kept"
            }),
        )
        .await?;
        println!("Dropped secondary rebuild indexes before fast inserts");
    }

    let phase_started = Instant::now();
    let mut symbols = match candle_source {
        CandleSource::EquityCandles => {
            db.get_symbols_above_average_volume(minimum_average_volume)
                .await?
        }
        CandleSource::FuturesContracts => {
            db.get_futures_contract_symbols(futures_root.as_deref())
                .await?
        }
    };

    if symbol_offset > 0 {
        if symbol_offset >= symbols.len() {
            symbols.clear();
        } else {
            symbols.drain(0..symbol_offset);
        }
    }

    if symbol_limit > 0 && symbols.len() > symbol_limit {
        symbols.truncate(symbol_limit);
    }
    db.record_engine_phase_timing(
        &run_id,
        None,
        "select_symbols",
        Some(symbols.len() as i64),
        phase_started.elapsed(),
        None,
    )
    .await?;

    match candle_source {
        CandleSource::EquityCandles => println!(
            "Running xabcd scan on {} symbols with recent average volume >= {}",
            symbols.len(),
            minimum_average_volume
        ),
        CandleSource::FuturesContracts => println!(
            "Running xabcd scan on {} futures contract symbols (root {}, filter none)",
            symbols.len(),
            futures_root.as_deref().unwrap_or("all")
        ),
    }
    println!("Using candle source {}", candle_source.label());
    println!(
        "Using scan concurrency {}, write batch size {}, symbol limit {}",
        scan_concurrency,
        write_batch_size,
        if symbol_limit > 0 {
            symbol_limit
        } else {
            symbols.len()
        }
    );
    println!("Using symbol offset {}", symbol_offset);
    println!(
        "Using max X bars left {}",
        max_x_bars_left
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unlimited".to_string())
    );
    println!(
        "Using fast rebuild {}, build tables {}, pattern setups {}, harmonic scores {}, prop outcomes {}, target-ready outcomes only {}",
        fast_rebuild,
        use_build_tables,
        write_pattern_setups,
        write_harmonic_scores,
        write_prop_outcomes,
        target_ready_outcomes_only
    );

    let total_symbols = symbols.len();
    let mut completed_symbols = 0usize;
    let mut failed_symbols = 0usize;
    let mut total_setups_written = 0usize;
    let mut total_prop_reversal_rows = 0usize;
    let mut total_bearish_patterns = 0usize;
    let mut total_bullish_patterns = 0usize;
    let mut pending_patterns: Vec<PatternXABCD> = Vec::new();
    let mut pending_prop_reversal_outcomes: Vec<PropReversalOutcome> = Vec::new();

    let mut tasks: JoinSet<Result<SymbolScanResult, String>> = JoinSet::new();
    let mut symbol_iter = symbols.into_iter();

    let phase_started = Instant::now();
    let mut queued_symbols = 0i64;
    for _ in 0..scan_concurrency {
        if let Some(symbol) = symbol_iter.next() {
            spawn_symbol_scan(
                &mut tasks,
                pool.clone(),
                symbol,
                candle_source,
                max_x_bars_left,
            );
            queued_symbols += 1;
        }
    }
    db.record_engine_phase_timing(
        &run_id,
        None,
        "queue_initial_symbols",
        Some(queued_symbols),
        phase_started.elapsed(),
        None,
    )
    .await?;

    while !tasks.is_empty() {
        let phase_started = Instant::now();
        let task_result = match tasks.join_next().await {
            Some(task_result) => task_result,
            None => break,
        };
        let await_symbol_duration = phase_started.elapsed();

        completed_symbols += 1;

        let result = match task_result {
            Ok(Ok(result)) => result,
            Ok(Err(message)) => {
                failed_symbols += 1;
                eprintln!(
                    "Skipping symbol after worker error ({}/{}): {}",
                    completed_symbols, total_symbols, message
                );
                db.record_engine_phase_timing(
                    &run_id,
                    None,
                    "await_symbol_result",
                    None,
                    await_symbol_duration,
                    Some("worker error"),
                )
                .await?;
                let phase_started = Instant::now();
                if let Some(symbol) = symbol_iter.next() {
                    spawn_symbol_scan(
                        &mut tasks,
                        pool.clone(),
                        symbol,
                        candle_source,
                        max_x_bars_left,
                    );
                    db.record_engine_phase_timing(
                        &run_id,
                        None,
                        "queue_next_symbol",
                        Some(1),
                        phase_started.elapsed(),
                        None,
                    )
                    .await?;
                }
                continue;
            }
            Err(error) => {
                failed_symbols += 1;
                eprintln!(
                    "Worker join failed ({}/{}): {}",
                    completed_symbols, total_symbols, error
                );
                db.record_engine_phase_timing(
                    &run_id,
                    None,
                    "await_symbol_result",
                    None,
                    await_symbol_duration,
                    Some("worker join error"),
                )
                .await?;
                let phase_started = Instant::now();
                if let Some(symbol) = symbol_iter.next() {
                    spawn_symbol_scan(
                        &mut tasks,
                        pool.clone(),
                        symbol,
                        candle_source,
                        max_x_bars_left,
                    );
                    db.record_engine_phase_timing(
                        &run_id,
                        None,
                        "queue_next_symbol",
                        Some(1),
                        phase_started.elapsed(),
                        None,
                    )
                    .await?;
                }
                continue;
            }
        };

        let symbol_name = result.symbol.clone();
        db.record_engine_phase_timing(
            &run_id,
            Some(&symbol_name),
            "await_symbol_result",
            Some(result.patterns.len() as i64),
            await_symbol_duration,
            Some("wall-clock wait for candle load and scanner worker"),
        )
        .await?;

        let phase_started = Instant::now();
        let mut patterns = result.patterns;
        let symbol_pattern_count = patterns.len() as i64;
        let mut prop_reversal_outcomes = result.prop_reversal_outcomes;
        total_setups_written += patterns.len();
        total_prop_reversal_rows += prop_reversal_outcomes.len();
        total_bearish_patterns += result.bearish_count;
        total_bullish_patterns += result.bullish_count;

        println!(
            "Symbol: {}, setup total: {}, Bear: {}, Bull: {} ({}/{})",
            symbol_name,
            patterns.len(),
            result.bearish_count,
            result.bullish_count,
            completed_symbols,
            total_symbols
        );

        pending_patterns.append(&mut patterns);
        pending_prop_reversal_outcomes.append(&mut prop_reversal_outcomes);
        db.record_engine_phase_timing(
            &run_id,
            Some(&symbol_name),
            "process_symbol_result",
            Some(symbol_pattern_count),
            phase_started.elapsed(),
            None,
        )
        .await?;

        let phase_started = Instant::now();
        if let Some(symbol) = symbol_iter.next() {
            spawn_symbol_scan(
                &mut tasks,
                pool.clone(),
                symbol,
                candle_source,
                max_x_bars_left,
            );
            db.record_engine_phase_timing(
                &run_id,
                None,
                "queue_next_symbol",
                Some(1),
                phase_started.elapsed(),
                None,
            )
            .await?;
        }

        if pending_patterns.len() >= write_batch_size {
            let flush_size = pending_patterns.len();
            flush_pending_patterns(
                &db,
                &run_id,
                &mut pending_patterns,
                &mut pending_prop_reversal_outcomes,
                fast_rebuild,
                use_build_tables,
                write_pattern_setups,
                write_harmonic_scores,
                target_ready_outcomes_only,
            )
            .await?;
            println!("Flushed {} pattern setups to DB", flush_size);
        }
    }

    flush_pending_patterns(
        &db,
        &run_id,
        &mut pending_patterns,
        &mut pending_prop_reversal_outcomes,
        fast_rebuild,
        use_build_tables,
        write_pattern_setups,
        write_harmonic_scores,
        target_ready_outcomes_only,
    )
    .await?;

    if fast_rebuild {
        let phase_started = Instant::now();
        if use_build_tables {
            db.recreate_build_rebuild_secondary_indexes(output_write_options)
                .await?;
        } else {
            db.recreate_rebuild_secondary_indexes().await?;
        }
        db.record_engine_phase_timing(
            &run_id,
            None,
            "recreate_rebuild_indexes",
            None,
            phase_started.elapsed(),
            Some(if use_build_tables {
                "secondary lookup indexes restored on build tables before swap"
            } else {
                "secondary lookup indexes restored after fast inserts"
            }),
        )
        .await?;
        println!("Recreated secondary rebuild indexes after fast inserts");
    }

    if use_build_tables {
        let phase_started = Instant::now();
        db.swap_build_output_tables(output_write_options).await?;
        db.record_engine_phase_timing(
            &run_id,
            None,
            "swap_build_tables",
            None,
            phase_started.elapsed(),
            Some("renamed indexed build tables into final output table names"),
        )
        .await?;
        println!("Swapped build tables into final output table names");
    }

    if refresh_prop_family_summaries {
        println!("Refreshing prop strategy family summaries");
        db.refresh_prop_strategy_family_rollups(Some(&run_id))
            .await?;
        println!("Refreshing prop contract week summary");
        db.refresh_prop_contract_week_summary(Some(&run_id)).await?;
        println!("Refreshing prop family weekly cadence");
        db.refresh_prop_family_weekly_cadence(Some(&run_id)).await?;
        println!("Marked prop strategy family summaries ready");
    } else {
        println!("Skipped prop strategy family summaries refresh");
    }

    println!(
        "Pattern setups total: {}, Prop reversal rows: {}, Bear: {}, Bull: {}, Failed symbols: {}",
        total_setups_written,
        total_prop_reversal_rows,
        total_bearish_patterns,
        total_bullish_patterns,
        failed_symbols
    );

    let duration = start.elapsed();
    println!("Execution time: {:?}", duration);

    Ok(())
}
