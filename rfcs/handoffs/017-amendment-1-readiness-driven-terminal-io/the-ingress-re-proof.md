---
title: "RFC-017 Amendment 1 — The ingress re-proof: implementation handoff"
rfc: "RFC-017 Amendment 1"
rfc_file: "../../done/017-terminal-renderer-and-immersion-mode.md"
status: "Required reading before any code"
created: "2026-08-15"
---

# The proofs you are about to invalidate

## Why this document exists

PR-017-B and PR-017-C enumerated and ablated four properties against the terminal ingress
path. Those proofs are green today and will still be green after you change the path,
**because they were written against the shape you are replacing.** A passing test suite
after this change is not evidence that the properties still hold; it is evidence that the
tests still compile.

This is the highest-risk work in `0.8.0` and none of the risk is in the performance.

## P1 — single ingress

**What it says:** exactly one production path writes bytes into the emulator, and it goes
through the security filter.

**Why a channel threatens it:** you are adding a reader thread that produces bytes and a
channel that carries them. That is a new producer and a new consumer. The natural way to
write it — reader thread reads PTY, pushes to channel, UI drains channel into the pane —
introduces a second place where bytes can enter the emulator. If any path drains that
channel *without* going through the filter, P1 is dead and every test still passes,
because the tests enumerate the *old* call sites.

**Required:** re-enumerate. Name every production write site into the emulator under the
new design. The enumeration test must fail *by name* when a new one appears. Do not amend
the existing enumeration to accommodate the new path without first checking whether the new
path belongs there.

**Ablate it:** add a second, filter-bypassing write path deliberately, confirm the
enumeration catches it by name, remove it.

## P2 — no side channels

**What it says:** no path carries terminal bytes anywhere except the one ingress.

**Why a channel threatens it:** a channel is a side channel by construction. The question
is whether anything reads from it other than the sanctioned consumer — a debug log, a
metrics counter that samples content, a test hook left in production, a second receiver
clone.

**Required:** enumerate every receiver/consumer of the new channel, and prove the count is
one. If your channel type permits multiple receivers, say why that cannot be exercised, or
pick a type that cannot express it. **Prefer a type that makes a second consumer
unrepresentable over a test that checks there is not one** — that is the parse-don't-
validate discipline this project uses everywhere else (`DisplayText`, `VerifiedCwd`,
`paste_bytes_within_bound`).

**Ablate it:** add a second consumer, confirm the property fails.

## P3 and modal exclusivity — the one that fails silently

**What it says today:** while a modal is open, terminal input is *not produced* —
`SubscriptionMode::for_modal` stops the subscription, and an `is_none()` guard at the write
site is defence in depth.

**Why this change threatens it specifically:** that guarantee currently rests on the
subscription being the only thing that runs. **A reader thread does not stop when the
subscription stops.** It will keep reading the PTY and keep filling the channel while a
paste-confirmation dialog is open, and if anything drains it, bytes reach the emulator
during a modal — which is exactly what RFC-018's whole trusted-UI argument assumes cannot
happen.

Note the asymmetry, because it is easy to get half right: **output reaching the grid while
a modal is open is not automatically wrong** — a terminal continuing to render output
behind a dialog is normal. What must not happen is *input* production, and what must not
break is the guard's assumption about what runs when.

**Required:** state explicitly how the new design preserves modal exclusivity, and which
mechanism now carries it. If the answer is "the reader thread keeps running but nothing
drains it during a modal," say so and prove the not-draining. If it is "the reader thread
pauses," say how it resumes without losing bytes — a pause that drops is the dropping this
amendment forbids.

**Ablate it:** with a modal open, confirm no input reaches the PTY, under a live positive
control proving keystrokes were reaching the app (PR-018-E's methodology — a `Tab` that
visibly moves the focus marker in the same capture).

## P4 — why it still covers you, and the one way to lose that

**P4 covers chunking where every byte arrives.** Backpressure keeps that true: bytes are
delayed, never discarded, so the existing proof continues to apply unchanged. **This is the
main reason backpressure was chosen over drop-with-a-count**, not a happy side effect.

**You lose it the moment anything drops.** A `try_send` that discards on a full channel, a
bounded queue that evicts oldest, a reader that skips ahead after a stall — any of these
silently converts the system into one P4 has never covered, and the failure mode is a hole
mid-escape-sequence with the parser consuming later output as that sequence's parameters.

**Required:** `dropped_bytes` becomes **unreachable**, proven by enumeration rather than
asserted. Grep-level proof that no code path can produce a non-zero count. If the type can
still express a drop, that is a finding worth raising, not a comment worth writing.

## The order to build this in

1. The reader thread and channel, with `dropped_bytes` unreachable, **before** removing the
   poll tick — so the two designs can be compared against the same tests.
2. The P1/P2 re-enumeration, against the new shape.
3. Modal exclusivity, with its positive control.
4. Remove the tick and the sleep.
5. Measure `NFR-PERF-004`, then the terminal-count limit.

**Do not measure the limit before the tick is gone.** The current `Some(3)` is a function
of the ~10.1 ms/pane cost this change removes; measuring against the old cost would
reproduce the old answer.
