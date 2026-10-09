# RFC-066 acceptance and QA checklist

Tick a box only when the thing it names has been **run**.

## PR-066-A — reproduce, then repair

- [x] **Reproduced first, against real things** (D5): a real project, a real spawned terminal, a
      real unsaved edit, a real confirmed close — the terminal dies, the project stays open.
      **If it did not reproduce, that is reported instead of a fix.**
      Reproduced exactly as read: `a_confirmed_close_blocked_by_a_dirty_file_leaves_the_terminal_alive`
      (`shell::tests`), run against the unmodified code with its assertions inverted from their
      current form, passed — the real terminal's pane was gone and the project stayed open.
- [x] The assessment completes before anything live is terminated (D1, §row 1).
      `apply_project_close_confirmation` calls the read-only `assess_project_close` first;
      `close_assessment_blocked_by_more_than_running_processes` (`shell.rs`) decides whether
      anything live is touched at all.
- [x] A confirmed close refused for an unsaved file **leaves every running terminal alive**, proven
      against real spawned sessions, not a synthetic list.
      `a_confirmed_close_blocked_by_a_dirty_file_leaves_the_terminal_alive` (`shell::tests`), same
      test as above, assertions now proving the fix.
- [x] A close that *is* permitted still terminates and still closes — the repair must not cost the
      working path.
      `confirming_the_close_terminates_the_real_process_and_removes_the_project`,
      `closing_a_project_with_a_backgrounded_descendant_kills_it_through_a_real_close`
      (`shell::tests`) both pass unchanged.
- [x] The reproduction is kept as the regression test.
      Same test, renamed and re-asserted rather than replaced — `qa-evidence.md` records both what
      it proved before the fix and what it proves now.
- [x] **Where the audit record is written is decided, not inherited** (D11):
      `terminal_session_confirmed_empty` comes from the termination, which D1 moves.
      Decided: the refused branch never terminates, so the field is inert there (`&& closed`
      already forces the recorded value to `false` whenever `closed` is, regardless of this
      field) — same recorded outcome as today for the refused case, decided explicitly rather than
      left to fall out of the refactor by accident. Full reasoning in `qa-evidence.md`.
- [x] RFC-027's `remove_project_recovery_records_best_effort` still runs only on the closing path,
      and the termination now sits beside it (D12).
      Unchanged — still inside `if closed`, termination now sits in the branch that leads there,
      not restructured relative to it.

### Required at review 501

The slice is right. **Reproducing first, against a real `/bin/sh` and a real unsaved edit, and
watching the terminal die while the project stayed open** — rather than trusting the chain I traced
in code at review 489 — is exactly what D5 asked for, and it is the half of this RFC I could not do
myself. The guard handles combinations correctly (`any(|r| r.code != RunningProcess)`, so
`{RunningProcess, DirtyFile}` refuses without touching anything), `UnsupportedOrUnknown` is
conservative, and the stale *"never the reverse"* doc comment — which described the opposite of what
the code had done for two releases — was worth finding. My own ablation agrees with yours: forcing
the old branch fails `a_confirmed_close_blocked_by_a_dirty_file_leaves_the_terminal_alive`. Gate
reproduces: `752 + 17 + 1117`, 0 fixture entries.

- [x] **`terminal_session_confirmed_empty: true` on the refused branch** — **fixed at review 502**,
  and anchored better than I argued it. I reasoned from review 490's masked-sentinel shape; the dev
  team found that **this codebase had already decided it explicitly for this identical field**:
  `terminated_outcome_and_session_confirmation`'s own doc says *"`confirmed` defaults to `false`,
  never a hopeful `true` … (D3's own honesty rule)"*, three lines away. The rule was already there;
  `true` broke it. Gate reproduces: `752 + 17 + 1117`, 0 fixture entries.

  ~~Original finding:~~ **`terminal_session_confirmed_empty: true` on the refused branch is a false value kept safe by
  code the next slice is about to rewrite.** Nothing was terminated, so nothing was confirmed empty
  — **`false` is equally inert and also true.** `true` is only harmless because `&& closed` at the
  write site forces it, and **D10 is PR-066-B's job to change that exact write site.** A value that
  is wrong but masked by an assumption held elsewhere is the shape that produced review 490's
  data-loss defect: `content_hash: None` was safe *"whenever the file is within the editable
  bound"*, and then one was not.

  One character, and it removes a trap laid directly in the path of the next slice.

### PR-066-A closed at review 502

Reproduced against real processes before repairing, ordering fixed, the guard correct for
combinations, D11 decided and D12 checked, both my ablation and theirs showing the regression test
load-bearing. Nothing outstanding.

## PR-066-B — refuse up front

- [x] A close the assessment blocks shows the reasons and **offers no confirm button** (D3).
      `ProjectCloseModal.can_close`; `project_close_dialog_view` builds its footer from a `Vec`
      that only ever pushes `Close` when `can_close` is `true` -- not disabled, absent. Proved by
      `a_close_blocked_by_a_dirty_file_opens_with_no_close_to_focus` (focus can never reach
      `Close`) and live capture (`blocked-modal-no-confirm-button.png`: a single `Dismiss`
      control, no `Close` anywhere in the dialog).
- [x] The reasons are read from `assess_close`'s own result, not re-derived (D4, §row 4).
      `the_blocked_modals_reasons_are_assess_closes_own_result_not_a_second_opinion`: a second,
      independent `assess_project_close` call after the modal opens agrees with `modal.reasons`
      field-for-field, because `attempt_close_project_tab` destructures `reasons` directly out of
      the same match arm the modal is built from -- no second predicate exists.
- [x] **No forced close exists anywhere in the change** (§row 3).
      `Message::ProjectCloseClosePressed`'s one dispatch site is gone from the view when blocked;
      `activate_current_modal`'s new `!modal.can_close` guard sits before the `focus == Close`
      guard in the same match as defense in depth, so even a stray `Close`-focused activation
      would still record `Blocked`, never reach `apply_project_close_confirmation`. Full reasoning
      in `qa-evidence.md`.
- [x] Live capture of a refused close, with the blocking reason named on screen.
      `rfcs/handoffs/066-refused-close/evidence/pr-066-b/blocked-modal-no-confirm-button.png`: a
      real dirty file blocking a real close attempt (`Delete` on the project's own tab), "This
      project can't be closed yet", the real path, "This will end: 1 unsaved file", one `Dismiss`
      control. Confirmed against the live, isolated `XDG_STATE_HOME` under `/dev/shm`, never a
      path under `$HOME`. The real audit write confirmed independently with `sqlite3` against the
      live store, not only through the unit tests.
- [x] **A refused close is not recorded as `Closed` in the audit store** (D10), and not as
      `Cancelled` either — that already means the user dismissed the modal. **Proved by a test;
      nothing pins the refused case's record today**, which is why it went unnoticed.
      `activating_a_blocked_close_records_blocked_not_cancelled_or_closed`,
      `escaping_a_blocked_close_also_records_blocked_not_cancelled`. Writing these found a real,
      independent bug beyond the Rust-level plumbing: the `safe_close_decision` family's own SQL
      `CHECK` constraint had no branch admitting `outcome = 'blocked'`, so every such write was
      silently rejected at the SQLite layer -- fixed with a real schema migration (`AUDIT_SCHEMA_
      VERSION` `2 -> 3`, a new `MigrationStep`, following RFC-013 Amendment 1's own established
      shape exactly), not a workaround. Full account, including the ablation that reproduces the
      original rejection, in `qa-evidence.md`.

### Required at review 503

Substantial and well-evidenced. The schema migration is the right call over a workaround, follows
RFC-013 Amendment 1's own established shape, and `v2_fixture_with_existing_rows_migrates_to_v3_preserving_sequence`
proves the part that matters — an existing database survives it. Two guards, each ablated
separately, is the right way to show defense-in-depth is actually two things. The capture is clean:
isolated `/dev/shm` state, one throwaway project, *"This project can't be closed yet"* as a statement
rather than a question with a withheld yes, one `Dismiss`, no `Close` anywhere. Audit suite 130/130;
gate reproduces `757 + 17 + 1119`, 0 fixture entries.

- [ ] **The reasons line still uses the confirm modal's own prefix.** `project-close-dialog-live-work-prefix`
  = *"This will end:"* is shared, so the blocked modal reads:

  > This project can't be closed yet
  > `/dev/shm/tsd066b-proj`
  > **This will end: 1 unsaved file**

  Nothing will end. The close is not happening — that unsaved file is the **reason** it cannot, not
  a consequence of it proceeding. On the confirm modal the string is correct; on this one it asserts
  the opposite of the title two lines above it.

  **This is the RFC's own defect in miniature**: the refusal is right and something downstream still
  speaks as though the close is going ahead. You gave the blocked modal its own title, its own
  dismiss control and its own hint — this is the fourth string that needed the same treatment.

**Not required here, raised to the owner instead.** Your `Blocked` write passing `valid_safe_close`
and still producing **zero** records — rejected by a SQL `CHECK` the Rust validator only mirrored,
and swallowed by best-effort — is an instance of a class: **52 `CHECK` constraints in
`audit/schema.rs`, and nothing anywhere asserts that a given family and outcome actually lands.** I
checked. Any future producer fails the same way: silently. Recorded in `future-work.md` as a pre-1.0
item, with `enumeration_confirms_only_the_closed_list_reads_full_file_content` named as the shape
and `record.rs`'s 69 family arms as the enumeration. **You found it only because the checklist
demanded a test for this one case** — nothing systemic would have.

## Whole-RFC

- [ ] The colour-alone, i18n completeness and internal-identifier scans still pass.
- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check`
      **after staging**, `rfc_docs_invariants`, `cargo test --doc --workspace`, **three consecutive
      full-workspace runs with `--no-fail-fast`**, **0 fixture entries left** in a fresh short fixed
      `TMPDIR` — a literal, not `mktemp`.
- [ ] Every new intermittent has a dated row in `test-process-leak.md`.
- [ ] The changelog is written **incrementally as each slice closes**, and re-read against the
      finished set at the candidate — not against the last slice's diff.
- [ ] The book is read against the changelog **in both directions**, including any user-visible word
      this RFC's commits touch.
- [ ] `what-works-today.md` says what a blocked close now does, since the behaviour a user meets
      changes.
- [ ] The core pin bumps with the version. *(Release-cut item.)*
- [ ] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
