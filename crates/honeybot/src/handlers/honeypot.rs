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
use crate::domain::Action;
use crate::i18n;

#[derive(FromRow)]
struct HoneypotRow {
    action: String,
    action_duration_s: Option<i64>,
    whitelist_role_ids: String,
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
        "SELECT action, action_duration_s, whitelist_role_ids
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

    let action = Action::from_db(&hp.action, hp.action_duration_s)
        .context("decode persisted honeypot action")?;

    let author_role_ids: Vec<u64> = msg
        .member
        .as_ref()
        .map(|m| m.roles.iter().map(|r| r.get()).collect())
        .unwrap_or_default();

    if whitelist_matches(&author_role_ids, &hp.whitelist_role_ids) {
        info!(
            guild_id = guild_id.get(),
            channel_id = msg.channel_id.get(),
            user_id = msg.author.id.get(),
            "honeypot skipped: author has exempted role"
        );
        return Ok(());
    }

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
        action = %action,
        "honeypot triggered"
    );

    // Best-effort DM. Failure here must not block the action.
    if let Err(err) = send_dm(&state, &msg, &guild.locale, action).await {
        warn!(?err, "failed to DM user before honeypot action");
    }

    apply_action(&state, guild_id, msg.author.id, action).await?;

    if let Some(notif) = guild.notification_channel_id {
        let channel = Id::<ChannelMarker>::new(notif as u64);
        let content = format!(
            "Honeypot triggered: <@{}> in <#{}> — action: {}",
            msg.author.id.get(),
            msg.channel_id.get(),
            action
        );
        if let Err(err) = state.http.create_message(channel).content(&content).await {
            warn!(?err, "failed to post honeypot notification");
        }
    }

    Ok(())
}

/// Returns true when any of the author's `member_role_ids` is listed in the
/// channel's `whitelist_role_ids` JSON. Closed by default: any parse failure
/// or empty whitelist returns false.
fn whitelist_matches(member_role_ids: &[u64], whitelist_json: &str) -> bool {
    let Ok(whitelisted): Result<Vec<String>, _> = serde_json::from_str(whitelist_json) else {
        return false;
    };
    if whitelisted.is_empty() || member_role_ids.is_empty() {
        return false;
    }
    member_role_ids
        .iter()
        .any(|r| whitelisted.iter().any(|w| w == &r.to_string()))
}

async fn send_dm(state: &AppState, msg: &Message, locale: &str, action: Action) -> Result<()> {
    let mut args = FluentArgs::new();
    args.set(
        "guild",
        msg.guild_id.map(|g| g.get()).unwrap_or(0).to_string(),
    );
    args.set("channel", msg.channel_id.get().to_string());
    args.set("contact", "your server administrator");

    let text = i18n::get().t(locale, action.dm_key(), Some(&args));

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
    action: Action,
) -> Result<()> {
    let reason = "honeybot: honeypot trigger";

    match action {
        Action::Ban => {
            state
                .http
                .create_ban(guild_id, user_id)
                .reason(reason)
                .await
                .context("create ban")?;
        }
        Action::Kick => {
            state
                .http
                .remove_guild_member(guild_id, user_id)
                .reason(reason)
                .await
                .context("kick member")?;
        }
        Action::Timeout(secs) => {
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
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitelist_empty_json_array_matches_nothing() {
        assert!(!whitelist_matches(&[1, 2, 3], "[]"));
    }

    #[test]
    fn whitelist_invalid_json_is_closed_by_default() {
        // A malformed `whitelist_role_ids` must NEVER let users through —
        // closed by default is the safer failure mode for a moderation rule.
        assert!(!whitelist_matches(&[1], "not json"));
        assert!(!whitelist_matches(&[1], "{\"oops\": true}"));
    }

    #[test]
    fn whitelist_member_with_no_roles_is_not_exempt() {
        assert!(!whitelist_matches(&[], r#"["42"]"#));
    }

    #[test]
    fn whitelist_matches_when_role_present() {
        assert!(whitelist_matches(&[10, 42, 99], r#"["42"]"#));
    }

    #[test]
    fn whitelist_does_not_match_when_role_absent() {
        assert!(!whitelist_matches(&[10, 99], r#"["42", "1000"]"#));
    }

    #[test]
    fn whitelist_handles_large_u64_role_ids() {
        // Discord snowflakes don't fit in i64 in the worst case; the JSON
        // round-trip stores them as strings precisely to keep precision.
        let big = u64::MAX - 7;
        let json = format!(r#"["{big}"]"#);
        assert!(whitelist_matches(&[big], &json));
    }
}
