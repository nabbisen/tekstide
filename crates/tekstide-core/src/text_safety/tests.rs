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
