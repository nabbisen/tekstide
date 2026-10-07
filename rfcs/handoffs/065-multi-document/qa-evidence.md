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
