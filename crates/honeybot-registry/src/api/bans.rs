//! `POST /pools/{id}/bans` — ingest a `PublishBanRequest`. Verifies HMAC,
//! rate-limits per publisher, persists the event, hands it to `fanout` for
//! distribution to subscribed SSE consumers.
//!
//! `POST /pools/{id}/bans/{ban_id}/dispute` — subscriber reports a false
//! positive; updates publisher reputation.
