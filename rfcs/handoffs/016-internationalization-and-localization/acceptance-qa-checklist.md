---
title: "RFC-016: Internationalization and Localization - Acceptance / QA Checklist"
rfc: "RFC-016"
rfc_file: "../../proposed/016-internationalization-and-localization.md"
status: "Proposed — implementation pending"
target_milestone: "M8"
source_rfc_status: "Proposed"
created: "2026-07-29"
updated: "2026-07-29"
---

# RFC-016 Acceptance / QA Checklist

**A checked box means evidence exists, not that the result was favourable.**

## Text Safety Checklist (PR-016-C) — security-critical

- [ ] U+202A..U+202E escaped to a visible representation, not obeyed.
- [ ] U+2066..U+2069 escaped.
- [ ] Zero-width joiner/non-joiner, zero-width space, soft hyphen escaped.
- [ ] **Trojan Source pattern defeated** — displayed order matches logical argv.
- [ ] Untrusted span isolated; unterminated override cannot affect adjacent trusted labels.
- [ ] **Escaping is render-time only; stored values keep byte fidelity** — verified against audit and transcript paths.
- [ ] Applied to: approval, trust, paste, destructive, safe-close dialogs.
- [ ] Applied to: Project Board rows (branch and project names).
- [ ] Applied to: notifications, audit viewer, transcript viewer.
- [ ] **Editor surface exception implemented deliberately** and documented.
- [ ] **Terminal surface exception implemented deliberately** — no escaping, no reordering.
- [ ] Escaping lives on the shared render path; bypass requires deliberate effort.
- [ ] Legitimate RTL chrome renders naturally, unescaped.

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

- [ ] Commit/PR list; gate output.
- [ ] Bidi corpus results including the Trojan Source case.
- [ ] Byte-fidelity test results for audit and transcript paths.
- [ ] Dependency-cost measurement.
- [ ] Screenshot of a non-Latin locale rendering the shell.
- [ ] Known limitations; answers to the RFC's open questions.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.
- [ ] Blocked — text safety cannot be applied reliably at a shared render path.

Reviewer notes:

```text
Pending implementation.
```
