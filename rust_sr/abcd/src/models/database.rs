// use std::str::pattern;

use std::vec;

// use mysql::{Pool, PooledConn, OptsBuilder, SslOpts};
use mysql::*;
// use mysql::prelude::*;
use crate::models::{PatternX, candle::*}; 
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
// use sqlx::{QueryBuilder};
use rust_decimal::prelude::ToPrimitive;

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
    harmonic_type: HarmonicType

}

pub struct ScatterPlotDataBase {
    pub accuracy: f64,
    pub return_pct: f64,    
    pub harmonic_type: String,

}

impl Database {
 
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
    
    pub async fn get_stored_candles(&self, symbol: &str) -> Result<Vec<Candle>, sqlx::Error> {
            
        let mut candles_decimal: Vec<CandleDecimal> = sqlx::query_as!(
                CandleDecimal,
                r#"
                SELECT symbol, date, open, high, low, close, volume
                FROM abcd.candles
                WHERE symbol = ?
                ORDER BY date DESC
                LIMIT 90
                "#,
                symbol
            )
            .fetch_all(&self.pool)
            .await?;

            candles_decimal.reverse();

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
        for pat in patterns {


            let p = self.from_pattern(pat); 

            sqlx::query!(
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
                    three_month, six_month, twelve_month, pattern_group_id, harmonic_type, xa_price_length, ab_price_length, bc_price_length, cd_price_length
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
                
                p.symbol, p.x_date, p.x_open, p.x_high, p.x_low, p.x_close, p.x_length, p.x_min_max,
                p.a_date, p.a_open, p.a_high, p.a_low, p.a_close, p.a_length, p.a_min_max,
                p.b_date, p.b_open, p.b_high, p.b_low, p.b_close, p.b_length, p.b_min_max,
                p.c_date, p.c_open, p.c_high, p.c_low, p.c_close, p.c_length, p.c_min_max,
                p.d_date, p.d_open, p.d_high, p.d_low, p.d_close, p.d_length, p.d_min_max,
                p.trade_open, p.trade_risk_exit_price, p.trade_reward_exit_price,
                p.trade_enter_price, p.trade_current_price, p.trade_length, p.trade_pnl,
                p.trade_result, p.trade_date, p.trade_symbol,
                p.trade_ab_price_retracement, p.trade_bc_price_retracement,
                p.trade_cd_xa_price_retracement, p.trade_cd_price_retracement,
                p.trade_bc_bar_retracement, p.trade_cd_bar_retracement,
                p.trade_cd_bc_price_retracement,
                p.trade_snr, p.trade_year, p.trade_month, p.trade_day,
                format!("{:?}", p.reversal_type), format!("{:?}", p.market),
                p.three_month, p.six_month, p.twelve_month, p.pattern_group_id, format!("{:?}", p.harmonic_type),
                p.xa_price_length, p.ab_price_length, p.bc_price_length, p.cd_price_length
                
            )
            .execute(&self.pool)
            .await?;
                    }
        Ok(())
    }    
    
    pub fn from_pattern(&self, p: &PatternXABCD) -> XABCD_CSV  {
        
        XABCD_CSV {
            symbol: p.symbol.clone(),
            x_date: p.x.date.clone(),
            x_open: p.x.open,
            x_high: p.x.high,
            x_low: p.x.low,
            x_close: p.x.close,
            x_length: p.x.length,
            x_min_max: p.x.min_max,
            a_date: p.a.date.clone(),
            a_open: p.a.open,
            a_high: p.a.high,
            a_low: p.a.low,
            a_close: p.a.close,
            a_length: p.a.length,
            xa_price_length: p.a.leg_price_length,
            a_min_max: p.a.min_max,
            b_date: p.b.date.clone(),
            b_open: p.b.open,
            b_high: p.b.high,
            b_low: p.b.low,
            b_close: p.b.close,
            b_length: p.b.length,
            b_min_max: p.b.min_max,
            ab_price_length: p.b.leg_price_length,
            c_date: p.c.date.clone(),
            c_open: p.c.open,
            c_high: p.c.high,
            c_low: p.c.low,
            c_close: p.c.close,
            c_length: p.c.length,
            c_min_max: p.c.min_max,
            bc_price_length: p.c.leg_price_length,
            d_date: p.d.date.clone(),
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
            trade_date: p.trade.date.clone(),
            trade_symbol: p.trade.symbol.clone(),
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
            pattern_group_id: p.pattern_group_id.clone(),
            harmonic_type: p.abcd_type.clone(),
        }
    }

    pub async fn insert_scatter_plot(&self, patterns: &Vec<PatternXABCD>) -> Result<(), sqlx::Error> {
        
        for pattern in patterns {
            let acc = &pattern.accuracies;
   
            for (name, value) in [
                ("Bat", &acc.bat),
                ("Butterfly", &acc.butterfly),
                ("Gartley", &acc.gartley),
                ("Crab", &acc.crab),
                ("Shark", &acc.shark),
            ] {
                let scatter_data = ScatterPlotDataBase {
                    accuracy: value.pattern_accuracy,
                    return_pct: pattern.trade.pnl,
                    harmonic_type: name.to_string(),
                };

                sqlx::query!(
                    r#"
                        INSERT INTO accuracies (
                            accuracy, return_pct, harmonic_type
                        ) VALUES (?, ?, ?)
                    "#,
                    scatter_data.accuracy, scatter_data.return_pct, scatter_data.harmonic_type
                )
                .execute(&self.pool)
                .await
                .unwrap();


                // println!("Scatter Data - Harmonic: {}, Accuracy: {:.2}, Return %: {:.2}", 
                //     scatter_data.harmonic_type, scatter_data.accuracy, scatter_data.return_pct);
            }
        }

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