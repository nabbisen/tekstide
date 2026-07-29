---
title: "RFC-016: Internationalization and Localization - Implementation Handoff"
rfc: "RFC-016"
rfc_file: "../../proposed/016-internationalization-and-localization.md"
target_milestone: "M8"
source_rfc_status: "Proposed"
created: "2026-07-29"
updated: "2026-07-29"
---

# RFC-016 Implementation Handoff

Covers PR-016-B through PR-016-F. Product code — thorough tests expected.

## 1. Where this code lives

```
crates/tekstide/src/i18n.rs            lookup, locale selection, fallback
crates/tekstide/src/i18n/catalog.rs    catalog loading; source locale compiled in
crates/tekstide/src/i18n/text_safety.rs  escape-and-isolate (see §3)
crates/tekstide/locales/               catalog files
```

RFC-015 creates `i18n.rs` with a placeholder lookup and an English default. You replace the placeholder; **do not change the call shape** — RFC-015's shell code already calls it everywhere, and changing the signature would ripple through every surface.

## 2. Catalog decision (PR-016-B)

Recommended: **Fluent** (`fluent-bundle`). Native plural categories, gendered forms, interpolation, and an asymmetric-translation model.

**Measure the dependency cost before committing to it.** RFC-013's T-033 and RFC-014's R3 both establish that this project weighs dependency surface deliberately — record `Cargo.lock` package delta the same way PR-014-B recorded iced's +345.

If the cost is disproportionate, the specified fallback is a plain TOML key-value catalog with an explicit plural-category function. Less capable, materially lighter. **Record the decision and its basis either way** — this is a judgement call the reviewer needs to see reasoning for, not just an outcome.

Source-locale catalog is **compiled into the binary**, so a missing or corrupt catalog file can never make the application unusable.

## 3. Text safety (PR-016-C) — the security-critical slice

### What to build

A single function on the untrusted-text render path:

```rust
/// Render untrusted text safely inside a trusted surface.
/// Escapes directionality and invisible-format characters so they are
/// visible rather than obeyed, and isolates the span so it cannot alter
/// surrounding chrome.
pub fn quote_untrusted(text: &str) -> DisplayText;
```

Requirements:

1. **Escape, do not strip.** U+202A..U+202E and U+2066..U+2069 render as a visible representation (e.g. `<U+202E>`). Stripping produces a display that differs from the real value — the same class of problem, differently shaped.
2. **Isolate the span** so its content cannot change the directionality of surrounding trusted labels, whatever it contains.
3. **Escape other invisibles:** zero-width joiner/non-joiner, zero-width space, soft hyphen.
4. **Never mutate stored values.** Escaping happens at render. RFC-013 audit records and RFC-011 transcripts keep byte fidelity.

### Where it must be applied

Approval, trust, paste-confirmation, destructive, and safe-close dialogs; Project Board rows (branch and project names are untrusted Git display text per threat model §8.10); notifications; audit and transcript viewers.

### Where it must NOT be applied

- **Editor surface** — renders file content as-is. Deliberate exception; document it.
- **Terminal surface** — no escaping, no reordering. Real terminals do not do bidi, and reordering the grid would break cursor and column arithmetic.

### Make bypassing it hard

Put the escaping in the shared untrusted-text render path, not in each surface's view function. A surface that forgets to call it reintroduces the vulnerability, and "remember to call this" does not survive four more surface RFCs.

If the type system can make untrusted text unrenderable without passing through this function, prefer that — same reasoning as RFC-015's input classes.

## 4. Fallback (PR-016-B)

```
requested locale → language without region → source locale (en) → the key itself
```

**Never blank. Never panic.** A missing key rendering as `project_board.title` is ugly and immediately obvious — the correct failure mode. Log missing keys at debug level in development; they are not audit events.

## 5. What is never localized

Commands, argv, paths, file names, ids, terminal output, transcripts, file contents, Git branch/commit/author strings, audit field values and reason codes, configuration keys, diagnostics.

Labels *around* these values are localized; the values themselves are data.

## 6. Enforcement (PR-016-E)

- Scan shell-crate sources for string literals passed to widget text constructors; fail on anything not routed through the lookup.
- Catalog-completeness test: every key referenced in code exists in the source locale.
- Unused-key report advisory only — keys legitimately outlive UI revisions.

Mechanical enforcement is required. A single hardcoded string is trivial to add, invisible in review, and expensive to find later.

## 7. What you must not build

- Actual translations beyond what proves the machinery. Content work, not this RFC.
- Runtime locale switching. Selection resolves once at startup for M8.
- Full RTL layout mirroring. Text renders RTL; layout stays LTR, recorded as a limitation.
- Confusable/homoglyph detection. Explicitly out of scope; a partial implementation would imply a guarantee that does not exist.
- Terminal-grid bidi reordering. Wrong by design.
- Wide-cell CJK in the terminal — that gap belongs to RFC-017.

## 8. What I will probe at review

- **Trojan Source pattern:** a command string whose displayed order differs from its logical order. The display must match the logical argv.
- **Isolation:** untrusted text with an unterminated directionality override, checking adjacent trusted labels are unaffected.
- **Byte fidelity:** the audit and transcript paths, confirming stored values are untransformed.
- **Over-escaping:** legitimate Arabic UI chrome must render naturally, unescaped.
- **Bypass:** a surface rendering untrusted text without going through the shared path.

## 9. Gates

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
git diff --check
```
