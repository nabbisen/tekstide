---
title: "RFC-017 Amendment 1: Readiness-driven terminal I/O - QA Evidence"
rfc: "RFC-017 Amendment 1"
rfc_file: "../../done/017-terminal-renderer-and-immersion-mode.md"
status: "PR-A1-A closed 2026-08-15 (responses 201/202, commits 79d9c23/85dcbef). PR-A1-B implemented 2026-08-15, reviewed (response 203), required fix applied same day (commit e35d690), not yet re-reviewed — C, D not started"
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

**Implemented 2026-08-15, reviewed (response 203), required fix applied same day (commit
`e35d690`).** `crates/tekstide-core` untouched; all changes
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

*Not started.*

## PR-A1-D — Measurement and closeout

*Not started.*
