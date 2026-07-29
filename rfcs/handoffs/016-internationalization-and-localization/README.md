# RFC-016: Internationalization and Localization - Developer Handoff Pack

Source RFC: [RFC-016](../../proposed/016-internationalization-and-localization.md)
Target milestone: **M8**
Source RFC status: **Proposed**

**Start here.** This file is the entry point. Everything is linked below in reading order.

## Read in this order

| # | Document | Purpose |
| --- | --- | --- |
| 1 | [RFC-016](../../proposed/016-internationalization-and-localization.md) | Catalog, locale, fallback, RTL — **and the text-safety policy in §Security, which is the security-critical half.** |
| 2 | This file | Orientation and what is binding. |
| 3 | [`implementation-handoff.md`](./implementation-handoff.md) | Module layout, catalog decision, escaping implementation, enforcement. |
| 4 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Slice boundaries and review gates. |
| 5 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | Required evidence. |
| 6 | [`qa-evidence.md`](./qa-evidence.md) | Where you record gates, findings, and limitations. |

Read before starting:

- [RFC-015](../../proposed/015-application-shell-and-rendered-surface-model.md) — creates the i18n seam this RFC fills. If RFC-015 has not landed, PR-016-C (text safety) can still proceed; it has no catalog dependency.
- [RFC-009](../../done/009-terminal-security-boundary.md) — why terminal output is untrusted.

## Where to start work

**Begin at PR-016-B**, or PR-016-C if RFC-015's seam is not yet available. PR-016-A is design acceptance.

## Four things that are binding

1. **This RFC is half security.** Bidi control characters are currently handled *nowhere* in Tekstide, and the substrate obeys them. A command string containing U+202E renders reversed in an approval dialog. That is Trojan Source pointed at the product's most security-critical surface.
2. **Escape at render, never at ingest.** Stored values keep byte fidelity — RFC-013 audit records and RFC-011 transcripts must stay exact. Only the display is transformed. Transforming at ingest would corrupt evidence.
3. **The editor is a deliberate exception.** It must render file content as-is. An editor that silently rewrites content is broken. Implement the exception on purpose and document it.
4. **Fail visible, never blank.** A missing key renders the key. A UI that silently shows empty strings is worse than one showing an untranslated identifier.

## The distinction that governs everything here

**Untrusted text** (commands, branch names, terminal output, transcripts, file paths) is *quoted* — displayed but never obeyed.
**Trusted chrome** (labels, buttons, headings) is *localized* — translated and rendered naturally, including genuine RTL.

Getting this backwards in either direction is a defect: escaping legitimate Arabic UI text harms real users, and failing to escape an untrusted command string reintroduces the vulnerability.
