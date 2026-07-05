# RFC-005 Task Breakdown / PR Plan

Source RFC: [RFC-005](../../done/005-application-shell-and-project-board.md)

Each PR should state its scope, non-goals, test evidence, and rollback risk.

## PR-005-A: Application Shell Skeleton

Scope:

- create top-level window/application shell;
- add route enum or equivalent navigation state;
- add global command routing seam;
- add first-run empty Project Board;
- no persistence yet.

Non-goals:

- no project persistence;
- no add-project validation beyond disabled/placeheld UI;
- no terminal, editor, AgentRun, or plugin implementation.

Test evidence:

- build/check command output;
- screenshot or text evidence of empty Project Board.

Rollback risk:

- low; rollback removes shell routing and empty board only.

## PR-005-B: ProjectSession Collection and IDs

Scope:

- add `ProjectId`;
- add in-memory ProjectSession collection;
- add active project selection;
- add switch-project behavior;
- keep mutation APIs project-explicit.

Non-goals:

- no disk persistence;
- no root validation;
- no trust-decision restoration.

Test evidence:

- tests for ProjectSession collection and active selection;
- cross-project mutation guard tests or equivalent design evidence.

Rollback risk:

- medium; later PRs depend on the ProjectSession API shape.

## PR-005-C: Root Validation and Add-Project Flow

Scope:

- dialog path and CLI path share one validator;
- directory-only validation;
- canonical duplicate detection;
- permission error handling;
- symlink ambiguity handling;
- Restricted-by-default trust assignment.

Non-goals:

- no final trust-decision store;
- no recent-project persistence;
- no workspace automation, LSP startup, script execution, plugin loading, or network calls.

Test evidence:

- tests for valid directory, non-directory, permission error, duplicate canonical root, and symlink ambiguity;
- security note confirming no workspace automation runs on add.

Rollback risk:

- medium; rollback disables adding projects but should preserve the shell and in-memory collection.

## PR-005-D: Recent-Project Persistence

Scope:

- versioned JSON local state file;
- Linux path provider using `$XDG_STATE_HOME/tekstide/recent-projects.json` or `~/.local/state/tekstide/recent-projects.json`;
- stable persisted `ProjectId`;
- restore after restart;
- corrupt/missing state recovery;
- stale/missing path display.

Non-goals:

- no final RFC-016 storage backend;
- no ProjectSession runtime persistence;
- no terminal, AgentRun, transcript, dirty-buffer, or plugin persistence.

Test evidence:

- tests for write/read, restart restore, corrupt state, missing state, and stale path;
- privacy note confirming local-only path storage.

Rollback risk:

- medium; rollback may drop recent-project restore while leaving in-memory project behavior intact.

## PR-005-E: Project Board View Model

Scope:

- compact row Project Board view model;
- trust badge;
- placeholder count display model;
- pure attention calculation;
- sort order.

Non-goals:

- no live terminal, AgentRun, approval, or review providers;
- no Git process invocation or branch/status probing;
- no card layout unless it reuses the same view model with negligible extra cost.

Test evidence:

- view-model tests for row generation, attention priority, sort order, and placeholder count display;
- UI/manual evidence that unavailable features are not shown as meaningful zeroes.

Rollback risk:

- low to medium; rollback affects board presentation and attention display.

## PR-005-F: Remove Project and CloseAssessment

Scope:

- implement `CloseAssessment`;
- support `SafeToClose`;
- support `NeedsConfirmation`;
- support `UnsupportedOrUnknown`;
- prevent silent discard of active or unknown resources.

Non-goals:

- no full RFC-017 safe-close implementation;
- no process termination or dirty-buffer save workflow.

Test evidence:

- tests for idle removal, known active resources, and unknown active-resource state;
- manual evidence for warning/blocking path if UI exists.

Rollback risk:

- low; rollback can disable removal while preserving add/switch behavior.

## PR-005-G: QA Evidence and Cleanup

Scope:

- complete RFC-005 QA checklist;
- update docs or known limitations;
- verify no deferred features slipped in.

Non-goals:

- no new product behavior unless needed to close checklist gaps;
- no unrelated refactors.

Test evidence:

- `cargo fmt --check`;
- `cargo clippy --all-targets --all-features -- -D warnings`;
- `cargo test --all-targets`;
- manual or automated evidence for first-run board, five projects added, duplicate root handling, Restricted badge, restart restore, corrupt state recovery, no workspace automation on add, and no Git/status provider active in RFC-005.

Rollback risk:

- low; rollback should only remove checklist/docs cleanup, not implemented behavior.
