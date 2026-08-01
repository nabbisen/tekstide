---
title: "RFC-016: Internationalization and Localization - Task Breakdown and PR Plan"
rfc: "RFC-016"
rfc_file: "../../done/016-internationalization-and-localization.md"
target_milestone: "M8"
created: "2026-07-29"
updated: "2026-07-29"
---

# RFC-016 Task Breakdown and PR Plan

Six slices. **PR-016-C is implemented first**, ahead of PR-016-B — see Sequencing.

## PR-016-A — Design and handoff acceptance

Maintainer sign-off on the catalog recommendation, the fallback chain, the escape-and-isolate policy, and the editor/terminal exceptions.

## PR-016-B — Catalog, locale selection, fallback

Scope: catalog format decision with **measured** dependency cost; source-locale catalog compiled into the binary; lookup replacing RFC-015's placeholder without changing the call shape; locale selection precedence; fallback chain.

Review gate:

- Dependency cost recorded as a `Cargo.lock` delta, with the decision reasoning visible — not just the outcome.
- Missing locale, missing region, and missing key each render something visible; never blank, never panic.
- RFC-015's call shape unchanged.

## PR-016-C — Text safety: escape and isolate

**The security-critical slice, and the first to implement.** It has no catalog dependency, so it does not wait on PR-016-B.

Scope: `quote_untrusted`; application across all trusted surfaces; deliberate editor and terminal exceptions.

Review gate:

- **Trojan Source pattern defeated** — displayed order matches logical argv.
- Isolation holds against an unterminated directionality override.
- **Byte fidelity preserved** in audit and transcript paths — escaping is render-time only.
- Legitimate RTL chrome renders unescaped.
- Escaping lives on the shared render path; bypassing it requires deliberate effort.
- Editor and terminal exceptions implemented on purpose and documented.

If the type system can make untrusted text unrenderable without passing through the escaper, prefer that and say so.

## PR-016-D — Pluralization and interpolation

Scope: plural categories and parameter interpolation, proven with a second locale.

Review gate: plural rules correct for a language whose categories differ from English; interpolation cannot be used to inject markup or escape the quoting from PR-016-C.

## PR-016-E — Enforcement

Scope: no-hardcoded-strings scan; catalog-completeness test; advisory unused-key report.

Review gate: the scan actually catches a deliberately introduced hardcoded string — demonstrate it, do not assert it.

## PR-016-F — Closeout evidence

Scope: checklist, QA evidence, known limitations (confusables, RTL layout mirroring, terminal wide-CJK ownership), answers to the RFC's open questions.

## Sequencing

**C first.** Then B → D, with E after B, and F last.

The reason C leads: the bidi vulnerability it closes is live on `main` today, RFC-021's approval model is under implementation, and RFC-022 will render a dialog showing adapter-supplied command text. Landing text safety before any such surface exists is materially cheaper and safer than retrofitting it afterwards. C has no catalog dependency, so nothing is gained by making it wait.

Approved by the human owner, 2026-07-29.

**If PR-016-C finds that escaping cannot be reliably applied at a shared render path**, stop and escalate before surfaces multiply — retrofitting text safety across six surfaces is far more expensive than fixing the render path now.
