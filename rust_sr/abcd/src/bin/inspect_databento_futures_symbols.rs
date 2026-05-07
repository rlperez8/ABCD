use std::env;

use chrono::NaiveDateTime;
use sqlx::mysql::MySqlPool;
use sqlx::Row;

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

#[derive(Default)]
struct RootSummary {
    contracts: i64,
    first_available_ns: Option<i64>,
    last_available_ns: Option<i64>,
    first_symbol: Option<String>,
    last_symbol: Option<String>,
}

fn known_futures_root(symbol: &str) -> String {
    let uppercase = symbol.trim().to_uppercase();
    let clean_symbol = uppercase
        .split(['.', ' ', '_'])
        .next()
        .unwrap_or(uppercase.as_str());
    let known_roots = [
        "M6A", "M6B", "M6C", "M6E", "M6J", "M6S", "M6N", "MES", "MNQ", "MYM", "M2K", "MGC", "MCL",
        "MBT", "MET", "RTY", "EMD", "NKD", "6A", "6B", "6C", "6E", "6J", "6S", "6N", "ES", "NQ",
        "YM", "CL", "QM", "NG", "QG", "HO", "RB", "GC", "SI", "HG", "PL", "PA", "QI", "QO", "ZC",
        "ZW", "ZS", "ZM", "ZL", "HE", "LE", "GF", "ZN", "ZB",
    ];

    if let Some(root) = known_roots
        .iter()
        .find(|root| clean_symbol.starts_with(**root))
    {
        return (*root).to_string();
    }

    let month_codes = ['F', 'G', 'H', 'J', 'K', 'M', 'N', 'Q', 'U', 'V', 'X', 'Z'];
    let chars = clean_symbol.chars().collect::<Vec<_>>();
    if chars.len() >= 3
        && chars[chars.len() - 2].is_ascii_digit()
        && chars[chars.len() - 1].is_ascii_digit()
        && month_codes.contains(&chars[chars.len() - 3])
    {
        return chars[..chars.len() - 3].iter().collect();
    }
    if chars.len() >= 2
        && chars[chars.len() - 1].is_ascii_digit()
        && month_codes.contains(&chars[chars.len() - 2])
    {
        return chars[..chars.len() - 2].iter().collect();
    }

    clean_symbol
        .chars()
        .take_while(|ch| ch.is_ascii_alphabetic() || ch.is_ascii_digit())
        .collect()
}

fn metadata_root(raw_symbol: &str, asset: Option<&str>, underlying: Option<&str>) -> String {
    for candidate in [underlying, asset] {
        let Some(value) = candidate else {
            continue;
        };
        let value = value.trim();
        if !value.is_empty() {
            let root = known_futures_root(value);
            if !root.is_empty() {
                return root;
            }
        }
    }

    known_futures_root(raw_symbol)
}

fn format_ns(ns: Option<i64>) -> String {
    let Some(ns) = ns else {
        return String::new();
    };
    let seconds = ns.div_euclid(1_000_000_000);
    let nanos = ns.rem_euclid(1_000_000_000) as u32;

    NaiveDateTime::from_timestamp_opt(seconds, nanos)
        .map(|value| value.to_string())
        .unwrap_or_else(|| ns.to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url = database_url_from_env()?;
    let pool = MySqlPool::connect(&database_url).await?;
    let root = env::var("ABCD_FUTURES_ROOT")
        .ok()
        .map(|value| value.trim().to_uppercase())
        .filter(|value| !value.is_empty());

    println!("databento_futures_symbols columns");
    let columns = sqlx::query(
        r#"
        SELECT column_name, column_type
        FROM INFORMATION_SCHEMA.COLUMNS
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_NAME = 'databento_futures_symbols'
        ORDER BY ordinal_position
        "#,
    )
    .fetch_all(&pool)
    .await?;

    for row in columns {
        let column_name: String = row.try_get(0)?;
        let column_type: String = row.try_get(1)?;
        println!("{column_name}\t{column_type}");
    }

    let symbol_rows = sqlx::query(
        r#"
        SELECT
            raw_symbol,
            asset,
            underlying,
            activation_ns,
            expiration_ns
        FROM databento_futures_symbols
        WHERE security_type = 'FUT'
        ORDER BY raw_symbol
        "#,
    )
    .fetch_all(&pool)
    .await?;

    let mut summaries = std::collections::BTreeMap::<String, RootSummary>::new();
    let mut contracts_by_root =
        std::collections::BTreeMap::<String, Vec<(String, Option<i64>, Option<i64>)>>::new();

    for row in symbol_rows {
        let raw_symbol: String = row.try_get("raw_symbol")?;
        let asset: Option<String> = row.try_get("asset").ok();
        let underlying: Option<String> = row.try_get("underlying").ok();
        let activation_ns: Option<i64> = row.try_get("activation_ns").ok();
        let expiration_ns: Option<i64> = row.try_get("expiration_ns").ok();
        let root_symbol = metadata_root(&raw_symbol, asset.as_deref(), underlying.as_deref());

        let summary = summaries.entry(root_symbol.clone()).or_default();
        summary.contracts += 1;
        summary.first_available_ns = match (summary.first_available_ns, activation_ns) {
            (Some(current), Some(value)) => Some(current.min(value)),
            (None, Some(value)) => Some(value),
            (current, None) => current,
        };
        summary.last_available_ns = match (summary.last_available_ns, expiration_ns) {
            (Some(current), Some(value)) => Some(current.max(value)),
            (None, Some(value)) => Some(value),
            (current, None) => current,
        };
        summary.first_symbol = Some(
            summary
                .first_symbol
                .as_ref()
                .map(|current| current.min(&raw_symbol).clone())
                .unwrap_or_else(|| raw_symbol.clone()),
        );
        summary.last_symbol = Some(
            summary
                .last_symbol
                .as_ref()
                .map(|current| current.max(&raw_symbol).clone())
                .unwrap_or_else(|| raw_symbol.clone()),
        );
        contracts_by_root.entry(root_symbol).or_default().push((
            raw_symbol,
            activation_ns,
            expiration_ns,
        ));
    }

    println!();
    println!("root\tcontracts\tfirst_available\tlast_available\tfirst_symbol\tlast_symbol");

    for (root_symbol, summary) in summaries.iter() {
        if root
            .as_ref()
            .map(|wanted| wanted != root_symbol)
            .unwrap_or(false)
        {
            continue;
        }

        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            root_symbol,
            summary.contracts,
            format_ns(summary.first_available_ns),
            format_ns(summary.last_available_ns),
            summary.first_symbol.clone().unwrap_or_default(),
            summary.last_symbol.clone().unwrap_or_default()
        );
    }

    if let Some(root) = root.as_deref() {
        println!();
        println!("contracts for {root}");
        if let Some(contracts) = contracts_by_root.get(root) {
            for (symbol, activation_ns, expiration_ns) in contracts.iter().take(200) {
                println!(
                    "{}\t{}\t{}",
                    symbol,
                    format_ns(*activation_ns),
                    format_ns(*expiration_ns)
                );
            }
        }
    }

    Ok(())
}
