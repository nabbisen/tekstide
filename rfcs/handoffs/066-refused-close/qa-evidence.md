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

## Review 501: a false value kept safe only by code the next slice is about to rewrite

Required item, one character. The refused branch's own `terminal_session_confirmed_empty: true` was
inert today only because `&& closed` at the write site forces the recorded value to `false`
regardless -- but `true` is itself wrong (nothing was terminated, so nothing was confirmed empty),
where `false` is correct on its own terms, not merely equally masked. **D10 is PR-066-B's own job to
rewrite that exact write site**, and a value that is wrong but currently hidden by an assumption
held somewhere else is the identical shape that produced review 490's own data-loss defect at
RFC-027 (`content_hash: None` documented safe "whenever the file is within the policy's editable
bound," until a file was not).

Fixed: `(false, false)` instead of `(true, false)`, with the reasoning -- and the explicit pointer
to why it matters for the slice about to touch this exact site -- recorded in the code's own
comment, not only here.

### Gate, review 501's fix

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `a_confirmed_close_blocked_by_a_dirty_file_leaves_the_terminal_alive`,
  `confirming_the_close_terminates_the_real_process_and_removes_the_project`: both pass unchanged
  (neither asserts on this field's own value directly, so the fix is observationally silent today,
  exactly as the review's own framing said it would be).
- `cargo test --doc --workspace`: clean.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/g12a1r{1,2,3}`): `752 + 17 + 1117`, 0 failed, 0 fixture entries left each time -- clean
  on the first attempt.

## PR-066-B — refuse up front

### A real, production bug found by the audit-record test the checklist itself demanded

The checklist's own PR-066-B row says *"nothing pins the refused case's record today, which is why
it went unnoticed."* Writing that test (`activating_a_blocked_close_records_blocked_not_cancelled_
or_closed`) found exactly that: the first version of the new `SafeCloseDecision::Blocked` write --
plumbed correctly at the Rust level, `valid_safe_close` updated to accept it -- still produced
**zero** audit records. Not a validation rejection (`AuditObservationStatus::Degraded` would have
been silent in production the same way, but the test asserted the record directly and got `[]`
instead of one row).

Traced to the real cause, not patched around the symptom: `crates/tekstide-core/src/audit/
schema.rs`'s `safe_close_decision` family CHECK -- the actual SQLite constraint, not `valid_safe_
close`'s own Rust-level mirror of it -- only ever admitted `outcome IN ('authorized', 'applied',
'failed')` with an `operation_id`, or `outcome = 'cancelled'` with none. There was no branch for
`'blocked'` at all. `valid_safe_close` passing was necessary but not sufficient; SQLite itself
rejected the insert (`AuditStoreError { reason: Io }`, confirmed directly in the ablation below),
and `append_observation`'s own best-effort design means that failure is swallowed in production,
exactly the shape that let the gap stay unnoticed until a test queried the store back rather than
trusting that the Rust-level validator agreeing meant the write would succeed.

**This family's own CHECK is baked into the table DDL, and SQLite cannot `ALTER` a `CHECK`
constraint** -- RFC-013 Amendment 1's own `1 -> 2` migration already established the fix shape for
exactly this situation (a new `MigrationStep` and a new `AUDIT_SCHEMA_VERSION`, never hand-editing
a shipped version's DDL in place), so this follows that shape rather than inventing a new one:

- `AUDIT_SCHEMA_VERSION` bumped `2 -> 3`.
- `audit_events_v3_table_ddl!` -- a full second copy of the v2 macro (the same reasoning v1 and v2
  are independent literal blocks, not a shared template: each frozen version's text must stay
  exactly what a real installation at that version ran) with exactly one changed line: the
  `safe_close_decision` branch now reads `OR (outcome IN ('cancelled', 'blocked') AND operation_id
  IS NULL)`.
- `CREATE_SCHEMA_V3` for a fresh install; `CREATE_SCHEMA_V2` kept (`#[allow(dead_code)]`, matching
  `CREATE_SCHEMA_V1`'s own established disposition) as the historical record and the companion
  fixture test's own subject.
- A `2 -> 3` `MigrationStep` in `migration.rs`, the identical table-rebuild/`sqlite_sequence`-carry/
  index-recreate pattern the `1 -> 2` step already established and documents in full -- no column
  added or removed, so the `INSERT ... SELECT` column list is unchanged from that step's own.
- `audit-v3.sql` fixture added (mirrors `audit-v2.sql`, `user_version = 3`, the one CHECK branch
  updated); `create_schema_v3_constant_matches_the_expected_fixture_exactly`,
  `canonical_v3_fixture_opens_and_remains_current` added, mirroring their v1/v2 siblings exactly.
- `canonical_v2_fixture_opens_and_remains_current` converted to `v2_fixture_with_existing_rows_
  migrates_to_v3_preserving_sequence_and_accepts_the_new_blocked_outcome` (the identical shape the
  `1 -> 2` step's own conversion already used for its anomaly fix): two rows inserted directly
  against the raw v2 schema, opened through the real `AuditStore::open` (which runs the real
  migration), a `Blocked` safe-close write proven to persist, both pre-existing rows proven to keep
  their original `sequence`, database ends at `user_version = 3`.
- `missing_store_initializes_current_identity`, `future_schema_and_foreign_application_are_
  rejected_without_writes` (both `migration.rs` and `recovery.rs`'s own copy), and the renamed
  `fresh_install_and_migrated_v1_fixture_produce_identical_schema` convergence test all updated for
  the new current version (`3`, not `2`; the future-schema probe moved `3 -> 4`) -- the convergence
  test's own property (a fresh install and a migrated-from-v1 database produce byte-identical
  `sqlite_master` DDL) is unaffected by which version is current and needed no logic change, only
  its doc comment and name updated for honesty about what it now proves.

**Ablated** (`ablate.sh`, the new CHECK branch reverted to the old `OR (outcome = 'cancelled' AND
operation_id IS NULL)`): four tests fail --
`activating_a_blocked_close_records_blocked_not_cancelled_or_closed`,
`escaping_a_blocked_close_also_records_blocked_not_cancelled` (both now get `[]` back, the exact
shape that found the bug), and `create_schema_v3_constant_matches_the_expected_fixture_exactly` /
`v2_fixture_with_existing_rows_migrates_to_v3_preserving_sequence_and_accepts_the_new_blocked_
outcome` (the latter's own assertion failure names the cause directly: `a Blocked safe-close
decision must persist on a migrated database: Err(AuditStoreError { reason: Io })`) -- load-bearing,
not accidentally green.

### D3 — refuse up front, no confirm button at all

`ProjectCloseModal` gained `can_close: bool`, computed once in `attempt_close_project_tab` from the
same `assess_project_close` call the modal's own `reasons` already come from (`close_assessment_
blocked_by_more_than_running_processes`, unchanged from PR-066-A), and threaded everywhere the
modal's own focus or activation is decided:

- `ModalFocusNext`/`ModalFocusPrevious`: a no-op against this modal while `can_close` is `false` --
  focus cannot cycle to a `Close` that was never offered.
- `activate_current_modal`: a new guarded arm, checked *before* the `focus == Close` arm, fires
  unconditionally when `!can_close` and records `Blocked` -- never reaches `apply_project_close_
  confirmation`.
- `ModalDismiss` (Escape): the same `can_close` check, `Blocked` instead of `Cancelled` when it is
  `false`.
- `project_close_dialog_view`: renders no `Close` control at all when blocked (not a disabled one --
  the footer is built from a `Vec` that only ever pushes the `Close` button when `can_close` is
  `true`), a distinct title (`project-close-dialog-blocked-title`, "This project can't be closed
  yet" -- not a question with a withheld "yes"), a single `Dismiss` control (not "Cancel," which
  would imply a close was on offer to decline), and a hint that names only the one real affordance
  (`project-close-dialog-blocked-hint`, "Escape dismisses.").

Proved by `a_close_blocked_by_a_dirty_file_opens_with_no_close_to_focus`: `can_close` is `false` for
the same dirty-file fixture PR-066-A's own ordering test uses, focus opens on `Cancel`, and two
`ModalFocusNext` plus one `ModalFocusPrevious` all leave it there. The ordering regression test
itself (`a_confirmed_close_blocked_by_a_dirty_file_leaves_the_terminal_alive`) updated to match:
driving it through `ModalFocusNext` to reach `Close` is no longer a thing this modal permits, so it
now activates straight from the default `Cancel` focus -- the one reachable path, and still exactly
what D1's own fix must get right regardless of how the button is reached.

**Ablated**: removing `activate_current_modal`'s new `!modal.can_close` guard entirely (`ablate.sh`)
falls through to the pre-existing unguarded `ProjectClose` arm, and `activating_a_blocked_close_
records_blocked_not_cancelled_or_closed` fails with `left: Cancelled, right: Blocked` -- the exact
wrong value a user-declined-an-offered-close record would carry, proving the guard is what prevents
it. Removing `ModalDismiss`'s own `can_close` branch (separately ablated) fails `escaping_a_blocked_
close_also_records_blocked_not_cancelled` the same way -- two independent guards, each shown
load-bearing on its own, not one covering for the other.

### D4 — the reasons are read from `assess_close`'s own result, not re-derived

`the_blocked_modals_reasons_are_assess_closes_own_result_not_a_second_opinion`: after the modal
opens, a second, independent call to `assess_project_close` (the project's state unchanged in
between) is compared against `modal.reasons` field-for-field. They agree because they are the same
data -- `attempt_close_project_tab` destructures `reasons` directly out of the `NeedsConfirmation`
match arm into the modal, never recomputing or filtering it. No second predicate exists anywhere in
this change that decides which reasons to show.

### §row 3 — no forced close exists anywhere in the change

Traced, not assumed: `Message::ProjectCloseClosePressed` (the only message that can reach `Close`'s
own `focus = Close; activate_current_modal(state)` path) is dispatched from exactly one place in the
view, and that view no longer renders the `Close` control at all when `can_close` is `false` -- so
the message cannot originate. Defense in depth beyond the view: even if `ProjectCloseClosePressed`
somehow reached `activate_current_modal` anyway (a stray `Message` construction, not a real
affordance), the new `!modal.can_close` guard sits *before* the `focus == Close` guard in the same
match, so it would still be caught and recorded as `Blocked`, never reaching `apply_project_close_
confirmation`. No code path bypasses the assessment.

### Live capture -- a real blocked close, naming its own blocking reason on screen

`cargo build --release -p tekstide`, isolated `XDG_STATE_HOME` under `/dev/shm` (a throwaway state
dir, matching PR-027-C's own corrected practice -- never the real one), a throwaway project
(`/dev/shm/tsd066b-proj`, one file, `notes.txt`) opened from the CLI argument. `niri msg action
screenshot-window` + `wl-paste --type image/png` the capture method every prior slice established;
every screenshot confirmed to show only this throwaway project before saving, per `committed-
screenshots-throwaway-state-only`'s own requirement.

Real keyboard navigation throughout (Tab cycles `FocusZone::MainArea -> Sidebar -> TabStrip`, the
same zones `input.rs` defines): opened `notes.txt` from the Sidebar/Explorer (real `Enter`), typed a
real character into the editor (`notes.txt (unsaved changes)` appears), moved focus to the TabStrip
and pressed `Delete` on the open project's own tab -- the real `Message::CloseProjectTabPressed`
path, not a synthetic message.

- `rfcs/handoffs/066-refused-close/evidence/pr-066-b/dirty-file-before-close-attempt.png` -- the
  real unsaved edit, before any close attempt: `notes.txt (unsaved changes)`.
- `rfcs/handoffs/066-refused-close/evidence/pr-066-b/blocked-modal-no-confirm-button.png` -- **the
  required proof for D3**: "This project can't be closed yet", the real canonical path
  (`/dev/shm/tsd066b-proj`), "This will end: 1 unsaved file" (the real count, read from `assess_
  close`'s own result), a single `Dismiss` control -- no `Close` button anywhere in the dialog --
  and the hint "Escape dismisses."
- `rfcs/handoffs/066-refused-close/evidence/pr-066-b/after-dismiss-project-still-open.png` -- real
  `Escape`: the project and its unsaved edit both untouched, exactly as the unit tests already
  proved.

**The real audit write, confirmed independently of the unit tests**: with the app still running
(WAL mode, concurrent reads permitted), `sqlite3 audit.sqlite3 "SELECT family, outcome,
operation_id, created_at FROM audit_events ORDER BY sequence DESC LIMIT 5"` against the live store
showed `safe_close_decision|blocked||2026-10-09T08:42:31Z` as the most recent row -- the real
`Escape` dismissal above, recorded as `Blocked` with no `operation_id`, read directly out of the
real database rather than only through `AuditStore`'s own query API.

Process terminated cleanly with `SIGTERM` after the capture session; throwaway state, project
directory and intermediate screenshots removed from `/dev/shm` afterward, keeping only the three
images copied into this RFC's own `evidence/pr-066-b/` directory.

### Gate, PR-066-B

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `i18n::enforcement` (23/23, three new catalog keys -- `project-close-dialog-blocked-title`,
  `project-close-dialog-dismiss`, `project-close-dialog-blocked-hint` -- none introducing a new
  Fluent variable, so `generic_args` needed no new entry), `rfc_docs_invariants` (17/17): clean.
- `cargo test --doc --workspace`: clean.
- `cargo test -p tekstide-core audit::`: 130/130, including the new `v2_fixture_with_existing_rows_
  migrates_to_v3_...` and `canonical_v3_fixture_opens_and_remains_current`.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/g066b1`, `/dev/shm/g066b1` recreated each run): `757 + 17 + 1119`, 0 failed, 0 fixture
  entries left each time.
