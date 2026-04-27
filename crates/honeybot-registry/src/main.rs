//! `honeybot-registry` — federation server binary.
//!
//! Subcommands:
//! - `serve`   : run the HTTP API
//! - `migrate` : apply pending Postgres migrations and exit

use anyhow::Result;

mod api;
mod auth;
mod config;
mod db;
mod fanout;
mod reputation;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let args: Vec<String> = std::env::args().collect();
    let subcommand = args.get(1).map(String::as_str).unwrap_or("serve");

    match subcommand {
        "serve" => api::serve().await,
        "migrate" => db::migrate().await,
        other => {
            eprintln!("unknown subcommand `{other}`. expected: serve | migrate");
            std::process::exit(2);
        }
    }
}

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .json()
        .init();
}
