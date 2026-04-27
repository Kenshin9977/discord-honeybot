//! `POST /pools/{id}/bans` — publish a ban event to a pool. The caller must
//! be a `publisher` or `owner` of the pool.
//!
//! v1: the registry assigns the event id and `signed_at`. HMAC signing is
//! deferred — TLS to the registry is the integrity boundary for now.

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use honeybot_proto::BanEvent;
use serde::Deserialize;
use uuid::Uuid;

use crate::api::AppState;
use crate::auth::AuthGuildId;

#[derive(Debug, Deserialize)]
pub struct PublishBody {
    pub target_user_id: u64,
    pub reason: String,
    pub evidence_hash: Option<String>,
}

pub async fn publish(
    State(state): State<AppState>,
    Extension(AuthGuildId(actor)): Extension<AuthGuildId>,
    Path(pool_id): Path<Uuid>,
    Json(body): Json<PublishBody>,
) -> Result<impl IntoResponse, StatusCode> {
    if body.reason.is_empty() || body.reason.len() > 1024 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Caller must be at least `publisher` in the pool. Subscribers cannot
    // publish.
    let role: Option<(String,)> = sqlx::query_as(
        "SELECT role::text FROM pool_members
         WHERE pool_id = $1 AND guild_id = $2",
    )
    .bind(pool_id)
    .bind(actor)
    .fetch_optional(&state.db)
    .await
    .map_err(internal)?;

    match role.as_ref().map(|r| r.0.as_str()) {
        Some("publisher" | "owner") => {}
        Some(_) => return Err(StatusCode::FORBIDDEN),
        None => return Err(StatusCode::NOT_FOUND),
    }

    let id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO ban_events
            (id, pool_id, publisher_guild_id, target_user_id,
             reason, evidence_hash, signed_at, signature)
         VALUES ($1, $2, $3, $4, $5, $6, $7, '')",
    )
    .bind(id)
    .bind(pool_id)
    .bind(actor)
    .bind(body.target_user_id as i64)
    .bind(&body.reason)
    .bind(&body.evidence_hash)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(internal)?;

    let event = BanEvent {
        id,
        pool_id,
        publisher_guild_id: actor as u64,
        target_user_id: body.target_user_id,
        reason: body.reason,
        evidence_hash: body.evidence_hash,
        signed_at: now,
        signature: String::new(),
        created_at: now,
    };

    state.fanout.publish(pool_id, event.clone());

    Ok(Json(event))
}

fn internal(err: sqlx::Error) -> StatusCode {
    tracing::warn!(?err, "registry db error");
    StatusCode::INTERNAL_SERVER_ERROR
}
