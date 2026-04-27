//! `/warn add|list|remove` and `/warn thresholds set|list` — strike system
//! and configurable auto-escalation. After every successful `add`, the
//! handler counts the user's open warns and applies any threshold action
//! whose `count` ≤ the new total.

use anyhow::{Context, Result, anyhow};
use std::sync::Arc;
use tracing::warn as warn_log;
use twilight_http::request::AuditLogReason;
use twilight_model::application::command::{Command, CommandType};
use twilight_model::application::interaction::Interaction;
use twilight_model::application::interaction::application_command::{
    CommandData, CommandDataOption, CommandOptionValue,
};
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};
use twilight_model::id::Id;
use twilight_model::id::marker::{ApplicationMarker, GuildMarker, UserMarker};
use twilight_model::util::Timestamp;
use twilight_util::builder::command::{
    CommandBuilder, IntegerBuilder, StringBuilder, SubCommandBuilder, SubCommandGroupBuilder,
    UserBuilder,
};

use crate::bot::AppState;

pub fn definition() -> Command {
    CommandBuilder::new(
        "warn",
        "Issue, list, remove warns, and configure auto-escalation thresholds.",
        CommandType::ChatInput,
    )
    .option(
        SubCommandBuilder::new("add", "Issue a warn to a member.")
            .option(UserBuilder::new("user", "The member to warn.").required(true))
            .option(StringBuilder::new("reason", "Why this warn is being issued.").required(true)),
    )
    .option(
        SubCommandBuilder::new("list", "List warns for a member.")
            .option(UserBuilder::new("user", "The member whose warns to list.").required(true)),
    )
    .option(
        SubCommandBuilder::new("remove", "Delete a single warn by id.")
            .option(IntegerBuilder::new("id", "Warn id, as shown by /warn list.").required(true)),
    )
    .option(
        SubCommandGroupBuilder::new("thresholds", "Configure auto-escalation thresholds.")
            .subcommands([
                SubCommandBuilder::new("set", "Set or replace a threshold.")
                    .option(
                        IntegerBuilder::new("count", "Number of warns that triggers the action.")
                            .required(true)
                            .min_value(1),
                    )
                    .option(
                        StringBuilder::new("action", "What to do at this threshold.")
                            .required(true)
                            .choices([
                                ("Timeout", "timeout".to_owned()),
                                ("Kick", "kick".to_owned()),
                                ("Ban", "ban".to_owned()),
                            ]),
                    )
                    .option(IntegerBuilder::new(
                        "duration_min",
                        "Timeout duration in minutes (only for timeout).",
                    )),
                SubCommandBuilder::new("list", "List configured thresholds."),
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

    let mod_id = interaction
        .member
        .as_ref()
        .and_then(|m| m.user.as_ref().map(|u| u.id))
        .or_else(|| interaction.user.as_ref().map(|u| u.id))
        .ok_or_else(|| anyhow!("missing actor identity"))?;

    let sub = command
        .options
        .first()
        .ok_or_else(|| anyhow!("missing subcommand"))?;

    let content = match (&sub.name[..], &sub.value) {
        ("add", CommandOptionValue::SubCommand(opts)) => {
            add(&state, guild_id, mod_id, opts).await?
        }
        ("list", CommandOptionValue::SubCommand(opts)) => list(&state, guild_id, opts).await?,
        ("remove", CommandOptionValue::SubCommand(opts)) => remove(&state, guild_id, opts).await?,
        ("thresholds", CommandOptionValue::SubCommandGroup(group)) => {
            thresholds(&state, guild_id, group).await?
        }
        (other, _) => format!("Unknown subcommand `{other}`."),
    };

    reply(&state, application_id, &interaction, &content).await
}

async fn add(
    state: &AppState,
    guild_id: Id<GuildMarker>,
    mod_id: Id<UserMarker>,
    options: &[CommandDataOption],
) -> Result<String> {
    let user_id = option_user(options, "user")?;
    let reason = option_string(options, "reason")?;

    sqlx::query("INSERT OR IGNORE INTO guilds (id) VALUES (?)")
        .bind(guild_id.get() as i64)
        .execute(&state.db)
        .await
        .context("upsert guild")?;

    sqlx::query("INSERT INTO warns (guild_id, user_id, mod_id, reason) VALUES (?, ?, ?, ?)")
        .bind(guild_id.get() as i64)
        .bind(user_id.get() as i64)
        .bind(mod_id.get() as i64)
        .bind(&reason)
        .execute(&state.db)
        .await
        .context("insert warn")?;

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM warns WHERE guild_id = ? AND user_id = ?")
            .bind(guild_id.get() as i64)
            .bind(user_id.get() as i64)
            .fetch_one(&state.db)
            .await
            .context("count warns")?;

    let mut summary = format!(
        "Warned <@{}> — `{}`. Total warns: {}.",
        user_id.get(),
        reason,
        count
    );

    if let Some(action) = applicable_threshold(state, guild_id, count).await? {
        match escalate(state, guild_id, user_id, &action).await {
            Ok(()) => {
                summary.push_str(&format!(
                    "\nThreshold reached — auto action: `{}`.",
                    describe_action(&action)
                ));
            }
            Err(err) => {
                warn_log!(?err, "auto-escalation failed");
                summary.push_str("\n⚠️ Threshold reached but escalation failed; check bot perms.");
            }
        }
    }

    Ok(summary)
}

async fn list(
    state: &AppState,
    guild_id: Id<GuildMarker>,
    options: &[CommandDataOption],
) -> Result<String> {
    let user_id = option_user(options, "user")?;

    let rows: Vec<(i64, i64, String, String)> = sqlx::query_as(
        "SELECT id, mod_id, reason, created_at
         FROM warns
         WHERE guild_id = ? AND user_id = ?
         ORDER BY created_at DESC, id DESC
         LIMIT 25",
    )
    .bind(guild_id.get() as i64)
    .bind(user_id.get() as i64)
    .fetch_all(&state.db)
    .await
    .context("list warns")?;

    if rows.is_empty() {
        return Ok(format!("<@{}> has no warns.", user_id.get()));
    }

    let mut out = format!("**Warns for <@{}>** ({}):\n", user_id.get(), rows.len());
    for (id, mod_id, reason, created_at) in rows {
        out.push_str(&format!(
            "• `#{id}` — by <@{mod_id}> on {created_at}: {reason}\n"
        ));
    }
    Ok(out)
}

async fn remove(
    state: &AppState,
    guild_id: Id<GuildMarker>,
    options: &[CommandDataOption],
) -> Result<String> {
    let id = option_int(options, "id")?;

    let result = sqlx::query("DELETE FROM warns WHERE id = ? AND guild_id = ?")
        .bind(id)
        .bind(guild_id.get() as i64)
        .execute(&state.db)
        .await
        .context("delete warn")?;

    if result.rows_affected() == 0 {
        Ok(format!("No warn with id `{id}` in this server."))
    } else {
        Ok(format!("Warn `#{id}` removed."))
    }
}

async fn thresholds(
    state: &AppState,
    guild_id: Id<GuildMarker>,
    group: &[CommandDataOption],
) -> Result<String> {
    let inner = group
        .first()
        .ok_or_else(|| anyhow!("missing thresholds subcommand"))?;
    let CommandOptionValue::SubCommand(opts) = &inner.value else {
        return Err(anyhow!("expected subcommand value under thresholds"));
    };

    match inner.name.as_str() {
        "set" => threshold_set(state, guild_id, opts).await,
        "list" => threshold_list(state, guild_id).await,
        other => Ok(format!("Unknown thresholds subcommand `{other}`.")),
    }
}

async fn threshold_set(
    state: &AppState,
    guild_id: Id<GuildMarker>,
    options: &[CommandDataOption],
) -> Result<String> {
    let count = option_int(options, "count")?;
    let action = option_string(options, "action")?;
    let duration_min = option_int_opt(options, "duration_min");

    let action_duration_s = if action == "timeout" {
        Some(duration_min.unwrap_or(60).max(1) * 60)
    } else {
        None
    };

    sqlx::query("INSERT OR IGNORE INTO guilds (id) VALUES (?)")
        .bind(guild_id.get() as i64)
        .execute(&state.db)
        .await
        .context("upsert guild")?;

    sqlx::query(
        "INSERT INTO warn_thresholds (guild_id, count, action, action_duration_s)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(guild_id, count) DO UPDATE SET
            action = excluded.action,
            action_duration_s = excluded.action_duration_s",
    )
    .bind(guild_id.get() as i64)
    .bind(count)
    .bind(&action)
    .bind(action_duration_s)
    .execute(&state.db)
    .await
    .context("upsert threshold")?;

    Ok(format!(
        "Threshold set: at {count} warns → `{}`.",
        describe_action(&ThresholdAction {
            action: action.clone(),
            duration_s: action_duration_s,
        })
    ))
}

async fn threshold_list(state: &AppState, guild_id: Id<GuildMarker>) -> Result<String> {
    let rows: Vec<(i64, String, Option<i64>)> = sqlx::query_as(
        "SELECT count, action, action_duration_s
         FROM warn_thresholds
         WHERE guild_id = ?
         ORDER BY count",
    )
    .bind(guild_id.get() as i64)
    .fetch_all(&state.db)
    .await
    .context("list thresholds")?;

    if rows.is_empty() {
        return Ok("No warn thresholds configured.".into());
    }

    let mut out = String::from("**Warn thresholds:**\n");
    for (count, action, duration_s) in rows {
        out.push_str(&format!(
            "• {count} warns → `{}`\n",
            describe_action(&ThresholdAction { action, duration_s })
        ));
    }
    Ok(out)
}

struct ThresholdAction {
    action: String,
    duration_s: Option<i64>,
}

fn describe_action(t: &ThresholdAction) -> String {
    match t.action.as_str() {
        "timeout" => format!("timeout {}m", t.duration_s.unwrap_or(3600) / 60),
        other => other.to_owned(),
    }
}

async fn applicable_threshold(
    state: &AppState,
    guild_id: Id<GuildMarker>,
    warn_count: i64,
) -> Result<Option<ThresholdAction>> {
    let row: Option<(String, Option<i64>)> = sqlx::query_as(
        "SELECT action, action_duration_s
         FROM warn_thresholds
         WHERE guild_id = ? AND count <= ?
         ORDER BY count DESC
         LIMIT 1",
    )
    .bind(guild_id.get() as i64)
    .bind(warn_count)
    .fetch_optional(&state.db)
    .await
    .context("find threshold")?;

    Ok(row.map(|(action, duration_s)| ThresholdAction { action, duration_s }))
}

async fn escalate(
    state: &AppState,
    guild_id: Id<GuildMarker>,
    user_id: Id<UserMarker>,
    t: &ThresholdAction,
) -> Result<()> {
    let reason = "honeybot: warn threshold reached";
    match t.action.as_str() {
        "ban" => {
            state
                .http
                .create_ban(guild_id, user_id)
                .reason(reason)
                .await
                .context("create ban")?;
        }
        "kick" => {
            state
                .http
                .remove_guild_member(guild_id, user_id)
                .reason(reason)
                .await
                .context("kick member")?;
        }
        "timeout" => {
            let secs = t.duration_s.unwrap_or(3600);
            let until_unix = chrono::Utc::now().timestamp() + secs;
            let until = Timestamp::from_secs(until_unix).context("invalid timestamp")?;
            state
                .http
                .update_guild_member(guild_id, user_id)
                .communication_disabled_until(Some(until))
                .reason(reason)
                .await
                .context("apply timeout")?;
        }
        other => {
            warn_log!(action = other, "unknown threshold action");
        }
    }
    Ok(())
}

fn option_user(options: &[CommandDataOption], name: &str) -> Result<Id<UserMarker>> {
    options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| match o.value {
            CommandOptionValue::User(id) => Some(id),
            _ => None,
        })
        .ok_or_else(|| anyhow!("missing user option `{name}`"))
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

fn option_int(options: &[CommandDataOption], name: &str) -> Result<i64> {
    option_int_opt(options, name).ok_or_else(|| anyhow!("missing integer option `{name}`"))
}

fn option_int_opt(options: &[CommandDataOption], name: &str) -> Option<i64> {
    options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| match o.value {
            CommandOptionValue::Integer(n) => Some(n),
            _ => None,
        })
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
