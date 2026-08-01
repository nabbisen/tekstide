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
//!
//! **Interpolation ([`Catalog::get_with_args`]) is for trusted data;
//! untrusted text must go through `tekstide_core::text_safety` first --
//! [`CatalogArgs`] makes that a type-level fact, not a documentation
//! promise (response 125 Required 1).** An earlier version of this
//! module publicly re-exported `fluent_bundle::{FluentArgs, FluentValue}`
//! directly, which meant `FluentValue::from(any_str)` compiled for any
//! runtime string, untrusted or not -- probed and confirmed real: a
//! project name containing a live `U+202E` interpolated straight through,
//! indistinguishable from any other localized text, in the one place
//! Trojan Source (RFC-016's own motivating threat) is most likely to
//! reappear, since RFC-015 PR-015-D's first real caller renders project
//! names. `CatalogArgs` closes this the same way `DisplayText` (no public
//! constructor but `quote_untrusted`), `VerifiedCwd` (RFC-021), and
//! `RunCapabilityToken`'s narrowed accessor (RFC-021) all closed the same
//! shape of gap: `fluent_bundle::{FluentArgs, FluentValue}` are no longer
//! re-exported at all, so nothing outside this module can construct a
//! `FluentValue::String` from an arbitrary runtime `&str`. Every argument
//! must go through one of `CatalogArgs`'s three constructors, each
//! naming what it allows:
//!
//! - [`CatalogArgs::number`] -- a number. Cannot carry a directionality
//!   control; CLDR plural-category selection applies.
//! - [`CatalogArgs::untrusted`] -- takes `&text_safety::DisplayText`, not
//!   `&str`. The only way to obtain a `DisplayText` is `quote_untrusted`
//!   (`text_safety`'s own guarantee), so this constructor inherits that
//!   guarantee rather than re-implementing it.
//! - [`CatalogArgs::trusted_symbol`] -- takes `&'static str`. A
//!   compile-time literal (`CountDisplay`'s state names and similar)
//!   cannot be a project name, branch name, or anything else read at
//!   runtime -- the `'static` bound excludes attacker-influenceable data
//!   by construction, not by caller discipline. **This is a strong
//!   barrier, not an impossible one** (response 126): `String::leak`
//!   turns runtime data into `&'static str`, so a caller determined to
//!   launder untrusted text through the trusted path can. The bar this
//!   module holds to is "bypassing it requires deliberate effort," which
//!   leaking memory to defeat the boundary clears comfortably -- this is
//!   not a claim that `trusted_symbol` is unbypassable under any caller
//!   behavior whatsoever.
//!
//! Fluent itself treats every argument value as an opaque literal
//! regardless of which constructor produced it: it is never re-parsed as
//! FTL syntax (no markup injection is possible through an argument,
//! proven in `tests::interpolated_values_cannot_inject_fluent_syntax`).
//! What Fluent does *not* do on its own is escape bidi/format characters
//! -- that is what [`CatalogArgs::untrusted`] forces through
//! `text_safety` for.
//!
//! **A genuine, welcome property discovered along the way: Fluent wraps
//! every interpolated placeable in Unicode bidi isolate marks (First
//! Strong Isolate / Pop Directional Isolate) by default.** `text_safety::
//! quote_untrusted` also isolates, so a value that went through
//! `CatalogArgs::untrusted` ends up isolated twice -- legal (isolates
//! nest to depth 125) and harmless; this is accepted as a redundant but
//! inert mark pair, not worked around.
//!
//! **The `number`/`untrusted`/`trusted_symbol` split doubles as the
//! answer to "how does a caller express a non-numeric count?"**
//! `$count`-shaped selectors in this module's catalogs match either CLDR
//! plural categories (for a number, via [`CatalogArgs::number`]) or
//! literal string variants (for a symbol, via
//! [`CatalogArgs::trusted_symbol`]) in the *same* message, against the
//! *same* variable -- so a caller with a `tekstide_core::project_board::
//! CountDisplay`-shaped value (`KnownCount(u32)` plus three non-numeric
//! states) has exactly one lookup to make, not two. See
//! `locales/en.ftl`'s `blocked-automation-count` key for the pattern.

mod catalog;

use std::path::Path;

use fluent_bundle::{FluentArgs, FluentValue};
use tekstide_core::text_safety::DisplayText;
use unic_langid::LanguageIdentifier;

/// Numeric primitive types [`CatalogArgs::number`] accepts. Sealed
/// (private trait, cannot be implemented outside this module) so the
/// set is exactly "types `fluent_bundle::FluentValue` already treats as
/// numbers," never extensible to a string-shaped type by an outside
/// impl.
mod sealed {
    pub trait CatalogNumber: Copy {}
    macro_rules! impl_catalog_number {
        ($($ty:ty),+ $(,)?) => {
            $(impl CatalogNumber for $ty {})+
        };
    }
    impl_catalog_number!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, f32, f64);
}

/// Interpolation arguments for [`Catalog::get_with_args`] -- see the
/// module doc for why this exists instead of exposing
/// `fluent_bundle::FluentArgs` directly.
#[derive(Default)]
pub struct CatalogArgs<'a> {
    inner: FluentArgs<'a>,
}

impl<'a> CatalogArgs<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// A number. See the module doc; cannot carry a directionality
    /// control by construction (`N` is bounded to numeric primitives).
    pub fn number<N>(mut self, name: impl Into<std::borrow::Cow<'a, str>>, value: N) -> Self
    where
        N: sealed::CatalogNumber,
        FluentValue<'a>: From<N>,
    {
        self.inner.set(name, FluentValue::from(value));
        self
    }

    /// Untrusted text, already escaped and isolated via
    /// `text_safety::quote_untrusted` -- there is no constructor for
    /// this method that accepts a raw `&str`, so a caller cannot skip
    /// `text_safety` and still compile.
    pub fn untrusted(
        mut self,
        name: impl Into<std::borrow::Cow<'a, str>>,
        value: &DisplayText,
    ) -> Self {
        self.inner
            .set(name, FluentValue::from(value.as_str().to_string()));
        self
    }

    /// A compile-time literal symbol. See the module doc for why
    /// `&'static str` is the boundary that matters here.
    pub fn trusted_symbol(
        mut self,
        name: impl Into<std::borrow::Cow<'a, str>>,
        value: &'static str,
    ) -> Self {
        self.inner.set(name, FluentValue::from(value));
        self
    }
}

/// Locale preference inputs above OS-locale detection, in precedence
/// order. Both fields are `None` today (no CLI flag parsing exists yet
/// in `main.rs`, and RFC-023's configuration setting does not exist) --
/// the fields exist so [`Catalog::resolve`]'s signature does not need to
/// change when those callers arrive.
///
/// `#[non_exhaustive]` (response 122 Q2 recommended): a future
/// precedence level -- an environment variable, a per-project override
/// -- can then be added without breaking struct-literal construction
/// anywhere this type is built, the same reason `cli_flag`/`configured`
/// exist ahead of their real callers, generalized instead of predicting
/// each slot in advance.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
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
        self.get_with_args_impl(key, None)
    }

    /// [`Self::get`], with interpolation arguments. Same fallback chain;
    /// `args` is passed through to every stage it reaches. See the
    /// module doc for why [`CatalogArgs`] -- not raw `FluentArgs` -- is
    /// the only way to build these.
    pub fn get_with_args(&self, key: &str, args: &CatalogArgs) -> String {
        self.get_with_args_impl(key, Some(&args.inner))
    }

    fn get_with_args_impl(&self, key: &str, args: Option<&FluentArgs>) -> String {
        if let Some(value) = Self::lookup_in(&self.resolved, key, args) {
            return value;
        }
        if let Some(source) = &self.source_fallback
            && let Some(value) = Self::lookup_in(source, key, args)
        {
            log_missing_key(key, &self.resolved_locale);
            return value;
        }
        log_missing_key(key, catalog::SOURCE_LOCALE);
        key.to_string()
    }

    fn lookup_in(bundle: &catalog::Bundle, key: &str, args: Option<&FluentArgs>) -> Option<String> {
        let message = bundle.get_message(key)?;
        let pattern = message.value()?;
        let mut errors = vec![];
        let value = bundle.format_pattern(pattern, args, &mut errors);
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
mod enforcement;
#[cfg(test)]
mod tests;
