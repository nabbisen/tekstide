---
title: "RFC-048 — QA evidence"
rfc: "RFC-048"
rfc_file: "../../done/048-agentrun-termination-records.md"
source_rfc_status: "Implemented and closed 2026-09-16 — M12"
target_milestone: "M12"
created: "2026-09-16"
---

# Evidence

## Two premises in the pack, corrected before anything was written

**1. The producer already existed.** The README says the record *"already exists in the schema and
nothing writes it"* and the plan says the coordinator *"gains a method"*. It had one:
`apply_managed_agent_terminal_outcome` applies the outcome, maps the reason codes, writes
`Terminated` with the launch's operation id, and is best-effort — built, tested, and **called only
from its own test suite**. That is RFC-036's shape, and the third instance this pack's own §4 names.

What was actually missing was **the mapping for `Failed`** (grouped with `OrphanedUnknown`, recording
nothing) and **a production caller**.

**2. Production could not have called it.** The method takes an `AuditedAgentLaunch`, and production
**drops that value at the launch**: it keeps the run id and moves `runtime_events` and
`approval_endpoint` out, discarding the `operation_id`. Without that id there is no `Terminated`
phase to write, because D6's ordering is keyed on it. A caller that simply "moved to the coordinator"
would not have compiled, and if it had, the store would have refused every record.

## What that made necessary

- **`AuditedAgentRunIdentity`** — the four ids an ending needs, separable from the launch's one-shot
  payload. `AuditedAgentLaunch::identity()` produces one; the existing method delegates, so its tests
  keep testing the same path.
- **`AuditCoordinator::apply_agent_terminal_outcome`**, taking that identity. This is the method §4
  describes: it applies and records together, so a caller cannot take the first half and forget the
  second.
- **`State.audited_agent_runs`** in the shell, filled at launch. Session-scoped, which is the honest
  bound: the two terminations this product observes — a terminal exiting, and a project close — both
  happen in the session that launched the run, and a run whose process outlives the application is a
  detached run, which records nothing anyway (D2).
- **One shell helper**, `apply_agent_terminal_outcome_and_record`, called by both sites.

**The type is a guarantee, not a discipline** (§2): the identity carries no exit status, no signal
and no `BoundedRuntimeSummary`, so there is nothing for a producer to leak even by mistake.

## §4, honestly

Both production sites call the helper, and grep finds no other production caller of
`apply_agent_terminal_outcome*`. **The helper still has a second branch**: a launch with no audit
store, or one whose plan was not auditable, has no `Started` phase, so its outcome is applied without
a record — and the store would refuse a `Terminated` for it anyway (D6). So "one path" here means
**one decision point**, not one branch. Named rather than glossed.

## Tests, each read back from a real store

| Test | Proves |
| --- | --- |
| `a_run_that_exits_records_terminated_with_no_payload_from_the_outcome` | the ending is in the store, after its own `Started`, under the **same operation id**; every field asserted, so §2 has nowhere left to hide |
| `a_run_killed_by_a_signal_records_process_terminated_without_the_signal` | the close flow's own outcome maps to `ProcessTerminated`; no signal name reaches the record |
| `a_post_start_runtime_failure_records_runtime_failure_without_its_summary` | D1's new arm, with a path-shaped sentinel in the summary asserted absent |
| `the_store_refuses_a_termination_for_a_run_that_never_started` | D6 is the store's job; the producer reports the refusal rather than hiding it |
| `a_real_agent_run_that_exits_records_its_termination_through_production` | **§4**: a real launch, `record_terminal_exit` as production calls it, and the record read back |

Two existing tests already held the rest: `orphaned_runtime_truth_is_not_mislabeled_as_durable_termination`
(§1) and `termination_truth_survives_observational_audit_failure` (D4).

## Ablations, each restored and hash-checked, `--no-fail-fast`

| | Ablation | Fails |
| --- | --- | --- |
| C1 | record an ending for a detached run | the orphaned test **alone** |
| C2 | drop the reason code | **five tests** — every one that asserts a reason code; one mechanism, disclosed |
| C3 | put the exit status in `subject_ref` | **three tests** — `valid_managed_process` rejects the record outright, so every test that reads one fails. The schema refusing the payload is a stronger result than one assertion catching it |
| C4 | `append_required` instead of `append_observation` | the degraded-store test **and** the never-started test — the same property from both sides |
| C5 | production applies the outcome without the coordinator | `a_real_agent_run_that_exits_records_its_termination_through_production` **alone** |

**C3 and C4 did not compile on their first attempt** (`AuditReference::new` returns `Option`, and
`append_required` returns a different type), and my harness reported that as *"not evidence"* rather
than as an absence of failures — the rule this register earned at RFC-049 PR-049-C. Both were
rewritten and re-run.

## An intermittent I could not name

One test in the `tekstide` binary failed once during this slice's first gate (`537 passed; 1 failed`
where the suite has 538) and I **cannot say which**, because I piped that run through `grep` for the
result lines and then re-ran the suite to look — so the failing log never existed. Not reproduced in
11 consecutive runs afterwards. Dated row in `test-process-leak.md`, with the lesson: read the
failing log, do not re-run to inspect it.

## PR-048-B — the documentation

- **`crates/tekstide-core/README.md`'s sentence is corrected in PR-048-A's own commit**, not here.
  The checklist requires it *"in the commit that makes it false"*, and the commit that makes it false
  is the producer's. The pack's plan puts documentation in B; that box cannot be satisfied by a later
  commit, so the sentence moved and the rest of the documentation stayed.
- **The changelog** says both halves in a user's words: an ending is recorded and what it can never
  contain, and a detached run has none, on purpose.
- **The book's audit description** gains an *AI CLI runs* bullet naming the ending, the three ways it
  can be described, and the silence for a run Tekstide stopped supervising.
- **No text claims the trail answers whether a run is still going.** Both the README and the book say
  it does not; grepped.

## Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`: clean — clippy rejected
three `drop(coordinator)` calls in my new tests (the type implements no `Drop`), and they are gone
rather than silenced. `rfc_docs_invariants`: 9 passed. `mdbook build docs`: clean. **Three consecutive
full-workspace runs with `--no-fail-fast`: 538 + 9 + 823, green every time** (+1 shell, +4 core).
`git diff --cached --check` after staging: clean.
