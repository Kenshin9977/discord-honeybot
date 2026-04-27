//! `POST /auth/register` — first contact for a guild. Verifies via Discord API
//! that the bot is in the claimed guild and that the triggering user has
//! `MANAGE_GUILD`, then mints a per-guild API token (Argon2-hashed at rest).
//!
//! `POST /auth/refresh` — rotate an existing token before expiry.
