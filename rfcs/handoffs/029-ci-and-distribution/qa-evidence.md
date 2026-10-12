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

## PR-029-B — the CI job (not yet actually blocking, see below)

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

### Required correction, review 544: this job is advisory, not actually blocking

**D1 and D2 both speak of a *blocking* job. What exists after this slice is not one.** The check
runs correctly, on every push and PR — but `gh api repos/nabbisen/tekstide/branches/main/protection`
still returns `404` (unprotected) as of this slice, which means nothing in GitHub itself stops a
merge when this job is red. Confirmed independently by the reviewer too, same result. **The job
and the gate it is meant to be are two different things until branch protection is enabled**, and
this section's own earlier heading ("the blocking CI job") was the unqualified claim review 544
caught.

**Branch protection — making this check actually required before a merge — is a repository
security setting, not a line in the workflow file, and is not configured here.** Configuring who
can bypass a required check, and which checks are required, is the kind of account-level decision
this project's own standing practice leaves to the human owner rather than an implementer deciding
unilaterally. Flagged in the review request rather than left implicit; the reviewer is raising the
setting with the owner separately.

### Gate

Workflow-only change; no Rust source touched on `main` by this slice (the deliberate-breakage
commit and its revert both lived and died on the throwaway branch). The real CI job's own four
runs above are the evidence this slice's "gate" actually consists of — a local re-run of
`cargo fmt`/`clippy`/the full suite would exercise nothing this slice itself changed.

## PR-029-C — release artifacts and checksums

### Reproducibility, measured rather than assumed

Task breakdown item 2 requires recording whether the build is actually byte-reproducible from a
fresh clone of the tag, not implying it either way. Measured directly: two independent `--locked`
release builds of the identical `0.36.0` tagged source, in two separate `git clone`d directories,
same machine, same toolchain —

```
20cd652f4c924ef20fd1d67960620c463e2cb3f22fef15b27fd362ef8c0a2bd5  clone-a/target/release/tekstide
521014b788a61a0aa61ec0ea7ed7b2739e4b9e4d4e149bd33eeb7dd4cf02d167  clone-b/target/release/tekstide
```

Different checksums, and different file sizes (29889344 vs 29889264 bytes). **Not
byte-reproducible.** The general cause is already documented upstream in the Rust ecosystem:
`file!()`/panic-location strings and other build-path-derived debug info embed the absolute
checkout path, which differs between any two independent clones — achieving true reproducibility
needs `--remap-path-prefix` and related flags, not attempted here, out of this slice's own scope.
Stated plainly in `release.yml`'s own header and in `release-checklist.md`'s new section, both
times with the correct consequence: **the published checksum proves the download arrived intact,
not that the release can be independently rebuilt** — exactly the distinction
`what-the-installer-must-not-do.md` §3 already draws for the installer script, carried here to the
artifact itself.

### `.github/workflows/release.yml`

Triggers on a version tag (`*.*.*`, matching this repository's own existing tag shape with no `v`
prefix) or `workflow_dispatch` with an explicit tag input, for a re-run without re-tagging. A
guard step refuses any tag that is not exactly `x.y.z` numeric before building or publishing
anything under its name. Builds `--locked` from the tag's own ref (not whatever `main` happens to
be at run time), names the artifact `tekstide-x86_64-linux` matching the RFC's own example
naming, computes its SHA-256, and publishes both to the GitHub Release for that tag — creating a
minimal release if none exists yet, uploading with `--clobber` if one does, so a re-run replaces
stale assets rather than failing. Reuses the identical `actions/checkout` pin and the identical
`rustup update stable` approach `ci.yml` already carries (same reasoning, not re-derived); no
other `uses:` step — the release itself is created and uploaded to through `gh`, preinstalled and
pre-authenticated on every GitHub-hosted runner, which is a more first-party path than any
third-party release-upload action would be. States plainly in its own header that it does not
publish to crates.io, write `release-0.X.0.md`, or decide release notes — that stays the owner's
own step, per this project's own standing practice throughout every candidate-cut review.

### What was verified live, and what was not

**Verified live, against the real repository, using a `--draft` release so nothing was publicly
visible or triggered a release notification**: built the real binary (`cargo build --release
--locked`), staged and named it exactly as the workflow does, computed its checksum, created a
throwaway draft release (`gh release create --draft`), uploaded both assets (`gh release upload
--clobber`, including a second upload proving the idempotent re-run path), confirmed both assets
present with the right sizes via the API, **downloaded the asset back and verified it against its
own published checksum** (`sha256sum -c` reported `OK`) — the full publish-then-verify loop closes
— then deleted the draft release, confirmed no release and no stray tag remain on the real repo.

**Not exercised: the actual `on: push: tags` trigger, through a real tag matching this project's
own `x.y.z` version shape.** Pushing such a tag to this public repository would create a real,
non-draft GitHub Release and (depending on default notification settings) could notify watchers —
a more consequential, harder-to-cleanly-undo action than the throwaway branch-plus-PR RFC PR-029-B
used, and not one taken unilaterally here. Every individual mechanism this workflow depends on has
now been verified live by one slice or the other (`actions/checkout` and the `rustup` steps by
`ci.yml`'s own four runs; the `gh release` create/upload/download/verify/delete cycle by the draft
release above); what has not been proven live is their composition as one triggered Actions job.
Flagged explicitly in the review request rather than claimed equivalent to PR-029-B's own full
live-run proof — the owner's own next real tag (`0.37.0`, this RFC's own target version) is the
first opportunity to see it run for real, unless the reviewer would rather a throwaway numeric tag
be pushed and cleaned up first.

### `release-checklist.md` grows the publish-and-verify step

New `## Release Artifacts` section, placed after `## Tagging` (since the artifacts publish only
once the tag exists): confirms the workflow actually ran and succeeded (not assumed from the tag's
own existence), both assets are attached, the asset is **downloaded and verified**, not only
listed, the binary reports the right version, and the checksum wording rule from §3 is carried
into the release notes explicitly, with the measured non-reproducibility stated as the reason.

### Gate

`release.yml` is YAML, parsed and confirmed valid (`python3 -c "import yaml; ..."`); its
`actions/checkout`/`rustup` steps are byte-identical to `ci.yml`'s own already-live-verified ones.
`rfc_docs_invariants`: 22/22, unaffected (no RFC document edited; `release-checklist.md` is a
handoff doc, not an RFC itself). No Rust source changed.

## PR-029-D — the one-liner installer

### `install.sh`

Platform-checked (Linux/`x86_64` only, honest refusal naming RFC-028 otherwise), downloads the
named asset and its checksum, verifies with `sha256sum -c` before installing anything, installs
exactly one file (`install -m 755`) to exactly one location (`$HOME/.local/bin/tekstide`), prints
where it installed and whether that directory is already on `PATH`. No `sudo`, no shell rc edit,
no repository added — the script has no code path that could do any of those. Wording is only ever
*"verified against its published checksum"* (D10, §3), with the header stating directly why
nothing stronger may be claimed: the binary is not byte-reproducible (PR-029-C's own measurement),
so the checksum proves the download arrived intact, not that the release is untampered or
rebuildable. `shellcheck -s sh`: 0 findings.

### Proven, not read and agreed it looks right

All tests against a throwaway `$HOME` (`/dev/shm`, removed after) and a local HTTP server serving
the real release binary built earlier in this session, so nothing depended on a GitHub release
existing — **see "What was not tested" below for the one thing that gap means**.

- **The real install path, end to end**: downloaded, verified, installed; the installed binary's
  own SHA-256 matched the source binary exactly, and `tekstide --version` reported `0.36.0` from
  the installed copy.
- **One binary, plus the directory it goes in — required correction, review 547.** `find "$HOME"
  -mindepth 1` on a genuinely fresh `$HOME` shows three new paths, not one: `.local`, `.local/bin`,
  `.local/bin/tekstide` — `mkdir -p "$install_dir"` creates the destination directory chain when
  it does not already exist, which is part of installing one binary to one location, not a defect
  (the review's own framing: failing instead "would be worse for the beginner this is for"). The
  first draft of this evidence said "exactly one new path," and the script's own header made the
  same imprecise claim ("installs ONE file to ONE location ... touches nothing else") — both
  corrected. What remains true and is the actual property this bullet exists to prove: no rc file,
  no other top-level directory, nothing outside `.local/bin` present either before or after.
- **Both `PATH` branches**: installed with the target directory absent from `PATH` (correctly
  reported not-yet-on-`PATH`, with the exact command to add it) and present (correctly reported
  already-on-`PATH`).
- **Checksum mismatch refuses, installs nothing**: served an asset paired with a deliberately wrong
  checksum file; the script exited 1 with the stated reason, and `find "$HOME" -mindepth 1`
  afterward showed zero entries — no partial or corrupted file left behind.
- **Truncation safety, proven by truncation (§2, not reading the wrapper and agreeing it looks
  right) — across eight cutoff points, not one.** Served `install.sh` itself from a local server,
  truncated to 10%, 25%, 40%, 60%, 75%, 90%, and 99% of its own byte length, piped through
  `curl | sh` exactly as the real one-liner would be run, against a fresh throwaway `$HOME` each
  time: **zero files created at every single cutoff.** The two short cutoffs (10%, 25%, comment-
  only prefixes) parsed as empty-but-valid shell and exited 0 having done nothing; every cutoff at
  40% or past (inside the function body) failed with a syntax error (*"unexpected EOF while
  looking for matching `"'"*) before executing anything — the whole-body-function wrapper does
  exactly what D11 asks.

### What was not tested, flagged rather than glossed

**The script was never run against this repository's own real GitHub release assets, because none
exist yet** — the same residual PR-029-C's own review left open, pending the owner's word on the
first `workflow_dispatch`. Once that run lands, the real one-liner
(`curl -fsSL https://raw.githubusercontent.com/nabbisen/tekstide/main/install.sh | sh`) should be
run for real against it, not only against the local mock server this slice used.

### Where the scope is stated — a question, not a silent decision

Task breakdown item 3 and the checklist both ask that the scope (§1: one binary, one location,
nothing else) be *"stated where the command is offered, not only in the script."* `README.md` and
`getting-started.md` still lead with `cargo install` — rewriting them to lead with the one-liner is
explicitly PR-029-E's own job (D7, D13, D14), not this slice's. **Not added a one-liner mention to
either page here**, to avoid half-doing PR-029-E's work ahead of it; the script's own header states
its scope in full for now. If a minimal pointer is wanted before PR-029-E lands, say so — not
assumed either way.

### Gate

`sh -n install.sh` and `shellcheck -s sh install.sh`: clean. No Rust source changed;
`rfc_docs_invariants` unaffected (no RFC document touched this slice).
