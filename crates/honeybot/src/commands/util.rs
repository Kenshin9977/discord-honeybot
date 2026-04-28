//! Shared helpers for slash command modules: option extraction and the
//! ephemeral interaction-response builder. Every command module imports
//! from here rather than re-implementing parsing.

use anyhow::{Context, Result, anyhow};
use twilight_model::application::interaction::Interaction;
use twilight_model::application::interaction::application_command::{
    CommandDataOption, CommandOptionValue,
};
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};
use twilight_model::id::Id;
use twilight_model::id::marker::{ApplicationMarker, ChannelMarker, RoleMarker, UserMarker};

use crate::bot::AppState;

pub fn option_string(options: &[CommandDataOption], name: &str) -> Result<String> {
    options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| match &o.value {
            CommandOptionValue::String(s) => Some(s.clone()),
            _ => None,
        })
        .ok_or_else(|| anyhow!("missing string option `{name}`"))
}

pub fn option_channel(options: &[CommandDataOption], name: &str) -> Result<Id<ChannelMarker>> {
    options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| match o.value {
            CommandOptionValue::Channel(id) => Some(id),
            _ => None,
        })
        .ok_or_else(|| anyhow!("missing channel option `{name}`"))
}

pub fn option_user(options: &[CommandDataOption], name: &str) -> Result<Id<UserMarker>> {
    options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| match o.value {
            CommandOptionValue::User(id) => Some(id),
            _ => None,
        })
        .ok_or_else(|| anyhow!("missing user option `{name}`"))
}

pub fn option_role(options: &[CommandDataOption], name: &str) -> Result<Id<RoleMarker>> {
    options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| match o.value {
            CommandOptionValue::Role(id) => Some(id),
            _ => None,
        })
        .ok_or_else(|| anyhow!("missing role option `{name}`"))
}

pub fn option_int(options: &[CommandDataOption], name: &str) -> Result<i64> {
    option_int_opt(options, name).ok_or_else(|| anyhow!("missing integer option `{name}`"))
}

pub fn option_int_opt(options: &[CommandDataOption], name: &str) -> Option<i64> {
    options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| match o.value {
            CommandOptionValue::Integer(n) => Some(n),
            _ => None,
        })
}

/// Send an ephemeral interaction reply. Ephemeral keeps the response
/// visible only to the invoking moderator, which is the right default for
/// every command this bot exposes.
pub async fn reply(
    state: &AppState,
    application_id: Id<ApplicationMarker>,
    interaction: &Interaction,
    content: &str,
) -> Result<()> {
    let response = InteractionResponse {
        kind: InteractionResponseType::ChannelMessageWithSource,
        data: Some(InteractionResponseData {
            content: Some(content.to_owned()),
            flags: Some(twilight_model::channel::message::MessageFlags::EPHEMERAL),
            ..Default::default()
        }),
    };

    state
        .http
        .interaction(application_id)
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .context("send interaction response")?;

    Ok(())
}
