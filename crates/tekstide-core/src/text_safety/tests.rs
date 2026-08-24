use std::path::{Path, PathBuf};

use super::{escape_untrusted_chars, is_untrusted_display_control, quote_untrusted};

/// RFC-016 §Test and Evidence Requirements' bidi corpus, and README's
/// instruction (2026-07-30) to carry over the exact ten-codepoint set
/// already proven in `approval::tests::coordinator` against response 115's
/// probe, so the canonical implementation here is tested at least as hard
/// as the private copy it replaces.
const BIDI_AND_FORMAT_PROBE: &[(char, &str)] = &[
    ('\u{202E}', "<U+202E>"), // RLO -- the classic Trojan Source override
    ('\u{2066}', "<U+2066>"), // LRI -- an isolate, not just an override
    ('\u{200E}', "<U+200E>"), // LRM
    ('\u{200F}', "<U+200F>"), // RLM
    ('\u{061C}', "<U+061C>"), // ALM
    ('\u{200B}', "<U+200B>"), // ZWSP
    ('\u{200C}', "<U+200C>"), // ZWNJ
    ('\u{200D}', "<U+200D>"), // ZWJ
    ('\u{00AD}', "<U+00AD>"), // soft hyphen
    ('\u{FEFF}', "<U+FEFF>"), // BOM
];

/// RFC-016's checklist names this exact four-codepoint range explicitly
/// (`U+2066..U+2069 escaped`) -- LRI, RLI, FSI, and PDI, the isolate
/// initiators/terminator. `U+2066`/`U+2068` are already in the probe set
/// above via other fixtures; this closes the remaining two explicitly,
/// including `U+2069` (PDI) -- the same codepoint `quote_untrusted` itself
/// appends as the *closing* isolate mark, so this also proves that an
/// attacker-supplied PDI inside the untrusted text is escaped to a
/// literal marker and cannot be mistaken for (or prematurely close) the
/// isolate this module adds.
#[test]
fn the_full_isolate_initiator_and_terminator_range_is_escaped() {
    for codepoint in ['\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}'] {
        let escaped = escape_untrusted_chars(&codepoint.to_string());
        assert_eq!(escaped, format!("<U+{:04X}>", codepoint as u32));
    }
}

#[test]
fn every_bidi_and_format_probe_escapes_to_its_visible_marker() {
    for (codepoint, expected_marker) in BIDI_AND_FORMAT_PROBE {
        let text = format!("before{codepoint}after");
        let escaped = escape_untrusted_chars(&text);
        assert_eq!(
            escaped,
            format!("before{expected_marker}after"),
            "codepoint U+{:04X} must escape to a visible marker",
            *codepoint as u32
        );
        assert!(is_untrusted_display_control(*codepoint));
    }
}

/// RFC-016's Trojan Source requirement, generalized from RFC-021's
/// argv-shaped version to a single untrusted string: a value whose
/// logical bytes read one way must not display differently because a
/// renderer obeyed an embedded directionality override. With the
/// override escaped to a literal `<U+XXXX>` marker, there is no live
/// bidi instruction left for any renderer to obey -- the displayed
/// character sequence is, byte for byte (modulo the marker
/// substitution), the logical sequence.
#[test]
fn the_trojan_source_pattern_is_defeated() {
    // The canonical pattern: an attacker wants "evil.exe" to display as
    // "exe.evil" (or similar) by embedding RLO before the extension.
    // Logical bytes: "good" + RLO + "exe." -- a bidi-obeying renderer
    // would show "good" followed by "exe." reversed to ".exe" reading
    // right-to-left, i.e. "goodexe." rendered with the tail flipped.
    let logical = "good\u{202E}exe.";
    let quoted = quote_untrusted(logical);

    // The RLO survives only as an inert, visible marker -- never as a
    // live override a shaper could act on.
    assert!(quoted.as_str().contains("<U+202E>"));
    assert!(!quoted.as_str().contains('\u{202E}'));

    // Stripping the isolate wrapper this module adds, the remaining
    // character sequence must appear in the SAME order as the logical
    // input -- nothing has been reordered, only substituted.
    let inner = quoted
        .as_str()
        .trim_start_matches('\u{2068}')
        .trim_end_matches('\u{2069}');
    assert_eq!(inner, "good<U+202E>exe.");
}

/// RFC-016's isolation requirement: an untrusted span containing an
/// **unterminated** directionality override must not affect adjacent
/// trusted labels. Two properties are checked together: the override
/// survives only as an inert marker (so there is nothing live to leak in
/// the first place), and the span is additionally wrapped in bidi
/// isolate marks as a second, structural layer -- so even a
/// hypothetical gap in the escaping table could not let a directional
/// effect cross into surrounding trusted text.
///
/// This checks structural properties (marker substitution, isolate
/// wrapping) rather than running a full Unicode Bidi Algorithm resolver
/// -- this crate does not implement one, and does not need to: the
/// escaping step removes every live directional instruction before the
/// isolate wrapping is even relevant.
#[test]
fn an_unterminated_override_cannot_affect_adjacent_trusted_text() {
    let untrusted = "start\u{202E}no matching pop-directional-formatting";
    let quoted = quote_untrusted(untrusted);

    let trusted_before = "Trusted Before Label";
    let trusted_after = "Trusted After Label";
    let composed = format!("{trusted_before} {quoted} {trusted_after}");

    // No live bidi/format control anywhere in the composed string except
    // the two isolate marks this function itself added.
    for c in composed.chars() {
        if c == '\u{2068}' || c == '\u{2069}' {
            continue;
        }
        assert!(
            !is_untrusted_display_control(c),
            "a live control character survived escaping: U+{:04X}",
            c as u32
        );
    }
    // The trusted labels are untouched, byte for byte.
    assert!(composed.starts_with(trusted_before));
    assert!(composed.ends_with(trusted_after));
    // The isolate wrapper is present around the untrusted span.
    assert!(quoted.as_str().starts_with('\u{2068}'));
    assert!(quoted.as_str().ends_with('\u{2069}'));
}

/// RFC-016's over-escaping risk, directly: legitimate right-to-left
/// script text (real Arabic letters, not bidi *control* characters) must
/// render naturally, unescaped -- only Control/Format category
/// characters are in scope, never ordinary letters from any script.
#[test]
fn legitimate_rtl_letters_are_not_escaped() {
    let arabic = "مرحبا"; // "hello" -- ordinary Arabic letters, no controls
    let escaped = escape_untrusted_chars(arabic);
    assert_eq!(
        escaped, arabic,
        "genuine RTL script text must pass through unescaped -- only \
         Control/Format characters are ever substituted"
    );
    for c in arabic.chars() {
        assert!(!is_untrusted_display_control(c));
    }
}

/// Byte fidelity: `escape_untrusted_chars`/`quote_untrusted` are pure
/// `&str -> String`/`DisplayText` transforms. Neither takes the input by
/// mutable reference, so the caller's original value is structurally
/// unchanged after the call -- guarding against a future signature
/// change reintroducing an ingest-time mutation. There is currently no
/// call site that persists this module's *output* anywhere durable (no
/// RFC-011 transcript or RFC-013 audit write-path calls into
/// `text_safety` at all); the one existing real integration point,
/// `approval::coordinator`, is covered by its own
/// `sentinel_command_text_never_reaches_the_durable_audit_store`, which
/// this refactor does not change.
#[test]
fn escaping_does_not_mutate_the_caller_s_original_value() {
    let original = String::from("git status \u{202E} --sentinel");
    let original_clone = original.clone();
    let _ = escape_untrusted_chars(&original);
    let _ = quote_untrusted(&original);
    assert_eq!(
        original, original_clone,
        "escape_untrusted_chars/quote_untrusted must never mutate their input"
    );
}

/// An empty string is a legitimate input (an empty untrusted field) and
/// must round-trip through both functions without panicking, producing
/// just the isolate wrapper for `quote_untrusted`.
#[test]
fn an_empty_string_does_not_panic_and_produces_the_bare_isolate_wrapper() {
    assert_eq!(escape_untrusted_chars(""), "");
    assert_eq!(quote_untrusted("").as_str(), "\u{2068}\u{2069}");
}

/// Response 118 Required 1: the exact five codepoints the reviewer's
/// probe found unescaped under the earlier `Cc` + `Cf` rule, all outside
/// `Cf`, now covered by `is_default_ignorable_extra`. `U+3164`/`U+FFA0`/
/// `U+115F`/`U+1160` are Hangul filler *letters* (`Lo`) that render
/// blank; `U+2065` is the one unassigned (`Cn`) codepoint RFC-016
/// §Security names explicitly.
const DEFAULT_IGNORABLE_EXTRA_PROBE: &[(char, &str)] = &[
    ('\u{3164}', "<U+3164>"), // HANGUL FILLER
    ('\u{FFA0}', "<U+FFA0>"), // HALFWIDTH HANGUL FILLER
    ('\u{115F}', "<U+115F>"), // HANGUL CHOSEONG FILLER
    ('\u{1160}', "<U+1160>"), // HANGUL JUNGSEONG FILLER
    ('\u{2065}', "<U+2065>"), // <reserved>, named explicitly by RFC-016
];

#[test]
fn every_default_ignorable_extra_probe_escapes_to_its_visible_marker() {
    for (codepoint, expected_marker) in DEFAULT_IGNORABLE_EXTRA_PROBE {
        let text = format!("before{codepoint}after");
        let escaped = escape_untrusted_chars(&text);
        assert_eq!(
            escaped,
            format!("before{expected_marker}after"),
            "codepoint U+{:04X} must escape to a visible marker",
            *codepoint as u32
        );
        assert!(is_untrusted_display_control(*codepoint));
    }
}

/// Response 118 Required 1's reproduction of the exact attack: two
/// different filenames that render identically to the eye because
/// `U+3164` (a Hangul *letter*, not a control/format character) was not
/// escaped under the response-115-era rule -- the same class of finding
/// as `U+200B` in response 115, one Unicode general category over. With
/// the fix, the two are no longer visually identical: the filler renders
/// as a literal, visible marker.
#[test]
fn the_hangul_filler_display_spoofing_attack_is_defeated() {
    let benign = escape_untrusted_chars("important.txt");
    let spoofed = escape_untrusted_chars("impor\u{3164}tant.txt");
    assert_ne!(
        benign, spoofed,
        "a Hangul filler must not let two different filenames render identically"
    );
    assert_eq!(spoofed, "impor<U+3164>tant.txt");
}

/// Response 118 Required 1: `U+2800` BRAILLE PATTERN BLANK renders blank
/// to a sighted reader unfamiliar with Braille -- the same visual
/// symptom as the codepoints above -- but it is category `So` (Symbol,
/// other), not `Default_Ignorable_Code_Point`, and Braille content is
/// legitimate text a Braille-literate user is entitled to see rendered
/// unescaped. This is a deliberate boundary decision, recorded as a
/// fixture so a future change to it is a visible diff, not a silent one.
#[test]
fn braille_pattern_blank_is_not_escaped() {
    assert!(!is_untrusted_display_control('\u{2800}'));
    assert_eq!(escape_untrusted_chars("\u{2800}"), "\u{2800}");
}

/// Response 118's over-escaping control cases, alongside the five
/// must-escape ones above: an ordinary space and a genuine CJK letter
/// must never be escaped, confirming the expanded predicate did not
/// widen into legitimate text while closing the invisible-character gap.
#[test]
fn ordinary_space_and_cjk_letters_are_not_escaped() {
    for c in ['\u{0020}', '\u{3042}'] {
        assert!(
            !is_untrusted_display_control(c),
            "U+{:04X} must not escape",
            c as u32
        );
    }
    assert_eq!(escape_untrusted_chars(" \u{3042}"), " \u{3042}");
}

// RFC-038 PR-038-E: response 304's own correction to response 300's Guard
// 1. That guard (a `.untrusted(`-vs-`quote_untrusted(` count-equality
// check scoped to one GUI module) was redundant with a compile-time
// guarantee that has existed since RFC-016: `DisplayText`'s field is
// private and `quote_untrusted` is its only constructor, so an untrusted
// value literally cannot reach a `.untrusted(`-typed parameter (`&
// DisplayText`) without having passed through this module's escaping --
// there is no way to construct one otherwise, and no runtime test adds
// anything to that. Worse, the deleted guard would fail on *correct*
// code the moment `surface/explorer.rs` legitimately escaped a value and
// rendered it directly rather than through `.untrusted(` -- exactly the
// shape `board.rs` already uses (0 `.untrusted(` against 3
// `quote_untrusted(`), which response 300 itself named as a reason not
// to extend the invariant there, without noticing it is also a hazard
// for the module the guard *was* written against, on its very next
// refactor.
//
// The property actually worth guarding, per response 304: `DisplayText`
// must keep exactly one constructor, and its field must stay private.
// Add a second `-> DisplayText` function, or make the field `pub`, and
// this whole compile-time guarantee silently degrades into a mere
// convention -- with every existing `.untrusted(` call site across the
// crate instantly unproven and nothing failing. Guarded here, not
// per-caller, since the property belongs to the type, not to any one
// module that happens to use it.

fn crate_src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
}

/// The constructor count: exactly one function in this crate's own,
/// non-test source may return `DisplayText`. Test files are excluded
/// from the scan, the same convention
/// `only_one_production_call_site_ever_restores_a_projects_trust_state`
/// (`app::tests`) already uses -- not because a test-only constructor
/// would be safe (it would be exactly the kind of second entry point
/// this guard exists to catch), but because this guard's own doc
/// comments and assertion messages necessarily contain the literal text
/// `-> DisplayText` in prose, which a raw source-text scan cannot
/// distinguish from a real function signature. A genuine test-only
/// constructor would still be new *production-shaped* code inside a
/// `#[cfg(test)]` module and would still be worth catching, but that is
/// a narrower, different check than this one; this test's job is the
/// production API surface.
#[test]
fn exactly_one_function_in_the_crate_returns_displaytext() {
    let mut files = Vec::new();
    collect_rs_files(&crate_src_dir(), &mut files);

    let sites: Vec<String> = files
        .into_iter()
        .flat_map(|path| {
            let relative = path
                .strip_prefix(crate_src_dir())
                .expect("file must be under src/")
                .to_str()
                .expect("path must be valid UTF-8")
                .replace(std::path::MAIN_SEPARATOR, "/");
            if relative.contains("/tests/") || relative.ends_with("tests.rs") {
                return Vec::new();
            }
            let source = std::fs::read_to_string(&path).expect("scannable file must be readable");
            let count = source.matches("-> DisplayText").count();
            std::iter::repeat_with(move || relative.clone())
                .take(count)
                .collect()
        })
        .collect();

    assert_eq!(
        sites,
        vec!["text_safety.rs".to_string()],
        "exactly one function may return DisplayText (quote_untrusted, this file) -- a second \
         constructor anywhere silently degrades the compile-time \"untrusted text cannot reach \
         a DisplayText-typed parameter unescaped\" guarantee into a mere convention: {sites:?}"
    );
}

/// The other half: `DisplayText`'s own field must stay private. Not
/// mechanically enforceable as a property of the *language* (Rust has no
/// "assert this field is private" reflection), so asserted the same way
/// `no_raw_color_construction_anywhere_in_the_crate`-style tests already
/// assert other structural properties elsewhere in this codebase: a
/// direct source-text check against the one line that declares the
/// struct, which is honest about being a text match rather than a type-
/// system fact, but is still enough to fail loudly the moment someone
/// changes `struct DisplayText(String)` to `struct DisplayText(pub String)`.
#[test]
fn displaytexts_field_is_declared_private_in_source() {
    let source =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/text_safety.rs"))
            .expect("text_safety.rs must be readable");

    assert!(
        source.contains("struct DisplayText(String)"),
        "DisplayText's field must stay private (declared as `struct DisplayText(String)`, not \
         `pub String`) -- a public field would let any caller construct one from raw, \
         unescaped text, defeating the guarantee exactly_one_function_in_the_crate_returns_displaytext \
         only proves for the constructor side"
    );
}
