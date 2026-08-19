---
title: "RFC-033 acceptance and QA checklist"
status: "Final Acceptance recorded 2026-08-19"
rfc_file: "../../done/033-transcript-lifecycle-controls.md"
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
    grep; rfcs/accepted/036-dormant-capability-closure.md already names the latter as its own,
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
[x] valid_transcript_purge() read before the record was designed; what it permits is stated.
    (crates/tekstide-core/src/audit/record.rs — Completed|Failed only, operation_id: None,
    subject_kind forced Some(Transcript) which then forces subject_ref: Some(..) via the
    crate-wide subject_kind.is_some() == subject_ref.is_some() invariant. Documented in
    AuditCoordinator::purge_project_transcripts's own doc comment as the family's own valid_*
    function settling the "Authorized-then-Applied or single-record?" question before any
    design choice was made, per PR-023-D's own precedent the task breakdown named.)
[x] No path and no byte count in the record.
    (subject_ref is the fixed literal "project" — a compile-time constant, not derived from any
    transcript's real identity or path. purge_persists_a_completed_record_naming_only_the_project_scope,
    crates/tekstide-core/src/audit/tests/integration.rs, asserts every other field is None and
    that the record's own Debug text contains neither the real transcript's real path nor its
    content. Ablated: temporarily set subject_ref to a wrong literal, confirmed the test fails.)
[x] The trade is stated in the closeout: the store retains a record of a privacy action.
    (README.md's Local Data and Privacy section: "so does a transcript_purge entry in the local
    audit store — recording that a purge happened and its scope, never a path or a byte count.")
```

## Claims that must not be made

```text
[x] Not claimed: purge removes every trace. A tombstone and an audit record remain.
    (transcript-purge-dialog-body names only the transcript bytes; PR-033-D's own
    transcript_purge record is now the audit-record half of this line — README.md states the
    trade explicitly rather than leaving it for a user to discover.)
[x] Not claimed: opting out removes existing transcripts.
    (PR-033-B's own trust-settings-capture-current-state wording, unchanged this slice.)
[x] Not claimed: the store is viewable. Nothing renders it.
    (no surface added or changed this slice renders the durable audit store; still true.)
```

## Published documentation

```text
[x] README.md's "no in-app way to turn capture off or to purge it" sentence narrowed to what
    is still true, in the same commit as the last slice.
    (README.md's Local Data and Privacy section: both former limitations replaced with what
    Ctrl+Alt+U's Space/Delete controls actually do, including the tombstone and audit-record
    trade; the top-level feature-list bullet updated too. The one limitation that remains --
    plain terminals are not recorded -- is kept, unchanged, since it is still true.)
[x] crates/tekstide-core/README.md's equivalent bullet updated.
    (the transcript-capture bullet and the durable-audit-producer sentence both updated;
    transcript-purge moved from the "defined but not yet wired" list to the list of currently
    wired producers.)
```

## Final Acceptance Decision

- [x] Accepted
- [ ] Accepted with required follow-up
- [ ] Rejected

Reviewer notes: Accepted 2026-08-19 (requests 276-280). Suite re-run by the reviewer:
**1006 passed, 0 failed**.

**RFC-033 closes the limitation `0.11.1` published on a privacy claim.** A user can decline
capture per project, delete what exists, and see what is retained — all from the Trust Settings
surface, all proven from real key presses.

Four things this RFC produced beyond its own scope:

1. **PR-033-A took up a fix that had sat unused for three weeks.** `approval_state_root` was
   added by RFC-022 response 216 precisely so capture and approval would not be coupled; the
   GUI never called it. Proven at the production launch path, where a second model-layer test
   would have passed the whole time.
2. **PR-033-C caught a defect in this pack's own instruction.** The handoff said to wire
   `transcript_local_data_summary`; doing so would have put "0 bytes" in a purge confirmation
   for a real transcript, because the field it sums is set only by dormant recorders. The
   implementer raised it instead of implementing it. Recorded in RFC-036: a tracked counter is
   only ever correct prospectively, so wiring those recorders would not have fixed it either.
3. **PR-033-D's first version refused to delete when the audit store would not open** — a
   silent no-op after a user confirmed "cannot be undone." Corrected: the deletion is what the
   user asked for, the record is best-effort, and the two failure modes of one subsystem are now
   handled consistently.
4. **The third flake disclosure in the approval/socket area** prompted scheduling the known
   cause (`handoffs/test-process-leak.md`), diagnosed 2026-08-16 and never fixed.

Known limitations, stated and not claimed away: purge leaves a tombstone and an audit record, so
it does not remove every trace; declining capture does not delete what exists, and the surface
says so; `revoke_workspace_trust` still refuses silently on the same store-open failure, milder
because trust state is visible on the same surface, recorded rather than fixed here.
