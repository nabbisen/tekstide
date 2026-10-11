---
title: "RFC-059 acceptance and QA checklist"
source_rfc: "../../done/059-seeing-the-audit.md"
source_rfc_status: "Implemented and closed 2026-10-12; `0.35.0` candidate, not yet published."
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

- [x] **Accepted.**
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Accepted 2026-10-12 as the `0.35.0` candidate, across review rounds **527 through 534**. Every
required item raised in those rounds is closed; nothing is carried as a condition of acceptance.

Reviewer notes:

```text
What acceptance rests on -- the reviewer's own measurements, not the implementer's reports:

  * Full release-checklist gate re-run by the reviewer at review 534: fmt clean, clippy
    --workspace --all-targets --all-features -D warnings 0 diagnostics, cargo test --doc
    --workspace clean, cargo test --workspace --all-targets --no-fail-fast giving
    779 + 22 + 1135 with 0 failed, 0 fixture entries in a fresh short /dev/shm TMPDIR, and
    the owner's real state directory at 0.
  * Both packages built, smoke-checked (no stray .rs, LICENSE/NOTICE/README present, no
    .git-exclude or agent config) and the published-shape manifest read back: the app pins
    tekstide-core = "0.35.0", not "0". cargo publish --workspace --dry-run --locked clean.
  * Unpacked tekstide-core-0.35.0, all [[bin]]s built first: 1134 passed, 1 failed, that one
    being the documented real_repository_filesystem_scan_cost_headless_benchmark, which
    refuses to report a number for something that is not a repository.
  * cargo audit: 0 vulnerabilities; three allowed warnings (paste, ttf-parser, lru) matching
    dependency-advisories.md exactly.
  * The compile guard ablated by the reviewer in a disposable git worktree: a thirteenth
    AuditEventFamily produces E0004 naming family_cycle_index, so a new family cannot be
    silently omitted from the filter cycle.
  * The first-creation race ablated by the reviewer, twice: 5 of 5 failures with the lock's
    call site removed, 0 fixture directories leaked across those failures once the drop
    guard landed (6 leaked from 6 failures before it).

Defects found in review and fixed before acceptance:

  * 527 -- the documented reproduction recipe did not work: `--lib` does not build [[bin]]
    targets, so the probe was missing and the test failed for an unrelated reason, reading
    as a successful reproduction both before and after the fix.
  * 527 -- fixture cleanup was a trailing call, not a drop guard; measured, six failing runs
    leaked six directories.
  * 527 -- the pre-fix proof was taken on a hand-dirtied tree with a git checkout restore,
    the exact practice ablate.sh exists to replace. Redone in a worktree.
  * 530 -- the view never said what it was. Approval History discloses that it is lossy; the
    audit view had no equivalent, so two legitimately disagreeing lists had nothing
    explaining the difference. Now the first line of the surface.
  * 530 -- ALL_FAMILIES/ALL_OUTCOMES were hand-maintained arrays of a thing PR-059-A had
    just made compile-time exhaustive. Now guarded by index functions the cycle navigates
    through, so the array and the match cannot disagree.
  * 532 -- purging audit records also revokes trust, because trust is decided by querying
    the exact rows a purge deletes, and the revocation lands at the next project open rather
    than immediately. The confirmation described the act as deleting a log and said nothing
    about it. Now stated in the dialog, both scopes, and in the book.

Deliberately out of scope, recorded rather than fixed:

  * REQ-SEC-023's log half -- a transcript holds raw run output, and a content scanner that
    misses one secret is worse than no claim. RFC-068, reserved 2026-10-11, not started.
  * The audit directory's permission modes (0755/0644 under a 0700 ~/.local/state), and the
    mouse-only reachability of several surfaces' controls. rfcs/future-work.md.

A fourth failure shape of the creation race, UnsupportedApplication, was found by the
reviewer's own ablation after the fix had landed: the loser opens a database the winner has
created but not yet stamped, and reads it as foreign. Recorded because it is the clearest
argument for why the lock covers the whole exists()-through-pragmas sequence.
```
