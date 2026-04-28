//! SQLite access layer and migrations.

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{ConnectOptions, SqlitePool};
use std::str::FromStr;
use twilight_model::id::Id;
use twilight_model::id::marker::GuildMarker;

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

/// Make sure a row exists in `guilds` for this guild id. Idempotent — safe
/// to call before any per-guild write that has a foreign key into `guilds`.
/// Replaces the four `INSERT OR IGNORE` copies that used to live in command
/// handlers and the `GuildCreate` event path.
pub async fn ensure_guild(pool: &SqlitePool, guild_id: Id<GuildMarker>) -> Result<()> {
    sqlx::query("INSERT OR IGNORE INTO guilds (id) VALUES (?)")
        .bind(guild_id.get() as i64)
        .execute(pool)
        .await
        .context("ensure guild row exists")?;
    Ok(())
}
