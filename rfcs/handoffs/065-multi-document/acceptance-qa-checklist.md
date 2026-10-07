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

## PR-065-D — save-all

- [ ] **A partial save-all says which files were written and which were not.**
- [ ] Each save is the existing temp-and-rename path; N saves cost N watcher notices, and the
      changelog says so.

- [x] **The changelog carries the per-document refresh figure** (review 470): `CHANGELOG.md` now has
      a `## 0.30.0` entry, `Status: in progress`, answering `0.29.0`'s own open question directly —
      about **+4.2 ms** for one document and **+38.9 ms** for ten per burst window, between keystrokes,
      a **9.2×** ratio against the 10× a linear cost predicts, extrapolating to roughly **78 ms** at
      D4's bound of twenty. Written incrementally as slices close, not held back for the release.

## Whole-RFC

- [ ] The requirements gap — **no `REQ-` names multi-document** — is written up for the owner, and no
      `REQ-` is minted.
- [ ] The colour-alone, i18n completeness and internal-identifier scans still pass.
- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, **`cargo test --doc --workspace`** (the step `0.29.0` added
      after `--all-targets` was found never to run the doctest guard), **three consecutive
      full-workspace runs with `--no-fail-fast`**, **0 fixture entries left** in a fresh short
      `TMPDIR` — a short fixed literal, not `mktemp`.
- [ ] Every new intermittent failure has a dated row in `test-process-leak.md`.
- [ ] The core pin bumps with the version.
- [ ] Commits are pushed once the gate is green.

### Required at the candidate (review 471)

- [ ] **Re-read `## 0.30.0` against what actually shipped.** It was written incrementally — which is
      the better habit, because the numbers are in hand and the limitations are described by whoever
      just met them — but it was written **before slices C and D existed**. A section that was accurate
      when written can stop being accurate without anyone touching it. That is the cost of writing
      early, and it is smaller than the one it avoids.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
