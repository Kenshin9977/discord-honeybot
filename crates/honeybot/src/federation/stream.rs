//! Long-lived SSE consumer with exponential-backoff reconnect. Forwards every
//! incoming `BanEvent` to `handlers::federation`.
