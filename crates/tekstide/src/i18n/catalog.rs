//! RFC-016 PR-016-B: Fluent catalog loading.
//!
//! The source-locale (`en`) catalog is compiled into the binary via
//! `include_str!`, so a missing or corrupt catalog file on disk can
//! never make the application unusable -- this module's `source_locale_
//! bundle` cannot fail. Additional locales load from disk, and can fail
//! (missing file, parse error); callers treat that as "this locale is
//! unavailable," never as a reason to fail startup.

use std::path::Path;

use fluent_bundle::{FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

pub(crate) type Bundle = FluentBundle<FluentResource>;

/// The source locale. Never translated by this RFC -- RFC-016 delivers
/// the machinery, not translations (§Non-Goals).
pub(crate) const SOURCE_LOCALE: &str = "en";

const SOURCE_LOCALE_FTL: &str = include_str!("../../locales/en.ftl");

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CatalogError {
    /// The locale identifier itself is not well-formed BCP 47.
    InvalidLocale,
    /// The FTL source did not parse. Carries the error count, not the
    /// errors themselves or any of the source text -- catalog content
    /// is not security- or privacy-sensitive, but there is no reason to
    /// carry more than a caller needs to log something useful.
    Parse(usize),
}

/// Builds the compiled-in source-locale bundle. Infallible in practice:
/// the source catalog is checked into the repository and covered by
/// [`super::tests::source_locale_catalog_parses`], so a build with a
/// broken `en.ftl` fails in CI/tests before it ever reaches this
/// `expect`, not at a user's runtime.
pub(crate) fn source_locale_bundle() -> Bundle {
    build_bundle(SOURCE_LOCALE, SOURCE_LOCALE_FTL)
        .expect("the compiled-in source-locale catalog must always parse")
}

/// Parses `ftl_text` as a Fluent resource for `locale` and builds a
/// bundle from it. The one function both the compiled-in source locale
/// and any disk-loaded additional locale go through, so both are held
/// to the same parsing standard.
pub(crate) fn build_bundle(locale: &str, ftl_text: &str) -> Result<Bundle, CatalogError> {
    let langid: LanguageIdentifier = locale.parse().map_err(|_| CatalogError::InvalidLocale)?;
    let resource = FluentResource::try_new(ftl_text.to_string())
        .map_err(|(_, errors)| CatalogError::Parse(errors.len()))?;
    let mut bundle = FluentBundle::new(vec![langid]);
    bundle
        .add_resource(resource)
        .map_err(|errors| CatalogError::Parse(errors.len()))?;
    Ok(bundle)
}

/// Loads `locale`'s catalog from `<locales_dir>/<locale>.ftl`. `None` on
/// any failure (missing file, unreadable, invalid locale id, parse
/// error) -- callers fall back to the next link in RFC-016's fallback
/// chain rather than treating a bad additional-locale file as fatal.
pub(crate) fn load_locale_from_disk(locales_dir: &Path, locale: &str) -> Option<Bundle> {
    let path = locales_dir.join(format!("{locale}.ftl"));
    let text = std::fs::read_to_string(path).ok()?;
    build_bundle(locale, &text).ok()
}
