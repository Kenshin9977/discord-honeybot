//! `ModerationActions`: the side-effecting Discord operations the moderation
//! flow performs, behind a trait so handlers can be tested without a real
//! `twilight_http::Client` and a live HTTP transport.
//!
//! The production implementation `TwilightActions` wraps an `Arc<HttpClient>`
//! and translates each method into the appropriate twilight call. The
//! `tests` module ships a `RecordingActions` mock that just appends to a
//! call log, used by handler-level tests to assert that the right operation
//! was performed with the right arguments.

use anyhow::{Context, Result};
use async_trait::async_trait;
use std::sync::Arc;
use twilight_http::Client as HttpClient;
use twilight_http::request::AuditLogReason;
use twilight_model::id::Id;
use twilight_model::id::marker::{ChannelMarker, GuildMarker, UserMarker};
use twilight_model::util::Timestamp;

#[async_trait]
pub trait ModerationActions: Send + Sync {
    async fn ban(&self, guild: Id<GuildMarker>, user: Id<UserMarker>, reason: &str) -> Result<()>;

    async fn kick(&self, guild: Id<GuildMarker>, user: Id<UserMarker>, reason: &str) -> Result<()>;

    /// `until_unix_secs` is an absolute Unix timestamp; the bot computes it
    /// once at the call site. Discord caps timeouts at 28 days from now.
    async fn timeout(
        &self,
        guild: Id<GuildMarker>,
        user: Id<UserMarker>,
        until_unix_secs: i64,
        reason: &str,
    ) -> Result<()>;

    async fn send_dm(&self, user: Id<UserMarker>, content: &str) -> Result<()>;

    async fn post_message(&self, channel: Id<ChannelMarker>, content: &str) -> Result<()>;
}

pub struct TwilightActions {
    http: Arc<HttpClient>,
}

impl TwilightActions {
    pub fn new(http: Arc<HttpClient>) -> Self {
        Self { http }
    }
}

#[async_trait]
impl ModerationActions for TwilightActions {
    async fn ban(&self, guild: Id<GuildMarker>, user: Id<UserMarker>, reason: &str) -> Result<()> {
        self.http
            .create_ban(guild, user)
            .reason(reason)
            .await
            .context("create ban")?;
        Ok(())
    }

    async fn kick(&self, guild: Id<GuildMarker>, user: Id<UserMarker>, reason: &str) -> Result<()> {
        self.http
            .remove_guild_member(guild, user)
            .reason(reason)
            .await
            .context("remove guild member")?;
        Ok(())
    }

    async fn timeout(
        &self,
        guild: Id<GuildMarker>,
        user: Id<UserMarker>,
        until_unix_secs: i64,
        reason: &str,
    ) -> Result<()> {
        let until = Timestamp::from_secs(until_unix_secs).context("invalid timeout timestamp")?;
        self.http
            .update_guild_member(guild, user)
            .communication_disabled_until(Some(until))
            .reason(reason)
            .await
            .context("apply timeout")?;
        Ok(())
    }

    async fn send_dm(&self, user: Id<UserMarker>, content: &str) -> Result<()> {
        let dm_channel = self
            .http
            .create_private_channel(user)
            .await
            .context("open DM channel")?
            .model()
            .await
            .context("decode DM channel")?;

        self.http
            .create_message(dm_channel.id)
            .content(content)
            .await
            .context("send DM")?;

        Ok(())
    }

    async fn post_message(&self, channel: Id<ChannelMarker>, content: &str) -> Result<()> {
        self.http
            .create_message(channel)
            .content(content)
            .await
            .context("post message")?;
        Ok(())
    }
}

#[cfg(test)]
pub mod test_support {
    //! Recording mock used by handler tests. Each call is captured to a
    //! `Vec<Call>` accessible via [`RecordingActions::calls`]; methods all
    //! return `Ok(())` so tests can drive the handler without configuring
    //! response bodies.
    use super::*;
    use std::sync::Mutex;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Call {
        Ban {
            guild: u64,
            user: u64,
            reason: String,
        },
        Kick {
            guild: u64,
            user: u64,
            reason: String,
        },
        Timeout {
            guild: u64,
            user: u64,
            until_unix_secs: i64,
            reason: String,
        },
        Dm {
            user: u64,
            content: String,
        },
        Post {
            channel: u64,
            content: String,
        },
    }

    #[derive(Default)]
    pub struct RecordingActions {
        calls: Mutex<Vec<Call>>,
    }

    impl RecordingActions {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn calls(&self) -> Vec<Call> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl ModerationActions for RecordingActions {
        async fn ban(
            &self,
            guild: Id<GuildMarker>,
            user: Id<UserMarker>,
            reason: &str,
        ) -> Result<()> {
            self.calls.lock().unwrap().push(Call::Ban {
                guild: guild.get(),
                user: user.get(),
                reason: reason.to_owned(),
            });
            Ok(())
        }

        async fn kick(
            &self,
            guild: Id<GuildMarker>,
            user: Id<UserMarker>,
            reason: &str,
        ) -> Result<()> {
            self.calls.lock().unwrap().push(Call::Kick {
                guild: guild.get(),
                user: user.get(),
                reason: reason.to_owned(),
            });
            Ok(())
        }

        async fn timeout(
            &self,
            guild: Id<GuildMarker>,
            user: Id<UserMarker>,
            until_unix_secs: i64,
            reason: &str,
        ) -> Result<()> {
            self.calls.lock().unwrap().push(Call::Timeout {
                guild: guild.get(),
                user: user.get(),
                until_unix_secs,
                reason: reason.to_owned(),
            });
            Ok(())
        }

        async fn send_dm(&self, user: Id<UserMarker>, content: &str) -> Result<()> {
            self.calls.lock().unwrap().push(Call::Dm {
                user: user.get(),
                content: content.to_owned(),
            });
            Ok(())
        }

        async fn post_message(&self, channel: Id<ChannelMarker>, content: &str) -> Result<()> {
            self.calls.lock().unwrap().push(Call::Post {
                channel: channel.get(),
                content: content.to_owned(),
            });
            Ok(())
        }
    }
}
