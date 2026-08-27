---
title: "What containment must not become"
rfc: "RFC-043"
rfc_file: "../../done/043-terminal-process-containment.md"
source_rfc_status: "Implemented and closed 2026-08-27 — RFC-043 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-26"
---

# What containment must not become

**Required reading before writing code.** Every previous document in this family was about a
surface claiming more than it could support. **This one is different: this slice kills processes.**
The failure mode is not a false sentence, it is a user losing work.

## §1 The thing that must not happen

**Tekstide must never kill a process it was not responsible for.**

Not "rarely." A containment routine that enumerates `/proc` and signals by pid is racing against
pid reuse: between reading a pid and signalling it, that pid can belong to something else — a
build, an editor, a database, a different Tekstide project's terminal.

An orphaned shell costs a PTY slot. A wrongly-signalled pid costs a user their unsaved work, and
they will have no idea what did it, because nothing on screen will say so.

**Asymmetry to internalise: leaving an orphan is a bug. Killing a stranger is an incident.**
When the two trade against each other, leave the orphan.

Concretely: re-verify the session id immediately before every signal, bound your iterations, and
if the enumeration cannot establish that a pid is still in the target session, **do not signal
it.**

## §2 The opt-out is load-bearing, not a nicety

D1 says kill. D1 is only defensible because D2 says *session*, and a user has a real way to leave
the session: `nohup`, `disown`, `setsid`.

**If you widen the mechanism, D1 stops being justified.** A cgroup, a process-tree walk by parent,
a "kill everything with this project's cwd" heuristic — each is tidier and each removes the user's
only way to say "this should outlive my terminal."

The RFC decided against cgroups for exactly this. If you become convinced session scope is
insufficient, **that is an argument to reopen D1 and D2 together, in writing, before building** —
not a reason to quietly implement the stronger thing.

## §3 Say it before the click

RFC-034 D4 established the rule for one-way controls: **an irreversible action says so while the
control is still live**, not afterwards.

Killing a user's backgrounded build is irreversible. The close confirmation already names what
will be lost by count; after this slice, that count must include what the session contains, and
the wording must make clear that things started from those terminals end too.

**Not acceptable**: shipping the kill and leaving the dialog as it is because it was "already
about right." It was already about right for a behaviour that did not happen.

## §4 The audit record must not improve faster than the evidence

`terminal_process_groups_confirmed_empty` has its current name because a field once claimed more
than its check could see (request 328). The temptation now runs the other way: a better mechanism
makes it tempting to assert a stronger fact.

**The field may claim exactly what step 4's re-enumeration observed.** A grace period that
expired, a `/proc` read that failed, an enumeration that raced — all `false`. Not "almost
certainly empty."

And its doc must say what remains outside the claim: a process that left the session. That is not
a gap to be embarrassed about; it is §2's opt-out working. Say so, so the next reader does not
"fix" it.

## §5 What you may not do to make this easier

- **Do not make the shell non-interactive.** It would remove job control and thus the separate
  process groups — "fixing" the leak by changing what a terminal is. The RFC names it a non-goal.
- **Do not skip the SIGHUP step** because enumeration alone appears to work. The shell hupping its
  own jobs is the path that lets processes exit cleanly; enumeration-then-SIGKILL kills things
  that would have shut down properly, and this project runs AI agents that may be mid-write.
- **Do not treat the test suite's cleanliness as the deliverable.** The leak is the symptom that
  made this visible. The defect that ships is that closing a terminal does not do what it appears
  to. A green suite with `request_terminate` unchanged has fixed nothing a user will ever see.
- **Do not let D4's guard become opt-in.** One week ago the audit-store slice wired a guard into
  the 23 sites its handoff named and the suite failed 58 *other* tests reaching the same path.
  Per-site wiring misses sites. The guard belongs in `RunningTerminal::drop`.

## §6 If the honest answer is that session scope is not enough

It might not be. A process that calls `setsid` for reasons of its own — not because a user
detached it — leaves the session and survives, and you may find a real case where that matters.

**Report it. Do not widen the mechanism to cover it.** The right response is a decision about
whether D1's promise can be kept at all, made deliberately, with the confirmation's wording
changed to match — which is the same trade this project took when it renamed a field rather than
narrowing a predicate it could not honestly narrow.

A containment routine that mostly works, with a dialog that promises it always does, is worse
than today's honest gap.
