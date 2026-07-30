use std::fs;
use std::path::PathBuf;

use super::{Catalog, LocalePreference, catalog};

fn scratch_locales_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "tekstide-i18n-test-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// The compiled-in source catalog must parse -- if it did not, every
/// other test in this module would fail confusingly at the fallback
/// stage instead of here, at the one place that names the real cause.
#[test]
fn source_locale_catalog_parses() {
    let bundle = catalog::source_locale_bundle();
    assert!(bundle.get_message("app-title").is_some());
}

/// A key that exists in the source locale resolves to its real value,
/// not the key itself -- the baseline the fallback tests below are
/// contrasted against.
#[test]
fn an_existing_key_resolves_to_its_value() {
    let dir = scratch_locales_dir("existing-key");
    let cat = Catalog::resolve(LocalePreference::default(), Some(&dir));
    assert_eq!(cat.resolved_locale(), "en");
    assert_eq!(cat.get("app-title"), "Tekstide");
}

/// RFC-016: "a missing key renders the key... never blank, never
/// panic." A key absent from every catalog in the chain must render as
/// the literal key string.
#[test]
fn a_missing_key_renders_as_the_key_itself_never_blank() {
    let dir = scratch_locales_dir("missing-key");
    let cat = Catalog::resolve(LocalePreference::default(), Some(&dir));
    let rendered = cat.get("no-such-key-anywhere");
    assert_eq!(rendered, "no-such-key-anywhere");
    assert!(!rendered.is_empty());
}

/// A locale with no catalog file on disk at all (and no matching
/// language-only file either) falls all the way through to the source
/// locale -- never panics, never leaves the catalog in a broken state.
#[test]
fn a_missing_locale_falls_back_to_the_source_locale() {
    let dir = scratch_locales_dir("missing-locale");
    let preference = LocalePreference {
        cli_flag: Some("xx-YY".to_string()),
        ..Default::default()
    };
    let cat = Catalog::resolve(preference, Some(&dir));
    assert_eq!(cat.resolved_locale(), "en");
    assert_eq!(cat.get("app-title"), "Tekstide");
}

/// The second link in the fallback chain: a region-specific locale with
/// no exact file, but whose bare language subtag DOES have a file, uses
/// that language-level catalog rather than skipping straight to source.
#[test]
fn a_region_specific_locale_falls_back_to_its_language_subtag_catalog() {
    let dir = scratch_locales_dir("language-fallback");
    fs::write(dir.join("fr.ftl"), "app-title = Tekstide (fr)\n").unwrap();
    let preference = LocalePreference {
        cli_flag: Some("fr-CA".to_string()),
        ..Default::default()
    };
    let cat = Catalog::resolve(preference, Some(&dir));
    assert_eq!(
        cat.resolved_locale(),
        "fr",
        "must resolve to the language-only catalog, not stay at fr-CA or fall to en"
    );
    assert_eq!(cat.get("app-title"), "Tekstide (fr)");
}

/// A key missing from a successfully-loaded non-source locale falls
/// back to the source locale's value, not straight to the bare key --
/// the fallback chain's middle link, distinct from both the "resolves
/// directly" and "key missing everywhere" tests above.
#[test]
fn a_key_missing_from_a_loaded_locale_falls_back_to_the_source_value() {
    let dir = scratch_locales_dir("key-fallback");
    fs::write(
        dir.join("fr.ftl"),
        "app-title = Tekstide (fr)\n", // no project-board-title key
    )
    .unwrap();
    let preference = LocalePreference {
        cli_flag: Some("fr".to_string()),
        ..Default::default()
    };
    let cat = Catalog::resolve(preference, Some(&dir));
    assert_eq!(cat.resolved_locale(), "fr");
    assert_eq!(
        cat.get("project-board-title"),
        "Project Board",
        "must fall back to the source locale's value for a key the resolved locale lacks"
    );
}

/// Locale precedence: an explicit CLI flag wins over everything else,
/// including a configured value -- both are supplied here to prove the
/// CLI flag is the one actually used, not merely present.
#[test]
fn cli_flag_takes_precedence_over_configured_locale() {
    let dir = scratch_locales_dir("precedence-cli");
    fs::write(dir.join("de.ftl"), "app-title = Tekstide (de)\n").unwrap();
    fs::write(dir.join("es.ftl"), "app-title = Tekstide (es)\n").unwrap();
    let preference = LocalePreference {
        cli_flag: Some("de".to_string()),
        configured: Some("es".to_string()),
    };
    let cat = Catalog::resolve(preference, Some(&dir));
    assert_eq!(cat.resolved_locale(), "de");
}

/// No CLI flag, no configured value: OS-locale detection is consulted
/// (via `sys_locale::get_locale`, not injected here since it is a thin,
/// already-tested third-party call) and, whatever it returns, falls
/// through this crate's own chain correctly -- proven indirectly by
/// confirming an empty preference with no matching catalog on disk
/// still resolves to a real, usable catalog rather than panicking.
#[test]
fn an_empty_preference_still_resolves_to_a_usable_catalog() {
    let dir = scratch_locales_dir("no-preference");
    let cat = Catalog::resolve(LocalePreference::default(), Some(&dir));
    assert!(!cat.resolved_locale().is_empty());
    assert_eq!(cat.get("app-title"), "Tekstide");
}

/// No locales directory at all (e.g. it does not exist yet on a fresh
/// install) must not panic -- every non-source locale request simply
/// has nothing to load and falls through to source.
#[test]
fn no_locales_directory_falls_back_to_source_without_panicking() {
    let preference = LocalePreference {
        cli_flag: Some("de".to_string()),
        ..Default::default()
    };
    let cat = Catalog::resolve(preference, None);
    assert_eq!(cat.resolved_locale(), "en");
    assert_eq!(cat.get("app-title"), "Tekstide");
}
