//! Axum router root and HTTP server.

use anyhow::{Context, Result};
use axum::{Router, http::StatusCode, middleware::from_fn_with_state, routing::get, routing::post};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::info;

pub mod auth;
pub mod bans;
pub mod pools;
pub mod stream;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    /// Wired up by the upcoming HMAC verification in `bans::ingest`.
    #[allow(dead_code)]
    pub registry_secret: Arc<[u8]>,
}

pub async fn serve() -> Result<()> {
    let config = crate::config::Config::load()?;

    let db = crate::db::ensure_ready(&config.database_url).await?;

    let secret_bytes: Arc<[u8]> = Arc::from(config.registry_secret.as_bytes());
    let state = AppState {
        db,
        registry_secret: secret_bytes,
    };

    // Routes that require Bearer auth.
    let authed = Router::new()
        .route("/pools", post(pools::create))
        .route("/pools/join", post(pools::join))
        .route("/pools/{id}", get(pools::get))
        .route_layer(from_fn_with_state(
            state.clone(),
            crate::auth::require_token,
        ));

    let router = Router::new()
        .route("/health", get(health))
        .route("/auth/register", post(auth::register))
        .merge(authed)
        .with_state(state);

    let addr: std::net::SocketAddr = config
        .bind_addr
        .parse()
        .with_context(|| format!("invalid BIND_ADDR `{}`", config.bind_addr))?;

    info!(%addr, "registry listening");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;

    axum::serve(listener, router)
        .await
        .context("axum serve loop")?;

    Ok(())
}

async fn health(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<&'static str, StatusCode> {
    sqlx::query("SELECT 1")
        .execute(&state.db)
        .await
        .map_err(|err| {
            tracing::warn!(?err, "health check db query failed");
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    Ok("ok")
}
