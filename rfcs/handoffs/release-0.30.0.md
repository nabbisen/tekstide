---
title: "Release 0.30.0: the multi-document model"
status: "**Published 2026-10-08.** Authorised by the owner; published, tagged and verified by the architect. RFC-065 was closed before the publish, in the candidate commit, not after it."
rfc_file: "../done/065-the-multi-document-model.md"
target_milestone: "M13"
created: "2026-10-08"
---

# Release 0.30.0

## What this release is

**A repair, and the model that makes the repair possible.** `0.29.0` shipped a live data-loss
defect: opening a second file silently discarded the first's unsaved edits — no prompt, no refusal,
no record. RFC-065's first slice fixed it by giving the editor an **open set** rather than one
document, and the rest of the RFC exists to make that set usable: dedup by path, a bound of twenty,
a keyboard switcher (`Ctrl+Alt+F`), and save-all (`Ctrl+Shift+S`).

Everything else in the release follows from the set existing. `REQ-EDIT-004` has said "dirty
*buffers*", plural, since it was written, and the product had only ever had one; the counts are a
real plural for the first time.

## What the review sequence actually cost, and why it was worth it

Four slices took **thirteen review rounds** (468–480). The implementation was rarely the problem.
Nine of the required items were **descriptions that stopped being true** when behaviour widened
underneath them — the `[open]` marker alone took four rounds, because it shipped meaning *the* open
file and became set membership without a single sentence being updated.

Two lessons were general enough to be written into `release-checklist.md`:

- **A word that widens leaves no trace in a changelog's Added section** — only in the diff. The
  book-against-changelog step now runs in both directions.
- **A one-directional fix makes it worse.** Correcting the book while leaving `locales/en.ftl`
  saying "Save the open file" turned two agreeing stale sentences into a contradiction. Fix both
  sides in one commit or neither.

## What was found that the tests did not

- **A clean document is never written.** `save()` returns `Ok(SaveDecision::Saved)` from an early
  `is_dirty` return, so "N saves cost N watcher notices" — stated in the changelog and ticked on the
  checklist — was false: the cost is one notice per *dirty* document. Both save-all tests edited
  every document first, so nothing exercised it. Found by reading the code, confirmed by measuring
  an unchanged mtime, and then shown on screen in PR-065-D's own capture: *"Save all: 2 of 2 saved"*
  beside a file that was never touched.
- **`was_written` did not mean written.** Renamed to `succeeded`; `SaveDecision` deliberately not
  widened.

## The gate, and the publish

Reproduced independently at the candidate and again at the published commit: `738 + 16 + 1089`
(+ `0+1+1` doctests), 0 failures, **0 fixture entries left** under a fresh short `TMPDIR`.
`cargo package --workspace` verified both crates, and the packaged app archive's own `Cargo.lock`
names `tekstide-core 0.30.0`.

Published with `cargo publish --workspace` (core first), then tagged. **The tag and the publish are
the same commit, `c1435f4`** — `.cargo_vcs_info.json`'s `sha1` matches the tag exactly, so the
known provenance-drift case in the checklist did not arise this time. `post-publish-check.sh`
passes for **`0.30.0` and `0.29.0`**; `LICENSE` and `NOTICE` are in both published archives, and
`NOTICE`'s hardcoded `rusqlite 0.40.2 and libsqlite3-sys 0.38.2` match the published lockfile.

## Left open, deliberately

- **Two terminal PTY tests are registered intermittents**, diagnosed but not yet fixed
  (`test-process-leak.md`, "New rows, 2026-10-08"): the wait is weaker than the parse, and they
  leak their fixture on panic because cleanup is a call rather than a drop guard. **They fail the
  gate twice when they fail.** Not a release blocker — a passing run leaves nothing behind.
- **No `REQ-` names multi-document.** Disclosed, not minted, in `delivery-plan.md`.
- **Nothing marks which open document is active in the sidebar.** The editor's header does, and the
  book now says so. A deliberate separation, not a gap.
