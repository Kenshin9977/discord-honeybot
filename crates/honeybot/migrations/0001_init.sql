-- Per-guild config and runtime state for the bot.

CREATE TABLE guilds (
    id                       INTEGER PRIMARY KEY,           -- Discord guild id (u64 fits in i64 once we mask)
    locale                   TEXT    NOT NULL DEFAULT 'en',
    notification_channel_id  INTEGER,
    registry_token           TEXT,                          -- nullable when federation is off
    created_at               TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE honeypot_channels (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id            INTEGER NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    channel_id          INTEGER NOT NULL,
    action              TEXT    NOT NULL CHECK (action IN ('ban', 'kick', 'timeout')),
    action_duration_s   INTEGER,                            -- only for timeout
    dm_template         TEXT,                               -- nullable ⇒ use locale default
    whitelist_role_ids  TEXT    NOT NULL DEFAULT '[]',      -- JSON array of u64
    created_at          TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE (guild_id, channel_id)
);

CREATE TABLE warns (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id    INTEGER NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    user_id     INTEGER NOT NULL,
    mod_id      INTEGER NOT NULL,
    reason      TEXT    NOT NULL,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX warns_guild_user_idx ON warns(guild_id, user_id);

CREATE TABLE warn_thresholds (
    guild_id          INTEGER NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    count             INTEGER NOT NULL,
    action            TEXT    NOT NULL CHECK (action IN ('timeout', 'kick', 'ban')),
    action_duration_s INTEGER,
    PRIMARY KEY (guild_id, count)
);

CREATE TABLE pool_subscriptions (
    guild_id         INTEGER NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    pool_id          TEXT    NOT NULL,                     -- UUID
    mode             TEXT    NOT NULL CHECK (mode IN ('auto_apply', 'alert_only')),
    publish_enabled  INTEGER NOT NULL DEFAULT 0,
    min_reputation   INTEGER NOT NULL DEFAULT 0,
    joined_at        TEXT    NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (guild_id, pool_id)
);

CREATE TABLE pending_remote_bans (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id             INTEGER NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    pool_id              TEXT    NOT NULL,
    ban_event_id         TEXT    NOT NULL,                 -- UUID
    target_user_id       INTEGER NOT NULL,
    reason               TEXT    NOT NULL,
    publisher_guild_id   INTEGER NOT NULL,
    received_at          TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE (guild_id, ban_event_id)
);
