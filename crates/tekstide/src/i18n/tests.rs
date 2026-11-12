use tekstide_core::text_safety;

use super::{Catalog, CatalogArgs, LocalePreference, catalog};

/// The compiled-in source catalog must parse -- if it did not, every
/// other test in this module would fail confusingly at the fallback
/// stage instead of here, at the one place that names the real cause.
#[test]
fn source_locale_catalog_parses() {
    let bundle = catalog::source_locale_bundle();
    assert!(bundle.get_message("app-title").is_some());
}

/// **Review 673 Required 1.** `catalog::load_compiled_locale` folds a
/// parse failure into `None`, silently falling back to English rather
/// than failing the build or the boot -- correct behaviour for a
/// runtime path, but it means a locale added to `locales/` *and* to
/// `ADDITIONAL_LOCALES` with a syntax error in its `.ftl` would compile,
/// pass `the_compiled_in_locale_list_matches_every_ftl_file_in_locales`
/// (it is present on both sides), and serve English with nothing ever
/// failing -- *listed but unparseable*, the same silent shape RFC-073
/// D4 says stops existing, re-entered through the new mechanism instead
/// of the old one. This is the sibling `source_locale_catalog_parses`
/// above has for `en`, covering every other compiled-in locale
/// explicitly rather than by the accident of `pl`'s own plural tests
/// happening to exercise it.
#[test]
fn every_additional_locale_actually_parses() {
    for (name, ftl_text) in catalog::ADDITIONAL_LOCALES {
        assert!(
            catalog::build_bundle(name, ftl_text).is_ok(),
            "locales/{name}.ftl is listed in ADDITIONAL_LOCALES but its compiled-in text does \
             not parse -- it would silently serve English instead, the exact state RFC-073 D4 \
             says stops existing"
        );
    }
}

/// A key that exists in the source locale resolves to its real value,
/// not the key itself -- the baseline the fallback tests below are
/// contrasted against.
#[test]
fn an_existing_key_resolves_to_its_value() {
    let cat = Catalog::resolve(LocalePreference::default());
    assert_eq!(cat.resolved_locale(), "en");
    assert_eq!(cat.get("app-title"), "Tekstide");
}

/// RFC-016: "a missing key renders the key... never blank, never
/// panic." A key absent from every catalog in the chain must render as
/// the literal key string.
#[test]
fn a_missing_key_renders_as_the_key_itself_never_blank() {
    let cat = Catalog::resolve(LocalePreference::default());
    let rendered = cat.get("no-such-key-anywhere");
    assert_eq!(rendered, "no-such-key-anywhere");
    assert!(!rendered.is_empty());
}

/// A locale this build never compiled in falls all the way through to
/// the source locale -- never panics, never leaves the catalog in a
/// broken state. RFC-073 D4: the only reason a requested locale does
/// not resolve is that this build does not carry it, which is exactly
/// what `xx-YY` and its bare `xx` subtag both are. Also covers what a
/// separate, now-redundant test used to call "no locales directory" --
/// that state is unconditional after RFC-073 D1, not a case to
/// construct separately.
#[test]
fn a_missing_locale_falls_back_to_the_source_locale() {
    let preference = LocalePreference {
        cli_flag: Some("xx-YY".to_string()),
        ..Default::default()
    };
    let cat = Catalog::resolve(preference);
    assert_eq!(cat.resolved_locale(), "en");
    assert_eq!(cat.get("app-title"), "Tekstide");
}

/// The second link in the fallback chain: a region-specific locale with
/// no exact compiled-in match, but whose bare language subtag does have
/// one, uses that language-level catalog rather than skipping straight
/// to source.
///
/// **Re-pointed by RFC-073 D1/D3** from a synthetic `fr-CA` whose
/// `fr.ftl` was written to a scratch disk directory -- that mechanism
/// no longer exists, so this now exercises the real, compiled-in `pl`
/// catalog via `pl-PL`, the same property against real data instead of
/// a throwaway fixture.
#[test]
fn a_region_specific_locale_falls_back_to_its_language_subtag_catalog() {
    let preference = LocalePreference {
        cli_flag: Some("pl-PL".to_string()),
        ..Default::default()
    };
    let cat = Catalog::resolve(preference);
    assert_eq!(
        cat.resolved_locale(),
        "pl",
        "must resolve to the language-only catalog, not stay at pl-PL or fall to en"
    );
    assert_eq!(cat.get("project-board-title"), "Tablica projektu");
}

/// A key missing from a successfully-resolved non-source locale falls
/// back to the source locale's value, not straight to the bare key --
/// the fallback chain's middle link, distinct from both the "resolves
/// directly" and "key missing everywhere" tests above.
///
/// **Re-pointed by RFC-073 D1/D3** from a synthetic `fr.ftl` missing one
/// key on purpose -- the real, compiled-in `pl.ftl` already lacks
/// `main-area-switch-to-terminal-button` (it defines only 3 keys,
/// RFC-016 §Non-Goals), so the real catalog exercises the same property
/// with no fixture needed.
#[test]
fn a_key_missing_from_a_loaded_locale_falls_back_to_the_source_value() {
    let preference = LocalePreference {
        cli_flag: Some("pl".to_string()),
        ..Default::default()
    };
    let cat = Catalog::resolve(preference);
    assert_eq!(cat.resolved_locale(), "pl");
    assert_eq!(
        cat.get("main-area-switch-to-terminal-button"),
        "Switch to Terminal",
        "must fall back to the source locale's value for a key the real pl.ftl lacks"
    );
}

/// Locale precedence: an explicit CLI flag wins over everything else,
/// including a configured value -- both are supplied here to prove the
/// CLI flag is the one actually used, not merely present.
///
/// **Re-pointed by RFC-073 D1** from two synthetic `de.ftl`/`es.ftl`
/// scratch files to the two real, always-available locales (`pl`
/// compiled in, `en` the source) -- both resolve regardless of which
/// one precedence picks, so choosing the wrong one is still
/// detectable.
#[test]
fn cli_flag_takes_precedence_over_configured_locale() {
    let preference = LocalePreference {
        cli_flag: Some("pl".to_string()),
        configured: Some("en".to_string()),
    };
    let cat = Catalog::resolve(preference);
    assert_eq!(cat.resolved_locale(), "pl");
}

/// No CLI flag, no configured value: OS-locale detection is consulted
/// (via `sys_locale::get_locale`, not injected here since it is a thin,
/// already-tested third-party call) and, whatever it returns, falls
/// through this crate's own chain correctly -- proven indirectly by
/// confirming an empty preference still resolves to a real, usable
/// catalog rather than panicking.
#[test]
fn an_empty_preference_still_resolves_to_a_usable_catalog() {
    let cat = Catalog::resolve(LocalePreference::default());
    assert!(!cat.resolved_locale().is_empty());
    assert_eq!(cat.get("app-title"), "Tekstide");
}

// --- PR-016-D: pluralization and interpolation -----------------------

/// **RFC-073 D3: replaces `real_polish_catalog_loads_from_the_shipped_
/// locales_directory`, which asserted the wrong property.** That test
/// passed because the test process happened to run from a source
/// checkout where the (then disk-loaded) `locales/` directory was
/// exactly where a build-time-baked path expected it to be -- true of
/// every checkout and CI run, never true of an installed copy, which is
/// why the defect survived four shipped channels undetected. This
/// asserts the actual property: `pl` resolves with no locales directory
/// involved at all. `Catalog::resolve` no longer even has a parameter to
/// pass a directory through -- that absence *is* the assertion, not a
/// condition this test has to construct. That is the state every
/// installed copy is in, and it is what stops this defect from
/// returning, not the fix by itself.
#[test]
fn the_compiled_in_polish_catalog_resolves_with_no_locales_directory_involved_at_all() {
    let preference = LocalePreference {
        cli_flag: Some("pl".to_string()),
        ..Default::default()
    };
    let cat = Catalog::resolve(preference);
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
    let cat = Catalog::resolve(preference);

    let one = cat.get_with_args(
        "blocked-automation-count",
        &CatalogArgs::new().number("count", 1u32),
    );
    let few = cat.get_with_args(
        "blocked-automation-count",
        &CatalogArgs::new().number("count", 3u32),
    );
    let many = cat.get_with_args(
        "blocked-automation-count",
        &CatalogArgs::new().number("count", 5u32),
    );

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
    let cat = Catalog::resolve(LocalePreference::default());
    assert_eq!(cat.resolved_locale(), "en");

    let one = cat.get_with_args(
        "blocked-automation-count",
        &CatalogArgs::new().number("count", 1u32),
    );
    assert_eq!(one, "\u{2068}1\u{2069} blocked automation");

    let other = cat.get_with_args(
        "blocked-automation-count",
        &CatalogArgs::new().number("count", 9u32),
    );
    assert_eq!(other, "\u{2068}9\u{2069} blocked automations");
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
    let cat = Catalog::resolve(LocalePreference::default());

    assert_eq!(
        cat.get_with_args(
            "blocked-automation-count",
            &CatalogArgs::new().number("count", 9u32)
        ),
        "\u{2068}9\u{2069} blocked automations"
    );

    for (state, expected) in [
        ("not_implemented", "blocked automation: not implemented"),
        ("unavailable", "blocked automation: not available"),
        ("unknown", "blocked automation: unknown"),
    ] {
        let rendered = cat.get_with_args(
            "blocked-automation-count",
            &CatalogArgs::new().trusted_symbol("count", state),
        );
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
/// The FTL-lookalike text is a `&'static str` literal in this test, so
/// it goes through [`CatalogArgs::trusted_symbol`] -- that constructor
/// is the right fit here (this test is about Fluent's parser, not about
/// `text_safety`; see `an_escaped_untrusted_value_survives_correctly`
/// below for the `untrusted()` path).
///
/// **Built directly from literal FTL text, not a scratch disk file
/// (RFC-073 D1).** The disk-loading machinery this test used to route a
/// throwaway `xx.ftl` through is gone; `Catalog::for_test_from_literal_
/// ftl` bypasses locale resolution and the compiled-in list entirely,
/// which is the right fit here too -- this test is about `get_with_args`
/// itself, not about which locale resolved. It also removes the old
/// locale-collision trap this test's own history recorded (a scratch
/// file literally named `en.ftl` was never actually read, because `en`
/// is always the compiled-in bundle): that trap cannot recur when there
/// is no disk path to collide on at all.
#[test]
fn interpolated_values_cannot_inject_fluent_syntax() {
    let cat = Catalog::for_test_from_literal_ftl("xx", "greeting = Hello, {$name}!\n");

    let malicious = "{ -brand-name } { $secret ->  *[other] leaked }";
    let rendered = cat.get_with_args(
        "greeting",
        &CatalogArgs::new().trusted_symbol("name", malicious),
    );

    assert_eq!(
        rendered,
        format!("Hello, \u{2068}{malicious}\u{2069}!"),
        "an interpolated value must render as a literal string (modulo Fluent's own \
         isolate-mark wrapping), never re-parsed as FTL syntax"
    );
}

/// Response 125 Required 1: `Catalog::get_with_args` used to accept raw
/// `fluent_bundle::FluentValue::String` built from any runtime `&str`,
/// so a caller could interpolate untrusted text -- a project name, a
/// branch name -- with a live bidi override still in it, unescaped. The
/// probe that proved this real: `"blocked automation: proj\u{202E}gpj.exe"`
/// rendered with the override still live, visually reordering to
/// `projexe.jpg` inside its own isolate.
///
/// `CatalogArgs` closes this structurally: there is no constructor that
/// accepts a raw `&str` at all. [`CatalogArgs::untrusted`] only accepts
/// `&text_safety::DisplayText`, obtainable only via
/// `text_safety::quote_untrusted` -- so a caller who runs untrusted text
/// through `quote_untrusted` first (the only way to reach interpolation
/// with it) gets the escaped, visible-marker form, never the raw
/// override, proven here with the reviewer's own probe text.
///
/// **Built directly from literal FTL text, not a scratch disk file
/// (RFC-073 D1)** -- same reason as the sibling test above.
///
/// The compile-time half of this guarantee -- that
/// `CatalogArgs::untrusted(name, "raw str")` does not typecheck -- is
/// not expressible as a `compile_fail` doctest, since `tekstide` is a
/// `[[bin]]`-only crate with no `[lib]` target for a doctest to link
/// against; recorded instead as a note in `qa-evidence.md`, per response
/// 125's offered alternative.
#[test]
fn an_escaped_untrusted_value_survives_correctly_through_interpolation() {
    let cat = Catalog::for_test_from_literal_ftl("xx", "greeting = Hello, {$name}!\n");

    let malicious = "proj\u{202E}gpj.exe";
    let escaped = text_safety::quote_untrusted(malicious);
    let rendered = cat.get_with_args("greeting", &CatalogArgs::new().untrusted("name", &escaped));

    assert!(
        !rendered.contains('\u{202E}'),
        "a raw, live bidi override must never survive interpolation of an untrusted \
         value: {rendered:?}"
    );
    assert!(
        rendered.contains("<U+202E>"),
        "the override must have been escaped to its visible marker by text_safety \
         before interpolation, not silently dropped: {rendered:?}"
    );
}

// --- RFC-073 D2: the compiled-in list is gated against `locales/` -----

/// **The gate D2 asks for.** Rust cannot glob `include_str!` targets at
/// compile time without a build script -- deliberately not used here
/// (a build script is a second mechanism to audit, complicates both
/// published packages, and hides the list from review, per RFC-073 D2)
/// -- so [`catalog::ADDITIONAL_LOCALES`] is a hand-written list, and a
/// hand-written list is exactly the kind of thing that goes stale: a
/// new `locales/*.ftl` added and forgotten from the list would compile
/// fine and simply never be reachable, silently, which is this
/// codebase's own recurring failure shape (an enumerated set with a
/// writer nobody lists). This test is that writer's check.
///
/// Resolves `locales/` from `CARGO_MANIFEST_DIR`, never by walking up
/// for `.git/`, so it keeps working from an unpacked package -- the
/// same rule PR-074-A's own gate follows, for the same reason.
#[test]
fn the_compiled_in_locale_list_matches_every_ftl_file_in_locales() {
    let locales_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
    let mut on_disk: Vec<String> = std::fs::read_dir(&locales_dir)
        .expect("locales dir must exist")
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("ftl") {
                return None;
            }
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_owned)
        })
        .filter(|name| name != catalog::SOURCE_LOCALE)
        .collect();
    on_disk.sort();

    let mut compiled: Vec<String> = catalog::ADDITIONAL_LOCALES
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .collect();
    compiled.sort();

    assert_eq!(
        on_disk, compiled,
        "catalog::ADDITIONAL_LOCALES must name exactly every non-source .ftl file in \
         locales/ -- a file present on one side and not the other is either shipped but \
         unreachable, or listed but missing from disk"
    );
}
