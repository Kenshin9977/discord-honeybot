//! `POST /auth/register` — first contact for a guild. Mints a per-guild API
//! token (SHA-256 hashed at rest) and ensures the guild row exists.
//!
//! For v1 this trusts whatever `guild_id` the caller declares. A future
//! revision will verify via the Discord API (using
//! `DISCORD_VERIFICATION_TOKEN`) that the requesting bot is in the claimed
//! guild and that the triggering user has `MANAGE_GUILD`.

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use chrono::{Duration, Utc};
use honeybot_proto::{RegisterGuildRequest, RegisterGuildResponse};

use crate::api::AppState;
use crate::auth::issue_token;

const TOKEN_LIFETIME_DAYS: i64 = 90;

pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterGuildRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let guild_id = body.guild_id as i64;
    let expires_at = Utc::now() + Duration::days(TOKEN_LIFETIME_DAYS);
    let (plaintext, hash) = issue_token();

    let mut tx = state.db.begin().await.map_err(internal)?;

    sqlx::query("INSERT INTO guilds (id) VALUES ($1) ON CONFLICT (id) DO NOTHING")
        .bind(guild_id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;

    sqlx::query("INSERT INTO api_tokens (guild_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(guild_id)
        .bind(&hash)
        .bind(expires_at)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;

    tx.commit().await.map_err(internal)?;

    Ok(Json(RegisterGuildResponse {
        api_token: plaintext,
        expires_at,
    }))
}

fn internal(err: sqlx::Error) -> StatusCode {
    tracing::warn!(?err, "registry db error");
    StatusCode::INTERNAL_SERVER_ERROR
}
