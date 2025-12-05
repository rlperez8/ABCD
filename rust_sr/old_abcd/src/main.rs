use std::time::Instant;
use old_abcd::db::Database;
use old_abcd::db::Candle;
use old_abcd::abcd_::ABCD;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();

    // Connect  
    let mut db = Database::new()?;

    let symbols = db.get_distinct_symbols()?;

    // // Patterns
    let patterns = ABCD::detect_patterns(&mut db)?;
    

    println!("Total patterns detected: {}", patterns.len());

    // // CSV
    ABCD::write_patterns_to_csv(&patterns, "../../server/patterns_all.csv")?;

    let duration = start.elapsed();
    println!("{:?}",  duration);

    Ok(())
}
