---
title: "RFC-043: QA evidence"
rfc: "RFC-043"
rfc_file: "../../accepted/043-terminal-process-containment.md"
source_rfc_status: "Accepted 2026-08-26 — M12"
target_milestone: "M12"
created: "2026-08-26"
---

# QA evidence

## PR-043-A — make the leak red

### The guard, and where it had to live

`RunningTerminal::drop` (`crates/tekstide-core/src/runtime/terminal/launch.rs`), after the
existing `SIGKILL`-the-process-group-and-wait sequence: enumerates every live process whose
`/proc/<pid>/stat` session field matches `self.process_group_id` (which is also this terminal's
session id -- `spawn_pty_child`'s `pre_exec` calls `setsid()` before `exec`, making the freshly
`fork`ed shell both its own process group leader and its own session leader at launch) and panics,
naming every survivor, if the session is not empty.

**Not wired per test, not opt-in** -- placed in the destructor itself, the same "put it where the
process is created" instruction `what-containment-must-not-become.md` §5 states directly, citing
the audit-store slice's own per-site mistake as the thing not to repeat.

### A real cross-crate gap found and fixed before the guard could actually cover anything

First attempt gated the guard with `#[cfg(test)]`. It fired correctly under `cargo test -p
tekstide-core` (caught the case below immediately), and was **silently absent** -- not skipped,
not disabled, the code did not exist in that build at all -- under `cargo test -p tekstide`, where
almost every real terminal-launching test actually lives. `#[cfg(test)]` inside a library crate
only activates when *that crate's own* test suite is what's compiling; it cannot see across a
dependency edge into a consuming crate's test mode. The benchmark already known to leak 28
processes (`test-process-leak.md`'s own "~28/run" figure) ran clean under this first version --
confirmed directly, not assumed, before reporting it fixed.

**Fixed with a Cargo feature, not a workaround.** `tekstide-core/Cargo.toml` gained a
`test-support` feature (empty, gates nothing on its own); the guard's own `#[cfg]` became
`#[cfg(any(test, feature = "test-support"))]`; `tekstide/Cargo.toml`'s `[dev-dependencies]` now
also depends on `tekstide-core` with that feature enabled, on top of the ordinary
`[dependencies]` entry -- Cargo unifies the two into one build for `cargo test -p tekstide`.
Verified this does **not** leak into a release build: `cargo tree -e features` shows
`test-support` only under the dev-dependency edge, and `strings` on a plain `cargo build -p
tekstide --bin tekstide` binary (no `--tests`) contains zero occurrences of the feature name.

### Ablated

Temporarily commented out the `#[cfg(...)]` attribute and the guard call in `RunningTerminal::drop`
(both lines, so nothing partially applies). Re-ran
`linux_runtime_does_not_overclaim_when_child_outlives_direct_shell_after_sigterm` (below) alone:
**passed**, silently, despite the real backgrounded descendant surviving exactly as it does today.
Restored both lines, re-ran: failed again, same message. No `TEMP ABLATION` markers left.

### The inventory

Deliverable this PR-A slice exists to produce, per its own gate item: "the count of tests that
fail once the guard is live, and which ones. Nobody has one today." Ran each crate's suite three
times; the same four tests failed every time, no others:

| Test | Crate | Why it leaks |
| --- | --- | --- |
| `runtime::terminal::tests::linux_runtime_does_not_overclaim_when_child_outlives_direct_shell_after_sigterm` | `tekstide-core` | Deliberately backgrounds a `trap '' TERM` descendant to prove `request_terminate` reports `KilledAfterTimeout` honestly rather than overclaiming success -- the test's own scenario *is* the escape this RFC exists to close |
| `shell::tests::terminal_session_limit_headless_n_pane_wake_throughput_benchmark` | `tekstide` | Launches `1+3+6+8+10=28` panes running the backgrounded `FLOOD_SCRIPT` loop |
| `shell::tests::terminal_poll_handler_cost_under_a_real_wake_driven_flood_headless_benchmark` | `tekstide` | Same `FLOOD_SCRIPT`, one pane |
| `shell::tests::closing_a_project_with_a_backgrounded_descendant_still_records_applied_while_it_survives` | `tekstide` | Its own name states the scenario -- a real, deliberately backgrounded descendant that is expected to survive a close today; this is D3/D4's own audit-honesty test, and its fixture is exactly what PR-043-B/C's containment has to change the outcome of |

**No other test in either crate's suite went red** across three runs each (742/742 and 441/441
elsewhere, stable). One pre-existing, already-documented flake
(`approval::tests::channel::bind_recovers_from_a_stale_socket_file`, `test-process-leak.md`'s own
row 1, "the original, response 213") appeared once in a `--no-fail-fast` full run and passed
cleanly on three immediate isolated re-runs -- unrelated to this guard (a socket bind error, not a
process/session one), not counted in the inventory above.

### Gate

`cargo build --workspace --all-targets`, `cargo fmt --all -- --check`, `cargo clippy --workspace
--all-targets --all-features -- -D warnings`, `git diff --check`: all clean.

**The suite is not green, by design.** `cargo test --workspace`: 441/444 (`tekstide`), 741/743
(`tekstide-core`), 2/2 (`rfc_docs_invariants`) -- the 4-test inventory above is the only thing red,
stable across repeated runs. Per this slice's own gate item: "the suite will not be green at the
end of this slice, by design. Say so plainly; do not skip the guard's rollout to keep a green
run." Said plainly here.

## PR-043-B — the sequence

### What changed, and where

`termination.rs`'s `request_terminate`, and `launch.rs`'s `RunningTerminal::drop`, both replaced
with the same sequence:

1. `SIGHUP` the session leader alone (a single, positive pid -- never `-process_group_id`, never
   `-session_id`). Wait a bounded grace period.
2. Enumerate the session; `SIGKILL` every survivor, re-verifying immediately before each signal
   that the pid is still a member (security document §1).
3. Re-enumerate to confirm empty -- this observation, and only this one, is what
   `TerminalRuntimeEvent::SessionConfirmedEmpty` reports.

**Both call sites, not only `request_terminate`.** The task breakdown says so explicitly
("The containment routine, in `request_terminate` and `RunningTerminal::drop`") and a first pass
here missed it: `Drop`'s own kill logic was left as the old `SIGKILL`-the-process-group-only call,
with only PR-043-A's diagnostic guard added *after* it. This shipped a genuine gap, caught by
running the two `FLOOD_SCRIPT` benchmarks (which rely on `Drop`-based cleanup, never call
`request_terminate`) and watching PR-043-A's own guard still fire against them. `Drop` now runs the
identical two-phase sequence with short, fixed grace periods (200ms/200ms -- bounded, since this
runs synchronously during an unrelated panic's unwind and must not hang) instead of
`request_terminate`'s caller-supplied ones.

`TerminationSignal` gained `Sighup`; `TerminationOutcome::TerminatedBySignal` and
`outcome_from_exit_status` handle it. `TerminalRuntimeEvent` gained `SessionConfirmedEmpty { handle,
confirmed }`, pushed exactly once, immediately before `Terminated`.

### A second real bug found while writing the required "false" test

`what-containment-must-not-become.md` §4 requires `false` for "a `/proc` read that failed," not
just a grace period expiring with survivors. The first version of `processes_in_session` defaulted
a failed `read_dir("/proc")` to an empty `Vec`, which downstream code then read as "confirmed
empty" -- the exact unearned confidence that document forbids. Found writing this slice's own
required negative test, not by inspection. Fixed: `processes_in_session` now returns
`Option<Vec<pid_t>>` (`None` only when enumeration itself failed, distinct from `Some(vec![])`),
and a new `session_confirmed_empty` is the one place that turns it into the honest boolean
(`false` for both "found survivors" and "could not check").

### Ablated, both required points

- **Remove step 1** (temporarily disabled the `SIGHUP` send): the "clean exit" test
  (`linux_runtime_terminates_session_leader_with_sighup`) still eventually succeeds via the
  `SIGKILL` fallback, but took the full grace period doing it (2.02s vs near-instant) -- "a slice
  that adds session enumeration and keeps the SIGKILL-first order will work, badly," measured, not
  quoted.
- **Remove the session re-verification** (temporarily disabled the check in `signal_candidates`):
  `signal_candidates_never_signals_a_pid_outside_the_target_session` fails -- the deliberately
  unrelated stranger process gets signalled too (`left: 2, right: 1`). Restored, passes again. Not
  built by winning a real PID-reuse race (inherently flaky); `signal_candidates` was factored out
  from its own live-enumeration caller specifically so a test could hand it a controlled candidate
  list including a pid that is definitely not a session member, without needing to race anything.

### Required tests

| Requirement | Test |
| --- | --- |
| A real backgrounded job is gone after a real close, by `kill -0` | `a_real_backgrounded_job_is_dead_after_a_real_close` |
| A `setsid`-detached process survives, asserted on purpose | `a_job_that_leaves_the_session_via_setsid_survives_a_real_close` |
| Grace period expiring produces `false`, not a hopeful `true` | `session_confirmed_empty_reports_false_when_its_own_enumeration_fails` (see "a second real bug," above, for why this is built the way it is rather than by racing `SIGKILL` reaping speed) |
| PR-043-A's guard now passes for the benchmark | Both `FLOOD_SCRIPT` benchmarks, and the sigterm-overclaim test, all pass -- see next section |

### Three tests this slice's own fix made false, corrected rather than left stale

Discovered by running the existing suite after the fix landed, not decided in advance:

- `linux_runtime_does_not_overclaim_when_child_outlives_direct_shell_after_sigterm`
  (`tekstide-core`): its own `trap '' TERM` descendant does not trap `SIGHUP`, so the shell's own
  job-control hangup now reaps it before any escalation -- `confirmed: true`,
  `TerminatedBySignal { signal: Sighup }`, no timeout at all. Renamed to
  `a_real_backgrounded_job_is_dead_after_a_real_close`, rebuilt around a plain `sleep 300 &`
  scenario (the required test above), with the old scenario's history kept in its own doc comment
  rather than deleted.
- `linux_runtime_uses_sigkill_fallback_for_foreground_child_after_sigterm_timeout`
  (`tekstide-core`): a bare foreground `sleep 30` also turned out not to survive step 1 -- a session
  leader blocked in a foreground `wait()` still receives and acts on `SIGHUP` immediately.
  Rewritten around a job that explicitly traps `SIGHUP` (the same shape the overclaim test used for
  `SIGTERM`), which is what actually needs the `SIGKILL` escalation now.
- `closing_a_project_with_a_backgrounded_descendant_still_records_applied_while_it_survives`
  (`tekstide`): identical shape to the first, at the GUI-close level -- the descendant no longer
  survives. Renamed to `..._kills_it_through_a_real_close`, assertions inverted (was
  `still_alive`, now `!still_alive`), the now-unneeded manual cleanup `kill -9` removed since
  nothing survives to clean up.

None of these are broken tests. Each is the fix working on the exact scenario that used to
demonstrate the defect it fixes -- a real, measured before/after, not an assumption.

### Gate

`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D
warnings`, `git diff --check`: all clean. **The suite is green again**: 444/444 (`tekstide`),
746/746 (`tekstide-core`, four new tests), 2/2 (`rfc_docs_invariants`).

Four full-workspace runs (`--no-fail-fast`), `/dev/pts` flat at 13 across every one: three clean
outright (runs 1, 2, 4); run 3 hit one pre-existing, already-documented flake
(`approval::tests::coordinator::is_still_answerable_reflects_the_real_connection_state`,
`test-process-leak.md`'s own row 5/6, a socket-timing test unrelated to any file this slice
touches) that passed on immediate isolated re-run. Not re-run further, per the standing lesson
about not hammering a shared machine chasing a lucky streak once the property actually being
tested (a clean, stable `/dev/pts`) already holds.

## Response 340 — two required fixes, and the timing investigation asked for

### Required 1 — a zombie counted as a survivor

The reviewer's own repro: one run's failures all had `survivors: Some([session_id])` -- the
survivor pid *was* the session id, i.e. the terminal's own already-`SIGKILL`ed leader, not an
escaped job. `processes_in_session` read only the session field of `/proc/<pid>/stat`, never the
state field, so a zombie (killed, not yet reaped by its parent) was counted as a live member.
Confirmed directly, the same way the reviewer did: a zombie really does keep its own
`/proc/<pid>/stat` entry, session field unchanged, state `Z`.

Fixed: `is_live_member_of_session` now excludes state `Z`. `session_id_of` (the separate,
single-pid lookup `termination.rs`'s own re-verification calls) is deliberately left
non-zombie-aware -- a zombie's session field is still authoritative, and `kill(2)` on a zombie is
already a harmless no-op, so there was no false-positive-survivor risk on that path to begin with.

**A second, smaller bug the fix itself exposed, found immediately by re-running the suite rather
than assumed fixed:** excluding zombies makes `session_confirmed_empty` able to report `true` a
few microseconds before this same process's own `try_wait()`-based reaping call catches up (the OS
marks a killed child a zombie in its own time, independent of when *we* next call `try_wait()` on
it) -- `wait_for_session_outcome` could reach "confirmed empty" with `child_outcome` still `None`,
reporting a vague `OrphanedUnknown` for what was actually a clean `SIGHUP`/`SIGKILL` exit. Since
only this process can ever reap its own direct child, "confirmed empty and not yet reaped by us"
can only mean "currently an unreaped zombie" -- so a blocking `wait()` at exactly that point returns
essentially instantly (it is already dead) and recovers the real exit status.
`linux_runtime_terminates_session_leader_with_sighup` caught this immediately after the zombie fix
landed, before it was reported fixed.

### Required 2 — the message asserted a cause it had not established

Reworded `assert_session_is_empty`'s panic: states what was observed (the survivor list) and names
the possible causes (an actual escape, a failed `/proc` read, something else not yet excluded)
without picking one. The reviewer's own parallel to the `change_review_content_view_build_cost...`
finding was the right frame -- a message that answers a question the code cannot actually answer
tells the next reader to stop looking.

### The timing investigation

Required 1's fix alone took `tekstide`'s own suite from 17.6s back to 11.4s -- most, not all, of
the 3.4× regression. Investigated the remainder rather than accepting a partial recovery: the
N-pane `FLOOD_SCRIPT` benchmark's own 28 panes are where essentially all of it lives (10.95s alone,
vs ~5s before PR-043-B). Traced one directly: 20ms after `SIGHUP`, the session leader is `state=S`,
`wchan=iterate_tty_write`, blocked in `write(2)` to fd 2. `FLOOD_SCRIPT` produces continuous,
unread output once the benchmark's own read loop stops (nothing drains the PTY's master side
during `Drop`); the interactive shell's own job-control status message on receiving `SIGHUP`
(printed to the same saturated PTY) blocks in exactly the same buffer, and stays blocked until
`SIGKILL` forces it through. **Not a bug in this sequence** -- `SIGKILL` cannot be blocked, and the
escalation correctly reaches and clears it every time (confirmed: `kill_worked=true` in ~1.5ms once
sent) -- it is a real, explained cost specific to a workload that deliberately floods its own PTY
and stops reading it, not a property of ordinary terminal use. Three consecutive full-workspace
runs after both fixes: clean, `/dev/pts` flat at 13, `tekstide` at a stable ~11.4s.

## Response 341 — closing the master, for real

Required: "close the master before or with the `SIGHUP`, so step 1 does its job on a busy terminal
rather than only an idle one" -- RFC-043's own step 1 disjunction, the half left unimplemented
after PR-043-B.

### `TIOCVHANGUP` tried first, and ruled out

Reasoned that `ioctl(fd, TIOCVHANGUP)` would force the pty's hangup regardless of how many other
fds referenced the same master, sidestepping the need to coordinate with the reader thread's own
duplicate. Wired it into both `RunningTerminal::drop` and `request_terminate`, all 70
`runtime::terminal::` tests still passed, and the `FLOOD_SCRIPT` benchmark showed **zero**
improvement (10.98s vs 10.95s). Added `eprintln!` diagnostics around the call and found why:
`hangup_master_ok=false errno=Os { code: 1, kind: PermissionDenied, message: "Operation not
permitted" }`. `TIOCVHANGUP` requires `CAP_SYS_ADMIN` unconditionally on Linux, including for a
process that created the pty itself. Reverted both call sites and the function entirely rather than
leave a primitive that always fails silently in the tree.

### The real mechanism, and the actual fix

Went back to the literal suggestion: a plain `close(2)` on the master. The reason PR-043-B's own
measurement showed this mattering is real -- a pty only hangs up once its *last* referencing fd
closes, and `spawn_output_reader` (`launch.rs`) hands the reader thread (`TerminalReader`, owned by
`TerminalPane` in `crates/tekstide`) its own `try_clone()`d duplicate of the same master. Closing
`RunningTerminal.master` alone was never going to be the last reference on its own.

The fix did not need new coordination machinery, because `TerminalReader` already has it:
`Drop for TerminalReader` signals its shutdown `eventfd`, drops its `receiver` (unblocking a thread
parked in a full-channel `send`), and **joins the thread** before returning -- by the time that
`drop` call returns, the reader thread's own duplicate of the master (a local, owned `fs::File`
inside the thread's closure) is provably closed. The only problem was ordering: `TerminalPane`
declared `runtime` (containing `RunningTerminal`) *before* `reader`, and Rust drops struct fields in
declaration order, so `RunningTerminal::drop`'s whole `SIGHUP`/enumerate/`SIGKILL` sequence ran
*before* `reader`'s graceful shutdown ever closed its own duplicate.

Two changes, both required together:

1. `crates/tekstide/src/surface/terminal.rs`: reordered `TerminalPane`'s fields so `reader` is
   declared (and therefore dropped) before `runtime`.
2. `crates/tekstide-core/src/runtime/terminal/launch.rs`: `RunningTerminal.master` became
   `Option<fs::File>` (a `master()` accessor panics if read after it is taken, which only `Drop`
   does), and `Drop for RunningTerminal` now does `self.master.take()` first, before the `SIGHUP`.
   Taking this reference is always safe regardless of who else holds a duplicate (closing one
   `dup`/`try_clone`'d fd number never disturbs another live one over the same open file
   description) -- it is only *effective* here because, by the time this runs as part of
   `TerminalPane`'s own teardown, the reordering above already guarantees the reader's own
   duplicate is gone, making this close the last reference.

### Measured

`tekstide`'s own 444-test suite: **5.59-5.61s** across three consecutive runs, `/dev/pts` flat at
13 -- not just a recovery from the 11.4s post-Required-1 baseline, but back at (marginally better
than) the original ~5.2s pre-PR-043-B baseline. Full workspace, three consecutive runs: 444 + 746 +
2, clean, no flakes, 8.3-12.3s wall time. `fmt`, `clippy -D warnings`, `git diff --check`: clean.

### The gap this does not close: `request_terminate`

The fix above only helps the **`Drop`** path -- a `TerminalPane` (reader and all) being torn down
together. `termination::request_terminate`'s own real caller
(`crates/tekstide/src/shell.rs`'s project-close flow, `terminal_process_groups_confirmed_empty`)
does something different: it removes the `TerminalPane` from its tracked list first, then calls
`pane.request_terminate(...)` directly on the now-isolated value, and nothing drains
`TerminalPane`'s `reader` for the entire span of that blocking call. The reader thread stays alive
and holds its own duplicate of the master throughout -- so even if `request_terminate` closed
`RunningTerminal`'s own copy at the same point `Drop` now does, it would not be the last reference,
would not trigger a real hangup, and would not unblock a session leader stuck writing into a
saturated pty. Closing it there was never unsafe (the same "closing one fd number never disturbs
another" property applies); it would simply do nothing.

This is the same busy-terminal scenario response 341 raised (an agent mid-write, project closed),
reached through the code path Tekstide's own UI actually uses for closing a project, not only
through the synthetic `Drop`-only path the benchmark exercises. Making it effective there needs
`TerminalPane` itself to shut down or drain its reader as part of requesting termination, before
`request_terminate`'s own `SIGHUP`, not only afterward when the pane is finally dropped -- a change
to what `crates/tekstide` and `crates/tekstide-core` agree the termination sequence's shape is, not
a narrow fix inside either crate alone. Recorded in `termination.rs`'s own doc comment on
`request_terminate` rather than left implied. Not fixed in this slice; see review request 342.

## Response 342 — the `request_terminate` gap, closed

The reviewer's answer to the open question: not "accept it" -- `request_terminate` is the path a
user actually reaches by closing a project, and the `Drop` path just fixed is the synthetic one the
benchmark exercises. Also corrected my own scoping: the reviewer expected the reader-draining
reasoning to be wrong (draining should keep the pty from saturating during the call) and checked it
rather than trusting it -- it held, because `reader.rs` uses a *bounded*, *blocking*
`mpsc::sync_channel(8)`: with nothing polling the receiver during `request_terminate`'s blocking
call, the channel fills in milliseconds regardless, the reader parks in `send`, and the pty
saturates exactly as in the `Drop` case. And the fix turned out smaller than the "redesign spanning
both crates" framing in the request: the machinery this needed already existed.

### What changed

1. `crates/tekstide-core/src/runtime/terminal/reader.rs`: extracted `Drop for TerminalReader`'s
   body into a new `pub fn shutdown(&mut self)` -- signals the shutdown `eventfd`, drops the
   receiver, joins the thread, synchronously. `Drop::drop` now just calls it. Idempotent (every
   step is: `Option::take` on an already-taken field is a no-op, a second `eventfd` write is
   harmless), so a caller invoking this explicitly and then letting the value drop anyway performs
   no second shutdown.
2. `crates/tekstide-core/src/runtime/terminal/launch.rs`: added `RunningTerminal::close_master`
   (`pub(super)`, for `termination.rs`), doing exactly what `Drop::drop`'s own early `master.take()`
   does, so both termination paths share the same primitive.
3. `crates/tekstide-core/src/runtime/terminal/termination.rs`: `request_terminate` now calls
   `close_master()` first, before the `SIGHUP` -- the same disjunction half `Drop` already applies.
4. `crates/tekstide/src/surface/terminal.rs`: `TerminalPane::request_terminate` now calls
   `self.reader.shutdown()` *before* delegating to `self.runtime.request_terminate(...)`. This is
   what makes step 3's close the pty's last reference at the moment it happens, exactly mirroring
   what `TerminalPane`'s own field order already does for the `Drop` path -- no new contract between
   the two crates, an ordering change at the one real call site plus an accessor, as the reviewer
   predicted.

### The recorded cost

Shutting the reader down before requesting termination means output the shell produces from that
point until it actually exits -- including a final line of transcript capture, if configured for
this terminal -- is not read. Written down in both `request_terminate`'s own doc comment
(`tekstide-core`) and `TerminalPane::request_terminate`'s (`crates/tekstide`), not left as a silent
side effect of the ordering change: a requested close accepting a truncated transcript tail is
preferable to the failure mode this fixes, a busy terminal's `SIGHUP` being a silent no-op with
every job in it reaching `SIGKILL` instead.

### Ablated

Added `request_terminate_on_a_busy_terminal_succeeds_without_falling_back_to_sigkill`
(`crates/tekstide/src/shell/tests.rs`): launches a real `/bin/sh`, writes `FLOOD_SCRIPT`, calls
`TerminalPane::request_terminate`, and asserts no `TerminationSignalSent { signal: Sigkill, .. }`
event appears. Stashed the fix (keeping the test) and re-ran: failed exactly as predicted --
`TerminationTimedOut` after `SIGHUP`, then `Sigkill`, `KilledAfterTimeout` -- confirming the test
catches the real regression rather than passing vacuously. Restored the fix; passes.

The first version of this test used a 2s hangup timeout (matching
`linux_runtime_terminates_session_leader_with_sighup`'s own) and flaked once under
`cargo test --workspace`'s full concurrent load -- a real `/bin/sh` running a live flood is
heavier than that test's `/bin/cat`, and scheduling contention from hundreds of other real-process
tests running alongside it, not this fix's own correctness, was the cause. Raised to 5s (this test
asserts correctness, not a tight timing bound) and reran `cargo test --workspace` five consecutive
times: clean every time.

### Gate

Full workspace, five consecutive runs: 445 + 746 + 2, clean, no flakes. `tekstide` stable at
5.59-6.22s. `/dev/pts` flat at 12. `fmt`, `clippy -D warnings`, `git diff --check`: clean.

## PR-043-C

Not started. Owns: the close-confirmation wording (D1 + RFC-034 D4's rule), the
`terminal_process_groups_confirmed_empty` → `terminal_session_confirmed_empty` rename and its
audit-side wiring in `shell.rs` (this slice only produced the real
`TerminalRuntimeEvent::SessionConfirmedEmpty` signal `terminate_project_live_work` will need to
read from instead of inferring from the outcome variant, as it still does today), and correcting
the remaining disclosed-limitation text in `test-process-leak.md` and its own doc comments that
this slice's fix has made stale (the "not fixed here" framing around the backgrounded-job-survives
defect itself, as distinct from the pool-exhaustion mechanism the fd-inheritance fix already
closed).
