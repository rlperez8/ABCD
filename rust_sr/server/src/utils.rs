
use csv::ReaderBuilder;
use std::error::Error;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::pattern::*;   

#[derive(Serialize, Clone)]
pub struct YearlySummary {
    year: i32,
    total: i32,
    wins: i32,
    win_pct: i32,
}


pub fn read_csv_top10_yext(path: &str) -> Result<Vec<Pattern>, Box<dyn Error>> {
   let file = std::fs::File::open(path)?;
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .from_reader(file);

    let mut all_patterns = Vec::new();
    for result in rdr.deserialize::<Pattern>() {
        let record: Pattern = result?;
        all_patterns.push(record);
    }

    Ok(all_patterns)
}

pub async fn load_patterns() -> Vec<Pattern> {

    tokio::task::spawn_blocking(|| {
        
        match read_csv_top10_yext("patterns_all.csv") {
            Ok(rows) => rows,
            Err(e) => {
                println!("Error reading CSV: {}", e);
                Vec::new()
            }
        }
    })
    .await
    .unwrap_or_default()
}

