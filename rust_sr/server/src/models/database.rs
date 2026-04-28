// use sqlx::mysql::MySqlPool;
// use sqlx::Error;
// use crate::pattern::Pattern; // your Pattern struct
// use crate::candles::Candle;
// pub use sqlx::Database;
// pub use sqlx::Error::Database;

use crate::pattern::Pattern;
use sqlx::mysql::MySqlPool;
use sqlx::Error;

pub struct Database {
    pub pool: MySqlPool,
}

impl Database {
    pub async fn fetch_all(&self) -> Result<Vec<Pattern>, Error> {
        let patterns: Vec<Pattern> = sqlx::query_as::<_, Pattern>("SELECT * FROM xabcd_patterns")
            .fetch_all(&self.pool)
            .await?;

        Ok(patterns)
    }

    // Azure DB
    // pub fn new_azure() -> Result<Self, Box<dyn std::error::Error>> {
    //     let builder = OptsBuilder::new()
    //         .ip_or_hostname(Some("abcd-mysql.mysql.database.azure.com"))
    //         .user(Some("rperezkc"))
    //         .pass(Some("Nar888uto!"))
    //         .db_name(Some("abcd"))
    //         .tcp_port(3306)
    //         .ssl_opts(Some(SslOpts::default()));

    //     let pool = Pool::new(builder)?;  // create pool once
    //     Ok(Self { pool })
    // }

    // // Get a connection from the pool when needed
    // pub fn get_conn(&self) -> Result<PooledConn, mysql::Error> {
    //     self.pool.get_conn()
    // }
    // --- fetch stored candles ---
    // pub fn get_stored_candles(&self, symbol: &str) -> mysql::Result<Vec<Candle>> {
    //     // borrow a connection from the pool
    //     let mut conn = self.get_conn()?;

    //     let query = r#"
    //         SELECT symbol,
    //                DATE_FORMAT(date, '%Y-%m-%d') AS date_str,
    //                open, high, low, close, volume
    //         FROM abcd.candles
    //         WHERE symbol = :symbol
    //     "#;

    //     let params = params! { "symbol" => symbol };

    //     let candles: Vec<Candle> = conn.exec_map(
    //         query,
    //         params,
    //         |(symbol, date_str, open, high, low, close, volume):
    //             (String, String, f64, f64, f64, f64, u64)|
    //         {
    //             let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
    //                 .expect("Failed to parse date");
    //             Candle {
    //                 symbol,
    //                 date,
    //                 open,
    //                 high,
    //                 low,
    //                 close,
    //                 volume,
    //             }
    //         },
    //     )?;

    //     Ok(candles)
    // }
    // pub fn fetch_all(&self) -> Result<Vec<Pattern>, Box<dyn Error>> {
    //     // borrow a connection from the pool
    //     let mut conn = self.get_conn()?;

    //     let rows: Vec<mysql::Row> = conn.query(
    //         "SELECT
    //             symbol, x_date, x_open, x_high, x_low, x_close, x_length, x_min_max,
    //             a_date, a_open, a_high, a_low, a_close, a_length, a_min_max,
    //             b_date, b_open, b_high, b_low, b_close, b_length, b_min_max,
    //             c_date, c_open, c_high, c_low, c_close, c_length, c_min_max,
    //             d_date, d_open, d_high, d_low, d_close, d_length, d_min_max,
    //             trade_open, trade_risk_exit_price, trade_reward_exit_price,
    //             trade_enter_price, trade_current_price, trade_length, trade_pnl,
    //             trade_result, trade_date, trade_ab_price_retracement, trade_bc_price_retracement,
    //             trade_cd_bc_price_retracement, trade_cd_price_retracement, trade_cd_xa_price_retracement,
    //             trade_bc_bar_retracement, trade_cd_bar_retracement,
    //             trade_snr, trade_year, trade_month, trade_day, market, three_month, six_month, twelve_month, pattern_group_id, harmonic_type
    //          FROM xabcd_patterns"
    //     )?;

    //     let mut patterns = Vec::with_capacity(rows.len());

    //     for row in rows {
    //         let pattern = Pattern {
    //             symbol: row.get("symbol").unwrap_or_default(),
    //             x_date: row.get("x_date").unwrap_or_default(),
    //             x_open: row.get("x_open").unwrap_or(0.0),
    //             x_high: row.get("x_high").unwrap_or(0.0),
    //             x_low: row.get("x_low").unwrap_or(0.0),
    //             x_close: row.get("x_close").unwrap_or(0.0),
    //             x_length: row.get("x_length").unwrap_or(0.0),
    //             x_min_max: row.get("x_min_max").unwrap_or(0.0),
    //             a_date: row.get("a_date").unwrap_or_default(),
    //             a_open: row.get("a_open").unwrap_or(0.0),
    //             a_high: row.get("a_high").unwrap_or(0.0),
    //             a_low: row.get("a_low").unwrap_or(0.0),
    //             a_close: row.get("a_close").unwrap_or(0.0),
    //             a_length: row.get("a_length").unwrap_or(0.0),
    //             a_min_max: row.get("a_min_max").unwrap_or(0.0),
    //             b_date: row.get("b_date").unwrap_or_default(),
    //             b_open: row.get("b_open").unwrap_or(0.0),
    //             b_high: row.get("b_high").unwrap_or(0.0),
    //             b_low: row.get("b_low").unwrap_or(0.0),
    //             b_close: row.get("b_close").unwrap_or(0.0),
    //             b_length: row.get("b_length").unwrap_or(0.0),
    //             b_min_max: row.get("b_min_max").unwrap_or(0.0),
    //             c_date: row.get("c_date").unwrap_or_default(),
    //             c_open: row.get("c_open").unwrap_or(0.0),
    //             c_high: row.get("c_high").unwrap_or(0.0),
    //             c_low: row.get("c_low").unwrap_or(0.0),
    //             c_close: row.get("c_close").unwrap_or(0.0),
    //             c_length: row.get("c_length").unwrap_or(0.0),
    //             c_min_max: row.get("c_min_max").unwrap_or(0.0),
    //             d_date: row.get("d_date").unwrap_or_default(),
    //             d_open: row.get("d_open").unwrap_or(0.0),
    //             d_high: row.get("d_high").unwrap_or(0.0),
    //             d_low: row.get("d_low").unwrap_or(0.0),
    //             d_close: row.get("d_close").unwrap_or(0.0),
    //             d_length: row.get("d_length").unwrap_or(0.0),
    //             d_min_max: row.get("d_min_max").unwrap_or(0.0),
    //             trade_open: row.get("trade_open").unwrap_or_default(),
    //             trade_risk_exit_price: row.get("trade_risk_exit_price").unwrap_or(0.0),
    //             trade_reward_exit_price: row.get("trade_reward_exit_price").unwrap_or(0.0),
    //             trade_enter_price: row.get("trade_enter_price").unwrap_or(0.0),
    //             trade_current_price: row.get("trade_current_price").unwrap_or(0.0),
    //             trade_length: row.get("trade_length").unwrap_or(0.0),
    //             trade_pnl: row.get("trade_pnl").unwrap_or(0.0),
    //             trade_result: row.get("trade_result").unwrap_or_default(),
    //             trade_date: row.get("trade_date").unwrap_or_default(),
    //             trade_ab_price_retracement: row.get("trade_ab_price_retracement").unwrap_or(0.0),
    //             trade_bc_price_retracement: row.get("trade_bc_price_retracement").unwrap_or(0.0),
    //             trade_cd_bc_price_retracement: row.get("trade_cd_bc_price_retracement").unwrap_or(0.0),
    //             trade_cd_price_retracement: row.get("trade_cd_price_retracement").unwrap_or(0.0),
    //             trade_cd_xa_price_retracement: row.get("trade_cd_xa_price_retracement").unwrap_or(0.0),
    //             trade_bc_bar_retracement: row.get("trade_bc_bar_retracement").unwrap_or(0.0),
    //             trade_cd_bar_retracement: row.get("trade_cd_bar_retracement").unwrap_or(0.0),
    //             trade_snr: row.get("trade_snr").unwrap_or(0.0),
    //             trade_year: row.get("trade_year").unwrap_or(0.0),
    //             trade_month: row.get("trade_month").unwrap_or(0.0),
    //             trade_day: row.get("trade_day").unwrap_or(0.0),
    //             market: row.get("market").unwrap_or_default(),
    //             three_month: row.get("three_month").unwrap_or_default(),
    //             six_month: row.get("six_month").unwrap_or_default(),
    //             twelve_month: row.get("twelve_month").unwrap_or_default(),
    //             pattern_group_id: row.get("pattern_group_id").unwrap_or_default(),
    //             harmonic_type: row.get("harmonic_type").unwrap_or_default(),
    //         };
    //         patterns.push(pattern);
    //     }

    //     Ok(patterns)
    // }
}
