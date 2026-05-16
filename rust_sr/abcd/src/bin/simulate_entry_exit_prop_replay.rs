use std::collections::{HashMap, HashSet};
use std::env;
use std::time::Instant;

use chrono::{NaiveDate, NaiveDateTime};
use sqlx::{mysql::MySqlPool, Row};

#[derive(Debug)]
struct Args {
    router_run_id: Option<String>,
    profit_target_r: f64,
    max_drawdown_r: f64,
    daily_loss_r: f64,
    min_bucket_tests: i64,
    exclude_root_symbols: HashSet<String>,
    exclude_family_template_sum_lte: Option<f64>,
}

#[derive(Clone)]
struct ReplayTrade {
    symbol: String,
    market: String,
    template_label: String,
    template_name: String,
    harmonic_type: String,
    family_bin: String,
    family_size_bucket: String,
    family_time_bin: String,
    family_x_strictness: String,
    event_date: NaiveDateTime,
    outcome: String,
    result_r: f64,
}

#[derive(Default, Clone)]
struct BucketStats {
    tests: i64,
    wins: i64,
    losses: i64,
    no_entries: i64,
    sum_r: f64,
    worst_r: f64,
    best_r: f64,
}

#[derive(Default, Clone)]
struct DailyStats {
    date: Option<NaiveDate>,
    trades: i64,
    wins: i64,
    losses: i64,
    sum_r: f64,
}

struct CompletedCycle {
    cycle_number: i64,
    start_date: NaiveDate,
    end_date: NaiveDate,
    trades: i64,
    wins: i64,
    losses: i64,
    sum_r: f64,
    max_drawdown_r: f64,
    worst_day_r: f64,
    outcome: String,
}

struct OpenCycle {
    cycle_number: i64,
    start_date: NaiveDate,
    current_date: NaiveDate,
    trades: i64,
    wins: i64,
    losses: i64,
    equity_r: f64,
    peak_r: f64,
    max_drawdown_r: f64,
    daily_r: f64,
    worst_day_r: f64,
}

impl OpenCycle {
    fn new(cycle_number: i64, date: NaiveDate) -> Self {
        Self {
            cycle_number,
            start_date: date,
            current_date: date,
            trades: 0,
            wins: 0,
            losses: 0,
            equity_r: 0.0,
            peak_r: 0.0,
            max_drawdown_r: 0.0,
            daily_r: 0.0,
            worst_day_r: 0.0,
        }
    }

    fn apply_trade(&mut self, trade: &ReplayTrade) {
        let date = trade.event_date.date();
        if date != self.current_date {
            self.current_date = date;
            self.daily_r = 0.0;
        }

        self.trades += 1;
        if trade.outcome == "pass" {
            self.wins += 1;
        } else if trade.outcome == "fail" {
            self.losses += 1;
        }

        self.equity_r += trade.result_r;
        self.daily_r += trade.result_r;
        self.peak_r = self.peak_r.max(self.equity_r);
        self.max_drawdown_r = self.max_drawdown_r.max(self.peak_r - self.equity_r);
        self.worst_day_r = self.worst_day_r.min(self.daily_r);
    }

    fn finish(&self, outcome: &str) -> CompletedCycle {
        CompletedCycle {
            cycle_number: self.cycle_number,
            start_date: self.start_date,
            end_date: self.current_date,
            trades: self.trades,
            wins: self.wins,
            losses: self.losses,
            sum_r: self.equity_r,
            max_drawdown_r: self.max_drawdown_r,
            worst_day_r: self.worst_day_r,
            outcome: outcome.to_string(),
        }
    }
}

fn usage() -> &'static str {
    "Usage: cargo run --bin simulate_entry_exit_prop_replay -- [--router-run-id RUN_ID] [--profit-target-r 30] [--max-drawdown-r 20] [--daily-loss-r 10] [--min-bucket-tests 10] [--exclude-root-symbols ES,NQ] [--exclude-family-template-sum-lte -10]\n\nReplays family-router trades in chronological order and prints prop-style account diagnostics in R units."
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|items| items[0] == name)
        .map(|items| items[1].clone())
}

fn parse_csv_set(value: Option<String>) -> HashSet<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(|item| item.trim().to_uppercase())
        .filter(|item| !item.is_empty())
        .collect()
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let raw_args = env::args().skip(1).collect::<Vec<_>>();
    if raw_args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", usage());
        std::process::exit(0);
    }

    Ok(Args {
        router_run_id: arg_value(&raw_args, "--router-run-id")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        profit_target_r: arg_value(&raw_args, "--profit-target-r")
            .as_deref()
            .unwrap_or("30")
            .parse()?,
        max_drawdown_r: arg_value(&raw_args, "--max-drawdown-r")
            .as_deref()
            .unwrap_or("20")
            .parse()?,
        daily_loss_r: arg_value(&raw_args, "--daily-loss-r")
            .as_deref()
            .unwrap_or("10")
            .parse()?,
        min_bucket_tests: arg_value(&raw_args, "--min-bucket-tests")
            .as_deref()
            .unwrap_or("10")
            .parse()?,
        exclude_root_symbols: parse_csv_set(arg_value(&raw_args, "--exclude-root-symbols")),
        exclude_family_template_sum_lte: arg_value(&raw_args, "--exclude-family-template-sum-lte")
            .map(|value| value.parse())
            .transpose()?,
    })
}

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

async fn latest_router_run_id(pool: &MySqlPool) -> Result<String, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT router_run_id
        FROM entry_exit_template_family_router_runs
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .fetch_one(pool)
    .await
}

async fn load_trades(
    pool: &MySqlPool,
    router_run_id: &str,
) -> Result<Vec<ReplayTrade>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            r.symbol,
            r.market,
            r.template_label,
            COALESCE(c.template_name, r.template_label) AS template_name,
            COALESCE(c.harmonic_type, 'Unknown') AS harmonic_type,
            COALESCE(c.family_bin, 'Unknown') AS family_bin,
            COALESCE(c.family_size_bucket, 'Unknown') AS family_size_bucket,
            COALESCE(c.family_time_bin, 'Unknown') AS family_time_bin,
            COALESCE(c.family_x_strictness, 'Unknown') AS family_x_strictness,
            COALESCE(r.entry_date, r.d_confirm_date, r.d_date) AS event_date,
            r.outcome,
            COALESCE(r.result_r, 0) AS result_r
        FROM entry_exit_template_family_router_results r
        LEFT JOIN entry_exit_template_family_router_choices c
          ON c.router_run_id = r.router_run_id
         AND c.family_key = r.family_key
         AND c.template_uid = r.template_uid
        WHERE r.router_run_id = ?
        ORDER BY COALESCE(r.entry_date, r.d_confirm_date, r.d_date) ASC, r.id ASC
        "#,
    )
    .bind(router_run_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(ReplayTrade {
                symbol: row.try_get("symbol")?,
                market: row.try_get("market")?,
                template_label: row.try_get("template_label")?,
                template_name: row.try_get("template_name")?,
                harmonic_type: row.try_get("harmonic_type")?,
                family_bin: row.try_get("family_bin")?,
                family_size_bucket: row.try_get("family_size_bucket")?,
                family_time_bin: row.try_get("family_time_bin")?,
                family_x_strictness: row.try_get("family_x_strictness")?,
                event_date: row.try_get("event_date")?,
                outcome: row.try_get("outcome")?,
                result_r: row.try_get("result_r")?,
            })
        })
        .collect()
}

fn root_symbol(symbol: &str) -> String {
    let chars = symbol.chars().collect::<Vec<_>>();
    for index in 0..chars.len().saturating_sub(1) {
        let is_contract_month = matches!(
            chars[index],
            'F' | 'G' | 'H' | 'J' | 'K' | 'M' | 'N' | 'Q' | 'U' | 'V' | 'X' | 'Z'
        );
        if is_contract_month && chars[index + 1].is_ascii_digit() && index > 0 {
            return chars[..index].iter().collect();
        }
    }
    symbol
        .trim_end_matches(|ch: char| ch.is_ascii_digit())
        .to_string()
}

fn bucket_label(trade: &ReplayTrade) -> String {
    format!(
        "{} {} {} {} {} {}",
        trade.harmonic_type,
        trade.market,
        trade.family_bin,
        trade.family_size_bucket,
        trade.family_time_bin,
        trade.family_x_strictness
    )
}

fn family_template_label(trade: &ReplayTrade) -> String {
    format!(
        "{} + {} {}",
        bucket_label(trade),
        trade.template_label,
        trade.template_name
    )
}

fn update_bucket(map: &mut HashMap<String, BucketStats>, key: String, trade: &ReplayTrade) {
    let entry = map.entry(key).or_default();
    entry.tests += 1;
    if trade.outcome == "pass" {
        entry.wins += 1;
    } else if trade.outcome == "fail" {
        entry.losses += 1;
    } else {
        entry.no_entries += 1;
    }
    entry.sum_r += trade.result_r;
    if entry.tests == 1 {
        entry.best_r = trade.result_r;
        entry.worst_r = trade.result_r;
    } else {
        entry.best_r = entry.best_r.max(trade.result_r);
        entry.worst_r = entry.worst_r.min(trade.result_r);
    }
}

fn filter_trades(
    trades: Vec<ReplayTrade>,
    args: &Args,
) -> (Vec<ReplayTrade>, i64, i64, Vec<(String, BucketStats)>) {
    let mut family_template_candidates = HashMap::new();
    for trade in trades.iter() {
        if args
            .exclude_root_symbols
            .contains(&root_symbol(&trade.symbol).to_uppercase())
        {
            continue;
        }
        update_bucket(
            &mut family_template_candidates,
            family_template_label(trade),
            trade,
        );
    }

    let mut excluded_family_templates = Vec::new();
    let excluded_family_template_keys = if let Some(threshold) =
        args.exclude_family_template_sum_lte
    {
        family_template_candidates
            .into_iter()
            .filter(|(_, stats)| stats.tests >= args.min_bucket_tests && stats.sum_r <= threshold)
            .map(|item| {
                excluded_family_templates.push(item.clone());
                item.0
            })
            .collect::<HashSet<_>>()
    } else {
        HashSet::new()
    };

    excluded_family_templates.sort_by(|left, right| {
        left.1
            .sum_r
            .partial_cmp(&right.1.sum_r)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut root_excluded = 0_i64;
    let mut family_template_excluded = 0_i64;
    let filtered = trades
        .into_iter()
        .filter(|trade| {
            if args
                .exclude_root_symbols
                .contains(&root_symbol(&trade.symbol).to_uppercase())
            {
                root_excluded += 1;
                return false;
            }

            if excluded_family_template_keys.contains(&family_template_label(trade)) {
                family_template_excluded += 1;
                return false;
            }

            true
        })
        .collect();

    (
        filtered,
        root_excluded,
        family_template_excluded,
        excluded_family_templates,
    )
}

fn print_bucket_table(title: &str, rows: &[(String, BucketStats)], descending: bool) {
    println!("\n{title}");
    let mut sorted = rows.to_vec();
    sorted.sort_by(|left, right| {
        let order = left
            .1
            .sum_r
            .partial_cmp(&right.1.sum_r)
            .unwrap_or(std::cmp::Ordering::Equal);
        if descending {
            order.reverse()
        } else {
            order
        }
    });

    for (name, stats) in sorted.into_iter().take(12) {
        let win_rate = if stats.tests > 0 {
            stats.wins as f64 / stats.tests as f64 * 100.0
        } else {
            0.0
        };
        let avg_r = if stats.tests > 0 {
            stats.sum_r / stats.tests as f64
        } else {
            0.0
        };
        println!(
            "{name} | tests={} WR={:.1}% avg={:.3}R sum={:.2}R worst={:.2}R",
            stats.tests, win_rate, avg_r, stats.sum_r, stats.worst_r
        );
    }
}

fn print_daily_table(title: &str, days: &[DailyStats], descending: bool) {
    println!("\n{title}");
    let mut sorted = days.to_vec();
    sorted.sort_by(|left, right| {
        let order = left
            .sum_r
            .partial_cmp(&right.sum_r)
            .unwrap_or(std::cmp::Ordering::Equal);
        if descending {
            order.reverse()
        } else {
            order
        }
    });

    for day in sorted.into_iter().take(10) {
        let date = day
            .date
            .map(|date| date.to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let win_rate = if day.trades > 0 {
            day.wins as f64 / day.trades as f64 * 100.0
        } else {
            0.0
        };
        println!(
            "{date} | trades={} WR={:.1}% sum={:.2}R wins={} losses={}",
            day.trades, win_rate, day.sum_r, day.wins, day.losses
        );
    }
}

fn main_summary(
    trades: &[ReplayTrade],
    args: &Args,
) -> (
    Vec<CompletedCycle>,
    Vec<DailyStats>,
    HashMap<String, BucketStats>,
    HashMap<String, BucketStats>,
    HashMap<String, BucketStats>,
    HashMap<String, BucketStats>,
) {
    let mut equity_r: f64 = 0.0;
    let mut peak_r: f64 = 0.0;
    let mut max_drawdown_r: f64 = 0.0;
    let mut wins = 0_i64;
    let mut losses = 0_i64;
    let mut no_entries = 0_i64;
    let mut max_loss_streak = 0_i64;
    let mut current_loss_streak = 0_i64;
    let mut first_profit_target: Option<(NaiveDate, f64)> = None;
    let mut first_drawdown_breach: Option<(NaiveDate, f64)> = None;

    let mut day_map: HashMap<NaiveDate, DailyStats> = HashMap::new();
    let mut family_map = HashMap::new();
    let mut template_map = HashMap::new();
    let mut symbol_map = HashMap::new();
    let mut family_template_map = HashMap::new();

    let mut cycles = Vec::new();
    let mut open_cycle: Option<OpenCycle> = None;
    let mut next_cycle_number = 1_i64;

    for trade in trades {
        let date = trade.event_date.date();
        let day = day_map.entry(date).or_insert_with(|| DailyStats {
            date: Some(date),
            ..DailyStats::default()
        });
        day.trades += 1;
        day.sum_r += trade.result_r;
        if trade.outcome == "pass" {
            day.wins += 1;
        } else if trade.outcome == "fail" {
            day.losses += 1;
        }

        equity_r += trade.result_r;
        peak_r = peak_r.max(equity_r);
        max_drawdown_r = max_drawdown_r.max(peak_r - equity_r);

        if equity_r >= args.profit_target_r && first_profit_target.is_none() {
            first_profit_target = Some((date, equity_r));
        }
        if peak_r - equity_r >= args.max_drawdown_r && first_drawdown_breach.is_none() {
            first_drawdown_breach = Some((date, peak_r - equity_r));
        }

        if trade.outcome == "pass" {
            wins += 1;
            current_loss_streak = 0;
        } else if trade.outcome == "fail" {
            losses += 1;
            current_loss_streak += 1;
            max_loss_streak = max_loss_streak.max(current_loss_streak);
        } else {
            no_entries += 1;
        }

        let family_name = bucket_label(trade);
        update_bucket(&mut family_map, family_name.clone(), trade);
        update_bucket(
            &mut template_map,
            format!("{} {}", trade.template_label, trade.template_name),
            trade,
        );
        update_bucket(&mut symbol_map, root_symbol(&trade.symbol), trade);
        update_bucket(
            &mut family_template_map,
            format!(
                "{family_name} + {} {}",
                trade.template_label, trade.template_name
            ),
            trade,
        );

        if open_cycle.is_none() {
            open_cycle = Some(OpenCycle::new(next_cycle_number, date));
        }

        let mut close_cycle_as: Option<String> = None;
        if let Some(cycle) = open_cycle.as_mut() {
            cycle.apply_trade(trade);

            if cycle.daily_r <= -args.daily_loss_r {
                close_cycle_as = Some("failed_daily_loss".to_string());
            } else if cycle.max_drawdown_r >= args.max_drawdown_r {
                close_cycle_as = Some("failed_drawdown".to_string());
            } else if cycle.equity_r >= args.profit_target_r {
                close_cycle_as = Some("passed_profit_target".to_string());
            }
        }

        if let Some(outcome) = close_cycle_as {
            if let Some(cycle) = open_cycle.take() {
                cycles.push(cycle.finish(&outcome));
                next_cycle_number += 1;
            }
        }
    }

    if let Some(cycle) = open_cycle.take() {
        cycles.push(cycle.finish("open_incomplete"));
    }

    println!("\nChronological replay");
    println!("trades={}", trades.len());
    println!(
        "wins={} losses={} no_entry={} WR={:.2}%",
        wins,
        losses,
        no_entries,
        if trades.is_empty() {
            0.0
        } else {
            wins as f64 / trades.len() as f64 * 100.0
        }
    );
    println!(
        "net={equity_r:.2}R max_drawdown={max_drawdown_r:.2}R max_loss_streak={max_loss_streak}"
    );
    if let Some((date, value)) = first_profit_target {
        println!(
            "first_profit_target_hit={} at {:.2}R target={:.2}R",
            date, value, args.profit_target_r
        );
    } else {
        println!(
            "first_profit_target_hit=none target={:.2}R",
            args.profit_target_r
        );
    }
    if let Some((date, value)) = first_drawdown_breach {
        println!(
            "first_drawdown_breach={} at {:.2}R limit={:.2}R",
            date, value, args.max_drawdown_r
        );
    } else {
        println!(
            "first_drawdown_breach=none limit={:.2}R",
            args.max_drawdown_r
        );
    }

    let mut days = day_map.into_values().collect::<Vec<_>>();
    days.sort_by_key(|day| day.date);

    (
        cycles,
        days,
        family_map,
        template_map,
        symbol_map,
        family_template_map,
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let started = Instant::now();
    let args = parse_args()?;
    let pool = MySqlPool::connect(&database_url_from_env()?).await?;
    let router_run_id = match args.router_run_id.clone() {
        Some(run_id) => run_id,
        None => latest_router_run_id(&pool).await?,
    };

    println!("Prop replay");
    println!("router_run_id={router_run_id}");
    println!(
        "rules=target {:.2}R / max drawdown {:.2}R / daily loss {:.2}R",
        args.profit_target_r, args.max_drawdown_r, args.daily_loss_r
    );

    let raw_trades = load_trades(&pool, &router_run_id).await?;
    let raw_trade_count = raw_trades.len();
    let (trades, root_excluded, family_template_excluded, excluded_family_templates) =
        filter_trades(raw_trades, &args);

    if !args.exclude_root_symbols.is_empty() {
        let mut roots = args
            .exclude_root_symbols
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        roots.sort();
        println!("excluded_roots={}", roots.join(","));
    }
    if let Some(threshold) = args.exclude_family_template_sum_lte {
        println!(
            "excluded_family_template_zones={} threshold_sum_lte={:.2}R min_tests={}",
            excluded_family_templates.len(),
            threshold,
            args.min_bucket_tests
        );
        for (name, stats) in excluded_family_templates.iter().take(12) {
            println!(
                "  skip {name} | tests={} sum={:.2}R",
                stats.tests, stats.sum_r
            );
        }
        if excluded_family_templates.len() > 12 {
            println!(
                "  ... {} more skipped zones",
                excluded_family_templates.len() - 12
            );
        }
    }
    println!(
        "loaded_trades={} filtered_trades={} root_excluded={} family_template_excluded={}",
        raw_trade_count,
        trades.len(),
        root_excluded,
        family_template_excluded
    );

    if trades.is_empty() {
        println!("No router trades found.");
        return Ok(());
    }
    println!(
        "date_range={} -> {}",
        trades.first().unwrap().event_date.date(),
        trades.last().unwrap().event_date.date()
    );

    let (cycles, days, family_map, template_map, symbol_map, family_template_map) =
        main_summary(&trades, &args);

    let passes = cycles
        .iter()
        .filter(|cycle| cycle.outcome == "passed_profit_target")
        .count();
    let daily_fails = cycles
        .iter()
        .filter(|cycle| cycle.outcome == "failed_daily_loss")
        .count();
    let drawdown_fails = cycles
        .iter()
        .filter(|cycle| cycle.outcome == "failed_drawdown")
        .count();
    let incomplete = cycles
        .iter()
        .filter(|cycle| cycle.outcome == "open_incomplete")
        .count();

    println!("\nProp account cycles");
    let prop_pass_rate = if cycles.is_empty() {
        0.0
    } else {
        passes as f64 / cycles.len() as f64 * 100.0
    };
    let closed_cycles = passes + daily_fails + drawdown_fails;
    let closed_prop_pass_rate = if closed_cycles > 0 {
        passes as f64 / closed_cycles as f64 * 100.0
    } else {
        0.0
    };
    println!(
        "cycles={} passed={} daily_fails={} drawdown_fails={} incomplete={} prop_pass_rate={:.2}% closed_prop_pass_rate={:.2}%",
        cycles.len(),
        passes,
        daily_fails,
        drawdown_fails,
        incomplete,
        prop_pass_rate,
        closed_prop_pass_rate
    );
    for cycle in cycles.iter().take(12) {
        println!(
            "#{} {} -> {} {} trades={} WR={:.1}% net={:.2}R dd={:.2}R worst_day={:.2}R losses={}",
            cycle.cycle_number,
            cycle.start_date,
            cycle.end_date,
            cycle.outcome,
            cycle.trades,
            if cycle.trades > 0 {
                cycle.wins as f64 / cycle.trades as f64 * 100.0
            } else {
                0.0
            },
            cycle.sum_r,
            cycle.max_drawdown_r,
            cycle.worst_day_r,
            cycle.losses
        );
    }
    if cycles.len() > 12 {
        println!("... {} more cycles", cycles.len() - 12);
    }

    print_daily_table("Worst days", &days, false);
    print_daily_table("Best days", &days, true);

    let min_tests = args.min_bucket_tests;
    let eligible_families = family_map
        .into_iter()
        .filter(|(_, stats)| stats.tests >= min_tests)
        .collect::<Vec<_>>();
    let eligible_templates = template_map
        .into_iter()
        .filter(|(_, stats)| stats.tests >= min_tests)
        .collect::<Vec<_>>();
    let eligible_symbols = symbol_map
        .into_iter()
        .filter(|(_, stats)| stats.tests >= min_tests)
        .collect::<Vec<_>>();
    let eligible_family_templates = family_template_map
        .into_iter()
        .filter(|(_, stats)| stats.tests >= min_tests)
        .collect::<Vec<_>>();

    print_bucket_table("Families that dragged most", &eligible_families, false);
    print_bucket_table("Families that helped most", &eligible_families, true);
    print_bucket_table(
        "Family + template drag zones",
        &eligible_family_templates,
        false,
    );
    print_bucket_table("Templates that dragged most", &eligible_templates, false);
    print_bucket_table("Symbols that dragged most", &eligible_symbols, false);

    println!("\nelapsed={:.2}s", started.elapsed().as_secs_f64());

    Ok(())
}
