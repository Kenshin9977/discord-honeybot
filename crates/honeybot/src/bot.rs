//! Bot lifecycle: connect to the Discord gateway, dispatch events to handlers.

use anyhow::{Context, Result, anyhow};
use sqlx::SqlitePool;
use std::sync::{Arc, OnceLock};
use tracing::{info, warn};
use twilight_gateway::{ConfigBuilder, EventTypeFlags, Shard, ShardId, StreamExt as _};
use twilight_http::Client as HttpClient;
use twilight_model::gateway::Intents;
use twilight_model::gateway::event::Event;
use twilight_model::id::Id;
use twilight_model::id::marker::ApplicationMarker;

/// Shared state passed to every handler.
pub struct AppState {
    pub http: Arc<HttpClient>,
    pub db: SqlitePool,
    /// Filled on the first `Ready`. Reads must wait until then.
    pub application_id: OnceLock<Id<ApplicationMarker>>,
}

impl AppState {
    pub fn application_id(&self) -> Result<Id<ApplicationMarker>> {
        self.application_id
            .get()
            .copied()
            .ok_or_else(|| anyhow!("application_id not yet known (Ready not received)"))
    }
}

pub async fn run() -> Result<()> {
    let config = crate::config::Config::load()?;

    if config.discord_token.trim().is_empty() {
        anyhow::bail!("DISCORD_TOKEN must not be empty");
    }

    let db = crate::db::ensure_ready(&config.database_url).await?;
    crate::i18n::init().context("failed to initialise i18n bundles")?;

    let token = config.discord_token.clone();
    let http = Arc::new(HttpClient::new(token.clone()));

    let state = Arc::new(AppState {
        http,
        db,
        application_id: OnceLock::new(),
    });

    let intents = Intents::GUILDS
        | Intents::GUILD_MESSAGES
        | Intents::MESSAGE_CONTENT
        | Intents::GUILD_MODERATION
        | Intents::GUILD_MEMBERS;

    let shard_config = ConfigBuilder::new(token, intents).build();
    let mut shard = Shard::with_config(ShardId::ONE, shard_config);

    info!("honeybot connecting to Discord");

    while let Some(item) = shard.next_event(EventTypeFlags::all()).await {
        let event = match item {
            Ok(event) => event,
            Err(source) => {
                warn!(?source, "shard receive error");
                continue;
            }
        };

        let state = state.clone();
        tokio::spawn(async move {
            if let Err(err) = dispatch(state, event).await {
                warn!(?err, "event handler error");
            }
        });
    }

    Ok(())
}

async fn dispatch(state: Arc<AppState>, event: Event) -> Result<()> {
    match event {
        Event::Ready(ready) => {
            let app_id = ready.application.id;
            let _ = state.application_id.set(app_id);
            info!(user = %ready.user.name, app_id = %app_id, "connected as bot");
        }
        Event::GuildCreate(guild) => {
            let guild_id = guild.id();

            crate::db::ensure_guild(&state.db, guild_id).await?;

            if let Ok(app_id) = state.application_id()
                && let Err(err) =
                    crate::commands::register_for_guild(&state, app_id, guild_id).await
            {
                warn!(
                    ?err,
                    guild_id = guild_id.get(),
                    "failed to register commands"
                );
            }
        }
        Event::MessageCreate(msg) => {
            crate::handlers::honeypot::on_message(state, msg.0).await?;
        }
        Event::InteractionCreate(interaction) => {
            let app_id = state.application_id()?;
            crate::commands::dispatch(state, app_id, interaction.0).await?;
        }
        _ => {}
    }
    Ok(())
}
