# RFC-007: Runtime Substrate and PTY Feasibility Gate — QA Evidence

Status: PR-007-D evidence started
Date opened: 2026-07-09

## Scope

RFC-007 is a feasibility gate. This evidence file must not be used to claim production TerminalSession, AgentRun, transcript, command approval, or durable audit behavior.

## Spike Location

- Location: `crates/tekstide-pty-spike/`
- Run command: `cargo run -p tekstide-pty-spike`
- Observed run result: passed on 2026-07-09; command prints explicit PR-007-A spike-only scope and does not start PTY/runtime behavior.
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
- p50: 1054 us in the final PR-007-D run.
- p95: 1061 us in the final PR-007-D run.
- Worst observed: 1061 us in the final PR-007-D run.
- Measurement limitations: single local run, scripted shell, stdout renderer only, includes shell echo/prompt overhead.

## Security Observations

- Terminal output containment: PR-007-B renders captured output only inside the spike process stdout. No Tekstide app state, trust state, approval state, clipboard, or command history integration exists in this slice.
- Unsupported control-sequence behavior: observed shell output included bracketed-paste control sequences (`<ESC>[?2004h` / `<ESC>[?2004l`) in sanitized output. PR-007-B does not implement ANSI/VT policy; this is evidence for RFC-009.
- Application chrome/trust/approval/clipboard/history effects: no production app chrome or state is connected to the spike.
- Multiline paste interception before PTY write: Pending later RFC-007 slice; PR-007-B writes scripted input directly.
- Future native dialog separation risk: Pending later GUI/substrate work; PR-007-B has no native dialog surface.
- RFC-009 follow-up notes: terminal renderer must preserve or sanitize unsupported control sequences intentionally; simply stripping ESC would be misleading because it can leave visible suffix text.

## Known Limitations

- PR-007-A only establishes the spike crate, run command, and dependency quarantine.
- PR-007-B starts a shell, writes scripted input, renders sanitized captured PTY output, and detects a marker.
- PR-007-C verifies PTY resize propagation and records foreground-child process-group termination behavior.
- PR-007-D enforces a temporary output-flood cap and records basic scripted input/echo latency measurements.
- No TUI full-screen/raw-mode renderer exists yet; output rendering is a deterministic spike stdout rendering.
- Paste-interception and stronger terminal-containment evidence remain pending.

## Go / No-Go Recommendation

- Recommendation: Pending
- Rationale:
