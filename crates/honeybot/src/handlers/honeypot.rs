//! Reacts to `MessageCreate` in a configured honeypot channel: send a DM to
//! the author, perform the configured action (ban/kick/timeout), and post a
//! notification embed in the per-guild notification channel if one is set.

use anyhow::{Context, Result};
use fluent::FluentArgs;
use sqlx::FromRow;
use std::sync::Arc;
use tracing::{info, warn};
use twilight_model::channel::Message;
use twilight_model::id::Id;
use twilight_model::id::marker::ChannelMarker;

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

    action
        .execute(
            &*state.actions,
            guild_id,
            msg.author.id,
            "honeybot: honeypot trigger",
        )
        .await?;

    if let Some(notif) = guild.notification_channel_id {
        let channel = Id::<ChannelMarker>::new(notif as u64);
        let content = format!(
            "Honeypot triggered: <@{}> in <#{}> — action: {}",
            msg.author.id.get(),
            msg.channel_id.get(),
            action
        );
        if let Err(err) = state.actions.post_message(channel, &content).await {
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
    state.actions.send_dm(msg.author.id, &text).await
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

    // ── End-to-end via RecordingActions ────────────────────────────────────
    //
    // We drive the real `on_message` against a live in-memory SQLite, but
    // swap the `ModerationActions` impl for a mock that records every
    // call. The mock has no HTTP transport at all, so tests stay
    // deterministic, fast, and don't require a real Discord token.

    use crate::actions::ModerationActions;
    use crate::actions::test_support::{Call, RecordingActions};
    use crate::bot::AppState;
    use sqlx::SqlitePool;
    use std::sync::OnceLock;
    use twilight_http::Client as HttpClient;

    const TEST_GUILD_ID: u64 = 1_000_000_000_000_000_001;
    const TEST_HONEYPOT_CHANNEL: u64 = 1_000_000_000_000_000_002;
    const TEST_NOTIF_CHANNEL: u64 = 1_000_000_000_000_000_003;
    const TEST_VICTIM: u64 = 1_000_000_000_000_000_004;

    async fn fixture_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    async fn insert_honeypot(pool: &SqlitePool, action: &str, duration_s: Option<i64>) {
        sqlx::query("INSERT INTO guilds (id, locale, notification_channel_id) VALUES (?, 'en', ?)")
            .bind(TEST_GUILD_ID as i64)
            .bind(TEST_NOTIF_CHANNEL as i64)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO honeypot_channels (guild_id, channel_id, action, action_duration_s)
             VALUES (?, ?, ?, ?)",
        )
        .bind(TEST_GUILD_ID as i64)
        .bind(TEST_HONEYPOT_CHANNEL as i64)
        .bind(action)
        .bind(duration_s)
        .execute(pool)
        .await
        .unwrap();
    }

    fn synthetic_message(channel_id: u64, role_ids: &[u64]) -> Message {
        let roles: Vec<String> = role_ids.iter().map(|r| r.to_string()).collect();
        serde_json::from_value(serde_json::json!({
            "id": "9999999999999999",
            "channel_id": channel_id.to_string(),
            "guild_id": TEST_GUILD_ID.to_string(),
            "author": {
                "id": TEST_VICTIM.to_string(),
                "username": "victim",
                "discriminator": "0",
                "global_name": null,
                "avatar": null,
                "bot": false,
                "system": false,
                "public_flags": 0
            },
            "member": {
                "roles": roles,
                "joined_at": "2026-04-01T00:00:00.000000+00:00",
                "deaf": false,
                "mute": false,
                "flags": 0
            },
            "content": "anything",
            "timestamp": "2026-04-28T00:00:00.000000+00:00",
            "edited_timestamp": null,
            "tts": false,
            "mention_everyone": false,
            "mentions": [],
            "mention_roles": [],
            "attachments": [],
            "embeds": [],
            "pinned": false,
            "flags": 0,
            "type": 0,
            "components": []
        }))
        .expect("synthetic Message JSON must deserialize")
    }

    fn make_state(pool: SqlitePool, recording: Arc<RecordingActions>) -> Arc<AppState> {
        let actions: Arc<dyn ModerationActions> = recording;
        Arc::new(AppState {
            // Required by AppState but the honeypot path never touches it
            // since every IO goes through `actions`. The token is rejected
            // by Discord, but we never make a real call.
            http: Arc::new(HttpClient::new("Bot test_token".to_owned())),
            actions,
            db: pool,
            application_id: OnceLock::new(),
        })
    }

    #[tokio::test]
    async fn ban_action_invokes_ban_call_and_posts_notification() {
        let _ = i18n::init();
        let pool = fixture_pool().await;
        insert_honeypot(&pool, "ban", None).await;

        let recording = Arc::new(RecordingActions::new());
        let state = make_state(pool, recording.clone());

        let msg = synthetic_message(TEST_HONEYPOT_CHANNEL, &[]);
        on_message(state, msg).await.expect("handler succeeds");

        let calls = recording.calls();
        let banned = calls
            .iter()
            .any(|c| matches!(c, Call::Ban { user, .. } if *user == TEST_VICTIM));
        assert!(banned, "expected Ban call, got: {calls:?}");

        let notified = calls
            .iter()
            .any(|c| matches!(c, Call::Post { channel, .. } if *channel == TEST_NOTIF_CHANNEL));
        assert!(notified, "expected notification post, got: {calls:?}");

        let dmed = calls
            .iter()
            .any(|c| matches!(c, Call::Dm { user, .. } if *user == TEST_VICTIM));
        assert!(dmed, "expected pre-action DM, got: {calls:?}");
    }

    #[tokio::test]
    async fn message_outside_honeypot_channel_does_nothing() {
        let _ = i18n::init();
        let pool = fixture_pool().await;
        insert_honeypot(&pool, "ban", None).await;

        let recording = Arc::new(RecordingActions::new());
        let state = make_state(pool, recording.clone());

        let unrelated = TEST_HONEYPOT_CHANNEL + 999;
        let msg = synthetic_message(unrelated, &[]);
        on_message(state, msg).await.expect("handler succeeds");

        assert!(
            recording.calls().is_empty(),
            "no IO must happen for non-honeypot channels"
        );
    }

    #[tokio::test]
    async fn whitelisted_role_skips_action_and_makes_no_calls() {
        let _ = i18n::init();
        let pool = fixture_pool().await;
        insert_honeypot(&pool, "ban", None).await;

        // Mark role 42 as exempt for this honeypot.
        sqlx::query(
            "UPDATE honeypot_channels SET whitelist_role_ids = '[\"42\"]'
             WHERE guild_id = ? AND channel_id = ?",
        )
        .bind(TEST_GUILD_ID as i64)
        .bind(TEST_HONEYPOT_CHANNEL as i64)
        .execute(&pool)
        .await
        .unwrap();

        let recording = Arc::new(RecordingActions::new());
        let state = make_state(pool, recording.clone());

        // Author has the exempt role.
        let msg = synthetic_message(TEST_HONEYPOT_CHANNEL, &[42]);
        on_message(state, msg).await.expect("handler succeeds");

        assert!(
            recording.calls().is_empty(),
            "exempt members must not trigger any action, got: {:?}",
            recording.calls()
        );
    }

    #[tokio::test]
    async fn timeout_action_sends_timeout_call_with_configured_duration() {
        let _ = i18n::init();
        let pool = fixture_pool().await;
        insert_honeypot(&pool, "timeout", Some(900)).await;

        let recording = Arc::new(RecordingActions::new());
        let state = make_state(pool, recording.clone());

        let now = chrono::Utc::now().timestamp();
        let msg = synthetic_message(TEST_HONEYPOT_CHANNEL, &[]);
        on_message(state, msg).await.expect("handler succeeds");

        let calls = recording.calls();
        let timeout = calls.iter().find_map(|c| match c {
            Call::Timeout {
                until_unix_secs, ..
            } => Some(*until_unix_secs),
            _ => None,
        });
        let until = timeout.expect("expected a Timeout call");
        // The handler computed `now + 900`. Allow a small clock skew margin.
        assert!(
            (until - (now + 900)).abs() <= 5,
            "timeout target {until} should be ~{} (now+900s)",
            now + 900
        );
    }
}
