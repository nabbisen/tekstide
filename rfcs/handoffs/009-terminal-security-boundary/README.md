# RFC-009: Terminal Security Boundary — Developer Handoff Pack

Source RFC: [RFC-009](../../done/009-terminal-security-boundary.md)
Target milestone: **M4**
Source RFC status: **Implemented with documented limitations**

## Files

- `implementation-handoff.md` — developer-facing boundary constraints and policy guidance.
- `task-breakdown-pr-plan.md` — recommended implementation slices and review gates.
- `acceptance-qa-checklist.md` — acceptance traceability, QA checklist, and evidence requirements.
- `qa-evidence.md` — placeholder for observed implementation gates, security notes, and known limitations.

Review disposition:

- RFC-009 design/handoff accepted with notes on 2026-07-11. See `.git-exclude/reviewed/tekstide-review-request-047-rfc009-terminal-security-boundary-design-response.md`.
- RFC-009 closeout accepted with documented limitations on 2026-07-17. See `.git-exclude/reviewed/tekstide-review-request-055-rfc009-closeout-evidence-response.md`.

This handoff inherits the source RFC lifecycle state. RFC-009 is implemented with documented limitations and now lives in `rfcs/done/`.

Design review notes to carry into implementation:

- PR-009-B must pin exact accepted and inert sequence families before claiming parser coverage.
- Terminal-generated replies must be blocked by default or implemented as bounded terminal-local capabilities with tests.
- Paste blocking must use active/modal trusted UI state, not focus alone.
- Diagnostics must preserve sequence-family/policy-reason metadata without raw private payloads.

## Source Summary

RFC-009 defines the terminal security boundary for untrusted terminal output and terminal input. It covers the supported ANSI/VT subset, unsupported sequence behavior, OSC clipboard blocking, paste interception, multiline paste confirmation state, approval-dialog spoofing boundaries, and honest Plain/Supervised/Managed labels.

RFC-009 does not implement the final GUI terminal widget, AgentRun launch, transcript retention, durable audit storage, or managed command approval.
