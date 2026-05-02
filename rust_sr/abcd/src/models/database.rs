// use std::str::pattern;

// use mysql::{Pool, PooledConn, OptsBuilder, SslOpts};
use mysql::*;
// use mysql::prelude::*;
use crate::models::candle::*;
// use chrono::NaiveDate;
use chrono::{Datelike, NaiveDate, NaiveDateTime};
use futures_util::TryStreamExt;
use sqlx::mysql::MySqlPool;
// use std::fs::File;
// use csv::WriterBuilder;
use crate::models::market::Market;
use crate::models::pattern_abcd::PatternXABCD;
use crate::models::prop_reversal_route::PropReversalOutcome;
use crate::models::reversal_type::ReversalType;
use serde::ser::Serializer;
use serde::Serialize;
use sqlx::{MySql, QueryBuilder};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
// use sqlx::{QueryBuilder};
use rust_decimal::prelude::ToPrimitive;

const PATTERN_INSERT_CHUNK_SIZE: usize = 1000;
const CANDLE_TREND_UPSERT_CHUNK_SIZE: usize = 500;
const THREE_MONTH_SMA_PERIOD: usize = 63;
const SIX_MONTH_SMA_PERIOD: usize = 126;
const TWELVE_MONTH_SMA_PERIOD: usize = 252;
const TREND_SMA_SLOPE_LOOKBACK: usize = 10;
const HARMONIC_SCORE_COLUMNS: [(&str, &str, &str); 7] = [
    ("Bat", "bat_accuracy", "bat_time_accuracy"),
    (
        "AlternateBat",
        "alternate_bat_accuracy",
        "alternate_bat_time_accuracy",
    ),
    ("Butterfly", "butterfly_accuracy", "butterfly_time_accuracy"),
    ("Gartley", "gartley_accuracy", "gartley_time_accuracy"),
    ("Crab", "crab_accuracy", "crab_time_accuracy"),
    ("DeepCrab", "deep_crab_accuracy", "deep_crab_time_accuracy"),
    ("Shark", "shark_accuracy", "shark_time_accuracy"),
];

const REBUILD_SECONDARY_INDEXES: [(&str, &str, &str); 19] = [
    (
        "pattern_harmonic_scores",
        "uniq_pattern_harmonic_score",
        "CREATE UNIQUE INDEX uniq_pattern_harmonic_score ON pattern_harmonic_scores (setup_id, harmonic_type)",
    ),
    (
        "pattern_harmonic_scores",
        "idx_pattern_harmonic_scores_type_price",
        "CREATE INDEX idx_pattern_harmonic_scores_type_price ON pattern_harmonic_scores (harmonic_type, price_accuracy)",
    ),
    (
        "pattern_setups",
        "uniq_pattern_setups_setup_id",
        "CREATE UNIQUE INDEX uniq_pattern_setups_setup_id ON pattern_setups (setup_id)",
    ),
    (
        "pattern_setups",
        "idx_pattern_setups_pattern_id",
        "CREATE INDEX idx_pattern_setups_pattern_id ON pattern_setups (pattern_id)",
    ),
    (
        "pattern_setups",
        "idx_pattern_setups_x_bars_left",
        "CREATE INDEX idx_pattern_setups_x_bars_left ON pattern_setups (x_bars_left)",
    ),
    (
        "pattern_setups",
        "idx_pattern_setups_symbol_d_date",
        "CREATE INDEX idx_pattern_setups_symbol_d_date ON pattern_setups (symbol, d_date)",
    ),
    (
        "pattern_setups",
        "idx_pattern_setups_prop_strategy_id",
        "CREATE INDEX idx_pattern_setups_prop_strategy_id ON pattern_setups (prop_strategy_id)",
    ),
    (
        "pattern_outcomes_swing",
        "uniq_pattern_outcomes_swing_setup_id",
        "CREATE UNIQUE INDEX uniq_pattern_outcomes_swing_setup_id ON pattern_outcomes_swing (setup_id)",
    ),
    (
        "pattern_outcomes_swing",
        "idx_pattern_outcomes_swing_strategy_id",
        "CREATE INDEX idx_pattern_outcomes_swing_strategy_id ON pattern_outcomes_swing (swing_strategy_id)",
    ),
    (
        "pattern_outcomes_prop",
        "uniq_pattern_outcomes_prop_row_id",
        "CREATE UNIQUE INDEX uniq_pattern_outcomes_prop_row_id ON pattern_outcomes_prop (outcome_row_id)",
    ),
    (
        "pattern_outcomes_prop",
        "idx_pattern_outcomes_prop_setup_id",
        "CREATE INDEX idx_pattern_outcomes_prop_setup_id ON pattern_outcomes_prop (setup_id)",
    ),
    (
        "pattern_outcomes_prop",
        "idx_pattern_outcomes_prop_strategy_id",
        "CREATE INDEX idx_pattern_outcomes_prop_strategy_id ON pattern_outcomes_prop (prop_strategy_id)",
    ),
    (
        "pattern_outcomes_prop",
        "idx_pattern_outcomes_prop_x_bars_left",
        "CREATE INDEX idx_pattern_outcomes_prop_x_bars_left ON pattern_outcomes_prop (x_bars_left)",
    ),
    (
        "pattern_outcomes_prop",
        "idx_pattern_outcomes_prop_pattern_id",
        "CREATE INDEX idx_pattern_outcomes_prop_pattern_id ON pattern_outcomes_prop (pattern_id, d_date)",
    ),
    (
        "pattern_outcomes_prop",
        "idx_pattern_outcomes_prop_group_detail",
        "CREATE INDEX idx_pattern_outcomes_prop_group_detail ON pattern_outcomes_prop (pattern_group_id, d_date, market, harmonic_type, size_bucket)",
    ),
    (
        "pattern_outcomes_prop",
        "idx_pattern_outcomes_prop_symbol_d_date",
        "CREATE INDEX idx_pattern_outcomes_prop_symbol_d_date ON pattern_outcomes_prop (symbol, d_date)",
    ),
    (
        "pattern_outcomes_prop",
        "idx_pattern_outcomes_prop_lookup",
        "CREATE INDEX idx_pattern_outcomes_prop_lookup ON pattern_outcomes_prop (outcome_model, market, harmonic_type, bin, has_reversal, reversal_type, size_bucket, time_bin)",
    ),
    (
        "pattern_outcomes_prop",
        "idx_pattern_outcomes_prop_target_ready",
        "CREATE INDEX idx_pattern_outcomes_prop_target_ready ON pattern_outcomes_prop (target_ready, entry_date)",
    ),
    (
        "pattern_outcomes_prop",
        "idx_pattern_outcomes_prop_contract_week",
        "CREATE INDEX idx_pattern_outcomes_prop_contract_week ON pattern_outcomes_prop (symbol, contract_week_index)",
    ),
];

const DROP_ONLY_REBUILD_INDEXES: [(&str, &str); 1] = [(
    "pattern_harmonic_scores",
    "idx_pattern_harmonic_scores_setup_price",
)];

#[derive(Clone, Copy)]
struct PatternOutputTables {
    pattern_setups: &'static str,
    harmonic_scores: &'static str,
    swing_outcomes: &'static str,
    prop_outcomes: &'static str,
}

impl PatternOutputTables {
    fn final_tables() -> Self {
        Self {
            pattern_setups: "pattern_setups",
            harmonic_scores: "pattern_harmonic_scores",
            swing_outcomes: "pattern_outcomes_swing",
            prop_outcomes: "pattern_outcomes_prop",
        }
    }

    fn build_tables() -> Self {
        Self {
            pattern_setups: "pattern_setups_build",
            harmonic_scores: "pattern_harmonic_scores_build",
            swing_outcomes: "pattern_outcomes_swing_build",
            prop_outcomes: "pattern_outcomes_prop_build",
        }
    }
}

#[derive(Clone, Copy)]
pub struct OutputWriteOptions {
    pub write_pattern_setups: bool,
    pub write_harmonic_scores: bool,
    pub write_swing_outcomes: bool,
    pub write_prop_outcomes: bool,
}

impl OutputWriteOptions {
    fn should_rebuild_indexes_for(self, table: &str) -> bool {
        match table {
            "pattern_setups" => self.write_pattern_setups,
            "pattern_harmonic_scores" => self.write_harmonic_scores,
            "pattern_outcomes_swing" => self.write_swing_outcomes,
            "pattern_outcomes_prop" => self.write_prop_outcomes,
            _ => true,
        }
    }
}

fn target_rebuild_table(table: &str, use_build_tables: bool) -> String {
    if use_build_tables {
        format!("{table}_build")
    } else {
        table.to_string()
    }
}

fn target_rebuild_index_sql(table: &str, create_sql: &str, use_build_tables: bool) -> String {
    if !use_build_tables {
        return create_sql.to_string();
    }

    create_sql.replacen(&format!(" ON {table} "), &format!(" ON {table}_build "), 1)
}

struct HarmonicPatternDefinition {
    harmonic_type: &'static str,
    display_name: &'static str,
    scanner_family: &'static str,
    is_enabled: bool,
    ab_xa_min: Option<f64>,
    ab_xa_max: Option<f64>,
    bc_ab_min: Option<f64>,
    bc_ab_max: Option<f64>,
    cd_bc_min: Option<f64>,
    cd_bc_max: Option<f64>,
    cd_xa_min: Option<f64>,
    cd_xa_max: Option<f64>,
    notes: &'static str,
}

const HARMONIC_PATTERN_DEFINITIONS: [HarmonicPatternDefinition; 11] = [
    HarmonicPatternDefinition {
        harmonic_type: "Bat",
        display_name: "Bat",
        scanner_family: "xabcd",
        is_enabled: true,
        ab_xa_min: Some(0.500),
        ab_xa_max: Some(0.500),
        bc_ab_min: Some(0.382),
        bc_ab_max: Some(0.382),
        cd_bc_min: Some(1.618),
        cd_bc_max: Some(1.618),
        cd_xa_min: Some(0.886),
        cd_xa_max: Some(0.886),
        notes: "Enabled in the current XABCD scanner.",
    },
    HarmonicPatternDefinition {
        harmonic_type: "AlternateBat",
        display_name: "Alternate Bat",
        scanner_family: "xabcd",
        is_enabled: true,
        ab_xa_min: Some(0.382),
        ab_xa_max: Some(0.382),
        bc_ab_min: Some(0.382),
        bc_ab_max: Some(0.382),
        cd_bc_min: Some(2.000),
        cd_bc_max: Some(2.000),
        cd_xa_min: Some(1.130),
        cd_xa_max: Some(1.130),
        notes: "Enabled in the current XABCD scanner.",
    },
    HarmonicPatternDefinition {
        harmonic_type: "Butterfly",
        display_name: "Butterfly",
        scanner_family: "xabcd",
        is_enabled: true,
        ab_xa_min: Some(0.786),
        ab_xa_max: Some(0.786),
        bc_ab_min: Some(0.382),
        bc_ab_max: Some(0.382),
        cd_bc_min: Some(1.618),
        cd_bc_max: Some(1.618),
        cd_xa_min: Some(1.272),
        cd_xa_max: Some(1.272),
        notes: "Enabled in the current XABCD scanner.",
    },
    HarmonicPatternDefinition {
        harmonic_type: "Gartley",
        display_name: "Gartley",
        scanner_family: "xabcd",
        is_enabled: true,
        ab_xa_min: Some(0.618),
        ab_xa_max: Some(0.618),
        bc_ab_min: Some(0.382),
        bc_ab_max: Some(0.382),
        cd_bc_min: Some(1.272),
        cd_bc_max: Some(1.272),
        cd_xa_min: Some(0.786),
        cd_xa_max: Some(0.786),
        notes: "Enabled in the current XABCD scanner.",
    },
    HarmonicPatternDefinition {
        harmonic_type: "Crab",
        display_name: "Crab",
        scanner_family: "xabcd",
        is_enabled: true,
        ab_xa_min: Some(0.382),
        ab_xa_max: Some(0.382),
        bc_ab_min: Some(0.382),
        bc_ab_max: Some(0.382),
        cd_bc_min: Some(2.618),
        cd_bc_max: Some(2.618),
        cd_xa_min: Some(1.618),
        cd_xa_max: Some(1.618),
        notes: "Enabled in the current XABCD scanner.",
    },
    HarmonicPatternDefinition {
        harmonic_type: "DeepCrab",
        display_name: "Deep Crab",
        scanner_family: "xabcd",
        is_enabled: true,
        ab_xa_min: Some(0.886),
        ab_xa_max: Some(0.886),
        bc_ab_min: Some(0.382),
        bc_ab_max: Some(0.382),
        cd_bc_min: Some(2.618),
        cd_bc_max: Some(2.618),
        cd_xa_min: Some(1.618),
        cd_xa_max: Some(1.618),
        notes: "Enabled in the current XABCD scanner.",
    },
    HarmonicPatternDefinition {
        harmonic_type: "Shark",
        display_name: "Shark",
        scanner_family: "xabcd",
        is_enabled: true,
        ab_xa_min: Some(0.500),
        ab_xa_max: Some(0.500),
        bc_ab_min: Some(1.130),
        bc_ab_max: Some(1.130),
        cd_bc_min: Some(1.618),
        cd_bc_max: Some(1.618),
        cd_xa_min: Some(0.886),
        cd_xa_max: Some(0.886),
        notes: "Enabled in the current XABCD scanner.",
    },
    HarmonicPatternDefinition {
        harmonic_type: "Cypher",
        display_name: "Cypher",
        scanner_family: "cypher",
        is_enabled: false,
        ab_xa_min: Some(0.382),
        ab_xa_max: Some(0.618),
        bc_ab_min: None,
        bc_ab_max: None,
        cd_bc_min: None,
        cd_bc_max: None,
        cd_xa_min: None,
        cd_xa_max: None,
        notes: "Disabled: needs C as XA extension and D as XC retracement.",
    },
    HarmonicPatternDefinition {
        harmonic_type: "FiveZero",
        display_name: "5-0",
        scanner_family: "five_zero",
        is_enabled: false,
        ab_xa_min: Some(1.130),
        ab_xa_max: Some(1.618),
        bc_ab_min: Some(1.618),
        bc_ab_max: Some(2.240),
        cd_bc_min: Some(0.500),
        cd_bc_max: Some(0.500),
        cd_xa_min: None,
        cd_xa_max: None,
        notes: "Disabled: needs its own 5-0 scanner validation.",
    },
    HarmonicPatternDefinition {
        harmonic_type: "ABCD",
        display_name: "ABCD",
        scanner_family: "abcd",
        is_enabled: false,
        ab_xa_min: None,
        ab_xa_max: None,
        bc_ab_min: Some(0.382),
        bc_ab_max: Some(0.886),
        cd_bc_min: Some(1.130),
        cd_bc_max: Some(2.618),
        cd_xa_min: None,
        cd_xa_max: None,
        notes: "Disabled: belongs in an ABCD scanner without the X leg.",
    },
    HarmonicPatternDefinition {
        harmonic_type: "ThreeDrives",
        display_name: "Three Drives",
        scanner_family: "three_drives",
        is_enabled: false,
        ab_xa_min: None,
        ab_xa_max: None,
        bc_ab_min: None,
        bc_ab_max: None,
        cd_bc_min: None,
        cd_bc_max: None,
        cd_xa_min: None,
        cd_xa_max: None,
        notes: "Disabled: needs a separate five-swing scanner.",
    },
];

fn structure_total_bars_expr() -> &'static str {
    "CAST(COALESCE(x_length, 0) + COALESCE(a_length, 0) + COALESCE(b_length, 0) + COALESCE(c_length, 0) AS DOUBLE)"
}

fn structure_sum_abc_expr() -> &'static str {
    "CAST(COALESCE(a_length, 0) + COALESCE(b_length, 0) + COALESCE(c_length, 0) AS DOUBLE)"
}

fn structure_pair_ratio_expr(left: &str, right: &str) -> String {
    format!(
        "CASE
            WHEN COALESCE({left}, 0) <= 0 OR COALESCE({right}, 0) <= 0 THEN NULL
            ELSE GREATEST(CAST({left} AS DOUBLE), CAST({right} AS DOUBLE))
                / LEAST(CAST({left} AS DOUBLE), CAST({right} AS DOUBLE))
        END",
        left = left,
        right = right,
    )
}

fn structure_balance_ratio_expr() -> String {
    let x_vs_pattern = format!(
        "CASE
            WHEN COALESCE(x_length, 0) <= 0 OR {sum_abc} <= 0 THEN NULL
            ELSE CAST(x_length AS DOUBLE) / {sum_abc}
        END",
        sum_abc = structure_sum_abc_expr(),
    );
    let a_vs_x = structure_pair_ratio_expr("a_length", "x_length");
    let b_vs_a = structure_pair_ratio_expr("b_length", "a_length");
    let c_vs_b = structure_pair_ratio_expr("c_length", "b_length");

    format!(
        "CASE
            WHEN {x_vs_pattern} IS NULL
                AND {a_vs_x} IS NULL
                AND {b_vs_a} IS NULL
                AND {c_vs_b} IS NULL THEN NULL
            ELSE GREATEST(
                COALESCE({x_vs_pattern}, 1.0),
                COALESCE({a_vs_x}, 1.0),
                COALESCE({b_vs_a}, 1.0),
                COALESCE({c_vs_b}, 1.0)
            )
        END",
        x_vs_pattern = x_vs_pattern,
        a_vs_x = a_vs_x,
        b_vs_a = b_vs_a,
        c_vs_b = c_vs_b,
    )
}

fn structure_size_bucket_expr(total_bars_expr: &str) -> String {
    format!(
        "CASE
            WHEN {total} <= 20 THEN 'Micro'
            WHEN {total} <= 60 THEN 'Small'
            WHEN {total} <= 180 THEN 'Normal'
            WHEN {total} <= 365 THEN 'Large'
            ELSE 'Massive'
        END",
        total = total_bars_expr,
    )
}

fn structure_size_bucket_order_expr(total_bars_expr: &str) -> String {
    format!(
        "CASE
            WHEN {total} <= 20 THEN 1
            WHEN {total} <= 60 THEN 2
            WHEN {total} <= 180 THEN 3
            WHEN {total} <= 365 THEN 4
            ELSE 5
        END",
        total = total_bars_expr,
    )
}

fn structure_balance_bucket_expr(balance_ratio_expr: &str) -> String {
    format!(
        "CASE
            WHEN {ratio} IS NULL THEN 'Unknown'
            WHEN {ratio} <= 1.25 THEN 'Tight'
            WHEN {ratio} <= 1.5 THEN 'Balanced'
            WHEN {ratio} <= 2.0 THEN 'Stretched'
            ELSE 'Distorted'
        END",
        ratio = balance_ratio_expr,
    )
}

fn structure_balance_bucket_order_expr(balance_ratio_expr: &str) -> String {
    format!(
        "CASE
            WHEN {ratio} IS NULL THEN 0
            WHEN {ratio} <= 1.25 THEN 1
            WHEN {ratio} <= 1.5 THEN 2
            WHEN {ratio} <= 2.0 THEN 3
            ELSE 4
        END",
        ratio = balance_ratio_expr,
    )
}

fn leg_accuracy_sql_expr(current_expr: &str, target_leg: f64) -> String {
    format!(
        "CASE
            WHEN COALESCE({current_expr}, 0) <= 0 THEN 0.0
            ELSE LEAST(
                GREATEST(
                    100.0 * (1.0 - ABS((CAST({current_expr} AS DOUBLE) / 100.0) - {target_leg}) / {target_leg}),
                    0.0
                ),
                100.0
            )
        END",
        current_expr = current_expr,
        target_leg = target_leg,
    )
}

fn harmonic_time_accuracy_sql_expr(ab_xa: f64, bc_ab: f64, cd_bc: f64, cd_xa: f64) -> String {
    format!(
        "(
            {ab_xa_expr}
            + {bc_ab_expr}
            + {cd_bc_expr}
            + {cd_xa_expr}
        ) / 4.0",
        ab_xa_expr = leg_accuracy_sql_expr("trade_ab_bar_retracement", ab_xa),
        bc_ab_expr = leg_accuracy_sql_expr("trade_bc_bar_retracement", bc_ab),
        cd_bc_expr = leg_accuracy_sql_expr("trade_cd_bc_bar_retracement", cd_bc),
        cd_xa_expr = leg_accuracy_sql_expr("trade_cd_xa_bar_retracement", cd_xa),
    )
}

fn time_bin_expr(time_accuracy_expr: &str) -> String {
    format!(
        "CASE
            WHEN {time_accuracy_expr} IS NULL THEN NULL
            WHEN {time_accuracy_expr} <= 10 THEN '0-10'
            WHEN {time_accuracy_expr} <= 20 THEN '10-20'
            WHEN {time_accuracy_expr} <= 30 THEN '20-30'
            WHEN {time_accuracy_expr} <= 40 THEN '30-40'
            WHEN {time_accuracy_expr} <= 50 THEN '40-50'
            WHEN {time_accuracy_expr} <= 60 THEN '50-60'
            WHEN {time_accuracy_expr} <= 70 THEN '60-70'
            WHEN {time_accuracy_expr} <= 80 THEN '70-80'
            WHEN {time_accuracy_expr} <= 90 THEN '80-90'
            ELSE '90-100'
        END",
        time_accuracy_expr = time_accuracy_expr,
    )
}

fn dominant_harmonic_bin_expr() -> String {
    let accuracy_expr = dominant_harmonic_accuracy_expr();
    format!(
        "CASE
            WHEN {accuracy_expr} <= 10 THEN '0-10'
            WHEN {accuracy_expr} <= 20 THEN '10-20'
            WHEN {accuracy_expr} <= 30 THEN '20-30'
            WHEN {accuracy_expr} <= 40 THEN '30-40'
            WHEN {accuracy_expr} <= 50 THEN '40-50'
            WHEN {accuracy_expr} <= 60 THEN '50-60'
            WHEN {accuracy_expr} <= 70 THEN '60-70'
            WHEN {accuracy_expr} <= 80 THEN '70-80'
            WHEN {accuracy_expr} <= 90 THEN '80-90'
            ELSE '90-100'
        END"
    )
}

fn dominant_harmonic_accuracy_expr() -> String {
    let accuracy_terms = HARMONIC_SCORE_COLUMNS
        .iter()
        .map(|(_, price_column, _)| format!("COALESCE(CAST({price_column} AS DOUBLE), 0.0)"))
        .collect::<Vec<_>>()
        .join(", ");

    format!("GREATEST({accuracy_terms})")
}

fn dominant_harmonic_type_expr() -> String {
    let mut cases = Vec::new();

    for (harmonic_type, price_column, _) in HARMONIC_SCORE_COLUMNS.iter() {
        let current_expr = format!("COALESCE(CAST({price_column} AS DOUBLE), 0.0)");
        let comparisons = HARMONIC_SCORE_COLUMNS
            .iter()
            .filter(|(_, other_price_column, _)| other_price_column != price_column)
            .map(|(_, other_price_column, _)| {
                format!("{current_expr} >= COALESCE(CAST({other_price_column} AS DOUBLE), 0.0)")
            })
            .collect::<Vec<_>>()
            .join(" AND ");

        cases.push(format!("WHEN {comparisons} THEN '{harmonic_type}'"));
    }

    format!("CASE {} ELSE 'Shark' END", cases.join(" "))
}

fn dominant_harmonic_time_accuracy_expr() -> String {
    let mut cases = Vec::new();

    for (_, price_column, time_column) in HARMONIC_SCORE_COLUMNS.iter() {
        let current_expr = format!("COALESCE(CAST({price_column} AS DOUBLE), 0.0)");
        let comparisons = HARMONIC_SCORE_COLUMNS
            .iter()
            .filter(|(_, other_price_column, _)| other_price_column != price_column)
            .map(|(_, other_price_column, _)| {
                format!("{current_expr} >= COALESCE(CAST({other_price_column} AS DOUBLE), 0.0)")
            })
            .collect::<Vec<_>>()
            .join(" AND ");

        cases.push(format!(
            "WHEN {comparisons} THEN COALESCE(CAST({time_column} AS DOUBLE), 0.0)"
        ));
    }

    format!(
        "CASE {} ELSE COALESCE(CAST(shark_time_accuracy AS DOUBLE), 0.0) END",
        cases.join(" ")
    )
}

fn trend_bucket_expr(trend_column: &str) -> String {
    format!(
        "CASE
            WHEN {trend_column} IS TRUE THEN 'Bullish'
            WHEN {trend_column} IS FALSE THEN 'Bearish'
            ELSE 'Unknown'
        END"
    )
}

fn x_strictness_expr(x_bars_left_column: &str, x_length_column: &str) -> String {
    format!(
        "CASE
            WHEN COALESCE({x_length_column}, 0) <= 0 THEN 'Loose'
            WHEN COALESCE({x_bars_left_column}, 0) >= COALESCE({x_length_column}, 0) THEN 'Strict'
            WHEN COALESCE({x_bars_left_column}, 0) * 2 >= COALESCE({x_length_column}, 0) THEN 'Normal'
            ELSE 'Loose'
        END"
    )
}

struct PropFamilySpec {
    family_name: &'static str,
    dimensions: &'static [&'static str],
}

const PROP_FAMILY_DIMENSIONS: [&str; 11] = [
    "outcome_model",
    "market",
    "harmonic_type",
    "bin",
    "reversal_type",
    "size_bucket",
    "time_bin",
    "x_strictness",
    "three_month_trend",
    "six_month_trend",
    "twelve_month_trend",
];

const PROP_FAMILY_SPECS: [PropFamilySpec; 1] = [PropFamilySpec {
    family_name: "concrete_route",
    dimensions: &[
        "outcome_model",
        "market",
        "harmonic_type",
        "bin",
        "reversal_type",
        "size_bucket",
        "time_bin",
        "x_strictness",
        "three_month_trend",
        "six_month_trend",
        "twelve_month_trend",
    ],
}];

fn prop_family_dimension_expr(_spec: &PropFamilySpec, dimension: &str) -> String {
    dimension.to_string()
}

fn prop_family_included_dimensions(spec: &PropFamilySpec) -> String {
    if spec.dimensions.is_empty() {
        "none".to_string()
    } else {
        spec.dimensions.join(",")
    }
}

fn concrete_prop_family_key_expr(table_alias: &str) -> String {
    let x_strictness = x_strictness_expr(
        &format!("{table_alias}.x_bars_left"),
        &format!("{table_alias}.x_length"),
    );

    format!(
        "LEFT(MD5(CONCAT_WS('|',
            'prop-family-v3',
            'concrete_route',
            LOWER({alias}.outcome_model),
            LOWER({alias}.market),
            LOWER({alias}.harmonic_type),
            LOWER({alias}.bin),
            LOWER(COALESCE(NULLIF({alias}.reversal_type, ''), 'None')),
            LOWER({alias}.size_bucket),
            LOWER({alias}.time_bin),
            LOWER({x_strictness}),
            LOWER({alias}.three_month_trend),
            LOWER({alias}.six_month_trend),
            LOWER({alias}.twelve_month_trend)
        )), 16)",
        alias = table_alias,
        x_strictness = x_strictness,
    )
}

fn swing_strategy_id_expr(
    harmonic_type_expr: &str,
    bin_expr: &str,
    time_bin_expr: &str,
    reversal_type_expr: &str,
) -> String {
    let size_bucket_expr = structure_size_bucket_expr(structure_total_bars_expr());
    let three_month_trend_expr = trend_bucket_expr("three_month");
    let six_month_trend_expr = trend_bucket_expr("six_month");
    let twelve_month_trend_expr = trend_bucket_expr("twelve_month");

    format!(
        "LEFT(MD5(CONCAT_WS('|',
            LOWER(market),
            LOWER({harmonic_type_expr}),
            LOWER({bin_expr}),
            LOWER({reversal_type_expr}),
            LOWER({size_bucket_expr}),
            LOWER({time_bin_expr}),
            LOWER({three_month_trend_expr}),
            LOWER({six_month_trend_expr}),
            LOWER({twelve_month_trend_expr})
        )), 16)"
    )
}

fn sma_slope_trend(
    prefix_sums: &[f64],
    candle_index: usize,
    sma_period: usize,
    slope_lookback: usize,
) -> Option<bool> {
    if candle_index + 1 < sma_period + slope_lookback {
        return None;
    }

    let current_end = candle_index + 1;
    let current_start = current_end - sma_period;
    let previous_end = current_end - slope_lookback;
    let previous_start = previous_end - sma_period;
    let current_sum = prefix_sums[current_end] - prefix_sums[current_start];
    let previous_sum = prefix_sums[previous_end] - prefix_sums[previous_start];

    Some(current_sum >= previous_sum)
}

fn apply_sma_trends_to_candles(candles: &mut [Candle]) {
    let mut prefix_sums = Vec::with_capacity(candles.len() + 1);
    prefix_sums.push(0.0);

    for candle in candles.iter() {
        prefix_sums.push(prefix_sums.last().copied().unwrap_or(0.0) + candle.close);
    }

    for index in 0..candles.len() {
        candles[index].three_month = sma_slope_trend(
            &prefix_sums,
            index,
            THREE_MONTH_SMA_PERIOD,
            TREND_SMA_SLOPE_LOOKBACK,
        );
        candles[index].six_month = sma_slope_trend(
            &prefix_sums,
            index,
            SIX_MONTH_SMA_PERIOD,
            TREND_SMA_SLOPE_LOOKBACK,
        );
        candles[index].twelve_month = sma_slope_trend(
            &prefix_sums,
            index,
            TWELVE_MONTH_SMA_PERIOD,
            TREND_SMA_SLOPE_LOOKBACK,
        );
    }
}

pub struct Database {
    pub pool: MySqlPool,
}
#[allow(non_camel_case_types)]
#[derive(Serialize)]
pub struct XABCD_CSV {
    symbol: String,
    pattern_id: String,
    x_bars_left: i64,
    x_date: String,
    #[serde(serialize_with = "two_decimals")]
    x_open: f64,
    #[serde(serialize_with = "two_decimals")]
    x_high: f64,
    #[serde(serialize_with = "two_decimals")]
    x_low: f64,
    #[serde(serialize_with = "two_decimals")]
    x_close: f64,

    x_length: i64,

    #[serde(serialize_with = "two_decimals")]
    x_min_max: f64,

    a_date: String,

    #[serde(serialize_with = "two_decimals")]
    a_open: f64,
    #[serde(serialize_with = "two_decimals")]
    a_high: f64,
    #[serde(serialize_with = "two_decimals")]
    a_low: f64,
    #[serde(serialize_with = "two_decimals")]
    a_close: f64,

    a_length: i64,

    #[serde(serialize_with = "two_decimals")]
    a_min_max: f64,
    #[serde(serialize_with = "two_decimals")]
    xa_price_length: f64,

    b_date: String,

    #[serde(serialize_with = "two_decimals")]
    b_open: f64,
    #[serde(serialize_with = "two_decimals")]
    b_high: f64,
    #[serde(serialize_with = "two_decimals")]
    b_low: f64,
    #[serde(serialize_with = "two_decimals")]
    b_close: f64,

    b_length: i64,

    #[serde(serialize_with = "two_decimals")]
    b_min_max: f64,
    #[serde(serialize_with = "two_decimals")]
    ab_price_length: f64,

    c_date: String,

    #[serde(serialize_with = "two_decimals")]
    c_open: f64,
    #[serde(serialize_with = "two_decimals")]
    c_high: f64,
    #[serde(serialize_with = "two_decimals")]
    c_low: f64,
    #[serde(serialize_with = "two_decimals")]
    c_close: f64,

    c_length: i64,

    #[serde(serialize_with = "two_decimals")]
    c_min_max: f64,
    #[serde(serialize_with = "two_decimals")]
    bc_price_length: f64,

    d_date: String,

    #[serde(serialize_with = "two_decimals")]
    d_open: f64,
    #[serde(serialize_with = "two_decimals")]
    d_high: f64,
    #[serde(serialize_with = "two_decimals")]
    d_low: f64,
    #[serde(serialize_with = "two_decimals")]
    d_close: f64,

    d_confirm_date: String,
    target_ready: bool,
    target_date: Option<String>,
    target_open: Option<f64>,
    target_high: Option<f64>,
    target_low: Option<f64>,
    target_close: Option<f64>,
    target_volume: Option<i64>,
    target_is_green: Option<bool>,
    target_close_vs_open_pct: Option<f64>,
    target_high_vs_open_pct: Option<f64>,
    target_low_vs_open_pct: Option<f64>,
    target_range_pct: Option<f64>,
    target_breaks_d_high: Option<bool>,
    target_breaks_d_low: Option<bool>,

    d_length: i64,
    full_pattern_length: i64,

    #[serde(serialize_with = "two_decimals")]
    d_min_max: f64,

    #[serde(serialize_with = "two_decimals")]
    cd_price_length: f64,

    trade_open: bool,

    #[serde(serialize_with = "two_decimals")]
    trade_risk_exit_price: f64,
    #[serde(serialize_with = "two_decimals")]
    trade_reward_exit_price: f64,
    #[serde(serialize_with = "two_decimals")]
    trade_enter_price: f64,
    #[serde(serialize_with = "two_decimals")]
    trade_current_price: f64,

    trade_length: i64,

    #[serde(serialize_with = "two_decimals")]
    trade_pnl: f64,

    trade_result: i32,
    trade_date: String,
    trade_symbol: String,

    #[serde(serialize_with = "two_decimals")]
    trade_ab_price_retracement: f64,

    #[serde(serialize_with = "two_decimals")]
    trade_bc_price_retracement: f64,

    #[serde(serialize_with = "two_decimals")]
    trade_cd_xa_price_retracement: f64,

    #[serde(serialize_with = "two_decimals")]
    trade_cd_price_retracement: f64,

    #[serde(serialize_with = "two_decimals")]
    trade_ab_bar_retracement: f64,

    #[serde(serialize_with = "two_decimals")]
    trade_bc_bar_retracement: f64,

    #[serde(serialize_with = "two_decimals")]
    trade_cd_bar_retracement: f64,

    #[serde(serialize_with = "two_decimals")]
    trade_cd_bc_bar_retracement: f64,

    #[serde(serialize_with = "two_decimals")]
    trade_cd_xa_bar_retracement: f64,

    #[serde(serialize_with = "two_decimals")]
    trade_cd_bc_price_retracement: f64,

    #[serde(serialize_with = "two_decimals")]
    trade_snr: f64,
    trade_year: i64,
    trade_month: i64,
    trade_day: i64,

    reversal_type: ReversalType,
    bullish_key_reversal: bool,
    bearish_key_reversal: bool,
    bullish_engulfing: bool,
    bearish_engulfing: bool,
    bullish_outside_reversal: bool,
    bearish_outside_reversal: bool,
    hammer: bool,
    shooting_star: bool,
    morning_star: bool,
    evening_star: bool,
    three_white_soldiers: bool,
    three_black_crows: bool,
    market: Market,
    three_month: Option<bool>,
    six_month: Option<bool>,
    twelve_month: Option<bool>,
    pattern_group_id: String,
    prop_strategy_id: String,
    swing_strategy_id: String,
    harmonic_type: String,
    route_harmonic_type: String,
    route_bin: String,
    route_time_bin: String,
    #[serde(serialize_with = "two_decimals")]
    bat_accuracy: f64,
    #[serde(serialize_with = "two_decimals")]
    alternate_bat_accuracy: f64,
    shark_accuracy: f64,
    butterfly_accuracy: f64,
    gartley_accuracy: f64,
    crab_accuracy: f64,
    deep_crab_accuracy: f64,
    #[serde(serialize_with = "two_decimals")]
    time_accuracy: f64,
    #[serde(serialize_with = "two_decimals")]
    bat_time_accuracy: f64,
    #[serde(serialize_with = "two_decimals")]
    alternate_bat_time_accuracy: f64,
    #[serde(serialize_with = "two_decimals")]
    butterfly_time_accuracy: f64,
    #[serde(serialize_with = "two_decimals")]
    gartley_time_accuracy: f64,
    #[serde(serialize_with = "two_decimals")]
    crab_time_accuracy: f64,
    #[serde(serialize_with = "two_decimals")]
    deep_crab_time_accuracy: f64,
    #[serde(serialize_with = "two_decimals")]
    shark_time_accuracy: f64,
}

fn build_pattern_setup_id(csv: &XABCD_CSV) -> String {
    if !csv.pattern_id.is_empty() {
        return csv.pattern_id.clone();
    }

    let canonical_key = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        format!("{:?}", csv.market).to_lowercase(),
        csv.symbol,
        csv.x_date,
        format!("{:.2}", csv.x_min_max),
        csv.a_date,
        format!("{:.2}", csv.a_min_max),
        csv.b_date,
        format!("{:.2}", csv.b_min_max),
        csv.c_date,
        format!("{:.2}", csv.c_min_max),
        csv.d_date,
        format!("{:.2}", csv.d_min_max),
    );

    let hash = format!("{:x}", md5::compute(canonical_key.as_bytes()));
    hash[..24].to_string()
}

fn trend_label(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "Bullish",
        Some(false) => "Bearish",
        None => "Unknown",
    }
}

fn route_size_bucket(x_length: i64, a_length: i64, b_length: i64, c_length: i64) -> &'static str {
    let total_bars = x_length + a_length + b_length + c_length;

    if total_bars <= 20 {
        "Micro"
    } else if total_bars <= 60 {
        "Small"
    } else if total_bars <= 180 {
        "Normal"
    } else if total_bars <= 365 {
        "Large"
    } else {
        "Massive"
    }
}

fn build_swing_strategy_id(csv: &XABCD_CSV) -> String {
    let canonical_key = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}",
        format!("{:?}", csv.market).to_lowercase(),
        csv.route_harmonic_type.to_lowercase(),
        csv.route_bin.to_lowercase(),
        format!("{:?}", csv.reversal_type).to_lowercase(),
        route_size_bucket(csv.x_length, csv.a_length, csv.b_length, csv.c_length).to_lowercase(),
        csv.route_time_bin.to_lowercase(),
        trend_label(csv.three_month).to_lowercase(),
        trend_label(csv.six_month).to_lowercase(),
        trend_label(csv.twelve_month).to_lowercase(),
    );

    let hash = format!("{:x}", md5::compute(canonical_key.as_bytes()));
    hash[..16].to_string()
}

fn harmonic_score_values(csv: &XABCD_CSV) -> [(&'static str, f64, f64); 7] {
    [
        ("Bat", csv.bat_accuracy, csv.bat_time_accuracy),
        (
            "AlternateBat",
            csv.alternate_bat_accuracy,
            csv.alternate_bat_time_accuracy,
        ),
        (
            "Butterfly",
            csv.butterfly_accuracy,
            csv.butterfly_time_accuracy,
        ),
        ("Gartley", csv.gartley_accuracy, csv.gartley_time_accuracy),
        ("Crab", csv.crab_accuracy, csv.crab_time_accuracy),
        (
            "DeepCrab",
            csv.deep_crab_accuracy,
            csv.deep_crab_time_accuracy,
        ),
        ("Shark", csv.shark_accuracy, csv.shark_time_accuracy),
    ]
}

fn pattern_id_expr() -> &'static str {
    "LEFT(MD5(CONCAT_WS('|', \
        LOWER(market), \
        symbol, \
        x_date, CAST(CAST(ROUND(x_min_max, 2) AS DECIMAL(18,2)) AS CHAR), \
        a_date, CAST(CAST(ROUND(a_min_max, 2) AS DECIMAL(18,2)) AS CHAR), \
        b_date, CAST(CAST(ROUND(b_min_max, 2) AS DECIMAL(18,2)) AS CHAR), \
        c_date, CAST(CAST(ROUND(c_min_max, 2) AS DECIMAL(18,2)) AS CHAR), \
        d_date, CAST(CAST(ROUND(d_min_max, 2) AS DECIMAL(18,2)) AS CHAR) \
    )), 24)"
}

fn prop_result_value(csv: &XABCD_CSV) -> i32 {
    prop_result_from_market_target(csv.market, csv.target_is_green)
}

fn prop_result_from_market_target(market: Market, target_is_green: Option<bool>) -> i32 {
    match target_is_green {
        None => 0,
        Some(is_green) => match market {
            Market::Bullish => {
                if is_green {
                    1
                } else {
                    2
                }
            }
            Market::Bearish => {
                if is_green {
                    2
                } else {
                    1
                }
            }
        },
    }
}

#[derive(sqlx::FromRow)]
struct AccuracyBinCacheRow {
    source_scope: String,
    market_scope: String,
    trade_result_scope: i32,
    harmonic_type: String,
    bin: String,
    count: i64,
    total_count: i64,
    open_count: i64,
    win_count: i64,
    loss_count: i64,
    avg_return: f64,
    expectancy: f64,
    win_rate: f64,
    closed_rate: f64,
    avg_win: f64,
    avg_loss: f64,
}

struct AccuracyBinRollupRow {
    rollup_year: i32,
    market: String,
    trade_result_value: i32,
    source_scope: String,
    harmonic_type: String,
    bin: String,
    total_count: i64,
    closed_count: i64,
    open_count: i64,
    win_count: i64,
    loss_count: i64,
    return_sum: f64,
    return_count: i64,
    expectancy_sum: f64,
    expectancy_count: i64,
    win_return_sum: f64,
    win_return_count: i64,
    loss_return_sum: f64,
    loss_return_count: i64,
}

#[derive(sqlx::FromRow)]
struct AccuracySourceRow {
    d_date: NaiveDate,
    market: String,
    trade_result: Option<i32>,
    trade_pnl: Option<f64>,
    harmonic_type: String,
    price_accuracy: Option<f64>,
}

#[derive(sqlx::FromRow)]
struct MarketDateRange {
    market: String,
    min_d_date: Option<NaiveDate>,
    max_d_date: Option<NaiveDate>,
}

#[derive(Default, Clone)]
struct AccuracyBinAccumulator {
    total_count: i64,
    closed_count: i64,
    open_count: i64,
    win_count: i64,
    loss_count: i64,
    avg_return_sum: f64,
    avg_return_samples: i64,
    expectancy_sum: f64,
    expectancy_samples: i64,
    avg_win_sum: f64,
    avg_win_samples: i64,
    avg_loss_sum: f64,
    avg_loss_samples: i64,
}

impl AccuracyBinAccumulator {
    fn record(&mut self, trade_result: Option<i32>, trade_pnl: Option<f64>) {
        self.total_count += 1;

        if let Some(return_pct) = trade_pnl {
            self.avg_return_sum += return_pct;
            self.avg_return_samples += 1;
        }

        match trade_result {
            Some(1) => {
                self.closed_count += 1;
                self.win_count += 1;

                if let Some(return_pct) = trade_pnl {
                    self.expectancy_sum += return_pct;
                    self.expectancy_samples += 1;
                    self.avg_win_sum += return_pct;
                    self.avg_win_samples += 1;
                }
            }
            Some(2) => {
                self.closed_count += 1;
                self.loss_count += 1;

                if let Some(return_pct) = trade_pnl {
                    self.expectancy_sum += return_pct;
                    self.expectancy_samples += 1;
                    self.avg_loss_sum += return_pct;
                    self.avg_loss_samples += 1;
                }
            }
            _ => {
                self.open_count += 1;
            }
        }
    }
}

type AccuracyCacheKey = (String, String, i32, String, String);
type AccuracyRollupKey = (i32, String, i32, String, String, String);

const CACHE_SOURCE_SCOPES: [&str; 1] = ["all_patterns"];
const CACHE_MARKET_SCOPES: [&str; 3] = ["All", "Bullish", "Bearish"];
const CACHE_TRADE_RESULT_SCOPES: [i32; 4] = [-1, 0, 1, 2];
const CACHE_HARMONIC_TYPES: [&str; 7] = [
    "Bat",
    "AlternateBat",
    "Butterfly",
    "Gartley",
    "Crab",
    "DeepCrab",
    "Shark",
];
const CACHE_BINS: [&str; 10] = [
    "0-10", "10-20", "20-30", "30-40", "40-50", "50-60", "60-70", "70-80", "80-90", "90-100",
];

fn accuracy_bin_label(accuracy: f64) -> &'static str {
    if accuracy <= 10.0 {
        "0-10"
    } else if accuracy <= 20.0 {
        "10-20"
    } else if accuracy <= 30.0 {
        "20-30"
    } else if accuracy <= 40.0 {
        "30-40"
    } else if accuracy <= 50.0 {
        "40-50"
    } else if accuracy <= 60.0 {
        "50-60"
    } else if accuracy <= 70.0 {
        "60-70"
    } else if accuracy <= 80.0 {
        "70-80"
    } else if accuracy <= 90.0 {
        "80-90"
    } else {
        "90-100"
    }
}

fn matching_market_scopes(market: &str) -> [&str; 2] {
    match market {
        "Bullish" => ["All", "Bullish"],
        "Bearish" => ["All", "Bearish"],
        _ => ["All", "All"],
    }
}

fn matching_trade_result_scopes(trade_result: Option<i32>) -> [i32; 2] {
    match trade_result {
        Some(0) => [-1, 0],
        Some(1) => [-1, 1],
        Some(2) => [-1, 2],
        _ => [-1, -1],
    }
}

fn update_accuracy_cache(
    aggregates: &mut HashMap<AccuracyCacheKey, AccuracyBinAccumulator>,
    source_scope: &str,
    harmonic_type: &str,
    accuracy: f64,
    market: &str,
    trade_result: Option<i32>,
    trade_pnl: Option<f64>,
) {
    let bin = accuracy_bin_label(accuracy);
    let market_scopes = matching_market_scopes(market);
    let trade_result_scopes = matching_trade_result_scopes(trade_result);

    for market_scope in market_scopes {
        if market_scope == "All" || market_scope == market {
            for trade_result_scope in trade_result_scopes {
                if trade_result_scope == -1 || Some(trade_result_scope) == trade_result {
                    let aggregate = aggregates
                        .entry((
                            source_scope.to_string(),
                            market_scope.to_string(),
                            trade_result_scope,
                            harmonic_type.to_string(),
                            bin.to_string(),
                        ))
                        .or_default();

                    aggregate.record(trade_result, trade_pnl);
                }
            }
        }
    }
}

fn market_label(market: Market) -> &'static str {
    match market {
        Market::Bullish => "Bullish",
        Market::Bearish => "Bearish",
    }
}

fn update_accuracy_rollup(
    aggregates: &mut HashMap<AccuracyRollupKey, AccuracyBinAccumulator>,
    rollup_year: i32,
    market: &str,
    trade_result_value: i32,
    source_scope: &str,
    harmonic_type: &str,
    accuracy: f64,
    trade_result: Option<i32>,
    trade_pnl: Option<f64>,
) {
    let bin = accuracy_bin_label(accuracy).to_string();
    let key = (
        rollup_year,
        market.to_string(),
        trade_result_value,
        source_scope.to_string(),
        harmonic_type.to_string(),
        bin,
    );

    aggregates
        .entry(key)
        .or_default()
        .record(trade_result, trade_pnl);
}

fn finalize_accuracy_cache_rows(
    aggregates: HashMap<AccuracyCacheKey, AccuracyBinAccumulator>,
) -> Vec<AccuracyBinCacheRow> {
    aggregates
        .into_iter()
        .map(
            |((source_scope, market_scope, trade_result_scope, harmonic_type, bin), aggregate)| {
                let avg_return = if aggregate.avg_return_samples > 0 {
                    aggregate.avg_return_sum / aggregate.avg_return_samples as f64
                } else {
                    0.0
                };
                let expectancy = if aggregate.expectancy_samples > 0 {
                    aggregate.expectancy_sum / aggregate.expectancy_samples as f64
                } else {
                    0.0
                };
                let win_rate = if aggregate.closed_count > 0 {
                    aggregate.win_count as f64 / aggregate.closed_count as f64
                } else {
                    0.0
                };
                let closed_rate = if aggregate.total_count > 0 {
                    aggregate.closed_count as f64 / aggregate.total_count as f64
                } else {
                    0.0
                };
                let avg_win = if aggregate.avg_win_samples > 0 {
                    aggregate.avg_win_sum / aggregate.avg_win_samples as f64
                } else {
                    0.0
                };
                let avg_loss = if aggregate.avg_loss_samples > 0 {
                    aggregate.avg_loss_sum / aggregate.avg_loss_samples as f64
                } else {
                    0.0
                };

                AccuracyBinCacheRow {
                    source_scope,
                    market_scope,
                    trade_result_scope,
                    harmonic_type,
                    bin,
                    count: aggregate.closed_count,
                    total_count: aggregate.total_count,
                    open_count: aggregate.open_count,
                    win_count: aggregate.win_count,
                    loss_count: aggregate.loss_count,
                    avg_return,
                    expectancy,
                    win_rate,
                    closed_rate,
                    avg_win,
                    avg_loss,
                }
            },
        )
        .collect()
}

fn finalize_accuracy_rollup_rows(
    aggregates: HashMap<AccuracyRollupKey, AccuracyBinAccumulator>,
) -> Vec<AccuracyBinRollupRow> {
    aggregates
        .into_iter()
        .map(
            |(
                (rollup_year, market, trade_result_value, source_scope, harmonic_type, bin),
                aggregate,
            )| {
                AccuracyBinRollupRow {
                    rollup_year,
                    market,
                    trade_result_value,
                    source_scope,
                    harmonic_type,
                    bin,
                    total_count: aggregate.total_count,
                    closed_count: aggregate.closed_count,
                    open_count: aggregate.open_count,
                    win_count: aggregate.win_count,
                    loss_count: aggregate.loss_count,
                    return_sum: aggregate.avg_return_sum,
                    return_count: aggregate.avg_return_samples,
                    expectancy_sum: aggregate.expectancy_sum,
                    expectancy_count: aggregate.expectancy_samples,
                    win_return_sum: aggregate.avg_win_sum,
                    win_return_count: aggregate.avg_win_samples,
                    loss_return_sum: aggregate.avg_loss_sum,
                    loss_return_count: aggregate.avg_loss_samples,
                }
            },
        )
        .collect()
}

impl Database {
    async fn table_exists(&self, table: &str) -> Result<bool, sqlx::Error> {
        let exists: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM INFORMATION_SCHEMA.TABLES
            WHERE TABLE_SCHEMA = DATABASE()
              AND TABLE_NAME = ?
            "#,
        )
        .bind(table)
        .fetch_one(&self.pool)
        .await?;

        Ok(exists > 0)
    }

    fn is_missing_index_error(error: &sqlx::Error) -> bool {
        match error {
            sqlx::Error::Database(db_error) => {
                db_error.message().contains("check that column/key exists")
                    || db_error.message().contains("Can't DROP")
                    || db_error.message().contains("doesn't exist")
            }
            _ => false,
        }
    }

    fn is_duplicate_index_error(error: &sqlx::Error) -> bool {
        match error {
            sqlx::Error::Database(db_error) => {
                db_error.message().contains("Duplicate key name")
                    || db_error.message().contains("already exists")
            }
            _ => false,
        }
    }

    async fn drop_rebuild_secondary_indexes_for_target(
        &self,
        use_build_tables: bool,
        output_options: OutputWriteOptions,
    ) -> Result<(), sqlx::Error> {
        for (table, index_name, _) in REBUILD_SECONDARY_INDEXES {
            if !output_options.should_rebuild_indexes_for(table) {
                continue;
            }

            let target_table = target_rebuild_table(table, use_build_tables);
            if !self.table_exists(&target_table).await? {
                continue;
            }

            let sql = format!("ALTER TABLE {target_table} DROP INDEX {index_name}");
            if let Err(error) = sqlx::query(&sql).execute(&self.pool).await {
                if !Self::is_missing_index_error(&error) {
                    return Err(error);
                }
            }
        }

        for (table, index_name) in DROP_ONLY_REBUILD_INDEXES {
            if !output_options.should_rebuild_indexes_for(table) {
                continue;
            }

            let target_table = target_rebuild_table(table, use_build_tables);
            if !self.table_exists(&target_table).await? {
                continue;
            }

            let sql = format!("ALTER TABLE {target_table} DROP INDEX {index_name}");
            if let Err(error) = sqlx::query(&sql).execute(&self.pool).await {
                if !Self::is_missing_index_error(&error) {
                    return Err(error);
                }
            }
        }

        Ok(())
    }

    pub async fn drop_rebuild_secondary_indexes(&self) -> Result<(), sqlx::Error> {
        self.drop_rebuild_secondary_indexes_for_target(
            false,
            OutputWriteOptions {
                write_harmonic_scores: true,
                write_pattern_setups: true,
                write_swing_outcomes: true,
                write_prop_outcomes: true,
            },
        )
        .await
    }

    pub async fn drop_build_rebuild_secondary_indexes(
        &self,
        output_options: OutputWriteOptions,
    ) -> Result<(), sqlx::Error> {
        self.drop_rebuild_secondary_indexes_for_target(true, output_options)
            .await
    }

    async fn recreate_rebuild_secondary_indexes_for_target(
        &self,
        use_build_tables: bool,
        output_options: OutputWriteOptions,
    ) -> Result<(), sqlx::Error> {
        for (table, _, create_sql) in REBUILD_SECONDARY_INDEXES {
            if !output_options.should_rebuild_indexes_for(table) {
                continue;
            }

            let target_table = target_rebuild_table(table, use_build_tables);
            if !self.table_exists(&target_table).await? {
                continue;
            }

            let target_sql = target_rebuild_index_sql(table, create_sql, use_build_tables);
            if let Err(error) = sqlx::query(&target_sql).execute(&self.pool).await {
                if !Self::is_duplicate_index_error(&error) {
                    return Err(error);
                }
            }
        }

        Ok(())
    }

    pub async fn recreate_rebuild_secondary_indexes(&self) -> Result<(), sqlx::Error> {
        self.recreate_rebuild_secondary_indexes_for_target(
            false,
            OutputWriteOptions {
                write_harmonic_scores: true,
                write_pattern_setups: true,
                write_swing_outcomes: true,
                write_prop_outcomes: true,
            },
        )
        .await
    }

    pub async fn recreate_build_rebuild_secondary_indexes(
        &self,
        output_options: OutputWriteOptions,
    ) -> Result<(), sqlx::Error> {
        self.recreate_rebuild_secondary_indexes_for_target(true, output_options)
            .await
    }

    pub async fn recreate_fast_rebuild_output_tables(&self) -> Result<(), sqlx::Error> {
        for table in [
            "pattern_outcomes_prop",
            "pattern_setups",
            "pattern_harmonic_scores",
        ] {
            let sql = format!("DROP TABLE IF EXISTS {table}");
            sqlx::query(&sql).execute(&self.pool).await?;
        }

        self.ensure_pattern_mode_tables().await?;

        for table in ["pattern_harmonic_scores"] {
            let sql = format!("DROP TABLE IF EXISTS {table}");
            sqlx::query(&sql).execute(&self.pool).await?;
        }

        Ok(())
    }

    fn build_output_tables(output_options: OutputWriteOptions) -> Vec<&'static str> {
        let mut tables = Vec::new();
        if output_options.write_pattern_setups {
            tables.push("pattern_setups");
        }
        if output_options.write_harmonic_scores {
            tables.push("pattern_harmonic_scores");
        }
        if output_options.write_swing_outcomes {
            tables.push("pattern_outcomes_swing");
        }
        tables.push("pattern_outcomes_prop");
        tables
    }

    async fn ensure_output_tables_for_build(
        &self,
        _output_options: OutputWriteOptions,
    ) -> Result<(), sqlx::Error> {
        self.ensure_pattern_mode_tables().await?;

        Ok(())
    }

    pub async fn recreate_build_output_tables(
        &self,
        output_options: OutputWriteOptions,
    ) -> Result<(), sqlx::Error> {
        self.ensure_output_tables_for_build(output_options).await?;

        for table in Self::build_output_tables(output_options) {
            let old_table = format!("{table}_old");
            let build_table = format!("{table}_build");
            sqlx::query(&format!("DROP TABLE IF EXISTS {old_table}"))
                .execute(&self.pool)
                .await?;
            sqlx::query(&format!("DROP TABLE IF EXISTS {build_table}"))
                .execute(&self.pool)
                .await?;
            sqlx::query(&format!("CREATE TABLE {build_table} LIKE {table}"))
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }

    pub async fn clear_generated_rollups_for_output_swap(&self) -> Result<(), sqlx::Error> {
        self.ensure_prop_strategy_family_yearly_table().await?;
        self.ensure_prop_strategy_family_summary_table().await?;
        self.ensure_dashboard_cache_state_table().await?;

        for table in [
            "prop_strategy_family_yearly",
            "prop_strategy_family_summary",
        ] {
            sqlx::query(&format!("TRUNCATE TABLE {table}"))
                .execute(&self.pool)
                .await?;
        }

        self.set_dashboard_cache_state(
            "prop_strategy_family_rollups",
            false,
            None,
            Some("cleared"),
        )
        .await?;

        Ok(())
    }

    pub async fn swap_build_output_tables(
        &self,
        output_options: OutputWriteOptions,
    ) -> Result<(), sqlx::Error> {
        let tables = Self::build_output_tables(output_options);

        for table in &tables {
            let build_table = format!("{table}_build");
            if !self.table_exists(&build_table).await? {
                return Err(sqlx::Error::Protocol(format!(
                    "build table {build_table} does not exist"
                )));
            }
        }

        for table in &tables {
            let old_table = format!("{table}_old");
            sqlx::query(&format!("DROP TABLE IF EXISTS {old_table}"))
                .execute(&self.pool)
                .await?;
        }

        let rename_sql = tables
            .iter()
            .flat_map(|table| {
                [
                    format!("{table} TO {table}_old"),
                    format!("{table}_build TO {table}"),
                ]
            })
            .collect::<Vec<_>>()
            .join(", ");

        sqlx::query(&format!("RENAME TABLE {rename_sql}"))
            .execute(&self.pool)
            .await?;

        self.clear_generated_rollups_for_output_swap().await?;

        for table in &tables {
            let old_table = format!("{table}_old");
            sqlx::query(&format!("DROP TABLE IF EXISTS {old_table}"))
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }

    pub async fn ensure_engine_phase_timings_table(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS engine_phase_timings (
                id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
                run_id VARCHAR(64) NOT NULL,
                symbol VARCHAR(32) NULL,
                phase VARCHAR(64) NOT NULL,
                row_count BIGINT NULL,
                duration_ms BIGINT NOT NULL,
                note VARCHAR(255) NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_engine_phase_timings_run_phase (run_id, phase),
                INDEX idx_engine_phase_timings_symbol (symbol, phase),
                INDEX idx_engine_phase_timings_created_at (created_at)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn record_engine_phase_timing(
        &self,
        run_id: &str,
        symbol: Option<&str>,
        phase: &str,
        row_count: Option<i64>,
        duration: Duration,
        note: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO engine_phase_timings (
                run_id,
                symbol,
                phase,
                row_count,
                duration_ms,
                note
            )
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(run_id)
        .bind(symbol)
        .bind(phase)
        .bind(row_count)
        .bind(duration.as_millis() as i64)
        .bind(note)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn record_optional_engine_phase_timing(
        &self,
        run_id: Option<&str>,
        phase: &str,
        row_count: Option<i64>,
        duration: Duration,
        note: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        if let Some(run_id) = run_id {
            self.record_engine_phase_timing(run_id, None, phase, row_count, duration, note)
                .await?;
        }

        Ok(())
    }

    pub async fn ensure_harmonic_pattern_definitions_table(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS harmonic_pattern_definitions (
                harmonic_type VARCHAR(32) NOT NULL,
                display_name VARCHAR(64) NOT NULL,
                scanner_family VARCHAR(32) NOT NULL,
                is_enabled BOOLEAN NOT NULL,
                ab_xa_min DOUBLE NULL,
                ab_xa_max DOUBLE NULL,
                bc_ab_min DOUBLE NULL,
                bc_ab_max DOUBLE NULL,
                cd_bc_min DOUBLE NULL,
                cd_bc_max DOUBLE NULL,
                cd_xa_min DOUBLE NULL,
                cd_xa_max DOUBLE NULL,
                notes VARCHAR(255) NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                PRIMARY KEY (harmonic_type),
                INDEX idx_harmonic_pattern_definitions_enabled (is_enabled, scanner_family),
                INDEX idx_harmonic_pattern_definitions_family (scanner_family)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        for definition in HARMONIC_PATTERN_DEFINITIONS {
            sqlx::query(
                r#"
                INSERT INTO harmonic_pattern_definitions (
                    harmonic_type, display_name, scanner_family, is_enabled,
                    ab_xa_min, ab_xa_max, bc_ab_min, bc_ab_max,
                    cd_bc_min, cd_bc_max, cd_xa_min, cd_xa_max, notes
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON DUPLICATE KEY UPDATE
                    display_name = VALUES(display_name),
                    scanner_family = VALUES(scanner_family),
                    is_enabled = VALUES(is_enabled),
                    ab_xa_min = VALUES(ab_xa_min),
                    ab_xa_max = VALUES(ab_xa_max),
                    bc_ab_min = VALUES(bc_ab_min),
                    bc_ab_max = VALUES(bc_ab_max),
                    cd_bc_min = VALUES(cd_bc_min),
                    cd_bc_max = VALUES(cd_bc_max),
                    cd_xa_min = VALUES(cd_xa_min),
                    cd_xa_max = VALUES(cd_xa_max),
                    notes = VALUES(notes)
                "#,
            )
            .bind(definition.harmonic_type)
            .bind(definition.display_name)
            .bind(definition.scanner_family)
            .bind(definition.is_enabled)
            .bind(definition.ab_xa_min)
            .bind(definition.ab_xa_max)
            .bind(definition.bc_ab_min)
            .bind(definition.bc_ab_max)
            .bind(definition.cd_bc_min)
            .bind(definition.cd_bc_max)
            .bind(definition.cd_xa_min)
            .bind(definition.cd_xa_max)
            .bind(definition.notes)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    async fn table_column_exists(&self, table: &str, column: &str) -> Result<bool, sqlx::Error> {
        let exists: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM INFORMATION_SCHEMA.COLUMNS
            WHERE TABLE_SCHEMA = DATABASE()
              AND TABLE_NAME = ?
              AND COLUMN_NAME = ?
            "#,
        )
        .bind(table)
        .bind(column)
        .fetch_one(&self.pool)
        .await?;

        Ok(exists > 0)
    }

    async fn add_column_if_missing(
        &self,
        table: &str,
        column: &str,
        column_sql: &str,
    ) -> Result<(), sqlx::Error> {
        if self.table_column_exists(table, column).await? {
            return Ok(());
        }

        let sql = format!("ALTER TABLE {table} ADD COLUMN {column_sql}");
        sqlx::query(&sql).execute(&self.pool).await?;
        Ok(())
    }

    async fn create_index_if_missing(&self, create_sql: &str) -> Result<(), sqlx::Error> {
        if let Err(error) = sqlx::query(create_sql).execute(&self.pool).await {
            let is_duplicate = match &error {
                sqlx::Error::Database(db_error) => {
                    db_error.message().contains("Duplicate key name")
                        || db_error.message().contains("already exists")
                }
                _ => false,
            };

            if !is_duplicate {
                return Err(error);
            }
        }

        Ok(())
    }

    async fn drop_column_if_exists(&self, table: &str, column: &str) -> Result<(), sqlx::Error> {
        if !self.table_column_exists(table, column).await? {
            return Ok(());
        }

        let sql = format!("ALTER TABLE {table} DROP COLUMN {column}");
        sqlx::query(&sql).execute(&self.pool).await?;
        Ok(())
    }

    async fn ensure_pattern_harmonic_scores_table(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS pattern_harmonic_scores (
                id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
                setup_id CHAR(24) NOT NULL,
                harmonic_type VARCHAR(24) NOT NULL,
                price_accuracy DOUBLE NOT NULL,
                time_accuracy DOUBLE NOT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                UNIQUE KEY uniq_pattern_harmonic_score (setup_id, harmonic_type),
                INDEX idx_pattern_harmonic_scores_type_price (harmonic_type, price_accuracy)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn migrate_pattern_setup_scores(&self) -> Result<(), sqlx::Error> {
        self.ensure_pattern_harmonic_scores_table().await?;

        for (harmonic_type, price_column, time_column) in HARMONIC_SCORE_COLUMNS {
            if !self
                .table_column_exists("pattern_setups", price_column)
                .await?
            {
                continue;
            }

            let time_expr = if self
                .table_column_exists("pattern_setups", time_column)
                .await?
            {
                format!("COALESCE({time_column}, 0.0)")
            } else {
                "0.0".to_string()
            };
            let sql = format!(
                r#"
                INSERT INTO pattern_harmonic_scores (
                    setup_id,
                    harmonic_type,
                    price_accuracy,
                    time_accuracy
                )
                SELECT
                    setup_id,
                    '{harmonic_type}',
                    COALESCE({price_column}, 0.0),
                    {time_expr}
                FROM pattern_setups
                WHERE {price_column} IS NOT NULL
                ON DUPLICATE KEY UPDATE
                    price_accuracy = VALUES(price_accuracy),
                    time_accuracy = VALUES(time_accuracy),
                    updated_at = CURRENT_TIMESTAMP
                "#,
            );

            sqlx::query(&sql).execute(&self.pool).await?;
        }

        Ok(())
    }

    async fn drop_pattern_setup_score_columns(&self) -> Result<(), sqlx::Error> {
        for column in [
            "bat_accuracy",
            "alternate_bat_accuracy",
            "butterfly_accuracy",
            "gartley_accuracy",
            "crab_accuracy",
            "deep_crab_accuracy",
            "shark_accuracy",
            "time_accuracy",
            "bat_time_accuracy",
            "alternate_bat_time_accuracy",
            "butterfly_time_accuracy",
            "gartley_time_accuracy",
            "crab_time_accuracy",
            "deep_crab_time_accuracy",
            "shark_time_accuracy",
        ] {
            self.drop_column_if_exists("pattern_setups", column).await?;
        }

        Ok(())
    }

    async fn upsert_harmonic_scores_from_xabcd(
        &self,
        setup_id_expr: &str,
    ) -> Result<(), sqlx::Error> {
        self.ensure_pattern_harmonic_scores_table().await?;

        for (harmonic_type, price_column, time_column) in HARMONIC_SCORE_COLUMNS {
            let sql = format!(
                r#"
                INSERT INTO pattern_harmonic_scores (
                    setup_id,
                    harmonic_type,
                    price_accuracy,
                    time_accuracy
                )
                SELECT
                    {setup_id_expr},
                    '{harmonic_type}',
                    COALESCE({price_column}, 0.0),
                    COALESCE({time_column}, 0.0)
                FROM xabcd_patterns
                WHERE {price_column} IS NOT NULL
                ON DUPLICATE KEY UPDATE
                    price_accuracy = VALUES(price_accuracy),
                    time_accuracy = VALUES(time_accuracy),
                    updated_at = CURRENT_TIMESTAMP
                "#,
            );

            sqlx::query(&sql).execute(&self.pool).await?;
        }

        Ok(())
    }

    pub async fn ensure_xabcd_patterns_table(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS xabcd_patterns (
                id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
                symbol VARCHAR(32) NOT NULL,
                pattern_id CHAR(24) NULL,
                x_bars_left BIGINT NOT NULL DEFAULT 0,
                x_date DATETIME NOT NULL,
                x_open DOUBLE NOT NULL,
                x_high DOUBLE NOT NULL,
                x_low DOUBLE NOT NULL,
                x_close DOUBLE NOT NULL,
                x_length BIGINT NOT NULL,
                x_min_max DOUBLE NOT NULL,
                a_date DATETIME NOT NULL,
                a_open DOUBLE NOT NULL,
                a_high DOUBLE NOT NULL,
                a_low DOUBLE NOT NULL,
                a_close DOUBLE NOT NULL,
                a_length BIGINT NOT NULL,
                a_min_max DOUBLE NOT NULL,
                b_date DATETIME NOT NULL,
                b_open DOUBLE NOT NULL,
                b_high DOUBLE NOT NULL,
                b_low DOUBLE NOT NULL,
                b_close DOUBLE NOT NULL,
                b_length BIGINT NOT NULL,
                b_min_max DOUBLE NOT NULL,
                c_date DATETIME NOT NULL,
                c_open DOUBLE NOT NULL,
                c_high DOUBLE NOT NULL,
                c_low DOUBLE NOT NULL,
                c_close DOUBLE NOT NULL,
                c_length BIGINT NOT NULL,
                c_min_max DOUBLE NOT NULL,
                d_date DATETIME NOT NULL,
                d_open DOUBLE NOT NULL,
                d_high DOUBLE NOT NULL,
                d_low DOUBLE NOT NULL,
                d_close DOUBLE NOT NULL,
                d_confirm_date DATE NULL,
                target_ready BOOLEAN NULL,
                target_date DATE NULL,
                target_open DOUBLE NULL,
                target_high DOUBLE NULL,
                target_low DOUBLE NULL,
                target_close DOUBLE NULL,
                target_volume BIGINT NULL,
                target_is_green BOOLEAN NULL,
                target_close_vs_open_pct DOUBLE NULL,
                target_high_vs_open_pct DOUBLE NULL,
                target_low_vs_open_pct DOUBLE NULL,
                target_range_pct DOUBLE NULL,
                target_breaks_d_high BOOLEAN NULL,
                target_breaks_d_low BOOLEAN NULL,
                d_length BIGINT NOT NULL,
                full_pattern_length BIGINT NULL,
                d_min_max DOUBLE NOT NULL,
                trade_open BOOLEAN NOT NULL,
                trade_risk_exit_price DOUBLE NOT NULL,
                trade_reward_exit_price DOUBLE NOT NULL,
                trade_enter_price DOUBLE NOT NULL,
                trade_current_price DOUBLE NOT NULL,
                trade_length BIGINT NOT NULL,
                trade_pnl DOUBLE NOT NULL,
                trade_result INT NOT NULL,
                trade_date DATE NOT NULL,
                trade_symbol VARCHAR(32) NOT NULL,
                trade_ab_price_retracement DOUBLE NOT NULL,
                trade_bc_price_retracement DOUBLE NOT NULL,
                trade_cd_xa_price_retracement DOUBLE NOT NULL,
                trade_cd_price_retracement DOUBLE NOT NULL,
                trade_ab_bar_retracement DOUBLE NULL,
                trade_bc_bar_retracement DOUBLE NULL,
                trade_cd_bar_retracement DOUBLE NULL,
                trade_cd_bc_bar_retracement DOUBLE NULL,
                trade_cd_xa_bar_retracement DOUBLE NULL,
                trade_cd_bc_price_retracement DOUBLE NOT NULL,
                trade_snr DOUBLE NOT NULL,
                trade_year BIGINT NOT NULL,
                trade_month BIGINT NOT NULL,
                trade_day BIGINT NOT NULL,
                reversal_type VARCHAR(32) NULL,
                bullish_key_reversal BOOLEAN NULL,
                bearish_key_reversal BOOLEAN NULL,
                bullish_engulfing BOOLEAN NULL,
                bearish_engulfing BOOLEAN NULL,
                bullish_outside_reversal BOOLEAN NULL,
                bearish_outside_reversal BOOLEAN NULL,
                hammer BOOLEAN NULL,
                shooting_star BOOLEAN NULL,
                morning_star BOOLEAN NULL,
                evening_star BOOLEAN NULL,
                three_white_soldiers BOOLEAN NULL,
                three_black_crows BOOLEAN NULL,
                market VARCHAR(16) NOT NULL,
                three_month BOOLEAN NULL,
                six_month BOOLEAN NULL,
                twelve_month BOOLEAN NULL,
                pattern_group_id VARCHAR(64) NOT NULL,
                prop_strategy_id CHAR(16) NULL,
                harmonic_type VARCHAR(24) NOT NULL,
                xa_price_length DOUBLE NOT NULL,
                ab_price_length DOUBLE NOT NULL,
                bc_price_length DOUBLE NOT NULL,
                cd_price_length DOUBLE NOT NULL,
                bat_accuracy DOUBLE NOT NULL,
                alternate_bat_accuracy DOUBLE NOT NULL,
                butterfly_accuracy DOUBLE NOT NULL,
                gartley_accuracy DOUBLE NOT NULL,
                crab_accuracy DOUBLE NOT NULL,
                deep_crab_accuracy DOUBLE NOT NULL,
                shark_accuracy DOUBLE NOT NULL,
                time_accuracy DOUBLE NULL,
                bat_time_accuracy DOUBLE NULL,
                alternate_bat_time_accuracy DOUBLE NULL,
                butterfly_time_accuracy DOUBLE NULL,
                gartley_time_accuracy DOUBLE NULL,
                crab_time_accuracy DOUBLE NULL,
                deep_crab_time_accuracy DOUBLE NULL,
                shark_time_accuracy DOUBLE NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                INDEX idx_xabcd_pattern_id (pattern_id),
                INDEX idx_xabcd_x_bars_left (x_bars_left),
                INDEX idx_xabcd_symbol_d_date (symbol, d_date),
                INDEX idx_xabcd_pattern_group_d_date (pattern_group_id, d_date),
                INDEX idx_xabcd_prop_strategy_id (prop_strategy_id),
                INDEX idx_xabcd_lookup (market, harmonic_type, d_date)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn ensure_candle_trend_columns(&self) -> Result<(), sqlx::Error> {
        let trend_columns = [
            (
                "three_month",
                "ALTER TABLE candles ADD COLUMN three_month BOOLEAN NULL AFTER volume",
            ),
            (
                "six_month",
                "ALTER TABLE candles ADD COLUMN six_month BOOLEAN NULL AFTER three_month",
            ),
            (
                "twelve_month",
                "ALTER TABLE candles ADD COLUMN twelve_month BOOLEAN NULL AFTER six_month",
            ),
        ];

        for (column_name, alter_sql) in trend_columns {
            let exists: i64 = sqlx::query_scalar(
                r#"
                SELECT COUNT(*)
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_SCHEMA = DATABASE()
                  AND TABLE_NAME = 'candles'
                  AND COLUMN_NAME = ?
                "#,
            )
            .bind(column_name)
            .fetch_one(&self.pool)
            .await?;

            if exists == 0 {
                sqlx::query(alter_sql).execute(&self.pool).await?;
            }
        }

        Ok(())
    }

    pub async fn ensure_xabcd_trend_columns(&self) -> Result<(), sqlx::Error> {
        self.ensure_xabcd_patterns_table().await?;

        let trend_columns = ["three_month", "six_month", "twelve_month"];

        for column_name in trend_columns {
            let existing_column: Option<String> = sqlx::query_scalar(
                r#"
                SELECT COLUMN_TYPE
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_SCHEMA = DATABASE()
                    AND TABLE_NAME = 'xabcd_patterns'
                    AND COLUMN_NAME = ?
                "#,
            )
            .bind(column_name)
            .fetch_optional(&self.pool)
            .await?;

            let alter_statement = match existing_column.as_deref() {
                None => format!(
                    "ALTER TABLE xabcd_patterns ADD COLUMN {} BOOLEAN NULL",
                    column_name
                ),
                Some(column_type) if column_type.eq_ignore_ascii_case("tinyint(1)") => continue,
                Some(_) => format!(
                    "ALTER TABLE xabcd_patterns MODIFY COLUMN {} BOOLEAN NULL",
                    column_name
                ),
            };

            sqlx::query(&alter_statement).execute(&self.pool).await?;
        }

        Ok(())
    }

    pub async fn ensure_xabcd_reversal_columns(&self) -> Result<(), sqlx::Error> {
        self.ensure_xabcd_patterns_table().await?;

        for sql in [
            "ALTER TABLE xabcd_patterns ADD COLUMN bullish_key_reversal BOOLEAN NULL AFTER reversal_type",
            "ALTER TABLE xabcd_patterns ADD COLUMN bearish_key_reversal BOOLEAN NULL AFTER bullish_key_reversal",
            "ALTER TABLE xabcd_patterns ADD COLUMN bullish_engulfing BOOLEAN NULL AFTER bearish_key_reversal",
            "ALTER TABLE xabcd_patterns ADD COLUMN bearish_engulfing BOOLEAN NULL AFTER bullish_engulfing",
            "ALTER TABLE xabcd_patterns ADD COLUMN bullish_outside_reversal BOOLEAN NULL AFTER bearish_engulfing",
            "ALTER TABLE xabcd_patterns ADD COLUMN bearish_outside_reversal BOOLEAN NULL AFTER bullish_outside_reversal",
            "ALTER TABLE xabcd_patterns ADD COLUMN hammer BOOLEAN NULL AFTER bearish_outside_reversal",
            "ALTER TABLE xabcd_patterns ADD COLUMN shooting_star BOOLEAN NULL AFTER hammer",
            "ALTER TABLE xabcd_patterns ADD COLUMN morning_star BOOLEAN NULL AFTER shooting_star",
            "ALTER TABLE xabcd_patterns ADD COLUMN evening_star BOOLEAN NULL AFTER morning_star",
            "ALTER TABLE xabcd_patterns ADD COLUMN three_white_soldiers BOOLEAN NULL AFTER evening_star",
            "ALTER TABLE xabcd_patterns ADD COLUMN three_black_crows BOOLEAN NULL AFTER three_white_soldiers",
        ] {
            if let Err(error) = sqlx::query(sql).execute(&self.pool).await {
                let is_duplicate_column = match &error {
                    sqlx::Error::Database(db_error) => db_error.message().contains("Duplicate column name"),
                    _ => false,
                };

                if !is_duplicate_column {
                    return Err(error);
                }
            }
        }

        Ok(())
    }

    async fn persist_candle_trends(&self, candles: &[Candle]) -> Result<(), sqlx::Error> {
        if candles.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;

        for chunk in candles.chunks(CANDLE_TREND_UPSERT_CHUNK_SIZE) {
            let mut builder = QueryBuilder::<MySql>::new(
                r#"
                INSERT INTO candles (
                    symbol,
                    date,
                    open,
                    high,
                    low,
                    close,
                    volume,
                    three_month,
                    six_month,
                    twelve_month
                )
                "#,
            );

            builder.push_values(chunk, |mut row, candle| {
                row.push_bind(&candle.symbol)
                    .push_bind(candle.date.date())
                    .push_bind(candle.open)
                    .push_bind(candle.high)
                    .push_bind(candle.low)
                    .push_bind(candle.close)
                    .push_bind(candle.volume)
                    .push_bind(candle.three_month)
                    .push_bind(candle.six_month)
                    .push_bind(candle.twelve_month);
            });

            builder.push(
                r#"
                ON DUPLICATE KEY UPDATE
                    three_month = VALUES(three_month),
                    six_month = VALUES(six_month),
                    twelve_month = VALUES(twelve_month)
                "#,
            );

            builder.build().execute(&mut *tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn ensure_xabcd_time_columns(&self) -> Result<(), sqlx::Error> {
        self.ensure_xabcd_patterns_table().await?;

        for sql in [
            "ALTER TABLE xabcd_patterns ADD COLUMN trade_ab_bar_retracement DOUBLE NULL AFTER trade_cd_price_retracement",
            "ALTER TABLE xabcd_patterns ADD COLUMN trade_cd_bc_bar_retracement DOUBLE NULL AFTER trade_cd_bar_retracement",
            "ALTER TABLE xabcd_patterns ADD COLUMN trade_cd_xa_bar_retracement DOUBLE NULL AFTER trade_cd_bc_bar_retracement",
            "ALTER TABLE xabcd_patterns ADD COLUMN alternate_bat_accuracy DOUBLE NOT NULL DEFAULT 0 AFTER bat_accuracy",
            "ALTER TABLE xabcd_patterns ADD COLUMN deep_crab_accuracy DOUBLE NOT NULL DEFAULT 0 AFTER crab_accuracy",
            "ALTER TABLE xabcd_patterns ADD COLUMN time_accuracy DOUBLE NULL AFTER shark_accuracy",
            "ALTER TABLE xabcd_patterns ADD COLUMN bat_time_accuracy DOUBLE NULL AFTER time_accuracy",
            "ALTER TABLE xabcd_patterns ADD COLUMN alternate_bat_time_accuracy DOUBLE NULL AFTER bat_time_accuracy",
            "ALTER TABLE xabcd_patterns ADD COLUMN butterfly_time_accuracy DOUBLE NULL AFTER alternate_bat_time_accuracy",
            "ALTER TABLE xabcd_patterns ADD COLUMN gartley_time_accuracy DOUBLE NULL AFTER butterfly_time_accuracy",
            "ALTER TABLE xabcd_patterns ADD COLUMN crab_time_accuracy DOUBLE NULL AFTER gartley_time_accuracy",
            "ALTER TABLE xabcd_patterns ADD COLUMN deep_crab_time_accuracy DOUBLE NULL AFTER crab_time_accuracy",
            "ALTER TABLE xabcd_patterns ADD COLUMN shark_time_accuracy DOUBLE NULL AFTER deep_crab_time_accuracy",
        ] {
            if let Err(error) = sqlx::query(sql).execute(&self.pool).await {
                let is_duplicate_column = match &error {
                    sqlx::Error::Database(db_error) => db_error.message().contains("Duplicate column name"),
                    _ => false,
                };

                if !is_duplicate_column {
                    return Err(error);
                }
            }
        }

        Ok(())
    }

    pub async fn ensure_xabcd_target_columns(&self) -> Result<(), sqlx::Error> {
        self.ensure_xabcd_patterns_table().await?;

        for sql in [
            "ALTER TABLE xabcd_patterns ADD COLUMN d_confirm_date DATE NULL AFTER d_close",
            "ALTER TABLE xabcd_patterns ADD COLUMN target_ready BOOLEAN NULL AFTER d_confirm_date",
            "ALTER TABLE xabcd_patterns ADD COLUMN target_date DATE NULL AFTER target_ready",
            "ALTER TABLE xabcd_patterns ADD COLUMN target_open DOUBLE NULL AFTER target_date",
            "ALTER TABLE xabcd_patterns ADD COLUMN target_high DOUBLE NULL AFTER target_open",
            "ALTER TABLE xabcd_patterns ADD COLUMN target_low DOUBLE NULL AFTER target_high",
            "ALTER TABLE xabcd_patterns ADD COLUMN target_close DOUBLE NULL AFTER target_low",
            "ALTER TABLE xabcd_patterns ADD COLUMN target_volume BIGINT NULL AFTER target_close",
            "ALTER TABLE xabcd_patterns ADD COLUMN target_is_green BOOLEAN NULL AFTER target_volume",
            "ALTER TABLE xabcd_patterns ADD COLUMN target_close_vs_open_pct DOUBLE NULL AFTER target_is_green",
            "ALTER TABLE xabcd_patterns ADD COLUMN target_high_vs_open_pct DOUBLE NULL AFTER target_close_vs_open_pct",
            "ALTER TABLE xabcd_patterns ADD COLUMN target_low_vs_open_pct DOUBLE NULL AFTER target_high_vs_open_pct",
            "ALTER TABLE xabcd_patterns ADD COLUMN target_range_pct DOUBLE NULL AFTER target_low_vs_open_pct",
            "ALTER TABLE xabcd_patterns ADD COLUMN target_breaks_d_high BOOLEAN NULL AFTER target_range_pct",
            "ALTER TABLE xabcd_patterns ADD COLUMN target_breaks_d_low BOOLEAN NULL AFTER target_breaks_d_high",
        ] {
            if let Err(error) = sqlx::query(sql).execute(&self.pool).await {
                let is_duplicate_column = match &error {
                    sqlx::Error::Database(db_error) => db_error.message().contains("Duplicate column name"),
                    _ => false,
                };

                if !is_duplicate_column {
                    return Err(error);
                }
            }
        }

        Ok(())
    }

    pub async fn ensure_xabcd_prop_strategy_id_column(&self) -> Result<(), sqlx::Error> {
        self.ensure_xabcd_patterns_table().await?;

        if let Err(error) = sqlx::query(
            "ALTER TABLE xabcd_patterns ADD COLUMN prop_strategy_id CHAR(16) NULL AFTER pattern_group_id",
        )
        .execute(&self.pool)
        .await
        {
            let is_duplicate_column = match &error {
                sqlx::Error::Database(db_error) => db_error.message().contains("Duplicate column name"),
                _ => false,
            };

            if !is_duplicate_column {
                return Err(error);
            }
        }

        if let Err(error) = sqlx::query(
            "CREATE INDEX idx_xabcd_prop_strategy_id ON xabcd_patterns (prop_strategy_id)",
        )
        .execute(&self.pool)
        .await
        {
            let is_duplicate_index = match &error {
                sqlx::Error::Database(db_error) => {
                    db_error.message().contains("Duplicate key name")
                        || db_error.message().contains("already exists")
                }
                _ => false,
            };

            if !is_duplicate_index {
                return Err(error);
            }
        }

        Ok(())
    }

    pub async fn ensure_xabcd_x_bars_left_column(&self) -> Result<(), sqlx::Error> {
        self.ensure_xabcd_patterns_table().await?;

        self.add_column_if_missing(
            "xabcd_patterns",
            "x_bars_left",
            "x_bars_left BIGINT NOT NULL DEFAULT 0 AFTER pattern_id",
        )
        .await?;
        self.create_index_if_missing(
            "CREATE INDEX idx_xabcd_x_bars_left ON xabcd_patterns (x_bars_left)",
        )
        .await?;
        sqlx::query(
            r#"
            UPDATE xabcd_patterns
            SET x_bars_left = COALESCE(x_length, 0)
                + COALESCE(a_length, 0)
                + COALESCE(b_length, 0)
                + COALESCE(c_length, 0)
            WHERE x_bars_left = 0
            "#,
        )
        .execute(&self.pool)
        .await?;
        self.drop_column_if_exists("xabcd_patterns", "x_mode")
            .await?;

        Ok(())
    }

    pub async fn ensure_xabcd_pattern_id_column(&self) -> Result<(), sqlx::Error> {
        self.ensure_xabcd_patterns_table().await?;

        if let Err(error) = sqlx::query(
            "ALTER TABLE xabcd_patterns ADD COLUMN pattern_id CHAR(24) NULL AFTER symbol",
        )
        .execute(&self.pool)
        .await
        {
            let is_duplicate_column = match &error {
                sqlx::Error::Database(db_error) => {
                    db_error.message().contains("Duplicate column name")
                }
                _ => false,
            };

            if !is_duplicate_column {
                return Err(error);
            }
        }

        if let Err(error) =
            sqlx::query("CREATE INDEX idx_xabcd_pattern_id ON xabcd_patterns (pattern_id)")
                .execute(&self.pool)
                .await
        {
            let is_duplicate_index = match &error {
                sqlx::Error::Database(db_error) => {
                    db_error.message().contains("Duplicate key name")
                        || db_error.message().contains("already exists")
                }
                _ => false,
            };

            if !is_duplicate_index {
                return Err(error);
            }
        }

        Ok(())
    }

    pub async fn ensure_pattern_mode_tables(&self) -> Result<(), sqlx::Error> {
        self.ensure_harmonic_pattern_definitions_table().await?;
        self.ensure_pattern_harmonic_scores_table().await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS pattern_setups (
                id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
                setup_id CHAR(24) NOT NULL,
                pattern_id CHAR(24) NOT NULL,
                symbol VARCHAR(32) NOT NULL,
                pattern_group_id VARCHAR(64) NOT NULL,
                x_bars_left BIGINT NOT NULL DEFAULT 0,
                market VARCHAR(16) NOT NULL,
                harmonic_type VARCHAR(24) NOT NULL,
                prop_strategy_id CHAR(16) NULL,
                x_date DATE NOT NULL,
                x_open DOUBLE NOT NULL,
                x_high DOUBLE NOT NULL,
                x_low DOUBLE NOT NULL,
                x_close DOUBLE NOT NULL,
                x_length BIGINT NOT NULL,
                x_min_max DOUBLE NOT NULL,
                a_date DATE NOT NULL,
                a_open DOUBLE NOT NULL,
                a_high DOUBLE NOT NULL,
                a_low DOUBLE NOT NULL,
                a_close DOUBLE NOT NULL,
                a_length BIGINT NOT NULL,
                a_min_max DOUBLE NOT NULL,
                xa_price_length DOUBLE NOT NULL,
                b_date DATE NOT NULL,
                b_open DOUBLE NOT NULL,
                b_high DOUBLE NOT NULL,
                b_low DOUBLE NOT NULL,
                b_close DOUBLE NOT NULL,
                b_length BIGINT NOT NULL,
                b_min_max DOUBLE NOT NULL,
                ab_price_length DOUBLE NOT NULL,
                c_date DATE NOT NULL,
                c_open DOUBLE NOT NULL,
                c_high DOUBLE NOT NULL,
                c_low DOUBLE NOT NULL,
                c_close DOUBLE NOT NULL,
                c_length BIGINT NOT NULL,
                c_min_max DOUBLE NOT NULL,
                bc_price_length DOUBLE NOT NULL,
                d_date DATE NOT NULL,
                d_open DOUBLE NOT NULL,
                d_high DOUBLE NOT NULL,
                d_low DOUBLE NOT NULL,
                d_close DOUBLE NOT NULL,
                d_length BIGINT NOT NULL,
                d_min_max DOUBLE NOT NULL,
                cd_price_length DOUBLE NOT NULL,
                full_pattern_length BIGINT NOT NULL,
                bullish_key_reversal BOOLEAN NOT NULL,
                bearish_key_reversal BOOLEAN NOT NULL,
                bullish_engulfing BOOLEAN NOT NULL,
                bearish_engulfing BOOLEAN NOT NULL,
                bullish_outside_reversal BOOLEAN NOT NULL,
                bearish_outside_reversal BOOLEAN NOT NULL,
                hammer BOOLEAN NOT NULL,
                shooting_star BOOLEAN NOT NULL,
                morning_star BOOLEAN NOT NULL,
                evening_star BOOLEAN NOT NULL,
                three_white_soldiers BOOLEAN NOT NULL,
                three_black_crows BOOLEAN NOT NULL,
                three_month BOOLEAN NULL,
                six_month BOOLEAN NULL,
                twelve_month BOOLEAN NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                UNIQUE KEY uniq_pattern_setups_setup_id (setup_id),
                INDEX idx_pattern_setups_pattern_id (pattern_id),
                INDEX idx_pattern_setups_x_bars_left (x_bars_left),
                INDEX idx_pattern_setups_symbol_d_date (symbol, d_date),
                INDEX idx_pattern_setups_prop_strategy_id (prop_strategy_id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        for sql in [
            "ALTER TABLE pattern_setups ADD COLUMN pattern_id CHAR(24) NULL AFTER setup_id",
            "ALTER TABLE pattern_setups ADD INDEX idx_pattern_setups_pattern_id (pattern_id)",
        ] {
            if let Err(error) = sqlx::query(sql).execute(&self.pool).await {
                let is_duplicate = match &error {
                    sqlx::Error::Database(db_error) => {
                        db_error.message().contains("Duplicate column name")
                            || db_error.message().contains("Duplicate key name")
                            || db_error.message().contains("already exists")
                    }
                    _ => false,
                };

                if !is_duplicate {
                    return Err(error);
                }
            }
        }

        self.add_column_if_missing(
            "pattern_setups",
            "x_bars_left",
            "x_bars_left BIGINT NOT NULL DEFAULT 0 AFTER pattern_group_id",
        )
        .await?;
        self.create_index_if_missing(
            "CREATE INDEX idx_pattern_setups_x_bars_left ON pattern_setups (x_bars_left)",
        )
        .await?;
        sqlx::query(
            r#"
            UPDATE pattern_setups
            SET x_bars_left = COALESCE(x_length, 0)
                + COALESCE(a_length, 0)
                + COALESCE(b_length, 0)
                + COALESCE(c_length, 0)
            WHERE x_bars_left = 0
            "#,
        )
        .execute(&self.pool)
        .await?;
        self.drop_column_if_exists("pattern_setups", "x_mode")
            .await?;

        self.migrate_pattern_setup_scores().await?;
        self.drop_pattern_setup_score_columns().await?;

        if let Err(error) = sqlx::query("ALTER TABLE pattern_setups DROP COLUMN reversal_type")
            .execute(&self.pool)
            .await
        {
            let is_missing = match &error {
                sqlx::Error::Database(db_error) => {
                    db_error.message().contains("Can't DROP")
                        || db_error.message().contains("Unknown column")
                        || db_error.message().contains("doesn't exist")
                        || db_error.message().contains("check that column/key exists")
                }
                _ => false,
            };

            if !is_missing {
                return Err(error);
            }
        }

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS pattern_outcomes_swing (
                id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
                setup_id CHAR(24) NOT NULL,
                swing_strategy_id CHAR(16) NULL,
                harmonic_type VARCHAR(24) NOT NULL DEFAULT 'Multi',
                bin VARCHAR(16) NOT NULL DEFAULT 'Multi',
                time_bin VARCHAR(16) NOT NULL DEFAULT 'Multi',
                trade_open BOOLEAN NOT NULL,
                trade_risk_exit_price DOUBLE NOT NULL,
                trade_reward_exit_price DOUBLE NOT NULL,
                trade_enter_price DOUBLE NOT NULL,
                trade_current_price DOUBLE NOT NULL,
                trade_length BIGINT NOT NULL,
                trade_pnl DOUBLE NOT NULL,
                trade_result INT NOT NULL,
                trade_date DATETIME NULL,
                trade_symbol VARCHAR(32) NOT NULL,
                trade_ab_price_retracement DOUBLE NOT NULL,
                trade_bc_price_retracement DOUBLE NOT NULL,
                trade_cd_xa_price_retracement DOUBLE NOT NULL,
                trade_cd_price_retracement DOUBLE NOT NULL,
                trade_ab_bar_retracement DOUBLE NOT NULL,
                trade_bc_bar_retracement DOUBLE NOT NULL,
                trade_cd_bar_retracement DOUBLE NOT NULL,
                trade_cd_bc_bar_retracement DOUBLE NOT NULL,
                trade_cd_xa_bar_retracement DOUBLE NOT NULL,
                trade_cd_bc_price_retracement DOUBLE NOT NULL,
                trade_snr DOUBLE NOT NULL,
                trade_year BIGINT NOT NULL,
                trade_month BIGINT NOT NULL,
                trade_day BIGINT NOT NULL,
                reversal_type VARCHAR(32) NOT NULL DEFAULT 'None',
                bullish_key_reversal BOOLEAN NOT NULL DEFAULT FALSE,
                bearish_key_reversal BOOLEAN NOT NULL DEFAULT FALSE,
                bullish_engulfing BOOLEAN NOT NULL DEFAULT FALSE,
                bearish_engulfing BOOLEAN NOT NULL DEFAULT FALSE,
                bullish_outside_reversal BOOLEAN NOT NULL DEFAULT FALSE,
                bearish_outside_reversal BOOLEAN NOT NULL DEFAULT FALSE,
                hammer BOOLEAN NOT NULL DEFAULT FALSE,
                shooting_star BOOLEAN NOT NULL DEFAULT FALSE,
                morning_star BOOLEAN NOT NULL DEFAULT FALSE,
                evening_star BOOLEAN NOT NULL DEFAULT FALSE,
                three_white_soldiers BOOLEAN NOT NULL DEFAULT FALSE,
                three_black_crows BOOLEAN NOT NULL DEFAULT FALSE,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                UNIQUE KEY uniq_pattern_outcomes_swing_setup_id (setup_id),
                INDEX idx_pattern_outcomes_swing_strategy_id (swing_strategy_id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        for sql in [
            "ALTER TABLE pattern_outcomes_swing ADD COLUMN swing_strategy_id CHAR(16) NULL AFTER setup_id",
            "ALTER TABLE pattern_outcomes_swing ADD INDEX idx_pattern_outcomes_swing_strategy_id (swing_strategy_id)",
            "ALTER TABLE pattern_outcomes_swing ADD COLUMN harmonic_type VARCHAR(24) NOT NULL DEFAULT 'Multi' AFTER setup_id",
            "ALTER TABLE pattern_outcomes_swing ADD COLUMN bin VARCHAR(16) NOT NULL DEFAULT 'Multi' AFTER harmonic_type",
            "ALTER TABLE pattern_outcomes_swing ADD COLUMN time_bin VARCHAR(16) NOT NULL DEFAULT 'Multi' AFTER bin",
            "ALTER TABLE pattern_outcomes_swing ADD COLUMN reversal_type VARCHAR(32) NOT NULL DEFAULT 'None' AFTER trade_day",
            "ALTER TABLE pattern_outcomes_swing ADD COLUMN bullish_key_reversal BOOLEAN NOT NULL DEFAULT FALSE AFTER reversal_type",
            "ALTER TABLE pattern_outcomes_swing ADD COLUMN bearish_key_reversal BOOLEAN NOT NULL DEFAULT FALSE AFTER bullish_key_reversal",
            "ALTER TABLE pattern_outcomes_swing ADD COLUMN bullish_engulfing BOOLEAN NOT NULL DEFAULT FALSE AFTER bearish_key_reversal",
            "ALTER TABLE pattern_outcomes_swing ADD COLUMN bearish_engulfing BOOLEAN NOT NULL DEFAULT FALSE AFTER bullish_engulfing",
            "ALTER TABLE pattern_outcomes_swing ADD COLUMN bullish_outside_reversal BOOLEAN NOT NULL DEFAULT FALSE AFTER bearish_engulfing",
            "ALTER TABLE pattern_outcomes_swing ADD COLUMN bearish_outside_reversal BOOLEAN NOT NULL DEFAULT FALSE AFTER bullish_outside_reversal",
            "ALTER TABLE pattern_outcomes_swing ADD COLUMN hammer BOOLEAN NOT NULL DEFAULT FALSE AFTER bearish_outside_reversal",
            "ALTER TABLE pattern_outcomes_swing ADD COLUMN shooting_star BOOLEAN NOT NULL DEFAULT FALSE AFTER hammer",
            "ALTER TABLE pattern_outcomes_swing ADD COLUMN morning_star BOOLEAN NOT NULL DEFAULT FALSE AFTER shooting_star",
            "ALTER TABLE pattern_outcomes_swing ADD COLUMN evening_star BOOLEAN NOT NULL DEFAULT FALSE AFTER morning_star",
            "ALTER TABLE pattern_outcomes_swing ADD COLUMN three_white_soldiers BOOLEAN NOT NULL DEFAULT FALSE AFTER evening_star",
            "ALTER TABLE pattern_outcomes_swing ADD COLUMN three_black_crows BOOLEAN NOT NULL DEFAULT FALSE AFTER three_white_soldiers",
        ] {
            if let Err(error) = sqlx::query(sql).execute(&self.pool).await {
                let is_duplicate = match &error {
                    sqlx::Error::Database(db_error) => {
                        db_error.message().contains("Duplicate column name")
                            || db_error.message().contains("Duplicate key name")
                            || db_error.message().contains("already exists")
                    }
                    _ => false,
                };

                if !is_duplicate {
                    return Err(error);
                }
            }
        }

        if self.table_exists("pattern_outcomes_prop").await?
            && !self
                .table_column_exists("pattern_outcomes_prop", "outcome_model")
                .await?
        {
            sqlx::query("DROP TABLE pattern_outcomes_prop")
                .execute(&self.pool)
                .await?;
        }

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS pattern_outcomes_prop (
                id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
                outcome_row_id CHAR(32) NOT NULL,
                setup_id CHAR(24) NOT NULL,
                prop_strategy_id CHAR(16) NULL,
                outcome_model VARCHAR(24) NOT NULL,
                has_reversal BOOLEAN NOT NULL,
                reversal_type VARCHAR(32) NOT NULL DEFAULT 'None',
                reversal_detect_date DATETIME NULL,
                reversal_bars_after_d BIGINT NULL,
                pattern_id CHAR(24) NULL,
                pattern_group_id VARCHAR(64) NOT NULL,
                x_bars_left BIGINT NOT NULL DEFAULT 0,
                symbol VARCHAR(32) NOT NULL,
                d_date DATETIME NOT NULL,
                contract_week_index BIGINT NULL,
                contract_days_from_start BIGINT NULL,
                entry_date DATETIME NOT NULL,
                harmonic_type VARCHAR(24) NOT NULL DEFAULT 'Multi',
                bin VARCHAR(16) NOT NULL DEFAULT 'Multi',
                size_bucket VARCHAR(16) NOT NULL,
                time_bin VARCHAR(16) NOT NULL DEFAULT 'Multi',
                market VARCHAR(16) NOT NULL,
                three_month_trend VARCHAR(16) NOT NULL,
                six_month_trend VARCHAR(16) NOT NULL,
                twelve_month_trend VARCHAR(16) NOT NULL,
                x_length BIGINT NOT NULL,
                a_length BIGINT NOT NULL,
                b_length BIGINT NOT NULL,
                c_length BIGINT NOT NULL,
                d_length BIGINT NOT NULL,
                full_pattern_length BIGINT NOT NULL,
                trade_enter_price DOUBLE NOT NULL,
                trade_risk_exit_price DOUBLE NOT NULL,
                trade_reward_exit_price DOUBLE NOT NULL,
                d_confirm_date DATETIME NOT NULL,
                target_ready BOOLEAN NOT NULL,
                target_date DATETIME NULL,
                target_open DOUBLE NULL,
                target_high DOUBLE NULL,
                target_low DOUBLE NULL,
                target_close DOUBLE NULL,
                target_volume BIGINT NULL,
                target_is_green BOOLEAN NULL,
                target_close_vs_open_pct DOUBLE NULL,
                target_high_vs_open_pct DOUBLE NULL,
                target_low_vs_open_pct DOUBLE NULL,
                target_range_pct DOUBLE NULL,
                target_breaks_entry_high BOOLEAN NULL,
                target_breaks_entry_low BOOLEAN NULL,
                prop_result INT NOT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                UNIQUE KEY uniq_pattern_outcomes_prop_row_id (outcome_row_id),
                INDEX idx_pattern_outcomes_prop_setup_id (setup_id),
                INDEX idx_pattern_outcomes_prop_strategy_id (prop_strategy_id),
                INDEX idx_pattern_outcomes_prop_x_bars_left (x_bars_left),
                INDEX idx_pattern_outcomes_prop_pattern_id (pattern_id, d_date),
                INDEX idx_pattern_outcomes_prop_group_detail (
                    pattern_group_id,
                    d_date,
                    market,
                    harmonic_type,
                    size_bucket
                ),
                INDEX idx_pattern_outcomes_prop_symbol_d_date (symbol, d_date),
                INDEX idx_pattern_outcomes_prop_lookup (
                    outcome_model,
                    market,
                    harmonic_type,
                    bin,
                    has_reversal,
                    reversal_type,
                    size_bucket,
                    time_bin
                ),
                INDEX idx_pattern_outcomes_prop_target_ready (target_ready, entry_date)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        self.add_column_if_missing(
            "pattern_outcomes_prop",
            "x_bars_left",
            "x_bars_left BIGINT NOT NULL DEFAULT 0 AFTER pattern_group_id",
        )
        .await?;
        self.add_column_if_missing(
            "pattern_outcomes_prop",
            "contract_week_index",
            "contract_week_index BIGINT NULL AFTER d_date",
        )
        .await?;
        self.add_column_if_missing(
            "pattern_outcomes_prop",
            "contract_days_from_start",
            "contract_days_from_start BIGINT NULL AFTER contract_week_index",
        )
        .await?;
        self.create_index_if_missing(
            "CREATE INDEX idx_pattern_outcomes_prop_contract_week ON pattern_outcomes_prop (symbol, contract_week_index)",
        )
        .await?;
        self.create_index_if_missing(
            "CREATE INDEX idx_pattern_outcomes_prop_x_bars_left ON pattern_outcomes_prop (x_bars_left)",
        )
        .await?;
        self.create_index_if_missing(
            "CREATE INDEX idx_pattern_outcomes_prop_pattern_id ON pattern_outcomes_prop (pattern_id, d_date)",
        )
        .await?;
        self.create_index_if_missing(
            "CREATE INDEX idx_pattern_outcomes_prop_group_detail ON pattern_outcomes_prop (pattern_group_id, d_date, market, harmonic_type, size_bucket)",
        )
        .await?;
        self.create_index_if_missing(
            "CREATE INDEX idx_pattern_outcomes_prop_symbol_d_date ON pattern_outcomes_prop (symbol, d_date)",
        )
        .await?;
        sqlx::query(
            r#"
            UPDATE pattern_outcomes_prop
            SET x_bars_left = COALESCE(x_length, 0)
                + COALESCE(a_length, 0)
                + COALESCE(b_length, 0)
                + COALESCE(c_length, 0)
            WHERE x_bars_left = 0
            "#,
        )
        .execute(&self.pool)
        .await?;
        self.drop_column_if_exists("pattern_outcomes_prop", "x_mode")
            .await?;

        Ok(())
    }

    pub async fn rebuild_pattern_mode_tables_from_xabcd(&self) -> Result<(), sqlx::Error> {
        self.ensure_xabcd_patterns_table().await?;
        self.ensure_xabcd_pattern_id_column().await?;
        self.ensure_xabcd_x_bars_left_column().await?;
        self.ensure_xabcd_time_columns().await?;
        self.backfill_xabcd_pattern_ids().await?;
        self.ensure_pattern_mode_tables().await?;

        for sql in [
            "TRUNCATE TABLE pattern_outcomes_prop",
            "TRUNCATE TABLE pattern_outcomes_swing",
            "TRUNCATE TABLE pattern_harmonic_scores",
            "TRUNCATE TABLE pattern_setups",
        ] {
            sqlx::query(sql).execute(&self.pool).await?;
        }

        let setup_id_expr = "COALESCE(NULLIF(pattern_id, ''), LEFT(MD5(CONCAT_WS('|', LOWER(market), symbol, x_date, CAST(CAST(ROUND(x_min_max, 2) AS DECIMAL(18,2)) AS CHAR), a_date, CAST(CAST(ROUND(a_min_max, 2) AS DECIMAL(18,2)) AS CHAR), b_date, CAST(CAST(ROUND(b_min_max, 2) AS DECIMAL(18,2)) AS CHAR), c_date, CAST(CAST(ROUND(c_min_max, 2) AS DECIMAL(18,2)) AS CHAR), d_date, CAST(CAST(ROUND(d_min_max, 2) AS DECIMAL(18,2)) AS CHAR))), 24))";
        let route_harmonic_type_expr = dominant_harmonic_type_expr();
        let route_bin_expr = dominant_harmonic_bin_expr();
        let route_time_accuracy_expr = dominant_harmonic_time_accuracy_expr();
        let route_time_bin_expr = time_bin_expr(&route_time_accuracy_expr);
        let route_reversal_type_expr = "COALESCE(NULLIF(reversal_type, ''), 'None')";
        let size_bucket_expr = structure_size_bucket_expr(structure_total_bars_expr());
        let three_month_trend_expr = trend_bucket_expr("three_month");
        let six_month_trend_expr = trend_bucket_expr("six_month");
        let twelve_month_trend_expr = trend_bucket_expr("twelve_month");
        let swing_strategy_id_expr = swing_strategy_id_expr(
            &route_harmonic_type_expr,
            &route_bin_expr,
            &route_time_bin_expr,
            route_reversal_type_expr,
        );

        let insert_setups_sql = format!(
            r#"
            INSERT INTO pattern_setups (
                setup_id, pattern_id, symbol, pattern_group_id, x_bars_left, market, harmonic_type, prop_strategy_id,
                x_date, x_open, x_high, x_low, x_close, x_length, x_min_max,
                a_date, a_open, a_high, a_low, a_close, a_length, a_min_max, xa_price_length,
                b_date, b_open, b_high, b_low, b_close, b_length, b_min_max, ab_price_length,
                c_date, c_open, c_high, c_low, c_close, c_length, c_min_max, bc_price_length,
                d_date, d_open, d_high, d_low, d_close, d_length, d_min_max, cd_price_length,
                full_pattern_length,
                bullish_key_reversal, bearish_key_reversal,
                bullish_engulfing, bearish_engulfing,
                bullish_outside_reversal, bearish_outside_reversal,
                hammer, shooting_star, morning_star, evening_star,
                three_white_soldiers, three_black_crows,
                three_month, six_month, twelve_month
            )
            SELECT
                {setup_id_expr},
                {setup_id_expr},
                symbol,
                pattern_group_id,
                COALESCE(NULLIF(x_bars_left, 0), x_length + a_length + b_length + c_length),
                market,
                'Multi',
                NULLIF(prop_strategy_id, ''),
                x_date, x_open, x_high, x_low, x_close, x_length, x_min_max,
                a_date, a_open, a_high, a_low, a_close, a_length, a_min_max, xa_price_length,
                b_date, b_open, b_high, b_low, b_close, b_length, b_min_max, ab_price_length,
                c_date, c_open, c_high, c_low, c_close, c_length, c_min_max, bc_price_length,
                d_date, d_open, d_high, d_low, d_close, d_length, d_min_max, cd_price_length,
                full_pattern_length,
                bullish_key_reversal, bearish_key_reversal,
                bullish_engulfing, bearish_engulfing,
                bullish_outside_reversal, bearish_outside_reversal,
                hammer, shooting_star, morning_star, evening_star,
                three_white_soldiers, three_black_crows,
                three_month, six_month, twelve_month
            FROM xabcd_patterns
            "#,
            setup_id_expr = setup_id_expr,
        );
        sqlx::query(&insert_setups_sql).execute(&self.pool).await?;
        self.upsert_harmonic_scores_from_xabcd(setup_id_expr)
            .await?;

        let insert_swing_sql = format!(
            r#"
            INSERT INTO pattern_outcomes_swing (
                setup_id, swing_strategy_id, harmonic_type, bin, time_bin,
                trade_open, trade_risk_exit_price, trade_reward_exit_price,
                trade_enter_price, trade_current_price, trade_length, trade_pnl, trade_result,
                trade_date, trade_symbol,
                trade_ab_price_retracement, trade_bc_price_retracement,
                trade_cd_xa_price_retracement, trade_cd_price_retracement,
                trade_ab_bar_retracement, trade_bc_bar_retracement, trade_cd_bar_retracement,
                trade_cd_bc_bar_retracement, trade_cd_xa_bar_retracement,
                trade_cd_bc_price_retracement, trade_snr,
                trade_year, trade_month, trade_day,
                reversal_type, bullish_key_reversal, bearish_key_reversal,
                bullish_engulfing, bearish_engulfing,
                bullish_outside_reversal, bearish_outside_reversal,
                hammer, shooting_star, morning_star, evening_star,
                three_white_soldiers, three_black_crows
            )
            SELECT
                {setup_id_expr},
                {swing_strategy_id_expr},
                {route_harmonic_type_expr},
                {route_bin_expr},
                {route_time_bin_expr},
                trade_open, trade_risk_exit_price, trade_reward_exit_price,
                trade_enter_price, trade_current_price, trade_length, trade_pnl, trade_result,
                trade_date, trade_symbol,
                trade_ab_price_retracement, trade_bc_price_retracement,
                trade_cd_xa_price_retracement, trade_cd_price_retracement,
                trade_ab_bar_retracement, trade_bc_bar_retracement, trade_cd_bar_retracement,
                trade_cd_bc_bar_retracement, trade_cd_xa_bar_retracement,
                trade_cd_bc_price_retracement, trade_snr,
                trade_year, trade_month, trade_day,
                {route_reversal_type_expr},
                COALESCE(bullish_key_reversal, FALSE),
                COALESCE(bearish_key_reversal, FALSE),
                COALESCE(bullish_engulfing, FALSE),
                COALESCE(bearish_engulfing, FALSE),
                COALESCE(bullish_outside_reversal, FALSE),
                COALESCE(bearish_outside_reversal, FALSE),
                COALESCE(hammer, FALSE),
                COALESCE(shooting_star, FALSE),
                COALESCE(morning_star, FALSE),
                COALESCE(evening_star, FALSE),
                COALESCE(three_white_soldiers, FALSE),
                COALESCE(three_black_crows, FALSE)
            FROM xabcd_patterns
            "#,
            setup_id_expr = setup_id_expr,
            route_harmonic_type_expr = route_harmonic_type_expr,
            route_bin_expr = route_bin_expr,
            route_time_bin_expr = route_time_bin_expr,
            swing_strategy_id_expr = swing_strategy_id_expr,
            route_reversal_type_expr = route_reversal_type_expr,
        );
        sqlx::query(&insert_swing_sql).execute(&self.pool).await?;

        let insert_prop_sql = format!(
            r#"
            INSERT INTO pattern_outcomes_prop (
                outcome_row_id, setup_id, prop_strategy_id,
                outcome_model, has_reversal, reversal_type, reversal_detect_date,
                reversal_bars_after_d, pattern_id, pattern_group_id, x_bars_left, symbol,
                d_date, entry_date, market, harmonic_type, bin, size_bucket, time_bin,
                three_month_trend, six_month_trend, twelve_month_trend,
                x_length, a_length, b_length, c_length, d_length, full_pattern_length,
                trade_enter_price, trade_risk_exit_price, trade_reward_exit_price,
                d_confirm_date, target_ready, target_date,
                target_open, target_high, target_low, target_close, target_volume,
                target_is_green, target_close_vs_open_pct, target_high_vs_open_pct,
                target_low_vs_open_pct, target_range_pct, target_breaks_entry_high,
                target_breaks_entry_low, prop_result
            )
            SELECT
                MD5(CONCAT({setup_id_expr}, '|D')),
                {setup_id_expr},
                NULLIF(prop_strategy_id, ''),
                'D',
                FALSE,
                'None',
                NULL,
                NULL,
                {setup_id_expr},
                pattern_group_id,
                COALESCE(NULLIF(x_bars_left, 0), x_length + a_length + b_length + c_length),
                symbol,
                d_date,
                d_confirm_date,
                market,
                {route_harmonic_type_expr},
                {route_bin_expr},
                {size_bucket_expr},
                {route_time_bin_expr},
                {three_month_trend_expr},
                {six_month_trend_expr},
                {twelve_month_trend_expr},
                x_length,
                a_length,
                b_length,
                c_length,
                d_length,
                full_pattern_length,
                trade_enter_price,
                trade_risk_exit_price,
                trade_reward_exit_price,
                d_confirm_date,
                target_ready,
                target_date,
                target_open,
                target_high,
                target_low,
                target_close,
                target_volume,
                target_is_green,
                target_close_vs_open_pct,
                target_high_vs_open_pct,
                target_low_vs_open_pct,
                target_range_pct,
                target_breaks_d_high,
                target_breaks_d_low,
                CASE
                    WHEN target_is_green IS NULL THEN 0
                    WHEN market = 'Bullish' AND target_is_green THEN 1
                    WHEN market = 'Bullish' THEN 2
                    WHEN market = 'Bearish' AND target_is_green THEN 2
                    ELSE 1
                END
            FROM xabcd_patterns
            "#,
            setup_id_expr = setup_id_expr,
            route_harmonic_type_expr = route_harmonic_type_expr,
            route_bin_expr = route_bin_expr,
            route_time_bin_expr = route_time_bin_expr,
            size_bucket_expr = size_bucket_expr,
            three_month_trend_expr = three_month_trend_expr,
            six_month_trend_expr = six_month_trend_expr,
            twelve_month_trend_expr = twelve_month_trend_expr,
        );
        sqlx::query(&insert_prop_sql).execute(&self.pool).await?;

        Ok(())
    }

    pub async fn backfill_xabcd_pattern_ids(&self) -> Result<(), sqlx::Error> {
        self.ensure_xabcd_patterns_table().await?;
        self.ensure_xabcd_pattern_id_column().await?;
        self.ensure_xabcd_x_bars_left_column().await?;

        let sql = format!(
            r#"
            UPDATE xabcd_patterns
            SET pattern_id = {pattern_id_expr}
            WHERE pattern_id IS NULL
               OR pattern_id = ''
               OR pattern_id <> {pattern_id_expr}
            "#,
            pattern_id_expr = pattern_id_expr(),
        );

        sqlx::query(&sql).execute(&self.pool).await?;
        Ok(())
    }

    async fn upsert_pattern_mode_tables(
        &self,
        patterns: &[XABCD_CSV],
        run_id: Option<&str>,
        fast_rebuild: bool,
        use_build_tables: bool,
        write_pattern_setups: bool,
        write_harmonic_scores: bool,
        write_swing_outcomes: bool,
        write_prop_outcomes: bool,
    ) -> Result<(), sqlx::Error> {
        if patterns.is_empty() {
            return Ok(());
        }

        if !fast_rebuild && !use_build_tables {
            self.ensure_pattern_mode_tables().await?;
        }

        let tables = if use_build_tables {
            PatternOutputTables::build_tables()
        } else {
            PatternOutputTables::final_tables()
        };
        let mut setup_duration = Duration::ZERO;
        let mut score_duration = Duration::ZERO;
        let mut swing_duration = Duration::ZERO;
        let mut prop_duration = Duration::ZERO;
        let mut setup_rows = 0i64;
        let mut score_rows_written = 0i64;
        let mut swing_rows = 0i64;
        let mut prop_rows = 0i64;
        let mut tx = self.pool.begin().await?;

        for chunk in patterns.chunks(PATTERN_INSERT_CHUNK_SIZE) {
            if write_pattern_setups {
                let phase_started = Instant::now();
                let mut setup_builder = QueryBuilder::<MySql>::new(format!(
                    r#"
                    INSERT INTO {} (
                        setup_id, pattern_id, symbol, pattern_group_id, x_bars_left, market, harmonic_type, prop_strategy_id,
                        x_date, x_open, x_high, x_low, x_close, x_length, x_min_max,
                        a_date, a_open, a_high, a_low, a_close, a_length, a_min_max, xa_price_length,
                        b_date, b_open, b_high, b_low, b_close, b_length, b_min_max, ab_price_length,
                        c_date, c_open, c_high, c_low, c_close, c_length, c_min_max, bc_price_length,
                        d_date, d_open, d_high, d_low, d_close, d_length, d_min_max, cd_price_length,
                        full_pattern_length,
                        bullish_key_reversal, bearish_key_reversal,
                        bullish_engulfing, bearish_engulfing,
                        bullish_outside_reversal, bearish_outside_reversal,
                        hammer, shooting_star, morning_star, evening_star,
                        three_white_soldiers, three_black_crows,
                        three_month, six_month, twelve_month
                    )
                    "#,
                    tables.pattern_setups
                ));

                setup_builder.push_values(chunk, |mut row, p| {
                    let pattern_id = build_pattern_setup_id(p);
                    row.push_bind(pattern_id.clone())
                        .push_bind(pattern_id)
                        .push_bind(&p.symbol)
                        .push_bind(&p.pattern_group_id)
                        .push_bind(p.x_bars_left)
                        .push_bind(format!("{:?}", p.market))
                        .push_bind(&p.harmonic_type)
                        .push_bind(&p.prop_strategy_id)
                        .push_bind(&p.x_date)
                        .push_bind(p.x_open)
                        .push_bind(p.x_high)
                        .push_bind(p.x_low)
                        .push_bind(p.x_close)
                        .push_bind(p.x_length)
                        .push_bind(p.x_min_max)
                        .push_bind(&p.a_date)
                        .push_bind(p.a_open)
                        .push_bind(p.a_high)
                        .push_bind(p.a_low)
                        .push_bind(p.a_close)
                        .push_bind(p.a_length)
                        .push_bind(p.a_min_max)
                        .push_bind(p.xa_price_length)
                        .push_bind(&p.b_date)
                        .push_bind(p.b_open)
                        .push_bind(p.b_high)
                        .push_bind(p.b_low)
                        .push_bind(p.b_close)
                        .push_bind(p.b_length)
                        .push_bind(p.b_min_max)
                        .push_bind(p.ab_price_length)
                        .push_bind(&p.c_date)
                        .push_bind(p.c_open)
                        .push_bind(p.c_high)
                        .push_bind(p.c_low)
                        .push_bind(p.c_close)
                        .push_bind(p.c_length)
                        .push_bind(p.c_min_max)
                        .push_bind(p.bc_price_length)
                        .push_bind(&p.d_date)
                        .push_bind(p.d_open)
                        .push_bind(p.d_high)
                        .push_bind(p.d_low)
                        .push_bind(p.d_close)
                        .push_bind(p.d_length)
                        .push_bind(p.d_min_max)
                        .push_bind(p.cd_price_length)
                        .push_bind(p.full_pattern_length)
                        .push_bind(p.bullish_key_reversal)
                        .push_bind(p.bearish_key_reversal)
                        .push_bind(p.bullish_engulfing)
                        .push_bind(p.bearish_engulfing)
                        .push_bind(p.bullish_outside_reversal)
                        .push_bind(p.bearish_outside_reversal)
                        .push_bind(p.hammer)
                        .push_bind(p.shooting_star)
                        .push_bind(p.morning_star)
                        .push_bind(p.evening_star)
                        .push_bind(p.three_white_soldiers)
                        .push_bind(p.three_black_crows)
                        .push_bind(p.three_month)
                        .push_bind(p.six_month)
                        .push_bind(p.twelve_month);
                });

                if !fast_rebuild {
                    setup_builder.push(
                        r#"
                        ON DUPLICATE KEY UPDATE
                            pattern_id = VALUES(pattern_id),
                            x_bars_left = VALUES(x_bars_left),
                            prop_strategy_id = VALUES(prop_strategy_id),
                            bullish_key_reversal = VALUES(bullish_key_reversal),
                            bearish_key_reversal = VALUES(bearish_key_reversal),
                            bullish_engulfing = VALUES(bullish_engulfing),
                            bearish_engulfing = VALUES(bearish_engulfing),
                            bullish_outside_reversal = VALUES(bullish_outside_reversal),
                            bearish_outside_reversal = VALUES(bearish_outside_reversal),
                            hammer = VALUES(hammer),
                            shooting_star = VALUES(shooting_star),
                            morning_star = VALUES(morning_star),
                            evening_star = VALUES(evening_star),
                            three_white_soldiers = VALUES(three_white_soldiers),
                            three_black_crows = VALUES(three_black_crows),
                            three_month = VALUES(three_month),
                            six_month = VALUES(six_month),
                            twelve_month = VALUES(twelve_month),
                            updated_at = CURRENT_TIMESTAMP
                        "#,
                    );
                }

                setup_builder.build().execute(&mut *tx).await?;
                setup_duration += phase_started.elapsed();
                setup_rows += chunk.len() as i64;
            }

            if write_harmonic_scores {
                let phase_started = Instant::now();
                let mut score_builder = QueryBuilder::<MySql>::new(format!(
                    r#"
                    INSERT INTO {} (
                        setup_id,
                        harmonic_type,
                        price_accuracy,
                        time_accuracy
                    )
                    "#,
                    tables.harmonic_scores
                ));

                score_rows_written += (chunk.len() * HARMONIC_SCORE_COLUMNS.len()) as i64;
                let score_rows = chunk.iter().flat_map(|p| {
                    let setup_id = build_pattern_setup_id(p);
                    harmonic_score_values(p).into_iter().map(
                        move |(harmonic_type, price_accuracy, time_accuracy)| {
                            (
                                setup_id.clone(),
                                harmonic_type,
                                price_accuracy,
                                time_accuracy,
                            )
                        },
                    )
                });
                score_builder.push_values(score_rows, |mut row, item| {
                    row.push_bind(item.0)
                        .push_bind(item.1)
                        .push_bind(item.2)
                        .push_bind(item.3);
                });

                if !fast_rebuild {
                    score_builder.push(
                        r#"
                        ON DUPLICATE KEY UPDATE
                            price_accuracy = VALUES(price_accuracy),
                            time_accuracy = VALUES(time_accuracy),
                            updated_at = CURRENT_TIMESTAMP
                        "#,
                    );
                }

                score_builder.build().execute(&mut *tx).await?;
                score_duration += phase_started.elapsed();
            }

            if write_swing_outcomes {
                let phase_started = Instant::now();
                let mut swing_builder = QueryBuilder::<MySql>::new(format!(
                    r#"
                    INSERT INTO {} (
                        setup_id, swing_strategy_id, harmonic_type, bin, time_bin,
                        trade_open, trade_risk_exit_price, trade_reward_exit_price,
                        trade_enter_price, trade_current_price, trade_length, trade_pnl, trade_result,
                        trade_date, trade_symbol,
                        trade_ab_price_retracement, trade_bc_price_retracement,
                        trade_cd_xa_price_retracement, trade_cd_price_retracement,
                        trade_ab_bar_retracement, trade_bc_bar_retracement, trade_cd_bar_retracement,
                        trade_cd_bc_bar_retracement, trade_cd_xa_bar_retracement,
                        trade_cd_bc_price_retracement, trade_snr,
                        trade_year, trade_month, trade_day,
                        reversal_type, bullish_key_reversal, bearish_key_reversal,
                        bullish_engulfing, bearish_engulfing,
                        bullish_outside_reversal, bearish_outside_reversal,
                        hammer, shooting_star, morning_star, evening_star,
                        three_white_soldiers, three_black_crows
                    )
                    "#,
                    tables.swing_outcomes
                ));

                swing_builder.push_values(chunk, |mut row, p| {
                    row.push_bind(build_pattern_setup_id(p))
                        .push_bind(&p.swing_strategy_id)
                        .push_bind(&p.route_harmonic_type)
                        .push_bind(&p.route_bin)
                        .push_bind(&p.route_time_bin)
                        .push_bind(p.trade_open)
                        .push_bind(p.trade_risk_exit_price)
                        .push_bind(p.trade_reward_exit_price)
                        .push_bind(p.trade_enter_price)
                        .push_bind(p.trade_current_price)
                        .push_bind(p.trade_length)
                        .push_bind(p.trade_pnl)
                        .push_bind(p.trade_result)
                        .push_bind(&p.trade_date)
                        .push_bind(&p.trade_symbol)
                        .push_bind(p.trade_ab_price_retracement)
                        .push_bind(p.trade_bc_price_retracement)
                        .push_bind(p.trade_cd_xa_price_retracement)
                        .push_bind(p.trade_cd_price_retracement)
                        .push_bind(p.trade_ab_bar_retracement)
                        .push_bind(p.trade_bc_bar_retracement)
                        .push_bind(p.trade_cd_bar_retracement)
                        .push_bind(p.trade_cd_bc_bar_retracement)
                        .push_bind(p.trade_cd_xa_bar_retracement)
                        .push_bind(p.trade_cd_bc_price_retracement)
                        .push_bind(p.trade_snr)
                        .push_bind(p.trade_year)
                        .push_bind(p.trade_month)
                        .push_bind(p.trade_day)
                        .push_bind(format!("{:?}", p.reversal_type))
                        .push_bind(p.bullish_key_reversal)
                        .push_bind(p.bearish_key_reversal)
                        .push_bind(p.bullish_engulfing)
                        .push_bind(p.bearish_engulfing)
                        .push_bind(p.bullish_outside_reversal)
                        .push_bind(p.bearish_outside_reversal)
                        .push_bind(p.hammer)
                        .push_bind(p.shooting_star)
                        .push_bind(p.morning_star)
                        .push_bind(p.evening_star)
                        .push_bind(p.three_white_soldiers)
                        .push_bind(p.three_black_crows);
                });

                if !fast_rebuild {
                    swing_builder.push(
                        r#"
                        ON DUPLICATE KEY UPDATE
                            swing_strategy_id = VALUES(swing_strategy_id),
                            harmonic_type = VALUES(harmonic_type),
                            bin = VALUES(bin),
                            time_bin = VALUES(time_bin),
                            trade_open = VALUES(trade_open),
                            trade_risk_exit_price = VALUES(trade_risk_exit_price),
                            trade_reward_exit_price = VALUES(trade_reward_exit_price),
                            trade_enter_price = VALUES(trade_enter_price),
                            trade_current_price = VALUES(trade_current_price),
                            trade_length = VALUES(trade_length),
                            trade_pnl = VALUES(trade_pnl),
                            trade_result = VALUES(trade_result),
                            trade_date = VALUES(trade_date),
                            trade_symbol = VALUES(trade_symbol),
                            trade_ab_price_retracement = VALUES(trade_ab_price_retracement),
                            trade_bc_price_retracement = VALUES(trade_bc_price_retracement),
                            trade_cd_xa_price_retracement = VALUES(trade_cd_xa_price_retracement),
                            trade_cd_price_retracement = VALUES(trade_cd_price_retracement),
                            trade_ab_bar_retracement = VALUES(trade_ab_bar_retracement),
                            trade_bc_bar_retracement = VALUES(trade_bc_bar_retracement),
                            trade_cd_bar_retracement = VALUES(trade_cd_bar_retracement),
                            trade_cd_bc_bar_retracement = VALUES(trade_cd_bc_bar_retracement),
                            trade_cd_xa_bar_retracement = VALUES(trade_cd_xa_bar_retracement),
                            trade_cd_bc_price_retracement = VALUES(trade_cd_bc_price_retracement),
                            trade_snr = VALUES(trade_snr),
                            trade_year = VALUES(trade_year),
                            trade_month = VALUES(trade_month),
                            trade_day = VALUES(trade_day),
                            reversal_type = VALUES(reversal_type),
                            bullish_key_reversal = VALUES(bullish_key_reversal),
                            bearish_key_reversal = VALUES(bearish_key_reversal),
                            bullish_engulfing = VALUES(bullish_engulfing),
                            bearish_engulfing = VALUES(bearish_engulfing),
                            bullish_outside_reversal = VALUES(bullish_outside_reversal),
                            bearish_outside_reversal = VALUES(bearish_outside_reversal),
                            hammer = VALUES(hammer),
                            shooting_star = VALUES(shooting_star),
                            morning_star = VALUES(morning_star),
                            evening_star = VALUES(evening_star),
                            three_white_soldiers = VALUES(three_white_soldiers),
                            three_black_crows = VALUES(three_black_crows),
                            updated_at = CURRENT_TIMESTAMP
                        "#,
                    );
                }

                swing_builder.build().execute(&mut *tx).await?;
                swing_duration += phase_started.elapsed();
                swing_rows += chunk.len() as i64;
            }

            if write_prop_outcomes {
                let phase_started = Instant::now();
                let mut prop_builder = QueryBuilder::<MySql>::new(format!(
                    r#"
                    INSERT INTO {} (
                        setup_id, prop_strategy_id, harmonic_type, bin, time_bin,
                        d_confirm_date, target_ready, target_date,
                        target_open, target_high, target_low, target_close, target_volume,
                        target_is_green, target_close_vs_open_pct, target_high_vs_open_pct,
                        target_low_vs_open_pct, target_range_pct, target_breaks_d_high,
                        target_breaks_d_low, prop_result
                    )
                    "#,
                    tables.prop_outcomes
                ));

                prop_builder.push_values(chunk, |mut row, p| {
                    row.push_bind(build_pattern_setup_id(p))
                        .push_bind(&p.prop_strategy_id)
                        .push_bind(&p.route_harmonic_type)
                        .push_bind(&p.route_bin)
                        .push_bind(&p.route_time_bin)
                        .push_bind(&p.d_confirm_date)
                        .push_bind(p.target_ready)
                        .push_bind(&p.target_date)
                        .push_bind(p.target_open)
                        .push_bind(p.target_high)
                        .push_bind(p.target_low)
                        .push_bind(p.target_close)
                        .push_bind(p.target_volume)
                        .push_bind(p.target_is_green)
                        .push_bind(p.target_close_vs_open_pct)
                        .push_bind(p.target_high_vs_open_pct)
                        .push_bind(p.target_low_vs_open_pct)
                        .push_bind(p.target_range_pct)
                        .push_bind(p.target_breaks_d_high)
                        .push_bind(p.target_breaks_d_low)
                        .push_bind(prop_result_value(p));
                });

                if !fast_rebuild {
                    prop_builder.push(
                        r#"
                        ON DUPLICATE KEY UPDATE
                            prop_strategy_id = VALUES(prop_strategy_id),
                            harmonic_type = VALUES(harmonic_type),
                            bin = VALUES(bin),
                            time_bin = VALUES(time_bin),
                            d_confirm_date = VALUES(d_confirm_date),
                            target_ready = VALUES(target_ready),
                            target_date = VALUES(target_date),
                            target_open = VALUES(target_open),
                            target_high = VALUES(target_high),
                            target_low = VALUES(target_low),
                            target_close = VALUES(target_close),
                            target_volume = VALUES(target_volume),
                            target_is_green = VALUES(target_is_green),
                            target_close_vs_open_pct = VALUES(target_close_vs_open_pct),
                            target_high_vs_open_pct = VALUES(target_high_vs_open_pct),
                            target_low_vs_open_pct = VALUES(target_low_vs_open_pct),
                            target_range_pct = VALUES(target_range_pct),
                            target_breaks_d_high = VALUES(target_breaks_d_high),
                            target_breaks_d_low = VALUES(target_breaks_d_low),
                            prop_result = VALUES(prop_result),
                            updated_at = CURRENT_TIMESTAMP
                        "#,
                    );
                }

                prop_builder.build().execute(&mut *tx).await?;
                prop_duration += phase_started.elapsed();
                prop_rows += chunk.len() as i64;
            }
        }

        let commit_started = Instant::now();
        tx.commit().await?;
        let commit_duration = commit_started.elapsed();

        if let Some(run_id) = run_id {
            self.record_engine_phase_timing(
                run_id,
                None,
                "write_pattern_setups",
                Some(setup_rows),
                setup_duration,
                None,
            )
            .await?;
            self.record_engine_phase_timing(
                run_id,
                None,
                "write_harmonic_scores",
                Some(score_rows_written),
                score_duration,
                Some(if write_harmonic_scores {
                    "7 harmonic score rows per setup"
                } else {
                    "disabled; dominant harmonic fields are stored in route outcome tables"
                }),
            )
            .await?;
            self.record_engine_phase_timing(
                run_id,
                None,
                "write_swing_outcomes",
                Some(swing_rows),
                swing_duration,
                if write_swing_outcomes {
                    None
                } else {
                    Some("disabled; prop routes only")
                },
            )
            .await?;
            self.record_engine_phase_timing(
                run_id,
                None,
                "write_prop_outcomes",
                Some(prop_rows),
                prop_duration,
                if write_prop_outcomes {
                    None
                } else {
                    Some("disabled; prop reversal route only")
                },
            )
            .await?;
            self.record_engine_phase_timing(
                run_id,
                None,
                "commit_pattern_mode_tables",
                None,
                commit_duration,
                None,
            )
            .await?;
        }

        Ok(())
    }

    pub async fn ensure_xabcd_length_columns(&self) -> Result<(), sqlx::Error> {
        self.ensure_xabcd_patterns_table().await?;

        for sql in [
            "ALTER TABLE xabcd_patterns ADD COLUMN full_pattern_length BIGINT NULL AFTER d_length",
        ] {
            if let Err(error) = sqlx::query(sql).execute(&self.pool).await {
                let is_duplicate_column = match &error {
                    sqlx::Error::Database(db_error) => {
                        db_error.message().contains("Duplicate column name")
                    }
                    _ => false,
                };

                if !is_duplicate_column {
                    return Err(error);
                }
            }
        }

        Ok(())
    }

    pub async fn backfill_xabcd_time_columns(&self) -> Result<(), sqlx::Error> {
        self.ensure_xabcd_patterns_table().await?;
        self.ensure_xabcd_time_columns().await?;

        let bat_time_accuracy_expr = harmonic_time_accuracy_sql_expr(0.382, 0.382, 2.618, 1.618);
        let butterfly_time_accuracy_expr =
            harmonic_time_accuracy_sql_expr(0.786, 0.382, 1.27, 1.618);
        let gartley_time_accuracy_expr = harmonic_time_accuracy_sql_expr(0.618, 0.382, 1.27, 0.786);
        let crab_time_accuracy_expr = harmonic_time_accuracy_sql_expr(0.382, 0.382, 3.618, 2.618);
        let shark_time_accuracy_expr = harmonic_time_accuracy_sql_expr(0.886, 0.382, 1.13, 1.618);
        let update_sql = format!(
            r#"
            UPDATE xabcd_patterns
            SET
                trade_ab_bar_retracement = CASE
                    WHEN COALESCE(x_length, 0) <= 0 THEN 0.0
                    ELSE (CAST(a_length AS DOUBLE) / CAST(x_length AS DOUBLE)) * 100.0
                END,
                trade_cd_bc_bar_retracement = CASE
                    WHEN COALESCE(b_length, 0) <= 0 THEN 0.0
                    ELSE (CAST(c_length AS DOUBLE) / CAST(b_length AS DOUBLE)) * 100.0
                END,
                trade_cd_xa_bar_retracement = CASE
                    WHEN COALESCE(x_length, 0) <= 0 THEN 0.0
                    ELSE (CAST(c_length AS DOUBLE) / CAST(x_length AS DOUBLE)) * 100.0
                END,
                bat_time_accuracy = {bat_time_accuracy_expr},
                butterfly_time_accuracy = {butterfly_time_accuracy_expr},
                gartley_time_accuracy = {gartley_time_accuracy_expr},
                crab_time_accuracy = {crab_time_accuracy_expr},
                shark_time_accuracy = {shark_time_accuracy_expr},
                time_accuracy = GREATEST(
                    {bat_time_accuracy_expr},
                    {butterfly_time_accuracy_expr},
                    {gartley_time_accuracy_expr},
                    {crab_time_accuracy_expr},
                    {shark_time_accuracy_expr}
                )
            "#,
        );

        sqlx::query(&update_sql).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn ensure_accuracy_bin_cache_table(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS accuracy_bin_cache (
                source_scope VARCHAR(32) NOT NULL,
                market_scope VARCHAR(16) NOT NULL,
                trade_result_scope INT NOT NULL,
                harmonic_type VARCHAR(16) NOT NULL,
                bin VARCHAR(16) NOT NULL,
                count BIGINT NOT NULL,
                total_count BIGINT NOT NULL,
                open_count BIGINT NOT NULL,
                win_count BIGINT NOT NULL,
                loss_count BIGINT NOT NULL,
                avg_return DOUBLE NOT NULL,
                expectancy DOUBLE NOT NULL,
                win_rate DOUBLE NOT NULL,
                closed_rate DOUBLE NOT NULL,
                avg_win DOUBLE NOT NULL,
                avg_loss DOUBLE NOT NULL,
                PRIMARY KEY (source_scope, market_scope, trade_result_scope, harmonic_type, bin),
                INDEX idx_accuracy_bin_lookup (source_scope, harmonic_type, market_scope, trade_result_scope)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn ensure_accuracy_bin_rollup_table(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS accuracy_bin_rollup (
                rollup_year INT NOT NULL,
                market VARCHAR(16) NOT NULL,
                trade_result_value INT NOT NULL,
                source_scope VARCHAR(32) NOT NULL,
                harmonic_type VARCHAR(16) NOT NULL,
                bin VARCHAR(16) NOT NULL,
                total_count BIGINT NOT NULL DEFAULT 0,
                closed_count BIGINT NOT NULL DEFAULT 0,
                open_count BIGINT NOT NULL DEFAULT 0,
                win_count BIGINT NOT NULL DEFAULT 0,
                loss_count BIGINT NOT NULL DEFAULT 0,
                return_sum DOUBLE NOT NULL DEFAULT 0,
                return_count BIGINT NOT NULL DEFAULT 0,
                expectancy_sum DOUBLE NOT NULL DEFAULT 0,
                expectancy_count BIGINT NOT NULL DEFAULT 0,
                win_return_sum DOUBLE NOT NULL DEFAULT 0,
                win_return_count BIGINT NOT NULL DEFAULT 0,
                loss_return_sum DOUBLE NOT NULL DEFAULT 0,
                loss_return_count BIGINT NOT NULL DEFAULT 0,
                PRIMARY KEY (
                    rollup_year,
                    market,
                    trade_result_value,
                    source_scope,
                    harmonic_type,
                    bin
                ),
                INDEX idx_accuracy_rollup_lookup (
                    source_scope,
                    harmonic_type,
                    market,
                    trade_result_value,
                    bin
                )
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn ensure_pattern_structure_rollup_table(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS pattern_structure_rollup (
                market VARCHAR(16) NOT NULL,
                trade_result_value INT NOT NULL,
                harmonic_type VARCHAR(16) NOT NULL,
                bin VARCHAR(16) NOT NULL,
                size_bucket VARCHAR(16) NOT NULL,
                size_order INT NOT NULL,
                balance_bucket VARCHAR(16) NOT NULL,
                balance_order INT NOT NULL,
                total_count BIGINT NOT NULL DEFAULT 0,
                closed_count BIGINT NOT NULL DEFAULT 0,
                open_count BIGINT NOT NULL DEFAULT 0,
                win_count BIGINT NOT NULL DEFAULT 0,
                expectancy_sum DOUBLE NOT NULL DEFAULT 0,
                expectancy_count BIGINT NOT NULL DEFAULT 0,
                total_bars_sum DOUBLE NOT NULL DEFAULT 0,
                total_bars_count BIGINT NOT NULL DEFAULT 0,
                balance_ratio_sum DOUBLE NOT NULL DEFAULT 0,
                balance_ratio_count BIGINT NOT NULL DEFAULT 0,
                distorted_count BIGINT NOT NULL DEFAULT 0,
                PRIMARY KEY (
                    market,
                    trade_result_value,
                    harmonic_type,
                    bin,
                    size_bucket,
                    balance_bucket
                ),
                INDEX idx_pattern_structure_lookup (
                    harmonic_type,
                    bin,
                    market,
                    trade_result_value,
                    size_bucket,
                    balance_bucket
                )
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn ensure_accuracy_structure_rollup_table(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS accuracy_structure_rollup (
                source_scope VARCHAR(32) NOT NULL,
                market VARCHAR(16) NOT NULL,
                trade_result_value INT NOT NULL,
                harmonic_type VARCHAR(16) NOT NULL,
                bin VARCHAR(16) NOT NULL,
                size_bucket VARCHAR(16) NOT NULL,
                balance_bucket VARCHAR(16) NOT NULL,
                total_count BIGINT NOT NULL DEFAULT 0,
                closed_count BIGINT NOT NULL DEFAULT 0,
                open_count BIGINT NOT NULL DEFAULT 0,
                win_count BIGINT NOT NULL DEFAULT 0,
                loss_count BIGINT NOT NULL DEFAULT 0,
                return_sum DOUBLE NOT NULL DEFAULT 0,
                return_count BIGINT NOT NULL DEFAULT 0,
                expectancy_sum DOUBLE NOT NULL DEFAULT 0,
                expectancy_count BIGINT NOT NULL DEFAULT 0,
                win_return_sum DOUBLE NOT NULL DEFAULT 0,
                win_return_count BIGINT NOT NULL DEFAULT 0,
                loss_return_sum DOUBLE NOT NULL DEFAULT 0,
                loss_return_count BIGINT NOT NULL DEFAULT 0,
                PRIMARY KEY (
                    source_scope,
                    market,
                    trade_result_value,
                    harmonic_type,
                    bin,
                    size_bucket,
                    balance_bucket
                ),
                INDEX idx_accuracy_structure_lookup (
                    source_scope,
                    harmonic_type,
                    market,
                    trade_result_value,
                    bin,
                    size_bucket,
                    balance_bucket
                )
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn ensure_swing_strategy_yearly_table(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS swing_strategy_yearly (
                trade_year INT NOT NULL,
                swing_strategy_id CHAR(16) NOT NULL,
                market VARCHAR(16) NOT NULL,
                harmonic_type VARCHAR(16) NOT NULL,
                bin VARCHAR(16) NOT NULL,
                reversal_type VARCHAR(32) NOT NULL,
                size_bucket VARCHAR(16) NOT NULL,
                time_bin VARCHAR(16) NOT NULL,
                three_month_trend VARCHAR(16) NOT NULL,
                six_month_trend VARCHAR(16) NOT NULL,
                twelve_month_trend VARCHAR(16) NOT NULL,
                total_count BIGINT NOT NULL DEFAULT 0,
                closed_count BIGINT NOT NULL DEFAULT 0,
                open_count BIGINT NOT NULL DEFAULT 0,
                win_count BIGINT NOT NULL DEFAULT 0,
                loss_count BIGINT NOT NULL DEFAULT 0,
                return_sum DOUBLE NOT NULL DEFAULT 0,
                return_count BIGINT NOT NULL DEFAULT 0,
                expectancy_sum DOUBLE NOT NULL DEFAULT 0,
                expectancy_count BIGINT NOT NULL DEFAULT 0,
                win_return_sum DOUBLE NOT NULL DEFAULT 0,
                win_return_count BIGINT NOT NULL DEFAULT 0,
                loss_return_sum DOUBLE NOT NULL DEFAULT 0,
                loss_return_count BIGINT NOT NULL DEFAULT 0,
                trade_length_sum DOUBLE NOT NULL DEFAULT 0,
                trade_length_count BIGINT NOT NULL DEFAULT 0,
                ab_xa_sum DOUBLE NOT NULL DEFAULT 0,
                ab_xa_count BIGINT NOT NULL DEFAULT 0,
                bc_ab_sum DOUBLE NOT NULL DEFAULT 0,
                bc_ab_count BIGINT NOT NULL DEFAULT 0,
                cd_bc_sum DOUBLE NOT NULL DEFAULT 0,
                cd_bc_count BIGINT NOT NULL DEFAULT 0,
                cd_xa_sum DOUBLE NOT NULL DEFAULT 0,
                cd_xa_count BIGINT NOT NULL DEFAULT 0,
                PRIMARY KEY (
                    trade_year,
                    swing_strategy_id,
                    market,
                    harmonic_type,
                    bin,
                    reversal_type,
                    size_bucket,
                    time_bin,
                    x_strictness,
                    three_month_trend,
                    six_month_trend,
                    twelve_month_trend
                ),
                INDEX idx_strategy_yearly_lookup (
                    swing_strategy_id,
                    market,
                    harmonic_type,
                    bin,
                    reversal_type,
                    size_bucket,
                    time_bin,
                    three_month_trend,
                    six_month_trend,
                    twelve_month_trend,
                    trade_year
                )
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        for sql in [
            "ALTER TABLE swing_strategy_yearly ADD COLUMN swing_strategy_id CHAR(16) NULL AFTER trade_year",
            "ALTER TABLE swing_strategy_yearly ADD INDEX idx_strategy_yearly_id (swing_strategy_id, trade_year)",
        ] {
            if let Err(error) = sqlx::query(sql).execute(&self.pool).await {
                let is_duplicate = match &error {
                    sqlx::Error::Database(db_error) => {
                        db_error.message().contains("Duplicate column name")
                            || db_error.message().contains("Duplicate key name")
                            || db_error.message().contains("already exists")
                    }
                    _ => false,
                };

                if !is_duplicate {
                    return Err(error);
                }
            }
        }

        Ok(())
    }

    pub async fn ensure_swing_strategy_summary_table(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS swing_strategy_summary (
                swing_strategy_id CHAR(16) NOT NULL,
                market VARCHAR(16) NOT NULL,
                harmonic_type VARCHAR(16) NOT NULL,
                bin VARCHAR(16) NOT NULL,
                reversal_type VARCHAR(32) NOT NULL,
                size_bucket VARCHAR(16) NOT NULL,
                time_bin VARCHAR(16) NOT NULL,
                three_month_trend VARCHAR(16) NOT NULL,
                six_month_trend VARCHAR(16) NOT NULL,
                twelve_month_trend VARCHAR(16) NOT NULL,
                worst_year_expectancy DOUBLE NOT NULL DEFAULT 0,
                down_years BIGINT NOT NULL DEFAULT 0,
                total_count BIGINT NOT NULL DEFAULT 0,
                closed_count BIGINT NOT NULL DEFAULT 0,
                open_count BIGINT NOT NULL DEFAULT 0,
                win_count BIGINT NOT NULL DEFAULT 0,
                loss_count BIGINT NOT NULL DEFAULT 0,
                expectancy DOUBLE NOT NULL DEFAULT 0,
                avg_return DOUBLE NOT NULL DEFAULT 0,
                win_rate DOUBLE NOT NULL DEFAULT 0,
                closed_rate DOUBLE NOT NULL DEFAULT 0,
                avg_win DOUBLE NOT NULL DEFAULT 0,
                avg_loss DOUBLE NOT NULL DEFAULT 0,
                avg_trade_length DOUBLE NOT NULL DEFAULT 0,
                avg_ab_xa DOUBLE NOT NULL DEFAULT 0,
                avg_bc_ab DOUBLE NOT NULL DEFAULT 0,
                avg_cd_bc DOUBLE NOT NULL DEFAULT 0,
                avg_cd_xa DOUBLE NOT NULL DEFAULT 0,
                PRIMARY KEY (
                    swing_strategy_id,
                    market,
                    harmonic_type,
                    bin,
                    reversal_type,
                    size_bucket,
                    time_bin,
                    x_strictness,
                    three_month_trend,
                    six_month_trend,
                    twelve_month_trend
                ),
                INDEX idx_strategy_cohort_summary_rank (
                    expectancy,
                    closed_count,
                    win_rate
                ),
                INDEX idx_strategy_cohort_summary_id (
                    swing_strategy_id
                )
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        for sql in [
            "ALTER TABLE swing_strategy_summary ADD COLUMN swing_strategy_id CHAR(16) NULL FIRST",
            "ALTER TABLE swing_strategy_summary ADD INDEX idx_strategy_cohort_summary_id (swing_strategy_id)",
        ] {
            if let Err(error) = sqlx::query(sql).execute(&self.pool).await {
                let is_duplicate = match &error {
                    sqlx::Error::Database(db_error) => {
                        db_error.message().contains("Duplicate column name")
                            || db_error.message().contains("Duplicate key name")
                            || db_error.message().contains("already exists")
                    }
                    _ => false,
                };

                if !is_duplicate {
                    return Err(error);
                }
            }
        }

        Ok(())
    }

    pub async fn ensure_prop_strategy_family_yearly_table(&self) -> Result<(), sqlx::Error> {
        if self.table_exists("prop_strategy_family_yearly").await?
            && (self
                .table_column_exists("prop_strategy_family_yearly", "has_reversal")
                .await?
                || !self
                    .table_column_exists("prop_strategy_family_yearly", "x_strictness")
                    .await?)
        {
            sqlx::query("DROP TABLE prop_strategy_family_yearly")
                .execute(&self.pool)
                .await?;
        }

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS prop_strategy_family_yearly (
                trade_year INT NOT NULL,
                family_key CHAR(16) NOT NULL,
                family_name VARCHAR(64) NOT NULL,
                family_level INT NOT NULL,
                included_dimensions VARCHAR(255) NOT NULL,
                outcome_model VARCHAR(24) NOT NULL,
                market VARCHAR(16) NOT NULL,
                harmonic_type VARCHAR(24) NOT NULL,
                bin VARCHAR(16) NOT NULL,
                reversal_type VARCHAR(32) NOT NULL,
                size_bucket VARCHAR(16) NOT NULL,
                time_bin VARCHAR(16) NOT NULL,
                x_strictness VARCHAR(16) NOT NULL,
                three_month_trend VARCHAR(16) NOT NULL,
                six_month_trend VARCHAR(16) NOT NULL,
                twelve_month_trend VARCHAR(16) NOT NULL,
                total_count BIGINT NOT NULL DEFAULT 0,
                closed_count BIGINT NOT NULL DEFAULT 0,
                open_count BIGINT NOT NULL DEFAULT 0,
                win_count BIGINT NOT NULL DEFAULT 0,
                loss_count BIGINT NOT NULL DEFAULT 0,
                return_sum DOUBLE NOT NULL DEFAULT 0,
                return_count BIGINT NOT NULL DEFAULT 0,
                expectancy_sum DOUBLE NOT NULL DEFAULT 0,
                expectancy_count BIGINT NOT NULL DEFAULT 0,
                win_return_sum DOUBLE NOT NULL DEFAULT 0,
                win_return_count BIGINT NOT NULL DEFAULT 0,
                loss_return_sum DOUBLE NOT NULL DEFAULT 0,
                loss_return_count BIGINT NOT NULL DEFAULT 0,
                target_range_sum DOUBLE NOT NULL DEFAULT 0,
                target_range_count BIGINT NOT NULL DEFAULT 0,
                PRIMARY KEY (trade_year, family_key),
                INDEX idx_prop_family_yearly_family (family_key, trade_year),
                INDEX idx_prop_family_yearly_lookup (
                    family_name,
                    family_level,
                    market,
                    harmonic_type,
                    bin,
                    outcome_model,
                    reversal_type
                )
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn ensure_prop_strategy_family_summary_table(&self) -> Result<(), sqlx::Error> {
        if self.table_exists("prop_strategy_family_summary").await?
            && (self
                .table_column_exists("prop_strategy_family_summary", "has_reversal")
                .await?
                || !self
                    .table_column_exists("prop_strategy_family_summary", "x_strictness")
                    .await?)
        {
            sqlx::query("DROP TABLE prop_strategy_family_summary")
                .execute(&self.pool)
                .await?;
        }

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS prop_strategy_family_summary (
                family_key CHAR(16) NOT NULL PRIMARY KEY,
                family_name VARCHAR(64) NOT NULL,
                family_level INT NOT NULL,
                included_dimensions VARCHAR(255) NOT NULL,
                outcome_model VARCHAR(24) NOT NULL,
                market VARCHAR(16) NOT NULL,
                harmonic_type VARCHAR(24) NOT NULL,
                bin VARCHAR(16) NOT NULL,
                reversal_type VARCHAR(32) NOT NULL,
                size_bucket VARCHAR(16) NOT NULL,
                time_bin VARCHAR(16) NOT NULL,
                x_strictness VARCHAR(16) NOT NULL,
                three_month_trend VARCHAR(16) NOT NULL,
                six_month_trend VARCHAR(16) NOT NULL,
                twelve_month_trend VARCHAR(16) NOT NULL,
                worst_year_expectancy DOUBLE NOT NULL DEFAULT 0,
                down_years BIGINT NOT NULL DEFAULT 0,
                total_count BIGINT NOT NULL DEFAULT 0,
                closed_count BIGINT NOT NULL DEFAULT 0,
                open_count BIGINT NOT NULL DEFAULT 0,
                win_count BIGINT NOT NULL DEFAULT 0,
                loss_count BIGINT NOT NULL DEFAULT 0,
                expectancy DOUBLE NOT NULL DEFAULT 0,
                avg_return DOUBLE NOT NULL DEFAULT 0,
                win_rate DOUBLE NOT NULL DEFAULT 0,
                closed_rate DOUBLE NOT NULL DEFAULT 0,
                avg_win DOUBLE NOT NULL DEFAULT 0,
                avg_loss DOUBLE NOT NULL DEFAULT 0,
                avg_target_range DOUBLE NOT NULL DEFAULT 0,
                score DOUBLE NOT NULL DEFAULT 0,
                INDEX idx_prop_family_summary_rank (
                    score,
                    expectancy,
                    closed_count,
                    win_rate
                ),
                INDEX idx_prop_family_summary_lookup (
                    family_name,
                    family_level,
                    market,
                    harmonic_type,
                    bin,
                    outcome_model,
                    reversal_type
                )
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn ensure_prop_strategy_contract_week_summary_table(
        &self,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS prop_strategy_contract_week_summary (
                family_key CHAR(16) NOT NULL,
                family_name VARCHAR(64) NOT NULL,
                family_level INT NOT NULL,
                included_dimensions VARCHAR(255) NOT NULL,
                outcome_model VARCHAR(24) NOT NULL,
                market VARCHAR(16) NOT NULL,
                harmonic_type VARCHAR(24) NOT NULL,
                bin VARCHAR(16) NOT NULL,
                reversal_type VARCHAR(32) NOT NULL,
                size_bucket VARCHAR(16) NOT NULL,
                time_bin VARCHAR(16) NOT NULL,
                x_strictness VARCHAR(16) NOT NULL,
                three_month_trend VARCHAR(16) NOT NULL,
                six_month_trend VARCHAR(16) NOT NULL,
                twelve_month_trend VARCHAR(16) NOT NULL,
                symbol VARCHAR(32) NOT NULL,
                contract_week_index BIGINT NOT NULL,
                total_count BIGINT NOT NULL DEFAULT 0,
                closed_count BIGINT NOT NULL DEFAULT 0,
                open_count BIGINT NOT NULL DEFAULT 0,
                win_count BIGINT NOT NULL DEFAULT 0,
                loss_count BIGINT NOT NULL DEFAULT 0,
                expectancy DOUBLE NOT NULL DEFAULT 0,
                avg_return DOUBLE NOT NULL DEFAULT 0,
                win_rate DOUBLE NOT NULL DEFAULT 0,
                PRIMARY KEY (family_key, symbol, contract_week_index),
                INDEX idx_prop_contract_week_family (family_key, contract_week_index),
                INDEX idx_prop_contract_week_symbol (symbol, contract_week_index)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn ensure_prop_strategy_family_weekly_cadence_table(
        &self,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS prop_strategy_family_weekly_cadence (
                family_key CHAR(16) NOT NULL PRIMARY KEY,
                family_name VARCHAR(64) NOT NULL,
                family_level INT NOT NULL,
                included_dimensions VARCHAR(255) NOT NULL,
                outcome_model VARCHAR(24) NOT NULL,
                market VARCHAR(16) NOT NULL,
                harmonic_type VARCHAR(24) NOT NULL,
                bin VARCHAR(16) NOT NULL,
                reversal_type VARCHAR(32) NOT NULL,
                size_bucket VARCHAR(16) NOT NULL,
                time_bin VARCHAR(16) NOT NULL,
                x_strictness VARCHAR(16) NOT NULL,
                three_month_trend VARCHAR(16) NOT NULL,
                six_month_trend VARCHAR(16) NOT NULL,
                twelve_month_trend VARCHAR(16) NOT NULL,
                total_calendar_weeks BIGINT NOT NULL DEFAULT 0,
                active_weeks BIGINT NOT NULL DEFAULT 0,
                zero_setup_weeks BIGINT NOT NULL DEFAULT 0,
                zero_setup_week_rate DOUBLE NOT NULL DEFAULT 0,
                total_setups BIGINT NOT NULL DEFAULT 0,
                avg_setups_per_week DOUBLE NOT NULL DEFAULT 0,
                max_setups_per_week BIGINT NOT NULL DEFAULT 0,
                first_week_start DATE NULL,
                last_week_start DATE NULL,
                INDEX idx_prop_family_cadence_rate (zero_setup_week_rate, active_weeks),
                INDEX idx_prop_family_cadence_activity (active_weeks, avg_setups_per_week)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn ensure_dashboard_cache_state_table(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS dashboard_cache_state (
                cache_name VARCHAR(64) NOT NULL PRIMARY KEY,
                is_ready BOOLEAN NOT NULL DEFAULT FALSE,
                last_completed_year INT NULL,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                note TEXT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn set_dashboard_cache_state(
        &self,
        cache_name: &str,
        is_ready: bool,
        last_completed_year: Option<i32>,
        note: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        self.ensure_dashboard_cache_state_table().await?;

        sqlx::query(
            r#"
            INSERT INTO dashboard_cache_state (
                cache_name,
                is_ready,
                last_completed_year,
                note
            )
            VALUES (?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE
                is_ready = VALUES(is_ready),
                last_completed_year = VALUES(last_completed_year),
                note = VALUES(note),
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(cache_name)
        .bind(is_ready)
        .bind(last_completed_year)
        .bind(note)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn clear_generated_outputs(&self) -> Result<(), sqlx::Error> {
        self.ensure_prop_strategy_family_yearly_table().await?;
        self.ensure_prop_strategy_family_summary_table().await?;
        self.ensure_prop_strategy_contract_week_summary_table()
            .await?;
        self.ensure_prop_strategy_family_weekly_cadence_table()
            .await?;
        self.ensure_dashboard_cache_state_table().await?;
        self.ensure_pattern_mode_tables().await?;

        for table in [
            "prop_strategy_family_members",
            "pattern_outcomes_prop_reversal",
            "pattern_outcomes_swing",
            "pattern_harmonic_scores",
            "xabcd_patterns",
            "swing_strategy_yearly",
            "swing_strategy_summary",
            "strategy_cohort_summary_cache",
            "strategy_yearly_rollup",
            "strategy_trade_summary_cache",
            "current_open_setups_cache",
            "prop_strategy_cohort_summary_cache",
            "prop_strategy_yearly_rollup",
            "prop_strategy_yearly",
            "prop_strategy_summary",
            "prop_reversal_strategy_cohort_summary_cache",
            "prop_reversal_strategy_yearly_rollup",
            "prop_reversal_strategy_yearly",
            "prop_reversal_strategy_summary",
        ] {
            sqlx::query(&format!("DROP TABLE IF EXISTS {table}"))
                .execute(&self.pool)
                .await?;
        }

        sqlx::query("TRUNCATE TABLE pattern_outcomes_prop")
            .execute(&self.pool)
            .await?;

        sqlx::query("TRUNCATE TABLE pattern_setups")
            .execute(&self.pool)
            .await?;

        sqlx::query("TRUNCATE TABLE prop_strategy_family_yearly")
            .execute(&self.pool)
            .await?;

        sqlx::query("TRUNCATE TABLE prop_strategy_family_summary")
            .execute(&self.pool)
            .await?;

        sqlx::query("TRUNCATE TABLE prop_strategy_contract_week_summary")
            .execute(&self.pool)
            .await?;

        sqlx::query("TRUNCATE TABLE prop_strategy_family_weekly_cadence")
            .execute(&self.pool)
            .await?;

        self.set_dashboard_cache_state("accuracy_bin_rollup", false, None, Some("cleared"))
            .await?;

        self.set_dashboard_cache_state("structure_rollups", false, None, Some("cleared"))
            .await?;

        self.set_dashboard_cache_state("swing_strategy_rollups", false, None, Some("cleared"))
            .await?;

        self.set_dashboard_cache_state(
            "prop_strategy_family_rollups",
            false,
            None,
            Some("cleared"),
        )
        .await?;

        Ok(())
    }

    pub async fn get_distinct_symbols(&self) -> Result<Vec<String>, sqlx::Error> {
        println!("✅ get_distinct_symbols");

        let symbols: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT DISTINCT symbol
            FROM abcd.candles
            ORDER BY symbol
            "#,
        )
        .fetch_all(&self.pool) // async fetch
        .await?; // must await

        Ok(symbols)
    }

    pub async fn get_symbols_above_average_volume(
        &self,
        minimum_average_volume: f64,
    ) -> Result<Vec<String>, sqlx::Error> {
        println!(
            "Filtering symbols by recent average volume >= {}",
            minimum_average_volume
        );

        let symbols: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT ls.symbol
            FROM abcd.listing_status ls
            INNER JOIN (
                SELECT DISTINCT symbol
                FROM abcd.candles
            ) candles ON candles.symbol = ls.symbol
            WHERE ls.status = 'Active'
              AND (ls.bugged IS NULL OR ls.bugged = false)
              AND ls.average_volume_30d IS NOT NULL
              AND ls.average_volume_30d >= ?
            ORDER BY ls.symbol
            "#,
        )
        .bind(minimum_average_volume)
        .fetch_all(&self.pool)
        .await?;

        Ok(symbols)
    }

    pub async fn get_futures_contract_symbols(
        &self,
        root_symbol: Option<&str>,
    ) -> Result<Vec<String>, sqlx::Error> {
        println!("Selecting futures contract symbols");

        let symbols: Vec<String> = if let Some(root_symbol) = root_symbol {
            sqlx::query_scalar(
                r#"
                SELECT DISTINCT symbol
                FROM abcd.futures_contract_1m_candles
                WHERE root_symbol = ?
                ORDER BY symbol
                "#,
            )
            .bind(root_symbol)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_scalar(
                r#"
                SELECT DISTINCT symbol
                FROM abcd.futures_contract_1m_candles
                ORDER BY symbol
                "#,
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(symbols)
    }

    pub async fn get_stored_candles(&self, symbol: &str) -> Result<Vec<Candle>, sqlx::Error> {
        self.get_stored_candles_with_trend_persist(symbol, false)
            .await
    }

    pub async fn get_stored_futures_contract_candles(
        &self,
        symbol: &str,
    ) -> Result<Vec<Candle>, sqlx::Error> {
        let candles: Vec<Candle> = sqlx::query_as::<_, Candle>(
            r#"
            SELECT
                symbol,
                ts_utc AS date,
                open,
                high,
                low,
                close,
                volume,
                CAST(NULL AS SIGNED) AS three_month,
                CAST(NULL AS SIGNED) AS six_month,
                CAST(NULL AS SIGNED) AS twelve_month
            FROM abcd.futures_contract_1m_candles
            WHERE symbol = ?
            ORDER BY ts_utc
            "#,
        )
        .bind(symbol)
        .fetch_all(&self.pool)
        .await?;

        Ok(candles)
    }

    pub async fn get_stored_candles_with_trend_persist(
        &self,
        symbol: &str,
        persist_trends: bool,
    ) -> Result<Vec<Candle>, sqlx::Error> {
        let candles_decimal: Vec<CandleDecimal> = sqlx::query_as::<_, CandleDecimal>(
            r#"
                SELECT
                    symbol,
                    CAST(date AS DATETIME) AS date,
                    open,
                    high,
                    low,
                    close,
                    volume,
                    three_month,
                    six_month,
                    twelve_month
                FROM abcd.candles
                WHERE symbol = ?
                ORDER BY date
                "#,
        )
        .bind(symbol)
        .fetch_all(&self.pool)
        .await?;

        let mut candles: Vec<Candle> = candles_decimal
            .into_iter()
            .map(|c| Candle {
                symbol: c.symbol,
                date: c.date,
                open: c.open.to_f64().unwrap(),
                high: c.high.to_f64().unwrap(),
                low: c.low.to_f64().unwrap(),
                close: c.close.to_f64().unwrap(),
                volume: c.volume,
                three_month: c.three_month,
                six_month: c.six_month,
                twelve_month: c.twelve_month,
            })
            .collect();

        apply_sma_trends_to_candles(&mut candles);
        if persist_trends {
            self.persist_candle_trends(&candles).await?;
        }

        Ok(candles)
    }

    pub async fn insert_pattern_setups(
        &self,
        patterns: &[PatternXABCD],
    ) -> Result<(), sqlx::Error> {
        self.insert_pattern_setups_with_timings(
            patterns, None, false, false, true, true, true, false,
        )
        .await
    }

    pub async fn insert_pattern_setups_with_timings(
        &self,
        patterns: &[PatternXABCD],
        run_id: Option<&str>,
        fast_rebuild: bool,
        use_build_tables: bool,
        write_pattern_setups: bool,
        write_harmonic_scores: bool,
        write_swing_outcomes: bool,
        write_prop_outcomes: bool,
    ) -> Result<(), sqlx::Error> {
        if patterns.is_empty() {
            return Ok(());
        }

        if !fast_rebuild && !use_build_tables {
            self.ensure_pattern_mode_tables().await?;
        }

        let serialize_started = Instant::now();
        let mut seen_setup_ids = HashSet::new();
        let serialized_patterns: Vec<XABCD_CSV> = patterns
            .iter()
            .map(|pattern| self.from_pattern(pattern))
            .filter(|pattern| seen_setup_ids.insert(build_pattern_setup_id(pattern)))
            .collect();
        let serialize_duration = serialize_started.elapsed();

        if let Some(run_id) = run_id {
            self.record_engine_phase_timing(
                run_id,
                None,
                "serialize_patterns",
                Some(patterns.len() as i64),
                serialize_duration,
                None,
            )
            .await?;
        }

        self.upsert_pattern_mode_tables(
            &serialized_patterns,
            run_id,
            fast_rebuild,
            use_build_tables,
            write_pattern_setups,
            write_harmonic_scores,
            write_swing_outcomes,
            write_prop_outcomes,
        )
        .await?;
        Ok(())
    }

    pub async fn drop_legacy_xabcd_patterns_table(&self) -> Result<(), sqlx::Error> {
        for table in ["xabcd_patterns", "xabcd_patterns_build"] {
            sqlx::query(&format!("DROP TABLE IF EXISTS {table}"))
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }

    pub async fn insert_patterns(&self, patterns: &[PatternXABCD]) -> Result<(), sqlx::Error> {
        self.insert_pattern_setups(patterns).await
    }

    pub async fn sync_prop_outcomes(
        &self,
        patterns: &[PatternXABCD],
        outcomes: &[PropReversalOutcome],
        use_build_tables: bool,
        target_ready_only: bool,
    ) -> Result<i64, sqlx::Error> {
        if patterns.is_empty() {
            return Ok(0);
        }

        if !use_build_tables {
            self.ensure_pattern_mode_tables().await?;
        }

        let tables = if use_build_tables {
            PatternOutputTables::build_tables()
        } else {
            PatternOutputTables::final_tables()
        };

        if !use_build_tables {
            let mut setup_ids: Vec<String> = patterns
                .iter()
                .map(|pattern| pattern.pattern_id.clone())
                .filter(|pattern_id| !pattern_id.is_empty())
                .collect();

            setup_ids.sort();
            setup_ids.dedup();

            for chunk in setup_ids.chunks(PATTERN_INSERT_CHUNK_SIZE) {
                let mut builder = QueryBuilder::<MySql>::new(format!(
                    "DELETE FROM {} WHERE setup_id IN (",
                    tables.prop_outcomes
                ));
                let mut separated = builder.separated(", ");
                for setup_id in chunk {
                    separated.push_bind(setup_id);
                }
                separated.push_unseparated(")");
                builder.build().execute(&self.pool).await?;
            }
        }

        if patterns.is_empty() && outcomes.is_empty() {
            return Ok(0);
        }

        let mut seen_direct_row_ids: HashSet<String> = HashSet::new();
        let unique_patterns: Vec<&PatternXABCD> = patterns
            .iter()
            .filter(|pattern| !pattern.pattern_id.is_empty())
            .filter(|pattern| !target_ready_only || pattern.target_candle.is_some())
            .filter(|pattern| {
                let outcome_row_id =
                    format!("{:x}", md5::compute(format!("{}|D", pattern.pattern_id)));
                seen_direct_row_ids.insert(outcome_row_id)
            })
            .collect();

        let mut seen_reversal_row_ids: HashSet<String> = HashSet::new();
        let unique_outcomes: Vec<&PropReversalOutcome> = outcomes
            .iter()
            .filter(|item| !target_ready_only || item.target_ready)
            .filter(|item| seen_reversal_row_ids.insert(item.reversal_row_id.clone()))
            .collect();

        let mut rows_written = 0i64;
        let mut tx = self.pool.begin().await?;

        for chunk in unique_patterns.chunks(PATTERN_INSERT_CHUNK_SIZE) {
            let mut builder = QueryBuilder::<MySql>::new(format!(
                r#"
                INSERT INTO {} (
                    outcome_row_id,
                    setup_id,
                    prop_strategy_id,
                    outcome_model,
                    has_reversal,
                    reversal_type,
                    reversal_detect_date,
                    reversal_bars_after_d,
                    pattern_id,
                    pattern_group_id,
                    x_bars_left,
                    symbol,
                    d_date,
                    contract_week_index,
                    contract_days_from_start,
                    entry_date,
                    market,
                    harmonic_type,
                    bin,
                    size_bucket,
                    time_bin,
                    three_month_trend,
                    six_month_trend,
                    twelve_month_trend,
                    x_length,
                    a_length,
                    b_length,
                    c_length,
                    d_length,
                    full_pattern_length,
                    trade_enter_price,
                    trade_risk_exit_price,
                    trade_reward_exit_price,
                    d_confirm_date,
                    prop_result,
                    target_ready,
                    target_date,
                    target_open,
                    target_high,
                    target_low,
                    target_close,
                    target_volume,
                    target_is_green,
                    target_close_vs_open_pct,
                    target_high_vs_open_pct,
                    target_low_vs_open_pct,
                    target_range_pct,
                    target_breaks_entry_high,
                    target_breaks_entry_low
                )
                "#,
                tables.prop_outcomes
            ));

            let chunk_rows = chunk.len() as i64;
            if chunk_rows == 0 {
                continue;
            }

            builder.push_values(chunk.iter().copied(), |mut row, pattern| {
                let setup_id = pattern.pattern_id.clone();
                let outcome_row_id = format!("{:x}", md5::compute(format!("{setup_id}|D")));
                let pattern_group_id = format!("{}{}", pattern.symbol, pattern.a.date);
                let market = format!("{:?}", pattern.market);
                let lens = pattern.dominant_harmonic_lens();
                let size_bucket = route_size_bucket(
                    pattern.x.length,
                    pattern.a.length,
                    pattern.b.length,
                    pattern.c.length,
                );
                let target = pattern.target_candle;

                row.push_bind(outcome_row_id)
                    .push_bind(setup_id.clone())
                    .push_bind(&pattern.prop_strategy_id)
                    .push_bind("D")
                    .push_bind(false)
                    .push_bind("None")
                    .push_bind(None::<NaiveDateTime>)
                    .push_bind(None::<i64>)
                    .push_bind(Some(setup_id.clone()))
                    .push_bind(pattern_group_id)
                    .push_bind(pattern.x_bars_left)
                    .push_bind(pattern.symbol.as_ref())
                    .push_bind(pattern.d.date)
                    .push_bind(pattern.contract_week_index)
                    .push_bind(pattern.contract_days_from_start)
                    .push_bind(pattern.trade.entry_date)
                    .push_bind(market)
                    .push_bind(lens.harmonic_type)
                    .push_bind(lens.bin)
                    .push_bind(size_bucket)
                    .push_bind(lens.time_bin)
                    .push_bind(trend_label(pattern.three_month))
                    .push_bind(trend_label(pattern.six_month))
                    .push_bind(trend_label(pattern.twelve_month))
                    .push_bind(pattern.x.length)
                    .push_bind(pattern.a.length)
                    .push_bind(pattern.b.length)
                    .push_bind(pattern.c.length)
                    .push_bind(pattern.d.length)
                    .push_bind(
                        pattern.x.length
                            + pattern.a.length
                            + pattern.b.length
                            + pattern.c.length
                            + pattern.d.length,
                    )
                    .push_bind(pattern.trade.enter_price)
                    .push_bind(pattern.trade.risk_exit_price)
                    .push_bind(pattern.trade.reward_exit_price)
                    .push_bind(pattern.d_confirm_date)
                    .push_bind(pattern.trade.result)
                    .push_bind(target.is_some())
                    .push_bind(target.map(|target| target.date))
                    .push_bind(target.map(|target| target.open))
                    .push_bind(target.map(|target| target.high))
                    .push_bind(target.map(|target| target.low))
                    .push_bind(target.map(|target| target.close))
                    .push_bind(target.map(|target| target.volume))
                    .push_bind(target.map(|target| target.is_green))
                    .push_bind(target.map(|target| target.close_vs_open_pct))
                    .push_bind(target.map(|target| target.high_vs_open_pct))
                    .push_bind(target.map(|target| target.low_vs_open_pct))
                    .push_bind(target.map(|target| target.range_pct))
                    .push_bind(target.map(|target| target.breaks_d_high))
                    .push_bind(target.map(|target| target.breaks_d_low));
            });

            builder.build().execute(&mut *tx).await?;
            rows_written += chunk_rows;
        }

        for chunk in unique_outcomes.chunks(PATTERN_INSERT_CHUNK_SIZE) {
            let mut builder = QueryBuilder::<MySql>::new(format!(
                r#"
                INSERT INTO {} (
                    outcome_row_id,
                    setup_id,
                    prop_strategy_id,
                    outcome_model,
                    has_reversal,
                    reversal_type,
                    reversal_detect_date,
                    reversal_bars_after_d,
                    pattern_id,
                    pattern_group_id,
                    x_bars_left,
                    symbol,
                    d_date,
                    contract_week_index,
                    contract_days_from_start,
                    entry_date,
                    market,
                    harmonic_type,
                    bin,
                    size_bucket,
                    time_bin,
                    three_month_trend,
                    six_month_trend,
                    twelve_month_trend,
                    x_length,
                    a_length,
                    b_length,
                    c_length,
                    d_length,
                    full_pattern_length,
                    trade_enter_price,
                    trade_risk_exit_price,
                    trade_reward_exit_price,
                    d_confirm_date,
                    prop_result,
                    target_ready,
                    target_date,
                    target_open,
                    target_high,
                    target_low,
                    target_close,
                    target_volume,
                    target_is_green,
                    target_close_vs_open_pct,
                    target_high_vs_open_pct,
                    target_low_vs_open_pct,
                    target_range_pct,
                    target_breaks_entry_high,
                    target_breaks_entry_low
                )
                "#,
                tables.prop_outcomes
            ));

            let chunk_rows = chunk.len() as i64;
            if chunk_rows == 0 {
                continue;
            }

            builder.push_values(chunk.iter().copied(), |mut row, item| {
                row.push_bind(&item.reversal_row_id)
                    .push_bind(&item.setup_id)
                    .push_bind(&item.prop_strategy_id)
                    .push_bind("DReversal")
                    .push_bind(true)
                    .push_bind(&item.reversal_type)
                    .push_bind(Some(item.reversal_detect_date))
                    .push_bind(Some(item.reversal_bars_after_d))
                    .push_bind(&item.pattern_id)
                    .push_bind(&item.pattern_group_id)
                    .push_bind(item.x_bars_left)
                    .push_bind(&item.symbol)
                    .push_bind(item.d_date)
                    .push_bind(item.contract_week_index)
                    .push_bind(item.contract_days_from_start)
                    .push_bind(item.entry_date)
                    .push_bind(&item.market)
                    .push_bind(&item.harmonic_type)
                    .push_bind(&item.bin)
                    .push_bind(&item.size_bucket)
                    .push_bind(&item.time_bin)
                    .push_bind(&item.three_month_trend)
                    .push_bind(&item.six_month_trend)
                    .push_bind(&item.twelve_month_trend)
                    .push_bind(item.x_length)
                    .push_bind(item.a_length)
                    .push_bind(item.b_length)
                    .push_bind(item.c_length)
                    .push_bind(item.d_length)
                    .push_bind(item.full_pattern_length)
                    .push_bind(item.trade_enter_price)
                    .push_bind(item.trade_risk_exit_price)
                    .push_bind(item.trade_reward_exit_price)
                    .push_bind(item.d_confirm_date)
                    .push_bind(item.trade_result)
                    .push_bind(item.target_ready)
                    .push_bind(item.target_date)
                    .push_bind(item.target_open)
                    .push_bind(item.target_high)
                    .push_bind(item.target_low)
                    .push_bind(item.target_close)
                    .push_bind(item.target_volume)
                    .push_bind(item.target_is_green)
                    .push_bind(item.target_close_vs_open_pct)
                    .push_bind(item.target_high_vs_open_pct)
                    .push_bind(item.target_low_vs_open_pct)
                    .push_bind(item.target_range_pct)
                    .push_bind(item.target_breaks_reversal_high)
                    .push_bind(item.target_breaks_reversal_low);
            });

            builder.build().execute(&mut *tx).await?;
            rows_written += chunk_rows;
        }

        tx.commit().await?;

        Ok(rows_written)
    }

    pub fn from_pattern(&self, p: &PatternXABCD) -> XABCD_CSV {
        let harmonic_lens = p.dominant_harmonic_lens();
        let mut csv = XABCD_CSV {
            symbol: p.symbol.to_string(),
            pattern_id: p.pattern_id.clone(),
            x_bars_left: p.x_bars_left,
            x_date: p.x.date.to_string(),
            x_open: p.x.open,
            x_high: p.x.high,
            x_low: p.x.low,
            x_close: p.x.close,
            x_length: p.x.length,
            x_min_max: p.x.min_max,
            a_date: p.a.date.to_string(),
            a_open: p.a.open,
            a_high: p.a.high,
            a_low: p.a.low,
            a_close: p.a.close,
            a_length: p.a.length,
            xa_price_length: p.a.leg_price_length,
            a_min_max: p.a.min_max,
            b_date: p.b.date.to_string(),
            b_open: p.b.open,
            b_high: p.b.high,
            b_low: p.b.low,
            b_close: p.b.close,
            b_length: p.b.length,
            b_min_max: p.b.min_max,
            ab_price_length: p.b.leg_price_length,
            c_date: p.c.date.to_string(),
            c_open: p.c.open,
            c_high: p.c.high,
            c_low: p.c.low,
            c_close: p.c.close,
            c_length: p.c.length,
            c_min_max: p.c.min_max,
            bc_price_length: p.c.leg_price_length,
            d_date: p.d.date.to_string(),
            d_open: p.d.open,
            d_high: p.d.high,
            d_low: p.d.low,
            d_close: p.d.close,
            d_confirm_date: p.d_confirm_date.to_string(),
            target_ready: p.target_candle.is_some(),
            target_date: p.target_candle.map(|target| target.date.to_string()),
            target_open: p.target_candle.map(|target| target.open),
            target_high: p.target_candle.map(|target| target.high),
            target_low: p.target_candle.map(|target| target.low),
            target_close: p.target_candle.map(|target| target.close),
            target_volume: p.target_candle.map(|target| target.volume),
            target_is_green: p.target_candle.map(|target| target.is_green),
            target_close_vs_open_pct: p.target_candle.map(|target| target.close_vs_open_pct),
            target_high_vs_open_pct: p.target_candle.map(|target| target.high_vs_open_pct),
            target_low_vs_open_pct: p.target_candle.map(|target| target.low_vs_open_pct),
            target_range_pct: p.target_candle.map(|target| target.range_pct),
            target_breaks_d_high: p.target_candle.map(|target| target.breaks_d_high),
            target_breaks_d_low: p.target_candle.map(|target| target.breaks_d_low),
            d_length: p.d.length,
            full_pattern_length: p.x.length + p.a.length + p.b.length + p.c.length + p.d.length,
            d_min_max: p.d.min_max,
            cd_price_length: p.d.leg_price_length,
            trade_open: p.trade.open,
            trade_risk_exit_price: p.trade.risk_exit_price,
            trade_reward_exit_price: p.trade.reward_exit_price,
            trade_enter_price: p.trade.enter_price,
            trade_current_price: p.trade.current_price,
            trade_length: p.trade.length,
            trade_pnl: p.trade.pnl,
            trade_result: p.trade.result,
            trade_date: p.trade.date.to_string(),
            trade_symbol: p.symbol.to_string(),
            trade_ab_price_retracement: p.trade.ab_price_retracement,
            trade_bc_price_retracement: p.trade.bc_price_retracement,
            trade_cd_xa_price_retracement: p.trade.cd_xa_price_retracement,
            trade_cd_price_retracement: p.trade.cd_price_retracement,
            trade_ab_bar_retracement: p.trade.ab_bar_retracement,
            trade_bc_bar_retracement: p.trade.bc_bar_retracement,
            trade_cd_bar_retracement: p.trade.cd_bar_retracement,
            trade_cd_bc_bar_retracement: p.trade.cd_bc_bar_retracement,
            trade_cd_xa_bar_retracement: p.trade.cd_xa_bar_retracement,
            trade_cd_bc_price_retracement: p.trade.cd_bc_price_retracement,
            trade_snr: p.trade.snr,
            trade_year: p.trade.year,
            trade_month: p.trade.month,
            trade_day: p.trade.day,
            reversal_type: p.trade.reversal_type.clone(),
            bullish_key_reversal: p.trade.bullish_key_reversal,
            bearish_key_reversal: p.trade.bearish_key_reversal,
            bullish_engulfing: p.trade.bullish_engulfing,
            bearish_engulfing: p.trade.bearish_engulfing,
            bullish_outside_reversal: p.trade.bullish_outside_reversal,
            bearish_outside_reversal: p.trade.bearish_outside_reversal,
            hammer: p.trade.hammer,
            shooting_star: p.trade.shooting_star,
            morning_star: p.trade.morning_star,
            evening_star: p.trade.evening_star,
            three_white_soldiers: p.trade.three_white_soldiers,
            three_black_crows: p.trade.three_black_crows,
            market: p.market,
            three_month: p.three_month,
            six_month: p.six_month,
            twelve_month: p.twelve_month,
            pattern_group_id: format!("{}{}", p.symbol, p.a.date),
            prop_strategy_id: p.prop_strategy_id.clone(),
            swing_strategy_id: String::new(),
            harmonic_type: "Multi".to_string(),
            route_harmonic_type: harmonic_lens.harmonic_type.to_string(),
            route_bin: harmonic_lens.bin.to_string(),
            route_time_bin: harmonic_lens.time_bin.to_string(),
            bat_accuracy: p.accuracies.bat.pattern_accuracy,
            alternate_bat_accuracy: p.accuracies.alternate_bat.pattern_accuracy,
            butterfly_accuracy: p.accuracies.butterfly.pattern_accuracy,
            gartley_accuracy: p.accuracies.gartley.pattern_accuracy,
            crab_accuracy: p.accuracies.crab.pattern_accuracy,
            deep_crab_accuracy: p.accuracies.deep_crab.pattern_accuracy,
            shark_accuracy: p.accuracies.shark.pattern_accuracy,
            time_accuracy: p.time_accuracies.compatibility_scalar(),
            bat_time_accuracy: p.time_accuracies.bat.time_accuracy,
            alternate_bat_time_accuracy: p.time_accuracies.alternate_bat.time_accuracy,
            butterfly_time_accuracy: p.time_accuracies.butterfly.time_accuracy,
            gartley_time_accuracy: p.time_accuracies.gartley.time_accuracy,
            crab_time_accuracy: p.time_accuracies.crab.time_accuracy,
            deep_crab_time_accuracy: p.time_accuracies.deep_crab.time_accuracy,
            shark_time_accuracy: p.time_accuracies.shark.time_accuracy,
        };
        csv.swing_strategy_id = build_swing_strategy_id(&csv);
        csv
    }

    pub async fn refresh_accuracy_bin_cache(&self) -> Result<(), sqlx::Error> {
        self.ensure_accuracy_bin_cache_table().await?;
        self.ensure_pattern_harmonic_scores_table().await?;
        let mut aggregates: HashMap<AccuracyCacheKey, AccuracyBinAccumulator> = HashMap::new();

        for source_scope in CACHE_SOURCE_SCOPES {
            for market_scope in CACHE_MARKET_SCOPES {
                for trade_result_scope in CACHE_TRADE_RESULT_SCOPES {
                    for harmonic_type in CACHE_HARMONIC_TYPES {
                        for bin in CACHE_BINS {
                            aggregates.insert(
                                (
                                    source_scope.to_string(),
                                    market_scope.to_string(),
                                    trade_result_scope,
                                    harmonic_type.to_string(),
                                    bin.to_string(),
                                ),
                                AccuracyBinAccumulator::default(),
                            );
                        }
                    }
                }
            }
        }

        let ranges = sqlx::query_as::<_, MarketDateRange>(
            r#"
            SELECT
                s.market,
                DATE(MIN(s.d_date)) AS min_d_date,
                DATE(MAX(s.d_date)) AS max_d_date
            FROM pattern_setups s
            GROUP BY s.market
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        for range in ranges {
            let Some(min_d_date) = range.min_d_date else {
                continue;
            };
            let Some(max_d_date) = range.max_d_date else {
                continue;
            };

            let mut chunk_start =
                NaiveDate::from_ymd_opt(min_d_date.year(), 1, 1).unwrap_or(min_d_date);
            if chunk_start > min_d_date {
                chunk_start = min_d_date;
            }

            while chunk_start <= max_d_date {
                let next_year = chunk_start.year() + 1;
                let chunk_end = NaiveDate::from_ymd_opt(next_year, 1, 1)
                    .unwrap_or_else(|| max_d_date.succ_opt().unwrap_or(max_d_date));

                let mut rows = sqlx::query_as::<_, AccuracySourceRow>(
                    r#"
                    SELECT
                        DATE(s.d_date) AS d_date,
                        s.market,
                        sw.trade_result,
                        CAST(sw.trade_pnl AS DOUBLE) AS trade_pnl,
                        hs.harmonic_type,
                        CAST(hs.price_accuracy AS DOUBLE) AS price_accuracy
                    FROM pattern_setups s
                    INNER JOIN pattern_harmonic_scores hs
                        ON hs.setup_id = s.setup_id
                    LEFT JOIN pattern_outcomes_swing sw
                        ON sw.setup_id = s.setup_id
                    WHERE s.market = ?
                      AND s.d_date >= ?
                      AND s.d_date < ?
                    "#,
                )
                .bind(&range.market)
                .bind(chunk_start)
                .bind(chunk_end)
                .fetch(&self.pool);

                while let Some(row) = rows.try_next().await? {
                    let _ = row.d_date;

                    if let Some(accuracy) = row.price_accuracy {
                        update_accuracy_cache(
                            &mut aggregates,
                            "all_patterns",
                            &row.harmonic_type,
                            accuracy,
                            &row.market,
                            row.trade_result,
                            row.trade_pnl,
                        );
                    }
                }

                chunk_start = chunk_end;
            }
        }

        let cache_rows = finalize_accuracy_cache_rows(aggregates);

        sqlx::query("TRUNCATE TABLE accuracy_bin_cache")
            .execute(&self.pool)
            .await?;

        let mut tx = self.pool.begin().await?;

        for chunk in cache_rows.chunks(500) {
            let mut builder = QueryBuilder::<MySql>::new(
                r#"
                INSERT INTO accuracy_bin_cache (
                    source_scope,
                    market_scope,
                    trade_result_scope,
                    harmonic_type,
                    bin,
                    count,
                    total_count,
                    open_count,
                    win_count,
                    loss_count,
                    avg_return,
                    expectancy,
                    win_rate,
                    closed_rate,
                    avg_win,
                    avg_loss
                )
                "#,
            );

            builder.push_values(chunk, |mut row, item| {
                row.push_bind(&item.source_scope)
                    .push_bind(&item.market_scope)
                    .push_bind(item.trade_result_scope)
                    .push_bind(&item.harmonic_type)
                    .push_bind(&item.bin)
                    .push_bind(item.count)
                    .push_bind(item.total_count)
                    .push_bind(item.open_count)
                    .push_bind(item.win_count)
                    .push_bind(item.loss_count)
                    .push_bind(item.avg_return)
                    .push_bind(item.expectancy)
                    .push_bind(item.win_rate)
                    .push_bind(item.closed_rate)
                    .push_bind(item.avg_win)
                    .push_bind(item.avg_loss);
            });

            builder.build().execute(&mut *tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn upsert_accuracy_bin_cache_from_patterns(
        &self,
        patterns: &[PatternXABCD],
    ) -> Result<(), sqlx::Error> {
        if patterns.is_empty() {
            return Ok(());
        }

        self.ensure_accuracy_bin_cache_table().await?;

        let mut aggregates: HashMap<AccuracyCacheKey, AccuracyBinAccumulator> = HashMap::new();

        for pattern in patterns {
            let market = market_label(pattern.market);
            let trade_result = Some(pattern.trade.result);
            let trade_pnl = Some(pattern.trade.pnl);

            for (harmonic_type, accuracy) in [
                ("Bat", pattern.accuracies.bat.pattern_accuracy),
                (
                    "AlternateBat",
                    pattern.accuracies.alternate_bat.pattern_accuracy,
                ),
                ("Butterfly", pattern.accuracies.butterfly.pattern_accuracy),
                ("Gartley", pattern.accuracies.gartley.pattern_accuracy),
                ("Crab", pattern.accuracies.crab.pattern_accuracy),
                ("DeepCrab", pattern.accuracies.deep_crab.pattern_accuracy),
                ("Shark", pattern.accuracies.shark.pattern_accuracy),
            ] {
                update_accuracy_cache(
                    &mut aggregates,
                    "all_patterns",
                    harmonic_type,
                    accuracy,
                    market,
                    trade_result,
                    trade_pnl,
                );
            }
        }

        let cache_rows = finalize_accuracy_cache_rows(aggregates);
        let mut tx = self.pool.begin().await?;

        for chunk in cache_rows.chunks(500) {
            let mut builder = QueryBuilder::<MySql>::new(
                r#"
                INSERT INTO accuracy_bin_cache (
                    source_scope,
                    market_scope,
                    trade_result_scope,
                    harmonic_type,
                    bin,
                    count,
                    total_count,
                    open_count,
                    win_count,
                    loss_count,
                    avg_return,
                    expectancy,
                    win_rate,
                    closed_rate,
                    avg_win,
                    avg_loss
                )
                "#,
            );

            builder.push_values(chunk, |mut row, item| {
                row.push_bind(&item.source_scope)
                    .push_bind(&item.market_scope)
                    .push_bind(item.trade_result_scope)
                    .push_bind(&item.harmonic_type)
                    .push_bind(&item.bin)
                    .push_bind(item.count)
                    .push_bind(item.total_count)
                    .push_bind(item.open_count)
                    .push_bind(item.win_count)
                    .push_bind(item.loss_count)
                    .push_bind(item.avg_return)
                    .push_bind(item.expectancy)
                    .push_bind(item.win_rate)
                    .push_bind(item.closed_rate)
                    .push_bind(item.avg_win)
                    .push_bind(item.avg_loss);
            });

            builder.push(
                r#"
                ON DUPLICATE KEY UPDATE
                    `count` = `count` + VALUES(`count`),
                    total_count = total_count + VALUES(total_count),
                    open_count = open_count + VALUES(open_count),
                    win_count = win_count + VALUES(win_count),
                    loss_count = loss_count + VALUES(loss_count),
                    avg_return = CASE
                        WHEN total_count + VALUES(total_count) > 0 THEN
                            ((avg_return * total_count) + (VALUES(avg_return) * VALUES(total_count)))
                            / (total_count + VALUES(total_count))
                        ELSE 0.0
                    END,
                    expectancy = CASE
                        WHEN `count` + VALUES(`count`) > 0 THEN
                            ((expectancy * `count`) + (VALUES(expectancy) * VALUES(`count`)))
                            / (`count` + VALUES(`count`))
                        ELSE 0.0
                    END,
                    win_rate = CASE
                        WHEN `count` + VALUES(`count`) > 0 THEN
                            (win_count + VALUES(win_count)) / (`count` + VALUES(`count`))
                        ELSE 0.0
                    END,
                    closed_rate = CASE
                        WHEN total_count + VALUES(total_count) > 0 THEN
                            (`count` + VALUES(`count`)) / (total_count + VALUES(total_count))
                        ELSE 0.0
                    END,
                    avg_win = CASE
                        WHEN win_count + VALUES(win_count) > 0 THEN
                            ((avg_win * win_count) + (VALUES(avg_win) * VALUES(win_count)))
                            / (win_count + VALUES(win_count))
                        ELSE 0.0
                    END,
                    avg_loss = CASE
                        WHEN loss_count + VALUES(loss_count) > 0 THEN
                            ((avg_loss * loss_count) + (VALUES(avg_loss) * VALUES(loss_count)))
                            / (loss_count + VALUES(loss_count))
                        ELSE 0.0
                    END
                "#,
            );

            builder.build().execute(&mut *tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn refresh_structure_rollups(&self) -> Result<(), sqlx::Error> {
        self.ensure_dashboard_cache_state_table().await?;

        self.set_dashboard_cache_state("structure_rollups", false, None, Some("refreshing"))
            .await?;
        self.set_dashboard_cache_state("swing_strategy_rollups", false, None, Some("refreshing"))
            .await?;

        sqlx::query("DROP TABLE IF EXISTS pattern_structure_rollup")
            .execute(&self.pool)
            .await?;
        sqlx::query("DROP TABLE IF EXISTS accuracy_structure_rollup")
            .execute(&self.pool)
            .await?;
        sqlx::query("DROP TABLE IF EXISTS swing_strategy_yearly")
            .execute(&self.pool)
            .await?;
        sqlx::query("DROP TABLE IF EXISTS swing_strategy_summary")
            .execute(&self.pool)
            .await?;

        self.ensure_pattern_structure_rollup_table().await?;
        self.ensure_accuracy_structure_rollup_table().await?;
        self.ensure_swing_strategy_yearly_table().await?;
        self.ensure_swing_strategy_summary_table().await?;
        self.ensure_pattern_harmonic_scores_table().await?;

        let total_bars_expr = structure_total_bars_expr();
        let size_bucket_expr = structure_size_bucket_expr(total_bars_expr);
        let size_order_expr = structure_size_bucket_order_expr(total_bars_expr);
        let balance_ratio_expr = structure_balance_ratio_expr();
        let balance_bucket_expr = structure_balance_bucket_expr(&balance_ratio_expr);
        let balance_order_expr = structure_balance_bucket_order_expr(&balance_ratio_expr);
        let three_month_trend_expr = trend_bucket_expr("three_month");
        let six_month_trend_expr = trend_bucket_expr("six_month");
        let twelve_month_trend_expr = trend_bucket_expr("twelve_month");

        let pattern_structure_sql = format!(
            r#"
            INSERT INTO pattern_structure_rollup (
                market,
                trade_result_value,
                harmonic_type,
                bin,
                size_bucket,
                size_order,
                balance_bucket,
                balance_order,
                total_count,
                closed_count,
                open_count,
                win_count,
                expectancy_sum,
                expectancy_count,
                total_bars_sum,
                total_bars_count,
                balance_ratio_sum,
                balance_ratio_count,
                distorted_count
            )
            SELECT
                market,
                trade_result_value,
                harmonic_type,
                bin,
                size_bucket,
                size_order,
                balance_bucket,
                balance_order,
                CAST(COUNT(*) AS SIGNED) AS total_count,
                CAST(SUM(CASE WHEN trade_result_value IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
                CAST(SUM(CASE WHEN trade_result_value NOT IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS open_count,
                CAST(SUM(CASE WHEN trade_result_value = 1 THEN 1 ELSE 0 END) AS SIGNED) AS win_count,
                COALESCE(SUM(CASE WHEN trade_result_value IN (1, 2) THEN trade_pnl ELSE 0.0 END), 0.0) AS expectancy_sum,
                CAST(SUM(CASE WHEN trade_result_value IN (1, 2) AND trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS expectancy_count,
                COALESCE(SUM(total_bars), 0.0) AS total_bars_sum,
                CAST(SUM(CASE WHEN total_bars IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS total_bars_count,
                COALESCE(SUM(COALESCE(balance_ratio, 0.0)), 0.0) AS balance_ratio_sum,
                CAST(SUM(CASE WHEN balance_ratio IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS balance_ratio_count,
                CAST(SUM(CASE WHEN balance_bucket = 'Distorted' THEN 1 ELSE 0 END) AS SIGNED) AS distorted_count
            FROM (
                SELECT
                    s.market AS market,
                    COALESCE(sw.trade_result, 0) AS trade_result_value,
                    'Multi' AS harmonic_type,
                    COALESCE(NULLIF(sw.bin, ''), 'Multi') AS bin,
                    {size_bucket_expr} AS size_bucket,
                    {size_order_expr} AS size_order,
                    {balance_ratio_expr} AS balance_ratio,
                    {balance_bucket_expr} AS balance_bucket,
                    {balance_order_expr} AS balance_order,
                    {total_bars_expr} AS total_bars,
                    CAST(sw.trade_pnl AS DOUBLE) AS trade_pnl
                FROM pattern_setups s
                LEFT JOIN pattern_outcomes_swing sw
                    ON sw.setup_id = s.setup_id
                WHERE 1 = 1
            ) source_rows
            WHERE bin IS NOT NULL
            GROUP BY
                market,
                trade_result_value,
                harmonic_type,
                bin,
                size_bucket,
                size_order,
                balance_bucket,
                balance_order
            "#,
        );
        sqlx::query(&pattern_structure_sql)
            .execute(&self.pool)
            .await?;

        let accuracy_structure_all_patterns_sql = format!(
            r#"
            INSERT INTO accuracy_structure_rollup (
                source_scope,
                market,
                trade_result_value,
                harmonic_type,
                bin,
                size_bucket,
                balance_bucket,
                total_count,
                closed_count,
                open_count,
                win_count,
                loss_count,
                return_sum,
                return_count,
                expectancy_sum,
                expectancy_count,
                win_return_sum,
                win_return_count,
                loss_return_sum,
                loss_return_count
            )
            SELECT
                'all_patterns' AS source_scope,
                market,
                trade_result_value,
                harmonic_type,
                bin,
                size_bucket,
                balance_bucket,
                CAST(COUNT(*) AS SIGNED) AS total_count,
                CAST(SUM(CASE WHEN trade_result_value IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
                CAST(SUM(CASE WHEN trade_result_value NOT IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS open_count,
                CAST(SUM(CASE WHEN trade_result_value = 1 THEN 1 ELSE 0 END) AS SIGNED) AS win_count,
                CAST(SUM(CASE WHEN trade_result_value = 2 THEN 1 ELSE 0 END) AS SIGNED) AS loss_count,
                COALESCE(SUM(COALESCE(trade_pnl, 0.0)), 0.0) AS return_sum,
                CAST(SUM(CASE WHEN trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS return_count,
                COALESCE(SUM(CASE WHEN trade_result_value IN (1, 2) THEN COALESCE(trade_pnl, 0.0) ELSE 0.0 END), 0.0) AS expectancy_sum,
                CAST(SUM(CASE WHEN trade_result_value IN (1, 2) AND trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS expectancy_count,
                COALESCE(SUM(CASE WHEN trade_result_value = 1 THEN COALESCE(trade_pnl, 0.0) ELSE 0.0 END), 0.0) AS win_return_sum,
                CAST(SUM(CASE WHEN trade_result_value = 1 AND trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS win_return_count,
                COALESCE(SUM(CASE WHEN trade_result_value = 2 THEN COALESCE(trade_pnl, 0.0) ELSE 0.0 END), 0.0) AS loss_return_sum,
                CAST(SUM(CASE WHEN trade_result_value = 2 AND trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS loss_return_count
            FROM (
                SELECT
                    s.market AS market,
                    COALESCE(sw.trade_result, 0) AS trade_result_value,
                    CAST(sw.trade_pnl AS DOUBLE) AS trade_pnl,
                    hs.harmonic_type AS harmonic_type,
                    CASE
                        WHEN hs.price_accuracy <= 10 THEN '0-10'
                        WHEN hs.price_accuracy <= 20 THEN '10-20'
                        WHEN hs.price_accuracy <= 30 THEN '20-30'
                        WHEN hs.price_accuracy <= 40 THEN '30-40'
                        WHEN hs.price_accuracy <= 50 THEN '40-50'
                        WHEN hs.price_accuracy <= 60 THEN '50-60'
                        WHEN hs.price_accuracy <= 70 THEN '60-70'
                        WHEN hs.price_accuracy <= 80 THEN '70-80'
                        WHEN hs.price_accuracy <= 90 THEN '80-90'
                        ELSE '90-100'
                    END AS bin,
                    {size_bucket_expr} AS size_bucket,
                    {balance_bucket_expr} AS balance_bucket
                FROM pattern_setups s
                INNER JOIN pattern_harmonic_scores hs
                    ON hs.setup_id = s.setup_id
                LEFT JOIN pattern_outcomes_swing sw
                    ON sw.setup_id = s.setup_id
                WHERE hs.price_accuracy IS NOT NULL
            ) source_rows
            WHERE bin IS NOT NULL
            GROUP BY market, trade_result_value, harmonic_type, bin, size_bucket, balance_bucket
            "#,
        );
        sqlx::query(&accuracy_structure_all_patterns_sql)
            .execute(&self.pool)
            .await?;

        let accuracy_structure_dominant_sql = format!(
            r#"
            INSERT INTO accuracy_structure_rollup (
                source_scope,
                market,
                trade_result_value,
                harmonic_type,
                bin,
                size_bucket,
                balance_bucket,
                total_count,
                closed_count,
                open_count,
                win_count,
                loss_count,
                return_sum,
                return_count,
                expectancy_sum,
                expectancy_count,
                win_return_sum,
                win_return_count,
                loss_return_sum,
                loss_return_count
            )
            SELECT
                'all_patterns' AS source_scope,
                market,
                trade_result_value,
                harmonic_type,
                bin,
                size_bucket,
                balance_bucket,
                CAST(COUNT(*) AS SIGNED) AS total_count,
                CAST(SUM(CASE WHEN trade_result_value IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
                CAST(SUM(CASE WHEN trade_result_value NOT IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS open_count,
                CAST(SUM(CASE WHEN trade_result_value = 1 THEN 1 ELSE 0 END) AS SIGNED) AS win_count,
                CAST(SUM(CASE WHEN trade_result_value = 2 THEN 1 ELSE 0 END) AS SIGNED) AS loss_count,
                COALESCE(SUM(COALESCE(trade_pnl, 0.0)), 0.0) AS return_sum,
                CAST(SUM(CASE WHEN trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS return_count,
                COALESCE(SUM(CASE WHEN trade_result_value IN (1, 2) THEN COALESCE(trade_pnl, 0.0) ELSE 0.0 END), 0.0) AS expectancy_sum,
                CAST(SUM(CASE WHEN trade_result_value IN (1, 2) AND trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS expectancy_count,
                COALESCE(SUM(CASE WHEN trade_result_value = 1 THEN COALESCE(trade_pnl, 0.0) ELSE 0.0 END), 0.0) AS win_return_sum,
                CAST(SUM(CASE WHEN trade_result_value = 1 AND trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS win_return_count,
                COALESCE(SUM(CASE WHEN trade_result_value = 2 THEN COALESCE(trade_pnl, 0.0) ELSE 0.0 END), 0.0) AS loss_return_sum,
                CAST(SUM(CASE WHEN trade_result_value = 2 AND trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS loss_return_count
            FROM (
                SELECT
                    s.market AS market,
                    COALESCE(sw.trade_result, 0) AS trade_result_value,
                    'Multi' AS harmonic_type,
                    COALESCE(NULLIF(sw.bin, ''), 'Multi') AS bin,
                    {size_bucket_expr} AS size_bucket,
                    {balance_bucket_expr} AS balance_bucket,
                    CAST(sw.trade_pnl AS DOUBLE) AS trade_pnl
                FROM pattern_setups s
                LEFT JOIN pattern_outcomes_swing sw
                    ON sw.setup_id = s.setup_id
                WHERE 1 = 1
            ) source_rows
            WHERE bin IS NOT NULL
            GROUP BY market, trade_result_value, harmonic_type, bin, size_bucket, balance_bucket
            "#,
        );
        sqlx::query(&accuracy_structure_dominant_sql)
            .execute(&self.pool)
            .await?;

        let strategy_yearly_sql = format!(
            r#"
            INSERT INTO swing_strategy_yearly (
                trade_year,
                swing_strategy_id,
                market,
                harmonic_type,
                bin,
                reversal_type,
                size_bucket,
                time_bin,
                x_strictness,
                three_month_trend,
                six_month_trend,
                twelve_month_trend,
                total_count,
                closed_count,
                open_count,
                win_count,
                loss_count,
                return_sum,
                return_count,
                expectancy_sum,
                expectancy_count,
                win_return_sum,
                win_return_count,
                loss_return_sum,
                loss_return_count,
                trade_length_sum,
                trade_length_count,
                ab_xa_sum,
                ab_xa_count,
                bc_ab_sum,
                bc_ab_count,
                cd_bc_sum,
                cd_bc_count,
                cd_xa_sum,
                cd_xa_count
            )
            SELECT
                trade_year,
                swing_strategy_id,
                market,
                harmonic_type,
                bin,
                reversal_type,
                size_bucket,
                time_bin,
                x_strictness,
                three_month_trend,
                six_month_trend,
                twelve_month_trend,
                CAST(COUNT(*) AS SIGNED) AS total_count,
                CAST(SUM(CASE WHEN trade_result_value IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
                CAST(SUM(CASE WHEN trade_result_value NOT IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS open_count,
                CAST(SUM(CASE WHEN trade_result_value = 1 THEN 1 ELSE 0 END) AS SIGNED) AS win_count,
                CAST(SUM(CASE WHEN trade_result_value = 2 THEN 1 ELSE 0 END) AS SIGNED) AS loss_count,
                COALESCE(SUM(COALESCE(trade_pnl, 0.0)), 0.0) AS return_sum,
                CAST(SUM(CASE WHEN trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS return_count,
                COALESCE(SUM(CASE WHEN trade_result_value IN (1, 2) THEN COALESCE(trade_pnl, 0.0) ELSE 0.0 END), 0.0) AS expectancy_sum,
                CAST(SUM(CASE WHEN trade_result_value IN (1, 2) AND trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS expectancy_count,
                COALESCE(SUM(CASE WHEN trade_result_value = 1 THEN COALESCE(trade_pnl, 0.0) ELSE 0.0 END), 0.0) AS win_return_sum,
                CAST(SUM(CASE WHEN trade_result_value = 1 AND trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS win_return_count,
                COALESCE(SUM(CASE WHEN trade_result_value = 2 THEN COALESCE(trade_pnl, 0.0) ELSE 0.0 END), 0.0) AS loss_return_sum,
                CAST(SUM(CASE WHEN trade_result_value = 2 AND trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS loss_return_count,
                COALESCE(SUM(COALESCE(trade_length, 0.0)), 0.0) AS trade_length_sum,
                CAST(SUM(CASE WHEN trade_length IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS trade_length_count,
                COALESCE(SUM(COALESCE(trade_ab_price_retracement, 0.0)), 0.0) AS ab_xa_sum,
                CAST(SUM(CASE WHEN trade_ab_price_retracement IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS ab_xa_count,
                COALESCE(SUM(COALESCE(trade_bc_price_retracement, 0.0)), 0.0) AS bc_ab_sum,
                CAST(SUM(CASE WHEN trade_bc_price_retracement IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS bc_ab_count,
                COALESCE(SUM(COALESCE(trade_cd_bc_price_retracement, 0.0)), 0.0) AS cd_bc_sum,
                CAST(SUM(CASE WHEN trade_cd_bc_price_retracement IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS cd_bc_count,
                COALESCE(SUM(COALESCE(trade_cd_xa_price_retracement, 0.0)), 0.0) AS cd_xa_sum,
                CAST(SUM(CASE WHEN trade_cd_xa_price_retracement IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS cd_xa_count
            FROM (
                SELECT
                    CAST(sw.trade_year AS SIGNED) AS trade_year,
                    sw.swing_strategy_id AS swing_strategy_id,
                    s.market AS market,
                    COALESCE(sw.trade_result, 0) AS trade_result_value,
                    COALESCE(NULLIF(sw.harmonic_type, ''), 'Multi') AS harmonic_type,
                    COALESCE(NULLIF(sw.bin, ''), 'Multi') AS bin,
                    COALESCE(NULLIF(sw.reversal_type, ''), 'None') AS reversal_type,
                    {size_bucket_expr} AS size_bucket,
                    COALESCE(NULLIF(sw.time_bin, ''), 'Multi') AS time_bin,
                    {three_month_trend_expr} AS three_month_trend,
                    {six_month_trend_expr} AS six_month_trend,
                    {twelve_month_trend_expr} AS twelve_month_trend,
                    CAST(sw.trade_pnl AS DOUBLE) AS trade_pnl,
                    CAST(sw.trade_length AS DOUBLE) AS trade_length,
                    CAST(sw.trade_ab_price_retracement AS DOUBLE) AS trade_ab_price_retracement,
                    CAST(sw.trade_bc_price_retracement AS DOUBLE) AS trade_bc_price_retracement,
                    CAST(sw.trade_cd_bc_price_retracement AS DOUBLE) AS trade_cd_bc_price_retracement,
                    CAST(sw.trade_cd_xa_price_retracement AS DOUBLE) AS trade_cd_xa_price_retracement
                FROM pattern_setups s
                LEFT JOIN pattern_outcomes_swing sw
                    ON sw.setup_id = s.setup_id
                WHERE 1 = 1
            ) source_rows
            WHERE bin IS NOT NULL
              AND swing_strategy_id IS NOT NULL
              AND size_bucket IS NOT NULL
              AND time_bin IS NOT NULL
            GROUP BY
                trade_year,
                swing_strategy_id,
                market,
                harmonic_type,
                bin,
                reversal_type,
                size_bucket,
                time_bin,
                x_strictness,
                three_month_trend,
                six_month_trend,
                twelve_month_trend
            "#,
        );
        sqlx::query(&strategy_yearly_sql)
            .execute(&self.pool)
            .await?;

        self.refresh_swing_strategy_rollups().await?;

        self.set_dashboard_cache_state("structure_rollups", true, None, Some("ready"))
            .await?;

        Ok(())
    }

    pub async fn refresh_swing_strategy_rollups(&self) -> Result<(), sqlx::Error> {
        self.ensure_dashboard_cache_state_table().await?;
        self.ensure_swing_strategy_yearly_table().await?;
        self.ensure_swing_strategy_summary_table().await?;

        self.set_dashboard_cache_state("swing_strategy_rollups", false, None, Some("refreshing"))
            .await?;

        sqlx::query("TRUNCATE TABLE swing_strategy_summary")
            .execute(&self.pool)
            .await?;

        let strategy_cohort_summary_sql = r#"
            INSERT INTO swing_strategy_summary (
                swing_strategy_id,
                market,
                harmonic_type,
                bin,
                reversal_type,
                size_bucket,
                time_bin,
                x_strictness,
                three_month_trend,
                six_month_trend,
                twelve_month_trend,
                worst_year_expectancy,
                down_years,
                total_count,
                closed_count,
                open_count,
                win_count,
                loss_count,
                expectancy,
                avg_return,
                win_rate,
                closed_rate,
                avg_win,
                avg_loss,
                avg_trade_length,
                avg_ab_xa,
                avg_bc_ab,
                avg_cd_bc,
                avg_cd_xa
            )
            SELECT
                swing_strategy_id,
                market,
                harmonic_type,
                bin,
                reversal_type,
                size_bucket,
                time_bin,
                x_strictness,
                three_month_trend,
                six_month_trend,
                twelve_month_trend,
                COALESCE(
                    MIN(
                        CASE
                            WHEN expectancy_count > 0 THEN expectancy_sum / expectancy_count
                            ELSE NULL
                        END
                    ),
                    0.0
                ) AS worst_year_expectancy,
                CAST(
                    COALESCE(
                        SUM(
                            CASE
                                WHEN expectancy_count > 0
                                    AND (expectancy_sum / expectancy_count) <= 0 THEN 1
                                ELSE 0
                            END
                        ),
                        0
                    ) AS SIGNED
                ) AS down_years,
                CAST(COALESCE(SUM(total_count), 0) AS SIGNED) AS total_count,
                CAST(COALESCE(SUM(closed_count), 0) AS SIGNED) AS closed_count,
                CAST(COALESCE(SUM(open_count), 0) AS SIGNED) AS open_count,
                CAST(COALESCE(SUM(win_count), 0) AS SIGNED) AS win_count,
                CAST(COALESCE(SUM(loss_count), 0) AS SIGNED) AS loss_count,
                COALESCE(
                    CAST(COALESCE(SUM(expectancy_sum), 0) / NULLIF(COALESCE(SUM(expectancy_count), 0), 0) AS DOUBLE),
                    0.0
                ) AS expectancy,
                COALESCE(
                    CAST(COALESCE(SUM(return_sum), 0) / NULLIF(COALESCE(SUM(return_count), 0), 0) AS DOUBLE),
                    0.0
                ) AS avg_return,
                COALESCE(
                    CAST(COALESCE(SUM(win_count), 0) / NULLIF(COALESCE(SUM(closed_count), 0), 0) AS DOUBLE),
                    0.0
                ) AS win_rate,
                COALESCE(
                    CAST(COALESCE(SUM(closed_count), 0) / NULLIF(COALESCE(SUM(total_count), 0), 0) AS DOUBLE),
                    0.0
                ) AS closed_rate,
                COALESCE(
                    CAST(COALESCE(SUM(win_return_sum), 0) / NULLIF(COALESCE(SUM(win_return_count), 0), 0) AS DOUBLE),
                    0.0
                ) AS avg_win,
                COALESCE(
                    CAST(COALESCE(SUM(loss_return_sum), 0) / NULLIF(COALESCE(SUM(loss_return_count), 0), 0) AS DOUBLE),
                    0.0
                ) AS avg_loss,
                COALESCE(
                    CAST(COALESCE(SUM(trade_length_sum), 0) / NULLIF(COALESCE(SUM(trade_length_count), 0), 0) AS DOUBLE),
                    0.0
                ) AS avg_trade_length,
                COALESCE(
                    CAST(COALESCE(SUM(ab_xa_sum), 0) / NULLIF(COALESCE(SUM(ab_xa_count), 0), 0) AS DOUBLE),
                    0.0
                ) AS avg_ab_xa,
                COALESCE(
                    CAST(COALESCE(SUM(bc_ab_sum), 0) / NULLIF(COALESCE(SUM(bc_ab_count), 0), 0) AS DOUBLE),
                    0.0
                ) AS avg_bc_ab,
                COALESCE(
                    CAST(COALESCE(SUM(cd_bc_sum), 0) / NULLIF(COALESCE(SUM(cd_bc_count), 0), 0) AS DOUBLE),
                    0.0
                ) AS avg_cd_bc,
                COALESCE(
                    CAST(COALESCE(SUM(cd_xa_sum), 0) / NULLIF(COALESCE(SUM(cd_xa_count), 0), 0) AS DOUBLE),
                    0.0
                ) AS avg_cd_xa
            FROM swing_strategy_yearly
            GROUP BY
                swing_strategy_id,
                market,
                harmonic_type,
                bin,
                reversal_type,
                size_bucket,
                time_bin,
                three_month_trend,
                six_month_trend,
                twelve_month_trend
        "#;
        sqlx::query(strategy_cohort_summary_sql)
            .execute(&self.pool)
            .await?;

        self.set_dashboard_cache_state("swing_strategy_rollups", true, None, Some("ready"))
            .await?;

        Ok(())
    }

    pub async fn refresh_prop_strategy_family_rollups(
        &self,
        run_id: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let total_started = Instant::now();

        let phase_started = Instant::now();
        self.ensure_dashboard_cache_state_table().await?;
        self.ensure_pattern_mode_tables().await?;
        self.ensure_prop_strategy_family_yearly_table().await?;
        self.ensure_prop_strategy_family_summary_table().await?;
        self.record_optional_engine_phase_timing(
            run_id,
            "refresh_prop_family_prepare_tables",
            None,
            phase_started.elapsed(),
            None,
        )
        .await?;

        let phase_started = Instant::now();
        self.set_dashboard_cache_state(
            "prop_strategy_family_rollups",
            false,
            None,
            Some("refreshing"),
        )
        .await?;

        sqlx::query("TRUNCATE TABLE prop_strategy_family_yearly")
            .execute(&self.pool)
            .await?;
        sqlx::query("TRUNCATE TABLE prop_strategy_family_summary")
            .execute(&self.pool)
            .await?;
        self.record_optional_engine_phase_timing(
            run_id,
            "refresh_prop_family_truncate",
            None,
            phase_started.elapsed(),
            Some("summary and yearly tables cleared"),
        )
        .await?;

        let mut conn = self.pool.acquire().await?;

        let phase_started = Instant::now();
        sqlx::query("DROP TEMPORARY TABLE IF EXISTS prop_strategy_family_source")
            .execute(&mut *conn)
            .await?;

        sqlx::query(
            r#"
            CREATE TEMPORARY TABLE prop_strategy_family_source (
                outcome_model VARCHAR(32) NOT NULL,
                reversal_type VARCHAR(64) NOT NULL,
                trade_year INT NOT NULL,
                market VARCHAR(16) NOT NULL,
                harmonic_type VARCHAR(32) NOT NULL,
                bin VARCHAR(16) NOT NULL,
                size_bucket VARCHAR(16) NOT NULL,
                time_bin VARCHAR(16) NOT NULL,
                x_strictness VARCHAR(16) NOT NULL,
                three_month_trend VARCHAR(16) NOT NULL,
                six_month_trend VARCHAR(16) NOT NULL,
                twelve_month_trend VARCHAR(16) NOT NULL,
                total_count BIGINT NOT NULL,
                closed_count BIGINT NOT NULL,
                open_count BIGINT NOT NULL,
                win_count BIGINT NOT NULL,
                loss_count BIGINT NOT NULL,
                return_sum DOUBLE NOT NULL,
                return_count BIGINT NOT NULL,
                expectancy_sum DOUBLE NOT NULL,
                expectancy_count BIGINT NOT NULL,
                win_return_sum DOUBLE NOT NULL,
                win_return_count BIGINT NOT NULL,
                loss_return_sum DOUBLE NOT NULL,
                loss_return_count BIGINT NOT NULL,
                target_range_sum DOUBLE NOT NULL,
                target_range_count BIGINT NOT NULL
            )
            "#,
        )
        .execute(&mut *conn)
        .await?;

        let (min_source_id, max_source_id) = sqlx::query_as::<_, (Option<i64>, Option<i64>)>(
            r#"
            SELECT
                CAST(MIN(id) AS SIGNED),
                CAST(MAX(id) AS SIGNED)
            FROM pattern_outcomes_prop
            WHERE harmonic_type IS NOT NULL
              AND bin IS NOT NULL
              AND size_bucket IS NOT NULL
              AND time_bin IS NOT NULL
              AND COALESCE(target_date, entry_date) IS NOT NULL
            "#,
        )
        .fetch_one(&mut *conn)
        .await?;

        let insert_source_sql = format!(
            r#"
            INSERT INTO prop_strategy_family_source (
                outcome_model,
                reversal_type,
                trade_year,
                market,
                harmonic_type,
                bin,
                size_bucket,
                time_bin,
                x_strictness,
                three_month_trend,
                six_month_trend,
                twelve_month_trend,
                total_count,
                closed_count,
                open_count,
                win_count,
                loss_count,
                return_sum,
                return_count,
                expectancy_sum,
                expectancy_count,
                win_return_sum,
                win_return_count,
                loss_return_sum,
                loss_return_count,
                target_range_sum,
                target_range_count
            )
            SELECT
                outcome_model,
                reversal_type,
                trade_year,
                market,
                harmonic_type,
                bin,
                size_bucket,
                time_bin,
                x_strictness,
                three_month_trend,
                six_month_trend,
                twelve_month_trend,
                CAST(COUNT(*) AS SIGNED) AS total_count,
                CAST(SUM(CASE WHEN trade_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
                CAST(SUM(CASE WHEN trade_result NOT IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS open_count,
                CAST(SUM(CASE WHEN trade_result = 1 THEN 1 ELSE 0 END) AS SIGNED) AS win_count,
                CAST(SUM(CASE WHEN trade_result = 2 THEN 1 ELSE 0 END) AS SIGNED) AS loss_count,
                COALESCE(SUM(COALESCE(directional_return_pct, 0.0)), 0.0) AS return_sum,
                CAST(SUM(CASE WHEN directional_return_pct IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS return_count,
                COALESCE(SUM(CASE WHEN trade_result IN (1, 2) THEN COALESCE(directional_return_pct, 0.0) ELSE 0.0 END), 0.0) AS expectancy_sum,
                CAST(SUM(CASE WHEN trade_result IN (1, 2) AND directional_return_pct IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS expectancy_count,
                COALESCE(SUM(CASE WHEN trade_result = 1 THEN COALESCE(directional_return_pct, 0.0) ELSE 0.0 END), 0.0) AS win_return_sum,
                CAST(SUM(CASE WHEN trade_result = 1 AND directional_return_pct IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS win_return_count,
                COALESCE(SUM(CASE WHEN trade_result = 2 THEN COALESCE(directional_return_pct, 0.0) ELSE 0.0 END), 0.0) AS loss_return_sum,
                CAST(SUM(CASE WHEN trade_result = 2 AND directional_return_pct IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS loss_return_count,
                COALESCE(SUM(COALESCE(target_range_pct, 0.0)), 0.0) AS target_range_sum,
                CAST(SUM(CASE WHEN target_range_pct IS NOT NULL THEN 1 ELSE 0 END) AS SIGNED) AS target_range_count
            FROM (
                SELECT
                    p.outcome_model,
                    COALESCE(NULLIF(p.reversal_type, ''), 'None') AS reversal_type,
                    CAST(YEAR(COALESCE(p.target_date, p.entry_date)) AS SIGNED) AS trade_year,
                    p.market,
                    p.harmonic_type,
                    p.bin,
                    p.size_bucket,
                    p.time_bin,
                    {x_strictness_expr} AS x_strictness,
                    p.three_month_trend,
                    p.six_month_trend,
                    p.twelve_month_trend,
                    p.prop_result AS trade_result,
                    CASE
                        WHEN p.target_close_vs_open_pct IS NULL THEN NULL
                        WHEN p.market = 'Bearish' THEN -CAST(p.target_close_vs_open_pct AS DOUBLE)
                        ELSE CAST(p.target_close_vs_open_pct AS DOUBLE)
                    END AS directional_return_pct,
                    CAST(p.target_range_pct AS DOUBLE) AS target_range_pct
                FROM pattern_outcomes_prop p
                WHERE p.harmonic_type IS NOT NULL
                  AND p.bin IS NOT NULL
                  AND p.size_bucket IS NOT NULL
                  AND p.time_bin IS NOT NULL
                  AND COALESCE(p.target_date, p.entry_date) IS NOT NULL
                  AND p.id BETWEEN ? AND ?
            ) source
            GROUP BY
                outcome_model,
                reversal_type,
                trade_year,
                market,
                harmonic_type,
                bin,
                size_bucket,
                time_bin,
                x_strictness,
                three_month_trend,
                six_month_trend,
                twelve_month_trend
            "#,
            x_strictness_expr = x_strictness_expr("p.x_bars_left", "p.x_length")
        );
        let mut rows_written = 0_i64;
        if let (Some(min_id), Some(max_id)) = (min_source_id, max_source_id) {
            let source_chunk_size = 25_000_i64;
            let mut chunk_start = min_id;
            while chunk_start <= max_id {
                let chunk_end = (chunk_start + source_chunk_size - 1).min(max_id);
                let result = sqlx::query(&insert_source_sql)
                    .bind(chunk_start)
                    .bind(chunk_end)
                    .execute(&mut *conn)
                    .await?;
                rows_written += result.rows_affected() as i64;
                chunk_start = chunk_end + 1;
            }
        }
        self.record_optional_engine_phase_timing(
            run_id,
            "refresh_prop_family_source",
            Some(rows_written),
            phase_started.elapsed(),
            Some(
                "temporary detailed aggregate built from pattern_outcomes_prop in id-range chunks",
            ),
        )
        .await?;

        let phase_started = Instant::now();
        for index_sql in [
            "CREATE INDEX idx_prop_family_source_core ON prop_strategy_family_source (market, harmonic_type, bin)",
            "CREATE INDEX idx_prop_family_source_outcome ON prop_strategy_family_source (outcome_model, reversal_type)",
            "CREATE INDEX idx_prop_family_source_year ON prop_strategy_family_source (trade_year)",
        ] {
            sqlx::query(index_sql).execute(&mut *conn).await?;
        }
        self.record_optional_engine_phase_timing(
            run_id,
            "refresh_prop_family_source_indexes",
            Some(3),
            phase_started.elapsed(),
            Some("temporary source indexes"),
        )
        .await?;

        let trade_years = sqlx::query_scalar::<_, i64>(
            "SELECT DISTINCT trade_year FROM prop_strategy_family_source ORDER BY trade_year ASC",
        )
        .fetch_all(&mut *conn)
        .await?;

        for spec in PROP_FAMILY_SPECS.iter() {
            let phase_started = Instant::now();
            let included_dimensions = prop_family_included_dimensions(spec);
            let dimension_exprs = PROP_FAMILY_DIMENSIONS
                .iter()
                .map(|dimension| prop_family_dimension_expr(spec, dimension))
                .collect::<Vec<_>>();
            let dimension_selects = PROP_FAMILY_DIMENSIONS
                .iter()
                .zip(dimension_exprs.iter())
                .map(|(dimension, expression)| format!("{expression} AS {dimension}"))
                .collect::<Vec<_>>()
                .join(",\n                ");
            let dimension_group_by = dimension_exprs.join(",\n                ");
            let family_key_values = dimension_exprs
                .iter()
                .map(|expression| format!("LOWER({expression})"))
                .collect::<Vec<_>>()
                .join(", ");

            let family_yearly_sql = format!(
                r#"
                INSERT INTO prop_strategy_family_yearly (
                    trade_year,
                    family_key,
                    family_name,
                    family_level,
                    included_dimensions,
                    outcome_model,
                    market,
                    harmonic_type,
                    bin,
                    reversal_type,
                    size_bucket,
                    time_bin,
                    x_strictness,
                    three_month_trend,
                    six_month_trend,
                    twelve_month_trend,
                    total_count,
                    closed_count,
                    open_count,
                    win_count,
                    loss_count,
                    return_sum,
                    return_count,
                    expectancy_sum,
                    expectancy_count,
                    win_return_sum,
                    win_return_count,
                    loss_return_sum,
                    loss_return_count,
                    target_range_sum,
                    target_range_count
                )
                SELECT
                    trade_year,
                    LEFT(MD5(CONCAT_WS('|', 'prop-family-v3', '{family_name}', {family_key_values})), 16) AS family_key,
                    '{family_name}' AS family_name,
                    {family_level} AS family_level,
                    '{included_dimensions}' AS included_dimensions,
                    {dimension_selects},
                    CAST(COALESCE(SUM(total_count), 0) AS SIGNED) AS total_count,
                    CAST(COALESCE(SUM(closed_count), 0) AS SIGNED) AS closed_count,
                    CAST(COALESCE(SUM(open_count), 0) AS SIGNED) AS open_count,
                    CAST(COALESCE(SUM(win_count), 0) AS SIGNED) AS win_count,
                    CAST(COALESCE(SUM(loss_count), 0) AS SIGNED) AS loss_count,
                    COALESCE(SUM(return_sum), 0.0) AS return_sum,
                    CAST(COALESCE(SUM(return_count), 0) AS SIGNED) AS return_count,
                    COALESCE(SUM(expectancy_sum), 0.0) AS expectancy_sum,
                    CAST(COALESCE(SUM(expectancy_count), 0) AS SIGNED) AS expectancy_count,
                    COALESCE(SUM(win_return_sum), 0.0) AS win_return_sum,
                    CAST(COALESCE(SUM(win_return_count), 0) AS SIGNED) AS win_return_count,
                    COALESCE(SUM(loss_return_sum), 0.0) AS loss_return_sum,
                    CAST(COALESCE(SUM(loss_return_count), 0) AS SIGNED) AS loss_return_count,
                    COALESCE(SUM(target_range_sum), 0.0) AS target_range_sum,
                    CAST(COALESCE(SUM(target_range_count), 0) AS SIGNED) AS target_range_count
                FROM prop_strategy_family_source
                WHERE trade_year = ?
                GROUP BY
                    trade_year,
                    {dimension_group_by}
                "#,
                family_name = spec.family_name,
                family_level = spec.dimensions.len(),
                included_dimensions = included_dimensions,
                family_key_values = family_key_values,
                dimension_selects = dimension_selects,
                dimension_group_by = dimension_group_by,
            );
            let mut rows_written = 0_i64;
            for trade_year in trade_years.iter() {
                let result = sqlx::query(&family_yearly_sql)
                    .bind(*trade_year)
                    .execute(&mut *conn)
                    .await?;
                rows_written += result.rows_affected() as i64;
            }
            self.record_optional_engine_phase_timing(
                run_id,
                "refresh_prop_family_yearly_template",
                Some(rows_written),
                phase_started.elapsed(),
                Some("yearly rows inserted in trade_year chunks"),
            )
            .await?;
        }

        // Release the connection to commit the yearly table inserts and free locks
        // before reading from it in the summary insert. This prevents lock table overflow.
        drop(conn);

        let mut conn = self.pool.acquire().await?;

        let outcome_models = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT outcome_model FROM prop_strategy_family_yearly ORDER BY outcome_model ASC",
        )
        .fetch_all(&mut *conn)
        .await?;

        let phase_started = Instant::now();
        let summary_sql = r#"
            INSERT INTO prop_strategy_family_summary (
                family_key,
                family_name,
                family_level,
                included_dimensions,
                outcome_model,
                market,
                harmonic_type,
                bin,
                reversal_type,
                size_bucket,
                time_bin,
                x_strictness,
                three_month_trend,
                six_month_trend,
                twelve_month_trend,
                worst_year_expectancy,
                down_years,
                total_count,
                closed_count,
                open_count,
                win_count,
                loss_count,
                expectancy,
                avg_return,
                win_rate,
                closed_rate,
                avg_win,
                avg_loss,
                avg_target_range,
                score
            )
            SELECT
                agg.family_key,
                agg.family_name,
                agg.family_level,
                agg.included_dimensions,
                agg.outcome_model,
                agg.market,
                agg.harmonic_type,
                agg.bin,
                agg.reversal_type,
                agg.size_bucket,
                agg.time_bin,
                agg.x_strictness,
                agg.three_month_trend,
                agg.six_month_trend,
                agg.twelve_month_trend,
                agg.worst_year_expectancy,
                agg.down_years,
                agg.total_count,
                agg.closed_count,
                agg.open_count,
                agg.win_count,
                agg.loss_count,
                agg.expectancy,
                agg.avg_return,
                agg.win_rate,
                agg.closed_rate,
                agg.avg_win,
                agg.avg_loss,
                agg.avg_target_range,
                agg.expectancy
                    * LEAST(1.0, SQRT(CAST(agg.closed_count AS DOUBLE) / 100.0))
                    * CASE
                        WHEN agg.down_years <= 0 THEN 1.0
                        ELSE 1.0 / (1.0 + CAST(agg.down_years AS DOUBLE))
                    END AS score
            FROM (
                SELECT
                    family_key,
                    family_name,
                    family_level,
                    included_dimensions,
                    outcome_model,
                    market,
                    harmonic_type,
                    bin,
                    reversal_type,
                    size_bucket,
                    time_bin,
                    x_strictness,
                    three_month_trend,
                    six_month_trend,
                    twelve_month_trend,
                    COALESCE(
                        MIN(
                            CASE
                                WHEN expectancy_count > 0 THEN expectancy_sum / expectancy_count
                                ELSE NULL
                            END
                        ),
                        0.0
                    ) AS worst_year_expectancy,
                    CAST(
                        COALESCE(
                            SUM(
                                CASE
                                    WHEN expectancy_count > 0
                                        AND (expectancy_sum / expectancy_count) <= 0 THEN 1
                                    ELSE 0
                                END
                            ),
                            0
                        ) AS SIGNED
                    ) AS down_years,
                    CAST(COALESCE(SUM(total_count), 0) AS SIGNED) AS total_count,
                    CAST(COALESCE(SUM(closed_count), 0) AS SIGNED) AS closed_count,
                    CAST(COALESCE(SUM(open_count), 0) AS SIGNED) AS open_count,
                    CAST(COALESCE(SUM(win_count), 0) AS SIGNED) AS win_count,
                    CAST(COALESCE(SUM(loss_count), 0) AS SIGNED) AS loss_count,
                    COALESCE(
                        CAST(COALESCE(SUM(expectancy_sum), 0) / NULLIF(COALESCE(SUM(expectancy_count), 0), 0) AS DOUBLE),
                        0.0
                    ) AS expectancy,
                    COALESCE(
                        CAST(COALESCE(SUM(return_sum), 0) / NULLIF(COALESCE(SUM(return_count), 0), 0) AS DOUBLE),
                        0.0
                    ) AS avg_return,
                    COALESCE(
                        CAST(COALESCE(SUM(win_count), 0) / NULLIF(COALESCE(SUM(closed_count), 0), 0) AS DOUBLE),
                        0.0
                    ) AS win_rate,
                    COALESCE(
                        CAST(COALESCE(SUM(closed_count), 0) / NULLIF(COALESCE(SUM(total_count), 0), 0) AS DOUBLE),
                        0.0
                    ) AS closed_rate,
                    COALESCE(
                        CAST(COALESCE(SUM(win_return_sum), 0) / NULLIF(COALESCE(SUM(win_return_count), 0), 0) AS DOUBLE),
                        0.0
                    ) AS avg_win,
                    COALESCE(
                        CAST(COALESCE(SUM(loss_return_sum), 0) / NULLIF(COALESCE(SUM(loss_return_count), 0), 0) AS DOUBLE),
                        0.0
                    ) AS avg_loss,
                    COALESCE(
                        CAST(COALESCE(SUM(target_range_sum), 0) / NULLIF(COALESCE(SUM(target_range_count), 0), 0) AS DOUBLE),
                        0.0
                    ) AS avg_target_range
                FROM prop_strategy_family_yearly
                WHERE outcome_model = ?
                GROUP BY
                    family_key,
                    family_name,
                    family_level,
                    included_dimensions,
                    outcome_model,
                    market,
                    harmonic_type,
                    bin,
                    reversal_type,
                    size_bucket,
                    time_bin,
                    x_strictness,
                    three_month_trend,
                    six_month_trend,
                    twelve_month_trend
            ) agg
            "#;
        let mut rows_written = 0_i64;
        for outcome_model in outcome_models.iter() {
            let result = sqlx::query(summary_sql)
                .bind(outcome_model)
                .execute(&mut *conn)
                .await?;
            rows_written += result.rows_affected() as i64;
        }
        self.record_optional_engine_phase_timing(
            run_id,
            "refresh_prop_family_summary",
            Some(rows_written),
            phase_started.elapsed(),
            Some("summary rows aggregated from yearly rows in outcome_model chunks"),
        )
        .await?;

        let phase_started = Instant::now();
        sqlx::query("DROP TEMPORARY TABLE IF EXISTS prop_strategy_family_source")
            .execute(&mut *conn)
            .await?;

        drop(conn);

        self.set_dashboard_cache_state("prop_strategy_family_rollups", true, None, Some("ready"))
            .await?;
        self.record_optional_engine_phase_timing(
            run_id,
            "refresh_prop_family_cleanup",
            None,
            phase_started.elapsed(),
            Some("temporary aggregate source dropped and cache marked ready"),
        )
        .await?;

        self.record_optional_engine_phase_timing(
            run_id,
            "refresh_prop_strategy_family_rollups",
            None,
            total_started.elapsed(),
            Some("total family rollup refresh"),
        )
        .await?;

        Ok(())
    }

    pub async fn refresh_prop_contract_week_summary(
        &self,
        run_id: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let total_started = Instant::now();
        self.ensure_prop_strategy_contract_week_summary_table()
            .await?;

        sqlx::query("TRUNCATE TABLE prop_strategy_contract_week_summary")
            .execute(&self.pool)
            .await?;

        let phase_started = Instant::now();
        let family_key_expr = concrete_prop_family_key_expr("p");
        let sql = format!(
            r#"
            INSERT INTO prop_strategy_contract_week_summary (
                family_key,
                family_name,
                family_level,
                included_dimensions,
                outcome_model,
                market,
                harmonic_type,
                bin,
                reversal_type,
                size_bucket,
                time_bin,
                x_strictness,
                three_month_trend,
                six_month_trend,
                twelve_month_trend,
                symbol,
                contract_week_index,
                total_count,
                closed_count,
                open_count,
                win_count,
                loss_count,
                expectancy,
                avg_return,
                win_rate
            )
            SELECT
                s.family_key,
                s.family_name,
                s.family_level,
                s.included_dimensions,
                s.outcome_model,
                s.market,
                s.harmonic_type,
                s.bin,
                s.reversal_type,
                s.size_bucket,
                s.time_bin,
                s.x_strictness,
                s.three_month_trend,
                s.six_month_trend,
                s.twelve_month_trend,
                p.symbol,
                CAST(COALESCE(p.contract_week_index, 0) AS SIGNED) AS contract_week_index,
                CAST(COUNT(*) AS SIGNED) AS total_count,
                CAST(SUM(CASE WHEN p.prop_result IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS closed_count,
                CAST(SUM(CASE WHEN p.prop_result NOT IN (1, 2) THEN 1 ELSE 0 END) AS SIGNED) AS open_count,
                CAST(SUM(CASE WHEN p.prop_result = 1 THEN 1 ELSE 0 END) AS SIGNED) AS win_count,
                CAST(SUM(CASE WHEN p.prop_result = 2 THEN 1 ELSE 0 END) AS SIGNED) AS loss_count,
                COALESCE(AVG(
                    CASE
                        WHEN p.prop_result IN (1, 2) AND p.target_close_vs_open_pct IS NOT NULL THEN
                            CASE
                                WHEN p.market = 'Bearish' THEN -CAST(p.target_close_vs_open_pct AS DOUBLE)
                                ELSE CAST(p.target_close_vs_open_pct AS DOUBLE)
                            END
                        ELSE NULL
                    END
                ), 0.0) AS expectancy,
                COALESCE(AVG(
                    CASE
                        WHEN p.target_close_vs_open_pct IS NOT NULL THEN
                            CASE
                                WHEN p.market = 'Bearish' THEN -CAST(p.target_close_vs_open_pct AS DOUBLE)
                                ELSE CAST(p.target_close_vs_open_pct AS DOUBLE)
                            END
                        ELSE NULL
                    END
                ), 0.0) AS avg_return,
                COALESCE(
                    CAST(SUM(CASE WHEN p.prop_result = 1 THEN 1 ELSE 0 END) AS DOUBLE)
                    / NULLIF(CAST(SUM(CASE WHEN p.prop_result IN (1, 2) THEN 1 ELSE 0 END) AS DOUBLE), 0.0),
                    0.0
                ) AS win_rate
            FROM pattern_outcomes_prop p
            INNER JOIN prop_strategy_family_summary s
                ON s.family_key = {family_key_expr}
            WHERE p.contract_week_index IS NOT NULL
              AND p.harmonic_type IS NOT NULL
              AND p.bin IS NOT NULL
              AND p.size_bucket IS NOT NULL
              AND p.time_bin IS NOT NULL
            GROUP BY
                s.family_key,
                s.family_name,
                s.family_level,
                s.included_dimensions,
                s.outcome_model,
                s.market,
                s.harmonic_type,
                s.bin,
                s.reversal_type,
                s.size_bucket,
                s.time_bin,
                s.x_strictness,
                s.three_month_trend,
                s.six_month_trend,
                s.twelve_month_trend,
                p.symbol,
                p.contract_week_index
            "#,
            family_key_expr = family_key_expr,
        );
        let rows = sqlx::query(&sql).execute(&self.pool).await?.rows_affected() as i64;

        self.record_optional_engine_phase_timing(
            run_id,
            "refresh_prop_contract_week_summary",
            Some(rows),
            phase_started.elapsed(),
            Some("contract week rows aggregated by family and futures contract"),
        )
        .await?;
        self.record_optional_engine_phase_timing(
            run_id,
            "refresh_prop_contract_week_summary_total",
            None,
            total_started.elapsed(),
            Some("total contract week summary refresh"),
        )
        .await?;

        Ok(())
    }

    pub async fn refresh_prop_family_weekly_cadence(
        &self,
        run_id: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let total_started = Instant::now();
        self.ensure_prop_strategy_family_weekly_cadence_table()
            .await?;

        sqlx::query("TRUNCATE TABLE prop_strategy_family_weekly_cadence")
            .execute(&self.pool)
            .await?;

        let mut conn = self.pool.acquire().await?;
        let family_key_expr = concrete_prop_family_key_expr("p");

        let phase_started = Instant::now();
        sqlx::query("DROP TEMPORARY TABLE IF EXISTS prop_family_weekly_counts")
            .execute(&mut *conn)
            .await?;
        let weekly_counts_sql = format!(
            r#"
            CREATE TEMPORARY TABLE prop_family_weekly_counts AS
            SELECT
                source.family_key,
                source.week_start,
                CAST(COUNT(*) AS SIGNED) AS setup_count
            FROM (
                SELECT
                    {family_key_expr} AS family_key,
                    DATE_SUB(DATE(p.d_date), INTERVAL WEEKDAY(p.d_date) DAY) AS week_start
                FROM pattern_outcomes_prop p
                WHERE p.d_date IS NOT NULL
                  AND p.harmonic_type IS NOT NULL
                  AND p.bin IS NOT NULL
                  AND p.size_bucket IS NOT NULL
                  AND p.time_bin IS NOT NULL
            ) source
            GROUP BY source.family_key, source.week_start
            "#,
            family_key_expr = family_key_expr,
        );
        sqlx::query(&weekly_counts_sql).execute(&mut *conn).await?;
        sqlx::query(
            "CREATE INDEX idx_prop_family_weekly_counts_family ON prop_family_weekly_counts (family_key, week_start)",
        )
        .execute(&mut *conn)
        .await?;
        self.record_optional_engine_phase_timing(
            run_id,
            "refresh_prop_family_weekly_counts",
            None,
            phase_started.elapsed(),
            Some("temporary weekly family counts built from D dates"),
        )
        .await?;

        let phase_started = Instant::now();
        let insert_sql = r#"
            INSERT INTO prop_strategy_family_weekly_cadence (
                family_key,
                family_name,
                family_level,
                included_dimensions,
                outcome_model,
                market,
                harmonic_type,
                bin,
                reversal_type,
                size_bucket,
                time_bin,
                x_strictness,
                three_month_trend,
                six_month_trend,
                twelve_month_trend,
                total_calendar_weeks,
                active_weeks,
                zero_setup_weeks,
                zero_setup_week_rate,
                total_setups,
                avg_setups_per_week,
                max_setups_per_week,
                first_week_start,
                last_week_start
            )
            SELECT
                s.family_key,
                s.family_name,
                s.family_level,
                s.included_dimensions,
                s.outcome_model,
                s.market,
                s.harmonic_type,
                s.bin,
                s.reversal_type,
                s.size_bucket,
                s.time_bin,
                s.x_strictness,
                s.three_month_trend,
                s.six_month_trend,
                s.twelve_month_trend,
                stats.total_calendar_weeks,
                stats.active_weeks,
                GREATEST(stats.total_calendar_weeks - stats.active_weeks, 0) AS zero_setup_weeks,
                COALESCE(
                    CAST(GREATEST(stats.total_calendar_weeks - stats.active_weeks, 0) AS DOUBLE)
                    / NULLIF(CAST(stats.total_calendar_weeks AS DOUBLE), 0.0),
                    0.0
                ) AS zero_setup_week_rate,
                stats.total_setups,
                COALESCE(
                    CAST(stats.total_setups AS DOUBLE)
                    / NULLIF(CAST(stats.total_calendar_weeks AS DOUBLE), 0.0),
                    0.0
                ) AS avg_setups_per_week,
                stats.max_setups_per_week,
                stats.first_week_start,
                stats.last_week_start
            FROM prop_strategy_family_summary s
            INNER JOIN (
                SELECT
                    family_key,
                    CAST(TIMESTAMPDIFF(WEEK, MIN(week_start), MAX(week_start)) + 1 AS SIGNED) AS total_calendar_weeks,
                    CAST(COUNT(*) AS SIGNED) AS active_weeks,
                    CAST(COALESCE(SUM(setup_count), 0) AS SIGNED) AS total_setups,
                    CAST(COALESCE(MAX(setup_count), 0) AS SIGNED) AS max_setups_per_week,
                    MIN(week_start) AS first_week_start,
                    MAX(week_start) AS last_week_start
                FROM prop_family_weekly_counts
                GROUP BY family_key
            ) stats
                ON stats.family_key = s.family_key
            "#;
        let rows = sqlx::query(insert_sql)
            .execute(&mut *conn)
            .await?
            .rows_affected() as i64;

        sqlx::query("DROP TEMPORARY TABLE IF EXISTS prop_family_weekly_counts")
            .execute(&mut *conn)
            .await?;
        drop(conn);

        self.record_optional_engine_phase_timing(
            run_id,
            "refresh_prop_family_weekly_cadence",
            Some(rows),
            phase_started.elapsed(),
            Some("family weekly cadence rows inserted"),
        )
        .await?;
        self.record_optional_engine_phase_timing(
            run_id,
            "refresh_prop_family_weekly_cadence_total",
            None,
            total_started.elapsed(),
            Some("total weekly cadence refresh"),
        )
        .await?;

        Ok(())
    }

    pub async fn upsert_accuracy_bin_rollup_from_patterns(
        &self,
        patterns: &[PatternXABCD],
    ) -> Result<(), sqlx::Error> {
        if patterns.is_empty() {
            return Ok(());
        }

        self.ensure_accuracy_bin_rollup_table().await?;

        let mut aggregates: HashMap<AccuracyRollupKey, AccuracyBinAccumulator> = HashMap::new();

        for pattern in patterns {
            let market = market_label(pattern.market);
            let trade_result = Some(pattern.trade.result);
            let trade_result_value = pattern.trade.result;
            let trade_pnl = Some(pattern.trade.pnl);
            let rollup_year = pattern.d.date.year();

            for (harmonic_type, accuracy) in [
                ("Bat", pattern.accuracies.bat.pattern_accuracy),
                (
                    "AlternateBat",
                    pattern.accuracies.alternate_bat.pattern_accuracy,
                ),
                ("Butterfly", pattern.accuracies.butterfly.pattern_accuracy),
                ("Gartley", pattern.accuracies.gartley.pattern_accuracy),
                ("Crab", pattern.accuracies.crab.pattern_accuracy),
                ("DeepCrab", pattern.accuracies.deep_crab.pattern_accuracy),
                ("Shark", pattern.accuracies.shark.pattern_accuracy),
            ] {
                update_accuracy_rollup(
                    &mut aggregates,
                    rollup_year,
                    market,
                    trade_result_value,
                    "all_patterns",
                    harmonic_type,
                    accuracy,
                    trade_result,
                    trade_pnl,
                );
            }
        }

        let rollup_rows = finalize_accuracy_rollup_rows(aggregates);
        let mut tx = self.pool.begin().await?;

        for chunk in rollup_rows.chunks(500) {
            let mut builder = QueryBuilder::<MySql>::new(
                r#"
                INSERT INTO accuracy_bin_rollup (
                    rollup_year,
                    market,
                    trade_result_value,
                    source_scope,
                    harmonic_type,
                    bin,
                    total_count,
                    closed_count,
                    open_count,
                    win_count,
                    loss_count,
                    return_sum,
                    return_count,
                    expectancy_sum,
                    expectancy_count,
                    win_return_sum,
                    win_return_count,
                    loss_return_sum,
                    loss_return_count
                )
                "#,
            );

            builder.push_values(chunk, |mut row, item| {
                row.push_bind(item.rollup_year)
                    .push_bind(&item.market)
                    .push_bind(item.trade_result_value)
                    .push_bind(&item.source_scope)
                    .push_bind(&item.harmonic_type)
                    .push_bind(&item.bin)
                    .push_bind(item.total_count)
                    .push_bind(item.closed_count)
                    .push_bind(item.open_count)
                    .push_bind(item.win_count)
                    .push_bind(item.loss_count)
                    .push_bind(item.return_sum)
                    .push_bind(item.return_count)
                    .push_bind(item.expectancy_sum)
                    .push_bind(item.expectancy_count)
                    .push_bind(item.win_return_sum)
                    .push_bind(item.win_return_count)
                    .push_bind(item.loss_return_sum)
                    .push_bind(item.loss_return_count);
            });

            builder.push(
                r#"
                ON DUPLICATE KEY UPDATE
                    total_count = total_count + VALUES(total_count),
                    closed_count = closed_count + VALUES(closed_count),
                    open_count = open_count + VALUES(open_count),
                    win_count = win_count + VALUES(win_count),
                    loss_count = loss_count + VALUES(loss_count),
                    return_sum = return_sum + VALUES(return_sum),
                    return_count = return_count + VALUES(return_count),
                    expectancy_sum = expectancy_sum + VALUES(expectancy_sum),
                    expectancy_count = expectancy_count + VALUES(expectancy_count),
                    win_return_sum = win_return_sum + VALUES(win_return_sum),
                    win_return_count = win_return_count + VALUES(win_return_count),
                    loss_return_sum = loss_return_sum + VALUES(loss_return_sum),
                    loss_return_count = loss_return_count + VALUES(loss_return_count)
                "#,
            );

            builder.build().execute(&mut *tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn backfill_accuracy_bin_rollup(&self) -> Result<(), sqlx::Error> {
        self.ensure_accuracy_bin_rollup_table().await?;
        self.ensure_dashboard_cache_state_table().await?;
        self.ensure_pattern_harmonic_scores_table().await?;

        self.set_dashboard_cache_state("accuracy_bin_rollup", false, None, Some("backfilling"))
            .await?;

        let year_range = sqlx::query_as::<_, (Option<i32>, Option<i32>)>(
            r#"
            SELECT
                MIN(YEAR(d_date)) AS min_year,
                MAX(YEAR(d_date)) AS max_year
            FROM pattern_setups
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        let Some(min_year) = year_range.0 else {
            self.set_dashboard_cache_state("accuracy_bin_rollup", true, None, Some("empty source"))
                .await?;
            return Ok(());
        };
        let Some(max_year) = year_range.1 else {
            self.set_dashboard_cache_state("accuracy_bin_rollup", true, None, Some("empty source"))
                .await?;
            return Ok(());
        };

        for year in min_year..=max_year {
            let start = format!("{year}-01-01");
            let end = format!("{}-01-01", year + 1);

            for market in ["Bullish", "Bearish"] {
                sqlx::query(
                    r#"
                    DELETE FROM accuracy_bin_rollup
                    WHERE rollup_year = ?
                      AND market = ?
                    "#,
                )
                .bind(year)
                .bind(market)
                .execute(&self.pool)
                .await?;

                sqlx::query(
                    r#"
                    INSERT INTO accuracy_bin_rollup (
                        rollup_year,
                        market,
                        trade_result_value,
                        source_scope,
                        harmonic_type,
                        bin,
                        total_count,
                        closed_count,
                        open_count,
                        win_count,
                        loss_count,
                        return_sum,
                        return_count,
                        expectancy_sum,
                        expectancy_count,
                        win_return_sum,
                        win_return_count,
                        loss_return_sum,
                        loss_return_count
                    )
                    SELECT
                        ? AS rollup_year,
                        market,
                        trade_result_value,
                        'all_patterns' AS source_scope,
                        harmonic_type,
                        bin,
                        COUNT(*) AS total_count,
                        SUM(CASE WHEN trade_result_value IN (1, 2) THEN 1 ELSE 0 END) AS closed_count,
                        SUM(CASE WHEN trade_result_value NOT IN (1, 2) THEN 1 ELSE 0 END) AS open_count,
                        SUM(CASE WHEN trade_result_value = 1 THEN 1 ELSE 0 END) AS win_count,
                        SUM(CASE WHEN trade_result_value = 2 THEN 1 ELSE 0 END) AS loss_count,
                        SUM(COALESCE(trade_pnl, 0.0)) AS return_sum,
                        SUM(CASE WHEN trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS return_count,
                        SUM(CASE WHEN trade_result_value IN (1, 2) THEN COALESCE(trade_pnl, 0.0) ELSE 0.0 END) AS expectancy_sum,
                        SUM(CASE WHEN trade_result_value IN (1, 2) AND trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS expectancy_count,
                        SUM(CASE WHEN trade_result_value = 1 THEN COALESCE(trade_pnl, 0.0) ELSE 0.0 END) AS win_return_sum,
                        SUM(CASE WHEN trade_result_value = 1 AND trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS win_return_count,
                        SUM(CASE WHEN trade_result_value = 2 THEN COALESCE(trade_pnl, 0.0) ELSE 0.0 END) AS loss_return_sum,
                        SUM(CASE WHEN trade_result_value = 2 AND trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS loss_return_count
                    FROM (
                        SELECT
                            s.market,
                            COALESCE(sw.trade_result, 0) AS trade_result_value,
                            CAST(sw.trade_pnl AS DOUBLE) AS trade_pnl,
                            hs.harmonic_type,
                            CASE
                                WHEN hs.price_accuracy <= 10 THEN '0-10'
                                WHEN hs.price_accuracy <= 20 THEN '10-20'
                                WHEN hs.price_accuracy <= 30 THEN '20-30'
                                WHEN hs.price_accuracy <= 40 THEN '30-40'
                                WHEN hs.price_accuracy <= 50 THEN '40-50'
                                WHEN hs.price_accuracy <= 60 THEN '50-60'
                                WHEN hs.price_accuracy <= 70 THEN '60-70'
                                WHEN hs.price_accuracy <= 80 THEN '70-80'
                                WHEN hs.price_accuracy <= 90 THEN '80-90'
                                ELSE '90-100'
                            END AS bin
                        FROM pattern_setups s
                        INNER JOIN pattern_harmonic_scores hs
                            ON hs.setup_id = s.setup_id
                        LEFT JOIN pattern_outcomes_swing sw
                            ON sw.setup_id = s.setup_id
                        WHERE s.market = ?
                          AND s.d_date >= ?
                          AND s.d_date < ?
                          AND hs.price_accuracy IS NOT NULL
                    ) source_rows
                    GROUP BY market, trade_result_value, harmonic_type, bin
                    "#,
                )
                .bind(year)
                .bind(market)
                .bind(start.as_str())
                .bind(end.as_str())
                .execute(&self.pool)
                .await?;

                sqlx::query(
                    r#"
                    INSERT INTO accuracy_bin_rollup (
                        rollup_year,
                        market,
                        trade_result_value,
                        source_scope,
                        harmonic_type,
                        bin,
                        total_count,
                        closed_count,
                        open_count,
                        win_count,
                        loss_count,
                        return_sum,
                        return_count,
                        expectancy_sum,
                        expectancy_count,
                        win_return_sum,
                        win_return_count,
                        loss_return_sum,
                        loss_return_count
                    )
                    SELECT
                        ? AS rollup_year,
                        market,
                        trade_result_value,
                        'all_patterns' AS source_scope,
                        harmonic_type,
                        bin,
                        COUNT(*) AS total_count,
                        SUM(CASE WHEN trade_result_value IN (1, 2) THEN 1 ELSE 0 END) AS closed_count,
                        SUM(CASE WHEN trade_result_value NOT IN (1, 2) THEN 1 ELSE 0 END) AS open_count,
                        SUM(CASE WHEN trade_result_value = 1 THEN 1 ELSE 0 END) AS win_count,
                        SUM(CASE WHEN trade_result_value = 2 THEN 1 ELSE 0 END) AS loss_count,
                        SUM(COALESCE(trade_pnl, 0.0)) AS return_sum,
                        SUM(CASE WHEN trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS return_count,
                        SUM(CASE WHEN trade_result_value IN (1, 2) THEN COALESCE(trade_pnl, 0.0) ELSE 0.0 END) AS expectancy_sum,
                        SUM(CASE WHEN trade_result_value IN (1, 2) AND trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS expectancy_count,
                        SUM(CASE WHEN trade_result_value = 1 THEN COALESCE(trade_pnl, 0.0) ELSE 0.0 END) AS win_return_sum,
                        SUM(CASE WHEN trade_result_value = 1 AND trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS win_return_count,
                        SUM(CASE WHEN trade_result_value = 2 THEN COALESCE(trade_pnl, 0.0) ELSE 0.0 END) AS loss_return_sum,
                        SUM(CASE WHEN trade_result_value = 2 AND trade_pnl IS NOT NULL THEN 1 ELSE 0 END) AS loss_return_count
                    FROM (
                        SELECT
                            s.market,
                            COALESCE(sw.trade_result, 0) AS trade_result_value,
                            CAST(sw.trade_pnl AS DOUBLE) AS trade_pnl,
                            'Multi' AS harmonic_type,
                            COALESCE(NULLIF(sw.bin, ''), 'Multi') AS bin
                        FROM pattern_setups s
                        LEFT JOIN pattern_outcomes_swing sw
                            ON sw.setup_id = s.setup_id
                        WHERE s.market = ?
                          AND s.d_date >= ?
                          AND s.d_date < ?
                          AND sw.bin IS NOT NULL
                    ) source_rows
                    WHERE bin IS NOT NULL
                    GROUP BY market, trade_result_value, harmonic_type, bin
                    "#,
                )
                .bind(year)
                .bind(market)
                .bind(start.as_str())
                .bind(end.as_str())
                .execute(&self.pool)
                .await?;
            }

            self.set_dashboard_cache_state(
                "accuracy_bin_rollup",
                false,
                Some(year),
                Some("backfilling"),
            )
            .await?;
        }

        self.set_dashboard_cache_state("accuracy_bin_rollup", true, Some(max_year), Some("ready"))
            .await?;

        Ok(())
    }
}

fn two_decimals<S>(val: &f64, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let v = (val * 100.0).round() / 100.0;
    s.serialize_f64(v)
}
