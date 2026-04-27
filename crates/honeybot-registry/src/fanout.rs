//! Per-pool tokio broadcast channels. New SSE subscribers attach to the
//! corresponding `Sender::subscribe()`; `bans::ingest` calls `send` after
//! persisting the event to Postgres.
