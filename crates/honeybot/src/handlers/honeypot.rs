//! Reacts to `MessageCreate` in a configured honeypot channel: send DM to the
//! author, perform the configured action (ban/kick/timeout), post a notification
//! embed, and — if a publishing pool is configured — emit a federation ban event.
