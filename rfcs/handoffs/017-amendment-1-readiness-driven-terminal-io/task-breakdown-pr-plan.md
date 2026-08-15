---
title: "RFC-017 Amendment 1: Readiness-driven terminal I/O — Task Breakdown / PR Plan"
rfc: "RFC-017 Amendment 1"
rfc_file: "../../done/017-terminal-renderer-and-immersion-mode.md"
status: "PR-A1-A closed 2026-08-15 (responses 201/202, commits 79d9c23/85dcbef) — B, C, D not started"
target_milestone: "M9 (carried), shipping in 0.8.0"
created: "2026-08-15"
---

# Task Breakdown

Four slices. **[`the-ingress-re-proof.md`](./the-ingress-re-proof.md) is required reading
before any of them.**

## PR-A1-A — The reader thread and the bounded channel

Build the new path **alongside** the poll tick, not in place of it. Both designs then run
against the same tests, and the comparison is real rather than remembered.

Review gate:

- A dedicated thread blocks on PTY readability; **no sleep, no busy-wait**. State the
  mechanism used.
- The channel is **bounded**; a full channel stops the reader rather than dropping.
- **`dropped_bytes` is unreachable, proven by enumeration** — no code path can produce a
  non-zero count. Not a comment.
- **The UI thread never blocks.** Show it, do not assert it; a blocking call reachable from
  the update thread is the defect this whole amendment exists to remove.
- Backpressure demonstrated end to end: a producer faster than the consumer **stalls on
  `write()`** and resumes correctly, with no byte loss across the stall.

**Implemented 2026-08-15 (commit `79d9c23`), reviewed (response 201).** Every gate item above
is met; full detail and figures in `qa-evidence.md`. Two real bugs were found and fixed by
this slice's own tests before commit — a `Drop`-ordering deadlock, and two test-methodology
bugs in the backpressure fixture (a false-positive marker match against the shell's own
echoed, unevaluated command text; `ONLCR`'s LF→CRLF translation) — both disclosed in the
commit message and in `qa-evidence.md` rather than folded quietly into a clean diff. Nothing
in `crates/tekstide` changed; nothing drains this reader in production yet.

**Response 201 accepted PR-A1-A with one required fix — `Drop` could still block forever.**
Dropping `receiver` only unblocks a thread parked in `sender.send` on a full channel; it does
nothing for a thread parked in `poll(2)` on a live, silent child producing no output — the
common case, not a corner case, once a real caller drops a reader for an idle terminal. Fixed
(commit `85dcbef`) with a shutdown `eventfd` added to the `poll(2)` set; `Drop` writes to it
before dropping `receiver` and joining, so both unblock paths are now independent of the
child's behaviour. Proven by
`dropping_a_reader_over_a_live_silent_child_completes_promptly`, which waits on the drop with
a real 5-second timeout so a regression fails that test rather than hanging the suite;
ablated by removing the `eventfd` write and confirming the test fails cleanly at its own
timeout rather than hanging.

**PR-A1-A closed 2026-08-15 (response 202, commit `85dcbef`).** Response 201 also noted the
shutdown `eventfd` is itself a second channel this module owns, which **P2's re-enumeration
in PR-A1-B must account for** — carried forward into PR-A1-B's own gate below. Response 202
additionally flagged **modal exclusivity** as the item to be most careful about in PR-A1-B,
since it is the one that fails silently (the reader thread does not stop when the
subscription stops).

## PR-A1-B — The ingress re-proof

The security work. Do not fold it into A.

Review gate:

- **P1 re-enumerated** against the new shape; a new production write site fails the test by
  name. Ablated with a deliberate filter-bypassing path.
- **P2 re-enumerated**: exactly one consumer of the channel, preferably made
  unrepresentable by the type rather than checked by a test. Ablated with a second consumer.
- **Modal exclusivity stated and proven** — which mechanism now carries it, and a live
  positive control (a `Tab` visibly moving the focus marker in the same capture) proving
  keystrokes reached the app while none reached the PTY.
- The output-vs-input asymmetry addressed explicitly: output rendering behind a modal is
  acceptable; input production is not.

## PR-A1-C — Remove the tick and the sleep

Only after B is accepted. Removing the old path before the new one is proven would leave a
window with no reviewed ingress at all.

Review gate:

- `terminal_demo_subscription`'s 50 ms tick removed; no polling path remains.
- The 10 ms `WouldBlock` sleep removed from `read_available_bounded_for`.
- The 64 KiB truncation behaviour **gone**, not merely unreached — if a truncation path
  still exists, the fix is incomplete.
- `TerminalOutputSummary`'s `dropped_bytes` either removed or proven unreachable; if it
  survives, say why and what now guarantees it stays zero.
- Full suite green with no test amended to accommodate the removal. **A test that had to be
  changed to keep passing is a finding** — report it rather than changing it.

## PR-A1-D — Measurement and closeout

Review gate:

- **`NFR-PERF-004` measured**, not inferred from the mechanism being better. Non-contamination
  proven per criterion. **Never reintroduce `iced::window::frames()`.**
- If it is still not met, **that is the honest outcome** and is recorded as such. A second
  evidenced "not met" is worth more than an unevidenced "met".
- **Throughput re-measured** against the ~374 KB/s figure this replaces.
- **`terminal_session_limit` raised from a new measurement**, taken after the tick is gone.
  State the new per-pane cost and the number it justifies. Raising it by assumption
  reopens the saturation risk the default exists to prevent.
- Claim statement checked **against the amendment's own text**, not only the evidence file.
- `rfcs/future-work.md`'s §Readiness-driven terminal I/O entry updated to record the
  outcome, in the same commit.

## Sequencing

```
A ─→ B ─→ C ─→ D
```

**B before C is not negotiable.** C removes the reviewed ingress; B is what makes the
replacement reviewed. Doing C first leaves a period where the only ingress into the
emulator has no current proof behind it.

## What this hands forward

- The new per-pane cost, since `terminal_session_limit` is a function of it and will be
  revisited again.
- Whether `NFR-PERF-004` is met, since it has now been recorded unmet twice.
- The shape of the re-proved P1/P2 enumeration, since the adapter-spawn pathway (M11's
  priority) will add another producer near this path.
