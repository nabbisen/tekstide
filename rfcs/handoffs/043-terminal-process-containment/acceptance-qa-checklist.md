---
title: "RFC-043 acceptance and QA checklist"
rfc: "RFC-043"
rfc_file: "../../accepted/043-terminal-process-containment.md"
source_rfc_status: "Accepted 2026-08-26 — M12"
target_milestone: "M12"
created: "2026-08-26"
---

# Acceptance and QA checklist

## The claim this slice exists to be able to make

- [x] **A real backgrounded process is dead after a real close**, proven by `kill -0` on its real
      pid returning failure. An OS-level check, not an inference from the dialog.
      `a_real_backgrounded_job_is_dead_after_a_real_close`.

## D1 / D2 — kill, session-scoped

- [x] The containment routine signals by **session**, and nothing else. The session leader gets a
      single, positive-pid `SIGHUP`; survivors are enumerated and `SIGKILL`ed individually. No
      `-process_group_id`/`-session_id` group-wide target anywhere in the new sequence.
- [x] **A process that left the session survives, and a test asserts it does.** This is the
      opt-out D1's justification rests on — it is a property, not a leftover.
      `a_job_that_leaves_the_session_via_setsid_survives_a_real_close`.
- [x] No cgroup. Not reached for; session-scoped signalling as decided.

## The sequence

- [x] `SIGHUP`/master-close comes **first**, with a bounded grace period.
- [x] Ablated: remove it, watch a process that would have exited cleanly get `SIGKILL`ed instead.
      Measured, not just observed a difference: the clean-exit test still eventually succeeds via
      the fallback, but in 2.02s instead of near-instant -- "works, badly," exactly as warned.
      `qa-evidence.md`.
- [x] Escalation to `SIGKILL` only after the grace period.
- [x] Step 4 re-enumerates and its result is what D3 records.

## §1 — never kill a stranger

- [x] The session id is re-verified immediately before every signal.
- [x] Iterations are bounded (`MAX_SESSION_SIGNAL_ITERATIONS`, 256 -- defensive, not realistic).
- [x] A pid whose session cannot be established is **not signalled**, and a test covers that path.
      `signal_candidates_never_signals_a_pid_outside_the_target_session`, a real unrelated
      `sleep 300` process standing in for what a PID-reuse race could hand the enumeration.
      Ablated: removing the check signals the stranger too (`left: 2, right: 1`); restored.

## D3 — the audit record claims only what was observed

- [x] `TerminalRuntimeEvent::SessionConfirmedEmpty { confirmed }`, `true` only from step 4's real
      observation -- the production-side signal PR-043-C will read for the field rename below;
      not yet wired into `shell.rs`'s own audit recording (that wiring is PR-043-C's own scope).
- [x] **A test proves the `false` case** — enumeration failed
      (`session_confirmed_empty_reports_false_when_its_own_enumeration_fails`). Not built by
      racing a grace period against real `SIGKILL` reaping speed -- measured directly that this
      machine reaps close enough to instantaneously that a `Duration::ZERO` grace period still
      observed a real survivor as gone; see `qa-evidence.md` for why the enumeration-failure path
      is the one this test forces instead, and the real, second bug in `processes_in_session`
      found while trying.
- [x] Its doc states what remains outside the claim, and that this is by design. Present on
      `SessionConfirmedEmpty`'s own doc comment (`types.rs`); the *audit-facing* doc -- the renamed
      `SafeCloseDecision::Closed::terminal_session_confirmed_empty` (`audit/integration.rs`) --
      states it too, and `terminate_project_live_work` (`shell.rs`) now reads this field's own
      value directly instead of inferring it, PR-043-C's own scope, done.

## D4 — a leaking test is red

- [x] The guard is in `RunningTerminal::drop`, not wired per test. `assert_session_is_empty`,
      called unconditionally at the end of the existing `Drop` impl.
- [x] **The inventory is recorded**: which tests failed when the guard first went live. Four,
      stable across three runs on each crate -- full table and per-test reason in `qa-evidence.md`.
- [x] Ablated: reintroduce a leak, watch the guard fail. Commented out the guard's `#[cfg]` and
      call, re-ran the sigterm-overclaim test: passed silently despite the real leak. Restored:
      failed again. `qa-evidence.md`.

## Wording

- [x] The close confirmation says things started from these terminals end too, **before the
      click**, while the controls are live (RFC-034 D4's rule). `project-close-dialog-running-process-detail`,
      shown only when the close names a running process as a reason -- proven present and absent
      by test, not only present.
- [x] Every statement this slice makes false is corrected — `test-process-leak.md`'s third cause,
      the README, and anywhere else the surviving-job limitation is recorded. `test-process-leak.md`'s
      own three "still open, PR-043-C" notes (frontmatter, the job-escapes-termination section, the
      `KilledAfterTimeout` question) updated to state what this slice did.

## Measurement

- [x] **Per-run leaked-process count, before and after**, taken *after* the fd-inheritance fix
      landed so its improvement is not attributed here. Before (fd-inheritance fix present,
      PR-043-B not yet): 28 orphaned shells immediately after the N-pane benchmark (measured in
      that RFC's own evidence). After PR-043-B: **0** -- re-measured directly against this slice's
      own code, not assumed to carry over.
- [x] `/dev/pts` occupancy across a full suite run, before and after. Flat at 13 (this session's
      own baseline) across four consecutive full-workspace runs with PR-043-B applied, identical
      to PR-043-A's own flat baseline -- this slice does not change PTY occupancy (the
      fd-inheritance fix already closed that), it changes whether the processes behind that
      occupancy are actually gone.

## Live GUI evidence

- [x] Against a **`mktemp -d` fixture project**, and a fresh `mktemp -d` `XDG_STATE_HOME`
      (response 345's own required addition). No path under `$HOME`, no real project name, no
      real file content.
- [x] Shows: a real backgrounded process (`REALPID=2194464`, a genuine `sleep 300`), the close
      confirmation with its new wording ("Anything started from these terminals ends too,
      including a backgrounded job."), and the `kill -0` failing afterwards -- an OS-level check,
      confirmed both immediately before the click (still alive) and after (`no such process`).
      Three screenshots, `evidence/EVIDENCE-{1,2,3}-*.png`.
- [x] Whether a real mouse click was sent is stated either way. **Yes** -- the confirming `Close`
      press was a real `xdotool` mouse click, targeted by window id.

## Gates

- [x] `fmt`, `clippy -D warnings`, `git diff --check`, `rfc_docs_invariants`. Clean as of
      PR-043-B's own diff. Whole-RFC gate; re-check once more after PR-043-C lands.
- [x] Full workspace suite, **three consecutive runs**, each logged to a file. Four runs taken:
      three clean (444/444, 746/746, 2/2), one hit a single pre-existing, already-documented,
      unrelated flake (`is_still_answerable_reflects_the_real_connection_state`,
      `test-process-leak.md` row 5/6) that passed on immediate isolated re-run. `/dev/pts` flat at
      13 across all four -- see Measurement, above.
- [x] **PR-043-A is expected to end red.** Say so; do not delay the guard to keep a green run.
      Said: 4 tests red, stable across 3 runs on each crate, full inventory in `qa-evidence.md`.
      (Historical, from that slice's own review -- the suite is green again as of PR-043-B, above.)

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
