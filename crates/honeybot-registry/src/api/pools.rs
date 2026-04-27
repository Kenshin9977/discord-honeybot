//! Pool CRUD and membership.
//!
//! All routes here are mounted behind `auth::require_token` and pull the
//! caller's bound `guild_id` from request extensions.

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use honeybot_proto::{Pool, PoolRole, PoolVisibility};
use rand::RngCore;
use serde::Deserialize;
use uuid::Uuid;

use crate::api::AppState;
use crate::auth::AuthGuildId;

#[derive(Debug, Deserialize)]
pub struct CreatePoolRequest {
    pub name: String,
    pub description: Option<String>,
    pub visibility: PoolVisibility,
}

#[derive(Debug, Deserialize)]
pub struct JoinPoolRequest {
    pub invite_code: String,
}

pub async fn create(
    State(state): State<AppState>,
    Extension(AuthGuildId(actor)): Extension<AuthGuildId>,
    Json(body): Json<CreatePoolRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    if body.name.trim().is_empty() || body.name.len() > 80 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let id = Uuid::new_v4();
    let invite_code = generate_invite_code();
    let visibility_db = match body.visibility {
        PoolVisibility::Public => "public",
        PoolVisibility::Private => "private",
    };

    let mut tx = state.db.begin().await.map_err(internal)?;

    sqlx::query(
        "INSERT INTO pools
            (id, name, description, visibility, invite_code, owner_guild_id)
         VALUES ($1, $2, $3, $4::pool_visibility, $5, $6)",
    )
    .bind(id)
    .bind(&body.name)
    .bind(&body.description)
    .bind(visibility_db)
    .bind(&invite_code)
    .bind(actor)
    .execute(&mut *tx)
    .await
    .map_err(internal)?;

    sqlx::query(
        "INSERT INTO pool_members (pool_id, guild_id, role)
         VALUES ($1, $2, 'owner'::pool_role)",
    )
    .bind(id)
    .bind(actor)
    .execute(&mut *tx)
    .await
    .map_err(internal)?;

    let created_at: DateTime<Utc> =
        sqlx::query_scalar("SELECT created_at FROM pools WHERE id = $1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .map_err(internal)?;

    tx.commit().await.map_err(internal)?;

    Ok(Json(Pool {
        id,
        name: body.name,
        description: body.description,
        visibility: body.visibility,
        invite_code,
        owner_guild_id: actor as u64,
        created_at,
    }))
}

pub async fn join(
    State(state): State<AppState>,
    Extension(AuthGuildId(actor)): Extension<AuthGuildId>,
    Json(body): Json<JoinPoolRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let row: Option<(Uuid, String)> =
        sqlx::query_as("SELECT id, visibility::text FROM pools WHERE invite_code = $1")
            .bind(&body.invite_code)
            .fetch_optional(&state.db)
            .await
            .map_err(internal)?;

    let Some((pool_id, visibility)) = row else {
        return Err(StatusCode::NOT_FOUND);
    };

    if visibility != "public" {
        // v1: private pools require an owner-side approval step that isn't
        // implemented yet; reject for now so we don't accidentally expose
        // private pools.
        return Err(StatusCode::FORBIDDEN);
    }

    sqlx::query(
        "INSERT INTO pool_members (pool_id, guild_id, role)
         VALUES ($1, $2, 'subscriber'::pool_role)
         ON CONFLICT (pool_id, guild_id) DO NOTHING",
    )
    .bind(pool_id)
    .bind(actor)
    .execute(&state.db)
    .await
    .map_err(internal)?;

    Ok(Json(serde_json::json!({
        "pool_id": pool_id,
        "role": "subscriber",
    })))
}

pub async fn get(
    State(state): State<AppState>,
    Extension(AuthGuildId(actor)): Extension<AuthGuildId>,
    Path(pool_id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
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
    .map_err(internal)?;

    if !is_member {
        return Err(StatusCode::NOT_FOUND);
    }

    type PoolRow = (
        Uuid,
        String,
        Option<String>,
        String,
        String,
        i64,
        DateTime<Utc>,
    );
    let pool: Option<PoolRow> = sqlx::query_as(
        "SELECT id, name, description, visibility::text, invite_code, owner_guild_id, created_at
         FROM pools WHERE id = $1",
    )
    .bind(pool_id)
    .fetch_optional(&state.db)
    .await
    .map_err(internal)?;

    let Some((id, name, description, visibility, invite_code, owner_guild_id, created_at)) = pool
    else {
        return Err(StatusCode::NOT_FOUND);
    };

    let members: Vec<(i64, String)> = sqlx::query_as(
        "SELECT guild_id, role::text FROM pool_members
         WHERE pool_id = $1
         ORDER BY joined_at",
    )
    .bind(pool_id)
    .fetch_all(&state.db)
    .await
    .map_err(internal)?;

    let visibility = match visibility.as_str() {
        "public" => PoolVisibility::Public,
        _ => PoolVisibility::Private,
    };

    let members_json = members
        .into_iter()
        .map(|(g, r)| {
            let role = match r.as_str() {
                "owner" => PoolRole::Owner,
                "publisher" => PoolRole::Publisher,
                _ => PoolRole::Subscriber,
            };
            serde_json::json!({ "guild_id": g, "role": role })
        })
        .collect::<Vec<_>>();

    Ok(Json(serde_json::json!({
        "pool": Pool {
            id,
            name,
            description,
            visibility,
            invite_code,
            owner_guild_id: owner_guild_id as u64,
            created_at,
        },
        "members": members_json,
    })))
}

fn generate_invite_code() -> String {
    let mut bytes = [0u8; 9];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn internal(err: sqlx::Error) -> StatusCode {
    tracing::warn!(?err, "registry db error");
    StatusCode::INTERNAL_SERVER_ERROR
}
