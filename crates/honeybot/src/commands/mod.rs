//! Slash command registration and dispatch.

use anyhow::Result;
use std::sync::Arc;
use tracing::warn;
use twilight_model::application::interaction::Interaction;
use twilight_model::application::interaction::InteractionData;
use twilight_model::id::Id;
use twilight_model::id::marker::{ApplicationMarker, GuildMarker};

use crate::bot::AppState;

pub mod honeypot;
pub mod setup;
pub mod warn;

/// Build the full command list this bot exposes. Called once and pushed to
/// every guild as the bot connects.
pub fn definitions() -> Vec<twilight_model::application::command::Command> {
    vec![honeypot::definition(), warn::definition()]
}

/// Push `definitions()` to a single guild. Idempotent.
pub async fn register_for_guild(
    state: &AppState,
    application_id: Id<ApplicationMarker>,
    guild_id: Id<GuildMarker>,
) -> Result<()> {
    let commands = definitions();
    state
        .http
        .interaction(application_id)
        .set_guild_commands(guild_id, &commands)
        .await?;
    Ok(())
}

/// Route an `InteractionCreate` to the right command module.
pub async fn dispatch(
    state: Arc<AppState>,
    application_id: Id<ApplicationMarker>,
    interaction: Interaction,
) -> Result<()> {
    let Some(InteractionData::ApplicationCommand(command_data)) = interaction.data.clone() else {
        return Ok(());
    };

    match command_data.name.as_str() {
        "honeypot" => honeypot::handle(state, application_id, interaction, *command_data).await,
        "warn" => warn::handle(state, application_id, interaction, *command_data).await,
        other => {
            warn!(name = other, "unknown slash command");
            Ok(())
        }
    }
}
