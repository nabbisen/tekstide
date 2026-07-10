# tekstide-pty-spike

This crate is a quarantined RFC-007 feasibility harness.

It is not a production Tekstide runtime crate and must not expose stable APIs for `TerminalSession`, `AgentRun`, transcript persistence, command approval, or durable audit storage.

Current scope:

- PR-007-A: establish spike location, run command, dependency quarantine, and evidence placeholders.
- PR-007-B: start a real Linux PTY-backed shell, send scripted input, render sanitized output, and record evidence.
- PR-007-C: resize the PTY, verify child-observed dimensions, and record foreground-child termination/orphan behavior.
- PR-007-D: enforce output-flood capture bounds and record basic scripted input/echo latency measurements.
- PR-007-E: record security-boundary observations, multiline paste pre-write interception feasibility, and final RFC-007 closeout recommendation.

Still out of scope:

- production `TerminalSession` behavior;
- AgentRun launch;
- transcript persistence;
- command approval;
- durable audit storage;
- full terminal security policy;
- user-facing terminal feature claims.

Run command:

```sh
cargo run -p tekstide-pty-spike
```
