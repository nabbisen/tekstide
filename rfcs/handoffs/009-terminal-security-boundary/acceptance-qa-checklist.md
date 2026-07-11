---
title: "RFC-009: Terminal Security Boundary — Acceptance / QA Checklist"
rfc: "RFC-009"
rfc_file: "../../proposed/009-terminal-security-boundary.md"
status: "Accepted for implementation"
target_milestone: "M4"
source_rfc_status: "Proposed"
created: "2026-07-11"
---

# RFC-009: Terminal Security Boundary — Acceptance / QA Checklist

## Acceptance Status

This checklist is prepared for RFC-009 design review. Acceptance means the project has reviewed the terminal security boundary and implementation plan, not that production terminal security behavior is already implemented.

## Scope Checklist

- [ ] Terminal output bytes are treated as untrusted input.
- [ ] Terminal effects are limited to terminal-local display/model state.
- [ ] Unsupported control sequences fail inertly.
- [ ] Paste is classified before PTY write.
- [ ] Approval/security UI remains outside terminal output.
- [ ] Plain/Supervised/Managed labels remain honest.
- [ ] No AgentRun launch is introduced.
- [ ] No transcript retention is introduced.
- [ ] No durable audit storage is introduced.
- [ ] No final GUI terminal widget claim is introduced.

## ANSI / VT / OSC Checklist

- [ ] Supported sequence subset is documented.
- [ ] Unsupported sequence behavior is documented.
- [ ] Exact accepted and inert sequence families are enumerated before parser coverage is claimed.
- [ ] OSC 52 clipboard behavior is blocked or inert.
- [ ] App/window title mutation is blocked or terminal-local only.
- [ ] Desktop notification / host integration sequences are blocked or inert.
- [ ] Hyperlinks cannot auto-open or mutate app state.
- [ ] Terminal-generated replies are blocked by default or explicitly bounded and terminal-local.
- [ ] Invalid bytes are handled without leaking private output in diagnostics.

## Paste Checklist

- [ ] Typed input and paste input are distinguished.
- [ ] Single-line paste policy is explicit.
- [ ] Multiline paste requires confirmation.
- [ ] Control-containing paste is blocked or requires explicit policy.
- [ ] Paste bytes requiring confirmation are not written to the PTY before decision.
- [ ] Paste routing is ProjectId/TerminalId addressed.
- [ ] Cross-project paste routing is rejected.
- [ ] Paste is blocked or queued while trusted approval/security UI is active or modal.
- [ ] Paste blocking does not depend only on focus state.

## Trusted UI / Spoofing Checklist

- [ ] Approval/trust/paste/destructive dialogs are trusted app/native UI, not terminal output.
- [ ] Terminal output cannot synthesize approve/reject/trust decisions.
- [ ] Terminal output cannot mutate app chrome, Project Board state, or focus for trusted dialogs.
- [ ] Spoofing examples remain terminal content only.
- [ ] Plain terminals do not claim managed command approval.

## Security and Privacy Checklist

- [ ] Diagnostics are bounded.
- [ ] Diagnostics include sequence-family and policy-reason metadata for review.
- [ ] Diagnostics avoid raw private terminal output.
- [ ] Diagnostics avoid raw OSC payloads, pasted text, shell output, and environment-like values.
- [ ] No clipboard writes occur from terminal output.
- [ ] Terminal output cannot mutate trust state, approvals, command history, audit state, file buffers, or project metadata.
- [ ] RFC-010/RFC-011/RFC-012 dependencies remain visible where relevant.

## Automated Test Checklist

- [ ] Parser/policy tests cover supported sequence families.
- [ ] Parser/policy tests cover blocked sequence families.
- [ ] Parser/policy tests cover terminal-generated reply policy.
- [ ] Tests prove OSC 52 does not mutate clipboard.
- [ ] Tests prove title/app-chrome sequences do not mutate app chrome.
- [ ] Tests prove terminal output cannot mutate trust/approval/file/project state.
- [ ] Paste policy tests cover typed, single-line, multiline, control-containing, and cross-project cases.
- [ ] Tests prove confirmation-required paste bytes are withheld before PTY write.
- [ ] Spoofing tests cover approval-like terminal output.
- [ ] Label tests or evidence prove no command-approval overclaim.

## Release Evidence Required

Attach or link the following evidence before marking RFC-009 implemented:

- [ ] Commit/PR list.
- [ ] Test command output.
- [ ] Supported/blocked sequence summary.
- [ ] Paste policy evidence.
- [ ] Spoofing-boundary evidence.
- [ ] Security/privacy note.
- [ ] Migration note or "no migration" statement.
- [ ] Known limitations.
- [ ] Follow-up RFCs/issues for GUI terminal, AgentRun, transcript, audit, and command approval work.

## Final Acceptance Decision

- [ ] Accepted as complete.
- [ ] Accepted with documented limitations.
- [ ] Blocked pending fixes.
- [ ] Requires RFC amendment.

Reviewer notes:

```text
Design/handoff accepted with notes on 2026-07-11.

Carry-forward notes:
- PR-009-B must enumerate exact accepted and inert sequence families before claiming parser coverage.
- Terminal-generated replies are explicit capabilities and are blocked by default.
- Paste blocking must use active/modal trusted UI state, not focus alone.
- Diagnostics need sequence-family and policy-reason metadata without raw private payloads.
```
