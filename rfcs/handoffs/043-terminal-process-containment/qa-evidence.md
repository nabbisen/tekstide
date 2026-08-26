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
