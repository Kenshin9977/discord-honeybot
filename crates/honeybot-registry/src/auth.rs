//! Token verification middleware: extracts `Authorization: Bearer ...`, looks
//! up the token hash, returns the bound `guild_id` to handlers as a request
//! extension. Also performs Discord-API checks at registration time.
