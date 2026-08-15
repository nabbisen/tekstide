---
title: "RFC-017 Amendment 1: Readiness-driven terminal I/O - QA Evidence"
rfc: "RFC-017 Amendment 1"
rfc_file: "../../done/017-terminal-renderer-and-immersion-mode.md"
status: "PR-A1-A through PR-A1-D closed 2026-08-15 — NFR-PERF-004 recorded as structural cause removed, unverified end-to-end (not met, not claimed met)"
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

**Closed 2026-08-15 — reviewed and accepted (responses 201/202, commits `79d9c23`/`85dcbef`).**
New module
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
across multiple runs) and not fixed here. **Recorded in `rfcs/future-work.md` by the
reviewer (response 201)** rather than left dependent on either of us remembering it, and
connected there to a plausible fork-window mechanism behind the RFC-021 socket flake.

**Correction (response 201): `Drop` could still block forever.** The version above proved
`drain_available()` never blocks, but not `Drop` — the gate's own wording ("a blocking call
reachable from the update thread is the defect this whole amendment exists to remove")
covers both entry points. Dropping `receiver` only unblocks a thread parked in `sender.send`
on a full channel; it does nothing for a thread parked in `poll(2)` on a live, silent child
producing no output right now — the common case once a real caller drops a reader for an
idle terminal, not a corner case. **Fixed 2026-08-15 (commit `85dcbef`)**: a shutdown
`eventfd` is added to the `poll(2)` set alongside the PTY master; `Drop` writes to it first
(wakes a `poll(2)`-parked thread regardless of PTY state), then still drops `receiver` (the
independent unblock path for a thread parked in `send`), then joins. `TerminalReader::spawn`
is now fallible (`eventfd(2)` can fail on resource exhaustion), propagated as
`TerminalRuntimeError::Io`.

**Evidence, per the required-fix instruction**:
`dropping_a_reader_over_a_live_silent_child_completes_promptly` spawns a real, live shell,
sends it no command (so the reader thread parks in `poll(2)` with nothing to read), drops the
reader on its own thread, and waits on a channel with a real 5-second `recv_timeout` — a
regression fails *this test* with a clear message rather than hanging the suite, matching how
the original `Drop` deadlock was actually found (a test that hung 30+ seconds). **Ablated for
real**: removing the `eventfd` write and re-running reproduces exactly that — the test fails
cleanly at its own 5.05s internal timeout with the expected panic message, rather than a bare
hang, confirming both the fix and the test are real. Reverted before commit.

**Gates**: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features
-- -D warnings`, full workspace suite (`tekstide-core` 552 passed, up from 547 — the 5 reader
tests now including the liveness test; `tekstide` 206 passed, unchanged — no
`crates/tekstide` changes), `git diff --check`. All clean, re-run after the fix (commit
`85dcbef`).

**Not done in this checkpoint**: the P1/P2 re-enumeration against the new shape (now also
covering the shutdown `eventfd` as a second channel, per response 201's own note), modal
exclusivity, and wiring this reader into any production consumer — all PR-A1-B's job, not
started.

## PR-A1-B — The ingress re-proof

**Closed 2026-08-15 — reviewed and accepted (responses 203/204, commits
`9f098ba`/`e35d690`).** `crates/tekstide-core` untouched; all changes
in `crates/tekstide`: `TerminalPane` now owns a `TerminalReader` (`launch()` spawns it via
`spawn_output_reader` right after `launch_project_shell`), and `poll()` reads from
`reader.drain_available()` instead of `runtime.read_available_bounded_for`. The old,
now-unused `read_available_bounded_for` and the 50ms tick subscription are both still
present in the source — deliberately not removed here, per the pack's own sequencing
("C removes the reviewed ingress; B is what makes the replacement reviewed").

**P1, re-enumerated against the new shape, not assumed from the old suite still
passing.** `only_one_call_site_ever_advances_a_terminal_processor_in_the_crate` scans every
`.rs` file under `crates/tekstide/src` (excluding test files) for occurrences of the literal
substring `.advance(`, asserting the **total count** is exactly 1, in `surface/terminal.rs`.
**Ablated for real, twice**: first (before response 203) a throwaway file elsewhere in the
crate with a second `.advance(` call; the test failed, listing the offending file. Removed.
Second (response 203's required addition): a second `.advance(` call added *inside*
`surface/terminal.rs` itself; the test failed on total count (`(2, [("surface/terminal.rs",
2)])` against the expected `(1, [("surface/terminal.rs", 1)])`) — the case a file-level check
would have missed, since it already contains the one allowed occurrence. Removed before
commit (`e35d690`).

**Correction (response 203): the original enumeration counted files, not call sites.** Both
tests originally collected files *containing* the substring and asserted the file list —
passing silently for a second occurrence added inside `surface/terminal.rs` itself, which
the reviewer named as the single most likely real regression (a resize handler, a replay
path, a fast path for large writes all naturally land in the file that already owns the
emulator). The test's own name
(`only_one_call_site_ever_advances_a_terminal_processor_in_the_crate`) claimed a stronger
guarantee than the file-level check actually gave. **Fixed**: both tests now count total
occurrences via `count_occurrences_in_crate` (shared helper), asserting the count is exactly
1 and naming which file it's in. Both proven to now catch the same-file case via the second
ablation above.

**What the `.advance(` scan does not cover, now stated in the test's own doc comment
(response 203, required)**: it proves "one use of this API," not "nothing else mutates the
emulator" — a direct manipulation of `self.term` through a different `alacritty_terminal`
entry point would be a second ingress the scan cannot see. The closest existing check is
`Term::grid_mut()` not being called anywhere in this module (the module doc's P2 note), but
that is a narrower claim.

**P2, re-enumerated against the new shape.** `TerminalReader` is not `Clone` (PR-A1-A), so a
second *owner* of the channel is already unrepresentable by the type; this enumeration
covers what the type alone cannot: a second *call site* draining the one owner this crate
has through a borrow.
`only_this_field_drains_a_terminalreader_in_the_crate` uses the same
`count_occurrences_in_crate` helper against `.drain_available(`, same total-count assertion,
same two-stage ablation (a throwaway file, then a second call inside
`surface/terminal.rs` itself, per response 203) — both confirmed and reverted. Response
201's own addition to this gate — "the shutdown `eventfd` is a second channel this module
owns" — needs no crate-side enumeration: nothing in `crates/tekstide` ever reaches it (it is
private to `tekstide-core::runtime::terminal::reader`, touched only by
`TerminalReader::spawn` and its own `Drop`), so P2's claim for this crate covers the data
channel completely and the `eventfd` is out of this crate's reach by construction, not
merely by convention.

**Modal exclusivity: unchanged, re-checked at both the state level and with a live GUI
positive control.** The mechanism is untouched by this slice — `write_terminal_input`
(`shell.rs`), the one production caller of `TerminalPane::write_input`, is still gated on
`state.modal.is_some()`, and `SubscriptionMode::for_modal` still structurally replaces the
whole non-modal input-routing subscription with `modal_subscription()` (Tab/Shift+Tab/
Enter/Escape only) whenever a modal is open — nothing about *output* changed this in any
way, since the reader thread has no write access to anything. Re-checked two ways:

1. **State level, unmodified**: `shell::tests::modal_open_blocks_pty_write_and_closing_it_resumes_delivery`
   and `shell::tests::tab_cycles_shell_focus_with_a_real_terminal_focused_and_writes_nothing`
   both pass against this slice's new reader-based `poll()` without any change to either
   test — the property holds against the new shape, not merely against the old one the
   tests were written for.
2. **A live GUI capture**, since the pack requires "a live positive control (a Tab visibly
   moving the focus marker in the same capture)" and this is the property `the-ingress-re-proof.md`
   names as "the one that fails silently." Captured 2026-08-15 against the release binary
   (`cargo build --release -p tekstide`), launched via `.git-exclude/tools/launch-scratch-gui.sh`
   against a scratch project. Real synthetic input (`xdotool windowfocus`, `--clearmodifiers`),
   screenshots via `niri msg action screenshot-window` (this session's `screenshot-path null`
   config routes captures through the clipboard, read back with `wl-paste --type image/png`
   rather than a file path). Four screenshots under `evidence/pr-a1-b/`:
   - **`00`** — a real terminal (`Ctrl+Alt+T`), running a bounded, self-terminating counter
     script for continuous, observable live output, matching PR-018-E's own convention.
   - **`01`** — a real modal open over a live document conflict (`ExternalChangeModal`,
     triggered by editing a file, modifying it externally on disk, then `Ctrl+S` — chosen
     over the paste-confirmation dialog PR-018-E used because this environment's `iced`
     clipboard integration did not return real clipboard content to the app under synthetic
     `xdotool` input despite `wl-copy`/`wl-paste` working correctly at the CLI level; not
     investigated further since `ExternalChangeModal` reaches the identical property through
     a clipboard-independent trigger and needed no code change to exercise).
   - **`02`, the primary evidence** — `xdotool type` sends a distinctive marker
     (`MODALKEYS_A1B_TEST`) intended for the terminal, then `Tab`, captured in **one
     screenshot**: the modal's own focus marker (`"> "`) has visibly moved from `Dismiss` to
     `Reload`, and the typed marker text appears nowhere in the modal body — proving the
     keystrokes reached the application (Tab moved real state) at the exact moment the
     typed text was suppressed, not merely that nothing happened.
   - **`03`** — after `Escape` dismisses the modal and the view switches back to the
     terminal, its tick count is substantially higher than before the modal sequence began
     (elapsed wall-clock time, not a frozen or broken pane) and `MODALKEYS_A1B_TEST` is
     absent from the visible transcript.

   **The output-vs-input asymmetry, addressed explicitly**: `03`'s tick-count comparison is
   evidence the *hidden* terminal pane kept polling and producing output for the whole
   modal episode (this modal opens from Content mode, so the terminal is not visually behind
   it the way the paste dialog sits over `TerminalImmersion` mode) — output continuing is
   the expected, correct behaviour per the pack's own framing, not a defect.

   **One secondary check attempted and not cleanly captured, disclosed rather than
   omitted**: PR-018-E also captured "the same stream, sent after the modal closes, reaches
   the PTY" as a belt-and-suspenders proof the pane itself was never broken. Attempted here
   too, but the test fixture's own counter script proved resistant to synthetic `SIGINT` in
   this session (traced to real, if surprising, causes — a duplicated counter invocation
   from an earlier retry, and a builtin-`sleep` shell not forking a distinctly-killable
   child) — a fixture problem, not a finding about `tekstide`. Not pursued further: the
   pack's own required evidence is the Tab-during-suppression capture (`02`), which does not
   depend on this secondary check, and this project's "avoid rabbit holes" convention favours
   disclosing a stalled secondary attempt over continuing to force it.

**Gates**: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features
-- -D warnings`, full workspace suite (`tekstide-core` 552 passed, unchanged — no core
changes this slice; `tekstide` 208 passed, up from 206 — the two new enumeration tests, same
count after response 203's tightening since no test was added or removed, only made
stronger), `git diff --check`. All clean, re-run after the tightening (commit `e35d690`).

**Not done in this checkpoint**: removing the old tick/sleep path (PR-A1-C's job, which this
slice deliberately leaves in place); measurement (PR-A1-D's job).

## PR-A1-C — Remove the tick and the sleep

**Closed 2026-08-15 — reviewed and accepted (responses 205/206, commits `19dfc36`/`564cbc9`).**
Two decisions preceded any code: a genuine design fork (response 205, "Build Option B" — a
dedicated wake `eventfd`, mirroring PR-A1-A's shutdown signal, rather than routing raw PTY
bytes through `Message`) and a scope question (response 206) about whether the gate's
"truncation gone" language reached `read_available_bounded_for` outside the terminal-pane
path. Both are recorded in full in review requests 205/206 and their responses; summarized
here as what they changed about the implementation.

**The wake mechanism (response 205, Option B, all five build constraints).** New in
`tekstide-core::runtime::terminal::reader`: a second `eventfd`, orthogonal to the existing
shutdown one, plus a `WakeNotifier` (`try_clone`, `block_until_woken`) that a caller polls on
with `poll(2)` — never a sleep. `TerminalReader::spawn` creates it, the reader thread writes
to it on every successful `send` **and** on every exit path (`stop_reading`, shared by
shutdown/EOF/error), gated by an `Arc<AtomicBool>` (`reader_alive`) so the notifier can tell
"woken because data/exit happened" from "woken because the reader is gone for good" — an
eventfd's own counter has no such distinction on its own.

1. **The wake message carries only a `TerminalId`.** `Message::TerminalWoke(TerminalId)`
   replaces `TerminalPollTick`; no bytes, no length, nothing content-derived. Response 205's
   own reasoning, not just mine: `Message` derives `Debug, Clone, PartialEq`, so a bytes-
   carrying variant would make terminal content formattable/clonable outside the one
   reviewed ingress — the inverse of RFC-024's deliberate choice to give `DiffContent`
   neither `Clone` nor `Serialize`, and the exact shape P2 exists to deny.
2. **Fires on exit, not only on a successful `send`.** The reader thread already sees
   `POLLHUP`/`read() == 0` on child exit; `stop_reading` signals the wake there too, so a
   `WakeNotifier` held by a pane that's about to close doesn't leak its bridging thread.
3. **`check_exit()` stays on the wake path.** `handle_terminal_woke` preserves the old tick
   handler's exact two-pass logic (check already-exited, then `check_exit()`/`poll()`),
   scoped to the one pane the wake names instead of iterating all panes.
4. **P2 extended to the wake `eventfd`.** Two new enumeration tests in
   `surface/terminal/tests.rs`, same `count_occurrences_in_crate` pattern PR-A1-B's response
   203 established:
   `only_one_call_site_ever_asks_a_terminalpane_for_its_wake_notifier` (`.wake_notifier(`,
   total 1, in `shell.rs`) and `only_one_call_site_ever_blocks_on_a_wake_notifier`
   (`.block_until_woken(`, total 1, in `shell.rs`). Both ablated (a redundant second call
   site added, confirmed the count-based assertion catches it) and reverted.
5. **Thread stability proven, not assumed.** `iced_futures`'s own documented dedup behaviour
   (`Recipe::stream` runs once per unique `Subscription` hash across rebuilds) is real but
   was previously unverified against this specific `Hash` impl.
   `shell::tests::terminal_bridge_thread_count_is_stable_across_many_view_rebuilds` drives
   `iced_futures::subscription::Tracker::update` — the actual runtime dedup mechanism
   `Subscription::run_with` relies on, not a description of it — across 50 rebuilds of the
   same `TerminalId`'s wake subscription, each with its own freshly `try_clone()`'d
   `WakeNotifier` exactly as a real `subscription()` rebuild would produce. Asserts exactly
   one new future is ever spawned. `iced_futures = "0.14"` added as a `[dev-dependencies]`
   entry in `crates/tekstide/Cargo.toml` to reach `Tracker`/`into_recipes` (neither is
   re-exported by the top-level `iced` crate); resolves to the version already locked
   transitively, no version conflict. **Ablated for real**: temporarily made
   `TerminalWakeSource`'s `Hash` impl include a per-call counter (simulating "iced treats
   every rebuild as a new subscription"); the test failed correctly (`left: 50, right: 1`);
   reverted.

`TerminalWakeSource`'s `Hash` impl is hand-written, not derived, and deliberately hashes only
`terminal_id` — documented at the impl site, since this is precisely what lets a fresh
`dup()`'d fd each rebuild still dedupe against the one already-running bridging thread.

`terminal_poll_subscription` and `Message::TerminalPollTick` are both gone; `subscription()`
now batches `terminal_wake_subscriptions(&state.terminal_panes)` (one `Subscription` per pane,
`filter_map`ping out any pane whose `wake_notifier()` call fails — no rendering surface exists
to report that failure on, so the pane simply stops receiving event-driven wakes, documented
as an accepted degradation rather than a silent one).

**The truncation-scope question (response 206) — Option A, with the gate corrected, not just
applied.** I asked whether "the 64 KiB truncation behaviour gone, not merely unreached" (the
original PR-A1-C gate text) reached `read_available_bounded_for`'s use *outside* the
terminal-pane path, since `agent::tests` has a real regression test that deliberately forces
`dropped_bytes > 0` to prove agent-run output capture stays memory-bounded while the
transcript file itself is never truncated. Response 206 found something neither of us had:
`read_available_bounded_for` is **not just an old read loop** — it is **the only code in the
workspace that writes to a transcript** (the sole non-test `.append(`/`.flush(` calls on a
`BoundedTranscriptWriter`), and PR-A1-A/B's `TerminalReader` replacement has no transcript
capture of any kind (`reader.rs` contains the string "transcript" zero times). Reading the
original gate literally would have deleted transcript capture as a side effect of a
performance amendment. **Resolution, corrected and recorded in `rfcs/future-work.md` as a
blocking prerequisite on adapter-spawn** (not fixed here — re-homing capture onto
`TerminalReader` needs its own design decision about file I/O on the reader thread, mid-
stream write failure, and interaction with backpressure; RFC-011's territory):

- `read_available_bounded_for`, its 10ms `WouldBlock` sleep, its truncation logic, and
  `TerminalOutputSummary::dropped_bytes` all **stay untouched**. The sleep in particular:
  removing it in isolation would make the function's `while started.elapsed() < duration`
  loop busy-spin for the full duration on every `WouldBlock`, burning a core in every test
  that still calls it — worse than the thing it would "fix," and `dropped_bytes` staying zero
  in that function depends on the sleep starving the reader, so the two are coupled, not
  independent cleanups.
- The gate is corrected to: no polling, sleeping, or truncating path remains **on the
  terminal-pane ingress** — already fully satisfied by PR-A1-A/B, since
  `read_available_bounded_for` has zero production callers anywhere in the workspace
  (confirmed by grep; its only non-test caller is its own definition).
- `TerminalPane::dropped_bytes_total` (the GUI-side field, distinct from
  `TerminalOutputSummary::dropped_bytes`) **is** removed — nothing has incremented it since
  PR-A1-B replaced the pane's ingress, so unlike the runtime-level field it has no live
  producer left anywhere.

**Gates**: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features
-- -D warnings`, full workspace suite (`tekstide-core` 555 passed, up from 552 — the three new
wake-notifier tests in `reader/tests.rs`; `tekstide` 211 passed, up from 208 — the two new
enumeration tests plus the thread-stability test), `git diff --check`. All clean. No test was
changed to keep passing — every test touched this slice is either new or, per response 205's
sharpened reading of that gate (a wholesale contract change is different from a removal
quietly threatening an existing test), the ones intentionally moved from
`Message::TerminalPollTick` to `Message::TerminalWoke` because the message they drive no
longer exists, not because their own assertions had to bend to stay green.

## PR-A1-D — Measurement and closeout

**Closed 2026-08-15 (response 209, commits `56b8af3`/`6ad48f4`/`67f01b2`).** The most
consequential finding of this slice is methodological, not numerical: measuring
"keystroke-to-echo-visible" turned out to be genuinely hard to do honestly, in three
distinct ways this section records in the order they were found.

### `NFR-PERF-004`: structural cause removed, criterion unverified end-to-end

**Not recorded as met. Not going to be.** Response 209's own framing, adopted verbatim
because it is exactly right: the original "not met" verdict (RFC-017 PR-017-G) was proven by
*arithmetic* -- the 50ms tick was the only path to the grid, so poll-wait alone put p95 near
47.5ms, a **lower bound** that a budget above it is sufficient to fail. **"Met" needs an
*upper* bound** -- evidence that nothing in the real path exceeds 16ms -- and this slice
cannot produce one, because the real path includes compositor/GPU present, which this
project's own `frames()`-avoidance (RFC-015 PR-015-F) means no criterion in this codebase can
measure on any machine, by design.

What *is* proven: the structural cause of the old "not met" is gone.
`terminal_poll_handler_cost_under_a_real_wake_driven_flood_headless_benchmark` (pure CPU, no
GUI, no GPU) measured wake-to-`poll()` cost at **p50=0µs, p99=1µs, max~400-430µs**, sustaining
~500,000 real wakes/sec with zero backlog, across repeated runs. The 10ms sleep and the 50ms
tick that produced the arithmetic floor are both gone (PR-A1-A through C). That closes the
*known* cause. It does not open a *new* proof that the whole path clears 16ms, and this file
does not claim one.

### Three live GUI attempts, all confounded, all disclosed rather than reported

`TEKSTIDE_MEASURE_CRITERION=terminal_flood`, `xdotool` at PR-017-G's own 15ms
`--repeat-delay`, release binary, scratch state, three separate attempts across this slice:

1. **First attempt** (before the marker fix below): `input` (dispatch+write) p50=33.3ms
   p95=43.3ms max=63.6ms; `echo` p50=33.6ms p95=77.1ms p99=300.5ms max=584.4ms. 1,100/1,100
   samples, 0% loss. Plausible-looking, but see "the redraw finding" below for why `echo`
   specifically cannot be trusted from this run.
2. **Second attempt**: `input`/`echo` both landed on round ~1s/2s/3s/4s plateaus -- the exact
   major-page-fault signature responses 155/156 diagnosed on this same shared machine.
   `free -h` showed 26-28GiB swap in use during both attempts.
3. **Third attempt, after the marker fix, this slice's one capped run per response 209**:
   `input` p50=17.1ms p95=33.6ms max=34.9ms; `echo` (653 of 1,100 markers found before the
   run was killed) p50=1.07s(!) p95=13.2s p99=22.9s max=26.1s. `tick` (pure `poll()` cost,
   untouched by `iced`'s event loop or GPU) stayed clean throughout: p50=0µs p95=5µs p99=84µs
   max=374µs, **zero** samples over 1 second. Swap at 29GiB. Killed per response 209's "stop
   regardless of outcome" once a single `tick` sample read 48,286,176µs (48 real seconds) --
   the confound signature recurring, not a new finding.

**Control, and the best result in this file**: `Criterion::Typing` (no terminal pane)
measured p50=20µs p95=29µs max=33µs on this same machine at the same time -- the historical
microsecond figures, exactly. Run again *with three real terminal panes open* (their wake
subscriptions batched in, receiving genuine echo-driven wakes from real typed input): still
p50=22µs max=85µs. **This rules out the explanation that would have hurt this whole design
most** -- that a wake subscription's mere existence degrades unrelated input processing.
It doesn't. Whatever confounds `TerminalFlood`'s own live numbers is specific to
`TerminalFlood`'s own path (almost certainly the same swap pressure that has dogged every
live GUI attempt on this machine since PR-017-G), not a property of the wake mechanism
itself.

**No further live attempts.** Response 209: "Cap the effort... take one more live run, and
stop regardless of outcome. If it is confounded again, record the instrument as fixed and the
measurement as unavailable on this machine." Attempt 3 confirmed the confound again. Stopped.

### The redraw finding, and the marker-based fix

Building a headless proxy for `echo`-visible latency (to get a clean number the live runs
couldn't), a real, reproducible property of this environment's PTY canonical-mode echo
surfaced: sending the same character repeatedly with no `Enter` (exactly what
`MeasuredTerminalInput` already did, and what every live run above also does via `xdotool`),
past roughly 20 accumulated characters on one unterminated line, the terminal occasionally
**re-echoes the entire current line in one wake**. Traced directly:

```
DIAG count=20
DIAG count=41
```

20 already present + 21 (a full re-echo of the whole line) = 41, in one step. The original
`check_echo_visible` design ("grid occurrence count of a repeated character reaches N")
cannot distinguish a genuine Nth echo from an (N-1)th reappearing in a redraw batch -- a
sample caught in the batch would have recorded a latency that may be an over-report, a
**plausible-looking wrong number**, worse than a missing one.

**Fixed** (`6ad48f4`): `Measurement::next_echo_marker` generates a fresh, never-reused marker
string per send (`"j{index}"`); `check_echo_visible` checks each pending send's own marker
for **substring presence**, not occurrence count. A *new* marker's first appearance cannot be
confused with an *old* marker's reappearance the way two occurrences of one repeated
character can -- immune to the same failure by construction, not by hoping the redraw doesn't
recur. Also fixed while there: `check_echo_visible`'s first version called
`TerminalPane::rendered_text` (a real `O(grid)` cost, 13-46µs measured) unconditionally on
every wake; `Measurement::should_check_echo` throttles this to a 1ms wall-clock floor, gated
on `pending_echo` being non-empty.

**One caution not fully resolved, deliberately, and stated rather than engineered around**:
the accumulated input line is never cleared. A `Ctrl+U` line-clear was built and reverted --
it would have added a fourth production `TerminalPane::write_input` call site, and
`write_terminal_input_has_exactly_the_three_named_production_call_sites` (RFC-018's own "one
PTY ingress" enumeration) exists specifically to catch exactly that addition. Expanding a
security-reviewed enumeration for a diagnostic-only line-length concern, without review, was
judged not worth it; a bounded run (`target`, default 1,100 sends) times a short marker is a
bounded total line length instead, the accepted tradeoff.

### `FLOOD_SCRIPT` re-characterised, not replaced

Unchanged since PR-017-G, but its *meaning* changed without anyone choosing it: under the old
tick it could never exceed one drain per 50ms, so its intensity was irrelevant. Measured this
slice, headlessly, driving a real reader thread: **~250,000-500,000 wakes/sec** (263,715 in
1s at N=1 pane; 504,712 in 2s in the dedicated benchmark). That is not "bounded background
output" -- it is a **saturating** producer, the most aggressive background load this
measurement could plausibly represent. Per response 209: keep the script (not worth replacing
now), but the evidence must stop implying the old characterisation. Recorded here, plainly:
`NFR-PERF-004`'s own phrase has never been evaluated against a realistic *bounded* load, on
either architecture.

### The instrumentation bug, generalised

`rendered_text` at `O(grid)` on every wake, at ~500,000 wakes/sec, is measurement
infrastructure perturbing the thing it measures -- and it only became *possible* because the
wake rate rose two to three orders of magnitude once PR-A1-C removed the tick. Every
per-event instrument in this project was designed under the old tick-throttled assumption;
this one was caught because its own headless benchmark stalled at 2 of 200 samples in 25s.
Others may not be caught the same way. Flagged in `rfcs/future-work.md`.

### `terminal_session_limit`: raised from `Some(3)` to `Some(6)`, headlessly

Measured, not assumed, per response 209 ("the limit is a throughput/keep-up question, not a
paint question... immune to everything that spoiled the live runs").
`terminal_session_limit_headless_n_pane_wake_throughput_benchmark`: N real panes, each
running `FLOOD_SCRIPT` concurrently, drained by **one** single-threaded round-robin loop --
deliberately not N threads each servicing its own pane, since `iced`'s `update()` is
single-threaded in production and every pane's wake funnels through that one consumer
regardless of pane count.

| panes | poll p50 | poll p99 | poll max | aggregate throughput | per-pane |
| --- | --- | --- | --- | --- | --- |
| 1 | 0µs | 0µs | 429µs | 18.2 MB/s | 18.2 MB/s |
| 3 | 0µs | 3µs | 781µs | 51.3 MB/s | 17.1 MB/s |
| 6 | 2µs | 40µs | 545µs | 99.6 MB/s | 16.6 MB/s |
| 8 | 20µs | 95µs | 897µs | 135.9 MB/s | 17.0 MB/s |
| 10 | 132µs | 188µs | 908µs | 141.3 MB/s | 14.1 MB/s |

N≤6: poll cost stays at low single-digit microseconds, aggregate throughput scales linearly
with N at ~17MB/s/pane, matching `FLOOD_SCRIPT`'s own standalone rate. **Degradation first
becomes measurable at N=8** (poll cost jumps ~10x to ~20µs, though throughput is still
linear) **and is unambiguous at N=10** (poll cost ~130µs+, aggregate throughput falling
meaningfully below linear scaling -- the reader genuinely falling behind, not just costing
more per call).

**New limit: `6`**, not `8` -- the same margin-below-first-measurable-degradation philosophy
the old `Some(3)` used (headroom below its own ~5-pane saturation point), this time backed by
real measured headroom instead of the sleep-imposed one it replaces.
`ProjectResourceLimits::default`'s own doc comment carries the full reasoning and figures for
whoever revisits this next; the two tests exercising the default end-to-end
(`terminal_session_limit_is_enforced_end_to_end_with_a_visible_notice`,
`ablation_a_seventh_real_process_would_spawn_without_the_limit_check`) were updated to the
new number and the `tekstide-core` default-value test
(`project_session_starts_with_correct_defaults`, `project/tests/metadata.rs`) likewise.

### Throughput re-measured against the ~374KB/s baseline

**~17.4-18MB/s**, matching `FLOOD_SCRIPT`'s own standalone rate almost exactly (confirmed
across every headless benchmark run this slice performed) -- the replacement figure, roughly
a **47x** improvement, and no longer an architectural ceiling: observed throughput now tracks
whatever the producer actually writes, not a fixed drain rate independent of it.

### Claim statement, checked against the amendment's own text

The handoff pack's own README states this amendment "discharges `NFR-PERF-004` one way or the
other." Checked directly against that: it does discharge it, but not into either of the two
outcomes the phrase anticipates. **Neither "met" nor "not met" -- a third, more precise state
this slice's own evidence forces**: the structural cause of the prior "not met" (an arithmetic
lower bound proven by the tick) is removed and evidenced two ways (arithmetic and a headless
benchmark); the criterion itself is not verified end-to-end, because that requires an upper
bound this project's own `frames()`-avoidance discipline cannot produce on any machine, not
only this one. Recording "met" would be a stronger claim than the available evidence supports,
on weaker grounds than the standard the criterion itself sets. Recording "not met" would
misstate what changed. **What may be claimed**: RFC-017 Amendment 1 removed the specific,
named, arithmetically-proven cause of `NFR-PERF-004`'s prior failure, replaced the ~374KB/s
throughput ceiling with a real, measured ~47x improvement, and re-derived `terminal_session_limit`
from the new mechanism rather than carrying the old number forward by assumption. **What may
not be claimed**: that `NFR-PERF-004` passes, or that this slice's live-GUI attempts produced
usable end-to-end evidence -- three attempts, all confounded, all disclosed rather than
reported as clean numbers.

### Gates

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D
warnings`, full workspace suite, `git diff --check` -- all clean throughout. Final counts:
`tekstide-core` 555 (unchanged -- `terminal_session_limit`'s new value is exercised by
existing tests, not new ones, in that crate), `tekstide` 212 (up from 211 -- one net new test,
the N-pane session-limit benchmark; the marker-fix work and the wake-driven benchmark's
rename touched existing tests without adding or removing any). No test was changed to keep
passing for an unrelated reason -- every touched test either exercises the deliberately new
`terminal_session_limit` value (an intentional default change, not a bent assertion) or is
new.
