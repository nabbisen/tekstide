---
title: "RFC-059 acceptance and QA checklist"
source_rfc: "../../accepted/059-seeing-the-audit.md"
source_rfc_status: "Accepted by the human owner 2026-10-11"
---

# RFC-059 — acceptance and QA checklist

Every box here stays unchecked in this file: it is a template per slice, not a record. Record a
slice's own run in its review request and its `qa-evidence.md`.

## PR-059-A — the store can be believed

- [ ] The enumeration is derived from the type, not from a hand-written list of combinations.
      **A reviewer must be able to break it by adding a family arm** and see the test fail.
- [ ] The pre-fix run is recorded: which family-and-outcome combinations did **not** land against
      the tree as it stood, with output. `safe_close_decision` + `blocked` is the one already
      known.
- [ ] If other combinations were found, they were reported before PR-059-B started.
- [ ] The first-creation race is **reproduced** with two genuinely overlapping processes before it
      is fixed, and the reproduction is the regression test.
- [ ] The fix is ablated: with it reverted, the reproduction fails; with it in place, it passes.
- [ ] No UI changed in this slice.

## PR-059-B — the view

- [ ] A real store with real records renders; counts and filters are checked against the store, not
      against a fixture built beside it.
- [ ] A bounded page prints `"{kept} listed, {omitted} more not recorded"` — the existing wording,
      not a new one.
- [ ] Filtering offers exactly `AuditQuery`'s own fields. No free-text search was added.
- [ ] Paging uses `before_sequence`. No offset path was added beside it.
- [ ] The wording guard asserts the text the view composes, via a factored-out function, not a
      string built in the test.
- [ ] `AuditHealth` is visible in the view when the store failed to open or a write was rejected.
- [ ] The keybinding is in `Ctrl+Alt+K`'s list **and** in `--help`, and `locales/en.ftl` is the
      source of every string shown.
- [ ] Live capture: a real window, throwaway project and throwaway `XDG_STATE_HOME`, verified-fresh
      per the pack README's clipboard recipe.

## PR-059-C — purge, reachable

- [ ] The confirmation names the scope — this project, or everything.
- [ ] The number shown after a purge is the receipt's `deleted_record_count`.
- [ ] A purge whose transaction failed does not report success.
- [ ] `docs/src/users/local-data-and-privacy.md`'s *"There is no in-app command to purge the audit
      store"* is corrected **in this slice**, and the correction says what a reader who relied on
      the old sentence should now do.
- [ ] Live capture of a purge, before and after, same capture discipline.

## Whole-RFC

- [ ] `REQ-SEC-025` is met for audit data, and the changelog says so.
- [ ] `REQ-SEC-023` is **not** claimed beyond what is true: met structurally for audit views
      because the schema has no field that can hold a value; **unmet for logs**, which is RFC-068.
      No entry anywhere implies the product redacts transcript content.
- [ ] `RedactionClaimScope` still has exactly one variant.
- [ ] No free-text column was added to `audit_events`.
- [ ] Approval History still exists and still answers its own question; neither it nor the audit
      view is worded so a reader takes it for the other.
- [ ] The book describes every family the view can display.
- [ ] `rfcs/handoffs/release-checklist.md` run in full for the candidate.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
