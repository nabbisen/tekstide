---
title: "RFC-036 acceptance and QA checklist"
rfc: "RFC-036"
rfc_file: "../../accepted/036-dormant-capability-closure.md"
source_rfc_status: "Accepted 2026-08-18, D0–D4 decided 2026-08-27 — M12"
target_milestone: "M12"
created: "2026-08-27"
---

# Acceptance and QA checklist

## D0 — the table is of the tree, not the document

- [ ] Every row re-verified against the current tree.
- [ ] `request_terminate` (3 production callers) and `shutdown` (1) are **not** listed as orphans.
- [ ] The measurement method is stated once and used for every row.
- [ ] Each row's count is split: production in `tekstide`, `tekstide-core`-internal, test-only.

## The table itself (PR-036-A)

- [ ] One row per orphan, with verdict, reason, counts, and **what the capability was for**.
- [ ] **Per D4, each row names the search shape that would have found it.**
- [ ] Reviewed **before** any deletion happens.

## D1 — deletion

- [ ] Deletions batched into one release (`0.16.0`), not trickled.
- [ ] Each "delete" row cites what was searched, rather than asserting dormancy.
- [ ] `CHANGELOG.md` names every removed public item individually — **written in `0.16.0`'s own
      entry at release time**, never by editing a released entry.

## D2 — "keep, documented" is earned

- [ ] Every "keep" row cites an **RFC number** as its named consumer.
- [ ] The four configuration-conditioned rows cite **RFC-045**.
- [ ] No row uses this verdict without a number. If one wants to, it is an "own RFC" row instead.

## D3 — the two that left

- [ ] `recover` and `purge_all_records` are **not** triage rows.
- [ ] What a user experiences with a corrupt store today is **reproduced**, not reasoned about —
      a corrupted store in a scratch `XDG_STATE_HOME`, against the release binary.
- [ ] The outcome is either a fix or a written RFC recommendation. Both are acceptable; silence
      is not.

## Wiring (PR-036-B)

- [ ] Each wired item has a real production caller and a test that the caller reaches it.
- [ ] Ablated: remove the call site, watch that test fail.

## Gates

- [ ] `fmt`, `clippy -D warnings`, `git diff --check`, `rfc_docs_invariants`.
- [ ] Full workspace suite, **three consecutive runs**, each logged to a file; any flake named
      against the register **with a row**.
- [ ] For deletions: `cargo publish --workspace --dry-run --locked` still passes.

## The outcome this slice is allowed to reach

- [ ] A row saying "own RFC" is a **finished** row, not an unfinished one. A table whose hard
      cases all read "keep, documented" has hidden them.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
