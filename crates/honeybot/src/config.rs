//! Process-level configuration loaded once at startup.
//!
//! All per-guild settings live in SQLite and are mutated at runtime via slash
//! commands; only secrets and infrastructure-level URLs come from env.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub discord_token: String,
    pub database_url: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        use figment::{
            Figment,
            providers::{Env, Serialized},
        };

        let figment = Figment::from(Serialized::defaults(Defaults::default())).merge(Env::raw());

        figment
            .extract::<Config>()
            .context("failed to load configuration from environment")
    }
}

#[derive(Debug, Serialize)]
struct Defaults {
    database_url: String,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            database_url: "sqlite:///data/honeybot.db?mode=rwc".to_owned(),
        }
    }
}
