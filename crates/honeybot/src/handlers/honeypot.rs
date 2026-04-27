//! Reacts to `MessageCreate` in a configured honeypot channel: send a DM to
//! the author, perform the configured action (ban/kick/timeout), and post a
//! notification embed in the per-guild notification channel if one is set.

use anyhow::{Context, Result};
use fluent::FluentArgs;
use sqlx::FromRow;
use std::sync::Arc;
use tracing::{info, warn};
use twilight_http::request::AuditLogReason;
use twilight_model::channel::Message;
use twilight_model::id::Id;
use twilight_model::id::marker::{ChannelMarker, GuildMarker};
use twilight_model::util::Timestamp;

use crate::bot::AppState;
use crate::i18n;

#[derive(FromRow)]
struct HoneypotRow {
    action: String,
    action_duration_s: Option<i64>,
}

#[derive(FromRow)]
struct GuildRow {
    locale: String,
    notification_channel_id: Option<i64>,
}

pub async fn on_message(state: Arc<AppState>, msg: Message) -> Result<()> {
    let Some(guild_id) = msg.guild_id else {
        return Ok(()); // DMs and group DMs are out of scope
    };
    if msg.author.bot {
        return Ok(());
    }

    let guild_db = guild_id.get() as i64;
    let channel_db = msg.channel_id.get() as i64;

    let hp: Option<HoneypotRow> = sqlx::query_as(
        "SELECT action, action_duration_s
         FROM honeypot_channels
         WHERE guild_id = ? AND channel_id = ?",
    )
    .bind(guild_db)
    .bind(channel_db)
    .fetch_optional(&state.db)
    .await
    .context("query honeypot_channels")?;

    let Some(hp) = hp else {
        return Ok(()); // not a honeypot channel
    };

    let guild: GuildRow =
        sqlx::query_as("SELECT locale, notification_channel_id FROM guilds WHERE id = ?")
            .bind(guild_db)
            .fetch_optional(&state.db)
            .await
            .context("query guilds")?
            .unwrap_or(GuildRow {
                locale: "en".into(),
                notification_channel_id: None,
            });

    info!(
        guild_id = guild_id.get(),
        channel_id = msg.channel_id.get(),
        user_id = msg.author.id.get(),
        action = %hp.action,
        "honeypot triggered"
    );

    // Best-effort DM. Failure here must not block the action.
    if let Err(err) = send_dm(&state, &msg, &guild.locale, &hp.action).await {
        warn!(?err, "failed to DM user before honeypot action");
    }

    apply_action(&state, guild_id, msg.author.id, &hp).await?;

    if let Some(notif) = guild.notification_channel_id {
        let channel = Id::<ChannelMarker>::new(notif as u64);
        let content = format!(
            "Honeypot triggered: <@{}> in <#{}> — action: {}",
            msg.author.id.get(),
            msg.channel_id.get(),
            hp.action
        );
        if let Err(err) = state.http.create_message(channel).content(&content).await {
            warn!(?err, "failed to post honeypot notification");
        }
    }

    Ok(())
}

async fn send_dm(state: &AppState, msg: &Message, locale: &str, action: &str) -> Result<()> {
    let key = match action {
        "ban" => "honeypot-ban-dm",
        "kick" => "honeypot-kick-dm",
        "timeout" => "honeypot-timeout-dm",
        _ => "honeypot-ban-dm",
    };

    let mut args = FluentArgs::new();
    args.set(
        "guild",
        msg.guild_id.map(|g| g.get()).unwrap_or(0).to_string(),
    );
    args.set("channel", msg.channel_id.get().to_string());
    args.set("contact", "your server administrator");

    let text = i18n::get().t(locale, key, Some(&args));

    let dm_channel = state
        .http
        .create_private_channel(msg.author.id)
        .await
        .context("open DM channel")?
        .model()
        .await
        .context("decode DM channel")?;

    state
        .http
        .create_message(dm_channel.id)
        .content(&text)
        .await
        .context("send DM")?;

    Ok(())
}

async fn apply_action(
    state: &AppState,
    guild_id: Id<GuildMarker>,
    user_id: twilight_model::id::Id<twilight_model::id::marker::UserMarker>,
    hp: &HoneypotRow,
) -> Result<()> {
    let reason = "honeybot: honeypot trigger";

    match hp.action.as_str() {
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
            let secs = hp.action_duration_s.unwrap_or(3600);
            let until_unix = chrono::Utc::now().timestamp() + secs;
            let until = Timestamp::from_secs(until_unix).context("invalid timeout timestamp")?;
            state
                .http
                .update_guild_member(guild_id, user_id)
                .communication_disabled_until(Some(until))
                .reason(reason)
                .await
                .context("apply timeout")?;
        }
        other => {
            warn!(action = other, "unknown honeypot action");
        }
    }

    Ok(())
}
