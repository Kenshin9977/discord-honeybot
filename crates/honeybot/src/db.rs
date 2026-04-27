//! SQLite access layer and migrations.

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{ConnectOptions, SqlitePool};
use std::str::FromStr;

pub async fn ensure_ready(database_url: &str) -> Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str(database_url)
        .with_context(|| format!("invalid database url `{database_url}`"))?
        .create_if_missing(true)
        .disable_statement_logging();

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opts)
        .await
        .context("failed to open SQLite pool")?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to apply SQLite migrations")?;

    Ok(pool)
}

pub async fn migrate() -> Result<()> {
    let config = crate::config::Config::load()?;
    ensure_ready(&config.database_url).await?;
    println!("migrations applied");
    Ok(())
}
