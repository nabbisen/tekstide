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
//! 1. **Escape, never obey.** Every Unicode **Control (`Cc`)** character
//!    plus every character with the **`Default_Ignorable_Code_Point`**
//!    property -- bidi overrides/isolates, zero-width joiners, soft
//!    hyphen, BOM, the Hangul filler letters, variation selectors, and
//!    anything else in that set -- is rendered as a visible `<U+XXXX>`
//!    marker, never passed to a shaper as a directionality instruction
//!    or left to render invisibly. `Default_Ignorable_Code_Point` is the
//!    standard Unicode concept for exactly this (response 118: the
//!    earlier `Cc` + `Cf` rule left several genuine invisible shapes
//!    unescaped -- see [`is_default_ignorable_extra`]'s doc for the
//!    probe that found them, all of which sit outside `Cf`). Stripping
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
//!
//! **`U+2800` BRAILLE PATTERN BLANK is deliberately NOT escaped**
//! (response 118 Required 1): it renders blank to a sighted reader
//! unfamiliar with Braille, which makes it *look* like the invisible
//! characters this module targets, but it is not
//! `Default_Ignorable_Code_Point` and there is a real argument that
//! Braille content is legitimate text a Braille-literate user is
//! entitled to see rendered, not a security-relevant invisible
//! character. Recorded as a deliberate boundary decision, not an
//! oversight -- see `text_safety::tests::braille_pattern_blank_is_not_escaped`.
//!
//! **Full Unicode `Cn` (unassigned-codepoint) coverage is deliberately
//! out of scope for this slice** (response 118): correctly tracking
//! every unassigned codepoint, and keeping `Default_Ignorable_Code_Point`
//! itself correct as Unicode revises it, needs real Unicode property
//! data, not a hand-rolled table -- the same "no dependency yet" call
//! this module already made for `Cf` in RFC-021 response 115, now
//! reaching its limit. Folding a small Unicode-properties crate into
//! RFC-016 PR-016-B's already-required dependency measurement (alongside
//! Fluent) is the deferred decision; `U+2065` (an unassigned codepoint
//! RFC-016 §Security explicitly names) is hand-covered below as the one
//! demonstrated case that could not wait.
//!
//! **The `<U+XXXX>` marker itself is spoofable** (RFC-021 response 115,
//! restated here since this is now shared API): literal ASCII text
//! `<U+202E>` typed by an adapter renders identically to this module's
//! own escape output for a real `U+202E`. This errs toward
//! over-suspicion -- a real override renders as inert text either way --
//! which is the safe direction, so it is accepted rather than chased.

/// Every Unicode **Format (`Cf`)** category codepoint, as a hand-
/// transcribed snapshot of **Unicode 15.0**'s `UnicodeData.txt` -- not
/// verified mechanically against that or any other version's published
/// data, and not guaranteed current for later Unicode revisions. This is
/// a version-pinned table, not a live category query: record the version
/// so a future reader can check it against the real data rather than
/// trust the claim (response 118 Required 2 -- an earlier version of
/// this doc claimed the rule was "whatever invisible-character shape
/// nobody has thought of yet," which a hand-transcribed table can never
/// actually promise). A known open question, deliberately not resolved
/// either way here: response 115 listed the Shorthand Format Controls
/// block as extending to `0x1343F`; this table stops at `0x13438`. Not
/// asserting which bound is correct -- that discrepancy is exactly the
/// kind of drift a version-pinned, dated table makes checkable instead
/// of invisible.
///
/// Private: this is an implementation slice of the shared policy, not
/// itself the API (response 118 Required 3) -- publishing it would let a
/// caller assemble a different escape rule from its parts, which is the
/// duplication this module exists to end. [`is_untrusted_display_control`]
/// is the public predicate.
fn is_format_char(c: char) -> bool {
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

/// The rest of Unicode's **`Default_Ignorable_Code_Point`** property --
/// everything in that set not already covered by [`is_format_char`]'s
/// `Cf` table. Added in response 118: probing the original `Cc` + `Cf`
/// rule against a small set of genuinely invisible-rendering characters
/// found five it did not cover, all outside `Cf`:
///
/// ```text
/// U+3164 HANGUL FILLER            (Lo -- a LETTER that renders blank)
/// U+FFA0 HALFWIDTH HANGUL FILLER  (Lo)
/// U+115F HANGUL CHOSEONG FILLER   (Lo)
/// U+1160 HANGUL JUNGSEONG FILLER  (Lo)
/// U+2065 <reserved>               (Cn -- RFC-016 §Security point 3 names
///                                   "unassigned-but-rendering-invisible
///                                   codepoints" explicitly)
/// ```
///
/// The attack this closes is the same one response 115 found for
/// `U+200B`, one Unicode general category over: `impor<U+3164>tant.txt`
/// and `important.txt` render identically to the eye, because
/// `U+3164` is classified `Lo` (a Letter), not a control or format
/// character, so the response-115 fix did not catch it.
///
/// Hand-transcribed against **Unicode 15.0**'s `DerivedCoreProperties.txt`
/// for `Default_Ignorable_Code_Point`, with the same not-mechanically-
/// verified caveat as [`is_format_char`]. Deliberately **not** attempting
/// full `Cn` (every unassigned codepoint) coverage here -- see the
/// module doc's note on why that is a dependency decision for PR-016-B,
/// not a hand-rolled addition. `U+2065` is included explicitly because
/// RFC-016 §Security names "unassigned-but-rendering-invisible
/// codepoints" as in scope and it is the demonstrated case; the rest of
/// `Cn` is not.
///
/// **`U+2800` BRAILLE PATTERN BLANK is deliberately excluded** -- see the
/// module doc's dedicated note. It renders blank but is category `So`
/// (Symbol, other), not `Default_Ignorable_Code_Point`, and Braille
/// content is legitimate text.
fn is_default_ignorable_extra(c: char) -> bool {
    matches!(
        c as u32,
        0x034F
            | 0x115F..=0x1160
            | 0x17B4..=0x17B5
            | 0x180B..=0x180D
            | 0x180F
            | 0x2065
            | 0x3164
            | 0xFE00..=0xFE0F
            | 0xFFA0
            | 0xFFF0..=0xFFF8
            | 0xE0000
            | 0xE0002..=0xE001F
            | 0xE0080..=0xE00FF
            | 0xE0100..=0xE01EF
            | 0xE01F0..=0xE0FFF
    )
}

/// Any character this module escapes to a visible `<U+XXXX>` marker
/// rather than passing through raw: every Unicode **Control (`Cc`)**
/// character (`char::is_control`) plus every character with the
/// **`Default_Ignorable_Code_Point`** property ([`is_format_char`]'s
/// `Cf` table plus [`is_default_ignorable_extra`]'s remainder).
///
/// This is a documented *policy* -- a specific, version-pinned snapshot
/// of Unicode data chosen to cover the invisible-character shapes found
/// so far, not a live query guaranteed to cover every future one (see
/// both tables' docs for the versions they reflect and their known
/// gaps).
pub fn is_untrusted_display_control(c: char) -> bool {
    c.is_control() || is_format_char(c) || is_default_ignorable_extra(c)
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
    /// **Losing the isolation wrapping is the caller's responsibility to
    /// avoid, not something this type prevents** (response 118 Q2): this
    /// returns the escaped-and-isolated text as a plain `&str`, and
    /// nothing stops a caller from slicing, truncating, or concatenating
    /// it into a larger string in a way that separates the isolate marks
    /// from the content they wrap. `DisplayText` guarantees that raw,
    /// *unescaped* untrusted text cannot reach a widget requiring this
    /// type -- it does not guarantee that whatever a caller builds
    /// *after* calling this stays isolated. Render the result as-is; do
    /// not reprocess it.
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
