# RFC-007: Runtime Substrate and PTY Feasibility Gate — QA Evidence

Status: PR-007-A evidence started  
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

## Dependency Notes

| Dependency | Version | Purpose | Spike-only or candidate production input | Notes |
| --- | --- | --- | --- | --- |
| None added in PR-007-A | N/A | Establish location and run command only | N/A | PTY/TUI/process dependencies remain pending PR-007-B or later evidence needs. |

## Safe Shell Startup

- Shell executable: Pending PR-007-B.
- Shell arguments: Pending PR-007-B.
- Working directory category: Pending PR-007-B; must use synthetic or explicitly non-sensitive root.
- Environment policy: Pending PR-007-B; default must be minimal/documented.
- Login shell: Pending PR-007-B; avoid where practical.
- Startup files allowed: Pending PR-007-B; avoid where practical.
- Evidence sanitization notes: PR-007-A does not start a shell or capture shell output.

## PTY Behavior Evidence

| Behavior | Result | Evidence |
| --- | --- | --- |
| Shell starts | Pending | Pending |
| Output renders | Pending | Pending |
| Keyboard input reaches PTY | Pending | Pending |
| Simple command output visible | Pending | Pending |
| Resize sent to PTY | Pending | Pending |
| Child observes resized rows/columns | Pending | Pending |

## Termination Evidence

- Shell termination result:
- Foreground child command:
- Signal sequence:
- Timeout behavior:
- Process-group/session observations:
- Orphan detection result:
- RFC-008 blocker notes:

## Output Flood Evidence

- Output-flood command:
- Output amount:
- Temporary buffer cap:
- Dropped/truncated behavior:
- Truncation marker:
- Memory before:
- Memory after:
- Recovery behavior:

## Latency Evidence

- Reference machine summary:
- Terminal dimensions:
- Measurement procedure:
- p50:
- p95:
- Worst observed:
- Measurement limitations:

## Security Observations

- Terminal output containment:
- Unsupported control-sequence behavior:
- Application chrome/trust/approval/clipboard/history effects:
- Multiline paste interception before PTY write:
- Future native dialog separation risk:
- RFC-009 follow-up notes:

## Known Limitations

- PR-007-A only establishes the spike crate, run command, and dependency quarantine.
- No PTY is started yet.
- No terminal output is rendered yet.
- No input, resize, termination, output-flood, latency, paste-interception, or terminal-containment evidence exists yet.

## Go / No-Go Recommendation

- Recommendation: Pending
- Rationale:
