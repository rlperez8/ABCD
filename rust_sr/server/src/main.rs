
use actix_web::{post, get, web, App, HttpServer, Responder, HttpResponse};
use std::collections::HashMap;
use actix_cors::Cors;
use serde::{Serialize, Deserialize};
use chrono::NaiveDate;
mod utils;
mod peformance;
use crate::utils::{load_patterns};
use crate::peformance::{YearlySummary, MonthlySummary};
mod pattern;
use crate::pattern::{Pattern};
use actix_web::middleware::Logger;
mod models; 
use crate::models::*;


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
    market: String
}

#[derive(Deserialize, Debug)]
pub struct Params {
    symbol: String,
}


// #[actix_web::main]
// async fn main() -> std::io::Result<()> {
//     HttpServer::new(|| {
//         App::new()
//             .wrap(
//                 Cors::default()
//                     .allowed_origin("http://localhost:3000") // React frontend origin
//                     .allowed_methods(vec!["GET", "POST"])
//                     .allowed_headers(vec!["Content-Type"])
//                     .max_age(3600),
//             )
//             .service(fetch_patterns)
//             .service(fetch_candles)
            
//     })
//     .bind(("127.0.0.1", 8080))?
//     .run()
//     .await
// }

// #[post("/")]
// async fn fetch_patterns(filter: web::Json<FilterParams>) -> impl Responder {

//     // === CONNECT TO DB ===
//     let mut db = match Database::new_azure() {
//         Ok(db) => db,
//         Err(e) => {
//             println!("Database connection failed: {}", e);
//             return HttpResponse::InternalServerError().body("Database connection error");
//         }
//     };

//     // === FETCH PATTERNS ===
//     let patterns = match Database::fetch_all(&mut db.conn) {
//         Ok(p) => p,
//         Err(e) => {
//             println!("Failed to fetch patterns: {}", e);
//             return HttpResponse::InternalServerError().body("Failed to fetch patterns");
//         }
//     };

//     // === FILTER PATTERNS ===
//     let patterns_2025: Vec<Pattern> = patterns
//         .iter()
//         .filter(|p| {
//             p.trade_year == 2026.0 
//         })
//         .cloned()
//         .collect();


//     // === GET MONTHLY SUMMARY ===
//     let grouped_by_month = MonthlySummary::group_patterns_by_month(&patterns_2025);
//     let monthly_stats = MonthlySummary::from_grouped_patterns(&grouped_by_month);

//     // === GET YEARLY SUMMARY ===
//     let grouped_by_year = YearlySummary::group_patterns_by_year(&patterns_2025);
//     let yearly_summary = YearlySummary::get_yearly_summary(&grouped_by_year);

//     // === RETURN RESPONSE ===
//     let response = PatternsResponse {
//         patterns: patterns_2025,
//         monthly_stats,
//         yearly_summary,
//     };

//     HttpResponse::Ok().json(response)
// }

// #[post("/candles")]
// async fn fetch_candles(params: web::Json<Params>) -> impl Responder {

//     println!("{:?}", params);

//     // === CONNECT TO DB ===
//     let mut db = match Database::new_azure() {
//         Ok(db) => db,
//         Err(e) => {
//             println!("Database connection failed: {}", e);
//             return HttpResponse::InternalServerError().body("Database connection error");
//         }
//     };

//     // === FETCH CANDLES ===
//     let candles = match Database::get_stored_candles(&params.symbol) {
//         Ok(p) => p,
//         Err(e) => {
//             println!("Failed to fetch patterns: {}", e);
//             return HttpResponse::InternalServerError().body("Failed to fetch patterns");
//         }
//     };

//     // println!("{:?}", candles);

//     // === RETURN RESPONSE ===
//     HttpResponse::Ok().json(candles)
// }

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // === CREATE DATABASE POOL ONCE ===
    let db = match Database::new_azure() {
        Ok(db) => {
            println!("✅ Successfully connected to Azure database!");
            web::Data::new(db)
        },
        Err(e) => {
            eprintln!("❌ Failed to create database pool: {}", e);
            std::process::exit(1);
        }
    };

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .app_data(db.clone()) // share the pool with all handlers
            .wrap(
                Cors::default()
                    .allowed_origin("http://localhost:3000")
                    .allowed_origin("https://abcd-finder.vercel.app/")
                    .allowed_methods(vec!["GET", "POST"])
                    .allowed_headers(vec!["Content-Type"])
                    .max_age(3600),
            )
            .service(fetch_patterns)
            .service(fetch_candles)
    })
    // .bind(("127.0.0.1", 8080))?
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

#[post("/candles")]
async fn fetch_candles(
    db: web::Data<Database>,
    params: web::Json<Params>,
    ) -> impl Responder {

    println!("{:?}", params);

    let candles = match db.get_stored_candles(&params.symbol) {
        Ok(c) => c,
        Err(e) => {
            println!("Failed to fetch candles: {}", e);
            return HttpResponse::InternalServerError().body("Failed to fetch candles");
        }
    };

    HttpResponse::Ok().json(candles)
}

#[post("/patterns")]
async fn fetch_patterns(
    db: web::Data<Database>,
    filter: web::Json<FilterParams>,
) -> impl Responder {

    println!("📥 Filter params: {:?}", filter);

    // === FETCH PATTERNS FROM DB USING POOL ===
    println!("⏳ Fetching patterns from database...");
    let patterns = match db.fetch_all() {
        Ok(p) => {
            println!("✅ Successfully fetched {} patterns", p.len());
            p
        },
        Err(e) => {
            eprintln!("❌ Failed to fetch patterns: {}", e);
            return HttpResponse::InternalServerError().body("Failed to fetch patterns");
        }
    };

    // === FILTER PATTERNS ===
    let patterns_2025: Vec<Pattern> = patterns
        .iter()
        .filter(|p| p.trade_year == 2026.0)
        .cloned()
        .collect();
    println!("📊 Filtered down to {} patterns for 2026", patterns_2025.len());

    // === GET MONTHLY SUMMARY ===
    let grouped_by_month = MonthlySummary::group_patterns_by_month(&patterns_2025);
    let monthly_stats = MonthlySummary::from_grouped_patterns(&grouped_by_month);

    // === GET YEARLY SUMMARY ===
    let grouped_by_year = YearlySummary::group_patterns_by_year(&patterns_2025);
    let yearly_summary = YearlySummary::get_yearly_summary(&grouped_by_year);

    // === RETURN RESPONSE ===
    let response = PatternsResponse {
        patterns: patterns_2025,
        monthly_stats,
        yearly_summary,
    };

    println!("✅ Finished preparing PatternsResponse");
    HttpResponse::Ok().json(response)
}