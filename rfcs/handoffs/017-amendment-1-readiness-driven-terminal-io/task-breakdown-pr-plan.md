---
title: "RFC-017 Amendment 1: Readiness-driven terminal I/O — Task Breakdown / PR Plan"
rfc: "RFC-017 Amendment 1"
rfc_file: "../../done/017-terminal-renderer-and-immersion-mode.md"
status: "PR-A1-A and PR-A1-B closed 2026-08-15 (responses 201/202/203/204, commits 79d9c23/85dcbef/9f098ba/e35d690) — C, D not started"
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

**Implemented 2026-08-15, reviewed and accepted (responses 203/204).** Every gate item above is met; full
detail, figures, and screenshot references in `qa-evidence.md`. `TerminalPane` now owns a
`TerminalReader` and `poll()` drains it instead of calling
`runtime.read_available_bounded_for` — the old path and the 50ms tick are both still
present, deliberately not removed (PR-A1-C's job). Modal exclusivity re-checked against two
existing, unmodified headless tests plus a fresh live GUI capture (`evidence/pr-a1-b/`)
using the `ExternalChangeModal` rather than the paste-confirmation dialog PR-018-E used,
since this session's `iced` clipboard integration did not deliver real clipboard content
under synthetic input — disclosed in `qa-evidence.md`, not hidden. One secondary positive
control (input works again immediately after the modal closes) was attempted and not
cleanly captured, due to a test-fixture quirk rather than an application defect; disclosed
rather than omitted or forced.

**Response 203 accepted PR-A1-B with one required tightening, applied same day (commit
`e35d690`)**: the original P1/P2 enumerations
(`only_one_call_site_ever_advances_a_terminal_processor_in_the_crate`,
`only_this_field_drains_a_terminalreader_in_the_crate`) collected *files* containing the
target substring rather than counting *occurrences*, so a second `.advance(`/
`.drain_available(` call added inside `surface/terminal.rs` itself — the single most likely
real regression, not a hypothetical — would have passed both tests silently. Fixed: both now
count total occurrences via a shared `count_occurrences_in_crate` helper, asserting the
count is exactly 1. Ablated twice each (a throwaway file, and — the case response 203
specifically required — a second occurrence added inside `surface/terminal.rs` itself),
both confirmed to now fail on total count. Also added, per response 203: the `.advance(`
test's own doc comment now states what the scan does not cover (a direct mutation of
`self.term` through a different `alacritty_terminal` entry point, not caught by this seam).

**PR-A1-B closed 2026-08-15 (response 204, commit `e35d690`).** Response 204's own reminders
for PR-A1-C, in the order most likely to bite: (1) a test that had to be changed to keep
passing is a finding to report, not a change to make; (2) the 64 KiB truncation must be
gone, not merely unreached; (3) `dropped_bytes_total` and its field are dead state since
PR-A1-B — C is where they die, or where the guarantee that keeps them at zero is stated.

## PR-A1-C — Remove the tick and the sleep

**Closed 2026-08-15 (responses 205/206, commits `19dfc36`/`564cbc9`).** Two questions raised
before writing code, both answered by review rather than guessed: response 205 picked Option
B (a dedicated wake `eventfd`, not raw bytes through `Message`) for the redraw trigger the
tick removal leaves with no replacement; response 206 corrected the gate below after finding
`read_available_bounded_for` is the workspace's only transcript-write code path, not a dead
read loop — see `qa-evidence.md`'s PR-A1-C section for the full reasoning on both.

Gate, as actually discharged:

- `terminal_poll_subscription`'s 50 ms tick removed; no polling path remains — replaced by
  one `Subscription` per pane (`terminal_wake_subscriptions`), each driven by a real `poll(2)`
  wait on its own wake `eventfd`, proven not to respawn its bridging thread across rebuilds
  (`terminal_bridge_thread_count_is_stable_across_many_view_rebuilds`).
- The 10 ms `WouldBlock` sleep in `read_available_bounded_for` **stays** — response 206:
  removing it in isolation makes the loop busy-spin for the rest of its `duration` on every
  `WouldBlock`, and `dropped_bytes` staying zero in that function already depends on the sleep
  starving the reader; the two are coupled, not independently removable.
- The 64 KiB truncation behaviour **stays**, scope-corrected: `read_available_bounded_for` is
  the sole transcript-capture code in the workspace (the only non-test
  `.append(`/`.flush(` calls on a `BoundedTranscriptWriter`), used by nothing in
  `crates/tekstide` and by three `tekstide-core` test suites — one of which
  (`agent::tests`) tests the truncation itself as a real, load-bearing property (bounded
  memory for a live read, full fidelity in the transcript file). The original gate text
  ("truncation gone, not merely unreached") was written against a model of this function that
  didn't account for its transcript-writing half; read literally it would have deleted
  transcript capture as a side effect of a performance amendment. Corrected scope: no
  polling/sleeping/truncating path remains **on the terminal-pane ingress** — already
  satisfied, since this function has zero production callers anywhere in the workspace.
- `TerminalOutputSummary::dropped_bytes` **stays**, live for `read_available_bounded_for`.
  `TerminalPane::dropped_bytes_total` — the GUI-side field, unrelated to the runtime-level
  one above — **is removed**: nothing has incremented it since PR-A1-B, so unlike the
  runtime field it had no live producer left.
- Full suite green (`tekstide-core` 555, up from 552; `tekstide` 211, up from 208), no test
  changed to keep passing — every touched test is either new or moved from
  `Message::TerminalPollTick` to `Message::TerminalWoke` because the message it drove no
  longer exists.
- **Found in the course of this slice, not fixed in it**: `TerminalReader` (PR-A1-A/B's
  ingress replacement) has no transcript-capture hook at all — `reader.rs` contains the
  string "transcript" zero times. Invisible today only because no production code creates an
  `AgentRun` yet, so no transcript writer is ever configured. Recorded in
  `rfcs/future-work.md` as a **blocking prerequisite on adapter-spawn** (M11), since whichever
  work wires terminal output through the reviewed `TerminalReader` path will otherwise
  silently produce empty transcripts forever. Deliberately not built here — re-homing capture
  needs its own decision about file I/O on the reader thread, mid-stream write failure, and
  interaction with backpressure; RFC-011's territory, not this amendment's.

## PR-A1-D — Measurement and closeout

**Closed 2026-08-15 (response 209, commits `56b8af3`/`6ad48f4`/`67f01b2`).** Full reasoning
and figures in `qa-evidence.md`'s own PR-A1-D section; summarized against the gate below.

- **`NFR-PERF-004` measured, not inferred.** The headless wake-cost benchmark (pure CPU, no
  GUI/GPU) proves the *structural cause* of the old "not met" verdict is gone: sub-microsecond
  typical wake-to-`poll()` cost, ~500,000 wakes/sec sustained, zero backlog. **Recorded verdict:
  structural cause removed, criterion unverified end-to-end — not met, and not claimed met.**
  A lower-bound arithmetic proof (the old tick) is sufficient to prove failure; an upper bound
  on the true end-to-end path (which includes compositor/GPU present) is required to prove
  "met," and this project's own `frames()`-avoidance means no criterion here can produce one,
  on any machine. `iced::window::frames()` was never reintroduced.
- **Three live GUI attempts, all confounded** (the same swap-pressure signature PR-017-G's
  responses 155/156 diagnosed on this shared machine), **disclosed rather than reported as
  clean numbers** — capped at three per response 209's "stop regardless of outcome." The
  `Typing`-with-three-real-panes control (p50=22µs even with genuine echo-driven wakes firing
  in the background) is promoted to a named finding: rules out a wake subscription's mere
  existence degrading unrelated input.
- **A real measurement-methodology defect found and fixed, not merely disclosed**: grid
  occurrence counting for echo detection is defeated by a real PTY canonical-mode redraw
  behaviour (traced directly: count jumped 20→41 in one wake — a full line re-echo, not a
  single new character). Replaced with per-send distinctive markers checked by substring
  presence, immune to the same failure by construction.
- **Throughput re-measured**: ~17.4-18 MB/s, matching `FLOOD_SCRIPT`'s own standalone rate —
  the replacement figure for the ~374 KB/s ceiling this slice removes, roughly 47x.
- **`terminal_session_limit` raised from a new measurement**, headless (`terminal_session_limit_headless_n_pane_wake_throughput_benchmark`),
  taken after the tick is gone, against the mechanism that actually replaced it (one
  single-threaded consumer servicing N panes' wakes, not a shared tick period). `Some(3)` →
  **`Some(6)`** — clean through N=6, degradation first measurable at N=8, unambiguous at
  N=10; 6 keeps the old default's own margin-below-first-degradation philosophy.
- **Claim statement checked against the amendment's own text** — see the updated claim
  statement below.
- **Two further real findings, out of scope to fix here, recorded in `rfcs/future-work.md`**:
  `FLOOD_SCRIPT` now drives ~250,000-500,000 wakes/sec (a **saturating** producer, not
  "bounded background output" — its meaning changed when the tick's throttling was removed,
  without anyone choosing it); and any per-wake consumer overhead beyond a bare `poll()`
  becomes a real bottleneck at that wake rate — every per-event instrument in this project was
  designed under the old tick-throttled assumption, and this slice's own `check_echo_visible`
  was not the only one that could be affected.

## Sequencing

```
A ─→ B ─→ C ─→ D
```

**B before C is not negotiable.** C removes the reviewed ingress; B is what makes the
replacement reviewed. Doing C first leaves a period where the only ingress into the
emulator has no current proof behind it.

## What this hands forward

- The new per-pane throughput/cost figures, since `terminal_session_limit` is a function of
  them and will be revisited again if the mechanism changes.
- **`NFR-PERF-004`'s status: structural cause removed, unverified end-to-end** — not "unmet
  twice." A future slice with access to a clean GUI-timing environment (or a machine-
  independent way to bound compositor/GPU present cost) is what would actually close this,
  not another attempt on this same shared machine.
- The shape of the re-proved P1/P2 enumeration, since the adapter-spawn pathway (M11's
  priority) will add another producer near this path.
- The three PR-A1-D findings recorded in `rfcs/future-work.md`: the redraw-duplication
  property of this environment's PTY echo, the instrumentation-scales-with-wake-rate risk for
  any future per-event instrument, and `FLOOD_SCRIPT`'s changed (saturating, not bounded)
  character.
