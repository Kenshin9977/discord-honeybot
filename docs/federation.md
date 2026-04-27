# Federation

> Status: planning document. The protocol is specified here; the code is not
> yet wired up.

## Why

A user banned from server A for raid behaviour is, statistically, the same
user about to raid servers B, C, D. Discord audit logs are per-server; there
is no native cross-server signal. Federated pools fill that gap.

## Concepts

A **pool** is a group of Discord guilds that share ban events.

- **Visibility** — `public` (anyone with the invite code can join) or
  `private` (owner must approve).
- **Roles**
  - `owner` — created the pool, can revoke members.
  - `publisher` — may publish ban events.
  - `subscriber` — receives ban events; cannot publish.
- A guild may belong to many pools simultaneously.

A subscribing guild chooses a **mode** per pool:

- `auto_apply` — incoming bans are applied immediately.
- `alert_only` — incoming bans land in the notification channel as embeds
  with `Apply` / `Ignore` buttons; a moderator decides.

Optional filter: `min_reputation` — ignore publishers below a score.

## Wire protocol (HTTPS to the registry)

| Method | Path | Description |
|---|---|---|
| `POST` | `/auth/register` | First-contact registration; mints an API token. Server verifies via Discord API that the requesting bot is in the claimed guild and the user has `MANAGE_GUILD`. |
| `POST` | `/auth/refresh` | Rotate token before `expires_at`. |
| `POST` | `/pools` | Create a pool. |
| `POST` | `/pools/join` | Join via invite code. |
| `GET`  | `/pools/{id}` | Pool info + members (visible to members). |
| `DELETE` | `/pools/{id}/members/{guild}` | Owner-only revoke. |
| `POST` | `/pools/{id}/bans` | Publish a ban event (publisher role required). |
| `POST` | `/pools/{id}/bans/{ban}/dispute` | Subscriber reports a false positive. |
| `GET`  | `/pools/{id}/stream` | Server-Sent Events feed for the calling guild's subscribed pool. Honors `Last-Event-Id` for resume. |

All authenticated calls use `Authorization: Bearer <api_token>`. The token is
guild-scoped; the registry rejects requests where the path's `guild_id`
doesn't match the bound guild.

`PublishBanRequest` bodies are HMAC-signed with a secret derived from the
token, ensuring that even if a token is leaked at rest it can't be replayed
out of context.

## Anti-abuse

- **Rate limit** — default 30 publishes/hour/guild, tunable per pool by the
  owner.
- **Reputation** — every guild starts at 100. Each accepted dispute against
  a publisher reduces its reputation; below 30 it is auto-quarantined and
  publishes are dropped. Subscribers can independently set
  `min_reputation` filters.
- **Owner controls** — pool owners may kick any member at any time.
- **Registry-wide** — registry administrators can mark a guild `banned`,
  rejecting all its calls.
- **Privacy** — ban events contain a Discord user id (already
  publicly-correlatable) and a free-text reason. Reasons should not include
  PII; the registry exposes `DELETE /bans/{id}` for right-to-erasure
  requests, and the official instance purges events older than 365 days.

## Trust models you can build on top

- One pool per language / region community.
- Vetted "raid alert" pools where membership is by application only.
- Internal pools for an organisation that runs many servers.
