---
title: "The leaked-child test flake — cause known since 2026-08-16, still unfixed"
status: "Leak fixed at the two approval call sites 2026-08-20 (request 282). **The second cause — runtime/terminal/launch.rs — FIXED 2026-08-25** (RunningTerminal now has a Drop impl); measured 3,899 leaked shells before, near-zero (the pre-existing baseline) after, across three clean full-suite runs. **A third, distinct cause found while verifying the second fix**: a backgrounded job inside a terminal gets its own process group that neither this Drop nor the production request_terminate path's process-group signal reaches — request_terminate reports Terminated/KilledAfterTimeout while the job survives, orphaned. Confirmed against both paths directly. **The job-escapes-termination defect itself — FIXED 2026-08-26, RFC-043 PR-043-B** (rfcs/handoffs/043-terminal-process-containment/): request_terminate and RunningTerminal::drop now SIGHUP the session leader first, then enumerate and SIGKILL whatever remains in the session (not the process group); measured 0 leaked processes after the 28-pane benchmark that used to leak 28, every time. **The close-confirmation dialog's wording and the audit field's own rename/rewiring — FIXED, RFC-043 PR-043-C**: the dialog now says, before the `Close` click, that anything a terminal started (including a backgrounded job) ends too; `terminal_process_groups_confirmed_empty` is renamed `terminal_session_confirmed_empty` and now reads `SessionConfirmedEmpty`'s own real, session-wide `confirmed` field directly, rather than inferring it from which `TerminationOutcome` variant the leader itself produced — a backgrounded job surviving inside the session (this document's own third cause) is exactly what that inference could not see and this rewiring now can. **What made it pool exhaustion anyway — FIXED 2026-08-26** (rfcs/handoffs/pty-master-fd-inheritance.md): every PTY master was inherited by every child this process ever spawned, so a leaked shell's blocked write into a PTY another process still held open never got EIO and never exited; masters now close on exec, so the same transient orphan now self-terminates within its own script's deadline instead of surviving indefinitely, and /dev/pts occupancy no longer rises across a run at all. This was also a production security defect (RFC-009's terminal boundary crossed by any process able to read/write another project's terminal via the inherited descriptor), not only a test-harness one. The socket flake is separate and also unfixed. **The shared-audit-store cause — FIXED 2026-08-26** (`rfcs/handoffs/audit-store-test-isolation.md`): every test now opens its own store automatically, so the one-shared-SQLite-database contention this document traced row 3 to (and confirmed rows 4 and 6 also transitively reach) can no longer occur in any test in this binary; five consecutive clean full-workspace parallel runs against a fresh `XDG_STATE_HOME`, plus a serial run, confirmed after. A related, separate, and still-open gap found while verifying this: with `XDG_STATE_HOME` unset, the suite's `transcripts/` (and `approval/`) subtrees under the developer's real state root still receive real writes during a run — the audit subtree itself is confirmed untouched, but the same class of defect exists one directory over, out of the audit-store handoff's own scope"
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
| `approval::tests::channel::bind_recovers_from_a_stale_socket_file` | the original, response 213 |
| `approval::tests::coordinator::agent_run_queue_limit_is_enforced_and_only_counts_live_entries` | request 260 |
| `command_approval_family_produces_real_durable_audit_records_through_the_pipeline` | request 276 |
| `shell::tests::a_real_low_risk_proposal_is_received_mirrored_and_stays_queued_without_promoting` | request 296 (2026-08-24) |
| `approval::tests::coordinator::is_still_answerable_reflects_the_real_connection_state` | request 326 (2026-08-25) |
| `shell::tests::change_review_surface_renders_a_real_change_set_from_a_real_agent_run` | request 329 (2026-08-26) — **candidate, not confirmed** |
| `shell::tests::change_review_content_view_build_cost_by_line_count_measurement` | review 338 (2026-08-26) — **not this document's own cause; see below** |

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
