//! Token issuance, hashing, and Bearer-auth middleware.
//!
//! Tokens are 32 random bytes shown to the caller exactly once at
//! registration; only the SHA-256 hash is persisted. SHA-256 is sufficient
//! because tokens are high-entropy random, not user-chosen passwords.

use anyhow::Result;
use axum::{
    extract::{Request, State},
    http::{StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::Response,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::api::AppState;

/// Marker inserted into request extensions by the auth middleware so handlers
/// can recover the calling guild without re-parsing the header.
#[derive(Debug, Clone, Copy)]
pub struct AuthGuildId(pub i64);

/// Generate a fresh `(plaintext_token, hash)` pair. The plaintext is returned
/// to the caller exactly once; only the hash is stored.
pub fn issue_token() -> (String, String) {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let plaintext = URL_SAFE_NO_PAD.encode(bytes);
    let hash = hash_token(&plaintext);
    (plaintext, hash)
}

pub fn hash_token(plaintext: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(plaintext.as_bytes());
    hex::encode(hasher.finalize())
}

/// Tower middleware that requires `Authorization: Bearer <token>` on every
/// request. Looks up the token by its SHA-256 hash, verifies it is not
/// expired or revoked, and attaches the bound `guild_id` to request
/// extensions for handlers to read.
pub async fn require_token(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?
        .trim();
    if token.is_empty() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let hash = hash_token(token);

    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT guild_id
         FROM api_tokens
         WHERE token_hash = $1
           AND revoked_at IS NULL
           AND expires_at > now()
         LIMIT 1",
    )
    .bind(&hash)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        tracing::warn!(?err, "auth token lookup failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let Some((guild_id,)) = row else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    req.extensions_mut().insert(AuthGuildId(guild_id));
    Ok(next.run(req).await)
}
