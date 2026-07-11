# RFC-008: TerminalSession and Process Lifecycle — Developer Handoff Pack

Source RFC: [RFC-008](../../done/008-terminalsession-process-lifecycle.md)
Target milestone: **M4**
Source RFC status: **Implemented with documented limitations**

## Files

- `implementation-handoff.md` — developer-facing runtime/lifecycle constraints and architecture notes.
- `task-breakdown-pr-plan.md` — recommended implementation slices and review gates.
- `acceptance-qa-checklist.md` — acceptance traceability, QA checklist, and evidence requirements.
- `qa-evidence.md` — placeholder for observed implementation gates, lifecycle smoke results, security notes, and known limitations.

Review disposition: RFC-008 design/handoff accepted with notes on 2026-07-10. See `.git-exclude/reviewed/tekstide-review-request-038-rfc008-terminalsession-process-lifecycle-design-response.md`.

Implementation closeout disposition: accepted with documented limitations on 2026-07-11. See `.git-exclude/reviewed/tekstide-review-request-046-rfc008-closeout-evidence-response.md`.

## Source Summary

RFC-008 turns the RFC-007 Linux PTY feasibility evidence into production-oriented TerminalSession/process lifecycle foundations: project-owned local shell launch, observed lifecycle state, process-group termination policy, hidden/background sessions, visible slot policy, and safe-close summaries for real running terminals.

RFC-009 security policy must be designed alongside this work before terminal security, paste-protection, clipboard, ANSI/VT, or approval-dialog spoofing claims are shipped.
