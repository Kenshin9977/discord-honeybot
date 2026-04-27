//! Axum router root.

use anyhow::Result;

pub mod auth;
pub mod bans;
pub mod pools;
pub mod stream;

pub async fn serve() -> Result<()> {
    // TODO: build axum::Router by merging the submodule routers, attach
    // tracing/cors/auth middleware, bind to BIND_ADDR (default 0.0.0.0:8080).
    todo!("registry HTTP server not implemented yet")
}
