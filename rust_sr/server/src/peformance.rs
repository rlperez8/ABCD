use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::pattern::Pattern;


// === Stuct ===
#[derive(Serialize, Clone)]
pub struct YearlySummary {
    year: i32,
    total: u32,
    wins: u32,
    win_pct: f64,
}
#[derive(Serialize, Clone)]
pub struct MonthlySummary {
    name: String,   
    month: u32,    
    closed: u32,
    wins: u32,
    win_pct: f64,
}

// === Trait ===
pub trait CalcWinPct {
    fn calc_win_pct(closed: usize, wins: usize) -> f64 {
        if closed > 0 {
            (wins as f64 / closed as f64) * 100.0
        } else {
            0.0
        }
    }
}

// === Impl ===
impl MonthlySummary {
    // Constructor
    pub fn new(name: &str, month: u32) -> Self {
        Self {
            name: name.to_string(),
            month,
            closed: 0,
            wins: 0,
            win_pct: 0.0,
        }
    }

    /// Generate all 12 months with zeroed statistics
    pub fn empty_monthly_summary() -> Vec<Self> {
        const MONTH_NAMES: [&str; 12] = [
            "January", "February", "March", "April", "May", "June",
            "July", "August", "September", "October", "November", "December",
        ];

        MONTH_NAMES.iter()
            .enumerate()
            .map(|(i, &name)| Self {
                name: name.to_string(),
                month: (i + 1) as u32,
                closed: 0,
                wins: 0,
                win_pct: 0.0,
            })
            .collect()
    }
    // Group patterns by month (take a slice instead of moving)
    pub fn group_patterns_by_month(patterns: &[Pattern]) -> HashMap<u32, Vec<Pattern>> {
        let mut grouped: HashMap<u32, Vec<Pattern>> = HashMap::new();
        for p in patterns {
            grouped.entry(p.trade_month as u32)
                .or_insert_with(Vec::new)
                .push(p.clone());
        }
        grouped
    }

    // Generate monthly summaries from grouped patterns
    pub fn from_grouped_patterns(grouped_by_month: &std::collections::HashMap<u32, Vec<Pattern>>) -> Vec<Self> {
        
        let mut monthly_stats = MonthlySummary::empty_monthly_summary();


        for (month, patterns) in grouped_by_month {
        let total = patterns.iter().filter(|p| p.trade_open == "false").count();
        let wins = patterns.iter().filter(|p| p.trade_result == "true").count();
        let win_pct = MonthlySummary::calc_win_pct(total, wins);

        // Find the existing MonthlySummary by month and update it
        if let Some(summary) = monthly_stats.iter_mut().find(|m| m.month == *month as u32) {
            summary.closed = total as u32;
            summary.wins = wins as u32;
            summary.win_pct = win_pct;
        } else {
            // Optional: if month not found, push a new one
            monthly_stats.push(MonthlySummary {
                name: MonthlySummary::month_name(*month),
                month: *month as u32,
                closed: total as u32,
                wins: wins as u32,
                win_pct,
            });
        }
    }

        monthly_stats
    }

    pub fn month_name(month: u32) -> String {
        match month {
            1 => "January",
            2 => "February",
            3 => "March",
            4 => "April",
            5 => "May",
            6 => "June",
            7 => "July",
            8 => "August",
            9 => "September",
            10 => "October",
            11 => "November",
            12 => "December",
            _ => "Unknown",
        }.to_string()
    }
}

impl YearlySummary {
    
    // Contructor
    pub fn new(year: i32, total: u32, wins: u32, win_pct: f64) -> Self {
        Self { year, total, wins, win_pct }
    }
    pub fn group_patterns_by_year(patterns: &[Pattern]) -> HashMap<i32, Vec<Pattern>> {
        let mut grouped = HashMap::new();
        for p in patterns {
            grouped.entry(p.trade_year as i32)
                .or_insert_with(Vec::new)
                .push(p.clone());
        }
        grouped
    }
    pub fn get_yearly_summary(grouped_by_year: &HashMap<i32, Vec<Pattern>>) -> Vec<Self> {
        let mut yearly_summary = Vec::new();

        for (year, patterns) in grouped_by_year {
            let closed: Vec<&Pattern> = patterns
                .iter()
                .filter(|p| p.trade_open == "false")
                .collect();

            let total = closed.len();
            let wins = closed.iter().filter(|p| p.trade_result == "true").count();
            let win_pct = MonthlySummary::calc_win_pct(total, wins);

            yearly_summary.push(YearlySummary::new(*year, total as u32, wins as u32, win_pct));
        }

        yearly_summary
    }
}

impl CalcWinPct for MonthlySummary {}

impl CalcWinPct for YearlySummary {}