---
title: "RFC-033 QA evidence"
status: "PR-033-A/B/C accepted 2026-08-19; PR-033-D implemented 2026-08-19, awaiting review"
rfc_file: "../../proposed/033-transcript-lifecycle-controls.md"
target_milestone: "M11"
created: "2026-08-19"
---

# RFC-033 — QA evidence

Fill as slices land. Each section states what was proven, how, and **what it does not
establish**.

## PR-033-A — approval/capture decoupling

**Implemented 2026-08-19.** `crates/tekstide/src/shell.rs`'s GUI launch call site
(`attempt_agent_run_launch_with_profile_and_state_root`) never called
`AgentRunLaunchRequest::with_approval_channel` — RFC-022 PR-022-C response 216 built the escape
hatch specifically so `without_transcript_capture()` and the approval channel's own state root
would not be coupled, but the only production caller never took it up. Latent today because
`claude_code_linux_default` is `Supervised` (no `Managed` profile is reachable in the shipped
product), which is exactly why the task breakdown required landing this before PR-033-B's opt-out
makes it live.

**The fix.** A third testability split on the same function this crate has now used three times
(`_and_state_root` for the state root itself, now `_state_root_and_capture` for whether capture
applies): `attempt_agent_run_launch_with_profile_state_root_and_capture(state, profile,
state_root, capture_enabled)`. When `state_root` is `Some`, `approval_state_root` is now set
explicitly and unconditionally via `.with_approval_channel(state_root)`, regardless of whether
`capture_enabled` also causes `.with_local_bounded_transcript` to run. `capture_enabled` is the
exact seam PR-033-B's real per-project opt-out will drive (a persisted setting instead of a test
literal); both existing 2-arg/3-arg wrappers pass `true`, so every existing caller's behavior is
unchanged.

**Proven from the real, production launch path** (not a direct call into
`tekstide-core::agent::launch`, which RFC-022 response 216 already covers): a real `Managed`
profile pointed at the real `reference_adapter` binary, launched through
`attempt_agent_run_launch_with_profile_state_root_and_capture` with `capture_enabled: false`.
Asserts the launch succeeds (`Ok(())`, not a refusal), the approval channel is actually live
(`state.approval_channels.len() == 1`, not merely "no error"), and no `transcripts/` directory was
created under the state root (capture genuinely did not run).

**Ablated for real**: temporarily commented out the `.with_approval_channel(state_root)` line,
confirmed `a_managed_launch_with_capture_disabled_still_binds_its_approval_channel` fails with
`Runtime(AdapterApproval(StateRootMissing))` — the exact failure mode the handoff names — restored,
confirmed green again.

**What this does not establish.** There is no real per-project opt-out yet — `capture_enabled`
is driven by a test literal, not a persisted setting or a GUI toggle. That is PR-033-B's own
scope. This slice proves only that the GUI launch call site is now correctly decoupled, ahead of
the toggle that will exercise it for real.

**Gates run**, 2026-08-19: `cargo fmt --all --check` clean. `cargo clippy --workspace
--all-targets --all-features -- -D warnings` clean. `cargo test --workspace --all-targets
--all-features` run four times: `tekstide` 304 passed (was 303 — the one new test), `tekstide-core`
689 passed (unchanged, not touched this slice), `reference_adapter` 0 tests. Three of the four
runs were fully clean; one run showed
`shell::tests::command_approval_family_produces_real_durable_audit_records_through_the_pipeline`
fail — a pre-existing, unrelated real-process/socket test (not touched by this slice's diff),
confirmed non-deterministic and unrelated to this change by running it in isolation three times
in a row, all passing. Recorded honestly rather than silently re-run past. `git diff --check`
clean.

## PR-033-B — per-run opt-out

**Implemented 2026-08-19.** A per-project, persisted opt-out from transcript capture, surfaced on
Trust Settings (`Ctrl+Alt+U`) rather than as a dialog gating `Ctrl+Alt+A` — the task breakdown's
own placement choice. `ProjectSession` gained a plain `transcript_capture_declined: bool` field
with `pub` getter/setter (not `pub(crate)` like `grant_trust`/`revoke_trust`: those are
audit-gated through `AuditCoordinator`, and this toggle has no audit producer — confirmed by
grepping for the only reader of the legacy `ProjectSession.audit_events` log,
`AuditEventClass::ConfigChanged`/`TranscriptPurged`, and finding it is constructed nowhere in the
codebase, i.e. dead machinery predating RFC-013's durable store, deliberately not extended here).
Persistence mirrors `WorkspaceTrust` exactly: a new `#[serde(default)]` field on `RecentProject`
(defaults `false` for pre-RFC-033 on-disk records), a chainable
`.with_transcript_capture_declined(bool)` builder (not a new constructor parameter — the existing
`RecentProject::new`/`with_timestamps` have 11 call sites across the test suite that don't care
about this field), and a `recent_transcript_capture_declined_by_canonical_root` lookup wired into
`add_project_session` alongside the existing trust-restore logic.

**The GUI surface.** Trust Settings previously showed exactly one control (Grant xor Revoke,
Enter-only — response 248's own documented design for a surface with "nothing to move a cursor
between"). The capture toggle is a second, always-present, independent control, so it did not get
the `handle_approval_history_key` highlight-index treatment (wrong fit — that pattern is for
interchangeable list rows). Instead it got its own fixed key, Space (confirmed unclaimed as a
local key handler anywhere relevant before use), leaving Enter's existing trust-action behavior
unchanged. `trust_settings_view` always renders a capture-state line and a Decline/Allow button
regardless of trust state. The real production wiring is in
`attempt_agent_run_launch_with_profile_and_state_root` (the 3-arg wrapper): `capture_enabled` is
now computed from the active project's real `transcript_capture_declined()`, not hardcoded `true`
— this is the one production call site PR-033-A's decoupling was built ahead of.

**Proven with three tests**, all from real input paths, none asserting against the request's own
field:
- `pressing_the_capture_toggle_through_a_real_key_sequence_declines_capture`
  (`crates/tekstide/src/shell/tests.rs`) — opens Trust Settings via the real navigation input,
  presses Space through `send_main_area_key` twice, asserts `transcript_capture_declined()` flips
  `false → true → false`.
- `declining_capture_through_a_real_key_press_produces_no_transcript_file`
  (`crates/tekstide/src/shell/tests.rs`) — declines via a real key press, launches a real
  `Supervised` profile against the real `transcript_marker_script_path()` binary, polls
  `Message::TerminalWoke` for up to 5s, then asserts no file exists at the documented
  `state_root/transcripts/<project_id>/<agent_run_id>/transcript.log` path and that the
  `transcripts` directory itself was never created.
- `declining_transcript_capture_persists_and_survives_a_reopen`
  (`crates/tekstide-core/src/app/tests.rs`) — mirrors
  `revoking_trust_persists_and_survives_a_reopen` exactly: declines on a first `AppState`, takes
  `recent_project_state()`, restores it into a fresh second `AppState`, reopens the same project
  root by path, asserts the reopened project's `transcript_capture_declined()` is still `true`.

**Ablated for real**: temporarily forced `capture_enabled = true` unconditionally in
`attempt_agent_run_launch_with_profile_and_state_root`, ignoring the real per-project setting.
`declining_capture_through_a_real_key_press_produces_no_transcript_file` failed, with a real
transcript file written to the real documented path. Reverted, confirmed green again.

**Wording.** `crates/tekstide/locales/en.ftl`'s new strings
(`trust-settings-capture-current-state`, `trust-settings-capture-decline-button`) say "for future
runs" / "Decline Future Capture" throughout — no string in this slice claims or implies deletion
of anything already captured, per `what-purge-must-remove.md`'s requirement that declining future
capture must never read as purging existing transcripts. `pl.ftl` needed no changes; confirmed the
i18n enforcement tests (`every_source_locale_key_resolves_in_every_shipped_locale`) still pass with
the new keys falling back to English, the tested and correct behavior for that locale's
deliberately partial coverage.

**What this does not establish.** No confirmation dialog exists for the toggle — the task
breakdown states none is wanted, since declining is the safe direction. Nothing in this slice
deletes any existing transcript, changes the retained-data visibility surface, or produces an
audit record for the toggle itself — those are PR-033-C's and PR-033-D's scope respectively.

**Gates run**, 2026-08-19: `cargo fmt --all --check` clean (one line in `shell.rs` needed
`cargo fmt --all`, applied and reverified clean). `cargo clippy --workspace --all-targets
--all-features -- -D warnings` clean. `cargo test --workspace --all-targets --all-features` run
three times, all fully clean: `tekstide` 306 passed (was 304 — the two new shell tests),
`tekstide-core` 690 passed (was 689 — the one new persistence test), `reference_adapter` 0 tests.
No flake observed this round. `git diff --check` clean.

## PR-033-C — purge and visibility

**Implemented 2026-08-19.** Per-project purge, from the same Trust Settings surface (`Ctrl+Alt+U`)
— the task breakdown's own recommended scope, not application-wide. The model layer
(`ProjectSession::purge_project_transcripts`/`purge_transcript`/`purge_agent_run_transcripts`,
real `fs::remove_file` via `remove_transcript_file`, the `UnsafeProjectPath` refusal, the
tombstone) already existed and was already fully tested
(`crates/tekstide-core/src/project/tests/transcripts.rs`) — this slice's own job, per
`what-purge-must-remove.md`'s explicit instruction ("do not rebuild it; wire it"), was wiring a
real GUI confirmation dialog and a real key path to it, and it does not touch that model code at
all.

**The confirmation dialog.** A third `ModalContent` variant, `TranscriptPurge`, the same
manual-open/two-button/default-to-safe shape `TrustGrant` already establishes: `Purge`/`Cancel`
focus cycle, defaults to `Cancel`, opened by `Message::OpenTranscriptPurgeDialog`
(`open_transcript_purge_dialog`), real deletion only on `ModalActivate` with focus on `Purge`
(`apply_transcript_purge`, calling `purge_project_transcripts` directly — no audit store call,
since the task breakdown assigns the `transcript_purge` record to PR-033-D, not this slice).
Reached by a third fixed key on Trust Settings, Delete (Enter and Space were already taken by
PR-033-B; `handle_trust_settings_key`'s own doc comment explains why a highlight index still
does not fit three independent controls any better than it fit two). `transcript_count`/
`retained_bytes` are captured at dialog-open time, the same "captured, not re-read" shape
`TrustGrantModal` already uses, so what the confirmation states is exactly what was true when
the user chose to open it.

**The confirmation wording**, `crates/tekstide/locales/en.ftl`:
`transcript-purge-dialog-body` names the scope ("stored locally for this project. Other
projects are not affected.") and the irreversibility ("This cannot be undone."), and states only
what disappears — the transcript bytes — never claiming every trace is removed (a tombstone
remains, by design).

**A real, unexpected finding, and how it was resolved.** Wiring `transcript_local_data_summary`
(the task breakdown's own literal instruction: "exists and has no caller... wire it to the same
surface") as written would have been actively wrong. `ProjectSession::transcript_local_data_summary`
computes `project_retained_bytes` from each `Transcript`'s tracked `byte_count` field — but that
field is only ever updated by `record_transcript_write_summary`/
`record_terminal_transcript_write_summary`, and grepping the whole workspace found **zero
production call sites** for either, only test call sites. `rfcs/proposed/036-dormant-capability-closure.md`
already names `record_terminal_transcript_write_summary` as dormant and assigns its own
wire/delete/keep decision to that RFC, explicitly not RFC-033's. The practical effect: every real,
non-empty transcript's tracked `byte_count` is permanently `0` today, so wiring the surface
literally as instructed would have shown "Retained locally: 1 transcript (0 bytes)" for a real
transcript with real content on disk — a variant of exactly the "tells a user something false
about their own data" failure `what-purge-must-remove.md` warns against for purge itself, except
here it would have reached the purge confirmation dialog's own stated byte count too, not only a
secondary visibility line.

Resolved without touching the dormant recorder or RFC-036's own undecided question: added
`ProjectSession::real_retained_transcript_bytes`, a new method reading real bytes directly via
`fs::metadata` on each non-tombstone transcript's own `storage_path` — the same real-filesystem
source of truth `remove_transcript_file` already uses at purge time. `ProjectSession::transcript_local_data_summary`
itself is left unchanged (its own existing tests construct transcripts with a tracked `byte_count`
and no real backing file, a pure/I/O-free contract this fix does not disturb); the GUI's
`transcript_local_data_summary_for` builds `TranscriptLocalDataSummary` from the new real-bytes
method instead of delegating to that existing one. `AppState::app_wide_retained_transcript_bytes`
(the `app_retained_bytes` input) is built the same way, across all open projects.

**Proven with six new tests**, none asserting against a return value or in-memory field alone:
- `pressing_delete_on_trust_settings_opens_the_purge_confirmation_dialog` (shell/tests.rs) —
  reachability, a real Delete keypress.
- `purging_transcripts_through_a_real_key_sequence_removes_the_real_file` (shell/tests.rs) — the
  required gate: real launch, real transcript file, real key sequence (Delete, `ModalFocusNext`,
  `ModalActivate`), then asserts the real file is gone from disk and the tombstone remains.
- `cancelling_the_purge_dialog_leaves_the_real_transcript_file_untouched` (shell/tests.rs) — the
  default-focus-`Cancel` safety half, mirroring `trust_grant_dialog_defaults_focus_to_cancel_and_activating_it_grants_nothing`.
- `retained_transcript_visibility_reflects_a_real_transcripts_real_byte_count` (shell/tests.rs) —
  the GUI-level proof that the wired summary reports the real file's real byte count.
- `real_retained_transcript_bytes_reads_real_disk_content_not_the_tracked_field`
  (`crates/tekstide-core/src/project/tests/transcripts.rs`) — the model-level proof of the fix
  above: a transcript with `byte_count` left at `0` and a real file with real bytes on disk;
  the real sum still comes back correct.
- `real_retained_transcript_bytes_excludes_purged_transcripts` (same file) — a purged
  transcript's real (already-deleted) file must not be double-counted.

**Ablated for real**: temporarily reverted `real_retained_transcript_bytes` to the
`byte_count`-based sum (the same computation `transcript_local_data_summary` still correctly
uses for its own, different, pure contract). `real_retained_transcript_bytes_reads_real_disk_content_not_the_tracked_field`
failed with a concrete `0` vs `22` real-byte mismatch. Reverted, confirmed green again.

**What this does not establish.** No audit record is produced by purging — PR-033-D's own scope.
The `UnsafeProjectPath` refusal and the tombstone mechanism are unmodified by this slice, not
newly built; their own existing tests (unchanged) are cited, not re-proven here. The real-bytes
finding above does not resolve `record_terminal_transcript_write_summary`'s own dormancy — that
remains RFC-036's decision to make, untouched by this slice.

**Gates run**, 2026-08-19: `cargo fmt --all --check` clean (three sites needed `cargo fmt --all`,
reverified clean). `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean.
`cargo test --workspace --all-targets --all-features` run three times, all fully clean: `tekstide`
310 passed (was 306 — four new shell tests), `tekstide-core` 692 passed (was 690 — two new
model-layer tests), `reference_adapter` 0 tests. No flake this round. `git diff --check` clean.

## PR-033-D — audit producer and closeout

**Implemented 2026-08-19.** Response 278's own closing instruction: read `valid_transcript_purge()`
before designing the record, and narrow the published sentence in the same commit.

**The schema had already settled the shape.** `valid_transcript_purge` permits only `Completed`/
`Failed` outcomes (no `Authorized`/`Applied` pair the way `TrustGrant` uses) and forces
`operation_id: None` — there is no schema-representable "refused because we could not
pre-authorize it" state for this family, unlike granting trust. That is not a limitation worked
around; it is the correct shape for a delete-then-report action, and PR-023-D's own precedent
(`valid_config_change` forcing `subject_kind: None`) predicted exactly this: read the family's
own `valid_*` function before assuming a judgement call is still open. `AuditCoordinator::purge_project_transcripts`
runs the real, already-tested `ProjectSession::purge_project_transcripts` first, then records
`Completed` or `Failed` after the fact — the same "record what happened, do not gate on
recording it" shape `revoke_project_trust` already uses, and for the same reason: the deletion
has already taken effect on the real filesystem by the time the record is built, so a failed
*write of the record* cannot roll back the deletion. Best-effort (`append_observation`).

**Where scope actually goes.** `valid_transcript_purge` forces `subject_kind: Some(Transcript)`,
and the crate-wide `subject_kind.is_some() == subject_ref.is_some()` invariant then forces
`subject_ref: Some(..)` too — unlike `sensitive_config_changed_record`'s own family, where
`subject_kind` is forced `None` and `subject_ref` is therefore structurally unable to hold
anything. This slice only ever purges an entire project's transcripts (PR-033-C's own scope
decision), so `subject_ref` is the fixed literal `"project"` — a compile-time constant, naming
the purge's breadth without naming which transcript, never a path.

**`apply_transcript_purge` (`crates/tekstide/src/shell.rs`) never gates the deletion on the
audit store opening.** The version reviewed in request 279 did — mirroring
`revoke_workspace_trust`'s own precedent of a silent no-op when `open_real_audit_store` fails —
and response 279 required a fix: "this cannot be undone" (the confirmation's own wording)
describes the *deletion*, not the record, so refusing to delete when the record can't be written
does not weaken that promise, it leaves it unfulfilled, silently, after the user deliberately
moved focus off `Cancel` and activated. There is also no accountability property being protected
here the way there might be for a third-party-facing record: these are the user's own local
transcripts and the audit store is local too, so refusing the deletion buys nothing.
`revoke_workspace_trust`'s own refusal is milder for a reason that does not transfer: trust
state is rendered on the same surface, so a silently-failed revoke at least leaves a visible,
contradicting "Trusted" label — deleted bytes have no equivalent tell. Not this slice's place to
fix that one; noted so the asymmetry is not mistaken for a rule to reapply elsewhere.

Fixed: the purge now runs through `AuditCoordinator::purge_project_transcripts` when the store
opens, and through `ProjectSession::purge_project_transcripts` directly when it does not — the
deletion happens either way, and only the *recording* of it depends on the store being
available. Once the store is open, that one record write stays best-effort, for the same reason
as before: the deletion has already happened by then, so a transient write failure for the
record alone does not and cannot roll it back.

**Proven with four new tests**, none asserting a return value alone:
- `purge_persists_a_completed_record_naming_only_the_project_scope`
  (`crates/tekstide-core/src/audit/tests/integration.rs`) — a real transcript with real,
  sensitive content, purged through the coordinator; asserts the persisted record's every field
  (`subject_kind`, `subject_ref == "project"`, `operation_id`/`terminal_id`/`agent_run_id`/
  `approval_id`/`risk_level`/`adapter_profile_ref`/`reason_code` all `None`), and that the
  record's own `Debug` text contains neither the real transcript's real path nor its content.
  Ablated: temporarily set `subject_ref` to a wrong literal (`"everything"`), confirmed the test
  fails on a concrete `Some("everything")` vs `Some("project")` mismatch, reverted, confirmed
  green.
- `purge_failure_still_persists_a_failed_record` (same file) — a project-local transcript path
  (the existing, untouched `UnsafeProjectPath` refusal) still gets a `Failed` record, not
  silence.
- `purge_write_failure_degrades_health_but_the_deletion_already_happened` (same file) — a
  `RecordingWriter` that fails the one audit write; asserts the deletion still happened (the
  real file is gone from disk, `bytes_removed` is correct) while `audit_status` reports
  `Degraded` — the two failure modes' independence, proven directly.
- `purging_transcripts_through_a_real_key_sequence_records_a_real_audit_record`
  (`crates/tekstide/src/shell/tests.rs`) — the full GUI-level proof, mirroring
  `granting_trust_through_the_real_route_records_both_audit_records`'s own standard ("audit
  records queried and asserted, not implied"): a real launch, a real transcript, a real key
  sequence (Delete, `ModalFocusNext`, `ModalActivate`), then the real audit store queried and
  filtered by project id and family, asserting exactly one `TranscriptPurge` record, `Completed`.

**The closeout.** `README.md`'s *Local Data and Privacy* section: the "no in-app way to turn
capture off or to purge it" sentence is replaced with what `Ctrl+Alt+U`'s Space/Delete controls
actually do, including the tombstone and the new audit-record trade, stated plainly rather than
left for a user to discover later (`what-purge-must-remove.md`'s own instruction for this exact
trade). The plain-terminal-not-recorded limitation is kept, unchanged, since it is still true.
The top-level feature-list bullet updated too. `crates/tekstide-core/README.md`: the
transcript-capture bullet and the durable-audit-producer sentence both updated — `transcript_purge`
moved from "defined but not yet wired" to the list of real producers. `CHANGELOG.md`'s `0.11.1`
entry is a dated, released record of what was true on 2026-08-18 and is deliberately left
untouched — narrowing the README does not rewrite history.

**What this does not establish.** No `Authorized`-phase pre-check exists for purge, by the
schema's own design, not an oversight — see above. Application-wide purge, per-run purge, and
purge confirmation copy naming the audit trade explicitly are all out of this slice's scope (the
first two were PR-033-C's own scope decision, not revisited here). The `open_real_audit_store`
returning `None` branch in `apply_transcript_purge` has no dedicated test: that function has no
injectable seam (it calls the real, `$XDG_STATE_HOME`-derived path directly, the same as
`revoke_workspace_trust`/`apply_workspace_trust_grant`), and no existing test anywhere in this
crate forces that branch for any of the three functions that share it — confirmed by search
before deciding not to add one here, rather than assumed. Confidence in that branch rests on it
being a direct, unconditional call to `ProjectSession::purge_project_transcripts`, already
proven correct standalone by the model-layer tests in
`crates/tekstide-core/src/project/tests/transcripts.rs`.

**Gates run**, 2026-08-19: `cargo fmt --all --check` clean. `cargo clippy --workspace
--all-targets --all-features -- -D warnings` clean. `cargo test --workspace --all-targets
--all-features` run three times, all fully clean: `tekstide` 311 passed, `tekstide-core` 695
passed, `reference_adapter` 0 tests (test counts unchanged from the pre-fix commit — this was a
logic correction, not new coverage). No flake this round. `git diff --check` clean.

**Response 279's required fix, applied 2026-08-19.** See the corrected reasoning above,
replacing what request 279 originally described.
