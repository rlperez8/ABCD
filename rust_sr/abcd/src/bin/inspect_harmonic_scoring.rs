use std::env;

use sqlx::{
    mysql::{MySqlPool, MySqlRow},
    Row,
};

#[derive(Clone, Copy)]
struct HarmonicTarget {
    harmonic_type: &'static str,
    ab_xa: f64,
    bc_ab: f64,
    cd_bc: f64,
    d_xa: f64,
}

#[derive(Clone, Copy)]
enum CompletionMode {
    CdXaLength,
    AdXaCompletion,
}

#[derive(Clone, Copy)]
struct SetupRatios {
    ab_xa_price: f64,
    bc_ab_price: f64,
    cd_bc_price: f64,
    cd_xa_price: f64,
    ad_xa_price: f64,
    ab_xa_time: f64,
    bc_ab_time: f64,
    cd_bc_time: f64,
    cd_xa_time: f64,
}

#[derive(Clone, Default)]
struct DominantStats {
    wins: i64,
    price_sum: f64,
    time_sum: f64,
    margin_sum: f64,
    close_1pt: i64,
    close_5pt: i64,
}

const LEGACY_TARGETS: [HarmonicTarget; 7] = [
    HarmonicTarget {
        harmonic_type: "Bat",
        ab_xa: 0.382,
        bc_ab: 0.382,
        cd_bc: 2.618,
        d_xa: 1.618,
    },
    HarmonicTarget {
        harmonic_type: "AlternateBat",
        ab_xa: 0.382,
        bc_ab: 0.382,
        cd_bc: 2.000,
        d_xa: 1.130,
    },
    HarmonicTarget {
        harmonic_type: "Butterfly",
        ab_xa: 0.786,
        bc_ab: 0.382,
        cd_bc: 1.270,
        d_xa: 1.618,
    },
    HarmonicTarget {
        harmonic_type: "Gartley",
        ab_xa: 0.618,
        bc_ab: 0.382,
        cd_bc: 1.270,
        d_xa: 0.786,
    },
    HarmonicTarget {
        harmonic_type: "Crab",
        ab_xa: 0.382,
        bc_ab: 0.382,
        cd_bc: 3.618,
        d_xa: 2.618,
    },
    HarmonicTarget {
        harmonic_type: "DeepCrab",
        ab_xa: 0.886,
        bc_ab: 0.382,
        cd_bc: 2.618,
        d_xa: 1.618,
    },
    HarmonicTarget {
        harmonic_type: "Shark",
        ab_xa: 0.886,
        bc_ab: 0.382,
        cd_bc: 1.130,
        d_xa: 1.618,
    },
];

const DEFINITION_TARGETS: [HarmonicTarget; 7] = [
    HarmonicTarget {
        harmonic_type: "Bat",
        ab_xa: 0.500,
        bc_ab: 0.382,
        cd_bc: 1.618,
        d_xa: 0.886,
    },
    HarmonicTarget {
        harmonic_type: "AlternateBat",
        ab_xa: 0.382,
        bc_ab: 0.382,
        cd_bc: 2.000,
        d_xa: 1.130,
    },
    HarmonicTarget {
        harmonic_type: "Butterfly",
        ab_xa: 0.786,
        bc_ab: 0.382,
        cd_bc: 1.618,
        d_xa: 1.272,
    },
    HarmonicTarget {
        harmonic_type: "Gartley",
        ab_xa: 0.618,
        bc_ab: 0.382,
        cd_bc: 1.272,
        d_xa: 0.786,
    },
    HarmonicTarget {
        harmonic_type: "Crab",
        ab_xa: 0.382,
        bc_ab: 0.382,
        cd_bc: 2.618,
        d_xa: 1.618,
    },
    HarmonicTarget {
        harmonic_type: "DeepCrab",
        ab_xa: 0.886,
        bc_ab: 0.382,
        cd_bc: 2.618,
        d_xa: 1.618,
    },
    HarmonicTarget {
        harmonic_type: "Shark",
        ab_xa: 0.500,
        bc_ab: 1.130,
        cd_bc: 1.618,
        d_xa: 0.886,
    },
];

fn ratio(numerator: f64, denominator: f64) -> f64 {
    if denominator > 0.0 {
        (numerator / denominator) * 100.0
    } else {
        0.0
    }
}

fn leg_accuracy(current_leg: f64, target_leg: f64) -> f64 {
    if current_leg <= 0.0 || target_leg <= 0.0 {
        return 0.0;
    }

    let current_leg_fraction = current_leg / 100.0;
    let accuracy = 100.0 * (1.0 - (current_leg_fraction - target_leg).abs() / target_leg);
    accuracy.clamp(0.0, 100.0)
}

fn setup_ratios_from_row(row: &MySqlRow) -> Result<SetupRatios, sqlx::Error> {
    let xa_price_length: f64 = row.try_get("xa_price_length")?;
    let ab_price_length: f64 = row.try_get("ab_price_length")?;
    let bc_price_length: f64 = row.try_get("bc_price_length")?;
    let cd_price_length: f64 = row.try_get("cd_price_length")?;
    let a_min_max: f64 = row.try_get("a_min_max")?;
    let d_min_max: f64 = row.try_get("d_min_max")?;
    let x_length: i64 = row.try_get("x_length")?;
    let a_length: i64 = row.try_get("a_length")?;
    let b_length: i64 = row.try_get("b_length")?;
    let c_length: i64 = row.try_get("c_length")?;

    Ok(SetupRatios {
        ab_xa_price: ratio(ab_price_length, xa_price_length),
        bc_ab_price: ratio(bc_price_length, ab_price_length),
        cd_bc_price: ratio(cd_price_length, bc_price_length),
        cd_xa_price: ratio(cd_price_length, xa_price_length),
        ad_xa_price: ratio((a_min_max - d_min_max).abs(), xa_price_length),
        ab_xa_time: ratio(a_length as f64, x_length as f64),
        bc_ab_time: ratio(b_length as f64, a_length as f64),
        cd_bc_time: ratio(c_length as f64, b_length as f64),
        cd_xa_time: ratio(c_length as f64, x_length as f64),
    })
}

fn score_target(
    ratios: SetupRatios,
    target: HarmonicTarget,
    completion_mode: CompletionMode,
) -> (f64, f64) {
    let completion_price = match completion_mode {
        CompletionMode::CdXaLength => ratios.cd_xa_price,
        CompletionMode::AdXaCompletion => ratios.ad_xa_price,
    };

    let price_accuracy = (leg_accuracy(ratios.ab_xa_price, target.ab_xa)
        + leg_accuracy(ratios.bc_ab_price, target.bc_ab)
        + leg_accuracy(ratios.cd_bc_price, target.cd_bc)
        + leg_accuracy(completion_price, target.d_xa))
        / 4.0;

    let time_accuracy = (leg_accuracy(ratios.ab_xa_time, target.ab_xa)
        + leg_accuracy(ratios.bc_ab_time, target.bc_ab)
        + leg_accuracy(ratios.cd_bc_time, target.cd_bc)
        + leg_accuracy(ratios.cd_xa_time, target.d_xa))
        / 4.0;

    (price_accuracy, time_accuracy)
}

fn print_in_memory_dominant_rows(
    setup_rows: &[MySqlRow],
    setup_count: i64,
    title: &str,
    targets: &[HarmonicTarget],
    completion_mode: CompletionMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut stats = vec![DominantStats::default(); targets.len()];

    for row in setup_rows {
        let ratios = setup_ratios_from_row(row)?;
        let mut scored: Vec<(usize, f64, f64)> = targets
            .iter()
            .enumerate()
            .map(|(index, target)| {
                let (price_accuracy, time_accuracy) =
                    score_target(ratios, *target, completion_mode);
                (index, price_accuracy, time_accuracy)
            })
            .collect();

        scored.sort_by(|left, right| {
            right
                .1
                .partial_cmp(&left.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    right
                        .2
                        .partial_cmp(&left.2)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| {
                    targets[left.0]
                        .harmonic_type
                        .cmp(targets[right.0].harmonic_type)
                })
        });

        let Some((best_index, best_price, best_time)) = scored.first().copied() else {
            continue;
        };
        let second_price = scored.get(1).map(|item| item.1).unwrap_or(0.0);
        let margin = best_price - second_price;
        let target_stats = &mut stats[best_index];
        target_stats.wins += 1;
        target_stats.price_sum += best_price;
        target_stats.time_sum += best_time;
        target_stats.margin_sum += margin;
        if margin <= 1.0 {
            target_stats.close_1pt += 1;
        }
        if margin <= 5.0 {
            target_stats.close_5pt += 1;
        }
    }

    let mut summary: Vec<(HarmonicTarget, DominantStats)> =
        targets.iter().copied().zip(stats).collect();
    summary.sort_by(|left, right| right.1.wins.cmp(&left.1.wins));

    println!();
    println!("{title}");
    println!("harmonic\twins\twin_rate\tavg_price\tavg_time\tavg_margin\tclose_1pt\tclose_5pt");
    for (target, stat) in summary {
        if stat.wins <= 0 {
            continue;
        }

        let win_rate = if setup_count > 0 {
            stat.wins as f64 / setup_count as f64
        } else {
            0.0
        };
        println!(
            "{}\t{}\t{:.1}%\t{:.2}\t{:.2}\t{:.2}\t{}\t{}",
            target.harmonic_type,
            stat.wins,
            win_rate * 100.0,
            stat.price_sum / stat.wins as f64,
            stat.time_sum / stat.wins as f64,
            stat.margin_sum / stat.wins as f64,
            stat.close_1pt,
            stat.close_5pt
        );
    }

    Ok(())
}

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let pool = MySqlPool::connect(&database_url_from_env()?).await?;

    let totals = sqlx::query(
        r#"
        SELECT
            (SELECT COUNT(*) FROM pattern_setups) AS setup_count,
            (SELECT COUNT(*) FROM pattern_harmonic_scores) AS score_rows
        "#,
    )
    .fetch_one(&pool)
    .await?;
    let setup_count: i64 = totals.try_get("setup_count")?;
    let score_rows: i64 = totals.try_get("score_rows")?;
    println!("pattern_setups: {setup_count}");
    println!("pattern_harmonic_scores: {score_rows}");
    println!();

    println!("Stored dominant harmonic is recomputed below from pattern_setups to avoid a slow DB window pass.");
    println!();
    println!("All stored score rows by harmonic");
    println!("harmonic\trows\tavg_price\tavg_nonzero\t>=50\t>=60\t>=70\t>=80");
    let score_rows = sqlx::query(
        r#"
        SELECT
            harmonic_type,
            CAST(COUNT(*) AS SIGNED) AS rows_count,
            CAST(AVG(price_accuracy) AS DOUBLE) AS avg_price,
            CAST(AVG(NULLIF(price_accuracy, 0.0)) AS DOUBLE) AS avg_nonzero,
            CAST(SUM(CASE WHEN price_accuracy >= 50 THEN 1 ELSE 0 END) AS SIGNED) AS ge_50,
            CAST(SUM(CASE WHEN price_accuracy >= 60 THEN 1 ELSE 0 END) AS SIGNED) AS ge_60,
            CAST(SUM(CASE WHEN price_accuracy >= 70 THEN 1 ELSE 0 END) AS SIGNED) AS ge_70,
            CAST(SUM(CASE WHEN price_accuracy >= 80 THEN 1 ELSE 0 END) AS SIGNED) AS ge_80
        FROM pattern_harmonic_scores
        GROUP BY harmonic_type
        ORDER BY avg_price DESC
        "#,
    )
    .fetch_all(&pool)
    .await?;
    for row in score_rows {
        let harmonic_type: String = row.try_get("harmonic_type")?;
        let rows_count: i64 = row.try_get("rows_count")?;
        let avg_price: f64 = row.try_get("avg_price")?;
        let avg_nonzero: Option<f64> = row.try_get("avg_nonzero")?;
        let ge_50: i64 = row.try_get("ge_50")?;
        let ge_60: i64 = row.try_get("ge_60")?;
        let ge_70: i64 = row.try_get("ge_70")?;
        let ge_80: i64 = row.try_get("ge_80")?;
        println!(
            "{harmonic_type}\t{rows_count}\t{avg_price:.2}\t{:.2}\t{ge_50}\t{ge_60}\t{ge_70}\t{ge_80}",
            avg_nonzero.unwrap_or(0.0)
        );
    }

    println!();
    println!("Loading pattern_setups ratios for in-memory scorer comparison...");
    let setup_rows = sqlx::query(
        r#"
        SELECT
            xa_price_length,
            ab_price_length,
            bc_price_length,
            cd_price_length,
            a_min_max,
            d_min_max,
            x_length,
            a_length,
            b_length,
            c_length
        FROM pattern_setups
        "#,
    )
    .fetch_all(&pool)
    .await?;

    print_in_memory_dominant_rows(
        &setup_rows,
        setup_count,
        "Legacy dominant harmonic from old hard-coded targets + CD/XA length",
        &LEGACY_TARGETS,
        CompletionMode::CdXaLength,
    )?;

    print_in_memory_dominant_rows(
        &setup_rows,
        setup_count,
        "Alternative dominant harmonic from definition targets + CD/XA length",
        &DEFINITION_TARGETS,
        CompletionMode::CdXaLength,
    )?;

    print_in_memory_dominant_rows(
        &setup_rows,
        setup_count,
        "Alternative dominant harmonic from definition targets + AD/XA D-completion",
        &DEFINITION_TARGETS,
        CompletionMode::AdXaCompletion,
    )?;

    println!();
    println!("Pattern family summary by harmonic");
    println!("harmonic\tfamilies\tsetups");
    let family_rows = sqlx::query(
        r#"
        SELECT
            harmonic_type,
            CAST(COUNT(*) AS SIGNED) AS family_count,
            CAST(SUM(setup_count) AS SIGNED) AS setup_count
        FROM pattern_family_summary
        GROUP BY harmonic_type
        ORDER BY setup_count DESC
        "#,
    )
    .fetch_all(&pool)
    .await?;
    for row in family_rows {
        let harmonic_type: String = row.try_get("harmonic_type")?;
        let family_count: i64 = row.try_get("family_count")?;
        let family_setup_count: i64 = row.try_get("setup_count")?;
        println!("{harmonic_type}\t{family_count}\t{family_setup_count}");
    }

    Ok(())
}
