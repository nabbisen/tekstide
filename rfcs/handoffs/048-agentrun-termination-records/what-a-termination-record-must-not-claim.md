---
title: "What a termination record must not claim"
rfc: "RFC-048"
rfc_file: "../../accepted/048-agentrun-termination-records.md"
source_rfc_status: "Accepted 2026-09-16 — M12"
target_milestone: "M12"
created: "2026-09-16"
---

# What a termination record must not claim

**Required reading before writing code.** A durable record outlives the session that wrote it. The
failure that matters here is not a missing record — it is a record that states an ending nobody
observed.

## §1 A run Tekstide stopped supervising has no ending it can state

`OrphanedUnknown` means the runtime lost track of the process. **Write nothing.** Not `Terminated`
with a hedged reason, not a `Failed` phase, not an anomaly. The existing test
`orphaned_runtime_truth_is_not_mislabeled_as_durable_termination` holds this line for plain
terminals, and `record_plain_terminal_terminated` returns `NotRequired` for exactly this case.

**The trail of a detached run therefore ends at `Started`, forever.** That is the honest shape, and
it is disclosed in the changelog and the book (D2) rather than left for a reader to discover.

## §2 Nothing from the outcome's payload reaches the record

`Failed` and `OrphanedUnknown` carry a `BoundedRuntimeSummary`, which is runtime text and **can
contain paths**. `Exited` carries an exit status; the killed variants carry signal numbers. **None of
it goes in.** The reason code is the whole of the detail — `ProcessExited`, `ProcessTerminated`, or
`RuntimeFailure`.

Whether the run *succeeded* is `AgentRunStatus`'s answer. The trail says the process ended and how it
was ended, which is what RFC-013 lets it say.

## §3 The record is never a precondition

`append_observation`, best-effort. **A run must never fail to be terminated because the store could
not be written** (RFC-046 D1, RFC-047 D4). A degraded store makes an ending unrecorded, not a
termination refused.

## §4 One path, or it will be forgotten

This project has twice shipped a correct decision with one call site unguarded — the adapter launch
site (response 390) and the command-line project open (response 397). Both were fixed by removing the
choice, not by adding a test.

**The coordinator applies the outcome and records it**, and production stops calling
`ProjectSession::apply_agent_terminal_outcome` directly. If a reviewer can delete the recording call
and see every test pass, the wiring is wrong.
