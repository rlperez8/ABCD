// use std::str::pattern;

// use mysql::{Pool, PooledConn, OptsBuilder, SslOpts};
use mysql::*;
// use mysql::prelude::*;
use crate::models::candle::*;
// use chrono::NaiveDate;
use sqlx::mysql::MySqlPool;
// use std::fs::File;
// use csv::WriterBuilder;
use serde::Serialize;
use serde::ser::Serializer;
use crate::models::reversal_type::ReversalType;
use crate::models::pattern_abcd::PatternXABCD;
use crate::models::harmonic_types::HarmonicType;
use crate::models::market::Market;
use sqlx::{MySql, QueryBuilder};
// use sqlx::{QueryBuilder};
use rust_decimal::prelude::ToPrimitive;

const PATTERN_INSERT_CHUNK_SIZE: usize = 200;
const ACCURACY_INSERT_CHUNK_SIZE: usize = 1000;

pub struct Database {
    pub pool: MySqlPool,
}
#[allow(non_camel_case_types)]

#[derive(Serialize)]
pub struct XABCD_CSV {
    symbol: String,
    x_date: String,
    #[serde(serialize_with = "two_decimals")]
    x_open: f64,
    #[serde(serialize_with = "two_decimals")]
    x_high: f64,
    #[serde(serialize_with = "two_decimals")]
    x_low: f64,
    #[serde(serialize_with = "two_decimals")]
    x_close: f64,

    x_length: i64,

    #[serde(serialize_with = "two_decimals")]
    x_min_max: f64,

    a_date: String,

    #[serde(serialize_with = "two_decimals")]
    a_open: f64,
    #[serde(serialize_with = "two_decimals")]
    a_high: f64,
    #[serde(serialize_with = "two_decimals")]
    a_low: f64,
    #[serde(serialize_with = "two_decimals")]
    a_close: f64,

    a_length: i64,

    #[serde(serialize_with = "two_decimals")]
    a_min_max: f64,
    #[serde(serialize_with = "two_decimals")]
    xa_price_length: f64,

    b_date: String,

    #[serde(serialize_with = "two_decimals")]
    b_open: f64,
    #[serde(serialize_with = "two_decimals")]
    b_high: f64,
    #[serde(serialize_with = "two_decimals")]
    b_low: f64,
    #[serde(serialize_with = "two_decimals")]
    b_close: f64,

    b_length: i64,

    #[serde(serialize_with = "two_decimals")]
    b_min_max: f64,
    #[serde(serialize_with = "two_decimals")]
    ab_price_length: f64,

    c_date: String,

    #[serde(serialize_with = "two_decimals")]
    c_open: f64,
    #[serde(serialize_with = "two_decimals")]
    c_high: f64,
    #[serde(serialize_with = "two_decimals")]
    c_low: f64,
    #[serde(serialize_with = "two_decimals")]
    c_close: f64,

    c_length: i64,

    #[serde(serialize_with = "two_decimals")]
    c_min_max: f64,
    #[serde(serialize_with = "two_decimals")]
    bc_price_length: f64,

    d_date: String,

    #[serde(serialize_with = "two_decimals")]
    d_open: f64,
    #[serde(serialize_with = "two_decimals")]
    d_high: f64,
    #[serde(serialize_with = "two_decimals")]
    d_low: f64,
    #[serde(serialize_with = "two_decimals")]
    d_close: f64,

    d_length: i64,

    #[serde(serialize_with = "two_decimals")]
    d_min_max: f64,

    #[serde(serialize_with = "two_decimals")]
    cd_price_length: f64,

    trade_open: bool,

    #[serde(serialize_with = "two_decimals")]
    trade_risk_exit_price: f64,
    #[serde(serialize_with = "two_decimals")]
    trade_reward_exit_price: f64,
    #[serde(serialize_with = "two_decimals")]
    trade_enter_price: f64,
    #[serde(serialize_with = "two_decimals")]
    trade_current_price: f64,

    trade_length: i64,

    #[serde(serialize_with = "two_decimals")]
    trade_pnl: f64,

    trade_result: i32,
    trade_date: String,
    trade_symbol: String,

    #[serde(serialize_with = "two_decimals")]
    trade_ab_price_retracement: f64,

    #[serde(serialize_with = "two_decimals")]
    trade_bc_price_retracement: f64,

    #[serde(serialize_with = "two_decimals")]
    trade_cd_xa_price_retracement: f64,

    #[serde(serialize_with = "two_decimals")]
    trade_cd_price_retracement: f64,

    #[serde(serialize_with = "two_decimals")]
    trade_bc_bar_retracement: f64,

    #[serde(serialize_with = "two_decimals")]
    trade_cd_bar_retracement: f64,

    #[serde(serialize_with = "two_decimals")]
    trade_cd_bc_price_retracement: f64,

    #[serde(serialize_with = "two_decimals")]
    trade_snr: f64,
    trade_year: i64,
    trade_month: i64,
    trade_day: i64,

    reversal_type: ReversalType,
    market: Market,
    three_month: Option<bool>,
    six_month: Option<bool>,
    twelve_month: Option<bool>,
    pattern_group_id: String,
    harmonic_type: HarmonicType,
    #[serde(serialize_with = "two_decimals")]
    bat_accuracy: f64,
    shark_accuracy: f64,
    butterfly_accuracy: f64,
    gartley_accuracy: f64,
    crab_accuracy: f64,

}

pub struct ScatterPlotDataBase {
    pub accuracy: f64,
    pub return_pct: f64,    
    pub harmonic_type: &'static str,

}

impl Database {
 
    pub async fn clear_generated_outputs(&self) -> Result<(), sqlx::Error> {
        sqlx::query("TRUNCATE TABLE xabcd_patterns")
            .execute(&self.pool)
            .await?;

        sqlx::query("TRUNCATE TABLE accuracies")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_distinct_symbols(&self) -> Result<Vec<String>, sqlx::Error> {
        println!("✅ get_distinct_symbols");

        let symbols: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT DISTINCT symbol
            FROM abcd.candles
            ORDER BY symbol
            "#
        )
        .fetch_all(&self.pool)  // async fetch
        .await?;                // must await

        Ok(symbols)
    }
    
    pub async fn get_symbols_above_average_volume(
        &self,
        minimum_average_volume: f64,
    ) -> Result<Vec<String>, sqlx::Error> {
        println!(
            "Filtering symbols by recent average volume >= {}",
            minimum_average_volume
        );

        let symbols: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT ls.symbol
            FROM abcd.listing_status ls
            INNER JOIN (
                SELECT DISTINCT symbol
                FROM abcd.candles
            ) candles ON candles.symbol = ls.symbol
            WHERE ls.status = 'Active'
              AND (ls.bugged IS NULL OR ls.bugged = false)
              AND ls.average_volume_30d IS NOT NULL
              AND ls.average_volume_30d >= ?
            ORDER BY ls.symbol
            "#
        )
        .bind(minimum_average_volume)
        .fetch_all(&self.pool)
        .await?;

        Ok(symbols)
    }

    pub async fn get_stored_candles(&self, symbol: &str) -> Result<Vec<Candle>, sqlx::Error> {
            
        let candles_decimal: Vec<CandleDecimal> = sqlx::query_as!(
                CandleDecimal,
                r#"
                SELECT symbol, date, open, high, low, close, volume
                FROM abcd.candles
                WHERE symbol = ?
                ORDER BY date
                "#,
                symbol
            )
            .fetch_all(&self.pool)
            .await?;

            let candles: Vec<Candle> = candles_decimal
            .into_iter()
            .map(|c| Candle {
                symbol: c.symbol,
                date: c.date,
                open: c.open.to_f64().unwrap(),
                high: c.high.to_f64().unwrap(),
                low: c.low.to_f64().unwrap(),
                close: c.close.to_f64().unwrap(),
                volume: c.volume.to_i64().unwrap(),
            })
            .collect();
            Ok(candles)
    }
    
    pub async fn insert_patterns(&self, patterns: &[PatternXABCD]) -> Result<(), sqlx::Error> {
        if patterns.is_empty() {
            return Ok(());
        }

        let serialized_patterns: Vec<XABCD_CSV> = patterns
            .iter()
            .map(|pattern| self.from_pattern(pattern))
            .collect();

        let mut tx = self.pool.begin().await?;

        for chunk in serialized_patterns.chunks(PATTERN_INSERT_CHUNK_SIZE) {
            let mut builder = QueryBuilder::<MySql>::new(
                r#"
                INSERT INTO xabcd_patterns (
                    symbol, x_date, x_open, x_high, x_low, x_close,
                    x_length, x_min_max, a_date, a_open, a_high, a_low, a_close,
                    a_length, a_min_max, b_date, b_open, b_high, b_low, b_close,
                    b_length, b_min_max, c_date, c_open, c_high, c_low, c_close,
                    c_length, c_min_max, d_date, d_open, d_high, d_low, d_close,
                    d_length, d_min_max, trade_open, trade_risk_exit_price,
                    trade_reward_exit_price, trade_enter_price, trade_current_price,
                    trade_length, trade_pnl, trade_result, trade_date, trade_symbol,
                    trade_ab_price_retracement, trade_bc_price_retracement,
                    trade_cd_xa_price_retracement, trade_cd_price_retracement,
                    trade_bc_bar_retracement, trade_cd_bar_retracement,
                    trade_cd_bc_price_retracement, trade_snr, trade_year,
                    trade_month, trade_day, reversal_type, market,
                    three_month, six_month, twelve_month, pattern_group_id, harmonic_type,
                    xa_price_length, ab_price_length, bc_price_length, cd_price_length,
                    bat_accuracy, butterfly_accuracy, gartley_accuracy, crab_accuracy, shark_accuracy
                )
                "#
            );

            builder.push_values(chunk, |mut row, p| {
                row.push_bind(&p.symbol)
                    .push_bind(&p.x_date)
                    .push_bind(p.x_open)
                    .push_bind(p.x_high)
                    .push_bind(p.x_low)
                    .push_bind(p.x_close)
                    .push_bind(p.x_length)
                    .push_bind(p.x_min_max)
                    .push_bind(&p.a_date)
                    .push_bind(p.a_open)
                    .push_bind(p.a_high)
                    .push_bind(p.a_low)
                    .push_bind(p.a_close)
                    .push_bind(p.a_length)
                    .push_bind(p.a_min_max)
                    .push_bind(&p.b_date)
                    .push_bind(p.b_open)
                    .push_bind(p.b_high)
                    .push_bind(p.b_low)
                    .push_bind(p.b_close)
                    .push_bind(p.b_length)
                    .push_bind(p.b_min_max)
                    .push_bind(&p.c_date)
                    .push_bind(p.c_open)
                    .push_bind(p.c_high)
                    .push_bind(p.c_low)
                    .push_bind(p.c_close)
                    .push_bind(p.c_length)
                    .push_bind(p.c_min_max)
                    .push_bind(&p.d_date)
                    .push_bind(p.d_open)
                    .push_bind(p.d_high)
                    .push_bind(p.d_low)
                    .push_bind(p.d_close)
                    .push_bind(p.d_length)
                    .push_bind(p.d_min_max)
                    .push_bind(p.trade_open)
                    .push_bind(p.trade_risk_exit_price)
                    .push_bind(p.trade_reward_exit_price)
                    .push_bind(p.trade_enter_price)
                    .push_bind(p.trade_current_price)
                    .push_bind(p.trade_length)
                    .push_bind(p.trade_pnl)
                    .push_bind(p.trade_result)
                    .push_bind(&p.trade_date)
                    .push_bind(&p.trade_symbol)
                    .push_bind(p.trade_ab_price_retracement)
                    .push_bind(p.trade_bc_price_retracement)
                    .push_bind(p.trade_cd_xa_price_retracement)
                    .push_bind(p.trade_cd_price_retracement)
                    .push_bind(p.trade_bc_bar_retracement)
                    .push_bind(p.trade_cd_bar_retracement)
                    .push_bind(p.trade_cd_bc_price_retracement)
                    .push_bind(p.trade_snr)
                    .push_bind(p.trade_year)
                    .push_bind(p.trade_month)
                    .push_bind(p.trade_day)
                    .push_bind(format!("{:?}", p.reversal_type))
                    .push_bind(format!("{:?}", p.market))
                    .push_bind(p.three_month)
                    .push_bind(p.six_month)
                    .push_bind(p.twelve_month)
                    .push_bind(&p.pattern_group_id)
                    .push_bind(format!("{:?}", p.harmonic_type))
                    .push_bind(p.xa_price_length)
                    .push_bind(p.ab_price_length)
                    .push_bind(p.bc_price_length)
                    .push_bind(p.cd_price_length)
                    .push_bind(p.bat_accuracy)
                    .push_bind(p.butterfly_accuracy)
                    .push_bind(p.gartley_accuracy)
                    .push_bind(p.crab_accuracy)
                    .push_bind(p.shark_accuracy);
            });

            builder.build().execute(&mut *tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }    
    
    pub fn from_pattern(&self, p: &PatternXABCD) -> XABCD_CSV  {
        
        XABCD_CSV {
            symbol: p.symbol.to_string(),
            x_date: p.x.date.to_string(),
            x_open: p.x.open,
            x_high: p.x.high,
            x_low: p.x.low,
            x_close: p.x.close,
            x_length: p.x.length,
            x_min_max: p.x.min_max,
            a_date: p.a.date.to_string(),
            a_open: p.a.open,
            a_high: p.a.high,
            a_low: p.a.low,
            a_close: p.a.close,
            a_length: p.a.length,
            xa_price_length: p.a.leg_price_length,
            a_min_max: p.a.min_max,
            b_date: p.b.date.to_string(),
            b_open: p.b.open,
            b_high: p.b.high,
            b_low: p.b.low,
            b_close: p.b.close,
            b_length: p.b.length,
            b_min_max: p.b.min_max,
            ab_price_length: p.b.leg_price_length,
            c_date: p.c.date.to_string(),
            c_open: p.c.open,
            c_high: p.c.high,
            c_low: p.c.low,
            c_close: p.c.close,
            c_length: p.c.length,
            c_min_max: p.c.min_max,
            bc_price_length: p.c.leg_price_length,
            d_date: p.d.date.to_string(),
            d_open: p.d.open,
            d_high: p.d.high,
            d_low: p.d.low,
            d_close: p.d.close,
            d_length: p.d.length,
            d_min_max: p.d.min_max,
            cd_price_length: p.d.leg_price_length,
            trade_open: p.trade.open,
            trade_risk_exit_price: p.trade.risk_exit_price,
            trade_reward_exit_price: p.trade.reward_exit_price,
            trade_enter_price: p.trade.enter_price,
            trade_current_price: p.trade.current_price,
            trade_length: p.trade.length,
            trade_pnl: p.trade.pnl,
            trade_result: p.trade.result,
            trade_date: p.trade.date.to_string(),
            trade_symbol: p.symbol.to_string(),
            trade_ab_price_retracement: p.trade.ab_price_retracement,
            trade_bc_price_retracement: p.trade.bc_price_retracement,
            trade_cd_xa_price_retracement: p.trade.cd_xa_price_retracement,
            trade_cd_price_retracement: p.trade.cd_price_retracement,
            trade_bc_bar_retracement: p.trade.bc_bar_retracement,
            trade_cd_bar_retracement: p.trade.cd_bar_retracement,
            trade_cd_bc_price_retracement: p.trade.cd_bc_price_retracement,
            trade_snr: p.trade.snr,
            trade_year: p.trade.year,
            trade_month: p.trade.month,
            trade_day: p.trade.day,
            reversal_type: p.trade.reversal_type.clone(),
            market: p.market,
            three_month: p.three_month,
            six_month: p.six_month,
            twelve_month: p.twelve_month,
            pattern_group_id: format!("{}{}", p.symbol, p.a.date),
            harmonic_type: p.abcd_type,
            bat_accuracy: p.accuracies.bat.pattern_accuracy,
            butterfly_accuracy: p.accuracies.butterfly.pattern_accuracy,
            gartley_accuracy: p.accuracies.gartley.pattern_accuracy,    
            crab_accuracy: p.accuracies.crab.pattern_accuracy,
            shark_accuracy: p.accuracies.shark.pattern_accuracy,
            
        }
    }

    pub async fn insert_scatter_plot(&self, patterns: &[PatternXABCD]) -> Result<(), sqlx::Error> {
        if patterns.is_empty() {
            return Ok(());
        }

        let mut scatter_rows: Vec<ScatterPlotDataBase> = Vec::with_capacity(patterns.len() * 5);

        for pattern in patterns {
            let acc = &pattern.accuracies;

            for (name, value) in [
                ("Bat", &acc.bat),
                ("Butterfly", &acc.butterfly),
                ("Gartley", &acc.gartley),
                ("Crab", &acc.crab),
                ("Shark", &acc.shark),
            ] {
                scatter_rows.push(ScatterPlotDataBase {
                    accuracy: value.pattern_accuracy,
                    return_pct: pattern.trade.pnl,
                    harmonic_type: name,
                });
            }
        }

        let mut tx = self.pool.begin().await?;

        for chunk in scatter_rows.chunks(ACCURACY_INSERT_CHUNK_SIZE) {
            let mut builder = QueryBuilder::<MySql>::new(
                r#"
                INSERT INTO accuracies (
                    accuracy, return_pct, harmonic_type
                )
                "#
            );

            builder.push_values(chunk, |mut row, item| {
                row.push_bind(item.accuracy)
                    .push_bind(item.return_pct)
                    .push_bind(item.harmonic_type);
            });

            builder.build().execute(&mut *tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }
     
}

fn two_decimals<S>(val: &f64, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let v = (val * 100.0).round() / 100.0;
    s.serialize_f64(v)
}
