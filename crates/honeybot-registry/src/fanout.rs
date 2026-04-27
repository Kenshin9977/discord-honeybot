//! Per-pool tokio broadcast channels. New SSE subscribers attach to the
//! corresponding `Sender::subscribe()`; `bans::publish` calls `send` after
//! persisting the event to Postgres.
//!
//! v1 simplification: ban events are not durably queued — a subscriber that
//! disconnects misses any events that arrive while it is gone. Reconnecting
//! consumers replay missed events by paginating the `ban_events` table on
//! the next phase (using the `Last-Event-Id` SSE header).

use honeybot_proto::BanEvent;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::broadcast;
use uuid::Uuid;

const CHANNEL_CAPACITY: usize = 256;

#[derive(Default)]
pub struct Fanout {
    pools: Mutex<HashMap<Uuid, broadcast::Sender<BanEvent>>>,
}

impl Fanout {
    pub fn subscribe(&self, pool_id: Uuid) -> broadcast::Receiver<BanEvent> {
        let mut map = self.pools.lock().expect("fanout mutex poisoned");
        let sender = map
            .entry(pool_id)
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0);
        sender.subscribe()
    }

    /// Best-effort publish. Returns silently if no subscribers are attached.
    pub fn publish(&self, pool_id: Uuid, event: BanEvent) {
        let map = self.pools.lock().expect("fanout mutex poisoned");
        if let Some(sender) = map.get(&pool_id) {
            let _ = sender.send(event);
        }
    }
}
