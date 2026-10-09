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

### PR-066-B closed at review 504

*"Blocked by:"* is the right wording — it names the relationship (these are what prevent the close)
rather than a consequence of one that is not happening. My own ablation reproduces the defect's
exact symptom: `expected the blocked prefix, got "This will end: 1 unsaved file"`. Gate reproduces:
`758 + 17 + 1119`, 0 fixture entries.

Both intermittents from the failed first attempt have dated rows, and the gate was **redone rather
than counted** — the discipline review 497 had to ask for is now being applied unprompted. Those two
rows also make the case for the disposition pass already riding alongside this release.

## Whole-RFC

- [x] The colour-alone, i18n completeness and internal-identifier scans still pass.
      `shell::tests::no_raw_color_construction_anywhere_in_the_crate`,
      `i18n::enforcement::no_catalog_string_names_an_internal_identifier`,
      `i18n::enforcement::every_source_locale_key_resolves_in_every_shipped_locale` — each run
      directly in isolation, all three pass.
- [x] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check`
      **after staging**, `rfc_docs_invariants`, `cargo test --doc --workspace`, **three consecutive
      full-workspace runs with `--no-fail-fast`**, **0 fixture entries left** in a fresh short fixed
      `TMPDIR` — a literal, not `mktemp`.
      `rfc_docs_invariants`: 17/17. Doctests: clean. Three consecutive runs (`/dev/shm/g066wrfc2`):
      `758 + 17 + 1119`, 0 failed, 0 fixture entries left each time — the first attempt's own run 2
      failed on `closing_a_project_with_a_backgrounded_descendant_kills_it_through_a_real_close`,
      an *already-registered* row (`0.16.0` release gate, 2026-08-28 — PTY read timing), passed
      immediately in isolation; the gate was redone, not counted, per this register's own
      convention — no new row needed since this one already has one.
- [x] Every new intermittent has a dated row in `test-process-leak.md`.
      Both of review 503's own gate failures (row 787's fourth occurrence,
      `terminal_poll_handler_cost_under_a_real_wake_driven_flood_headless_benchmark` new) already
      recorded with a dated row in `qa-evidence.md`'s own PR-066-B section; nothing new surfaced in
      this pass.
- [x] The changelog is written **incrementally as each slice closes**, and re-read against the
      finished set at the candidate — not against the last slice's diff.
      `## 0.32.0` section re-read against the finished two-slice set: added the lead paragraph
      review 504 asked for ("this started as one defect... and turned out to be three"), tying the
      three repairs (terminals, silence, audit record) to the one underlying claim rather than
      leaving them as three separate, unconnected paragraphs a reader of the title would not
      connect on their own.
- [x] The book is read against the changelog **in both directions**, including any user-visible word
      this RFC's commits touch.
      Swept `docs/src/users/*.md` for every existing mention of project-closing behaviour.
      `what-works-today.md:218`'s "every choice also has a real clickable button" still holds for
      the blocked modal's own single `Dismiss` — not stale, left unchanged.
      `configuration.md:95-96`'s `open_safe_close_dialog` entry describes routing, not dialog
      content — not stale. The one real gap: `working-with-projects.md`'s own "Closing a project"
      section described only the offered-confirm case; added a new paragraph for the blocked-
      up-front case (D3's own title, prefix and `Dismiss` wording, named exactly as the catalog
      strings read).
- [x] `what-works-today.md` says what a blocked close now does, since the behaviour a user meets
      changes.
      **Named the wrong page; the substance is covered.** `what-works-today.md` itself needed no
      edit (see above — its one close-dialog claim already generalizes correctly). The page that
      actually walks through close behaviour in the detail this item asks for is
      `working-with-projects.md`'s own "Closing a project" section, updated above.
- [x] The core pin bumps with the version. *(Release-cut item.)*
      `[workspace.package] version` and the `tekstide-core` pin both bumped `0.31.0 -> 0.32.0`;
      `the_workspace_pins_tekstide_core_to_its_own_version` passes.
- [x] Commits are pushed once the gate is green.
      Three consecutive full-workspace runs clean on the first attempt (`758 + 17 + 1119`, 0
      failed, 0 fixture entries left); RFC-066 moved `rfcs/accepted/` -> `rfcs/done/` with a new
      `## Closed (2026-10-09)` section, `rfcs/README.md`'s three tables and `rfcs/delivery-plan.md`'s
      own register row updated to match, `CHANGELOG.md`'s status line promoted to "candidate, not
      yet published." Pushed.

### Required at review 505 — a false defect is about to be immortalised

The candidate itself is sound: version, pin and `Cargo.lock` at `0.32.0`; the lifecycle move —
`done/`, the README tables **and the delivery-plan row** — all in `63b7288`; `cargo package
--workspace` verifying both crates with the packaged archive naming `tekstide-core 0.32.0`; audit
matching the register's three rows; no accesskit; gate `758 + 17 + 1119`, 0 fixture entries. **The
check I required at review 499 caught the stale register row for you** — that is the first time it
has earned itself.

- [x] **`Ctrl+Alt+N` is not broken, and RFC-066's own `Closed` section now says it is.** Lines
  185–191 claim *"`NavigationAction::SwitchActiveProject` has no production caller in the crate at
  all"* and that `working-with-projects.md` is wrong to say the chord works. **Both are false, and
  the RFC is already in `done/`.**

  - `shell.rs:2479` dispatches it: `if action == NavigationAction::SwitchActiveProject {
    cycle_to_next_active_project(state); }`.
  - `cycle_to_next_active_project` (`shell.rs:5229`) **returns early when `project_count < 2`** —
    a deliberate no-op, the same design as `Ctrl+Alt+F`, which has its own
    `is_a_no_op_with_fewer_than_two_documents_open` test.
  - `ctrl_alt_n_cycles_to_the_next_open_project_wrapping` passes. I ran it.
  - **Your live capture had one project** — "1 project" in its own status bar. So `Ctrl+Alt+N` did
    exactly what it is specified to do.

  **Where the misreading came from**, because it is an easy one: `shell.rs:10955`'s comment says
  *"`AppState::switch_active_project` has no production caller anywhere in this crate;
  `NavigationAction::SwitchActiveProject` itself maps to no `AppCommand`."* Both halves are true and
  neither means the chord is unwired — the action is handled directly in the shell's own arm rather
  than through `AppCommand`, which is what that comment is explaining.

  Remove the claim. **The book is correct and must not be "fixed" to match a defect that does not
  exist** — that would turn a false finding into a real one.

  **Fixed, same review.** The claim removed from the `Closed` section; `working-with-projects.md`
  left untouched. The mistake, and the correct verification (`shell.rs:2479`,
  `cycle_to_next_active_project`'s own no-op, `ctrl_alt_n_cycles_to_the_next_open_project_wrapping`),
  recorded in `qa-evidence.md` rather than silently erased.

**Worth keeping from the same passage:** the *"did nothing"* observation was real and worth chasing.
The error was confirming it against a comment instead of against the dispatch; one `grep` for the
action in a non-test file would have shown line 2479.

### Closed at review 506 — the `0.32.0` candidate is accepted

The false claim is gone from `rfcs/done/`, `working-with-projects.md` is untouched, and **the
mistake is recorded in `qa-evidence.md` rather than deleted** — including *why* it happened
(verified against `shell.rs:10955`'s comment, which is true but about `AppCommand` routing, instead
of against the dispatch). A corrected record that erases the error teaches the next reader nothing;
this one does not.

Verifying my correction before acting on it was also right. I could have been wrong, and the cost of
checking was one `grep`.

Gate: `758 + 17 + 1119`, 0 failures, 0 fixture entries, clean on the first attempt. Tree clean,
nothing unpushed. **`0.32.0` is recommended to the owner.**

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
