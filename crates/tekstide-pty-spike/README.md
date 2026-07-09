# tekstide-pty-spike

This crate is a quarantined RFC-007 feasibility harness.

It is not a production Tekstide runtime crate and must not expose stable APIs for `TerminalSession`, `AgentRun`, transcript persistence, command approval, or durable audit storage.

Current scope:

- PR-007-A: establish spike location, run command, dependency quarantine, and evidence placeholders.

Run command:

```sh
cargo run -p tekstide-pty-spike
```

