//! `GET /pools/{id}/stream` — long-lived Server-Sent Events feed of new
//! `BanEvent`s for the authenticated guild's subscribed pool.
//!
//! v1 simplification: replay-on-reconnect (`Last-Event-Id`) is not yet
//! implemented; consumers that disconnect will miss any events that arrived
//! while they were gone.

use axum::{
    Extension,
    extract::{Path, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::Stream;
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

use crate::api::AppState;
use crate::auth::AuthGuildId;

pub async fn stream(
    State(state): State<AppState>,
    Extension(AuthGuildId(actor)): Extension<AuthGuildId>,
    Path(pool_id): Path<Uuid>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let is_member: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM pool_members
            WHERE pool_id = $1 AND guild_id = $2
        )",
    )
    .bind(pool_id)
    .bind(actor)
    .fetch_one(&state.db)
    .await
    .map_err(|err| {
        tracing::warn!(?err, "stream membership check failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if !is_member {
        return Err(StatusCode::FORBIDDEN);
    }

    let rx = state.fanout.subscribe(pool_id);

    let stream = BroadcastStream::new(rx).filter_map(|res| match res {
        Ok(event) => match serde_json::to_string(&event) {
            Ok(json) => Some(Ok(Event::default().id(event.id.to_string()).data(json))),
            Err(err) => {
                tracing::warn!(?err, "failed to serialise ban event");
                None
            }
        },
        // Subscriber lagged behind broadcast capacity — the next event is
        // newer than the one it expected. Drop and keep going.
        Err(_) => None,
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}
