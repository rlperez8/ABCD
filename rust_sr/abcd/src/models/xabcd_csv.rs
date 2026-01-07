use std::fs::File;
use csv::WriterBuilder;
use serde::Serialize;
use serde::ser::Serializer;
use crate::models::reversal_type::ReversalType;
use crate::models::pattern_abcd::PatternXABCD;
use crate::models::market::Market;

fn two_decimals<S>(val: &f64, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let v = (val * 100.0).round() / 100.0;
    s.serialize_f64(v)
}

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

    trade_result: Option<bool>,
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

}

impl XABCD_CSV {
      pub fn write_patterns_to_csv(patterns: &[PatternXABCD], filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Map nested structs into flat CSV structs
        let csv_patterns: Vec<XABCD_CSV> = patterns.iter().map(|p| XABCD_CSV {
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
            trade_ab_price_retracement:  p.trade.ab_price_retracement,
            trade_bc_price_retracement:  p.trade.bc_price_retracement,
            trade_cd_bc_price_retracement: p.trade.cd_bc_price_retracement,
            trade_cd_price_retracement: p.trade.cd_price_retracement,
            trade_cd_xa_price_retracement: p.trade.cd_xa_price_retracement,
            trade_bc_bar_retracement:  p.trade.bc_bar_retracement,
            trade_cd_bar_retracement: p.trade.cd_bar_retracement,
            
            trade_snr: p.trade.snr,
            trade_year: p.trade.year,
            trade_month: p.trade.month,
            trade_day: p.trade.day,
            reversal_type: p.trade.reversal_type.clone(),
            market: p.market,
            three_month: p.three_month,
            six_month: p.six_month,
            twelve_month: p.twelve_month
        }).collect();

        let file = File::create(filename)?;
        let mut writer = WriterBuilder::new().has_headers(true).from_writer(file);

        for p in csv_patterns {
            writer.serialize(p)?;
        }

        writer.flush()?;
        Ok(())
    }
}