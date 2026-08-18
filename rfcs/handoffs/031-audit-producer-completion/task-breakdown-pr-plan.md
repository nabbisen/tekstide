---
title: "RFC-031 task breakdown and PR plan"
status: "PR-031-A and PR-031-B implemented 2026-08-19, awaiting review -- see acceptance-qa-checklist.md's correction note on this file's own restore-vs-add framing"
rfc_file: "../../proposed/031-audit-producer-completion.md"
target_milestone: "M11"
created: "2026-08-18"
---

# RFC-031 — task breakdown

Two slices. **They are independent** — neither producer depends on the other, and each carries
its own decision. Split rather than bundled so each gets its own review, because the decisions
are the substance and the code is small.

## PR-031-A — `restricted_mode_blocked`

**The trigger already exists.** `attempt_agent_run_launch_with_profile` refuses with
`AgentRunLaunchValidationError::WorkspaceDiscoveryBlocked` when a profile that may discover
workspace files is launched in an untrusted project (`shell.rs`, near the refusal-symbol
mapping at ~1761). A user reaches it by pressing `Ctrl+Alt+A` on any project they have not
granted trust to — which, before `0.10.0`, was every project.

**Build:** an `AuditCoordinator` producer, following `record_paste_blocked`'s shape exactly
(`audit/integration.rs:586`) — a `*_record()` free function building a
`DurableAuditRecordV1`, and a `record_*` method calling `append_observation`.

**Decide and state:**

- `action_kind` — `RestrictedFeature` is the obvious fit; confirm nothing else is closer.
- `reason_code` — `RestrictedMode`. It already exists.
- `subject_kind` / `subject_ref` — **`None`**, per `what-the-store-may-hold.md`.
- `outcome` — which of `AuditOutcome`'s variants a *refusal* is. Read the variants before
  choosing; a blocked action is not obviously `Failed`.

**Only the workspace-discovery refusal, not every refusal.** `RunLimitExceeded` and
`ExecutableUnavailable` are not restricted-mode blocks and must not produce this family.
The existing `agent_run_launch_refusal_symbol` match already separates them; use that
distinction rather than recreating it.

**Gate:**

- Proven **from a real key press** on an untrusted project, through `update`, to a record in a
  real store. Not from a dispatched command.
- **A record appears for `WorkspaceDiscoveryBlocked` and does not for the other refusals** —
  asserted, both directions. This is the ablation-shaped half.
- `subject_ref` asserted `None`.
- The one-code coarseness recorded: this cannot say which of RFC-004's nine features blocked.

## PR-031-B — `project_added`

**The trigger already exists.** `AppState::add_project_from_path` →
`add_project_session` (`app.rs:161`/`115`), reached when a project is opened from the CLI or
restored from recent projects.

**Decide and state:**

- **Where the producer is called.** `add_project_session` is `pub(crate)` and is also the
  restore path — check whether restoring a recent project on startup should produce a record
  or only a genuinely new add. **State which and why**; "every session start writes N records,
  one per remembered project" is a plausible and probably wrong outcome that a careless call
  site would produce.
- `subject_ref` — **`None`**. `project_id` identifies it.
- `outcome` and `action_kind` — `ProjectAdd` is the obvious kind.

**Gate:**

- Proven from the real path a user reaches — opening a project — not a direct call.
- **Restore-vs-add settled and asserted**, whichever way it goes. If restoring produces no
  record, a test must show that restoring produces none; if it does, a test must show the
  count is what you intended.
- `subject_ref` asserted `None`.

## Both slices

- **Audit-store availability is not guaranteed.** Every existing producer goes through
  `AuditObservationStatus` and none of them treat a failed observation as fatal. Follow that;
  a record that cannot be written must not break the action it was observing.
- **Update the public statement.** Both `README.md` and `crates/tekstide-core/README.md`
  currently list these families as "defined in the audit schema but not yet wired." Narrow it
  to what is still true, in the same commit as the producer.

## Not in scope

- `safe_close_decision` — no surface exists.
- `SensitiveConfigChanged` (RFC-023), `TranscriptPurge` (RFC-033).
- Any view of the store. Nothing renders it, and this slice does not change that.
