use std::env;

use chrono::NaiveDateTime;
use sqlx::{mysql::MySqlPool, QueryBuilder, Row};

#[derive(Clone, Debug)]
struct Args {
    symbol: Option<String>,
    all_symbols: bool,
    source_table: Option<String>,
    timeframe: Option<String>,
    year: Option<i64>,
    list_top: Option<i64>,
    limit: Option<i64>,
    d_window_minutes: i64,
    wide_d_window_minutes: i64,
    anchor_window_minutes: i64,
    price_tolerance_ratio: f64,
    apply: bool,
}

#[derive(Clone, Debug)]
struct PatternSetup {
    id: u64,
    setup_id: String,
    symbol: String,
    root_symbol: String,
    source_table: String,
    source_timeframe: String,
    market: String,
    harmonic_type: String,
    pattern_family_key: Option<String>,
    x_date: NaiveDateTime,
    x_price: f64,
    a_date: NaiveDateTime,
    a_price: f64,
    b_date: NaiveDateTime,
    b_price: f64,
    c_date: NaiveDateTime,
    c_price: f64,
    d_date: NaiveDateTime,
    d_confirm_date: NaiveDateTime,
    d_price: f64,
    xa_price_length: f64,
    ab_price_length: f64,
    bc_price_length: f64,
    cd_price_length: f64,
    full_pattern_length: i64,
}

#[derive(Clone, Debug)]
struct EventCluster {
    members: Vec<usize>,
    first_d_confirm_date: NaiveDateTime,
    last_d_confirm_date: NaiveDateTime,
}

#[derive(Clone, Debug)]
struct Assignment {
    setup_index: usize,
    event_id: String,
    event_rank: i64,
    is_primary: bool,
    sister_count: i64,
    similarity_score: f64,
}

#[derive(Clone, Debug)]
struct MatchResult {
    is_match: bool,
    score: f64,
}

#[derive(Clone, Debug)]
struct SymbolGroup {
    symbol: String,
    source_table: Option<String>,
    source_timeframe: Option<String>,
    setup_count: i64,
}

fn usage() -> &'static str {
    "Usage:\n  cargo run --bin assign_pattern_events -- --list-top 20 [--year 2026]\n  cargo run --bin assign_pattern_events -- --symbol ESM6 [--year 2026] [--source-table futures_contract_1m_candles] [--timeframe 1m] [--apply]\n  cargo run --bin assign_pattern_events -- --all-symbols [--year 2026] [--source-table futures_contract_1m_candles] [--timeframe 1m] [--apply]\n\nAssigns twin/event IDs to pattern_setups for one contract symbol at a time. Without --apply it only prints a dry run."
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|items| items[0] == name)
        .map(|items| items[1].clone())
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let raw_args = env::args().skip(1).collect::<Vec<_>>();
    if raw_args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", usage());
        std::process::exit(0);
    }

    Ok(Args {
        symbol: arg_value(&raw_args, "--symbol")
            .or_else(|| arg_value(&raw_args, "--contract"))
            .map(|value| value.trim().to_uppercase())
            .filter(|value| !value.is_empty()),
        all_symbols: raw_args.iter().any(|arg| arg == "--all-symbols"),
        source_table: arg_value(&raw_args, "--source-table")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        timeframe: arg_value(&raw_args, "--timeframe")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        year: arg_value(&raw_args, "--year")
            .map(|value| value.parse::<i64>())
            .transpose()?,
        list_top: arg_value(&raw_args, "--list-top")
            .map(|value| value.parse::<i64>())
            .transpose()?,
        limit: arg_value(&raw_args, "--limit")
            .map(|value| value.parse::<i64>())
            .transpose()?,
        d_window_minutes: arg_value(&raw_args, "--d-window-minutes")
            .map(|value| value.parse::<i64>())
            .transpose()?
            .unwrap_or(15)
            .max(1),
        wide_d_window_minutes: arg_value(&raw_args, "--wide-d-window-minutes")
            .map(|value| value.parse::<i64>())
            .transpose()?
            .unwrap_or(60)
            .max(1),
        anchor_window_minutes: arg_value(&raw_args, "--anchor-window-minutes")
            .map(|value| value.parse::<i64>())
            .transpose()?
            .unwrap_or(240)
            .max(1),
        price_tolerance_ratio: arg_value(&raw_args, "--price-tolerance-ratio")
            .map(|value| value.parse::<f64>())
            .transpose()?
            .unwrap_or(0.08)
            .max(0.000001),
        apply: raw_args.iter().any(|arg| arg == "--apply"),
    })
}

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

fn minutes_between(left: NaiveDateTime, right: NaiveDateTime) -> i64 {
    left.signed_duration_since(right).num_minutes().abs()
}

fn clean_symbol(value: &str) -> String {
    value.trim().to_uppercase()
}

fn price_scale(left: &PatternSetup, right: &PatternSetup) -> f64 {
    [
        left.xa_price_length.abs(),
        left.ab_price_length.abs(),
        left.bc_price_length.abs(),
        left.cd_price_length.abs(),
        right.xa_price_length.abs(),
        right.ab_price_length.abs(),
        right.bc_price_length.abs(),
        right.cd_price_length.abs(),
    ]
    .into_iter()
    .fold(0.000001, f64::max)
}

fn price_match(left: f64, right: f64, tolerance: f64) -> bool {
    (left - right).abs() <= tolerance
}

fn closeness(diff: f64, max_diff: f64) -> f64 {
    if max_diff <= 0.0 {
        return 0.0;
    }
    (1.0 - (diff / max_diff)).clamp(0.0, 1.0)
}

fn pivot_match_score(
    left_price: f64,
    right_price: f64,
    left_date: NaiveDateTime,
    right_date: NaiveDateTime,
    price_tolerance: f64,
    time_window_minutes: i64,
) -> (bool, f64) {
    let price_diff = (left_price - right_price).abs();
    let time_diff = minutes_between(left_date, right_date);
    let matched = price_diff <= price_tolerance && time_diff <= time_window_minutes;
    let price_score = closeness(price_diff, price_tolerance);
    let time_score = closeness(time_diff as f64, time_window_minutes as f64);
    (matched, (price_score * 0.65) + (time_score * 0.35))
}

fn is_sister_pattern(left: &PatternSetup, right: &PatternSetup, args: &Args) -> MatchResult {
    if clean_symbol(&left.symbol) != clean_symbol(&right.symbol) {
        return MatchResult {
            is_match: false,
            score: 0.0,
        };
    }
    if !left.market.eq_ignore_ascii_case(&right.market) {
        return MatchResult {
            is_match: false,
            score: 0.0,
        };
    }
    if !left.source_table.eq_ignore_ascii_case(&right.source_table)
        || !left
            .source_timeframe
            .eq_ignore_ascii_case(&right.source_timeframe)
    {
        return MatchResult {
            is_match: false,
            score: 0.0,
        };
    }

    let tolerance = price_scale(left, right) * args.price_tolerance_ratio;
    let d_confirm_diff = minutes_between(left.d_confirm_date, right.d_confirm_date);
    let d_date_diff = minutes_between(left.d_date, right.d_date);
    let pattern_minutes = left
        .full_pattern_length
        .max(right.full_pattern_length)
        .saturating_mul(2)
        .clamp(args.d_window_minutes, args.anchor_window_minutes);
    let anchor_window = pattern_minutes.max(args.d_window_minutes);

    let pivots = [
        pivot_match_score(
            left.x_price,
            right.x_price,
            left.x_date,
            right.x_date,
            tolerance,
            anchor_window,
        ),
        pivot_match_score(
            left.a_price,
            right.a_price,
            left.a_date,
            right.a_date,
            tolerance,
            anchor_window,
        ),
        pivot_match_score(
            left.b_price,
            right.b_price,
            left.b_date,
            right.b_date,
            tolerance,
            anchor_window,
        ),
        pivot_match_score(
            left.c_price,
            right.c_price,
            left.c_date,
            right.c_date,
            tolerance,
            anchor_window,
        ),
        pivot_match_score(
            left.d_price,
            right.d_price,
            left.d_date,
            right.d_date,
            tolerance,
            args.wide_d_window_minutes,
        ),
    ];

    let xabc_matches = pivots[..4].iter().filter(|(matched, _)| *matched).count();
    let all_matches = pivots.iter().filter(|(matched, _)| *matched).count();
    let d_price_close = price_match(left.d_price, right.d_price, tolerance);
    let d_is_near = d_confirm_diff <= args.d_window_minutes || d_date_diff <= args.d_window_minutes;
    let d_is_wide =
        d_confirm_diff <= args.wide_d_window_minutes || d_date_diff <= args.wide_d_window_minutes;

    let is_match = (all_matches >= 4 && d_is_wide)
        || (xabc_matches >= 4 && d_is_wide)
        || (xabc_matches >= 3 && d_price_close && d_is_near);

    let score = pivots.iter().map(|(_, score)| score).sum::<f64>() / pivots.len() as f64;

    MatchResult { is_match, score }
}

fn event_id_for(primary: &PatternSetup) -> String {
    let input = format!(
        "{}|{}|{}|{}|{}|{}",
        primary.symbol,
        primary.root_symbol,
        primary.source_table,
        primary.source_timeframe,
        primary.market,
        primary.setup_id
    );
    let hash = blake3::hash(input.as_bytes()).to_hex().to_string();
    format!("evt_{}", &hash[..16])
}

fn build_assignments(patterns: &[PatternSetup], args: &Args) -> Vec<Assignment> {
    let mut events: Vec<EventCluster> = Vec::new();

    for (candidate_index, candidate) in patterns.iter().enumerate() {
        let mut best_event_index: Option<usize> = None;
        let mut best_score = 0.0;

        for (event_index, event) in events.iter().enumerate().rev() {
            if minutes_between(event.last_d_confirm_date, candidate.d_confirm_date)
                > args.anchor_window_minutes.max(args.wide_d_window_minutes)
            {
                continue;
            }

            let primary_index = event.members[0];
            let primary_match = is_sister_pattern(&patterns[primary_index], candidate, args);
            let last_index = *event.members.last().unwrap_or(&primary_index);
            let last_match = is_sister_pattern(&patterns[last_index], candidate, args);
            let score = primary_match.score.max(last_match.score);

            if (primary_match.is_match || last_match.is_match) && score > best_score {
                best_score = score;
                best_event_index = Some(event_index);
            }
        }

        if let Some(event_index) = best_event_index {
            let event = &mut events[event_index];
            event.members.push(candidate_index);
            event.first_d_confirm_date = event.first_d_confirm_date.min(candidate.d_confirm_date);
            event.last_d_confirm_date = event.last_d_confirm_date.max(candidate.d_confirm_date);
        } else {
            events.push(EventCluster {
                members: vec![candidate_index],
                first_d_confirm_date: candidate.d_confirm_date,
                last_d_confirm_date: candidate.d_confirm_date,
            });
        }
    }

    let mut assignments = Vec::with_capacity(patterns.len());
    for event in events {
        let primary_index = event.members[0];
        let event_id = event_id_for(&patterns[primary_index]);
        let sister_count = event.members.len() as i64;

        for (rank_index, setup_index) in event.members.iter().enumerate() {
            let similarity_score = if *setup_index == primary_index {
                1.0
            } else {
                is_sister_pattern(&patterns[primary_index], &patterns[*setup_index], args).score
            };
            assignments.push(Assignment {
                setup_index: *setup_index,
                event_id: event_id.clone(),
                event_rank: rank_index as i64 + 1,
                is_primary: rank_index == 0,
                sister_count,
                similarity_score,
            });
        }
    }

    assignments.sort_by_key(|assignment| patterns[assignment.setup_index].id);
    assignments
}

async fn list_top_symbols(pool: &MySqlPool, args: &Args) -> Result<(), sqlx::Error> {
    let limit = args.list_top.unwrap_or(20).clamp(1, 100);
    let mut query = QueryBuilder::new(
        r#"
        SELECT
            symbol,
            COALESCE(NULLIF(source_table, ''), 'Unknown') AS source_table,
            COALESCE(NULLIF(source_timeframe, ''), 'Unknown') AS source_timeframe,
            CAST(COUNT(*) AS SIGNED) AS setup_count,
            MIN(d_confirm_date) AS first_confirm,
            MAX(d_confirm_date) AS last_confirm
        FROM pattern_setups
        WHERE 1 = 1
        "#,
    );
    if let Some(year) = args.year {
        query.push(" AND YEAR(d_confirm_date) = ");
        query.push_bind(year);
    }
    query.push(
        r#"
        GROUP BY symbol, COALESCE(NULLIF(source_table, ''), 'Unknown'), COALESCE(NULLIF(source_timeframe, ''), 'Unknown')
        ORDER BY setup_count DESC
        LIMIT "#,
    );
    query.push_bind(limit);

    let rows = query.build().fetch_all(pool).await?;
    println!("symbol\tsource\ttimeframe\tsetups\tfirst_confirm\tlast_confirm");
    for row in rows {
        let first_confirm: Option<NaiveDateTime> = row.try_get("first_confirm").ok();
        let last_confirm: Option<NaiveDateTime> = row.try_get("last_confirm").ok();
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            row.try_get::<String, _>("symbol")?,
            row.try_get::<String, _>("source_table")?,
            row.try_get::<String, _>("source_timeframe")?,
            row.try_get::<i64, _>("setup_count")?,
            first_confirm
                .map(|value| value.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            last_confirm
                .map(|value| value.to_string())
                .unwrap_or_else(|| "N/A".to_string())
        );
    }

    Ok(())
}

async fn load_symbol_groups(
    pool: &MySqlPool,
    args: &Args,
) -> Result<Vec<SymbolGroup>, sqlx::Error> {
    let mut query = QueryBuilder::new(
        r#"
        SELECT
            symbol,
            COALESCE(NULLIF(source_table, ''), '') AS source_table,
            COALESCE(NULLIF(source_timeframe, ''), '') AS source_timeframe,
            CAST(COUNT(*) AS SIGNED) AS setup_count
        FROM pattern_setups
        WHERE symbol IS NOT NULL
          AND symbol <> ''
          AND d_confirm_date IS NOT NULL
        "#,
    );
    if let Some(source_table) = args.source_table.as_deref() {
        query.push(" AND source_table = ");
        query.push_bind(source_table);
    }
    if let Some(timeframe) = args.timeframe.as_deref() {
        query.push(" AND source_timeframe = ");
        query.push_bind(timeframe);
    }
    if let Some(year) = args.year {
        query.push(" AND YEAR(d_confirm_date) = ");
        query.push_bind(year);
    }
    query.push(
        r#"
        GROUP BY symbol, COALESCE(NULLIF(source_table, ''), ''), COALESCE(NULLIF(source_timeframe, ''), '')
        ORDER BY MIN(d_confirm_date) ASC, symbol ASC
        "#,
    );

    let rows = query.build().fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            let source_table: String = row.try_get("source_table")?;
            let source_timeframe: String = row.try_get("source_timeframe")?;
            Ok(SymbolGroup {
                symbol: row.try_get("symbol")?,
                source_table: if source_table.is_empty() {
                    None
                } else {
                    Some(source_table)
                },
                source_timeframe: if source_timeframe.is_empty() {
                    None
                } else {
                    Some(source_timeframe)
                },
                setup_count: row.try_get("setup_count")?,
            })
        })
        .collect()
}

async fn load_patterns(pool: &MySqlPool, args: &Args) -> Result<Vec<PatternSetup>, sqlx::Error> {
    let Some(symbol) = args.symbol.as_deref() else {
        return Ok(Vec::new());
    };

    let mut query = QueryBuilder::new(
        r#"
        SELECT
            id,
            setup_id,
            symbol,
            COALESCE(root_symbol, '') AS root_symbol,
            COALESCE(source_table, '') AS source_table,
            COALESCE(source_timeframe, '') AS source_timeframe,
            market,
            harmonic_type,
            pattern_family_key,
            x_date,
            x_min_max AS x_price,
            a_date,
            a_min_max AS a_price,
            b_date,
            b_min_max AS b_price,
            c_date,
            c_min_max AS c_price,
            d_date,
            d_confirm_date,
            d_min_max AS d_price,
            xa_price_length,
            ab_price_length,
            bc_price_length,
            cd_price_length,
            full_pattern_length
        FROM pattern_setups
        WHERE symbol = 
        "#,
    );
    query.push_bind(symbol);
    if let Some(source_table) = args.source_table.as_deref() {
        query.push(" AND source_table = ");
        query.push_bind(source_table);
    }
    if let Some(timeframe) = args.timeframe.as_deref() {
        query.push(" AND source_timeframe = ");
        query.push_bind(timeframe);
    }
    if let Some(year) = args.year {
        query.push(" AND YEAR(d_confirm_date) = ");
        query.push_bind(year);
    }
    query.push(" ORDER BY d_confirm_date ASC, d_date ASC, setup_id ASC");
    if let Some(limit) = args.limit {
        query.push(" LIMIT ");
        query.push_bind(limit.max(1));
    }

    let rows = query.build().fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            Ok(PatternSetup {
                id: row.try_get("id")?,
                setup_id: row.try_get("setup_id")?,
                symbol: row.try_get("symbol")?,
                root_symbol: row.try_get("root_symbol")?,
                source_table: row.try_get("source_table")?,
                source_timeframe: row.try_get("source_timeframe")?,
                market: row.try_get("market")?,
                harmonic_type: row.try_get("harmonic_type")?,
                pattern_family_key: row.try_get("pattern_family_key")?,
                x_date: row.try_get("x_date")?,
                x_price: row.try_get("x_price")?,
                a_date: row.try_get("a_date")?,
                a_price: row.try_get("a_price")?,
                b_date: row.try_get("b_date")?,
                b_price: row.try_get("b_price")?,
                c_date: row.try_get("c_date")?,
                c_price: row.try_get("c_price")?,
                d_date: row.try_get("d_date")?,
                d_confirm_date: row.try_get("d_confirm_date")?,
                d_price: row.try_get("d_price")?,
                xa_price_length: row.try_get("xa_price_length")?,
                ab_price_length: row.try_get("ab_price_length")?,
                bc_price_length: row.try_get("bc_price_length")?,
                cd_price_length: row.try_get("cd_price_length")?,
                full_pattern_length: row.try_get("full_pattern_length")?,
            })
        })
        .collect()
}

async fn apply_assignments(
    pool: &MySqlPool,
    patterns: &[PatternSetup],
    assignments: &[Assignment],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    for assignment in assignments {
        let pattern = &patterns[assignment.setup_index];
        sqlx::query(
            r#"
            UPDATE pattern_setups
            SET event_id = ?,
                event_rank = ?,
                is_event_primary = ?,
                event_sister_count = ?,
                event_similarity_score = ?
            WHERE id = ?
            "#,
        )
        .bind(&assignment.event_id)
        .bind(assignment.event_rank)
        .bind(if assignment.is_primary { 1 } else { 0 })
        .bind(assignment.sister_count)
        .bind(assignment.similarity_score)
        .bind(pattern.id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await
}

fn print_summary(patterns: &[PatternSetup], assignments: &[Assignment], apply: bool) {
    let event_count = assignments
        .iter()
        .filter(|assignment| assignment.is_primary)
        .count();
    let sister_rows = assignments
        .iter()
        .filter(|assignment| assignment.sister_count > 1)
        .count();
    let duplicate_rows = assignments
        .iter()
        .filter(|assignment| assignment.event_rank > 1)
        .count();
    let largest_event = assignments
        .iter()
        .map(|assignment| assignment.sister_count)
        .max()
        .unwrap_or(0);

    println!(
        "{} setups -> {} events | {} twin rows | {} duplicate rows | largest event {} | {}",
        patterns.len(),
        event_count,
        sister_rows,
        duplicate_rows,
        largest_event,
        if apply { "APPLIED" } else { "DRY RUN" }
    );

    println!();
    println!("sample twin events");
    println!("event_id\trank/count\tsymbol\tmarket\tfamily\tsetup\td_confirm\tscore");
    let mut sister_assignments = assignments
        .iter()
        .filter(|assignment| assignment.sister_count > 1)
        .collect::<Vec<_>>();
    sister_assignments.sort_by(|left, right| {
        left.event_id
            .cmp(&right.event_id)
            .then(left.event_rank.cmp(&right.event_rank))
    });

    for assignment in sister_assignments.into_iter().take(30) {
        let pattern = &patterns[assignment.setup_index];
        println!(
            "{}\t{}/{}\t{}\t{}\t{}\t{}\t{}\t{:.3}",
            assignment.event_id,
            assignment.event_rank,
            assignment.sister_count,
            pattern.symbol,
            pattern.market,
            pattern
                .pattern_family_key
                .as_deref()
                .unwrap_or(&pattern.harmonic_type),
            pattern.setup_id,
            pattern.d_confirm_date,
            assignment.similarity_score
        );
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let args = parse_args()?;
    let pool = MySqlPool::connect(&database_url_from_env()?).await?;

    if args.list_top.is_some() {
        list_top_symbols(&pool, &args).await?;
        return Ok(());
    }

    if args.all_symbols {
        let groups = load_symbol_groups(&pool, &args).await?;
        if groups.is_empty() {
            println!("No symbol groups matched the requested filters.");
            return Ok(());
        }

        println!(
            "assigning events for {} symbol/source/timeframe group(s) / year {} / {}",
            groups.len(),
            args.year
                .map(|value| value.to_string())
                .unwrap_or_else(|| "All".to_string()),
            if args.apply { "APPLY" } else { "DRY RUN" }
        );

        for (group_index, group) in groups.iter().enumerate() {
            let mut group_args = args.clone();
            group_args.symbol = Some(group.symbol.clone());
            group_args.all_symbols = false;
            group_args.source_table = group.source_table.clone();
            group_args.timeframe = group.source_timeframe.clone();

            let patterns = load_patterns(&pool, &group_args).await?;
            if patterns.is_empty() {
                println!(
                    "[{}/{}] {} skipped: no rows loaded",
                    group_index + 1,
                    groups.len(),
                    group.symbol
                );
                continue;
            }

            let assignments = build_assignments(&patterns, &group_args);
            if group_args.apply {
                apply_assignments(&pool, &patterns, &assignments).await?;
            }

            let event_count = assignments
                .iter()
                .filter(|assignment| assignment.is_primary)
                .count();
            let twin_rows = assignments
                .iter()
                .filter(|assignment| assignment.sister_count > 1)
                .count();
            let duplicate_rows = assignments
                .iter()
                .filter(|assignment| assignment.event_rank > 1)
                .count();
            let largest_event = assignments
                .iter()
                .map(|assignment| assignment.sister_count)
                .max()
                .unwrap_or(0);

            println!(
                "[{}/{}] {} / {} / {}: listed={} loaded={} events={} twin_rows={} duplicates={} largest={} {}",
                group_index + 1,
                groups.len(),
                group.symbol,
                group
                    .source_table
                    .as_deref()
                    .unwrap_or(""),
                group
                    .source_timeframe
                    .as_deref()
                    .unwrap_or(""),
                group.setup_count,
                patterns.len(),
                event_count,
                twin_rows,
                duplicate_rows,
                largest_event,
                if group_args.apply { "APPLIED" } else { "DRY RUN" }
            );
        }

        return Ok(());
    }

    if args.symbol.is_none() {
        println!("{}", usage());
        return Err("Missing --symbol or --list-top".into());
    }

    let patterns = load_patterns(&pool, &args).await?;
    if patterns.is_empty() {
        println!("No pattern_setups rows matched the requested symbol/filter.");
        return Ok(());
    }

    let first = patterns.first().unwrap();
    println!(
        "assigning events for {} / {} / {} / year {}",
        first.symbol,
        first.source_table,
        first.source_timeframe,
        args.year
            .map(|value| value.to_string())
            .unwrap_or_else(|| "All".to_string())
    );
    println!(
        "rules: d={}m wide_d={}m anchor={}m price_tol={:.3}",
        args.d_window_minutes,
        args.wide_d_window_minutes,
        args.anchor_window_minutes,
        args.price_tolerance_ratio
    );

    let assignments = build_assignments(&patterns, &args);
    if args.apply {
        apply_assignments(&pool, &patterns, &assignments).await?;
    }
    print_summary(&patterns, &assignments, args.apply);

    Ok(())
}
