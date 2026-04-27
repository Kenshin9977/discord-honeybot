//! Fluent-based i18n. Bundles are loaded from `i18n/<locale>.ftl` files and
//! looked up by message id at runtime.

use std::sync::OnceLock;

static INITIALIZED: OnceLock<()> = OnceLock::new();

pub fn init() {
    INITIALIZED.get_or_init(|| {
        // TODO: load fluent bundles from compiled-in `include_str!` resources
        // for `en` and `fr`, expose `t(locale, key, args)` helper.
    });
}
