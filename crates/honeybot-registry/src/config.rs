//! Process configuration. Everything that varies per deployment lives here;
//! per-pool settings live in Postgres.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    /// Optional Discord bot token used to verify guild membership at
    /// registration time. If unset, registration accepts any caller (only
    /// suitable for closed/private deployments).
    #[allow(dead_code)]
    pub discord_verification_token: Option<String>,
    /// Hex-encoded 32-byte secret used to derive per-token HMAC keys for
    /// validating signed ban events. Auto-generated on first start if missing
    /// and stored alongside the registry data — but for v1 it must be set
    /// explicitly to make multi-instance deployments deterministic.
    pub registry_secret: String,
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
    bind_addr: String,
    registry_secret: String,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:8080".to_owned(),
            // Empty default forces operators to set REGISTRY_SECRET in
            // production. The bot publishes signed events; rotating this
            // secret invalidates outstanding tokens.
            registry_secret: String::new(),
        }
    }
}
