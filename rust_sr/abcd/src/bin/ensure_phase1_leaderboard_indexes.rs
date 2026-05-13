use std::env;

use sqlx::mysql::MySqlPool;

fn database_url_from_env() -> Result<String, Box<dyn std::error::Error>> {
    env::var("ABCD_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| {
            "Missing required environment variable: ABCD_DATABASE_URL or DATABASE_URL".into()
        })
}

fn is_duplicate_index_error(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(db_error) => {
            matches!(db_error.code().as_deref(), Some("1061") | Some("42000"))
                && db_error.message().contains("Duplicate key name")
        }
        _ => false,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url = database_url_from_env()?;
    let pool = MySqlPool::connect(&database_url).await?;
    let statements = [
        (
            "idx_phase1_runs_latest_family",
            r#"
            ALTER TABLE phase1_strategy_runs
            ADD INDEX idx_phase1_runs_latest_family (family_key, source_scope, period_year, id),
            ALGORITHM=INPLACE,
            LOCK=NONE
            "#,
        ),
        (
            "idx_phase1_runs_latest_scope",
            r#"
            ALTER TABLE phase1_strategy_runs
            ADD INDEX idx_phase1_runs_latest_scope (source_scope, period_year, family_key, id),
            ALGORITHM=INPLACE,
            LOCK=NONE
            "#,
        ),
        (
            "idx_phase1_results_best_leaderboard",
            r#"
            ALTER TABLE phase1_strategy_results
            ADD INDEX idx_phase1_results_best_leaderboard (
                source_scope,
                period_year,
                result_rank,
                score DESC,
                avg_r DESC,
                trade_count DESC
            ),
            ALGORITHM=INPLACE,
            LOCK=NONE
            "#,
        ),
        (
            "idx_phase1_results_all_leaderboard",
            r#"
            ALTER TABLE phase1_strategy_results
            ADD INDEX idx_phase1_results_all_leaderboard (
                source_scope,
                period_year,
                score DESC,
                avg_r DESC,
                trade_count DESC
            ),
            ALGORITHM=INPLACE,
            LOCK=NONE
            "#,
        ),
    ];

    for (name, statement) in statements {
        match sqlx::query(statement).execute(&pool).await {
            Ok(_) => println!("created {name}"),
            Err(error) if is_duplicate_index_error(&error) => println!("exists {name}"),
            Err(error) => return Err(error.into()),
        }
    }

    Ok(())
}
