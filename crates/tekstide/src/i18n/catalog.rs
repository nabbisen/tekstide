//! RFC-016 PR-016-B: Fluent catalog loading.
//!
//! **RFC-073 D1: every shipped catalog is compiled in, not loaded from
//! disk.** A path baked in at build time (`locales_dir()`, deleted by
//! this RFC) resolves on the machine that built the binary and nowhere
//! else -- the GitHub release binary, the macOS artifact and the AUR
//! `tekstide-bin` package all shipped a `locales_dir()` that pointed at
//! a path that does not exist on a user's machine, so every non-`en`
//! catalog silently fell back to English. `include_str!` each
//! `locales/*.ftl` the same way the source locale already was, selected
//! at runtime by name: the only way a catalog now fails to resolve is
//! that it was never shipped, which is a fact about the build, not
//! about the machine it runs on (RFC-073 D4).

use fluent_bundle::{FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

pub(crate) type Bundle = FluentBundle<FluentResource>;

/// The source locale. Never translated by this RFC -- RFC-016 delivers
/// the machinery, not translations (§Non-Goals).
pub(crate) const SOURCE_LOCALE: &str = "en";

const SOURCE_LOCALE_FTL: &str = include_str!("../../locales/en.ftl");

/// Every compiled-in catalog other than the source locale. RFC-073 D2:
/// an explicit list, not a build-script glob -- a build script is a
/// second mechanism to audit, complicates both published packages, and
/// hides the list from review. An explicit list is exactly the kind of
/// thing that goes stale, so `i18n::tests::
/// the_compiled_in_locale_list_matches_every_ftl_file_in_locales` gates
/// it against `locales/`'s own real contents; see that test's own doc
/// comment for the gate.
pub(crate) const ADDITIONAL_LOCALES: &[(&str, &str)] =
    &[("pl", include_str!("../../locales/pl.ftl"))];

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
/// `super::tests::source_locale_catalog_parses` (a `#[cfg(test)]` item,
/// not linkable from a normal `cargo doc` build), so a build with a
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

/// Looks `locale` up in the compiled-in [`ADDITIONAL_LOCALES`] list and
/// builds a bundle from it. `None` if `locale` is not one of the
/// catalogs this build carries, or if its compiled-in text somehow
/// failed to parse (covered by `tests::source_locale_catalog_parses`'s
/// sibling for every additional locale, so this should never actually
/// happen) -- callers fall back to the next link in RFC-016's fallback
/// chain either way.
pub(crate) fn load_compiled_locale(locale: &str) -> Option<Bundle> {
    let (_, ftl_text) = ADDITIONAL_LOCALES
        .iter()
        .find(|(name, _)| *name == locale)?;
    build_bundle(locale, ftl_text).ok()
}
