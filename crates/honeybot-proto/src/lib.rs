//! Wire types shared by `honeybot` (client) and `honeybot-registry` (server).
//!
//! These DTOs define the federation protocol over HTTPS + Server-Sent Events.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PoolVisibility {
    Public,
    Private,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PoolRole {
    Owner,
    Publisher,
    Subscriber,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionMode {
    AutoApply,
    AlertOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pool {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub visibility: PoolVisibility,
    pub invite_code: String,
    pub owner_guild_id: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanEvent {
    pub id: Uuid,
    pub pool_id: Uuid,
    pub publisher_guild_id: u64,
    pub target_user_id: u64,
    pub reason: String,
    pub evidence_hash: Option<String>,
    pub signed_at: DateTime<Utc>,
    pub signature: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishBanRequest {
    pub target_user_id: u64,
    pub reason: String,
    pub evidence_hash: Option<String>,
    pub signed_at: DateTime<Utc>,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeBanRequest {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterGuildRequest {
    pub guild_id: u64,
    pub triggering_user_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterGuildResponse {
    pub api_token: String,
    pub expires_at: DateTime<Utc>,
}
