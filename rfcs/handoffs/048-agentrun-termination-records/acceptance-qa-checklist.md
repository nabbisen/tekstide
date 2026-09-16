---
title: "RFC-048 — acceptance and QA checklist"
rfc: "RFC-048"
rfc_file: "../../done/048-agentrun-termination-records.md"
source_rfc_status: "Implemented and closed 2026-09-16 — M12"
target_milestone: "M12"
created: "2026-09-16"
---

# Acceptance and QA checklist

Every box is a property. A box whose plan assigns it elsewhere, or that cannot be satisfied as
written, stays unticked with the contradiction named — the reviewer's error to fix, not the
implementer's to paper over.

## PR-048-A — the producer and its one path

- [x] A run that exits records `Terminated`/`ProcessExited` after its `Started`, **read back from a
      real store**, same operation id.
      *`a_run_that_exits_records_terminated_with_no_payload_from_the_outcome`, against a real
      `AuditStore`; the two phases are asserted to share an operation id.*
- [x] A run killed through the close flow records `Terminated`/`ProcessTerminated`.
      *`a_run_killed_by_a_signal_records_process_terminated_without_the_signal`, with the outcome a
      close produces (`KilledAfterTimeout`). The signal names are asserted absent from the record.*
- [x] A post-start runtime failure records `Terminated`/`RuntimeFailure`.
      *`a_post_start_runtime_failure_records_runtime_failure_without_its_summary`. **This is the one
      behaviour change to the existing producer**: `Failed` was grouped with `OrphanedUnknown` and
      recorded nothing, which said of an observed failure what §1 says only of an unobserved one.*
- [x] **A detached run records nothing.** **Ablation:** record one anyway; the test fails alone.
      *The existing `orphaned_runtime_truth_is_not_mislabeled_as_durable_termination` holds it; C1
      (record an ending for a detached run) fails it alone.*
- [x] **The record's fields are asserted exhaustively**, so no exit status, signal, or
      `BoundedRuntimeSummary` text can have reached it (§2). **Ablation:** put the exit status in a
      field; the test fails alone.
      *Every field of the `Terminated` record is named in the exit test. C3 fails **three** tests, not
      one: putting the exit status in `subject_ref` makes `valid_managed_process` reject the record
      outright, so every test that reads one fails. Disclosed — the schema refusing the payload is a
      stronger result than one assertion catching it.*
- [x] **A degraded store does not block termination**: the run still reaches its end status.
      **Ablation:** `append_required`; the test fails alone.
      *The existing `termination_truth_survives_observational_audit_failure`. C4 fails it **and** the
      never-started test, which is the same property seen from the other side; disclosed.*
- [x] **The store refuses a termination with no `started` phase** (D6), asserted against the store.
      *`the_store_refuses_a_termination_for_a_run_that_never_started`: the producer reports `Degraded`
      and no `Terminated` row exists. The producer does not re-check the ordering.*
- [x] **One path, by construction** (§4): production calls the coordinator, never
      `ProjectSession::apply_agent_terminal_outcome`. **Grep**, and **ablation:** delete the recording
      from the coordinator; a test fails alone.
      *Both production sites call one helper, `apply_agent_terminal_outcome_and_record`; grepped, no
      other production call exists. C5 (production applies without the coordinator) fails
      `a_real_agent_run_that_exits_records_its_termination_through_production` **alone**. **Disclosed:**
      the helper still has an unaudited branch — a launch with no store or no `Started` phase must
      still be applied, and the store would refuse its `Terminated` anyway (D6) — so "one path" is one
      *decision point*, not one branch.*

## PR-048-B — the documentation

- [x] `crates/tekstide-core/README.md`'s "no termination record exists" sentence is corrected **in the
      commit that makes it false**, and says what a detached run's trail does instead.
      *Corrected in **PR-048-A's** commit, not this one: the commit that makes it false is the
      producer's, and a box asking for the same commit cannot be satisfied by a later one. The
      pack's plan puts documentation in B; this one sentence had to move.*
- [x] The changelog says both halves in a user's words.
      *An `Unreleased` entry: what is recorded and what it can never contain, and that a detached run
      has no recorded ending on purpose.*
- [x] The book's audit description states the limit.
      *An *AI CLI runs* bullet in `local-data-and-privacy.md`, beside the other families.*
- [x] **No text claims the trail answers whether a run is still going.** It does not.
      *Both the crate README and the book say so explicitly; grepped for the claim and found none.*

## Whole-RFC

- [x] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, and **three consecutive full-workspace runs with
      `--no-fail-fast`**, output redirected to files.
      *All clean; 538 + 9 + 823, green every time. Clippy rejected three `drop(coordinator)` calls in
      the new tests (no `Drop` impl); removed rather than silenced.*
- [x] Every new intermittent failure has a dated row in `test-process-leak.md`.
      *One, and I could not name it: a single failure in the `tekstide` binary whose log I destroyed by
      re-running instead of reading. Row records what is known and the lesson.*
- [x] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [x] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Accepted 2026-09-16 (response 400). Four reviewer ablations, each restored and
hash-checked: a detached run recording a termination, the launch keeping no identity,
an observed runtime failure recording nothing, and production applying the outcome
without the coordinator. Each failed exactly one test. Gate: 538 + 9 + 823, three runs.

Two of this pack's own premises were wrong and were corrected in implementation; both
are recorded in the RFC rather than quietly fixed.
```
