use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;

use serde_json::Value;
use sqlx::{mysql::MySqlPool, Row};

#[derive(Clone)]
struct TemplateSummary {
    label: String,
    uid: String,
    rule: String,
    direction: String,
    entry_offset: i64,
    risk: f64,
    eval_count: i64,
    pass_count: i64,
    avg_r: f64,
}

#[derive(Clone)]
struct FamilyPerf {
    template_uid: String,
    family_key: String,
    harmonic_type: String,
    market: String,
    bin: String,
    size_bucket: String,
    time_bin: String,
    eval_count: i64,
    pass_count: i64,
    fail_count: i64,
    no_entry_count: i64,
    avg_r: f64,
}

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

fn template_rule_short(direction_mode: &str, rule_json: &str, risk: f64) -> (String, i64) {
    let parsed = serde_json::from_str::<Value>(rule_json).ok();
    let entry_offset = parsed
        .as_ref()
        .and_then(|value| value.pointer("/entry/offset_from_confirmation"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let direction = if direction_mode == "inverse_pattern" {
        "INV"
    } else {
        "PAT"
    };
    (
        format!("{direction} C+{entry_offset} {risk:.3}CD 4R"),
        entry_offset,
    )
}

fn is_qualified(row: &FamilyPerf) -> bool {
    row.eval_count >= 25 && row.pass_count > 0 && row.avg_r > 0.0
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let pool = MySqlPool::connect(&database_url_from_env()?).await?;

    let run_id = sqlx::query_scalar::<_, String>(
        r#"
        SELECT run_id
        FROM entry_exit_template_creator_runs
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .fetch_one(&pool)
    .await?;

    let template_rows = sqlx::query(
        r#"
        SELECT
            t.template_uid,
            t.direction_mode,
            t.risk_multiple,
            CAST(t.rule_json AS CHAR) AS rule_json,
            COALESCE(s.eval_count, 0) AS eval_count,
            COALESCE(s.pass_count, 0) AS pass_count,
            COALESCE(s.avg_r, 0) AS avg_r
        FROM entry_exit_templates t
        LEFT JOIN entry_exit_template_ui_stats s
          ON s.run_id = t.origin_run_id
         AND s.template_uid = t.template_uid
        WHERE t.origin_run_id = ?
        ORDER BY pass_count DESC, avg_r DESC, eval_count DESC, t.created_at ASC
        "#,
    )
    .bind(&run_id)
    .fetch_all(&pool)
    .await?;

    let mut templates = HashMap::new();
    let mut ordered_templates = Vec::new();
    for (index, row) in template_rows.into_iter().enumerate() {
        let uid: String = row.try_get("template_uid")?;
        let direction: String = row.try_get("direction_mode")?;
        let risk: f64 = row.try_get("risk_multiple")?;
        let rule_json: String = row.try_get("rule_json")?;
        let (rule, entry_offset) = template_rule_short(&direction, &rule_json, risk);
        let summary = TemplateSummary {
            label: format!("T{:02}", index + 1),
            uid: uid.clone(),
            rule,
            direction,
            entry_offset,
            risk,
            eval_count: row.try_get("eval_count")?,
            pass_count: row.try_get("pass_count")?,
            avg_r: row.try_get("avg_r")?,
        };
        ordered_templates.push(summary.clone());
        templates.insert(uid, summary);
    }

    let family_rows = sqlx::query(
        r#"
        SELECT
            r.template_uid,
            COALESCE(NULLIF(r.pattern_family_key, ''), NULLIF(ps.pattern_family_key, ''), 'Unknown') AS family_key,
            COALESCE(NULLIF(ps.pattern_family_harmonic_type, ''), NULLIF(ps.harmonic_type, ''), 'Unknown') AS harmonic_type,
            COALESCE(NULLIF(ps.market, ''), NULLIF(r.market, ''), 'Unknown') AS market,
            COALESCE(NULLIF(ps.pattern_family_bin, ''), 'Unknown') AS family_bin,
            COALESCE(NULLIF(ps.pattern_family_size_bucket, ''), 'Unknown') AS family_size_bucket,
            COALESCE(NULLIF(ps.pattern_family_time_bin, ''), 'Unknown') AS family_time_bin,
            CAST(COUNT(*) AS SIGNED) AS eval_count,
            CAST(SUM(CASE WHEN r.outcome = 'pass' THEN 1 ELSE 0 END) AS SIGNED) AS pass_count,
            CAST(SUM(CASE WHEN r.outcome = 'fail' THEN 1 ELSE 0 END) AS SIGNED) AS fail_count,
            CAST(SUM(CASE WHEN r.outcome = 'no_entry' THEN 1 ELSE 0 END) AS SIGNED) AS no_entry_count,
            COALESCE(AVG(COALESCE(r.result_r, 0)), 0) AS avg_r
        FROM entry_exit_template_results r
        LEFT JOIN pattern_setups ps
          ON ps.setup_id = r.setup_id
        WHERE r.run_id = ?
        GROUP BY
            r.template_uid,
            COALESCE(NULLIF(r.pattern_family_key, ''), NULLIF(ps.pattern_family_key, ''), 'Unknown'),
            COALESCE(NULLIF(ps.pattern_family_harmonic_type, ''), NULLIF(ps.harmonic_type, ''), 'Unknown'),
            COALESCE(NULLIF(ps.market, ''), NULLIF(r.market, ''), 'Unknown'),
            COALESCE(NULLIF(ps.pattern_family_bin, ''), 'Unknown'),
            COALESCE(NULLIF(ps.pattern_family_size_bucket, ''), 'Unknown'),
            COALESCE(NULLIF(ps.pattern_family_time_bin, ''), 'Unknown')
        HAVING eval_count >= 25
        "#,
    )
    .bind(&run_id)
    .fetch_all(&pool)
    .await?;

    let mut family_perfs = Vec::new();
    for row in family_rows {
        family_perfs.push(FamilyPerf {
            template_uid: row.try_get("template_uid")?,
            family_key: row.try_get("family_key")?,
            harmonic_type: row.try_get("harmonic_type")?,
            market: row.try_get("market")?,
            bin: row.try_get("family_bin")?,
            size_bucket: row.try_get("family_size_bucket")?,
            time_bin: row.try_get("family_time_bin")?,
            eval_count: row.try_get("eval_count")?,
            pass_count: row.try_get("pass_count")?,
            fail_count: row.try_get("fail_count")?,
            no_entry_count: row.try_get("no_entry_count")?,
            avg_r: row.try_get("avg_r")?,
        });
    }

    let qualified = family_perfs
        .iter()
        .filter(|row| is_qualified(row))
        .cloned()
        .collect::<Vec<_>>();

    let mut best_by_family: HashMap<String, FamilyPerf> = HashMap::new();
    for row in &qualified {
        best_by_family
            .entry(row.family_key.clone())
            .and_modify(|current| {
                if row.avg_r > current.avg_r
                    || (row.avg_r == current.avg_r && row.pass_count > current.pass_count)
                {
                    *current = row.clone();
                }
            })
            .or_insert_with(|| row.clone());
    }

    let mut owned_by_template: HashMap<String, Vec<FamilyPerf>> = HashMap::new();
    for row in best_by_family.values() {
        owned_by_template
            .entry(row.template_uid.clone())
            .or_default()
            .push(row.clone());
    }

    println!("Entry/Exit template blend scan");
    println!("run_id={run_id}");
    println!(
        "templates={} family_rows={} qualified_family_template_rows={} best_family_owners={}",
        ordered_templates.len(),
        family_perfs.len(),
        qualified.len(),
        best_by_family.len()
    );
    println!();

    println!("Rule clusters");
    let mut clusters: BTreeMap<String, Vec<&TemplateSummary>> = BTreeMap::new();
    for template in &ordered_templates {
        let direction = if template.direction == "inverse_pattern" {
            "INV"
        } else {
            "PAT"
        };
        clusters
            .entry(format!("{direction} C+{}", template.entry_offset))
            .or_default()
            .push(template);
    }
    for (cluster, members) in clusters.iter().filter(|(_, members)| members.len() >= 2) {
        let labels = members
            .iter()
            .map(|template| {
                format!(
                    "{}({:.3}CD,{:.3}R)",
                    template.label, template.risk, template.avg_r
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        println!("{cluster}\t{} templates\t{labels}", members.len());
    }
    println!();

    println!("Top template owners by best positive family performance");
    let mut owners = owned_by_template
        .iter()
        .map(|(uid, rows)| {
            let wins: i64 = rows.iter().map(|row| row.pass_count).sum();
            let losses: i64 = rows.iter().map(|row| row.fail_count).sum();
            let no_entries: i64 = rows.iter().map(|row| row.no_entry_count).sum();
            let eval: i64 = rows.iter().map(|row| row.eval_count).sum();
            let weighted_r: f64 = rows
                .iter()
                .map(|row| row.avg_r * row.eval_count as f64)
                .sum();
            let mut harmonic_counts: HashMap<String, i64> = HashMap::new();
            let mut shape_counts: HashMap<String, i64> = HashMap::new();
            for row in rows {
                *harmonic_counts
                    .entry(row.harmonic_type.clone())
                    .or_default() += 1;
                *shape_counts
                    .entry(format!(
                        "{} {} {} {}",
                        row.market, row.bin, row.size_bucket, row.time_bin
                    ))
                    .or_default() += 1;
            }
            let dominant = harmonic_counts
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(key, count)| format!("{key}:{count}"))
                .unwrap_or_else(|| "N/A".to_string());
            let dominant_shape = shape_counts
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(key, count)| format!("{key}:{count}"))
                .unwrap_or_else(|| "N/A".to_string());
            (
                uid.clone(),
                rows.len(),
                wins,
                losses,
                no_entries,
                eval,
                weighted_r / eval.max(1) as f64,
                dominant,
                dominant_shape,
            )
        })
        .collect::<Vec<_>>();
    owners.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| right.2.cmp(&left.2)));
    for (uid, families, wins, losses, no_entries, eval, avg_r, dominant, dominant_shape) in
        owners.iter().take(15)
    {
        let template = templates.get(uid).unwrap();
        println!(
            "{}\t{}\tfamilies={families}\twins={wins}\tlosses={losses}\tno_entry={no_entries}\ttests={eval}\towned_avg={avg_r:.3}R\tdominant={dominant}\tshape={dominant_shape}",
            template.label, template.rule
        );
    }
    println!();

    println!("Greedy family-owner blend candidates");
    let mut uncovered: HashSet<String> = best_by_family.keys().cloned().collect();
    let mut blend = Vec::new();
    while blend.len() < 8 && !uncovered.is_empty() {
        let best = owned_by_template
            .iter()
            .filter(|(uid, _)| !blend.iter().any(|selected: &String| selected == *uid))
            .map(|(uid, rows)| {
                let new_rows = rows
                    .iter()
                    .filter(|row| uncovered.contains(&row.family_key))
                    .collect::<Vec<_>>();
                let new_wins: i64 = new_rows.iter().map(|row| row.pass_count).sum();
                (uid.clone(), new_rows.len(), new_wins)
            })
            .max_by(|left, right| left.1.cmp(&right.1).then_with(|| left.2.cmp(&right.2)));

        let Some((uid, new_families, new_wins)) = best else {
            break;
        };
        if new_families == 0 {
            break;
        }
        for row in owned_by_template.get(&uid).into_iter().flatten() {
            uncovered.remove(&row.family_key);
        }
        blend.push(uid.clone());
        let template = templates.get(&uid).unwrap();
        println!(
            "{}\t{}\tadds_families={new_families}\tadds_wins={new_wins}\tremaining={}",
            template.label,
            template.rule,
            uncovered.len()
        );
    }
    println!();

    println!("High-overlap merge/redundancy candidates");
    let template_family_sets = ordered_templates
        .iter()
        .map(|template| {
            let set = qualified
                .iter()
                .filter(|row| row.template_uid == template.uid)
                .map(|row| row.family_key.clone())
                .collect::<HashSet<_>>();
            (template.uid.clone(), set)
        })
        .collect::<HashMap<_, _>>();
    let mut overlaps = Vec::new();
    for left_index in 0..ordered_templates.len() {
        for right_index in (left_index + 1)..ordered_templates.len() {
            let left = &ordered_templates[left_index];
            let right = &ordered_templates[right_index];
            let left_set = template_family_sets.get(&left.uid).unwrap();
            let right_set = template_family_sets.get(&right.uid).unwrap();
            let intersection = left_set.intersection(right_set).count();
            let union = left_set.union(right_set).count();
            if union == 0 {
                continue;
            }
            let jaccard = intersection as f64 / union as f64;
            if jaccard >= 0.60 && intersection >= 20 {
                overlaps.push((
                    left.uid.clone(),
                    right.uid.clone(),
                    intersection,
                    union,
                    jaccard,
                ));
            }
        }
    }
    overlaps.sort_by(|left, right| {
        right
            .4
            .partial_cmp(&left.4)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.2.cmp(&left.2))
    });
    for (left_uid, right_uid, intersection, union, jaccard) in overlaps.iter().take(12) {
        let left = templates.get(left_uid).unwrap();
        let right = templates.get(right_uid).unwrap();
        let overlap_percent = jaccard * 100.0;
        println!(
            "{} {}  <->  {} {}\toverlap={intersection}/{union} ({overlap_percent:.0}%)",
            left.label, left.rule, right.label, right.rule
        );
    }
    println!();

    println!("Top individual template baselines");
    for template in ordered_templates.iter().take(12) {
        let wr = template.pass_count as f64 / template.eval_count.max(1) as f64 * 100.0;
        println!(
            "{}\t{}\twins={}\ttests={}\tWR={wr:.1}%\tavg={:.3}R",
            template.label, template.rule, template.pass_count, template.eval_count, template.avg_r
        );
    }

    Ok(())
}
