---
title: "RFC-031 QA evidence"
status: "PR-031-A and PR-031-B implemented 2026-08-19, awaiting review"
rfc_file: "../../proposed/031-audit-producer-completion.md"
target_milestone: "M11"
created: "2026-08-18"
---

# RFC-031 — QA evidence

Fill as slices land. Each section states what was proven, how, and **what it does not
establish** — this pack's own gate requires the third of those as much as the first two.

## PR-031-A — `restricted_mode_blocked`

**Producer.** `AuditCoordinator::record_restricted_mode_blocked` /
`restricted_mode_blocked_record` (`tekstide-core/src/audit/integration.rs`), shaped exactly
like `record_paste_blocked`/`paste_blocked_record`. `action_kind: RestrictedFeature`,
`reason_code: Some(RestrictedMode)`, `outcome: Blocked` (a refusal is a block, not a
failure — no error occurred, the action was correctly prevented), `actor_kind: AppPolicy`,
`action_source: PolicyEngine` (the block is the policy engine's decision, not the user's
or a trusted-UI widget's), no domain links, `subject_kind`/`subject_ref: None`.
`record.validate()` asserted `Ok` against the frozen `valid_restricted_mode_blocked`
family rule
(`restricted_mode_blocked_persists_a_valid_record_conforming_to_the_frozen_family`, core).

**Schema-boundary ablation.** `restricted_mode_blocked_schema_rejects_any_outcome_other_than_blocked`
(core) constructs the record with `outcome: Failed` in place of `Blocked` and asserts
`validate().is_err()` — the frozen family rule, not this producer's own logic, is what
rejects it; this producer could not silently drift to a wrong outcome without the schema
catching it.

**Wiring.** `shell.rs`'s `update()`, `AppCommand::LaunchAgentRun` handler: on
`Err(refusal)` from `attempt_agent_run_launch`, calls
`record_restricted_mode_blocked_if_applicable(state, &refusal)` before setting the notice.
That function matches only `AgentRunLaunchRefusal::Validation(WorkspaceDiscoveryBlocked)` —
reusing the discrimination `agent_run_launch_refusal_symbol` already made (`"workspace-blocked"`
distinct from `"limit"`/`"not-found"`/`"error"`) rather than re-deriving it — and returns
early for every other refusal shape.

**Reachability, from a real key press.**
`a_real_workspace_discovery_refusal_writes_a_real_restricted_mode_blocked_record`
(shell/tests.rs): a real `Ctrl+Alt+A` (`shell_input_for_test(LaunchAgentRun)`) through
`update`, on a freshly created untrusted project directory. Confirms the precondition
(`WorkspaceDiscoveryBlocked`), then queries the real audit store and asserts exactly one
`RestrictedModeBlocked` record for that `project_id`, with `subject_ref: None` and
`reason_code: Some(RestrictedMode)`.

**Discrimination, both directions.**
`a_restricted_mode_blocked_record_appears_only_for_workspace_discovery_refusals`
(shell/tests.rs), three controlled-profile cases via
`attempt_agent_run_launch_with_profile` directly (skipping `update` — reachability through
`update` is already proven by the sibling test above): (1) `MayDiscoverWorkspaceFiles` on
an untrusted project → `WorkspaceDiscoveryBlocked` → a record IS found; (2)
`AiCliExecutable::PathLookup` with an empty lookup directory → `ExecutableUnavailable` →
NO record; (3) `set_resource_limits { agent_run_limit: Some(0) }` →
`RunLimitExceeded { limit: 0 }` → NO record.

**What this does not establish.** Which of RFC-004's nine restricted features blocked the
launch — the schema has one `RestrictedMode` reason code and no field to carry more. Not
claimed anywhere in this pack's public-statement changes. Nothing renders the store; this
proves only that the record exists, not that any UI surfaces it.

## PR-031-B — `project_added`

**Producer.** `AuditCoordinator::record_project_added` / `project_added_record`
(`tekstide-core/src/audit/integration.rs`), same shape as PR-031-A's. `action_kind:
ProjectAdd`, `outcome: Applied`, `actor_kind: User`, `action_source: AppCommand` (a
judgment call, documented on the function itself: the real caller runs in `boot()`,
before any GUI widget exists, so `TrustedUi` does not fit; `AppCommand` is the closer of
the two schema-allowed options for a `User` actor), no domain links, no optional context,
`subject_kind`/`subject_ref: None`. `record.validate()` asserted `Ok` against
`valid_project_added`
(`project_added_persists_a_valid_record_conforming_to_the_frozen_family`, core).

**Schema-boundary ablation.** `project_added_schema_rejects_any_outcome_other_than_applied`
(core): `outcome: Requested` instead of `Applied`, asserts `validate().is_err()`.

**Wiring, and a real testability gap closed to make it wiring at all.**
`boot()`'s CLI-argument loop originally called `app_shell.add_project_from_path` inline —
unreachable from a test, since `iced`'s `BootFn` and `std::env::args_os()` cannot be driven
from one. Extracted `open_cli_project_path_and_record` (`main.rs`), the same
testability-split shape this project has used three times now
(`attempt_agent_run_launch_with_profile_and_state_root`,
`agent_run_transcript_window_with_state_root`): the real logic a CLI argument reaches, now
directly callable with a controlled path. It matches `AddProjectOutcome::Added` and calls
`record_project_added_if_possible`; `FocusedExisting` (an already-open project re-focused,
nothing new) does not.

**Restore-vs-add.** Settled by reading the code, not by guessing: see the "Correction to
this pack's own task breakdown" note in `acceptance-qa-checklist.md`. `restore_recent_projects`
never reaches `add_project_session`; only the CLI-argument path does, in the shipped
application today. Asserted directly, three tests in `crates/tekstide/src/tests.rs`:

- `opening_a_real_new_project_from_the_cli_path_writes_exactly_one_real_project_added_record`
  — a genuinely new project, opened through `open_cli_project_path_and_record`, produces
  exactly one record, with `subject_ref: None`, `outcome: Applied`, `action_kind: ProjectAdd`.
- `reopening_the_same_project_path_focuses_it_instead_of_writing_a_second_record` — the same
  path opened a second time (`FocusedExisting`) does not add a second record; the count
  stays 1.
- `restoring_recent_projects_on_boot_writes_no_project_added_record` — takes the
  `RecentProjectState` produced by the first test, restores it into a **fresh**
  `ApplicationShell` (the shape a real second boot takes, with no CLI arguments), and
  asserts the record count for that `project_id` is still exactly 1 — restoring alone adds
  nothing.

**What this does not establish.** That an interactive "Add Project" GUI flow behaves the
same way — no such flow exists yet in the shipped application; only the CLI-argument path
is reachable, and that is the only path proven here. If an interactive add flow is built
later, it will need its own reachability test through whatever command it dispatches, not
an assumption that this producer's wiring already covers it.

## Closeout

Full workspace gate run, 2026-08-19: `cargo fmt --all --check` clean; `cargo clippy
--workspace --all-targets --all-features -- -D warnings` clean (one pre-existing
`field_reassign_with_default` lint surfaced in a PR-031-A test written earlier in this
same session, fixed in the same pass); `cargo test --workspace --all-targets
--all-features` run twice, both green (`tekstide`: 302 passed; `tekstide-core`: 619
passed; `reference_adapter`: 0 tests, no failures either run); `git diff --check` clean.
`README.md` and `crates/tekstide-core/README.md`'s audit paragraphs narrowed in the same
commit as the producers, per the task breakdown's explicit instruction.
