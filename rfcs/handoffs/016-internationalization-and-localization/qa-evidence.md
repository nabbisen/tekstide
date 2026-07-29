# RFC-016: Internationalization and Localization - QA Evidence

Status: Proposed — implementation pending
Date opened: 2026-07-29
Date accepted: Pending

## Scope

RFC-016 delivers the localization machinery and the text-safety rules for untrusted text rendered in trusted surfaces.

Evidence in this file must not be used to claim confusable/homoglyph protection, RTL layout mirroring, runtime locale switching, terminal-grid bidi reordering, terminal wide-cell CJK, or completed translations into any specific language — unless later reviewed implementation explicitly supports that claim.

## The security finding this RFC exists to fix

Verified 2026-07-29, before implementation:

- The RFC-009 `SecurityFilter` forwards `input(c: char)` unconditionally. Bidi controls are printable Unicode, not C0/C1, so they were never part of the accepted/inert classification.
- `tekstide-core`'s `TerminalSecurityParser` contains no bidi handling; U+202A..U+202E and U+2066..U+2069 pass through as ordinary text.
- RFC-014 C10 demonstrated the editor surface performs full Unicode bidi reordering via `cosmic-text`.

Consequence: a command string containing U+202E would render reversed in RFC-021's approval dialog — Trojan Source (CVE-2021-42574) applied to the surface whose entire purpose is showing the user exactly what will run. PR-016-C closes this.

## Design Review

Pending PR-016-A acceptance.

## Implementation Evidence

### PR-016-B — Catalog, locale selection, fallback

Pending implementation.

### PR-016-C — Text safety: escape and isolate

Pending implementation. **Security-critical.** Escape at render, never at ingest — stored values must keep byte fidelity so RFC-013 audit records and RFC-011 transcripts stay exact.

### PR-016-D — Pluralization and interpolation

Pending implementation.

### PR-016-E — Enforcement

Pending implementation.

### PR-016-F — Closeout evidence

Pending implementation.

## Known Limitations

- **Confusable and homoglyph attacks are not addressed.** Full detection is heavy and error-prone; a partial implementation would imply a guarantee that does not exist. A later RFC may revisit if command approval proves to need it.
- **RTL layout mirroring is deferred.** Text renders correctly RTL; surrounding layout stays LTR.
- **Terminal-grid bidi reordering is out of scope by design** — real terminals do not implement it, and reordering would break cursor and column arithmetic.
- **Terminal wide-cell CJK is a genuine gap owned by RFC-017**, not by this RFC.
- Runtime locale switching is out of scope for M8; selection resolves once at startup.
