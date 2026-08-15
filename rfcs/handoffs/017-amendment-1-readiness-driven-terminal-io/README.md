---
title: "RFC-017 Amendment 1: Readiness-driven terminal I/O — handoff pack"
rfc: "RFC-017 Amendment 1"
rfc_file: "../../done/017-terminal-renderer-and-immersion-mode.md"
status: "Ready for implementation — authorised by the owner 2026-08-15"
target_milestone: "M9 (carried), shipping in 0.8.0"
created: "2026-08-15"
---

# Start here

This is `0.8.0`'s spine. It replaces the 50 ms poll tick with readiness-driven I/O, fixes
two coupled defects that must be fixed together, and discharges `NFR-PERF-004` one way or
the other.

**It is the only work currently scheduled that improves something a user can actually
reach.** Terminals launch with `Ctrl+Alt+T` today. RFC-020's surfaces, RFC-021's command
approval and RFC-024's diff content are all correct, reviewed, and unreachable — see
[`../../delivery-plan.md`](../../delivery-plan.md) §Re-plan for why that pile is now the
project's dominant risk, and why this slice is not part of it.

## Reading order

1. **[`the-ingress-re-proof.md`](./the-ingress-re-proof.md)** — required before any code.
   The new reader is a **new ingress path**, and P1/P2's existing proofs say nothing about
   it. This is where the risk is.
2. [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) — the slices and their gates.
3. [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) — tick at closeout.
4. [`qa-evidence.md`](./qa-evidence.md) — record results as you go.

The amendment itself (RFC-017 §Amendment 1) is the reasoning behind all of it, including
the measured figures. Read it first if you want the *why*; this pack is the *how*.

## What is being fixed, and why it is one change

`read_available_bounded_for` (`crates/tekstide-core/src/runtime/terminal/launch.rs:147-150`)
has two defects:

1. A hardcoded **10 ms `WouldBlock` sleep** against a caller-supplied 5 ms bound, run
   synchronously on `iced`'s update thread. Caps throughput at ~**374 KB/s** measured,
   against a reader that sustains ~69 MB/s while actually reading.
2. A **64 KiB per-poll cap that truncates mid-read**, discards the remainder, and keeps
   reading — feeding the emulator a stream **with a hole in it** — while
   `TerminalPane::poll()` discards the `TerminalOutputSummary` carrying `dropped_bytes`.

**`dropped_bytes` is zero today only because the sleep starves the reader** (~18.7 KB per
poll against a 64 KiB cap). Fix the sleep alone and a 5 ms window offers ~104 KB, and the
truncation goes live. **Fixing the sleep in isolation trades a throughput cap for a
stream-corruption bug.** That is why this is one change.

## The decision that shapes everything: backpressure

A dedicated reader thread blocks on PTY readability and pushes into a **bounded** channel.
When the channel is full the reader stops reading, the PTY buffer fills, and the child
blocks on `write()`. The UI thread never blocks, because the blocking lives on the reader
thread — which is the point of moving off the tick.

**The owner authorised the trade explicitly**: a process producing output faster than the
terminal renders it will stall rather than have its output silently thinned. A stalled
process is more honest than a corrupted display.

**Why this is not just the nicest of three options**, and the thing to keep in mind while
building it:

- **P4 (stream-position independence) covers chunking where every byte arrives.** It does
  not cover *dropped* bytes. A hole landing mid-escape-sequence leaves the parser consuming
  later output as that sequence's parameters.
- Choosing drop-with-a-count would mean establishing a whole new property, with its own
  enumeration and ablation, for a failure mode with no bound on how wrong the rendering
  gets.
- **Backpressure makes dropping structurally impossible, so P4's existing proof keeps
  covering the system unchanged.**

Growing the buffer was rejected outright: unbounded buffering against a verbose or hostile
producer is a memory-exhaustion path.

So: **`dropped_bytes` must become unreachable, proven by enumeration, not asserted in a
comment.** If you cannot make it unreachable, stop and raise it — a silently discarded
`TerminalOutputSummary` is how this defect survived in the first place.

## What must not regress

- **P1 (single ingress) and P2 (no side channels)** — re-enumerated and re-ablated, not
  assumed. See the ingress document.
- **Modal exclusivity.** `SubscriptionMode::for_modal` plus the `is_none()` guard currently
  rely on the subscription not producing input while a modal is open. A reader thread that
  keeps pushing regardless would defeat that at the source.
- **The security filter's classification.** Untouched. Only what feeds it changes.

## Out of scope

- Any rendering or UX change; no new surface, no new keybinding.
- Windows/macOS readiness primitives. Linux only.
- Anything in RFC-020's blocked surfaces.
