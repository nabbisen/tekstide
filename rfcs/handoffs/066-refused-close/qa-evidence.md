# RFC-066 QA evidence

## PR-066-A — reproduce, then repair the ordering

### Reproduced first, against real things (D5)

**Nobody had watched this happen before this response.** Read in the code at review 489, not
observed. `state_with_a_real_terminal_and_a_dirty_document` (`shell::tests`) builds the exact
combination the reproduction needs and no existing fixture had: a real project, a real `/bin/sh`
spawned and attached as a terminal session, and a real unsaved edit (`replace_active_text`) on a
real opened document -- then the real production path, `Message::CloseProjectTabPressed` ->
`Message::ModalFocusNext` (focuses `Close`) -> `Message::ModalActivate` (confirms), the identical
sequence `confirming_the_close_terminates_the_real_process_and_removes_the_project` already proves
reaches `apply_project_close_confirmation` for real.

**It reproduced exactly as read.** Run against the unmodified code, `a_confirmed_close_blocked_by_
a_dirty_file_leaves_the_terminal_alive` (named for what it proves *now*; see below for what it
proved before the fix) passed with its assertions **inverted** from their current form: the project
stayed open (the dirty file blocks `assess_close`) and the real terminal's pane was already gone --
a real `request_terminate` against a real shell had already run, before the refusal was known. This
is not a hypothetical; it is the defect, on a real process, confirmed before any repair was written.

### The assessment completes before anything live is terminated (D1, §row 1)

`apply_project_close_confirmation` (`shell.rs`) now calls the read-only `assess_project_close`
*first*, against every terminal still alive, and only proceeds to `terminate_project_live_work` when
`close_assessment_blocked_by_more_than_running_processes` says nothing *other than* a live process
is blocking. A project blocked by a dirty file, a pending approval, a review-ready change, or a
provider-state problem it cannot enumerate (`UnsupportedOrUnknown`, treated conservatively as
blocked, matching `attempt_close_project_tab`'s own existing reading of that case) now refuses
without touching anything live at all.

### A confirmed close refused for an unsaved file leaves every running terminal alive

`a_confirmed_close_blocked_by_a_dirty_file_leaves_the_terminal_alive` (`shell::tests`), the
reproduction kept as the regression test (D5's own instruction) with its assertions corrected to
prove the fix rather than the defect: the project still refuses (a dirty file still blocks), and now
the real terminal's own pane is still present -- proven against a real spawned session, not a
synthetic list.

**Ablated**: forcing the old always-terminate branch (`ablate.sh`, the `if
close_assessment_blocked_by_more_than_running_processes(&pre_assessment)` guard replaced with `if
false`) fails this exact test, naming the real assertion that would have let the old ordering
through -- load-bearing, not accidentally green.

### A close that is permitted still terminates and still closes

The repair must not cost the working path. Both existing real-process tests pass unchanged:
`confirming_the_close_terminates_the_real_process_and_removes_the_project` (a clean real shell,
nothing else blocking, terminates and closes) and `closing_a_project_with_a_backgrounded_
descendant_kills_it_through_a_real_close` (RFC-043's own session-wide termination, unaffected by
where in the function the call now sits).

### Where the audit record is written (D11)

Decided as part of D1, not discovered mid-slice, per the RFC's own instruction. The refused branch
never calls `terminate_project_live_work` at all, so `terminal_session_confirmed_empty` has nothing
real to report there; it is set to `true` on that branch, which is inert -- the existing `&&
closed` at the write site already makes the recorded value `false` whenever `closed` is `false`,
regardless of what this field holds, so the refused case's own recorded value is unchanged from
today's. **Not fixed here, disclosed in the function's own trailing comment**: the audit record
itself still unconditionally claims `SafeCloseDecision::Closed{..}` on the refused path too --
Amendment 1's own D10, explicitly PR-066-B's job, not this slice's.

### RFC-027's interaction, checked (D12)

`remove_project_recovery_records_best_effort` already sat inside `if closed`; nothing about it
changed. The termination call now sits beside it, inside the same conditional branch the ordering
fix adds, not restructured relative to it.

### A stale doc comment, found and corrected

`apply_project_close_confirmation`'s own leading doc comment said *"Never the reverse"* about the
termination-then-assessment ordering -- which was the ordering the code actually used, making the
comment state the opposite of what shipped for over two releases. Corrected in place, with the
finding and the fix both named, rather than silently rewritten as if the comment had always been
accurate.

### Gate, PR-066-A

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `i18n::enforcement` (8/8), `rfc_docs_invariants` (17/17): clean (this slice touches no catalog
  string and no RFC document, so neither suite has anything new to check; both pass unchanged).
- `cargo test --doc --workspace`: clean.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/g11a1r{1,2,3}`): `752 + 17 + 1117`, 0 failed, 0 fixture entries left each time -- clean
  on the first attempt.
