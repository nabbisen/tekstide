---
title: "RFC-033 acceptance and QA checklist"
status: "Open"
rfc_file: "../../proposed/033-transcript-lifecycle-controls.md"
target_milestone: "M11"
created: "2026-08-19"
---

# RFC-033 — acceptance and QA checklist

## Prerequisite (PR-033-A)

```text
[x] approval_state_root set explicitly at the GUI launch call site.
    (attempt_agent_run_launch_with_profile_state_root_and_capture, crates/tekstide/src/shell.rs)
[x] A test opts out of capture AND binds an approval channel successfully.
    (a_managed_launch_with_capture_disabled_still_binds_its_approval_channel, shell/tests.rs)
[x] Ablated: removing the explicit approval_state_root fails that test specifically.
    (fails with Runtime(AdapterApproval(StateRootMissing)), the exact failure mode the handoff
    describes)
```

## Opt-out (PR-033-B)

```text
[x] Reachable from a real key press, not a dispatched command.
    (pressing_the_capture_toggle_through_a_real_key_sequence_declines_capture,
    crates/tekstide/src/shell/tests.rs — opens Trust Settings via the real navigation input
    path, then dispatches a real Space keypress through send_main_area_key)
[x] A run launched with the opt-out set produces NO transcript file, asserted against the
    documented path shape — not against the request's own field.
    (declining_capture_through_a_real_key_press_produces_no_transcript_file,
    crates/tekstide/src/shell/tests.rs — declines via a real key press, launches a real
    Supervised profile, polls TerminalWoke for up to 5s, then asserts the documented
    state_root/transcripts/<project_id>/<agent_run_id>/transcript.log path does not exist,
    and that the transcripts directory itself was never created. Ablated: forcing
    capture_enabled = true unconditionally in
    attempt_agent_run_launch_with_profile_and_state_root made this test fail with a real
    transcript file written to that real path; reverted after confirming.)
[x] The setting persists per project across a restart (or does not, with a stated reason).
    (declining_transcript_capture_persists_and_survives_a_reopen,
    crates/tekstide-core/src/app/tests.rs — mirrors
    revoking_trust_persists_and_survives_a_reopen exactly: declines on a first AppState,
    takes recent_project_state(), restores it into a fresh second AppState, reopens the
    same project root, asserts transcript_capture_declined() is still true)
[x] The surface distinguishes "do not record future runs" from "delete what exists".
    (crates/tekstide/locales/en.ftl, trust-settings-capture-current-state /
    trust-settings-capture-decline-button: "for future runs" / "Decline Future Capture" —
    no wording anywhere in this slice claims or implies deletion of existing transcripts,
    per what-purge-must-remove.md's requirement; deletion is PR-033-C's responsibility, not
    introduced here)
```

## Purge (PR-033-C)

```text
[ ] Bytes are gone from the real filesystem, asserted directly.
[ ] The tombstone remains and the surface does not claim otherwise.
[ ] The project-local refusal (UnsafeProjectPath) is preserved, not weakened.
[ ] The confirmation names the scope of what is deleted.
[ ] Retained-data visibility wired — transcript_local_data_summary has a real caller.
```

## Audit (PR-033-D)

```text
[ ] valid_transcript_purge() read before the record was designed; what it permits is stated.
[ ] No path and no byte count in the record.
[ ] The trade is stated in the closeout: the store retains a record of a privacy action.
```

## Claims that must not be made

```text
[ ] Not claimed: purge removes every trace. A tombstone and an audit record remain.
[ ] Not claimed: opting out removes existing transcripts.
[ ] Not claimed: the store is viewable. Nothing renders it.
```

## Published documentation

```text
[ ] README.md's "no in-app way to turn capture off or to purge it" sentence narrowed to what
    is still true, in the same commit as the last slice.
[ ] crates/tekstide-core/README.md's equivalent bullet updated.
```

## Final Acceptance Decision

- [ ] Accepted
- [ ] Accepted with required follow-up
- [ ] Rejected

Reviewer notes:
