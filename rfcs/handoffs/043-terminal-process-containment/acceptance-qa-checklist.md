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

- [ ] **A real backgrounded process is dead after a real close**, proven by `kill -0` on its real
      pid returning failure. An OS-level check, not an inference from the dialog.

## D1 / D2 — kill, session-scoped

- [ ] The containment routine signals by **session**, and nothing else.
- [ ] **A process that left the session survives, and a test asserts it does.** This is the
      opt-out D1's justification rests on — it is a property, not a leftover.
- [ ] No cgroup. If you believe D2 is wrong, that argument is in writing and this box is
      unchecked, not quietly satisfied by a tidier mechanism.

## The sequence

- [ ] `SIGHUP`/master-close comes **first**, with a bounded grace period.
- [ ] Ablated: remove it, watch a process that would have exited cleanly get `SIGKILL`ed instead.
- [ ] Escalation to `SIGKILL` only after the grace period.
- [ ] Step 4 re-enumerates and its result is what D3 records.

## §1 — never kill a stranger

- [ ] The session id is re-verified immediately before every signal.
- [ ] Iterations are bounded.
- [ ] A pid whose session cannot be established is **not signalled**, and a test covers that path.

## D3 — the audit record claims only what was observed

- [ ] `terminal_session_confirmed_empty`, `true` only from step 4's real observation.
- [ ] **A test proves the `false` case** — grace period expired, or enumeration failed. A test of
      only the `true` case proves the easy half.
- [ ] Its doc states what remains outside the claim, and that this is by design.

## D4 — a leaking test is red

- [x] The guard is in `RunningTerminal::drop`, not wired per test. `assert_session_is_empty`,
      called unconditionally at the end of the existing `Drop` impl.
- [x] **The inventory is recorded**: which tests failed when the guard first went live. Four,
      stable across three runs on each crate -- full table and per-test reason in `qa-evidence.md`.
- [x] Ablated: reintroduce a leak, watch the guard fail. Commented out the guard's `#[cfg]` and
      call, re-ran the sigterm-overclaim test: passed silently despite the real leak. Restored:
      failed again. `qa-evidence.md`.

## Wording

- [ ] The close confirmation says things started from these terminals end too, **before the
      click**, while the controls are live (RFC-034 D4's rule).
- [ ] Every statement this slice makes false is corrected — `test-process-leak.md`'s third cause,
      the README, and anywhere else the surviving-job limitation is recorded.

## Measurement

- [ ] **Per-run leaked-process count, before and after**, taken *after* the fd-inheritance fix
      landed so its improvement is not attributed here.
- [ ] `/dev/pts` occupancy across a full suite run, before and after.

## Live GUI evidence

- [ ] Against a **`mktemp -d` fixture project**. No path under `$HOME`, no real project name, no
      real file content.
- [ ] Shows: a real backgrounded process, the close confirmation with its new wording, and the
      `kill -0` failing afterwards.
- [ ] Whether a real mouse click was sent is stated either way.

## Gates

- [ ] `fmt`, `clippy -D warnings`, `git diff --check`, `rfc_docs_invariants`. Clean as of
      PR-043-A's own diff (`qa-evidence.md`); re-check after PR-043-B/C land, since this is a
      whole-RFC gate, not a per-slice one.
- [ ] Full workspace suite, **three consecutive runs**, each logged to a file. Not applicable to
      PR-043-A on its own -- it is expected to end red, by design (below). PR-043-B is what makes
      three-clean-runs a meaningful gate again.
- [x] **PR-043-A is expected to end red.** Say so; do not delay the guard to keep a green run.
      Said: 4 tests red, stable across 3 runs on each crate, full inventory in `qa-evidence.md`.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
