//! Bot lifecycle: connect to the Discord gateway, dispatch events to handlers.

use anyhow::Result;
use tracing::info;

pub async fn run() -> Result<()> {
    let config = crate::config::Config::load()?;

    if config.discord_token.trim().is_empty() {
        anyhow::bail!("DISCORD_TOKEN must not be empty");
    }

    crate::db::ensure_ready(&config.database_url).await?;
    crate::i18n::init();

    info!(
        registry = config.registry_url.is_some(),
        token_len = config.discord_token.len(),
        "honeybot starting"
    );

    // TODO: build twilight Shard with required intents (Guilds, GuildMessages,
    // MessageContent, GuildMembers, GuildModeration), spawn event loop,
    // register slash commands, optionally start federation::stream::run().
    todo!("gateway loop not implemented yet")
}
