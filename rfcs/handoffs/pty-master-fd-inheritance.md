---
title: "PTY master fd inheritance — security fix handoff"
rfc: "none"
source_rfc_status: "No RFC. A security fix with no product decision in it; see 'Why this is not in RFC-043'."
target_milestone: "M12"
created: "2026-08-26"
---

# Every spawned shell inherits every other terminal's PTY master

**Security fix. Start this before RFC-043 is accepted.**

Full finding, with the measurements:
`.git-exclude/reviewed/tekstide-finding-pty-master-fds-inherited-by-every-child.md`.

## The defect in one paragraph

`OpenPty::new` (`crates/tekstide-core/src/runtime/terminal/pty.rs`) opens the PTY with
`libc::openpty(...)`. glibc's `openpty` does not set `O_CLOEXEC`, and nothing here adds it —
`set_nonblocking` is applied to the master, `FD_CLOEXEC` is not; the only `CLOEXEC` in this
runtime is on an `eventfd` in `reader.rs`. `spawn_pty_child` duplicates the slave for
stdin/stdout/stderr and the controlling terminal, calls `setsid`, attaches the ctty, closes that
one duplicate — and never closes the master, or any other descriptor the process already holds.
So at `exec`, every child inherits **every PTY master open in the parent at that moment**.

Measured on a live process: one `/bin/sh` holding **27 `/dev/ptmx` descriptors** for terminals
that are not its own.

## Why this is a security fix and not tidying

A PTY master is read/write access to that terminal. A user with two projects open has two
terminals; the shell in the second inherits the master of the first. Anything running there —
**including an AI CLI agent, which is what this application exists to run** — can read what the
other project's terminal displays and inject input into it. No capability check, no audit record,
nothing visible.

RFC-009 defines the terminal security boundary. This crosses it, in production code, in the
shipped binary.

## Why this is not in RFC-043

RFC-043 covers termination semantics and contains a real product decision (what closing a
terminal should do to a job the user backgrounded). **This has no decision in it.** Descriptors
the child does not need should not reach the child; there is no version of this project's
security model where the current behaviour is intended.

Making a security fix wait behind a decision is how a defect stays shipped while people discuss
it. Take this one now.

## Scope

1. **`FD_CLOEXEC` on the PTY master**, at open. Setting it in `OpenPty::new` right where
   `set_nonblocking` already runs is the obvious home.
2. **Audit every other descriptor this runtime opens** and give it the same treatment, or close it
   in `pre_exec`. The master is the one I measured; it is not necessarily the only one. The
   `eventfd` in `reader.rs` already gets `EFD_CLOEXEC` — take that as the pattern, and **enumerate
   the sites rather than fixing the one this document names.**
3. **A belt-and-braces `pre_exec` close** of everything above fd 2 is a legitimate addition, not a
   substitute for (1). If you add it, say why — it protects against descriptors opened by code
   that has not been audited yet, which is a real category.

## Acceptance

- [ ] **The defect is reproduced before it is fixed.** Launch two real terminals, list the second
      child's `/proc/<pid>/fd`, show a `/dev/ptmx` that belongs to the first. Record it. A fix for
      a defect nobody watched happen is a fix for a defect nobody has established.
- [ ] **After: the same walkthrough shows no `/dev/ptmx` in the child's fd table at all.**
- [ ] **A test holds this property.** A real launched child's fd table contains no master. This is
      the one that stops it coming back — a comment cannot.
- [ ] Ablated: remove the `CLOEXEC`, watch that test fail, restore.
- [ ] Every descriptor site enumerated (item 2), with the list in `qa-evidence.md` — including any
      you decided needs no change, and why.
- [ ] Gates. **Three consecutive full-workspace runs**, and note that this fix is expected to
      reduce the leak: with masters closed on exec, a runaway job's writes get `EIO` instead of
      blocking forever, so it reaches its own exit. **Measure the per-run leak before and after**
      and record both numbers. That measurement is part of RFC-043's evidence too.

## What this does not fix

The backgrounded job still escapes termination — it is in its own process group inside the
shell's session, and `request_terminate`/`RunningTerminal::drop` signal one group. That is
RFC-043. This fix stops the leak sustaining itself and closes the descriptor hole; it does not
change what "close this terminal" means.


---

## Accepted 2026-08-26 (review 338, commit `0673d80`)

Verified by the reviewer by breaking it, not by reading it:

- Both layers ablated → the test fails, `found 2 such descriptor(s)`.
- `set_cloexec` alone → passes. `close_range` alone → passes. **Each layer independently closes
  the hole**, so neither is decoration and deleting either is a real regression.
- Benchmark run: `/dev/pts` **3 → 3**, shells 6 → 34. Descriptor hole closed; the
  job-escapes-termination defect untouched, correctly left to RFC-043.
- The 28 orphans are `state=R`, `ptmx_fds=0` — no inherited masters, running rather than blocked —
  and were **back to the pre-run baseline of 6** minutes later. They now hit `EIO`, reach
  `FLOOD_SCRIPT`'s own 30-second deadline, and exit. **The leak is no longer self-sustaining**,
  which was what turned a bounded per-run cost into an exhausted machine.
- The descriptor enumeration re-run independently: `eventfd` has `EFD_CLOEXEC`, the approval
  channel's `open`/`openat` both pass `O_CLOEXEC`, and `duplicate_slave`'s `dup` correctly has
  none because those become the child's stdio. The master and slave were the only two.

**Baseline for RFC-043 to improve on:** 28 orphans per benchmark run, PTY occupancy flat, orphans
self-terminating within 30 seconds.

Gate accepted at 12 full-workspace runs (5 clean) rather than three consecutive: `/dev/pts` held
at baseline in **all 12**, which is direct evidence about the property this fix changes, where
three green suites would have been weaker evidence at higher cost.
