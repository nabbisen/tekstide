# RFC-008: TerminalSession and Process Lifecycle — QA Evidence

Status: PR-008-A implementation ready for review
Date opened: 2026-07-10

## Scope

RFC-008 implements production-oriented TerminalSession/process lifecycle foundations. This evidence file must not be used to claim AgentRun launch, transcript retention, durable audit storage, production ANSI/VT safety, clipboard policy, command approval, or final GUI terminal behavior.

## Implementation Evidence

RFC-008 design/handoff review was accepted with notes on 2026-07-10 in `.git-exclude/reviewed/tekstide-review-request-038-rfc008-terminalsession-process-lifecycle-design-response.md`.

### PR-008-A — Runtime Boundary and Lifecycle Model

Status: ready for implementation review.

Implemented:

- `TerminalSession` status is now changed through `transition_to` instead of direct public status mutation.
- `TerminalSession` exposes `status()`, `visible_slot()`, and `assign_visible_slot()` accessors/helpers.
- Invalid terminal lifecycle transitions return `TerminalTransitionError`.
- Added `runtime::terminal` boundary types for launch specs, runtime handles, runtime snapshots, runtime events, bounded output summaries, termination requests, termination signals, termination outcomes, terminal dimensions, and bounded runtime summaries.
- Runtime boundary types carry `TerminalId` and `ProjectId` identity only; they do not introduce PTY file descriptors, child handles, reader threads, PIDs, transcript bytes, or durable audit storage.
- Added tests for valid/invalid terminal transitions, orphaned/unknown recovery, visible-slot assignment, plain shell launch specs, runtime handle identity, output truncation summaries, bounded runtime summaries, and termination request summaries.

Observed gates on 2026-07-10:

- `cargo test -p tekstide-core domain::tests::terminal_lifecycle` passed; 4 tests passed.
- `cargo test -p tekstide-core runtime::terminal::tests` passed; 5 tests passed.
- `cargo check --workspace` passed.
- `cargo fmt --check --all` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `cargo test --workspace` passed; 175 core tests passed and spike crate test target had 0 tests.

Security/privacy note:

- PR-008-A is model-only. It does not launch processes, read terminal output, write terminal input, inspect environment variables, persist runtime handles, capture transcripts, write audit records, or add GUI behavior.

Migration note:

- No local data schema or persisted state migration is introduced.

Known limitations:

- PR-008-A does not start a shell, create a PTY, route IO, resize, terminate a process group, update safe-close from real runtime state, or implement RFC-009 security policy.

Required future evidence will be recorded per later implementation slice:

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
