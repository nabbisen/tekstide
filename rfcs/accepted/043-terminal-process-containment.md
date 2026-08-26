# RFC-043: Terminal Process Containment

Status: **Accepted by the human owner 2026-08-26.** Proposed the same day, after a leak
investigation found that this project's termination path cannot reach processes it is responsible
for, and that the recorded explanation for why was wrong. **D1–D4 were decided by the architect on
acceptance** — see "Decided on acceptance" at the end.
Target milestone: **M12**
Date: 2026-08-26

Related RFCs:

- [RFC-008](../done/008-terminalsession-process-lifecycle.md) — owns the process lifecycle this
  RFC changes the ending of.
- [RFC-009](../done/009-terminal-security-boundary.md) — owns the boundary. The descriptor half of
  this finding is a straight violation of it and is being fixed separately and first, in
  [`pty-master-fd-inheritance.md`](../handoffs/pty-master-fd-inheritance.md).
- [RFC-013](../done/013-durable-audit-store-and-local-data-policy.md) — owns the safe-close audit
  record whose honesty depends on the answer to D3.
- [RFC-039](../done/039-interaction-model-and-visible-affordances.md) — shipped the close
  confirmation that tells a user what closing will end.

## Summary

Closing a terminal signals one process group. The processes that terminal started are not all in
that group. Decide what "close this terminal" means, then make the implementation able to mean it.

## What is actually true today, measured

`request_terminate` and `RunningTerminal::drop` both signal `-process_group_id` — the process
group of the shell they launched.

**The shell's own jobs are not in that group.** This application launches `/bin/sh` with no `-c`,
reading commands from the PTY, which makes bash **interactive**, which enables **monitor mode**,
which puts every `&` job in a process group of its own.

Confirmed on 2,115 live orphans: every one has `PID == PGID` — its own group — and a `SID`
belonging to the shell that started it. They are siblings of the signalled group, inside the same
session, and are missed by construction.

**The previously recorded explanation was wrong.** `test-process-leak.md`'s third cause attributed
this to the controlling terminal, *"regardless of whether the shell is 'interactive' in the
traditional sense."* Measured twice — no tty and with a controlling tty — a non-interactive shell
keeps its background job in its own process group, `monitor` off. The controlling terminal is not
what does it. Interactivity is.

That distinction is the reason this RFC exists rather than a one-line patch: **because every job
group lives inside the shell's session, containment is achievable.** Had the cause been what the
document said, nothing short of a container would have worked.

## Why now

Three things forced it, in ascending order of seriousness:

1. **The leak reached the machine.** `/dev/pts` hit 4096 of 4096 twice in one day, blocking a
   developer's gate and a reviewer's. This project requires three to five full-suite runs per
   slice; the leak scales with exactly that.
2. **The disclosed limitation is user-facing.** *"A backgrounded job survives closing its
   terminal"* has been recorded and deferred since 2026-08-25. A user who closes a project after
   `make -j &` has a build still running and a confirmation dialog that told them what would end.
3. **A durable audit record's honesty rests on it.** `SafeCloseDecision::Closed::terminal_process_groups_confirmed_empty`
   was renamed from `fully_confirmed` (request 328) *precisely because* it could not see these
   processes. The rename made the field honest about a gap. Closing the gap changes what the field
   can say.

## The question that makes this an RFC

**What should closing a terminal do to a process the user deliberately backgrounded?**

Both answers are defensible and they are not the same product:

- **Kill it.** "Close" means the terminal and everything it started are gone. Predictable, matches
  what the close confirmation already implies by counting "what will be lost", and is what a user
  who has never thought about process groups expects.
- **Leave it.** A real terminal emulator often does; `nohup` exists for exactly this; a user who
  typed `&` may have meant "outlive this window." Killing it is a larger blast radius than
  today's contract describes.

This project cannot keep deferring it, because the current behaviour is **neither** — it is
whatever the process-group signal happens to reach, which is an accident that a user cannot
predict and the audit trail cannot describe.

## Decisions required

**D1 — kill or leave?** Decide, and make the close confirmation say which *before* the click, in
the shape RFC-034's D4 established for one-way controls. If the answer is "leave", the
confirmation must stop implying otherwise; if "kill", it must say a backgrounded job will be
ended.

**D2 — the containment mechanism.** Candidates: enumerate `/proc` for the session id and signal
each; or a cgroup v2 per terminal with `cgroup.kill`, which a systemd user session already
delegates and which is what terminal emulators and container runtimes use. Enumeration is
portable and racy (PID reuse); cgroups are exact and add a runtime dependency. **Decide against
this project's actual deployment, not in the abstract**, and record what happens where cgroup
delegation is unavailable.

**D3 — what does the safe-close audit record claim afterwards?** If containment is real, the field
renamed in request 328 can say something stronger — but only as strong as the mechanism actually
verifies. **Do not widen the claim ahead of the evidence.** The rename exists because that already
happened once.

**D4 — how does a leaking test fail?** This defect survived a week and exhausted the machine twice
because **a leaking test passes**. A teardown assertion — nothing from this test's session is
alive when it ends — turns a silent leak red. Decide its scope: every test that launches a real
process, or the harness globally.

## Scope

1. Containment per D2, in the real termination path, not only in tests.
2. The close confirmation's wording per D1.
3. The audit record per D3.
4. The leak assertion per D4.

## Non-goals

- **The descriptor inheritance fix.** Separate, first, and already handed off: no product decision
  in it, and a security fix should not wait behind one.
- Changing what a terminal *is*, or how it is launched. Not making the shell non-interactive to
  dodge job control — that would change the product to avoid fixing the runtime.
- Windows or macOS. Linux is the only supported target and cgroup availability is a Linux
  question.

## Risks

- **A blast radius larger than the contract.** If D1 says kill, a user loses a backgrounded build
  they expected to survive. Mitigated only by saying so before the click.
- **Widening the audit claim past the evidence.** D3, and the reason the field has its current
  name.
- **Fixing the harness and calling the product fixed.** The leak is the symptom that made this
  visible; the user-facing defect is that closing a terminal does not do what it appears to.
  A slice that makes the tests clean and leaves `request_terminate` unchanged has fixed nothing
  that ships.

## Acceptance-time decisions

**D1–D4 are decided by the architect on acceptance and recorded in this file before implementation
begins**, the same rule RFC-041, RFC-042 and RFC-034 were accepted under.

---

## Decided on acceptance, 2026-08-26

D1 and D2 turn out to be one decision. Deciding them separately is what would have produced a
wrong answer.

### D1 — **kill.** Closing a terminal ends what that terminal started.

The two candidate answers looked evenly matched until the question was asked in this product's
terms rather than a terminal emulator's.

**Tekstide is not a terminal emulator. It is a supervision surface.** Its reason to exist is
letting a person see and control what an AI CLI agent did. "Closed, except for the things it
started that happen to still be running" defeats the property the whole application is built
around — and the process most likely to have been backgrounded inside an agent's terminal was
backgrounded *by the agent*, not by the user.

**The close confirmation already promises this.** RFC-039 shipped a dialog that names *what will
be lost* by count. Making the behaviour match a promise the product already makes is more honest,
and cheaper, than weakening the promise to match the implementation.

**And the objection is answered by D2 rather than traded away.** "A user who typed `&` may have
meant it to outlive the window" is a real concern. It is not answered by leaving every job
running; it is answered by respecting the boundary Unix already provides for exactly this —
see below.

### D2 — **session-scoped. Not a cgroup.** And this is what makes D1 safe.

A cgroup per terminal is the airtight mechanism, and airtight is the wrong property here.

**A cgroup contains everything, including processes the user deliberately detached.** `nohup`,
`disown`, `setsid` — the standard, documented ways to say "this should outlive my terminal" — all
work by leaving the session. A cgroup ignores that and kills them anyway. Choosing cgroups would
make D1's blast radius genuinely larger than the contract describes, and would remove the user's
only way to opt out.

**Session scope is not a weaker approximation of containment. It is the correct boundary**, and
it is the one Unix already uses to mean "belongs to this terminal." A user who wants a process to
survive has a real, documented, deliberate way to say so, and it works. A user who does nothing
special gets what closing a terminal appears to mean.

So: signal the **session**, and treat a process that has left the session as out of scope **by
design**, not by accident.

### The implementation note that changes the sequence

`request_terminate` today sends SIGTERM, then SIGKILL, to the shell's process group.

**SIGKILL on the shell destroys the very mechanism that would have cleaned up its jobs.** A shell
that receives SIGHUP hups its own jobs — that is what job control is for, and this shell has job
control on, which is the whole reason the jobs are in separate groups. Killing it first removes
its ability to do that, then leaves the orphans behind.

The correct sequence, and the one this RFC asks for:

1. **Close the PTY master**, or send `SIGHUP` to the session leader, and give it a bounded grace
   period. A cooperating shell hups its own jobs here, and most of the work is done by the shell
   that already knows what it started.
2. **Enumerate the session** and signal whatever remains.
3. **Escalate to SIGKILL** for what survives the grace period.
4. **Re-enumerate to confirm empty**, which is what D3 depends on.

Step 1 is not an optimisation. It is the step whose absence created this defect.

### D3 — the audit record may claim exactly what step 4 observed, and no more.

The field renamed in request 328 (`fully_confirmed` → `terminal_process_groups_confirmed_empty`)
exists because a record claimed more than its check could see. **Do not repeat that in the
opposite direction by widening it on the strength of a better mechanism rather than a better
observation.**

- The field becomes **`terminal_session_confirmed_empty`**, and is `true` **only** when step 4's
  re-enumeration actually observed zero processes in that session.
- A grace period that expires, an enumeration that fails, a `/proc` read that races — all produce
  `false`. Not "probably true."
- Its doc comment states what it still does not cover: **a process that left the session is
  outside this claim by design** (D2), and that is the user's opt-out, not a gap.

### D4 — the leak guard lives where the process is created, not in each test.

This defect survived a week and exhausted the machine twice because **a leaking test passes**.

**Do not wire an assertion into each test that launches a process.** The audit-store slice tried
exactly that shape one week ago: an opt-in guard, wired into the 23 sites the handoff named, and
the suite immediately failed 58 *other* tests that reached the same path without anyone knowing.
Per-site wiring is how the twenty-fourth site gets missed.

**`RunningTerminal::drop`, in test builds, verifies its own session is empty and fails loudly if
it is not.** Automatic, unwired, and it covers every test that launches a terminal — including the
ones nobody has written yet.

A test that leaks should be red. Today it is green, and that is the reason all of this is here.
