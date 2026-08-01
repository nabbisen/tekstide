# RFC-016: Internationalization and Localization - Developer Handoff Pack

Source RFC: [RFC-016](../../done/016-internationalization-and-localization.md)
Target milestone: **M8**
Source RFC status: **Proposed**

**Start here.** This file is the entry point. Everything is linked below in reading order.

## Read in this order

| # | Document | Purpose |
| --- | --- | --- |
| 1 | [RFC-016](../../done/016-internationalization-and-localization.md) | Catalog, locale, fallback, RTL — **and the text-safety policy in §Security, which is the security-critical half.** |
| 2 | This file | Orientation and what is binding. |
| 3 | [`implementation-handoff.md`](./implementation-handoff.md) | Module layout, catalog decision, escaping implementation, enforcement. |
| 4 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Slice boundaries and review gates. |
| 5 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | Required evidence. |
| 6 | [`qa-evidence.md`](./qa-evidence.md) | Where you record gates, findings, and limitations. |

Read before starting:

- [RFC-015](../../done/015-application-shell-and-rendered-surface-model.md) — creates the i18n seam this RFC fills. (Historical: when this pack was written RFC-015 had not landed, so PR-016-C was sequenced to proceed without it. Both closed 2026-08-01.)
- [RFC-009](../../done/009-terminal-security-boundary.md) — why terminal output is untrusted.

## Active work: PR-016-E (added 2026-08-01)

**Unblocked.** PR-016-E's scan scope depended on whether RFC-015 PR-015-D would delete `tekstide_core::shell::render_text`. It investigated and deliberately kept it (response 130), so the question is settled and this slice can start.

- **[`pr-016-e-enforcement.md`](./pr-016-e-enforcement.md)** — read this before `task-breakdown-pr-plan.md`'s PR-016-E entry, which is three lines from 2026-07-29 and predates everything this slice inherited.

`task-breakdown-pr-plan.md` still states the scope correctly; what it does not carry is the four hardcoded-string sites found across responses 122/123/128/132, the scan-scope decision those force, the Fluent-type-exposure guard folded in at response 126, or the overlap with RFC-015 PR-015-B's seam scans. Those accumulated in `qa-evidence.md`'s Known Limitations — the file where results are recorded, not the one you read before starting. The consolidation above is the fix.

## Historical: PR-016-B/C/D sequencing (added 2026-07-30)

**Land the two required fixes from response 116 before starting PR-016-C.** They close RFC-021 PR-021-E2:

- `.git-exclude/reviewed/tekstide-review-request-116-rfc021-pr021e2-coordination-and-audit-response.md`

Both are small (reclassify before authorizing; record a `cwd`-mismatch anomaly), confirmed by diff, no re-review. RFC-021 PR-021-F closeout is the architect's slice, not yours — you do not wait for it.

## PR-016-C has one added requirement, and it changes the answer to an Open Question

RFC-016 §Open Questions asked whether the escaping function should live in `tekstide-core` (shared with RFC-021's approval model) or stay shell-local, and reserved the decision for PR-016-C. **That decision is now forced: it lives in `tekstide-core` and is shared.**

Why: response 115 required a full Unicode `Cc`+`Cf` escaping implementation inside `approval::coordinator::display_argv`, because `ApprovalRequest::display_command` was being constructed there unsafely and could not wait. That was the reviewer's sequencing error, not yours — but it means **a second implementation of this RFC's security policy already exists in the tree**, which is exactly what §Risks warns against: *"escaping belongs to the shared untrusted-text render path, not to each surface."*

**So PR-016-C must:**

1. Adopt `approval::coordinator::display_argv`'s escaping (`Cc` + `Cf` by category, `<U+XXXX>` markers, per-entry POSIX quoting, empty entries visible) as the **canonical** implementation, in whatever `tekstide-core` module this RFC gives it.
2. Make `approval::coordinator` call it and delete its private copy — the argv-specific quoting may stay in `approval` if it is argv-shaped rather than text-shaped, but the character-escaping half must not be duplicated.
3. Keep the no-confusables boundary already recorded in that module's doc comment. Invisible characters are unambiguously wrong in a security display; visible non-Latin characters are the point of having i18n. Do not relitigate it, and do not extend to homoglyphs.
4. Carry over the ten-codepoint fixture set already in `approval::tests::coordinator` (U+202E, U+2066, U+200E, U+200F, U+061C, U+200B, U+200C, U+200D, U+00AD, U+FEFF) into this RFC's bidi corpus, so the canonical implementation is tested at least as hard as the copy it replaces.

## Where to start work

**Begin at PR-016-C — the text-safety slice — not at PR-016-B.**

This ordering is deliberate and was approved by the human owner on 2026-07-29. PR-016-C has no catalog dependency, and the vulnerability it closes is live *now*: RFC-021's approval model is under implementation, and RFC-022 will render its dialog. The bidi fix must exist before any surface displays an adapter-supplied command string.

Do not reorder this back to numeric sequence for tidiness. B, D, and E are ordinary localization plumbing and can follow.

PR-016-A is design acceptance.

## Four things that are binding

1. **This RFC is half security.** Bidi control characters are handled in exactly *one* place in Tekstide — `approval::coordinator::display_argv`, added under review pressure in PR-021-E1 — and nowhere else; the substrate obeys them. A command string containing U+202E renders reversed in any surface that has not been taught otherwise. That is Trojan Source pointed at the product's most security-critical surface. (Corrected 2026-07-30: this line previously read "handled *nowhere*", which was true when RFC-016 was authored on 2026-07-29 and stopped being true the next day. Consolidating that one implementation is now part of PR-016-C — see above.)
2. **Escape at render, never at ingest.** Stored values keep byte fidelity — RFC-013 audit records and RFC-011 transcripts must stay exact. Only the display is transformed. Transforming at ingest would corrupt evidence.
3. **The editor is a deliberate exception.** It must render file content as-is. An editor that silently rewrites content is broken. Implement the exception on purpose and document it.
4. **Fail visible, never blank.** A missing key renders the key. A UI that silently shows empty strings is worse than one showing an untranslated identifier.

## The distinction that governs everything here

**Untrusted text** (commands, branch names, terminal output, transcripts, file paths) is *quoted* — displayed but never obeyed.
**Trusted chrome** (labels, buttons, headings) is *localized* — translated and rendered naturally, including genuine RTL.

Getting this backwards in either direction is a defect: escaping legitimate Arabic UI text harms real users, and failing to escape an untrusted command string reintroduces the vulnerability.
