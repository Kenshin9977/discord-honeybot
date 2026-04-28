//! `/honeypot add|remove|list` and `/honeypot whitelist add|remove|list` —
//! manage honeypot channels and per-channel role exemptions for a guild.

use anyhow::{Context, Result, anyhow};
use std::sync::Arc;
use twilight_model::application::command::{Command, CommandType};
use twilight_model::application::interaction::Interaction;
use twilight_model::application::interaction::application_command::{
    CommandData, CommandDataOption, CommandOptionValue,
};
use twilight_model::id::Id;
use twilight_model::id::marker::{ApplicationMarker, ChannelMarker, GuildMarker};
use twilight_util::builder::command::{
    ChannelBuilder, CommandBuilder, RoleBuilder, StringBuilder, SubCommandBuilder,
    SubCommandGroupBuilder,
};

use crate::bot::AppState;
use crate::commands::util::{option_channel, option_role, option_string, reply};
use crate::domain::{Action, DEFAULT_TIMEOUT_SECS};

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
    .option(
        SubCommandGroupBuilder::new(
            "whitelist",
            "Manage role exemptions for a honeypot channel.",
        )
        .subcommands([
            SubCommandBuilder::new("add", "Exempt a role from a honeypot channel.")
                .option(ChannelBuilder::new("channel", "The honeypot channel.").required(true))
                .option(RoleBuilder::new("role", "Role to exempt.").required(true)),
            SubCommandBuilder::new("remove", "Stop exempting a role from a honeypot channel.")
                .option(ChannelBuilder::new("channel", "The honeypot channel.").required(true))
                .option(RoleBuilder::new("role", "Role to stop exempting.").required(true)),
            SubCommandBuilder::new("list", "List exempted roles for a honeypot channel.")
                .option(ChannelBuilder::new("channel", "The honeypot channel.").required(true)),
        ]),
    )
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

    let content = match (&sub.name[..], &sub.value) {
        ("add", CommandOptionValue::SubCommand(opts)) => add(&state, guild_id, opts).await?,
        ("remove", CommandOptionValue::SubCommand(opts)) => remove(&state, guild_id, opts).await?,
        ("list", CommandOptionValue::SubCommand(_)) => list(&state, guild_id).await?,
        ("whitelist", CommandOptionValue::SubCommandGroup(group)) => {
            whitelist(&state, guild_id, group).await?
        }
        (other, _) => format!("Unknown subcommand `{other}`."),
    };

    reply(&state, application_id, &interaction, &content).await
}

async fn add(
    state: &AppState,
    guild_id: twilight_model::id::Id<twilight_model::id::marker::GuildMarker>,
    options: &[CommandDataOption],
) -> Result<String> {
    let channel = option_channel(options, "channel")?;
    let action = Action::from_db(
        &option_string(options, "action")?,
        Some(DEFAULT_TIMEOUT_SECS),
    )?;

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
    .bind(action.kind_str())
    .bind(action.duration_secs())
    .execute(&state.db)
    .await
    .context("upsert honeypot channel")?;

    Ok(format!(
        "Honeypot configured for <#{}> — action: `{action}`.",
        channel.get()
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
    for (channel_id, action_str, duration) in rows {
        let action = Action::from_db(&action_str, duration)?;
        out.push_str(&format!("• <#{channel_id}> — {action}\n"));
    }
    Ok(out)
}

async fn whitelist(
    state: &AppState,
    guild_id: Id<GuildMarker>,
    group: &[CommandDataOption],
) -> Result<String> {
    let inner = group
        .first()
        .ok_or_else(|| anyhow!("missing whitelist subcommand"))?;
    let CommandOptionValue::SubCommand(opts) = &inner.value else {
        return Err(anyhow!("expected subcommand value under whitelist"));
    };

    match inner.name.as_str() {
        "add" => whitelist_add(state, guild_id, opts).await,
        "remove" => whitelist_remove(state, guild_id, opts).await,
        "list" => whitelist_list(state, guild_id, opts).await,
        other => Ok(format!("Unknown whitelist subcommand `{other}`.")),
    }
}

async fn whitelist_add(
    state: &AppState,
    guild_id: Id<GuildMarker>,
    opts: &[CommandDataOption],
) -> Result<String> {
    let channel = option_channel(opts, "channel")?;
    let role = option_role(opts, "role")?;

    let mut roles = read_whitelist(state, guild_id, channel).await?;
    let role_str = role.get().to_string();
    if !roles.iter().any(|r| r == &role_str) {
        roles.push(role_str);
        write_whitelist(state, guild_id, channel, &roles).await?;
    }

    Ok(format!(
        "Role <@&{}> exempted from <#{}>.",
        role.get(),
        channel.get()
    ))
}

async fn whitelist_remove(
    state: &AppState,
    guild_id: Id<GuildMarker>,
    opts: &[CommandDataOption],
) -> Result<String> {
    let channel = option_channel(opts, "channel")?;
    let role = option_role(opts, "role")?;

    let mut roles = read_whitelist(state, guild_id, channel).await?;
    let role_str = role.get().to_string();
    let before = roles.len();
    roles.retain(|r| r != &role_str);
    if roles.len() == before {
        return Ok(format!("Role <@&{}> was not in the whitelist.", role.get()));
    }
    write_whitelist(state, guild_id, channel, &roles).await?;

    Ok(format!(
        "Role <@&{}> removed from <#{}> whitelist.",
        role.get(),
        channel.get()
    ))
}

async fn whitelist_list(
    state: &AppState,
    guild_id: Id<GuildMarker>,
    opts: &[CommandDataOption],
) -> Result<String> {
    let channel = option_channel(opts, "channel")?;
    let roles = read_whitelist(state, guild_id, channel).await?;

    if roles.is_empty() {
        return Ok(format!("No exempted roles for <#{}>.", channel.get()));
    }

    let mut out = format!("**Exempted roles for <#{}>:**\n", channel.get());
    for role in roles {
        out.push_str(&format!("• <@&{role}>\n"));
    }
    Ok(out)
}

async fn read_whitelist(
    state: &AppState,
    guild_id: Id<GuildMarker>,
    channel: Id<ChannelMarker>,
) -> Result<Vec<String>> {
    let json: Option<String> = sqlx::query_scalar(
        "SELECT whitelist_role_ids FROM honeypot_channels
         WHERE guild_id = ? AND channel_id = ?",
    )
    .bind(guild_id.get() as i64)
    .bind(channel.get() as i64)
    .fetch_optional(&state.db)
    .await
    .context("read whitelist")?;

    let Some(json) = json else {
        return Err(anyhow!(
            "<#{}> is not configured as a honeypot.",
            channel.get()
        ));
    };

    let roles: Vec<String> =
        serde_json::from_str(&json).context("parse whitelist_role_ids JSON")?;
    Ok(roles)
}

async fn write_whitelist(
    state: &AppState,
    guild_id: Id<GuildMarker>,
    channel: Id<ChannelMarker>,
    roles: &[String],
) -> Result<()> {
    let json = serde_json::to_string(roles).context("serialize whitelist")?;
    sqlx::query(
        "UPDATE honeypot_channels SET whitelist_role_ids = ?
         WHERE guild_id = ? AND channel_id = ?",
    )
    .bind(json)
    .bind(guild_id.get() as i64)
    .bind(channel.get() as i64)
    .execute(&state.db)
    .await
    .context("update whitelist")?;
    Ok(())
}
