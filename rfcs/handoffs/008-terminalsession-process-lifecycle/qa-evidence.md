# RFC-008: TerminalSession and Process Lifecycle — QA Evidence

Status: Design/handoff accepted; implementation pending
Date opened: 2026-07-10

## Scope

RFC-008 implements production-oriented TerminalSession/process lifecycle foundations. This evidence file must not be used to claim AgentRun launch, transcript retention, durable audit storage, production ANSI/VT safety, clipboard policy, command approval, or final GUI terminal behavior.

## Evidence Placeholder

Implementation has not started. RFC-008 design/handoff review was accepted with notes on 2026-07-10 in `.git-exclude/reviewed/tekstide-review-request-038-rfc008-terminalsession-process-lifecycle-design-response.md`.

Required evidence will be recorded per accepted implementation slice:

- runtime/domain boundary evidence;
- project-owned PTY shell launch evidence;
- bounded output/input/resize evidence;
- process-group termination evidence;
- ProjectSession visible-slot and mode-switch evidence;
- safe-close evidence;
- security and privacy notes;
- migration note;
- known limitations.

## Recommendation

Pending.
