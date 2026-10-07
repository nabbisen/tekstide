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

- [ ] `active` keeps its meaning: one active document, a plural open set.
- [ ] **`open_buffer_count` and `dirty_file_count` count the whole set.** A test fails when either
      returns one while two are open.
- [ ] The close dialog counts the set — **by test, not by a second code path** (D10: `session.rs:1706`
      already feeds it from `dirty_file_count`).
- [ ] The bound is stated when reached.
- [ ] The watcher's scope follows the set: opening and closing documents changes the watched count,
      counted before and after.
- [ ] **The per-document refresh measured on RFC-026's harness unchanged**, N documents against one,
      with the control in the same run.
- [ ] `REQ-EDIT-004` met **for its own plural**, and the coverage row corrected from implying it
      already was.

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
