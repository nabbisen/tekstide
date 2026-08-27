---
title: "What a triage must not become"
rfc: "RFC-036"
rfc_file: "../../done/036-dormant-capability-closure.md"
source_rfc_status: "Implemented and closed 2026-08-28 — RFC-036 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-27"
---

# What a triage must not become

**Required reading before writing code.** This slice deletes API from a published crate and
assigns a permanent verdict to capabilities nobody is currently defending. Both directions can go
wrong quietly.

## §1 The comfortable bin

Three verdicts are on offer. **One of them costs nothing to choose and is almost always
defensible in the moment**: *keep, documented*.

It requires no deletion, breaks no consumer, and reads as prudence. It is also indistinguishable
from "we did not decide" once the reviewer is gone — which is exactly how ~104 capabilities came
to be dormant in a codebase where every single one was, at the time, reasonable to leave.

That is what D2's named-consumer rule is for, and it is why the rule demands **an RFC number, not
an intention**. If a row cannot cite one, it is not eligible for this verdict. "A future GUI might
want it" is the sentence this rule exists to reject.

## §2 The other direction: deleting something that was evidence

Deletion is recoverable from git, which is why D1 permits it. But two things are not recovered by
`git revert`:

- **A test that was the only caller.** `set_resource_limits` has exactly one — RFC-031's
  discrimination test, which uses it to force a real `RunLimitExceeded` refusal. Delete the
  function and you delete that test's ability to produce the state it discriminates. The RFC says
  *"delete it is no longer free"* for this row; the general rule is: **a test-only caller is a
  reason to look harder, not a reason to discount the count.**
- **The reason something existed.** A deleted function takes its doc comment with it. If the row's
  reason line says only "no callers," the *why* is gone from the tree and lives only in this
  slice's own table. Say what it was for, in the row, before removing it.

## §3 A count is a measurement, and measurements get cited

This project has a standing rule that a claim about behaviour cites the command that produced it.
**A caller count is a claim about behaviour.**

"0 callers" is not a row. `grep -rn '\bfoo(' crates/tekstide/src --include='*.rs' | grep -v tests.rs`
returning nothing, stated as the method used for every row, is a row.

And the count must distinguish production callers, `tekstide-core`-internal callers, and test-only
callers — because the three imply different verdicts and collapsing them is how a row gets decided
on the wrong number. D0 exists because two rows in the RFC's own list already changed while the
document sat still.

## §4 Two categories the audit could not see, and the third you may find

The reachability audit searched for **functions with no callers**. That shape found 104
capabilities and could not, even in principle, find:

- **Actions a user cannot take.** `close_project` was reviewed, tested core API with no GUI caller
  — a user could not close a project at all. Found by the owner asking why there was no button.
- **Paths that never run.** `recover` and `purge_all_records` are not missing callers so much as
  missing *executions*: an error path nothing has ever exercised from the application.

RFC-040 and RFC-044 have since built mechanical answers to the first (`control_coverage`,
`surface_keyboard_coverage`). **Nothing exists for the second**, which is why D3 pulls those two
rows out as a defect.

**If you find a third category while triaging, that is the most valuable thing in this slice.**
Say so in its own line. D4 asks every row to name the search that would have found it precisely so
this stays visible rather than being smoothed into "no callers."

## §5 What you may not do

- **Do not decide a row from the RFC's written list.** D0. Two entries are already wrong.
- **Do not use "keep, documented" without an RFC number.** D2.
- **Do not trickle deletions.** D1 says one release. A consumer whose build breaks twice for the
  same reason has been failed twice.
- **Do not fix what you triage.** The audit's own instruction was *do not fix anything you find*,
  and this RFC replaces it only for the wire/delete/document decision. Anything needing real design
  gets an RFC and a row pointing at it — the RFC's stated risk is that triage becomes a rewrite.
- **Do not delete `recover` or `purge_all_records`.** D3. They are unreached, not unwanted, and
  unreached recovery is a finding about the product.

## §6 If the honest answer is "this needs its own RFC"

Then the row says that, with the number if one is reserved and a recommendation if not.

That is not a failure to triage. **RFC-045 exists because of exactly this** — four rows whose
answer was "keep" but whose consumer had no number, which made D2 unenforceable until the number
existed. Reserving it converted a standing exception into four ordinary rows.

A table where some rows say "own RFC, recommended number: NNN" is a finished triage. A table where
those rows say "keep, documented" is a triage that hid its own hard cases.
