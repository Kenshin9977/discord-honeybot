CREATE TYPE guild_status AS ENUM ('active', 'quarantined', 'banned');
CREATE TYPE pool_visibility AS ENUM ('public', 'private');
CREATE TYPE pool_role AS ENUM ('owner', 'publisher', 'subscriber');

CREATE TABLE guilds (
    id                BIGINT PRIMARY KEY,                  -- Discord guild id
    registered_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    reputation_score  INTEGER NOT NULL DEFAULT 100,
    status            guild_status NOT NULL DEFAULT 'active'
);

CREATE TABLE api_tokens (
    id           BIGSERIAL PRIMARY KEY,
    guild_id     BIGINT NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    token_hash   TEXT NOT NULL,                            -- argon2
    expires_at   TIMESTAMPTZ NOT NULL,
    revoked_at   TIMESTAMPTZ
);
CREATE INDEX api_tokens_guild_idx ON api_tokens(guild_id) WHERE revoked_at IS NULL;

CREATE TABLE pools (
    id               UUID PRIMARY KEY,
    name             TEXT NOT NULL,
    description      TEXT,
    visibility       pool_visibility NOT NULL,
    invite_code      TEXT NOT NULL UNIQUE,
    owner_guild_id   BIGINT NOT NULL REFERENCES guilds(id),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE pool_members (
    pool_id    UUID NOT NULL REFERENCES pools(id) ON DELETE CASCADE,
    guild_id   BIGINT NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    role       pool_role NOT NULL,
    joined_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (pool_id, guild_id)
);

CREATE TABLE ban_events (
    id                   UUID PRIMARY KEY,
    pool_id              UUID NOT NULL REFERENCES pools(id) ON DELETE CASCADE,
    publisher_guild_id   BIGINT NOT NULL REFERENCES guilds(id),
    target_user_id       BIGINT NOT NULL,
    reason               TEXT NOT NULL,
    evidence_hash        TEXT,
    signed_at            TIMESTAMPTZ NOT NULL,
    signature            TEXT NOT NULL,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX ban_events_pool_created_idx ON ban_events(pool_id, created_at DESC);
CREATE INDEX ban_events_publisher_idx ON ban_events(publisher_guild_id, created_at DESC);

CREATE TABLE ban_disputes (
    ban_event_id      UUID NOT NULL REFERENCES ban_events(id) ON DELETE CASCADE,
    disputer_guild_id BIGINT NOT NULL REFERENCES guilds(id),
    reason            TEXT NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (ban_event_id, disputer_guild_id)
);

CREATE TABLE audit (
    id              BIGSERIAL PRIMARY KEY,
    actor_guild_id  BIGINT REFERENCES guilds(id),
    action          TEXT NOT NULL,
    resource        TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
