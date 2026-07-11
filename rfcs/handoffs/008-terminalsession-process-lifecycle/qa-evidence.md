# RFC-008: TerminalSession and Process Lifecycle — QA Evidence

Status: Accepted with documented limitations
Date opened: 2026-07-10
Date accepted: 2026-07-11

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

### PR-008-D — Process-Group Termination and Lifecycle Closeout

Status: ready for implementation review.

Implemented:

- Added `runtime::terminal::termination` to keep process cleanup policy separate from launch and IO plumbing.
- `LinuxTerminalRuntime` now stores the launched process group id as runtime-only state.
- Added `LinuxTerminalRuntime::request_terminate`.
- Termination requests emit ordered lifecycle events for request receipt, signal sends, timeout, and final termination outcome.
- Termination sends SIGTERM to the process group rather than killing only the child process handle.
- SIGTERM timeout falls back to SIGKILL against the process group.
- SIGKILL timeout checks whether the process group remains observable with `kill(-pgid, 0)` and records `OrphanedUnknown` when cleanup cannot be proven.
- Termination finality is based on process-group disappearance, not only direct child exit.
- Direct child exit during termination handling no longer removes runtime session state while same-process-group descendants remain observable.
- `wait_for_exit` now maps Unix signal exits into bounded `TerminationOutcome` values instead of losing non-code exits.
- Cross-project termination handles are rejected before signal delivery.
- PTY creation now closes both raw descriptors if nonblocking setup fails after `openpty` succeeds.
- Added Linux smoke tests for SIGTERM process-group termination, cross-project termination rejection, and SIGKILL fallback for a foreground-child shell scenario.
- Added a regression test where the direct shell exits on SIGTERM while a same-process-group descendant ignores SIGTERM; the runtime now continues to timeout and SIGKILL fallback instead of overclaiming final termination.

Observed gates on 2026-07-10:

- `cargo test -p tekstide-core runtime::terminal::tests` passed; 15 runtime terminal tests passed.
- `cargo check --workspace` passed.
- `cargo fmt --check --all` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `cargo test --workspace` passed; 185 `tekstide-core` tests passed, `tekstide` had 0 tests, `tekstide-pty-spike` had 0 tests, and doc tests had 0 tests.
- `git diff --check` passed.

Review follow-up after `.git-exclude/reviewed/tekstide-review-request-042-rfc008-pr008d-process-group-termination-response.md`:

- Fixed the blocking issue where direct child exit was treated as sufficient proof of terminal process-group termination.
- Added process-group observation during termination waits. The runtime now keeps the process group id and session state available until `kill(-pgid, 0)` reports the group is gone or cleanup is honestly reported as unresolved.
- If the direct child exits after SIGTERM but the process group remains observable, termination continues to the SIGTERM timeout and SIGKILL fallback path.
- After SIGKILL, `KilledAfterTimeout` is reported only when process-group disappearance is observed; otherwise `OrphanedUnknown` remains the honest unresolved outcome.

Observed follow-up gates on 2026-07-11:

- `cargo test -p tekstide-core runtime::terminal::tests` passed; 16 runtime terminal tests passed.
- `cargo check --workspace` passed.
- `cargo fmt --check --all` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `cargo test --workspace` passed; 186 `tekstide-core` tests passed, `tekstide` had 0 tests, `tekstide-pty-spike` had 0 tests, and doc tests had 0 tests.
- `git diff --check` passed.

Security/privacy note:

- Termination tests use synthetic temporary roots and short marker strings only.
- Tests do not print environment dumps, shell history, project files, token-like values, or private output.
- This slice still does not implement RFC-009 ANSI/VT filtering, paste protection, clipboard policy, approval-dialog containment, transcript retention, command approval, or durable audit storage.

Migration note:

- No local data schema or persisted state migration is introduced.
- Process ids and process group ids remain runtime-only and are not persisted as durable truth.

Known limitations:

- PR-008-D does not integrate launched terminals into `ProjectSession` collections.
- PR-008-D does not update safe-close summaries from live runtime state.
- PR-008-D does not implement final GUI terminal behavior.
- PR-008-D does not implement RFC-009 ANSI/VT, paste, clipboard, or approval-dialog security policy.
- PR-008-D does not introduce transcript persistence or durable audit records.

Review follow-up:

- `.git-exclude/reviewed/tekstide-review-request-043-rfc008-pr008d-process-group-termination-rereview-response.md` accepted PR-008-D with notes.
- Later ProjectSession/safe-close integration must not treat child-only `wait_for_exit` as durable process-group cleanup proof.
- The new `runtime::terminal::termination` source file was included in the PR-008-D commit.

### PR-008-E — ProjectSession Integration and Visibility

Status: ready for implementation review.

Implemented:

- Added project-level terminal lookup with `ProjectSession::terminal_session`.
- Added `ProjectSession::visible_terminal_sessions`.
- Added `ProjectSession::transition_terminal_status` for controlled lifecycle updates from observed runtime state.
- Added `ProjectSession::mark_terminal_exited` to record known exit status and refresh runtime summaries.
- Added `ProjectSession::assign_terminal_visible_slot`.
- Visible slot assignment supports `Hidden`, `Primary`, and `Secondary`; assigning a primary or secondary slot hides any previous terminal in that same slot, preserving at most two visible terminals.
- Added `ProjectTerminalError` to distinguish ownership/reference failures from invalid terminal lifecycle transitions.
- Project runtime summaries now refresh after terminal lifecycle updates, so running/failed process counts and Project Board terminal attention derive from real terminal collection state.
- Added tests for terminal lifecycle summary updates, invalid transition rejection, visible-slot replacement/cap behavior, mode-switch preservation, missing terminal mutation rejection, and Project Board summary derivation from an actual terminal collection.

Observed gates on 2026-07-11:

- `cargo test -p tekstide-core project::tests::collections` passed; 10 project collection tests passed.
- `cargo test -p tekstide-core project_board::tests` passed; 8 project board tests passed.
- `cargo check --workspace` passed.
- `cargo fmt --check --all` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `cargo test --workspace` passed; 192 `tekstide-core` tests passed, `tekstide` had 0 tests, `tekstide-pty-spike` had 0 tests, and doc tests had 0 tests.
- `git diff --check` passed.

Security/privacy note:

- PR-008-E is in-memory project state integration only.
- It does not read terminal output, inspect environment variables, print shell history, persist transcript bytes, or start workspace automation.
- This slice still does not implement RFC-009 ANSI/VT filtering, paste protection, clipboard policy, approval-dialog containment, transcript retention, command approval, or durable audit storage.

Migration note:

- No local data schema or persisted state migration is introduced.
- Runtime handles, process ids, process group ids, PTY descriptors, and terminal output buffers remain outside `TerminalSession` durable metadata.

Known limitations:

- PR-008-E does not introduce app/UI commands for launching or selecting terminals.
- PR-008-E does not integrate safe-close summaries from terminal termination choices; PR-008-F owns that.
- PR-008-E does not implement final GUI terminal behavior.
- PR-008-E does not implement RFC-009 ANSI/VT, paste, clipboard, or approval-dialog security policy.
- PR-008-E does not introduce transcript persistence or durable audit records.

### PR-008-F — Safe-Close Integration for Real Running Terminals

Status: ready for implementation review.

Implemented:

- `assess_close` now preserves known active-resource blockers before handling incomplete provider state.
- A real running terminal can now cause `CloseAssessment::NeedsConfirmation` even when other active-resource provider state remains unavailable.
- Incomplete provider state is still surfaced as an additional close reason when known blockers exist.
- Idle projects with unavailable/not-implemented/unknown provider state still return `UnsupportedOrUnknown`; this slice does not make unknown projects safe to close.
- `AppState::close_project` now refuses to remove a project with a real running `TerminalSession` because the project close assessment returns confirmation-required.
- Added close assessment coverage for known resources with unavailable provider state.
- Added app-level coverage for a real running terminal attached through `ProjectSession::add_terminal_session`.

Observed gates on 2026-07-11:

- `cargo test -p tekstide-core close::tests` passed; 5 close tests passed.
- `cargo test -p tekstide-core app::tests::active_project_with_real_running_terminal_needs_confirmation_and_stays_open` passed.
- `cargo test -p tekstide-core project::tests::metadata` passed; 7 project metadata tests passed.
- `cargo test -p tekstide-core app::tests` passed; 18 app tests passed.
- `cargo check --workspace` passed.
- `cargo fmt --check --all` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `cargo test --workspace` passed; 194 `tekstide-core` tests passed, `tekstide` had 0 tests, `tekstide-pty-spike` had 0 tests, and doc tests had 0 tests.
- `git diff --check` passed.

Security/privacy note:

- PR-008-F is assessment-only close integration.
- It does not terminate processes, read terminal output, inspect environment variables, print shell history, persist transcript bytes, or start workspace automation.
- This slice still does not implement RFC-009 ANSI/VT filtering, paste protection, clipboard policy, approval-dialog containment, transcript retention, command approval, or durable audit storage.

Migration note:

- No local data schema or persisted state migration is introduced.
- Runtime handles, process ids, process group ids, PTY descriptors, and terminal output buffers remain outside `TerminalSession` durable metadata.

Known limitations:

- PR-008-F does not add terminate/keep UI choices for confirmation dialogs.
- PR-008-F does not introduce app/UI commands for launching or selecting terminals.
- PR-008-F does not implement final GUI terminal behavior.
- PR-008-F does not implement RFC-009 ANSI/VT, paste, clipboard, or approval-dialog security policy.
- PR-008-F does not introduce transcript persistence or durable audit records.

Review follow-up:

- `.git-exclude/reviewed/tekstide-review-request-045-rfc008-pr008f-safe-close-running-terminals-response.md` accepted PR-008-F with notes.
- Future app-wide close aggregation should preserve per-project provider uncertainty while surfacing known running terminal blockers.

### PR-008-G — Closeout Evidence and Recommendation

Status: accepted with documented limitations.

Committed slice list:

- `82ca416 Add RFC-008 terminal lifecycle design`
- `b581be1 Add RFC-008 terminal runtime boundary`
- `97d0bdd Add RFC-008 project-owned PTY shell launch`
- `8195578 Implement bounded terminal IO and resize plumbing`
- `a6b4e97 Implement terminal process-group termination`
- `e90b955 Integrate terminal sessions with project visibility`
- `a5308b7 Require close confirmation for running terminals`

Implemented foundation:

- Linux project-owned PTY shell launch through `/bin/sh` or a requested executable shell path.
- Launch validation for project ownership, canonical root, cwd containment, supported terminal kind, and executable shell path.
- Minimal shell environment and no explicit login/startup-file loading.
- Runtime-only PTY/process/session ownership; no PTY descriptors, process ids, process groups, reader threads, or output bytes persisted through `TerminalSession`.
- Bounded PTY output reads with dropped-byte accounting.
- Project-addressed terminal input writes and cross-project rejection.
- PTY resize routing and child-observed row/column evidence.
- Process-group termination with SIGTERM, timeout, SIGKILL fallback, process-group observation, and honest `OrphanedUnknown` fallback when cleanup cannot be proven.
- Regression evidence that direct child exit is not treated as process-group cleanup proof.
- ProjectSession terminal collection integration, visible-slot policy, mode-switch preservation, and Project Board/runtime summaries from real terminal state.
- Project-close assessment requiring confirmation for real running terminals.

Documented limitations:

- No app/UI command for launching, selecting, or closing terminals.
- No app-wide close aggregation surface.
- No terminate/keep confirmation action UI.
- No final GUI terminal widget or terminal renderer.
- No AgentRun launch or AI CLI profile execution.
- No transcript capture, transcript retention, or durable audit storage.
- No command approval.
- No production ANSI/VT parser, paste protection, clipboard behavior, or approval-dialog spoofing boundary; RFC-009 owns these.
- Linux-only runtime evidence; macOS and Windows terminal/process behavior need later evidence.
- `LinuxTerminalRuntime::wait_for_exit` remains child-status oriented and must not be used as durable process-group cleanup proof by later safe-close/app integration.

Observed closeout gates on 2026-07-11:

- `cargo check --workspace` passed.
- `cargo fmt --check --all` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `cargo test -p tekstide-core runtime::terminal::tests` passed; 16 runtime terminal tests passed.
- `cargo test -p tekstide-core project::tests::collections` passed; 10 project collection tests passed.
- `cargo test -p tekstide-core close::tests` passed; 5 close tests passed.
- `cargo test -p tekstide-core app::tests::active_project_with_real_running_terminal_needs_confirmation_and_stays_open` passed.
- `cargo test --workspace` passed; 194 `tekstide-core` tests passed, `tekstide` had 0 tests, `tekstide-pty-spike` had 0 tests, and doc tests had 0 tests.
- `git diff --check` passed.

Security/privacy note:

- RFC-008 evidence uses synthetic temporary roots and short marker strings.
- No environment dumps, shell history, private project files, token-like values, or private output are printed in evidence.
- Plain terminal sessions remain labeled honestly; no managed command approval, transcript retention, or durable audit behavior is claimed.
- Terminal output does not mutate trust state, approvals, clipboard, command history, or app chrome in this implementation.
- RFC-009 remains a required follow-up before production terminal rendering, paste protection, clipboard, or approval-dialog spoofing claims.

Migration note:

- No local data schema or persisted state migration is introduced.
- Runtime handles, PTY descriptors, process ids, process group ids, terminal output bytes, and transcript bytes are not persisted as durable truth.

Closeout disposition:

- RFC-008 closeout was accepted with documented limitations on 2026-07-11.
- Source RFC moved from `rfcs/proposed/` to `rfcs/done/`.
- `rfcs/README.md` and handoff metadata were updated to reflect the done-state.

Future evidence will be recorded under later RFCs or milestone slices:

- RFC-009 terminal security boundary;
- RFC-010 AgentRun launch model;
- transcript/audit storage RFCs;
- app/UI terminal controls and app-wide close aggregation;
- GUI terminal renderer and cross-platform runtime evidence.

## Recommendation

RFC-008 is accepted as implemented with documented limitations.
