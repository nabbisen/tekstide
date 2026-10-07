---
title: "RFC-065 — QA evidence"
rfc: "RFC-065"
rfc_file: "../../accepted/065-the-multi-document-model.md"
source_rfc_status: "Accepted 2026-10-07 — M13"
target_milestone: "M13"
created: "2026-10-07"
---

# QA evidence

## PR-065-A — the repair

### The test was written first and shown failing against today's code

Commit `b1305c4` (`RFC-065 D1: the open set -- opening a second file no longer discards the
first`): `opening_a_second_file_leaves_the_first_s_text_and_dirty_state_intact` was written
against the pre-repair code (`open_text_document` still replacing, `self.documents =
vec![document]`, marked `TODO`) and failed with `open_buffer_count() == 1` where `2` was
expected -- the exact shape of the live defect (`what-the-open-set-must-not-do.md` §1: `Action::
Open(path)` reaching `self.active_document = Some(document)` with no `dirty`/`unsaved`/`confirm`
check anywhere on the chain). One line then flipped push-not-replace and the same test passed
with no other change. The commit message records the exact failure.

**Re-verified by ablation this response**, against the committed repair:

```
$ bash rfcs/handoffs/ablate.sh "RFC-065-D1-regression" crates/tekstide-core/src/project/content.rs \
    "self.documents.push(document);
                self.active_index = Some(self.documents.len() - 1);" \
    "self.documents = vec![document];
                self.active_index = Some(0);" \
    -p tekstide-core opening_a_second_file

=== RFC-065-D1-regression
project::tests::content::opening_a_second_file_leaves_the_first_s_text_and_dirty_state_intact
project::tests::content::opening_a_second_file_leaves_the_first_s_undo_history_intact
result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 1079 filtered out; finished in 0.00s
thread '...text_and_dirty_state_intact' panicked:
assertion `left == right` failed: both documents must still be open -- the first was not discarded
  left: 1
 right: 2
thread '...undo_history_intact' panicked:
the first document must still be in the open set
```

Both tests fail for exactly the defect's own reason when the repair is reverted, and the file
was restored by `ablate.sh` immediately after (confirmed: working tree clean, `git status
--porcelain` empty, before and after).

### Editing a file and opening another leaves the first's text and dirty state intact

`opening_a_second_file_leaves_the_first_s_text_and_dirty_state_intact`
(`crates/tekstide-core/src/project/tests/content.rs`): edits `first.txt`, opens `second.txt`,
asserts `open_buffer_count() == 2`, `dirty_file_count() == 1`, and that the first document's own
text and `TextDocumentState::Dirty` survive untouched. Passing, part of the 1079-test
`tekstide-core` unit run.

### Captured live: edits made, a second file opened, the first still there

`rfcs/handoffs/065-multi-document/evidence/pr-065-a/` (committed) -- see its own `README.md` for
the full sequence and two disclosed findings made while capturing it (the explorer's `[open]`
tag reflecting only the active document, and reopening an already-open path producing a second
entry rather than switching to the first). Also documents a false alarm (an apparently-zero
board count in an earlier, less careful capture) run down with a dedicated shell-level test
rather than either dismissed or reported as a defect without evidence.

### The undo history of the first document survives

`opening_a_second_file_leaves_the_first_s_undo_history_intact`
(`crates/tekstide-core/src/project/tests/content.rs`): plants a real undo entry on `first.txt`
via the actual two-step edit path (`replace_active_text` + `record_active_edit_operation`, the
same shape a real keystroke uses -- `replace_active_text` alone, used by the text/dirty-state
test above, never records an undo entry), opens `second.txt`, and asserts `first.can_undo()`
still holds. Not captured live (no checklist item asks for that); the undo stack is owned by the
`TextDocument` value itself, which the open set never replaces or drops, so the same mechanism
the text/dirty-state proof demonstrates live also carries the undo stack.

### Nothing else in this slice

The diff is `ProjectContentWorkspace`'s open set (`documents: Vec<TextDocument>` +
`active_index: Option<usize>`, replacing `active_document: Option<TextDocument>`),
`open_buffer_count`/`dirty_file_count` counting the whole set (the two sites D2 names), the
`open_documents()` read-only iterator the tests and the live evidence both needed, and the
tests themselves. No switcher, no bound, no `REQ-EDIT-004` coverage-row change, no watcher-scope
change beyond the one-line consequence of `active_document()`'s own type -- all explicitly
PR-065-B/C/D's own work, per the task-breakdown's own ordering.

## Gate (run this response, against commit `a1a3222`)

- `cargo fmt --check`: clean (one line in `content.rs` was not run through `cargo fmt` before
  `b1305c4`'s own commit -- caught and fixed here, disclosed in that commit's own message).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `git diff --cached --check` after staging: clean, each time something was staged.
- `cargo test --doc --workspace`: 2 passed, 0 failed (both doctests live in `project::watch::
  owner`, untouched by this slice).
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**:
  run 1 clean (`731 + 16 + 1079`, plus 0+1+1 doctests), run 2 clean (same), run 3 showed one
  failure in the long-standing, already-documented `bind_recovers_from_a_stale_socket_file`
  flake (`rfcs/handoffs/test-process-leak.md`, row 1) -- not the slice (the whole diff is
  `content.rs`, its own tests, and one `shell/tests.rs` test; nothing near the approval
  channel). Passed in isolation; a dated recurrence row was added to the register and **the gate
  was redone, not counted**, per that register's own 2026-09-29 convention. Final accepted three
  runs: `731 + 16 + 1079`, `731 + 16 + 1079`, `731 + 16 + 1079`, every run.
- **0 fixture entries left** in each run's own fresh short `TMPDIR`, checked after.
- The colour-alone, i18n-completeness and internal-identifier scans are part of the 731-test
  `tekstide` unit run above and passed with it; no separate invocation exists for them.
- No core-pin or version bump: this slice is not its own release (`0.30.0` is the whole RFC's
  target, per its own `README.md`), so `Cargo.toml` is untouched here.
- Commits pushed once this gate was green (see the response's own final commit list).

## PR-065-B — the set

### Required at review 468 (landed in B, per the ruling)

- **Dedup by path.** `open_text_document` now switches to an already-open path's existing
  entry in place (no disk read) instead of adding a second -- checked after dedup, before any
  disk read, the same structural-then-policy order `add_terminal_session`'s own
  `terminal_session_limit` uses.
  `reopening_an_already_open_path_switches_to_the_existing_entry_rather_than_adding_a_second`
  (`project/tests/content.rs`) proves it, including that the entry's own text, dirty state and
  cursor survive untouched. The old behaviour's own test is renamed, not deleted, with a doc
  comment recording that it was once true and deliberate (slice A's own boundary).
  **Fallout caught by the full workspace run, not by inspection**: the Reload button relied on
  the old "open an already-open path always re-reads from disk" behaviour to discard local
  edits past a conflict -- exactly what the dedup switch stops doing. Split into its own entry
  point, `reload_active_document` (`content.rs`/`session.rs`/`app.rs`/`shell.rs`'s
  `ApplicationShell`), which always replaces the active entry's own slot from disk regardless
  of the open set. Four shell tests caught this before it shipped.
- **The explorer's `[open]` tag marks set membership.** `RowContext::open_paths: &[PathBuf]`
  (was `open_path: Option<&Path>`), built from `open_documents()` rather than
  `active_document()` alone. `open_marks_every_member_of_the_open_set_not_only_the_active_one`
  (`surface/explorer/tests.rs`) opens two files, leaves one active, and asserts both rows (and
  no others) carry the tag.

### The two counts, the close dialog, the watcher's scope

- `open_buffer_count`/`dirty_file_count` already counted the whole set as of slice A
  (`opening_a_second_file_leaves_the_first_s_text_and_dirty_state_intact`); re-confirmed here
  since B is this acceptance criterion's own slice.
- **D10, the close dialog needs no separate change.**
  `the_close_dialog_counts_the_whole_open_set_through_the_existing_wiring`
  (`project/tests/content.rs`): two documents open, one dirty, `close_resource_summary().
  dirty_files == 1` -- passes with no code change, since `session.rs`'s `set_file_state`
  already feeds it from `dirty_file_count`. The acceptance criterion's own test, not a second
  code path nobody needed.
- **The watcher's scope follows the set.** `watch_inputs()` changed from
  `active_document().into_iter()` to `open_documents()`.
  `the_watched_scope_grows_as_documents_in_new_directories_open` (`project/tests/content.rs`)
  opens documents across two directories and a third in an already-watched one, asserting the
  document-derived watched count grows by exactly one per new directory and not at all for a
  repeat.
- **Found while working this: a notice naming a background document's file was previously
  dropped.** `drain_project_watches` only ever refreshed the *active* document
  (`ProjectWatch.document_touched: bool`, keyed to one path) -- a real correctness gap this
  criterion is about closing, not just a measurement precondition: a background document's own
  external changes and conflicts would never be detected until it happened to become active.
  `ProjectContentWorkspace::refresh_document_by_canonical_path` generalizes
  `refresh_active_document` to any open document; `self.status` updates only when the refreshed
  document is the active one, so an unrelated background file changing cannot leak into what
  the chrome renders for the document on screen.
  `a_background_document_is_refreshed_by_its_own_path_without_touching_the_active_status`
  (core-level) and `an_external_change_to_a_background_open_document_reaches_it_too`
  (shell-level, real notice + real drain) both prove the fix and the isolation.

### D4, the bound is stated when reached

`ProjectResourceLimits.open_document_limit` (default `Some(20)`, reasoned the same way as the
struct's other bounds -- see its own doc comment). Enforced in `open_text_document`, after the
dedup switch (a path already open always belongs) and before any disk read.
`ProjectContentError::OpenSetAtLimit { open, limit }` names the refusal.
`opening_past_the_bound_is_refused_and_the_refusal_is_stated` (`project/tests/content.rs`)
proves the dedup switch bypasses the bound entirely, then that a genuinely new path is refused
with the exact count and limit, stated. **Stated even with a document active** (unlike every
other `OpenError`, which only ever occurred with nothing open): `open_error_line_while_active`
(`surface/editor.rs`), factored out and tested directly
(`open_error_line_while_active_is_none_ordinarily_then_states_the_bound`,
`surface/editor/tests.rs`). This also retroactively covers a pre-existing gap: any open
failure (too-large, not-UTF8, etc.) while a document was already active was previously set as
status but never rendered anywhere.

### D3, `REQ-EDIT-004`'s coverage row

`rfcs/delivery-plan.md`'s Text document row: "**`REQ-EDIT-004` was half met**... **and is met
by RFC-065 PR-065-B (2026-10-07)**: the open set is real, `open_buffer_count`/
`dirty_file_count` count every open document, and the close dialog counts the same way through
the same field (D10)" -- the same treatment RFC-057 gave `REQ-EDIT-002`'s own half-met row.

### D7/D13, the per-document refresh measurement

RFC-026's paired-control harness (`shell/tests/editor_baseline.rs`,
`editor_typing_latency_under_a_watched_burst`) is extended, not rebuilt, per D13: the single
`touch_document: Option<(PathBuf, String)>` generalizes to `touch_documents: Vec<...>`, and a
fifth condition, `WatchCondition::BurstWithNDocuments` (10 real documents, well below the real
`open_document_limit` default of 20, opened and rewritten through the same path the explorer
uses), joins the existing four in a 5-by-5 Latin square.

**Review 469's required correction**: the first pass computed the N/1 ratio on p95 keystroke
latency (read 1.11x) rather than delivery work -- the refresh runs *between* keystrokes,
exactly where p95 of the keystroke total cannot see it. `delivery_ms[round][condition]` now
parallels `p95`, recording `run.delivery` (what the runtime does between keystrokes) for every
condition; the per-condition note (previously `BurstBeforeD8`/`BurstWithD8` only) widens to
include `BurstWithNDocuments`. The summary reports both ratios, labeled for what each answers:
the p95 keystroke-latency one ("does holding N documents slow typing" -- a true, separate
question, 0.63x this run) and the delivery-work one (D7's own question).

**The release-mode build needed `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true`** -- not an
`iced` version mismatch, as review 469 identified: the setting this project's own review-462
evidence already records, needed because the suite's `()` renderer exists only with debug
assertions on. `cargo test --release -p tekstide --tests` is clean with it set; the 57
`E0277`/`E0599` errors without it are this project's own known precondition, not a defect.

**Real numbers, release mode, five rounds**:

```
median D8 cost over the five rounds (delivery work, with minus before): +4.244 ms
median 10-document cost over the five rounds (delivery work, N minus before): +37.294 ms
delivery-work ratio (10-document cost / one-document cost): 8.79x
```

8.79x against an expected ~10x **confirms measurement 9's own assumption**: the per-document
refresh cost is close to linear in the number of open documents, each refresh costing roughly
one whole-file read. The watcher's scope following the open set (this response's own earlier
fix, `watch_inputs()`) is therefore a real, measured ~4 ms-per-document cost at `N=10`, not an
assumed one -- and D4's own bound (`open_document_limit`, default 20) now has a measured
figure behind it rather than only the reasoned one its own doc comment gives.

Full output, including the per-round breakdown and the `BurstWithNDocuments` delivery notes
(1000 files written, 2218 notices, 3 explorer scans applied, 37-40 ms of delivery per round),
is in this response's own commit (`3d50652`)'s test run; not separately captured to a file,
since the harness's own `#[ignore]`d, by-hand-run nature (its own doc comment) means this is
not evidence a future gate re-derives automatically.

## Gate, PR-065-B (run this response, against commit `19b5235`, plus the review-469 fix)

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean,
  throughout (checked after every change in this response, not only at the end).
- `git diff --cached --check` after staging: clean, every time.
- `cargo test --doc --workspace`: 2 passed, 0 failed.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**:
  run 1 clean (`734 + 16 + 1083`), run 2 showed one failure in the already-documented
  `closing_a_project_with_a_backgrounded_descendant_kills_it_through_a_real_close` flake
  (`test-process-leak.md`, row 8) -- not the slice (the whole diff is the delivery-work ratio
  fix in `editor_baseline.rs`, a test-only file; nothing touches terminal termination). Passed
  in isolation; a dated recurrence row was added and **the gate was redone, not counted**.
  Final accepted three runs: `734 + 16 + 1083`, every run, 0 fixture entries left each time.
- No core-pin or version bump: `0.30.0` is the whole RFC's own target, not this slice's.
- Commits pushed once this gate was green.

## Review 470: independent reproduction, and the changelog

The architect reproduced the delivery-work measurement independently (load 14.65): D8 cost
`+4.219 ms`, 10-document cost `+38.901 ms`, ratio **9.22×** (submitted: `8.79×`) — measurement
9 confirmed a second time, from a second run. The p95 keystroke-latency ratio also reproduced
close to the submitted figure (`1.08×` against `1.11×` at review 469).

`CHANGELOG.md` gained a `## 0.30.0` entry, `Status: in progress`, directly answering `0.29.0`'s
own "what this release does not do" question about multi-document refresh cost -- written now,
incrementally, rather than held back for the eventual release candidate's own pass (the
convention `0.29.0`'s own entry was written under). Updated as C and D close.

## PR-065-C — reaching the second document

### A switcher, keyboard-first, reachable, captured live, no environment variable

`NavigationAction::SwitchActiveDocument`, bound to `Ctrl+Alt+F` -- the direct analogue of
`SwitchActiveProject`'s own `Ctrl+Alt+N`, for the open document set rather than the project
list. Cycles `ProjectContentWorkspace::cycle_to_next_open_document` through `self.documents`'s
own order, wrapping, a no-op with fewer than two open. Deliberately no visible control (D5's own
non-goal: "a tab bar for documents, if a simpler switcher reaches the same place") --
`control_coverage`'s own `KeyboardOnly` entry cites this directly, `Permanent`, not a tracked
gap awaiting one.

Live capture: `rfcs/handoffs/065-multi-document/evidence/pr-065-c/` (committed) -- three real
documents opened, each given a distinct cursor position with no edit, then three real
`Ctrl+Alt+F` presses showing the full cycle (third → first → second → third), each document's
own cursor read back exactly on return. See its own `README.md` for the full sequence and one
disclosed observation (the explorer's own keyboard highlight does not follow the switch --
consistent with the pre-existing separation between "the keyboard highlight" and "the open
file", not a defect).

### Switching restores each document's own cursor and viewport -- asserted, not assumed

`cycling_the_active_document_wraps_and_restores_cursor_and_viewport`
(`crates/tekstide-core/src/project/tests/content.rs`): three documents, cursor and viewport set
on two before a third is opened; cycling wraps through all three and each document's own cursor
and viewport read back exactly, never touched by the cycle itself (both live on `TextDocument`,
restored for free by construction). Also asserts the no-op case directly: cycling with zero or
one document open changes nothing and does not panic.

`ctrl_alt_f_cycles_to_the_next_open_document_wrapping` and `ctrl_alt_f_is_a_no_op_with_fewer_
than_two_documents_open` (`crates/tekstide/src/shell/tests.rs`): the same two properties proven
through real key routing (`NavigationAction::SwitchActiveDocument` dispatched exactly as a real
`Ctrl+Alt+F` press would be), not just at the core level.

### Any new sidebar text fits 32 columns -- not applicable

No new sidebar text was added. The switcher is keyboard-only by design (the non-goal above), so
there is no new visible surface to measure against the 32-column bound RFC-055 set; the
existing `[open]` tag (PR-065-B) and row text are unchanged by this slice.

### Chord-count bookkeeping

Every place that counts the keybinding policy's own chords updated: `navigation/tests.rs`'s
`every_advertised_chord_round_trips` (eighteen chords held → nineteen) and
`advertised_bindings_are_exactly_the_live_ones` (the ordered list, `Ctrl+Alt+F` appended);
`keyboard_help/tests.rs`'s `every_live_binding_is_described_to_the_user` and `shell/tests.rs`'s
`opening_help_through_a_real_key_event_shows_every_live_binding` (seventeen → eighteen);
`rfc_docs_invariants.rs`'s `the_configuration_page_lists_every_rebindable_action_with_its_
default_chord` (seventeen → eighteen); `docs/src/users/configuration.md` and
`docs/src/users/keyboard-reference.md` both gain a row and their own wording update
("seventeen... eighteen" → "eighteen... nineteen"). A new collision-check test,
`switch_active_document_shortcut_is_a_candidate_that_collides_with_no_other_rule`
(`navigation/tests.rs`), proves `Ctrl+Alt+F` collides with nothing, mechanically, not by
inspection -- the same discipline every other chord in this table already has.

## Gate, PR-065-C

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace --no-fail-fast`: `736 + 16 + 1085` (+ `0+1+1` doctests), 0 failed.
- Commits pushed once this gate was green.

## Review 472: three required fixes, none in the switcher's own code

The switcher itself was accepted outright, independently reproduced (gate figures matched
exactly; the collision test and the live captures were checked directly, not taken from the
report). Three documentation gaps were required before the candidate, two of them repairs to
PR-065-B's own aftermath that PR-065-C inherited:

1. **`CHANGELOG.md`'s `## 0.30.0` status line** said the switcher was not done while both C
   commits were already in the tree -- the first slice to close after review 471's own
   "write the changelog incrementally" ruling closed without doing so. Fixed: the status line
   now names A, B and C done; a new paragraph describes the switcher and, in the same place,
   states that `[open]` widened its meaning.
2. **`[open]`'s meaning widened in PR-065-B** (one file, introduced `bd88978`, to the whole open
   set) **and nothing that described it was updated** -- found and fixed in three places, not
   the two review 472 named: `what-works-today.md`'s own `[open]` sentence, the same page's
   watcher-scope sentence (the identical staleness: "the folder holding the open file" was still
   singular), and `surface::explorer`'s own `node_line_with` doc comment. Also corrected
   `keyboard-reference.md`'s `Ctrl+S`/`Ctrl+Z` rows, where "the open file" had become ambiguous
   (now "the active document").
3. **The book did not say how a user tells which open document is active.** Added directly:
   "the editor's own header, above the cursor line, is what names which open document is
   active," in a new `what-works-today.md` paragraph describing the open set as a whole (the
   bound, the switcher, `[open]`, and this). The page's "Not built" section also still claimed
   "no multi-document editing" -- corrected to the real remaining gap, save-all, while in the
   area.

## Gate, review 472's fixes

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace --no-fail-fast`: `736 + 16 + 1085` (+ `0+1+1` doctests), 0 failed.
- Commits pushed once this gate was green.

## Review 473: the fix ran one direction only, and created a new contradiction

Review 472's fix corrected the *book* (`docs/`) but left the *program's own words* (`en.ftl`,
which `keyboard_help.rs` turns into the Help modal and `tekstide --help`) saying "the open
file" -- a contradiction that did not exist before `5c20605`: book and program previously
agreed, stale together. `0.28.0`'s own defect shape, inverted.

Fixed, all four items: `en.ftl`'s `keyboard-help-save-active-document`/`-undo-active-document`/
`-redo-active-document` ("the open file" / "needs an open file" → "the active document" /
"needs an active document"); the `en.ftl` comment above the `[open]` string (singular →
set membership, matching `surface/explorer.rs`'s own fix); `surface/explorer/tests.rs`'s own
doc comment (the exact sentence my own evidence `README.md` had cited as authority -- that
citation is corrected too, now disclosing its own source's staleness rather than silently
standing as if still accurate); and `keyboard-reference.md`'s "edits the open document" nit.

**Then ran the review's own named method** (a repo-wide grep for the phrase) before declaring
this done, rather than waiting for a fifth round to find the next spot -- found one more:
`ProjectContentStatus::ExternalDeleted`'s own doc comment in `content.rs`, "the open file was
deleted," tightened to "the active document's file" (this status is deliberately
active-document-scoped, PR-065-B's own design, so "open" was ambiguous there too).

## Gate, review 473's fixes

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace --no-fail-fast`: `736 + 16 + 1085` (+ `0+1+1` doctests), 0 failed.
- Commits pushed once this gate was green.

## Review 474: one more documentation item, reachable only once C shipped

`what-works-today.md`'s own "the open file's own header names a change or a deletion" was still
singular. With the open set, each document carries its own external-change state
(`a_background_document_is_refreshed_by_its_own_path_without_touching_the_active_status`,
PR-065-B) and the header shows only the *active* one's -- so a background document's own disk
change is real and detected, but invisible until the user switches to it. Not reachable before
PR-065-C: before a switcher existed, there was no way to *reach* a second document at all, so
the question did not arise before this slice.

Fixed: the paragraph now says the header names the active document's state, that a background
document's own change is detected the same way, and that it surfaces on switching, not before. A
repo-wide sweep (now the standing habit after reviews 472-474) found nothing else user-facing;
the one remaining hit for the same phrase, `explorer/tests.rs:1169`, describes that test's own
single-file fixture correctly and is not stale.

The flake (`is_still_answerable_reflects_the_real_connection_state`, row 5) has its own
disposition now, recorded by the architect at `test-process-leak.md`'s own "Disposition,
2026-10-07" entry: ruled, scheduled before the candidate, not mine to action this response.

## Gate, review 474's fix

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace --no-fail-fast`: `736 + 16 + 1085` (+ `0+1+1` doctests), 0 failed.
- Commits pushed once this gate was green.

## PR-065-D — save-all

### A partial save-all says which files were written and which were not

`ProjectContentWorkspace::save_all_documents` attempts every open document regardless of an
earlier one's own failure, each through the identical `document.save(root, policy)`
temp-and-rename path `save_active_document` already uses (`status_for_save_result` is the
shared mapping factored out so both compute the same `ProjectContentStatus` for whichever
entry is active). `SaveAllOutcome`/`DocumentSaveOutcome` name which documents were written and
which were not, per-path.

`saving_all_documents_writes_every_one_through_the_real_path` and `a_partial_save_all_reports_
which_documents_were_written_and_which_were_not`
(`crates/tekstide-core/src/project/tests/content.rs`): three real documents, the middle one
blocked by a real external deletion, `save_all_documents` called once -- the outcome correctly
splits written/not-written, and the two unblocked documents' own edits are verified **on
disk**, not only in the in-memory result, proving the blocked one costs nothing to the other
two (attempted in order, one after the blocked one).

`save_all_notice_lines_summarizes_and_names_every_document_not_written`
(`crates/tekstide/src/surface/editor/tests.rs`): the chrome's own message against a *real*
blocked save (a file deleted out from under an open document, not a hand-built error) --
proves the summary line, the one row for the document not written, and that its path is
escaped the same way `chrome_line`'s own path already is (a bidi-override character in the
fixture's own filename, checked both ways: the escaped marker present, the raw character
absent).

`ctrl_shift_s_saves_every_open_document_through_real_routing`
(`crates/tekstide/src/shell/tests.rs`, real key routing): two documents edited, only one
active, a real `Ctrl+Shift+S` press writes both to disk and stores a notice with the real
counts -- the end-to-end path, not only the core-level pieces.

**Not captured live.** Unlike PR-065-C's own checklist item, PR-065-D's acceptance criteria
do not say "captured live" (re-checked against the RFC's own wording); the four tests above
already demonstrate the real behavior end to end (real files on disk, a real blocked save, real
key routing), so a live GUI capture was judged not to add further evidence proportionate to its
own cost. Flagged as a scope decision, not an oversight, in case the reviewer wants one anyway.

### Each save is the existing temp-and-rename path; N saves cost N watcher notices, and the changelog says so

**Structural claim, not a new measurement.** `save_all_documents`'s own per-document call is
`document.save(root, policy)` -- the identical method `save_active_document` already calls for
a single save, unchanged. RFC-026 already measured what one call through this path costs the
watcher (one scan, one re-read of the file just written, `0.29.0`'s own changelog entry); N
calls to the same method in one `save_all_documents` run is N of that already-measured cost,
not a new combinatorial effect needing its own fresh measurement the way D7's per-document
*refresh* cost did (that one was genuinely new: N documents' refresh in one drain window, a
real interaction the single-document model never had). `CHANGELOG.md`'s own `## 0.30.0` entry
states this directly.

A dedicated real-kernel-watcher test proving N saves produce N *observed* notices (as opposed
to N calls to the known-costly method) was considered and not written: the project's own
existing tests for "a real external change reaches an open document" use synthetic notice
injection (`record_project_watch_notice` called directly) rather than a real inotify wait,
precisely to avoid the kernel-timing flakiness this session's own flake-register work (rows 2
and 5, fixed and disposed 2026-10-07) just finished diagnosing and fixing elsewhere in this same
codebase. Building a new real-timing test for this claim risked reintroducing exactly that
class of flake for a claim that is already structurally true by construction (same method, same
call), not speculative.

## Gate, PR-065-D

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace --no-fail-fast`: `738 + 16 + 1088` (+ `0+1+1` doctests), 0 failed.
- Commits pushed once this gate was green.

## Review 477: the watcher-notice claim was false, and the naming is what hid it

Independently verified before touching anything: `TextDocument::save` (`content/document.rs`)
returns `Ok(SaveDecision::Saved)` from an `if !self.is_dirty()` early return, *before*
`write_text_via_temp_rename` is ever reached. The architect measured this directly (two open
documents, one edited, one untouched: `written_count() == 2`, untouched file's mtime
unchanged) rather than inferring it -- the review's own finding reproduces exactly on reading
the function.

**Fix 1: the false claim, in both places it was written.** `CHANGELOG.md`'s `## 0.30.0` save-all
paragraph claimed "an N-document save-all costs the watcher the same N scans and N re-reads of
the files just written that N separate single saves always would have." Rewritten: the cost is
one scan and one re-read per document that was *dirty* when the action was pressed, not per
document attempted -- a mostly-clean save-all (one edited document among many open) costs the
watcher the same single notice one `Ctrl+S` on that one document already would have. The
PR-065-D checklist box asserting the old claim is corrected to state the dirty-only cost
directly, citing the new clean-document test (fix 3) as its evidence.

**Fix 2: `was_written`/`written_count`/`all_written` renamed, `SaveDecision` left alone.** All
three were `matches!(result, Ok(SaveDecision::Saved))`, true for a document nothing touched --
the word "written" is what made fix 1's claim look true when it was written down. Renamed to
`succeeded`/`succeeded_count`/`all_succeeded` (`crates/tekstide-core/src/project/content.rs`),
with each accessor's own doc comment now stating directly that `Ok(Saved)` is not proof of a
write and citing this review by number. `SaveDecision` itself is unchanged, per the review's own
instruction -- the user does not need "1 written, 4 already saved," only a name that does not
imply a write that did not happen. Call sites updated: `editor.rs`'s `save_all_notice_lines`
(plus its own doc comment), the Fluent key `editor-save-all-summary`'s `$written` ->
`$succeeded` (`en.ftl`), the i18n completeness registration in `enforcement.rs`'s
`generic_args()`, and the two renamed-method call sites each in
`crates/tekstide-core/src/project/tests/content.rs` and
`crates/tekstide/src/shell/tests.rs`. The rendered English text itself is unchanged ("Save all:
N of M saved") -- only the internal identifier and Fluent variable name changed, consistent with
the review's own ruling that the user-facing "saved" wording is defensible.

**Fix 3: a test that leaves a document clean.**
`a_clean_document_in_the_open_set_succeeds_without_being_rewritten`
(`crates/tekstide-core/src/project/tests/content.rs`): two documents, one edited, one never
touched after opening; `save_all_documents` called once. Asserts `all_succeeded()` (true -- the
clean document's own `save()` still returns `Ok(Saved)`), `succeeded_count() == 2`, the clean
document's own file **mtime is unchanged** (`std::fs::metadata(...).modified()`, compared before
and after, the same technique `a_found_transcript_takes_its_age_from_its_mtime` already uses in
`project/tests/loading.rs`), and its content on disk is still the original text -- the
deterministic, no-timing test the review said would have caught this, in contrast to the
real-kernel notice-count test that was rightly declined.

**Fix 4: a live capture, specifically the misleading case.** `evidence/pr-065-d/` (new): a
throwaway `/dev/shm` fixture, two documents opened, only the first edited, `Ctrl+Shift+S`
pressed once. The notice reads **"Save all: 2 of 2 saved"** with the second document never
touched -- captured on screen, then proved false-looking-but-true on disk: `stat` mtimes taken
immediately before and after show the first document's mtime advancing and the second's
byte-identical, with the second file's content on disk unchanged. This is the first capture in
RFC-065 to show the "Save All" button and the save-all notice at all, and the first to show the
exact case the review named. Full sequence and the on-disk proof are in that folder's own
`README.md`.

**Scope decision 2 (declining a real-kernel notice-count test), accepted in part, nothing
further required.** The review's own ruling was that declining a *timing* test was right, and
the error was concluding no test at all was needed; fix 3 is the deterministic test that closes
that gap. No new real-kernel test was added, consistent with the review's own ruling that one
was not being asked for.

## Gate, review 477's fixes

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**:
  `738 + 16 + 1089` (+ `0+1+1` doctests), 0 failed, 0 fixture entries left in each run's own
  `TMPDIR` afterward.
- Commits pushed once this gate was green.
