use std::cmp;
use std::collections::BTreeMap;
use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use abcd::models::accuracy::Accuracies;
use abcd::models::database::{Database, EngineCsvOutputWriter, OutputWriteOptions};
use abcd::models::*;
use sqlx::mysql::MySqlPool;
use tokio::task::JoinSet;

const PRICE_KEY_SCALE: f64 = 100_000_000.0;
type TradePriceIndex = BTreeMap<i64, Vec<usize>>;

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
    load_duration: Duration,
    scan_duration: Duration,
    total_duration: Duration,
    phase_timings: ScanPhaseTimings,
    patterns: Vec<PatternXABCD>,
    prop_reversal_outcomes: Vec<PropReversalOutcome>,
    bearish_count: usize,
    bullish_count: usize,
}

struct CandleRangeExtrema {
    log2: Vec<usize>,
    low_table: Vec<Vec<f64>>,
    high_table: Vec<Vec<f64>>,
}

struct XBarsLeftLookup {
    previous_higher_high: Vec<Option<usize>>,
    previous_lower_low: Vec<Option<usize>>,
}

impl XBarsLeftLookup {
    fn new(candles: &[Candle]) -> Self {
        let mut previous_higher_high = vec![None; candles.len()];
        let mut previous_lower_low = vec![None; candles.len()];
        let mut high_stack: Vec<usize> = Vec::new();
        let mut low_stack: Vec<usize> = Vec::new();

        for (index, candle) in candles.iter().enumerate() {
            let high_threshold = truncate_to_2_decimals(candle.high);
            while let Some(previous_index) = high_stack.last().copied() {
                if candles[previous_index].high <= high_threshold {
                    high_stack.pop();
                } else {
                    break;
                }
            }
            previous_higher_high[index] = high_stack.last().copied();
            high_stack.push(index);

            let low_threshold = truncate_to_2_decimals(candle.low);
            while let Some(previous_index) = low_stack.last().copied() {
                if candles[previous_index].low >= low_threshold {
                    low_stack.pop();
                } else {
                    break;
                }
            }
            previous_lower_low[index] = low_stack.last().copied();
            low_stack.push(index);
        }

        Self {
            previous_higher_high,
            previous_lower_low,
        }
    }

    fn bars_left(&self, x_index: usize, x: &Pivot) -> i64 {
        if x_index >= self.previous_higher_high.len() {
            return 0;
        }

        let previous_break = match x.type_ {
            PivotType::High => self.previous_higher_high[x_index],
            PivotType::Low => self.previous_lower_low[x_index],
        };

        previous_break
            .map(|previous_index| x_index.saturating_sub(previous_index + 1))
            .unwrap_or(x_index) as i64
    }
}

impl CandleRangeExtrema {
    fn new(candles: &[Candle]) -> Self {
        let candle_count = candles.len();
        let mut log2 = vec![0usize; candle_count + 1];
        for index in 2..=candle_count {
            log2[index] = log2[index / 2] + 1;
        }

        let level_count = log2[candle_count] + 1;
        let mut low_table = Vec::with_capacity(level_count);
        let mut high_table = Vec::with_capacity(level_count);
        low_table.push(candles.iter().map(|candle| candle.low).collect::<Vec<_>>());
        high_table.push(candles.iter().map(|candle| candle.high).collect::<Vec<_>>());

        for level in 1..level_count {
            let span = 1usize << level;
            let half_span = span >> 1;
            let mut low_values = vec![0.0; candle_count];
            let mut high_values = vec![0.0; candle_count];
            for index in 0..=candle_count.saturating_sub(span) {
                low_values[index] =
                    low_table[level - 1][index].min(low_table[level - 1][index + half_span]);
                high_values[index] =
                    high_table[level - 1][index].max(high_table[level - 1][index + half_span]);
            }
            low_table.push(low_values);
            high_table.push(high_values);
        }

        Self {
            log2,
            low_table,
            high_table,
        }
    }

    fn query(&self, start_index: usize, end_index: usize) -> (f64, f64) {
        let length = end_index.saturating_sub(start_index) + 1;
        let level = self.log2[length];
        let span = 1usize << level;
        let right_index = end_index + 1 - span;
        let lowest_price =
            self.low_table[level][start_index].min(self.low_table[level][right_index]);
        let highest_price =
            self.high_table[level][start_index].max(self.high_table[level][right_index]);
        (lowest_price, highest_price)
    }
}

#[derive(Default)]
struct OpenTradeIndexes {
    take_profit_on_high: TradePriceIndex,
    take_profit_on_low: TradePriceIndex,
    stop_loss_on_high: TradePriceIndex,
    stop_loss_on_low: TradePriceIndex,
}

#[derive(Default)]
struct XabcDIndexes {
    d_high_trigger: TradePriceIndex,
    d_low_trigger: TradePriceIndex,
    invalidate_on_high_break: TradePriceIndex,
    invalidate_on_low_break: TradePriceIndex,
}

#[derive(Clone, Copy)]
enum TradeCloseKind {
    Profit,
    Stop,
}

#[derive(Clone, Copy, Default)]
struct ScanPhaseTimings {
    support_resistance: Duration,
    x_to_a: Duration,
    xa_to_b: Duration,
    xab_to_c: Duration,
    xabc_to_d: Duration,
    open_trade: Duration,
    retain_candidates: Duration,
    final_open_trades: Duration,
    accuracy_build: Duration,
    x_bars_left: Duration,
    max_x_filter: Duration,
    default_fit_filter: Duration,
    market_counts: Duration,
    prop_reversal_outcomes: Duration,
    x_to_a_checks: u64,
    xa_to_b_checks: u64,
    xab_to_c_checks: u64,
    xabc_to_d_checks: u64,
    open_trade_checks: u64,
}

impl ScanPhaseTimings {
    fn add_assign(&mut self, other: Self) {
        self.support_resistance += other.support_resistance;
        self.x_to_a += other.x_to_a;
        self.xa_to_b += other.xa_to_b;
        self.xab_to_c += other.xab_to_c;
        self.xabc_to_d += other.xabc_to_d;
        self.open_trade += other.open_trade;
        self.retain_candidates += other.retain_candidates;
        self.final_open_trades += other.final_open_trades;
        self.accuracy_build += other.accuracy_build;
        self.x_bars_left += other.x_bars_left;
        self.max_x_filter += other.max_x_filter;
        self.default_fit_filter += other.default_fit_filter;
        self.market_counts += other.market_counts;
        self.prop_reversal_outcomes += other.prop_reversal_outcomes;
        self.x_to_a_checks += other.x_to_a_checks;
        self.xa_to_b_checks += other.xa_to_b_checks;
        self.xab_to_c_checks += other.xab_to_c_checks;
        self.xabc_to_d_checks += other.xabc_to_d_checks;
        self.open_trade_checks += other.open_trade_checks;
    }

    fn pre_pattern_total(self) -> Duration {
        self.x_to_a + self.xa_to_b + self.xab_to_c + self.xabc_to_d
    }

    fn post_scan_total(self) -> Duration {
        self.final_open_trades
            + self.accuracy_build
            + self.x_bars_left
            + self.max_x_filter
            + self.default_fit_filter
            + self.market_counts
            + self.prop_reversal_outcomes
    }
}

fn price_key(price: f64) -> i64 {
    (price * PRICE_KEY_SCALE).round() as i64
}

fn push_trade_index(index: &mut TradePriceIndex, key: i64, trade_id: usize) {
    index.entry(key).or_default().push(trade_id);
}

fn remove_trade_index(index: &mut TradePriceIndex, key: i64, trade_id: usize) {
    let Some(ids) = index.get_mut(&key) else {
        return;
    };
    if let Some(position) = ids.iter().position(|id| *id == trade_id) {
        ids.swap_remove(position);
    }
    if ids.is_empty() {
        index.remove(&key);
    }
}

fn index_open_trade(trade_id: usize, pattern: &PatternXABCD, indexes: &mut OpenTradeIndexes) {
    let take_profit_key = price_key(pattern.trade.reward_exit_price);
    let stop_loss_key = price_key(pattern.trade.risk_exit_price);
    match pattern.market {
        Market::Bullish => {
            push_trade_index(&mut indexes.take_profit_on_high, take_profit_key, trade_id);
            push_trade_index(&mut indexes.stop_loss_on_low, stop_loss_key, trade_id);
        }
        Market::Bearish => {
            push_trade_index(&mut indexes.take_profit_on_low, take_profit_key, trade_id);
            push_trade_index(&mut indexes.stop_loss_on_high, stop_loss_key, trade_id);
        }
    }
}

fn unindex_open_trade(trade_id: usize, pattern: &PatternXABCD, indexes: &mut OpenTradeIndexes) {
    let take_profit_key = price_key(pattern.trade.reward_exit_price);
    let stop_loss_key = price_key(pattern.trade.risk_exit_price);
    match pattern.market {
        Market::Bullish => {
            remove_trade_index(&mut indexes.take_profit_on_high, take_profit_key, trade_id);
            remove_trade_index(&mut indexes.stop_loss_on_low, stop_loss_key, trade_id);
        }
        Market::Bearish => {
            remove_trade_index(&mut indexes.take_profit_on_low, take_profit_key, trade_id);
            remove_trade_index(&mut indexes.stop_loss_on_high, stop_loss_key, trade_id);
        }
    }
}

fn take_high_triggered_trade_ids(index: &mut TradePriceIndex, high: f64) -> Vec<usize> {
    let high_key = price_key(high);
    let triggered_keys = index
        .range(..=high_key)
        .map(|(key, _)| *key)
        .collect::<Vec<_>>();
    let mut trade_ids = Vec::new();
    for key in triggered_keys {
        if let Some(mut ids) = index.remove(&key) {
            trade_ids.append(&mut ids);
        }
    }
    trade_ids
}

fn take_low_triggered_trade_ids(index: &mut TradePriceIndex, low: f64) -> Vec<usize> {
    let low_key = price_key(low);
    let triggered_keys = index
        .range(low_key..)
        .map(|(key, _)| *key)
        .collect::<Vec<_>>();
    let mut trade_ids = Vec::new();
    for key in triggered_keys {
        if let Some(mut ids) = index.remove(&key) {
            trade_ids.append(&mut ids);
        }
    }
    trade_ids
}

fn take_high_broken_ids(index: &mut TradePriceIndex, high: f64) -> Vec<usize> {
    let high_key = price_key(high);
    let broken_keys = index
        .range(..high_key)
        .map(|(key, _)| *key)
        .collect::<Vec<_>>();
    let mut ids = Vec::new();
    for key in broken_keys {
        if let Some(mut key_ids) = index.remove(&key) {
            ids.append(&mut key_ids);
        }
    }
    ids
}

fn take_low_broken_ids(index: &mut TradePriceIndex, low: f64) -> Vec<usize> {
    let low_key = price_key(low);
    let broken_keys = index
        .range((
            std::ops::Bound::Excluded(low_key),
            std::ops::Bound::Unbounded,
        ))
        .map(|(key, _)| *key)
        .collect::<Vec<_>>();
    let mut ids = Vec::new();
    for key in broken_keys {
        if let Some(mut key_ids) = index.remove(&key) {
            ids.append(&mut key_ids);
        }
    }
    ids
}

fn xabc_d_high_trigger_key(pattern: &PatternXABC) -> i64 {
    price_key(pattern.c.min_max.max(pattern.b.high))
}

fn xabc_d_low_trigger_key(pattern: &PatternXABC) -> i64 {
    price_key(pattern.c.min_max.min(pattern.b.low))
}

fn index_xabc_waiting_for_d(
    candidate_id: usize,
    pattern: &PatternXABC,
    indexes: &mut XabcDIndexes,
) {
    match pattern.c.type_ {
        PivotType::Low => {
            push_trade_index(
                &mut indexes.d_high_trigger,
                xabc_d_high_trigger_key(pattern),
                candidate_id,
            );
            push_trade_index(
                &mut indexes.invalidate_on_low_break,
                price_key(pattern.c.low),
                candidate_id,
            );
        }
        PivotType::High => {
            push_trade_index(
                &mut indexes.d_low_trigger,
                xabc_d_low_trigger_key(pattern),
                candidate_id,
            );
            push_trade_index(
                &mut indexes.invalidate_on_high_break,
                price_key(pattern.c.high),
                candidate_id,
            );
        }
    }
}

fn unindex_xabc_waiting_for_d(
    candidate_id: usize,
    pattern: &PatternXABC,
    indexes: &mut XabcDIndexes,
) {
    match pattern.c.type_ {
        PivotType::Low => {
            remove_trade_index(
                &mut indexes.d_high_trigger,
                xabc_d_high_trigger_key(pattern),
                candidate_id,
            );
            remove_trade_index(
                &mut indexes.invalidate_on_low_break,
                price_key(pattern.c.low),
                candidate_id,
            );
        }
        PivotType::High => {
            remove_trade_index(
                &mut indexes.d_low_trigger,
                xabc_d_low_trigger_key(pattern),
                candidate_id,
            );
            remove_trade_index(
                &mut indexes.invalidate_on_high_break,
                price_key(pattern.c.high),
                candidate_id,
            );
        }
    }
}

fn remove_xabc_candidates(
    candidate_ids: Vec<usize>,
    active_patterns: &mut [Option<PatternXABC>],
    indexes: &mut XabcDIndexes,
    active_count: &mut usize,
) {
    for candidate_id in candidate_ids {
        let Some(slot) = active_patterns.get_mut(candidate_id) else {
            continue;
        };
        let Some(pattern) = slot.take() else {
            continue;
        };
        unindex_xabc_waiting_for_d(candidate_id, &pattern, indexes);
        *active_count = (*active_count).saturating_sub(1);
    }
}

fn update_trade_snapshot(
    pattern: &mut PatternXABCD,
    candles: &[Candle],
    extrema: &CandleRangeExtrema,
    entry_index: usize,
    current_index: usize,
) {
    let current = &candles[current_index];
    let (lowest_price, highest_price) = extrema.query(entry_index, current_index);

    pattern.d.length = current_index.saturating_sub(pattern.reversal_context.d_index) as i64;
    pattern.trade.length = pattern.d.length;
    pattern.trade.lowest_price = lowest_price;
    pattern.trade.highest_price = highest_price;

    match pattern.market {
        Market::Bullish => {
            pattern.trade.adverse_price = lowest_price;
            pattern.trade.favorable_price = highest_price;
            pattern.trade.max_adverse_points = (pattern.trade.enter_price - lowest_price).max(0.0);
            pattern.trade.max_favorable_points =
                (highest_price - pattern.trade.enter_price).max(0.0);
            pattern.trade.pnl = current.close - pattern.trade.enter_price;
        }
        Market::Bearish => {
            pattern.trade.adverse_price = highest_price;
            pattern.trade.favorable_price = lowest_price;
            pattern.trade.max_adverse_points = (highest_price - pattern.trade.enter_price).max(0.0);
            pattern.trade.max_favorable_points =
                (pattern.trade.enter_price - lowest_price).max(0.0);
            pattern.trade.pnl = pattern.trade.enter_price - current.close;
        }
    }

    pattern.trade.bars_held = current_index.saturating_sub(entry_index) as i64 + 1;
    pattern.trade.minutes_held = current
        .date
        .signed_duration_since(pattern.trade.entry_date)
        .num_minutes()
        .max(0);
    pattern.trade.date = current.date;
    pattern.trade.current_price = truncate_to_2_decimals(current.close);
}

fn trade_close_hit(pattern: &PatternXABCD, candle: &Candle, close_kind: TradeCloseKind) -> bool {
    match (pattern.market, close_kind) {
        (Market::Bullish, TradeCloseKind::Profit) => candle.high >= pattern.trade.reward_exit_price,
        (Market::Bearish, TradeCloseKind::Profit) => candle.low <= pattern.trade.reward_exit_price,
        (Market::Bullish, TradeCloseKind::Stop) => candle.low <= pattern.trade.risk_exit_price,
        (Market::Bearish, TradeCloseKind::Stop) => candle.high >= pattern.trade.risk_exit_price,
    }
}

fn close_price(pattern: &PatternXABCD, close_kind: TradeCloseKind) -> f64 {
    match close_kind {
        TradeCloseKind::Profit => pattern.trade.reward_exit_price,
        TradeCloseKind::Stop => pattern.trade.risk_exit_price,
    }
}

fn close_result(close_kind: TradeCloseKind) -> i32 {
    match close_kind {
        TradeCloseKind::Profit => 1,
        TradeCloseKind::Stop => 2,
    }
}

fn is_default_fit_bin(value: &str) -> bool {
    matches!(value, "50-60" | "60-70" | "70-80" | "80-90" | "90-100")
}

fn pattern_family_size_bucket(pattern: &PatternXABCD) -> &'static str {
    let total_bars = pattern.x.length + pattern.a.length + pattern.b.length + pattern.c.length;

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

fn is_default_fit_pattern(pattern: &PatternXABCD) -> bool {
    let lens = pattern.dominant_harmonic_lens();

    is_default_fit_bin(lens.bin)
        && is_default_fit_bin(lens.time_bin)
        && matches!(
            pattern_family_size_bucket(pattern),
            "Micro" | "Small" | "Normal"
        )
        && pattern.x_strictness() == "Strict"
}

fn process_open_trade_candidates(
    trade_ids: Vec<usize>,
    close_kind: TradeCloseKind,
    open_patterns: &mut [Option<PatternXABCD>],
    indexes: &mut OpenTradeIndexes,
    completed_patterns: &mut Vec<PatternXABCD>,
    candles: &[Candle],
    extrema: &CandleRangeExtrema,
    current_index: usize,
    open_trade_count: &mut usize,
) {
    let current = &candles[current_index];

    for trade_id in trade_ids {
        let Some(slot) = open_patterns.get_mut(trade_id) else {
            continue;
        };
        let Some(mut pattern) = slot.take() else {
            continue;
        };

        unindex_open_trade(trade_id, &pattern, indexes);

        let Some(entry_index) = pattern.entry_index else {
            completed_patterns.push(pattern);
            *open_trade_count = (*open_trade_count).saturating_sub(1);
            continue;
        };

        if current_index < entry_index || !trade_close_hit(&pattern, current, close_kind) {
            index_open_trade(trade_id, &pattern, indexes);
            *slot = Some(pattern);
            continue;
        }

        let Some(entry_candle) = candles.get(entry_index) else {
            completed_patterns.push(pattern);
            *open_trade_count = (*open_trade_count).saturating_sub(1);
            continue;
        };

        update_trade_snapshot(&mut pattern, candles, extrema, entry_index, current_index);
        pattern.trade.open = false;
        pattern.trade.current_price = close_price(&pattern, close_kind);
        pattern.trade.result = close_result(close_kind);
        pattern.target_candle = Some(TargetCandle::from_candle(current, entry_candle));
        apply_closed_trade_reversal(&mut pattern, candles, current_index);
        completed_patterns.push(pattern);
        *open_trade_count = (*open_trade_count).saturating_sub(1);
    }
}

fn push_completed_xabcd_from_xabc(
    pattern: &PatternXABC,
    symbol: &Arc<str>,
    source_context: &PatternSourceContext,
    support_level: f64,
    candles: &[Candle],
    prev1: &Candle,
    current: &Candle,
    prev1_index: usize,
    current_index: usize,
    completed_patterns: &mut Vec<PatternXABCD>,
) {
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
    let (contract_week_index, contract_days_from_start) = contract_age_fields(candles, prev1);

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

    completed_patterns.push(PatternXABCD {
        symbol: Arc::clone(symbol),
        root_symbol: source_context.root_symbol.clone(),
        contract_symbol: source_context.contract_symbol.clone(),
        source_table: Arc::clone(&source_context.source_table),
        source_timeframe: Arc::clone(&source_context.source_timeframe),
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

fn process_xabc_d_high_candidates(
    candidate_ids: Vec<usize>,
    active_patterns: &mut [Option<PatternXABC>],
    indexes: &mut XabcDIndexes,
    pattern_xabcd_holder: &mut Vec<PatternXABCD>,
    symbol: &Arc<str>,
    source_context: &PatternSourceContext,
    support_level: f64,
    candles: &[Candle],
    prev2: &Candle,
    prev1: &Candle,
    current: &Candle,
    prev1_index: usize,
    current_index: usize,
    active_count: &mut usize,
) {
    let pivot_check = check_for_pivot(current, prev1, prev2, PivotType::High);

    for candidate_id in candidate_ids {
        let Some(slot) = active_patterns.get_mut(candidate_id) else {
            continue;
        };
        let Some(mut pattern) = slot.take() else {
            continue;
        };

        unindex_xabc_waiting_for_d(candidate_id, &pattern, indexes);
        pattern.c.min_max = pattern.c.min_max.max(prev1.high);

        if prev1.high >= pattern.x.high {
            *active_count = (*active_count).saturating_sub(1);
            continue;
        }

        if pivot_check
            && prev1.high >= pattern.c.min_max
            && prev1.high > pattern.b.high
            && prev1.high < pattern.x.high
        {
            push_completed_xabcd_from_xabc(
                &pattern,
                symbol,
                source_context,
                support_level,
                candles,
                prev1,
                current,
                prev1_index,
                current_index,
                pattern_xabcd_holder,
            );
        }

        index_xabc_waiting_for_d(candidate_id, &pattern, indexes);
        *slot = Some(pattern);
    }
}

fn process_xabc_d_low_candidates(
    candidate_ids: Vec<usize>,
    active_patterns: &mut [Option<PatternXABC>],
    indexes: &mut XabcDIndexes,
    pattern_xabcd_holder: &mut Vec<PatternXABCD>,
    symbol: &Arc<str>,
    source_context: &PatternSourceContext,
    support_level: f64,
    candles: &[Candle],
    prev2: &Candle,
    prev1: &Candle,
    current: &Candle,
    prev1_index: usize,
    current_index: usize,
    active_count: &mut usize,
) {
    let pivot_check = check_for_pivot(current, prev1, prev2, PivotType::Low);

    for candidate_id in candidate_ids {
        let Some(slot) = active_patterns.get_mut(candidate_id) else {
            continue;
        };
        let Some(mut pattern) = slot.take() else {
            continue;
        };

        unindex_xabc_waiting_for_d(candidate_id, &pattern, indexes);
        pattern.c.min_max = pattern.c.min_max.min(prev1.low);

        if prev1.low <= pattern.x.low {
            *active_count = (*active_count).saturating_sub(1);
            continue;
        }

        if pivot_check
            && prev1.low <= pattern.c.min_max
            && prev1.low < pattern.b.low
            && prev1.low > pattern.x.low
        {
            push_completed_xabcd_from_xabc(
                &pattern,
                symbol,
                source_context,
                support_level,
                candles,
                prev1,
                current,
                prev1_index,
                current_index,
                pattern_xabcd_holder,
            );
        }

        index_xabc_waiting_for_d(candidate_id, &pattern, indexes);
        *slot = Some(pattern);
    }
}

#[derive(Debug, Clone, Copy)]
enum CandleSource {
    EquityCandles,
    FuturesContracts(FuturesTimeframe),
}

#[derive(Debug, Clone, Copy)]
struct FuturesTimeframe {
    label: &'static str,
    source_table: &'static str,
}

impl FuturesTimeframe {
    fn from_env() -> Self {
        let value = env::var("ABCD_FUTURES_TIMEFRAME")
            .or_else(|_| env::var("ABCD_TIMEFRAME"))
            .unwrap_or_else(|_| "1m".to_string());
        let normalized = value
            .trim()
            .to_ascii_lowercase()
            .replace([' ', '_', '-'], "")
            .replace("minutes", "m")
            .replace("minute", "m")
            .replace("mins", "m")
            .replace("min", "m")
            .replace("hours", "h")
            .replace("hour", "h")
            .replace("hrs", "h")
            .replace("hr", "h");

        match normalized.as_str() {
            "3" | "3m" => Self {
                label: "3m",
                source_table: "futures_contract_3m_candles",
            },
            "5" | "5m" => Self {
                label: "5m",
                source_table: "futures_contract_5m_candles",
            },
            "15" | "15m" => Self {
                label: "15m",
                source_table: "futures_contract_15m_candles",
            },
            "30" | "30m" => Self {
                label: "30m",
                source_table: "futures_contract_30m_candles",
            },
            "60" | "60m" | "1h" => Self {
                label: "1h",
                source_table: "futures_contract_1h_candles",
            },
            "240" | "240m" | "4h" => Self {
                label: "4h",
                source_table: "futures_contract_4h_candles",
            },
            "720" | "720m" | "12h" => Self {
                label: "12h",
                source_table: "futures_contract_12h_candles",
            },
            "1440" | "1440m" | "24h" | "1d" | "d" | "day" | "daily" => Self {
                label: "1d",
                source_table: "futures_contract_1d_candles",
            },
            _ => Self {
                label: "1m",
                source_table: "futures_contract_1m_candles",
            },
        }
    }
}

impl CandleSource {
    fn from_env() -> Self {
        match env::var("ABCD_CANDLE_SOURCE")
            .unwrap_or_else(|_| "candles".to_string())
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "futures_contracts" | "futures" | "contracts" => {
                Self::FuturesContracts(FuturesTimeframe::from_env())
            }
            _ => Self::EquityCandles,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::EquityCandles => "candles",
            Self::FuturesContracts(_) => "futures_contracts",
        }
    }

    fn source_table(self) -> &'static str {
        match self {
            Self::EquityCandles => "candles",
            Self::FuturesContracts(timeframe) => timeframe.source_table,
        }
    }

    fn source_timeframe(self) -> &'static str {
        match self {
            Self::EquityCandles => "daily",
            Self::FuturesContracts(timeframe) => timeframe.label,
        }
    }
}

#[derive(Clone)]
struct PatternSourceContext {
    root_symbol: Option<Arc<str>>,
    contract_symbol: Option<Arc<str>>,
    source_table: Arc<str>,
    source_timeframe: Arc<str>,
}

fn futures_root_symbol(symbol: &str) -> String {
    let uppercase = symbol.trim().to_uppercase();
    let clean_symbol = uppercase
        .split(['.', ' ', '_'])
        .next()
        .unwrap_or(uppercase.as_str());
    let known_roots = [
        "M6A", "M6B", "M6C", "M6E", "M6J", "M6S", "M6N", "MES", "MNQ", "MYM", "M2K", "MGC", "MCL",
        "MBT", "MET", "RTY", "EMD", "NKD", "6A", "6B", "6C", "6E", "6J", "6S", "6N", "ES", "NQ",
        "YM", "CL", "QM", "NG", "QG", "HO", "RB", "GC", "SI", "HG", "PL", "PA", "QI", "QO", "ZC",
        "ZW", "ZS", "ZM", "ZL", "HE", "LE", "GF",
    ];

    if let Some(root) = known_roots
        .iter()
        .find(|root| clean_symbol.starts_with(**root))
    {
        return (*root).to_string();
    }

    let root = clean_symbol
        .chars()
        .take_while(|ch| ch.is_ascii_alphabetic() || ch.is_ascii_digit())
        .collect::<String>();

    if root.is_empty() {
        uppercase
    } else {
        root
    }
}

fn pattern_source_context(symbol: &str, candle_source: CandleSource) -> PatternSourceContext {
    match candle_source {
        CandleSource::EquityCandles => PatternSourceContext {
            root_symbol: None,
            contract_symbol: None,
            source_table: Arc::from(candle_source.source_table()),
            source_timeframe: Arc::from(candle_source.source_timeframe()),
        },
        CandleSource::FuturesContracts(_) => PatternSourceContext {
            root_symbol: Some(Arc::from(futures_root_symbol(symbol))),
            contract_symbol: Some(Arc::from(symbol.to_string())),
            source_table: Arc::from(candle_source.source_table()),
            source_timeframe: Arc::from(candle_source.source_timeframe()),
        },
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
    source_context: PatternSourceContext,
    max_x_bars_left: Option<i64>,
    default_fit_only: bool,
    progress_every: usize,
    phase_timings_enabled: bool,
) -> SymbolScanResult {
    let symbol = Arc::<str>::from(symbol);

    if candles.len() < 3 {
        return SymbolScanResult {
            symbol: symbol.to_string(),
            candle_count: candles.len(),
            load_duration: Duration::ZERO,
            scan_duration: Duration::ZERO,
            total_duration: Duration::ZERO,
            phase_timings: ScanPhaseTimings::default(),
            patterns: Vec::new(),
            prop_reversal_outcomes: Vec::new(),
            bearish_count: 0,
            bullish_count: 0,
        };
    }

    let mut phase_timings = ScanPhaseTimings::default();

    let phase_started = phase_timings_enabled.then(Instant::now);
    let sr = SrLine::new(0.0, 0.0);
    let support_level = sr
        .create_support_resistance(&candles)
        .first()
        .map(|line| line.price)
        .unwrap_or(0.0);
    if let Some(phase_started) = phase_started {
        phase_timings.support_resistance += phase_started.elapsed();
    }

    let candle_extrema = CandleRangeExtrema::new(&candles);

    let mut pattern_x: Vec<PatternX> = Vec::new();
    let mut pattern_xa: Vec<PatternXA> = Vec::new();
    let mut pattern_xab: Vec<PatternXAB> = Vec::new();
    let mut pattern_xabc: Vec<Option<PatternXABC>> = Vec::new();
    let mut pattern_xabc_holder: Vec<PatternXABC> = Vec::new();
    let mut xabc_d_indexes = XabcDIndexes::default();
    let mut xabc_active_count = 0usize;
    let mut open_pattern_xabcd: Vec<Option<PatternXABCD>> = Vec::new();
    let mut open_trade_indexes = OpenTradeIndexes::default();
    let mut open_trade_count = 0usize;
    let mut completed_pattern_xabcd: Vec<PatternXABCD> = Vec::new();
    let mut pattern_xabcd_holder: Vec<PatternXABCD> = Vec::new();
    let candle_count = candles.len();

    for (window_idx, window) in candles.windows(3).enumerate() {
        let prev2 = &window[0];
        let prev1 = &window[1];
        let current = &window[2];
        let prev1_index = window_idx + 1;
        let current_index = window_idx + 2;
        let current_position = current_index + 1;

        if progress_every > 0
            && (current_position % progress_every == 0 || current_position == candle_count)
        {
            let percent_complete = (current_position as f64 / candle_count as f64) * 100.0;
            println!(
                "[scan] {} candle {}/{} ({:.1}%) date {} open trades {}",
                symbol.as_ref(),
                current_position,
                candle_count,
                percent_complete,
                current.date,
                open_trade_count
            );
        }

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

        let phase_started = phase_timings_enabled.then(Instant::now);
        if phase_timings_enabled {
            phase_timings.x_to_a_checks += pattern_x.len() as u64;
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
        if let Some(phase_started) = phase_started {
            phase_timings.x_to_a += phase_started.elapsed();
        }

        let phase_started = phase_timings_enabled.then(Instant::now);
        if phase_timings_enabled {
            phase_timings.xa_to_b_checks += pattern_xa.len() as u64;
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
        if let Some(phase_started) = phase_started {
            phase_timings.xa_to_b += phase_started.elapsed();
        }

        let phase_started = phase_timings_enabled.then(Instant::now);
        if phase_timings_enabled {
            phase_timings.xab_to_c_checks += pattern_xab.len() as u64;
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
        if let Some(phase_started) = phase_started {
            phase_timings.xab_to_c += phase_started.elapsed();
        }

        let phase_started = phase_timings_enabled.then(Instant::now);
        let d_high_candidate_ids =
            take_high_triggered_trade_ids(&mut xabc_d_indexes.d_high_trigger, prev1.high);
        let d_low_candidate_ids =
            take_low_triggered_trade_ids(&mut xabc_d_indexes.d_low_trigger, prev1.low);
        if phase_timings_enabled {
            phase_timings.xabc_to_d_checks +=
                (d_high_candidate_ids.len() + d_low_candidate_ids.len()) as u64;
        }
        process_xabc_d_high_candidates(
            d_high_candidate_ids,
            &mut pattern_xabc,
            &mut xabc_d_indexes,
            &mut pattern_xabcd_holder,
            &symbol,
            &source_context,
            support_level,
            &candles,
            prev2,
            prev1,
            current,
            prev1_index,
            current_index,
            &mut xabc_active_count,
        );
        process_xabc_d_low_candidates(
            d_low_candidate_ids,
            &mut pattern_xabc,
            &mut xabc_d_indexes,
            &mut pattern_xabcd_holder,
            &symbol,
            &source_context,
            support_level,
            &candles,
            prev2,
            prev1,
            current,
            prev1_index,
            current_index,
            &mut xabc_active_count,
        );
        if let Some(phase_started) = phase_started {
            phase_timings.xabc_to_d += phase_started.elapsed();
        }

        let phase_started = phase_timings_enabled.then(Instant::now);
        let mut profit_trade_ids = take_high_triggered_trade_ids(
            &mut open_trade_indexes.take_profit_on_high,
            current.high,
        );
        profit_trade_ids.extend(take_low_triggered_trade_ids(
            &mut open_trade_indexes.take_profit_on_low,
            current.low,
        ));
        if phase_timings_enabled {
            phase_timings.open_trade_checks += profit_trade_ids.len() as u64;
        }
        process_open_trade_candidates(
            profit_trade_ids,
            TradeCloseKind::Profit,
            &mut open_pattern_xabcd,
            &mut open_trade_indexes,
            &mut completed_pattern_xabcd,
            &candles,
            &candle_extrema,
            current_index,
            &mut open_trade_count,
        );

        let mut stop_trade_ids =
            take_high_triggered_trade_ids(&mut open_trade_indexes.stop_loss_on_high, current.high);
        stop_trade_ids.extend(take_low_triggered_trade_ids(
            &mut open_trade_indexes.stop_loss_on_low,
            current.low,
        ));
        if phase_timings_enabled {
            phase_timings.open_trade_checks += stop_trade_ids.len() as u64;
        }
        process_open_trade_candidates(
            stop_trade_ids,
            TradeCloseKind::Stop,
            &mut open_pattern_xabcd,
            &mut open_trade_indexes,
            &mut completed_pattern_xabcd,
            &candles,
            &candle_extrema,
            current_index,
            &mut open_trade_count,
        );
        if let Some(phase_started) = phase_started {
            phase_timings.open_trade += phase_started.elapsed();
        }

        let phase_started = phase_timings_enabled.then(Instant::now);
        for pattern in pattern_xabcd_holder.drain(..) {
            if pattern.trade.open && pattern.entry_index.is_some() {
                let trade_id = open_pattern_xabcd.len();
                index_open_trade(trade_id, &pattern, &mut open_trade_indexes);
                open_pattern_xabcd.push(Some(pattern));
                open_trade_count += 1;
            } else {
                completed_pattern_xabcd.push(pattern);
            }
        }
        for pattern in pattern_xabc_holder.drain(..) {
            let candidate_id = pattern_xabc.len();
            index_xabc_waiting_for_d(candidate_id, &pattern, &mut xabc_d_indexes);
            pattern_xabc.push(Some(pattern));
            xabc_active_count += 1;
        }
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
        let mut invalid_xabc_ids =
            take_low_broken_ids(&mut xabc_d_indexes.invalidate_on_low_break, current.low);
        invalid_xabc_ids.extend(take_high_broken_ids(
            &mut xabc_d_indexes.invalidate_on_high_break,
            current.high,
        ));
        remove_xabc_candidates(
            invalid_xabc_ids,
            &mut pattern_xabc,
            &mut xabc_d_indexes,
            &mut xabc_active_count,
        );
        if let Some(phase_started) = phase_started {
            phase_timings.retain_candidates += phase_started.elapsed();
        }
    }

    if progress_every > 0 {
        println!("[scan] {} finalizing results", symbol.as_ref());
    }

    let phase_started = phase_timings_enabled.then(Instant::now);
    let final_index = candle_count.saturating_sub(1);
    for pattern_slot in open_pattern_xabcd {
        let Some(mut pattern) = pattern_slot else {
            continue;
        };
        if let Some(entry_index) = pattern.entry_index {
            if final_index >= entry_index {
                update_trade_snapshot(
                    &mut pattern,
                    &candles,
                    &candle_extrema,
                    entry_index,
                    final_index,
                );
            }
        }
        completed_pattern_xabcd.push(pattern);
    }
    if let Some(phase_started) = phase_started {
        phase_timings.final_open_trades += phase_started.elapsed();
    }

    let phase_started = phase_timings_enabled.then(Instant::now);
    let mut patterns = Accuracies::new().get_accuracy(completed_pattern_xabcd);
    if let Some(phase_started) = phase_started {
        phase_timings.accuracy_build += phase_started.elapsed();
    }

    let phase_started = phase_timings_enabled.then(Instant::now);
    let x_bars_left_lookup = XBarsLeftLookup::new(&candles);
    for pattern in &mut patterns {
        pattern.x_bars_left = x_bars_left_lookup.bars_left(pattern.x_index, &pattern.x);
    }
    if let Some(phase_started) = phase_started {
        phase_timings.x_bars_left += phase_started.elapsed();
    }

    let phase_started = phase_timings_enabled.then(Instant::now);
    if let Some(max_x_bars_left) = max_x_bars_left {
        patterns.retain(|pattern| pattern.x_bars_left <= max_x_bars_left);
    }
    if let Some(phase_started) = phase_started {
        phase_timings.max_x_filter += phase_started.elapsed();
    }

    let phase_started = phase_timings_enabled.then(Instant::now);
    if default_fit_only {
        let before_count = patterns.len();
        patterns.retain(is_default_fit_pattern);
        println!(
            "[scan] {} default fit kept {}/{} patterns",
            symbol.as_ref(),
            patterns.len(),
            before_count
        );
    }
    if let Some(phase_started) = phase_started {
        phase_timings.default_fit_filter += phase_started.elapsed();
    }

    let phase_started = phase_timings_enabled.then(Instant::now);
    let bearish_count = patterns
        .iter()
        .filter(|p| p.market == Market::Bearish)
        .count();
    let bullish_count = patterns
        .iter()
        .filter(|p| p.market == Market::Bullish)
        .count();
    if let Some(phase_started) = phase_started {
        phase_timings.market_counts += phase_started.elapsed();
    }

    let phase_started = phase_timings_enabled.then(Instant::now);
    let prop_reversal_outcomes = build_prop_reversal_outcomes(&patterns, &candles);
    if let Some(phase_started) = phase_started {
        phase_timings.prop_reversal_outcomes += phase_started.elapsed();
    }

    SymbolScanResult {
        symbol: symbol.to_string(),
        candle_count: candles.len(),
        load_duration: Duration::ZERO,
        scan_duration: Duration::ZERO,
        total_duration: Duration::ZERO,
        phase_timings,
        patterns,
        prop_reversal_outcomes,
        bearish_count,
        bullish_count,
    }
}

fn format_duration(duration: Duration) -> String {
    format!("{:.2}s", duration.as_secs_f64())
}

fn format_phase_duration(duration: Duration, scan_duration: Duration) -> String {
    let scan_seconds = scan_duration.as_secs_f64();
    let percent = if scan_seconds > 0.0 {
        (duration.as_secs_f64() / scan_seconds) * 100.0
    } else {
        0.0
    };
    format!("{} ({:.1}%)", format_duration(duration), percent)
}

fn format_count(value: u64) -> String {
    let text = value.to_string();
    let mut output = String::with_capacity(text.len() + text.len() / 3);
    for (index, ch) in text.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            output.push(',');
        }
        output.push(ch);
    }
    output.chars().rev().collect()
}

fn format_phase_metric(
    label: &str,
    duration: Duration,
    checks: u64,
    scan_duration: Duration,
) -> String {
    format!(
        "{} {} / {} checks",
        label,
        format_phase_duration(duration, scan_duration),
        format_count(checks)
    )
}

fn print_phase_timing_lines(label: &str, timings: ScanPhaseTimings, scan_duration: Duration) {
    println!(
        "[phase timing] {} support/resistance {}",
        label,
        format_phase_duration(timings.support_resistance, scan_duration)
    );
    println!(
        "[phase timing] {} pre-pattern total: {}",
        label,
        format_phase_duration(timings.pre_pattern_total(), scan_duration)
    );
    println!(
        "[phase timing] {} {}",
        label,
        format_phase_metric("X->A", timings.x_to_a, timings.x_to_a_checks, scan_duration)
    );
    println!(
        "[phase timing] {} {}",
        label,
        format_phase_metric(
            "XA->B",
            timings.xa_to_b,
            timings.xa_to_b_checks,
            scan_duration
        )
    );
    println!(
        "[phase timing] {} {}",
        label,
        format_phase_metric(
            "XAB->C",
            timings.xab_to_c,
            timings.xab_to_c_checks,
            scan_duration
        )
    );
    println!(
        "[phase timing] {} {}",
        label,
        format_phase_metric(
            "XABC->D",
            timings.xabc_to_d,
            timings.xabc_to_d_checks,
            scan_duration
        )
    );
    println!(
        "[phase timing] {} open trades {} / {} checks",
        label,
        format_phase_duration(timings.open_trade, scan_duration),
        format_count(timings.open_trade_checks)
    );
    println!(
        "[phase timing] {} retain candidates {}",
        label,
        format_phase_duration(timings.retain_candidates, scan_duration)
    );
    println!(
        "[phase timing] {} post-scan total: {}",
        label,
        format_phase_duration(timings.post_scan_total(), scan_duration)
    );
    println!(
        "[phase timing] {} final open trade snapshots {}",
        label,
        format_phase_duration(timings.final_open_trades, scan_duration)
    );
    println!(
        "[phase timing] {} accuracy build {}",
        label,
        format_phase_duration(timings.accuracy_build, scan_duration)
    );
    println!(
        "[phase timing] {} x bars left {}",
        label,
        format_phase_duration(timings.x_bars_left, scan_duration)
    );
    println!(
        "[phase timing] {} max x filter {}",
        label,
        format_phase_duration(timings.max_x_filter, scan_duration)
    );
    println!(
        "[phase timing] {} default fit filter {}",
        label,
        format_phase_duration(timings.default_fit_filter, scan_duration)
    );
    println!(
        "[phase timing] {} market counts {}",
        label,
        format_phase_duration(timings.market_counts, scan_duration)
    );
    println!(
        "[phase timing] {} prop reversal outcomes {}",
        label,
        format_phase_duration(timings.prop_reversal_outcomes, scan_duration)
    );
}

fn spawn_symbol_scan(
    tasks: &mut JoinSet<Result<SymbolScanResult, String>>,
    pool: MySqlPool,
    symbol: String,
    candle_source: CandleSource,
    max_x_bars_left: Option<i64>,
    default_fit_only: bool,
    progress_every: usize,
    phase_timings_enabled: bool,
) {
    tasks.spawn(async move {
        let total_started = Instant::now();
        let db = Database { pool };
        println!(
            "[scan] {} loading candles from {}",
            symbol,
            candle_source.source_table()
        );
        let load_started = Instant::now();
        let candles = match candle_source {
            CandleSource::EquityCandles => db.get_stored_candles(&symbol).await,
            CandleSource::FuturesContracts(_) => {
                db.get_stored_futures_contract_candles(&symbol, candle_source.source_table())
                    .await
            }
        }
        .map_err(|error| format!("{}: failed to load candles: {}", symbol, error))?;
        let load_duration = load_started.elapsed();
        let candle_count = candles.len();
        let first_date = candles
            .first()
            .map(|candle| candle.date.to_string())
            .unwrap_or_else(|| "empty".to_string());
        let last_date = candles
            .last()
            .map(|candle| candle.date.to_string())
            .unwrap_or_else(|| "empty".to_string());
        println!(
            "[scan] {} loaded {} candles from {} to {}",
            symbol, candle_count, first_date, last_date
        );

        let symbol_for_error = symbol.clone();
        let source_context = pattern_source_context(&symbol, candle_source);
        let scan_started = Instant::now();
        let mut result = tokio::task::spawn_blocking(move || {
            scan_symbol(
                symbol,
                candles,
                source_context,
                max_x_bars_left,
                default_fit_only,
                progress_every,
                phase_timings_enabled,
            )
        })
        .await
        .map_err(|error| format!("{}: scanner worker failed: {}", symbol_for_error, error))?;
        result.load_duration = load_duration;
        result.scan_duration = scan_started.elapsed();
        result.total_duration = total_started.elapsed();
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
    replace_existing_prop_outcomes: bool,
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
            fast_rebuild,
            use_build_tables,
            target_ready_outcomes_only,
            replace_existing_prop_outcomes,
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
    let benchmark_only = env_flag("ABCD_ENGINE_WORKSTATION")
        || env_flag("ABCD_BENCHMARK_ONLY")
        || env_flag("ABCD_SCAN_ONLY");
    let csv_export = !benchmark_only && env_flag_or("ABCD_EXPORT_CSV", false);
    let db_writes_enabled = !benchmark_only && !csv_export;
    let compact_scan_log = env_flag_or("ABCD_COMPACT_SCAN_LOG", !benchmark_only);
    let verbose_engine_log = !compact_scan_log;
    let phase_timings_enabled = benchmark_only || env_flag("ABCD_PHASE_TIMINGS");
    let reset_outputs = db_writes_enabled && env_flag("ABCD_RESET_OUTPUTS");
    let skip_processed_symbols =
        !benchmark_only && !reset_outputs && env_flag_or("ABCD_SKIP_PROCESSED_SYMBOLS", false);
    let use_build_tables = db_writes_enabled && env_flag("ABCD_USE_BUILD_TABLES");
    let fast_rebuild = db_writes_enabled
        && (use_build_tables
            || reset_outputs
            || skip_processed_symbols
            || env_flag("ABCD_FAST_REBUILD"));
    let defer_rebuild_indexes =
        fast_rebuild && !use_build_tables && env_flag_or("ABCD_DEFER_REBUILD_INDEXES", false);
    let replace_existing_prop_outcomes =
        db_writes_enabled && !fast_rebuild && !skip_processed_symbols;
    let write_pattern_setups = !benchmark_only && env_flag_or("ABCD_WRITE_PATTERN_SETUPS", true);
    let write_harmonic_scores = !benchmark_only && env_flag("ABCD_WRITE_HARMONIC_SCORES");
    let write_prop_outcomes = db_writes_enabled;
    let default_fit_only = env_flag_or("ABCD_DEFAULT_FIT_ONLY", false);
    let target_ready_outcomes_only = env_flag_or("ABCD_TARGET_READY_OUTCOMES_ONLY", false)
        && !env_flag("ABCD_WRITE_OPEN_PROP_OUTCOMES");
    let candle_source = CandleSource::from_env();
    let futures_root = env::var("ABCD_FUTURES_ROOT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let futures_contract_symbol = env::var("ABCD_FUTURES_CONTRACT_SYMBOL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let output_write_options = OutputWriteOptions {
        write_pattern_setups: write_pattern_setups && db_writes_enabled,
        write_harmonic_scores: write_harmonic_scores && db_writes_enabled,
        write_swing_outcomes: false,
        write_prop_outcomes,
    };
    let refresh_prop_family_summaries = db_writes_enabled
        && env_flag_or("ABCD_REFRESH_PROP_FAMILY_SUMMARIES", write_prop_outcomes)
        && !env_flag("ABCD_SKIP_PROP_FAMILY_SUMMARIES");
    let csv_output_dir = env::var("ABCD_CSV_OUTPUT_DIR")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("engine_csv_output/{}", run_id));
    let scan_concurrency = cmp::max(1, env_usize("ABCD_SCAN_CONCURRENCY", 4)?);
    let write_batch_size = cmp::max(1, env_usize("ABCD_WRITE_BATCH_SIZE", 1000)?);
    let progress_every = env_usize("ABCD_PROGRESS_EVERY", 10_000)?;
    let symbol_offset = env_usize("ABCD_SYMBOL_OFFSET", 0)?;
    let symbol_limit = env_usize("ABCD_SYMBOL_LIMIT", 0)?;
    let print_phase_logs =
        phase_timings_enabled && env_flag_or("ABCD_PRINT_PHASE_TIMINGS", verbose_engine_log);
    let max_x_bars_left = match env_i64("ABCD_MAX_X_BARS_LEFT", 0)? {
        value if value > 0 => Some(value),
        _ => None,
    };

    let pool = match MySqlPool::connect(&database_url).await {
        Ok(pool) => {
            if verbose_engine_log {
                println!("Connected to local DB");
            }
            pool
        }
        Err(error) => {
            eprintln!("Failed to connect to DB: {:?}", error);
            return Err(error.into());
        }
    };

    let db = Database { pool: pool.clone() };

    db.ensure_engine_phase_timings_table().await?;
    if verbose_engine_log {
        println!("Engine run id: {}", run_id);
    }
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
        if verbose_engine_log {
            println!("Cleared xabcd output tables before run");
        }
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
        if verbose_engine_log {
            println!("Created fresh build tables for output swap");
        }
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
        if verbose_engine_log {
            println!("Recreated generated output tables with append-friendly schema");
        }
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
        if verbose_engine_log {
            println!("Dropped secondary rebuild indexes before fast inserts");
        }
    }

    let phase_started = Instant::now();
    let mut symbols = match candle_source {
        CandleSource::EquityCandles => {
            db.get_symbols_above_average_volume(minimum_average_volume)
                .await?
        }
        CandleSource::FuturesContracts(_) => {
            if skip_processed_symbols {
                db.get_unprocessed_futures_contract_symbols(
                    futures_root.as_deref(),
                    candle_source.source_table(),
                    candle_source.source_timeframe(),
                )
                .await?
            } else {
                db.get_futures_contract_symbols(
                    futures_root.as_deref(),
                    candle_source.source_table(),
                )
                .await?
            }
        }
    };

    if let Some(contract_symbol) = futures_contract_symbol.as_deref() {
        let original_count = symbols.len();
        symbols.retain(|symbol| symbol.eq_ignore_ascii_case(contract_symbol));
        if symbols.is_empty() {
            return Err(format!(
                "Futures contract symbol {contract_symbol} was not found in {} after selecting {} symbols",
                candle_source.source_table(),
                original_count
            )
            .into());
        }
    }

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

    if verbose_engine_log {
        match candle_source {
            CandleSource::EquityCandles => println!(
                "Running xabcd scan on {} symbols with recent average volume >= {}",
                symbols.len(),
                minimum_average_volume
            ),
            CandleSource::FuturesContracts(_) => println!(
                "Running xabcd scan on {} futures contract symbols (root {}, contract {}, timeframe {})",
                symbols.len(),
                futures_root.as_deref().unwrap_or("all"),
                futures_contract_symbol.as_deref().unwrap_or("all"),
                candle_source.source_timeframe()
            ),
        }
        println!("Using candle source {}", candle_source.label());
        println!(
            "Using scan concurrency {}, write batch size {}, progress every {} candles, symbol limit {}",
            scan_concurrency,
            write_batch_size,
            if progress_every > 0 {
                progress_every.to_string()
            } else {
                "disabled".to_string()
            },
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
            "Using workstation {}, CSV export {}, phase timings {}, fast rebuild {}, defer rebuild indexes {}, build tables {}, pattern setups {}, harmonic scores {}, prop outcomes {}, default fit only {}, target-ready outcomes only {}, skip processed symbols {}, replace existing prop outcomes {}",
            benchmark_only,
            csv_export,
            phase_timings_enabled,
            fast_rebuild,
            defer_rebuild_indexes,
            use_build_tables,
            write_pattern_setups,
            write_harmonic_scores,
            write_prop_outcomes,
            default_fit_only,
            target_ready_outcomes_only,
            skip_processed_symbols,
            replace_existing_prop_outcomes
        );
        if benchmark_only {
            println!(
                "Engine workstation mode: scanner runs normally, but pattern/output rows are not written."
            );
        }
        if csv_export {
            println!(
                "CSV export mode: generated rows will be written to {} instead of DB output tables.",
                csv_output_dir
            );
        }
    }

    let mut csv_output_writer = if csv_export {
        let writer = EngineCsvOutputWriter::create(&csv_output_dir, write_pattern_setups)?;
        println!(
            "[csv] exporting generated rows to {}",
            writer.output_dir().display()
        );
        println!("[csv] load script: {}", writer.load_script_path().display());
        Some(writer)
    } else {
        None
    };

    let total_symbols = symbols.len();
    let mut completed_symbols = 0usize;
    let mut failed_symbols = 0usize;
    let mut total_setups_written = 0usize;
    let mut total_prop_reversal_rows = 0usize;
    let mut total_bearish_patterns = 0usize;
    let mut total_bullish_patterns = 0usize;
    let mut total_phase_timings = ScanPhaseTimings::default();
    let mut total_symbol_scan_duration = Duration::ZERO;
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
                default_fit_only,
                progress_every,
                phase_timings_enabled,
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
                        default_fit_only,
                        progress_every,
                        phase_timings_enabled,
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
                        default_fit_only,
                        progress_every,
                        phase_timings_enabled,
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
        let symbol_load_duration = result.load_duration;
        let symbol_scan_duration = result.scan_duration;
        let symbol_total_duration = result.total_duration;
        let symbol_candle_count = result.candle_count;
        let symbol_phase_timings = result.phase_timings;
        total_phase_timings.add_assign(symbol_phase_timings);
        total_symbol_scan_duration += symbol_scan_duration;
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
        let scan_seconds = symbol_scan_duration.as_secs_f64().max(0.000_001);
        if verbose_engine_log {
            println!(
                "[timing] {} candles: {}, load: {}, scan: {}, total: {}, scan candles/sec: {:.0}, setups/sec: {:.0}",
                symbol_name,
                symbol_candle_count,
                format_duration(symbol_load_duration),
                format_duration(symbol_scan_duration),
                format_duration(symbol_total_duration),
                symbol_candle_count as f64 / scan_seconds,
                symbol_pattern_count as f64 / scan_seconds
            );
        }
        if print_phase_logs && total_symbols > 1 {
            print_phase_timing_lines(&symbol_name, symbol_phase_timings, symbol_scan_duration);
        }

        if benchmark_only {
            if verbose_engine_log {
                println!(
                    "[workstation] {} DB output skipped; generated {} setup rows and {} prop reversal rows in memory",
                    symbol_name,
                    symbol_pattern_count,
                    prop_reversal_outcomes.len()
                );
            }
        } else if let Some(writer) = csv_output_writer.as_mut() {
            let csv_counts = writer.write_symbol(
                &db,
                &patterns,
                &prop_reversal_outcomes,
                target_ready_outcomes_only,
            )?;
            if compact_scan_log {
                println!(
                    "[csv] wrote {} setup rows and {} prop outcome rows",
                    csv_counts.setup_rows, csv_counts.prop_outcome_rows
                );
            }
        } else {
            pending_patterns.append(&mut patterns);
            pending_prop_reversal_outcomes.append(&mut prop_reversal_outcomes);
        }
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
                default_fit_only,
                progress_every,
                phase_timings_enabled,
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

        if db_writes_enabled && pending_patterns.len() >= write_batch_size {
            let flush_size = pending_patterns.len();
            if compact_scan_log {
                println!(
                    "[db] writing {} setup rows and {} prop outcome rows",
                    flush_size,
                    pending_prop_reversal_outcomes.len()
                );
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
                replace_existing_prop_outcomes,
            )
            .await?;
            if compact_scan_log {
                println!("[db] wrote {} setup rows", flush_size);
            } else if verbose_engine_log {
                println!("Flushed {} pattern setups to DB", flush_size);
            }
        }
    }

    if benchmark_only {
        if verbose_engine_log {
            println!("Engine workstation mode: final DB output flush skipped.");
        }
    } else if let Some(writer) = csv_output_writer.as_mut() {
        writer.finish()?;
        println!(
            "[csv] complete: {} setup rows, {} prop outcome rows",
            writer.setup_rows(),
            writer.prop_outcome_rows()
        );
        if let Some(path) = writer.pattern_setups_path() {
            println!("[csv] pattern setups: {}", path.display());
        }
        println!(
            "[csv] prop outcomes: {}",
            writer.prop_outcomes_path().display()
        );
        println!("[csv] load script: {}", writer.load_script_path().display());
    } else {
        let flush_size = pending_patterns.len();
        if compact_scan_log && flush_size > 0 {
            println!(
                "[db] writing {} setup rows and {} prop outcome rows",
                flush_size,
                pending_prop_reversal_outcomes.len()
            );
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
            replace_existing_prop_outcomes,
        )
        .await?;
        if compact_scan_log && flush_size > 0 {
            println!("[db] wrote {} setup rows", flush_size);
        }
    }

    if fast_rebuild && !defer_rebuild_indexes {
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
        if verbose_engine_log {
            println!("Recreated secondary rebuild indexes after fast inserts");
        }
    } else if fast_rebuild {
        db.record_engine_phase_timing(
            &run_id,
            None,
            "recreate_rebuild_indexes",
            None,
            Duration::ZERO,
            Some("deferred by ABCD_DEFER_REBUILD_INDEXES"),
        )
        .await?;
        println!(
            "[db] deferred secondary index rebuild; run cargo run --bin rebuild_engine_indexes when loading is done"
        );
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
        if verbose_engine_log {
            println!("Swapped build tables into final output table names");
        }
    }

    if refresh_prop_family_summaries {
        if verbose_engine_log {
            println!("Refreshing prop strategy family summaries");
        }
        db.refresh_prop_strategy_family_rollups(Some(&run_id))
            .await?;
        if verbose_engine_log {
            println!("Refreshing prop contract week summary");
        }
        db.refresh_prop_contract_week_summary(Some(&run_id)).await?;
        if verbose_engine_log {
            println!("Refreshing prop family weekly cadence");
        }
        db.refresh_prop_family_weekly_cadence(Some(&run_id)).await?;
        if verbose_engine_log {
            println!("Marked prop strategy family summaries ready");
        }
    } else {
        if verbose_engine_log {
            println!("Skipped prop strategy family summaries refresh");
        }
    }

    if verbose_engine_log {
        println!(
            "Pattern setups total: {}, Prop reversal rows: {}, Bear: {}, Bull: {}, Failed symbols: {}",
            total_setups_written,
            total_prop_reversal_rows,
            total_bearish_patterns,
            total_bullish_patterns,
            failed_symbols
        );
    }
    if print_phase_logs {
        print_phase_timing_lines("total", total_phase_timings, total_symbol_scan_duration);
    }

    let duration = start.elapsed();
    if verbose_engine_log {
        println!("Execution time: {:?}", duration);
    }

    Ok(())
}
