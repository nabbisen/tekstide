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

- [x] Every row re-verified against the current tree (compiler-based method, 35 candidates,
      one `cargo build -p tekstide`).
- [x] `request_terminate` and `shutdown` (`TerminalReader`'s) are **not** listed as orphans.
      **Correction found**: the RFC's own D0 claimed `request_terminate` had 3 production callers;
      re-measured today, it has **1**. Recorded in `orphan-triage-table.md`'s own D0 section, not
      silently fixed without comment.
- [x] The measurement method is stated once (`orphan-triage-table.md`'s own opening section) and
      used for every row.
- [x] Each row's count is split: production in `tekstide`, `tekstide-core`-internal, test-only.

## The table itself (PR-036-A)

- [x] One row per orphan, with verdict, reason, counts, and **what the capability was for**.
- [x] **Per D4, each row names the search shape that would have found it.**
- [ ] Reviewed **before** any deletion happens. *(Filed for review; PR-036-B has not started.)*

## D1 — deletion

- [ ] Deletions batched into one release (`0.16.0`), not trickled. *(PR-036-B, not started —
      table names 7 delete-verdict functions, none removed yet.)*
- [x] Each "delete" row cites what was searched, rather than asserting dormancy — and, per §2 of
      the risk document, what the real path is instead, so the *why* is not lost when the row's
      own function is removed.
- [ ] `CHANGELOG.md` names every removed public item individually — **written in `0.16.0`'s own
      entry at release time**, never by editing a released entry. *(PR-036-B, not started.)*

## D2 — "keep, documented" is earned

- [x] Every "keep" row cites an **RFC number** as its named consumer.
- [x] The four configuration-conditioned rows cite **RFC-045**.
- [x] No row uses this verdict without a number. Nine rows this triage found undesigned are
      "own RFC" rows instead, none forced into "keep, documented."

## D3 — the two that left

- [x] `recover` and `purge_all_records` are **not** triage rows. **`purge_project_records` folded
      into the same defect slice**, recorded and reasoned rather than silently added or omitted.
- [ ] What a user experiences with a corrupt store today is **reproduced**, not reasoned about —
      a corrupted store in a scratch `XDG_STATE_HOME`, against the release binary. *(PR-036-C, not
      started.)*
- [ ] The outcome is either a fix or a written RFC recommendation. Both are acceptable; silence
      is not. *(PR-036-C, not started.)*

## Wiring (PR-036-B)

- [ ] Each wired item has a real production caller and a test that the caller reaches it.
- [ ] Ablated: remove the call site, watch that test fail.

## Gates

- [ ] `fmt`, `clippy -D warnings`, `git diff --check`, `rfc_docs_invariants`.
- [ ] Full workspace suite, **three consecutive runs**, each logged to a file; any flake named
      against the register **with a row**.
- [ ] For deletions: `cargo publish --workspace --dry-run --locked` still passes.

## The outcome this slice is allowed to reach

- [x] A row saying "own RFC" is a **finished** row, not an unfinished one. A table whose hard
      cases all read "keep, documented" has hidden them. Nine rows in this table say "own RFC";
      none say "keep, documented" without the RFC-045 number D2 requires.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
