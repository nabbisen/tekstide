---
title: "Audit-store test isolation — implementation handoff"
rfc: "none"
source_rfc_status: "No RFC. Test infrastructure; the one design decision is settled below."
target_milestone: "M12"
created: "2026-08-26"
---

# Give every test its own audit store

**No RFC.** Nothing about the product's behaviour changes. The single decision an implementer
would otherwise have to make alone is settled in "Decided" below.

Scoped as the third theme for `0.15.0`, ahead of the doc invariants, because it corrupts the
evidence every other slice is accepted on.

## The finding, measured

`rfcs/handoffs/test-process-leak.md`'s "ROOT CAUSE, CONFIRMED 2026-08-26" section has the full
record. In short, every figure below from a **fresh** `XDG_STATE_HOME`:

| Condition | Result |
| --- | --- |
| Parallel (default) | **6 failures** |
| Parallel, query window raised 50 → 100000, three runs | **17, 23, 17 failures** |
| **Serial (`--test-threads=1`)** | **444 passed, 0 failures** |

A full run appends **111–130 records**. Serial passes anyway, so the 50-record window is not the
cause — and raising it made things measurably *worse*, consistent with longer-held locks.

**The cause is that 23 test call sites share one SQLite database and run in parallel.**
`open_real_audit_store` resolves `AppStatePathProvider::linux_default()` — `$XDG_STATE_HOME`, or
`$HOME/.local/state/tekstide` when that is unset — for all of them.

Two signatures, one cause: `the real audit store must open` (the store would not open), and
`left: 0, right: 1` (it opened and returned nothing for this test's project).

**This is the cause of every audit-store row in the flake register, including row 3**, open since
request 276. It never reproduced in isolation because isolation is what makes it pass.

## Decided: prevent, do not merely redirect

An implementer could reasonably do the small thing — point the test callers at a temp directory
and stop. **Do more than that.**

> **A test that opens the real state directory must fail loudly, not silently succeed.**

Pointing tests elsewhere fixes today's 23 call sites. It does nothing about the twenty-fourth,
written six months from now by someone who copies an existing test and does not know this
document exists. This project's own idiom is to make the wrong thing unrepresentable rather than
merely absent — `DisplayText`'s single constructor, `ChangeReviewContentLine` behind a module
boundary, `ModalAbsent` as a proof token. The same move applies here.

Shape is yours. A test-only guard that panics with a clear message when the resolved state dir is
the real one, a seam that makes the real path unreachable from test code, or something better —
what matters is that the *next* person cannot reintroduce this without being told.

**The second problem is why this matters beyond flakiness.** With `XDG_STATE_HOME` unset — the
ordinary case for anyone running `cargo test` — the suite reads and writes **the developer's real
audit store**, and has for the life of this project. `shell/tests.rs`'s own comment on
`fresh_state_root_dir` says a test state root "must never be the developer's real
`$XDG_STATE_HOME`." That discipline exists in this file already; it was applied to the transcript
root and not to this path.

## Scope

1. **Every `open_real_audit_store` test caller gets its own store.** 23 sites. `temp_audit_state_dir`
   already exists in the same file and is used by other tests — reuse it or its shape rather than
   inventing a second helper.
2. **The guard from "Decided" above.**
3. **`AuditQuery::latest(50)` at the 22 test sites** — a query by recency standing in for a query
   about one project. Once stores are isolated this is no longer a correctness risk, but it is
   still the wrong shape, and it is why the first diagnosis of this bug was wrong. Fix it if the
   store's API supports a project-scoped query; if it does not, say so and leave it, rather than
   adding an API for a test.
4. **`recovery_event_exists` in `crates/tekstide-core/src/audit/recovery.rs`** — the one production
   caller of `latest(N)`. It answers "was this recovery already recorded?" by looking at the newest
   10. Bounded and unlikely to bite (recovery runs at startup, one process), but it is the same
   shape. **Judge it on its own merits and say what you decided** — fixing it and leaving it are
   both defensible; not mentioning it is not.

## What a fix must not do

- **Do not serialise the test suite.** `--test-threads=1` is the diagnostic that found this, not
  the remedy. It trades a 5-second parallel run for a 25-second serial one and leaves the
  shared-real-store problem exactly where it is.
- **Do not tune the query window.** Measured above: raising it made things worse. A suite whose
  correctness depends on a query limit is wrong even on the runs where it passes.
- **Do not change production audit behaviour** beyond item 4, and not without saying so.

## Acceptance

- [ ] **The confirmation is re-run and recorded**: parallel and serial, fresh `XDG_STATE_HOME`,
      before and after. The "after" must show parallel clean across **five** consecutive runs —
      more than the usual three, because the failure rate observed was 6 to 23 per run and three
      clean runs is weak evidence against something that intermittent.
- [ ] Every one of the 23 call sites isolated; none left resolving the real path.
- [ ] **The guard is ablated**: point one test at the real state dir, watch it fail loudly with a
      message that explains itself, restore.
- [ ] `XDG_STATE_HOME` unset — the ordinary developer case — and the suite still touches nothing
      under `$HOME`. **Verified, not assumed.**
- [ ] The `latest(50)` sites and `recovery_event_exists` each addressed or explicitly declined,
      with the reason.
- [ ] The flake register's audit-store rows updated: what this closes, and what remains.
- [ ] `fmt`, `clippy -D warnings`, `git diff --check`, `rfc_docs_invariants`.

## A note on how this was found, worth keeping

The first published diagnosis of this bug — mine, the same day — blamed `latest(50)` truncation.
It was wrong. The experiment that would have confirmed it refuted it: raising the window made
failures triple.

**A hypothesis that has not been tested is not a finding, however well it reads.** This document
exists because the test was run before the fix was scoped, and the answer was the opposite of the
expected one.
