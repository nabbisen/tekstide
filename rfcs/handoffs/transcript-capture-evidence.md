---
title: "Transcript capture evidence — assert what the real launch path actually does"
status: "Scheduled 2026-08-18, awaiting implementation"
rfc_file: "../done/011-transcript-retention-and-local-data-policy.md"
target_milestone: "M11"
created: "2026-08-18"
---

# Transcript capture evidence

## Why

`0.10.0` and `0.11.0` both told users that Tekstide writes no transcripts. It writes one for
every AI CLI run. The owner has confirmed the **behaviour is intended**; the documentation
was wrong and is corrected.

**The reason it shipped twice is that no test asserts transcript behaviour on the real launch
path.** `shell/tests.rs` has real-process agent-run tests — including one that drives a run to
exit and checks a `ChangeSet` — and none of them looks at whether a transcript exists. The
suite could not contradict the claim, so the claim was checked by grep, in one crate, and
asserted about two.

This slice makes the behaviour observable to the suite.

## What to build

A test on the **real launch path** — the one `Ctrl+Alt+A` reaches, the same
`attempt_agent_run_launch_with_profile` the existing end-to-end tests use — asserting that a
real agent run produces a real transcript file with real content in it.

Point the state root at a temporary directory rather than the user's real
`$XDG_STATE_HOME`. `open_real_agent_run_state_root()` resolves the live one, so the test needs
the injectable seam; if none exists, adding one is part of this slice and is preferable to a
test that writes into the developer's own state directory.

Assert, at minimum:

1. A transcript file exists at the documented path shape —
   `<state root>/transcripts/<project>/<agent-run>/transcript.log`. The **path shape is part
   of the published privacy disclosure** now, so a test that only checks "some file exists"
   does not protect the claim.
2. It contains bytes the run actually produced. Have the controlled test executable emit a
   known marker and assert the marker is present — otherwise an empty file passes.
3. **A plain terminal (`Ctrl+Alt+T`) produces no transcript.** The README says only AI CLI
   runs are recorded; that is a second claim and it needs its own assertion.

## The ablation

Remove the `with_local_bounded_transcript` call in `attempt_agent_run_launch_with_profile`,
confirm the test fails naming the missing transcript, restore it. A test that cannot notice
capture being switched off is not evidence that capture happens.

## The gate

- The test drives the **real launch path**, not a directly-constructed request.
- The asserted path shape matches `README.md`'s *Local Data and Privacy* section exactly. If
  they differ, **the README is what needs correcting** — it is the published claim — and say
  so rather than adjusting the test to match the code.
- The plain-terminal negative is asserted, not assumed.
- No test writes into the developer's real `$XDG_STATE_HOME`.
- The evidence states what this does **not** establish: that the retention bounds are
  enforced. Those are RFC-011's and already tested at the core level; this slice is about
  whether capture is *reached* from the GUI, which is the thing that was wrong.

## Not in scope

- A per-run opt-out or an in-app purge. Both are designed in RFC-011 and neither has a
  user-facing route; that is a real gap, recorded in `README.md` as a stated limitation, and
  it is its own slice with its own UI decisions.
- Changing capture defaults. The owner has decided capture is intended.
