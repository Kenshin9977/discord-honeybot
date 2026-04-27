//! `honeybot` — Discord bot binary.
//!
//! Subcommands:
//! - `serve`   : connect to the Discord gateway and run the bot
//! - `migrate` : apply pending SQLite migrations and exit

use anyhow::Result;

mod bot;
mod commands;
mod config;
mod db;
mod handlers;
mod i18n;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let args: Vec<String> = std::env::args().collect();
    let subcommand = args.get(1).map(String::as_str).unwrap_or("serve");

    match subcommand {
        "serve" => bot::run().await,
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
