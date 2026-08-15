---
title: "RFC-011 Amendment 2 — The ordering and the failure: implementation handoff"
rfc: "RFC-011 Amendment 2"
rfc_file: "../../done/011-transcript-retention-and-local-data-policy.md"
status: "Required reading before any code"
created: "2026-08-15"
---

# Two things that are easy to implement plausibly and wrongly

## 1. Write before send, and prove it by ordering rather than by reading the code

**D2:** the transcript write happens **before** the bytes enter the channel.

The invariant it buys, stated so it can be checked: **the transcript is a superset of what
was displayed.** A crash between write and send loses nothing from the record. The reverse
ordering leaves a user having seen output the durable record does not contain, which is the
one outcome a record exists to prevent.

**Why this is easy to get wrong:** both orderings work. Both produce a complete transcript
and a complete display under every condition you will hit while developing. The difference
appears only on a crash, a kill, or a disk failure at a specific instant — which is exactly
the case the record is for.

So it cannot be verified by "the transcript looks right." It has to be verified by
**observing the ordering itself**.

**Evidence owed:** a test that observes the record already contains bytes the consumer has
**not yet drained**. Write into a real PTY, do not drain the channel, and read the
transcript file from the test — the bytes must be there. That is a direct observation of
D2, and it fails if someone later moves the write after the send.

**Ablate it:** move the write after the send, watch that test fail, restore.

## 2. The failure policy is the part with a real decision in it

**D3**, authorised as proposed. Mid-stream write failure has no prior policy in this RFC —
the analogous budget-exhaustion case is decided by failing preflight *before process start*,
which is not available once the process is running.

| Capture mode | On mid-stream write failure |
| --- | --- |
| `LocalBounded` | Mark `CaptureFailed`, stop writing, **keep reading**. The terminal stays usable; the record stopped and the user is told. |
| `RequiredLocalBounded` | Mark `CaptureFailed` and **stop reading**. Do not kill the child. |

### Why `RequiredLocalBounded` stops reading rather than terminating

Ceasing to drain applies RFC-017 Amendment 1's backpressure: the channel fills, the reader
stops, the child blocks on `write()`. It makes **no further unrecorded progress**, and it is
not killed — termination stays the caller's decision rather than a reader thread's.

This is backpressure used as a *safety* mechanism rather than a performance one. A mode
whose name says the record is required must not continue producing work that is not being
recorded; but a reader thread is the wrong place to decide a process should die.

**`CaptureFailed` already exists** (`domain/transcript.rs:93`). Do not invent a new state.

### The failure must be observable

A silent `CaptureFailed` is the same defect class as the old code discarding the
`TerminalOutputSummary` that carried `dropped_bytes` — the state was computed correctly and
thrown away, so nothing could act on it.

Say **where** the failure surfaces and **who can see it**. If nothing can currently observe
it because no surface exists, say that explicitly rather than leaving a state that is set
and never read.

### Evidence owed

- **A genuinely unwritable transcript**, not an injected error value: revoke permissions on
  the transcript file or its directory mid-run, or fill a small filesystem. RFC-019
  PR-019-D's conflict test is the shape — real file, real external condition, real
  operation, real refusal.
- **Both modes exercised**, separately. A test covering only `LocalBounded` proves the
  easier half.
- **`RequiredLocalBounded` shown to stall the child, not kill it** — the process is still
  alive and blocked, and termination still works afterwards through the normal path.
- **Ablate the `CaptureFailed` marking**: remove it and show what silently succeeds. If
  nothing observable changes, that is a finding about observability, not a passing test.

## 3. Do not "fix" D4 — it is the design, not a bug

Backpressure now includes the disk: a slow or stalled disk stalls the reader, which stalls
the child, whenever capture is on. That follows directly from D1 and D2.

Two apparent fixes are forbidden, because each breaks something load-bearing:

- **Writing after the send** — breaks D2's invariant, which is the whole point of the
  ordering.
- **Buffering writes on a separate unbounded queue** — reintroduces exactly the unbounded
  buffering RFC-017 Amendment 1's D1 rejected as a memory-exhaustion path.

If the coupling turns out to be intolerable in practice, that is a finding to raise, not a
thing to engineer around locally. The old design had the same coupling and worse — on the
UI thread.

## 4. The trap that created this work

The old path captured transcripts as a **side effect of a function named for something
else**. When that function stopped being the ingress, the capability vanished silently.

Whatever you write should be named for what it does, and its absence should be detectable
by a test rather than by someone reading a diff. **An enumeration naming the production
transcript-write call site** — the shape this project already uses for `.advance(`,
`.drain_available(` and `write_terminal_input` — is the obvious candidate, and it would
have caught the original disappearance.
