# RFC-029 — QA evidence

## PR-029-A — the flake dispositions

### Scope: three live rows, not two

The task breakdown named two rows this RFC's own reviews added — the RFC-058 knock test (review
527) and the terminal-resize poll (review 538). A third was already live and unverdicted going
into this pass: `runtime::git::tests::a_second_call_with_the_same_verified_executable_does_not_
respawn_version` (review 507), whose own register entry had speculated a verdict ("the same
verdict as its sibling, whatever that turns out to be") rather than stated one. All three are
closed below, each read against the current code directly rather than against this register's own
prior account of it — matching the 2026-10-09 disposition pass's own discipline.

### Fix — `a_second_call_with_the_same_verified_executable_does_not_respawn_version`

Review 507's own note guessed the inherited mitigation (`wait_until_executable`, already called by
`fake_git_with_config`) covered this test too. **Checked, not inherited, and the guess was wrong**:
this test calls `Fixture::marker_script`, a different constructor that wrote an equivalent script
with no warmup guard and no `wait_until_executable` call at all — the two constructors only look
alike.

Fixed: `marker_script`'s generated script gained the identical `[ "$1" = --tekstide-warmup ] &&
exit 0` first line `fake_git_with_config`'s template already has, and `marker_script` now calls
`wait_until_executable(&path)` before returning. The guard line matters specifically for this
test's own non-idempotent script body (`echo x >> counter`); without it, a bare warmup invocation
would have corrupted the exact counter the test's assertion depends on, not merely closed the
race.

Verified: `cargo test -p tekstide-core --lib runtime::git::tests::a_second_call_with_the_same_
verified_executable_does_not_respawn_version` still asserts exactly one counted invocation; the
whole `runtime::git::tests` module (62 tests) passes unchanged. Re-run 15 times under synthetic
load (16 `yes` spinners, load ~19–28): 15 of 15 clean.

### Accept — `a_blocked_second_process_names_the_real_holder_and_its_knock_is_received`

Re-investigated independently rather than resting on review 527's own measurement. Confirmed
directly: `add_project_from_path_protected` calls `bind_attention_listener` synchronously, and it
returns before `ACQUIRED` is printed — the attention socket is genuinely already `listen()`ing
before a second process can exist at all, so `UnixStream::connect`'s success does not depend on
the holder's own `accept()`-thread timing (a connection to a listening socket queues regardless of
whether `accept()` has been called). No plausible mechanism was found, the same conclusion review
527 reached independently.

Added this pass's own measurement rather than only re-reading the old one: 10 of 10 clean in plain
isolation, 5 of 5 clean under synthetic load (16 `yes` spinners, load ~46–50) — 20 additional clean
runs, 0 recurrence, on top of review 527's own 10/10 and 10/10-under-load. The only failures on
record are two, both on a now-stale pre-PR-059-A commit, never reproduced since.

**Accept** — same shape as row 1 (`bind_recovers_from_a_stale_socket_file`): sustained attention
from two independent investigators found no actionable cause, and the rate has not moved.

### Fix — `real_output_wraps_at_the_new_column_count_not_the_old_one`

Diagnosed at disclosure (review 538) as category A — the loop's own break condition
(`longest_run_in_any_row('Q') <= 20`) is weaker than the assertion it leads into (`== 20`), the
same shape four prior rows in this register already share.

Fixed: the loop now breaks on `== 20`, the exact value the assertion checks, so the wait and the
assertion can no longer disagree about "done." Ablation was attempted and, like this family's four
prior members, did not reproduce the failure under synthetic load either before or after the
change (16 `yes` spinners, 15 runs each, both sides). The same reasoning the 2026-10-09 pass
already used for this exact family applies unchanged: `== 20` is strictly stronger than `<= 20`
(implies it, never the reverse), so the new wait can only wait at least as long as the old one for
the identical final correctness property — there is no condition under which it returns earlier.
Verified: 15 of 15 clean under the same synthetic load.

### Quarantined rows, confirmed still in place

`terminal_poll_handler_cost_under_a_real_wake_driven_flood_headless_benchmark`
(`tekstide::shell::tests`) and `drain_available_never_blocks_the_caller_even_under_sustained_
production` (`tekstide-core::runtime::terminal::reader::tests`): both still `#[ignore]`d with
their 2026-10-09 reason strings, unchanged; both confirmed to still pass when run directly with
`-- --ignored`. PR-029-B's own job is wiring these into a non-blocking job; this pass's job was
only to confirm neither needed re-verdicting.

### The failure rate, re-measured after the pass

**Before** (this register's own repeated sampling, 2026-10-09 pass): ~3 failures in 17 full-
workspace runs (~18%).

**After this pass, 10 full-workspace runs, `--no-fail-fast`, fresh short `/dev/shm` `TMPDIR` each:
0 failures, 0 fixture entries left, every run.** Three as this slice's own required gate
(`782 + 22 + 1140` each), seven more as a dedicated re-measurement. Not assumed to have improved —
counted.

**Live rows in `test-process-leak.md` after this pass: zero.** Every row the register has ever
opened now carries exactly one verdict.

### Gate

`cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -D warnings`,
`cargo test --doc --workspace` (2 passed), `rfc_docs_invariants` (22/22, unaffected — this slice
touches no RFC document): all clean. Three consecutive full-workspace runs, fresh short `/dev/shm`
`TMPDIR` each: `782 + 22 + 1140`, 0 failed, 0 fixture entries.
