//! Fluent-based i18n. Bundles are compiled into the binary from `i18n/*.ftl`
//! and selected at runtime by the per-guild `locale` column.

use anyhow::{Context, Result, anyhow};
use fluent::{FluentArgs, FluentResource};
use fluent_bundle::concurrent::FluentBundle;
use std::collections::HashMap;
use std::sync::OnceLock;
use unic_langid::LanguageIdentifier;

const EN_FTL: &str = include_str!("../i18n/en.ftl");
const FR_FTL: &str = include_str!("../i18n/fr.ftl");
const FALLBACK_LANG: &str = "en";

pub struct I18n {
    bundles: HashMap<LanguageIdentifier, FluentBundle<FluentResource>>,
    fallback: LanguageIdentifier,
}

impl I18n {
    pub fn load() -> Result<Self> {
        let mut bundles = HashMap::new();
        for (lang, src) in [("en", EN_FTL), ("fr", FR_FTL)] {
            let id: LanguageIdentifier = lang.parse().context("invalid language identifier")?;

            let resource = FluentResource::try_new(src.to_owned())
                .map_err(|(_, errs)| anyhow!("ftl parse errors in {lang}: {errs:?}"))?;

            let mut bundle = FluentBundle::new_concurrent(vec![id.clone()]);
            // Disable Unicode isolation marks; messages are interpolated into
            // plain Discord chat where `\u{2068}…\u{2069}` would render as
            // garbage characters.
            bundle.set_use_isolating(false);

            bundle
                .add_resource(resource)
                .map_err(|errs| anyhow!("ftl add errors in {lang}: {errs:?}"))?;

            bundles.insert(id, bundle);
        }
        let fallback: LanguageIdentifier = FALLBACK_LANG
            .parse()
            .context("fallback language id is invalid")?;
        Ok(Self { bundles, fallback })
    }

    pub fn t(&self, lang: &str, key: &str, args: Option<&FluentArgs>) -> String {
        let id: LanguageIdentifier = lang.parse().unwrap_or_else(|_| self.fallback.clone());

        let bundle = self
            .bundles
            .get(&id)
            .or_else(|| self.bundles.get(&self.fallback));

        if let Some(bundle) = bundle {
            if let Some(message) = bundle.get_message(key) {
                if let Some(pattern) = message.value() {
                    let mut errors = vec![];
                    let formatted = bundle.format_pattern(pattern, args, &mut errors);
                    if !errors.is_empty() {
                        tracing::warn!(?errors, key, lang, "i18n format errors");
                    }
                    return formatted.into_owned();
                }
            }
        }

        tracing::warn!(key, lang, "missing i18n key");
        key.to_owned()
    }
}

static GLOBAL: OnceLock<I18n> = OnceLock::new();

pub fn init() -> Result<&'static I18n> {
    if let Some(i18n) = GLOBAL.get() {
        return Ok(i18n);
    }
    let loaded = I18n::load()?;
    let _ = GLOBAL.set(loaded);
    GLOBAL
        .get()
        .ok_or_else(|| anyhow!("i18n global not initialised"))
}

pub fn get() -> &'static I18n {
    GLOBAL.get().expect("i18n::init() must run first")
}
