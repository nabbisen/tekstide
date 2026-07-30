---
title: "RFC-016: Internationalization and Localization - Acceptance / QA Checklist"
rfc: "RFC-016"
rfc_file: "../../proposed/016-internationalization-and-localization.md"
status: "Proposed — implementation in progress (PR-016-C, PR-016-B complete and accepted; PR-016-D landed 2026-07-30, not yet reviewed; PR-016-E waits for RFC-015)"
target_milestone: "M8"
source_rfc_status: "Proposed"
created: "2026-07-29"
updated: "2026-07-30"
---

# RFC-016 Acceptance / QA Checklist

**A checked box means evidence exists, not that the result was favourable.**

## Text Safety Checklist (PR-016-C) — security-critical

- [x] U+202A..U+202E escaped to a visible representation, not obeyed. `text_safety::escape_untrusted_chars`; `every_bidi_and_format_probe_escapes_to_its_visible_marker` covers U+202E explicitly, the full range is covered by the `is_format_char` range check itself.
- [x] U+2066..U+2069 escaped. `the_full_isolate_initiator_and_terminator_range_is_escaped` covers all four explicitly, including U+2069 (PDI) — the same codepoint used to close `quote_untrusted`'s own isolate wrap.
- [x] Zero-width joiner/non-joiner, zero-width space, soft hyphen escaped. Covered in the ten-codepoint probe (U+200B/200C/200D/00AD).
- [x] Other invisible or format characters, including unassigned-but-rendering-invisible codepoints, made visible on the same principle. Response 118 Required 1: extended to Unicode's `Default_Ignorable_Code_Point` property after a probe found U+3164/U+FFA0/U+115F/U+1160 (Hangul filler letters, category `Lo`) and U+2065 (unassigned, `Cn`, named explicitly by this requirement) passing through unescaped under the earlier `Cc`+`Cf` rule. `every_default_ignorable_extra_probe_escapes_to_its_visible_marker`, `the_hangul_filler_display_spoofing_attack_is_defeated`. U+2800 BRAILLE PATTERN BLANK decided explicitly as **not** escaped (`braille_pattern_blank_is_not_escaped`) — not `Default_Ignorable`, and Braille content is legitimate text. Full `Cn` coverage remains deferred to PR-016-B's dependency measurement, not hand-rolled further.
- [x] **Trojan Source pattern defeated** — displayed order matches logical text. `the_trojan_source_pattern_is_defeated`.
- [x] Untrusted span isolated; unterminated override cannot affect adjacent trusted labels. `an_unterminated_override_cannot_affect_adjacent_trusted_text` — checks structural properties (no live control survives escaping; isolate marks present), not a full Unicode Bidi Algorithm resolution, which this crate does not implement.
- [ ] **Escaping is render-time only; stored values keep byte fidelity** — verified against audit and transcript paths. Verified against the audit path only (`approval::coordinator`'s pre-existing sentinel test, unchanged). **Not verified against a transcript path** — no RFC-011 transcript code calls into `text_safety`, so there is no integration point yet. Left unchecked rather than checked on the audit half alone.
- [ ] Applied to: approval, trust, paste, destructive, safe-close dialogs. Only the approval surface's argv rendering exists (`approval::coordinator`, pre-existing, now sharing this module's escaping); no trust/paste/destructive/safe-close dialog exists anywhere in the tree — RFC-015's shell is unimplemented.
- [ ] Applied to: Project Board rows (branch and project names). No Project Board rendering surface exists yet.
- [ ] Applied to: notifications, audit viewer, transcript viewer. None of these surfaces exist yet.
- [ ] **Editor surface exception implemented deliberately** and documented. No editor surface exists yet to except.
- [ ] **Terminal surface exception implemented deliberately** — no escaping, no reordering. The terminal genuinely does not reorder or escape (confirmed by RFC-014 C10 and the gui-spike filter tests), but this is pre-existing absence, not something this slice implemented as a deliberate opt-out of a shared path — left unchecked to avoid claiming an implementation that isn't there.
- [x] Escaping lives on the shared render path; bypass requires deliberate effort — **for the one real call site that exists.** `approval::coordinator` can no longer diverge without deliberately reimplementing the escape tables itself (`is_format_char` is now private, per response 118 Required 3, precisely to prevent a caller assembling a divergent rule from its parts); ablation-verified (breaking the shared function breaks both the shared tests and `approval`'s pre-existing tests identically). The wider claim (bypass-resistant across a whole shell crate) awaits real UI surfaces to make it meaningful.
- [x] Legitimate RTL chrome renders naturally, unescaped. `legitimate_rtl_letters_are_not_escaped`, at the primitive level (no chrome exists yet to render it in).

## Catalog and Locale Checklist

- [x] Catalog format chosen; **dependency cost measured and recorded**, scoped to the shipped `tekstide` binary. **Corrected per response 122** — the original `git diff Cargo.lock` measurement scored the whole workspace lock (including `tekstide-gui-spike`'s `iced` tree), not what `tekstide` actually ships. Re-measured with `cargo tree -p tekstide --edges normal`, before/after: whole binary tree **39 packages** (23 baseline + **16 net new**: Fluent/`unic-langid` +15, `sys-locale` +1 — not the originally-reported +0).
- [x] Decision reasoning recorded, not just the outcome. Compared against RFC-014 R3's iced precedent (+345 packages) in `qa-evidence.md`; +12 for native plurals/gendered forms/interpolation/asymmetric translation judged proportionate.
- [x] Source-locale catalog compiled into the binary. `crates/tekstide/locales/en.ftl` via `include_str!` in `i18n/catalog.rs`; `source_locale_bundle()` takes no disk path at all.
- [ ] **Lookup replaces RFC-015's placeholder without changing the call shape.** Not met as literally stated — **RFC-015 has not been implemented**, so no placeholder exists to replace and no call shape exists to preserve. This slice establishes the call shape (`i18n::Catalog::resolve`/`Catalog::get`) instead, documented as a disclosed sequencing gap in `qa-evidence.md`, not a silent substitution. Left unchecked pending the reviewer's view on whether establishing the shape now satisfies the intent.
- [x] Locale precedence: CLI flag → configuration → OS locale → source locale. `Catalog::resolve`; `cli_flag_takes_precedence_over_configured_locale` (both supplied, confirms CLI wins), ablation-verified.
- [x] Fallback chain: locale → language → source → key. `Catalog::get`; `a_region_specific_locale_falls_back_to_its_language_subtag_catalog`, `a_key_missing_from_a_loaded_locale_falls_back_to_the_source_value`, `a_missing_key_renders_as_the_key_itself_never_blank` — all three links ablation-verified independently.
- [x] Missing key renders the key. **Never blank, never panic.** `a_missing_key_renders_as_the_key_itself_never_blank`; `no_locales_directory_falls_back_to_source_without_panicking` covers the no-locales-directory-at-all case too.
- [x] Missing-key events logged at debug level, not as audit events. `log_missing_key`, gated on `cfg!(debug_assertions)`; plain `eprintln!`, no audit-store call anywhere near it.

## Pluralization and Interpolation Checklist

- [x] Plural categories correct for a language whose rules differ from English. Polish (`one`/`few`/`many`/`other` vs. English's `one`/`other`); `plural_categories_differ_correctly_for_polish`, ablation-verified (collapsing `few`/`many` to identical text made the test fail).
- [x] Interpolation works. `Catalog::get_with_args`; numeric and symbolic-string arguments both proven, through the real shipped `en.ftl`/`pl.ftl`.
- [x] Interpolation cannot inject markup or escape PR-016-C quoting. `interpolated_values_cannot_inject_fluent_syntax` (FTL-lookalike text renders as a literal, never re-parsed); `interpolation_does_not_substitute_for_text_safety_escaping` (a bidi override survives raw, proving interpolation is not a substitute for `text_safety::escape_untrusted_chars` and must not be treated as one). Both documented directly in `i18n.rs`'s module doc, not only here.
- [x] Second locale added purely to prove the machinery. Polish (`pl`), loaded from the real shipped `crates/tekstide/locales/pl.ftl`, not a test-only fixture. Translation quality unreviewed, per RFC-016 §Non-Goals.

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
- [x] Dependency-cost measurement. Corrected per response 122: `cargo tree -p tekstide --edges normal`, scoped to the shipped binary, not the workspace lock. 39 packages total; +16 net new (Fluent/`unic-langid` +15, `sys-locale` +1). See `qa-evidence.md` PR-016-B section.
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
