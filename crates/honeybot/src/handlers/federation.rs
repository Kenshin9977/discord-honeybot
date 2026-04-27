//! Consumes ban events from the registry SSE stream and applies them per
//! the local subscription mode (auto_apply ⇒ ban now, alert_only ⇒ post embed
//! with Apply/Ignore buttons in the notification channel).
