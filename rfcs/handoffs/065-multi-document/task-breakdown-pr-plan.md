---
title: "RFC-065 — task breakdown and PR plan"
rfc: "RFC-065"
rfc_file: "../../done/065-the-multi-document-model.md"
source_rfc_status: "Implemented and closed 2026-10-08 — 0.30.0 candidate, not yet published"
target_milestone: "M13"
created: "2026-10-07"
---

# Task breakdown and PR plan

Four slices. **A repairs something that is shipping** and goes first and alone.

## PR-065-A — opening a second file stops throwing the first away

- **Write the test first and show it red against today's code** (D12). Edit a file, open another,
  assert the first's text and dirty state survive. Today it fails; that failure is the evidence.
- **Prefer the set** — with an open set there is nothing to replace, and the defect is gone by
  construction. A refusal or a prompt is the interim and only if the set is not ready in this slice
  (risk §2: a prompt on every second file is worse than the defect for a user who is browsing).
- Captured live: edits made, a second file opened, the first still there with its edits.
- Nothing else in this slice. It is the release's headline and it should be reviewable in one sitting.

## PR-065-B — the set, the counts, the bound, the scope

- The open set, with **`active` keeping its meaning** (D2): one active document, a plural open set.
  Of 83 call sites, the two that must change are the counts.
- **`open_buffer_count` and `dirty_file_count` count the whole set** (D3). A test fails when either
  returns one while two are open. **The close dialog needs no separate change** (D10):
  `session.rs:1706` feeds it from `dirty_file_count`, so the dialog is a test, not a code path.
- **The bound** (D4), stated when reached — each document is up to 4 MiB plus two 500-deep undo
  stacks.
- **The watcher's scope follows the set.** `watch_inputs()` already returns
  `open_document_directories`, plural, fed by one; this is arithmetic. Counted before and after
  opening and closing documents.
- **The per-document refresh, measured** (D7, D13): N documents against one, on RFC-026's
  paired-control harness **unchanged** — it carries its control in the same run and took three
  attempts to get right. Add a condition; do not rebuild it.
- **`REQ-EDIT-004` moves to met for its own plural**, and the coverage row is corrected from implying
  it already was.

## PR-065-C — reaching the second document

- A switcher, **keyboard-first and reachable**, captured live with no environment variable (D5).
  RFC-021 is why: a model nobody can reach is not implemented, and counts describing a buffer the user
  cannot see are worse than no counts.
- **The chord space has room** (D11): eighteen are held, nearly all `Ctrl+Alt+<letter>`, and RFC-054's
  collision resolution refuses every participant on a clash. Adding one is a known operation.
- Per-document `cursor` and `viewport` already exist on `TextDocument`, so switching restores where the
  user was with no new storage — assert it rather than assume it.
- Mind the sidebar: it already spends four reserved lines at about 32 columns (risk §5).

## PR-065-D — save-all

- **New work, not a loop around the existing save** (D6). `save_active_document` saves *the active
  one*; saving several is a sequence of writes that can fail partway.
- **A partial save-all says which files were written and which were not.** That is the whole risk: a
  user who is told "saved" when three of five landed has lost the other two without knowing.
- Each save is still the existing temp-and-rename path, and each still causes the watcher's own notice
  (RFC-026 named it: a save costs a scan and a re-read of the file just written). N saves, N of those.

## Order, and why

A alone, because it repairs shipped data loss and should not wait behind the model. B before C because
a switcher needs a set to switch within. C before D because save-all is only meaningful once a user
can have several documents and see them. D last and separable — if the release is long, it is the
piece that can move.
