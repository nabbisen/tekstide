---
title: "RFC-011 Amendment 2: Re-homing transcript capture - Acceptance / QA Checklist"
rfc: "RFC-011 Amendment 2"
rfc_file: "../../done/011-transcript-retention-and-local-data-policy.md"
status: "Open"
target_milestone: "M11 prerequisite"
created: "2026-08-15"
---

# Acceptance / QA Checklist

Every unchecked line at closeout carries a stated reason.

## Capture and ordering (PR-A2-A)

- [ ] The writer lives in the reader thread; no file I/O on the UI thread.
- [ ] Transcript byte-identical to PTY output, against a real child process.
- [ ] **D2 proven by ordering** — the record contains bytes the consumer has not drained.
- [ ] D2 ablated: write moved after send, specific test fails.
- [ ] `transcript_write_summary`'s replacement decided, named, and not silently `None`.
- [ ] An enumeration names the production transcript-write call site.
- [ ] P1/P2 re-run against the new shape, not assumed to transfer.

## Failure policy (PR-A2-B)

- [ ] Failure induced by a **real** unwritable transcript, not an injected error.
- [ ] `LocalBounded`: `CaptureFailed`, stops writing, keeps reading.
- [ ] `RequiredLocalBounded`: `CaptureFailed`, stops reading, child stalls rather than dies.
- [ ] Normal termination still works after a `RequiredLocalBounded` stall.
- [ ] Both modes exercised separately.
- [ ] `CaptureFailed` reused, not reinvented.
- [ ] The failure is observable, and where it surfaces is stated.
- [ ] Marking ablated; what silently succeeds is reported.

## Honesty (PR-A2-C)

- [ ] Claim statement checked against the amendment's own text.
- [ ] No claim that adapter-spawn is unblocked beyond this one prerequisite.
- [ ] No claim that capture is exercised in production — nothing creates an `AgentRun`.
- [ ] D4's disk coupling stated as shipped behaviour.
- [ ] `future-work.md`'s blocking entry updated in the same commit.
- [ ] Every unchecked line above carries a stated reason.

## Final Acceptance Decision

- [ ] Accepted
- [ ] Accepted with required follow-up
- [ ] Rejected

Reviewer notes:
