# RFC-008: TerminalSession and Process Lifecycle — QA Evidence

Status: PR-008-C implementation ready for review
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

### PR-008-B — Project-Owned PTY Shell Launch

Status: ready for implementation review.

Implemented:

- Added `LinuxTerminalRuntime::launch_project_shell`.
- Split terminal runtime implementation into focused `types`, `pty`, `launch`, and test modules to keep future PR-008-C/PR-008-D changes reviewable.
- Launch accepts a `ProjectSession` plus `TerminalLaunchSpec` and returns `TerminalSession` metadata plus launch/process-started runtime events.
- Launch validates project ownership before creating a process.
- Launch validates ProjectSession canonical root exists and is a directory.
- Launch validates requested cwd exists, is a directory, and remains inside the canonical project root.
- Launch currently supports only `TerminalKind::Plain`.
- Launch rejects missing, non-file, and non-executable shell paths before PTY creation.
- Launch creates a real Linux PTY with `openpty`.
- Launch starts `/bin/sh` or the requested shell through the PTY with minimal environment and no explicit login/startup-file loading.
- Launch starts the shell with `setsid` and `TIOCSCTTY` so the PTY is the controlling terminal.
- Launch transitions the returned `TerminalSession` to `Running` only after process spawn succeeds.
- Runtime-owned process and PTY handles are stored inside `LinuxTerminalRuntime`, not inside `TerminalSession`.
- Added launch-smoke helpers for writing a marker command, reading available PTY bytes, and waiting for the shell to exit. These are only enough to prove launch/output in this slice; full bounded IO event plumbing remains PR-008-C and process-group termination remains PR-008-D.

Observed gates on 2026-07-10:

- `cargo test -p tekstide-core runtime::terminal::tests::linux_runtime` passed; 3 Linux runtime launch/rejection tests passed.
- `cargo test -p tekstide-core runtime::terminal::tests` passed; 9 runtime tests passed after review follow-up for non-executable shell rejection.
- `cargo check --workspace` passed.
- `cargo fmt --check --all` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `cargo test --workspace` passed; 178 core tests passed and spike crate test target had 0 tests.

Security/privacy note:

- The launch smoke writes only `printf 'tekstide-runtime-ok\n'` plus `exit` into a synthetic temporary test root and checks for that marker. It does not print environment dumps, shell history, project files, token-like values, or private output.

Migration note:

- No local data schema or persisted state migration is introduced.

Known limitations:

- PR-008-B does not integrate launched terminals into `ProjectSession` collections.
- PR-008-B does not implement bounded output buffers beyond the launch smoke's immediate read helper.
- PR-008-B does not implement resize routing.
- PR-008-B does not implement process-group termination policy or safe-close behavior.
- PR-008-B does not implement RFC-009 ANSI/VT, paste, clipboard, or approval-dialog security policy.

Review follow-up:

- After review request 040 was accepted with notes, launch validation was tightened to reject non-executable shell files before PTY creation.
- `spawn_shell` now closes the duplicated controlling-terminal fd even when `Command::spawn` fails.

### PR-008-C — Bounded IO and Resize Event Plumbing

Status: ready for implementation review.

Implemented:

- Replaced the temporary unbounded PTY read helper with `LinuxTerminalRuntime::read_available_bounded_for`.
- PTY output reads now accept an explicit maximum buffered-byte cap before data is returned to callers.
- Output beyond the caller-supplied cap is dropped from the returned chunk and recorded in `TerminalOutputSummary::dropped_bytes`.
- `TerminalOutputSummary::truncated` is set when output was dropped.
- Input writes continue to route through `TerminalRuntimeHandle` identity and reject cross-project handles before writing to the PTY.
- Added `LinuxTerminalRuntime::resize` to route terminal dimension changes to the PTY master and emit a `TerminalRuntimeEvent::Resized` event.
- Added Linux PTY smoke tests for cross-project input rejection, bounded output truncation/drop accounting, and resize propagation through `stty size`.

Observed gates on 2026-07-10:

- `cargo test -p tekstide-core runtime::terminal::tests` passed; 12 runtime terminal tests passed.
- `cargo check --workspace` passed.
- `cargo fmt --check --all` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `cargo test --workspace` passed; 182 `tekstide-core` tests passed, `tekstide` had 0 tests, `tekstide-pty-spike` had 0 tests, and doc tests had 0 tests.
- `git diff --check` passed.

Security/privacy note:

- The bounded-output smoke uses synthetic shell output generated inside a temporary test root and asserts only byte-cap/drop behavior.
- The resize smoke reads only `stty size` output from the synthetic PTY session.
- This slice still does not implement RFC-009 ANSI/VT filtering, paste protection, clipboard policy, approval-dialog containment, transcript retention, command approval, or durable audit storage.

Migration note:

- No local data schema or persisted state migration is introduced.

Known limitations:

- PR-008-C does not integrate launched terminals into `ProjectSession` collections.
- PR-008-C does not implement process-group termination policy or safe-close behavior.
- PR-008-C does not implement RFC-009 ANSI/VT, paste, clipboard, or approval-dialog security policy.
- PR-008-C does not introduce transcript persistence or durable audit records.

Required future evidence will be recorded per later implementation slice:

- process-group termination evidence;
- ProjectSession visible-slot and mode-switch evidence;
- safe-close evidence;
- security and privacy notes;
- migration note;
- known limitations.

## Recommendation

Pending.
