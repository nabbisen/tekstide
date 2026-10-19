---
title: "The leaked-child test flake and flake register — every leak cause fixed; disposition pass complete 2026-10-09"
status: "Leak fixed at the two approval call sites 2026-08-20 (request 282). **The second cause — runtime/terminal/launch.rs — FIXED 2026-08-25** (RunningTerminal now has a Drop impl); measured 3,899 leaked shells before, near-zero (the pre-existing baseline) after, across three clean full-suite runs. **A third, distinct cause found while verifying the second fix**: a backgrounded job inside a terminal gets its own process group that neither this Drop nor the production request_terminate path's process-group signal reaches — request_terminate reports Terminated/KilledAfterTimeout while the job survives, orphaned. Confirmed against both paths directly. **The job-escapes-termination defect itself — FIXED 2026-08-26, RFC-043 PR-043-B** (rfcs/handoffs/043-terminal-process-containment/): request_terminate and RunningTerminal::drop now SIGHUP the session leader first, then enumerate and SIGKILL whatever remains in the session (not the process group); measured 0 leaked processes after the 28-pane benchmark that used to leak 28, every time. **The close-confirmation dialog's wording and the audit field's own rename/rewiring — FIXED, RFC-043 PR-043-C**: the dialog now says, before the `Close` click, that anything a terminal started (including a backgrounded job) ends too; `terminal_process_groups_confirmed_empty` is renamed `terminal_session_confirmed_empty` and now reads `SessionConfirmedEmpty`'s own real, session-wide `confirmed` field directly, rather than inferring it from which `TerminationOutcome` variant the leader itself produced — a backgrounded job surviving inside the session (this document's own third cause) is exactly what that inference could not see and this rewiring now can. **What made it pool exhaustion anyway — FIXED 2026-08-26** (rfcs/handoffs/pty-master-fd-inheritance.md): every PTY master was inherited by every child this process ever spawned, so a leaked shell's blocked write into a PTY another process still held open never got EIO and never exited; masters now close on exec, so the same transient orphan now self-terminates within its own script's deadline instead of surviving indefinitely, and /dev/pts occupancy no longer rises across a run at all. This was also a production security defect (RFC-009's terminal boundary crossed by any process able to read/write another project's terminal via the inherited descriptor), not only a test-harness one. The socket flake is separate and also unfixed. **The shared-audit-store cause — FIXED 2026-08-26** (`rfcs/handoffs/audit-store-test-isolation.md`): every test now opens its own store automatically, so the one-shared-SQLite-database contention this document traced row 3 to (and confirmed rows 4 and 6 also transitively reach) can no longer occur in any test in this binary; five consecutive clean full-workspace parallel runs against a fresh `XDG_STATE_HOME`, plus a serial run, confirmed after. A related, separate, and still-open gap found while verifying this: with `XDG_STATE_HOME` unset, the suite's `transcripts/` (and `approval/`) subtrees under the developer's real state root still receive real writes during a run — the audit subtree itself is confirmed untouched, but the same class of defect exists one directory over, out of the audit-store handoff's own scope. **Test-flake-disposition pass COMPLETE 2026-10-09** (`rfcs/handoffs/test-flake-disposition.md`): eleven already-fixed rows confirmed and retired in place, eight genuinely live rows each given exactly one verdict (4 Fix, 2 Quarantine, 2 Accept), measured rate re-taken at 0 failures in 13 full-workspace runs against a ~18% baseline. See the dated disposition entry at the end of this document."
rfc_file: "none — a test-harness defect, not product behaviour"
target_milestone: "M12"
created: "2026-08-19"
---

# The leaked-child test flake

## Why this is being scheduled now

**The cause was found on 2026-08-16 and has never been fixed.** From `future-work.md`:

> `Child::drop` does not kill the process, so **any test that panics before reaching its own
> cleanup leaks a shell process**

Since then, **five distinct tests** have been reported failing intermittently under the
resulting pressure, each disclosed separately and each moved past:

| test | first reported |
| --- | --- |
| `approval::tests::channel::bind_recovers_from_a_stale_socket_file` | the original, response 213 — **row 1. Verdict, test-flake-disposition pass (2026-10-09): Accept. See the dated disposition entry.** |
| `approval::tests::coordinator::agent_run_queue_limit_is_enforced_and_only_counts_live_entries` | request 260 — **row 2. Same cause as row 5 (a real kernel scheduling gap between `drop(peer)` and the coordinator's own end observing the close), confirmed at the disposition 2026-10-07. Fixed: polls `is_still_answerable` to `false` against a bounded deadline before relying on the freed budget slot, rather than attempting the next receive on the very next line. See the dated disposition entry; retired from active tracking.** |
| `command_approval_family_produces_real_durable_audit_records_through_the_pipeline` | request 276 — **row 3. Closed by the shared-SQLite-audit-store fix (2026-08-26) -- directly observed transitively reaching `open_real_audit_store`, per the "ROOT CAUSE, CONFIRMED" section below. Confirmed by the test-flake-disposition pass (2026-10-09): no recurrence anywhere in this document in the six weeks since the fix, and the test passes in isolation. Retired from active tracking.** |
| `shell::tests::a_real_low_risk_proposal_is_received_mirrored_and_stays_queued_without_promoting` | request 296 (2026-08-24) — **row 4. Same closure as row 3: closed 2026-08-26, confirmed by the disposition pass (2026-10-09), no recurrence since. Retired from active tracking.** |
| `approval::tests::coordinator::is_still_answerable_reflects_the_real_connection_state` | request 326 (2026-08-25) — **row 5. `drop(peer)` closes the socket locally; the coordinator's own end observes it on the kernel's own schedule, not instantly — a defect in the test's single post-drop assertion, not in the product. Fixed, disposition 2026-10-07: replaced with a bounded poll for `is_still_answerable` to become `false` (seconds, not milliseconds). Retired from active tracking; the recurrence sections below stay as history.** |
| `shell::tests::change_review_surface_renders_a_real_change_set_from_a_real_agent_run` | request 329 (2026-08-26) — **row 6. Candidate confirmed closed by the test-flake-disposition pass (2026-10-09): the same shared-SQLite-audit-store fix (2026-08-26) directly observed reaching it, per the "ROOT CAUSE, CONFIRMED" section's own reachability audit; no recurrence anywhere in this document since. Retired from active tracking.** |
| `shell::tests::change_review_content_view_build_cost_by_line_count_measurement` | review 338 (2026-08-26) — **not this document's own cause; see below. Fixed, `0.29.0`: rewritten as a paired ratio against a reference workload timed in the same run, so load no longer decides it; see the dated entry. Re-confirmed closed by the test-flake-disposition pass (2026-10-09): still passes, no recurrence since the fix.** |
| `shell::tests::closing_a_project_with_a_backgrounded_descendant_kills_it_through_a_real_close` | `0.16.0` release gate (2026-08-28) — **row 8, PTY read timing, not process leak or audit store.** The assertion message was captured: *"the marker must be followed by a real, parseable PID"*, with the read having returned only the shell's **echo of the command line** and not yet the `descendant-pid:` line it prints. **Fixed by the test-flake-disposition pass (2026-10-09), the same shape as review 478's two backgrounded-job tests: polls the parse itself (`poll_demo_pane_until_parsed`) instead of a substring followed by a separate parse. See the dated disposition entry.** |
| `transcript::tests::a_live_writer_holds_an_exclusive_lock_until_it_is_dropped` | request 388 (2026-09-13) — **new test; fork-duplicated descriptor keeps an `flock` alive past the drop. Fixed in the test, see the dated entry. Confirmed by the test-flake-disposition pass (2026-10-09): still passes. Retired from active tracking.** |
| `runtime::terminal::reader::tests::local_bounded_marks_capture_failed_and_keeps_reading_when_the_transcript_is_genuinely_unwritable` and `…required_local_bounded_marks_capture_failed_stops_reading_and_stalls_the_child_without_killing_it` | request 388 (2026-09-13) — **caused by PR-050-A, not load: the two tests share `/dev/full`'s lock. First attributed to load, wrongly; see the dated entry's correction. First masked by a test mutex; fixed at response 388 by locking regular files only, and the mutex removed. Confirmed by the test-flake-disposition pass (2026-10-09): both still pass, no test mutex anywhere in the file. Retired from active tracking.** |
| `runtime::terminal::reader::tests::the_wake_notifier_wakes_when_real_pty_output_arrives` | review of request 391 (2026-09-15) — **a race inside the test; mechanism confirmed by measurement and fixed in RFC-050 PR-050-B's first commit (2026-09-16).** The test now holds `exit` back until the first wake is observed. See the dated entries. **Confirmed by the test-flake-disposition pass (2026-10-09): still passes on Linux. Retired from active tracking there.** **Reopened for macOS (2026-10-19): the same test fails deterministically on `macos-measurement` at a different assertion -- `wait_for_exit` returns `None` where `Exited { exit_status: 0 }` is expected (`reader/tests.rs:115`). Not the 2026-09-15 race, not an intermittent. See the dated entry at the end.** |
| `audit::tests::purge::purge_reports_deferred_cleanup_while_wal_reader_is_active` | RFC-030 PR-030-B's wiring-layer gate, run 3 of 3 (2026-09-22) — failed once under the third of three consecutive full-workspace runs; passed immediately in isolation (`cargo test -p tekstide-core audit::tests::purge::purge_reports_deferred_cleanup_while_wal_reader_is_active`, 1 passed, 0 failed). The name names its own contention (a WAL reader held open while purge runs); not investigated further this session — flagged as a candidate for the same class of SQLite-under-parallel-load timing sensitivity this document's other rows already describe, not confirmed as the same root cause. **Verdict, test-flake-disposition pass (2026-10-09): Accept. See the dated disposition entry.** |
| `runtime::git::tests::every_failure_to_answer_is_unknown_and_never_none_ignored` | RFC-055 PR-055-C, one ablation run (2026-09-25) — **suspected `ETXTBSY` on a stand-in `git` script; not reproduced; mitigated in the test, see the dated section at the end.** `wait_until_executable` already retries on `errno 26` before handing out the stand-in script. **Confirmed by the test-flake-disposition pass (2026-10-09): mitigation still in place, no recurrence since. Retired from active tracking.** |

**Row 7 is a different cause, added deliberately rather than by accident.** Every row above shares
the process-leak (later, audit-store) pressure this document investigates; row 7 does not -- it is
a wall-clock view-build measurement (`suite-assumes-it-owns-the-machine.md`, item 2) asserting a
500ms budget, and it failed 6 of 7 red runs in review 338's own gate at a load average of 59.7 on
a 32-core box, produced by this project's own repeated full-suite runs, not by anything leaked. Its
own message used to claim a failure "would indicate a real regression, not measurement noise" --
every observed failure was noise, and the message could not tell the two apart. Fixed to report
what was crossed and the load average alongside it, rather than claim which cause it hit; kept in
the default suite rather than `#[ignore]`d, matching this table's own precedent -- a recorded,
honestly-worded flake still runs and still catches a real regression the day one lands, where an
ignored test stops running at all. Recorded here for the same reason the other six are: so the
next person who meets it under load spends a lookup, not a day.

**Rows 3, 4, and 6's cause closed 2026-08-26** — `rfcs/handoffs/audit-store-test-isolation.md`'s fix isolates every test's own audit store, removing the one-shared-SQLite-database contention the "ROOT CAUSE, CONFIRMED" section below traced row 3 to. Checked reachability rather than assuming it: rows 1, 2, and 5 live in `tekstide-core`, which has no access to `open_real_audit_store` at all -- that function, and the `update()` write call sites that reach it, are defined entirely in the `tekstide` binary crate, so those three rows are **categorically not** caused by this (row 5's own text above already said as much speculatively; this confirms it structurally, not just as an unconfirmed guess). Rows 3, 4, and 6 all live in `tekstide`'s `shell/tests.rs`, and all three were directly observed transitively reaching `open_real_audit_store` (each panicked against an intermediate, over-strict version of this fix that required every reaching test to opt in -- see that handoff's own write-up for why that version was wrong and was replaced). Row 3 additionally had its pre-fix flake reproduced directly (3 of 4 quick repro runs failed, 2-3 failures each, against a fresh `XDG_STATE_HOME`) immediately before the fix, then 5 consecutive clean full-workspace parallel runs confirmed after. Rows 4 and 6 were not individually reproduced as standalone flakes the way row 3 was, so their closure rests on reachability plus the same structural fix, not on a matching before/after reproduction each.

**The fourth was added 2026-08-24 and the count above was edited with it**, because "three
distinct tests" is a count, and `ARCHITECTURE.md` records counts as state-asserting text that a
reference sweep cannot find. This document would otherwise have gone on saying three while
naming four, which is the failure it exists to describe, in the document describing it.

The fourth arrives through the same machinery as the others — `launch_real_managed_agent_run`
and `poll_approval_channels_until`, i.e. a real adapter process over a real socket — so it is
not a new cause. It was disclosed by the implementer with the correct reasoning attached: their
change touched only `rfc_docs_invariants.rs`, a separate test binary, so it cannot be the
source. **Not reproduced by the reviewer**, deliberately: this document's own instruction is not
to chase the symptoms individually, and a 3-in-150 event is not worth a sampling run to
re-observe what is already diagnosed.

**The fifth, added 2026-08-25, is a different shape than the first four**: it never calls
`launch_real_managed_agent_run` at all — `is_still_answerable_reflects_the_real_connection_state`
uses `ApprovalCoordinator::receive` directly with a real socket peer, `drop`s that peer, and
checks connection state immediately afterward, with no process spawn anywhere in the test itself.
Disclosed with the same reasoning applied honestly rather than assumed: the change under review
touched `change_detection.rs`, `changeset.rs`, `session.rs`, and `shell.rs`'s change-review
rendering — nothing in the approval/coordinator/socket path — so it cannot be the source. What
plausibly connects it to the same underlying pressure is the assertion's own shape: it depends on
the OS delivering a socket-close notification promptly enough for an immediate next check to
observe it, which is exactly the kind of timing this document's diagnosed process-leak pressure
(leaked shells, leaked PTYs, orphaned process groups) would perturb. Not confirmed as the same
root cause, only consistent with it — recorded rather than asserted.

Every one of those disclosures was the right individual call — reported rather than re-run
past, confirmed non-deterministic in isolation, not attributed to the slice that saw it.
**Collectively they are the problem**: a diagnosed defect is being re-observed instead of
repaired, and each new symptom costs a reviewer and an implementer the time to establish it is
the same old thing.

Measured rate, from the sampling already recorded: **3 failures in 150 full-suite runs.** Not
enough to block work. Enough that every contributor will meet it. That figure is from the
original sampling and has not been re-measured since; treat it as a floor rather than a current
rate, given that the number of distinct tests observed failing has grown from three to four
without anyone re-running the sample.

## A second, distinct cause: the production spawn path (added 2026-08-24)

Everything above concerns two `approval::tests` call sites, which the original fix covered.
**`runtime/terminal/launch.rs` — the real shell-spawn path the product itself uses — was never
given the same treatment**, and has the identical defect: `RunningTerminal` holds a `Child` with
no `Drop` impl, and nothing calls `.kill()`.

Found during RFC-038 PR-038-C (request 299) when a test run hit `PTY exhaustion (os error 28)`
and cascaded into roughly seventy unrelated failures. Investigation found **4023 orphaned idle
`/bin/sh` processes** (`PS1=tekstide$`, reparented to `systemd --user`, ages up to ~2.5 hours),
with the PTY pool at 4096/4096. They ignored `SIGTERM` — confirmed via `SigIgn` in
`/proc/<pid>/status` — and cleared only under `SIGKILL`.

The implementer stopped before killing anything, confirmed via `/proc/<pid>/environ` and
`/proc/<pid>/fd` that these were genuinely orphaned Tekstide artifacts rather than the owner's
real terminals, and asked for authorization first. That was the right call at that scale.

**This document's title and original framing invited the reading that the leak was fixed.** It
was fixed at two call sites. The path that spawns a shell for a real user was not among them,
so any panicking terminal test still leaks its shell.

**Not yet fixed.** `runtime/terminal/launch.rs` needs the `KillOnDropChild` treatment the
approval call sites already have. Deliberately not folded into a UI slice.

**Whether the shipped application can trigger this is unknown and worth determining** — on
ordinary exit the PTY master closes and a shell normally takes `SIGHUP`, which is likely why no
user has reported it. That is a plausible mechanism, not a verified one, and it is stated here
as unverified rather than as reassurance. What is certain: RFC-039 PR-039-C adds a
user-triggered close path, which is exactly the shape that would make it reachable. That slice
is required to call `request_terminate` before closing — see
`039-interaction-model-and-visible-affordances/what-closing-a-project-must-not-lose.md` §6.

## The fix for the second cause — SCHEDULED 2026-08-25, ahead of RFC-040 PR-040-C's re-gate

Authorised by the human owner after a **second incident**: 3,899 leaked shells, `/dev/pts` at
4096, ~80 tests failing in 0.24 seconds because no PTY could be allocated. Everything leaked was
under 3.3 hours old — one working session, not accumulated debt.

**Its own slice, its own commit, its own review.** Not folded into a UI slice; RFC-040 PR-040-C
is complete and waiting, and it re-gates *after* this lands, because three more full-suite runs
against an unfixed leak recreate the problem the runs are meant to detect.

### What to build

`RunningTerminal` (`runtime/terminal/launch.rs`) holds a bare `Child` with **no `Drop` impl**.
`KillOnDropChild` — the fix the two `approval::tests` call sites already have — lives in
`test_support.rs`, which is `pub(crate)` and compiled in production, so it is reachable; whether
to reuse it or give `RunningTerminal` its own `Drop` is yours, with one constraint below.

### The constraint that makes this dangerous, and is not obvious

**Today, dropping a `RunningTerminal` means nothing. After this change it means killing a user's
running shell.** Every existing drop site was written when drop was harmless, and none of them
was reviewed against the consequence you are about to give it.

The sessions map (`HashMap<TerminalId, RunningTerminal>`) has **three `remove` and two `insert`
sites**. An `insert` over an existing key drops the old value; a `remove` whose result is
discarded drops it there.

**Required before writing the `Drop` impl:** enumerate every one of those five sites and state,
per site, whether a drop there means "this terminal is finished" or "this value is moving". If
any is the second, a naive `Drop` kills a live terminal the user is still using — a far worse
defect than the leak, and one no existing test would catch, because no existing test asserts that
a terminal *survives* an unrelated map operation.

That is the reachability discipline applied to a destructor: before making a drop consequential,
name every place it happens.

### The process group, not just the child

`RunningTerminal` carries `process_group_id`, and this product's real termination path
(`request_terminate`) signals the **group**, SIGTERM escalating to SIGKILL, guarded against
signalling a group id ≤ 1. A `Drop` that kills only `child` leaves the rest of the group.

Drop is a **last-resort safety net, not the normal path** — RFC-039 PR-039-C made
`request_terminate` the normal one. Decide deliberately what Drop does when the normal path has
already run (it must be idempotent and must not block), and say what you decided.

### Evidence

- **Show the leak happening, then not happening**, per this document's own standing instruction:
  process count before and after a deliberately panicking terminal test. `test_support.rs`'s own
  `KillOnDropChild` tests are the shape.
- A test that a live terminal **survives** an unrelated sessions-map operation, if any of the
  five sites turns out to be a move.
- Ablate by removing the `Drop` impl and confirming the leak test fails.

## Evidence, 2026-08-25: the second cause fixed, and a third found

**The five sites enumerated before writing the `Drop` impl**, per this document's own required
discipline. `self.sessions.remove`/`.insert` in `runtime/terminal/launch.rs` and
`termination.rs`: exactly three `remove`, two `insert` — grepped, not assumed. Both `insert`
sites key on a freshly minted `TerminalId::new_uuid()` (`TerminalSession::new`'s own
construction), so neither can ever evict a session actually stored under that key. All three
`remove` sites are reached only after the child has exited on its own
(`wait_for_child_outcome`, a real `try_wait` confirms it) or the process group is confirmed
gone or given up on after a full SIGTERM/SIGKILL escalation
(`wait_for_process_group_outcome`, `request_terminate`'s own final give-up arm). None removes a
session the caller still expects to keep running — every one of the five is "this terminal is
finished," never "this value is moving." No sixth site found (`grep -rn "\.sessions\b"` across
the module returns exactly these seven references, the other two being read-only `get`/`get_mut`
borrows).

**The `Drop` impl.** `RunningTerminal::drop` signals `-process_group_id` (not just `child`) with
`SIGKILL` directly — no SIGTERM grace period, since a destructor that blocks on a timeout is
unacceptable and `request_terminate` is the normal path this is a last-resort net under, not a
replacement for — guarded by the same `<= 1` refusal `send_signal_to_process_group` already
applies, then reaps this value's own direct child (`child.wait()`, harmless/`Err` if already
reaped). Idempotent by construction: fires on every one of the five sites above, not only a
leak, and a signal to an already-dead group or a `wait` on an already-reaped child both no-op
harmlessly.

**Leak shown happening, then not happening — ablated.**
`dropping_a_running_terminal_kills_the_real_process_group`
(`runtime::terminal::tests`): launches a real shell through `LinuxTerminalRuntime`, panics inside
`catch_unwind` before any explicit cleanup, asserts the real process is dead by the time
`catch_unwind` returns. Temporarily removed the `Drop` impl's body, re-ran this test alone:
failed, `process_is_alive` still `true` — the exact defect this fix exists to prevent,
reproduced on demand. Restored, re-ran, green.

**Insert-does-not-evict, proven against two real terminals.**
`launching_a_second_terminal_does_not_kill_the_first`: launches two real shells in the same
runtime, asserts the first is still alive after the second's own `insert`. Real cleanup via
`request_terminate`, not relying on the guarantee this test is not the one proving.

**Measured, the gate's own required form.** Before this fix (the incident that authorised it):
3,899 leaked `/bin/sh` processes, `/dev/pts` at its 4,096 limit, ~80 tests failing in
0.2–0.8 seconds — too fast to have run, meaning no PTY could be allocated at all. After: three
consecutive `cargo test --workspace --all-targets --all-features` runs, all clean (404 tekstide +
736 tekstide-core, the two new tests included), with the leaked-process count settling at a
small, **constant** 32 after every run (2 pre-existing/unrelated + a consistent 28 from one
specific test, not a growing pool) — see the third-cause finding below for what that 28 actually
is. `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --all-features -- -D
warnings` both clean throughout. `git diff --check` clean.

**Do not chase symptoms individually — not done.** No change to
`terminal_session_limit_headless_n_pane_wake_throughput_benchmark` or any other test that happens
to trigger the third cause below; the harness/runtime defect enumerated and fixed here is
`RunningTerminal`'s missing `Drop`, not any individual test's own shape.

### A third, distinct cause, found while verifying the second fix was complete

**Not fixed by this response. Disclosed, not chased into a fix, per this document's own standing
instruction not to scope-creep a UI/harness slice into a deeper redesign.**

Isolating `cargo test -p tekstide` alone (clean, 404/404, zero panics) still left 28 processes
behind — the fix above only accounts for zero. Bisecting by test name found the source:
`terminal_session_limit_headless_n_pane_wake_throughput_benchmark` alone leaks exactly 28
(matching its own `1+3+6+8+10` pane counts across five loop passes) — every one confirmed, via a
temporary diagnostic, to have had `Drop for RunningTerminal` actually fire and its `kill(-pgid,
SIGKILL)` return success. The 28 processes still alive afterward are a **different** set of pids
from every one that `Drop` signalled.

**CORRECTED 2026-08-26 — the reason below is wrong; the conclusion is right.** A backgrounded job
does get its own process group, but **not** because the shell has a controlling terminal.
Measured twice (no tty, and with a controlling tty via `pty.fork`): a *non-interactive* shell
keeps its `&` job in its own process group, `monitor` off. What actually does it is that this
application launches `/bin/sh` **with no `-c`**, reading from the PTY — which makes bash
*interactive*, which enables monitor mode, which is job control. The distinction decides the fix:
every job group lives **inside the shell's session**, so containment is reachable. See
`rfcs/proposed/043-terminal-process-containment.md` and, for the descriptor half — the actual root
cause of the leak's persistence — `rfcs/handoffs/pty-master-fd-inheritance.md`.

**The descriptor half — FIXED 2026-08-26.** `OpenPty::new` now sets `O_CLOEXEC` on both the PTY
master and slave, plus a belt-and-brace `close_range` in `spawn_pty_child`'s own `pre_exec` (both
independently verified against a real, ablated test —
`rfcs/handoffs/pty-master-fd-inheritance-qa-evidence.md`). **This does not fix the backgrounded job
escaping termination** — that is still RFC-043's job, undone below — but it closes the mechanism
that made the escape *self-sustaining*: measured directly, before this fix the 28 leaked shells
from `terminal_session_limit_headless_n_pane_wake_throughput_benchmark` stayed alive indefinitely
(`state=S`, blocked on a `printf` into a PTY another process still held the master of); after, the
same 28 moved to `state=R` (running, hitting `EIO` instead of blocking) and were gone within
`FLOOD_SCRIPT`'s own 30-second deadline, and `/dev/pts` occupancy never rose above baseline during
the run at all. The orphan still exists, briefly; it no longer holds a PTY slot hostage while it
does.

**Original text, retained:**

**Root cause: a backgrounded job gets its own process group.** `FLOOD_SCRIPT` (this test's own
helper, and `TerminalFlood`'s measurement path) ends its loop with `&`, "so the shell stays
interactive." `/bin/sh` on this machine is `bash`, and bash places a background job into its own
new process group when it has a controlling terminal — which every terminal this runtime launches
does (`spawn_pty_child`'s own `TIOCSCTTY`) — regardless of whether the shell is "interactive" in
the traditional sense. The leaked processes are each their own session/group leader (`PID ==
PGID`), consistent with being the backgrounded job, not the original shell (which the `Drop`
above did successfully kill, by pid, confirmed).

**This is not new, and not specific to `Drop`.** `send_signal_to_process_group` — the
already-reviewed, production `request_terminate` path — signals the identical single
`-process_group_id`. Verified directly: launched a real shell, wrote a backgrounded loop into it,
called `request_terminate` with real timeouts. It returned `Terminated { outcome:
KilledAfterTimeout { .. } }` — **reporting success** — while the backgrounded job was still alive
half a second later, orphaned to `systemd --user`. **The production termination path can report a
terminal terminated while a process it launched keeps running.** Whether a shipped Tekstide can
reach this (a user's own terminal running any command that backgrounds a job — `sleep 30 &`, a
long build with `make -j &`, and so on — inside a session the user then closes) is not yet
determined; this response only confirms the mechanism, against a synthetic script, not real usage
patterns.

**Why not fixed here:** the fix requires deciding what "closing a terminal" *should* mean for a
job the user backgrounded inside it — kill it too (a bigger blast radius than today's contract
describes), or leave it running by design (the way a real terminal emulator's own "close tab"
often works, and `nohup`'s entire reason to exist) — a product decision, not a mechanical one,
and out of a leak-fix response's own scope. A `cgroup`-based or PTY-session-wide (`kill(-sid)` /
rely on `SIGHUP` reaching the whole session rather than one process group) approach would need its
own investigation into whether it over-reaches (killing things the current single-group signal
correctly leaves alone).

**Practical effect on this document's own gate:** RFC-040 PR-040-C's re-gate is not blocked by
this — 28 leaked processes per run of one specific benchmark test is a bounded, understood cost,
not the unbounded pool-exhaustion cascade the second cause produced. Anyone re-running the full
suite repeatedly (as gates in this project require) should expect the leaked-process count to
climb by ~28 per run until something cleans them up, and should not mistake that for a
regression in this fix.

**The job-escapes-termination mechanism itself — FIXED 2026-08-26, RFC-043 PR-043-B.** The
product decision this section said was needed got made (RFC-043's own D1/D2: kill, but only
within the session -- see that RFC's "Decided on acceptance"), and `request_terminate`/
`RunningTerminal::drop` both now `SIGHUP` the session leader first, then enumerate and `SIGKILL`
whatever remains in the *session* (not the process group `send_signal_to_process_group` used to
target). Measured directly against this exact document's own repro shape (a real backgrounded
loop, a real close): **0** leaked processes after the 28-pane benchmark, not 28. The "practical
effect" paragraph above -- expect ~28/run to accumulate -- **no longer describes this suite**;
left in place above as history rather than deleted, since the number this document's own gate
users should now expect is zero, and knowing what changed matters more than a smaller number by
itself. **The close-confirmation dialog's wording, and
`terminal_process_groups_confirmed_empty`'s own rename/rewiring in `shell.rs` -- FIXED, RFC-043
PR-043-C.** The dialog names the consequence before the `Close` click is pressed (RFC-034 D4's
rule), and the field -- renamed `terminal_session_confirmed_empty` -- now reads
`SessionConfirmedEmpty`'s own real field directly; see the "Reviewer addition" section below for
why that specifically closes the `KilledAfterTimeout` question it left open.

**Reviewer addition, 2026-08-25 — this reaches the audit trail, which the finding as filed did
not say.**

RFC-039 PR-039-C's close path computes `fully_confirmed` from the termination outcome
(`shell.rs:3740`):

```rust
let confirmed = matches!(
    outcome,
    TerminationOutcome::Exited { .. }
        | TerminationOutcome::TerminatedBySignal { .. }
        | TerminationOutcome::KilledAfterTimeout { .. }
);
```

`KilledAfterTimeout` counts as confirmation — and `KilledAfterTimeout` is exactly what the
investigation above observed `request_terminate` returning **while the backgrounded job was still
alive**. So `SafeCloseDecision::Closed { fully_confirmed: true }` can be written to the durable
audit store while a process that terminal launched keeps running.

RFC-013 anticipated half of this and does **not** cover this half. Its rule — *"a safe-close
`applied` outcome means Tekstide issued the selected terminate/abandon action; it does not mean
the process exited"* — makes the *outcome kind* honest. `fully_confirmed` is a separate, stronger
field added later by RFC-039 PR-039-C, and its name and meaning assert that termination **was**
confirmed. That assertion can be false.

This is the class this project treats most seriously: a durable record claiming more than it
knows, like the transcript privacy claim and restricted mode's blocked-feature count. Bounded —
it needs a backgrounded job — and not urgent, but not cosmetic either.

**The question to answer, not decided here:** is `KilledAfterTimeout` confirmation at all? It
means the escalation ran and observation was given up on, which is weaker than `Exited` or
`TerminatedBySignal`. Narrowing `confirmed` to exclude it, re-checking group emptiness after the
kill, or renaming the field to what it can support are three answers with different costs.
Whoever takes the third cause takes this with it.

**Answered, RFC-043 PR-043-C: none of the three, directly.** The field no longer asks "was this
outcome variant one of the confirmed-looking ones" at all -- it reads `SessionConfirmedEmpty`'s
own `confirmed` field, D3's real, independent re-enumeration of the *whole session*, taken
immediately before `request_terminate` returns. `KilledAfterTimeout` (or any other outcome) no
longer has anything to do with the computation; a `KilledAfterTimeout` paired with a real
`confirmed: true` correctly reports `true`, and a clean `Exited` paired with a real
`confirmed: false` (session re-check failed) correctly reports `false` -- proven in both
directions by `terminated_outcome_and_session_confirmation_does_not_infer_true_from_a_clean_exit`/
`..._does_not_infer_false_from_an_orphaned_outcome` (`shell/tests.rs`). This is closer to
"re-checking [session] emptiness after the kill" than to a narrowed match or a bare rename -- it
replaces the inference with the actual re-check RFC-043 D3 already performs, rather than trying to
approximate that re-check from a different signal.

**The sixth row is a candidate and is marked as one.** Observed **once in ten** full-workspace
runs by the reviewer, in the same run as `command_approval_family_...` — the co-occurrence is the
main evidence that it shares their cause rather than being a regression in the slice that
surfaced it. Not reproduced in nine further workspace runs, four `-p tekstide` runs, or five runs
in isolation.

**The reviewer did not capture its assertion message**, so what it actually failed on is unknown.
That is a weaker record than the other five and is stated as such rather than dressed up: if it
recurs, capture the message first — that is the difference between a sixth symptom and a sixth
guess.

## Recurrence, 2026-08-26 — response 329's required re-gate

Response 329 (RFC-041) required three consecutive full-workspace runs, recorded against this
table, after an earlier single-run gate was judged insufficient. Run 1: clean. **Run 2:
`approval::tests::coordinator::is_still_answerable_reflects_the_real_connection_state` failed** —
row 5 above, recurring, not a new test. Run 3: clean.

**The assertion message was not captured here either**, for the same reason as row 6: the run was
filtered to `test result:|FAILED` lines only, so the panic detail scrolled past uncaptured before
the omission was noticed. Fifteen further full-workspace runs immediately afterward, each logged
in full this time, did not reproduce it — consistent with the low, intermittent rate already
recorded for this row rather than a new or worsening cause. Whoever next hits this: redirect to a
file first (`cargo test ... > run.log 2>&1`), not a live grep, so the message survives.

*(Both this section and row 6 were first dated 2026-08-25 for work done on 2026-08-26; corrected
in RFC-041's closeout commit. The reviewer wrote the first wrong date and the dev team followed
it. In a table whose only purpose is correlating an intermittent failure over time, the date is
the data.)*

## Recurrence, 2026-08-26 — RFC-042 response 331's required re-gate

Response 331 required re-verifying the gate after three required fixes. Redirected every run to a
file this time, per this document's own standing advice. Runs 1 and 2 (of six total run) both
failed `command_approval_family_produces_real_durable_audit_records_through_the_pipeline` — row 3,
its first captured assertion message:

```
expected a real CommandRequest record for this agent run: [DurableAuditRecordV1 { ...,
family: CommandApproval, outcome: Applied, ... }, DurableAuditRecordV1 { ..., family:
CommandApproval, outcome: Authorized, ... }]
```

Two `CommandApproval` records present (`Authorized` then `Applied`) but the expected
`CommandRequest` record for the same operation absent — consistent with contention/ordering under
load, not a logic defect in this row's own already-diagnosed class. Runs 3 through 6, immediately
after, all clean. Unrelated to RFC-042 (no approval/audit code touched by that slice). Two
consecutive failures is more than the historically recorded ~2% rate would predict for one pair,
but not enough of a sample to revise the rate on -- recorded as an observation, not a new finding.

## ROOT CAUSE, CONFIRMED 2026-08-26 — one shared SQLite audit store, accessed in parallel

**This section replaces a wrong hypothesis published earlier the same day.** That version blamed
`AuditQuery::latest(50)` truncation. **It was wrong**, and the experiment that would have
confirmed it refuted it instead. What is below is measured, not reasoned.

### What was measured

Every run against a **fresh** `XDG_STATE_HOME`, so nothing depends on accumulated history:

| Condition | Result |
| --- | --- |
| Parallel (default), fresh store | **6 failures** |
| Parallel, `latest(50)` raised to `latest(100000)` — three runs | **17, 23, 17 failures** |
| **Serial (`--test-threads=1`), fresh store** | **444 passed, 0 failures** |

A full suite run appends **111–130 records** to the store — comfortably past the 50-record
window. **Serial passes anyway.** So a test's own record is found without difficulty when nothing
else is writing concurrently, and the window size is not the cause.

Raising the window made things **worse**, not better — consistent with longer-held read locks
under contention, and flatly inconsistent with truncation.

### The actual cause

**Every test that calls `open_real_audit_store` shares one SQLite database, and they run in
parallel.** `AppStatePathProvider::linux_default()` resolves `$XDG_STATE_HOME`, falling back to
`$HOME/.local/state/tekstide`. One path, 23 call sites, no per-test isolation —
`temp_audit_state_dir` exists and is used by other tests, but not on this path.

Two distinct failure signatures, both from that one cause:

1. **`the real audit store must open`** — `open_real_audit_store` returns `None`. The store could
   not be opened at all under concurrent access.
2. **`left: 0, right: 1` / `[]`** — the store opened, and the query returned nothing for this
   test's project.

Both are intermittent, both are load-dependent, and both vanish serially.

### What this explains

**Every audit-store row in the table above, including row 3** — on this list since request 276,
never reproduced in isolation, cause never established. It was never reproducible in isolation
because **isolation is the condition under which it passes.** Every attempt to chase it,
reviewer's and implementer's alike, was made under the one condition that hides it.

### The second problem, unchanged and independent

With `XDG_STATE_HOME` unset — the ordinary case — the suite reads and writes **the developer's
real audit store**. `shell/tests.rs`'s own comment on `fresh_state_root_dir` says a test state
root "must never be the developer's real `$XDG_STATE_HOME`"; that discipline was applied to the
transcript root and not to this path. Running the suite has been appending to a real user's audit
store for the life of this project.

### What a fix must do, and what it must not

**Isolate the store per test.** Not query tuning: the measurements above rule that out directly,
and a suite whose correctness depends on a query limit is wrong even when it passes.

**Do not "fix" this by serialising the tests.** Serial execution is the diagnostic that found the
cause, not the remedy — it trades a 5-second parallel suite for a 25-second serial one and leaves
the shared-real-store problem in place.

See `rfcs/handoffs/audit-store-test-isolation.md`.

### Fixed 2026-08-26

Isolated per test, automatically, with no per-test opt-in required (see that handoff's own
closeout for why an opt-in-only first attempt was insufficient: 58 unrelated tests transitively
reach `open_real_audit_store` through production write call sites inside `update()`, not just the
23 read call sites the handoff had counted). A belt-and-suspenders runtime assertion also panics
loudly if a test's resolved directory ever coincides with the real one, ablated and restored
during that fix's own review. Verified: 5 consecutive clean full-workspace parallel runs plus a
serial run, all against a fresh `XDG_STATE_HOME`; a real-`$HOME` run before/after diff confirmed
the audit subtree itself untouched (a separate, still-open gap in the adjacent `transcripts/`
subtree was found during that same diff -- see the status field above and that handoff's own
closeout for the reason it is not fixed here).

### This document's name no longer matches its contents

It began as a process-leak investigation. It is now the project's flake register: three leak
causes, six symptom rows, and this — most of which are audit-store, not process-leak. Worth
renaming when someone next touches it.

The affected area is **the command-approval and socket path** — the security-critical machinery
RFC-021 and RFC-022 built. A suite that fails intermittently there trains everyone to re-run
rather than investigate, which is precisely the habit that hides a real regression in that code
the first time one appears.

## What to build

A guard that kills a spawned child on drop, applied to the test helpers that spawn real
processes, so a panicking test cannot leak one. `std::process::Child::kill` plus `wait`, in a
`Drop` impl on a wrapper the helpers return instead of a bare `Child`.

Find every test helper that spawns a real process and returns it. The reachability-audit
technique applies: enumerate them mechanically rather than by reading.

## The gate

- **Measure before and after, the same way.** The existing figure is 3/150 full-suite runs. A
  fix claimed without a comparable post-measurement is a fix claimed on hope — and this
  project's own convention is to measure bounds rather than estimate them.

  **Corrected 2026-08-20, at acceptance: this gate item pointed the measurement at the wrong
  quantity, and it is mine.** The 3-in-150 baseline measures the *ambient* rate of the
  `bind_recovers_from_a_stale_socket_file` flake. This fix does not target that. It prevents a
  **panicking** test from leaking a process — so in a run where nothing panics, which is every
  passing run, the fix cannot have an effect and a before/after comparison of passing runs
  measures load, not the change. The implementer ran the comparison anyway (27/200 vs 29/200),
  recognised it did not discriminate, and said so instead of reporting the number. The property
  this fix actually has is **cascade reduction**: after any first failure, a leaked process no
  longer contends with tests still running in the same binary. The direct leak-then-no-leak
  demonstration proves the mechanism; no run-rate comparison can, without seeding a panic, and
  nothing is being sized by the number.
- **Show a leak happening, then not happening.** A test that panics deliberately, with the
  process count observed before and after. Without that, "no leaks" is unfalsifiable.
- **Do not chase the three symptoms individually.** If the cause is fixed and one still flakes,
  that is a *second* finding and worth having — but fixing the tests rather than the harness
  would hide it.

## What this does not establish — and the one that matters most

**That the socket flake is fixed. It is not.**
`approval::tests::channel::bind_recovers_from_a_stale_socket_file` has its own, separate cause
and will still fail intermittently. This handoff's title and the three tests listed above
invited the reading that repairing the leak repairs the flakes; it does not. The leak makes a
bad run *worse* by cascading after a first failure — it is not why the first one happens.
**Anyone reading a green suite after this and concluding the flake is gone will be wrong**, and
the next disclosure of it is not a regression.

That the product leaks processes. This is a test-harness defect: `Child::drop`'s documented
behaviour, met by helpers that assume otherwise. Nothing here says a shipped Tekstide leaks
anything, and the closeout must not imply it does.

## Evidence, 2026-08-20

**The guard.** `KillOnDropChild` (`crates/tekstide-core/src/test_support.rs`, alongside the
existing `RealProcessLimiter` this crate's real-process tests already share): `Drop::drop` kills
and reaps the wrapped `Child`, swallowing any error from an already-exited process (`let _ =`).
`kill`/`wait` proxy directly (`&mut self`, matching `Child`'s own signatures), so a caller that
already kills and reaps manually before this fix
(`reference_adapter.rs`'s `deciding_a_proposal_whose_real_adapter_process_has_already_exited_is_undeliverable`)
needed no change beyond the return type. `wait_with_output` takes `self` by value; the wrapped
`Child` is `Option`-held and `.take()`n there specifically, since a type implementing `Drop`
cannot have a field moved out of it directly.

**Every real-process-spawning test helper in the workspace found and wired**, mechanically: `grep
-rn "\.spawn()"` across `crates/` returned exactly three call sites total. One is production code
(`runtime/terminal/launch.rs`, out of scope — this is a test-harness fix). The other two are both
in `approval::tests`: `reference_adapter.rs`'s `spawn_adapter` helper (7 call sites reached through
it) and one inline spawn in `channel.rs`
(`a_real_process_presenting_the_wrong_token_over_a_separate_connection_is_rejected` — the exact
test name grep found it in). Both now return/hold `KillOnDropChild` instead of a bare
`std::process::Child`.

**Show a leak happening, then not happening** — the gate's own required form, in
`test_support.rs`'s own test module:

- `a_bare_child_leaks_across_a_panic_this_fix_exists_to_prevent`: a bare `Child` moved into a
  closure that panics; `catch_unwind` contains the panic; the real process (checked via
  `libc::kill(pid, 0)`, mirroring `runtime::terminal::termination::process_group_exists_by_id`'s
  own technique) is still alive afterward. Manually killed at the end, since this test's whole
  point is that nothing else would have.
- `kill_on_drop_child_does_not_leak_across_a_panic`: the identical scenario, `KillOnDropChild` in
  place of the bare `Child`. The process is gone — killed *and* reaped, not merely signalled —
  by the time `catch_unwind` returns, since `Drop::drop` runs synchronously during unwinding.
- `kill_on_drop_child_cleans_up_on_ordinary_drop_too`: the non-panicking path, proven separately,
  since a `Drop` impl exercised only via the panic path is not proven for the ordinary one.
- `wait_with_output_returns_the_real_exit_status`: the guard does not break the happy path —
  real exit status, real stdout, still correct after passing through it.

All four go through `RealProcessLimiter::acquire()` themselves, as the first local, matching
every other real-process test in this crate — an early version of this work omitted that and
measurably worsened contention on `approval::tests::channel::bind_recovers_from_a_stale_socket_file`
(see below) before the slot was added.

**Measured, and the measurement needed a correction along the way.** The originally-planned
"before vs after, the same way" comparison — repeated `cargo test`/direct-binary runs of
`approval::` — was run at N=200 both ways (this session's own machine, heavily loaded from a long
session of back-to-back builds and test runs): **27/200 (13.5%) unfixed, 29/200 (14.5%) fixed** —
statistically indistinguishable, both far above the historically recorded 3/150 (~2%) baseline.

**That comparison does not actually test what this fix changes, and it would have been dishonest
to present it as if it did.** Each of the 200 iterations invokes the compiled test binary as a
*fresh, independent OS process* — `approval::`-filtered, so none of the new `test_support::tests`
panic-and-leak tests run inside it. No test in a normal, passing `approval::` run panics at all,
so the leak this fix prevents never has an opportunity to occur during either the "before" or
"after" loop. Both loops necessarily measure the *ambient* rate of the pre-existing,
already-disclosed `bind_recovers_from_a_stale_socket_file` flake under this session's own current
system load — a real and useful number, but not a measurement of this fix's effect. The mechanism
this fix addresses only matters *within* a single, longer-lived test-binary invocation where an
unrelated test's real panic leaks a process that then contends with others still running in that
same process — which is what `RealProcessLimiter`'s own doc describes ("wall-clock overlap between
different test functions' real processes... within a single test binary run"), not something a
loop of independent, single-filter process invocations can reproduce.

**What was actually proven, stated precisely**: the causal mechanism — `Child::drop` leaks, this
guard does not — is proven directly and unambiguously by the four tests above. Whether that
translates into a lower *observed* full-suite flake rate depends on how often a real, unrelated
panic happens to occur near in time to `bind_recovers_from_a_stale_socket_file` (or either of the
other two historically-affected tests) within the *same* test-binary process — a condition this
session cannot control or reproduce cleanly in a short loop, and the recorded 3-in-150 baseline
itself was almost certainly gathered the same way: incidentally, across ordinary development
activity, not a tight synthetic repeat. The correct claim is the direct one; the loop comparison
is disclosed rather than presented as if it settled the question either way.

**Do not chase the three symptoms individually — not done.** No change to any of the three
named tests (`bind_recovers_from_a_stale_socket_file`,
`agent_run_queue_limit_is_enforced_and_only_counts_live_entries`,
`command_approval_family_produces_real_durable_audit_records_through_the_pipeline`) — the harness
defect is fixed at its source, not any individual symptom.

**Gates run**, 2026-08-20: `cargo fmt --all --check` clean. `cargo clippy --workspace
--all-targets --all-features -- -D warnings` clean. `cargo test --workspace --all-targets
--all-features` run three times, all fully clean, after both this fix and the separate,
same-session RFC-023 PR-023-E work were in the tree together: `tekstide` 311 passed,
`tekstide-core` 713 passed (this fix's own four new `test_support` tests are 699 of the count
increase from the last-recorded 695; PR-023-E's own fourteen account for the rest — the two were
not gated in isolation from each other, only combined, since both landed the same session before
either was reviewed), `reference_adapter` 0 tests. `git diff --check` clean. Committed as
`f0c5055`, staged by explicit path, separately from the unrelated RFC-023 work (`855d063`).

## Recurrence, 2026-08-27 — RFC-044 PR-044-C's required re-gate

RFC-044 PR-044-C required three consecutive full-workspace runs. Run 2 of the first pass failed
one test in the `tekstide-core` binary (**745 passed, 1 failed**, `tekstide`'s own 456 and
`rfc_docs_invariants`'s 4 both clean the same run). Runs 1 and 3 of that pass were clean.

**The assertion message was not captured** — the same mistake this document already names as the
difference between a sixth symptom and a sixth guess (see the row 6 section above): the run was
piped through `grep "test result:"` rather than redirected to a file, so the panic detail scrolled
past before the omission was noticed.

Failing in `tekstide-core` rules out rows 3, 4, and 6 categorically, the same reachability
argument this document already makes for rows 1, 2, and 5: `tekstide-core` has no access to
`open_real_audit_store`, so this is not that fix's own already-closed cause recurring. Consistent
with row 1, 2, or 5 instead, but not identifiable further than that without the message.

Twelve further runs immediately after, this time redirected to a file per this document's own
standing advice, were all clean (four `cargo test --workspace`, eight more of the same). One
failure in thirteen total runs (~7.7%) is within the range this document already records for this
class of flake and is not itself new information — recorded so a future recurrence in
`tekstide-core` with a captured message can be checked against this one by binary and count alone.
**Whoever next hits this in `tekstide-core`: redirect to a file first, and check whether it names
one of the three tests already tied to rows 1, 2, or 5** — that comparison is what turns this row
from a guess into a match.

Unrelated to RFC-044 (no `tekstide-core` code touched by that slice — the whole slice is
`tekstide`-crate-local: `keyboard_help.rs`, `en.ftl`, `shell.rs`, and their own tests).

## Recurrence, 2026-08-28 — RFC-047 PR-047-A's own gate

`bind_recovers_from_a_stale_socket_file` failed once in three full-workspace runs during PR-047-A's
gate (run 1 of 3; runs 2 and 3 clean, 460 + 4 + 738 both times). Assertion message captured this
time: `second bind must clear the stale file and succeed: ApprovalChannelError { reason: Io,
source: None }` — the same test and shape this document's own historical ~2% baseline already
covers, not a new cause. Unrelated to RFC-047 (no `approval::channel` code touched by that slice —
the whole slice so far is `shell.rs`, `main.rs`, `tests.rs`, and `AuditHealth::record_failure`'s
own visibility in `tekstide-core/src/audit/integration.rs`).

## Recurrence, 2026-08-28 — RFC-047 PR-047-A, again, third run of the same three-run gate

`shell::tests::closing_a_project_with_a_backgrounded_descendant_kills_it_through_a_real_close`
failed once, in the third and final of three full-workspace runs (the first two clean; the first
of the three, separately, hit the unrelated stale-socket recurrence logged just above). Same row
this document already carries (`0.16.0`'s own release gate, table row 8) — same assertion message,
captured again: *"the marker must be followed by a real, parseable PID"*, the read returning only
the shell's echo of the command line, not yet the `descendant-pid:` line. Not a new cause; recorded
so the row's own "once in three runs" rate has a second data point rather than resting on one.
Unrelated to RFC-047 — no terminal-termination code touched by this slice.

## Recurrence, 2026-08-28 — RFC-047 PR-047-A, third gate attempt

`approval::tests::coordinator::is_still_answerable_reflects_the_real_connection_state` failed once
(row 5's own test, first reported request 326), the only failure in that run's 737/738. Assertion
message: *"once the peer closes its end, the same request must no longer be answerable"* — the same
shape row 5 already names. Two further full-workspace runs immediately after, clean (460 + 4 + 738
both times). Three known, already-disclosed flakes recurring across this slice's own gate attempts
in one session (this row, the stale-socket row, and the backgrounded-descendant row above) is more
than usual, plausibly load from this same session's own repeated `cargo build`/`cargo test`
invocations rather than a new cause — consistent with this document's own prior observation that
its recorded rates were measured "incidentally, across ordinary development activity." Not treated
as a new finding; each recurrence is dated and message-captured per this document's own standing
rule, and none touches code this slice changed.

## Recurrence, 2026-09-05 — PR-047-C response 361's review gate (reviewer's run)

`approval::tests::coordinator::is_still_answerable_reflects_the_real_connection_state` failed once,
in run 2 of a three-run full-workspace gate. Row 5's recurring intermittent, unchanged in shape and
consistent with its existing low rate.

**Two caveats specific to this observation, recorded so it is not read as stronger evidence than it
is.** First, that same gate's run 3 also reported a failure —
`shell::tests::trust_grant_dialog_degraded_notice_does_not_imply_unsafe_or_fixable` — which was
**not a flake and not a defect**: the reviewer was running an `en.ftl` ablation against the same
working tree while that background gate was compiling, so run 3 built against a deliberately
corrupted catalog string. A mutating ablation and a background gate must not share a working tree;
the gate result is void, not merely noisy.

Second, the clean re-run immediately afterwards (tree untouched, nothing else running) was **476 + 4
+ 741 green three times**, so this row records the `is_still_answerable` occurrence only, and the
release-relevant gate for PR-047-C is the clean one.

## Recurrence, 2026-09-10 — RFC-046's close gate (reviewer's run)

`approval::tests::channel::bind_recovers_from_a_stale_socket_file` failed once in run 1 of a
three-run full-workspace gate on the closed tree; runs 2 and 3 clean at 487 + 4 + 746. The same
test and shape this document's ~2% historical baseline already covers.

**Rate, measured rather than impressionistic:** this is **one failure in roughly twenty-seven
full-workspace runs** by the reviewer across responses 365–373 — consistent with the existing
baseline, not elevated.

**One thing changed in this window that touches the same resource, recorded without claiming it is
the cause.** RFC-046 PR-046-B added `managed_launch_carries_a_real_bound_approval_endpoint`, which
binds a **real Unix domain socket** — the first test outside `approval::tests::channel` to do so. It
runs in the same binary, in parallel, and derives its directory from the test process's own PID
(`$TMPDIR/tk<pid>`), so every test in that binary shares the one path.

**No evidence links the two**, and a single observation at the baseline rate is not evidence. It is
recorded here only so that if this row's rate ever does climb, the next reader has the one candidate
that was introduced in the same window, instead of re-deriving it. Do not treat this paragraph as a
finding.

## Recurrence, 2026-09-10 — `0.17.0`'s release gate, and a reviewer mistake made for the third time

`shell::tests::change_review_content_view_build_cost_by_line_count_measurement` failed once in run 3
of the release gate's three runs. Runs 1 and 2 were green at 487 + 4 + 746.

**Evidence it was load, not a regression** — circumstantial, and labelled as such:

- Run 3's `tekstide` suite took **9.00s** against 5.85s and 6.24s for runs 1 and 2 on the same tree.
- `uptime` shortly afterwards showed a 5-minute load average of **5.63** and a 15-minute of 6.36.
- The test passes in isolation (0.25s), and **three further full-workspace runs immediately after
  were green** — including one where the load average reached **14.25**, higher than during the
  failure. A budget that holds at load 14 did not fail at load ~6 because of a code change.

**The test itself is exemplary and should be left alone.** Its assertion reads `/proc/loadavg` and
its own failure message says: *"This is either a real regression or this machine was under load when
it ran… A number well above the core count points at load, not a regression — rerun on an idle
machine before treating this as one."* That is precisely the correction made after the earlier
wall-clock assertion whose message claimed a failure **could not** be load.

**And the reviewer threw that away.** The gate was run as
`cargo test --workspace 2>&1 | grep -E "^test result:|FAILED|^error"`, which discards the panic body
— so the `elapsed_ms` and the load figure the test had gone to the trouble of measuring **were never
captured**. This document already records the same mistake twice (rows 5 and 6, both noting the run
was filtered to `test result:|FAILED` only). **This is the third time**, and the first two were the
dev team's while this one is the reviewer's, which makes it a property of the habit rather than of
the person.

**The rule, stated so it is not relearned a fourth time: a gate run whose output is filtered cannot
report a flake.** Redirect the full output to a file and grep the file. `0.17.0`'s re-run did that
(`.git-exclude/release-evidence/0.17.0/gate-full-output.txt`), which is why its three green runs are
worth more than the first attempt's two.

## Recurrence, 2026-09-12 — RFC-045 PR-045-A's gate, and what a self-inflicted load average buys

**Four failures in thirty full-workspace runs on the PR-045-A tree**, across three already-recorded
tests and one member of the same load-sensitive family:

| Test | Runs | Register row |
| --- | --- | --- |
| `approval::tests::channel::bind_recovers_from_a_stale_socket_file` | twice | row 1 |
| `approval::tests::coordinator::is_still_answerable_reflects_the_real_connection_state` | once | row 5 |
| `surface::terminal::tests::resize_makes_the_pty_the_emulator_and_the_render_path_agree` | once | new here — PTY timing, the same shape as row 7's cause, not process leak |

**13% is well above this document's ~2% baseline, and the load was the implementer's own.** Every
failure landed while running suites back to back — `uptime` read 4.65 at the first, 7.42 and 11.82
during the middle rounds, and **28.18** by the end, on a machine whose ordinary gate runs sit near
5. Twenty consecutive `-p tekstide-core` runs alone were **green**; eight consecutive
full-workspace runs in the quietest stretch were **green**. The failures cluster with load, not
with the change.

**The change itself has no mechanism here.** PR-045-A is a TOML parser in `tekstide-core`: no
sockets, no PTY, no processes, no audit store. The only contact with these tests is that it adds
nine net tests to the same binary, which is a 1.2% increase in that binary's parallel test count.
Recorded as the one candidate introduced in the same window, per the 2026-09-10 entry's own
convention — **not** as a finding.

### A controlled before/after was attempted twice and is not possible this way

Both attempts compared the working tree against a `git worktree` checked out at `HEAD`, and both
produced a baseline that failed **every** run for reasons that are properties of worktrees, not of
the code:

1. `cargo test --workspace` in the worktree ran before its own `[[bin]]` targets were built —
   23 deterministic failures per run, all `expected the reference_adapter binary at …`.
2. After `cargo build --workspace --bins` fixed that, the baseline still failed every run:
   **a worktree's `.git` is a file, not a directory**, so
   `project::tests::change_detection::real_repository_filesystem_scan_cost_headless_benchmark`
   refuses to run there — *"must be measuring a real repository with a real .git/, found none"*.

**So there is no controlled baseline for the numbers above, and they must not be read as one.**
The next person wanting one should use a second full clone, not a worktree, and should not run the
two trees back to back on one machine — doing so is what produced the load that this entry is
mostly about. Recorded so the attempt is not repeated a third time.

## Recurrence, 2026-09-12 — RFC-045 PR-045-C's review gate (reviewer's run), message captured

`approval::tests::coordinator::is_still_answerable_reflects_the_real_connection_state` failed once,
in run 3 of three full-workspace runs on `072c5df`; runs 1 and 2 green at 507 + 4 + 758. Row 5's
recurring intermittent, at its baseline rate. One-minute load average after the run: ?.

**The assertion message, captured this time** — the gate was run with output redirected to a file,
per the rule this document added on 2026-09-10 after losing it three times:

```
thread 'approval::tests::coordinator::is_still_answerable_reflects_the_real_connection_state' (2271980) panicked at crates/tekstide-core/src/approval/tests/coordinator.rs:350:5:
once the peer closes its end, the same request must no longer be answerable
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

Recorded so row 5 finally has one verbatim message on file. No new candidate: PR-045-C touches
configuration, a modal, and the audit producers; nothing near `approval::channel`.

## Recurrence, 2026-09-12 — RFC-045's close gate (reviewer's run), row 3's message captured

`approval::tests::coordinator::agent_run_queue_limit_is_enforced_and_only_counts_live_entries`
failed once in run 1 of three full-workspace runs on `0b72679`; runs 2 and 3 green at 507 + 4 + 758.
**Row 3** of the table above — first reported at request 260 — not a new test.

**The assertion message, captured** (gate output redirected to a file, per the 2026-09-10 rule):

```
an expired entry must not continue occupying the live budget: QueueLimitExceeded { scope: PerAgentRun, limit: 2 }
```

**Same mechanism as row 5, stated because the two rows have never been connected here.** The test
does `drop(first_peer)` and then requires the coordinator to see that entry as expired on the very
next receive. Row 5's `is_still_answerable_reflects_the_real_connection_state` requires the same
observation — *"once the peer closes its end, the same request must no longer be answerable"*, the
message captured on 2026-09-12 one entry above. **Both are a socket-close observation race**: the
close is initiated locally, and the coordinator's view of it is not instantaneous under load. They
are one cause with two names, and a future attempt to chase either should treat them as one.

**No mechanism from the change.** RFC-045's commits touch `config/*`, `navigation.rs`, and the
`tekstide` crate's shell — no `approval` code in either crate. Recorded as a known intermittent at
its baseline rate, not as a candidate.

## Recurrence, 2026-09-12 — `0.18.0`'s release-candidate gate (reviewer's run)

`approval::tests::channel::bind_recovers_from_a_stale_socket_file` failed once in run 2 of three
full-workspace runs on candidate `45aeb7b`; runs 1 and 3 green at 507 + 4 + 758. **Row 1** — the
original, response 213.

```
second bind must clear the stale file and succeed: ApprovalChannelError { reason: Io, source: None }
```

Same message as the 2026-08-28 occurrence, so this row's shape is now confirmed across three weeks
rather than asserted.

**Load was 18.18, and the reviewer's own back-to-back gate runs drove it** — the same self-inflicted
pattern the PR-045-A entry above documents, this time on the reviewing side. Worth stating plainly:
a three-run gate on one machine *is* a load generator, so a flake observed during one is observed
under conditions the release itself will never see.

**Row 1 is not rows 3 and 5.** Those two are a socket-close *observation* race, connected in the
entry above. This is a **bind against a stale socket file** — a different mechanism, and it must not
be folded into them by a future reader tidying the register.

**No mechanism from the candidate.** `0.18.0` carries RFC-045 (configuration, navigation, shell) and
a dependency bump; no `approval` code changed in either.

## Two observations, 2026-09-13 — RFC-050 PR-050-A, both under load I generated

**1. `transcript::tests::a_live_writer_holds_an_exclusive_lock_until_it_is_dropped` — a new test, and a
mechanism no row above has.** Its second assertion — the lock is free straight after the writer drops
— failed once during an ablation run. Measured afterwards in a loop:

| Run alongside | Runs | Release assertion failed |
| --- | --- | --- |
| the two PTY-spawning launch tests | 40 | **1** |
| nothing else | 40 | **0** |

The first assertion (the lock is held while the writer lives) never failed, and could not fail this
way: the race only makes a lock look held for longer.

**Mechanism — inferred from that measurement and from `flock` semantics, not observed directly.** A
concurrent `fork` copies every open descriptor into the child until its `exec` closes them, and an
`flock` lasts until the last copy of the open file description closes. So for the width of another
thread's fork-to-exec window, a dropped writer's lock is still held.

**Fixed in the test, not tolerated as a flake**: release is asserted within a 5 s bound. A writer that
genuinely never releases still fails it, and that is ablated. **Product consequence, for PR-050-B**:
the load-time probe can briefly read a writer that has just finished as live. That errs toward
keeping, which is the direction RFC-049 §1 requires.

**2. The two `/dev/full` reader tests — first recorded here as load-induced. That was wrong; corrected
2026-09-15.**

`local_bounded_marks_capture_failed_and_keeps_reading_when_the_transcript_is_genuinely_unwritable`
and `required_local_bounded_marks_capture_failed_stops_reading_and_stalls_the_child_without_killing_it`
both time out with:

```
transcript_write_summary did not reach CaptureFailed within 5s (last seen: None)
```

**The original entry said the load was mine and the change had no part in it.** Both halves were
wrong. The next ablation run showed these tests failing inside ablations that do not touch them, and
under the ablation that refuses a launch on `LockUnavailable` the failure named the cause directly:
`transcript write failed: LockUnavailable … agent-run-…/transcript.log`, on a path symlinked to
`/dev/full`.

**Mechanism, confirmed from the fixture.** Both tests go through
`launch_with_unwritable_transcript_capture`, which symlinks the transcript path to `/dev/full` — the
**same device inode** for both — and `RealProcessLimiter` admits six concurrent real-process tests.
Since PR-050-A the writer takes an `flock` on its file. When the two overlap, the second cannot take
the lock, its launch correctly starts **without capture** (RFC-050 D3), and it then waits for a
`CaptureFailed` that cannot come. Alone, each passes every time, which is why 5 isolated runs of 5
proved nothing about the cause.

**First fixed with a test-only mutex, and that fix was wrong in kind.** Both tests held a shared mutex
for their whole duration, so they could not overlap. **Response 388 removed it and narrowed D3
instead: only regular files are locked.** The loader only ever loads regular files (D7), so a lock on
a FIFO or a device protects nothing, and on a shared device it serialises unrelated writers — which is
exactly what the mutex was hiding rather than fixing. One `fstat` on the opened handle now decides
both the lock and the truncate, and the two tests run concurrently again with no mutex.

**The lesson, stated so it is not repeated:** a failure that passes in isolation was treated as load
without checking whether *my* change made concurrent tests interact. Isolation shows a test is not
broken on its own; it does not show the change is innocent.

## Recurrence, 2026-09-15 — RFC-050 PR-050-A's ablation run

`approval::tests::channel::bind_recovers_from_a_stale_socket_file` failed once, in the whole-library
run under ablation L8 (which removes UUID parsing from `from_persisted`). **Row 1** — the original,
response 213.

**Not the ablation, and not the slice.** L8 changes id validation; this test binds an approval socket
over a stale socket file and uses no persisted id. PR-050-A touches no `approval` code. Every other
run this slice made, before and after, passed it.

**The machine was not quiet.** Another project's `cargo build --release`, a wasm build, and a
`cargo test --all-features` were running throughout — none of them mine. Stated because a failure
observed during unrelated heavy load is observed under conditions this test's own runs never set.

## Recurrence, 2026-09-15 — RFC-050 PR-050-A's review (reviewer's run)

`approval::tests::coordinator::agent_run_queue_limit_is_enforced_and_only_counts_live_entries` failed
once, in a `-p tekstide-core` run under the reviewer's ablation M1, which treats every found-on-disk
transcript as not live. **This is the row first reported at request 260.**

**Not the ablation.** M1 changes one match arm in retention's liveness predicate. This test exercises
the approval coordinator's queue limit and touches no transcript. Afterwards it **passed 20 of 20 runs
alone**, and passed in all three of the reviewer's consecutive full-workspace runs on the restored
tree. The five-minute load average was 7.18 just after that gate, from the reviewer's own runs.

## Recurrence, 2026-09-15 — RFC-050 PR-050-A follow-up's review (reviewer's run)

`approval::tests::channel::bind_recovers_from_a_stale_socket_file` failed once, in a `-p tekstide-core`
run under the reviewer's ablation G4, which truncates a regular transcript file before taking its
lock. **This is row 1, the original from response 213.**

**Not the ablation.** G4 changes one line in `BoundedTranscriptWriter::create`. This test binds an
approval socket over a stale socket file, and no transcript writer is involved. The other three
ablation runs in the same batch passed it, and so did all three of the reviewer's consecutive
full-workspace runs on the restored tree, immediately afterwards. The load at the failure itself was
not measured; the one-minute load average was 7.77 just after that gate, from the reviewer's own runs.

## New intermittent, 2026-09-15 — `the_wake_notifier_wakes_when_real_pty_output_arrives` (reviewer's run)

**Failed once**, in run 3 of the reviewer's three-run gate on the RFC-050 PR-050-A second-follow-up tree
(`21e6892`). Runs 1 and 2 passed it. The message: *"the first wake from real PTY output should report
more wakes may still come, not that the reader has already stopped"* (`reader/tests.rs:88`).
Afterwards: **20 of 20 passed alone.** The one-minute load average was 4.91 just after the gate.

**Not the change under review.** The test launches a plain shell with no transcript configuration, so
`prepare_transcript_writer` returns at its first line, exactly as the inline block it replaced did.

**Mechanism: inferred from reading the test, not observed.** The test writes
`printf 'tekstide-wake-ok\n'` and `exit` to the shell **before** it spawns the thread that calls
`block_until_woken`. If the shell prints, exits, and the reader reaches end of file before that thread
first blocks, the notifier's first observable state is *stopped*. It reports that truthfully, and the
assertion fails. The product behaviour is correct; the race is in the test's ordering.

**Proposed fix, test-only:** spawn the waiting thread, and let it block, before writing the input.
Assigned to the next commit that touches `tekstide-core` tests.

## Fixed, 2026-09-16 — `the_wake_notifier_wakes_when_real_pty_output_arrives`

The 2026-09-15 entry inferred the mechanism from reading the test. **It is now measured, and the test
is fixed.**

**Why "start the waiter first" was not enough.** `WakeNotifier::block_until_woken` returns
`reader_alive` as it stands **when it reads the eventfd**, not when the wake was signalled. A waiter
already blocked can be woken by the output and still read after the shell has run `exit`. Starting the
waiter first narrows the window without closing it.

**The fix: hold `exit` back.** The test starts the waiter, writes the `printf` alone, waits for the
first wake, and only then writes `exit`. The shell is still waiting for input when that wake is read,
so the reader is alive. It is ordering, not timing.

**Measured with a throwaway probe**, which put a 500 ms sleep in the waiter before its read and was
removed afterwards, with the file's hash checked:

| Order | Runs | Failed with *"…not that the reader has already stopped"* |
| --- | --- | --- |
| old: `printf; exit` before the waiter starts | 10 | **9** |
| new: waiter, `printf`, first wake, then `exit` | 10 | **0** |

The one old-order pass under a 500 ms head start is itself a reason to fix the order rather than tune
a delay.

## New intermittent, 2026-09-16 — one I wrote, in RFC-049 PR-049-C's first commit

`shell::tests::the_reset_notice_keeps_the_boot_figure_after_the_live_one_changes` failed in **run 3 of
its own first three-run gate**; runs 1 and 2 passed. **Mine, and not a product fault.**

The test moves the live disk figure to `12` and asserts the notice still renders the boot figure
`777`. Its negative assertion was `!lines[1].contains("12")` — but the same line carries the state
root, whose per-run temporary name is `/tmp/tekstide-run-<pid>-<n>/transcripts`. That run's pid was
`554612`, which contains `12`, so the assertion matched the path and failed.

**Fixed by comparing the rendered figure, not a bare digit string**: Fluent's isolate marks are
stripped and the assertions read `777 bytes` and `!… 12 bytes`. A path cannot contain the unit.
**Five consecutive full-workspace runs afterwards: green.**

The lesson is narrow and repeatable: a substring assertion against a line that also contains a
generated path is a test that fails on about one pid in three, and only sometimes.

## Recurrence, 2026-09-16 — RFC-049 PR-049-C's first gate (my run)

`approval::tests::coordinator::agent_run_queue_limit_is_enforced_and_only_counts_live_entries` failed
once, in the same run 3 above, with *"an expired entry must not continue occupying the live budget:
QueueLimitExceeded { scope: PerAgentRun, limit: 2 }"*. **This is the row first reported at request
260.**

**Not this slice.** The commit touches config parsing, retention marking, the audit record's
actor/source pairing and the local-data summary's limits. This test exercises the approval
coordinator's queue expiry and touches none of them. It passed in runs 1 and 2 of that gate and in
**all five runs** of the gate that followed, on the same tree.

## Recurrence, 2026-09-16 — RFC-049 PR-049-C's second commit, ablation B5 (my run)

`approval::tests::channel::bind_recovers_from_a_stale_socket_file` failed once, inside ablation B5
(the launch confirmation's verdict recomputed after the click). **This is row 1, the original from
response 213.**

**Not the ablation.** B5 changes which boolean one GUI launch path passes; this test binds an approval
socket over a stale socket file and reaches no launch path at all. It passed in the other ten
ablation runs of the same batch and in the gate on the restored tree.

## Recurrence, 2026-09-16 — RFC-049 PR-049-C's review (reviewer's run)

`approval::tests::channel::bind_recovers_from_a_stale_socket_file` failed once, in the reviewer's
ablation T1 (a partially failed policy cleanup records `Completed`). **Row 1**, the original from
response 213.

**Not the ablation.** T1 changes one `if` in the audit producer. This test binds an approval socket
over a stale socket file and touches no audit record. The reviewer's other three ablations and all
three gate runs on the restored tree passed it. The five-minute load average was 5.37 shortly after.

## Unidentified single failure, 2026-09-16 — RFC-048 PR-048-A's first gate (my run)

**One test failed in the `tekstide` binary and I cannot say which.** The run reported
`537 passed; 1 failed` where the suite has 538. The core binary and the docs invariants were green in
the same run.

**The name was lost to my own mistake.** I piped that `cargo test` invocation through `grep` for the
result lines only, then ran the suite *again* to look for the failure — so the failing log never
existed, and the second run passed. **Read the failing log; do not re-run to inspect it.** The same
lesson as the filtered-gate rule at the top of this file, in a new disguise.

**What is known:** it was the first workspace run after the PR-048-A ablation batch restored the tree
and `cargo fmt` rewrote two files; the only new real-process test in that binary is
`a_real_agent_run_that_exits_records_its_termination_through_production`, which launches a real
process and writes to a real audit store.

**Not reproduced:** 6 consecutive `-p tekstide --bin tekstide` runs and 5 consecutive full-workspace
runs afterwards, all green at 538 + 9 + 823. Recorded rather than dropped, so a later recurrence has
something to attach to.

## Recurrence, 2026-09-22 — RFC-030 PR-030-B's R-a/R-b review (reviewer's run)

`approval::tests::channel::bind_recovers_from_a_stale_socket_file` failed once, in the reviewer's
second batch of full-workspace runs on the restored tree (`563 + 9 + 863`, one failure). **Row 1**,
the original from response 213. Passed immediately in isolation, and the three other runs of the same
batch were clean at `563 + 9 + 864`.

**Not the slice.** R-a adds a `trigger_git_summary_refresh` call in two terminal-termination branches
and touches no approval socket.

**A reviewer's own mistake worth recording, because it looked like a flake and was not.** In the batch
before this one, `tests::trigger_git_summary_refresh_is_called_from_every_expected_site` failed at
"5 times, expected 6". That was not an intermittent: a gate run was still in flight when the reviewer
started an ablation, and that test **scans the source tree**, so it read `shell.rs` with the ablated
line removed. Source-scanning tests make the working tree a shared resource — a gate run and a tree
mutation must never overlap, and a failure of one of these tests is worth checking against `git
status` before it is called flaky.

## Recurrence, 2026-09-22 — RFC-030 PR-030-C's per-file-status gate (implementer's run)

`shell::tests::closing_a_project_with_a_backgrounded_descendant_kills_it_through_a_real_close` failed
once, in run 2 of a three-run full-workspace gate (`565 + 9 + 868`, one failure; runs 1 and 3 both
clean at `566 + 9 + 868`). **Row 8**, the same `0.16.0`-release-gate row this document already
carries, recurring in its already-documented shape. Passed immediately in isolation. Not the slice:
this response's changes are confined to `runtime::git`'s status parsing and `ProjectGitSummary`'s new
`file_statuses` field; nothing here touches terminal termination or process-close code.

## Recurrence, 2026-09-23 — RFC-030 PR-030-D's symlink-fix gate (implementer's run)

`approval::tests::channel::bind_recovers_from_a_stale_socket_file` failed twice across this
response's own gate attempts: once in run 1 of the first three-run attempt (`570 + 9 + 875`, one
failure; runs 2/3 clean), and again in run 3 of the restarted attempt (`570 + 9 + 875`, one failure;
runs 1/2 of that attempt clean). **Row 1**, the original from response 213 — same assertion message
both times: *"second bind must clear the stale file and succeed: ApprovalChannelError { reason: Io,
source: None }"*. Passed immediately in isolation both times. Two occurrences in one session is above
row 1's own documented ~2% baseline, but this document's own prior observation already covers it:
several of its recorded rates were "measured incidentally, across ordinary development activity," and
repeated `cargo build`/`cargo test` invocations in one sitting are plausible load, not a new cause.
Not the slice either time: this response's changes are confined to `runtime::git`'s
executable-location fallback (`resolve_git_from_inherited_path`/`directories_are_the_same_or_nested`),
nothing here touches the approval socket.

## Recurrence, 2026-09-24 — RFC-052 PR-052-A's gate (implementer's run)

`shell::tests::closing_a_project_with_a_backgrounded_descendant_kills_it_through_a_real_close` failed
once, in run 3 of a three-run full-workspace gate (`592 + 9 + 885`, one failure; runs 1 and 2 both
clean at `593 + 9 + 885`). **Row 8**, in its already-documented shape: the marker matcher read the
echoed command line (`(trap '' TERM; …) & echo "descendant-pid:$!"; wait`) instead of the output line.
Passed in isolation. Not the slice: this slice adds two `#[cfg(test)]` modules under
`project/root/explorer` and touches no terminal or process-close code.

**An environment note, since it cost a run:** `/tmp` was a full tmpfs (30 GB, 100 %, other projects'
scratch), so the gate ran with `TMPDIR` elsewhere. A `TMPDIR` under the repository made **47 tests fail
deterministically** with `Bind(SocketPathTooLong)` — the approval socket lives under the temp directory
and a Unix socket path is limited to 108 bytes. That is a property of the environment, not a flake and
not a regression; `TMPDIR=/dev/shm/…` (short) ran clean. A gate that must move its temp directory needs
a *short* one.

## Recurrence, 2026-09-24 — RFC-052 PR-052-C's gate (implementer's run)

Two already-registered intermittents, one in each of two runs of a three-run full-workspace gate (`625 + 9 + 902`; the middle run clean at `626 + 9 + 902`): `shell::tests::change_review_content_view_build_cost_by_line_count_measurement` (the load-sensitive timing budget, its own panic message says to check the load average) and, in the other run, `shell::tests::closing_a_project_with_a_backgrounded_descendant_kills_it_through_a_real_close` (**row 8**, same shape as before). Both passed in isolation. Not the slice: this response's changes are the explorer's row text, the Project Board's cards and a wider sidebar; nothing touches terminal termination or the change-review view. The gate was redone, not counted.

## Recurrence, 2026-09-25 — RFC-055 PR-055-C, a new test of RFC-055 PR-055-A's own

`runtime::git::tests::every_failure_to_answer_is_unknown_and_never_none_ignored` (added in PR-055-A, gated green three
times at that slice) failed **once**, in a full-workspace ablation run (`ablate.sh`, a surface-only change that cannot touch
it) at load ~9. Passed alone 8/8 straight after, and **40 of 40 with 24 busy-loops pushing the load average past 33** — so
this is **not reproduced**, and the cause below is a hypothesis, not a measurement. The failure output was not captured (the
ablation script filters it), which is itself a lesson for the script.

**Hypothesis: `ETXTBSY`.** The test writes eight stand-in `git` scripts and executes each immediately. Tests in this binary
run in parallel and fork; a child forked by another thread while this one held the script open for writing inherits that
descriptor until it execs, and executing the script in the window fails with `Text file busy`, which the module reports as
`NotFound` — so a case that asserts `Unknown(QueryFailed)` would see `Unknown(GateRefused)`. That is the mechanism this
project has already met for `flock` (row `transcript::tests::a_live_writer_holds_an_exclusive_lock_until_it_is_dropped`, a
fork-duplicated descriptor) and it fits a once-in-hundreds failure that vanishes when the tests run alone.

**Mitigation, in the test:** `fake_git_with_config` now runs the script once with retries on `ETXTBSY` before handing it out
(`wait_until_executable`). Whether that closes it can only be learned by not seeing the failure again; **if it recurs,
capture the assertion message** (`cargo test … --no-fail-fast > file`, not through the ablation script's filter) before
anything else. The other tests that write executable scripts (`marker_script`) have the same exposure and were not changed.

## Recurrence, 2026-09-25 — RFC-056 PR-056-B's gate (implementer's run)

`shell::tests::closing_a_project_with_a_backgrounded_descendant_kills_it_through_a_real_close` (**row 8**) failed once in run 1 of a three-run full-workspace gate (`670 + 16 + 1014`, load 2.35 at the start, well under the loads of the earlier recurrences). The assertion message was captured: *"the marker must be followed by a real, parseable PID"*, with the pane holding only the echoed command line `(trap '' TERM; while :; do sleep 1; done) & echo "descendant-pid:$!"; wait` and no `descendant-pid:<digits>` line yet — the same shape as every earlier row-8 recurrence. Runs 2 and 3 were clean. Not the slice: PR-056-B touches run records, the Project Board's notices and the AgentRun Report, and nothing on terminal termination. **The gate was redone, not counted.** Worth recording: it recurred at a *low* load (2.35), so this row should not be read as a load-only failure; and the machine had a full `/tmp` throughout (another project's session scratchpad), which this test does not use (`TMPDIR` was `/dev/shm`).

## Recurrence, 2026-09-29 — RFC-057 PR-057-D's gate (implementer's run), plus a sharper reading of the `SocketPathTooLong` trigger

**A new trigger for the already-documented `SocketPathTooLong` class (2026-08-24's environment note), not a new cause.** The first two attempts at this response's three-run gate used `TMPDIR=$(mktemp -d /dev/shm/tekgate.XXXXXX)` — visibly under `/dev/shm`, the guidance that note already gives — and **every** approval/agent-run-adjacent test failed en route (22–34 of them per run, reproduced identically against a clean `git worktree` at this response's own base commit, so confirmed not-the-slice before anything else was tried). The captured message was the same one the 2026-08-24 note names: `Runtime(AdapterApproval(Bind(SocketPathTooLong)))`. The difference from that note's own case: this `TMPDIR` was never repo-relative, and `/dev/shm` was exactly the fix it recommends — but `mktemp`'s own `XXXXXX` suffix (six extra characters) was enough, once a project label and a run id nest under it, to cross the 108-byte `AF_UNIX` limit anyway. **The actual guidance is narrower than "short": a fixed, short literal (`/dev/shm/tk1`, as used successfully here) leaves more of the 108 bytes to the path segments the product itself controls than a `mktemp -d` pattern does, even one that looks short at a glance.**

With a fixed short `TMPDIR` per run, two of three runs were clean outright (`712 + 16 + 1045`). The middle run failed once: `approval::tests::channel::bind_recovers_from_a_stale_socket_file` — **row 1**, the original. Passed immediately in isolation. Redone once more under the same fixed `TMPDIR` and it recurred a second time, back to back — above row 1's own ~2% baseline for two single-run occurrences in a row, but the 2026-08-23 entry already anticipated exactly this ("repeated `cargo build`/`cargo test` invocations in one sitting are plausible load, not a new cause") and a third attempt came back clean. **The gate was redone, not counted** (final accepted runs: `712 + 16 + 1045`, `712 + 16 + 1045` after the row-1 redo, `712 + 16 + 1045`). Not the slice either time: PR-057-D's changes are confined to `content::document`/`content::undo`/`content::snapshot`, `surface::editor`'s edit/undo functions, and the `UndoActiveDocument`/`RedoActiveDocument` keybinding wiring — nothing here touches the approval socket or its stale-file recovery path.

## 2026-09-30 — `change_review_content_view_build_cost_by_line_count_measurement`, the reviewer's `0.28.0` gate

Failed once, in run 3 of a three-run verification gate on the `0.28.0` candidate (`fcdeda0`):
**503 ms against the 500 ms budget at 50,000 lines — a 0.6 % overshoot** — with machine load climbing
14.81 → 16.13 → **21.48** across the three runs, most of it the gate's own parallelism. Its own
message names load as the likely cause and it is right.

**Redone, not counted.** The repeat was clean three times running (1,774 each, 0 fixture entries) at
loads **25.73 / 20.68 / 19.92 — higher than the run that failed**, which is what a marginal
wall-clock threshold looks like rather than a load cliff.

**This test has now cost a gate attempt in three separate releases** (`0.26.0` per the row above,
the implementer's own `0.28.0` attempt, and this one). It measures the machine, and the fix recorded
in `delivery-plan.md` — assert a **ratio against a reference workload timed in the same process** —
is still unscheduled. RFC-057 PR-057-A's `#[ignore]`d baseline and snora 0.52.0's CI ratio are both
worked examples of the two halves.

## Recurrence, 2026-10-06 — RFC-026 PR-026-A's review-450 gate (implementer's run)

`shell::tests::change_review_content_view_build_cost_by_line_count_measurement` (the load-sensitive
500 ms build-cost budget, registered at review 338) failed in **two of the first three** full-workspace
runs of this gate, both at machine load about 25 (other sessions' builds on the shared machine). It
passed immediately in isolation at load 25 and in all three runs of the accepted gate at loads 22–25.
Not the slice: the change touches `project/watch` only, which nothing in the change-review view reads.
The failure message was not captured in the first two runs, which is the cost of filtering the gate's
output; the next attempt kept the full logs. **The gate was redone, not counted.** This is the test
that has now cost a gate attempt in three releases; the ratio fix is still unscheduled.

## Recurrence, 2026-10-07 — RFC-026 B2 steps 3–5's first gate, load ~16

**One failure in three full-workspace runs**, on the tree that adds the watcher's event feed, the
drain tick and the stopped sentence:

| Test | Runs | Register row |
| --- | --- | --- |
| `surface::terminal::tests::resize_makes_the_pty_the_emulator_and_the_render_path_agree` | once more (`stty size` did not parse) | row 787's entry, now a second occurrence |

**The same test passed in isolation five times in a row** straight after (`uptime` 10.4), so the
failure is the load-sensitive PTY timing already recorded, not the watcher. The other two runs
were green with the same tree: 1808 passed, 0 failed, 5 ignored each.

**What to watch.** The new watcher tests wait on real kernel events within 300 to 400 ms windows
(`the_hostile_tree_is_watched_only_where_the_policy_admits_and_never_recurses` and
`a_real_change_is_fed_to_the_batcher_and_drains_as_a_scan_request`). They passed under this load, but a
future flake in them should be recorded here with its load, not absorbed into a longer timeout.

## Recurrence, 2026-10-07 — RFC-026 review 459's floor fix, load ~16 to ~19

**One failure in four full-workspace runs**, on the tree that moves the row floor into one function:

| Test | Runs | Register row |
| --- | --- | --- |
| `shell::tests::change_review_content_view_build_cost_by_line_count_measurement` | once, 892 ms against 500 ms, load 19.04 | the `0.29.0` ratio item, already scheduled |

The test's own message names load as the likely cause. The same tree then passed three full runs in a
row at load 22 to 28. This is the known absolute-budget test the `0.29.0` ratio fix is for; no new
row is needed, and this entry is the dated record of its recurrence.

## 2026-10-07 — `change_review_content_view_build_cost_by_line_count_measurement`, fixed for `0.29.0`

**The absolute budget is gone.** Four episodes of the same failure shape — `0.26.0`, the `0.28.0`
candidate, the architect's own `0.28.0` verification, and RFC-026 PR-026-A's review 451 — each
fixed by reporting the load average beside the 500 ms budget it crossed, which explains a red run
after the fact and prevents none. Scheduled into `0.29.0` at review 451 rather than recorded a
fifth time (the implementer's own call, and the right one).

**The fix: a paired ratio, not a bigger or looser absolute number.** The test now times a
10,000-line reference workload and a 100,000-line one (ten times the work) back to back, in the
same process, in the same run, and asserts the larger costs under thirty times the smaller's own
time — RFC-026's own paired control (review 462), applied to a view-build cost instead of a
watched burst. An ambient load spike lands on both sides of the ratio at very nearly the same
moment, so it moves the absolute numbers without moving the ratio much; a real regression (an
accidental allocation per line, a quadratic pass) would move the ratio itself and nothing else can.

**Verified under artificial load.** Run quiet: reference ≈16 ms, scaled ≈162 ms, ratio ≈10.1x.
Run under 32 concurrent `yes` processes (load average 25, on a 32-core machine): reference ≈44 ms,
scaled ≈394 ms, ratio ≈9.0x — both comfortably inside the 30x budget, and the ratio barely moved
while the absolute numbers nearly tripled. The full per-line-count curve is still printed every
run, unchanged in spirit from before, but it no longer decides pass or fail.

**This closes the row.** No further entries are expected for this test; a future failure here
would mean the ratio itself moved, which is what the fix exists to catch.

## Recurrence, 2026-10-07 — RFC-065 PR-065-A's gate, load unmeasured

**One failure in run 3 of a three-run full-workspace gate**, `TMPDIR=/dev/shm/tk065g3` (fixed, short,
per the 2026-09-29 entry's own guidance): `approval::tests::channel::bind_recovers_from_a_stale_socket_file`
failed; runs 1 and 2 both clean (`731 + 16 + 1079`, both runs). **Row 1, recurring in its own
already-documented shape** -- nothing in this response touches the approval channel, the socket
directory, or anything near it: the whole diff is `ProjectContentWorkspace`'s open set
(`crates/tekstide-core/src/project/content.rs`), its own tests, and one new shell-level test in
`crates/tekstide/src/shell/tests.rs`. Re-run once, isolated, and passed. **The gate was redone, not
counted**, per the same convention the 2026-09-29 entry used: the final accepted three runs are
`731 + 16 + 1079`, `731 + 16 + 1079`, `731 + 16 + 1079`.

## Recurrence, 2026-10-07 — RFC-065 PR-065-B's review-469 fix gate, load unmeasured

**One failure in run 2 of a three-run full-workspace gate**, `TMPDIR=/dev/shm/tk065g2` (fixed, short):
`shell::tests::closing_a_project_with_a_backgrounded_descendant_kills_it_through_a_real_close`
failed; runs 1 and 3 both clean (`734 + 16 + 1083`, both runs). **Row 8, recurring in its own
already-documented shape** -- the whole diff this response adds is the delivery-work ratio fix in
`shell/tests/editor_baseline.rs` (a test-only file, the paired-control harness), nothing touching
terminal termination. Re-run once, isolated, and passed. **The gate was redone, not counted**:
the final accepted three runs are `734 + 16 + 1083`, `734 + 16 + 1083`, `734 + 16 + 1083`.

## Recurrence, 2026-10-07 — RFC-065 review 472's documentation-fix gate, load unmeasured

**One failure in run 2 of a three-run full-workspace gate**, `TMPDIR=/dev/shm/tk065r2` (fixed, short):
`approval::tests::coordinator::is_still_answerable_reflects_the_real_connection_state` failed; runs 1
and 3 both clean (`736 + 16 + 1085`, both runs). **Already-registered intermittent, recurring in its
own documented shape** -- the whole diff this response adds is CHANGELOG.md, two book pages and one
doc comment (the `[open]` meaning-widened fixes required at review 472), nothing near approval or its
connection state. Passed in isolation. **The gate was redone, not counted**: the final accepted three
runs are `736 + 16 + 1085`, `736 + 16 + 1085`, `736 + 16 + 1085`.

## Recurrence, 2026-10-07 — RFC-065 review 473's documentation-fix gate, load unmeasured

**One failure in run 2 of a three-run full-workspace gate**, `TMPDIR=/dev/shm/tk065r2` (fixed, short):
`approval::tests::coordinator::is_still_answerable_reflects_the_real_connection_state` failed again --
the second recurrence of this same row today, in the previous response's own gate as well. Runs 1 and
3 both clean (`736 + 16 + 1085`, both runs). The whole diff this response adds is `en.ftl`, two doc
comments and one evidence `README.md` (the four fixes required at review 473), nothing near approval.
Passed in isolation. **The gate was redone, not counted**: the final accepted three runs are
`736 + 16 + 1085`, `736 + 16 + 1085`, `736 + 16 + 1085`.

## Disposition, 2026-10-07 — review 474: row 5 stops being recorded and gets fixed

**Ruling by the reviewer. This register has done its job and the answer has not changed in six
weeks; that is now the problem.**
`approval::tests::coordinator::is_still_answerable_reflects_the_real_connection_state` has been
registered since **2026-08-25** (request 326) and appears in twelve places in this file. It failed
**twice on 2026-10-07 alone**, in two consecutive gates whose entire diffs were documentation and
doc comments. Each occurrence costs a redone three-run full-workspace gate. The disposition has
been "known intermittent, redo the gate" every single time.

A register exists to make an intermittent visible so that someone decides about it, not to
accumulate rows. Eight-plus recurrences is a decision that was never taken.

**The cause is already diagnosed here and I confirmed it in the test.** At
`crates/tekstide-core/src/approval/tests/coordinator.rs:374`, the body ends:

```rust
drop(peer);
assert!(
    !coordinator.is_still_answerable(&agent_run_id, &proposal_id),
    "once the peer closes its end, the same request must no longer be answerable"
);
```

`drop(peer)` closes the socket locally; `is_still_answerable` asks the coordinator's own end whether
the peer is gone. Between the local close and the coordinator's end observing it there is a kernel
scheduling gap, and the assertion requires that gap to be exactly zero. Under load it is not.
**This is a defect in the test, not in the product**: nothing in the design promises the close is
observed instantaneously, and the product behaves correctly when it is observed.

**The fix, and it closes two rows.** Poll `is_still_answerable` to `false` against a bounded
deadline rather than demanding it on the first call. That keeps what the test is actually for —
that the connection state is genuinely consulted and not cached — and drops the impossible
instantaneity. The 2026-09-12 entry above already establishes that
`an expired entry must not continue occupying the live budget` is *the same cause with a second
name*; the same pattern fixes it, and both rows should be retired in one change.

**Scheduled: before the `0.30.0` candidate**, because what it taxes is the candidate's own
three-run gate. It is test-only, so it does not open a second RFC in the release.

**Owner: the dev team — added at review 475, because the entry above did not say so and was read,
reasonably, as "not mine to action".** A scheduled fix with no name against it is the same failure
this disposition was written to end: recorded, agreed, and never done. The reviewer rules on
intermittents and does not write the code. Concretely, what is wanted:

- Replace the single post-`drop` assertion with a bounded poll — `is_still_answerable` must *become*
  `false` within a deadline generous enough to survive a loaded machine (seconds, not milliseconds),
  failing with the same message if it never does.
- Apply the same shape to `an expired entry must not continue occupying the live budget`, the second
  name for this cause.
- Retire both rows from the register in that change, and say in the commit that the register's
  recurrence sections stay as history.
- No product code. If the fix appears to need any, that finding outranks the fix: say so instead.

## Verification, 2026-10-07 — review 476: the fix is accepted on reasoning, not on measurement

**I could not reproduce rows 2 and 5 at all, with or without the fix.** Recorded in full, because an
ablation that found nothing is itself a finding, and because the next person to chase this cause
should not repeat the three attempts below.

| What was run | Result |
| --- | --- |
| Both tests, 60 iterations, **fixed** code, 16 `yes` spinners on 32 cores (load avg ~16) | 60 passed |
| Both tests, 60 iterations, **ablated** to the pre-fix single assertion, same contention | 60 passed |
| Full workspace, 8 runs, **ablated** | row 5 failed 0 times; no test failed at all |

The second line is the one that matters. **The ablated run passing is what makes the first line
worthless**: a harness that cannot fail with the bug present proves nothing when it passes with the
bug removed. Had I stopped at 60/60 on the fixed code I would have reported the fix as verified.

**So the fix is accepted on inspection, not evidence.** It is correct in form — bounded 5s poll,
10 ms sleep, original assertion message preserved — and it is strictly *weaker* in what it demands
than the code it replaces (it waits for a condition instead of requiring it already hold), so it
cannot fail where the old code passed. What is missing is any demonstration that it fails *less*.
The evidence will be the absence of recurrence across the coming gates. **If either row reappears,
this disposition reopens**; the rows are retired from active tracking, not declared solved.

**Method notes, for whoever chases this cause next.**

- **CPU contention alone does not reproduce it.** 68 attempts under deliberate load produced
  nothing, while the dev team's two occurrences on 2026-10-07 came out of ordinary gate runs on
  this same machine. Whatever the trigger is, it is not simply a loaded scheduler.
- **A detector that greps the test's name matches a passing run too.** My first full-workspace
  attempt reported "reproduced in 8 of 8" because `cargo test` prints the test's name whether it
  passes or fails. Match `^test .* FAILED`, never the name.
- **`error: test failed` is not a compile error.** My second attempt classified two runs as
  "DID NOT COMPILE" on that string and `continue`d past the row-5 check, so "0 of 8" was really
  "0 of the 6 runs I looked at". Match `^error\[E…\]` or `could not compile` for compilation.
- **Loose end, unidentified.** Those same two runs had a genuine test failure under
  `-p tekstide --bin tekstide`, and my filter discarded the names before I read them. The third
  attempt's 8 runs were completely clean, so it did not recur. Recorded so that a failure seen on
  2026-10-07 is not simply lost; it is not a new register row, because nothing names it yet.

## New rows, 2026-10-08 — review 478: two terminal PTY tests, diagnosed, not yet fixed

**Found in the reviewer's own gate run on review 478's tree, not by the dev team's** — their three
runs were clean, mine failed once in two. Load-dependent, as the cause below explains.

| Test | First seen |
| --- | --- |
| `runtime::terminal::tests::a_real_backgrounded_job_is_dead_after_a_real_close` (`tests.rs:553`) | review 478 (2026-10-08) |
| `runtime::terminal::tests::a_job_that_leaves_the_session_via_setsid_survives_a_real_close` (`tests.rs:627`) | review 478 (2026-10-08) |

Both failed together, in the same run, with the same shape of message:

```
real backgrounded job's own pid should be parseable from PTY output
```

**Cause, confirmed in the test code — the same class as rows 2 and 5, waiting on a weaker
condition than the one actually needed.** The test writes `sleep 300 & echo BGPID=$!` to the PTY
and then calls `read_until_contains(&mut runtime, &handle, b"BGPID=")`, which returns as soon as
the byte sequence `BGPID=` appears. But `parse_bgpid` needs the marker **followed by digits**, and
`parse_bgpid`'s own comment already records why those are different things: the shell echoes the
still-unexpanded command text, `echo BGPID=$!`, which contains the literal marker with no digits
after it. So the wait can be satisfied by the echo alone. Normally the echo and the command's real
output land inside the same 50 ms read and nothing is noticed; under load the read returns holding
only the echo, `parse_bgpid` finds no digits, and the test panics.

The author knew about the echo — it is why the parser takes the *last* occurrence — but the wait
condition was never given the same knowledge.

**Fix shape, and it is the row 2/5 pattern again.** Wait for the condition the test actually needs
rather than a proxy for it: poll until `parse_bgpid(&output).is_some()` against the existing
5-second deadline, and the same for the setsid test's own marker. No product code; both causes are
in the tests' own waiting.

**Owner: the dev team.** Not a blocker for the `0.30.0` candidate unless it fails a gate run, in
which case the run is redone and recorded as usual. Naming the owner here because the row 2/5
disposition had to learn that lesson at review 475.

**Possibly, but not provably, the pair referred to in "Verification, 2026-10-07" above**, where two
of eight ablated runs failed under a target I did not name because my filter discarded the names.
Those logs are gone, so this is a resemblance, not an identification.

## Addendum, 2026-10-08 — review 480: review 478's two rows also leak their fixture directory

Found while verifying the `0.30.0` candidate's own "0 fixture entries left" criterion. A reused
`TMPDIR` held exactly three entries, and two of them were these:

```
tekstide-terminate-real-backgrounded-job-677412-1791413026620745216
tekstide-terminate-setsid-survives-677412-1791413026620725036
```

Same process id, and it is the id of the failing run recorded at **New rows, 2026-10-08 — review
478**. A fresh short `TMPDIR` over a passing run leaves 0, so the criterion itself is sound.

**Why they leak:** the fixture is removed by `cleanup_root(root)`
(`runtime/terminal/tests.rs:821`), an ordinary call at the end of the test body rather than a drop
guard. Any panic before it — including the PTY-parse panic those two rows *are* — skips the removal.
This is the shape this whole register was opened for, so it is worth saying plainly: **these two
tests fail the gate twice, once on the assertion and once on the leak.**

**Fix it with the wait, in one change.** Whoever polls `parse_bgpid` to `Some` should also make the
fixture clean itself on unwinding, the way `test_support`'s own
`kill_on_drop_child_does_not_leak_across_a_panic` already establishes for a `Child`. Owner: the dev
team, with the rows above.

## New row, 2026-10-08 — RFC-027 PR-027-B gate

| Test | Runs | Register row |
| --- | --- | --- |
| `surface::terminal::tests::resize_makes_the_pty_the_emulator_and_the_render_path_agree` | once | row 787's entry, now a third occurrence |

Full workspace run otherwise clean (`743 + 16 + 1107`, 1 failed, 0 entries left); passed in
isolation on the immediate rerun. The load-sensitive PTY timing already recorded at row 787 and its
second occurrence above, not a regression in this response's own recovery-record work -- nothing
this response touches is anywhere near `surface::terminal`. Gate redone, not counted, per this
register's own convention.

## New row, 2026-10-09 — review 494: a terminal reader drain test, and a leak with it

Found in the reviewer's own gate run while closing RFC-027 PR-027-C. **Unregistered until now.**

| Test | First seen |
| --- | --- |
| `runtime::terminal::reader::tests::drain_available_never_blocks_the_caller_even_under_sustained_production` (`reader.rs`) | review 494 (2026-10-09) |

**Observed once** in a full-workspace run, then **5 of 5 clean in isolation and 2 of 2 clean in
further full-workspace runs**. Load-sensitive, like the rows above it.

**It leaks its fixture when it fails**, the same second symptom the review 478 rows have: the
failing run's `TMPDIR` held
`tekstide-reader-nonblocking-drain-1212031-1791505138300035978` afterwards, while every passing run
left 0. Whatever fixes the wait should make the fixture clean itself on unwinding, as
**Addendum, 2026-10-08 — review 480** already says for the other two.

**The assertion message was not captured, and that is the reviewer's own fault.** My gate filter
matched `^test .* FAILED` and `^test result:` but not `panicked at`, so the one run that could have
explained it printed the name and discarded the reason. **This is the third time this session a
filter has dropped the message it existed to find** — the ablation detector at review 476, the
`error: test failed` misclassification in the same review, and now this. The filter for any gate run
that might fail must include `panicked at`.

**Owner: the dev team**, with the rows above. Not a blocker for `0.31.0`: it recurs rarely and
passes on rerun, and the candidate's own three-run gate is where it would have to be redone if it
appears again.

## Disposition, 2026-10-09 — review 497: the register now costs release attempts, and that is the finding

**Not a new row. A statement about all of them, written because the cost has become measurable.**

At review 497 the dev team's three-run gate failed its first run on the review-478 PTY pair, and
**my own three runs, taken immediately after on the same tree, failed run 1 on a different
registered row** — `shell::tests::closing_a_project_with_a_backgrounded_descendant_kills_it_through_a_real_close`,
on file since the `0.16.0` gate. Two independent three-run gates, two different registered
intermittents, same afternoon.

**The rate, from runs I watched across reviews 494–497 rather than from memory:** about **3
failures in 17 full-workspace runs** (~18%), across both the dev team's machine and mine. The gate
rule is *three consecutive green*. At that rate a three-run gate passes first attempt roughly
**half** the time.

**So the register has stopped being a record and become a tax.** `0.31.0`'s candidate will likely
need its gate redone at least once, and every redo is a full three-run cycle. Nineteen rows are on
file; rows 2 and 5 were fixed at review 476 because somebody decided about them, and nothing has
been decided about the rest.

**What I am not asking for:** fixing nineteen tests before `0.31.0`. That would trade a release for
tidiness.

**What I am scheduling:** a disposition pass over the register **after `0.31.0` ships**, on the
same terms as the rows 2/5 ruling — for each row, fix, quarantine, or accept with a reason, rather
than carry it. The cause is the same in most of them and already diagnosed twice in this file: *the
wait is weaker than the condition it is waiting for.* The rows 2/5 fix is the worked example, and
review 480's addendum adds the second half: the fixture must clean itself on unwinding, or a
failing run fails the gate twice.

**Owner: the dev team**, after the release. Named here because the rows 2/5 disposition had to
learn at review 475 that a decision without a name against it does not happen.

## New row, 2026-10-09 — RFC-066 PR-066-B, review 503's required fix: a shared desktop under real load

| Test | First seen | Register row |
| --- | --- | --- |
| `surface::terminal::tests::resize_makes_the_pty_the_emulator_and_the_render_path_agree` | once | row 787's entry, now a fourth occurrence |
| `shell::tests::terminal_poll_handler_cost_under_a_real_wake_driven_flood_headless_benchmark` | once | **new here** |

Both failed in the same run (run 1 of 3), otherwise clean (`756 + 17 + 1119`, 2 failed, 0 entries
left); both passed immediately on an isolated rerun. This session's own desktop was independently
confirmed busy at the time — a live GUI capture taken earlier the same session (for this slice's
own D3 evidence) found Chromium playing video and several other agents' terminals already running
on the same real desktop — so this is exactly the load-sensitive PTY-timing shape row 787 and its
siblings already describe, not a regression from this response's own work (nothing here touches
`surface::terminal` or the poll loop).

**The new one's own assertion message claims more than it can support**, the same overconfidence
row 7 (`change_review_content_view_build_cost_by_line_count_measurement`) used to have before its
fix: `"a single wake-driven poll() cost blew past a sane bound: max={max_micros}us -- this is a
real regression, not measurement noise"` — stated unconditionally, with no load figure alongside it
to let a reader tell the two apart, which is exactly what happened here: a failure that was noise,
asserted as not noise. Not fixed in this response (out of this slice's own scope), but worth
pairing with row 7's own fix as a second instance of the identical wording defect if the dev team's
scheduled disposition pass reaches wall-clock-budget benchmarks generally.

## Disposition pass, 2026-10-09 — `rfcs/handoffs/test-flake-disposition.md`

**The pass this document scheduled, executed.** Every row in the register was read in full,
against the actual current test code, not only against this document's own account of it —
several already-fixed rows were only ever documented as "fixed," never explicitly retired, and
three more turned out to already be closed by the 2026-08-26 audit-store fix without anyone having
confirmed it. Both are corrected below.

### Scope count

**Nineteen distinct test names appear in this register.** Eleven were already fixed and are now
retired in the top table above (confirmed, not assumed, against the real current code for every
one). **Eight were genuinely live** and each now carries exactly one verdict:

| Test | Verdict |
| --- | --- |
| `bind_recovers_from_a_stale_socket_file` | Accept |
| `every_failure_to_answer_is_unknown_and_never_none_ignored` | Accept (already mitigated; retired above) |
| `purge_reports_deferred_cleanup_while_wal_reader_is_active` | Accept |
| `a_real_backgrounded_job_is_dead_after_a_real_close` | Fix |
| `a_job_that_leaves_the_session_via_setsid_survives_a_real_close` | Fix |
| `resize_makes_the_pty_the_emulator_and_the_render_path_agree` | Fix |
| `closing_a_project_with_a_backgrounded_descendant_kills_it_through_a_real_close` | Fix (retired above) |
| `terminal_poll_handler_cost_under_a_real_wake_driven_flood_headless_benchmark` | Quarantine |
| `drain_available_never_blocks_the_caller_even_under_sustained_production` | Quarantine |

(`every_failure_to_answer_is_unknown_and_never_none_ignored` is listed here because the handoff
pack named it as live; direct inspection found its mitigation already shipped, so it moved
straight to retired in the top table rather than needing new work.)

### Fix — one shared defect behind four tests, not two separate causes

**The handoff pack's own grouping treated "wait weaker than condition" (its Group A) and "PTY read
timing" (its Group B) as different causes. Direct inspection of all four tests' current code found
they are the identical defect**, the one review 478 already diagnosed for the first two: a wait
that returns as soon as a *marker substring* appears, with the actual parse attempted as a
*separate step afterward* — so a read that lands on the shell's own echo of the still-unexpanded
command (which contains the marker text with no real value after it yet) satisfies the wait and
fails the parse a moment later.

- `runtime::terminal::tests::a_real_backgrounded_job_is_dead_after_a_real_close` (`BGPID=`)
- `runtime::terminal::tests::a_job_that_leaves_the_session_via_setsid_survives_a_real_close`
  (`DETACHEDPID=`)
- `surface::terminal::tests::resize_makes_the_pty_the_emulator_and_the_render_path_agree`
  (`RESIZE_CHECK_017E:`) — this document's own row 787 and its four prior occurrences never named
  this cause; it was treated as a separate, undiagnosed "PTY timing" class until this pass read the
  actual code
- `shell::tests::closing_a_project_with_a_backgrounded_descendant_kills_it_through_a_real_close`
  (`descendant-pid:`, row 8) — same shape, same fix

**Fix, identical in all four**: poll the *parse* itself against the existing deadline, not a
substring. `read_until_parsed`/`poll_until_parsed`/`poll_demo_pane_until_parsed` (one per call
site's own existing helper family, not a shared crate-spanning function, since
`tekstide-core`/`tekstide` are separate crates and each site's own polling helper already existed
locally) loop the real read, re-attempt the caller's own parse function against the growing buffer
each iteration, and return only once the parse actually succeeds or the deadline passes. The
existing `rsplit`/`rsplit_once`-last-occurrence logic (already correct — it is *why* the parse
fails cleanly on the echo instead of silently matching it) is unchanged, just no longer reachable
only after the wait has already ended.

**Ablation attempted, and it did not reproduce — recorded in full, the same honesty rows 2/5's own
disposition (2026-10-07) required of itself.** The resize test has four prior real occurrences, the
most of any row in this family, so it was chosen as the representative case: `git checkout` back to
a staged pre-fix version, 16 `yes` spinners (load ~9), 25 runs, 0 failures; 40 more spinners (load
~18), 20 more runs, 0 failures. **45 runs at the pre-fix code, under real contention, produced
zero reproductions.** This matches rows 2/5's own finding exactly: "CPU contention alone does not
reproduce it... whatever the trigger is, it is not simply a loaded scheduler." All four historical
real occurrences (this row's own four, plus row 8's own two) happened on gate runs under ordinary,
not deliberately synthetic, load.

**So the fix is accepted on reasoning, the same standard rows 2/5 were held to, not on a forced
reproduction.** It is strictly *stronger* than what it replaces, not merely different: the new wait
demands everything the old one did (the marker must appear) *plus* the one thing missing (it must
parse), so there is no condition under which the new code returns earlier than the old one did —
it can only wait longer for the identical final correctness property. A wait that only ever waits
*at least as long* cannot be the cause of a *new* failure; it can only remove the old one. Whether
it does is the evidence to watch for across coming gates, exactly as rows 2/5's own disposition
said of itself: **if any of these four recurs, this disposition reopens.**

### Fix — fixture cleanup moved to a drop guard, ablated directly (not probabilistically)

Unlike the marker-wait race, this half **is** mechanically forceable, and was forced. Review 480's
own addendum: a failing test in this family fails the gate twice, once on its assertion and once on
the fixture it leaves behind, because `cleanup_root(root)` is an ordinary call at the end of the
test body, skipped by any panic above it.

Applied to the two `tekstide-core` tests whose own `root` previously relied on that ordinary call
(`a_real_backgrounded_job_is_dead_after_a_real_close`,
`a_job_that_leaves_the_session_via_setsid_survives_a_real_close`): `CleanupOnDrop`, a `PathBuf`
newtype with `Deref<Target = Path>` (so it passes anywhere `&root: PathBuf` already did, no other
call site needing its own type to change) and a `Drop` impl that removes the directory
unconditionally. The two `tekstide`-crate tests in this same family
(`resize_makes_the_pty_the_emulator_and_the_render_path_agree`,
`closing_a_project_with_a_backgrounded_descendant_kills_it_through_a_real_close`) needed no
equivalent change: the first already cleans up via `ScratchPane`'s own existing `Drop` impl, and
the second uses shared fixture state with its own cleanup elsewhere in `shell::tests`, not a
per-test temp directory.

**Ablated directly, both directions, on the real filesystem**: a forced `panic!()` inserted
immediately after constructing `root`, first with `CleanupOnDrop` in place — the directory was
gone immediately after the panicking run, confirmed by checking the exact path the test itself
printed. Reverted to the pre-fix bare `PathBuf` with the identical forced panic — the directory
**survived**, reproducing the leak directly rather than by inference. Restored; the real test
(no forced panic) passes.

### Quarantine — two wall-clock latency ceilings, not correctness properties

`terminal_poll_handler_cost_under_a_real_wake_driven_flood_headless_benchmark`
(`tekstide::shell::tests`) and `drain_available_never_blocks_the_caller_even_under_sustained_
production` (`tekstide-core::runtime::terminal::reader::tests`) both assert a single real-PTY
call's own wall-clock duration stays under an absolute ceiling (50ms, 20ms) across many samples
under sustained production. Both `#[ignore]`d, with a reason string and a doc comment naming this
pass, rather than deleted or left flaky; both still compile and run on demand with `-- --ignored`.

**The handoff pack grouped the second of these with the BGPID family ("the same family"). Direct
inspection shows it is not** — there is no marker/substring race anywhere in it, only a latency
budget under real flood pressure, the identical shape `terminal_poll_handler_cost...` already has.
Recorded as a correction to the handoff's own grouping, not silently reclassified.

**Why quarantine rather than the ratio-based fix `change_review_content_view_build_cost_by_line_
count_measurement` already established, and whose own doc comment explicitly argues against
`#[ignore]`** ("a budget that cannot be crossed by load at all is the better fix, which this is"):
that fix works because its property is *relative* — cost must scale linearly with line count, so a
paired reference measurement in the same run cancels out ambient load on both sides of a ratio.
Both of these tests assert something structurally different — a *single call's own absolute
latency ceiling* — which has no natural reference workload to divide by; there is nothing for a
ratio to be *of*. This is a considered departure from that precedent, not an oversight: the
precedent's own reasoning does not transfer to this property's shape, and quarantining a benchmark
that cannot be made load-invariant is exactly what the handoff pack itself asks this pass to decide
honestly rather than attempt to re-stabilise again.

`terminal_poll_handler_cost...`'s own failure message (already found overconfident at RFC-066
PR-066-B review 503 — *"this is a real regression, not measurement noise"*, stated unconditionally)
is softened in the same change: it now says the ceiling could be a real regression or could be
this machine under load, and names that this test is quarantined from the gate for exactly that
reason, rather than asserting which one it is.

### Accept — three rows, three different reasons, none of them scheduling

- **`bind_recovers_from_a_stale_socket_file`.** A single, synchronous bind attempt against a staged
  stale-socket condition — there is no wait loop anywhere in the test to strengthen the way the Fix
  rows above had. Historically ~2% across dozens of recurrences over two months, stable, never
  isolated to a more specific cause despite sustained attention from both the dev team and the
  reviewer. Accepted: the shape offers no actionable fix, and the rate has not moved.
- **`purge_reports_deferred_cleanup_while_wal_reader_is_active`.** One occurrence ever, in a run
  whose own third pass showed SQLite-under-parallel-load contention — the same class this register
  already documents extensively elsewhere (the shared-audit-store cause, fixed 2026-08-26, was
  exactly this shape before isolation). Never re-investigated past that single occurrence. Accepted
  at its current, very low rate rather than opening a fresh investigation into one data point.
- **`every_failure_to_answer_is_unknown_and_never_none_ignored`.** Already carries a bounded retry
  on `ETXTBSY` (`wait_until_executable`, `runtime/git/tests.rs`), confirmed in place and confirmed
  passing. Functionally already a Fix, by a prior response; recorded here as Accept-the-existing-
  mitigation and retired in the top table rather than redone.

### The measured rate, re-taken on the same basis

**Before**, from this document's own repeated sampling: ~3 failures in 17 full-workspace runs
(~18%), the figure reviews 494–497 converged on.

**After, 13 full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run: 0 failures.**

**One honestly-disclosed mistake inside this same measurement, corrected before the number was
trusted.** The first attempt at this re-measurement used a long `TMPDIR` path
(`/dev/shm/gflake-remeasure/runN`) and produced 7 of 7 runs "failing" — but every failure was
`Runtime(AdapterApproval(Bind(SocketPathTooLong)))`, a Unix-domain-socket path-length limit this
project's own established convention already exists to avoid (short literal `TMPDIR`s, the lesson
this register itself and several RFC handoffs already carry). Not a flake, not a regression: a
harness mistake in the measurement itself, caught by actually reading the panic message rather than
trusting the "FAILED" count — recorded here, not discarded, as a second instance of the exact
"count what actually happened, do not infer it" discipline rows 5/6's own captured-message lessons
already established for this file. Re-run correctly (short `TMPDIR`, e.g. `/dev/shm/gfrN`): clean
every time.

**13 is a smaller sample than the original 17**, stated plainly rather than rounded up to sound
more conclusive than it is. 0/13 against a ~18% baseline is still a strong result, and the honest
comparison is the same one every fix in this document has had to make do with: a post-fix count,
not a controlled before/after on the identical tree.

### Gate

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `rfc_docs_invariants`: unaffected (this pass touches no RFC document).
- `cargo test --doc --workspace`: clean.
- Three consecutive full-workspace runs, part of the fix commit's own required gate, then 10 more
  as the re-measurement above: 13 total, 0 failures, 0 fixture entries left each time, fresh short
  `TMPDIR` every run.
- 2 tests now `#[ignore]`d (quarantined); both confirmed to still pass when run with `-- --ignored`.

## New row, 2026-10-09 — review 507: a second `runtime::git` script-execution failure, the disposition pass's own next run

**Found in the reviewer's gate run taken immediately after the disposition pass reported 13 clean
runs.** Not a criticism of that pass — it is the evidence that 13 runs could not settle the question
it was asked to settle.

| Test | First seen |
| --- | --- |
| `runtime::git::tests::a_second_call_with_the_same_verified_executable_does_not_respawn_version` (`tests.rs:1357`) | review 507 (2026-10-09) |

Failed once in a full-workspace run; **5 of 5 clean in isolation** afterwards. The assertion is
`check_git_available(...).is_ok()` on a stand-in `git` script the test has just written.

**Almost certainly the `ETXTBSY` cause this document already describes** for its sibling
`every_failure_to_answer_is_unknown_and_never_none_ignored` (the 2026-09-25 hypothesis above): the
test writes a script and executes it immediately, tests in this binary run in parallel and fork, and
a child forked by another thread while this one holds the script open for writing inherits that
descriptor until it execs — `Text file busy` in the window between. Same module, same
write-then-exec shape, same parallel binary.

**Verdict: Fix.** Not, as speculated here, an inheritance of its sibling's verdict — checked
directly at the RFC-029 PR-029-A disposition below, and the inheritance assumption turned out to
be wrong: the sibling's own mitigation does not reach this test's actual code path at all. See
that entry for what was actually found and fixed.

## New row, 2026-10-10 — RFC-058 PR-058-B: a second `flock`-outlives-its-drop flake, same cause and same fix as the `transcript` row above

| Test | First seen |
| --- | --- |
| `tekstide_core::project_lock::tests::dropping_the_lock_releases_it_for_a_later_acquire` | PR-058-B's own development, 2026-10-10 |

Failed once in dev: a plain `drop(lock)` followed immediately by a second `acquire_project_lock`
on the same project saw `HeldByAnother` instead of `Acquired`. **The exact fork-pressure hazard
`transcript::tests::a_live_writer_holds_an_exclusive_lock_until_it_is_dropped`'s own comment
already documents** (row above, request 388): a sibling test in the same binary spawning a real
child process between this test's `fork` and `exec` can inherit a duplicate of the lock's file
descriptor, keeping the kernel-level `flock` held past the Rust-level `drop` for long enough that
an immediate re-acquire sees it as still taken.

**Fixed before the flake could recur in CI, not after**: read first, applied the identical
precedent — poll for `Acquired` against a 5s/10ms-interval deadline instead of asserting on the
very next line after `drop`. No separate investigation needed; the cause and the fix were both
already on record in this document. Confirmed clean across RFC-058's own three-slice gate (ten-plus
full-workspace runs across PR-058-A/B/C's review cycles, 0 recurrence). **Retired from active
tracking at the same time it was added** — fixed at first disclosure, the same shape as the
`transcript` row's own resolution.

## Cost tally, 2026-10-11 — review 533: row 1 again, one run in two

Not a new row and not a new disclosure — **row 1**
(`approval::tests::channel::bind_recovers_from_a_stale_socket_file`, verdict **Accept**) failed the
reviewer's first full-workspace run on PR-059-C's own tree, and the immediately following run on
the same tree was clean (`779 + 22 + 1135`). Isolated, it passed 5 of 5.

Recorded only to keep review 497's own cost argument current as `0.35.0` approaches its candidate:
an accepted row is accepted *because* re-running is cheaper than fixing it, and that trade only
stays true while someone is counting. This is one more re-run spent.

## New row, 2026-10-11 — RFC-059 review 527: `a_blocked_second_process_names_the_real_holder_and_its_knock_is_received`, 8 failures then 20 passes

**Disclosed by the reviewer, found while verifying a different slice.** Not caused by PR-059-A —
it fails at the commit before that slice too, which was checked in a worktree rather than assumed.

**The shape.** `BLOCKED <correct-pid> NO-KNOCK` — the second process names the real holder
correctly, and then `request_attention`'s own `UnixStream::connect` fails, so the test's
`ends_with("KNOCKED")` assertion fails. The pid assertion above it passes, so both processes agreed
on the project id and therefore on the socket path.

**What was measured, in order:**

| condition | result |
| --- | --- |
| `cargo test -p tekstide-core --lib <this test>`, main at `822265f` | **5 of 5 failed** |
| same, worktree at `e45f760` (before PR-059-A), bins freshly built | **3 of 3 failed** |
| full `cargo test -p tekstide-core --lib` (1134 tests) | passed |
| same filtered invocation, immediately after that full run | 10 of 10 passed |
| same filtered invocation, 4 busy cores of competing load | 10 of 10 passed |

**No cause established, and none is guessed here.** One ordinary explanation was checked and ruled
out: `bind_attention_listener` is called in `app.rs` inside the open path, so the holder has bound
its listener *before* it prints `ACQUIRED`, which is what the test's own spawn waits for. A naive
bind-after-announce race is therefore not it.

**Worth noticing about the assertion itself, for whoever dispositions this.** The product documents
this knock as **best-effort** — `request_attention`'s own doc says it "returns whether the holder
was reached, never blocks", RFC-058 D9 says the same, and
`docs/src/users/what-works-today.md` tells users "the request for attention is best-effort and may
do nothing visible". The test asserts `KNOCKED` **unconditionally**. A test that asserts a stronger
property than the product promises will fail whenever reality exercises the gap, and that may be
exactly what the five-then-three failures were. The disposition is therefore a real question rather
than a bug hunt: either the test synchronises on something that makes the strong assertion true, or
it is asserting more than RFC-058 ever claimed.

**Verdict: Accept.** See the PR-029-A disposition entry below.

## New row, 2026-10-12 — RFC-060 PR-060-C gate, third of three: `real_output_wraps_at_the_new_column_count_not_the_old_one`, 1 failure then 5 passes

**Disclosed while gating PR-060-C**, which added one new test
(`shell::tests::the_decided_approval_is_visible_through_the_real_audit_view`) and nothing else
touching `surface::terminal`. The first two of three full-workspace runs were clean (`781 + 22 +
1140`, `781 + 22 + 1140`); the third failed one test, unrelated to this slice's own change:

```
assertion `left == right` failed: the same 30-character line must now wrap at the new 20-column
width, not the old 80-column one -- proving the emulator is actually using the resized grid, not a
stale one
  left: 18
 right: 20
```

**The shape is category A from `test-flake-disposition.md`** ("the wait is weaker than the
condition it waits for"), not a new cause. The test's own poll loop
(`crates/tekstide/src/surface/terminal/tests.rs:472`) breaks on
`longest_run_in_any_row('Q') <= 20`, but the assertion below it demands exactly `== 20` — so a
transient render state where the 30-character marker has only partly arrived (visible as a shorter
run, 18 here) satisfies the loop's own weaker condition and lets it proceed to an assertion the
render had not actually settled into yet. The same family `resize_makes_the_pty_the_emulator_and_
the_render_path_agree` (disposition doc, category B, "PTY read timing," four prior occurrences) is
already filed under.

**What was measured:** isolated re-run, `cargo test -p tekstide --bin tekstide
surface::terminal::tests::real_output_wraps_at_the_new_column_count_not_the_old_one`, 5 of 5
passed. Consistent with a full-workspace-load-only timing gap, not a per-test defect.

**Not fixed here** — out of PR-060-C's own scope (approval/audit-view reachability, not terminal
resize timing), and `test-flake-disposition.md`'s own rule 4 forbids reporting an unablated fix.
Disclosed per this register's own standing rule that a disclosure is not complete without a row.

**Verdict: Fix.** See the PR-029-A disposition entry below — the fix shape taken is exactly the one
predicted here: poll the exact condition the assertion checks (`== 20`), not a weaker stand-in
for it.

## Disposition, 2026-10-13 — RFC-029 PR-029-A: the three rows live since the last disposition pass

**D1's own prerequisite**: `test-flake-disposition.md`'s open verdicts land before PR-029-B's
blocking CI job. Three rows were live going into this pass — the two "Verdict: none yet" rows
above, plus one (review 507's `runtime::git` row) whose own text speculated a verdict rather than
stating one. Each is read against the real, current code below, not against this document's own
prior account of it, matching the 2026-10-09 pass's own discipline.

### Fix — `a_second_call_with_the_same_verified_executable_does_not_respawn_version`

**Review 507's own note guessed the existing `wait_until_executable` mitigation already covered
this test**, "almost certainly the `ETXTBSY` cause... same module, same write-then-exec shape."
**That guess was checked, not inherited, and it was wrong.** `fake_git_with_config`
(`runtime/git/tests.rs`) calls `wait_until_executable(&path)` right after writing and `chmod`ing
its script, and its script's own template begins `[ "$1" = --tekstide-warmup ] && exit 0` so that
warmup call is side-effect-free. `marker_script` — the constructor this test actually calls,
through `Fixture::marker_script` — writes and `chmod`s an equivalent script with **no warmup guard
and no call to `wait_until_executable` at all**. The two constructors only look alike; the
mitigation review 507 pointed at does not reach the one this test uses.

**Fixed**: `marker_script`'s generated script gained the identical warmup-guard first line
`fake_git_with_config`'s already has, and `marker_script` now calls `wait_until_executable(&path)`
before returning, exactly mirroring its sibling constructor. The guard matters specifically for
this test: its own script body (`echo x >> counter; echo 'git version 2.99.0'`) is not idempotent
— without the guard, a bare warmup invocation would have appended an extra line to the same
counter file the test's own assertion counts, corrupting the property under test rather than only
closing the race. Verified directly: `cargo test -p tekstide-core --lib
runtime::git::tests::a_second_call_with_the_same_verified_executable_does_not_respawn_version`
still asserts exactly one counted invocation after the change, and the whole `runtime::git::tests`
module (62 tests) passes unchanged. Re-run 15 times under synthetic load (16 `yes` spinners, load
~19–28): 15 of 15 clean.

### Accept — `a_blocked_second_process_names_the_real_holder_and_its_knock_is_received`

**Re-investigated, not merely re-read.** Review 527's own measurement already ruled out a
bind-after-announce race by reading `app.rs`; this pass confirmed the same thing independently by
reading `add_project_from_path_protected` directly — `bind_attention_listener` is called and
returns before `ACQUIRED` is printed, synchronously, on the holder's own main thread, so the
attention socket is genuinely already listening by the time a second process can start at all.
`UnixStream::connect` to an already-`listen()`ing Unix socket succeeds independent of whether
`accept()` has been called yet (the connection queues), so the accept-side thread's own timing —
spawned by `project_process_probe` *after* its two status lines print — cannot be what makes
`request_attention`'s `connect()` itself fail, which is the specific call the test's own `KNOCKED`
assertion depends on.

**No plausible mechanism was found**, the same conclusion review 527 reached. This pass added its
own fresh measurement rather than resting on that one: 10 of 10 clean in plain isolation, 5 of 5
clean under synthetic load (16 `yes` spinners, load ~46–50), run today against the current tree —
**20 additional clean runs, 0 recurrence**, on top of review 527's own 10/10 and 10/10-under-load,
and the reviewer's own 10/10 plus four full-workspace runs at review 542. The only failures on
record for this row are the two from before PR-059-A existed (`5 of 5` and `3 of 3`).

**Review 542's own check, worth stating plainly rather than letting "before PR-059-A existed" imply
otherwise**: `git diff --stat 822265f..HEAD -- project_lock.rs project_lock/
bin/project_process_probe.rs app.rs` is **empty**. Every file in the path under test is unchanged
since the two failures. **Nothing was superseded, and nothing was fixed sideways — this is a
genuine unreproducible, not a defect that quietly went away.** A future reader who hits a
recurrence should know that, not wonder what regressed.

**The `connect()`-queues argument above also answers review 527's own open question directly**:
whether the test synchronises on something that makes its strong `KNOCKED` assertion true, or
asserts more than RFC-058 ever claimed. It is the former — `connect()` to an already-`listen()`ing
socket does not depend on the best-effort, may-do-nothing-visible half of D9 at all (that half
governs whether the *holder* ever calls `accept()` and reports `ATTENTION_RECEIVED`, a separate
assertion this test also makes and which the two historical failures did not touch). The `KNOCKED`
assertion is not over-asserting; the failure, whatever it was, is something else.

**Accept.** Same shape as row 1 (`bind_recovers_from_a_stale_socket_file`): sustained attention
from two independent investigators across two machines found no actionable cause, against code
that has not changed since the only two failures on record. Re-opens if it recurs.

### Fix — `real_output_wraps_at_the_new_column_count_not_the_old_one`

Disclosed above (PR-060-C gate), diagnosed there as category A — the loop's own break condition
(`longest_run_in_any_row('Q') <= 20`) is weaker than the assertion it leads into (`== 20`), so a
transient render of the second marker line (partially landed, read as a shorter run — 18 in the
one real recurrence) satisfies the wait and lets the assertion run against a grid that has not
actually finished settling.

**Fixed**: the loop's own break condition now requires `== 20` — the identical value the assertion
checks — so the wait and the assertion can no longer disagree about what "done" means. **Ablation
was attempted and, as with this family's three prior members (the 2026-10-09 disposition pass's
own "Fix — one shared defect behind four tests" section), did not reproduce the failure under
synthetic load either before or after the change** (16 `yes` spinners, 15 runs each). The same
reasoning that pass already used applies here without modification: the new condition is strictly
*stronger* than the old one (`== 20` implies `<= 20`, never the reverse), so the fixed wait can
only wait at least as long as the old one for the identical final correctness property — there is
no condition under which it returns earlier. A wait that only ever waits at least as long cannot
be the cause of a new failure; it can only remove the old one. Verified directly: 15 of 15 clean
under the same synthetic load this document's own precedent used to justify accepting reasoning
over a forced reproduction.

### Gate

`cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -D warnings`: clean.
Three consecutive full-workspace runs, fresh short `/dev/shm` `TMPDIR` each — recorded in
`rfcs/handoffs/029-ci-and-distribution/qa-evidence.md`'s own PR-029-A section, not repeated here.

**Live rows remaining after this pass: zero.** Every row this register has ever opened now carries
exactly one verdict — fixed (with its fix verified, and ablated where the family's own precedent
showed ablation could reproduce anything at all), quarantined with a reason, or accepted with a
reason that is not scheduling. RFC-029's own D1 — the prerequisite PR-029-B's blocking job depends
on — is satisfied.

**Corrected below, 2026-10-14: this is no longer true for one row.** The Accept two sections above
recurred, and is re-verdicted to Quarantine. Nothing above this note is rewritten, per this
register's own rule against deleting a row's history — read the row below for the current state.

## Re-opened, 2026-10-14 — RFC-029 candidate-cut gate: `a_blocked_second_process_names_the_real_holder_and_its_knock_is_received`, 3 of 3 failed

**The Accept above says "Re-opens if it recurs." It recurred — 3 of 3**, on three fresh
full-workspace runs gating the `0.37.0` candidate cut, disclosed rather than re-run to green.

**What was added to the picture, not just the recurrence itself:**

| condition | result |
| --- | --- |
| full-workspace runs, real contention (load 3.63/5.61/10.33; a CPU-pinned `qemu-system-riscv64`; six other sessions on this shared desktop) | **3 of 3 failed** |
| the same test, run alone (`-- --exact`), same machine, same moment | passed |
| full-workspace runs, moderate load (~1.1) | 3 of 3 green |
| the same test alone, under **96 `yes` spinners on 32 cores** (load 14.7) | 10 of 10 green |

**Synthetic CPU load does not reproduce it — now established twice**, by this pass's own 96
spinners and by review 527's original 16. So the trigger is not CPU contention as such, which the
Accept's own investigation had already suggested by ruling out a bind-after-announce race; what is
new here is that *real* machine contention (whatever its actual shape — six concurrent sessions,
not a synthetic spinner load) produces a result synthetic load of either size does not.

**The shape is the finding, not the rate.** Across every measurement on record — review 527's own
8 consecutive failures, then 40-plus clean runs since, now this pass's 3 consecutive failures — this
is not well modeled as a per-run probability. It reads as **bimodal**: under some machine state it
fails deterministically for the whole session, and otherwise it does not fail at all. A test with a
small constant failure rate and a test that is reliably red for an entire sitting look identical to
whoever reads one CI run, but they are different things to act on, and only the second one is
indistinguishable from a real regression to that reader.

### Quarantine — supersedes the Accept above

**D1 is RFC-029's own premise: a gate is trustworthy only once every live row has a verdict that
holds up under blocking use**, not under the looser standard of a human occasionally re-running a
local gate by hand. Accept was written and defensible against the latter. It does not survive the
former: `test-flake-disposition.md`'s own quarantine criterion is "the test does not belong in a
correctness gate," and a test that can run red for an entire session is exactly that gate's failure
mode from the outside — indistinguishable from the thing D1 exists to make trustworthy again.

**Moved out of the three-run gate.** `#[ignore]`d in
`crates/tekstide-core/src/app/tests.rs`, with the reason recorded in the attribute itself, matching
the convention this register's own category-C quarantines (`terminal_poll_handler_cost_under_a_
real_wake_driven_flood_headless_benchmark` and its sibling) already use. Still runs with
`-- --ignored`; its coverage of RFC-058 D10 (the real knock, over a real socket, between two real
processes) is not deleted, only taken out of the blocking set.

**No cause is claimed.** This disposition is about where the test runs, not why it sometimes fails
— the Accept's own investigation (the `connect()`-queues argument, ruling out a bind-after-announce
race) stands unchanged; nothing here contradicts it or claims to have found what actually happened
under real contention.

### Gate

`cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -D warnings`: clean.
`cargo test -p tekstide-core --lib` with the attribute in place: the test is skipped by default and
passes under `-- --ignored` (1 of 1). No other test's behavior changed.

### Correction, same day — "real contention" was the wrong variable too

**The reviewer did not take the row above on trust, and found it names the wrong cause.** Four
distinct reproduction attempts now on record, two of them new:

| condition | runs | result |
| --- | --- | --- |
| 16 synthetic spinners (review 527) | 15 | green |
| 96 synthetic spinners, 32 cores, load 14.7 (this row, above) | 10 | green |
| isolated, real load 18.39 — real concurrent builds and other sessions, no synthetic load | 8 | green |
| **in-suite, `#[ignore]` ablated away in a throwaway `git worktree`, real load 15.02/11.40/8.84** | **3** | **green — passed all three, full workspace** |

The last row is the one that matters: the reviewer removed the attribute this row just added, in a
disposable worktree, and ran the full suite three times across the middle of the load range my own
three failures sat in (3.63–10.33). **It passed 3 of 3 under the condition this row names as the
cause.** Contention — real or synthetic — is therefore **not sufficient on its own**, and remains
unreproduced by every attempt on record; nor does suite-context alone reproduce it, the reviewer's
own next hypothesis, killed by that same ablation. Neither is shown irrelevant — only that neither
alone is the trigger.

**The trigger is unidentified, not "real machine contention."** This row's own framing above is
wrong and is left in place rather than rewritten, per this register's standing rule — read it as
superseded by this section, not as the current account. A future reader checking the load and
finding it low must not conclude the test is fine: eighteen isolated runs and three in-suite runs
with the attribute ablated away all passed, at loads spanning the entire range this register has on
record. **That shows load is not sufficient, and so far unreproduced — not that it is ruled out.**
The three failures on record all occurred under load; what the ablation establishes is that load
alone does not reproduce them, not that load is irrelevant. Whether load is a necessary condition
remains untested either way.

**This strengthens the quarantine rather than weakening it.** A test that failed 8 consecutive times
in one session and 3 consecutive in another, with no identified trigger after four attempts, is
exactly what must not sit in a blocking gate — the verdict stands; only the stated reason changes.

**One more thing the reason string got wrong, now fixed alongside it**: "run with `-- --ignored` for
its own number" implied that number describes the property that broke. It does not — eighteen
isolated runs have never once failed, which is the one condition on record under which this test has
never been seen to fail at all. The `#[ignore]` attribute's text is corrected to say so, so a clean
`-- --ignored` run is not misread as the row being closed.

## Cost tally, 2026-10-14 — review 554: row 1 again, and its "no actionable fix" is no longer true

**Row 1** (`approval::tests::channel::bind_recovers_from_a_stale_socket_file`, verdict **Accept**)
failed run 2 of the reviewer's own three-run gate on the `0.37.0` candidate at `9037aee`, at load
5.03, with 0 fixture entries left. Runs 1 and 3 were clean, and a fresh three-run gate on the same
commit was **3 of 3 green** (loads 0.93 / 2.81 / 2.83) — so the candidate's gate is satisfied, and
this is the fourth re-run spent on this row since review 497 started counting.

**The panic, in full:**

```text
panicked at crates/tekstide-core/src/approval/tests/channel.rs:170:10:
second bind must clear the stale file and succeed: ApprovalChannelError { reason: Io, source: None }
```

### What `source: None` now narrows this to — and why that contradicts the Accept

The Accept above reads: *"never isolated to a more specific cause despite sustained attention … the
shape offers no actionable fix."* **`source: None` is a specific cause, or rather it excludes
almost everything else.** `clear_stale_socket` (`approval/channel.rs`) has exactly three failure
arms, and response 231 already fixed the third one to preserve its `io::Error` — with a comment
naming *this test's own flake panic* as the reason for that fix:

| arm | error | can produce `source: None`? |
| --- | --- | --- |
| the path exists and is **not a socket** | `new(Io)` | **yes** |
| `connect()` returns **`Ok`** — something is really listening | `new(Io)` | **yes** |
| any other `connect()` error (`EPERM`, `EMFILE`, …) | `io(error)`, response 231 | no — carries errno |

Every other `Io` in `bind()`'s chain goes through the source-preserving `::io` as well. **So this
occurrence came from one of the two arms response 231 left indistinguishable from each other**, and
the fix that was made specifically so a future occurrence of this flake would carry evidence
produced an occurrence carrying none.

**Three mundane explanations were checked and ruled out** rather than assumed:

- *Two tests colliding on one socket path.* No: `rand_seed()` is a process-wide monotonic
  `AtomicU64` counter, and `temp_state_root` already includes the pid, so every test's directory and
  socket path are unique.
- *An errno-bearing failure whose source was dropped.* No: that is the arm response 231 fixed, and
  it would print `source: Some(..)`.
- *`/proc/self/fd/<n>` resolving into another thread's directory after fd reuse.* No: `bind()` holds
  `dir_fd` as an `OwnedFd` across the whole `clear_stale_socket` call, so that fd number cannot be
  reused while the magic path is in use.

What is left is two conditions that should both be impossible in this test — a staged socket file
that is not a socket, and a dropped raw listener that still accepts a connection — and a panic that
cannot say which one happened.

### The actionable step the Accept said did not exist

**Give those two arms their own `ApprovalChannelErrorReason` variants.** `SocketPathTooLong` and
`ProcMagicPathUnavailable` are already in that enum for exactly this reason, both distinguished from
a bare `Io` on the argument that an identifiable condition must not be indistinguishable from any
other bind failure — and both are in this same function's own call path. The two arms here are the
remaining exception.

This is **core-only and needs no user-facing string**: the test's panic prints the error's `Debug`,
so distinct variants alone make the next occurrence self-diagnosing. Whether either condition also
deserves its own refusal symbol in `agent_run_launch_refusal_symbol` is a separate question for
whoever takes it — today both fall into the `Runtime(_) => "error"` catch-all, so a user who hits
either sees the generic *"Couldn't start an agent run."*, which is the same shape RFC-060 D4 fixed
for `SocketPathTooLong`.

**Not done here, and not a `0.37.0` blocker**: the flake predates this release, nothing in the
release touches that code, and the measured rate has not moved. Recorded so the next occurrence is
evidence rather than one more tally mark, and so the Accept is re-decided against what is now known
rather than left standing on a premise this entry contradicts.

## New row, 2026-10-14 — gating the `--licenses`/NOTICE fix: two `reference_adapter` tests, 1 failure each, not since

Disclosed rather than silently redone. Gating the NOTICE-embedding fix (`main.rs`, `release.yml`,
two new invariants; no change anywhere near `approval::tests`), one full-workspace run at load
~21.9 (this shared machine, heavily contended at the time — `uptime` read `21.94, 10.08, 7.84`)
failed two tests together: `approval::tests::reference_adapter::deciding_a_proposal_whose_real_
adapter_process_has_already_exited_is_undeliverable` and `approval::tests::reference_adapter::
a_real_adapter_process_refuses_to_run_without_a_token`. Both spawn a real `reference_adapter`
child process.

**Not reproduced since.** Both, isolated immediately after alongside their five siblings in
`reference_adapter::tests`: 7 of 7 passed. Three further full-workspace redos, same commit: 2 of 3
clean (the third hit row 1 above, `bind_recovers_from_a_stale_socket_file`, already an open Accept
— not these two). No cause investigated beyond confirming they are not a regression from the
commit being gated: neither test nor anything it calls was touched by this change, and the isolated
pass rules out a logic defect this specific run introduced. **Accept, carried as load noise** on a
machine shown contended at the time, consistent with this register's existing pattern for
real-process-spawning tests under genuine (not synthetic) load -- re-opens if either recurs with
enough frequency to investigate.

**Review 556's own correction, worth carrying forward rather than filing this as noise-shaped and
leaving it there**: load explains *a* failure, but these two failed together, in the same run, and
both spawn a real `reference_adapter` child process -- that correlation is not noise-shaped, it
points at a shared mechanism (process-spawn limits, the shared `RealProcessLimiter`, fd or PID
pressure), the same way the shared-audit-store cause this register already resolved once looked
"exactly this shape before isolation." A future occurrence should start from that correlation, not
from re-checking `uptime`.

## New row, 2026-10-14 — the `0.37.0` release gate: `linux_runtime_terminates_session_leader_with_sighup`, 1 in 6

**A new disclosure, not previously in this register.** Found by the reviewer's own release gate for
`0.37.0`, run in a **fresh clone** (see `release-checklist.md`'s own new note on why the gate ran
there), at commit `96cdc94`, load 16.25:

```text
panicked at crates/tekstide-core/src/runtime/terminal/tests.rs:365:5:
assertion `left == right` failed
  left:  Some(Terminated { ..., outcome: Exited { exit_status: 1 } })
  right: Some(Terminated { ..., outcome: TerminatedBySignal { signal: Sighup } })
```

**The shape**: the test asserts `request_terminate` reports the session leader as *killed by
SIGHUP*. It reported a *normal exit with status 1* instead. The product did terminate the leader
either way — what differed is which of the two ways `waitpid` saw it end, which is a race between
the signal arriving and the shell exiting of its own accord under load.

**1 failure in 6 runs** at the same commit: run 3 of the first clone gate failed; the three-run gate
re-run immediately after was **3 of 3 green at loads 27.11 / 28.53 / 26.94** — every one of them
*higher* than the 16.25 the failure occurred at, which is the second time today that a higher load
has failed to reproduce a contention-correlated failure. That failing run also left **1 fixture
entry**, consistent with this register's established pattern that a failing run leaks one; the three
green runs left 0.

**Verdict: Accept, as a first occurrence, with the family named.** No investigation beyond
establishing it is not a regression from anything in `0.37.0`: the release's own diff touches
`main.rs`, `release.yml`, two invariants and documentation — nothing in `runtime::terminal` or
anything it calls. Re-opens if it recurs.

### The family is now worth more than any of its rows

**Three distinct real-process tests failed today, each under genuine multi-agent contention on this
shared desktop, and none of them under synthetic load**: the knock test (the dev team's gate, 3 of
3, quarantined), the `reference_adapter` pair (the `--licenses` gate, failing *together* in one
run), and this one. Against that, every synthetic reproduction attempt on record has failed — 16
spinners at review 527, 96 spinners on 32 cores at review 552, and the in-suite ablation at review
553 that passed 3 of 3 at the load its row blamed.

What distinguishes the real condition from the synthetic one is **not CPU load** — the numbers above
rule that out directly, with failures at 16 and clean runs at 28. The untested variable is what
*other* processes on this machine are doing to the resources these tests actually contend for:
process-spawn slots, PTY devices, fds, PID pressure, scheduler latency on a signal delivery. Six
concurrent agent sessions building and testing different projects is a different load *shape* from
96 spinning CPUs, not merely a different magnitude.

**That is the one hypothesis this register has never tested**, and it is cheaper to test than another
spinner run: measure what the machine's `/dev/pts` occupancy, fd count and fork rate are doing during
a failing run, rather than its load average. Recorded here as the next thing to try for any member of
this family, in place of a fourth synthetic-load attempt that the first three predict will be green.

## Row 1 re-verdicted, 2026-10-14 — the diagnostic gap review 554 found is closed

**Row 1** (`approval::tests::channel::bind_recovers_from_a_stale_socket_file`, verdict **Accept**)
had its premise falsified at review 554: *"never isolated to a more specific cause … the shape
offers no actionable fix"* stopped being true the moment `source: None` narrowed the panic to one
of two indistinguishable arms in `clear_stale_socket`. That review named the actionable step —
distinct `ApprovalChannelErrorReason` variants for the two arms — and left it for this slice.

**Done**: `BindTargetNotASocket` (the bind path exists and is not a socket) and
`BindTargetAlreadyListening` (`connect()` returned `Ok` — something is genuinely listening) added
to `ApprovalChannelErrorReason`, replacing the bare `Io` both arms of `clear_stale_socket`
previously returned. Core-only, no user-facing string, exactly as review 554 scoped it — whether
either arm also deserves its own refusal symbol in `agent_run_launch_refusal_symbol` is left as a
separate, later question; today both still fall into that match's own `Runtime(_) => "error"`
catch-all, confirmed by reading the match itself rather than assumed, so adding the variants needed
no change there.

**Proven by two deterministic tests, not by waiting for the race to recur.** The existing
`stale_socket_clearing_refuses_a_non_socket_file` gained the `BindTargetNotASocket` assertion (it
already forced that arm, just never checked which reason it produced); a new
`a_live_listener_at_the_bind_path_is_refused_as_already_listening` forces the second arm by keeping
a raw listener alive through the second bind attempt, instead of the original flaky test's
drop-then-rebind race. **Ablated both**: reverted each new variant back to the bare `Io` both
replaced, confirmed both tests fail naming the exact mismatch (`left: Io, right:
BindTarget{NotASocket,AlreadyListening}`), restored, confirmed green again.

**Verdict: Accept stands, now instrumented rather than unactionable.** This does not fix the
underlying race — what makes the staged "stale" socket sometimes still look like a live listener,
or sometimes fail the socket-type check, immediately after its raw listener was dropped, remains
unexplained, and no cause is claimed here. What changes is that the *next* occurrence of this flake
will panic naming one of these two reasons instead of a bare `Io` with no source — turning a dead
end into a lead, which is the property review 554 found missing and asked this slice to restore.

### Gate

`cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -D warnings`: clean.
`cargo test -p tekstide-core --lib approval::tests::channel::`: 24/24.

**Required correction, review 570: the first draft of this gate line said "0 failed … did not
recur in any of the three," which was false and never re-checked against what those three runs
actually printed.** Three full-workspace runs **in this working tree** each failed exactly one
test, every time: `real_repository_filesystem_scan_cost_headless_benchmark`. Nothing else failed in
any of the three runs; 0 fixture entries left each time, approval-channel tests included.

**Second correction, review 571: `.git-exclude/` itself was the wrong set to blame.** The first
correction above named `.git-exclude/`'s own raw entry count (4,373) as what exceeds the 4,096 cap.
It does not — the scanner excludes `target`/`node_modules`/`.git` by name at any depth, and
`.git-exclude/`'s own scanner-visible share (after the same exclusion) is **~2,908, well under the
cap on its own**; most of its raw count sits inside two stray `target/` directories
(`.git-exclude/tmp/audit-open-probe/target`, `.git-exclude/tmp/pubcheck/target`) the scanner never
counts at all. **What is actually over the cap is the whole repository tree, and only just**: 4,067
scanner-visible entries against the 4,096 limit, roughly **1% over**, confirmed by running the test
directly (`excluded_status: Partial { limit: 4096 }` against the required `Complete`). That margin
means the right prediction is not "a stable, structural failure" but **"this comes and goes with a
few dozen files"** — a handful of review letters pushes the tree over; cleaning up unrelated scratch
files pulls it back under. A future reader measuring a tree that happens to pass is not evidence
the cap stopped being a real concern; the margin itself is why `release-checklist.md`'s own fix
(measure the repository's *tracked* content, not the working directory) matters even for a fresh
clone eventually, not only for this one. Gating this slice in a fresh clone, per that section's own
remedy, would show 0 failures today — not attempted here, since the working-tree result above is
the one that needed correcting, not replacing.

## Recurrence, 2026-10-14 — `linux_runtime_terminates_session_leader_with_sighup`, 2nd occurrence, and the first with a *different* outcome value

**Its own row said "re-opens if it recurs." It recurred, in the reviewer's own verification run for
review 575**, a single full-workspace run in the working tree at `df6a405`
(`TMPDIR=/dev/shm/tk575`, `--workspace --all-targets --all-features --no-fail-fast`):

```text
panicked at crates/tekstide-core/src/runtime/terminal/tests.rs:365:5:
assertion `left == right` failed
  left:  Some(Terminated { ..., outcome: Exited { exit_status: 0 } })
 right: Some(Terminated { ..., outcome: TerminatedBySignal { signal: Sighup } })
```

`1140 passed; 1 failed; 4 ignored` — this test and nothing else.

**What is new, and it is the first real discriminator this row has had**: the first occurrence
showed `Exited { exit_status: 1 }`; this one shows `Exited { exit_status: 0 }`. The shell did not
merely lose a race against `SIGHUP` in the same way twice — in one case it ended with a failure
status, in the other it exited **cleanly**. A clean exit is much harder to explain as "the signal
arrived a moment late and `waitpid` saw the other ending first": `exit_status: 0` is what a shell
reports when it reached end-of-input and left of its own accord, which points at the PTY master
side being closed (or the shell seeing EOF) before the signal was ever delivered, not at signal
timing alone. Two occurrences, two different exit statuses, is the shape that argues the row's own
"race between the signal arriving and the shell exiting of its own accord" has **two distinct
causes** behind it rather than one.

**Load context, since this row's family note turns on load shape rather than magnitude**:
`/proc/loadavg` immediately after the run read `2.15 10.90 17.63` on 32 cores — so the run itself
sat in the 11-18 range, in the same band as the first occurrence's 16.25 and well below the 27-29
at which three consecutive runs were green. Nothing new for the shape hypothesis: it was still not
instrumented, which is this family's standing next step and remains untested. **Said plainly: a
second occurrence happened under direct observation and the `/dev/pts`/fd/fork-rate measurement
the family note asks for was not attached, so that hypothesis is no better tested than it was this
morning.**

**One fixture entry left behind, and it is this test's own**:
`/dev/shm/tk575/tekstide-terminate-sighup-1435340-1791988447183316779`, an empty directory named
for the fixture the failing test creates. Consistent with this register's established
one-leak-per-failing-run pattern, and worth stating explicitly so a future reader does not read
the non-zero fixture count as an independent leak: when this row fires, the `0 fixture entries`
gate assertion fails with it, for this reason.

**Rate, updated: 2 failures in 7 recorded runs** at two different commits (`96cdc94`, `df6a405`),
all in the 11-18 load band, none reproduced under synthetic load.

**Verdict: Accept stands, re-opened as predicted, and promoted to the family's primary subject.**
No cause claimed. What changed is that this row now has two observations that differ in a way the
single-cause explanation does not cover, which makes it the cheapest member of the family to
investigate next: the next occurrence should capture the *child's* own exit path (whether the PTY
master was closed before the signal was sent) rather than only the load average. That is a
narrower, more answerable question than the family note's own `/dev/pts` measurement, and it does
not need the failure to be reproducible on demand — only for the next run that fails to carry the
instrumentation.

## Recurrence, 2026-10-15 — row 1, the first real occurrence since instrumentation, and it names an arm

**Row 1** (`approval::tests::channel::bind_recovers_from_a_stale_socket_file`) fired again, in my
own gate for review 575's two required edits, the third of three full-workspace runs
(`cargo test --workspace --no-fail-fast`) in this working tree at the new follow-up commit:

```text
thread 'approval::tests::channel::bind_recovers_from_a_stale_socket_file' panicked at
crates/tekstide-core/src/approval/tests/channel.rs:170:10:
second bind must clear the stale file and succeed: ApprovalChannelError { reason:
BindTargetAlreadyListening, source: None }
```

`1140 passed; 1 failed; 4 ignored` — this test and nothing else. Load at the time: `2.23 1.49 1.86`
on this machine, far below every load figure recorded for the SIGHUP-race family above — this row's
own race is not sharing that family's load sensitivity, at least not on this one occurrence.

**This is the first time the instrumentation added for the row-1 re-verdict above has actually
fired**, and it answers the question that re-verdict left open: the arm is
**`BindTargetAlreadyListening`** — `clear_stale_socket`'s connect probe against the staged bind path
returned `Ok`, i.e. something looked like a live listener, immediately after the raw listener that
was staged there had already been dropped. Not `BindTargetNotASocket`. That rules out one of the
two previously-indistinguishable arms outright, for this occurrence, and narrows the open question
from "which of two arms" to "why does a dropped `UnixListener`'s socket still answer `connect()`" —
a kernel-timing question (socket backlog/`SO_REUSEADDR`-shaped), not a file-type question.

No fixture leak: `temp_state_root`'s `remove_when_this_test_ends` guard ran despite the panic;
checked directly (`find /tmp -maxdepth 1 -name 't*-*'`) immediately after, nothing left.

**Verdict: Accept stands.** No fix attempted or claimed here — recording the first real data point
the instrumentation was built to produce is the whole contribution of this entry. The next
occurrence should capture whether `SO_REUSEADDR` or an in-flight (not yet accepted) connection on
the old listener's backlog explains a dropped listener still answering `connect()`.

### Correction, 2026-10-15, review 576 — neither `SO_REUSEADDR` nor backlog can explain this

The closing line above sent the next investigator at two mechanisms that turn out not to apply:

- **`SO_REUSEADDR` has no meaning for an `AF_UNIX` pathname socket on Linux.** It is not consulted
  when binding a filesystem socket; rebinding a path that still exists fails with `EADDRINUSE`
  regardless, which is exactly why `clear_stale_socket` has to `unlink` before binding at all. The
  option cannot make a `connect()` succeed or fail.
- **A backlog entry cannot make a *new* `connect()` succeed.** When a listening `AF_UNIX` socket is
  closed, queued-but-unaccepted connections are reset and subsequent connects to that path get
  `ECONNREFUSED`. Backlog state affects connections already queued, not the next one.

Two other hypotheses, ruled out in review rather than guessed at, worth recording so nobody
re-treads them:

- **Path collision between concurrent tests: impossible.** `rand_seed()` is a process-wide
  `AtomicU64` counter and `temp_state_root` includes `std::process::id()`, so two tests in one
  binary cannot land on the same socket path, and two binaries cannot share a pid.
- **`bind()` probing a path it had already bound: impossible.** There is no retry loop —
  `clear_stale_socket` runs once, strictly before the single `UnixListener::bind`.

That leaves the actual question open: something answered that connect, and neither of the two
mechanisms first guessed at explains it.

## New instruments, 2026-10-15, review 576 — and the staging-side one fired live, repeatedly, in the gate that added it

Review 576 commissioned two small instruments rather than a fix: capture the peer's `SO_PEERCRED`
(pid/uid/gid) on `clear_stale_socket`'s `Ok(_)` arm, attached to `BindTargetAlreadyListening`'s
`source` without changing the refusal itself; and, in `bind_recovers_from_a_stale_socket_file`,
assert the test's own unverified precondition — that connecting to the path right after
`drop(raw_listener)` returns `ConnectionRefused` — before the real second `bind()` call runs.

**Both landed and were ablated as specified**: the product-side instrument confirmed against
`a_live_listener_at_the_bind_path_is_refused_as_already_listening` (a deterministic, same-process
listener) — the diagnostic source names `pid=<this test process's own pid>`, proving the mechanism
correctly identifies an in-process answerer. The test-side precondition assert was ablated by
staging a listener that is deliberately *not* dropped (`std::mem::forget` in place of `drop`,
reverted immediately after, never committed) — confirmed it panics naming exactly the precondition
it is supposed to guard, then restored.

**Then, while re-running the gate on the restored, un-ablated file, the precondition assert fired
for real** — not the ablation, the committed code, run honestly:

```text
thread 'approval::tests::channel::bind_recovers_from_a_stale_socket_file' panicked at
crates/tekstide-core/src/approval/tests/channel.rs:176:18:
precondition failed: connecting to the path left by a dropped UnixListener should be refused, got
Ok(UnixStream { fd: FileDesc(OwnedFd { fd: 20 }), local: (unnamed), peer: "/tmp/t384545-3/approval/
agent-run-00000000-0000-0000-0000-000000000006.sock" (pathname) }) -- the staging itself, not
clear_stale_socket, is where this occurrence's phenomenon lives
```

**This answers review 576's own framing directly**: *"If it ever fires, the phenomenon lives in the
staging — a dropped `UnixListener` whose path still answers — and `clear_stale_socket` is merely the
messenger, which is a different investigation from the one the row currently describes."* It fired.
The peer in every capture below is a real, successfully-connected `UnixStream` at exactly this
test's own socket path, confirmed by the `peer:` field each time — not a different test's path, not
a stale directory collision.

**Measured rate, same afternoon, same machine (32 logical CPUs, `cargo test`'s default thread
count)**:

| Run shape | Failures | Total runs |
|---|---|---|
| `bind_recovers_from_a_stale_socket_file` alone (`--exact`, necessarily single-threaded) | 0 | 30 |
| Full `approval::tests::channel::` module (24–32 tests concurrently) | 6 | 31 |

Six occurrences, all with the same shape (`Ok(UnixStream { .. })` where `ConnectionRefused` was
expected), captured across two batches:

```text
run — pid     — fd  — agent_run_id suffix
   1 — 384545 — 20  — ...006   (first, found re-running the gate after `cargo fmt`)
   2 — 387155 — (location captured, message not — grep pattern missed the detail line)
   3 — 406585 —  5  — ...008
   4 — 412064 — 21  — ...008
   5 — 428632 —  7  — ...00b
   6 — 428811 — 13  — ...005
```

**Zero reproductions in 30 single-test runs, six in 31 concurrent-module runs — roughly 19%, and it
took minutes, not months.** This is the opposite experience from every other row in this document:
this one is cheap to reproduce on demand, which makes it the easiest flake in this entire register
to actually root-cause, if someone attaches the right instrumentation next. The correlation itself
is the finding — not a claimed mechanism. A plausible shape (Linux's `AF_UNIX` socket teardown on
`close()` is not required to be instantaneously visible to a concurrent `connect()` from another
thread in the same process, under heavy concurrent socket churn from sibling test threads) is
offered here as a hypothesis worth testing, explicitly **not** as a conclusion — nothing above rules
out an application-level explanation either, and none was tested.

No fixture leak in any of the runs that failed (same drop-guard mechanism as every other row above;
checked after each batch).

**Verdict: Accept stands, and row 1 is promoted ahead of the SIGHUP family as the cheapest flake in
this register to actually solve.** What the next investigator gets for free that no prior row in
this document has had: a ~1-in-5 reproduction rate, on demand, in under a minute
(`for i in $(seq 1 20); do cargo test -p tekstide-core --lib approval::tests::channel:: -- --test-threads=24; done`
— no synthetic load generation needed, unlike every attempt at the SIGHUP family above). The
product-side instrument (peer pid/uid/gid on the `Ok(_)` arm) has not yet fired on a real
occurrence — only on the deterministic same-process test — so whether a *real* stale-recovery path
ever shows a peer pid other than its own test process remains the open half of the question.

## Row 1 solved, 2026-10-15 — review 577's three-part slice: mechanism confirmed, staging fixed, a security property added

Review 577 named the mechanism (a sibling `Command::spawn` in this same test binary forks; the
child briefly holds a duplicate of whatever fd this process's own `UnixListener::bind` just
created, until the child reaches `exec`, where `CLOEXEC` closes it) and commissioned three ordered
parts. All three done, in order.

### Part 1 — confirm the window, name the holder

Instrumented `bind_recovers_from_a_stale_socket_file`'s precondition check so that when it finds
the path still answering, it measures how long the path keeps answering and attempts to identify
the holder via `/proc/net/unix` → inode → `/proc/*/fd` scan → `/proc/<pid>/comm`.

**A real measurement bug, caught and fixed before relying on the numbers**: the first version read
`/proc/net/unix` (and potentially scanned every process's `/proc/*/fd`) *before* starting the
timer, which is not free — doing so attributed the lookup's own latency to the window being
measured, producing readings of 7-9 **microseconds** that undersold how long the window really is.
Moving the timer to start first (the lookup still runs, now timed honestly, inside the loop)
produced the real figures: **1.9ms, 5.1ms, 5.3ms, 5.3ms** across the first four live catches that
afternoon, widening to **1.5ms-6.5ms** across eight catches total once review 578's own
verification gate ran the suite further — low single-digit milliseconds throughout, not
microseconds, and never approaching the 500ms budget below, but still squarely inside "the
hypothesis predicts a window that closes," not "never closes." The exact bound is not load-bearing
for the fix (`wait_for_path_to_stop_answering`'s own doc comment and failure message say
"single-digit milliseconds" rather than quote a specific range, deliberately, so neither drifts out
of date as more samples arrive).

**A second real finding, this one about the kernel rather than this test's own code**:
`/proc/net/unix` can list a socket still bound to its path with `Inode` printed as literal `0` —
confirmed as a genuine kernel-reported value, not a parsing bug, by checking the field alignment
against a real `/proc/net/unix` sample by hand. This happens when the socket is already mid-
teardown and has lost its inode association; no `/proc/*/fd` scan can find a holder past that
point, because no fd can reference inode `0`. The fix was not to treat this as "not found" silently
but to collect every line matching the path (not just the first) and prefer any non-`0` inode over
a `0` one, since both can appear for the same path while one is mid-teardown and a genuine live
listener also exists. Verified against a deterministic same-process ablation (a leaked, never-
dropped listener): before the fix, 1 of 2 ablation runs reported `inode=0, no holder found` for a
listener that was certainly still open and certainly held by this exact process; after the fix,
5 of 5 ablation runs correctly reported `pid=<this process> comm=tekstide_core-e`.

Final honest holder results on the four *real* (non-ablated) catches: two reported `inode=0`
(genuinely too transient for even the fixed lookup to catch a live fd, consistent with a window
this short), one reported an inode number but still no `/proc/*/fd` holder (the fd closed between
the `/proc/net/unix` read and the `/proc/*/fd` scan — the scan itself, iterating every process on
the system, is not fast enough for a sub-6ms window), and none positively named the forked child's
pid. **The mechanism is confirmed by the timing (consistently low-single-digit milliseconds,
exactly the shape a fork/exec window predicts) and by the deterministic ablation proving the lookup
code itself works — not by catching the actual child in the act, which this approach is
structurally too slow to do.**

### Part 2 — fix the staging, not the product

Per review 577's explicit instruction, `clear_stale_socket` was not touched. The test's
precondition (`wait_for_path_to_stop_answering`) now accepts the measured window explicitly: checks
once, cheaply, with no `/proc` work on the common path; only if that first check finds the path
still answering does it poll (with the holder-lookup attempt riding along) for up to 500ms before
failing. **Verified this actually removes the flake**: 60 consecutive runs of the full
`approval::tests::channel::` module, 0 failures (previously ~19% per the measured rate above) — and
separately, the ablation above proves a genuinely-never-closing listener still fails loudly, so the
tolerance is bounded, not a blanket pass.

### Part 3 — the security property this investigation earned

Added `the_approval_listener_is_close_on_exec`: binds a real `ApprovalChannelEndpoint`, reads
`fcntl(fd, F_GETFD)` directly against the kernel, asserts `FD_CLOEXEC` is set. This is independent
of parts 1-2 and was true before this slice (std sets it), but nothing had verified it for this
codebase's actual bind path until now — and the property is exactly what keeps the mechanism found
in part 1 from being a real vulnerability: a spawned AI CLI agent process inheriting the approval
channel's listening socket past its own `exec` could accept connections on the channel meant only
for the real adapter, without needing to connect or know the token at all. Ablated (inverted the
assertion, confirmed it fails naming the real risk, reverted).

### Gate

`cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`: clean.
`approval::tests::channel::`: 25/25 (one more test than before — the new `close-on-exec` check).
`rfc_docs_invariants`: 23/23. Three full-workspace runs: 0 failed each (1142 passed in
`tekstide-core`'s own suite, one more than the prior gate's 1141).

### Verdict: row 1 closed as a flake, open as a documented, accepted, bounded race

This is the first row in this entire document to go from "recurring, unexplained" to "mechanism
confirmed, staging fixed, verified not to recur across 60 runs" in a single slice. What remains
genuinely open, stated plainly rather than implied fixed: the holder was never positively named on
a live occurrence, only inferred from timing and a deterministic stand-in.

**Production reachability, review 578's own sharpening**: the race needs two conditions to
coexist, not one -- a stale socket *file* that outlives its own listener, **and** a sibling process
holding a duplicate of that listener's fd at the moment something else connects to the path. In
production, `ApprovalChannelEndpoint::drop` removes the socket file as part of the endpoint's own
teardown; the only way a file survives past that is an *ungraceful* exit, which takes the whole
process down -- and every forked child holding a duplicate of its fds goes with it. The two
conditions are not independent: the one way to get a surviving stale file (the process dies) is
also the one way to guarantee no sibling fork is still around to answer for it. This is a stated
structural reason the real launch path does not share this test's timing, not an unexamined
assumption -- still not investigated further, because nothing above changes that conclusion.

## A real `CLOEXEC` window found while chasing the SIGHUP row -- not its cause, review 579's required correction

Review 578 handed the SIGHUP row the method that just closed row 1 -- triangulate first (free),
then instrument the failing path -- and named a specific hypothesis: a sibling fork holding a
duplicate of the **PTY master** would explain both observations (`exit_status: 1`, then
`exit_status: 0` -- the session leader ending some way other than the signal this test expects).
**That hypothesis is wrong, and review 579 corrected it before this section first went out: a real,
independent `CLOEXEC` defect was found along the way (kept below, in full), but it is not this
row's mechanism, and an inherited master would actually mask the row's real cause, not produce it.**
The row's real mechanism and its fix are in the next section; what follows here is the experiment
that found the (separately real) defect.

### Triangulation: did not reproduce locally, and that is itself informative

Unlike row 1 (which reproduced at ~19% within the first few dozen runs), the SIGHUP row did not
fire once across 70 local runs in three shapes: the test alone (30 runs, `--exact`, necessarily
single-threaded), the full `runtime::terminal::tests::` module alone (30 runs, ~23 tests, most of
which open a real PTY), and the full `tekstide-core` lib suite (10 runs, 1142 tests). This is
consistent with, not evidence against, the hypothesis: this row has only ever fired twice in this
project's entire history, both during an actual release-candidate gate (a different load shape --
`cargo build` and `cargo test` running together, cold caches, a genuinely different machine state
-- than a warm, already-compiled, isolated loop on an otherwise idle box). A rate this low does not
triangulate the cheap way row 1 did; the mechanism needed a direct, synthetic test instead.

### A direct experiment, run twice -- the first run was unsafe and is why this section exists at all

**First attempt (not committed, caused real harm): a background thread spawned `sleep` child
processes in an unbounded, unthrottled loop while the main thread opened 3000 PTYs, each iteration
scanning *every process on the machine's* `/proc/*/fd`.** This machine is shared with other agents'
live sessions (confirmed via `ps aux` showing concurrent `cargo test` runs for unrelated projects
at the time). The spawn loop had no cap and no rate limit; if the (expensive, full-system) scan per
iteration took longer than a single `sleep` child's lifetime, new children could accumulate faster
than old ones exited, with nothing bounding the total. This froze the shared desktop's console
window -- confirmed by the user directly, not inferred. No lasting damage was found afterward (no
stray processes, no OOM-kill entries in the journal, working tree and build both verified clean),
but the experiment itself was the failure, independent of what it would have shown.

**Second attempt, hard-bounded this time**: at most 4 concurrent children (a retained-and-reaped
pool, not an unbounded `Vec`), each living 50ms, the whole experiment capped by **wall clock** (3
seconds) rather than an iteration count that could balloon on a slow machine, and the holder scan
narrowed to *only this experiment's own tracked child pids* -- never a scan of the whole system.
Verified safe before trusting any result from it: process count was 588 before, 587 after: no leak.

**A real methodology bug, caught before trusting the result, the same discipline as review 577's
own two bugs**: the first pass at the corrected experiment used `fstat()` dev/ino matching to prove
a forked child held *the same* pty instance this iteration had just opened -- and found "leaks" on
every single iteration against the same child, which was itself the tell. Checked by hand:
`stat -L /dev/ptmx` on this machine reports a fixed `dev=7 ino=186`, and **every fd ever opened
through `/dev/ptmx` reports that same static identity, regardless of which distinct pty instance it
represents** -- confirmed with a throwaway Python check opening `/dev/ptmx` twice and comparing
`fstat()` results. `TIOCGPTN` gives the real per-instance pty number, but there is no safe way to
run that ioctl against a *different* process's fd from outside it; re-opening
`/proc/<pid>/fd/<n>` for a ptmx-derived fd was also checked by hand and allocates a **new** pty
instance rather than reconnecting to the original one (ptmx's own `open()` method always allocates
afresh, regardless of the path used to reach it -- unlike most file types, for which
`/proc/<pid>/fd/<n>` is a magic symlink that reconnects to the exact same open file).

**Fixed by checking something simpler and just as conclusive**: whether a child's fd
*readlink target* is literally the text `/dev/ptmx` at all. A plain `sleep` binary has no
legitimate reason to ever hold such a descriptor, so any match proves inheritance regardless of
which specific pty instance it is.

### Result: the `CLOEXEC` window is real, confirmed deterministically in 3 seconds

**7,470 leaked-fd observations across 46,143 sampling checks, against 236 total forked children.**
Most forked children that landed in the window kept the leak for their entire lifetime (one child,
pid 1541025, held `/dev/ptmx` open at fd 3 across at least ten consecutive sampling iterations) --
not a microsecond flicker, a persistent handle for as long as that unrelated `sleep` process ran.

**Mechanism, confirmed rather than inferred**: `pty.rs`'s `OpenPty::new` calls `libc::openpty`
(which does not accept `O_CLOEXEC` -- glibc's convenience wrapper has no flag parameter for it),
then applies `FD_CLOEXEC` afterward via a separate `fcntl(F_SETFD, ...)` call once `set_nonblocking`
has also run. Any fork on *any other thread in the process* that lands between `openpty()` returning
and that `fcntl` call completing inherits the not-yet-protected fd; because `CLOEXEC` is a
per-descriptor flag copied at fork time, setting it afterward in *this* process has no effect on a
copy a sibling fork already took before that point -- and because the child typically `exec`s
immediately (as `Command::spawn` does), the inherited copy, lacking `CLOEXEC` at fork time, survives
that `exec` and persists for the rest of the child's life. This is the same defect class as row 1's
own fork/exec window (a gap between resource creation and `CLOEXEC` application, raced by a
concurrent fork), but structurally worse: row 1's window closed in low single-digit milliseconds
once the forked child reached its own `exec`; this one does not close at `exec` at all, because
`CLOEXEC` was never set on the inherited copy to begin with.

### Not the SIGHUP row's mechanism -- but a standing, narrow production concern on its own

`pty.rs`'s own comment on `set_cloexec` already names the defect class this guards against: *"every
child this process ever spawns inherited every PTY master already open at that moment, crossing
RFC-009's terminal security boundary."* That fix closes the case where a PTY is already fully open
(cloexec applied) before a *later* spawn happens. It does not close -- cannot close, with
`openpty()` plus a follow-up `fcntl` -- the case demonstrated here: a spawn on *another thread*,
anywhere in the process, racing into the brief window *while* a PTY is being opened. Tekstide opens
new terminals and spawns other subprocesses (git operations via `runtime::git`, among others)
routinely and concurrently in real use, not only in this synthetic test. A short-lived, unrelated
subprocess (a `git` helper, for instance) landing in this window would inherit a live, functional
handle to a *different* terminal's PTY master, persisting for that subprocess's own lifetime --
exactly the boundary RFC-009 states it closes, not closed in this one specific case.

**Fixed, authorized by review 579 as its own bounded change** (not an RFC -- inside RFC-009's
existing boundary, the same shape as row 1's own fix): `OpenPty::new` now calls `posix_openpt(O_RDWR
| O_NOCTTY | O_CLOEXEC)` for the master, `grantpt`/`unlockpt`/`ptsname_r` to find the slave path,
then `open(slave_name, O_RDWR | O_NOCTTY | O_CLOEXEC)` for the slave -- both flags requested
atomically at their respective `open()` calls, the same reason `std::os::unix::net::
UnixListener::bind` was never susceptible to row 1's own mechanism (`SOCK_CLOEXEC` at its own
`socket()` call). `set_cloexec` is kept, renamed `assert_cloexec`, as a **postcondition check** --
reads the flag back and errors loudly if it is somehow unset, so a future refactor that drops
`O_CLOEXEC` from either `open()` call is caught here rather than silently reopening this window.
`ptsname_r`, not the non-reentrant `ptsname` (which writes into a single process-wide static buffer
with no thread safety at all -- exactly the kind of race concurrent terminal creation, the thing
that surfaced this defect, would hit directly).

**`duplicate_slave` verified still works, not assumed**: `dup(2)` does not copy `FD_CLOEXEC` onto
the new descriptor (POSIX, by design), which is what lets a spawned child's stdin/stdout/stderr/
ctty copies survive its own `exec` even though the slave fd this process holds is itself
close-on-exec. The full `runtime::terminal::` module (69 tests, including every test that spawns a
real shell and reads its output) passes unchanged after the fix.

**A committed, bounded regression test**: `no_concurrently_forked_sibling_inherits_a_pty_master_fd`
-- the same shape as the experiment above, shrunk to the review's own spec (≤4 concurrent children,
a 2s wall-clock cap, a `/proc/<pid>/fd` scan over only its own tracked pids), asserting no child's
fd readlink is `/dev/ptmx`. **Ablated against the pre-fix code** (temporarily restored via `git show
HEAD:...pty.rs`, not hand-edited): failed exactly as expected -- `2866 forked sibling(s) inherited a
PTY master fd across 36272 iterations against 160 forks` -- then the fix was restored and the test
passes again, 0 leaks.

**Other `openpty` callers**: none. `grep -rn "openpty\b"` across the workspace, excluding comments,
finds `pty.rs` as the only call site this codebase has ever had; now migrated entirely. Nothing else
needed touching.

**RFC-028 D5 updated** (the draft, not a slice -- still with the owner, no implementation started):
`openpty` was scoped there as "already portable, only needs its link target confirmed." That was the
wrong property to scope it on -- atomicity of `CLOEXEC`, not mere portability, is what this defect
turned on, and macOS's own `posix_openpt` has not always honoured `O_CLOEXEC` the way Linux's
`/dev/ptmx` open does. PR-028-D now carries a requirement to verify this on the real `macos-latest`
runner with this same regression test's shape, not infer it from the call succeeding.

### Gate

The scratch experiment was never committed -- `git status`/`git diff` confirmed clean both times,
before and after, including during the ablation (restored via `git show`, never hand-edited).
`cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`: clean.
`runtime::terminal::`: 69/69. `rfc_docs_invariants`: 23/23.

## The SIGHUP row's actual mechanism, review 579 -- found in the code, not the experiment

**Verdict: Fix -- in the test's own expectation, not product code.** `request_terminate`
(`termination.rs:86,93`) does, in this exact order:

```rust
self.session_mut(handle)?.close_master();                            // line 86
if self.signal_session_leader(handle, TerminationSignal::Sighup)? {  // line 93
```

This row's own test launches `/bin/cat`, not a shell. `cat`'s blocking read on the PTY slave can
return **EOF** the instant the master closes (a clean read-end-of-file, `cat` exits 0) or **EIO**
(the master closed mid-read, `cat` reports exit 1) -- both **before** the `SIGHUP` on the very next
line is ever sent. The two `TerminationOutcome::Exited` values this row's two occurrences showed
(`exit_status: 0`, then `exit_status: 1`) are exactly those two cases, not a timing coincidence and
not load-dependent -- which is also why no amount of synthetic load, and no amount of local
triangulation, ever reproduced it: the race is deterministic in *shape*, just rare in which side
wins, and nothing about CPU contention moves that.

**The `CLOEXEC` window above would mask this, not cause it.** If a sibling fork held a duplicate of
the master, `close_master()` would not actually hang up the line -- `cat` would see neither EOF nor
`EIO`, and the `SIGHUP` a moment later would win by default, which is the test's *original*
(currently-wrong) expectation. The CLOEXEC hypothesis predicts the opposite of the two failures
actually observed.

**The fix is the test asserting its own documented contract, not a tighter one.**
`request_terminate`'s own doc comment already states it (D3): outcomes are reported *"as `waitpid`
reports... never inferred from which signal was sent or which `TerminationOutcome` the session
leader itself produced."* The test used to assert exactly that forbidden inference -- pinning the
session leader's own exit to `TerminatedBySignal { Sighup }`. Fixed: the deterministic parts of the
contract (`TerminationSignalSent { Sighup }`, `SessionConfirmedEmpty { confirmed: true }`) stay
asserted unconditionally; the session leader's own outcome now accepts any of its three real shapes
-- `TerminatedBySignal { Sighup }`, `Exited { exit_status: 0 }` (EOF), or `Exited { exit_status: 1 }`
(EIO) -- with a comment at the test's own definition explaining why, quoting D3 directly so a future
reader does not re-tighten it back into the same violation.

**Gate**: `linux_runtime_terminates_session_leader_with_sighup` passes. No product code changed for
this row -- `request_terminate`'s own ordering is correct as documented; the test was asserting more
than the contract promises.

## New row, 2026-10-15 -- `no_concurrently_forked_sibling_inherits_a_pty_master_fd` fails in CI, not locally, and the product fix's completeness is genuinely unconfirmed

**This is a new, open flake, logged honestly as unresolved rather than as a settled disposition.**
The regression test PR-028-A's own first push brought to real CI (`ubuntu-latest`, GitHub Actions)
failed the blocking `gate` job twice, on two separate pushes, both after the `CLOEXEC` fix above
had already landed:

```text
# Run 1 (commit 67c8539, "Complete 690c2a5")
5 forked sibling(s) inherited a PTY master fd across 11824 iterations against 156 forks:
["iteration 1682: child pid=13373 fd=\"5\" is /dev/ptmx ...", "iteration 1683: child pid=13373 ...",
 "iteration 2618: child pid=13422 fd=\"4\" ...", "iteration 2619: ...", "iteration 2620: ..."]

# Run 2 (commit c6b8a87, "PR-028-A")
15 forked sibling(s) inherited a PTY master fd across 5506 iterations against 150 forks:
["iteration 668: child pid=13177 fd=\"4\" is /dev/ptmx ...", "iteration 669: ...",
 "iteration 1660: child pid=13191 ...", "iteration 2058: child pid=13199 ...", "iteration 2059: ..."]
```

**Never reproduced locally**: many dozens of runs of this exact test, on this exact machine, both
against the pre-fix and post-fix `pty.rs`, across this entire investigation -- 0 failures outside
CI, ever.

### The pid-reuse hypothesis, raised and disproven

The test's own `known_child_pids` list was never pruned and carried only bare pids (review 580's
own flagged, "not worth a commit" cosmetic note). On a 2-4 vCPU CI runner (confirmed via GitHub's
own published hardware specs for `ubuntu-latest`), scheduling is far tighter than this project's own
32-core development machine, and a kernel reusing a reaped child's pid for an unrelated, legitimately
-running process (e.g. a *different*, concurrently-executing test that also opens a real pty) could
make this test misattribute that process's own entirely legitimate `/dev/ptmx` fd as its own leak.

**Fixed and tested** (commit `719bece`): each spawned child's kernel-assigned start time
(`/proc/<pid>/stat` field 22) is now recorded at spawn and re-verified to still match before any fd
scan is trusted -- a reused pid always carries a different start time. Verified the parsing against
a real `/proc/self/stat` line by hand before trusting it (field 19 after the last `)`, matching
`proc(5)`'s own recommended parsing discipline). Re-ablated against the real pre-fix `pty.rs`:
still fails loudly and correctly (4,561 leaks across 30,704 iterations) -- the identity check does
not hide a real defect, only a reuse-shaped false positive.

**Pushed, and the CI failure recurred anyway** (commit `719bece`'s own CI run), this time *with* the
recorded start time printed in the failure and matching across all three repeated reports in that
run (`child pid=13240 (start_time=69279)`, three consecutive iterations). **This disproves the
pid-reuse theory for this occurrence**: the start time match confirms the leaked fd genuinely
belongs to the process this test itself spawned, not a different process that reused its pid. The
identity-verification fix is a real, independent correctness improvement (keep it), but it is not
what was causing these two failures.

### What remains genuinely unknown

Two possibilities, neither confirmed:

1. **The `posix_openpt`/`O_CLOEXEC` atomicity this fix relies on does not hold in whatever
   environment GitHub's `ubuntu-latest` runner actually provides.** Checked directly, not assumed,
   on this project's own development machine: a standalone C program calling
   `posix_openpt(O_RDWR | O_NOCTTY | O_CLOEXEC)` and reading the flag back via `fcntl(F_GETFD)`
   confirms it is honoured atomically here (glibc 2.44). Whether the same holds on the CI runner's
   own glibc/kernel combination has **not** been verified directly on that runner -- it is the
   single most important unverified claim in this whole investigation, and the next step is to
   verify it there, not to assume parity with this machine.
2. **Some other mechanism, not yet identified**, specific to that runner's environment (lower core
   count changing scheduling in a way that exposes a window this investigation has not found;
   something about how GitHub's runner images configure `/dev/pts` or container/VM isolation;
   something else entirely).

**Quarantined, not Fixed, not Accepted, and not silently weakened**: `#[ignore]`d with a comment
stating plainly that this is unresolved, pointing back here, and warning explicitly against reading
the ignore as confirmation the underlying question is settled. `CHANGELOG.md`'s own entry for the
`CLOEXEC` fix corrected the same way -- it no longer claims the regression test "passes," only that
it passed in local testing, with the CI-only failure and its unresolved status stated plainly.

### Resolved, review 583, same day: the CI-only failure was this test's own premise, not the product fix

**`FD_CLOEXEC` is honoured at `execve`, not at `fork`.** A forked-but-not-yet-`exec`ed child's
`/proc/<pid>/fd` still lists a properly close-on-exec fd, because nothing has reached the point
where the kernel actually acts on the flag yet. This test's own detection checked fd *presence*
alone, which cannot distinguish "leaked" from "about to vanish, exactly as the fix intends" -- only
whether the child has already `exec`ed can tell the two apart. On a 2-4 vCPU CI runner, `fork`-to-
`exec` is slow enough, relative to this test's own scan cadence, that the scan routinely lands
inside that window; on this project's own 32-core development machine it almost never did, which is
why this never reproduced locally across many dozens of runs.

**Fixed**: a new `process_comm` check (`/proc/<pid>/comm`) requires the child to already show
`sleep`'s own name, not this test binary's, before counting any fd match as a leak -- a process
still mid-fork carries its *parent's* comm, not the name of whatever it is about to become. The
pid-reuse identity-verification fix (start time) stays; it is a real, separate correctness property,
orthogonal to this one, not the thing that was wrong.

**Re-ablated with the comm check in place, against the real pre-fix `pty.rs`**: still fails loudly
-- 2,974 leaks across 26,299 iterations, every single one explicitly reported with "already exec'd
into sleep." This does two things at once: proves the new gate did not become vacuous, and
independently re-confirms the original defect really did persist for the child's *whole life*, not
only for a fork-to-exec instant -- the severity claim already in this register and in
`CHANGELOG.md` stands, now re-proven under a corrected test rather than merely restated.

**Un-quarantined** (`#[ignore]` removed) and pushed for the real runner to confirm: a green `gate`
on `ubuntu-latest` with this test live is both the verification neither of us could do locally and
the direct `O_CLOEXEC`-on-the-real-runner check this register asked for two sections up -- if the
fd is gone after `exec` there, the flag genuinely works there. `CHANGELOG.md`'s own wording is held
until that run is confirmed green, then stated once, per review 583's own instruction not to flip it
a third time before the ground has actually stopped moving.

## New row, 2026-10-16 -- `a_transcript_from_an_earlier_run_is_counted_and_purged_after_a_real_restart`, found in CI, not caused by the commit it rode in on

PR-028-D's own gate (commit `e192847`, a doc-comment-only fix on macOS-`cfg`'d code -- no behaviour
change on any platform this runs tests on) failed the blocking `gate` job:

```text
thread 'agent::tests::a_transcript_from_an_earlier_run_is_counted_and_purged_after_a_real_restart'
panicked at crates/tekstide-core/src/agent/tests.rs:916:5:
assertion `left == right` failed
  left: 0
 right: 1
```

`project.purgeable_transcript_count()` read `0` where the test expects `1`, immediately after
`project.load_transcripts_from_disk(&state_root)` -- a real shell launch (`write_input`/
`wait_for_exit` against an actual PTY), a real transcript file write, a simulated restart
(`AppState::default()` plus `restore_recent_projects`), then a real disk read-back. Checked against
this register's own standing suspects before writing it up: no existing row names this test.

**Not caused by the commit it rode in on**: that commit only reworded a comment attached to a
`#[cfg(target_os = "macos")]` `extern "C"` block, which the Linux `gate` job never even compiles --
there is no mechanism by which it could change this test's own, Linux-only behaviour. **Not
reproduced locally**: 5/5 runs of this exact test, same machine, same binary shape. Consistent with
this register's own established pattern for real-process/real-file tests landing on a CI runner
under different timing than local -- a transcript write or a disk read racing the restart sequence
in a way neither this test nor its production code has been shown to handle wrong, only shown to be
sometimes slower than the test's own sequencing assumes.

**Re-verdicted, review 591: `Accept, pending a second occurrence` was wrong -- the mechanism was
identifiable from the code at the time, not actually unknown.** The panic is at line 916; line 915
(`assert_eq!(loaded.loaded, 1)`) passed, meaning the loader found and loaded the transcript and the
very next call said nothing was purgeable. Two methods on one object, single-threaded between them,
disagreeing is not what an environment-timing flake looks like -- it is what a filter looks like:

```rust
// session.rs:2555
fn transcript_is_purgeable(transcript: &Transcript) -> bool {
    !transcript.is_tombstone()
        && !matches!(transcript.origin,
            TranscriptOrigin::FoundOnDisk { writer_held_lock_at_load: true })
}
```

fed by a real probe (`transcript/loading.rs:212,234`: `probe_locks && lock_is_held(&path)`, which is
`file.try_lock().is_err()`). **The CI run loaded the transcript and observed the writer's lock still
held, and `transcript_is_purgeable` excluded it exactly as RFC-050 D3 intends** (`session.rs:1942`:
"purge obeys the lock"). The product behaved correctly; the test's own premise did not hold --
`drop(first)` does not synchronously guarantee the earlier session's transcript writer (a capture
thread) has released its file lock, and nothing enforces that it has by the time the test loads.
`File::try_lock` is per-open-file-description (`flock`), so a still-open writer handle in the same
process is enough to trip it, and that thread's own shutdown can lag `drop` by a few schedules, more
so under a 2-vCPU CI runner's own contention than locally.

**Verdict: Fix -- in the test's own precondition**, the third instance of one pattern in this
register (row 1's staging, the PTY test's fork→exec window, now this): a test asserting a
post-condition whose precondition it never established. Fixed the same shape as row 1's own
`wait_for_path_to_stop_answering`: a bounded wait (`wait_until_transcript_lock_is_released`,
2s budget) for the lock to actually be released before loading, failing loudly naming the budget if
it never is -- not an unconditional wait, so a genuine stuck lock (a real defect) still surfaces as
a clear panic rather than hanging the suite. **Ablated**: a throwaway test holding a real lock on a
real file confirmed the wait panics at the right message once its budget elapses and does not
return early; removed after confirming, never committed.

**Recorded, not acted on**: the identical window exists in real use -- a user closing a project and
immediately reopening it could, briefly, have its transcript read as still-being-written rather than
purgeable. The product already distinguishes those two states deliberately
(`transcripts_still_being_written_count`), so this is behaviour, not a defect; the only open
question is how long the window is in practice, and nobody needs to answer that today.

## Reopened on macOS, 2026-10-19 -- `the_wake_notifier_wakes_when_real_pty_output_arrives`

Row 33 said "still passes. Retired from active tracking." That was true of Linux and is **false on
macOS**, so the row is corrected rather than left standing. Found by the dev team on the real
`macos-measurement` runner at `afdf687`, and disclosed as a correction to their own RFC-028 596
triage assumption that every reader-module failure there was downstream of the containment bug.

**Not an intermittent.** It fails every run on that job, and at a different place from the
2026-09-15 race: `reader/tests.rs:115`, `left: None / right: Some(Exited { exit_status: 0 })`. It
is recorded here only because this document claimed the test was closed.

**Established by reading the code (review 608), not measured on Apple hardware:**

- `wait_for_exit` -> `wait_for_child_outcome` (`termination.rs:173-201`) is a bare
  `child.try_wait()` + 10 ms sleep loop capped at the caller's timeout. No session enumeration is
  involved, so `None` means the direct child was neither reapable nor errored for 5 s.
- The assertion above the failing one *passed*, so the write path, the shell and the reader were
  all live seconds earlier: the `printf` marker reached the slave, ran, and woke the notifier.
- `write_input` (`launch.rs:169-192`) is `write_all` + `flush` with both errors surfaced, and the
  test's `.expect` on the `exit\n` write did not fire, so those bytes were accepted in full.
- `try_wait` did not error either (its own `.expect` message is absent), so it returned `Ok(None)`
  throughout: the child exists, is unreaped, and is not a zombie.

**Falsified before it cost a run:** the reviewer's first hypothesis -- that a macOS master `read()`
after slave close returns `EIO` rather than `Ok(0)`, leaving the reader's EOF arm unfired -- cannot
be the cause. `reader.rs:562-565` ends in `Err(_) => return stop_reading(...)`, the same stop path
as EOF, and the reader does not reap the child in any case.

**Open hypothesis, with the probe named:** the child may be in `SSTOP`, not running. `try_wait`
passes no `WUNTRACED`, so a stopped child is indistinguishable from a live one and yields
`Ok(None)` forever -- exactly the signature above. A shell stopping on its way out (a tty
operation raising `SIGTTOU` during exit, where macOS job control diverges from Linux's) would
explain why `printf` worked and `exit` did not: the first needed the shell to read, the second
needed it to terminate. The existing `is_zombie` (`launch.rs:918-926`) already reads `pbi_status`
via `proc_pidinfo(PROC_PIDTBSDINFO)`, and `libc::SSTOP` exists on Apple in the same enum
(`SSTOP = 4`, `SZOMB = 5`), so one `cfg(test)` diagnostic at the timeout settles it in a single
run.

**Standing constraint on the fix:** widening the 5 s timeout is not a fix and is not accepted as
one unless a probe shows a real exit arriving late *and* how late. A stopped child never arrives.
