---
title: "RFC-065 — acceptance and QA checklist"
rfc: "RFC-065"
rfc_file: "../../accepted/065-the-multi-document-model.md"
source_rfc_status: "Accepted 2026-10-07 — M13"
target_milestone: "M13"
created: "2026-10-07"
---

# Acceptance and QA checklist

Tick a box when the evidence is in `qa-evidence.md`, not when the code looks right. A box naming a
measurement with no number in the evidence is not ticked.

## PR-065-A — the repair

- [x] **The test was written first and shown failing against today's code**, with the failure in the
      evidence. A repair whose test never saw the defect is a claim.
- [x] Editing a file and opening another leaves the first's text **and** dirty state intact.
- [x] Captured live: edits made, a second file opened, the first still there.
- [x] The undo history of the first document survives, or the product says it did not.
- [x] Nothing else in this slice.

## PR-065-B — the set

- [x] `active` keeps its meaning: one active document, a plural open set.
- [x] **`open_buffer_count` and `dirty_file_count` count the whole set.** A test fails when either
      returns one while two are open. (Landed in slice A; re-confirmed here since B is this
      criterion's own slice.)
- [x] The close dialog counts the set — **by test, not by a second code path** (D10: `session.rs:1706`
      already feeds it from `dirty_file_count`).
- [x] The bound is stated when reached.
- [x] The watcher's scope follows the set: opening and closing documents changes the watched count,
      counted before and after.
- [x] **The per-document refresh measured on RFC-026's harness unchanged**, N documents against one,
      with the control in the same run.
- [x] `REQ-EDIT-004` met **for its own plural**, and the coverage row corrected from implying it
      already was.

### Required at review 468

- [x] **Opening a path that is already open switches to the existing entry** rather than adding a
      second. Today two entries for one path means two documents that both believe they own the file;
      `save_active_document` saves the active one, so the older entry is unreachable — **until slice C
      adds a switcher, at which point saving both in either order silently overwrites one with the
      other.** A lost update the product creates against itself. Lands in **B**, not C: a fix in the
      same slice as the hazard is a race with review.
- [x] **The explorer's `[open]` tag marks set membership, not just the active document.** Correctly
      out of scope for A under D2; in scope here, as the visible counterpart to the counts.
- [x] When B changes the reopening behaviour, `reopening_an_already_open_path_adds_a_second_entry...`
      is **renamed to say what it now holds**, not deleted — the record that this was once true is
      worth keeping.

- [x] **The N-document ratio is computed on delivery work, not on keystroke p95.** Fixed:
      `delivery_ms[round][condition]` parallels `p95`, recording `run.delivery` for every condition
      including `BurstWithNDocuments`. Real numbers, release mode, with
      `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true`: median D8 delivery cost **+4.244 ms**, median
      10-document delivery cost **+37.294 ms**, **ratio 8.79×** — near the 10× measurement 9 assumed.
      Confirms the assumption. The p95 ratio (0.63×) is also still reported, labeled for what it
      actually answers ("does holding N documents slow typing", not D7's own question).
- [x] *(Unblocked at 469:* the release-mode build needs `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true`,
      the setting this project's own review-462 evidence records. 57 errors without it, clean with it.
      Not an `iced` mismatch.*)*

## PR-065-C — reaching the second document

- [x] A switcher, **keyboard-first, reachable, captured live**, no environment variable.
      `Ctrl+Alt+F`, cycling the open set, wrapping — `evidence/pr-065-c/`.
- [x] Switching restores each document's own cursor and viewport — asserted, not assumed.
      Core-level and shell-level (real routing) tests, plus the live capture.
- [x] **Not applicable: no new sidebar text.** The switcher is keyboard-only by design (the
      RFC's own non-goal), so there is no new visible surface to measure against the 32-column
      bound.

### Required at review 472

- [x] **The changelog's `## 0.30.0` status line still says the switcher is not done.** Fixed:
  the status line now names A, B and C as done and only D as not; a new paragraph describes the
  switcher (`Ctrl+Alt+F`, wrapping, cursor/viewport restored) and states the `[open]` tag's
  widened meaning directly.
- [x] **`[open]` changed meaning in PR-065-B and nothing that describes it was updated.** Fixed
  in three places, not two — found while fixing the first two that the watcher's own description
  had the same staleness: `docs/src/users/what-works-today.md` (the `[open]` sentence, corrected
  to set membership, plus the watcher-scope sentence, corrected from "the folder holding the open
  file" to "the folder of every open document"), `crates/tekstide/src/surface/explorer.rs`'s own
  doc comment on `node_line_with`, and `CHANGELOG.md`'s new paragraph. Also corrected
  `keyboard-reference.md`'s `Ctrl+S`/`Ctrl+Z` rows from "the open file" (now ambiguous) to "the
  active document".
- [x] **The book must say how a user tells which open document is active.** Added to
  `what-works-today.md`'s new "Multiple documents" paragraph: "the editor's own header, above
  the cursor line, is what names which open document is active" — bolded, the same sentence the
  live capture demonstrates. The "Not built" section's stale "no multi-document editing" claim
  was also corrected (to the real remaining gap, save-all) while in the area.

### Required at review 473

The three fixes are correct and the third place you found on your own (the watcher-scope sentence)
was not in my list. But the fix went one direction only — the book — and the program's own words
were left behind. A repo-wide grep for the phrase finds them; neither of us ran one at 472.

- [x] **The Help modal and `tekstide --help` still say "the open file".** Fixed: `en.ftl`'s
  `keyboard-help-save-active-document`/`-undo-active-document`/`-redo-active-document` now say
  "the active document"/"an active document", matching `keyboard-reference.md`'s own wording.
- [x] **`crates/tekstide/locales/en.ftl:581`**, the comment above the `[open]` string, corrected
  to set membership, matching `surface/explorer.rs`'s own fix.
- [x] **`crates/tekstide/src/surface/explorer/tests.rs:1124-1125`** corrected. The evidence
  `README.md`'s own citation of this sentence as authority was also corrected (not re-cited as
  if still accurate; the conclusion it supported is noted as holding independently of it, and
  the citation's own staleness is now disclosed in place rather than silently left standing).
- [x] *(nit)* `docs/src/users/keyboard-reference.md:47` now says "the active document".
- [x] **A repo-wide grep for the phrase** (the review's own named method, run this time) found
  one more: `crates/tekstide-core/src/project/content.rs`'s own `ExternalDeleted` doc comment
  said "the open file was deleted" — tightened to "the active document's file", since
  `ProjectContentStatus` is deliberately active-document-scoped (PR-065-B's own design) and
  "open" was ambiguous there too.

### Required at review 474

The four fixes are correct, the contradiction `5c20605` created is closed, and running the sweep
yourself before calling it done — finding `ExternalDeleted`'s doc comment — is the habit the last
two reviews were asking for. My own sweep now returns nothing user-facing but one item.

- [x] **`docs/src/users/what-works-today.md:20` still describes the status in the singular**: fixed.
  The paragraph now says the header names the *active* document's own state, that a background
  document's disk change is detected the same way (every open document is watched) but surfaces
  only on switching to it, and that the header — not a sidebar marker — is where it shows. A
  repo-wide sweep for the same phrase found nothing else user-facing (one remaining hit,
  `explorer/tests.rs:1169`, is accurate for its own single-file test fixture, not stale).

**Not an item for this slice, ruled separately:** `is_still_answerable_reflects_the_real_connection_state`
is no longer to be recorded and redone. It has flaked since 2026-08-25, twice on 2026-10-07 alone,
both times on documentation-only diffs. I have confirmed the cause in the test and ruled it a test
defect with a fix that retires two register rows at once; see **Disposition, 2026-10-07** at the end
of `rfcs/handoffs/test-process-leak.md`. Scheduled before the `0.30.0` candidate, test-only.

## PR-065-D — save-all

- [x] **A partial save-all says which files were written and which were not.** Proved on real
      files on disk, one document blocked by a real external deletion, at both the core and
      shell levels (real key routing) -- `qa-evidence.md`.
- [x] Each save is the existing temp-and-rename path, but only for a *dirty* document: `save()`
      returns `Ok(SaveDecision::Saved)` for a clean one from an early return before
      `write_text_via_temp_rename` is ever reached, so a save-all over N open documents costs the
      watcher one notice per dirty document actually written, not N -- review 477's own finding,
      measured directly rather than inferred. `CHANGELOG.md`'s `## 0.30.0` entry is corrected to say
      so, and a core-level test (`a_clean_document_in_the_open_set_succeeds_without_being_rewritten`)
      proves it on a real file's mtime; reasoning in `qa-evidence.md`.

- [x] **The changelog carries the per-document refresh figure** (review 470): `CHANGELOG.md` now has
      a `## 0.30.0` entry, `Status: in progress`, answering `0.29.0`'s own open question directly —
      about **+4.2 ms** for one document and **+38.9 ms** for ten per burst window, between keystrokes,
      a **9.2×** ratio against the 10× a linear cost predicts, extrapolating to roughly **78 ms** at
      D4's bound of twenty. Written incrementally as slices close, not held back for the release.

### Required at review 477

Save-all's write path, its partial-failure reporting, the chord and the bookkeeping are all correct,
and the gate reproduces on my own run (`738 + 16 + 1088`). But the one claim the slice chose *not*
to test is false, and it is stated in three places.

- [x] **"N saves cost N watcher notices" is wrong, and the changelog says it.** A clean document is
  never written: `content/document.rs`'s `save` returns `Ok(SaveDecision::Saved)` from an early
  `if !self.is_dirty()` return, *before* `write_text_via_temp_rename`. I measured it — two open
  documents, one edited, one untouched: `written_count()` reported **2**, and the untouched file's
  **mtime did not change**. So a save-all over N open documents costs the watcher one notice per
  **dirty** document, not N. At D4's bound of twenty open with one edited, that is 1, not 20. The
  *relative* half of the changelog sentence is right (it is the same as N separate `Ctrl+S`
  presses); the absolute "N scans and N re-reads of the files just written" is not. Fix the
  sentence in `CHANGELOG.md`, and untick the PR-065-D box that asserts it.
- [x] **`was_written`, `written_count` and `all_written` do not mean written.** All three rest on
  `matches!(self.result, Ok(SaveDecision::Saved))`, which is true for a document that was never
  touched. The user-facing string is defensible — a clean document *is* saved — but the identifiers
  and the changelog both say *written*, and that is the word that made the claim above look true.
  **Recommended: rename the three accessors; do not widen `SaveDecision`** for this. The enum is
  shared with the single-save path and the user does not need "1 written, 4 already saved"; what
  the user needs is that the summary is not read as a count of files touched.
- [x] **No test puts a clean document in the open set.** Both core tests edit every document before
  saving, which is why this survived. Add one that leaves a document clean and asserts the file is
  not rewritten — mtime, as above, is enough — and that the reporting about it is truthful.
- [x] **A live capture is required after all.** Your reading of the wording is accurate: PR-065-D's
  boxes do not say "captured live" the way PR-065-C's did, so this is me adding a requirement, not
  you missing one. Two reasons it is not ceremony here. This slice ships **the first visible control
  in RFC-065** — a "Save All" button and a multi-line notice — and no capture has ever shown either.
  And a capture of a save-all with one clean document in the set would have put *"2 of 2 saved"* on
  the screen beside a file that was never written, which is how the item above should have been
  found. Capture that case specifically.

**Scope decision 2, accepted in part.** Declining a real-kernel *timing* test was right, and citing
the flake work is a fair reason. The error was concluding that therefore no test was needed: what is
wrong here is a deterministic logic case that a plain test catches with no timing at all.

## Whole-RFC

- [x] The requirements gap — **no `REQ-` names multi-document** — is written up for the owner, and no
      `REQ-` is minted. `rfcs/delivery-plan.md`, "Requirements gap: no `REQ-` names multi-document"
      (2026-10-08, RFC-065 D8), the same disclosed-not-minted treatment undo got at RFC-057 D4.
- [x] The colour-alone, i18n completeness and internal-identifier scans still pass. RFC-065 adds no
      new colour-coded state (the `[open]` tag and the save-all notice are both text, not colour);
      `no_catalog_string_names_an_internal_identifier` and
      `every_source_locale_key_resolves_in_every_shipped_locale`
      (`crates/tekstide/src/i18n/enforcement.rs`) pass directly, part of the three-run gate below.
- [x] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, **`cargo test --doc --workspace`** (the step `0.29.0` added
      after `--all-targets` was found never to run the doctest guard), **three consecutive
      full-workspace runs with `--no-fail-fast`**, **0 fixture entries left** in a fresh short
      `TMPDIR` — a short fixed literal, not `mktemp`. All clean: `738 + 16 + 1089` (+ `0+1+1`
      doctests), every run, `.git-exclude/tmp/479-gate-{1,2,3}.log`.
- [x] Every new intermittent failure has a dated row in `test-process-leak.md`. None from these three
      runs (all clean); the two review 478 found in its own run are already registered there ("New
      rows, 2026-10-08 — review 478", `a79d808`), not mine to add a second time.
- [ ] The core pin bumps with the version. Not yet: `0.30.0` has not been cut as a release candidate
      (workspace version is still `0.29.0`), so there is no new version for the pin to bump to. A
      release-step item, left for the candidate itself.
- [x] Commits are pushed once the gate is green. Pushed at `710f6dd`.

### Required at the candidate (review 471)

- [x] **Re-read `## 0.30.0` against what actually shipped.** It was written incrementally — which is
      the better habit, because the numbers are in hand and the limitations are described by whoever
      just met them — but it was written **before slices C and D existed**. A section that was accurate
      when written can stop being accurate without anyone touching it. That is the cost of writing
      early, and it is smaller than the one it avoids. **Found stale, now fixed**: the entry covered
      D7's refresh measurement (B), the switcher (C) and save-all (D), but never slice A at all — the
      RFC's own headline, "opening a second file with unsaved edits in the first loses nothing" — nor
      B's own dedup/counts/bound/`REQ-EDIT-004` correction beyond one passing mention of the bound.
      Both are now their own paragraph, first in the section since the repair is the release's
      headline per the RFC's own acceptance criteria. The status line is also corrected: it said the
      requirements-gap write-up was not done, which was true when that sentence was written but not
      by the time anyone would read it this response.

### Required at review 479

The requirements-gap write-up, the scan reasoning and the gate are all accepted, and the re-read
genuinely found what it was meant to find — the entry had never named slice A. Three items remain,
two of them things the re-read should have caught on the same pass.

- [x] **`rfcs/delivery-plan.md:30` now contradicts itself.** `7d01570` appended `REQ-EDIT-004`'s
  correction to the Text-document coverage row — correctly — but the row still *opens* with
  "Model complete, **single active document**." The same cell now says both that there is one
  active document and that the open set is real and counts every one of them. This is the shape
  caught at review 472 with `[open]`: the correction gets appended and the summary the correction
  falsifies is left standing. The lead phrase must change. A repo-wide sweep for the claim finds
  only this and `release-0.29.0.md:49`, which is a frozen record and correct for its own release.
  Fixed: the lead now reads "Model complete, **an open set of documents with one active**", with a
  pointer to the `004` correction in the same cell rather than restating it.
- [x] **The entry never mentions undo — zero occurrences.** The release's headline is that opening
  a second file no longer discards the first's unsaved edits, and slice A's own test is
  `opening_a_second_file_leaves_the_first_s_undo_history_intact`. Undo and redo histories are
  per-document (`undo_stack`/`redo_stack` are fields of the document) and survive both the open and
  the switch. "Nothing is discarded" includes the undo history, and a user who lived with `0.29.0`
  has every reason to ask. One clause in the repair paragraph. Fixed: `CHANGELOG.md`'s repair
  paragraph now says the undo history survives too, since it lives on the document itself, and that
  the switcher can never undo into the wrong file.
- [x] **The entry never says every open document is watched.** Its only watcher mentions are
  save-all's cost. `what-works-today.md` says it — required at review 474 — that a background
  document's own disk change is detected the same way and is waiting when you switch to it. That is
  new, user-visible behaviour of the multi-document model with no line in the release that ships it.
  Fixed: one sentence added to the repair paragraph, naming that every open document is watched, not
  only the active one, and that a background change is already waiting, named, when you switch to it.

**The core pin and the version bump: your reading is right, leave them.** Cutting the candidate is a
separate step and the box's own wording ("bumps *with the version*") makes it a release-cut item.
It stays unticked with the reason you gave.

**Verified independently at this review**: the gate reproduces (`738 + 16 + 1089`, 0 failures); the
`REQ-EDIT-004` coverage correction the changelog claims really is in the tree at `7d01570`; undo and
redo are per-document by construction, so the switcher cannot undo into the wrong file.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
