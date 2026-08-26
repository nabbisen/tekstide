---
title: "RFC-043 task breakdown and PR plan"
rfc: "RFC-043"
rfc_file: "../../accepted/043-terminal-process-containment.md"
source_rfc_status: "Accepted 2026-08-26 — M12"
target_milestone: "M12"
created: "2026-08-26"
---

# Three slices

**`pty-master-fd-inheritance.md` lands before any of these.** It changes the leak's behaviour, so
every measurement here must be taken after it.

## PR-043-A — make the leak red

**No containment yet. No behaviour change for a user.** This is D4, and it goes first so that
every later slice has a test that fails when it leaks.

`RunningTerminal::drop`, in test builds, enumerates its own session and fails loudly if anything
is still alive.

- **Not opt-in, and not wired per test.** One week ago the audit-store slice wired a guard into
  the 23 sites its handoff named; the suite then failed 58 *other* tests reaching the same path.
  Put it where the process is created.
- Expect this to **turn the benchmark red immediately**. That is the slice working. Do not
  suppress it — record it as the reproduction.

**Evidence:** the count of tests that fail once the guard is live, and which ones. That list is
the real inventory of what leaks, and nobody has one today.

**Gate:** the suite will not be green at the end of this slice, by design. Say so plainly; do not
skip the guard's rollout to keep a green run.

## PR-043-B — the sequence

The containment routine, in `request_terminate` and `RunningTerminal::drop`:

1. `SIGHUP` the session leader (or close the PTY master), bounded grace period.
2. Enumerate the session; signal what remains.
3. Escalate to `SIGKILL` for survivors.
4. Re-enumerate to confirm empty.

**Step 1 first.** The current code's SIGKILL-first order is the defect; adding enumeration in
front of the same order fixes the symptom and keeps the cause.

**§1 of the security document governs step 2.** Re-verify the session id immediately before every
signal; bound iterations; if you cannot establish a pid is still in the target session, do not
signal it. Leaving an orphan is a bug. Killing a stranger is an incident.

**Required tests:**

- A real backgrounded job (`sleep 300 &`) in a real terminal is gone after a real close —
  proven by `kill -0` on its real pid, not inferred.
- A process that `setsid`s away **survives**, and that is asserted, not tolerated. This is D2's
  opt-out and it needs a test saying so on purpose.
- The grace period expiring produces `false` from step 4, not a hopeful `true`.
- PR-043-A's guard now passes for the benchmark.

**Ablations:** remove step 1 → the clean-exit test fails; remove the session re-verification →
the race test fails.

## PR-043-C — say it, and record it

- **The close confirmation** names that things started from these terminals end too (D1 + RFC-034
  D4's rule: before the click, while the control is live).
- **`terminal_process_groups_confirmed_empty` → `terminal_session_confirmed_empty`**, set only
  from step 4's real observation. Its doc states what stays outside the claim: a process that left
  the session, by design.
- The disclosed limitation in `test-process-leak.md` and anywhere else it is recorded is corrected
  — this slice makes those statements false, and correcting them is part of it.

**Required test:** the audit field is `false` when step 4 could not confirm. A test that only
proves the `true` case proves the easy half.

## Not in this plan

- Cgroups (D2, decided, with a reason — see the security document §2).
- Making the shell non-interactive.
- Non-Linux.
- The fd inheritance fix — separate, first, already handed off.
