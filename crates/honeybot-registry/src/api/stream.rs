//! `GET /pools/{id}/stream` — long-lived Server-Sent Events feed of new
//! `BanEvent`s for the authenticated guild's subscribed pool. Replays events
//! since the `Last-Event-Id` header on reconnect.
