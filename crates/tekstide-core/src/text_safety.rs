//! RFC-016 PR-016-C: the shared untrusted-text render-safety primitive.
//!
//! **History.** This escaping policy was first implemented ad hoc inside
//! `approval::coordinator` (RFC-021 PR-021-E1, hardened by response 115)
//! because `domain::ApprovalRequest::display_command` was being
//! constructed unsafely and could not wait for RFC-016. That was
//! response 115's own sequencing call, not a design decision -- RFC-016
//! §Risks warns explicitly against exactly this outcome: *"escaping
//! belongs to the shared untrusted-text render path, not to each
//! surface."* This module is the fix: the one canonical implementation,
//! consolidated here so `approval::coordinator` and every future
//! trusted-surface renderer (Project Board rows, notifications, audit
//! and transcript viewers, per RFC-016 §Security's table) share it
//! instead of each keeping its own copy of the same security policy.
//!
//! **The threat (RFC-016 §Security, Trojan Source / CVE-2021-42574).** A
//! command or other untrusted string may contain Unicode directionality
//! controls. A renderer that obeys them displays a different order than
//! the logical, executed bytes -- the user approves what they read; the
//! system runs what was sent. RFC-021's approval dialog exists
//! specifically to show the user what an adapter proposes to run, so a
//! text renderer that honours embedded overrides defeats that guarantee
//! completely.
//!
//! **Policy: escape and isolate**, exactly RFC-016 §Security's two points:
//!
//! 1. **Escape, never obey.** Every Unicode **Control (`Cc`)** and
//!    **Format (`Cf`)** category character -- bidi overrides/isolates,
//!    zero-width joiners, soft hyphen, BOM, and anything else in either
//!    category -- is rendered as a visible `<U+XXXX>` marker, never
//!    passed to a shaper as a directionality instruction. Stripping
//!    instead of escaping was considered and rejected (RFC-016
//!    Requirement 1): a stripped display still differs from the real
//!    value, which is the same defect in a different shape.
//! 2. **Isolate the span.** [`quote_untrusted`]'s output is wrapped in
//!    Unicode bidi isolate marks (First Strong Isolate / Pop Directional
//!    Isolate) so its content cannot influence the direction of
//!    surrounding trusted chrome, regardless of what it contains -- a
//!    second, structural layer of defense on top of point 1, not a
//!    substitute for it.
//!
//! **Escaping happens here, at render -- never at ingest.** Every
//! function in this module is a pure `&str -> String`/`DisplayText`
//! transform; nothing here mutates or truncates a caller's stored value.
//! RFC-013 audit records and RFC-011 transcripts must keep byte-exact
//! values; only what a surface *displays* goes through this module.
//!
//! **Deliberately not extended to homoglyphs/confusables** (RFC-016
//! §Residual Limitation, RFC-021 response 115 Q1): Cyrillic `о` versus
//! Latin `o` is a different problem class (script-mixing detection, a
//! skeleton algorithm), and a policy loose enough to catch it would
//! over-escape legitimate Cyrillic, Greek, Arabic, and CJK text --
//! breaking the very i18n requirement this RFC exists to satisfy.
//! Invisible characters are unambiguously wrong in a security display;
//! visible non-Latin characters are the point of having i18n. This is
//! the line, and it stops here.

/// Every Unicode **Format (`Cf`)** category codepoint currently assigned.
/// Hand-rolled rather than a dependency, per RFC-021 response 115: `Cf` is
/// a small, stable set of ranges. Escaping only the two bidi-override
/// ranges (RFC-016 §Security point 1 read narrowly) implements point 1
/// but not point 3 ("other invisible or format characters ... made
/// visible on the same principle") -- zero-width joiners/non-joiners,
/// zero-width space, soft hyphen, and other bidi *marks* (not just
/// overrides) all sit outside those two ranges, and a probe found three
/// genuine bidi controls (LRM `U+200E`, RLM `U+200F`, ALM `U+061C`) and
/// several zero-width/invisible characters (ZWSP, ZWNJ, ZWJ, soft hyphen,
/// BOM) passing through unescaped under the narrower rule.
pub fn is_format_char(c: char) -> bool {
    matches!(
        c as u32,
        0x00AD
            | 0x0600..=0x0605
            | 0x061C
            | 0x06DD
            | 0x070F
            | 0x0890..=0x0891
            | 0x08E2
            | 0x180E
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x13438
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
    )
}

/// Any character this module escapes to a visible `<U+XXXX>` marker
/// rather than passing through raw: every Unicode **Control (`Cc`)**
/// character (`char::is_control`) plus every **Format (`Cf`)** character
/// ([`is_format_char`]) -- a general-category rule rather than an
/// enumerated list, so it covers whatever invisible-character shape
/// nobody has thought of yet, not just the ones already found.
pub fn is_untrusted_display_control(c: char) -> bool {
    c.is_control() || is_format_char(c)
}

/// Escapes every [`is_untrusted_display_control`] character in `text` to
/// a visible `<U+XXXX>` marker; every other character -- including
/// legitimate non-Latin scripts, since this function does nothing about
/// confusables -- passes through unchanged.
///
/// Exposed as its own function (not folded only into [`quote_untrusted`])
/// because argv-shaped consumers like `approval::coordinator::display_argv`
/// need the character-escaping half without the whole-string isolation
/// wrapping, which does not make sense per-entry.
pub fn escape_untrusted_chars(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if is_untrusted_display_control(c) {
            escaped.push_str(&format!("<U+{:04X}>", c as u32));
        } else {
            escaped.push(c);
        }
    }
    escaped
}

/// First Strong Isolate (U+2068): opens a run whose base direction is
/// determined by its own first strong character, without affecting the
/// direction of text outside it either way.
const FIRST_STRONG_ISOLATE: char = '\u{2068}';
/// Pop Directional Isolate (U+2069): closes the isolate opened by
/// [`FIRST_STRONG_ISOLATE`].
const POP_DIRECTIONAL_ISOLATE: char = '\u{2069}';

/// An untrusted string that has been escaped and isolated for display in
/// a trusted surface (RFC-016 §Security: "escape and isolate"). The only
/// way to construct one is [`quote_untrusted`] -- there is no public
/// constructor that accepts an arbitrary string, so a caller cannot pass
/// raw, unescaped untrusted text to a widget whose API requires
/// `DisplayText` without going through this module's escaping. Per
/// `implementation-handoff.md`'s "if the type system can make untrusted
/// text unrenderable without passing through this function, prefer
/// that" -- the same reasoning RFC-015 applies to its input classes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayText(String);

impl DisplayText {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DisplayText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Renders untrusted `text` safely for display inside a trusted surface:
/// every directionality-override/isolate control and other Unicode
/// Format character becomes a visible `<U+XXXX>` marker (never obeyed as
/// a shaping instruction, via [`escape_untrusted_chars`]), and the whole
/// result is wrapped in bidi isolate marks so it cannot affect the
/// directionality of surrounding trusted chrome, regardless of what it
/// contains.
///
/// This is the whole-string API for a single untrusted value embedded in
/// otherwise-trusted chrome (a Project Board row, a notification, an
/// audit-viewer cell). Argv-shaped values with their own per-entry
/// quoting needs (`approval::coordinator::display_argv`) call
/// [`escape_untrusted_chars`] directly instead, since isolating a whole
/// joined command line the same way would not compose with per-entry
/// shell-style quoting.
///
/// **Never call this on a value before storing it.** Escaping happens at
/// render; RFC-013 audit records and RFC-011 transcripts must keep the
/// original, byte-exact text.
pub fn quote_untrusted(text: &str) -> DisplayText {
    let mut wrapped = String::with_capacity(text.len() + 2);
    wrapped.push(FIRST_STRONG_ISOLATE);
    wrapped.push_str(&escape_untrusted_chars(text));
    wrapped.push(POP_DIRECTIONAL_ISOLATE);
    DisplayText(wrapped)
}

#[cfg(test)]
mod tests;
