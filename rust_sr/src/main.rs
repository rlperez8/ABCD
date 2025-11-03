use mysql::*;
use mysql::prelude::*;
use chrono::NaiveDate;
use chrono::Datelike;
use chrono::Local;

#[derive(Debug)]
struct Candle {
    symbol: String,
    date: NaiveDate,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: u64,
}
struct SRLine {
    price: f64,
    score: f64,
    lower: f64,
    upper: f64,
}
// --- Fetch all distinct symbols ---
fn get_distinct_symbols(conn: &mut PooledConn) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let symbols: Vec<String> = conn.query_map(
        "SELECT DISTINCT symbol FROM abcd.candles",
        |symbol: String| symbol,
    )?;
    Ok(symbols)
}
// --- Fetch candles for a symbol in the current year ---
fn get_stored_candles(symbol: &str, conn: &mut PooledConn) -> Result<Vec<Candle>, Box<dyn std::error::Error>> {
    let current_year = Local::now().year();

    let query = r#"
        SELECT symbol,
               DATE_FORMAT(date, '%Y-%m-%d') AS date_str,
               open, high, low, close, volume
        FROM abcd.candles
        WHERE symbol = :symbol
   
    "#;

    let params = params! {
        "symbol" => symbol,
       
    };

    let candles: Vec<Candle> = conn.exec_map(
        query,
        params,
        |(symbol, date_str, open, high, low, close, volume): (String, String, f64, f64, f64, f64, u64)| {
            let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .expect("Failed to parse date");

            Candle {
                symbol,
                date,
                open,
                high,
                low,
                close,
                volume,
            }
        },
    )?;

    Ok(candles)
}
// --- Support/Resistance calculation in Rust ---
fn get_support_resistance(candles: &[Candle]) -> Vec<SRLine> {
    let decay_per_tick = 0.01;       // points lost per tick away
    let range_pct = 0.05;            // ±5% range for top SR selection
    let reaction_tolerance = 0.01;   // how close candle must touch/reverse

    // --- Adaptive tick levels ---
    let min_price = candles.iter()
        .flat_map(|c| vec![c.open, c.high, c.low, c.close])
        .fold(f64::INFINITY, |a, b| a.min(b));
    let max_price = candles.iter()
        .flat_map(|c| vec![c.open, c.high, c.low, c.close])
        .fold(f64::NEG_INFINITY, |a, b| a.max(b));
    let price_range = max_price - min_price;

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
                if candle.close > candle.low {
                    let tick_dist = (tick - candle.low).abs() / tick_interval;
                    scores[i] += (1.0 - tick_dist * decay_per_tick).max(0.0);
                }
            }
        }
    }

    // --- Pick top SR lines with ±5% removal ---
    let mut sr_lines = vec![];
    let mut remaining: Vec<(f64, f64)> = ticks.iter().copied().zip(scores.iter().copied()).collect();

    for _ in 0..1 {
        if remaining.is_empty() {
            break;
        }
        let (top_idx, top) = remaining.iter().enumerate()
            .max_by(|a, b| a.1.1.partial_cmp(&b.1.1).unwrap())
            .unwrap();

        let price = top.0;
        let score = top.1;

        sr_lines.push(SRLine {
            price,
            score,
            lower: price * 0.95,
            upper: price * 1.05,
        });

        let lower = price * (1.0 - range_pct);
        let upper = price * (1.0 + range_pct);
        remaining.retain(|&(p, _)| p < lower || p > upper);
    }

    sr_lines
}
fn insert_sr_lines(
    conn: &mut PooledConn,
    symbol: &str,
    sr_lines: &[SRLine],
) -> Result<(), Box<dyn std::error::Error>> {
    let insert_query = r#"
        INSERT INTO support_and_resistance (symbol, price, score, lower, upper)
        VALUES (:symbol, :price, :score, :lower, :upper)
    "#;

    for line in sr_lines {
        let params = params! {
            "symbol" => symbol,
            "price" => line.price,
            "score" => line.score,
            "lower" => line.lower,
            "upper" => line.upper,
        };
        conn.exec_drop(insert_query, params)?;
    }

    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "mysql://rperezkc:Nar8uto!@localhost:3306/abcd";
    let pool = Pool::new(url)?;
    let mut conn = pool.get_conn()?;

    let symbols = get_distinct_symbols(&mut conn)?;
    // // println!("First symbol: {:?}", symbols.get(0));
    // if let Some(first_symbol) = symbols.get(0) {
    //     let candles = get_stored_candles(first_symbol, &mut conn)?;
    //     println!("Fetched {} candles for {}", candles.len(), first_symbol);

    //     // Calculate support/resistance
    //     let sr_lines = get_support_resistance(&candles);

    //     // Insert into DB
    //     insert_sr_lines(&mut conn, first_symbol, &sr_lines)?;
    //     println!("Inserted {} SR lines for {}", sr_lines.len(), first_symbol);
    // } else {
    //     println!("No symbols found");
    // }
    for symbol in symbols {
    let candles = get_stored_candles(&symbol, &mut conn)?;

    if !candles.is_empty() {
        let sr_lines = get_support_resistance(&candles);
        insert_sr_lines(&mut conn, &symbol, &sr_lines)?;
        println!("Inserted {} SR lines for {}", sr_lines.len(), symbol);
    }
}
    println!("Done!");


    Ok(())
}
