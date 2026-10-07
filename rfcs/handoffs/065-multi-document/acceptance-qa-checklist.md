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
- [ ] **The per-document refresh measured on RFC-026's harness unchanged**, N documents against one,
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

### Required at review 469

- [ ] **The N-document ratio is computed on delivery work, not on keystroke p95.** Measured at 469:
      the ratio reads **1.11×**, but it is taken on p95 keystroke latency, and the refresh runs
      *between* keystrokes where that figure cannot see it. The harness already prints the right
      quantity — *"1.3 ms of delivery between keystrokes"* before D8, *"5.4 ms"* with it — and
      excludes `BurstWithNDocuments` from that line (`editor_baseline.rs:945–947`). Report it, and
      compute the ratio on it. **If it shows ~1× rather than ~10×, measurement 9 was wrong and that is
      worth more than the box.**
- [ ] *(Unblocked at 469:* the release-mode build needs `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true`,
      the setting this project's own review-462 evidence records. 57 errors without it, clean with it.
      Not an `iced` mismatch.*)*

## PR-065-C — reaching the second document

- [ ] A switcher, **keyboard-first, reachable, captured live**, no environment variable.
- [ ] Switching restores each document's own cursor and viewport — asserted, not assumed.
- [ ] Any new sidebar text fits 32 columns, like every other sentence there.

## PR-065-D — save-all

- [ ] **A partial save-all says which files were written and which were not.**
- [ ] Each save is the existing temp-and-rename path; N saves cost N watcher notices, and the
      changelog says so.

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

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
