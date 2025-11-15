
use std::time::Instant;

use tools::db_utils::Database;
use abcd::ABCD;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();

    // Connect  
    let mut db = Database::new()?;

    // Patterns
    let patterns = ABCD::detect_patterns(&mut db)?;
    println!("Total patterns detected: {}", patterns.len());

    // CSV
    ABCD::write_patterns_to_csv(&patterns, "../server/patterns_all.csv")?;

    let duration = start.elapsed();
    println!("{:?}",  duration);

 
    Ok(())
}
