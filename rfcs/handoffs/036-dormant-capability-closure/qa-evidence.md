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

## PR-036-B, PR-036-C

Not started. Per the task breakdown's own gate: **the table is reviewed before PR-036-B starts.**
Nothing in this table has been acted on.
