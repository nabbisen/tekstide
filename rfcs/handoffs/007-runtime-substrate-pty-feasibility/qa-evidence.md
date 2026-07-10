# RFC-007: Runtime Substrate and PTY Feasibility Gate — QA Evidence

Status: PR-007-E closeout evidence ready for review
Date opened: 2026-07-09

## Scope

RFC-007 is a feasibility gate. This evidence file must not be used to claim production TerminalSession, AgentRun, transcript, command approval, or durable audit behavior.

## Spike Location

- Location: `crates/tekstide-pty-spike/`
- Run command: `cargo run -p tekstide-pty-spike`
- PR-007-A run result: passed on 2026-07-09; command printed explicit PR-007-A spike-only scope and did not start PTY/runtime behavior.
- Workspace check result: `cargo check --workspace` passed on 2026-07-09.
- Workspace test result: `cargo test --workspace` passed on 2026-07-09; 166 core tests passed and spike crate test target had 0 tests.
- Formatting result: `cargo fmt --all` passed on 2026-07-09.
- Cleanup/quarantine decision: quarantined spike-only crate; not a production runtime crate and not a stable API boundary. Delete, keep quarantined, or promote only after RFC-007 evidence review.
- PR-007-B run result: `cargo run -p tekstide-pty-spike` passed on 2026-07-09 with a real PTY-backed `/bin/sh`, scripted input, rendered output, and marker detection.
- PR-007-B workspace check result: `cargo check --workspace` passed on 2026-07-09.
- PR-007-B workspace test result: `cargo test --workspace` passed on 2026-07-09; 166 core tests passed and spike crate test target had 0 tests.
- PR-007-B diff hygiene result: `git diff --check` passed on 2026-07-09.
- PR-007-B clippy follow-up result: after fixing the `openpty` winsize reference, `cargo fmt --check --all`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo run -p tekstide-pty-spike`, `cargo check --workspace`, `cargo test --workspace`, and `git diff --check` passed on 2026-07-09.
- PR-007-C run result: `cargo run -p tekstide-pty-spike` passed on 2026-07-09 with scripted input, resize, and foreground-child termination observations.
- PR-007-C formatting result: `cargo fmt --check --all` passed on 2026-07-09.
- PR-007-C workspace check result: `cargo check --workspace` passed on 2026-07-09.
- PR-007-C workspace test result: `cargo test --workspace` passed on 2026-07-09; 166 core tests passed and spike crate test target had 0 tests.
- PR-007-C clippy result: `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed on 2026-07-09.
- PR-007-C diff hygiene result: `git diff --check` passed on 2026-07-09.
- PR-007-D run result: `cargo run -p tekstide-pty-spike` passed on 2026-07-09 with output-flood bounds and latency observations.
- PR-007-D formatting result: `cargo fmt --check --all` passed on 2026-07-09.
- PR-007-D clippy result: `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed on 2026-07-09.
- PR-007-D workspace check result: `cargo check --workspace` passed on 2026-07-09.
- PR-007-D workspace test result: `cargo test --workspace` passed on 2026-07-09; 166 core tests passed and spike crate test target had 0 tests.
- PR-007-D diff hygiene result: `git diff --check` passed on 2026-07-09.
- PR-007-E run result: `cargo run -p tekstide-pty-spike` passed on 2026-07-10 with security-boundary observations, multiline paste pre-write detection, and closeout evidence.
- PR-007-E formatting result: `cargo fmt --check --all` passed on 2026-07-10.
- PR-007-E clippy result: `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed on 2026-07-10.
- PR-007-E workspace check result: `cargo check --workspace` passed on 2026-07-10.
- PR-007-E workspace test result: `cargo test --workspace` passed on 2026-07-10; 166 core tests passed and spike crate test target had 0 tests.
- PR-007-E diff hygiene result: `git diff --check` passed on 2026-07-10.

## Dependency Notes

| Dependency | Version | Purpose | Spike-only or candidate production input | Notes |
| --- | --- | --- | --- | --- |
| `libc` | `0.2.186` | Linux PTY creation (`openpty`), process-session setup (`setsid`, `TIOCSCTTY`), fd duplication/close, nonblocking PTY master reads | Candidate production input, but currently spike-only | Already present in the lockfile before PR-007-B; now declared as a workspace dependency. No TUI crate added yet. |

## Safe Shell Startup

- Shell executable: `/bin/sh`.
- Shell arguments: none.
- Working directory category: synthetic `target/tekstide-pty-spike-root` directory.
- Environment policy: minimal documented environment: `TERM=xterm-256color`, `LANG=C.UTF-8`, `LC_ALL=C.UTF-8`, `PATH=/usr/bin:/bin`, `PS1=tekstide-spike$ `.
- Login shell: no.
- Startup files allowed: no explicit startup files requested.
- Evidence sanitization notes: shell output is rendered through a sanitizer that makes ESC and CR bytes visible as `<ESC>` and `<CR>` markers. No environment dump, home listing, shell history, token-like value, or project file content is captured.

## PTY Behavior Evidence

| Behavior | Result | Evidence |
| --- | --- | --- |
| Shell starts | Passed | `cargo run -p tekstide-pty-spike` spawned `/bin/sh` through a real Linux PTY and exited with status 0. |
| Output renders | Passed | Harness captured 157 bytes and printed sanitized rendered PTY output. |
| Keyboard input reaches PTY | Passed | Harness wrote scripted input `printf 'tekstide-pty-ok\n'\nexit\n` to the PTY master; echoed input and command result were captured. |
| Simple command output visible | Passed | Sanitized output contains `tekstide-pty-ok`. |
| Resize sent to PTY | Passed | Harness resized the PTY from 24x80 to 40x100 using `TIOCSWINSZ`. |
| Child observes resized rows/columns | Passed | Child shell command `stty size` reported `40 100`. |

## Termination Evidence

- Shell termination result: foreground-child termination smoke passed after SIGKILL fallback; shell wait status was `signal: 9 (SIGKILL)`.
- Foreground child command: `sleep 60`.
- Signal sequence: sent SIGTERM to the shell process group; when the shell did not exit before the 2-second timeout, sent SIGKILL to the same process group.
- Timeout behavior: SIGTERM timed out; SIGKILL fallback was required.
- Process-group/session observations: shell process was started with `setsid`; observed process group id matched the shell pid/session leader before signaling.
- Orphan detection result: after shell wait, `kill(-pgid, 0)` returned ESRCH, so no process group remained observable.
- RFC-008 blocker notes: production design must not assume SIGTERM is sufficient for foreground jobs; it needs explicit timeout and fallback policy plus user-visible consequences.

## Output Flood Evidence

- Output-flood command: 10,000-line POSIX shell `printf` loop.
- Output amount: 10,000 lines; 1,030,284 bytes observed in the PR-007-D run.
- Temporary buffer cap: 262,144 bytes.
- Dropped/truncated behavior: 262,144 bytes stored; 768,140 bytes dropped after cap.
- Truncation marker: rendered sample includes `<TRUNCATED ...>` in retained output model; console sample is additionally shortened with `<SAMPLE SHORTENED FOR CONSOLE OUTPUT>`.
- Memory before: `VmRSS: 2688 kB` in the final PR-007-D run.
- Memory after: `VmRSS: 3012 kB` in the final PR-007-D run.
- Recovery behavior: flood shell exited successfully and the later latency smoke ran in the same spike process.

## Latency Evidence

- Reference machine summary: local Linux development environment used by this session; no private host details recorded.
- Terminal dimensions: 24x80.
- Measurement procedure: write `printf` marker to PTY, wait until the marker is observed in PTY output, repeat 20 times.
- p50: 1055 us in the final PR-007-E run.
- p95: 1065 us in the final PR-007-E run.
- Worst observed: 1065 us in the final PR-007-E run.
- Measurement limitations: single local run, scripted shell, stdout renderer only, includes shell echo/prompt overhead.

## Security Observations

- Terminal output containment: PR-007-E captures PTY bytes into process-local buffers and prints sanitized text only. No Tekstide app chrome, trust state, approval state, clipboard, or command history integration exists in this spike.
- Unsupported control-sequence behavior: PR-007-E emitted an OSC 52 clipboard-style sequence, BEL/control byte, and CSI clear-screen sequence from the child shell. The spike rendered them inertly as visible markers such as `<ESC>]52`, `<CTRL>`, and `<ESC>[2J`; raw ESC reached the harness console output: false.
- Application chrome/trust/approval/clipboard/history effects: no production app chrome or state is connected to the spike. The observation only proves the spike sanitizer did not forward raw escape bytes to its own console output; it does not prove production renderer safety.
- Multiline paste interception before PTY write: PR-007-E classified a multiline paste candidate before writing to the PTY, did not write that candidate to the PTY, and then wrote a harmless recovery command. The recovery marker `tekstide-paste-intercept-ok` was observed, while the blocked paste markers were not observed.
- Future native dialog separation risk: plausible but not proven by the TUI spike. RFC-009 must require native approval and paste dialogs outside terminal-rendered bytes, with a visual boundary that terminal output cannot spoof.
- RFC-009 follow-up notes: define supported ANSI/VT subset, paste approval UX, output containment boundaries, clipboard policy, and approval-dialog spoofing boundary before production terminal claims. Terminal renderer policy must preserve or sanitize unsupported control sequences intentionally; simply stripping ESC would be misleading because it can leave visible suffix text.

## Linux-Only and Portability Notes

- Linux-only assumption: PR-007 uses Linux PTY APIs through `openpty`, `setsid`, `TIOCSCTTY`, `TIOCSWINSZ`, `kill(-pgid, ...)`, and `/proc/self/status`.
- macOS risk: PTY startup and resize concepts are similar, but process-group behavior, shell defaults, `/proc` memory observation, and signal/orphan checks need separate evidence.
- Windows risk: ConPTY and Windows process-tree termination semantics are different enough that RFC-008 must not assume the Linux process-group model transfers directly.
- GUI risk: the TUI/stdout sanitizer proves PTY/process feasibility, not a desktop terminal widget, native dialog boundary, clipboard policy, or renderer performance under real GUI event-loop pressure.

## Known Limitations

- PR-007-A only establishes the spike crate, run command, and dependency quarantine.
- PR-007-B starts a shell, writes scripted input, renders sanitized captured PTY output, and detects a marker.
- PR-007-C verifies PTY resize propagation and records foreground-child process-group termination behavior.
- PR-007-D enforces a temporary output-flood cap and records basic scripted input/echo latency measurements.
- PR-007-E records spike-local terminal containment, unsupported control-sequence sanitization, multiline paste pre-write detection, and closeout recommendation.
- No TUI full-screen/raw-mode renderer exists yet; output rendering is a deterministic spike stdout rendering.
- No complete ANSI/VT safety claim is made.
- Native approval and paste dialogs remain future GUI work.
- Production lifecycle cleanup must use process-group/session semantics; the spike read helpers are not production TerminalSession APIs.

## Go / No-Go Recommendation

- Recommendation: Go to RFC-008 after PR-007-E review accepts this closeout.
- Rationale:
  - Real Linux PTY shell startup, output rendering, scripted input, resize propagation, foreground-child termination observation, output-flood bounds, recovery, latency measurement, and security-boundary observations were collected.
  - Output flood was explicitly capped before execution and the harness remained usable afterward.
  - Final PR-007-E latency observation was p95 1065 us, below the RFC-007 target threshold, with the limitation that this is input/echo latency in a local scripted shell and not production GUI renderer latency.
  - Terminal output did not obviously escape the spike surface: unsupported escape/control bytes were rendered as inert markers by the spike sanitizer and raw ESC did not reach the harness console output.
  - Multiline paste interception appears feasible before PTY write in the selected direction.
  - The TUI-first direction remains compatible as a feasibility harness, but not as a product UI decision.
  - RFC-008 must design production TerminalSession lifecycle, process-group timeout/fallback policy, and cleanup semantics before implementation hardens.
  - RFC-009 must be designed alongside RFC-008 for ANSI/VT subset policy, paste protection, clipboard behavior, and native approval-dialog spoofing boundaries.
