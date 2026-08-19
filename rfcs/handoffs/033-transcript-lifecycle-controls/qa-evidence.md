---
title: "RFC-033 QA evidence"
status: "PR-033-A implemented 2026-08-19, awaiting review"
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

*(not started)*

## PR-033-C — purge and visibility

*(not started)*

## PR-033-D — audit producer and closeout

*(not started)*
