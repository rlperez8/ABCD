use std::env;

use sqlx::mysql::{MySqlPool, MySqlPoolOptions};

fn usage() -> &'static str {
    "Usage: cargo run --bin finalize_entry_exit_build_run -- --run-id RUN_ID"
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|items| items[0] == name)
        .map(|items| items[1].trim().to_string())
        .filter(|value| !value.is_empty())
}

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

async fn ensure_entry_exit_template_build_coverage_rows_ui_table(
    pool: &MySqlPool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_template_build_coverage_rows_ui (
            test_id VARCHAR(64) NOT NULL,
            run_id VARCHAR(64) NOT NULL,
            source_scope VARCHAR(16) NOT NULL,
            exchange_name VARCHAR(32) NOT NULL DEFAULT 'Unknown',
            root_symbol VARCHAR(32) NOT NULL,
            source_timeframe VARCHAR(16) NOT NULL DEFAULT 'unknown',
            scanned_pattern_count BIGINT NOT NULL DEFAULT 0,
            universe_pattern_count BIGINT NOT NULL DEFAULT 0,
            contract_count BIGINT NOT NULL DEFAULT 0,
            status VARCHAR(16) NOT NULL DEFAULT 'Not scanned',
            sort_order BIGINT NOT NULL DEFAULT 0,
            first_d_confirm_date DATETIME NULL,
            last_d_confirm_date DATETIME NULL,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            PRIMARY KEY (test_id, root_symbol, source_timeframe),
            INDEX idx_entry_exit_build_coverage_rows_run_exchange (run_id, exchange_name, sort_order),
            INDEX idx_entry_exit_build_coverage_rows_status (run_id, status, scanned_pattern_count)
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn ensure_entry_exit_template_build_summary_ui_table(
    pool: &MySqlPool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS entry_exit_template_build_summary_ui (
            test_id VARCHAR(64) NOT NULL PRIMARY KEY,
            run_id VARCHAR(64) NOT NULL UNIQUE,
            build_label VARCHAR(32) NULL,
            source_scope VARCHAR(16) NOT NULL,
            source_timeframe VARCHAR(16) NOT NULL DEFAULT 'unknown',
            scan_year_start INT NULL,
            scan_year_end INT NULL,
            scan_year_label VARCHAR(32) NOT NULL DEFAULT 'All',
            patterns_scanned BIGINT NOT NULL DEFAULT 0,
            templates_created BIGINT NOT NULL DEFAULT 0,
            coverage_patterns BIGINT NOT NULL DEFAULT 0,
            root_count BIGINT NOT NULL DEFAULT 0,
            exchange_count BIGINT NOT NULL DEFAULT 0,
            requested_limit BIGINT NOT NULL DEFAULT 0,
            result_rows BIGINT NOT NULL DEFAULT 0,
            elapsed_ms BIGINT NOT NULL DEFAULT 0,
            created_at DATETIME NULL,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            INDEX idx_entry_exit_build_summary_run (run_id),
            INDEX idx_entry_exit_build_summary_created (created_at)
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn refresh_entry_exit_template_build_coverage_rows_ui(
    pool: &MySqlPool,
    run_id: &str,
) -> Result<u64, sqlx::Error> {
    ensure_entry_exit_template_build_coverage_rows_ui_table(pool).await?;

    let mut affected = 0_u64;
    sqlx::query("DELETE FROM entry_exit_template_build_coverage_rows_ui WHERE run_id = ?")
        .bind(run_id)
        .execute(pool)
        .await?;

    let scanned_result = sqlx::query(
        r#"
        INSERT INTO entry_exit_template_build_coverage_rows_ui (
            test_id,
            run_id,
            source_scope,
            exchange_name,
            root_symbol,
            source_timeframe,
            scanned_pattern_count,
            universe_pattern_count,
            contract_count,
            status,
            sort_order,
            first_d_confirm_date,
            last_d_confirm_date
        )
        SELECT
            scanned.run_id AS test_id,
            scanned.run_id,
            scanned.source_scope,
            scanned.exchange_name,
            scanned.root_symbol,
            scanned.source_timeframe,
            scanned.scanned_pattern_count,
            0 AS universe_pattern_count,
            scanned.contract_count,
            'Scanned' AS status,
            ROW_NUMBER() OVER (
                ORDER BY
                    CASE scanned.exchange_name
                        WHEN 'CME' THEN 1
                        WHEN 'CBOT' THEN 2
                        WHEN 'NYMEX' THEN 3
                        WHEN 'COMEX' THEN 4
                        ELSE 9
                    END,
                    scanned.scanned_pattern_count DESC,
                    scanned.root_symbol ASC,
                    scanned.source_timeframe ASC
            ) AS sort_order,
            scanned.first_d_confirm_date,
            scanned.last_d_confirm_date
        FROM (
            SELECT
                c.run_id,
                c.source_scope,
                CASE
                    WHEN normalized.root_symbol IN ('6A', '6B', '6C', '6E', '6J', '6M', '6N', '6S', 'BTC', 'EMD', 'ES', 'GF', 'HE', 'LE', 'M2K', 'MES', 'MNQ', 'NKD', 'NQ', 'RTY') THEN 'CME'
                    WHEN normalized.root_symbol IN ('KE', 'UB', 'YM', 'ZB', 'ZC', 'ZF', 'ZL', 'ZM', 'ZN', 'ZS', 'ZT', 'ZW') THEN 'CBOT'
                    WHEN normalized.root_symbol IN ('GC', 'HG', 'MGC', 'SI') THEN 'COMEX'
                    WHEN normalized.root_symbol IN ('CL', 'HO', 'MCL', 'NG', 'PA', 'PL', 'QG', 'QM', 'RB') THEN 'NYMEX'
                    ELSE COALESCE(MAX(NULLIF(c.exchange_name, '')), 'Unknown')
                END AS exchange_name,
                normalized.root_symbol,
                c.source_timeframe,
                CAST(SUM(c.pattern_count) AS SIGNED) AS scanned_pattern_count,
                CAST(SUM(c.contract_count) AS SIGNED) AS contract_count,
                MIN(c.first_d_confirm_date) AS first_d_confirm_date,
                MAX(c.last_d_confirm_date) AS last_d_confirm_date
            FROM entry_exit_template_build_coverage_ui c
            JOIN (
                SELECT
                    run_id,
                    source_timeframe,
                    root_symbol AS original_root_symbol,
                    CASE
                        WHEN root_symbol REGEXP '^ZB[FGHJKMNQUVXZ][0-9]{1,2}$' THEN 'ZB'
                        WHEN root_symbol REGEXP '^ZN[FGHJKMNQUVXZ][0-9]{1,2}$' THEN 'ZN'
                        ELSE root_symbol
                    END AS root_symbol
                FROM entry_exit_template_build_coverage_ui
                WHERE run_id = ?
            ) normalized
              ON normalized.run_id = c.run_id
             AND normalized.source_timeframe = c.source_timeframe
             AND normalized.original_root_symbol = c.root_symbol
            WHERE c.run_id = ?
            GROUP BY c.run_id, c.source_scope, normalized.root_symbol, c.source_timeframe
        ) scanned
        "#,
    )
    .bind(run_id)
    .bind(run_id)
    .execute(pool)
    .await?;
    affected += scanned_result.rows_affected();

    let universe_result = sqlx::query(
        r#"
        INSERT INTO entry_exit_template_build_coverage_rows_ui (
            test_id,
            run_id,
            source_scope,
            exchange_name,
            root_symbol,
            source_timeframe,
            scanned_pattern_count,
            universe_pattern_count,
            contract_count,
            status,
            sort_order,
            first_d_confirm_date,
            last_d_confirm_date
        )
        SELECT
            ? AS test_id,
            ? AS run_id,
            universe.source_scope,
            universe.exchange_name,
            universe.root_symbol,
            universe.source_timeframe,
            0 AS scanned_pattern_count,
            universe.universe_pattern_count,
            universe.contract_count,
            'Not scanned' AS status,
            100000 AS sort_order,
            universe.first_d_confirm_date,
            universe.last_d_confirm_date
        FROM (
            SELECT
                run_meta.source_scope,
                CASE
                    WHEN normalized.root_symbol IN ('6A', '6B', '6C', '6E', '6J', '6M', '6N', '6S', 'BTC', 'EMD', 'ES', 'GF', 'HE', 'LE', 'M2K', 'MES', 'MNQ', 'NKD', 'NQ', 'RTY') THEN 'CME'
                    WHEN normalized.root_symbol IN ('KE', 'UB', 'YM', 'ZB', 'ZC', 'ZF', 'ZL', 'ZM', 'ZN', 'ZS', 'ZT', 'ZW') THEN 'CBOT'
                    WHEN normalized.root_symbol IN ('GC', 'HG', 'MGC', 'SI') THEN 'COMEX'
                    WHEN normalized.root_symbol IN ('CL', 'HO', 'MCL', 'NG', 'PA', 'PL', 'QG', 'QM', 'RB') THEN 'NYMEX'
                    ELSE 'Unknown'
                END AS exchange_name,
                normalized.root_symbol,
                normalized.source_timeframe,
                CAST(COUNT(DISTINCT normalized.setup_id) AS SIGNED) AS universe_pattern_count,
                CAST(COUNT(DISTINCT normalized.contract_symbol) AS SIGNED) AS contract_count,
                MIN(normalized.d_confirm_date) AS first_d_confirm_date,
                MAX(normalized.d_confirm_date) AS last_d_confirm_date
            FROM (
                SELECT
                    ps.setup_id,
                    CASE
                        WHEN COALESCE(NULLIF(ps.root_symbol, ''), NULLIF(ps.symbol, ''), 'Unknown') REGEXP '^ZB[FGHJKMNQUVXZ][0-9]{1,2}$' THEN 'ZB'
                        WHEN COALESCE(NULLIF(ps.root_symbol, ''), NULLIF(ps.symbol, ''), 'Unknown') REGEXP '^ZN[FGHJKMNQUVXZ][0-9]{1,2}$' THEN 'ZN'
                        ELSE COALESCE(NULLIF(ps.root_symbol, ''), NULLIF(ps.symbol, ''), 'Unknown')
                    END AS root_symbol,
                    COALESCE(NULLIF(ps.contract_symbol, ''), NULLIF(ps.symbol, ''), 'Unknown') AS contract_symbol,
                    COALESCE(NULLIF(ps.source_timeframe, ''), 'unknown') AS source_timeframe,
                    COALESCE(ps.d_confirm_date, ps.d_date) AS d_confirm_date,
                    ps.source_table
                FROM pattern_setups ps
                JOIN (
                    SELECT DISTINCT source_timeframe
                    FROM entry_exit_template_build_coverage_ui
                    WHERE run_id = ?
                ) build_timeframes
                  ON build_timeframes.source_timeframe = COALESCE(NULLIF(ps.source_timeframe, ''), 'unknown')
                WHERE ps.d_date IS NOT NULL
                  AND ps.full_pattern_length > 0
                  AND ABS(ps.cd_price_length) > 0
            ) normalized
            JOIN entry_exit_template_creator_runs run_meta
              ON run_meta.run_id = ?
            WHERE (
                (run_meta.source_scope = 'futures' AND COALESCE(normalized.source_table, '') LIKE 'futures_contract_%_candles')
                OR (run_meta.source_scope = 'daily' AND COALESCE(normalized.source_table, '') = 'candles' AND normalized.source_timeframe = 'daily')
                OR (run_meta.source_scope NOT IN ('futures', 'daily'))
            )
            GROUP BY run_meta.source_scope, normalized.root_symbol, normalized.source_timeframe
        ) universe
        ON DUPLICATE KEY UPDATE
            universe_pattern_count = VALUES(universe_pattern_count),
            contract_count = GREATEST(entry_exit_template_build_coverage_rows_ui.contract_count, VALUES(contract_count)),
            status = CASE
                WHEN scanned_pattern_count > 0 THEN 'Scanned'
                ELSE VALUES(status)
            END
        "#,
    )
    .bind(run_id)
    .bind(run_id)
    .bind(run_id)
    .bind(run_id)
    .execute(pool)
    .await?;
    affected += universe_result.rows_affected();

    let sort_result = sqlx::query(
        r#"
        UPDATE entry_exit_template_build_coverage_rows_ui rows_ui
        JOIN (
            SELECT
                test_id,
                root_symbol,
                source_timeframe,
                ROW_NUMBER() OVER (
                    ORDER BY
                        CASE exchange_name
                            WHEN 'CME' THEN 1
                            WHEN 'CBOT' THEN 2
                            WHEN 'NYMEX' THEN 3
                            WHEN 'COMEX' THEN 4
                            ELSE 9
                        END,
                        CASE status WHEN 'Scanned' THEN 0 ELSE 1 END,
                        scanned_pattern_count DESC,
                        root_symbol ASC,
                        source_timeframe ASC
                ) AS next_sort_order
            FROM entry_exit_template_build_coverage_rows_ui
            WHERE test_id = ?
        ) ranked
          ON ranked.test_id = rows_ui.test_id
         AND ranked.root_symbol = rows_ui.root_symbol
         AND ranked.source_timeframe = rows_ui.source_timeframe
        SET rows_ui.sort_order = ranked.next_sort_order
        WHERE rows_ui.test_id = ?
        "#,
    )
    .bind(run_id)
    .bind(run_id)
    .execute(pool)
    .await?;
    affected += sort_result.rows_affected();

    Ok(affected)
}

async fn refresh_entry_exit_template_build_summary_ui(
    pool: &MySqlPool,
    run_id: &str,
) -> Result<u64, sqlx::Error> {
    ensure_entry_exit_template_build_summary_ui_table(pool).await?;

    let result = sqlx::query(
        r#"
        INSERT INTO entry_exit_template_build_summary_ui (
            test_id,
            run_id,
            build_label,
            source_scope,
            source_timeframe,
            scan_year_start,
            scan_year_end,
            scan_year_label,
            patterns_scanned,
            templates_created,
            coverage_patterns,
            root_count,
            exchange_count,
            requested_limit,
            result_rows,
            elapsed_ms,
            created_at
        )
        SELECT
            r.run_id AS test_id,
            r.run_id,
            NULL AS build_label,
            r.source_scope,
            COALESCE(NULLIF(coverage.source_timeframe, ''), 'unknown') AS source_timeframe,
            years.scan_year_start,
            years.scan_year_end,
            CASE
                WHEN years.scan_year_start IS NULL AND r.period_year > 0 THEN CAST(r.period_year AS CHAR)
                WHEN years.scan_year_start IS NULL THEN 'All'
                WHEN years.scan_year_start = years.scan_year_end THEN CAST(years.scan_year_start AS CHAR)
                ELSE CONCAT(years.scan_year_start, '-', years.scan_year_end)
            END AS scan_year_label,
            r.scanned_patterns AS patterns_scanned,
            r.templates_created,
            COALESCE(coverage.coverage_patterns, 0) AS coverage_patterns,
            COALESCE(coverage.root_count, 0) AS root_count,
            COALESCE(coverage.exchange_count, 0) AS exchange_count,
            r.requested_limit,
            r.result_rows,
            r.elapsed_ms,
            r.created_at
        FROM entry_exit_template_creator_runs r
        LEFT JOIN (
            SELECT
                run_id,
                CAST(SUM(pattern_count) AS SIGNED) AS coverage_patterns,
                CAST(COUNT(DISTINCT root_symbol) AS SIGNED) AS root_count,
                CAST(COUNT(DISTINCT CASE WHEN pattern_count > 0 THEN exchange_name ELSE NULL END) AS SIGNED) AS exchange_count,
                CASE
                    WHEN COUNT(DISTINCT source_timeframe) = 1 THEN MIN(source_timeframe)
                    WHEN COUNT(DISTINCT source_timeframe) > 1 THEN 'mixed'
                    ELSE 'unknown'
                END AS source_timeframe
            FROM entry_exit_template_build_coverage_ui
            WHERE run_id = ?
            GROUP BY run_id
        ) coverage
          ON coverage.run_id = r.run_id
        LEFT JOIN (
            SELECT
                run_id,
                MIN(YEAR(first_d_confirm_date)) AS scan_year_start,
                MAX(YEAR(last_d_confirm_date)) AS scan_year_end
            FROM entry_exit_template_build_coverage_ui
            WHERE run_id = ?
            GROUP BY run_id
        ) years
          ON years.run_id = r.run_id
        WHERE r.run_id = ?
        ON DUPLICATE KEY UPDATE
            run_id = VALUES(run_id),
            build_label = COALESCE(VALUES(build_label), build_label),
            source_scope = VALUES(source_scope),
            source_timeframe = VALUES(source_timeframe),
            scan_year_start = VALUES(scan_year_start),
            scan_year_end = VALUES(scan_year_end),
            scan_year_label = VALUES(scan_year_label),
            patterns_scanned = VALUES(patterns_scanned),
            templates_created = VALUES(templates_created),
            coverage_patterns = VALUES(coverage_patterns),
            root_count = VALUES(root_count),
            exchange_count = VALUES(exchange_count),
            requested_limit = VALUES(requested_limit),
            result_rows = VALUES(result_rows),
            elapsed_ms = VALUES(elapsed_ms),
            created_at = VALUES(created_at)
        "#,
    )
    .bind(run_id)
    .bind(run_id)
    .bind(run_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let raw_args = env::args().skip(1).collect::<Vec<_>>();
    let Some(run_id) = arg_value(&raw_args, "--run-id") else {
        return Err(usage().into());
    };

    let pool = MySqlPoolOptions::new()
        .max_connections(4)
        .connect(&database_url_from_env()?)
        .await?;

    let coverage_rows = refresh_entry_exit_template_build_coverage_rows_ui(&pool, &run_id).await?;
    let summary_rows = refresh_entry_exit_template_build_summary_ui(&pool, &run_id).await?;

    let coverage_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM entry_exit_template_build_coverage_rows_ui WHERE run_id = ?",
    )
    .bind(&run_id)
    .fetch_one(&pool)
    .await?;
    let summary_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM entry_exit_template_build_summary_ui WHERE run_id = ?",
    )
    .bind(&run_id)
    .fetch_one(&pool)
    .await?;

    println!("run_id={run_id}");
    println!("coverage_rows_affected={coverage_rows}");
    println!("summary_rows_affected={summary_rows}");
    println!("coverage_rows={coverage_count}");
    println!("summary_rows={summary_count}");

    Ok(())
}
