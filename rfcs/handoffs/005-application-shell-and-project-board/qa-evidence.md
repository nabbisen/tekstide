# RFC-005 QA Evidence

Date: 2026-07-05

Scope: RFC-005 implementation through PR-005-G cleanup.

## Gate Commands

Observed passing:

```text
cargo fmt --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
```

Observed test result:

```text
51 passed; 0 failed
```

## Implemented Slices

- PR-005-A: application shell skeleton.
- PR-005-B: ProjectSession collection and IDs, enough for RFC-005.
- PR-005-C: root validation and add-project flow.
- PR-005-D: recent-project persistence.
- PR-005-E: Project Board view model.
- PR-005-F: remove project and `CloseAssessment`.
- PR-005-G: QA evidence and cleanup.

## Manual Evidence

Isolated restore run:

```text
XDG_STATE_HOME=.git-exclude/tmp/manual-state cargo run -p tekstide
```

Rendered row:

```text
manual-project | .git-exclude/tmp/manual-project -> /home/nabbisen/Desktop/tekstide/tekstide-git/.git-exclude/tmp/manual-project | trust: Restricted | branch/status: not available | terminals: not implemented | agents: not implemented | approvals: not implemented | reviews: not implemented | dirty: not implemented | attention: Calm
```

Invalid path evidence from PR-005-C:

```text
cargo run -p tekstide -- LICENSE
path is not a folder: LICENSE
```

## Security / Scope Evidence

Observed no matches:

```text
rg -n "std::process::Command|Command::new|git |\.env|hooks|LSP|plugin|profile|network|reqwest|ureq" crates
```

RFC-005 implementation does not add:

- Git/status probing;
- shell command execution;
- workspace-local hooks or scripts;
- `.env` loading;
- LSP startup;
- workspace plugin loading;
- workspace AI profile/prompt loading;
- network clients.

## Close / Remove Evidence

Covered by tests:

- provider missing returns `UnsupportedOrUnknown`;
- provider `NotImplemented` returns `UnsupportedOrUnknown`;
- complete empty close summary returns `SafeToClose`;
- known active resources return `NeedsConfirmation` with structured reason codes;
- active project with missing provider is not closed;
- active idle project closes only when provider state is `Complete`;
- active close preserves recent metadata;
- stale recent removal removes only recent metadata;
- active project cannot be removed through recent-metadata removal;
- close/removal does not delete workspace contents.

User-facing label distinction for future GUI:

- `Close Project`: closes the active ProjectSession only after `CloseAssessment` allows it.
- `Remove from Recent`: removes local recent-project metadata only.

No RFC-005 operation deletes project files or directories.

## Known RFC-005 Deviations / Follow-up

The Project Board currently sorts by attention, row kind, display name, and ProjectId. RFC-005 asks for attention, recent activity, recent open, and display name. Timestamp-aware sorting is deferred until the UI layer consumes persisted timestamp fields; this should be revisited before any release that claims final Project Board UX polish.

The current executable is a CLI harness. The desktop GUI layer remains deferred, but the core shell, state, persistence, and view-model boundaries are implemented for GUI integration.
