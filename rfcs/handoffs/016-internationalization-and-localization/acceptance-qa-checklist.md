---
title: "RFC-016: Internationalization and Localization - Acceptance / QA Checklist"
rfc: "RFC-016"
rfc_file: "../../proposed/016-internationalization-and-localization.md"
status: "Proposed — implementation in progress (PR-016-C landed 2026-07-30, not yet reviewed)"
target_milestone: "M8"
source_rfc_status: "Proposed"
created: "2026-07-29"
updated: "2026-07-29"
---

# RFC-016 Acceptance / QA Checklist

**A checked box means evidence exists, not that the result was favourable.**

## Text Safety Checklist (PR-016-C) — security-critical

- [x] U+202A..U+202E escaped to a visible representation, not obeyed. `text_safety::escape_untrusted_chars`; `every_bidi_and_format_probe_escapes_to_its_visible_marker` covers U+202E explicitly, the full range is covered by the `is_format_char` range check itself.
- [x] U+2066..U+2069 escaped. `the_full_isolate_initiator_and_terminator_range_is_escaped` covers all four explicitly, including U+2069 (PDI) — the same codepoint used to close `quote_untrusted`'s own isolate wrap.
- [x] Zero-width joiner/non-joiner, zero-width space, soft hyphen escaped. Covered in the ten-codepoint probe (U+200B/200C/200D/00AD).
- [x] **Trojan Source pattern defeated** — displayed order matches logical text. `the_trojan_source_pattern_is_defeated`.
- [x] Untrusted span isolated; unterminated override cannot affect adjacent trusted labels. `an_unterminated_override_cannot_affect_adjacent_trusted_text` — checks structural properties (no live control survives escaping; isolate marks present), not a full Unicode Bidi Algorithm resolution, which this crate does not implement.
- [ ] **Escaping is render-time only; stored values keep byte fidelity** — verified against audit and transcript paths. Verified against the audit path only (`approval::coordinator`'s pre-existing sentinel test, unchanged). **Not verified against a transcript path** — no RFC-011 transcript code calls into `text_safety`, so there is no integration point yet. Left unchecked rather than checked on the audit half alone.
- [ ] Applied to: approval, trust, paste, destructive, safe-close dialogs. Only the approval surface's argv rendering exists (`approval::coordinator`, pre-existing, now sharing this module's escaping); no trust/paste/destructive/safe-close dialog exists anywhere in the tree — RFC-015's shell is unimplemented.
- [ ] Applied to: Project Board rows (branch and project names). No Project Board rendering surface exists yet.
- [ ] Applied to: notifications, audit viewer, transcript viewer. None of these surfaces exist yet.
- [ ] **Editor surface exception implemented deliberately** and documented. No editor surface exists yet to except.
- [ ] **Terminal surface exception implemented deliberately** — no escaping, no reordering. The terminal genuinely does not reorder or escape (confirmed by RFC-014 C10 and the gui-spike filter tests), but this is pre-existing absence, not something this slice implemented as a deliberate opt-out of a shared path — left unchecked to avoid claiming an implementation that isn't there.
- [x] Escaping lives on the shared render path; bypass requires deliberate effort — **for the one real call site that exists.** `approval::coordinator` can no longer diverge without deliberately reimplementing the `Cf` table itself; ablation-verified (breaking the shared function breaks both the shared tests and `approval`'s pre-existing tests identically). The wider claim (bypass-resistant across a whole shell crate) awaits real UI surfaces to make it meaningful.
- [x] Legitimate RTL chrome renders naturally, unescaped. `legitimate_rtl_letters_are_not_escaped`, at the primitive level (no chrome exists yet to render it in).

## Catalog and Locale Checklist

- [ ] Catalog format chosen; **dependency cost measured and recorded** as a lockfile delta.
- [ ] Decision reasoning recorded, not just the outcome.
- [ ] Source-locale catalog compiled into the binary.
- [ ] Lookup replaces RFC-015's placeholder without changing the call shape.
- [ ] Locale precedence: CLI flag → configuration → OS locale → source locale.
- [ ] Fallback chain: locale → language → source → key.
- [ ] Missing key renders the key. **Never blank, never panic.**
- [ ] Missing-key events logged at debug level, not as audit events.

## Pluralization and Interpolation Checklist

- [ ] Plural categories correct for a language whose rules differ from English.
- [ ] Interpolation works.
- [ ] Interpolation cannot inject markup or escape PR-016-C quoting.
- [ ] Second locale added purely to prove the machinery.

## Never-Localized Checklist

- [ ] Commands, argv, paths, file names untranslated.
- [ ] Identifiers untranslated.
- [ ] Terminal output, transcripts, file contents untranslated.
- [ ] Git branch, commit, author strings untranslated.
- [ ] Audit field values and reason codes untranslated.

## RTL Checklist

- [ ] RTL text renders correctly in chrome and dialogs.
- [ ] RTL renders correctly in the editor surface.
- [ ] Terminal non-reordering documented as correct, not as a gap.
- [ ] Terminal wide-cell CJK gap recorded as RFC-017's ownership.
- [ ] RTL layout mirroring recorded as deferred, not claimed.

## Enforcement Checklist

- [ ] No-hardcoded-strings scan exists.
- [ ] **Scan demonstrated to catch a deliberately introduced literal** — shown, not asserted.
- [ ] Catalog-completeness test: every referenced key exists in the source locale.
- [ ] Unused-key report advisory only.

## Evidence Required

- [x] Commit/PR list; gate output. See qa-evidence.md PR-016-C section (this slice only; B/D/E/F pending).
- [x] Bidi corpus results including the Trojan Source case. `crates/tekstide-core/src/text_safety/tests.rs`.
- [ ] Byte-fidelity test results for audit and transcript paths. Audit path only; no transcript integration point exists yet.
- [ ] Dependency-cost measurement. PR-016-B scope, not yet started.
- [ ] Screenshot of a non-Latin locale rendering the shell. Requires RFC-015's shell to exist first.
- [x] Known limitations; answers to the RFC's open questions. Open Question 1 (escaping function's home) answered by the README's 2026-07-30 addendum: `tekstide-core`, shared — see qa-evidence.md.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.
- [ ] Blocked — text safety cannot be applied reliably at a shared render path.

Reviewer notes:

```text
Pending implementation.
```
