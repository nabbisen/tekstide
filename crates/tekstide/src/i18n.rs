//! RFC-016 PR-016-B: catalog lookup, locale selection, and fallback.
//!
//! **A note on sequencing, not a silent assumption.**
//! `implementation-handoff.md` describes this slice as replacing a
//! placeholder `i18n.rs` that RFC-015 was to create first, with an
//! explicit instruction not to change the call shape RFC-015's shell
//! code would already be calling. As of this slice, RFC-015 has not
//! been implemented -- `crates/tekstide/src` had no module beyond
//! `main.rs` before this file, so there is no placeholder to replace
//! and no existing call shape to preserve. This module therefore
//! *establishes* the call shape ([`Catalog`], [`Catalog::get`]) rather
//! than conforming to one already fixed. Keep it stable once RFC-015's
//! shell code exists and calls it.
//!
//! **Locale selection precedence**, resolved once at startup (runtime
//! switching is out of scope for M8):
//!
//! 1. Explicit CLI flag
//! 2. Configuration setting (RFC-023 supplies this; not available yet,
//!    so [`LocalePreference::configured`] is always `None` until then)
//! 3. OS locale (via `sys_locale::get_locale`)
//! 4. Source locale (`en`)
//!
//! **Fallback chain**, per key lookup: requested locale -> requested
//! language without region -> source locale (`en`) -> the key itself.
//! Never blank, never panics -- a missing key renders as the key,
//! which is ugly and immediately obvious, the correct failure mode for
//! a translation gap.

mod catalog;

use std::path::Path;

use unic_langid::LanguageIdentifier;

/// Locale preference inputs above OS-locale detection, in precedence
/// order. Both fields are `None` today (no CLI flag parsing exists yet
/// in `main.rs`, and RFC-023's configuration setting does not exist) --
/// the fields exist so [`Catalog::resolve`]'s signature does not need to
/// change when those callers arrive.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LocalePreference {
    pub cli_flag: Option<String>,
    pub configured: Option<String>,
}

/// The resolved catalog for one process lifetime. Constructed once via
/// [`Catalog::resolve`]; [`Catalog::get`] is the lookup every trusted
/// surface calls for user-facing chrome text.
pub struct Catalog {
    resolved: catalog::Bundle,
    resolved_locale: String,
    /// The source-locale bundle, held separately for the fallback chain
    /// -- `None` when `resolved_locale` already IS the source locale,
    /// since falling back to itself would be redundant.
    source_fallback: Option<catalog::Bundle>,
}

impl Catalog {
    /// Resolves the active locale per the precedence order documented
    /// on the module, then builds the fallback-ready catalog.
    /// `locales_dir` is where additional (non-source) locale `.ftl`
    /// files are looked for; `None` means no additional locales can be
    /// found on disk (every request falls through to the source
    /// locale) -- a legitimate configuration, not an error.
    pub fn resolve(preference: LocalePreference, locales_dir: Option<&Path>) -> Self {
        let requested = preference
            .cli_flag
            .or(preference.configured)
            .or_else(sys_locale::get_locale)
            .unwrap_or_else(|| catalog::SOURCE_LOCALE.to_string());
        Self::resolve_fallback_chain(&requested, locales_dir)
    }

    fn resolve_fallback_chain(requested: &str, locales_dir: Option<&Path>) -> Self {
        if let Some(bundle) = Self::load(requested, locales_dir) {
            return Self::with_resolved(requested.to_string(), bundle);
        }
        if let Some(language) = language_subtag(requested)
            && language != requested
            && let Some(bundle) = Self::load(&language, locales_dir)
        {
            return Self::with_resolved(language, bundle);
        }
        Self::with_resolved(
            catalog::SOURCE_LOCALE.to_string(),
            catalog::source_locale_bundle(),
        )
    }

    fn load(locale: &str, locales_dir: Option<&Path>) -> Option<catalog::Bundle> {
        if locale == catalog::SOURCE_LOCALE {
            return Some(catalog::source_locale_bundle());
        }
        locales_dir.and_then(|dir| catalog::load_locale_from_disk(dir, locale))
    }

    fn with_resolved(locale: String, bundle: catalog::Bundle) -> Self {
        let source_fallback = if locale == catalog::SOURCE_LOCALE {
            None
        } else {
            Some(catalog::source_locale_bundle())
        };
        Self {
            resolved: bundle,
            resolved_locale: locale,
            source_fallback,
        }
    }

    /// The locale this catalog actually resolved to -- may differ from
    /// what was requested, per the fallback chain.
    pub fn resolved_locale(&self) -> &str {
        &self.resolved_locale
    }

    /// Looks up `key`: resolved locale -> source locale -> the key
    /// itself. Never blank, never panics. A miss at any stage past the
    /// first is logged at debug level (development builds only) --
    /// never as an audit event, since a missing translation is not a
    /// security decision.
    pub fn get(&self, key: &str) -> String {
        if let Some(value) = Self::lookup_in(&self.resolved, key) {
            return value;
        }
        if let Some(source) = &self.source_fallback
            && let Some(value) = Self::lookup_in(source, key)
        {
            log_missing_key(key, &self.resolved_locale);
            return value;
        }
        log_missing_key(key, catalog::SOURCE_LOCALE);
        key.to_string()
    }

    fn lookup_in(bundle: &catalog::Bundle, key: &str) -> Option<String> {
        let message = bundle.get_message(key)?;
        let pattern = message.value()?;
        let mut errors = vec![];
        let value = bundle.format_pattern(pattern, None, &mut errors);
        if errors.is_empty() {
            Some(value.into_owned())
        } else {
            None
        }
    }
}

/// The requested locale's language subtag alone (e.g. `fr` from
/// `fr-CA`) -- the second link in the fallback chain. `None` if
/// `locale` does not even parse as a language identifier.
fn language_subtag(locale: &str) -> Option<String> {
    let langid: LanguageIdentifier = locale.parse().ok()?;
    Some(langid.language.as_str().to_string())
}

fn log_missing_key(key: &str, fallback_locale_used: &str) {
    if cfg!(debug_assertions) {
        eprintln!(
            "[i18n] missing key `{key}`, rendered via `{fallback_locale_used}` or the key itself"
        );
    }
}

#[cfg(test)]
mod tests;
