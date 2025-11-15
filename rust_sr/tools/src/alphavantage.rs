// use mysql::*;
// use mysql::prelude::*;


// pub struct Database {
//     pub conn: PooledConn,
// }

// impl Database {
//     // Constructor 
//     pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
//         // Connect to db
//         let url = "mysql://rperezkc:Nar8uto!@localhost:3306/abcd";
//         let pool = Pool::new(url)?; // create connection pool
//         let conn = pool.get_conn()?; // get a connection from the pool

//         Ok(Self { conn }) // return the struct
//     }

//     pub fn get_distinct_symbols(@mut self) -> mysql::Result<Vec<String>> {
//     let query = r#"
//         SELECT DISTINCT symbol
//         FROM abcd.candles
//         ORDER BY symbol
//     "#;

//     let symbols: Vec<String> = conn.query_map(query, |symbol: String| symbol)?;
//     Ok(symbols)
// }
// }



// fn get_stored_candles(symbol: &str, conn: &mut PooledConn) -> mysql::Result<Vec<Candle>> {

//     let query = r#"
//         SELECT symbol,
//                DATE_FORMAT(date, '%Y-%m-%d') AS date_str,
//                open, high, low, close, volume
//         FROM abcd.candles
//         WHERE symbol = :symbol
//         ORDER BY date DESC
//         LIMIT 365
      
//     "#;

//     let params = params! { "symbol" => symbol };

//     let candles: Vec<Candle> = conn.exec_map(
//         query,
//         params,
//         |(symbol, date_str, open, high, low, close, volume): (String, String, f64, f64, f64, f64, u64)| {
//             let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
//                 .expect("Failed to parse date");
//             Candle { symbol, date, open, high, low, close, volume }
//         },
//     )?;

//     Ok(candles)
// }