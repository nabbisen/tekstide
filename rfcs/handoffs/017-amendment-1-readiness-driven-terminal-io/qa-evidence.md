---
title: "RFC-017 Amendment 1: Readiness-driven terminal I/O - QA Evidence"
rfc: "RFC-017 Amendment 1"
rfc_file: "../../done/017-terminal-renderer-and-immersion-mode.md"
status: "PR-A1-A implemented 2026-08-15 (commit 79d9c23), not yet reviewed — B, C, D not started"
target_milestone: "M9 (carried), shipping in 0.8.0"
created: "2026-08-15"
---

# QA Evidence

**This file holds results. The obligations live in
[`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md).** Four obligations in this
project have been lost to that gap. If a slice discovers a new obligation, put it in the
task breakdown, not only here.

## Conventions

- **Ablation**: break the property, watch the *specific* named test fail, restore. **A
  green ablation is a defect in the ablation, not a pass.**
- **Positive control**: prove the check reaches real data before asserting what it does
  not find.
- **Measurement**: measure bounds, never estimate them. Two estimated figures here were
  wrong once measured, and a third measured the wrong quantity.
- **GUI evidence**: `niri msg action screenshot-window`; `env -u WAYLAND_DISPLAY`,
  `xdotool windowfocus`, always `--clearmodifiers`. One window geometry per comparison.

## Baseline figures this amendment replaces

Recorded here so the after-figures have something to be compared against:

- Poll tick: **50 ms**, contributing an expected p95 near **47.5 ms** against a 16 ms budget.
- `poll()` cost: **~10.3 ms** against the 50 ms period (21% duty) — not saturating.
- Throughput ceiling: **~374 KB/s** measured, against a reader sustaining ~69 MB/s while
  actually reading.
- Per-pane poll cost: **~10.1 ms**, measured linear, saturating at 5 panes — which is why
  `terminal_session_limit` is `Some(3)`.
- `dropped_bytes`: always `0` today, **only because the sleep starves the reader** —
  ~18.7 KB accumulates per poll against a 64 KiB cap.

## PR-A1-A — The reader thread and bounded channel

**Implemented 2026-08-15 (commit `79d9c23`), not yet reviewed.** New module
`crates/tekstide-core/src/runtime/terminal/reader.rs`, built alongside
`read_available_bounded_for` (untouched) per the pack's own sequencing — nothing in
`crates/tekstide` consumes this yet.

**Mechanism, stated and shown**: the reader thread blocks on `libc::poll(2)` with an
infinite timeout on the PTY master's fd — a real kernel-level park, not a fixed delay.
`reader_thread_does_not_busy_wait_while_idle` measures the thread's own CPU ticks
(`/proc/self/task/<tid>/stat`, `utime + stime`) across a 300ms idle window against a real
PTY and asserts the delta is ≤2 clock ticks (≤20ms of CPU), rather than trusting the
mechanism's description.

**Bounded, and `dropped_bytes` is structurally unreachable, not asserted.** The channel is
`mpsc::sync_channel(8)` (~512 KiB at the 64 KiB per-message chunk size); `SyncSender::send`
blocks the reader thread when full — there is no `try_send`, no truncation arithmetic, and
no dropped-bytes field anywhere in the type. `Receiver<Vec<u8>>` is not `Clone`, so a second
consumer is unrepresentable by the type itself (P2's own preferred discipline, ahead of
schedule — full P2 re-enumeration is still PR-A1-B's job).

**Backpressure, demonstrated end to end against a real stall**:
`backpressure_stalls_the_producer_and_resumes_with_no_byte_loss_across_the_stall` writes a
real `dd | tr` pipeline producing 2 MiB (well over the ~512 KiB channel bound) into a real
PTY, does not drain for 300ms, confirms the real completion marker has not appeared (proving
the producer is still stalled on `write()`), then drains to completion and asserts the
extracted payload is **exactly** 2,097,152 bytes of the fill byte — no loss, no
duplication, across a stall it deliberately created.

**Ablated for real.** Reverting the blocking `send` to a drop-on-full `try_send` and
re-running the same test: only **4,097 of 2,097,152** payload bytes survived (a
2,093,055-byte deficit, ~99.8% loss) — the exact wrong value, not just "it failed". Reverted
before commit.

**The UI thread never blocks, shown under real load.**
`drain_available_never_blocks_the_caller_even_under_sustained_production` floods a real PTY
continuously, calls `drain_available()` 200 times while the flood is running, and asserts
the slowest individual call took under 20ms — measuring the call's actual wall time rather
than citing `mpsc::Receiver::try_recv`'s documented non-blocking contract as sufficient
proof on its own.

**Two real bugs found and fixed by this slice's own tests, before commit, both disclosed in
the commit message rather than folded silently into a clean-looking diff**:

1. **A `Drop` ordering bug that could deadlock.** A custom `Drop::drop` body runs *before*
   Rust's automatic per-field drops, not after — the first version of `Drop for
   TerminalReader` assumed the opposite ("drop `receiver` first, by field order") and joined
   the reader thread while `receiver` was still alive. If the channel was full and nothing
   was draining it (exactly the state the backpressure test deliberately creates), the
   reader thread is blocked inside `send`, and joining while its matching `receiver` is
   still alive waits for a send that can now neither succeed nor fail — a real deadlock.
   Found because a test that panicked mid-drain then hung forever instead of reporting its
   failure, rather than by inspection. Fixed by wrapping `receiver` in an `Option` and
   explicitly `take()`-ing it inside `drop()` before joining.
2. **Two test-methodology bugs in the backpressure fixture, not in production code**, found
   in sequence:
   - A naive substring search for a plain `END` completion marker matched the shell's own
     local echo of the *unevaluated command line* (which contains the literal characters
     `printf '\nEND\n'` as source text) before the command had even run, making the test
     falsely believe the producer had finished almost instantly.
   - After scoping the search to a real newline-bounded `\nEND`, the fix still didn't work:
     `ONLCR` (on by default) translates outgoing LF to CRLF, so the real bytes are
     `START\r\n` and `\r\nEND\r\n`, never a bare `\n`-only newline — a marker search using
     `b"\n"` never matches genuine output either. Fixed by matching the real `\r\n`-bounded
     markers throughout.
   The first (correct, real) failure of the fixed test showed the payload flowing through
   correctly at a measured ~104 KiB after 300ms undrained — consistent with the channel's
   ~512 KiB bound plus kernel PTY/pipe slack, not the full 2 MiB — before the marker bugs
   were traced back to test methodology rather than the reader itself.

**A pre-existing, unrelated finding, disclosed but out of this slice's scope**: the wider
`tekstide-core` test suite (547 tests, none touched by this slice) leaks real shell
processes it spawns — running them without the 4 new reader tests still leaves ~87 orphaned
`/bin/sh` processes (`PS1=tekstide$`, reparented to `systemd --user`) after the run. Not
introduced by this slice (the 4 new reader tests leak zero processes in isolation, confirmed
across multiple runs) and not fixed here — flagged for whoever owns general test hygiene,
since it was noticed only while diagnosing an unrelated timing issue in this slice's own
tests.

**Gates**: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features
-- -D warnings`, full workspace suite (`tekstide-core` 551 passed, up from 547 — the 4 new
reader tests; `tekstide` 206 passed, unchanged — no `crates/tekstide` changes), `git diff
--check`. All clean.

**Not done in this checkpoint**: the P1/P2 re-enumeration against the new shape, modal
exclusivity, and wiring this reader into any production consumer — all PR-A1-B's job, not
started.

## PR-A1-B — The ingress re-proof

*Not started.*

## PR-A1-C — Remove the tick and the sleep

*Not started.*

## PR-A1-D — Measurement and closeout

*Not started.*
