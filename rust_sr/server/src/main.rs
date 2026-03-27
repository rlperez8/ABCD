
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

#[derive(Debug, serde::Deserialize)]
struct FilterParams {
    pub bin: String,
    pub harmonic_type: String,
}

#[derive(Deserialize, Debug)]
pub struct Params {
    symbol: String,
    
}

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
            .service(fetch_accuracy)
            .service(fetch_candles)
            .service(fetch_patterns)
             .wrap(Logger::default())  // built-in Actix logs
             .wrap_fn(|req, srv| {     // <-- ADD THIS
                 println!("🔔 Incoming request: {} {}", req.method(), req.path());
                 let fut = srv.call(req);
                 async move { fut.await }
             })
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

#[route("/patterns", method = "GET", method = "POST")]
async fn fetch_patterns(
    pool: web::Data<MySqlPool>,
    params: web::Json<FilterParams>
) -> impl Responder {

    println!("🔔 Handler called: fetch_patterns");
    println!("📥 Received filter: {:?}", params);

    let (min, max) = match params.bin.split_once('-') {
        Some((min, max)) => (
            min.parse::<f64>().unwrap_or(0.0),
            max.parse::<f64>().unwrap_or(100.0),
        ),
        None => (0.0, 100.0),
    };

    let column = format!("{}_accuracy", params.harmonic_type);

    let query = format!(
        r#"
        SELECT *
        FROM xabcd_patterns
        WHERE {} > ?
        AND {} <= ?
        "#,
        column, column
    );

    let patterns: Vec<Pattern> = match sqlx::query_as::<_, Pattern>(&query)
        .bind(min)
        .bind(max)
        .fetch_all(pool.get_ref())
        .await
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("DB error: {:?}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };
    

    println!("📊 Found {} patterns", patterns.len());

    // === SUMMARY ===
    let grouped_by_month = MonthlySummary::group_patterns_by_month(&patterns);
    let monthly_stats = MonthlySummary::from_grouped_patterns(&grouped_by_month);

    let grouped_by_year = YearlySummary::group_patterns_by_year(&patterns);
    let yearly_summary = YearlySummary::get_yearly_summary(&grouped_by_year);

    let response = PatternsResponse {
        patterns,
        monthly_stats,
        yearly_summary,
    };

    HttpResponse::Ok().json(response)
}





#[route("/candles", method = "GET", method = "POST")]
async fn fetch_candles(pool: web::Data<MySqlPool>,params: web::Json<Params>) -> impl Responder {

    // println!("🔔 Handler called: fetch_candles");
    // println!("{:?}", params);

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

// #[route("/candles", method = "GET", method = "POST")]
// async fn fetch_accuracy(pool: web::Data<MySqlPool>,params: web::Json<Params>) -> impl Responder {
//     println!("🔥 HIT");
//     HttpResponse::Ok().body("works")
// }

#[derive(sqlx::FromRow, serde::Serialize)]
struct AccuracyBin {
    harmonic_type: String,
    bin: String,
    count: i32,
    avg_return: f64,
}

#[route("accuracy", method = "GET", method = "POST")]
async fn fetch_accuracy(pool: web::Data<MySqlPool>,params: web::Json<Params>) -> impl Responder {
    println!("🔔 Handler called: fetch_accuracy");
    let sql = r#"
        WITH bins AS (
            SELECT 1 AS bin_order, '0-10' AS bin UNION ALL
            SELECT 2, '10-20' UNION ALL
            SELECT 3, '20-30' UNION ALL
            SELECT 4, '30-40' UNION ALL
            SELECT 5, '40-50' UNION ALL
            SELECT 6, '50-60' UNION ALL
            SELECT 7, '60-70' UNION ALL
            SELECT 8, '70-80' UNION ALL
            SELECT 9, '80-90' UNION ALL
            SELECT 10, '90-100'
        ),
        agg AS (
            -- 🔹 your original query goes here
            SELECT
                harmonic_type,
                CASE
                    WHEN accuracy <= 10 THEN '0-10'
                    WHEN accuracy <= 20 THEN '10-20'
                    WHEN accuracy <= 30 THEN '20-30'
                    WHEN accuracy <= 40 THEN '30-40'
                    WHEN accuracy <= 50 THEN '40-50'
                    WHEN accuracy <= 60 THEN '50-60'
                    WHEN accuracy <= 70 THEN '60-70'
                    WHEN accuracy <= 80 THEN '70-80'
                    WHEN accuracy <= 90 THEN '80-90'
                    ELSE '90-100'
                END AS bin,
                COUNT(*) AS count,
                AVG(return_pct) AS avg_return
            FROM accuracies
            WHERE accuracy IS NOT NULL
            GROUP BY harmonic_type, bin
        ),
        types AS (
            SELECT DISTINCT harmonic_type FROM accuracies
        )

        SELECT
            t.harmonic_type,
            b.bin,
            COALESCE(a.count, 0) AS count,
            COALESCE(a.avg_return, 0) AS avg_return
        FROM types t
        CROSS JOIN bins b
        LEFT JOIN agg a
            ON a.harmonic_type = t.harmonic_type
            AND a.bin = b.bin
        ORDER BY t.harmonic_type, b.bin_order;
    "#;
    // println!("SQL:\n{}", sql);

    // SQLx automatically maps each row into AccuracyBin
    let rows: Vec<AccuracyBin> = match sqlx::query_as::<_, AccuracyBin>(sql)
        .fetch_all(pool.get_ref())
        .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("❌ DB error: {:?}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    println!("✅ Fetched {} aggregated bins", rows.len());

    HttpResponse::Ok().json(rows)
}


































