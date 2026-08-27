---
title: "RFC-036: Dormant Capability Closure — QA evidence"
rfc: "RFC-036"
rfc_file: "../../accepted/036-dormant-capability-closure.md"
source_rfc_status: "Accepted 2026-08-18, D0–D4 decided 2026-08-27 — M12"
target_milestone: "M12"
created: "2026-08-27"
---

# Evidence

## PR-036-A — the table

**No code change.** `rfcs/handoffs/036-dormant-capability-closure/orphan-triage-table.md` is the
whole deliverable.

### D0 — re-verified against the tree, not the document

Every row measured fresh via the audit's own proven method (`#[deprecated(note =
"reachability-audit")]` on 35 candidates across 12 files, one `cargo build -p tekstide`, revert),
not inherited from the RFC's own list or the original audit's. `git diff --stat` confirmed empty
after reverting the markers, before this document was written.

**Four discrepancies found and recorded in the triage table itself, per D0's own instruction to
correct rather than repeat stale numbers:**

1. `request_terminate`: the RFC's own D0 claims 3 production callers; measured **1**.
2. `switch_active_project`: wired (RFC-039 PR-039-B), but absent from the RFC's own "already
   assigned" list — an omission in that document, corrected here.
3. `shutdown` (ambiguous in the RFC's D0 table — two functions share the name):
   `TerminalReader::shutdown` has 1 production caller (very plausibly what the RFC's "1" meant);
   `approval::channel::shutdown` (the one the *original* audit actually named) has **0**.
4. `transition_change_set_review_state` — **this entry was wrong in the first pass, and review 354
   caught it.** The first pass found one internal caller (`app.rs:464`) and stopped without
   checking whether *that* caller itself had a production caller, concluding "0 production, still
   unwired." It has one: `AppState::transition_active_project_change_set_review_state`
   (`app.rs:452`), a wrapper RFC-034 added under a name that does not match the core function's
   own, called for real from `record_change_review_decision` (`shell.rs:8783`). **The count was
   right; the conclusion drawn from a count with a nonzero internal caller and no wrapper-name
   follow-up grep was not.** Corrected in `orphan-triage-table.md`; every other row with an
   internal-only caller in the first pass had its own wrapper independently marked
   `#[deprecated]` too (so its production reachability was checked directly, not assumed) —
   re-checked after this was found, and this was the only row with the gap.

`request_terminate` and `shutdown` (`TerminalReader`'s) are confirmed **not** orphans either way —
neither appears in the table as a triage row, matching the RFC's own trap warning.

### The table itself

One row (or one grouped finding, where treating siblings separately would be an arbitrary split of
one finding into two — `purge_project_records`/`purge_all_records`, the
`record_terminal_transcript_write_summary`/`record_transcript_write_summary` pair, and the
four-function managed-agent-audit cluster) per orphan, each carrying measured counts split three
ways, a verdict, a one-line reason naming what the capability was *for*, and the D4 search shape.
Full detail in `orphan-triage-table.md`; not duplicated here.

**No row uses "keep, documented" without an RFC number.** The four RFC-045-conditioned rows cite
it explicitly, per D2. Every row this triage found needing real design is an "own RFC" row with no
number reserved — nine rows, none absorbed into a wire verdict. **Zero wire verdicts** in this
pass; nothing found had both a real gap *and* a small enough fix to belong in PR-036-B's own
"real caller and a test" bar without also being new design.

**The finding this triage rates as its own most important result, confirmed a defect at review
354**: `launch_managed_agent_run` and its three siblings (`apply_managed_agent_terminal_outcome`,
`open_project_text_document`, `save_project_text_document`) are fully-built, tested audit-writing
paths that production never calls — a Managed-compatibility agent-run launch produces no durable
audit record today. Checked directly (read the function body, confirmed it calls
`append_required`/`append_observation` against the real `AuditStore` and does the real launch)
rather than inferred from the name. The first pass here handed "is this a defect or an unwritten
scope choice" to whoever authors the recommended RFC; the reviewer answered it directly instead, on
three independent grounds (the asymmetry against plain-terminal auditing cannot be a choice, a real
scope choice would be written down somewhere and isn't, and `tekstide-core/README.md` asserted the
opposite on a published crate). Recorded as the "third category" D4 asks the triage to name if
found, since it is neither "no route to a user" (RFC-040/044's shape) nor "an error path that never
runs" (D3's shape) — it is a capability a user reaches constantly, whose audit trail nothing
writes.

**Required, done immediately rather than deferred to whenever the recommended RFC lands**:
`crates/tekstide-core/README.md`'s claim that durable audit "currently records … managed AgentRun
lifecycle" was false in the shipped product (recorded in tests only) — corrected, with the finding
and its date noted in place of the false claim, per this project's own convention that a false
statement about a published crate gets fixed the moment it is found.

### Gate

`git diff --check`: clean. `rfc_docs_invariants` (4 tests): clean —
`every_relative_link_in_the_rfc_tree_resolves` in particular confirms this document's own links
resolve. `fmt`/`clippy` not run against a code change, since PR-036-A makes none; the temporary
`#[deprecated]` markers used for measurement were reverted before any gate ran, confirmed via
`git status`/`git diff --stat` both empty for every source file touched.

## PR-036-B — wire and delete

**Only what the reviewed table says.** Zero wire verdicts existed, so this slice is deletion only:
the 9 functions across 7 delete-verdict rows.

### A deviation from the literal verdict, found while executing it, and why

The table said "delete," verified against production reachability. What it did not check — because
checking it wasn't part of measuring reachability — was whether each function also served as
**test fixture infrastructure for unrelated tests**, not just its own dedicated tests. Attempting
the literal deletion surfaced this for four of the nine:

- `add_agent_run`: a dozen tests across `change_detection`, `collections`, `transcripts` use it as
  cheap setup ("a project with an agent run attached") for tests not about launch mechanics.
  `attach_agent_launch_plan`, the real production path, needs a full `AgentRunLaunchPlan` +
  `TerminalSession` — rewriting a dozen unrelated tests onto that ceremony is a materially larger
  change than this triage's own scope.
- `add_transcript`: same shape, including **one caller in `tekstide`'s own crate**
  (`shell/tests.rs:10781`) — the real private path (`attach_agent_run_transcript`) is not even
  visible across the crate boundary.
- `accept_proposal`: roughly a dozen tests in `approval::tests::channel` and
  `approval::tests::reference_adapter` use its single-connection, single-thread shape as their
  primary vehicle for testing authentication and frame parsing directly — `serve_concurrently`'s
  multi-threaded accept loop would be a materially larger, security-test-relevant rewrite to remove
  one dormant production entry point.
- `record_transcript_write_summary`: one remaining test (`local_data_summary_counts_retained_...`)
  used it as fixture setup — resolved differently, see below, since a smaller fix existed.

**Resolution: narrowed to `#[cfg(test)]` (or `#[cfg(any(test, feature = "test-support"))]` for the
one with a cross-crate caller — the same gate `runtime/terminal/launch.rs`'s own leak guard
already uses to cross that exact boundary) rather than deleted outright**, for `add_agent_run`,
`add_transcript`, and `accept_proposal`. This satisfies D1's actual goal — removed from the
*published, production-reachable* API, which is what "dead code in a published crate" means — without
forcing an unrelated, much larger rewrite of security- and behavior-relevant test coverage that PR-036-B's
own scope ("only what the table says") does not cover. Every reader of the deleted symbol from
outside this crate loses it exactly as if it had been deleted outright; only this crate's own test
suite (and, for `add_transcript`, `tekstide`'s test suite via the `test-support` feature) still
sees it. Documented at each function's own definition, not silently done.

**`record_transcript_write_summary`'s one remaining test call site needed no such gate.** Read
closely: `local_data_summary_counts_retained_bytes_without_transcript_content`'s own fixture helper
(`attach_agent_run_transcript`, this test file's own, distinct from the private core method of the
same name) already calls the public `Transcript::record_active_write` directly on the value before
attaching it — setting the exact `byte_count`/state the test's own subsequent
`record_transcript_write_summary` call redundantly re-set to the identical value. Removed the
redundant call; the test's own real assertions (`transcript_local_data_summary`'s retained-bytes
and budget-pressure accounting) are unaffected, verified by the still-passing test.

**The other five (`add_audit_event`, `add_approval`, `shutdown` [approval channel],
`record_terminal_transcript_write_summary`, `record_transcript_write_summary`) had no such
fixture-reuse pattern** — checked directly per function, not assumed: every one of their own test
call sites asserted something about *that function's own* behavior (a specific `OwnershipError`
variant, a duplicate-attachment rejection, socket cleanup timing), so deleting the function and its
own dedicated tests together lost no coverage of anything else. One exception handled individually:
`shutdown`'s own dedicated test
(`serve_concurrently_endpoint_is_dropped_and_socket_removed_after_shutdown`) became fully redundant
with a *stronger* sibling test already in the same file
(`dropping_serve_shutdown_while_the_loop_is_blocked_in_accept_still_cleans_up`, which proves the
same socket-cleanup invariant via `Drop` alone, in the harder case of a blocked `accept()`) —
deleted rather than adapted, since adapting it would have only reproduced a weaker version of a
test that already exists.

### A side effect, disclosed rather than chased

Deleting `record_transcript_write_summary` leaves `Transcript::record_active_write`,
`record_truncated_write`, and `record_lifecycle_state` with **zero remaining production or
`tekstide-core`-internal callers** (only test callers, via the fixture helpers above) — a new
orphan created by this removal, the same way RFC-023's own closure created three. **Not chased
further, deliberately**: these three methods were not part of PR-036-A's own reviewed table, and
deciding their own fate is a new measurement this slice's scope does not cover. Named here so it
does not go unnoticed the way RFC-023's own new orphans would have without RFC-036's own
"What is already assigned" section recording them.

### What was actually removed, and how it's verified

Nine functions, seven table rows — full detail and reasoning per item in `CHANGELOG.md`'s new
`0.16.0` entry, written now (not deferred) since `0.16.0` has not shipped, matching response 352's
own established precedent: a released entry is not rewritten, an unreleased one is exactly where
new work's own record belongs.

Verification, not assertion: `cargo build --workspace --all-targets` clean (catches every stray
reference the deletions missed, including two private helpers that became newly dead —
`ProjectSession::ensure_approval_exists` and `::transcript_mut` — removed alongside their own
callers). `cargo clippy --workspace --all-targets -- -D warnings` clean (catches unused imports the
build alone would only warn on). Full workspace suite: `tekstide-core` 746 → 738 (8 tests removed,
each verified above as testing only the deleted function's own behavior), `tekstide` unchanged at
456 (the `#[cfg(any(test, feature = "test-support"))]` gate keeps its one `add_transcript` caller
compiling).

### Version and publish gate

Workspace version bumped `0.15.0` → `0.16.0` (`Cargo.toml`), per D1: `tekstide-core` is on
crates.io, deletion is breaking, and the bump was already owed for RFC-044's own unreleased work
sitting in the same tree — this batches both into one release rather than two. `Cargo.lock`
auto-updated by the build. `cargo publish --workspace --dry-run --locked --allow-dirty` (dirty only
because this is pre-commit): packages, verifies, and would upload both crates at `0.16.0` cleanly.

### Gate

`fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`, `rfc_docs_invariants`
(4 tests): clean. Three consecutive full-workspace runs: **456 + 4 + 738, fully green** every
time — no flake.

## PR-036-C

Not started.
