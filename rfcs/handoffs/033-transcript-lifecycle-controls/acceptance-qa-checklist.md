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
[x] Bytes are gone from the real filesystem, asserted directly.
    (purging_transcripts_through_a_real_key_sequence_removes_the_real_file,
    crates/tekstide/src/shell/tests.rs — real launch, real transcript file with real marker
    content, real Delete keypress opens the confirmation, ModalFocusNext + ModalActivate
    confirms, then asserts the real file no longer exists on disk. Wires the existing,
    already-tested ProjectSession::purge_project_transcripts — not rebuilt, per
    what-purge-must-remove.md's own instruction.)
[x] The tombstone remains and the surface does not claim otherwise.
    (same test: asserts the purged transcript's own record is_tombstone() after purge.
    transcript-purge-dialog-body, crates/tekstide/locales/en.ftl, says only what is
    removed — "This permanently deletes N transcript(s) (X bytes)" — never "every trace" or
    "all data".)
[x] The project-local refusal (UnsafeProjectPath) is preserved, not weakened.
    (transcript_path_is_project_local/remove_transcript_file untouched by this slice; existing
    coverage — transcript_purge_never_deletes_project_files,
    crates/tekstide-core/src/project/tests/transcripts.rs — still passes unmodified.)
[x] The confirmation names the scope of what is deleted.
    (transcript-purge-dialog-body: "stored locally for this project. Other projects are not
    affected. This cannot be undone." — per-project scope named explicitly, matching the task
    breakdown's own scope decision; cancelling-the-purge-dialog-leaves-the-real-transcript-file-untouched,
    shell/tests.rs, proves the default focus is Cancel and activating it deletes nothing.)
[x] Retained-data visibility wired — transcript_local_data_summary has a real caller.
    (trust_settings_view, crates/tekstide/src/shell.rs, via transcript_local_data_summary_for —
    but NOT ProjectSession::transcript_local_data_summary's own byte_count-based sum: that
    field has no production writer (record_transcript_write_summary/
    record_terminal_transcript_write_summary have zero call sites outside tests, confirmed by
    grep; rfcs/proposed/036-dormant-capability-closure.md already names the latter as its own,
    separate, undecided question). Trusting it would have shown "0 bytes" for every real
    transcript. Added ProjectSession::real_retained_transcript_bytes — real fs::metadata reads
    on each non-tombstone transcript's own storage_path, the same real-filesystem source of
    truth purge's own remove_transcript_file already uses at delete time — and built
    TranscriptLocalDataSummary from that instead. Proven against real data:
    real_retained_transcript_bytes_reads_real_disk_content_not_the_tracked_field,
    crates/tekstide-core/src/project/tests/transcripts.rs, constructs a transcript with
    byte_count left at 0 and a real file with real bytes on disk, and asserts the real sum is
    still correct. Ablated: temporarily reverted to the byte_count-based sum, confirmed that
    test fails (0 vs 22 real bytes), reverted back, confirmed green.
    retained_transcript_visibility_reflects_a_real_transcripts_real_byte_count,
    shell/tests.rs, proves the same property through the full GUI-level call.
```

## Audit (PR-033-D)

```text
[ ] valid_transcript_purge() read before the record was designed; what it permits is stated.
[ ] No path and no byte count in the record.
[ ] The trade is stated in the closeout: the store retains a record of a privacy action.
```

## Claims that must not be made

```text
[x] Not claimed: purge removes every trace. A tombstone and an audit record remain.
    (transcript-purge-dialog-body names only the transcript bytes; PR-033-D still owes the
    audit-record half of this line once transcript_purge is wired.)
[x] Not claimed: opting out removes existing transcripts.
    (PR-033-B's own trust-settings-capture-current-state wording, unchanged this slice.)
[x] Not claimed: the store is viewable. Nothing renders it.
    (no surface added or changed this slice renders the durable audit store; still true.)
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
