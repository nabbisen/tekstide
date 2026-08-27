---
title: "RFC-036 PR-036-C: the corrupted audit store, reproduced"
rfc: "RFC-036"
rfc_file: "../../accepted/036-dormant-capability-closure.md"
source_rfc_status: "Accepted 2026-08-18, D0–D4 decided 2026-08-27 — M12"
target_milestone: "M12"
created: "2026-08-28"
---

# What a user experiences when the audit store can't be trusted

Per D3: `recover`, `resume`, `purge_all_records`, and `purge_project_records` are not triage rows —
they are a recovery path and a data-deletion path that have never run from the application. The
question is not wire/delete/document; it is *why can a user's corrupted audit store not be
recovered by the application that built the recovery?* This document reproduces the answer rather
than reasoning about it from the code, per the task breakdown's own requirement, then recommends
what should happen next.

## Method

Release binary (`cargo build --release -p tekstide`), a fresh `mktemp -d` fixture project and a
fresh `mktemp -d` `XDG_STATE_HOME`, `WAYLAND_DISPLAY` unset to force X11/XWayland — this project's
own established convention. Two corruption shapes, each reproduced against a real running instance
and screenshotted, not inferred:

1. **A genuinely corrupted database file** — the real `audit.sqlite3` `AuditStore::open` had
   already created, overwritten with random bytes.
2. **An interrupted-migration marker present** (`recovery_is_active()` — the exact condition
   `AuditStoreErrorReason::RecoveryIncomplete` exists to name) — the shape `resume()` is the
   designed answer to. Confirmed this shape was real, not assumed: temporary `eprintln!`
   instrumentation in `open_audit_store` (reverted before this document was written, `git diff
   --stat` confirmed empty) showed `AuditStore::open(...).is_ok() == false` for the exact
   directory under test, ruling out an earlier, contaminated attempt (a stale background process
   from a prior scenario, not a real product behavior) that had briefly suggested otherwise.

## What was found

**Both shapes produce the identical, silent result.** `EVIDENCE-1` (genuine corruption) and
`EVIDENCE-2` (recovery-blocked, instrumentation-confirmed) show the same screen: the project opens
normally, Restricted Mode, the runtime summary, "9 blocked automations" — nothing anywhere
indicates the audit trail is broken. No dialog, no banner, no degraded-mode marker. A user cannot
tell the difference between "everything is being recorded" and "nothing has been recorded since
the store broke," because `open_real_audit_store` (`shell.rs`) is deliberately "fail-silent,
log-nothing-to-the-user" by its own doc comment — reasonable for *this* function's own scope (an
observability path must never block the app from starting), but nothing sits above it to do what
that function itself explicitly does not.

**The corrupted file is never touched again.** Confirmed by hash comparison before and after a run:
byte-for-byte identical. Every subsequent audit-writing action during that session silently no-ops
against the same broken store, forever, until something outside the running application fixes it.

**Nothing in production ever calls `recover`, `resume`, `AuditRecovery` at all** — confirmed by
PR-036-A's own reachability sweep (0 production, 0 `tekstide-core`-internal callers for either) and
directly, by reading `main.rs` (no reference to `AuditRecovery`, `recovery`, or `AuditStore` at
all — audit-store handling exists only inside `shell.rs`'s per-event `open_audit_store`, which
never branches on `AuditStoreErrorReason::RecoveryIncomplete` specifically; every error, including
that one, collapses to the same `None`).

## Why this is not merely dormant, per this RFC's own opening argument

`recover()`/`resume()` are reviewed, tested, and have never run from the shipped application — the
same shape as `close_project` before RFC-039, which this RFC's own preamble already names as its
central argument: *"the two items anyone looked at were both shipped defects."* This is exactly
that shape a third time. Unlike a dormant capability nobody happens to need, this one is a recovery
path for the exact failure this document just reproduced.

## Decided: an RFC recommendation, not a fix, and why

The task breakdown explicitly allows either outcome. Two reasons this one is the recommendation:

1. **The two corruption shapes need different remedies, and one of them is a real product
   decision, not a wiring gap.** An interrupted-migration marker (`recovery_is_active()`) means the
   application was already, safely, in the middle of a known recovery when it stopped — calling
   `resume()` automatically the next time `AuditStoreErrorReason::RecoveryIncomplete` is seen is
   mechanically small and requires no new design; nothing is discarded that was not already being
   discarded by the current silent failure. A **genuinely corrupted file with no marker**, by
   contrast, is what `recover()` handles by *quarantining the existing database and starting
   fresh* — silently discarding a user's entire audit history is a materially bigger decision than
   "resume what was already safely in progress," and doing it automatically, without telling the
   user, is not obviously the right default. That is a UX question (auto-recover silently? ask
   first? just surface a banner so a user knows to look?), not a missing function call.
2. **Even the safe half is not free of product-visible consequences.** A successful automatic
   `resume()` means the user's audit history is intact again — but nothing tells them recovery
   happened, or that it was ever broken, unless something is built to say so. Building that
   notification layer is exactly the kind of design this RFC's own risk section warns against
   absorbing into a triage slice ("triage becomes a rewrite").

**Recommendation for the RFC this becomes, as a head start, not a substitute for it:**

- Split the two shapes rather than treating "the store is unusable" as one condition:
  `AuditStoreErrorReason::RecoveryIncomplete` specifically should attempt `AuditRecovery.resume()`
  once per session before falling back to today's silent `None` — mechanically small, matches
  `resume()`'s own designed purpose ("retries the exact recovery identified by the durable
  active-recovery marker"), and a caller-must-close-handles precondition is trivially satisfied
  here since `open_internal` never opened a live connection before this branch fires.
- A genuinely corrupted file (any other `AuditStoreError`) is the real product question: whether
  Tekstide should offer `recover()` at all without asking, and if so, whether a user is ever told
  their prior audit history is gone. Recommend this is decided explicitly, not defaulted by
  which of the two error variants a given corruption happens to produce.
- At minimum, in both cases: something a technical user can find (a log line, a diagnostic file)
  should record that the audit store failed to open and why — today there is not even that.

**Number**: none reserved. Recording the recommendation here is what RFC-036's own D2 (and
`what-a-triage-must-not-become.md` §6) asks for; reserving a number is the human owner's decision,
the same deferral RFC-036 itself already made once for RFC-045.

## Evidence

`evidence/EVIDENCE-1-genuinely-corrupted-store-silent.png`,
`evidence/EVIDENCE-2-recovery-blocked-store-silent.png` — both `/tmp` `mktemp -d` fixtures, no path
under `$HOME`, no real project name.
