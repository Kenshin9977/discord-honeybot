//! `/honeypot add|remove|list` — manage honeypot channels for a guild.

use anyhow::{Context, Result, anyhow};
use std::sync::Arc;
use twilight_model::application::command::{Command, CommandType};
use twilight_model::application::interaction::Interaction;
use twilight_model::application::interaction::application_command::{
    CommandData, CommandDataOption, CommandOptionValue,
};
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};
use twilight_model::id::Id;
use twilight_model::id::marker::{ApplicationMarker, ChannelMarker};
use twilight_util::builder::command::{
    ChannelBuilder, CommandBuilder, StringBuilder, SubCommandBuilder,
};

use crate::bot::AppState;

pub fn definition() -> Command {
    CommandBuilder::new(
        "honeypot",
        "Manage honeypot channels.",
        CommandType::ChatInput,
    )
    .option(
        SubCommandBuilder::new("add", "Add a channel to the honeypot list.")
            .option(
                ChannelBuilder::new("channel", "The channel to mark as a honeypot.").required(true),
            )
            .option(
                StringBuilder::new("action", "What to do when someone posts in this channel.")
                    .required(true)
                    .choices([
                        ("Ban", "ban".to_owned()),
                        ("Kick", "kick".to_owned()),
                        ("Timeout (1 hour)", "timeout".to_owned()),
                    ]),
            ),
    )
    .option(
        SubCommandBuilder::new("remove", "Remove a channel from the honeypot list.")
            .option(ChannelBuilder::new("channel", "The channel to remove.").required(true)),
    )
    .option(SubCommandBuilder::new(
        "list",
        "List configured honeypot channels.",
    ))
    .build()
}

pub async fn handle(
    state: Arc<AppState>,
    application_id: Id<ApplicationMarker>,
    interaction: Interaction,
    command: CommandData,
) -> Result<()> {
    let Some(guild_id) = interaction.guild_id else {
        return reply(
            &state,
            application_id,
            &interaction,
            "This command can only be used inside a server.",
        )
        .await;
    };

    let sub = command
        .options
        .first()
        .ok_or_else(|| anyhow!("missing subcommand"))?;

    let CommandOptionValue::SubCommand(sub_options) = &sub.value else {
        return Err(anyhow!("expected subcommand value"));
    };

    let content = match sub.name.as_str() {
        "add" => add(&state, guild_id, sub_options).await?,
        "remove" => remove(&state, guild_id, sub_options).await?,
        "list" => list(&state, guild_id).await?,
        other => format!("Unknown subcommand `{other}`."),
    };

    reply(&state, application_id, &interaction, &content).await
}

async fn add(
    state: &AppState,
    guild_id: twilight_model::id::Id<twilight_model::id::marker::GuildMarker>,
    options: &[CommandDataOption],
) -> Result<String> {
    let channel = option_channel(options, "channel")?;
    let action = option_string(options, "action")?;

    let action_duration_s: Option<i64> = if action == "timeout" {
        Some(3600)
    } else {
        None
    };

    sqlx::query("INSERT OR IGNORE INTO guilds (id) VALUES (?)")
        .bind(guild_id.get() as i64)
        .execute(&state.db)
        .await
        .context("upsert guild")?;

    sqlx::query(
        "INSERT INTO honeypot_channels (guild_id, channel_id, action, action_duration_s)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(guild_id, channel_id) DO UPDATE SET
            action = excluded.action,
            action_duration_s = excluded.action_duration_s",
    )
    .bind(guild_id.get() as i64)
    .bind(channel.get() as i64)
    .bind(&action)
    .bind(action_duration_s)
    .execute(&state.db)
    .await
    .context("upsert honeypot channel")?;

    Ok(format!(
        "Honeypot configured for <#{}> — action: `{}`.",
        channel.get(),
        action
    ))
}

async fn remove(
    state: &AppState,
    guild_id: twilight_model::id::Id<twilight_model::id::marker::GuildMarker>,
    options: &[CommandDataOption],
) -> Result<String> {
    let channel = option_channel(options, "channel")?;

    let result = sqlx::query("DELETE FROM honeypot_channels WHERE guild_id = ? AND channel_id = ?")
        .bind(guild_id.get() as i64)
        .bind(channel.get() as i64)
        .execute(&state.db)
        .await
        .context("delete honeypot channel")?;

    if result.rows_affected() == 0 {
        Ok(format!(
            "<#{}> was not configured as a honeypot.",
            channel.get()
        ))
    } else {
        Ok(format!("Honeypot removed from <#{}>.", channel.get()))
    }
}

async fn list(
    state: &AppState,
    guild_id: twilight_model::id::Id<twilight_model::id::marker::GuildMarker>,
) -> Result<String> {
    let rows: Vec<(i64, String, Option<i64>)> = sqlx::query_as(
        "SELECT channel_id, action, action_duration_s
         FROM honeypot_channels
         WHERE guild_id = ?
         ORDER BY channel_id",
    )
    .bind(guild_id.get() as i64)
    .fetch_all(&state.db)
    .await
    .context("list honeypot channels")?;

    if rows.is_empty() {
        return Ok("No honeypot channels configured.".into());
    }

    let mut out = String::from("**Honeypot channels:**\n");
    for (channel_id, action, duration) in rows {
        match action.as_str() {
            "timeout" => {
                let mins = duration.unwrap_or(3600) / 60;
                out.push_str(&format!("• <#{channel_id}> — timeout {mins}m\n"));
            }
            _ => {
                out.push_str(&format!("• <#{channel_id}> — {action}\n"));
            }
        }
    }
    Ok(out)
}

fn option_channel(options: &[CommandDataOption], name: &str) -> Result<Id<ChannelMarker>> {
    options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| match o.value {
            CommandOptionValue::Channel(id) => Some(id),
            _ => None,
        })
        .ok_or_else(|| anyhow!("missing channel option `{name}`"))
}

fn option_string(options: &[CommandDataOption], name: &str) -> Result<String> {
    options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| match &o.value {
            CommandOptionValue::String(s) => Some(s.clone()),
            _ => None,
        })
        .ok_or_else(|| anyhow!("missing string option `{name}`"))
}

async fn reply(
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
