---
title: "RFC-009: Terminal Security Boundary — Acceptance / QA Checklist"
rfc: "RFC-009"
rfc_file: "../../done/009-terminal-security-boundary.md"
status: "Accepted with documented limitations"
target_milestone: "M4"
source_rfc_status: "Implemented with documented limitations"
created: "2026-07-11"
---

# RFC-009: Terminal Security Boundary — Acceptance / QA Checklist

## Acceptance Status

This checklist records accepted RFC-009 implementation closeout evidence. Acceptance means the project has reviewed the core model/security boundary and its documented limitations, not that final GUI terminal rendering, app/UI paste wiring, AgentRun launch, transcript retention, durable audit storage, or command approval are implemented.

## Scope Checklist

- [x] Terminal output bytes are treated as untrusted input.
- [x] Terminal effects are limited to terminal-local display/model state.
- [x] Unsupported control sequences fail inertly.
- [x] Paste is classified before PTY write.
- [x] Approval/security UI remains outside terminal output.
- [x] Plain/Supervised/Managed labels remain honest.
- [x] No AgentRun launch is introduced.
- [x] No transcript retention is introduced.
- [x] No durable audit storage is introduced.
- [x] No final GUI terminal widget claim is introduced.

## ANSI / VT / OSC Checklist

- [x] Supported sequence subset is documented.
- [x] Unsupported sequence behavior is documented.
- [x] Exact accepted and inert sequence families are enumerated before parser coverage is claimed.
- [x] OSC 52 clipboard behavior is blocked or inert.
- [x] App/window title mutation is blocked or terminal-local only.
- [x] Desktop notification / host integration sequences are blocked or inert.
- [x] Hyperlinks cannot auto-open or mutate app state.
- [x] Terminal-generated replies are blocked by default or explicitly bounded and terminal-local.
- [x] Invalid bytes are handled without leaking private output in diagnostics.

## Paste Checklist

- [x] Typed input and paste input are distinguished.
- [x] Single-line paste policy is explicit.
- [x] Multiline paste requires confirmation.
- [x] Control-containing paste is blocked or requires explicit policy.
- [x] Paste bytes requiring confirmation are not written to the PTY before decision.
- [x] Paste routing is ProjectId/TerminalId addressed.
- [x] Cross-project paste routing is rejected.
- [x] Paste is blocked or queued while trusted approval/security UI is active or modal.
- [x] Paste blocking does not depend only on focus state.

## Trusted UI / Spoofing Checklist

- [x] Approval/trust/paste/destructive dialogs are trusted app/native UI, not terminal output.
- [x] Terminal output cannot synthesize approve/reject/trust decisions.
- [x] Terminal output cannot mutate app chrome, Project Board state, or focus for trusted dialogs.
- [x] Spoofing examples remain terminal content only.
- [x] Plain terminals do not claim managed command approval.

## Security and Privacy Checklist

- [x] Diagnostics are bounded.
- [x] Diagnostics include sequence-family and policy-reason metadata for review.
- [x] Diagnostics avoid raw private terminal output.
- [x] Diagnostics avoid raw OSC payloads, pasted text, shell output, and environment-like values.
- [x] No clipboard writes occur from terminal output.
- [x] Terminal output cannot mutate trust state, approvals, command history, audit state, file buffers, or project metadata.
- [x] RFC-010/RFC-011/RFC-012 dependencies remain visible where relevant.

## Automated Test Checklist

- [x] Parser/policy tests cover supported sequence-family metadata.
- [x] Parser/policy tests cover blocked app-effect vocabulary.
- [x] Parser/policy tests cover terminal-generated reply policy vocabulary.
- [x] Parser-boundary tests keep OSC 52 as inert diagnostics without clipboard-write effects.
- [x] Parser-boundary tests keep title/app-chrome sequences as inert diagnostics without app-chrome effects.
- [x] Parser-boundary tests restrict terminal output to terminal-local effects or diagnostics, with no trust/approval/file/project effect type.
- [x] Paste policy tests cover typed, single-line, multiline, control-containing, and cross-project cases.
- [x] Tests prove confirmation-required paste bytes are withheld before PTY write.
- [x] Spoofing tests cover approval-like terminal output.
- [x] Label tests or evidence prove no command-approval overclaim.

## Release Evidence Required

Attach or link the following evidence before marking RFC-009 implemented:

- [x] Commit/PR list.
- [x] Test command output.
- [x] Supported/blocked sequence summary.
- [x] Paste policy evidence.
- [x] Spoofing-boundary evidence.
- [x] Security/privacy note.
- [x] Migration note or "no migration" statement.
- [x] Known limitations.
- [x] Follow-up RFCs/issues for GUI terminal, AgentRun, transcript, audit, and command approval work.

## Final Acceptance Decision

- [ ] Accepted as complete.
- [x] Accepted with documented limitations.
- [ ] Blocked pending fixes.
- [ ] Requires RFC amendment.

Reviewer notes:

```text
Design/handoff accepted with notes on 2026-07-11.
Closeout accepted with documented limitations on 2026-07-17.

Carry-forward notes:
- PR-009-B must enumerate exact accepted and inert sequence families before claiming parser coverage.
- Terminal-generated replies are explicit capabilities and are blocked by default.
- Paste blocking must use active/modal trusted UI state, not focus alone.
- Diagnostics need sequence-family and policy-reason metadata without raw private payloads.

Implemented foundation:
- Conservative terminal output parser/security boundary.
- Bounded diagnostics without raw private payloads.
- Paste classification and pre-PTY write decisions.
- Model-level trusted UI/spoofing boundary.
- Honest Plain/Supervised/Managed labels.

Documented limitations:
- No final GUI terminal renderer or terminal widget acceptance.
- No app/UI paste-event integration or rendered paste confirmation UI.
- No screenshot-backed visual spoofing evidence.
- No AgentRun launch, transcript retention, durable audit storage, or command approval.
```
