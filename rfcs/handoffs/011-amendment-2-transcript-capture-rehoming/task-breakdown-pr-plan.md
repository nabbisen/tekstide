---
title: "RFC-011 Amendment 2: Re-homing transcript capture — Task Breakdown / PR Plan"
rfc: "RFC-011 Amendment 2"
rfc_file: "../../done/011-transcript-retention-and-local-data-policy.md"
status: "Ready for implementation"
target_milestone: "M11 prerequisite"
created: "2026-08-15"
---

# Task Breakdown

Three slices. **[`the-ordering-and-the-failure.md`](./the-ordering-and-the-failure.md) is
required reading before any of them.**

## PR-A2-A — Capture in the reader thread, and the ordering [Implemented — pending review]

Move the writer into `TerminalReader`, write before send, keep the happy path correct.

See [`qa-evidence.md`](./qa-evidence.md#pr-a2-a---capture-in-the-reader-thread-and-the-ordering)
for the full evidence record.

Review gate:

- **The writer lives in the reader thread**, not in `poll()` and not on the UI thread.
- **A transcript written through the new path is byte-identical to the PTY output**, proven
  against a real child process, not a synthesised stream.
- **D2 proven by ordering**: a test observes the transcript file already contains bytes the
  consumer has **not yet drained**. Ablated by moving the write after the send.
- **`transcript_write_summary`'s replacement decided and named.** It currently reads a
  writer the runtime owns; once the writer moves, it must not silently return `None` and
  look like "no transcript configured."
- **An enumeration names the production transcript-write call site**, so a future
  disappearance fails a test rather than going unnoticed for two months. This is the defect
  that created this amendment.
- **P1/P2 re-run**, not assumed to transfer. The writer is a new consumer of the byte
  stream inside the thread; prove it is not a second path out of it.

## PR-A2-B — The failure policy

Both capture modes, against real failures.

Review gate:

- **A genuinely unwritable transcript** — revoked permissions or a full filesystem — not an
  injected error value.
- **`LocalBounded`**: marks `CaptureFailed`, stops writing, **keeps reading**; the terminal
  stays usable.
- **`RequiredLocalBounded`**: marks `CaptureFailed`, **stops reading**; the child stalls on
  `write()` rather than being killed, and normal termination still works afterwards.
- **Both modes exercised separately.** Covering only `LocalBounded` proves the easier half.
- **`CaptureFailed` is the existing state** (`domain/transcript.rs:93`), not a new one.
- **The failure is observable, and where it surfaces is stated.** If nothing can currently
  observe it, say so rather than leaving a state that is set and never read.
- **Ablate the marking**: remove it, show what silently succeeds. If nothing observable
  changes, that is a finding about observability.

## PR-A2-C — Closeout

Review gate:

- Claim statement checked **against RFC-011 Amendment 2's own text**, not only the evidence
  file.
- **No claim that adapter-spawn is unblocked beyond this prerequisite** — other work may
  remain; this closes one named blocker.
- **No claim that transcript capture is exercised in production.** Nothing creates an
  `AgentRun` yet; this is correct-before-reachable, deliberately.
- D4's disk coupling stated as shipped behaviour, not omitted because it is unwelcome.
- `rfcs/future-work.md`'s blocking-prerequisite entry updated in the same commit to record
  that it is discharged.

## Sequencing

```
A ─→ B ─→ C
```

**A before B** because a failure policy cannot be tested against a path that does not yet
capture. Do not fold them: A is the mechanism, B is the decision, and bundling them makes
the ablations ambiguous about which property failed.

## What this hands forward

- Where the transcript write happens and what enumerates it, since adapter-spawn will add
  the first real producer next to it.
- The `transcript_write_summary` mechanism chosen in A.
- Whether `RequiredLocalBounded`'s stall behaves acceptably in practice — the first real
  data point on backpressure used as a safety mechanism.
