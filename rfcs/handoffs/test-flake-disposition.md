---
title: "Test flake disposition pass: decide every live row, do not carry it"
status: "**Scheduled 2026-10-09**, after `0.32.0`. Test-only, no RFC — the same arrangement the rows 2/5 fix already used during RFC-027's cycle."
rfc_file: "— no RFC; test-only, scheduled by the architect at review 497"
target_milestone: "M13"
created: "2026-10-09"
---

# Test flake disposition pass

## Why now, with the number

Measured across reviews 494–497, on both the dev team's machine and the architect's: **about 3
failures in 17 full-workspace runs.** The gate rule is *three consecutive green*, so at that rate a
three-run gate passes first attempt roughly **half** the time, and `0.32.0`'s own cycle needed its
gate redone twice.

`test-process-leak.md` has done its job — every one of these was recorded, dated and diagnosed. What
has not happened is a **decision**. Rows 2 and 5 were fixed at review 476 because somebody decided
about them; nothing has been decided about the rest.

## The scope is smaller than the register looks

A naive count of table-shaped lines gives 23. That over-counts: it includes the recurrence tables
and rows **already fixed but never retired**. The genuinely live set is **about a dozen**, and
separating the two is the first task — a register whose resolved rows still read as open makes the
problem look worse than it is and harder to act on.

**Already fixed** (confirm, then mark retired in place, as rows 2 and 5 already are):
`agent_run_queue_limit_is_enforced_and_only_counts_live_entries`,
`is_still_answerable_reflects_the_real_connection_state`,
`a_live_writer_holds_an_exclusive_lock_until_it_is_dropped`,
`local_bounded_marks_capture_failed_and_keeps_reading…` and its sibling,
`the_wake_notifier_wakes_when_real_pty_output_arrives`.

## The three verdicts

Every live row gets exactly one, written into the register beside it:

1. **Fix** — the cause is understood and the fix is in the test. Prove it by ablation: the test must
   fail without the fix.
2. **Quarantine** — the test does not belong in a correctness gate. Say so and move it out of the
   three-run gate (an `--ignored` benchmark, say), **with the reason**.
3. **Accept** — carried deliberately, with a reason that is not "we have not got to it".

**The pass is done when every live row has a verdict, not when every row is fixed.** A row accepted
with a stated reason is a finished row.

## What to look at first, grouped by cause

**A — the wait is weaker than the condition it waits for.** The cause this register has already
diagnosed twice, and the one rows 2 and 5 are the worked example of.
`a_real_backgrounded_job_is_dead_after_a_real_close` and
`a_job_that_leaves_the_session_via_setsid_survives_a_real_close` are **already diagnosed down to the
line** (review 478): the wait is for the marker `BGPID=`, which the shell's own echo of the
unexpanded command satisfies before the digits arrive. `drain_available_never_blocks_the_caller…`
is the same family. These are the cheapest wins and they are ready to fix.

**B — PTY read timing.** `resize_makes_the_pty_the_emulator_and_the_render_path_agree` (four
occurrences) and `closing_a_project_with_a_backgrounded_descendant_kills_it_through_a_real_close`
(on file since the `0.16.0` gate, and it failed the architect's own gate at review 497).

**C — wall-clock budgets in a correctness gate.** `terminal_poll_handler_cost_under_a_real_wake_driven_flood_headless_benchmark`
and `change_review_content_view_build_cost_by_line_count_measurement` — the latter *was* fixed for
`0.29.0` by rewriting it as a paired ratio, and recurred anyway. **These are the strongest
quarantine candidates**: a measurement that asserts a time budget is not a flaky correctness test,
it is a benchmark being asked to do a job it cannot do on a loaded shared machine. Deciding that
honestly is worth more than another attempt to stabilise them.

**D — OS and filesystem edges.** `bind_recovers_from_a_stale_socket_file`,
`every_failure_to_answer_is_unknown_and_never_none_ignored` (suspected `ETXTBSY`),
`purge_reports_deferred_cleanup_while_wal_reader_is_active`.

**E — unconfirmed.** `change_review_surface_renders_a_real_change_set_from_a_real_agent_run` and
`a_real_low_risk_proposal_is_received_mirrored_and_stays_queued_without_promoting` are candidates
never confirmed. **"Cannot reproduce" is a verdict**, if it is written down with what was tried.

## The second half, easy to forget

**A failing test in this family fails the gate twice** — once on its assertion, once on the fixture
it leaves behind, because cleanup is an ordinary call at the end of the test body rather than a drop
guard. See **Addendum, 2026-10-08 — review 480**. Whatever fixes a row's wait should make its
fixture clean itself on unwinding, in the same change.

## What this pass must not do

| # | It must not | Why |
| --- | --- | --- |
| 1 | **Delete a row's history.** | The dated recurrence sections are the evidence that a cause is real and how often. Mark rows retired in place, as rows 2 and 5 are. |
| 2 | **Weaken an assertion to make a test pass.** | A test that no longer checks the thing is worse than one that fails sometimes — it fails silently, forever. Widen a *wait*, never a *claim*. |
| 3 | **Fix by adding a sleep.** | A fixed sleep is a wait weaker than its condition with extra steps. Poll the condition, bounded. |
| 4 | **Report a fix that was not ablated.** | Reviews 482, 492 and 500 each found a check that could not fail. This pass is entirely about tests; a fix whose test still passes without it is not a fix. |
| 5 | **Count a gate run that failed.** | Three consecutive green, not three attempts (review 497). |

## Acceptance

- [ ] Already-fixed rows confirmed and marked retired in place; the live set stated as a number.
- [ ] **Every live row carries one of the three verdicts**, written in the register beside it.
- [ ] Every row verdicted **Fix** has its fix ablated — the test fails without it.
- [ ] Every row verdicted **Quarantine** names where it went and why it does not belong in the gate.
- [ ] Every row verdicted **Accept** gives a reason that is not scheduling.
- [ ] Fixture cleanup moved to a drop guard wherever a row's wait was fixed.
- [ ] The measured failure rate re-taken afterwards, on the same basis — **a count of runs, not an
      impression.** Before: ~3 in 17.
- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `rfc_docs_invariants`,
      `cargo test --doc --workspace`, **three consecutive full-workspace runs**, 0 fixture entries
      in a fresh short fixed `TMPDIR`.
