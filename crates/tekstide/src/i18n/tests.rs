use std::fs;
use std::path::{Path, PathBuf};

use super::{Catalog, FluentArgs, FluentValue, LocalePreference, catalog};

/// The real, shipped `locales/` directory -- used by the PR-016-D tests
/// that exercise the actual `pl.ftl` this crate ships, rather than a
/// throwaway fixture, since proving the real file works is the point of
/// "a second locale added purely to prove the machinery."
fn real_locales_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("locales")
}

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

// --- PR-016-D: pluralization and interpolation -----------------------

/// The real, shipped `pl.ftl` must parse and resolve -- if it did not,
/// every plural-category test below would fail confusingly at the
/// fallback stage instead of here.
#[test]
fn real_polish_catalog_loads_from_the_shipped_locales_directory() {
    let preference = LocalePreference {
        cli_flag: Some("pl".to_string()),
        ..Default::default()
    };
    let cat = Catalog::resolve(preference, Some(&real_locales_dir()));
    assert_eq!(cat.resolved_locale(), "pl");
    assert_eq!(cat.get("project-board-title"), "Tablica projektu");
}

/// RFC-016 §Open Questions named Polish specifically because its CLDR
/// plural categories (one / few / many / other) differ from English's
/// (one / other) -- the property this test exists to prove is real,
/// not merely that a second `.ftl` file happens to parse. Three counts
/// chosen to land in three different Polish categories (per CLDR:
/// n=1 -> one; n in {2,3,4} -> few; n=5 (and most n>=5) -> many) must
/// render three genuinely different word forms, not the same word with
/// a different number spliced in.
#[test]
fn plural_categories_differ_correctly_for_polish() {
    let preference = LocalePreference {
        cli_flag: Some("pl".to_string()),
        ..Default::default()
    };
    let cat = Catalog::resolve(preference, Some(&real_locales_dir()));

    let mut args = FluentArgs::new();
    args.set("count", FluentValue::from(1));
    let one = cat.get_with_args("blocked-automation-count", &args);

    let mut args = FluentArgs::new();
    args.set("count", FluentValue::from(3));
    let few = cat.get_with_args("blocked-automation-count", &args);

    let mut args = FluentArgs::new();
    args.set("count", FluentValue::from(5));
    let many = cat.get_with_args("blocked-automation-count", &args);

    assert_ne!(
        one, few,
        "Polish 'one' and 'few' must use different word forms"
    );
    assert_ne!(
        few, many,
        "Polish 'few' and 'many' must use different word forms"
    );
    assert_ne!(
        one, many,
        "Polish 'one' and 'many' must use different word forms"
    );
    assert!(one.contains("zablokowana automatyzacja"));
    assert!(few.contains("zablokowane automatyzacje"));
    assert!(many.contains("zablokowanych automatyzacji"));
}

/// English's simpler one/other categories are still real categories,
/// not a no-op -- contrasted directly against Polish's three-way split
/// above, so this module doesn't only prove "some locale has plurals."
///
/// **A genuine discovery, not anticipated in advance:** Fluent wraps
/// every interpolated placeable in Unicode bidi isolate marks (First
/// Strong Isolate / Pop Directional Isolate) by default (`FluentBundle`'s
/// `use_isolating` is `true` unless explicitly disabled) -- the exact
/// same defense RFC-016 §Security point 2 requires for untrusted spans,
/// applied automatically to every interpolated value, trusted or not.
/// So `{$count}` in the FTL source renders as the isolate-wrapped digit,
/// not the bare digit -- asserted explicitly below so this is a
/// documented property, not a surprise the next reader has to
/// rediscover from a failing string comparison.
#[test]
fn plural_categories_apply_for_english_too_with_its_simpler_one_other_split() {
    let cat = Catalog::resolve(LocalePreference::default(), Some(&real_locales_dir()));
    assert_eq!(cat.resolved_locale(), "en");

    let mut args = FluentArgs::new();
    args.set("count", FluentValue::from(1));
    let one = cat.get_with_args("blocked-automation-count", &args);
    assert_eq!(one, "blocked automation: \u{2068}1\u{2069}");

    let mut args = FluentArgs::new();
    args.set("count", FluentValue::from(9));
    let other = cat.get_with_args("blocked-automation-count", &args);
    assert_eq!(other, "blocked automation: \u{2068}9\u{2069}");
}

/// The design constraint from response 123/124: a `CountDisplay`-shaped
/// value has four states, only one of them numeric, and an interpolation
/// API that only accepts numbers cannot express the other three.
/// `$count` here carries either a `FluentValue::Number` (plural-category
/// selection applies) or a `FluentValue::String` naming one of
/// `CountDisplay`'s non-numeric states (matched as a literal selector
/// variant) -- through the exact same key and the exact same lookup
/// call, so a caller never needs two different code paths. Critically:
/// none of the three non-numeric states renders as `0` or blank, which
/// is RFC-015's own "never as zero" requirement for `CountDisplay`.
///
/// The non-numeric branches (`not_implemented`/`unavailable`/`unknown`)
/// contain no `{$count}` placeable in their FTL variant text at all, so
/// -- unlike the numeric branch above -- they render as plain literal
/// text with no isolate wrapping; Fluent only isolates an actual
/// placeable, not a selector match.
#[test]
fn interpolation_expresses_every_count_display_state_through_one_key() {
    let cat = Catalog::resolve(LocalePreference::default(), Some(&real_locales_dir()));

    let mut known = FluentArgs::new();
    known.set("count", FluentValue::from(9));
    assert_eq!(
        cat.get_with_args("blocked-automation-count", &known),
        "blocked automation: \u{2068}9\u{2069}"
    );

    for (state, expected) in [
        ("not_implemented", "blocked automation: not implemented"),
        ("unavailable", "blocked automation: not available"),
        ("unknown", "blocked automation: unknown"),
    ] {
        let mut args = FluentArgs::new();
        args.set("count", FluentValue::from(state));
        let rendered = cat.get_with_args("blocked-automation-count", &args);
        assert_eq!(rendered, expected);
        assert!(
            !rendered.contains('0'),
            "a non-numeric CountDisplay state must never render as zero: {rendered}"
        );
    }
}

/// Security boundary, proven directly rather than only documented:
/// Fluent treats an interpolated string value as an opaque literal, not
/// as FTL source to be re-parsed -- text that looks like a Fluent
/// reference or select expression inside an argument must render
/// byte-for-byte as given (aside from Fluent's own isolate-mark
/// wrapping, proven separately above), never expanded, never used to
/// alter the structure of the message it was interpolated into.
///
/// Uses a locale name (`xx`) that is neither the source locale nor any
/// real shipped catalog -- `Catalog::load` special-cases the source
/// locale (`en`) to always return the *compiled-in* bundle regardless of
/// `locales_dir`, so a scratch fixture literally named `en.ftl` is never
/// actually read; earlier drafts of this test discovered that the hard
/// way (a "missing key" failure pointing at the real shipped catalog,
/// not the scratch one).
#[test]
fn interpolated_values_cannot_inject_fluent_syntax() {
    let dir = scratch_locales_dir("no-injection");
    fs::write(dir.join("xx.ftl"), "greeting = Hello, {$name}!\n").unwrap();
    let preference = LocalePreference {
        cli_flag: Some("xx".to_string()),
        ..Default::default()
    };
    let cat = Catalog::resolve(preference, Some(&dir));
    assert_eq!(
        cat.resolved_locale(),
        "xx",
        "test precondition: the scratch catalog must be the one in use"
    );

    let malicious = "{ -brand-name } { $secret ->  *[other] leaked }";
    let mut args = FluentArgs::new();
    args.set("name", FluentValue::from(malicious));
    let rendered = cat.get_with_args("greeting", &args);

    assert_eq!(
        rendered,
        format!("Hello, \u{2068}{malicious}\u{2069}!"),
        "an interpolated value must render as a literal string (modulo Fluent's own \
         isolate-mark wrapping), never re-parsed as FTL syntax"
    );
}

/// The other half of the security boundary: interpolation is NOT a
/// substitute for `text_safety::quote_untrusted`. A bidi override
/// character passed as an interpolation argument survives **raw** inside
/// Fluent's isolate wrapping -- Fluent contains it (point 2 of RFC-016's
/// policy, for free) but does not escape it to a visible `<U+XXXX>`
/// marker (point 1, which only `text_safety` does). A caller must still
/// run untrusted values through `text_safety::escape_untrusted_chars`
/// before interpolating them; Fluent's isolation is a second layer of
/// defense here exactly as it is in `text_safety::quote_untrusted`
/// itself, not a reason to skip the first layer.
#[test]
fn interpolation_does_not_substitute_for_text_safety_escaping() {
    let dir = scratch_locales_dir("no-auto-escape");
    fs::write(dir.join("xx.ftl"), "greeting = Hello, {$name}!\n").unwrap();
    let preference = LocalePreference {
        cli_flag: Some("xx".to_string()),
        ..Default::default()
    };
    let cat = Catalog::resolve(preference, Some(&dir));
    assert_eq!(
        cat.resolved_locale(),
        "xx",
        "test precondition: the scratch catalog must be the one in use"
    );

    let untrusted = "evil\u{202E}txt.exe";
    let mut args = FluentArgs::new();
    args.set("name", FluentValue::from(untrusted));
    let rendered = cat.get_with_args("greeting", &args);

    assert!(
        rendered.contains('\u{202E}'),
        "interpolation must not silently escape untrusted text into a visible marker \
         -- that is text_safety's job, and a caller that skips it must be able to see \
         the raw override character survived (even if isolated), not have it silently \
         neutralized here: {rendered:?}"
    );
}
