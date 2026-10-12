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

### The failure rate: what 0 clean runs does and does not establish

**Required correction, review 542.** The first draft of this section reported *"0 failures in 10"*
and leaned on it as evidence a blocking CI job is safe. That claim is weaker than the number looks:
at the prior rate (~3 in 17, ≈0.176), `P(0 failures in 14 runs) ≈ 0.066` — fourteen clean runs
(this slice's own 10 plus the reviewer's own 10/10-isolated-plus-4-full-workspace at review 542)
make the old rate *unlikely*, but the 95% upper bound on the true rate after 0/14 is still
**≈0.193** — consistent with a rate as high as ~19%, which would still make CI red roughly once in
five pushes. A 5% upper bound would need **59** clean runs, verified independently here (the exact
same arithmetic the reviewer's own response gives, reproduced rather than taken on trust:
`(1 − 0.05^(1/14)) ≈ 0.193`; `ln(0.05)/ln(0.95) ≈ 58.4 → 59`). Reporting 0/10 as *the* rate, with
no bound stated, was exactly the "a number describing something adjacent to what it is used to
prove" shape this project has caught elsewhere.

**The real argument for D1 does not rest on the count at all.** Two of the three rows closed this
pass were fixed with an identified, causal mechanism — a missing `ETXTBSY` warmup guard, and a
poll condition weaker than its own assertion — neither of which depends on how many times the test
happened to pass afterward. The third (the attention-knock test) is carried as **Accept, genuinely
unreproducible** — not fixed, and not implied to be fixed by a run count either. **D1's
prerequisite is satisfied by the dispositions themselves** (every live row has a verdict, two of
them causal), not by a clean-run tally; the 14 clean runs since are consistent with a large
reduction and worth recording, but do not by themselves establish a low rate, and PR-029-B's own
case for blocking should cite the two fixed mechanisms, not this number.

**Live rows in `test-process-leak.md` after this pass: zero.** Every row the register has ever
opened now carries exactly one verdict.

### Gate

`cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -D warnings`,
`cargo test --doc --workspace` (2 passed), `rfc_docs_invariants` (22/22, unaffected — this slice
touches no RFC document): all clean. Three consecutive full-workspace runs, fresh short `/dev/shm`
`TMPDIR` each: `782 + 22 + 1140`, 0 failed, 0 fixture entries.

## PR-029-B — the blocking CI job

### What it runs, and what it states it does not cover

`.github/workflows/ci.yml`: `cargo fmt --check`, `clippy --workspace --all-targets --all-features
-- -D warnings`, `cargo test --doc --workspace`, then `cargo build --workspace --bins` before the
full workspace suite (`--all-targets --no-fail-fast`) against a short fixed `TMPDIR`
(`/tmp/tci`), with a final step asserting it is empty afterward (D2). The workflow's own header
comment states plainly what it does **not** cover — the Run It gate, the book-against-the-
changelog sweep, the release checklist's packaging/publish/audit steps, anything needing a real
display — so a green badge cannot be read as more than it is (D4, §7).

Triggers on `push` to `main`, on every `pull_request`, and `workflow_dispatch`. No `paths:` filter
— the identical reasoning `docs.yml`'s own header already gives for itself (a workspace-wide
build/test gate has no subset of the repo it can safely ignore), reached independently here for a
different file set. `actions/checkout` reuses the exact pinned commit SHA `docs.yml` already
carries; no other `uses:` step exists. Rust itself is installed via the runner's own preinstalled
`rustup`, updated to `stable` and given the `rustfmt`/`clippy` components — not pinned to an exact
version the way `mdbook` is, since `rustup`'s own install/update path already verifies what it
fetches, documented in the workflow's own comment rather than left implicit.

### A genuine first-run failure, found by actually running it — not a planned part of the proof

The **first** real run (a throwaway verification PR, never merged) failed for real, before any
deliberate breakage was introduced: `cargo test --workspace --all-targets` on a clean runner does
not build the `[[bin]]` helper binaries (`reference_adapter`, `project_process_probe`,
`audit_store_create_probe`) several tests spawn as real child processes via `Command::new` at
runtime — exactly the lesson `release-checklist.md`'s own "Package Smoke" section already
documents for an unpacked package, hit here for the first time against a genuinely clean `target/`
rather than a developer's own machine, where these binaries are normally already built as a side
effect of earlier work in the same directory. 14 tests failed with *"the `[[bin]]` target may not
have built."* Fixed by adding a `cargo build --workspace --bins` step before the test step, with
the workflow's own comment naming why the gap stayed invisible locally. Disclosed here rather than
quietly folded into the final diff, the same discipline this project's flake register already
holds itself to.

### Proven failing on a deliberately broken commit, then proven green again

Per the task breakdown's own required proof (not reading the file and agreeing it looks right):

1. **Green**, with the bin-build fix in place: [run 38315443311](https://github.com/nabbisen/tekstide/actions/runs/38315443311).
2. **Red**: a one-line `cargo fmt --check` violation committed and pushed to the same branch —
   [run 38315885701](https://github.com/nabbisen/tekstide/actions/runs/38315885701), stopped at
   the `cargo fmt --check` step exactly, every later step skipped.
3. **Green again**: the breakage reverted (`git revert`, not a hand-edit back) —
   [run 38315927629](https://github.com/nabbisen/tekstide/actions/runs/38315927629).
4. **Green on the real `push`-to-`main` trigger**, not only `pull_request` —
   [run 38316323302](https://github.com/nabbisen/tekstide/actions/runs/38316323302), after the
   workflow landed on `main` for real.

All four runs driven through a real, throwaway verification PR (`nabbisen/tekstide#1`, opened and
closed without merging, title and body stating it was verification-only) against a disposable
branch (`ci-pr-029-b-verify`, deleted after). `main`'s own `crates/tekstide-core/src/lib.rs` was
never touched by the deliberate breakage — it only ever existed on the throwaway branch, reverted
there before that branch was deleted.

### What this slice deliberately leaves to the owner

**Branch protection — making this check actually required before a merge — is a repository
security setting, not a line in the workflow file, and is not configured here.** `gh api
repos/nabbisen/tekstide/branches/main/protection` returns `404` (unprotected) as of this slice.
Configuring who can bypass a required check, and which checks are required, is the kind of
account-level decision this project's own standing practice leaves to the human owner rather than
an implementer deciding unilaterally. Flagged explicitly in the review request rather than left
implicit.

### Gate

Workflow-only change; no Rust source touched on `main` by this slice (the deliberate-breakage
commit and its revert both lived and died on the throwaway branch). The real CI job's own four
runs above are the evidence this slice's "gate" actually consists of — a local re-run of
`cargo fmt`/`clippy`/the full suite would exercise nothing this slice itself changed.
