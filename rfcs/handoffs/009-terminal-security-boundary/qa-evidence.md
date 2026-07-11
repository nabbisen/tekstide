# RFC-009: Terminal Security Boundary — QA Evidence

Status: design accepted; implementation pending
Date opened: 2026-07-11

## Scope

RFC-009 defines Tekstide's terminal security boundary for untrusted terminal output and terminal input. Evidence must not be used to claim AgentRun launch, transcript retention, durable audit storage, managed command approval, final GUI terminal behavior, or full terminal emulator compatibility unless later reviewed implementation explicitly supports those claims.

## Design Review

RFC-009 design/handoff review was accepted with notes on 2026-07-11 in `.git-exclude/reviewed/tekstide-review-request-047-rfc009-terminal-security-boundary-design-response.md`.

Carry-forward requirements:

- PR-009-B must pin exact accepted and inert sequence families before claiming parser coverage.
- Terminal-generated replies must be blocked by default or implemented as bounded terminal-local capabilities with tests.
- Paste blocking must use active/modal trusted UI state, not focus alone.
- Diagnostics must include sequence-family and policy-reason metadata without raw private payloads.

## Implementation Evidence

Pending.

### PR-009-A — Security Policy Model

Status: pending.

Evidence to record:

- policy type/module locations;
- supported terminal-local effect vocabulary;
- blocked app-level effect vocabulary;
- bounded diagnostic behavior;
- tests run.

### PR-009-B — ANSI/VT/OSC Parser Boundary

Status: pending.

Evidence to record:

- supported sequence families;
- blocked or inert sequence families;
- OSC 52 behavior;
- title/app-chrome behavior;
- private/control sequence behavior;
- tests run.

### PR-009-C — Paste Policy

Status: pending.

Evidence to record:

- typed/single-line/multiline/control-containing paste classification;
- allow/block/requires-confirmation behavior;
- proof that confirmation-required bytes are withheld before PTY write;
- cross-project paste rejection;
- trusted UI active-state behavior;
- tests run.

### PR-009-D — Trusted UI / Spoofing Boundary

Status: pending.

Evidence to record:

- trusted UI state model;
- approval-like terminal-output spoofing examples;
- label/read-model checks;
- tests run.

### PR-009-E — Closeout Evidence

Status: pending.

Evidence to record:

- final implemented scope;
- known limitations;
- security/privacy note;
- migration note;
- future-work alignment;
- closeout recommendation.

## Recommendation

Pending.
