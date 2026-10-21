# Changelog

## Unreleased

### Fixed — review 645 (continued): a fifth site, the same race on the well-behaved peer this time

The very next CI run found `serve_concurrently_accepts_a_second_connection_while_the_first_stalls`
failing under the identical mechanism, on the *well-behaved* peer's own connection this time (it
had passed under this same race in the prior run — confirmed load-dependent per-connection, not
fixed by the earlier four). The "block on a read before exiting" pattern used for the other four
does not fit here: a successful `authenticate_and_read` moves the stream into the returned
`AcceptedProposal` rather than closing it, so there is no "server is done" signal for the client to
wait on. Fixed by removing the race outright instead — the well-behaved peer's thread now returns
its stream rather than dropping it, and the test holds it open until well after the result has
already been obtained and asserted.

### Fixed — review 645: four `approval::tests::channel` tests had their own client-closes-too-early race, closed deterministically rather than tolerated

Review 644's `ConnectionClosed` mapping worked exactly as designed — and, in working, turned an
opaque `Io` failure into an honest one on three tests whose own client thread (or, for one, a real
`python3` process) wrote its frame and exited immediately: the race between "server reads the
frame" and "client's own close" could make the client win, correctly reporting `ConnectionClosed`
but never exercising what each test was actually named to prove (a malformed frame, an oversized
declared length, a wrong token). Tolerating `ConnectionClosed` as an acceptable outcome was
rejected outright: a test that passes on an outcome that never exercised its own property is the
same defect this investigation has already corrected four other times in different disguises
(satisfiable PTY markers, an echoed-command-line byte count, a corpse satisfying `kill(pid, 0)`, a
libproc-invisible process satisfying `session_confirmed_empty`).

Closed deterministically instead, no `sleep` in any of the four: `malformed_frame_is_rejected_not_
partially_parsed` and `oversized_declared_length_is_rejected_before_reading_the_body`'s own sender
threads now set a read timeout and block on one byte after writing, so the thread's own exit
cannot happen before the server has read the frame and dropped its own end (`cross_process_
impersonation_with_wrong_token_is_rejected`'s python script gets the identical shape — a bounded
`recv(1)` before `close()`). `serve_concurrently_refuses_connections_beyond_the_concurrency_cap`'s
own read timeout moved from after a 300ms sleep (by which time the endpoint had already refused
and closed the surplus connections) to inside the connect loop, while every peer this test owns is
still certainly alive — identical semantics, since the timeout only ever governed the later reads.

### Fixed — review 644: a disconnected peer on macOS was reported as an internal `Io` error instead of the verdict it is

Review 642's own `operation` label confirmed the mechanism behind the macOS "oscillating noise
pool": `setsockopt(SO_RCVTIMEO)` on an `AF_UNIX` stream whose peer has already closed returns
`EINVAL` on that platform. Every failing test has a peer that writes and disconnects quickly;
every passing one keeps its peer alive across the call — six of six, not five, once a sixth
(`wrong_token_is_rejected_without_a_dialog`) joined the failing set under this exact mechanism on
a later run.

`authenticate_and_read`'s `set_read_timeout` call now maps that specific macOS condition — matched
on the raw `EINVAL` errno, not `io::ErrorKind::InvalidInput` (which `std` also raises itself for a
zero `Duration`, before any syscall, with no `raw_os_error` attached — the wider match would have
reported a future zero-duration bug to the peer as "connection closed") — to
`ApprovalChannelErrorReason::ConnectionClosed`, the same verdict a zero-byte read already produces
for an identical disconnected peer, rather than `Io`. An inference, not an observation, documented
as such: it fails closed (a wrongly-inferred `ConnectionClosed` only drops a proposal, never grants
one), and the `operation` label is what would make a wrong inference diagnosable if a genuinely
live peer were ever observed producing this errno. Scoped to macOS only.

### Added — review 642: `ApprovalChannelError` now names which call produced an `Io` reason

An uncapped grep for every `approval::tests::channel` test exercising `accept_proposal`/
`serve_concurrently` split the macOS results five-pass/five-fail through the same functions, with
the happy path green — not a release blocker, and the `SO_RCVTIMEO`-duration theory dead twice over
(`silent_peer_triggers_a_read_timeout_rather_than_blocking_forever` passes, proving the timeout is
settable and fires on that exact path). The real question: `ApprovalChannelError { reason: Io,
source: Some(Os { code: 22 }) }` could not say *which* call produced it, so five rounds of guessing
at the same ambiguous error were possible.

`ApprovalChannelError` gained a new `operation: Option<&'static str>` field (`pub fn operation(&self)`),
set at every one of the thirteen `Io`-reason construction sites in `channel.rs` with a short, stable
label (`"set_read_timeout"`, `"accept"`, `"peer_credentials"`, `"read_frame_header"`,
`"read_frame_body"`, `"write_frame_header"`, `"write_frame_body"`, `"bind"`, and others) —
`map_read_error` in particular used to fold both of `read_frame`'s own `read_exact` calls into one
indistinguishable `Io`, which is now two. A production error-reporting fix, not a test probe: the
error type itself could not previously say what it was doing, on any platform, not only this one.

### Found, not yet fixed — review 641: the "oscillating noise pool" was two named causes, not noise, in six rounds of mislabeling

Six `approval::tests::channel` failures held as noise since review 629 are, grouped by panic
location and message, two distinct causes: `UnixStream::set_read_timeout` failing `EINVAL` on this
macOS runner independent of the requested duration (30ms and 2s both trip the identical errno,
falsifying the "sub-millisecond timeval" hypothesis; five of six failures share this one
mechanism, confirmed by reading rather than assumed for the three that don't set their own
timeout), and `dev_fd_magic_path_follows_the_fd_not_a_renamed_pathname`, an already-understood
platform fact (RFC-028 D4) rather than an open failure.

Given `#[ignore]` with its own reason, so it stops being counted as a failure every round (still
runnable via `cargo test --ignored`). The `EINVAL` mechanism itself is not yet fixed — its root
cause is unmeasured, and the duration-value hypothesis that motivated this investigation is now
known to be wrong; recorded in `test-process-leak.md` with a candidate next measurement (comparing
the raw `libc::setsockopt` call against `std`'s own wrapper) rather than guessed at further.

### Fixed — review 640: `process_is_alive` itself is now honest — a capped `grep` had read "two sites" where there were fourteen

Review 639's own `grep` for `process_is_alive`'s callers was piped through `head -12` and read as
complete; it was not. The real, uncapped count was fourteen call sites across four files, most
never examined for this defect. Rather than keep patching individual sites as each new failure
surfaced them (639 fixed two, a third surfaced for free in the very next CI run), the exclusion
moved into `process_is_alive` itself: `kill(pid, 0) == 0 && !is_gone_or_going(pid)`. Every caller —
the two 639 already fixed, the one that failed between rounds, and the eleven nobody had looked at
— is corrected at once, including `runtime::terminal::tests::
signal_candidates_never_signals_a_pid_outside_the_target_session`'s own security-property check:
a false pass there would have hidden containment killing an unrelated stranger process behind a
corpse that still answered `kill(pid, 0)`, the worst possible failure mode in this area. Read and
confirmed load-bearing before this change, not assumed.

The two call sites 639 patched directly, and the setsid-detached-job test's precondition, now call
the plain helper again — the exclusion they were each carrying belongs in one place, not repeated
at every caller. `kill_signal_zero_succeeds_against_an_unreaped_zombie` now proves both halves:
the raw POSIX fact (`kill` alone still succeeds against a zombie) and that the corrected helper is
immune to it (`process_is_alive` itself reads `false`).

`dropping_a_running_terminal_kills_the_real_process_group`'s new failure (test-process-leak.md,
2026-10-21) needed no separate fix — closed by this same change, since its own assertion was
already the correct shape and only the helper underneath it was dishonest.

### Fixed — review 639: the same blind spot ran the other way, in two assertions that pass on exactly what they exist to exclude

Review 638's fix corrected a false *failure*; two other `process_is_alive` callers had the same
defect producing a false *pass* instead, which is worse — it costs no CI cycle and nothing flags
it: `app::tests::HeldProbe::assert_still_holding` ("the held probe must still be running") and
`runtime::terminal::tests::a_real_backgrounded_job_is_dead_after_a_real_close`'s own precondition
("the real backgrounded job must be alive before the close"). Both now require `process_is_alive
(pid) && !is_gone_or_going(pid)`, the same gone-or-departing exclusion review 638 already built.

`is_gone_or_going` widened `pub(super)` → `pub(crate)` (both platform bodies) plus a `terminal::
mod`-level re-export, since `app::tests` is outside `runtime::terminal` entirely — the same
re-export shape `list_all_pids` already needed for the same reason.

Documented, not left for a reader to "complete": the macOS and Linux bodies share a name but not a
semantic — macOS excludes on `SZOMB` *or* `P_WEXIT` (a real, measured, seconds-long exiting-but-
not-corpse window); Linux has no observed analogue of that window, so its body stays `SZOMB`-only
rather than padding in an unverified `X`-state exclusion to look symmetric.

### Fixed — review 638: `kill(pid, 0)` cannot tell a survivor from a corpse, proved deterministically and corrected without waiting for the macOS flake to recur

Proved locally, on Linux, with no runner needed: `kill(pid, 0)` succeeds against an unreaped
zombie — the pid table entry still exists; the signal is simply discarded. New test,
`test_support::tests::kill_signal_zero_succeeds_against_an_unreaped_zombie`, `SIGKILL`s a real
child and deliberately does not reap it (confirming the exit via `waitid(..., WNOWAIT)` without
reaping), then asserts `process_is_alive` still reads true. Clean, 5 of 5 local repeats.

This makes `runtime::terminal::tests::a_real_backgrounded_job_is_dead_after_a_real_close`'s own
assertion — *"an OS-level `kill(pid, 0)` ... must find it gone"* — unsound by construction, on
every platform, not only intermittently on a loaded macOS runner: a `SIGKILL`ed job sitting
unreaped for a moment satisfies that assertion exactly as a genuine containment escape would. The
same confusion response 340 already corrected in the production enumeration, arriving in a test's
own assertion a year later.

Corrected to assert **gone or departing**, the same distinction production's own containment
already draws — `is_gone_or_going` on macOS (`SZOMB` or `P_WEXIT`), a new Linux counterpart of the
identical name (`process_stat`'s own `state == 'Z'`, mirroring `session_id_of`'s existing
same-name-two-bodies pattern) — not a relaxation of the guarantee: a corpse satisfies "closing a
project kills the backgrounded descendant" exactly as hard reaping does. Keeps the review
636/637 instrumentation in place, now gated on the corrected failure condition; any failure that
survives this fix gets a decisive three-way reading instead of an ambiguous one.

Deferred, not forgotten: the macOS-coverage gap in the layout-budget test's own load-average
message (reads empty on macOS, `getloadavg` being Linux-only there) — out of scope for the files
this round touches; folds in whenever `surface/explorer/tests.rs` is next open.

### Found, not yet fixed — review 636: `KERN_PROC_SESSION` is unusable on the real runner, and `kill(pid, 0)` may be the wrong instrument in the test that found it

Measured, not assumed: `report_session_members_via_kern_proc_session`'s own control call (fired
against the backgrounded job while it was certainly alive and certainly a session member, before
the close) returned `ENOENT` from `sysctl(CTL_KERN, KERN_PROC, KERN_PROC_SESSION, session_id)` —
not `ABSENT`, not truncated, an outright failure to answer at all, 3 of 3 firings including one in
an isolated `--nocapture` re-run where the test itself passed. The identical 4-int MIB shape
succeeds against `KERN_PROC_PID` moments earlier in the same function, so MIB construction is not
in question; the selector itself does not resolve on this kernel. Rules out review 634's own
proposed remedy (retiring the bulk-scan-plus-leader-special-case enumeration in favor of one
uniform `KERN_PROC_SESSION` call) before it was ever built.

The `checking pid` trail in the same run showed the backgrounded job's own pid visited by the bulk
scan 51 times across the test's own close sequence (interleaved with other concurrently running
tests' own output, so read as suggestive, not conclusive) — the wrong shape for "never enumerated,"
which weakens review 634's own "no pid to check directly" story for this specific failure. The
likelier explanation: `kill(pid, 0)` succeeds against a zombie (the pid table entry still exists;
the signal is simply discarded), so the test's own assertion cannot tell a real escaped survivor
apart from a `SIGKILL`ed job sitting unreaped for a few more milliseconds under load — the identical
confusion response 340 already corrected in the production enumeration, now possibly present in a
test's own assertion instead. Added `report_pid_p_stat_via_sysctl` (`launch.rs`,
`#[cfg(all(test, target_os = "macos"))]`, reusing the existing offset-verified `sysctl` reader, no
new FFI surface) to read the job's real `p_stat` at the exact moment the assertion is about to fail:
`SZOMB` would mean the test's own instrument is wrong, not the product; anything else keeps the
containment question open. Held for review before pushing, measurement only.

### Found, not yet fixed — review 634: the leader-only fix leaves a backgrounded job's own exit invisible the same way, and there is no pid to check directly for it

Review 633's own run surfaced a second, genuinely new failure in
`a_real_backgrounded_job_is_dead_after_a_real_close`: not its already-tracked, already-fixed
PID-parse-timing cause, but a real OS-level `kill(pid, 0)` finding the job still alive after a real
close. Reviews 625-633's own recovery works only because the session *leader's* pid is known a
priori — it *is* `session_id`, by construction — so a direct check is possible when the bulk
`proc_listallpids` scan omits it. A non-leader member (a backgrounded job) has no such handle: if
the bulk scan omits it, there is no pid to check directly, because nothing else in the enumeration
ever learned it. RFC-043's own target case — a backgrounded job outliving a close — is exactly what
stays open.

Added, as a measurement, not yet a fix: `report_session_members_via_kern_proc_session` (`launch.rs`,
`#[cfg(all(test, target_os = "macos"))]`) asks `sysctl(CTL_KERN, KERN_PROC, KERN_PROC_SESSION,
session_id)` directly — the kernel's own session-scoped view, not assembled from
`proc_listallpids`'s bulk one — and reports every record's `p_pid`/`p_stat`/`p_flag`, flagging
whether the backgrounded job's own pid appears. No `kinfo_proc` declaration needed (review 626's
own deviation, reused): the record stride is measured at runtime from a single-pid
`KERN_PROC_PID` call against this test process's own (always-alive) pid, then the session-wide
reply's own length divided by that stride gives the element count. Wired into
`a_real_backgrounded_job_is_dead_after_a_real_close`, firing only in the branch where the job is
found still alive, with the job's own pid now also named in the assertion message.

Not yet measured on the real runner: held for review before pushing. If `KERN_PROC_SESSION` lists
the job libproc omitted, it justifies retiring the bulk-scan-plus-leader-special-case enumeration in
favor of one uniform `KERN_PROC_SESSION`-based mechanism for both leader and ordinary members. If it
does not, the gap is stranger than the omission theory alone explains.

### Fixed — review 633: the two-predicate split never reached CI — collapsed into one observation with three states before it could race itself

Review 632's own two-predicate design below (`session_leader_still_alive` plus a new, separate
`session_leader_demonstrably_leaving`) was caught before it was ever pushed: it answered its two
questions from **two independent `sysctl` reads**, and a process can cross `P_WEXIT` between them —
the identical defect shape as the `assert!` message race fixed in the same round, one layer up in
the code that fix produced. Collapsed into `SessionLeaderState { Gone, Leaving, Alive }`, read from a
single `kill` + `sysctl` observation: `processes_in_session`'s own survivor check now matches on
`Alive` alone, and `wait_for_session_outcome`'s own "may I stop waiting" check matches on anything
other than `Gone` (`Alive` and `Leaving` both mean "keep waiting," since a leader mid-`proc_exit`
has not finished leaving even though it is also not a survivor to contain). One kernel read, one
decision point in the type, instead of two booleans whose conjunction every caller had to assemble
correctly by hand.

Also renamed `is_zombie` to `is_gone_or_going`: it had read `true` for a `P_WEXIT` process that is
not, in fact, a zombie, since review 632's own extension — the same "a name that asserts something
the code no longer does" defect already caught once this round on the hundred-thousand-entry scan
test.

Not yet measured on the real runner: held for review before pushing, same standing rule as every
macOS-only containment change this round. Report after the push, together: the six
`assert_session_is_empty` guard firings gone, and `OrphanedUnknown` still 0 — either alone is the
previous round repeated.

### Fixed — review 632: one predicate was answering two different questions about the session leader — now each call site asks its own

Review 630's own fix measurably closed the `OrphanedUnknown` miscount (confirmed 1 → 0 on the real
runner) but moved the failure rather than removing it: six `assert_session_is_empty` guard firings
appeared on ordinary, clean closes, every one naming the session's own leader as the lingering
"survivor." Both effects traced to the same root cause — `session_confirmed_empty` was being asked
two genuinely different questions through one predicate: *"may I stop waiting and report an
outcome?"* (`wait_for_session_outcome`, which must keep waiting while the leader is still inside
`proc_exit`) and *"is anything still a survivor to signal?"* (`assert_session_is_empty`/the
`SIGKILL` sweep, for which a process demonstrably leaving is not a survivor, the same response-340
reasoning that already excludes a zombie, one step earlier in the same story).

Split rather than reconciled into one shared answer: `is_zombie`'s own exclusion now covers
`p_stat == SZOMB` **or** `p_flag & P_WEXIT != 0` (a process demonstrably leaving), applied uniformly
to every session member through the existing bulk-scan path. The session leader's own direct check
(`session_leader_still_alive`, added in review 630) stays narrower on purpose — existence and
not-yet-reaped only — because `wait_for_session_outcome` needs that narrower answer specifically: a
leader with `P_WEXIT` set has emphatically not finished leaving, and excluding it there would
reinstate the exact defect just fixed. `processes_in_session`'s own survivor decision for the leader
applies the stricter, `P_WEXIT`-aware exclusion as a second, separate condition
(`session_leader_demonstrably_leaving`) rather than folding it into the narrower function a
different caller depends on. **The distinction that keeps this from drifting back toward the
original bug, stated in the code, not only here: exclusion happens on evidence of departure, never
on absence of evidence** — the defect this whole investigation started from was treating "libproc
cannot see it" as "it is gone"; this treats "the kernel affirmatively says it has begun exiting" as
"it is going," from a call measured to answer correctly in exactly that window, 13 of 13.

Also fixed in the same touch, found by the same investigation: `assert_session_is_empty`'s own
`assert!` evaluated its condition and its diagnostic message as two independent, non-atomic calls to
the same live-process enumeration — a real race that previously could not manifest (the enumeration
essentially never found anything during this window at all) but did once this fix made the
leader visible again, producing a failed assertion whose own message showed an empty survivor list.
Fixed the same way `BoundedTranscriptWriter::create` already does (RFC-050 D3, review 388): one
observation, both the condition and the message derived from it.

Not yet measured on the real runner: held for review before pushing, per this project's own standing
rule for substantial macOS-only containment changes. The next run needs to show the six guard
firings gone, `OrphanedUnknown` still at 0 — both together, since either alone would just be the
previous round repeated — and the three held sets plus row 8 visibly unchanged.

### Fixed — review 630: the macOS session-member check no longer trusts `list_all_pids` to always see the leader — a real `OrphanedUnknown` miscount, not only a wedge

`linux_runtime_terminates_session_leader_with_sighup` surfaced a second, distinct consequence of the
same `proc_exit` blindness (reviews 625-629): `wait_for_session_outcome` reads `session_confirmed_
empty` as its own signal that the session has nothing left to report on, and that predicate is built
on `processes_in_session`'s own bulk `list_all_pids` enumeration — which, measured directly, omits
a pid that is genuinely alive but mid-`proc_exit`. For an ordinary, healthy close, the session leader
*is* `proc_exit`'s own target, so the session could read "confirmed empty" microseconds before the
leader's own real exit status exists, reporting a clean `exit 0` as `OrphanedUnknown` instead.

Fixed at the two places that make this true rather than assumed: `is_zombie` now falls back to
`sysctl(CTL_KERN, KERN_PROC, KERN_PROC_PID)`'s own `p_stat` field when `proc_pidinfo` cannot answer
(the same `proc_find` refusal reviews 627-629 already measured, reused here as a production fallback
rather than only a test-only diagnostic), and `processes_in_session` now checks the session leader's
own pid directly — via `kill`/`sysctl`, never `getsid`, since only the first two were actually
measured to answer correctly during `proc_exit` — whenever the bulk scan never visited it at all,
rather than treating its absence from the enumeration as departure. Both offsets (`p_flag` at 32,
`p_stat` at 36) are the same ones measured off a real hex dump in review 627, now shared between the
`#[cfg(test)]` diagnostic that first measured them and this production code, rather than duplicated.

Not yet measured on the real runner: this push is held for review before going to CI, since it
changes production containment logic, not only test instrumentation. The before/after `OrphanedUnknown`
rate on `linux_runtime_terminates_session_leader_with_sighup` is the confirmation this fix needs.

### Fixed — review 629: the cluster's own remedy, generalised — `wait_for_exit_draining` at the remaining 12 sites, and the production ordering that already avoided this documented as deliberate

Both halves of review 628's experiment came back positive: `P_WEXIT` set and stable over three
samples 100ms apart (not sampling luck), and draining the master before writing the exit command let
the one measured test's shell actually finish exiting. Diagnosis closed: on macOS, a shell that has
begun exiting can wedge inside `proc_exit` for as long as its own PTY master end stays open and
unread.

Checked both production paths that wait for a shell to finish, and both already close the master
first — `request_terminate` (`termination.rs`) and `RunningTerminal::drop` (`launch.rs`) — so nothing
in the shipped product was exposed to this. That ordering was accidental, not designed for this
reason, and is now documented as deliberate at both sites: reordering either call for tidiness would
reintroduce a macOS-only, intermittent hang.

Generalised the one-test fix into `wait_for_exit_draining`, a `#[cfg(test)]` test-support helper
(same shape and choke-point discipline as `wait_for_shell_ready`) that spawns the output reader
before waiting for exit, used at the 12 remaining armed sites across `agent::tests`,
`audit::tests::integration`, and `runtime::terminal::tests`. The one site that proved the mechanism
as a throwaway measurement keeps its own direct `spawn_output_reader` call, unchanged, as the worked
example the helper generalises.

**Correction to review 625's own D5 entry, amended rather than appended to:** review 625 read the
libproc blindness as meaning the containment guard (`assert_session_is_empty`) cannot fail on macOS
at all, and that review 608's "~42 cleared" was therefore the guard going vacuous. Review 628's own
visibility data refutes the unqualified claim: a live, non-exiting PTY child is fully visible to
libproc. The blindness is specific to the `proc_exit` window, not every PTY child, so a real
survivor — the case RFC-043 exists for — is still visible and the guard still fires; only a process
genuinely mid-exit is invisible for the span of that window. Review 608's "42 cleared" is therefore
neither confirmed vacuous nor confirmed real: unmeasured, not proven either way. The standing watch
review 625 added to `release-checklist.md` (every containment guard must be shown to **fail**, on
every supported platform) is not yet discharged — the ablation itself (a live `trap '' HUP` survivor
on macOS, confirming the guard goes red there) is the next open question, now a measurement rather
than an argument.

### Found, not yet fixed — review 628: a hypothesis with its falsifier, and a one-test experiment to test it — `proc_exit`, wedged for lack of a draining reader

Review 627's own visibility experiment settled which half of review 625's open question was true:
both a plain child and a PTY child are fully visible to libproc at a normal point in their life (the
PTY child checked right after `wait_for_shell_ready` confirms it executes), so the earlier blindness
is a *state*, not a property of PTY children as a class.

Review 628 names the state and gives it a falsifier. On Darwin, a process that has called `exit` sets
`P_WEXIT` in `p_flag` and works through `proc_exit` before `p_stat` ever becomes `SZOMB`; `proc_pidinfo`
and `proc_listallpids` both route through `proc_find`, which refuses a process mid-exit, while
`sysctl(CTL_KERN, KERN_PROC, KERN_PROC_PID)` still reports it. Every one of this investigation's own
observations lines up with "inside `proc_exit`": `kill` succeeding, both libproc entry points failing
`ESRCH`, `waitid(WNOWAIT)` reporting no state change, `sysctl`'s own `p_stat = SRUN` (alive, not yet a
zombie), and the bash farewell line itself (627's own finding — exit has begun).

Added, this push: `p_flag` read alongside `p_stat` (same `sysctl` record, offset 32, cross-checked
against the plain child's own hex dump — decodes to `P_EXEC | P_LP64`, exactly what a freshly `exec`'d
64-bit process carries, confirming the offset from a second field independent of `p_pid`'s own match),
with the `P_WEXIT` (`0x2000`) bit checked explicitly; three samples 100ms apart rather than one, since
a single `SRUN` cannot tell a genuinely spinning child from one merely caught runnable by sampling
luck. And a one-test throwaway measurement (`agent::tests::project_session_launches_agent_run_
through_terminal_runtime_and_completes` only, not applied across the cluster): spawn the output reader
— which every test the handshake actually *fixed* already has, and every one of the 13 still-failing
sites lacks — before writing the exit command, testing whether an exiting shell wedges specifically
when nothing is draining its PTY master. If both come back positive, this is a real product finding
(a pane closed while output is pending could wedge the same way), not only a harness one.

Proposes no fix; reports the data, per the reviewer's own standing instruction for this
investigation.

### Found, not yet fixed — review 627: the bare `exit` in every re-sent probe's capture is bash's own farewell — the shell read and ran the real command, then got stuck leaving

Review 625's own reading of the re-sent-probe data (`tek2 returned = false` on all 13 armed sites)
was that input had stopped being honoured. Withdrawn: the capture's own middle fragment — a bare
`exit`, with no trailing status digit, identical across every site regardless of what each site's
own real command was (`exit 0`, `exit 7`, bare `exit`) — is not an echo of anything this crate's own
code writes (checked by grep). It is **interactive bash's own farewell line**, printed once it has
read and begun acting on the real command. So the shell is not ignoring input; it is **stuck inside
its own exit**, after which the re-sent probe's command is echoed but — like everything after it —
never run.

This also reconciles the libproc datum differently than first read: `kill(pid, 0)` succeeding,
`proc_pidinfo`/`proc_listallpids` reporting nothing, and `waitid(WNOWAIT)` reporting no state change
are not three separate contradictions once the child may be mid-exit or already a zombie — they are
three views of one state that needed naming, not three unrelated failures.

Named it with the instrument already built: `p_stat`, read via `sysctl(CTL_KERN, KERN_PROC,
KERN_PROC_PID)` (added in review 626) rather than through libproc, at `report_child_status_at_
timeout`'s own timeout path. The byte offset (36, inside Darwin's `struct extern_proc`) was
**measured off a real hex dump**, not reconstructed from headers — the same instrument review 626
built for exactly this and that had gone unmeasured until a dedicated `--nocapture` CI step (this
round's own fix for a silent `cargo test` default: a test that asserts nothing never prints its own
`eprintln!`s unless it fails) finally surfaced its output. Confirmed twice over before being trusted:
the bytes at offset 0–15 decode as a plausible `timeval` (a real Unix timestamp near this project's
own run dates, a microsecond count under 1,000,000), and the four bytes at offset 40 decode as that
exact child's own real pid — a known, independently-checkable field landing exactly where Darwin's
own stable struct layout says it must.

Proposes no fix for either finding; reports the data, per the reviewer's own standing instruction for
this investigation.

### Fixed — review 626: the re-sent readiness probe was itself a consuming read that could release a deliberately stalled test child; armed explicitly instead

Review 625's own re-sent-probe diagnostic fired at *every* `wait_for_child_outcome` timeout,
including `runtime::terminal::reader::tests::required_local_bounded_marks_capture_failed_stops_
reading_and_stalls_the_child_without_killing_it`, whose own precondition is a child deliberately
stalled on a `write(2)` into a full PTY buffer. A 250ms read drains exactly that buffer, releasing
the stall the test's own assertion is about — review 616/618's rule is wider than "do not reap": a
diagnostic must not consume state the subject under test depends on, and a PTY read is a consuming
operation. Fixed with the same `#[cfg(test)]` thread-local-guard shape
`force_session_enumeration_failure_for_test` already uses: `arm_pty_responsiveness_probe_for_test`,
default off, bound explicitly at the 13 `agent::tests`/`audit::tests::integration`/
`runtime::terminal::tests` sites under investigation for review 625's libproc-blindness finding.
Every other `Ok(None)` timeout — including the stall test above — now sees no behavior change at
all.

Also, review 626's correction to 625's own `sysctl` instruction: **do not** hand-declare `kinfo_proc`
(withdrawn, on the reviewer's own second look) — the probe's question ("does the kernel have a
record of this pid") is answered by the reply's presence and size alone, and a wrong
hand-reconstructed field layout of a large nested struct reads garbage silently on hardware this
project cannot reach, unlike a wrong numeric constant, which simply fails to compile or load. Two
additions instead: the written length reported as its own three-way datum (`failed: errno` / `0
bytes` / `N bytes`, the whole outcome space once the buffer is generously oversized), and a hex dump
of the plain child's own first 64 raw bytes — never parsed as a field, read back only as opaque
bytes — so a future reader who needs `kinfo_proc`'s real layout can read it off real hardware instead
of reconstructing it from headers.

### Found, not yet fixed — review 625: RFC-043's containment guard cannot fail on macOS; review 608's "42 cleared" was the guard going vacuous, not containment working

The readiness handshake (reviews 623/624) isolated a second, distinct failure cleanly: every
surviving `agent::tests`/`audit::tests::integration` member now confirms the shell is alive and
executing (`wait_for_shell_ready` succeeds), writes its own real command, and still times out with
the child alive. Review 623's own libproc diagnostic, extended, found why: `proc_pidinfo` and
`proc_listallpids` are both blind to a live, signalable PTY child (`kill(pid, 0)` succeeds while both
separate libproc query paths report `ESRCH`/omit the pid), uniform across every firing measured, with
`proc_pidinfo(getpid())` succeeding on this same process — ruling out a bad binding or call, not a bad
target.

Followed to where it actually lands: `session_confirmed_empty` is
`processes_in_session(id).is_some_and(|s| s.is_empty())`, built on the same blind enumeration — a
**successful but blind** enumeration returns `Some(vec![])`, which reads as "confirmed empty." So on
macOS, for a session libproc cannot enumerate, `RunningTerminal::drop`'s `SIGKILL` escalation never
runs after the `SIGHUP`, and `assert_session_is_empty` (the `#[cfg(any(test, feature =
"test-support"))]` guard built specifically so a leak would be *red*) cannot fire false. Review 608's
own "~42 containment failures cleared" was this: the enumeration started succeeding and seeing
nobody, not containment beginning to work. Dated correction in RFC-028 D5's own notes.

Added, this push, each proposing no fix: a `#[cfg(test, target_os = "macos")]` experiment
(`libproc_visibility_plain_child_vs_pty_child_and_a_non_libproc_view`) comparing a plain, PTY-less
child against a normal PTY child through the identical three views — `proc_pidinfo`,
`proc_listallpids`, and a new, independent `sysctl(CTL_KERN, KERN_PROC, KERN_PROC_PID, pid)` view
that does not touch libproc at all (an oversized, untyped byte buffer as the `sysctl` output target,
deliberately not a hand-declared `kinfo_proc` — that struct's own real field layout is unverified
here and a wrong one risks silently reading garbage on real hardware this project cannot reach,
where a wrong numeric constant would simply fail to compile or load); and a second diagnostic,
`report_pty_responsiveness_at_timeout`, re-sending the readiness probe at the exact moment
`wait_for_child_outcome` gives up, to tell "the shell is still alive and declined to act on the next
command" from "input stopped being honoured after the first command" — the question the handshake's
own placement made askable for the first time.

A new standing watch, added by the reviewer to `rfcs/handoffs/release-checklist.md`: every
containment guard must be shown to **fail**, on every supported platform, before it is trusted —
a guard only ever seen passing may simply be unable to fail.

### Fixed — review 624: the readiness handshake could itself be eaten by the race it detects; a 25th site; the two copies now share one definition

Review 623's own handshake wrote its probe once and read for up to 5s — but the race under test is
exactly "a write landing before the shell sets its terminal modes is discarded," and the probe is
such a write. A discarded probe would have replaced an informative assertion failure with an
uninformative handshake panic, learning nothing and costing the full timeout. Fixed: both
`wait_for_shell_ready` copies now re-send the probe every poll (`echo` is idempotent) instead of
once, and the first send that survives ends the loop.

Found while fixing it: `runtime::terminal::tests::linux_runtime_reads_output_through_bounded_buffer`
writes after launch too, and its `assert_eq!(output.len(), 256)` is satisfied by the echo of its own
over-256-byte command line whether or not the shell ever ran — the exact debt review 622 named, the
clearest instance of it found yet. Given the handshake, plus a
`contains_subsequence(&output, b"tekstide-output-000-")` alongside the length assertion, which only a
real, expanded first line can satisfy.

The probe command, marker and timeout are now one definition, not two that could silently drift:
`tekstide-core`'s own `test_support` module is `pub`, gated
`#[cfg(any(test, feature = "test-support"))]` rather than bare `#[cfg(test)]` (the same reason this
project's D4 leak guard and `ProjectSession::add_transcript` already use that gate — a consuming
crate's own tests cannot see a dependency's plain `#[cfg(test)]`), and both `wait_for_shell_ready`
copies build their own read loop around its new `SHELL_READINESS_PROBE_COMMAND`/`_MARKER`/`_TIMEOUT`
constants. Every pre-existing `test_support` item needed its own, now-explicit `#[cfg(test)]`
restated, since the module's own broadened gate no longer implies it for free.

The criterion for where the handshake belongs — and, as importantly, where it must not — is now
stated directly in `wait_for_shell_ready`'s own doc comment in both crates: it belongs at any site
that writes after launch and expects the write to be *executed*, and it must not go where a test's
own property under test is silence, an exact byte count, or reader-thread timing, since the
handshake's whole purpose is to guarantee output has already arrived — the negation of what such a
test asserts.

### Fixed — review 623: the macOS PTY-startup race — a readiness handshake before every write-after-launch site, in both crates

Review 622's own 18-test cluster was half right: on macOS `/bin/sh` is `bash`, which reconfigures its
own terminal modes at startup, and a terminal-mode change can discard input the kernel's line
discipline has *already echoed* — which is exactly why these tests saw their own marker and
concluded the command ran. Review 623's own run falsified the stronger claim ("the shell is not
executing anything"): `transcript::reader::tests`' two `capture_real_sgr_output()` callers passed
and failed in the *same run*, so the shell executes *sometimes* — a race, not broken wiring.

Fixed with one shared helper (`runtime::terminal::launch::wait_for_shell_ready`, `tekstide-core`;
a second, `tekstide`-local copy driving `TerminalPane` instead of `TerminalRuntime` directly, for the
one call site on that side of the crate boundary), used at every write-after-launch site in both
crates: `agent::tests` (12 sites), `audit::tests::integration` (6), `transcript::reader::tests` (one
shared helper, both callers), `runtime::terminal::reader::tests` (the wake notifier — inserted before
`spawn_output_reader` takes over the master fd, not after, since the handshake's own direct read
would otherwise race the reader thread for the same bytes), `runtime::terminal::tests` (3), and
`tekstide`'s own row 8. Writes `echo tek$((1+1))\n` and waits for `tek2` — an answer the echo of the
unexpanded command cannot itself produce — before any caller's real input goes in. This also pays
down the test-design debt review 622 named: every assertion in this suite that greps for a marker
which also appears in the written command can be satisfied by the echo alone; the readiness probe is
the first thing in this suite to test something the terminal cannot fake.

Not fixed here, and not yet understood: why `proc_pidinfo` (both `PROC_PIDTBSDINFO` and
`PROC_PIDT_SHORTBSDINFO`) answered `ESRCH` on all 18 of review 623's own firings while `kill(pid, 0)`
said the pid exists — `report_child_status_at_timeout` now also reads `proc_pidinfo(getpid())` on
this process's own pid and whether the child's pid appears in `proc_listallpids`'s own enumeration,
to discriminate "the binding or its usage is unsound" from "libproc genuinely cannot see this one
child" — reported, not resolved, per review 623's own order not to chase this ahead of the handshake.

### Fixed — review 622: `run_directories_in_other_uuid_spellings_are_skipped_and_still_present` degrades and records on a case-insensitive volume, the same class as review 620's explorer test

Two of the fixture's own UUID spellings differ only in case; on a case-folding volume the second
write opens the first spelling's own file instead of creating a sibling, same mechanism as review
620. Fixed the same way: create the lowercase spelling first, probe whether the uppercase spelling's
path already exists before writing it, and build the "other" set conditionally — only the pure-
uppercase variant collides by case alone, so the no-hyphens/braced/URN-prefixed variants are
unaffected and stay in the set on every volume. A runtime probe, not `cfg!(target_os = "macos")`, for
the same reason as before: case-folding is a property of the volume, not the OS.

### Added — review 622: `report_child_status_at_timeout` reads `pbsi_comm`/`pbsi_status` via `PROC_PIDT_SHORTBSDINFO`, to discriminate the 18-test macOS PTY cluster's two suspects in one datum

25 of the remaining macOS failures are not nine findings; 18 of them share one cause, proven from
bytes already captured in a prior run (no new run needed): a `/bin/sh` launched on a PTY produces no
real output, never acts on written input, and never exits — the kernel's own line-discipline echo of
the written command satisfies assertions that look for output, while the two assertions that need a
byte the terminal cannot fake (a real `0x1b` control byte, not literal backslash-`033` text) do not.
`pbsi_comm` (the real process name, up to 16 characters, via `proc_pidinfo(PROC_PIDT_SHORTBSDINFO)`
into `proc_bsdshortinfo`) answers which half: `"sh"` means the child exec'd and the shell itself is
not reading its controlling terminal (`TIOCSCTTY` ordering, fd assignment, or the slave's open
flags); the test binary's own name means `pre_exec` never finished and the child never became a
shell at all. Same discriminator this project already used for `comm == "sleep"` in the fork/exec
`CLOEXEC` work. Proposes nothing; the diagnostic only adds a read, nothing it reads is consumed, and
no fix for the cluster itself is attempted here — per review 622, the datum comes before the diff.

### Fixed — review 621: R6's own guarantee ("only the repository's own effective configuration is ever read") was false on macOS; `GIT_CONFIG_NOSYSTEM=1` closes it, pinned with a test

Measured, not inferred: `GIT_CONFIG_SYSTEM=/dev/null` (R6, review 407) relocates where git looks
for system-level config, but Apple's own git build (the Xcode command-line tools, which
`PATH=/usr/bin:/bin` always resolves to on macOS) reads its own bundled `gitconfig` as a distinct,
earlier layer that relocation does not reach. `git_gate_probe_origin_for_key`'s own read-only probe
named the exact mechanism: `credential.helper`, from `/Applications/Xcode_*.app/Contents/Developer/
usr/share/git-core/gitconfig`, on every one of 36 firings across `runtime::git::tests`'s 24 macOS
failures. Review 407's own defect, through a new door: a configuration layer outside the repository
withholds the content answer for every repository on the affected platform, forever — the same
blast radius `user.signingkey` once had on Linux, closed by the fix that did not anticipate this
second door. `GIT_CONFIG_NOSYSTEM=1` (git's own absolute switch, suppressing the system layer
outright rather than naming a replacement path for it) is now set alongside the two existing
overrides, applied last with them. The module's own R6 doc comment corrected to say which mechanism
closes the gap on which platform, rather than one claim covering both. Pinned with a new test
(`the_three_config_isolation_overrides_are_set_on_every_spawned_command`) asserting all three
overrides directly on the spawned `Command`'s own environment — before this, removing or typoing
any of the three failed nothing.

### Fixed — review 620: `scanner_lists_folders_first_and_compares_names_without_case` degrades and records on a case-insensitive volume, instead of silently testing one fewer row

APFS is case-insensitive (case-preserving) by default, where ext4 is case-sensitive.
`TestSandbox::create_file` is a truncating write, not an exclusive create, so writing `a.txt` after
`A.txt` on such a volume opens the same file by case-folded lookup instead of creating a sibling —
confirmed directly (the path already exists, under that exact spelling, before the write that
supposedly creates it, if and only if the volume folds case), not assumed. Case-insensitive
coverage loses exactly one row: the tie-break where two names share a lowercase form and the exact
bytes decide which sorts first. The two properties the test is actually named for — folders first,
names compared without case — both still hold on such a volume; the test now detects the volume's
own behavior and asserts the platform-correct list, naming the missing row rather than silently
testing fewer cases. No product change: the comparator's own tie-break rule stays exactly as it is,
since a macOS user cannot hold both spellings in one directory to exercise it in the first place.

### Fixed — review 620: a read-only probe inside `vet_configuration` names which config key and origin triggered `AcceptedBranchOnly`, for `runtime::git::tests`'s 24 macOS failures

`GIT_CONFIG_SYSTEM`/`GIT_CONFIG_GLOBAL` are already hardcoded to `/dev/null` in `spawn_git_command`
(R6), applied last so nothing can override them — so the hypothesis that a stock macOS git carries
unrecognized system-level config keys is not yet confirmed, only plausible. `git_gate_probe_origin_
for_key`, `#[cfg(test)]`-only, re-runs `git config --list --show-origin --null` (a second, read-only
subprocess; the first call never requests origins) and reports the real origin git names for the
triggering key, the moment `vet_configuration` is about to downgrade to `AcceptedBranchOnly`.
Deliberately not fixed by forwarding `GIT_CONFIG_NOSYSTEM`/`GIT_CONFIG_SYSTEM=/dev/null` to the test
fixtures yet: if a stock macOS install does carry such keys, every macOS user gets branch-only
answers for reasons they would never see, which is a release-relevant product question this probe
exists to answer with data rather than silence by making the suite look healthy. Proposes nothing;
reports the key and its origin.

### Fixed — review 619: one class, three faces — ten more test helpers hit `SymlinkAmbiguous`, `SymlinkEscape`, or a bare `starts_with`, the same uncanonicalized-root cause under three different error shapes

507 → 86 once review 618's own fix landed, but the residue was further from the predicted ~81 than
expected: `grep -c "SymlinkAmbiguous"` found only 5 remaining matches, yet 23 more failures carried
the identical root cause under a different name, `FileAccessError { .. reason: SymlinkEscape }` —
a file textually under its own root, refused because the root's own recorded "canonical" path was
never actually canonicalized, so `resolve_existing`'s real canonicalization of the file no longer
`starts_with` it. A third face, a bare `path.starts_with(&root)` assertion comparing the two
spellings directly, found in `audit::tests::path`. Ten more independently-built roots fixed the same
way: eight copies of a bare `fn test_root(name) -> PathBuf` (`runtime::terminal::tests`,
`recovery::tests`, `project::tests::content`, `agent::tests`, `project_lock::tests`,
`runtime::terminal::reader::tests`, `project_lock::attention::tests`, `config::tests::profile`),
two standalone functions in `ignore_tests.rs` separate from its own `Repo` struct already fixed,
`explorer_tree::tests::wide_tree`'s own inline construction, and `audit::tests::path`'s own
`TestDirs`. The tell, recorded for next time: any macOS-only failure naming a `/tmp` path together
with a containment or equality check is this class until proven otherwise, regardless of which of
the three error shapes it surfaces as.

### Fixed — review 617/618: the `SymlinkAmbiguous` class `watch_test_root()` was fixed for reaches 401 of 507 failures across both crates, not a handful in one

Measured, not estimated: 122 of `tekstide-core`'s 220 failures and **279 of `tekstide`'s 287** carry
this exact shape — 79% of everything still red on macOS. `TestSandbox` (six independent copies, in
`tekstide-core`'s `app`, `content`, `project::recent`, `project::root`, `project::root::explorer`,
and `project::tests::change_detection`, plus a seventh in `tekstide`'s own `surface::editor::tests`),
`project::diff::tests`'s own `Sandbox` (`tekstide-core`), `project::root::explorer::ignore_tests`'s
own `Repo` (`tekstide-core`), `HostileFixture::build` itself, and `fresh_project_dir` (independently
duplicated in `tekstide`'s `shell::tests` and `tests` modules, the single dominant cause of the
`tekstide` crate's own 279) all built a project root from `std::env::temp_dir()` without
canonicalizing it first. On macOS, `$TMPDIR` is reached through a symlink (`/tmp` ->
`/private/tmp`, or `$TMPDIR` itself under `/var` -> `/private/var`), so every caller that validates
a root under one of these helpers with `SymlinkPolicy::FailClosed` hit `SymlinkAmbiguous` — not a
product defect (`FailClosed`'s own containment check is correct and platform-independent; macOS
only changes which paths reach a symlink a user's own project could reach too, on either platform,
the ordinary case being `~/dev -> /mnt/data/dev`). Each helper now canonicalizes its root once,
right after creating it, the same fix already applied to `watch_test_root()`.
`ProjectRootValidator`/`SymlinkPolicy` themselves are unchanged. Nine near-identical sandbox
builders across two crates is its own, separately recorded observation (not acted on this round):
an input to RFC-074's own boundary work, not a task for this one.

### Fixed — review 617/618: the hostile fixture's `depth` row cannot be built on macOS at `DEPTH_LEVELS` 1500; `ENAMETOOLONG`, not the `unreadable` row

1500 single-byte directory levels is ~3 KB of path, fitting Linux's 4096-byte `PATH_MAX` but not
Darwin's 1024-byte one — confirmed on the real runner (`Os { code: 63, kind: InvalidFilename,
message: "File name too long" }`, Darwin's own `ENAMETOOLONG`; Linux's is `36`), 24 tests wide, all
of them already known to fail from item 4's own `EILSEQ` finding and simply advancing to the next
obstacle once that one cleared. Not a product gap: `FileExplorerScanner::scan_directory` and
`ProjectFileAccessPolicy::resolve_existing` both already degrade *any* `read_dir`/`canonicalize`
failure to a typed refusal via `map_err(|_| ...)`, discarding the specific `io::Error` rather than
matching it by errno — confirmed by reading both call sites, so a real deep project already refuses
cleanly on macOS rather than panicking. `DEPTH_LEVELS` is `200` there (computed with margin against
the real runner's own observed path length, not guessed), `1500` unchanged on Linux; the two test
files that sample specific depths now filter their sample points against whichever `DEPTH_LEVELS`
applies, rather than assuming the Linux figure. **Named, not left implicit**: 1500 levels was
chosen to guarantee a *recursive* walker would overflow its stack; 200 does not stress any stack, so
on macOS this test still proves "one level per scan, nothing recurses" but is no longer the stack
canary it is on Linux — recorded in the test's own doc comment.

### Fixed — review 617/618: the `SSTOP` probe is extended with two more syscalls, after its first run falsified the hypothesis and raised a question about `is_zombie` itself

The first run: 12 firings, all `proc_pidinfo` returning `ESRCH` — the pid does not exist by
libproc's own account, while `try_wait` had just reported neither exited nor errored, which cannot
both be true of a healthy process. `is_zombie` (`launch.rs`) rests on the same `proc_pidinfo` call;
if libproc genuinely cannot see an unreaped child's own existence on macOS, `is_zombie` can never
return `true` there, and the containment enumeration would treat an unreaped child as a live session
member. `report_child_status_at_timeout` now also reports `kill(pid, 0)` (the kernel's own
existence/permission check, independent of libproc) and `waitid(P_PID, pid, &info, WNOHANG |
WNOWAIT | WEXITED | WSTOPPED)` (what `try_wait` is really seeing, including a stopped child, which
`try_wait` cannot report) at the same instant — reports the three results; proposes nothing.
**`WNOWAIT`, not a first-draft `waitpid`**: a plain `waitpid(..., WNOHANG | WUNTRACED)` reaps the
child on a successful, non-zero return, and `RunningTerminal::drop` derives the session it
`SIGKILL`s from this same pid (`setsid()` in `pre_exec` makes it the session id) — reaping here
would free the pid for the kernel to recycle before that destructor runs, and a recycled pid handed
to the containment sweep enumerates and kills an unrelated session. `WNOWAIT` reports the same
state, including a stopped child, without the reap. Verified against Apple's real libc with a
dependency-free standalone crate and `cargo check --target aarch64-apple-darwin/
x86_64-apple-darwin --tests`, in all three shapes that matter (non-test Apple, test-profile Apple —
the runner's own shape, test-profile Linux); only the kernel's actual runtime answer remains open.

### Fixed — RFC-028 review 596's triage, item 5: `project::watch`'s tests hardcoded `/dev/shm`, which does not exist on macOS

Not a product defect, and review 602's own `kqueue`-opens-file-descriptors-where-`inotify`-doesn't
hypothesis for the `PermissionDenied` failures was not needed to explain them: this module's test
fixtures (14 `/dev/shm` literals across the file, 12 replaced; the other two are an operator's own
documented manual setup for an `#[ignore]`d measurement) hardcoded `/dev/shm` directly, a Linux
tmpfs mount with no macOS equivalent at all. The real runner's own log names the exact mechanism:
`mkdir("/dev/shm")` fails `EACCES`, not `ENOENT` — `/dev` exists (devfs populates it with device
nodes only) and simply refuses a new directory there, rather than a missing parent. 11 of this
file's tests failed this way, every one of them at setup, before any kernel watch was placed. One
shared helper, `watch_test_root()`, replaces the literal: `/dev/shm` unchanged on Linux (tmpfs
speed and a short path both already load-bearing reasons recorded elsewhere in this project),
`std::env::temp_dir()`, canonicalized, on every other platform.

Canonicalizing is not decoration: a second, Linux-runnable probe (forcing the helper through a
symlink, mirroring macOS's own `$TMPDIR` reached via `/var` -> `/private/var`) found that an
uncanonicalized root makes `ProjectRootValidator::validate(..., SymlinkPolicy::FailClosed)` refuse
every project root this file builds under it with `SymlinkAmbiguous` — its own selected and
canonical paths disagree, the exact ambiguity that policy exists to catch. Four tests hit this
before the fix added canonicalization; none do after. The same probe, run without symlinking the
root, also confirmed all 27 tests pass identically through the real `inotify` backend against an
ordinary, non-tmpfs directory — further evidence against the `kqueue`/`inotify` fd-semantics
hypothesis: the watch mechanism itself does not depend on which kind of filesystem backs the
watched directory.

### Fixed — two macOS test gaps from RFC-028 review 596's triage, item 6: `setsid(1)` and `/dev/full` are both Linux-only

Neither is a product defect; both are test fixtures that assumed a Linux-only command-line tool or
device exists.

- **`setsid(1)` does not exist on macOS at all** (it is a `util-linux` command; only the
  `setsid(2)` syscall is POSIX). `a_job_that_leaves_the_session_via_setsid_survives_a_real_close`
  now detaches via `perl -e 'fork and exit; use POSIX; POSIX::setsid() ... ; exec @ARGV'` instead
  — `perl`'s `POSIX::setsid()` reaches the same syscall directly and ships on both platforms by
  default (confirmed present on the real macOS CI runner image, an image-provided tool rather than
  an OS guarantee, so the test now checks for it by name first, rather than inferring it from a
  timeout). The same fork-before-`setsid` shape as `setsid --fork` (avoiding `EPERM` on a process
  that already leads its own group, from bash's own job-control backgrounding), with a failure
  printed as a parseable marker — and, review 614: `read_until_parsed`'s own timeout now returns
  what it actually captured (`Err(Vec<u8>)`, not `None`), so that marker (or any other unexpected
  output) reaches the panic message instead of being silently discarded, which is the whole reason
  it was worth adding.
- **`/dev/full` does not exist on macOS either.** The two tests exercising a genuinely-unwritable
  transcript now use a platform-specific mechanism behind one shared helper
  (`make_path_always_fail_writes`): Linux keeps the existing symlink to `/dev/full`; macOS creates
  a FIFO at the transcript path, opens its read end non-blocking (letting the real writer's own
  blocking `open()` for write complete immediately, mirroring `/dev/full`'s "open always
  succeeds"), and the caller drops that reader right after the real `open()` has happened — every
  write after that fails with `EPIPE`, the identical `io::Error` shape the writer already maps
  uniformly regardless of which errno produced it. Verified directly (not assumed) with a
  standalone probe reproducing `Os { code: 32, kind: BrokenPipe }` on this exact sequence, and by
  temporarily exercising the macOS code path itself on Linux (both are POSIX-portable mechanisms)
  before this landed.

### Fixed — a directory explorer scan was O(entries in the directory), not O(cap), past ~10,000 hidden entries

Review 612: `OMITTED_COUNT_LIMIT` (`project/root/explorer.rs`, RFC-052 PR-052-B) bounded the number
of omitted entries the scanner would *stat and count exactly*, not the number it would *walk* to
produce that count — a directory with more hidden entries than the limit (`1_000_000`) still had
every one of them pulled out of `read_dir` to be counted, even though none of them became a node.
A 100,000-entry directory's capped scan cost ~24 ms instead of ~1 ms as a result, found only once
the RFC-052 measurement crate's own month-stale `Cargo.lock` was refreshed and its D8 timing check
regressed against the rebuilt product. Lowered `OMITTED_COUNT_LIMIT` to `10_000` (worst case ~2.5
ms, measured at ~0.24 µs per entry pulled from `read_dir`); a directory with more hidden entries
than that now reports "at least `N`" instead of an exact count, using lower-bound machinery
(`omitted_is_lower_bound`) the scanner already had. The test that should have caught this
(`a_hundred_thousand_entry_directory_is_a_bounded_scan_not_a_walk_of_all_of_it`) asserted a flat,
2-second wall-clock budget — 80x too loose to see a 24x regression — and is now a machine-
independent paired ratio instead, timing a 300-entry and a 100,000-entry scan in the same run and
asserting the ratio between them stays under 15x. Dated corrections in `rfcs/done/052-*.md` and
`rfcs/handoffs/052-file-explorer/qa-evidence.md`, naming PR-052-B as the origin.

### Fixed — 28 `EILSEQ` panics on APFS, at five locations; the fixture degrades and records, four single-purpose tests are `ignore`d

RFC-028 review 596/609, item 4: five places in the test suite built a filename whose bytes are not
valid UTF-8 and unconditionally `.unwrap()`ed creating it, to exercise lossy-display handling.
APFS validates UTF-8 in filenames where ext4 accepts arbitrary bytes, so every one of those writes
panicked with `EILSEQ` on macOS — a filesystem guarantee, not a product regression, but a panic
instead of a told-apart failure. The real runner's log (28 panics, grouped by location) showed two
different shapes that need two different fixes:

- **`HostileFixture::build`** (24 of the 28, every test that shares the fixture): the hostile name
  is incidental to what these tests check — bidi overrides, newlines, symlink escapes, unreadable
  directories. The fixture now tolerates exactly this refusal and records it on a new
  `escaping_refused` field rather than silently building a fixture with fewer hostile rows: the
  three call sites that asserted an exact row count now subtract `escaping_refused.len()`, and a
  new test (`the_non_utf8_escaping_row_is_recorded_as_refused_on_a_filesystem_that_cannot_hold_it`)
  names the gap permanently so macOS's coverage here cannot look equal to Linux's without being so.
- **Four standalone tests** (`explorer/tests.rs::scanner_preserves_non_utf8_filename_identity_in_relative_path`,
  `ignore_tests.rs::a_non_utf8_name_is_asked_as_its_bytes_and_the_answer_lands_on_its_row`,
  `git/tests.rs::a_non_utf8_path_is_skipped_from_the_map_but_still_counted`,
  `git/tests.rs::a_non_utf8_filename_survives_the_round_trip`): the non-UTF-8 name *is* the
  property under test, each one's entire reason to exist. Tolerating the write here would leave a
  test that passes while checking nothing — the same shape already corrected three times in this
  project — so these four are `#[cfg_attr(target_os = "macos", ignore = "...")]` instead, the
  reason string naming the fixture's own test as where the underlying platform fact is still
  asserted.

Checked by raw errno rather than `libc` (the fixture is standard-library only on purpose, shared
with the standalone `rfcs/handoffs/052-file-explorer/measurement` crate) or `std::io::ErrorKind`
(confirmed on the real runner: `io::Error::kind()` reports `Uncategorized` for this failure, so an
`ErrorKind` match would never fire). `92` and the `Uncategorized` kind are both observed from the
runner's own log, not inferred from Darwin's ABI docs; the `cfg!(target_os = "macos")` guard is
load-bearing since `92` is `ENOPROTOOPT` on Linux, where `EILSEQ` is `84`.

### Changed — the attention socket's own directory moves on macOS, same as the approval channel's

RFC-028 D4, review 603: `project_lock::attention`'s socket has the same `sun_path`-length ceiling
the approval channel hit first, for the same reason — a real `ProjectId`-based path under a state
root as long as `~/Library/Application Support/tekstide/project-locks` does not fit macOS's
103-byte usable `sun_path`. On macOS only, the attention socket's own directory now lives under
`std::env::temp_dir()` (`$TMPDIR/tekstide-attention`, a sibling of the approval channel's own
`$TMPDIR/tekstide` anchor, not nested inside it — the two stay as independent on macOS as their
duplicated `max_socket_path_len` already keeps them everywhere else) instead of under the state
root. The general rule this and the approval channel now both follow: every `AF_UNIX` path this
product binds derives from a short per-user anchor, never from `state_root`, with a headroom test
on each platform that binds it.

### Fixed — the "already open" board notice no longer claims a knock was sent

RFC-028 review 603: `project-board-open-blocked`'s second sentence ("It has been asked for your
attention") described a knock that nothing in the real open path — any of the three GUI call
sites, or the CLI path — ever actually sends; only a standalone diagnostic probe binary calls
`project_lock::attention::request_attention`. True on neither platform today, not only on macOS
(where the attention socket cannot yet bind at all). Removed rather than qualified: making the
claim conditional on an outcome that does not yet exist would still say "could not ask" for the
wrong reason. The board now says only "This project is already open in another Tekstide window."
Wiring the knock itself and restoring a true second sentence is reserved as RFC-075, outside this
RFC's scope.

### Fixed — `project_lock::attention`'s own `max_socket_path_len` had the same macOS off-by-one

A deliberate, documented second copy of `approval::channel`'s `max_socket_path_len` (RFC-058
D10's own trade: a second, independent copy of three lines is cheaper than threading a dependency
between two otherwise-unrelated modules) — which meant it carried the exact same formula, and
therefore the exact same bug, once the original was found and fixed. Two independently-maintained
copies of one computation is a defect the day someone edits one without the other; fixed in the
same way and the same commit as the original, rather than left to diverge further.

### Changed — the approval channel's socket directory moves on macOS; `AgentRunId` prefix dropped from socket filenames

RFC-028 D4, review 600: `sockaddr_un`'s usable `sun_path` is 103 bytes on macOS (confirmed by
reading the field's own array length directly rather than deriving it — see the `Fixed` entry for
`max_socket_path_len` below), and no bind-time mechanism shortens the path an adapter must later
`connect()` to, since that path is never resolved relative to anything. A real `AgentRunId`-based
socket path under a state root as long as `~/Library/Application Support/tekstide/approval` does
not fit. On macOS only, the approval socket's own directory now lives under `std::env::temp_dir()`
(`$TMPDIR/tekstide`) instead of under the state root — the Apple-sanctioned, per-user, short
location for exactly this kind of non-durable IPC. D3's own `Application Support` decision for
*durable* state is unaffected; only the socket's own directory moves. On both platforms, socket
filenames drop `AgentRunId`'s own `"agent-run-"` prefix (meaningful in a log line, pure overhead in
a path this crate already knows holds nothing else), leaving `<uuid>.sock`.
`ApprovalChannelPathErrorReason` gains `ChannelAnchorUnavailable` (macOS only) for a failure
resolving or pinning that directory. **This is a breaking change for anything matching
`tekstide-core`'s `ApprovalChannelPathErrorReason` exhaustively.**

### Fixed — `max_socket_path_len` was one byte too permissive on macOS

The previous formula (`size_of::<sockaddr_un>() - size_of::<sa_family_t>()`) assumed no header
field precedes `sun_family` — true on Linux and false on macOS/BSD's own extra leading `sun_len`
byte, silently reporting 104 as the max where the real limit is 103. A 104-byte path passed this
check and would have failed only at the real `bind()` call. Now reads `sun_path`'s own array
length directly, correct by construction on any platform's `sockaddr_un` regardless of what
precedes it.

### Added — `ApprovalChannelErrorReason` gains `ThreadChdirUnavailable`

RFC-028 D4: the approval channel's own TOCTOU-safe bind path now has a macOS mechanism, not only
Linux's `/proc/self/fd` magic path. `/dev/fd` was tested on the real runner and confirmed not
traversable the way `/proc/self/fd` is, so macOS binds through `pthread_fchdir_np` (the calling
thread's own current directory, not the whole process's) plus a relative path instead.
`ThreadChdirUnavailable` is macOS's own distinguished reason for that call failing, parallel to
`ProcMagicPathUnavailable` on Linux. **This is a breaking change for anything matching
`tekstide-core`'s `ApprovalChannelErrorReason` exhaustively** — a `match` with no wildcard arm
over this enum no longer compiles without handling the new variant.

### Fixed — a subprocess could inherit another terminal's PTY master

Opening a terminal called `openpty()` and then applied close-on-exec to the two descriptors it
returned with a separate `fcntl`. Between those two steps the descriptors existed without the flag,
so a process spawned on any other thread in that window — a `git` helper, an AI CLI agent — took a
copy whose close-on-exec was never set and could not be: the flag is copied at fork time, and
setting it afterwards cannot reach a copy already taken. That copy survived the child's own `exec`
and lived as long as the child did, giving it a working handle to a *different* terminal's PTY
master: readable, writable, and outside the boundary RFC-009 states it closes.

Fixed by requesting close-on-exec atomically at each `open()` instead (`posix_openpt` for the
master, `open` for the slave), so no window exists to race into; the previous `fcntl` is kept only
as a read-back that refuses to open a terminal at all if the flag somehow is not set. A bounded
regression test forks siblings while opening PTYs and asserts none of them holds `/dev/ptmx`; it
fails against the previous code (measured: 1,512 inherited descriptors across 38,199 iterations)
and passed against this one in local testing across many dozens of runs.

**Status, verified rather than assumed**: the regression test's own apparent CI-only failure was
traced to the test, not the fix — `FD_CLOEXEC` is honoured at `execve`, not at `fork`, so a
forked-but-not-yet-exec'd child's `/proc/<pid>/fd` still lists a properly close-on-exec descriptor,
and the test's original fd-presence check could not tell that apart from a real leak. Corrected to
require the child to have already exec'd before counting a match; re-ablated against the real
pre-fix code to confirm the corrected check still catches the original defect (it does, every
report past its own exec), then run live on this project's own CI (`ubuntu-latest`) rather than only
locally — green. The fix is confirmed atomic on that runner, not only on this project's own
development machine.

**On how likely this was**: reaching it needs a spawn to land inside a window microseconds wide, and
the figures above come from an experiment built to hit that window deliberately — they are not a
rate anyone would see in normal use. No report of it exists; it was found while investigating an
unrelated intermittent test failure.

### Changed — `LinuxTerminalRuntime` renamed to `TerminalRuntime`

**This is a breaking change for any downstream crate naming `tekstide_core::runtime::terminal::
LinuxTerminalRuntime`** — the type no longer exists under that name. Renamed as part of RFC-028's
own platform-dispatch convention (D3): this type's internals are about to gain macOS-specific
mechanisms alongside the existing Linux ones, and a type named `Linux…` would have been exactly the
label-that-means-nothing that convention exists to abolish, one layer below the `cfg` split itself.
Not renamed to `UnixTerminalRuntime` either — that would overclaim the BSDs and Solaris, which this
project does not support. No behavior change; every call site updated in the same slice.

### Added — `ApprovalChannelErrorReason` gains two variants

`BindTargetNotASocket` and `BindTargetAlreadyListening`, distinguishing the two arms of a stale
approval-socket recovery that previously both reported a bare `Io` with no further detail — found
while re-verdicting a long-standing intermittent test failure whose panic carried no evidence of
which arm it came from. **This is a breaking change for anything matching `tekstide-core`'s
`ApprovalChannelErrorReason` exhaustively**: a `match` with no wildcard arm over this enum no
longer compiles without handling the two new variants. The enum is deliberately not
`#[non_exhaustive]` — see the type's own doc comment for why, which is a decision rather than an
oversight.

## 0.37.0 - Documentation, CI, and Release Automation

Status: **released 2026-10-14.** RFC-029's six slices and its Whole-RFC checklist are
all implemented, gated and closed, with one exception named below. This entry was written
incrementally as slices closed, then re-read against the finished set at the candidate; see
`rfcs/handoffs/029-ci-and-distribution/` for the full handoff pack.

**Installing no longer needs a Rust toolchain.** `cargo install tekstide` was the only documented
way to get this program; this release adds a prebuilt `x86_64` Linux binary, a one-liner installer,
and an Arch Linux `PKGBUILD`, and rewrites `README.md`'s *Quick Start* and the book's *Install* page
to lead with the one-liner. `cargo install` and a four-line (now five-line) manually-verified form
stay documented beside it as **equivalents**, never as the safer choice, since every form here
fetches code from the same server. **The prebuilt binary is published from this release onward**:
a reader reaching these pages before `0.37.0`'s own tag exists will see the installer fail with a
clean 404, not a broken program.

**This project's first build/test CI job, shown failing before it was trusted.** `ci.yml` runs
`fmt`, `clippy --workspace --all-targets --all-features -D warnings`, `cargo test --doc --workspace`
and the full workspace suite with `--all-targets --no-fail-fast` against a short fixed `TMPDIR`,
asserting zero fixture entries left. Proven on a real, deliberately broken commit — red at exactly
the `fmt` step, then green again after a revert — on a throwaway PR, not reasoned about from reading
the YAML. **It is advisory, not yet a required check**: branch protection is a repository setting
the owner has not configured, left to them rather than decided unilaterally here. **Its own second
half is unbuilt**: a quarantined test is meant to run in a separate job that is read, not only
skipped, and no slice built that job — three tests are `#[ignore]`d with a reason and run only when
asked for by name, found at review 553 and reserved as RFC-072.

**Every published artifact carries a published SHA-256**, verified live end to end — built,
staged, uploaded to a throwaway draft release, downloaded back, and checked against its own
checksum. **Measured, not assumed, to be unreproducible**: two independent `--locked` builds of the
same source produced two different binaries, byte-for-byte, so every surface that mentions the
checksum — the installer, the release notes, this entry — says only *verified against its published
checksum*, never *secure*, *safe*, or *signed*.

**The one-liner's truncation safety was proven by truncating it**, at eight separate cutoff points
from 10% to 99% of the script's own length, piped exactly as a real `curl | sh` would run it:
**zero files created at every cutoff.** A checksum mismatch refuses and installs nothing. The
installed binary ends up at exactly one location, `$HOME/.local/bin/tekstide`, plus the directory
chain leading to it when that does not already exist — a required correction after the first
draft's own header overstated "one path" and meant "one binary."

**A real, 100%-reproducible-under-`makepkg` packaging defect was found and fixed while building the
AUR package**: this distro's global `-flto=auto` reaches the `cc` crate's build of rusqlite's
bundled `sqlite3.c`, producing GCC LTO-bytecode objects `rust-lld` cannot resolve — every symbol
comes back undefined, every time, under a real `makepkg` run. Fixed by stripping `-flto=auto` from
`$CFLAGS` for that one build invocation only. The package is not published to the AUR itself; that
remains the owner's own account and action.

**`tekstide --licenses` prints the MIT notice `rusqlite`/`libsqlite3-sys` require travelling with
every copy of the software, found missing at the release step for the two channels that ship a bare
binary.** `cargo install` and the AUR package already carried `NOTICE` (it sits at the `.crate`'s
own root, and the `PKGBUILD` installs it explicitly); the one-liner and the GitHub Release asset did
not, because a bare binary has nowhere else for a reader to see it. Embedded in the binary itself
rather than taught to every channel individually, so it travels through all of them, present or
future. The GitHub Release also now carries `NOTICE` and `LICENSE` as their own assets, beside the
binary and its checksum.

**The book's own front page — `docs/src/introduction.md`, published as `index.html` — now states
the platform and no-screen-reader limits itself**, rather than only linking to pages that do,
matching `README.md` word for word so there is one form of each claim rather than a second that
could drift from it.

**Still Linux-only.** The artifacts this release adds are `x86_64` Linux only, same as the
product itself; nothing here implies otherwise, and that stays true until cross-platform support
ships in a future release.

## 0.36.0 - Command Approval A User Can Reach

Status: **released 2026-10-12.** RFC-060's four slices and its Whole-RFC checklist are
all implemented, gated and closed. This entry was written incrementally as slices closed, then
re-read against the finished set at the candidate; see
`rfcs/handoffs/060-command-approval-reachable/` for the full handoff pack.

**For nearly every user, this release changes nothing.** The command-approval UI and API were
finished and proven end to end as of RFC-021/RFC-022, and no configuration could ever produce a
`Managed` profile — the dialog existed and no user could reach it. This release makes the pathway
reachable for a profile that declares an adapter; it does not ship an adapter for any real AI CLI,
and as of this writing none that ship with any AI CLI speak Tekstide's own protocol. Declaring
`adapter = true` on an ordinary AI CLI's profile does not make Tekstide supervise, sandbox, or
intercept anything it runs — it only makes Tekstide wait for a proposal that will never arrive.

**`adapter = true` on a profile is the one key that requests `Managed` mode, and the only one** —
`ConfiguredAiCliProfile` is structurally incapable of representing `Managed` without it, not merely
refused when asked (`ConfiguredAiCliAdapter`, a two-variant enum one match derives both
`compatibility_level` and `structured_action_approval` from, so a third variant could not
half-decide the pair even by a future mistake). `REQ-AGENT-012`, `REQ-SEC-012` and `REQ-SEC-013`
are met **for a profile that declares an adapter** — a reachable pathway, not a property every
profile now has.

**Rejecting a proposal does not prevent execution, and this release is where that sentence first
reaches a real user**, not only the book. The dialog states it directly, in words read before the
decision: approving does not make a command safe, and rejecting cannot stop the AI CLI from running
it anyway — the model is cooperative, not enforced, and nothing about this release changes that.
`REQ-AGENT-013` is met by what the product does not say: no surface, book page or changelog entry
anywhere describes this as command-level enforcement.

**The whole path was demonstrated once, for real** — a configured profile, a real (test-and-proof-
only) reference adapter process, a real proposal, the real dialog, a real decision, and the
resulting `command_approval` audit records read back through `0.35.0`'s own audit view, not by a
direct store query. A real failure mode was found and named while doing it: a `Managed` launch's
approval socket binds under `$XDG_STATE_HOME`, whose path shares the small, fixed budget a Unix
domain socket's path is capped to — an unusually long `XDG_STATE_HOME` can now fail a launch with a
named reason and a fix, instead of the previous generic refusal.

## 0.35.0 - Seeing The Audit

Status: **released 2026-10-12.** RFC-059's three slices and its Whole-RFC checklist are
all implemented, gated and closed. This entry was written incrementally as slices closed, then
re-read against the finished set at the candidate; see `rfcs/handoffs/059-seeing-the-audit/` for
the full handoff pack.

**Twelve audit families recorded, and nothing could see or remove any of it.** `AuditStore::query`
and `AuditStore::purge_all_records`/`purge_project_records` were already written and tested in
`tekstide-core`, with zero production call sites — a storage layer finished ahead of any surface
that used it. This release spends itself on reaching both, and on two recorded defects that decide
whether what the view shows can be believed.

**Every family-and-outcome combination the type can produce was proven to actually land in the real
store, not assumed from the Rust-level validator alone.** An enumeration test, derived from the
type with no hand-written list, found the one gap already known (`safe_close_decision` +
`blocked`, closed by an earlier migration) and no others. A real, reproducible defect was found and
fixed alongside it: two processes racing to *create* the audit database for the first time could
livelock against SQLite's own fixed busy-handler retry schedule — not merely lose, but both retry
in lockstep and exhaust the timeout — fixed with a real blocking OS `flock` across the whole
create-or-open decision, the same primitive RFC-058's project lock already established for this
codebase.

**The audit view (`Ctrl+Alt+V`, or the "Audit" button in the top bar) is a new top-level
destination**, reachable with no project open, since the store is app-wide. It filters on exactly
what `AuditQuery` already supports — project, family, outcome, operation — with no free-text search
(there is no free text in the schema to search). A bounded page says what it left out in
`count_text`'s own existing words, `AuditHealth` is surfaced here when the store itself is
degraded, and a distinct notice covers the case `AuditHealth` cannot: a query against an otherwise
healthy store that fails on its own. The view leads with a disclosure naming which question it
answers — *what happened*, never *what is live right now*, which is Approval History's own
question and the two will legitimately disagree.

**Purge is reachable, per-project and global, each behind a confirmation that names its scope, its
real count, and that the act cannot be undone — this is the first release to track `REQ-SEC-025`
("Users must be able to purge local transcripts, logs, and audit data") for audit data at all,
named in no plan row before this RFC.** The number shown afterwards is `AuditPurgeReceipt::
deleted_record_count`, read off the real `DELETE`, never a count taken before it. A real
consequence was found while building the confirmation, not assumed: purging a project's audit
records also revokes its trust, since trust is decided by querying the exact rows a purge deletes.
The confirmation says so before the purge runs, naming the mechanism and that the revocation lands
at the project's next open, not immediately. The book's own *"There is no in-app command to purge
the audit store"* is corrected in the same slice that makes it false.

**`REQ-SEC-023` is met structurally for audit views, and only for audit views.** Every column in
`audit_events` is an id, an enumerated symbol, or a bounded reference — there is no field that
could hold free text, so no redaction logic was needed to make the claim true. The requirement's
log-content half is unmet and out of scope here; it is **RFC-068**, reserved and not started. No
entry anywhere in this release implies transcript or log content is redacted.

## 0.34.0 - A Project Held By One Process

Status: **released 2026-10-11.** RFC-058's three slices and its Whole-RFC checklist are all
implemented, gated and closed. This entry was written incrementally as slices closed, then re-read
against the finished set at the candidate; see `rfcs/handoffs/058-one-process-per-project/` for the
full handoff pack.

**Two Tekstide windows could hold the same project and neither knew.** `RFC-027`'s recovery records
are keyed by project id and a hash of the document's path, and `AppState::open_project` reuses the
same project id for the same canonical root — which is what lets recovery survive a restart at all,
and also what let two windows on one project write the exact same record file. Open one project
twice, edit the same file differently in each, and the last write wins with nothing saying the
other edit ever existed. **Reproduced against two real processes before anything was designed
against it**, and the reproduction became the regression test the fix is checked against.

**A project root is now held by one process at a time, with a real, process-visible lock — not an
in-memory flag.** A real OS `flock`, the same primitive this project's own transcript writer
already uses, at `$XDG_STATE_HOME/tekstide/project-locks/<project-id>.lock`. Proven released even
when its holder is killed outright, not only when it exits cleanly: a real child process held the
lock, was `SIGKILL`ed, and the lock was free again within a bounded poll. If the mechanism cannot
tell who, if anyone, holds a project — an unreadable state directory, say — opening proceeds
unprotected rather than refusing: a text editor that will not open because a lock file is confusing
has chosen the wrong failure.

**A second attempt at an already-open project is refused, not opened as a duplicate copy, and the
window that holds it is asked for your attention.** The project board says so — *"This project is
already open in another Tekstide window. It has been asked for your attention"* — and that is
exactly what happens, no more: a Wayland window cannot bring itself to the front on its own, so
nothing here claims a raise, switch, or focus took place. The request reaches the real holder over
a small local socket next to the lock file, proven end to end between two real processes — the
holder's own listener reports receiving the knock, not merely that the request was sent.

**A real, narrower hazard was found and deliberately left alone.** The audit store was assumed
already protected against two processes by SQLite's own file locking, and mostly is — but two
processes racing to *create* the database for the first time can still lose, a genuine TOCTOU with
no `IF NOT EXISTS` guarding it. Out of this RFC's own scope (the audit store is app-wide, not
project-scoped, so a per-project lock could never have guarded it regardless), and recorded in
`future-work.md` beside an earlier, related finding rather than left for this RFC's own "already
protected" claim to appear to cover a gap it does not.

## 0.33.0 - The Sidebar Stops Being Two Things

Status: **released 2026-10-10.** RFC-067's three slices and its Whole-RFC checklist are
all implemented, gated and closed. This entry was written incrementally as slices closed, then
re-read against the finished set at the candidate; see `rfcs/handoffs/067-sidebar-not-a-mode/` for
the full handoff pack.

**A project tab had one toggle, and it governed everything.** Switching to Terminal mode to run a
command took the file tree away with it — the sidebar fell back to one sentence, *"Files are
listed here in Content mode,"* explaining an absence it then asked you to leave your terminals to
fix. **The file tree is not a second thing the mode toggle owns any more.** It is there in both
modes now, the same tree either way, reachable the same way either way: `Up`/`Down` moves the
highlight, `Enter` opens a file or a folder, exactly as it already did in Content mode.

**Activating a file from Terminal mode switches you to Content mode and shows it — and nothing
else does.** That is the one new thing this slice adds, and it is deliberately narrow: the
owner's own standing rule is that a result a user cannot see or undo is the wrong shape, and
being moved somewhere you can see (and switch straight back from) is what being shown a file
should do. Nothing else may move you there on your behalf — not a background file-change refresh,
and not `0.31.0`'s own crash-recovery offer, which also opens documents and had to be checked
explicitly rather than assumed safe. A real, pre-existing bug was found and fixed while proving
that boundary: the function every document-open path shares had been forcing Content mode for
*every* caller, harmless while the explorer was the only reachable one, a real violation the
moment a second caller existed. It no longer does; only the explorer's own activation does.

**The cost of switching was measured, not assumed — and it decided this RFC needed nothing
further.** A mode switch is already free in the sense that matters: nothing is discarded, no
terminal stops, no document loses its cursor or its undo history. What was never measured is what
drawing the result afterward costs. Measured directly: switching into Content mode costs on the
order of **20 microseconds** to redraw; Terminal mode, under **5**. Against this project's own
existing latency criterion for "a user would notice" (16 ms), that is roughly three orders of
magnitude of headroom — near-real-time already, in the owner's own words from the RFC itself.
**Building anything further was considered and declined, with the number that made it
unnecessary recorded here rather than only in the evidence**, per the RFC's own explicit
standard: a slice that measures and correctly builds nothing is a success, not a shortfall.

## 0.32.0 - A Refused Close Means Nothing Happened

Status: **released 2026-10-09.** RFC-066's two slices and its Whole-RFC checklist are all
done, reviewed and closed. This entry was written incrementally as each slice closed, then re-read
against the finished set at the candidate rather than left as it stood after the last slice's own
diff; see `rfcs/handoffs/066-refused-close/` for the full handoff pack.

**This started as one defect — a dialog offering an action it then did not perform — and turned
out to be three.** Confirming a close that then refused anyway killed real, running work before
saying no; the dialog offered a button that sometimes silently did nothing; and the audit trail
recorded neither of those failures, because it had no outcome for "refused" at all. All three are
fixed, below, each the user-visible half of the same underlying claim: **a refused close now leaves
behind no trace that it was ever attempted** — nothing terminated, nothing silently declined,
nothing misrecorded.

**Confirming a close that gets refused anyway used to cost you your running terminals first.** A
project with a dirty file (or a pending approval, or a review-ready change) and a live terminal
both refused the close *and* killed every one of those terminals before refusing — real,
unrecoverable process state destroyed by a confirmation that never went through. The assessment now
runs, read-only, before anything live is touched; a project blocked by anything other than its own
running processes has those processes left exactly as they were.

**And now the dialog says so up front, instead of offering a button that sometimes does nothing.**
Attempting to close a project the assessment already blocks shows why — the real reasons, the real
counts — with no confirm button at all, not a disabled one: "This project can't be closed yet," one
`Dismiss` control, nothing to press that would have silently refused anyway. A close blocked only by
a live process still works the way it always did, since terminating that process is what resolves
the one reason stopping it.

**A refused close is no longer recorded as if it closed.** The audit trail had exactly one outcome
for "did not close" before this slice: `Cancelled`, which already means a close was offered and a
user actively declined it. A close the assessment blocked before any offer existed now gets its own
outcome, `Blocked` — found, while proving it, to be rejected by the audit store's own schema at the
SQLite level beneath the code that builds the record; fixed with a real schema migration, the same
shape RFC-013 Amendment 1 already established for changing a shipped version's `CHECK` constraints
without touching what a real installation already wrote.

## 0.31.0 - The Crash Is Detected, Not Guessed

Status: **released 2026-10-09.** RFC-027's three slices (the marker, the record with its
purge, the offer) and the Whole-RFC checklist are all done, reviewed and closed — `REQ-RECOVER-002`
and `REQ-RECOVER-005` met, with what their hedges were decided to mean in
`rfcs/delivery-plan.md`'s own coverage row rather than restated here. This entry was written
incrementally as each slice closed, then re-read against the finished set at the candidate rather
than left as it stood after the last slice's own diff — the lesson of RFC-065 review 479. See
`rfcs/handoffs/027-crash-recovery/` for the full handoff pack.

**A crash is detected, not guessed.** Tekstide now writes a small marker at every launch and
removes it at every clean exit, so a marker still there at the next launch means the previous one
did not exit cleanly — a real `SIGKILL` leaves it, a clean window close removes it, and two
Tekstides running at once never mistake each other for a crash. Detected only, for now: there is
nothing yet to offer back.

**An unsaved document now has somewhere to come back from.** Every **dirty** document gets a small
side record — its text, cursor and viewport, and the file it was opened against — written
periodically while you type, entirely outside your project; the file you are editing is never
touched by it. A clean document gets no record at all, and one that becomes dirty gets one the
moment it does: the same "only what is not already on disk" reasoning the rest of this product's
retained content already follows. The record is gone the instant its own reason ends — saved,
undone back to clean, or its project closed — and a byte bound, named to you at the moment it would
be exceeded, keeps the largest realistic document from writing more than it should.

**Counted and purged exactly like a transcript, from the same release that starts writing it.**
Trust Settings shows how much recovery data a project is retaining, in its own line beside the
transcript figure, and the same **Purge** button removes both. `[recovery]
persist_unsaved_buffers = true` is the setting, on by default and forward-only (turning it off
does not delete what already exists) — a protection a user has to find and enable first does not
prevent the loss this feature exists to prevent.

**What this costs, measured directly at ten and twenty documents, across several runs rather than
one.** Ten dirty documents, each at the largest realistic size, and twenty — the open set's own
bound — agree on a per-document rate to within 0.3% within any one run (about **1.2 ms per
document** either way), confirming the rate is linear between ten and twenty rather than assuming
it. **The absolute cost moves with the machine's own load, by as much as 1.7x between runs taken
over the course of this measurement**: three runs measured twenty documents directly, back to back
under this machine's own low-load state, at **24 to 25 ms** of delivery work per persist pass.
**41 ms is not a fourth measurement of twenty documents** — no run produced it — **it is the
confirmed per-document rate applied to the worst ten-document rate this same campaign actually
observed** (`20.4 ms` for ten, carried to twenty at the same ~1.2 ms/document). **24 to 41 ms is
therefore the range this write cadence (every 2 seconds) is chosen against**, to keep it rare
regardless of where in that range any one tick lands — not a claim that 41 ms was itself measured
at twenty documents. **One document's own cost could not be measured** in any run: its own signal sat
inside the harness's own round-to-round noise, straddling zero, rather than landing on a stable
number — the confirmed rate is between ten and twenty documents, not from one, and is not claimed
to hold below ten.

**And now there is somewhere to bring it back.** Opening a project with recovery records on disk —
whichever way it is opened — offers them back: each recoverable document, named by its own path,
restored on acceptance with the text, cursor and viewport it had when it was last written, but
never its undo history. Declining leaves every file, and every record, exactly as it was; nothing
here is a one-way door until a document is actually accepted. What the offer is driven by is the
record's own presence on disk, **never** whether the previous session left behind a crash marker —
a clean window close already removes that marker while leaving the records behind, and quitting
with unsaved work is unguarded today, so a record with no marker is the ordinary case this exists
for, not a defect. The file on disk is compared against what the record itself last saw: unchanged
restores the document dirty, exactly as it was; changed or deleted since restores it through the
same conflict machinery an already-open document's own external change already uses — no new
vocabulary for either case, and the deleted case is told apart from the changed one the identical
way it already is anywhere else in the editor.

## 0.30.0 - The Multi-Document Model

Status: **released 2026-10-08.** `REQ-EDIT-004` met for its own plural. No `REQ-` names
multi-document itself — disclosed, not minted, in `rfcs/delivery-plan.md`'s own "Requirements gap"
entry. RFC-065 is implemented and closed (`rfcs/done/065-the-multi-document-model.md`); see
`rfcs/handoffs/065-multi-document/` for the full handoff pack and its own evidence.

**The repair, and this release's headline**: opening a second file used to silently discard the
first's unsaved edits — no prompt, no refusal, no record, a live defect shipping in `0.29.0`. The
editor now holds an **open set** of documents rather than one, so opening a second file adds it to
the set instead of replacing what was there, and there is nothing left to discard. Nothing means
nothing: the first document's own undo history survives too, since it always lived on the document
itself, not on "whichever one is active" — the open set does not touch it, so the switcher can never
undo into the wrong file. Deduplicated by path: opening a file that is already in the set switches to
its existing entry (text, dirty state and cursor untouched) rather than adding a second. `REQ-EDIT-004`
has said "dirty *buffers*", plural, since it was written, and the product has only ever had one; the
open set makes that plural real for the first time, and the requirements coverage row is corrected
from implying it already was. The two counts a user reads from, and the close dialog, count the whole
set through the same wiring they always used — only what they were counting changed. The set is
bounded at **20 open documents**, refused with the open count and the limit stated once reached, even
with a document already active. **Every open document is watched, not only the active one**: a
background document's own external change or conflict is detected the same way the active one's
always was, and is already waiting, named, the moment you switch to it — not discovered only once a
document happens to become active.

**`0.29.0`'s own open question, answered**: that release's "what this release does not do" section said
of the open-document refresh, "with one document this is a few milliseconds; the multi-document RFC
inherits the question of what a burst touching several open documents at once should cost." Measured
(RFC-065 D7/D13, reusing RFC-026's own paired-control harness): the per-document refresh cost is close to
linear in the number of open documents. One document costs about **4.2 ms** of delivery work (between
keystrokes) per touched burst window; ten documents cost about **38.9 ms** — a ratio of **9.2×** against
the 10× a linear cost would predict. At the open set's own bound of twenty documents (PR-065-B, D4), that
extrapolates to roughly **78 ms** of delivery work per burst window, between keystrokes, invisible to
typing latency itself but real CPU cost on the machine. The bound is now a number with a measurement
behind it, not only a reasoned convention.

**The switcher**: `Ctrl+Alt+F` cycles the active document to the next one in the open set, wrapping —
keyboard-first and reachable, not a new tab bar (RFC-021's lesson: a model nobody can reach is not
implemented). Each document's own cursor and viewport are restored exactly on return, never reset,
since both already lived on the document itself and the switch never touches them. **The explorer's own
`[open]` tag widened its meaning along the way**: introduced in a previous release to mark the one file
open in the editor, it now marks every member of the open set — a project with three documents open
shows `[open]` on all three rows, not one. A user who learned the old, singular meaning should read the
tag as "open", not "the one on screen"; the editor's own header, above the cursor line, is what names
*which* open document is active.

**Save all**: `Ctrl+Shift+S` (or the editor's own "Save All" button) saves every open document in one
action — new work, not a loop around `Ctrl+S`'s own save, because saving several files is a sequence of
writes that can fail partway. One document's own save being blocked (an external change, an unsafe
symlink, a write failure) does not cost any other document its own save: every open document is
attempted regardless of an earlier one's own result, and the chrome says, afterward, how many of the
open set succeeded and names each one that failed, with why — success included, so "nothing has
happened yet" and "all of it worked" never look the same. Each *dirty* document's own save is the same
temp-and-rename write a single `Ctrl+S` already was — but a clean document's own save returns success
without ever reaching that write, so an N-document save-all costs the watcher one scan and one re-read
per document actually written, not N: at most, the count of documents that were dirty when the action
was pressed. A save-all over a mostly-clean open set (one edited document among many open) costs the
watcher the same single notice one `Ctrl+S` on that one document already would have.

## 0.29.0 - The Explorer Keeps Up On Its Own

Status: **released on 2026-10-07.** Published to crates.io (`tekstide-core` and `tekstide`) and tagged `0.29.0` at `e346dfe`. Post-publish: `0.29.0` passes all three checks, and **`0.28.0` still installs** now that a newer core exists. Scoped 2026-10-07: `REQ-FILE-003`, `REQ-FILE-004` and `NFR-PERF-007` met and measured. RFC-026
is now the watcher alone — the multi-document model it originally covered has its own RFC, with its own
number and its own release, `0.30.0`.

The explorer used to read a folder once and never again: a file created, deleted or renamed after you
opened it stayed invisible until you closed and reopened the folder. It now watches — the project root,
every folder you expand, and the folder of whatever file is open — and a change in any of them appears on
its own. A burst of a thousand files landing in one folder costs **one** re-read, not a thousand, about
250 ms after the first of them. Watching is per project, and if the kernel refuses another watch it stops
entirely for that project rather than failing partway; the sidebar says so, in words, and reopening the
project is what resumes it. The open file notices too: its header now names a change, a deletion, or a
genuine conflict as soon as the watcher sees it, and a **Reload** button beside Save reaches the same
dialog Ctrl+S already opened on a blocked save — directly, without first trying to save and having it
refused. **The first new dependency in many releases**: `notify` 8.2.0, five crates on Linux, eighteen
`Cargo.lock` entries counting every platform, CC0-1.0 licensed.

### Added — the explorer and an open file notice a change without being asked

- **A file created, deleted or renamed in an expanded folder (or the project root) appears without
  reopening anything**, captured live from the running app (`REQ-FILE-003`). The explorer's own watch
  scope is exactly what the user opened: the project root always, each expanded folder while it is
  expanded, and the folder holding whichever file is open — nothing wider.
- **A burst of changes is batched into one scan per directory, not one per change.** A thousand files
  written into one watched folder inside a quarter-second window cost **3 scans** total across the whole
  burst, measured against a simulated stream before any real watcher existed and again against a real one
  after. The window is 250 ms, stated as a constant, not tuned.
- **Typing does not stop while the watcher works** (`REQ-FILE-004`). Measured four ways in one process
  under one load, over four rounds — unwatched, watched and idle, watched during the burst above, and
  watched during the burst with the open file itself being rewritten — keystroke latency on a
  100,000-line file stayed at p95 8.2–11.1 ms and p99 up to 12.0 ms, against budgets of 16 ms and 33 ms,
  in every condition and every round but one, where a load spike from outside the measurement moved all
  four conditions together and is recorded rather than discarded.
- **The open document hears a change too, through the same states the save path already had.** A notice
  naming the open file's path reaches it on the next batching window: a clean document becomes
  `ExternalChanged` (its text kept, nothing reloaded), a dirty one `Conflict`, and a file that is gone is
  `ExternalDeleted` — a new, distinct state, since "deleted" and "changed" are different facts and the
  product no longer says the wrong one. The header names each: *(changed on disk, not reloaded)*,
  *(deleted on disk, not reloaded)*, *(conflict)*.
- **A Reload button beside Save reaches the external-change dialog directly**, without first attempting a
  save and having it refused. It is shown only when there is something real to reload from —
  `ExternalChanged` or `Conflict`, never a deleted file and never an ordinary document. Reload constructs a
  fresh document, the same mechanism `0.28.0`'s own undo already relies on not crossing, so a reload still
  discards undo history along with the content it replaces, and the product says so rather than pretending
  otherwise.
- **A kernel refusal stops watching for the whole project, never partway.** Forced in a test against a
  fake backend: every watch is dropped, cleanly, with no crash. The sidebar says *"Folders are no longer
  updating. Reopen the project to resume."* — two lines, chosen because both fit the sidebar's own width
  where two other candidates did not — and reopening the project is the only thing that resumes it.
- **A symlink that leaves the project root is never watched, and a loop never makes the watcher recurse.**
  The access policy is the only way a directory can enter the watch scope; proved against a real escaping
  symlink and a real loop, with a live watch, not by reasoning about the type.

### Fixed — a deleted file read as merely changed, in two places

- **Both the document refresh and a blocked save collapsed "the file is gone" into the same state as "the
  file changed."** A deletion is a different fact from a change, and the product now says which one
  happened, through `ProjectContentStatus::ExternalDeleted` and the header's own *(deleted on disk, not
  reloaded)*. The save path had the identical collapse the refresh path did, found by checking every path
  that can reach the dialog the new Reload button also opens, not only the one that surfaced first.
- **An edit made after an external change stayed in the "nothing local to lose" state.** A clean document
  the disk changed under, then edited, is local work the disk does not have — a conflict — and had been
  left reading as a clean external change instead, which a later refresh or the save dialog would have
  read as safe to discard without asking. Undoing back to the exact text that was opened still does not
  clear a genuine conflict (`0.28.0`'s own rule, kept): only its own resolution does.

### Fixed — a measurement that tested the machine instead of the code

**Not this release's subject**, carried in the way `0.27.0` carried an unrelated pin fix:
`change_review_content_view_build_cost_by_line_count_measurement` asserted an absolute 500 ms budget and
had crossed it under load four separate times — `0.26.0`, twice during `0.28.0`'s own candidate and
verification, and this release's own gate — each time explained after the fact by reporting the load
average beside it rather than fixed. It now times a 10,000-line reference workload
against a 100,000-line one — ten times the work — back to back in the same run, and asserts the larger
costs under thirty times the smaller's own time. An ambient load spike moves both sides of that ratio
together; a real regression moves only the ratio. Verified stable both quiet (≈10.1×) and under
artificial load (≈9.0× while the absolute times nearly tripled).

### Fixed — the book said things that were not true

Found while documenting this release, all three predating it: **"The editor has no undo"** (`0.28.0`
added real undo and redo, and the book was never updated); **"Not built"** still listing no file watcher,
after this release shipped one; and the keyboard reference's own chord count, stale since before undo and
redo existed (*"prints fifteen chords while the policy holds sixteen"* — the policy has held eighteen,
seventeen bound and one reserved, since `0.28.0`). A release checklist step now reads the book against
each release's own Added section for exactly this shape of staleness.

### What this release does not do

- **The multi-document model is not in this release.** One file open at a time, even across several
  projects — `active_document` keeps its meaning. It has its own RFC, scheduled for `0.30.0`.
- **Watching is per project and binary: live or stopped, nothing in between.** A refusal stops every
  watch in that project at once; it does not degrade one folder at a time, and nothing retries on its own.
- **The real kernel refusal path (`ENOSPC`, the inotify instance or watch limit) is evidenced by reading
  the platform's own source, not by forcing the real limit in a test** — the limit is per-user and shared
  with every other process on the machine, so a test cannot force it without disturbing everything else
  running.
- **A save costs one extra scan and a whole-file re-read of the file it just wrote**, since the save is
  itself a change inside a folder the project watches. Measured at about 4 ms for a 3.3 MB file.
- **The open-document refresh reads the whole file, every time**, bounded by the same 4 MiB editable cap
  everything else is. With one document this is a few milliseconds; the multi-document RFC inherits the question of
  what a burst touching several open documents at once should cost, rather than this release deciding it
  for a case it does not yet have.
- **Everything `0.28.0` listed still holds**, including there being **no screen-reader support**.

## 0.28.0 - The Editor Knows Where You Are

Status: **released on 2026-09-30.** Published to crates.io (`tekstide-core` and `tekstide`) and tagged `0.28.0` at `db1057d`. Post-publish: `0.28.0` passes all three checks, and **`0.27.0` still installs now that a newer core exists** — the first release whose predecessor the pin protected.

The editor used to draw the whole file as one string every frame, showed no line number, drew no
caret, and could not take back a keystroke. It now draws the rows that fit the window, numbers them
from the file's own index, shows a caret that is an element rather than a character, follows the
cursor sideways when a line runs past the right edge, and undoes and redoes by operation — bounded,
and it says so once the bound is reached. The title and the two global actions now share one row
above the project tabs, and the editor body's own default font is fixed-width, which is what makes
the sideways-scroll math exact rather than approximate for anyone who configures nothing. Checked by
running the release binary: a 200-character line of the widest ordinary character, cursor moved to
the exact column a proportional font used to lose the caret at — the caret stayed on screen.

### Added — line numbers, a real caret, sideways scrolling, and undo

- **The editor has line numbers.** They read the file's own line index, not the row's position on screen, and the gutter is sized to
  the whole file's own digit count — a file crossing 10,000 lines does not shift its text column as you scroll past the boundary
  (measured live, both sides of the crossing).
- **The editor has a visible caret**, drawn as its own element, never as a character inserted into the text: a file whose text
  contains a look-alike glyph (a full block, the character someone might reach for to draw a caret by hand) shows the real caret and
  the glyph as two visibly different things, at the same time, on adjacent lines.
- **A line wider than the editor now scrolls sideways to follow the cursor**, on the same rule the vertical scroll already
  used — the least movement that keeps the cursor on screen. A user typing past the right edge of a long line could type into text
  they could not see; they cannot now, at any column.
- **Lines still do not wrap.** One line is one drawn row; a long line's own width is windowed, the same way the file's height already
  is, rather than broken across more than one row — wrapping would trade away the exact per-row arithmetic the vertical window
  depends on.
- **The editor has undo and redo**, `Ctrl+Z`/`Ctrl+Shift+Z`. Operations, not snapshots: the four invertible edits are a character,
  `Enter`, `Space` and `Backspace`, each stored as what its own inverse needs, never a copy of the document. Depth is bounded at
  500 edits, and the editor says so on screen once that bound is actually reached, rather than dropping the oldest edit silently.
  Undoing all the way back to the text a file was opened with returns it to a clean, unmodified state, and undo does not reach
  back across a reload of the file's real, current content on disk.

### Changed — the editor draws only the lines that fit, and the window is two rows

- **The editor draws the lines that fit the window, not the whole file, and the window follows the cursor.** Typing in a 100,000-line
  file now takes **p95 5.3–6.8 ms and p99 5.3–6.8 ms** (down from 14.0–16.3 and 14.0–16.6 ms; painting is still not in the figure),
  across two batches of three runs. Each keystroke still copies the document once to apply the edit; that is now most of the cost and is
  not changed here. Arrow keys cost slightly more than before (2.8 ms against 1.8 at 100,000 lines: they now keep the window on the cursor).
- **The window moves only by the cursor**: there is still no scrollbar and no mouse wheel, and navigation is still the four arrow keys.
- **The top of the window is two rows, not three.** The title and the two global actions (Trust Settings, Help) now share one row;
  the project tabs are the row below it, instead of a third row of their own underneath both.
- **The editor body's own default font is now fixed-width**, matching the terminal and the file tree, rather than the interface's
  own general default (which may be proportional). A configured font `family` still reaches the editor body exactly as it reaches
  the rest of the interface — this only changes what you get if you configure nothing. This is what makes the horizontal-scroll
  approximation correct by default: measured against the shipped default, **0.0% off, zero caret overflow**, where the previous
  default could lose the caret over 50 columns off screen on a line of wide characters. Configuring a proportional family still
  carries that risk, disclosed in `configuration.md`.

### Measured — typing in a very large file

- **Typing in a 100,000-line file is at the 16 ms budget with no headroom, at the baseline this release started from.** Measured on
  a release build against a 3.3 MB, 100,000-line file (`NFR-PERF-003`: p95 ≤ 16 ms, p99 ≤ 33 ms), the code Tekstide ran on a keystroke
  before any change in this release — applying the edit, building the view, laying out the text — took **p95 14.0–16.3 ms and p99
  14.0–16.6 ms** over three runs; typing at the start of the file was over the 16 ms line in two of them. **That was a lower bound:**
  painting and presenting the frame were not in it. The cost grew with the length of the file, not with the edit (1,000 lines 0.4 ms,
  10,000 lines 1.7 ms, 100,000 lines 15.8 ms), because each keystroke copied and re-split the whole document. This was measured first,
  before anything else in this release changed, so the numbers above it are a claim with something to compare against.
- **A file of 100,000 lines can only be opened if its lines average under 42 bytes**, because the editor refuses anything over 4 MiB. A file whose
  lines average 51 bytes (this project's first test fixture) is refused at that many lines, so the requirement's "100,000-line file" and the size cap
  disagree for any file with longer lines. Neither has been changed.
- **The column width used to fit a character is an approximation** (the same kind of approximation row height already used
  for line pitch), not a measurement of the actual, per-character width `cosmic-text` will lay out — a window sized from it can hold very
  slightly more or fewer characters than the region's real pixel width allows. Exact for the shipped fixed-width default (see *Changed*,
  above); still an approximation, with its own measured risk, for a configured proportional family.

### What this release does not do

- **Each keystroke still copies the whole document once to apply the edit.** This is now roughly 80% of what typing costs at
  100,000 lines, and the budget has threefold headroom regardless — left alone deliberately, not overlooked.
- **Lines still do not wrap**, there is still no scrollbar and no mouse wheel, and navigation is still the four arrow keys.
- **The editor's own vocabulary is still four keys.** No selection, no clipboard, no delete-forward.
- **No syntax highlighting.** An owner decision to make, not a silent deferral.
- **A configured proportional font family still loses the approximation's safety.** Checked against the default face: a line of
  wide characters (a capital `W`, repeated) can leave the cursor **over 50 columns outside the window that is actually drawn**,
  disclosed in `configuration.md` beside the family setting. The shipped default does not have this risk (see *Changed*, above).

## 0.27.0 - A Run That Still Exists Tomorrow

Status: **released on 2026-09-26.** Published to crates.io (`tekstide-core` and `tekstide`) and tagged `0.27.0` at `a2e220f`. **The first release whose post-publish check passes in full**: the app pins its own core, the archive's lockfile names it, and `cargo install tekstide --version 0.27.0` builds.

A run used to be gone the moment Tekstide closed: reopen a project and its runs were transcript files with no prompt, profile or ending behind
them, and there was nowhere to say what a run was for or to hand on what it did. A run now leaves a small record beside its transcript, is listed
again when the project reopens, and says when it does not know how it ended. On the AgentRun Report you can classify it, write notes on it and
export a report that keeps your words apart from the run's. Checked by running the release binary: a run classified with a label containing a
right-to-left override, noted, exported, killed with the app and reopened came back intact — and purging it left the exported file untouched.
This release also fixes how an older release installs; see *Fixed*.

### Added — an agent run survives closing the app

- **A run leaves a record, `run.json`, beside its transcript**, and a project reopened later lists the run again: its profile,
  its prompt summary, its ids, and its transcript attached to it, instead of a transcript with no run behind it. The Project
  Board's *agent runs* count now includes them. The record is written when the run changes, not when it ends, atomically, and it
  holds references only — ids, timestamps and the prompt summary — never what the run printed; that stays in the transcript.
- **A run whose app was closed while it was running says it does not know when it ended.** It is not shown as finished, and
  it is not given the time it was reopened as an ending. A restored run is a record, not a process: it is never counted as
  running or failed, and does not count against `agent_run_limit`.
- **A record that cannot be read is renamed with a `.corrupt` name beside its transcript and never deleted**, the same way an
  unreadable `recent-projects.json` is; the Project Board says how many were set aside, and the run is listed as a transcript
  with no run. A record written by a newer Tekstide is treated the same way, and named as such. The disk-usage figure counts
  `run.json` as Tekstide's own file. An older Tekstide, which does not know it, counts it as bytes it does not recognise.
- **Purging removes a run's record along with its transcript; the retention age and the size limits do not.** They expire
  *transcripts*, and a run's record — its notes, once you can write them — is your own data, so it stays and the run is still
  listed; a later purge removes it, whether or not the transcript is still there. The purge dialog names what it removes — runs,
  their transcripts and their records — and counts the bytes of both; Trust Settings' *Retained locally* figure counts transcripts
  only, and says so. The run's folder is removed **only if nothing else is left in it** — a file Tekstide did not write is never
  deleted and keeps its folder. Files it does write and remove are exactly `run.json`, `run.json.tmp` (a write a kill interrupted)
  and the `run.json.corrupt` names set aside above; a name that only begins like one is not Tekstide's and is left alone.
- **Limits:** a record keeps at most 200 approval, change-set and audit-event ids each, and says when it left something out. A run started with transcript capture off, or one
  that could not be given a transcript directory, has no record. A change to a run's status reaches its record within a second;
  a change made to it by the user is written at once.

### Added — classify a run, write notes on it, and hand on a report

- **On the AgentRun Report (`Ctrl+Alt+R`) you can classify the run** — coding, review, documentation, testing, refactoring or
  release, or a label of your own — **and write notes on it**, by button or by key (`1`–`6`, `c`, `x`, `n`). Both are saved in the
  run's record the moment you change them, so a run killed a second later keeps them, and a reopened project lists the run with
  them. The notes are your own words: nothing the run printed is ever put in them.
- **`Export report…` (`e`) writes a Markdown report where you ask**, never over an existing file, readable only by you, and never
  inside Tekstide's own data folder. It says what the file will contain before you save and that Tekstide can no longer change or
  remove it afterwards. Tekstide keeps no copy, and **purging a run does not remove a report you exported.**
- **The file keeps three kinds of text apart**: your words on lines beginning `| `, what Tekstide recorded as plain lines, and
  what came from the run (changed paths, the last 16 KiB of the transcript) on lines beginning `> `. Every untrusted value is
  escaped, so a newline or a bidi control in a label, a note, a path or the transcript cannot forge a line of another kind.
- **Limits:** the report quotes the tail of the transcript, not all of it, and lists at most 500 changed paths, and says how many
  it left out. A note is at most 4,000 characters and a custom label 64. A run that has no record folder (transcript capture was
  off) can be classified and annotated, but only until Tekstide closes, and the report says so.

### Fixed — a pinned older release could not be installed

- **`cargo install tekstide --version 0.25.0` (and any older release) stopped compiling the day `0.26.0` was published.** Every
  published `tekstide` up to `0.26.0` declared its `tekstide-core` as `version = "0"` — *any* `0.x` — which opts out of Cargo's own
  rule that `0.26` and `0.25` are incompatible, so an older app resolved the *newest* core, and `0.26.0`'s core had added three
  public enum variants the older app's `match`es did not cover. **From this release the app pins its core to its own version**, and
  a test fails if the pin and the version ever disagree. **Releases already published cannot be repaired — their metadata is
  frozen.** The documented `cargo install tekstide` (no version) always resolved the newest pair and was never affected. **To install
  an older release, add `--locked`** — each published release's own lockfile names the core it was built with, and `cargo install
  tekstide --version 0.25.0 --locked` builds (measured); or install the latest release, or build from that release's git tag.
- The release procedure gained a post-publish check that resolves the release just published **and the previous one** from the
  registry.

### What this release does not do

- **A change to a run's status reaches its record within a second**, not instantly; a run killed inside that second loses that status
  change and nothing else. What you write — a classification or a note — is saved the moment you make it.
- **A run with no record folder has no record.** A run started with transcript capture off, or that could not be given a transcript
  folder, can be classified and annotated, but only until Tekstide closes, and the report says so.
- **A report quotes the last 16 KiB of the transcript and lists at most 500 changed paths**, and says how much it left out. It is written
  where you choose: **a report exported into the project folder is one more untracked file in a project Tekstide watches for changes.**
- **Exporting a report is not recorded in the audit store.** Whether it should be is an open question, not a decision.
- **An older Tekstide does not know `run.json`** and counts it as bytes it does not recognise. It will neither read nor delete it.
- **The report's changed-files section was tested, not watched with a real change set**, and the classification buttons were driven by key
  and by their click message, not by a mouse.
- **Everything `0.26.0` listed still holds**, including there being **no screen-reader support**.

## 0.26.0 - The Explorer Asks Git

Status: **released on 2026-09-25.** Published to crates.io (`tekstide-core` and `tekstide`) and tagged `0.26.0` at `3ae07ee`.

The file explorer stopped guessing which things are ignored from a three-name list and asks Git. Inside a repository an ignored file is
marked, an ignored folder is collapsed, `explorer.show_ignored` decides whether the ignored files are drawn at all, and the sidebar says where
its answer came from and how old it is. Checked by running the release binary, which found two things no test had: a sentence cut off in the
sidebar, and an ignored folder's row clipping its own name to one letter — the row now leads with the name. Git could not be asked as cheaply as
the plan said; the changelog below is where that, and what Tekstide still does not ask Git, is written down.

### Added — the explorer asks Git what is ignored

- **Inside a Git repository the explorer asks Git which entries are ignored**, about the entries the sidebar is about to
  draw, and marks them `[ignored]`. It does not read `.gitignore` itself: negation, precedence, nested ignore files and
  `.git/info/exclude` are Git's to decide. A repository whose `.gitignore` does not name `target/` now shows an ordinary,
  expandable `target/`; one that does shows it collapsed and ignored.
- **`[explorer] show_ignored`** (default `false`). Off, ignored **files** are not drawn and the directory says how many it
  left out (*2 ignored files hidden*); **an ignored folder keeps its row**, collapsed and marked `[ignored]`, so it can still be
  opened; on, the files are drawn too, with `[ignored]`. It governs ignored entries only — `.env`,
  `.gitignore` and every other dotfile are ordinary rows either way. A value that is not `true` or `false` is not used and the
  Project Board says so. `Ctrl+Alt+C` applies it to open projects.
- **The sidebar says where the rule came from and how old it is**: *project's Git*, *parent Git repo* (the project is inside a
  repository — the status bar's Git state reads only a repository at the project root, and the two may differ), *nested Git
  repo*, or *built-in* with why Git was not used; and *Marks are as old as the scan*.
- **A tracked file that matches an ignore pattern is tracked, not ignored.** Git says so, and so does the explorer.

### Changed

- **A row is an icon, then the name, then the words.** It was the words first, so that a narrow sidebar would clip the name.
  That made an ignored directory read `(collapsed) [ignored] t` — a name cut to a letter, indistinguishable from a file named
  `t`. A clipped *word* is visibly damaged; a clipped *name* invents a file. A long *name* is still clipped at the sidebar's
  right edge, without a marker.
- **The explorer of a repository looks different by default**: ignored *files* are no longer drawn, and a line says how many; an
  ignored *folder* stays, collapsed and marked. Set `show_ignored = true` to see the files.
- **`.git` stays collapsed** whichever rule decides. Git does not call it ignored, but it is not a folder to open by accident.
- **Git's answer is asked in about 3 ms** on a 100,000-entry folder (the folder read itself, unchanged, is about 22 ms), on the
  background thread that already reads it.

### Corrected

- **`0.24.0` said `.git`, `node_modules` and `target` are collapsed because they are on a fixed list, and that `.gitignore`
  handling, an `ignored` badge and a hidden-file toggle were "the next slice".** Inside a Git repository that list no longer
  decides: Git's answer does, so a `target/` your repository does not ignore is an ordinary folder and one it ignores is
  collapsed. **A reader who relied on `target/` always being collapsed should look at a repository whose `.gitignore` does not
  name it.** Outside a repository, and where Git cannot be asked, the list still decides and the sidebar says so. There is still
  no hidden-file toggle, deliberately: **dotfiles are not hidden**, and `show_ignored` is about ignored entries only.
- **`0.24.0` said a row says what it is before it says its name** (`[OTHER] (blocked) [symlink escapes root]` before the name).
  It now says the name first; see *Changed*.

### What this release does not do

- **The marks are as old as the scan, and nothing watches the disk.** A `.gitignore` you edit is not reflected until the folder is
  read again (close and reopen it); the sidebar says *Marks are as old as the scan*.
- **A global ignore file named by `core.excludesFile` is not applied.** Tekstide runs Git without your personal Git configuration —
  the restriction that keeps a repository from making it run a program — so `.gitignore` files, `.git/info/exclude` and the default
  `$XDG_CONFIG_HOME/git/ignore` are honoured, and `core.excludesFile` is not. **For a user who sets it, Tekstide and their own
  `git status` can disagree in both directions.**
- **A repository whose configuration names something Tekstide does not vouch for is not asked** (for example `core.fsmonitor`:
  `git check-ignore` would run it), nor is one rooted at your home directory (a dotfiles repository that ignores everything would
  hide every project under it). The built-in list decides, and the sidebar says why.
- **A long filename is clipped at the sidebar's right edge with no marker**, and a name that is only a longer name's prefix reads
  as a different file. The line under the tree shows the highlighted row whole. This was true of `0.24.0` and is unchanged.
- **The status bar's Git state and the explorer's ignore marks can come from different repositories** when a project sits inside a
  repository; the sidebar's first line says when they do.
- **Entries past the 256-per-folder cap are not asked about, so they are neither hidden nor marked** (the folder says how many
  it left out, as it always has).
- **Everything `0.25.0` listed still holds**, including there being **no screen-reader support**.

## 0.25.0 - The Window Takes Your Settings

Status: **released on 2026-09-24.** Published to crates.io as `tekstide 0.25.0` and
`tekstide-core 0.25.0`, tagged `0.25.0` on `87e3652`.

Five things became settable: the keyboard shortcuts, the colours, the font family, the text sizes and how much
scrollback a terminal keeps. `Ctrl+Alt+C` applies a change to the running window. A bad value falls back on its
own, the rest of the file still applies, and the Project Board names the setting and the reason. Every part of it
was checked by running the release binary, and two things were found that way that no unit test had: a Help
column that let a chord run into its description at a larger font, and — by measuring rather than reasoning — a
scrollback cap that was wrong until the allocator said so.

### Added — keybindings are settings

- **A `[keybindings]` section in `config.toml`.** One entry per action, the chord spelled exactly as the Help
  modal prints it (`open_help = "Ctrl+Alt+J"`); the Help modal shows the chords in force.
- **A bad entry falls back on its own, and the Project Board names the setting and the reason** — a chord it
  cannot read, a reserved action or chord (`Ctrl+Shift+P`), an action with no key at all, or a chord that
  already reaches another action. **There is no "last one wins"**: two entries for one chord are both refused,
  so the result never depends on the order you wrote them in.
- **Two actions have no key, and now say why**: `cycle_visible_terminal_session` and
  `open_safe_close_dialog` had a status that read as "bindable" and meant "dead". A rule is now reserved, bound or
  dead — one type, so it cannot be two — and each dead action states how a user reaches what it stands for, or
  that they cannot.

### Added — colours, font and sizes are settings

- **`[theme]` and `[font]`.** Seven colours (`#RRGGBB`), a font family, and the three text sizes. The family
  applies live, without a restart.
- **You cannot make the window unreadable.** Text must meet **4.5:1** against the two surfaces it sits on, and
  the **focus border 3:1**; a colour that would break either is not used and the board says the ratio it
  measured. Sizes outside 8–32 pixels fall back.
- **The scrim is capped at 90 % opaque**, so a dialog's backdrop can never look like a window the application did
  not draw. Above that it is *reduced*, not refused, and the board says so — in its own words, because a reduced
  value is still in force.
- **A font is a name, never a path.** The family is looked up in the fonts the window is already drawn from; a
  path is refused, an uninstalled name is named on the board, and no configured string is ever opened as a font.
  The terminal and the file tree keep a fixed-width face.

### Added — terminal scrollback is a setting

- **`[terminal] scrollback_lines`.** The default is unchanged (2,000). **The most it can be is 12,000**; a larger
  number is reduced to that with a note on the board. A terminal keeps its scrollback in memory and one should not
  take more than 64 MiB, so the cap is **measured** — the allocator's own count over real output — not chosen, and
  **a wider terminal keeps fewer lines** where the number asked for would pass the budget at that width, and
  gets them back when narrowed. A change applies to terminals that are already open.

### Changed

- **Buttons follow the theme.** They were the one thing in the window a configured colour did not reach, and they
  were iced's default blue. **This changes the look of a window with no configuration at all**: a button is now
  the same surface colour as the rest of the chrome, with a border, and a heavier border when the pointer is over
  it. Every button still has its label.
- **The Help window's key column is wider (126 px at the default size, from 110) and grows with the text size.**
  A fixed width let the longest chord run into its description once the size or font was configured.
- **`[ui]` says where its settings went** — `[theme]` and `[font]` — instead of "no effect yet".
- **A configuration file inside a project is never read**, and this is now pinned by tests that look at the
  loader's one caller, not only at the loader.

### Corrected

- **`0.24.0` and every release before it said the product could not be configured beyond AI CLI profiles, a
  retention period and a run limit** — the README said "four settings take effect", and a file with a
  `[keybindings]`, `[ui]` or `[terminal]` section was refused whole with "no effect yet". That is no longer true
  for `[keybindings]`, `[theme]`, `[font]` and `[terminal] scrollback_lines`. **A reader who relied on the old
  claim should look again at the Configuration page**: a file that was ignored whole because it carried one of
  those keys now loads, and each of them now does something. (`[ui]` is still refused, with a pointer to
  `[theme]` and `[font]`; nothing about data on disk changed.)

### What this release does not do

Re-read against what RFC-054 changed, not copied from `0.24.0`.

- **There is no settings screen.** It is a file, read at startup and at `Ctrl+Alt+C`.
- **Two actions cannot be rebound and `Ctrl+Shift+P` is reserved**, for a command palette that does not exist. A
  chord is `Ctrl` and/or `Alt`, optionally `Shift`, and one letter or digit; `Tab`, arrows and function keys are
  not rebindable. `tekstide --help` prints the shipped defaults (it runs before the file is read); the Help
  window prints the chords in force.
- **Contrast is measured for text and for the focus border, not for `accent` or the ordinary border**, which are
  decoration and yours to choose. A light theme has to set `surface_elevated` and `border_focused` as well, or the
  default dark surface and blue focus border will fail against it and *your background* is what gets taken back.
- **A font family must be an installed family's name.** `sans-serif` and `monospace` are not names. The family
  applies to the interface's text, including the editor's; the terminal and the file tree stay monospace.
- **The scrollback cap is for ordinary output.** *A 12,000-line terminal at 200 columns can reach roughly 200 MB
  if every cell carries a combining mark*, because each such cell holds its own allocation. The shipped 2,000-line
  default already had this exposure. Bounding combining marks per cell is a change to the terminal filter and is
  reserved as RFC-061; it will be chosen against real Devanagari, Arabic, Hebrew and Vietnamese text, not against
  a stress string.
- **Everything `0.24.0` listed still holds.** That includes the keyboard-only explorer, no file watcher, and
  there being **no screen-reader support**.

## 0.24.0 - The Sidebar Is A Tree

Status: **released on 2026-09-24.** Published to crates.io as `tekstide 0.24.0` and
`tekstide-core 0.24.0`, tagged `0.24.0` on `5b33f5a`.

The file explorer stopped being one-level debug text and became a tree you can read: folders open in place,
a row says what it is before its name, and nothing it cannot show is hidden without a row saying so. The same
release reworks the Project Board's cards at the owner's request. Both were checked by running the release
binary, and three defects in them were found that way and by no unit test.

### Changed — the explorer is a tree

- **Folders open and close in place.** `Enter` on a folder expands it under its name, indented; a change
  inside `src/` is visible without stepping into `src/`. The "go up" row is gone.
- **Opening a folder never freezes the window.** Each folder is read on a background thread; until it
  arrives its row says *Loading…*. Measured on a folder of 100 000 entries: the window does about 0.02 ms
  of work for it, and the read itself happens elsewhere.
- **Nothing is hidden silently.** A folder over 256 entries shows the first 256 and a row naming how many
  more there are ("44 more entries not shown."), and says "at least" if it stopped counting. The sidebar
  draws only the rows that fit, and a line says which ones ("Rows 18–60 of 272").
- **A row says what it is before it says its name.** `[FILE] [untracked] new.md`, not `[FILE] new.md
  [untracked]`: the sidebar is narrow and clips the end of a row, and the end used to be where the Git and
  link status was. The line under the tree shows the highlighted row whole.
- **Every name is still escaped, at every depth.**
- **Folders are listed first**, then files, each by name without regard to case.
- **A folder and a file have icons** (`▣ ▢` for a folder, closed and open, `▫` for a file) in place of the
  `[DIR]`/`[FILE]` words. They are text symbols, not emoji: emoji cost six times as much to lay out and need
  a font many machines lack. **Every status is still a word**; the icon carries only what the row's position
  and name already say. A link or special entry keeps `[OTHER]`.
- **The explorer is drawn in a monospaced font, and the sidebar is wider** (300 px, from 220), so depth, the
  `[+]`/`[-]` markers and the icons line up from row to row and a row's width is a column count.
- **The file open in the editor says `[open]`**, so the keyboard highlight (`>`) and the open file are two
  separate, readable things without relying on colour.

### Changed — the Project Board

- **The cards scroll, and follow the keyboard.** With more projects than the window is tall the last card was
  cut through — its "Open" button included — and the board said "15 projects" while showing three.
- **The open project's card says "Open now"** instead of being the one card with no button.
- **A card reads who, then state, then how much.** The name is a heading; trust, branch and attention, and
  each count, are **badges** — a word in a pill, never a replacement for one — that wrap in a narrow window.
  A project that needs attention gets a heavier border *and* says so in words.
- **A project that is not open says so once**, instead of five "unknown" badges: it has no session to count,
  so one badge says "Not open: counts appear once it is opened" and the counts it cannot know are left out.
- **The list of blocked automations is shown on the highlighted card**, not repeated on every card (the count
  stays on all of them).

### Corrected

- **`0.23.0` said the Project Board is "hard to read" and that no scheduled RFC covers it.** That is no
  longer true: at the owner's request the board was reworked in `0.24.0` (see above), unplanned and outside
  any RFC. `0.23.0`'s other limitations still hold. **A reader who took that limitation as a standing fact
  about the board should look at the board again** — its cards, badges and scrolling are different — and read
  the *What this release does not do* section below for what it still does not do.

### What this release does not do

Re-read against what RFC-052 and the board rework changed, not copied from `0.23.0`.

- **The explorer is keyboard-only.** `Enter` opens or closes a folder or opens a file, `Up`/`Down` move.
  A click does not select or toggle a row, as in the explorer it replaced; that is a new interaction with its
  own decision, recorded in `future-work.md`.
- **The explorer does not watch the disk.** A folder is read when you open it; a file created afterwards
  appears when you close and reopen the folder. `.gitignore` handling, an `ignored` badge and a hidden-file
  toggle are the next slice; until then `.git`, `node_modules` and `target` are labelled *(collapsed)* and can
  still be opened.
- **Two icons, not a set.** A folder and a file, as text symbols. Per-file-type icons need an icon font,
  which is an asset and a dependency decision. On a machine without a font for those symbols the icon is a
  blank box; everything the row says is still in words.
- **A row wider than the sidebar is clipped on the right** (the detail line under the tree shows the
  highlighted row whole), and the tree is drawn in a monospaced font at a fixed width. It is not only long
  *names*: a row whose status words are long — a blocked symlink reads
  `[OTHER] (blocked) [symlink escapes root]` before its name — can be cut before the name appears at all.
  Highlight the row to read it.
- **The board's scroll follows the highlight approximately**: it moves to the highlighted card's fraction of
  the way down the list, and the highlighted card is taller than the others (it shows the blocked
  automations), so the cards below it shift as you move.
- **The board shows the blocked-automation names on the highlighted card only**; every card shows the count.
- **A deleted file still carries no badge in the explorer**, and the cadence rule for Git state is unchanged.
  Both are in `0.22.0`'s entry and both still hold.
- **Everything else `0.23.0` listed still holds.** That includes there being **no screen-reader support**.

## 0.23.0 - What The Window Says Is True

Status: **released on 2026-09-24.** Published to crates.io as `tekstide 0.23.0` and
`tekstide-core 0.23.0`, tagged `0.23.0` on `8326e07`.


Every item below was found by running the `0.22.0` release binary, and every one is the product telling
a user something that was not true, or hiding something they needed.

### Fixed — the window says what is true

- **Terminal mode no longer names an internal RFC.** Its empty state said "RFC-017 adds the terminal
  here"; it now says what the mode is and how to start a terminal. A check over the shipped catalog
  refuses any message that names an RFC or a slice id, so it cannot come back.
- **Change Review's empty state no longer claims the whole project.** It read "No changes have been
  detected in this project" beside a status bar saying "2 changed". It now says no AI CLI run has
  produced changes, and where your own changes are shown.
- **A project you have just opened reports zero terminals and zero agent runs**, not "unknown".
- **The board's "dirty files" is now "unsaved files".** It counts open editor buffers with unsaved
  edits; the status bar's "changed" counts what Git reports. Two facts had near-identical words on
  adjacent surfaces. The attention state those buffers raise says "Unsaved edits" for the same reason.
- **The board names the automations Restricted Mode blocks**, not only how many.
- **Approval History says it is empty before it lists caveats about entries** that do not exist.
- **A terminal is sized to the space it has.** In `0.22.0` a terminal's last rows fell below the visible
  area — `seq 1 200` ended at line 177 in a full-size window — because its size was computed from
  constants for the heights of things around it, and the top bar had grown to three rows without its
  constant following; rows were also counted at a shorter line height than they are drawn with. The size now
  comes from the space the layout gives the terminal.
- **Dialogs keep their buttons on screen.** In a small window a dialog was centred at its full height and
  clipped at both ends — the keyboard reference lost `Close` and "Escape closes this." with no scrollbar
  and no hint. The body scrolls; the buttons and the dismiss hint stay in the window.
- **The status bar moves a field down whole instead of cutting a word.** At a narrow width it wraps onto
  further lines; the layout accounts for the height.
- **The close-project confirmation says "unsaved file", not "dirty file"**, matching the board.

### What this release does not do

Re-read against what RFC-053 changed, not copied from `0.22.0`.

- **The Project Board's row is truthful and still hard to read.** It is now eleven stacked lines for one
  project, a bare mood word ("Calm") and a list of blocked automations that wraps. This release made it
  say what is true; making it legible is later work that no scheduled RFC covers.
- **A deleted file still carries no badge in the explorer**, and the cadence rule for Git state is
  unchanged. Both are in `0.22.0`'s entry and both still hold.
- **Everything else `0.22.0` listed still holds.** That includes there being **no screen-reader
  support**.

## 0.22.0 - The Window Knows What Git Knows

Status: **released on 2026-09-23.** Published to crates.io as `tekstide 0.22.0` and
`tekstide-core 0.22.0`, tagged `0.22.0` on `c6a7449`.


### Added — the status bar and the file explorer both read real Git state

- **Branch and dirty state on the status bar**, read when a project opens and again whenever a
  process Tekstide launched for that project ends — a terminal or an AI CLI run closing. Read only
  for a repository whose configuration names nothing that could run a program; everything else
  reads branch-only or "not available" rather than guessed at (RFC-030's own safety gate).
- **The Project Board's own branch label now reads the same state.** It used to read "not
  available" unconditionally, contradicting the status bar in the same frame.
- **Per-file Git status in the file explorer** (REQ-GIT-003): each changed file is marked
  modified, added, renamed or copied, untracked, or in conflict, next to its name. A wholly new,
  entirely untracked folder is marked as a whole; the files inside are not marked individually
  until something inside is tracked or staged.
- **The literal rule, stated plainly**: a change is reflected only once the process that could
  have made it has ended, or the project is reopened. A `git commit` typed into a Tekstide
  terminal you leave running is **not** reflected while that terminal stays open — the same as a
  commit made outside Tekstide entirely. Deliberate, not a bug: it costs nothing when nothing is
  running, and needs no background timer (RFC-030 PR-030-B, reviews 410-411).

### Fixed — `git` is found beyond a fixed `/usr/bin:/bin`

Git detection used to search only `/usr/bin` and `/bin` for `git`, so every project silently read
"not available" on a distribution that installs it elsewhere — NixOS and Guix put it in a per-user
or per-system profile, not a rare setup. It now tries a small reviewed list of system directories
first, then the user's own `PATH`, filtered so a repository can never redirect which `git` binary
gets invoked (RFC-030 PR-030-D, reviews 414-415).

### Corrected

- **`0.21.0` said Git state reads "not available".** That was true when written and is no longer
  true: the status bar and the explorer both read real branch, dirty, and per-file state now,
  whenever the repository can be read safely. A reader who took "not available" as a standing
  fact about this project should re-read the *Git* section of
  [What works today](https://nabbisen.github.io/tekstide/users/what-works-today.html).

### What this release does not do

- **A deleted file carries no badge in the explorer.** It is a real status Tekstide reads and
  counts — the status bar's own changed-file count includes it — but the explorer only ever shows
  files that still exist on disk, and a deleted file has none left to show a badge on.
- **The cadence limitation applies to the explorer's per-file badges too, not only the status
  bar**: a change is reflected only once the process that could have made it has ended, or the
  project is reopened.
- **Git-based change detection still does not feed the Change Review surface.** Unrelated systems
  today.
- **Pending approvals can only ever read zero today.** Carried from `0.21.0`, unchanged this
  release: the field is correct and wired to a real count; there is simply no shipping AI CLI that
  speaks RFC-021's approval protocol.
- **Everything else `0.21.0` listed still holds.**

## 0.21.0 - The Bar Says What Is Happening

Status: **released on 2026-09-22.** Published to crates.io as `tekstide 0.21.0` and
`tekstide-core 0.21.0`, tagged `0.21.0` on `f4c608f`.


### Added — the status bar now says what is happening in the active project

Until now the status bar showed only its own one-line summary. It now also carries the four fields
REQ-NOTIFY-002 asks for, on the same line, present only when a project is active.

- **Trust state** — Trusted, Restricted, Revoked or Unknown — read straight from the project, not
  inferred.
- **Git state** — honestly *"not available"*. Nothing produces real Git state yet; RFC-030 is what
  will, and this release does not pretend otherwise.
- **Running, failed and pending-approval sessions**, each as an actionable label — `2 running`,
  `1 failed`, `1 awaiting approval` — never a bare count (REQ-NOTIFY-003), and **absent entirely at
  zero** so the bar stays readable rather than padded with zeroes.

### Changed — the four board notices now share one model

The Project Board's audit-health, configuration, recent-project-list-repair and transcript-retention
notices used to be four independent string-building functions. They now project from one
`Notification` type, ordered deterministically by kind rather than by which producer happened to
resolve first. **No notice's rendered text changed**, and every one of their twenty-one pre-existing
tests moved onto the new model unmodified — this is an internal migration, not a user-visible change,
and the changelog says so rather than implying otherwise.

### What this release does not do

Re-read against what RFC-025 changed, not copied from `0.20.0`.

- **Git state reads "not available".** Nothing in this release computes it; RFC-030 (already
  accepted, still in progress) is what will, once its own safety evidence is complete. Not a bug —
  the honest answer until something real produces the field.
- **Pending approvals can only ever read zero today.** The field is correct and wired to a real
  count; there is simply no shipping AI CLI that speaks RFC-021's approval protocol, and the
  `Managed` compatibility profile it would require is unreachable from configuration. A user who
  never sees this label move deserves to know why, rather than wonder if it is broken.
- **Everything `0.20.0` listed still holds.**

## 0.20.0 - The Ending Is Recorded, And The List Survives

Status: **released on 2026-09-16.** Published to crates.io as `tekstide 0.20.0` and
`tekstide-core 0.20.0`, tagged `0.20.0` on `26b5c58`.


### Fixed — a damaged recent-projects list no longer takes your trust decisions with it

**Before this, a `recent-projects.json` Tekstide could not read was replaced by an empty list saved
over it.** Every project then reopened under a new identity: trust had to be granted again, and the
transcripts of those projects belonged to nothing (they are shown, but no purge reaches them). If the
file could not be *parsed* it was at least moved aside first; if it could not be *read*, it was
overwritten with no copy kept.

- **Tekstide now keeps one previous-good copy**, `recent-projects.json.bak`, written whenever it
  saves — and **only** when the list it is saving really came from a file it read, so a session that
  started empty can never overwrite the good copy with an empty one.
- **A damaged list is restored from that copy, with its identities intact**, which is what makes
  trust re-verifiable and keeps transcripts attached to their projects.
- **A file that cannot be read is moved aside first**, exactly as an unparseable one already was. If
  it cannot even be moved, **nothing is saved at all** this session: your unreadable file is worth
  more than our empty one.
- **The project board says which happened** — restored from the last saved copy, or reset with
  nothing to recover. They are different sentences, and you should not have to guess.
- **Recovery restores identities, never trust.** Every restored project is still re-checked against
  the audit store, and one with no recorded grant is demoted to Restricted.

**What this cannot do:** a list already lost, before this release, has no copy to come back from.

### Removed — `RecentProjectStore::load`, superseded the day it gained a replacement

`load_or_recover` does everything `load` did and the recovery besides, so `load` was left with no
caller outside its own tests — the dormant-capability shape RFC-036 closed, created fresh by the
change that superseded it. Deleted rather than kept for a caller that may never come, along with the
`RecentProjectStoreError::CorruptState` variant, which nothing produced once `load` was gone.

**This is a breaking change for anything depending on `tekstide-core` directly.** Call
`load_or_recover`, which returns a `RecentProjectLoad` — the list, plus whether it was loaded,
recovered from the backup, or reset with nothing to recover. `load`'s four tests moved onto it
unchanged in what they prove.

### Added — the audit trail now says how an AI CLI run ended

Until now the trail could say a run was **launched** and never that it **finished**. The record for
it existed in the schema from the beginning and nothing wrote one.

- **An ending is recorded** when a run's process ends: that it ended, and how — the process exited,
  it was terminated, or the runtime failed. **Never an exit code, a signal number, or any text the
  run produced**; the schema has no field for those, so none can reach it.
- **A run Tekstide stopped supervising has no recorded ending, on purpose.** If it loses track of
  the process — a detached run — nobody observed an ending, so the trail stops at the launch and
  stays there. A record claiming an ending nobody saw would be worse than the silence, and it would
  outlive the session that could explain it.
- **The trail still does not say whether a run is going on right now.** It records what has already
  happened, not what is happening, and this release does not change that.
- A failure to write the record never stops a run from being terminated.

### What this release does not do

Re-read against what these two RFCs changed, not copied from `0.19.0`.

- **A detached run's ending is not recorded.** If Tekstide loses track of an AI CLI process, its
  trail stops at the launch and stays there: nobody observed an ending, and a record claiming one
  would outlive the session that could explain it. Deliberate, and the reason the rest of the trail
  can be trusted.
- **The trail still does not say whether a run is going on right now.** It records what has already
  happened.
- **A recent-projects list lost before this release cannot be recovered.** There was no previous-good
  copy to keep, so a reset that already happened stays a reset — those transcripts are shown, and no
  purge reaches them.
- **One previous-good copy is not a history.** Two consecutive bad saves take the good copy with
  them.
- **Project ids are still not derivable from the folder.** An id that is lost with no backup cannot
  be re-derived from the path, so such a reset still orphans that project's transcripts.
- **Everything `0.19.0` listed still holds**: no screen-reader support; command approval exercisable
  only by the reference adapter; no before/after diff and no undo; terminal latency unverified; plain
  terminals unrecorded; and the byte budgets gate new capture rather than acting as a hard ceiling.

## 0.19.0 - Purge Reaches What You Actually Have, And Retention Finally Acts

Status: **released on 2026-09-16.** Published to crates.io as `tekstide 0.19.0` and
`tekstide-core 0.19.0`, tagged `0.19.0` on `da66381`.


### Fixed — purge did not reach transcripts from earlier runs, `0.12.0` through `0.18.0`

**From `0.12.0` through `0.18.0`, purging a project's transcripts removed only those from runs launched
since the project was opened.** Tekstide never read transcripts back from disk. After a restart, or
after closing and reopening a project, Trust Settings counted only the newer runs, and the purge
confirmation could say it would delete *0 transcripts (0 bytes)* while every transcript from an
earlier run stayed in `$XDG_STATE_HOME/tekstide/transcripts/` (`~/.local/state/tekstide/transcripts/`
if `XDG_STATE_HOME` is unset). **Deleting that directory was the only complete removal.** If you
purged in any of those releases, transcripts from earlier runs may still be there.

**Now**, when a project opens, Tekstide loads the transcripts earlier runs left for it (RFC-050), so
the retained count and purge cover them. The count also no longer includes transcripts already purged.

**What purge still leaves in place:**

- a transcript that was still being written when the project opened — by a second Tekstide on the same
  state directory, for example. It is never deleted from under its writer.
- transcripts whose project is no longer in the recent list, for example after `recent-projects.json`
  could not be read. They belong to no project, so no project's purge reaches them. Deleting
  `transcripts/` removes them.

**What you will now see:**

- **Trust Settings** says how many bytes under the transcripts directory no purge will delete —
  transcripts belonging to no project in the recent list, and files Tekstide does not recognise —
  and where they are, when there are any.
- **The purge dialog** says when a file was still being written when the project opened and will be
  left, and when a transcript it deletes may belong to a run still in progress — the transcript is
  deleted either way, and deleting it does not stop the run.
- **The project board**, on a start where `recent-projects.json` could not be read, says the list was
  reset, how many bytes of earlier transcripts remain and where, and where the unreadable file was
  moved. It says nothing on any other start.

### Added — transcripts are now removed when they pass the retention age

**`transcript_retention_days` is enforced.** Until now Tekstide recorded the value, checked it for
validity, and never acted on it; RFC-011 specified this cleanup and it had no implementation.

**The first run after upgrading will delete every transcript already older than the configured age** —
with the default of 30 days, that is every transcript from more than a month ago. Nothing warned about
them before, because nothing was removing them.

- **Two moments, both of which you cause:** opening a project, and launching an AI CLI run in it.
  **No timer, no watcher, no background sweep.**
- **The project board says what was removed**, and says separately when a transcript could not be
  deleted — that one has a remedy, and the cleanup will stop at it again until it is taken.
- **A transcript a run may still be writing is never removed**, at any pressure. When that leaves the
  byte budgets over their limits, the next run **starts without a transcript** rather than deleting a
  live one: the launch confirmation says so before you start it, and the run's AgentRun Report says
  why it has none.
- **Policy removals are audited as the product's own act** (`AppPolicy`/`ExplicitCleanup`), never as a
  purge you clicked. A cleanup that removed nothing records nothing.
- The per-project and app-wide byte budgets are enforced **at launch** — no new capture begins while
  one is exhausted. They are not a hard ceiling: runs already capturing continue to their
  per-transcript limit.

### Changed — `[agent] transcript_retention_days = 0` is now refused

`0` was accepted and then ignored. It reads as *no age limit at all* inside the product, which is
the opposite of what someone writing it would expect, so a file containing it now starts Tekstide
with built-in defaults and a diagnostic naming the key — the same handling as any other invalid
value. **To keep no transcripts for a project, decline transcript capture for it in Trust
Settings.** Any other positive number is unchanged, and age-based removal still reads none of them
yet.

### Added — a user guide, and a README that is no longer a manual

**The documentation is published**: <https://nabbisen.github.io/tekstide/>. Six chapters written for
someone using Tekstide rather than building it — installing and first run, the keyboard, projects
and trust, AI CLI runs, local data and privacy, and configuration — built with mdBook and published
by CI on every push to `main`.

**`README.md` is 123 lines, down from 561.** It was a manual: keybinding tables, file layouts and
policy explanations that went stale in place, on the page crates.io renders for the `tekstide`
crate. What is left is what a reader needs before they install anything — what this is, what it
does not do, how to run it, where the data lives — and every link in it is **absolute**, because
crates.io resolves a relative link against the crate's own directory, where it 404s.

**`CONTRIBUTING.md` exists**, with the gate loop this project actually runs.

### What this release does not do

- **The byte budgets gate new capture; they are not a hard ceiling.** No new run begins capturing
  while a budget is exhausted, but runs already capturing continue to their own per-transcript
  limit — so a budget can be exceeded by up to *(capturing runs × 32 MiB)* until those runs end.
  Mid-stream enforcement needs byte accounting shared across writers in different terminals and is
  reserved, not built.
- **A detached run's transcript is never reclaimed.** Tekstide stops being able to tell whether such
  a run is still writing, so retention treats it as live for good and never deletes it. Deliberate
  under §2 of RFC-049's own rules — the alternative is deleting a file a live process may hold —
  and the cost is that those bytes stay until you remove them yourself.
- **Transcripts belonging to no project are shown, not deletable in-app.** After a
  `recent-projects.json` that could not be read, every project reopens under a new id and its
  earlier transcripts belong to nothing. Trust Settings says how many bytes and where; no purge
  reaches them. **Deleting the `transcripts/` directory is what removes them.**
- **A `recent-projects.json` that cannot be read loses that project's trust decisions**, not only
  its place in the list. The file is moved aside, the project board says so and says where it went,
  and trust must be granted again. *(Repaired in the next release: see the Unreleased entry above.)*
- **No screen-reader support.** Unchanged and stated every release: `iced` offers no accessibility
  bridge, so there is no partial support to describe.
- **Command approval is exercisable only by the reference adapter.** No shipped AI CLI speaks the
  structured-approval protocol yet.
- **No before/after diff, and no undo.** The before-bytes were never captured; blocked on
  Git-backed content, not on review UI.
- **Terminal latency is unverified.** No measurement of the real interactive path exists.
- **Plain terminals are not recorded.** Only AI CLI runs write transcripts.

## 0.18.0 - The Configuration File Does Something

Status: released on 2026-09-12.

**Before this release, Tekstide never read your configuration file.** RFC-023 shipped a complete
configuration system — path resolution, parsing, validation, diagnostics, a security-sensitive
reload rule — and nothing in the application constructed any of it. You could write a valid
`config.toml` and the product would behave exactly as if you had not. RFC-045.

### Added — a configuration file that is read, and says so when it is not

- **Tekstide reads `$XDG_CONFIG_HOME/tekstide/config.toml` at startup.** A missing file is normal.
  An unreadable or invalid one still starts the application with built-in defaults — and **the
  project board now says so**, naming the key that broke the file, rather than leaving you to
  wonder whether it was read. The line is absent when there is nothing wrong.

- **What a configuration file can change today**: which AI CLI profile the launch button runs
  (`[agent] default_profile`, with the profile defined in `[agent.profile.<id>]`), and how many
  agent runs a project allows (`[resources] agent_run_limit`).

- **`[agent] transcript_retention_days` sets the retention limit recorded on each launch's
  transcript policy** — recorded, and checked for validity. **No age-based purge reads it yet**:
  transcripts are not kept for N days and then removed, and nothing in this release makes them be.
  The only purge is the manual, per-project one that has existed since `0.12.0`. The value is stored
  where an age-based purge will read it when one is built (`rfcs/future-work.md`, RFC-011).

- **`Ctrl+Alt+C` re-reads the file** without restarting. RFC-023 specified "a command or API call"
  and only the API call existed.

### Changed — every other key is refused by name, not quietly ignored

RFC-023's schema defined roughly thirty keys, and **none of them had a consumer**: fonts, theme,
keybindings, scrollback, concurrency limits, and more were parsed into fields nothing read. A
profile's `args` were parsed and dropped, so `args = ["--model", "x"]` validated, appeared, launched,
and silently did nothing.

**A key the file accepts and the product ignores is a lie you read**, so each of those keys is now
**refused by name**, with a diagnostic saying it has no effect yet. Each returns to the file in the
same change as the code that reads it — not before. A key this version has simply never heard of (a
typo, or a key from a newer Tekstide) still loads with a warning, and the board names it, because
that is the case where nothing is wrong with the file itself.

Three keys are refused permanently rather than "not yet", and say why: configuration cannot grant
workspace trust, cannot disable multiline-paste confirmation, and cannot disable destructive-command
approval. Those are decisions per-use acts exist to make, and no file may make them in advance.

**If you already wrote a `config.toml`** — against RFC-023's published schema, since `0.12.0` said
plainly that writing one had no effect — **it is now refused rather than ignored, and the board says
so.** Nothing you set was taking effect before and nothing is taken away now; what changed is that
you are told. **To fix it: delete the key the board names, or delete the file.** The four keys above
are the ones worth keeping. The file lives at `$XDG_CONFIG_HOME/tekstide/config.toml`, or
`~/.config/tekstide/config.toml` when `XDG_CONFIG_HOME` is unset; Tekstide never writes it.

### Added (security) — nothing from a configuration file runs without a deliberate act

- **The first launch of a configuration-defined AI CLI asks first**, and the confirmation names the
  **executable path it actually resolved to** and the file that defined it — never only the display
  name, which is text the file itself supplies. A profile calling itself "Claude Code" while
  pointing somewhere else is exactly what this is for. Once per session, not once per launch.

- **A reload that weakens a security-relevant setting waits for the same kind of confirmation.** One
  that tightens applies immediately — tightening never needs permission. Changing a profile's
  definition re-arms the first-use question, because the same name now means a different executable.

- **Confirming is recorded** in the durable audit trail: that a sensitive setting was weakened, and
  that it was authorized. The record says the direction, never which setting or what value — there
  is no field on it that could.

### What this release does not do

- **`transcript_retention_days` is recorded, not enforced.** Said once above and once here because
  it is the claim most likely to be read as more than it is: there is no age-based purge, and
  nothing in this release makes one.

- **There is no settings GUI, and there will not be one soon.** The file is the interface; the
  board line and the two confirmations are the whole surface. Configuration is **not** re-read
  automatically when the file changes — `Ctrl+Alt+C` is the only way, until the file watcher lands.

- **Workspace configuration is not loaded.** Only defaults and your user-global file; a
  `.tekstide/config.toml` inside a project is not read, deliberately, and a cloned repository
  cannot configure anything.

- **Screen-reader support does not exist.** Unchanged, and stated every release: `iced` offers no
  accessibility bridge, so there is no partial support to describe. Re-checked this release
  (`cargo tree -p tekstide | grep -i accesskit`, empty).

### Dependencies

Maintenance, alongside the feature above: 72 in-semver dependency upgrades, and one manifest edit
that shrank the dependency graph by two crates. Scheduled deliberately after RFC-047 and RFC-046
closed, so a storage-engine change would not arrive entangled with the corruption-recovery and
audit-trail behaviour those RFCs were still writing. Handoff:
`rfcs/handoffs/dependency-currency-0.18.md`.

- **72 packages upgraded, 2 removed, 1 added, 0 downgraded** — a plain `cargo update` plus the
  `rusqlite` manifest edit below, checked as one before/after span rather than per step (the first
  pass checked only the `cargo update` half and missed that the manifest edit is what shrinks the
  graph — corrected before this line shipped). Checked explicitly for the one failure mode that
  matters and does not show up in a diff count: a lockfile shrink re-resolving a transitive
  dependency *downward* (the mechanism another project on this team hit, Windows-only, invisible on
  Linux). Confirmed by comparing every `(name, version)` pair across the whole change: nothing moved
  backward, and the 29 `windows*` crates plus `gpu-allocator` this workspace carries but never
  compiles (via `iced` → `wgpu`) are byte-for-byte unchanged.
- **`rusqlite` `0.39.0` → `0.40.2`.** Zero Rust code changes — it is a non-event at the API level.
  The two removals above are `0.40`'s own dependencies (`sqlite-wasm-rs` and, behind it,
  `rsqlite-vfs`) falling away, not anything this project depends on directly. What actually moved is
  **bundled SQLite `3.51.3` → `3.53.2`** (`libsqlite3-sys` `0.37.0` → `0.38.2`), a storage engine two
  minor versions newer underneath a durable, security-relevant audit store. `cargo build` has
  nothing to say about that; RFC-047's own corruption-detection fixtures — which depend on exactly
  how SQLite reports a damaged file, not a stability-guaranteed interface — are what actually
  licenses this bump, and they pass. `NOTICE` (root, `crates/tekstide-core`, `crates/tekstide`)
  updated to the resolved versions.
- **Checked directly: a store written by the new engine survives a downgrade back to the old one.**
  A real audit store created and populated under `rusqlite 0.40.2`/SQLite `3.53.2` opened and read
  back correctly under `rusqlite 0.39.0`/SQLite `3.51.3`, and the reverse direction succeeded too.
  This was the one risk the currency check itself could not cover — a user who downgrades Tekstide
  must still be able to read their own audit history.

## 0.17.0 - The Audit Store Says When It Cannot Record, And Records The Launch

Status: released on 2026-09-10.

Two RFCs that belong together. **Before this release, a corrupted audit store failed completely
silently, and launching an AI CLI agent was never recorded at all.** Both were found by RFC-036's
reachability audit, which reproduced the first against the release binary and watched the interface
report "Calm".

### Fixed — a broken audit store now says so, recovers, and says where the old file went

- **A corrupted audit store no longer fails silently.** It is detected, the unreadable database is
  moved aside (renamed, not deleted), a fresh one is started, and the project board says so —
  naming the directory the old file was moved to. Previously: no log line, no indicator, no dialog,
  and every audit-writing action quietly doing nothing for the whole session. RFC-047.

- **An interrupted migration resumes** instead of leaving the store permanently unopenable.
  RFC-047 D1.

- **A degraded audit store does not refuse anything.** Launching an agent run and granting workspace
  trust say, before the click and while the control is still live, that the action will not be
  recorded. Restricted Mode refuses actions whose *danger* it cannot bound; a broken audit store
  does not make an agent run more dangerous, only unrecorded — so the product says so and lets the
  person decide. RFC-047 D4.

- **The indicator tracks what is true now, in both directions.** A transient failure no longer
  leaves the board claiming "not recording" for the rest of the session once recording has
  recovered; and a session that had failures still says so, separately, after capability returns.
  Found during RFC-047's own review, not planned. RFC-047 PR-047-D.

### Fixed — launching an AI CLI agent is recorded

- **An agent-run launch now writes a durable audit record**: authorized, then started or failed,
  tied by one operation id. **It wrote nothing before.** This is the action workspace trust and
  command approval exist to control, and `crates/tekstide-core`'s own README claimed it was
  recorded until 2026-08-27, when RFC-036 corrected it. RFC-046.

- **The record is never a precondition for the launch.** A degraded audit store makes a run
  unrecorded, not refused — the same rule as above. The trail can therefore be *absent*, but it can
  never be *inconsistent*: a "started" record whose "authorized" never persisted is rejected by the
  store itself. RFC-046 D1.

### What this release does not do

- **It does not record how an agent run ends.** The trail answers *was a run launched, when, and
  under which adapter profile* — not *is it still running* or *how did it end*. Three separate
  paths already end a run, and a fourth record covering all of them would be wrong in a way nobody
  would notice. Reserved as RFC-048, not written. RFC-046 D3.

- **It does not record refused launches**, beyond the Restricted Mode refusal already recorded. A
  security control refusing is evidence it worked; a limit a user configured for themselves
  refusing their own action is not. RFC-046 D5.

- **Screen-reader support does not exist.** Unchanged, and stated every release: `iced` offers no
  accessibility bridge, so there is no partial support to describe.

## 0.16.0 - What The Reachability Audit Found, Two Sessions Later

Status: released on 2026-08-28.

### Fixed — a keyboard-only user can close a project, and every surface-local key is advertised

- This release's own gate found that a keyboard-only user could not close a project — the `×`
  on a project tab was the only thing that emitted a close, and no key reached it, so a
  keyboard-only user could not reach the termination behaviour the previous release was largely
  about. That is no longer true. `Delete`, with a project's own tab highlighted, closes it — the
  same `attempt_close_project_tab` the `×` button already reached, so both routes converge on the
  same confirmation dialog for a project with live work. RFC-044.

- `a`/`r` here, and `Enter`/`Space`/`Delete`/arrows elsewhere, appeared in neither the Help modal
  nor `--help`. They do now: a surface-grouped section, generated from the same registry that
  drives the fix above, lists every surface-local key by the surface it belongs to. RFC-044.

### Removed — dormant, dead public API from `tekstide-core`

RFC-036's reachability audit found capabilities with no production caller anywhere and no named
future consumer (its own D2 rule: an RFC number, not an intention). Batched into one release
rather than trickled, per that RFC's own D1 decision. Each removal, individually, because a
consumer's build breaking is the loudest thing this project can do to someone:

- **`ProjectSession::add_agent_run`** — superseded by `attach_agent_launch_plan`, the real
  production attachment path (which also enforces `agent_run_limit`, which this never did).
  Narrowed to `#[cfg(test)]` rather than deleted outright: a dozen `tekstide-core` tests use it as
  fixture setup unrelated to the function's own behavior, and rewriting each onto
  `attach_agent_launch_plan`'s heavier `AgentRunLaunchPlan`/`TerminalSession` ceremony would have
  coupled every one of them to launch internals for no reason relevant to what they check.
- **`ProjectSession::add_transcript`** — superseded by the private `attach_agent_run_transcript`,
  the real production attachment path. Narrowed to `#[cfg(any(test, feature = "test-support"))]`
  for the same fixture-reuse reason as `add_agent_run`, including one `tekstide`-crate test.
- **`ProjectSession::add_audit_event`** — superseded by `grant_trust`/`revoke_trust`
  (`pub(crate)`), the real writers of the same collection. Removed outright; its own six tests
  were entirely about its own rejection behavior, not fixtures for anything else.
- **`AgentRun::add_approval`** — superseded by `ProjectSession::add_approval_request`, the real
  approval-attachment path. Removed outright. Checked, not assumed: the field it wrote
  (`approval_ids`) is read nowhere either, so nothing was silently relying on stale data.
- **`ServeShutdown::shutdown`** (`approval::channel`) — calls the identical private helper
  `Drop` already calls; kept no determinism production ever needed. Removed outright; the one test
  that depended on the explicit method specifically was already redundant with a stronger sibling
  test proving the same cleanup via `Drop` alone.
- **`ApprovalChannelEndpoint::accept_proposal`** — superseded by `serve_concurrently`'s own
  internal accept loop, the real production path. Narrowed to `#[cfg(test)]`: roughly a dozen
  tests use this single-connection, single-thread shape as their primary vehicle for testing
  authentication and frame parsing directly, without `serve_concurrently`'s multi-threaded
  machinery in the way.
- **`ProjectSession::record_terminal_transcript_write_summary`** and
  **`ProjectSession::record_transcript_write_summary`** — RFC-033 PR-033-C deliberately built a
  different mechanism instead (reading real bytes from disk, correct retrospectively, rather than
  a counter that is only ever correct prospectively), so there is no future version of "wire this"
  that would not repeat the mistake RFC-033 already avoided. Removed outright.

`tekstide-core` is on crates.io; this is a breaking change to a published crate, which is why it
is a minor version bump rather than a patch.

### Known limitations, unchanged or newly found

- **No screen-reader support.** `iced` has no accessibility bridge; out of scope for that reason
  and no other.
- **A Managed-compatibility agent-run launch produces no durable audit record.** Found by this
  release's own reachability audit (RFC-036 PR-036-A): a fully-built, tested audit-writing path
  (`launch_managed_agent_run` and three siblings) exists and is never called — production launches
  through a different route that writes nothing to the audit trail. This crate's own `README.md`
  said otherwise until this release corrected it. A fix is recommended as its own RFC, not built
  here, since what to record and when is real design, not a wiring gap.

  **What to check:** if you have been treating the audit store as the record of which AI CLI runs
  were launched on your machine, **it does not contain them and never did.** Trust decisions,
  command approvals, restricted-mode refusals, paste refusals, plain-terminal starts and
  safe-close outcomes are all really there; agent-run launches are not. Nothing on your disk is
  wrong — the records were never written, so there is nothing to remove or repair.

- **A corrupted audit store fails completely silently.** Also found by this release's audit, and
  reproduced against the release binary rather than reasoned about: with a genuinely corrupted
  `audit.sqlite3`, or with an interrupted-migration marker present, Tekstide starts normally, the
  project board reports **"Calm"**, and nothing anywhere indicates the audit trail is broken. The
  recovery machinery this project built and tested (`recover`, `resume`) exists and nothing calls
  it.

  **What to check:** the store lives at `$XDG_STATE_HOME/tekstide/audit/audit.sqlite3`
  (`~/.local/state/tekstide/audit/audit.sqlite3` if unset). If you want to know whether yours is
  readable today, `sqlite3 <that path> 'pragma integrity_check;'` answers it — the application
  will not. A fix is recommended as its own RFC: resuming a safe, interrupted migration is small,
  but discarding a user's audit history to recover from real corruption is a decision that should
  be made deliberately rather than by default.
- **Still no two-sided diff.** The before-bytes were never captured; blocked on Git-backed
  detection.
- **A review decision does not survive closing Tekstide.**

## 0.15.0 - What The Terminal Started, And Who Could See It

Status: released on 2026-08-27.

**Security release.** Every shell this application launched inherited the PTY master file
descriptor of every other terminal open at that moment. A PTY master is read/write access to that
terminal, so code running in one project's terminal — including an AI CLI agent, which is what
this application exists to run — could read what another project's terminal displayed and inject
input into it. Fixed. Details and what to check are under **Security** below.

Also: closing a terminal now actually ends what that terminal started, a previewed file is
readable rather than one escaped line, and you can record a decision about a change set.

### Security — cross-terminal descriptor access (all releases through `0.14.0`)

- **What was wrong.** `openpty()` does not set close-on-exec, and nothing added it. At `exec`,
  every child inherited **every PTY master descriptor open in the parent** — not only its own. A
  live process was measured holding **27** of them.

- **What that allowed.** Anything running inside a Tekstide terminal could read the output of, and
  write input to, every other Tekstide terminal open when it started. No capability check, no
  audit record, nothing on screen. This crosses the terminal security boundary RFC-009 defines.

- **What it did not allow.** Nothing outside Tekstide's own terminals, and nothing across user
  accounts — the descriptors were only ever those of terminals this application itself opened in
  the same session.

- **The fix.** `FD_CLOEXEC` on the PTY master and slave at creation, plus a `close_range` sweep of
  everything above the child's own stdio before `exec`. **Either alone closes the hole** — proven
  by ablating each independently — so the second is a real net, not decoration.

- **What to check.** There is nothing to clean up on disk and no record to inspect; the exposure
  was between live processes. **If you ran code you would not fully trust — an AI CLI agent, a
  build script from an unfamiliar project — in a Tekstide terminal while another terminal was
  open, treat what was on those other terminals as potentially read.** In practice that means
  anything typed into them, secrets included. If you only ever ran your own commands, the exposure
  is theoretical. If you are unsure, `0.15.0` removes the possibility going forward.

### Fixed — closing a terminal now ends what that terminal started

- **`0.14.0` shipped this as a known limitation** — *"a backgrounded job survives closing the
  terminal that started it."* **That is no longer true**, and this is the correction to it.

  Closing signalled one process group. An interactive shell puts every `&` job in a process group
  of its own, so those jobs were never in the group being signalled. Termination now hangs up and
  re-scans the whole **session**: close the PTY master, `SIGHUP` the session leader so the shell
  hangs up its own jobs, escalate to `SIGKILL` for anything left, then re-enumerate to confirm.

  **The close confirmation says so before you click**: *"Anything started from these terminals ends
  too, including a backgrounded job."*

- **A process you deliberately detached still survives, by design.** `nohup`, `disown` and `setsid`
  work by leaving the session, and the session is exactly the boundary this fix respects. That is
  your opt-out and it is deliberate, not an oversight — it is also why this kills by session rather
  than by container.

  **If you relied on the old behaviour** — backgrounding a long build with `&` and closing the
  project expecting it to continue — **that no longer works, and `nohup` or `disown` is how to ask
  for it now.**

- **The safe-close audit record can now support a stronger claim, and does.**
  `terminal_process_groups_confirmed_empty` is renamed `terminal_session_confirmed_empty`, and it
  is no longer inferred from the termination outcome — it is read from a real, session-wide re-scan
  performed after the kill. `0.14.0`'s entry told you an `applied` outcome meant less than it
  looked like; it now means what it says, for everything inside the session. A process that left
  the session remains outside the claim, deliberately, and the field's own documentation says so.

### Fixed — a previewed file is readable

- **`0.14.0` shipped the content preview escaping every line break**, so a real source file
  rendered as one continuous run of text with `<U+000A>` between every line. Lines are lines now.

  Every other control character is still escaped — tab, carriage return, ANSI sequences, bidi
  overrides — and file content renders inside its own bordered container, kept in a type the rest
  of the surface cannot draw. A file whose first line reads `Review state: Accepted` cannot
  impersonate the interface around it.

- **The "not a diff" label can no longer be scrolled away from what it labels.** The framing text
  and the file list sit outside the content's own scroll region.

- **Content over 4,000 lines is refused whole rather than truncated**, matching the existing byte
  bound. A partial preview you cannot tell is partial is the trap this surface exists to avoid.

### Added — record a decision about a change set

- **Mark accepted / Mark rejected**, reachable by mouse or by `a` / `r`. The first time anything in
  this application could change a change set's review state.

- **It is a note, and it says so**: *"Marking this here only records your own note about it: it
  changes no file, cannot be undone, and disappears when you close Tekstide."* All three are true
  and all three would otherwise surprise you. Nothing is reverted, staged, or applied — there is no
  before-side to revert to.

- **No audit record is written for a review decision.** Said plainly so the audit store's silence
  is never read as evidence that no decision was made.

### Known limitations, unchanged

- **No screen-reader support.** `iced` has no accessibility bridge; out of scope for that reason
  and no other.
- **Some controls are mouse-only, and closing a project is one of them.** Found by this release's
  own gate, trying to exercise the close path by keyboard: the `×` on a project tab is the only
  thing that emits a close, and no key reaches it. **A keyboard-only user cannot close a project**,
  and therefore cannot reach the termination behaviour this release is largely about. Pre-existing
  since `0.13.0`, not introduced here, and not a reason to hold a security fix — but it is the
  fourth control of this shape this project has had to find, and the next release is scoped to it.
- **Surface-local keys are not advertised.** `a`/`r` here, and `Enter`/`Space`/`Delete`/arrows
  elsewhere, appear in neither the Help modal nor `--help`. Same work.
- **Still no two-sided diff.** The before-bytes were never captured; blocked on Git-backed
  detection.
- **A review decision does not survive closing Tekstide.**

## 0.14.0 - What It Changed, And What This Cannot Tell You

Status: released on 2026-08-26.

**`0.13.0` gave the product a change review surface that could not show you a single line of a
file.** It listed paths, counts and detection status and said so plainly. This release fills that
in — and, in doing so, found two places where change review had been quietly telling users less
than it knew, and one place where the audit trail claimed more than it could see.

### Added — see the content of a changed file

- **A change review file row is now a real button.** Selecting one renders that file's content
  according to what kind of change it was: the whole content for an added file, the current
  content for a modified one, and nothing at all for a deleted one.

  This machinery — gated, bounded, size-capped, reviewed — has existed in `tekstide-core` since
  `0.7.0` and had **zero production callers for six releases**. Nothing needed to be designed
  here; something needed to call it. What was actually missing was the detection result, which
  the code path that creates a change set discarded the moment it was done with it.

- **A modified file's content is labelled "not a diff", in the words the limitation deserves.**
  The surface says: *this is the file's current content, not a diff; it cannot show what changed,
  only what the file looks like now; whether the agent's own edit is still there, was reverted, or
  was overwritten by something else since, this screen cannot tell you.*

  There is no before/after comparison because the before-bytes were never captured. That is
  blocked on Git-backed detection, not cancelled, and the surface says which.

- **A stale baseline refuses, and names why.** If the file has moved on since the change was
  detected, you get a refusal that explains itself rather than content that silently describes a
  different moment.

- **Every line break in the previewed content is shown escaped, as `<U+000A>`, so a multi-line
  file renders as one continuous run of text rather than as lines.** File content is untrusted
  text drawn in trusted chrome — the same rule that escapes a project name or a path — and
  escaping control characters is what stops a file from forging the interface around it. The
  consequence was not designed, though: previewing a 300-line source file gives you a wrapped
  block with `<U+000A>` between every line, not something you would want to read. **The preview is
  honest but it is not yet readable for real source files**, and the surface itself does not say
  so. Found by running the release binary against a four-line file during this release's own
  gate. Preserving line structure while keeping every other character escaped is the obvious next
  step and has not been designed yet; it is a security decision, not a formatting one.

### Fixed — change detection was blind to git hooks

- **`.git/hooks/` and `.git/config` are now watched.** Change detection excludes `.git/`,
  `target/` and `node_modules/` — a deliberate default, and a necessary one: walking this repository in
  full reaches **64,415** entries and **1,506** after those three exclusions. But it meant an agent that installed a
  `.git/hooks/pre-commit`, or redirected `core.hooksPath`, changed something that would execute on
  your next commit and **never appeared in change review at all**. The file explorer collapses
  `.git/` too, so there was no second route by which you would have noticed.

  Everything else under `.git/` — `refs/`, `objects/`, `index` — is still skipped exactly as
  before. A `core.hooksPath` redirect is **not** followed to its target; watching `.git/config`
  reports that the hook location changed, which is the fact that matters, and resolving the
  redirect is separate work that has not been done.

  **If you used the change review surface on `0.13.0` — the only release where it was visible —
  and read it as what an agent run touched, that answer excluded git hooks.** Detection itself has
  had this blind spot since `0.11.0`, when it was first wired. Check `.git/hooks/` and
  `.git/config` in those projects directly: `ls -la .git/hooks/` and `git config --list --local`.
  Nothing on your disk was changed by this release; what changed is that these paths are now
  reported.

- **A large change set no longer reports as no change set at all.** Detection caps the changed-path
  list at 4,096 entries. Over that cap it used to discard the entire list and mark the scan
  incomplete — and an incomplete scan produces **no change set**, which on the surface is
  indistinguishable from "the agent changed nothing." An agent run that touched more than 4,096
  files therefore showed you nothing, silently. It now keeps the first 4,096 and reports the
  remainder as a count.

  **If an agent run showed no change review entry on `0.13.0`, that was not proof it changed
  nothing** — a run over the cap looked identical, and the missing change set was never built at
  all on any release from `0.11.0` on. There is no record to recover; re-check those projects with
  `git status` if it matters.

- **Two kinds of "not shown" are now two separate lines, never one number.** Files omitted because
  the surface showed a shorter list are **recoverable** — they are still in the change set. Files
  omitted because detection capped its own scan are **not** — they were dropped before the change
  set existed, and no larger limit brings them back. Summing them into a single count meant a
  reader could not tell which they were looking at, on a surface whose whole job is deciding
  whether to trust what an agent did.

### Fixed — an audit record that claimed more than it could see

- **`fully_confirmed` on a safe-close audit decision was renamed to
  `terminal_process_groups_confirmed_empty`, because that is all it ever knew.** When you close a
  project with live terminals, Tekstide signals the process group it launched and confirms that
  group is empty. A job you backgrounded inside that terminal (`sleep 30 &`, a build left running)
  gets its **own** process group from the shell, which that signal never reaches and that check
  never looks at. So a durable audit record could say a close was fully confirmed while a process
  that terminal started was still running.

  **No behaviour changed. The predicate is bit-for-bit identical**, and every `applied`/`failed`
  outcome this project has ever written is unaffected — the field was never itself persisted. What
  changed is that the name no longer asserts something the check cannot establish, and the doc
  comment now states both halves: what `true` means, and what it does not.

  **If you closed a project on `0.13.0` and read an `applied` safe-close outcome as "everything
  that terminal started is gone", that was more than the record could support.** Check with `ps`
  or `pgrep` for anything you backgrounded. The audit store is at
  `$XDG_STATE_HOME/tekstide/audit/audit.sqlite3` (`~/.local/state/tekstide/audit/audit.sqlite3` if
  unset); no record in it is rewritten or invalidated by this release, and nothing needs removing.

  Killing sibling process groups a terminal spawned is a **product decision that has not been
  made** — a real terminal emulator often leaves them running on purpose, which is `nohup`'s entire
  reason to exist. It is recorded as open work, not silently pending.

### Documentation

- `crates/tekstide-core/README.md` claimed safe-close audit producers were "defined in the audit
  schema but not yet wired." They were wired in `0.13.0`. Corrected, and split from
  configuration-change producers, which genuinely still have no caller. Found by the release
  gate's own instruction to check **every** README a published crate names, not only the workspace
  root — the third time that instruction has caught something on that specific page.

### Known limitations, unchanged by this release

- **Still no two-sided diff.** Blocked on Git-backed detection (RFC-030), which does not exist.
- **Absence of visible change is not absence of change.** The content preview shows a file as it
  is now, not as the agent left it.
- **Previewed content is not laid out as lines** — see the escaping note above.
- **`core.hooksPath` redirects are reported, not followed.**
- **A backgrounded job survives closing the terminal that started it**, and Tekstide does not yet
  say so on the close confirmation.
- **No screen-reader support.** `iced` has no accessibility bridge; this remains out of scope for
  that reason and no other.

## 0.13.0 - Something To Do, And A Way To Do It

Status: released on 2026-08-25.

**`0.12.1` made the product describe itself. It still could not be used.** A user starting
`tekstide` with no argument saw an honest empty board, a full keyboard reference, and no way to
open a project without leaving the window. Five keybindings were live and every one of them was
the *only* route to what it did.

This release is three RFCs' worth of answer to that: you can open a project, move between
projects, close one, see what an agent changed — and do nearly all of it with a mouse, because
eleven of the thirteen things this application can do now have a control you can see.

### Added — opening and moving between projects

- **A path field on the Project Board**, focused the moment the board is empty, and reachable on
  a populated board via `Ctrl+Alt+O`. A failed path renders a bounded, escaped diagnostic and
  leaves the field editable; it never exits.
- **A folder browser** (`Ctrl+Alt+B`, or the **Browse…** button) — choose a project by navigating
  the filesystem rather than typing a path. `Enter` moves into a folder, `Space` chooses the one
  shown.
- **Reopen a remembered project with one key.** The board already listed recent projects; now
  `Enter` on a highlighted row, or its own **Open** button, reopens it.
- **A project tab strip** in the top bar: which projects are open, which is active, a permanent
  **Projects** tab back to the board, and `×` to close one. `Ctrl+Alt+N` cycles.
- **Closing a project** — the first time this has been possible from the application at all. A
  project with nothing running closes immediately. One with live terminals or an agent run raises
  a confirmation naming **what will be lost** by count, and identifying the project by its
  **canonical path** rather than its display name, so two similarly-named projects cannot be
  confused at the moment it matters.

### Added — seeing what an agent run changed

- **A change review surface** (`Ctrl+Alt+D`, or **Change Review** in Trust Settings). Since
  `0.11.0` this product has detected what an agent run changed and been unable to show it.

  **It renders metadata, not a diff**: file paths, counts, detection status, review state. It
  says so on the surface itself, along with the limits that matter more than the feature —
  detection is conservative and excludes `.git/`, `target/` and `node_modules/`, so a change an
  agent makes in a git hook is not reported. A *truncated scan* and *nothing changed* are
  rendered as different facts, because they are.

### Added — you can use it with a mouse

- **Eleven of thirteen actions now have a visible control**, up from three. Launching a terminal,
  switching modes, saving a file, starting an AI CLI run, opening Trust Settings, Help, the
  AgentRun report, Approval History — all had keyboard-only routes and now have buttons where the
  action applies.
- **Every dialog can be finished or abandoned with a mouse.** Previously all nine were
  keyboard-only for their own decision — several of them *opened* by a button, so a user arrived
  with a mouse and was stranded.
- **A Help modal** (`Ctrl+Alt+K`) reachable from anywhere, including from inside Terminal
  Immersion, where no route to any help existed.

The two actions that remain keyboard-only are deliberate and recorded as such, not overlooked.

### Fixed (security)

- **A project's cached trust label could be restored without confirmation against the audit
  store.** A user who granted a project trust, closed it, and later reopened it by any route
  other than the original command line — retyping the path, browsing to it, or the new one-key
  reopen — got `Trusted` back on the strength of a **user-writable cache file alone**. Fixed at
  all three routes; each now re-confirms against the audit store the way a command-line-opened
  project already did.

### Fixed

- **Tekstide could leak shell processes.** A panicking terminal left its shell running with
  nothing owning it; at scale this exhausted the system's pseudo-terminal pool. Terminals now
  terminate their process group when dropped. **A related gap remains and is disclosed below.**
- **`close_project` could never report a project safe to close** — its resource assessment had no
  way to reach a "known" state, so the capability was unreachable even once wired.
- Two duplicated rows in the README's keyboard reference.

### Breaking changes

- **`ProjectBoardEmptyState`'s `primary_action` and `secondary_action` fields are removed** from
  `tekstide-core`'s public API. They held pre-baked English for two actions — "Add Project",
  "Open from path" — that were never reachable from anywhere. `0.12.1` stopped rendering them;
  this release removes the fields. Any external caller constructing or reading them will not
  compile.

### Known limitations

- **No diff content.** The change review surface shows which files changed, not what changed in
  them. Reading diff content is a separate, unbuilt piece of work.
- **A backgrounded job can outlive the terminal that launched it.** Closing a project terminates
  each terminal's process group — but a job the user backgrounded inside a shell gets its own
  group, so it survives, and the close is recorded as confirmed. Bounded and disclosed rather
  than fixed: what "closing a terminal" *should* mean for a job someone deliberately backgrounded
  is a product decision, not a mechanical one.
- **Two actions remain keyboard-only** by deliberate convention: pasting into a terminal, and the
  path field's own shortcut, whose workflow has a visible route through **Browse…**.
- **A change set produced by a real agent run has never been observed live** — only proven end to
  end in test, and seen on screen via a seeded change.
- Unchanged from `0.12.0`: the configuration system loads nothing, so writing a config file has
  no effect; no screen-reader support; no cross-platform evidence beyond Linux; the real Claude
  Code CLI still never exercised by the test suite; `NFR-PERF-004` still unverified.

## 0.12.1 - You Can See What It Does

Status: released on 2026-08-22.

**`0.12.0` shipped a window that told a new user nothing.** Started with no argument — which
this project's own Quick Start told people to do — it showed an empty Project Board naming two
actions that do not exist, while the nine keyboard shortcuts that do exist were named nowhere
in the running application. This release makes the product describe itself. It does not yet
add the missing action; see *Known limitations*.

### Fixed

- **The Project Board's empty state named two actions that do not exist.** It rendered
  **"Add Project"** and **"Open from path"** as plain text — no button, no handler, nothing
  behind either — from the day the surface landed. Both are gone. The empty state now says how
  a project is actually opened (`tekstide /path/to/project`), which is the truth, and the keys
  in the catalogue naming those two actions are deleted so no surface can render them again.

- **The running application named no keyboard shortcut anywhere.** The string `Ctrl` appeared
  **zero times** in the entire user-facing catalogue while nine bindings were live, so every
  capability the product had was reachable only by a user who had read `navigation.rs` or the
  repository README. The Project Board now lists every binding and what it needs — in both its
  empty and populated states, because help that disappears when you open your first project is
  not help. The status bar carries a standing pointer back to the board.

  The list is **derived from `KeybindingPolicy`**, not written: it cannot drift from the
  bindings the input layer dispatches on, and it cannot advertise the five actions that have
  no working binding. Four of those are `Configurable` with no key — dead rather than pending
  — and one is the reserved command-palette binding with nothing behind it.

- **`tekstide --help` printed `folder does not exist: --help`.** Every argument was treated as
  a project path, so the only documented entry point rejected the universal request for
  documentation. `-h`, `--help`, `-V` and `--version` now work, and `--help` prints usage plus
  the same derived keyboard list. An unrecognised `-`-prefixed argument is still treated as a
  path, deliberately: a real file can begin with a dash.

- **The published Quick Start led users into the dead end.** It said to run `tekstide` bare and
  mentioned passing a path second, as an option, when a path is the only way to put a project
  on the board. Corrected to lead with the path and to state the limitation.

### Known limitations

- **There is still no in-app way to add a project.** A path on the command line remains the
  only route; `tekstide` with no argument opens an empty board that now explains itself instead
  of misleading you. **RFC-038 is the fix** and is proposed, not scheduled — this release is a
  correction, and correcting a product that misrepresents itself came first.
- **A user inside Terminal Immersion still needs to know `Ctrl+Alt+P`** to reach the board
  where the keyboard list lives. A help surface independent of the board is RFC-038's Goal 4.
- **`--help`'s framing sentences are English only.** The shortcut descriptions are
  catalog-driven and localize; the surrounding usage text does not, because argument parsing
  happens before the catalog is resolved. Recorded in RFC-038 rather than left as a surprise.
- **`ProjectBoardEmptyState`'s `primary_action` / `secondary_action` fields still exist** in
  `tekstide-core`'s public API, still holding the pre-baked English for the two actions that do
  not exist. Nothing reads them. Removing them is a breaking change and belongs to RFC-038.
- Unchanged from `0.12.0`: the configuration system loads nothing, no screen-reader support, no
  cross-platform evidence beyond Linux, the real Claude Code CLI still never exercised by the
  test suite, and `NFR-PERF-004` still unverified.

## 0.12.0 - Your Transcripts, and What the Agent Did

Status: released on 2026-08-22.

`0.11.1` was a documentation-only release that corrected a false privacy claim and had to
publish an admission with it: transcripts are written for every AI CLI run, **and there was
no in-app way to stop it or delete them**. This release removes that sentence by fixing the
thing it described. It also makes an agent run's transcript readable inside the application
for the first time — until now it was recorded and never shown.

### Added

- **Per-project transcript capture opt-out, purge, and retained-size visibility**, all on the
  Trust Settings surface (`Ctrl+Alt+U`). You can decline capture for a project before running
  anything, see how many transcripts are retained and how many bytes they occupy, and purge
  them permanently through a confirmation dialog. The decision persists across sessions.
  Declining affects **future** runs; it does not delete what is already there, which is what
  purge is for.

  The retained-size figure is read from the files themselves at display time, not from a
  counter incremented as data is written. That is deliberate: a counter would read `0` for
  every transcript written before the counter existed, so the dialog would have understated
  exactly the data a user opting out most wants to find. The figure shown and the bytes
  deleted now come from the same source.

- **The AgentRun report surface**, with `Ctrl+Alt+R`: the transcript of the current project's
  most recent agent run, rendered in the application. It states whether the run is still
  active — in which case the transcript may still be growing — and it distinguishes two
  different kinds of "you are not seeing everything": the *window* (only the most recent
  bytes are shown, and it says which bytes) and *writer truncation* (the run produced more
  than storage kept, and that data is gone). Those are different facts and it does not
  collapse them. Transcript content is treated as untrusted and escaped.

- **Two more audit families have real producers**: `restricted_mode_blocked`, recorded when
  Restricted mode refuses a launch, and `project_added`, recorded when a project is added to
  the board. Purging transcripts records `transcript_purge`. The audit store still has **no
  surface** — nothing in the application renders it — so this improves the record on disk,
  not what you can see.

### Fixed

- **The modal scrim failed WCAG 2.1 SC 1.4.11 at its worst case.** `0.11.0`'s new contrast
  gate measured the pairs it was given; this cycle measured the *derived* pair nobody had —
  the modal card's own border against the scrim, with bright terminal content behind it —
  and found **2.40:1** where 3:1 is required. The scrim is now 0.75 alpha, measuring 3.62:1
  at that same worst case, and remains visibly translucent.

  The interesting part is why it was missed twice. Contrast of a translucent layer is not
  monotonic in the backdrop: the worst case sits at neither the darkest nor the brightest
  backdrop but at a crossing point in between, so **sampling a few plausible backdrops finds
  a passing ratio and stops.** It is now swept rather than sampled, and adding a new theme
  role that participates in a translucent pair cannot leave it unmeasured.

- **Restricted mode overstated what it blocks.** The board reported ten blocked automations
  when nine are actually enforced: the count was taken from the full `RestrictedModeFeature`
  vocabulary, which had grown a tenth variant reserved for a capability nothing enforces yet.
  A security surface that overstates its own protection is the wrong direction to be wrong
  in, so the count is now taken from what is enforced.

- **Purging transcripts no longer depends on the audit store opening.** Deletion was reached
  through the code path that records it, so a project whose audit store failed to open would
  have had its purge silently do nothing — failing closed on the audit record, but also on
  the deletion the user asked for. Deletion now happens on both paths; only the record is
  conditional.

- **Both published crates now ship `LICENSE` and `NOTICE`.** They live at the workspace root,
  which cargo does not include when packaging a crate in a subdirectory, so every release
  from `0.1.0` to `0.11.1` published without the Apache-2.0 licence text or the NOTICE —
  Apache-2.0 §4 requires both to travel with the distribution. The NOTICE also carries
  rusqlite's MIT terms and SQLite's public-domain notice, so third-party attribution was
  missing too. **If you are redistributing an earlier version, take the two files from the
  repository root**; the licensing itself is unchanged and always was Apache-2.0.

- A test-harness defect that leaked real child processes when a test panicked. No effect on
  the shipped application.

### Known limitations

- **Purge does not erase every trace.** The transcript files are deleted; a tombstone marking
  that the run had a transcript remains in project state, and the `transcript_purge` audit
  record remains in the audit store by design — an audit trail you can erase from inside the
  application is not an audit trail. Stated here rather than left for someone to discover.
- **The retained-bytes figure can understate**, never overstate: a file whose size cannot be
  read is counted as zero rather than guessed at.
- **The AgentRun report shows the most recent run only.** There is no run history, and no way
  to open a specific earlier run.
- **There is still no way to review generated changes.** `0.11.0` made change detection real;
  rendering it remains unbuilt (RFC-020 PR-020-C), so a detected change set is still
  something the product knows and cannot show you.
- **The audit store remains unviewable** from the application.
- Unchanged: no screen-reader support, no cross-platform evidence beyond Linux, the real
  Claude Code CLI still never exercised by the test suite, and `NFR-PERF-004` still
  unverified.

### Also in this cycle

- **A configuration system was implemented, and a configuration file still does nothing.**
  The file format, search precedence, atomic validation, bounded diagnostics, the rule that
  security-sensitive settings do not take effect on hot reload, and configuration-defined AI
  CLI profiles routed through the same launch validation as the built-in one are all built
  and tested (RFC-023). **Nothing in the application loads them** — no code constructs a
  configuration store, so writing a config file has no effect on the running product. This is
  named here explicitly because "configuration system" in a changelog reasonably reads as
  "you can configure it now," and you cannot. It is groundwork, and the slice that makes it
  reachable is not yet scheduled.
- The RFC process moved to a five-folder lifecycle, and RFC-023, RFC-031, RFC-033 and
  RFC-037 closed.

## 0.11.1 - Transcript Disclosure Correction

Status: released on 2026-08-18.

**A documentation-only release, published for one reason: the privacy claim it corrects is
wrong on the page people read.** crates.io serves the README of the published version, so
correcting it in the repository changes nothing for a user until a release carries it. No
code changed.

### Corrections

- **Tekstide records AI CLI session transcripts to disk, and `0.10.0` and `0.11.0` both said
  it did not.** Both releases' READMEs stated that transcript retention was "not wired into
  the desktop application" and that "no transcript is ever written." That was false from
  `0.10.0` onward, the release in which launching an agent run first became possible.

  What actually happens: pressing `Ctrl+Alt+A` in a trusted project writes that session's
  terminal output to
  `$XDG_STATE_HOME/tekstide/transcripts/<project>/<agent-run>/transcript.log`, bounded by
  RFC-011's policy — 32 MiB per transcript, 256 MiB per project, 1 GiB application-wide, 30
  days. There is no in-app way to disable capture or purge it; deleting the `transcripts/`
  directory is the only route today. Plain terminals (`Ctrl+Alt+T`) are not recorded.

  **The behaviour is intended** — RFC-011 designed capture, its bounds, and its purge policy
  deliberately. **The documentation was wrong**, and the reason is worth recording: the claim
  was verified by searching the `tekstide` crate only, where nothing configures a transcript
  writer. `tekstide-core` configures one on that crate's behalf, so a true premise carried a
  false conclusion into two releases. No test asserted transcript behaviour on the real launch
  path, so the suite could not contradict it.

  **What to do if you relied on the old claim** (added 2026-08-19; this entry originally
  described the correction without saying what a reader who had acted on it should check).
  **If you ran an AI CLI session on `0.10.0` or `0.11.0` believing nothing was recorded, those
  transcripts are on your disk now.** They are at
  `$XDG_STATE_HOME/tekstide/transcripts/` — `~/.local/state/tekstide/transcripts/` if
  `XDG_STATE_HOME` is unset. Delete that directory if you did not want them. Nothing was
  transmitted anywhere; this is local data only.

  `README.md`'s *Local Data and Privacy* section now describes what is written, where, under
  what bounds, and what has no user-facing control. Called out here rather than corrected
  quietly, because it is a privacy claim users may have relied on.


## 0.11.0 - What the Agent Changed

Status: released on 2026-08-18.

`0.10.0` made an agent run launchable. This release makes what it did **detectable** — and
fixes an accessibility defect found by measuring something nobody had measured before.

### Added

- **Change detection for agent runs.** Launching a run captures a filesystem baseline of the
  project **before the agent's process is spawned**, and when that run's terminal exits the
  two are compared, producing a real change set naming the files the run actually touched.
  Four capabilities that had been implemented, reviewed and dormant since RFC-012 —
  `capture_agent_run_filesystem_baseline`, `detect_filesystem_changes`,
  `add_detected_generated_change_set`, and `apply_agent_terminal_outcome` — have production
  callers for the first time.

  **This does not give you a way to look at the result.** There is still no diff review or
  AgentRun report surface. What changed is that the change set those surfaces need now
  exists rather than being structurally impossible: diff review is **buildable, not
  reachable**.

- **The approval-history surface opens**, with `Ctrl+Alt+H`. It was built and tested in
  `0.10.0` and had no key bound to it, so nothing could open it. This makes the *surface*
  reachable, not command approval itself — no shipping AI CLI speaks the protocol, so a real
  user will see the surface empty, which is correct rather than a bug.

- **A WCAG contrast gate over the theme**, asserting real ratios: 4.5:1 for text pairs, 3:1
  for non-text UI boundaries, and the translucent modal scrim composited over its backdrop
  before being measured. The suite it replaces checked that colour channels were within
  `0.0..=1.0` — a bound no plausible colour can fail.

### Fixed

- **Unfocused pane borders failed WCAG 2.1 SC 1.4.11.** They measured **2.63:1** against the
  background and 2.37:1 against elevated surfaces, below the 3:1 required for the visual
  boundary of a UI component; they are now 3.85:1 and 3.48:1. Text contrast was never the
  problem — it sits above 14:1 — and focus indication was unaffected, since the focused
  border is the accent at 6.38:1 and carries a second, non-colour channel. The gate above was
  written first and **observed failing at those exact ratios** before the colour changed.

### Known limitations

- **Change detection excludes `.git/`, `target/` and `node_modules/`** by design; build
  output and VCS metadata would otherwise drown the result and exceed the scan limit on any
  real project. The consequence is stated rather than left implied: **a change an agent makes
  inside those directories is not reported**, git hooks included.
- **Detection runs only when a run's terminal exits.** A long-lived interactive session
  reports nothing until it ends.
- **The baseline is held in memory.** If the application closes while a run is live, that
  run produces no change set — indistinguishable, from outside, from an agent that changed
  nothing.
- **A truncated scan is recorded as truncated, not as "no changes."** Those are different
  facts and the product does not collapse them — but no surface renders either yet.
- Unchanged from `0.10.0`: no screen-reader support, no cross-platform evidence beyond
  Linux, the real Claude Code CLI still never exercised by the test suite, and
  `NFR-PERF-004` still unverified.

### Also in this cycle

- An evaluation of the `snora` GUI framework at the owner's request, declined as a
  dependency with the reasoning recorded. It is what prompted measuring our own contrast in
  the first place, and the resulting exchange found defects in both projects.
- The `0.3.0` git tag, which had pointed into an orphaned line of history since a trailer
  rewrite, was re-pointed at its content-identical replacement; `0.4.0` and `0.4.1`, which
  had never been pushed, now exist on the remote.


## 0.10.0 - Trust, and the First Reachable Agent Run

Status: released on 2026-08-17.

**The release where the product's premise becomes reachable.** Every previous release
shipped an application in which no project could *ever* leave Restricted Mode — there
was no code path anywhere that granted trust — so AgentRun launch, the thing Tekstide
exists to do, was blocked for every user, permanently. This release grants trust,
launches the run, and sizes the terminal it runs in.

### Added

- **Workspace trust granting and revocation** (RFC-032). `Ctrl+Alt+U` opens a Workspace
  Trust surface showing the project's real state. Granting opens a confirmation dialog
  whose focus defaults to **Cancel** — granting takes two deliberate acts, revoking
  takes one, because revoking is the safe direction. The path shown is the **canonical**
  path, which is what trust binds to; a symlinked project also shows the path you opened
  it by, so a redirected symlink cannot quietly bind trust somewhere else. Trust
  persists across sessions, and the **audit store** — not the user-writable
  recent-projects cache — is what restores it, queried for an *applied* grant
  specifically so an interrupted grant does not come back as trust.

  The dialog says three things it would be easy to leave implied: that the grant covers
  files not yet written, including anything an AI agent run writes there; that it lasts
  for this session and every session after; and that revoking stops future loading but
  does **not** undo anything that has already run. It does not claim that trusting is
  safe, or that Tekstide polices what runs.

- **AgentRun launch, reachable for the first time.** With trust granted, `Ctrl+Alt+A`
  launches a real Claude Code session in a project-owned terminal. Proven end to end
  from a real key press: a profile that honestly declares it may discover workspace
  files is refused in a fresh Restricted project, and launches for real once trust is
  granted through the GUI route.

- **The adapter-spawn pathway and a rendered command-approval dialog** (RFC-022):
  per-run Unix domain socket, capability-token delivery, structural risk classification,
  promotion re-evaluated rather than decided once, a bounded approval queue with expiry
  tracking, and an approval-history surface. All built and proven against production
  code. See *Known limitations* for why no user can reach it.

### Fixed

- **Terminals were permanently 24×80.** `ROWS`/`COLS` were fixed constants shared by the
  spawned PTY and the rendered grid, and nothing in the application called terminal
  resize at all — so every terminal ignored the window regardless of size. Terminals now
  follow a live window drag, and a pane launched before you ever resize the window gets
  the real size immediately rather than starting wrong. One computed size is applied to
  the PTY, the emulator grid, and the render path together.

- **A completed trust grant could be undone by a later interrupted one**, and trust was
  restored from the user-writable recent-projects cache rather than the audit store.

### Known limitations

- **You still cannot see what an agent run changed.** There is no diff review or
  AgentRun report surface, and the reason is structural rather than scheduling: nothing
  in the shipped application runs change detection, so no change set can exist for
  either surface to render. This is the next theme.
- **The real Claude Code CLI has never been exercised by this project's tests.** Every
  automated proof uses a controlled test executable, because the live product needs
  interactive authentication and makes real network calls. The launch pathway is proven;
  the real binary's behaviour under it is not.
- **Command approval remains unreachable, for a new reason.** It is no longer missing
  machinery — it is missing an ecosystem. No shipping AI CLI speaks RFC-021's protocol,
  so `Managed` mode can only ever be exercised by this project's own reference adapter,
  a test artifact. Approval also remains **cooperative, not enforced**: Tekstide does not
  intercept process execution and cannot withhold it from an adapter that ignores a
  rejection.
- **The approval-history surface cannot be opened.** It is implemented and tested, but no
  key is bound to it — a defect found while reviewing RFC-032 and recorded rather than
  quietly fixed. The underlying cause is now named: a navigation action marked
  *configurable* with no default binding is **dead**, not pending, because there is no
  configuration system yet to bind it with.
- **`NFR-PERF-004` (terminal input latency, 16 ms p95) remains unverified**, unchanged
  from `0.8.0`. The structural cause is gone and proven gone; bounding the true
  end-to-end path needs presentation timing this project has no non-perturbing way to
  measure.
- No screen-reader support, no cross-platform evidence beyond Linux, no safe-close
  dialog, no file watcher, no editor undo.

### Also in this cycle

- **A reachability audit** across 132 candidate capabilities, using compiler-enforced
  deprecation markers rather than grep. **104 were dormant** — correct, reviewed, tested
  code with no route from the GUI — of which 30 have no caller anywhere, a floor rather
  than a count. Two of its priority items are discharged in this release (terminal
  resize; trust granting). The rest is recorded in `rfcs/future-work.md`.
- `ARCHITECTURE.md` gained two conventions learned the hard way this cycle:
  **reachability comes before correctness** (name the user's path and the production
  producer before scheduling a surface), and **latency criteria stop the clock at state
  change, not pixels**.


## 0.9.0 - Transcript Capture, Re-homed

Status: released on 2026-08-16.

**A correctness release with nothing a user can see.** `0.8.0` replaced the terminal's read
path; this repairs a capability that replacement silently removed. It is published because
the fix should not sit unreleased while the work that depends on it is designed — not
because it adds anything reachable.

### Fixed

- **Transcript capture, which `0.8.0` had silently stopped performing.** The old read path
  wrote every byte to the transcript as a side effect of a function named for something
  else, and it was the only transcript-writing code in the workspace. When `0.8.0` moved the
  terminal's ingress to a dedicated reader thread, capture went with the old path — nothing
  failed, because nothing in this release or any before it creates an AgentRun, so no
  transcript writer is ever configured.

  Capture now lives in the reader thread, and writes **before** the bytes reach the display,
  so the durable record is a superset of what was shown rather than the reverse. Mid-stream
  write failure has a real policy for the first time: best-effort capture marks itself
  failed and keeps the terminal usable, while required capture stops reading — so the
  process stalls on its own `write()` rather than making progress that is not being
  recorded, and is not killed.

### Breaking

- **`TranscriptWriterConfig` gained a public `mode` field**, and its `new` constructor a
  third parameter. Callers constructing it either way must be updated. This is what makes
  the release `0.9.0` rather than `0.8.1`.

### Not in this release

Nothing user-visible, deliberately. Transcript retention is still **not wired into the
desktop application** — nothing creates an AgentRun, so no transcript is ever written in
practice. This release makes the capability correct *before* the work that will depend on
it, rather than after.

Command approval, diff content, the transcript reader, and the diff/AgentRun surfaces all
remain implemented, reviewed and unreachable, waiting on an adapter-spawn pathway that does
not exist. `NFR-PERF-004` remains not met.

### Also in this cycle

- A test-concurrency flake in the terminal reader suite, which made the workspace gate fail
  roughly one run in five, is fixed. Three separate bugs were behind it, two of them found
  only because a full-serialisation experiment failed to resolve the flake and the theory
  was re-examined rather than the fix tuned.


## 0.8.0 - Readiness-Driven Terminal I/O

Status: released on 2026-08-15.

One theme: the terminal stopped waiting on a timer. RFC-017 Amendment 1 replaced the 50 ms
poll tick with a dedicated reader thread that blocks on PTY readiness and wakes the UI when
bytes actually arrive.

### Implemented

- **Terminal output throughput rose from roughly 374 KB/s to 17-18 MB/s.** The old ceiling
  was not a property of the hardware or the emulator: a hardcoded 10 ms sleep ran on the UI
  thread every time a read found nothing, so the reader spent about 0.5% of each tick
  actually reading. Output now keeps pace with what a producing process can write, measured
  against the same flood script's own standalone rate.

- **The concurrent-terminal limit rose from 3 to 6**, re-derived from a fresh measurement
  rather than carried forward. The old `3` was never a product judgement — it was a
  consequence of each pane's poll costing ~10.1 ms against a 50 ms tick, which saturated at
  5. The new number comes from an N-pane benchmark that stays clean through 6 and first
  degrades at 8.

- **Terminal output can no longer be silently dropped.** The old read path truncated at
  64 KiB mid-read, discarded the remainder, and threw away the event that recorded it —
  feeding the emulator a byte stream with a hole in it. The replacement applies
  backpressure instead: when the buffer fills, the reader stops reading and the producing
  process blocks on `write()`, the way a real terminal behaves. Dropping is not handled
  better; it is structurally impossible on this path.

### Not fixed, and stated rather than implied

- **`NFR-PERF-004` (terminal input latency, p95 ≤ 16 ms) is still not met.** This release
  removes the structural cause of the previous failure — the 50 ms interval put the floor
  near 47.5 ms by arithmetic alone — but removing a known cause does not measure the
  result. Proving failure needed only a lower bound; proving success needs an upper bound,
  and three attempts to obtain one were confounded by unrelated load on the measuring
  machine and discarded rather than reported.

- **The criterion's own wording was corrected in this cycle**, after it had been evaluated
  twice against a boundary it never stated. It now says where the measurement stops —
  application state change, excluding compositor and GPU present time — consistent with the
  two neighbouring latency criteria, which had always been measured that way without saying
  so. Restating the boundary does not discharge the criterion.

- **Nothing here makes AgentRun or diff-review surfaces reachable.** They remain
  implemented at the model level with no route to them, along with command approval. The
  release notes have said this since `0.5.0` and it is still true.

- **A defect this work uncovered is recorded, not fixed**: the new reader has no transcript
  capture, and the code path that had it is no longer on the terminal's ingress. This is
  invisible today because nothing in production creates an AgentRun, and is recorded as a
  blocking prerequisite on the work that would.


## 0.7.0 - A Content-Independent Trusted-UI Tell, and Diff Preview Policy

Status: released on 2026-08-12.

A small release with one user-visible change and one library-level policy. **It contains a
breaking change to `tekstide-core`**, which is what makes it `0.7.0` rather than `0.6.1`.

### Implemented

- **The window dims behind the paste-confirmation dialog** (RFC-018 PR-018-G). RFC-018
  shipped the dialog claiming a *spatial* tell — a real dialog occludes trusted chrome,
  and terminal output cannot draw outside its own pane. Measuring that claim showed it was
  content-dependent: the dialog's size follows the pasted content, so an attacker who
  keeps a paste short keeps the dialog inside the terminal's own pane, where imitation is
  possible. The scrim replaces that tell with one the attacker does not control — its
  extent is fixed by the window, so it dims chrome no terminal pane can draw into whether
  the paste is one byte or one megabyte. It is translucent deliberately: an opaque overlay
  would be indistinguishable from any solid rectangle a spoof could also draw.

  This does **not** make the dialog unspoofable, and it does not repair the spatial claim —
  that claim was replaced, not fixed, and RFC-018's disclosed limitation stands. Keystroke
  suppression remains the load-bearing defence; the scrim is an additional check a user can
  make, not a guarantee.

- **Diff preview policy** (RFC-024, closed). `tekstide-core` can now read the content
  behind a detected change under an explicit policy: refuse rather than truncate, classify
  binary before reading text, bound the read against file metadata before any content is
  loaded, and hold content in a type that cannot outlive the request — `DiffContent`
  derives neither `Clone` nor `Serialize`, so storing it in session state or handing it to
  the audit store is a compile error rather than a review comment.

  **No surface renders any of this.** It is a library capability with no UI in this
  release; the diff review surface is RFC-020.

  One limitation worth stating plainly, because it constrains what a diff can ever be
  here: for a **modified** file there is no two-sided diff. The before-bytes were never
  captured — review baselines are metadata-only by deliberate design — so they are gone,
  not merely unretained, by the time a diff is requested. What is available is the current
  content, and the API says so in its own type rather than in a doc comment.

### Breaking

- **`ChangePathKind` no longer has a `Deleted` variant, and `DetectedChangedPath` carries a
  new `ChangeLifecycle { Added, Modified, Deleted }`** (RFC-012 Amendment 1). The old enum
  conflated *what a path is* (file, directory, symlink) with *what happened to it*, so a
  deleted directory could not be represented at all, and the Added-vs-Modified distinction
  was computed during change detection and then discarded. Callers matching on
  `ChangePathKind::Deleted` should read `ChangeLifecycle` instead.

### Not in this release

No diff or AgentRun report surface (RFC-020), no configuration system, no Git integration,
no file watcher, and no cross-platform evidence beyond Linux. The project board still
reports `terminals: not implemented` for a project with no open terminal, which is false —
it is a known defect, recorded in `rfcs/future-work.md`, not a statement about the feature.

## 0.6.0 - Editor and File Explorer

Status: released on 2026-08-11.

Tekstide `0.6.0` opens milestone M10 with RFC-019: Content mode stops being a placeholder
and becomes a real file explorer and a real text editor. **RFC-019 is closed**
(`rfcs/done/`). Diff review and the AgentRun report are RFC-020, M10's second half, and
are not in this release.

### Implemented

- **A file explorer tree.** Renders the project's directory scan, with keyboard
  navigation; Enter on a directory rescans, Enter on a file opens it. Read-only: no
  rename, delete, or create.
- **A text editor with a real cursor.** Open a file, move with the arrow keys, insert and
  delete at the cursor position across multiple lines, and save with `Ctrl+S`. The cursor
  position is shown.
- **External-change handling that asks rather than assumes.** If a file changes on disk
  while you have it open, saving does not overwrite it — a dialog offers to reload,
  and every way of dismissing that dialog leaves the disk file untouched. The dialog
  distinguishes a genuine conflict from a clean file that merely changed underneath, and
  only claims local changes will be discarded when there are some.

### Text safety — an asymmetry worth understanding

A file's **name** and a file's **contents** are treated oppositely, deliberately.

Names in the explorer and in the editor's header are **escaped**: a file called
`proj<U+202E>gpj.exe` renders with that override character visible as `<U+202E>` rather
than silently displaying as `projexe.jpg`. A repository can contain such a name and
nobody typed it.

File **contents** in the editor are **not** escaped, and bidirectional text reorders
normally. An editor that rewrote what it displayed would be broken — you would edit
around a character that is not really there and save something you did not intend.

The consequence is that source containing a bidi override still *reads* differently from
how it compiles — the Trojan Source class. Tekstide shows you the file as it is; it does
not currently mark such characters. See `rfcs/done/016-internationalization-and-localization.md`.

### Deferred

- **No undo.** A mid-buffer edit is unrecoverable within the session past what Backspace
  can still reach.
- **No syntax highlighting, language server, multi-cursor, or search.**
- **Files larger than 4 MiB are not editable** — the existing open policy refuses them,
  and the refusal is shown rather than failing silently.
- **The explorer never modifies the filesystem** — no rename, delete, or create.
- **Symlinks show status, not their target.** Whether an entry is a symlink, broken, or
  points outside the project is shown; the target path is not.
- **Reaching Content mode needs a mode toggle.** No keybinding opens the project
  workspace directly; `Ctrl+Alt+M` gets you there as a side effect.
- **A known `tekstide-core` inaccuracy**: after a blocked save, the project content status
  reports a conflict even when the open buffer had no local edits. The dialog no longer
  relies on this, but the status itself is imprecise. Recorded in `rfcs/future-work.md`.
- **Nothing here changes the terminal.** `NFR-PERF-004` remains not met, the
  three-terminal limit and the ~374 KB/s output ceiling are unchanged.
- **No screen-reader support.** Checked again this release
  (`cargo tree -p tekstide | grep -i accesskit`, empty).
- Linux only.

### Dependencies

No new dependencies.

## 0.5.1 - Paste Protection and Trusted-UI Evidence

Status: released on 2026-08-10.

Tekstide `0.5.1` completes milestone M9 with RFC-018, the second half of the terminal
work `0.5.0` began. **RFC-018 is closed** (`rfcs/done/`). The `0.5.0`/`0.5.1` split
follows the same shape as `0.4.0`/`0.4.1`: one milestone, two releases, because the
scope was too large for one.

Press **`Ctrl+Shift+V`** to paste into a focused terminal. What happens next is decided
by RFC-009's policy, not by the paste widget.

### Implemented

- **Real clipboard paste, classified before it reaches the shell (RFC-018).** Pasted
  bytes go through `TerminalInputPolicy::evaluate` before any PTY write. A single-line
  paste is allowed; a multi-line paste opens a confirmation dialog; a paste containing
  control characters is refused outright. Paste reaches the PTY through the same single,
  modal-gated ingress keystrokes already used — it did not get its own.
- **A real confirmation dialog**, built on the existing modal layer. Every dismissal path
  defaults to **not** pasting: Escape cancels regardless of which button is focused, and
  only an explicit accept writes anything.
- **The pasted content is shown, escaped.** The preview runs through the same
  untrusted-text path the Project Board uses, so a paste containing bidi-override or
  control characters renders as `<U+XXXX>` markers rather than reordering the dialog's
  own text. Newlines are escaped too, so pasted content cannot fabricate extra rows that
  imitate the dialog's controls.
- **Audit: `paste_blocked` has a producer.** A policy-refused paste is recorded. A
  sentinel test proves no pasted content, clipboard text, or command text reaches the
  durable store, checked against raw on-disk bytes.
- **Trusted-UI evidence**, nine screenshots against a real terminal running live output.

### What the trusted-UI evidence shows, and does not

One property distinguishes the genuine paste dialog from terminal output imitating it:
**while the dialog is open, keystrokes never reach the terminal.** That was demonstrated
live, with a positive control proving the keystrokes were reaching the application at the
time — so their absence is suppression, not non-delivery. An imitation drawn by terminal
output cannot suppress input.

The terminal grid can never render outside its own pane, so chrome is always authentic.
But whether the genuine dialog *visibly* uses that headroom depends on how wide its
preview is — which depends on the pasted content, which an attacker may influence. It is
therefore recorded as an architectural fact and **not** offered as something a user can
rely on seeing.

**This evidence shows an imitation cannot occupy chrome and cannot suppress input. It
does not show that a user would notice one that tries.**

### Dependencies

No new dependencies.

### Deferred

- **Pastes larger than 256 KiB are refused whole**, not truncated. Truncating before
  classification would let truncation change the classification and would silently write
  a prefix of what was copied.
- **The audit family records paste refusals only.** A paste the user *approves* has no
  valid encoding in the frozen v1 schema, and an over-cap refusal has none either. Both
  are recorded as known limitations rather than fixed by amending a frozen schema.
- **No semantic detection of dangerous pasted commands.** RFC-009 excludes it by design;
  a classifier that catches some dangerous pastes invites the belief that it catches all.
- **Nothing here improves terminal performance.** `NFR-PERF-004` remains not met, the
  three-terminal limit and the ~374 KB/s output ceiling are unchanged. All are downstream
  of the same poll defect and owned by readiness-driven terminal I/O
  (`rfcs/future-work.md`).
- **No screen-reader support.** Checked again this release
  (`cargo tree -p tekstide | grep -i accesskit`, empty).
- Linux only. No macOS or Windows terminal runtime evidence exists.

## 0.5.0 - Terminal Renderer, and a Terminal You Can Open

Status: released on 2026-08-08.

Tekstide `0.5.0` delivers the first half of milestone M9: RFC-017's terminal renderer,
plus the launch UX that makes it reachable. **RFC-017 is closed** (`rfcs/done/`), accepted
with `NFR-PERF-004` recorded as **not met** — see Deferred. RFC-018 (rendered paste
protection and adversarial spoofing evidence) is M9's second half and is not in this
release.

Press **`Ctrl+Alt+T`** in an open project and you get a real, PTY-backed terminal with
RFC-009's accepted-sequence policy enforced in front of the emulator.

### Implemented

- **Terminal surface (RFC-017).** A real `alacritty_terminal` grid behind RFC-009's
  security filter, rendered as a surface under RFC-015's contract. The filter's four
  properties — single ingress, no side channels, classification parity, and
  stream-position independence under adversarially chunked input — were re-proven
  against product code rather than inherited from the RFC-014 spike, each independently
  ablated.
- **Terminal launch UX.** `Ctrl+Alt+T` launches a terminal in the active project and
  switches to Terminal Mode. Typing `exit` really closes it: exit detection transitions
  the session, frees its visible slot, and makes the slot reusable.
- **Immersion mode, split, and session bar.** At most two visible panes, with the split
  decided from real measured font metrics — a split that cannot give each pane a full
  grid width is refused rather than rendered clipped. Session state is distinguishable
  without colour (`NFR-UX-002`).
- **Audit: `plain_terminal_observation` has a producer.** The first audit write the
  desktop application has ever performed. Opening a terminal records that a session
  started; exiting records that it terminated. A sentinel test proves no command text,
  output, or path reaches the durable store, checked against raw on-disk bytes.
- **Bounded scrollback** at 2,000 lines, ablation-verified under sustained output.
- The RFC-014 and RFC-007 spike crates were deleted, their properties having product-code
  equivalents with their own tests.

### Local data

**Opening a terminal now creates an audit database** at
`$XDG_STATE_HOME/tekstide/audit/audit.sqlite3`. This is a behaviour change from `0.4.1`,
which created no such file. It records that a terminal session started and stopped, and
nothing else — the schema has no field for command text, output, or paths. Delete the
`audit/` directory to reset it; there is no in-app purge command yet.

### Dependencies

No new dependencies. Two workspace crates were removed (`tekstide-gui-spike`,
`tekstide-pty-spike`), both `publish = false` and neither reachable from a shipped crate.

### Deferred

- **`NFR-PERF-004` (terminal input latency p95 ≤ 16 ms) is NOT met**, and is recorded as
  such rather than redefined until it passed. PTY bytes reach the grid only on a 50 ms
  poll tick, so poll-wait alone contributes a p95 near 47.5 ms. The fix is readiness-driven
  terminal I/O, scheduled as follow-up (`rfcs/future-work.md`).
- **At most three concurrent terminals per project.** This is a consequence of the same
  poll defect, not a product decision: every live pane is polled sequentially each tick at
  roughly 10 ms per pane, and five panes would saturate the tick. The limit is expected to
  rise once readiness-driven I/O lands.
- **Terminal output throughput is capped near 374 KB/s**, again by the same defect.
- **No trusted-UI separation or spoofing-resistance claim.** Nothing in this release
  demonstrates that terminal content cannot imitate Tekstide's own chrome. That is RFC-018.
- **No paste path exists** — the terminal accepts keystrokes only, so RFC-009's paste
  policy has nothing to protect yet. Rendered paste protection is RFC-018.
- **No terminate-from-UI and no pane selection.** Close a terminal by typing `exit`; input
  goes to the `Primary` pane.
- `TextStream::to_pty_bytes` is a defined subset, not a complete VT100/xterm encoder.
- **No screen-reader support.** `iced` offers no accessibility bridge; checked again this
  release (`cargo tree -p tekstide | grep -i accesskit`, empty).
- Linux only. No macOS or Windows terminal runtime evidence exists.

## 0.4.1 - Mode Switching, Focus Indicator, and RFC-015 Closure

Status: released on 2026-08-01.

Tekstide `0.4.1` completes milestone M8 (GUI Foundation) with RFC-015 PR-015-E: the
`0.4.0`/`0.4.1` split deferred mode switching and its latency measurement here because
M8 had no second mode to switch into until this slice built it. **RFC-015 is closed**
(`rfcs/done/`) as of this release — both risks the RFC-014 substrate decision carried
unverified are now discharged: **R1** (input latency) by `0.4.0`'s C2/C5 measurements
and this release's C4, and **R6** (the focus-trap property not transferring from the
spike) by PR-015-C's real test. The RFC-014 substrate decision record has no open
items remaining.

### Implemented

- RFC-015 PR-015-E mode switching and Content-mode scaffolding:
  - Content ↔ Terminal route switching, dispatched through a real `Ctrl+Alt+M`
    keybinding (`NavigationAction::ToggleProjectMode`, previously unbound) to the
    pre-existing `AppCommand::ToggleActiveProjectMode`; no animation or interpolation
    in the switch path;
  - sidebar and main-area scaffolding (`FocusZone::Sidebar`, still `#[non_exhaustive]`
    for RFC-017/019/020) that required no changes to the input-routing structure
    PR-015-C established;
  - a visible, non-colour-only focus indicator (`NFR-UX-002`): border colour, border
    width, and a textual `"> "` marker all change together with `state.focus` —
    `0.4.0` shipped without one, defensibly, because the shell had only one focus
    zone; this release adds the second zone the indicator was always meant for.
- RFC-014 R1 discharge, completed: C4 (`NFR-PERF-002`, mode-switch latency, budget
  p95 ≤ 32ms), reusing `0.4.0`'s measurement harness rather than a new mechanism.
  Decomposed input-to-state-change (p95 29µs) and view-build cost (p95 39µs) sum to
  68µs, met by roughly 470× — **measured against the Content/Terminal-mode
  placeholders this release ships** (single-line catalog text each), not against the
  real editor (RFC-019) or terminal grid (RFC-017) those placeholders stand in for.
  RFC-017's handoff carries the obligation to re-check `NFR-PERF-002` once Terminal
  Mode renders a real grid.

### Dependencies

No new dependencies; this release is entirely `crates/tekstide-core`/`crates/tekstide`
source changes (one new default keybinding, no new crates).

### Deferred

- Terminal rendering, editor, file explorer, and diff/review surfaces — M9/M10, RFC-017/019/020.
- Rendered security dialogs and an adapter-spawn pathway that would make command
  approval reachable — M11. Command approval remains implemented but unreachable.
- Screen-reader support — out of scope for the life of the `iced` substrate decision
  (RFC-014 R2, owner-accepted), unchanged.
- `NFR-PERF-002`'s re-check against real Content/Terminal-mode content once RFC-017
  and RFC-019 render it — the placeholder boundary above, not a new finding.

## 0.4.0 - Application Shell and Project Board

Status: released on 2026-08-01.

Tekstide `0.4.0` covers milestone M8 (GUI Foundation): RFC-014's substrate decision,
RFC-016 PR-016-B/C/D's i18n and text-safety foundations, and RFC-015 PR-015-B/C/D/F/G's
application shell and Project Board. Owner-approved `0.4.0`/`0.4.1` split (2026-07-30):
mode switching and its latency measurement move to `0.4.1` because M8 has no second
mode to switch into that isn't the Project Board against an empty placeholder. It
remains a GUI shell over the headless core, not the full AI CLI workbench.

### Implemented

- RFC-014 desktop GUI substrate decision:
  - `iced` approved as the substrate, with Option A terminal filtering;
  - spike evidence and findings R1-R9 recorded; R1 (latency unverified) and R6
    (focus-trap property) discharged by RFC-015; R2 (no screen-reader support) and R9
    (survivorship bias in confirmed-only percentiles) owner-accepted and carried
    forward as standing findings, not defects.
- RFC-016 i18n, locale, and text-safety foundations (PR-016-B/C/D):
  - string catalog, locale selection with fallback, and the discipline that no
    user-facing string is hardcoded;
  - a canonical shared text-safety primitive (escaping and bidi isolation for
    untrusted text) adopted by both the shell and `approval::coordinator::display_argv`,
    retiring the duplicate-escaping debt recorded in `rfcs/delivery-plan.md`;
  - `CatalogArgs`' typed `number`/`untrusted`/`trusted_symbol` interpolation API,
    closing the untrusted-text interpolation bypass structurally rather than by
    convention, plus pluralization support.
- RFC-015 application shell and rendered surface model (PR-015-B/C/D/F/G):
  - a real `iced` desktop application replacing the headless text harness: window,
    chrome/content/modal layer composition via `stack`/`opaque`, with surface code
    structurally unable to open, populate, or dismiss a modal or render trusted chrome;
  - a keyboard-driven focus and input-routing model (`ShellInput`/`SurfaceInput`/
    `TextStream` as distinct, module-private types) with modal exclusivity and
    input-class privacy enforced by the compiler, not a runtime check;
  - a Project Board surface rendering live `ApplicationShell` state, with untrusted
    project names and paths escaped and honest `CountDisplay` fidelity
    (`Unavailable`/`NotImplemented` never render as `0`);
  - app-internal latency measurement (behind an opt-in flag, proven non-contaminating
    by idle-CPU comparison) discharging RFC-014 R1: typing latency
    (`NFR-PERF-003`, an upper-bound proxy from the sum of two measured streams' p95s)
    clears its budget by roughly two orders of magnitude, and warm start
    (`NFR-PERF-001`) clears its budget comfortably, at about a fifth of it.

### Dependencies

- Added to `tekstide` only (`tekstide-core` gains no GUI dependency, mechanically
  checked via `cargo tree -p tekstide-core --edges normal | grep -i iced`): `iced 0.14`
  (`tokio`, `advanced` features), `fluent-bundle 0.16`, `unic-langid 0.9`,
  `sys-locale 0.3`.

### Deferred

- Mode switching between Content and Terminal views, and the `NFR-PERF-002`
  mode-switch latency measurement that depends on it — `0.4.1`, RFC-015 PR-015-E.
- Visible focus indicators at the shell-chrome level. Low-stakes today because the
  shell has a single focus zone, but required before PR-015-E adds a second one —
  tracked for `0.4.1`.
- Terminal rendering, editor, file explorer, and diff/review surfaces — M9/M10.
- Rendered security dialogs (trust, safe-close, destructive, configuration change) and
  an adapter-spawn pathway that would make command approval reachable — M11. Command
  approval remains implemented but unreachable, as in `0.3.0`.
- Screen-reader support — out of scope for the life of the `iced` substrate decision
  (RFC-014 R2, owner-accepted).
- Cross-platform terminal, storage, and GUI evidence beyond
  `x86_64-unknown-linux-gnu`.
- **RFC-015 is not closed by this release.** Per RFC-000, it stays in
  `rfcs/proposed/` until PR-015-E and `NFR-PERF-002` land in `0.4.1`.

## 0.3.0 - AgentRun, Transcript, Review, and Durable Audit

Status: released on 2026-07-28. **Git tag re-pointed 2026-08-17** — the original `0.3.0`
tag pointed at commit `1f5100b5`, which a later rewrite (stripping `Co-Authored-By`
trailers) left on an orphaned line of history that no branch contains. The tag now points
at `de40d648`, that rewrite's content-identical replacement: same tree `5291a6b1`, same
message, same author date, reachable from `main`. **What `0.3.0` marks is unchanged**;
only the pointer was repaired. The `0.3.0` package on crates.io still records the old
hash in its own `.cargo_vcs_info.json`, which is not editable after publication.

Tekstide `0.3.0` consolidates three milestones — M5 AgentRun launch, M6 transcript
and review foundations, and M7 durable audit — covering RFC-010 through RFC-013.
These milestones were developed sequentially but never separately released, so they
ship together here. It remains a headless core, not the full AI CLI workbench.

### Implemented

- RFC-010 AgentRun launch model and AI CLI profiles:
  - AI CLI profiles as reviewed launch contracts covering executable provenance,
    argv shape, compatibility level, cwd, environment, prompt, and transcript policy;
  - Restricted Mode rejection of workspace-local executables, wrappers, shims,
    symlink targets resolving into the project root, and project-local `PATH` entries;
  - implicit CLI workspace-config/tool/prompt/plugin discovery blocked or rejected
    before process start;
  - launch validation for project, root, cwd, profile source, environment, and
    compatibility before any process is created;
  - AgentRun launch through project-owned TerminalSessions, with lifecycle derived
    from runtime observation;
  - honest Plain/Supervised/Managed labels; Managed requires adapter capability evidence;
  - active-document dirty, external-change, conflict, and save-error states block
    launch before process start, and safe-save conflict blocking is preserved while
    AgentRuns are active.
- RFC-011 transcript retention and local data policy:
  - capture modes Disabled, LocalBounded, and RequiredLocalBounded;
  - default retention of 32 MiB per transcript, 256 MiB per project, 1 GiB app-wide,
    and 30 days, with aggregate accounting;
  - transcript paths resolved under Tekstide state, outside project roots, with
    symlinked state-root rejection;
  - bounded append-only writer with truncation state;
  - per-run opt-out before process start;
  - purge by transcript, AgentRun, and ProjectSession scope with content-free tombstones.
- RFC-012 generated change review foundations:
  - ChangeSet review model with detection source/status, association confidence,
    bounded content-free summaries, and validated review-state transitions;
  - filesystem baseline capture and metadata-only changed-path detection;
  - project-relative path validation rejecting absolute escapes, `..` traversal, and
    escaping symlinks; symlink entries recorded without following targets;
  - conservative AgentRun association — strong linkage requires a same-run baseline,
    a closed target run, and no overlapping run; ambiguous cases stay unlinked.
- RFC-013 durable audit store and local data policy:
  - versioned durable record with stable string codes and an exhaustive
    family/field validation matrix;
  - local SQLite store with CHECK constraints mirroring that matrix independently of
    Rust validation, transactional append, exact-retry idempotency, operation
    correlation, and phase cardinality;
  - bounded descending cursor queries;
  - schema identity, read-only probe before write-capable open, and a sequential
    migration harness with a statement allowlist;
  - explicit comprehensive diagnostics separate from the bounded startup probe;
  - corruption classification, exact-artifact quarantine with content-free manifests,
    atomic fresh-store installation, and restart-safe resume;
  - project and global purge with ephemeral receipts and local-data accounting;
  - security-event integration for trust grant/revoke, managed AgentRun lifecycle,
    and blocked root/symlink access.

Only three of the twelve audit-schema event families have a wired runtime producer
(trust decisions, managed AgentRun lifecycle, blocked root/symlink access). See
Deferred below and the security threat model's T-035 for why this distinction matters.

### Dependencies

- Added `rusqlite 0.39.0` with `default-features = false` and only `bundled` enabled,
  resolving `libsqlite3-sys 0.37.0` and bundled SQLite `3.51.3`. This is the first
  third-party native dependency; it compiles the SQLite C amalgamation during build.
  Third-party notices are recorded in `NOTICE`.

### Deferred

- Desktop GUI runtime, rendered terminal surface, and rendered paste/approval/trust
  dialogs.
- App/UI commands for launching, selecting, and closing terminals and AgentRuns.
- Command approval.
- Audit producers for command approval, terminal paste, restricted-feature blocks,
  safe-close and destructive decisions, sensitive configuration changes, transcript
  purge, project added, and plain-terminal lifecycle. These families exist in the
  audit schema but have no runtime producer. Wiring `paste_blocked` headlessly was
  considered for this release (RFC-009 already classifies paste without a GUI) and
  deferred to keep 0.3.0 reconciliation-only; it remains available for a future
  release alongside `project_added`, `plain_terminal_observation`, and
  `transcript_purge`.
- Git-based change detection; the RFC-012 detector reports Git as unavailable.
- File watcher, overwrite-confirmation UI, and multi-document conflict workflow.
- Cross-platform terminal, storage, and native build evidence beyond
  `x86_64-unknown-linux-gnu`.
- Encryption at rest, tamper-evident audit, secure deletion, and automatic retention.

### Release Gate Status

Completed on a clean, committed tree:

- `git status --short` clean; `git diff --check`;
- `cargo fmt --all --check`;
- `cargo test --workspace --all-targets --all-features` — 375 `tekstide-core` tests, 0 elsewhere, 0 failures;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `cargo build --release --locked`;
- `cargo package -p tekstide-core --locked` (113 files, 872.3 KiB / 143.7 KiB compressed);
- `cargo publish --dry-run -p tekstide-core --locked`;
- `cargo package -p tekstide --locked` (6 files, 12.8 KiB / 4.7 KiB compressed);
- `cargo publish --dry-run -p tekstide --locked`;
- `cargo publish --workspace --dry-run --locked` (authoritative same-workspace pairing check; verifies `tekstide` against the local `tekstide-core 0.3.0`, not a stale registry version);
- package smoke test: `cargo build`/`cargo test` from the unpacked `tekstide-core-0.3.0` package artifact (not the working tree) — 375 tests passed;
- release tarball built via `git archive` at `tekstide-v0.3.0.tar`: no intermediate parent directory, `NOTICE` and `LICENSE` both at archive root, no `.git`/`.git-exclude`/`target`/local-agent-config paths present (249 entries).

Build-cost baseline (first captured; RFC-013 retained none, so there is no prior figure to compare against):

- Clean `cargo build --release --locked` on `x86_64-unknown-linux-gnu`, Rust 1.97.1: 27.3s wall-clock.
- `target/release/tekstide` binary size: 790,560 bytes unstripped, 605,368 bytes stripped.

Release-candidate review (request 104) found that both published READMEs undercounted wired audit producers (three instead of four — audit-store recovery was omitted) and that the ROADMAP M7 table row still listed producers the reconciled scope section had already moved to M8. Both were corrected in commit `1f5100b` before publishing; the gates above were re-run against the corrected tree and the release tarball and crate packages were rebuilt from that commit. The threat model's matching corrections live in `.git-exclude/specs/`, which is gitignored and carries no commit.

Post-publish verification on 2026-07-28:

- `cargo publish -p tekstide-core --locked` — published `tekstide-core 0.3.0` to crates.io.
- `cargo publish -p tekstide --locked` — published `tekstide 0.3.0` to crates.io, correctly resolved against the just-published `tekstide-core 0.3.0`.
- Tag `0.3.0` (signed) created at commit `1f5100b`, matching the `0.1.0`/`0.2.0` tagging convention.
- `crates.io` API confirms both `tekstide-core 0.3.0` and `tekstide 0.3.0` exist and are not yanked.

## 0.2.0 - Terminal Runtime Foundation

Status: released on 2026-07-17.

Tekstide `0.2.0` is scoped as an M4 terminal/runtime/security foundation release through RFC-009. It is not the full AI CLI workbench.

### Implemented

- RFC-007 Linux PTY feasibility evidence:
  - PTY-backed shell startup;
  - output capture/rendering in the spike harness;
  - scripted input;
  - resize observation;
  - foreground-child termination, timeout, and SIGKILL fallback observations;
  - output flood and latency evidence.
- RFC-008 TerminalSession/process lifecycle foundation:
  - project-owned Linux plain shell launch;
  - runtime boundary that keeps PTY/process handles out of persisted domain metadata;
  - bounded PTY output reads and dropped-byte accounting;
  - project-addressed input and resize routing;
  - process-group termination with SIGTERM, timeout, SIGKILL fallback, and honest unresolved cleanup outcomes;
  - ProjectSession terminal collection integration and visible-slot policy;
  - project close assessment for real running terminals.
- RFC-009 terminal security boundary:
  - conservative ANSI/VT/OSC parser/security boundary;
  - exact accepted and inert sequence-family policy;
  - inert/diagnostic OSC clipboard, title, hyperlink, host-integration, private-mode, query, reply, unsupported control, and invalid-byte behavior;
  - bounded diagnostics without raw private terminal output, OSC payloads, pasted text, shell output, or environment-like values;
  - typed-input vs paste-input classification before PTY write;
  - multiline paste confirmation decision before PTY write;
  - C0, DEL, and C1 control-containing paste blocking;
  - model-level trusted UI / terminal spoofing boundary;
  - honest Plain/Supervised/Managed labels without command-approval overclaim.

### Deferred

- Desktop GUI runtime and final terminal renderer.
- App/UI commands for launching, selecting, and closing terminals.
- App/UI paste-event wiring, rendered paste confirmation, paste queue, and replay behavior.
- Rendered trusted dialogs and screenshot-backed visual spoofing evidence.
- App-wide close aggregation.
- Cross-platform terminal runtime and GUI security evidence beyond Linux.
- AI CLI profile execution and AgentRun launch.
- Transcript capture, retention, purge, and review workflow.
- Durable audit storage.
- Command approval.
- File watcher and overwrite-confirmation UI.

### Release Gate Status

Completed before release:

- clean working tree;
- `git diff --check`;
- `cargo fmt --all --check`;
- `cargo test --workspace --all-targets`;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `cargo build --release --locked`;
- `cargo package -p tekstide-core --locked`;
- `cargo publish --dry-run -p tekstide-core --locked`;
- `cargo package -p tekstide --locked`;
- `cargo publish --dry-run -p tekstide --locked`;
- `cargo publish --workspace --dry-run --locked`;
- package smoke test from generated package artifacts;
- release-candidate review package accepted.

## 0.1.0 - Foundation Release

Status: released on 2026-07-06.

Tekstide `0.1.0` is scoped as a core/shell foundation release through RFC-006. It is not the full AI CLI workbench.

### Implemented

- Project Board and ProjectSession state.
- Core domain vocabulary for ProjectSession, TerminalSession, AgentRun, approvals, transcripts, change sets, and audit events.
- Navigation/mode policy for Project Board, Content Mode, and Terminal / Agent Immersion Mode.
- Restricted Mode policy/read-model baseline.
- Root-bound project file access policy.
- Bounded explorer read model.
- UTF-8 text document buffer.
- Safe save and external-change detection.
- Dirty-state propagation to project/runtime summaries.
- Shell-visible Content Mode evidence.

### Deferred

- Desktop GUI runtime.
- PTY terminal runtime.
- AI CLI profile execution and AgentRun launch.
- Transcript capture and review workflow.
- Generated diff/artifact review.
- Running-process safe close.
- Paste protection for real terminal input.
- File watcher.
- Overwrite-confirmation UI.
- Durable audit storage.
- Plugin marketplace, remote/container projects, debugger, cloud sync, and collaboration.

### Release Gate Status

Completed before release:

- clean working tree;
- `git diff --check`;
- `cargo fmt --check`;
- `cargo test --all-targets`;
- `cargo clippy --all-targets --all-features -- -D warnings`;
- `cargo build --release --locked`;
- `cargo package -p tekstide-core --locked`;
- `cargo package -p tekstide --locked`;
- package smoke test from generated package artifacts;
- release-candidate review package accepted;
- `tekstide-core` and `tekstide` published to crates.io.

### Future Work Themes

- Terminal/PTY runtime and process lifecycle.
- AgentRun launch and AI CLI profile execution.
- Transcript retention, review, and generated-change workflow.
- Durable audit storage and security evidence.
- Desktop GUI runtime and final Content Mode widgets.
- Release automation/checklist hardening.

See [`rfcs/future-work.md`](rfcs/future-work.md) for the durable deferred-theme index.
