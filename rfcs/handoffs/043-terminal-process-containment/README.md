---
title: "RFC-043: Terminal Process Containment — implementation handoff"
rfc: "RFC-043"
rfc_file: "../../done/043-terminal-process-containment.md"
source_rfc_status: "Implemented and closed 2026-08-27 — RFC-043 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-26"
---

# Make closing a terminal mean what the dialog already says

Source RFC: [RFC-043](../../done/043-terminal-process-containment.md)

| # | Read | Why |
| --- | --- | --- |
| 1 | [RFC-043](../../done/043-terminal-process-containment.md) | **Read "Decided on acceptance" first.** D1–D4 are settled |
| 2 | [`what-containment-must-not-become.md`](./what-containment-must-not-become.md) | **Required.** This slice kills processes; the risks are all blast radius |
| 3 | [`pty-master-fd-inheritance.md`](../pty-master-fd-inheritance.md) | Lands first. Changes the leak's behaviour, so measure after it, not before |
| 4 | [RFC-009](../../done/009-terminal-security-boundary.md) | Owns the boundary |
| 5 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Three slices |
| 6 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | What must be true and evidenced |

## The one-sentence version

Closing a terminal signals one process group, an interactive shell's jobs live in others, and the
first thing the current code does — `SIGKILL` the shell — destroys the mechanism that would have
cleaned them up.

## The sequence, which is the actual content of this slice

Today: `SIGTERM` the shell's process group, wait, `SIGKILL` it. Orphans remain.

Required:

1. **`SIGHUP` the session leader (or close the PTY master) and wait, bounded.** A shell with job
   control on hups its own jobs when it is hung up — that is what job control is for, and this
   shell has it on, which is *why* the jobs are in separate groups in the first place. Most of the
   work is done here, by the shell that already knows what it started.
2. **Enumerate the session** and signal whatever is left.
3. **Escalate to `SIGKILL`** for survivors of the grace period.
4. **Re-enumerate to confirm empty.** D3's audit claim is this observation and nothing else.

**Step 1 is not an optimisation.** Its absence is the defect. A slice that adds session
enumeration and keeps the SIGKILL-first order will work, badly, and will kill processes that would
have exited cleanly given a chance.

## What is already true, so you do not re-derive it

Measured 2026-08-26, on live processes — do not take these on trust, but do not spend a day
rediscovering them either:

- **The jobs are in their own process groups, inside the shell's session.** All 2,115 orphans:
  `PID == PGID`, `SID` belonging to the dead shell.
- **The cause is interactivity, not the controlling terminal.** A non-interactive shell keeps its
  `&` job in its own process group, `monitor` off, tty or no tty. This app runs `/bin/sh` with no
  `-c` reading from the PTY, so bash is interactive, so job control is on.
- **`test-process-leak.md`'s third cause states the wrong reason** and is annotated as corrected.
  The conclusion there is right; the mechanism is not.

## Ordering against the fd fix

`pty-master-fd-inheritance.md` lands first and **changes what you will measure**. With masters
closed on exec, a runaway job's writes get `EIO` instead of blocking forever, so it reaches its
own exit condition. Some of today's leak disappears without this RFC touching it.

**Measure the per-run leak after that fix, not before**, or you will attribute its improvement to
this work.

## Traps

- **Do not reach for a cgroup because it is airtight.** D2 decided against it with a reason:
  a cgroup also kills what a user deliberately detached, removing the only opt-out and making D1's
  blast radius larger than the RFC authorises. If you think D2 is wrong, say so in writing before
  building — do not implement the tidier mechanism and mention it in the report.
- **PID reuse.** Enumerating `/proc` and signalling by pid races against reuse. Re-verify the
  session id immediately before signalling, and bound your iterations. A containment routine that
  kills an unrelated process is far worse than one that leaves an orphan.
- **Do not make the shell non-interactive to dodge job control.** It would "fix" the leak by
  changing what a terminal is. Named as a non-goal in the RFC.
- **`--test-threads=1` is not a remedy.** It was the diagnostic that found the audit-store cause;
  it is not how this one gets closed either.

## Live evidence

Required, and against a **`mktemp -d` fixture project** — never a path under `$HOME`, per
`ARCHITECTURE.md`. State whether a real mouse click was sent, either way.

The walkthrough must show a real backgrounded process (`sleep 300 &` is enough), the close
confirmation, and — after — a real `kill -0` on that pid failing. **An OS-level check, not an
inference from the dialog.** That is the shape request 328's own evidence used to prove the
opposite property, and it is the shape that proves this one.

## Deferrals to state, not to solve

- Anything that left the session (`nohup`, `disown`, `setsid`) is **out of scope by design**, not
  a gap. It is the user's opt-out and D3's doc comment must say so.
- Non-Linux. Not supported, and session semantics are a Linux question here.
