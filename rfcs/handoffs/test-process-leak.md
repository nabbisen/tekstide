---
title: "The leaked-child test flake — cause known since 2026-08-16, still unfixed"
status: "Complete 2026-08-20 — accepted (request 282). Fixes the leak at its source; the socket flake is a separate, still-open defect"
rfc_file: "none — a test-harness defect, not product behaviour"
target_milestone: "M12"
created: "2026-08-19"
---

# The leaked-child test flake

## Why this is being scheduled now

**The cause was found on 2026-08-16 and has never been fixed.** From `future-work.md`:

> `Child::drop` does not kill the process, so **any test that panics before reaching its own
> cleanup leaks a shell process**

Since then, **three distinct tests** have been reported failing intermittently under the
resulting pressure, each disclosed separately and each moved past:

| test | first reported |
| --- | --- |
| `approval::tests::channel::bind_recovers_from_a_stale_socket_file` | the original, response 213 |
| `approval::tests::coordinator::agent_run_queue_limit_is_enforced_and_only_counts_live_entries` | request 260 |
| `command_approval_family_produces_real_durable_audit_records_through_the_pipeline` | request 276 |

Every one of those disclosures was the right individual call — reported rather than re-run
past, confirmed non-deterministic in isolation, not attributed to the slice that saw it.
**Collectively they are the problem**: a diagnosed defect is being re-observed instead of
repaired, and each new symptom costs a reviewer and an implementer the time to establish it is
the same old thing.

Measured rate, from the sampling already recorded: **3 failures in 150 full-suite runs.** Not
enough to block work. Enough that every contributor will meet it.

## Why it matters beyond tidiness

The affected area is **the command-approval and socket path** — the security-critical machinery
RFC-021 and RFC-022 built. A suite that fails intermittently there trains everyone to re-run
rather than investigate, which is precisely the habit that hides a real regression in that code
the first time one appears.

## What to build

A guard that kills a spawned child on drop, applied to the test helpers that spawn real
processes, so a panicking test cannot leak one. `std::process::Child::kill` plus `wait`, in a
`Drop` impl on a wrapper the helpers return instead of a bare `Child`.

Find every test helper that spawns a real process and returns it. The reachability-audit
technique applies: enumerate them mechanically rather than by reading.

## The gate

- **Measure before and after, the same way.** The existing figure is 3/150 full-suite runs. A
  fix claimed without a comparable post-measurement is a fix claimed on hope — and this
  project's own convention is to measure bounds rather than estimate them.

  **Corrected 2026-08-20, at acceptance: this gate item pointed the measurement at the wrong
  quantity, and it is mine.** The 3-in-150 baseline measures the *ambient* rate of the
  `bind_recovers_from_a_stale_socket_file` flake. This fix does not target that. It prevents a
  **panicking** test from leaking a process — so in a run where nothing panics, which is every
  passing run, the fix cannot have an effect and a before/after comparison of passing runs
  measures load, not the change. The implementer ran the comparison anyway (27/200 vs 29/200),
  recognised it did not discriminate, and said so instead of reporting the number. The property
  this fix actually has is **cascade reduction**: after any first failure, a leaked process no
  longer contends with tests still running in the same binary. The direct leak-then-no-leak
  demonstration proves the mechanism; no run-rate comparison can, without seeding a panic, and
  nothing is being sized by the number.
- **Show a leak happening, then not happening.** A test that panics deliberately, with the
  process count observed before and after. Without that, "no leaks" is unfalsifiable.
- **Do not chase the three symptoms individually.** If the cause is fixed and one still flakes,
  that is a *second* finding and worth having — but fixing the tests rather than the harness
  would hide it.

## What this does not establish — and the one that matters most

**That the socket flake is fixed. It is not.**
`approval::tests::channel::bind_recovers_from_a_stale_socket_file` has its own, separate cause
and will still fail intermittently. This handoff's title and the three tests listed above
invited the reading that repairing the leak repairs the flakes; it does not. The leak makes a
bad run *worse* by cascading after a first failure — it is not why the first one happens.
**Anyone reading a green suite after this and concluding the flake is gone will be wrong**, and
the next disclosure of it is not a regression.

That the product leaks processes. This is a test-harness defect: `Child::drop`'s documented
behaviour, met by helpers that assume otherwise. Nothing here says a shipped Tekstide leaks
anything, and the closeout must not imply it does.

## Evidence, 2026-08-20

**The guard.** `KillOnDropChild` (`crates/tekstide-core/src/test_support.rs`, alongside the
existing `RealProcessLimiter` this crate's real-process tests already share): `Drop::drop` kills
and reaps the wrapped `Child`, swallowing any error from an already-exited process (`let _ =`).
`kill`/`wait` proxy directly (`&mut self`, matching `Child`'s own signatures), so a caller that
already kills and reaps manually before this fix
(`reference_adapter.rs`'s `deciding_a_proposal_whose_real_adapter_process_has_already_exited_is_undeliverable`)
needed no change beyond the return type. `wait_with_output` takes `self` by value; the wrapped
`Child` is `Option`-held and `.take()`n there specifically, since a type implementing `Drop`
cannot have a field moved out of it directly.

**Every real-process-spawning test helper in the workspace found and wired**, mechanically: `grep
-rn "\.spawn()"` across `crates/` returned exactly three call sites total. One is production code
(`runtime/terminal/launch.rs`, out of scope — this is a test-harness fix). The other two are both
in `approval::tests`: `reference_adapter.rs`'s `spawn_adapter` helper (7 call sites reached through
it) and one inline spawn in `channel.rs`
(`a_real_process_presenting_the_wrong_token_over_a_separate_connection_is_rejected` — the exact
test name grep found it in). Both now return/hold `KillOnDropChild` instead of a bare
`std::process::Child`.

**Show a leak happening, then not happening** — the gate's own required form, in
`test_support.rs`'s own test module:

- `a_bare_child_leaks_across_a_panic_this_fix_exists_to_prevent`: a bare `Child` moved into a
  closure that panics; `catch_unwind` contains the panic; the real process (checked via
  `libc::kill(pid, 0)`, mirroring `runtime::terminal::termination::process_group_exists_by_id`'s
  own technique) is still alive afterward. Manually killed at the end, since this test's whole
  point is that nothing else would have.
- `kill_on_drop_child_does_not_leak_across_a_panic`: the identical scenario, `KillOnDropChild` in
  place of the bare `Child`. The process is gone — killed *and* reaped, not merely signalled —
  by the time `catch_unwind` returns, since `Drop::drop` runs synchronously during unwinding.
- `kill_on_drop_child_cleans_up_on_ordinary_drop_too`: the non-panicking path, proven separately,
  since a `Drop` impl exercised only via the panic path is not proven for the ordinary one.
- `wait_with_output_returns_the_real_exit_status`: the guard does not break the happy path —
  real exit status, real stdout, still correct after passing through it.

All four go through `RealProcessLimiter::acquire()` themselves, as the first local, matching
every other real-process test in this crate — an early version of this work omitted that and
measurably worsened contention on `approval::tests::channel::bind_recovers_from_a_stale_socket_file`
(see below) before the slot was added.

**Measured, and the measurement needed a correction along the way.** The originally-planned
"before vs after, the same way" comparison — repeated `cargo test`/direct-binary runs of
`approval::` — was run at N=200 both ways (this session's own machine, heavily loaded from a long
session of back-to-back builds and test runs): **27/200 (13.5%) unfixed, 29/200 (14.5%) fixed** —
statistically indistinguishable, both far above the historically recorded 3/150 (~2%) baseline.

**That comparison does not actually test what this fix changes, and it would have been dishonest
to present it as if it did.** Each of the 200 iterations invokes the compiled test binary as a
*fresh, independent OS process* — `approval::`-filtered, so none of the new `test_support::tests`
panic-and-leak tests run inside it. No test in a normal, passing `approval::` run panics at all,
so the leak this fix prevents never has an opportunity to occur during either the "before" or
"after" loop. Both loops necessarily measure the *ambient* rate of the pre-existing,
already-disclosed `bind_recovers_from_a_stale_socket_file` flake under this session's own current
system load — a real and useful number, but not a measurement of this fix's effect. The mechanism
this fix addresses only matters *within* a single, longer-lived test-binary invocation where an
unrelated test's real panic leaks a process that then contends with others still running in that
same process — which is what `RealProcessLimiter`'s own doc describes ("wall-clock overlap between
different test functions' real processes... within a single test binary run"), not something a
loop of independent, single-filter process invocations can reproduce.

**What was actually proven, stated precisely**: the causal mechanism — `Child::drop` leaks, this
guard does not — is proven directly and unambiguously by the four tests above. Whether that
translates into a lower *observed* full-suite flake rate depends on how often a real, unrelated
panic happens to occur near in time to `bind_recovers_from_a_stale_socket_file` (or either of the
other two historically-affected tests) within the *same* test-binary process — a condition this
session cannot control or reproduce cleanly in a short loop, and the recorded 3-in-150 baseline
itself was almost certainly gathered the same way: incidentally, across ordinary development
activity, not a tight synthetic repeat. The correct claim is the direct one; the loop comparison
is disclosed rather than presented as if it settled the question either way.

**Do not chase the three symptoms individually — not done.** No change to any of the three
named tests (`bind_recovers_from_a_stale_socket_file`,
`agent_run_queue_limit_is_enforced_and_only_counts_live_entries`,
`command_approval_family_produces_real_durable_audit_records_through_the_pipeline`) — the harness
defect is fixed at its source, not any individual symptom.

**Gates run**, 2026-08-20: `cargo fmt --all --check` clean. `cargo clippy --workspace
--all-targets --all-features -- -D warnings` clean. `cargo test --workspace --all-targets
--all-features` run three times, all fully clean, after both this fix and the separate,
same-session RFC-023 PR-023-E work were in the tree together: `tekstide` 311 passed,
`tekstide-core` 713 passed (this fix's own four new `test_support` tests are 699 of the count
increase from the last-recorded 695; PR-023-E's own fourteen account for the rest — the two were
not gated in isolation from each other, only combined, since both landed the same session before
either was reviewed), `reference_adapter` 0 tests. `git diff --check` clean. Committed as
`f0c5055`, staged by explicit path, separately from the unrelated RFC-023 work (`855d063`).
