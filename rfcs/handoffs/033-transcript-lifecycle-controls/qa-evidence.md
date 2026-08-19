---
title: "RFC-033 QA evidence"
status: "PR-033-A accepted 2026-08-19; PR-033-B implemented 2026-08-19, awaiting review"
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

*(not started)*

## PR-033-C — purge and visibility

*(not started)*

## PR-033-D — audit producer and closeout

*(not started)*
