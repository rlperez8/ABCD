
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
use actix_web::route;
use sqlx::MySqlPool;
use actix_web::dev::Service;
use crate::candles::Candle; 

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

// --- Main ---
// --- Main ---
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("🔹 Starting server...");

    // --- Connect to local DB ---
    let database_url = "mysql://rperezkc:Nar8uto!@localhost:3306/abcd";

    let pool = match MySqlPool::connect(database_url).await {
        Ok(pool) => {
            println!("✅ Connected to local DB");
            pool
        }
        Err(e) => {
            eprintln!("❌ Failed to connect to DB: {:?}", e);
            panic!();
        }
    };
    let pool = web::Data::new(pool); 

    // --- Start server ---
    HttpServer::new(move || {
        App::new()
            .app_data(pool.clone())
            .wrap(
                Cors::default()
                    .allowed_origin("http://localhost:3000")
                    .allowed_methods(vec!["GET", "POST"])
                    .allowed_headers(vec![actix_web::http::header::CONTENT_TYPE])
                    .max_age(3600)
            )
            .service(fetch_patterns)
            .service(fetch_candles)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

#[route("/patterns", method = "GET", method = "POST")]
async fn fetch_patterns(pool: web::Data<MySqlPool>,filter: web::Json<FilterParams>) -> impl Responder {

    println!("🔔 Handler called: fetch_patterns");
    // println!("📥 Received filter: {:?}", filter);

    println!("⏳ Fetching patterns from database...");

    let patterns: Vec<Pattern> = match sqlx::query_as::<_, Pattern>(
        "SELECT * FROM xabcd_patterns"
    )
    .fetch_all(pool.get_ref())
    .await
    {
        Ok(p) => {
            println!("✅ Successfully fetched {} patterns", p.len());
            p
        }
        Err(e) => {
            eprintln!("❌ Failed to fetch patterns: {}", e);
            return HttpResponse::InternalServerError()
                .body("Failed to fetch patterns");
        }
    };

     // === FILTER PATTERNS ===
    let patterns_2025: Vec<Pattern> = patterns
        .iter()
        // .filter(|p| p.trade_year == 2026.0 )
        // .filter(|p| p.symbol == "ADPT")
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
        patterns: patterns.clone(),
        monthly_stats,
        yearly_summary,
    };

    println!("📊 Returning {} patterns", patterns.len());
    // println!("📊 Returning {:?} patterns", patterns);

    HttpResponse::Ok().json(response)
}

#[route("/candles", method = "GET", method = "POST")]
async fn fetch_candles(pool: web::Data<MySqlPool>,params: web::Json<Params>) -> impl Responder {

    println!("🔔 Handler called: fetch_candles");
    println!("{:?}", params);

    let candles: Vec<Candle> = match sqlx::query_as::<_, Candle>(
        "SELECT * FROM candles 
        WHERE symbol =  ?"
    )
    .bind(params.symbol.clone())
    .fetch_all(pool.get_ref())
    .await
    {
        Ok(c) => {
            println!("✅ Successfully fetched {} candles", c.len());
            c
        }
        Err(e) => {
            eprintln!("❌ Failed to fetch candles: {}", e);
            return HttpResponse::InternalServerError()
                .body("Failed to fetch candles");
        }
    };

    // let candles = match pool.get_stored_candles(&params.symbol) {
    //     Ok(c) => c,
    //     Err(e) => {
    //         println!("Failed to fetch candles: {}", e);
    //         return HttpResponse::InternalServerError().body("Failed to fetch candles");
    //     }
    // };

    // println!("{:?}", candles);
    HttpResponse::Ok().json(candles)
}































    // // === CREATE DATABASE POOL ONCE ===
    // let db = match Database::new_azure() {
    //     Ok(db) => {
    //         println!("✅ Successfully connected to Azure database!");
    //         web::Data::new(db)
    //     },
    //     Err(e) => {
    //         eprintln!("❌ Failed to create database pool: {}", e);
    //         std::process::exit(1);
    //     }
    // };


// #[actix_web::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {

//     let database_url = "mysql://rperezkc:Nar8uto!@localhost:3306/abcd";
//     let pool = MySqlPool::connect(database_url).await?;
//     let pool = web::Data::new(pool);

//     HttpServer::new(move || {
//     App::new()
//         .wrap(Logger::default())  // built-in Actix logs
//         .wrap_fn(|req, srv| {     // <-- ADD THIS
//             println!("🔔 Incoming request: {} {}", req.method(), req.path());
//             let fut = srv.call(req);
//             async move { fut.await }
//         })
//         .app_data(pool.clone())
//         .wrap(
//             Cors::default()
//                 .allowed_origin("http://localhost:3000")
//                 .allowed_origin("https://abcd-finder.vercel.app")
//                 .allowed_methods(vec!["GET", "POST", "OPTIONS"])
//                 .allowed_headers(vec![
//                     actix_web::http::header::CONTENT_TYPE,
//                     actix_web::http::header::ACCEPT,
//                 ])
//                 .allow_any_header()
//                 .supports_credentials()
//                 .max_age(3600),
//         )
//         .service(fetch_patterns)
//         .service(fetch_candles)
// })
//     .bind(("0.0.0.0", 8080))?
//     .run()
//     .await?;  // <-- note the `?` here

//     Ok(())
// }

// #[route("/candles", method = "GET", method = "POST")]
// async fn fetch_candles(
//     db: web::Data<Database>,
//     params: web::Json<Params>,
//     ) -> impl Responder {

//     println!("{:?}", params);

//     let candles = match db.get_stored_candles(&params.symbol) {
//         Ok(c) => c,
//         Err(e) => {
//             println!("Failed to fetch candles: {}", e);
//             return HttpResponse::InternalServerError().body("Failed to fetch candles");
//         }
//     };

//     HttpResponse::Ok().json(candles)
// }

// #[route("/patterns",  method = "POST")]
// async fn fetch_patterns( pool: web::Data<Database>, filter: web::Json<FilterParams>) -> impl Responder {

//     println!("📥 Filter params: {:?}", filter);

//     // === FETCH PATTERNS FROM DB USING POOL ===
//     println!("⏳ Fetching patterns from database...");
//     // let patterns = match pool.fetch_all() {
//     //     Ok(p) => {
//     //         println!("✅ Successfully fetched {} patterns", p.len());
//     //         p
//     //     },
//     //     Err(e) => {
//     //         eprintln!("❌ Failed to fetch patterns: {}", e);
//     //         return HttpResponse::InternalServerError().body("Failed to fetch patterns");
//     //     }
//     // };

//     // // === FILTER PATTERNS ===
//     // let patterns_2025: Vec<Pattern> = patterns
//     //     .iter()
//     //     // .filter(|p| p.trade_year == 2026.0 )
//     //     // .filter(|p| p.symbol == "ADPT")
//     //     .cloned()
//     //     .collect();
//     // println!("📊 Filtered down to {} patterns for 2026", patterns_2025.len());

//     // // === GET MONTHLY SUMMARY ===
//     // let grouped_by_month = MonthlySummary::group_patterns_by_month(&patterns_2025);
//     // let monthly_stats = MonthlySummary::from_grouped_patterns(&grouped_by_month);

//     // // === GET YEARLY SUMMARY ===
//     // let grouped_by_year = YearlySummary::group_patterns_by_year(&patterns_2025);
//     // let yearly_summary = YearlySummary::get_yearly_summary(&grouped_by_year);

//     // === RETURN RESPONSE ===
//     let response = PatternsResponse {
//     patterns: Vec::new(),
//     monthly_stats: Vec::new(),
//     yearly_summary: Vec::new(),
// };

//     println!("✅ Finished preparing PatternsResponse");
//     HttpResponse::Ok().json(response)
// }

































// #[route("/patterns",  method = "POST")]
// async fn fetch_patterns( pool: web::Data<Database>, filter: web::Json<FilterParams>) -> impl Responder {

//     println!("📥 Filter params: {:?}", filter);

//     // === FETCH PATTERNS FROM DB USING POOL ===
//     println!("⏳ Fetching patterns from database...");
//     // let patterns = match pool.fetch_all() {
//     //     Ok(p) => {
//     //         println!("✅ Successfully fetched {} patterns", p.len());
//     //         p
//     //     },
//     //     Err(e) => {
//     //         eprintln!("❌ Failed to fetch patterns: {}", e);
//     //         return HttpResponse::InternalServerError().body("Failed to fetch patterns");
//     //     }
//     // };

//     // // === FILTER PATTERNS ===
//     // let patterns_2025: Vec<Pattern> = patterns
//     //     .iter()
//     //     // .filter(|p| p.trade_year == 2026.0 )
//     //     // .filter(|p| p.symbol == "ADPT")
//     //     .cloned()
//     //     .collect();
//     // println!("📊 Filtered down to {} patterns for 2026", patterns_2025.len());

//     // // === GET MONTHLY SUMMARY ===
//     // let grouped_by_month = MonthlySummary::group_patterns_by_month(&patterns_2025);
//     // let monthly_stats = MonthlySummary::from_grouped_patterns(&grouped_by_month);

//     // // === GET YEARLY SUMMARY ===
//     // let grouped_by_year = YearlySummary::group_patterns_by_year(&patterns_2025);
//     // let yearly_summary = YearlySummary::get_yearly_summary(&grouped_by_year);

//     // === RETURN RESPONSE ===
//     let response = PatternsResponse {
//     patterns: Vec::new(),
//     monthly_stats: Vec::new(),
//     yearly_summary: Vec::new(),
// };

//     println!("✅ Finished preparing PatternsResponse");
//     HttpResponse::Ok().json(response)
// }
