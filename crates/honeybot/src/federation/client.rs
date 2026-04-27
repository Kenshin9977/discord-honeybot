//! HTTP client for the federation registry (pool CRUD, ban publish, dispute).
//! Carries the per-guild API token in the `Authorization` header and HMAC-signs
//! the body of every `PublishBanRequest`.
