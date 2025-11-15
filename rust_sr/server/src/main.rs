
use actix_web::{post, get, web, App, HttpServer, Responder, HttpResponse};
use std::collections::HashMap;
use actix_cors::Cors;
use serde::{Serialize, Deserialize};

mod utils;
mod peformance;
use crate::utils::{load_patterns};
use crate::peformance::{YearlySummary, MonthlySummary};
mod pattern;
use crate::pattern::{Pattern};

#[derive(Serialize)]
pub struct MonthlyStat {
    pub year: i64,
    pub month: u32,
    pub total: usize,
    pub wins: usize,
    pub win_pct: f64,
}
#[derive(Serialize)]
struct PatternsResponse {
    patterns: Vec<Pattern>,
    monthly_stats: Vec<MonthlySummary>,
    yearly_summary: Vec<YearlySummary>,
}
#[derive(Deserialize, Debug)]
pub struct FilterParams {
    bc_greater: f64,
    bc_less: f64,
    cd_greater: Option<f64>,
    cd_less: Option<f64>,
}
#[post("/")]
async fn fetch_patterns(filter: web::Json<FilterParams>) -> impl Responder {
    // === Load CSV Data ===
    let patterns = load_patterns().await;

    // Extract parameters from the JSON body
    let bc_min = filter.bc_greater;
    let bc_max = filter.bc_less;
    let cd_min = filter.cd_greater.unwrap_or(0.0); 
    let cd_max = filter.cd_less.unwrap_or(f64::MAX);

    println!("{:?}", filter);

    // === Filter based on request parameters ===
    let patterns_2025: Vec<Pattern> = patterns
        .iter()
        .filter(|p| {
            p.trade_year == 2025.0 &&
            (bc_min..=bc_max).contains(&p.trade_bc_price_retracement) &&
            (cd_min..=cd_max).contains(&p.trade_cd_price_retracement)
        })
        .cloned()
        .collect();

    // === Monthly Summary ===
    let grouped_by_month = MonthlySummary::group_patterns_by_month(&patterns_2025);
    let monthly_stats = MonthlySummary::from_grouped_patterns(&grouped_by_month);

    // === Yearly Summary ===
    let grouped_by_year = YearlySummary::group_patterns_by_year(&patterns_2025);
    let yearly_summary = YearlySummary::get_yearly_summary(&grouped_by_year);

    // Return Pattern Data
    let response = PatternsResponse {
        patterns: patterns_2025,
        monthly_stats,
        yearly_summary,
    };

    HttpResponse::Ok().json(response)
}
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .wrap(
                Cors::default()
                    .allowed_origin("http://localhost:3000") // React frontend origin
                    .allowed_methods(vec!["GET", "POST"])
                    .allowed_headers(vec!["Content-Type"])
                    .max_age(3600),
            )
            .service(fetch_patterns)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}