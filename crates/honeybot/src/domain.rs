//! Domain types shared by the handler and command layers.
//!
//! `Action` replaces stringly-typed `"ban" | "kick" | "timeout"` matches that
//! used to live in three different files. Both the persisted SQLite shape
//! (`action TEXT`, `action_duration_s INTEGER`) and the slash-command
//! choices funnel through this enum, so adding a new action variant is one
//! match-exhaustiveness error away from compiling — not a multi-file
//! string-spelunking exercise.

use anyhow::{Result, anyhow};
use std::fmt;
use twilight_model::id::Id;
use twilight_model::id::marker::{GuildMarker, UserMarker};

use crate::actions::ModerationActions;

/// Default timeout when one isn't specified explicitly.
pub const DEFAULT_TIMEOUT_SECS: i64 = 3600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Ban,
    Kick,
    /// Discord member timeout, in seconds. Discord caps timeouts at 28 days.
    Timeout(i64),
}

impl Action {
    /// Reconstruct an `Action` from its persisted (kind, duration) pair.
    /// `duration_s` is ignored for kinds other than `timeout`, and falls
    /// back to `DEFAULT_TIMEOUT_SECS` if missing for a timeout.
    pub fn from_db(kind: &str, duration_s: Option<i64>) -> Result<Self> {
        match kind {
            "ban" => Ok(Self::Ban),
            "kick" => Ok(Self::Kick),
            "timeout" => Ok(Self::Timeout(duration_s.unwrap_or(DEFAULT_TIMEOUT_SECS))),
            other => Err(anyhow!("unknown action kind `{other}`")),
        }
    }

    /// The string written to the `action` column.
    pub fn kind_str(&self) -> &'static str {
        match self {
            Self::Ban => "ban",
            Self::Kick => "kick",
            Self::Timeout(_) => "timeout",
        }
    }

    /// The integer written to the `action_duration_s` column, or `None` for
    /// non-temporal actions.
    pub fn duration_secs(&self) -> Option<i64> {
        match self {
            Self::Timeout(s) => Some(*s),
            _ => None,
        }
    }

    /// Fluent message id for the DM sent before the action is applied.
    pub fn dm_key(&self) -> &'static str {
        match self {
            Self::Ban => "honeypot-ban-dm",
            Self::Kick => "honeypot-kick-dm",
            Self::Timeout(_) => "honeypot-timeout-dm",
        }
    }

    /// Dispatch this action through a `ModerationActions` impl. The single
    /// definition replaces what used to be a duplicated `match` in both the
    /// honeypot trigger handler and the warn auto-escalation path.
    pub async fn execute(
        self,
        actions: &dyn ModerationActions,
        guild: Id<GuildMarker>,
        user: Id<UserMarker>,
        reason: &str,
    ) -> Result<()> {
        match self {
            Self::Ban => actions.ban(guild, user, reason).await,
            Self::Kick => actions.kick(guild, user, reason).await,
            Self::Timeout(secs) => {
                let until = chrono::Utc::now().timestamp() + secs;
                actions.timeout(guild, user, until, reason).await
            }
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ban => f.write_str("ban"),
            Self::Kick => f.write_str("kick"),
            Self::Timeout(s) => write!(f, "timeout {}m", s / 60),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_db_round_trips_each_kind() {
        for action in [Action::Ban, Action::Kick, Action::Timeout(900)] {
            let parsed = Action::from_db(action.kind_str(), action.duration_secs()).unwrap();
            assert_eq!(parsed, action);
        }
    }

    #[test]
    fn from_db_rejects_unknown_kind() {
        assert!(Action::from_db("nuke", None).is_err());
    }

    #[test]
    fn timeout_without_duration_falls_back_to_default() {
        let parsed = Action::from_db("timeout", None).unwrap();
        assert_eq!(parsed, Action::Timeout(DEFAULT_TIMEOUT_SECS));
    }

    #[test]
    fn display_renders_timeout_in_minutes() {
        assert_eq!(Action::Timeout(3600).to_string(), "timeout 60m");
        assert_eq!(Action::Timeout(120).to_string(), "timeout 2m");
        assert_eq!(Action::Ban.to_string(), "ban");
        assert_eq!(Action::Kick.to_string(), "kick");
    }

    #[test]
    fn duration_secs_is_only_set_for_timeout() {
        assert_eq!(Action::Ban.duration_secs(), None);
        assert_eq!(Action::Kick.duration_secs(), None);
        assert_eq!(Action::Timeout(60).duration_secs(), Some(60));
    }
}
