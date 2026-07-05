# Release Notes

## 0.1.0 - Foundation Release

Status: release-candidate preparation; not yet tagged.

Tekstide `0.1.0` is scoped as a core/shell foundation release through RFC-006. It is not the full AI CLI workbench.

### Implemented

- Project Board and ProjectSession state.
- Core domain vocabulary for ProjectSession, TerminalSession, AgentRun, approvals, transcripts, change sets, and audit events.
- Navigation/mode policy for Project Board, Content Mode, and Terminal / Agent Immersion Mode.
- Restricted Mode policy/read-model baseline.
- Root-bound project file access policy.
- Bounded explorer read model.
- UTF-8 text document buffer.
- Safe save and external-change detection.
- Dirty-state propagation to project/runtime summaries.
- Shell-visible Content Mode evidence.

### Deferred

- Desktop GUI runtime.
- PTY terminal runtime.
- AI CLI profile execution and AgentRun launch.
- Transcript capture and review workflow.
- Generated diff/artifact review.
- Running-process safe close.
- Paste protection for real terminal input.
- File watcher.
- Overwrite-confirmation UI.
- Durable audit storage.
- Plugin marketplace, remote/container projects, debugger, cloud sync, and collaboration.

### Release Gate Status

Required before tagging:

- clean working tree;
- `git diff --check`;
- `cargo fmt --check`;
- `cargo test --all-targets`;
- `cargo clippy --all-targets --all-features -- -D warnings`;
- `cargo build --release --locked`;
- `cargo package -p tekstide-core --locked`;
- `cargo package -p tekstide --locked`;
- package smoke test from generated package artifacts;
- release-candidate review package accepted.

Do not create the `v0.1.0` tag until the release-candidate review package is accepted.

### Future Work Themes

- Terminal/PTY runtime and process lifecycle.
- AgentRun launch and AI CLI profile execution.
- Transcript retention, review, and generated-change workflow.
- Durable audit storage and security evidence.
- Desktop GUI runtime and final Content Mode widgets.
- Release automation/checklist hardening.

See [`rfcs/future-work.md`](rfcs/future-work.md) for the durable deferred-theme index.
