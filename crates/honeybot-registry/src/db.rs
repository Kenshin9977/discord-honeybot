//! Postgres pool and migration entrypoint.

use anyhow::{Context, Result};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{ConnectOptions, PgPool};
use std::str::FromStr;

pub async fn ensure_ready(database_url: &str) -> Result<PgPool> {
    let opts = PgConnectOptions::from_str(database_url)
        .with_context(|| format!("invalid database url `{database_url}`"))?
        .disable_statement_logging();

    let pool = PgPoolOptions::new()
        .max_connections(16)
        .connect_with(opts)
        .await
        .context("failed to open Postgres pool")?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to apply Postgres migrations")?;

    Ok(pool)
}

pub async fn migrate() -> Result<()> {
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    ensure_ready(&database_url).await?;
    println!("migrations applied");
    Ok(())
}
