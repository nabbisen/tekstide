---
title: "RFC-030 — QA evidence"
rfc: "RFC-030"
rfc_file: "../../accepted/030-git-integration.md"
source_rfc_status: "Accepted 2026-09-22 — M12; D1' decided 2026-09-22"
target_milestone: "M12"
created: "2026-09-22"
---

# Evidence

## PR-030-A — the fixture, and the gate

No user-visible change. `crates/tekstide-core/src/runtime/git.rs` (new module, declared in
`runtime.rs`) has no production caller yet — PR-030-B wires `evaluate` into
`ProjectSession::set_git_summary`.

### The gate itself

`evaluate(repository_root: &Path) -> GitGateOutcome`. Three outcomes: `Accepted` (branch, dirty
state and per-file status all safe to read), `AcceptedBranchOnly` (configuration is safe, but
attributes name a `filter=`/`diff=` driver — D1' item 6 — so only branch is safe), `Refused(reason)`
(nothing is safe; nothing was read).

Trust state is not a parameter. D1' item 7 is absolute — the gate makes detection safe in
Restricted projects, not a trust grant — so it is not represented in the function's own signature at
all, rather than threaded through and ignored.

### RFC-012's *Git Detector Safety* gate, item by item

The module's own doc comment (`git.rs:1-48`) states the claim for each item, with a citation to the
code that makes it true. Restated here with the test evidence:

1. **Reviewed non-project-local executable.** `GIT_EXECUTABLE = "git"`, resolved only against the
   fixed `PATH` set in `spawn_git_command`, never a path derived from the repository.
2. **Invoked directly, no shell.** `Command::new(git_executable).args(args)` throughout; no call
   site builds a shell string. Grep: `grep -rn "sh -c\|Command::new(\"sh\")" crates/tekstide-core/src/runtime/git.rs` — no matches.
3. **Deterministic argument vector, not aliases.** Every call passes a fixed argv
   (`["config", "--list", "--null"]`, `["--version"]`) — never a bare subcommand name a repository's
   `alias.*` could redefine. `alias.*` keys are also not on the allowlist, so a repository naming one
   is refused regardless. Proven for the general case by
   `an_unrecognised_but_harmless_key_is_still_refused` (uses `core.editor`, the same
   not-on-the-list shape `alias.*` would hit).
4. **No project-local `PATH`.** `spawn_git_command` sets `PATH=/usr/bin:/bin` unconditionally after
   `.env_clear()`.
5. **Sanitised environment.** `.env_clear()` first; only `PATH`, `LANG`, `LC_ALL` are fixed, plus
   whichever of `HOME`/`GIT_CONFIG_GLOBAL`/`GIT_CONFIG_SYSTEM`/`XDG_CONFIG_HOME` Tekstide's own
   process environment set, forwarded explicitly (never read from anything project-controlled). This
   is also the mechanism the D7 fixture uses to point "global"/"system" config at itself —
   `Fixture::new` in `git/tests.rs` sets these three to fixture-local files and passes them as
   `forwarded_env`, never mutating the real process environment (`std::env::set_var` is
   process-global and would race other tests in the same binary).
6. **No workspace hooks or config-driven automation.** The gate's whole purpose: an unrecognised
   configuration key refuses the repository before any command that could act on it runs. Proven by
   every refusal test below.
7. **Bounded execution time and output.** `run_bounded` enforces `SUBPROCESS_TIMEOUT` (5s) and
   `MAX_OUTPUT_BYTES` (1 MiB) on every subprocess call via a `Read::take(max_bytes)` wrapper and a
   poll-with-deadline wait loop that kills the child on timeout.
8. **Bounded diagnostics.** `GitGateRefusal`/`GitUnavailableReason` variants carry only a
   configuration key name (`String`, but drawn from git's own fixed key vocabulary) or a version
   string (`git --version`'s own output) — never file contents, diff output, or captured stderr.

### D7 — the fixture

`Fixture` (`crates/tekstide-core/src/runtime/git/tests.rs`): a fresh `std::env::temp_dir()` directory
per test, its own `HOME`/`GIT_CONFIG_GLOBAL`/`GIT_CONFIG_SYSTEM` (an empty global config, so the
developer's own `~/.gitconfig` is never consulted — §2 of `what-git-integration-must-not-do.md`),
cleaned up in `Drop`. Every marker script (`marker_script`) writes one file under the fixture's own
marker directory and nothing else (§3) — no network, no destructive action, nothing outside the
temporary directory.

Vectors (each its own fixture, per the checklist):

| Vector | Config | Proven hostile by |
| --- | --- | --- |
| clean filter | `filter.evil.clean` (required), `.gitattributes: * filter=evil` | `hostile_fixtures_are_provably_hostile`: unprotected `git status --porcelain=v2` runs it |
| fsmonitor | `core.fsmonitor` | same test: unprotected `git status --porcelain=v2` runs it |
| textconv | `diff.evil.textconv`, `.gitattributes: * diff=evil` | same test: unprotected `git diff` runs it (`git status`/`git diff --stat` do **not** — measured separately, see below) |
| include-hidden | `filter.evil.clean` defined only in a file pulled in by `include.path` | same test: unprotected `git status --porcelain=v2` still runs it |
| attributes-undefined | `.gitattributes` names a filter/diff driver with no matching config | no marker — nothing executes (measured, matches review 405 row 6); covered by correctness tests below, not the hostility ablation |
| control | nothing configured | `control_repository_is_accepted_and_executes_nothing` |

Every poisoned fixture in `hostile_fixtures_are_provably_hostile` modifies `tracked.txt` **in place
at the same byte length** (`commit_then_modify_same_length`) rather than changing its size. This is
deliberate, not incidental: the implementer's first probe of this vector last session used a
size-changing modification and got a clean (zero-marker) result from `gix`'s status computation —
which turned out to be a false negative, because a size mismatch lets `gix-status`'s `FastEq`
comparator answer "different" without ever reading file content, so the probe never exercised the
path a filter or textconv driver hooks. A same-length change forces the real content-comparison path.
This is the finding recorded as D1' in the RFC and is why every fixture here follows the same
same-length-modification shape.

Textconv specifically: measured directly (scratch probe, not committed) that `git status
--porcelain=v2` and `git diff --stat` do **not** invoke `diff.<name>.textconv` — only a plain `git
diff` (full hunks, not a stat summary) does. `hostile_fixtures_are_provably_hostile`'s textconv case
therefore runs `git diff`, not `git status`, to prove that vector hostile; `evaluate`'s allowlist
still refuses `diff.evil.textconv` regardless of which command would trigger it, since the key itself
is not recognised.

Setup itself (`git add`/`git commit`, run directly, not through `evaluate`) can touch a marker before
the property under test runs — `core.fsmonitor` and a required clean filter are both consulted during
a plain `git add`/`git commit`, confirmed empirically when the first version of these tests failed for
exactly this reason (marker present from setup, not from the step being measured). Every fixture calls
`clear_markers()` after setup and before the measured step, so a marker found afterward can only be
attributed to that step.

### Required tests (PR-030-A checklist)

All in `crates/tekstide-core/src/runtime/git/tests.rs`, `cargo test -p tekstide-core runtime::git::`:

- `hostile_fixtures_are_provably_hostile` — the ablation that must run first: with the gate not
  consulted at all, each poisoned repository's marker appears.
- `control_repository_is_accepted_and_executes_nothing` — a repository naming nothing is `Accepted`;
  no marker directory entries.
- `clean_filter_repository_is_refused_and_the_marker_never_runs`,
  `fsmonitor_repository_is_refused_and_the_marker_never_runs`,
  `textconv_repository_is_refused_and_the_marker_never_runs` — each poisoned repository is `Refused`
  via `evaluate`, and its marker is absent after.
- `include_hidden_repository_is_refused_on_the_include_key_itself` — asserts specifically
  `GitGateRefusal::ConfigInclude { key: "include.path" }`, not merely "refused for some reason" —
  proving the include-hiding bypass (review 405) is closed by name, not by accident.
- `includeif_repository_is_refused_on_the_includeif_key_itself` — the `includeIf.<condition>.path`
  form.
- `attributes_naming_an_undefined_driver_is_accepted_branch_only` and its `diff=` counterpart — D1'
  item 6: safe configuration, but `AcceptedBranchOnly`, not `Accepted`.
- `an_unrecognised_but_harmless_key_is_still_refused` — `core.editor`, on no fixture's poison list,
  still refused: proves the allowlist is default-deny, not a denylist of the specific vectors this
  suite happens to name.
- `the_gate_does_not_depend_on_trust_state` — a `Trusted` and a `Restricted` `ProjectSession` each
  constructed over an identical-shape repository; `evaluate` (no trust parameter) gives the same
  outcome for both, and a poisoned (`core.fsmonitor`) repository under each trust state is refused
  with its marker absent. Exercised for one vector, not all four: `evaluate`'s signature takes no
  trust argument at all, so trust-independence is structural (the same function, called on the same
  repository, cannot see which `ProjectSession` is asking) rather than conditional logic that could
  vary per vector — re-running every vector under both trust states would re-prove the same structural
  fact four more times, not test anything a fifth vector could make false.
- `git_not_found_reports_unavailable_not_a_panic` — `evaluate_with_environment` given a nonexistent
  executable name reports `Unavailable(NotFound)`.
- `parse_git_version_reads_the_real_toolchain_output` /
  `a_version_below_the_minimum_is_too_old` — the version-string parser and the `MIN_GIT_VERSION`
  (2.30.0) comparison, unit-tested directly; "too old" is proven at the parsing/comparison level
  since crafting a genuinely older `git` binary in CI is impractical.

All 14 tests: `cargo test -p tekstide-core runtime::git::` — 14 passed, 0 failed.

### The allowlist-vs-denylist ablation (checklist requirement, manual, reverted)

Not a permanent test — the checklist calls for a one-time proof that the allowlist is load-bearing,
matching this project's existing ablation convention for logic that a permanent regression test
would either duplicate or make brittle.

`sha256sum crates/tekstide-core/src/runtime/git.rs` before: `334885...ee9654` (recorded in full in
the session transcript). `config_key_is_allowed` temporarily rewritten from the allowlist to a
denylist of only the vector-specific bad keys (`core.fsmonitor`, `filter.evil.clean`,
`filter.evil.required`, `diff.evil.textconv`) — deliberately **excluding** `core.editor`, since a
denylist's whole failure mode is not knowing about a key nobody thought to list.

Result: `an_unrecognised_but_harmless_key_is_still_refused` failed —
`left: Accepted, right: Refused(UnknownConfigKey { key: "core.editor" })` — confirming a denylist
would have silently accepted `core.editor` (and by the same logic, any other key not specifically
anticipated). `include_hidden_repository_is_refused_on_the_include_key_itself` did **not** fail under
this specific ablation, because the `include.path`/`includeIf.*` check runs before
`config_key_is_allowed` is even consulted (a separate, always-first refusal — intentional defense in
depth, not a second copy of the allowlist). The checklist's "or" is satisfied by the unknown-key test
alone.

`config_key_is_allowed` reverted to the allowlist; `sha256sum` after matches the before value
exactly, confirming a clean revert with no residual diff.

### `git` absent or too old

`git_not_found_reports_unavailable_not_a_panic` covers "absent" end-to-end (a real `evaluate` call
against a nonexistent executable name, not a mocked failure). "Too old" is covered at the unit level
(`parse_git_version`, `MIN_GIT_VERSION` comparison) rather than end-to-end, since installing an
actually-older `git` binary in this environment is impractical; `check_git_available`'s own logic
composes these two already-tested pieces directly (parse, then compare), so no further integration
coverage was judged necessary.

### D1' confirmed against the committed fixture

`control_repository_is_accepted_and_executes_nothing` is exactly review 405's row 3 (and the RFC's
own D1' "row 3 carries the decision"): a repository naming nothing is accepted, and nothing runs.
`hostile_fixtures_are_provably_hostile` and the four vector-specific refusal tests reproduce rows 1,
2, and the include-hidden bypass from the RFC's own measurement table, now as permanent, committed
tests rather than a one-time scratch probe.

## Whole-RFC gate (as run for PR-030-A)

- `cargo fmt --all --check` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- `git diff --cached --check` after staging — clean.
- `cargo test -p tekstide --test rfc_docs_invariants` — 9 passed, unaffected.
- Three consecutive `cargo test --workspace --no-fail-fast` runs — 843 passed in
  `tekstide-core`'s lib target (was 842 passed / 1 failed on the very first run: see below), 556 +
  9 elsewhere, all three runs clean, no flakes observed.

### One real failure caught by the gate, not a flake

The first full-workspace run failed
`project::diff::tests::enumeration_confirms_only_the_closed_list_reads_full_file_content` — RFC-024's
closed-list scan for full-file-content reads, which found `runtime/git.rs`'s `read_bounded` calling
`Take::read_to_end` (bounding a subprocess's stdout/stderr pipe) and correctly refused to pass
silently. This is a real, deterministic consequence of this slice's own new code, not an
intermittent — fixed by adding `runtime/git.rs` to `FILES_ALLOWED_TO_READ_FULL_FILE_CONTENT` with a
disclosed reason (`project/diff/tests.rs`), matching the two pre-existing non-project-content entries'
own pattern. No row added to `test-process-leak.md`, since this was not a flake.

## Review 406 — R1-R5: a repository the gate accepted still ran a program

Commit `5a44d0c`'s gate assumed the configuration it read was the configuration `git` would use.
False wherever `git` consults *another* repository's configuration — a submodule's gitdir, entirely
outside anything the parent-repository read touches.

### R1 — the submodule bypass

`repository_contains_a_gitlink` runs `git ls-files -s` and checks for any `160000`-mode entry
(measured, review 406: executes nothing even in a poisoned repository) before deciding anything
about content. `worktree_names_a_content_driver` checks it first, unconditionally, and fails closed
(`true` → `AcceptedBranchOnly`) both on a real gitlink and on any failure to run/parse the listing —
"cannot determine" is not "assume clean".

Chose the simpler of review 406's two acceptable answers: refuse the content answer outright rather
than resolve and vet each submodule's own gitdir against the same allowlist. The more generous answer
is available later if a real need for submodule dirty-state shows up; nothing here forecloses it.

**Fixture** (`Fixture::add_poisoned_submodule`): a real `git submodule add` (not a hand-built
approximation — the gitlink and the nested checkout are exactly what real git produces), then every
`submodule.*` key stripped from the parent's config and `.gitmodules` deleted, so the parent's own
configuration is entirely allowlist-clean — reproducing review 406's own repro exactly, rather than a
weaker version that would only prove the unrelated fact that an unrecognised `submodule.*` key gets
refused. The submodule's own `filter.evil.clean` is poisoned and its tracked file modified in place
at the same byte length (same discipline as every other vector).

**Tests**: the submodule row in `hostile_fixtures_are_provably_hostile` (ablation first, per the
response's explicit instruction — unprotected `git status` in the parent runs the submodule's clean
filter) and `a_repository_with_a_poisoned_submodule_is_accepted_branch_only` (`evaluate` on the same
repository returns `AcceptedBranchOnly`, marker absent).

### R2 — the attributes walk now fails closed on budget exhaustion

`collect_nested_gitattributes` now returns `bool` (`true` = the walk finished; `false` = the budget
ran out mid-walk). `worktree_names_a_content_driver` treats `false` the same as "a driver was found"
— `AcceptedBranchOnly` — rather than silently reporting "nothing found" on a scan that never finished.

Also raised `MAX_ATTRIBUTE_WALK_ENTRIES` from 20,000 to 1,000,000 — **my own judgment call, not
something review 406 asked for directly**. Reasoning: this project's own working tree is 183,736
entries; without raising the cap, the fail-closed fix alone would mean this repository (and any
comparably sized one) *always* answers `AcceptedBranchOnly`, never `Accepted`, which is safe but
would make PR-030-B's dirty-state feature never actually work here. 1,000,000 gives generous headroom
while staying bounded (a pathological fixture with more entries than that still fails closed, per the
R2 fix, rather than hanging). Flagging this explicitly in the review request in case the number itself
warrants a second opinion — the fail-closed behavior is what R2 required; the specific cap value is
mine.

**Test**: `attributes_walk_budget_exhaustion_fails_closed`, via
`evaluate_with_environment_and_walk_budget` (a test-only entry point taking an explicit budget,
since actually creating a million-entry fixture in a test would be impractical) — a generous budget
over a few real directories finds `Accepted`; a budget of `1` over the same repository finds
`AcceptedBranchOnly`.

### R3 — the walk no longer follows symlinks

`collect_nested_gitattributes` now reads `DirEntry::file_type()` (which reports the entry's own type
without following a symlink, unlike the `Path::is_dir()` the previous version used, which does follow
one) and skips any entry whose type is a symlink outright — directory or file. `file_declares_content_driver`
separately uses `std::fs::symlink_metadata` (not `metadata`) on every candidate path, so a
`.gitattributes` that is itself a symlink is never followed either, including the two fixed candidates
(`.gitattributes` at the root, `info/attributes` in the resolved common dir) that never pass through
the recursive walk at all.

**Test**: `attributes_walk_does_not_follow_symlinks` — a symlinked directory pointing outside the
repository (containing a `.gitattributes` naming a driver) and a symlinked `.gitattributes` file
itself (pointing at a real file naming a driver) are both present; `evaluate` still reports `Accepted`.

### R4 — `.git` as a pointer file, and a second gap found while testing it

`resolve_git_common_dir` (renamed from `resolve_git_dir`) runs `git rev-parse --git-common-dir`
instead of joining `repository_root.join(".git")` directly, so a linked worktree or submodule
checkout's pointer-file `.git` no longer silently loses `info/attributes` and falls back to the
permissive answer.

While writing `a_linked_worktrees_pointer_file_git_dir_is_still_resolved`, the first version of this
fix (matching the review's own wording, `git rev-parse --git-dir`) turned out to have exactly the bug
it was meant to close, one level deeper: for a linked worktree, `--git-dir` resolves to that
worktree's own *private* metadata directory under the primary checkout's `.git/worktrees/<name>/`,
which has no `info/` subdirectory of its own — `info/attributes` is shared across every worktree and
lives only under the *common* dir. Measured directly (scratch probe, not committed): `git
rev-parse --git-dir` in a linked worktree gives `<primary>/.git/worktrees/<name>`; `git rev-parse
--git-common-dir` gives `<primary>/.git`, where `info/` actually exists. For a repository that is not
a linked worktree the two agree (`.git`). Switched to `--git-common-dir`; same safety class as
`--git-dir` (both are pure path resolution, no content read) so no separate hostility measurement was
needed for the substitution itself.

**Test**: `a_linked_worktrees_pointer_file_git_dir_is_still_resolved` — a real `git worktree add`
(not a hand-written pointer file), confirms `.git` is a file there, confirms `Accepted` before
poisoning, then poisons `info/attributes` under the *resolved* common dir and confirms
`AcceptedBranchOnly` after.

### R5 — the attributes read is now bounded, and the disclosure names both reads

`file_declares_content_driver` checks `metadata.len()` against `MAX_ATTRIBUTES_FILE_BYTES` (1 MiB)
before reading anything; an oversized file fails closed (`true`) rather than being read into memory
in full. `project/diff/tests.rs`'s `FILES_ALLOWED_TO_READ_FULL_FILE_CONTENT` entry for
`runtime/git.rs` now names both reads it exempts — `read_bounded`'s subprocess-pipe read (the one the
scan's pattern actually matches) and `file_declares_content_driver`'s now-bounded attributes read
(which the scan's pattern does not match, but which review 406 asked to be disclosed anyway, since
listing a file exempts every read in it, not only the one that happens to trip the regex).

**Test**: `an_oversized_attributes_file_fails_closed` — a real file just over `MAX_ATTRIBUTES_FILE_BYTES`
written to `.gitattributes`; `evaluate` reports `AcceptedBranchOnly`.

### Full test count and gate re-run

`cargo test -p tekstide-core runtime::git::` — 19 tests (5 new: the submodule refusal test plus one
each for R2, R3, R4, R5; the submodule row also added to `hostile_fixtures_are_provably_hostile`),
0 failed. `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` both
clean. `git diff --cached --check` after staging clean.

## Review 407 — R6-R7: the gate answering "not available" for the wrong reasons

### R6 — the developer's own global/system config no longer affects the outcome

`spawn_git_command` hardcodes `GIT_CONFIG_GLOBAL`/`GIT_CONFIG_SYSTEM` to `/dev/null`, applied **after**
iterating `forwarded_env` — so nothing a caller passes for those two keys (including a test fixture's
own D7-era entries for them, kept for the fixture's *setup* commands) can override the guarantee by
accident. `FORWARDED_ENV_VARS` dropped to just `HOME`/`XDG_CONFIG_HOME`.

**Test**: `the_users_own_global_configuration_does_not_affect_the_outcome` — writes an unrecognised
key (`user.signingkey`) into the fixture's `HOME/.gitconfig` (what git would read as the global config
if the hardcoded override were not in place, and the same path the fixture's own `GIT_CONFIG_GLOBAL`
entry points at) and confirms `evaluate` on an otherwise-clean repository still reports `Accepted`.

### R7 — an unrecognised key withholds content, not the whole answer (D1' amendment)

`GitGateOutcome::Refused` now carries `GitUnavailableReason` directly — `GitGateRefusal` (which used
to also wrap `UnknownConfigKey { key }`/`ConfigInclude { key }`) is gone, since both of those are no
longer refusal reasons at all. The config-key scan returns `AcceptedBranchOnly` the moment it finds an
unrecognised key or an `include`/`includeIf` key, rather than a typed `Refused` variant naming it.
`Refused` now means exactly "cannot answer at all" — `git` missing, too old, timed out, or output that
could not be parsed.

Every test whose name said "is refused" for a config-key reason was renamed to "is accepted branch
only" (`clean_filter_repository_is_accepted_branch_only_and_the_marker_never_runs`,
`fsmonitor_repository_is_accepted_branch_only_and_the_marker_never_runs`,
`textconv_repository_is_accepted_branch_only_and_the_marker_never_runs`,
`include_hidden_repository_is_accepted_branch_only`, `includeif_repository_is_accepted_branch_only`,
`an_unrecognised_but_harmless_key_is_accepted_branch_only`) — the marker-absence assertions are
unchanged; only the expected `GitGateOutcome` and the name describing it changed.
`the_gate_does_not_depend_on_trust_state`'s poisoned-repository assertions updated the same way.

The allowlist grows by four patterns, each its own reviewed judgment (module doc comment,
`SUBSECTION_ALLOWED_PATTERNS`/new `PREFIX_ONLY_ALLOWED_PATTERNS`): `branch.*.vscode-merge-base`
(VS Code), `remote.*.gh-resolved` (GitHub CLI), `submodule.*.active` (written by `git submodule add`;
R1 already withholds content for any gitlinked repository regardless of this pattern), `lfs.*` (Git
LFS's own bookkeeping — its content mechanism is `filter.lfs.clean`/`.smudge`, a `filter.*` key, never
on this allowlist).

**Test**: `tool_written_data_keys_are_allowed` — all four patterns set on an otherwise-clean
repository; `evaluate` reports `Accepted`.

**A confirmed-safe finding, not a fix**: review 407 checked whether skipping a symlinked
`.gitattributes` (R3) was itself a fail-open gap — measured `git check-attr filter -- f.txt` against a
symlinked root `.gitattributes` reports `unspecified`, i.e. git does not follow one either. Added as a
line to `file_declares_content_driver`'s doc comment; no code change.

### The release blocker: `runtime::git` made `pub(crate)`, and the dead-code tension that follows

`runtime.rs`: `pub mod git` → `pub(crate) mod git`, per review 407 — `evaluate` and its types have no
production caller yet and were reshaped twice in this review alone; publishing `0.21.0` before the
contract settles would put an in-flux gate into `tekstide-core`'s public API. No external references
existed (`grep -rn "runtime::git"` outside `runtime/git.rs` itself: none), so the change is mechanical.

This makes the entire module read as dead code to a plain (non-test) build — with `pub(crate)` and
zero non-test callers, nothing is reachable from any root. A prior, on-point ruling in this project
(`crates/tekstide/src/main.rs`'s own comment, "response 122 Required 3 precedent") says to prefer `pub`
over `#[allow(dead_code)]` for exactly this "written but not yet wired" shape — but that ruling was
for `tekstide`, a **binary** crate, where making an unwired module `pub` costs nothing (there is no
published API for it to join) and "keep the lint honest" was the only real consideration.
`tekstide-core` is a **published library crate**; here `pub(crate)` is not a free stylistic choice but
the thing review 407 explicitly required, for a reason `pub` would directly defeat. Added a single
module-level `#[allow(dead_code)]` with a comment naming both the reason and why it is not the
main.rs precedent's case, rather than either silently suppressing the lint or leaving `cargo clippy
--workspace --all-targets -D warnings` broken. Comes off the moment PR-030-B adds the real caller.
Flagged explicitly in the review request rather than assumed settled, since it turns on a judgment
call about how the two precedents relate that is genuinely arguable either way.

## PR-030-B (partial) — the computation layer, no production caller yet

Commit `00bbbe0`. `runtime::git` gains `pub fn compute_summary(repository_root: &Path) ->
ProjectGitSummary` and its supporting functions. **Deliberately stops short of wiring anything into
`ProjectSession`, `AppShell`, or the UI** -- see the review request for the architectural fork this
runs into (thread/subscription pattern, `pub(crate)` vs `pub`, how "pending" is represented, whether
Git state refreshes after project open at all) that needs a decision before that wiring is built, not
guessed.

### Branch, read without a subprocess (D1' item 8)

`resolve_git_dir_from_filesystem` resolves `.git` entirely from the filesystem -- a directory, or (a
linked worktree or submodule checkout) a pointer file's `gitdir: <path>` line, parsed directly, no
`git` invocation. **Deliberately the private per-worktree dir, not the common dir R4 uses for
attributes**: `HEAD` differs per linked worktree (each has its own current branch) where
`info/attributes` does not. `read_branch_from_head_file` then parses `ref: refs/heads/<name>`;
detached `HEAD` (a raw object id, no `ref:` prefix) and anything unreadable both read as "no branch
name", not a failure.

This makes branch attempted in **every** outcome but a genuine non-repository, including when `git`
itself is `Refused` as missing entirely (`.git/HEAD` never depends on the binary being installed) --
tested directly in `branch_is_still_read_when_git_itself_is_unavailable`, and the linked-worktree
distinction in `a_linked_worktrees_branch_is_its_own_not_the_primary_checkouts` (the primary checkout
reads `master`; the linked worktree, on a different branch, reads its own).

### Dirty state, changed-file count, ahead/behind (D5, REQ-GIT-001/002)

One call, only when `Accepted`: `git status --porcelain=v2 --branch --ignore-submodules=all`.
Porcelain v2's own header lines give branch (`# branch.head`), ahead/behind
(`# branch.ab +N -M`, absent entirely when no upstream is configured -- `ahead_count`/`behind_count`
stay `None`, not zero) and every remaining non-`#` line is one changed/untracked entry, counted.
`--ignore-submodules=all` is defence in depth behind R1's refusal (reviews 407/408), never a
substitute -- `read_status_summary` only runs once `evaluate` has already confirmed no gitlink exists,
and `ignore_submodules_all_suppresses_the_submodule_filter_on_its_own` calls it directly against a
poisoned submodule to prove the flag's own protection holds independently of R1.

Tests: a clean repository (`Some(0)`, no upstream → ahead/behind `None`); a dirty one (two changes,
one modified + one untracked, counted); a real local upstream two commits ahead and one behind (not
faked -- a second local branch, `--set-upstream-to`, then a real divergent commit on each side).

### The "not a repository" outcome, made cheap rather than merely correct

`compute_summary_with_environment` checks `resolve_git_dir_from_filesystem` **before** calling
`evaluate` at all: a non-repository resolves to `Unavailable` without ever spawning `git` --
`a_non_repository_is_unavailable`'s second assertion passes a nonexistent executable name and still
gets `Unavailable`, proving the short-circuit is real, not merely that the fallback path happens to
agree. Satisfies the checklist's "not a repository is its own outcome rather than `SpawnFailed`" in
the strongest available sense, and is free: the common case (opening an ordinary, non-Git folder)
never touches `git` at all rather than spending three subprocess calls discovering the same thing.

### `FILES_ALLOWED_TO_READ_FULL_FILE_CONTENT`, updated to four call sites

`resolve_git_dir_from_filesystem` and `read_branch_from_head_file` both read a few bytes from the
filesystem (`.git`'s pointer-file line; `HEAD`'s one line). Added to the existing `runtime/git.rs`
disclosure alongside the two from PR-030-A, per the same "name every read the entry exempts, not only
the one the scan's pattern matches" discipline review 406 established.

### Restricted projects are not treated differently

`compute_summary_does_not_depend_on_trust_state` -- the same structural argument
`the_gate_does_not_depend_on_trust_state` already makes for `evaluate` (no trust parameter anywhere in
the call chain), exercised again for `compute_summary` specifically since it is a separate entry point
with its own call graph, not provably a thin wrapper by inspection alone.

### Test count and gate

`cargo test -p tekstide-core runtime::git::` -- 31 tests (12 new), 0 failed. `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `git diff --cached --check`,
`rfc_docs_invariants` (9/9), three consecutive `cargo test --workspace --no-fail-fast` runs -- 860
passed in `tekstide-core`'s lib target, 556 + 9 elsewhere, all clean, all three runs.

## PR-030-B (the wiring layer) -- review 410's four rulings, implemented

### 1. `runtime::git` narrowed back to `pub`

`runtime.rs`: `pub(crate) mod git` -> `pub mod git`. Inside the module, only `compute_summary` is
`pub`; `evaluate`, `GitGateOutcome`, `GitUnavailableReason` became `pub(crate)` (were `pub`) -- the
GUI asks "what is this project's Git summary", never "is this repository accepted". The
`#![allow(dead_code)]` is gone: `compute_summary` has a real caller
(`git_summary_stream`), so nothing in the module reads as dead any more.

**A second dormant-capability finding, RFC-036, while narrowing this**: the bare
`evaluate(repository_root: &Path) -> GitGateOutcome` wrapper (distinct from
`evaluate_with_environment`, which every real caller -- `compute_summary_with_environment` and every
test -- actually calls) had no caller anywhere, production or test, once checked. Deleted rather than
kept `pub(crate)` for a caller that was never going to arrive; `forwarded_environment()` itself is
still real (called from `compute_summary`).

### 2. Refresh is event-driven: project open, and a managed process ending

`ProjectSession::begin_git_summary_refresh` / `set_git_summary` (now clearing the in-flight flag) --
full behaviour and its four tests already described under "Restricted projects" and the `Unknown`
ruling above; this section is the *trigger sites*, not the flag itself.

- **Project open**: `trigger_git_summary_refresh`, called from all four production
  `add_project_from_path` call sites (`shell.rs:4400`, `5011`, `5153` via one shared `Edit`;
  `main.rs:185` directly) -- the same "no single point every newly-opened project passes through, so
  each site calls its own helper" shape `verify_restored_trust`/`apply_configured_resource_limits`/
  `load_earlier_transcripts_for_opened_project` already established.
- **A managed process ending**: the same call, added unconditionally at the top of
  `apply_agent_terminal_outcome_and_record` -- RFC-048's own "the one place production applies an
  agent run's terminal outcome" -- ahead of either of its two branches (identity-and-store-available,
  or the bare fallback), so it fires regardless of whether an audit record could be written, matching
  "a termination is never refused for want of a trail".
- **No periodic poll added.** Checked directly against the requirement: nothing in `subscription()`
  re-triggers a Git evaluation on a timer.

**Enforcement, per review 410's own instruction** ("extend the existing guard, or add its sibling"):
added the sibling, `trigger_git_summary_refresh_is_called_from_every_expected_site`
(`crates/tekstide/src/tests.rs`), scanning for `trigger_git_summary_refresh(` the same way the
existing test scans for `.add_project_from_path(` -- with its own allowlist
(`files_with_one_allowed_call_to_trigger_git_summary_refresh`: `main.rs` 1, `shell.rs` 4 -- one more
than the add-project scan's 3, for the process-termination site, which has nothing to do with opening
a project) rather than reusing the add-project scan's map, which would have under-counted by one.
**Ablation, run and reverted**: removed one project-open site's trigger call, the test failed naming
the exact mismatch (`shell.rs calls trigger_git_summary_refresh 3 time(s), expected 4`); reverted,
`sha256sum` before/after `shell.rs` identical.

### 3. The subscription/threading mechanism

`git_summary_subscription`/`GitSummarySource`/`git_summary_stream` in `crates/tekstide/src/shell.rs`,
modelled directly on `terminal_wake_subscription`/`TerminalWakeSource`/`terminal_wake_stream` (the
only precedent for "background thread's result reaches `update()` as a `Message`" this codebase has --
confirmed by research: zero uses of `iced::Task::perform` anywhere). Differs from the terminal-wake
shape in exactly one way, deliberately: **one-shot, not a loop** -- the spawned thread calls
`compute_summary` once, sends exactly one `Message::GitSummaryComputed`, and returns, rather than
looping on a wake notifier. `subscription()` includes a project's `git_summary_subscription` only
while `git_summary_refresh_in_flight()` is true, so a completed evaluation stops being offered on the
next rebuild rather than needing to be explicitly torn down.

`Message::GitSummaryComputed { project_id, summary }` carries `ProjectGitSummary` directly (unlike
`TerminalWoke`'s deliberately bare `TerminalId` -- that restriction is about raw PTY bytes never
becoming `Debug`/`Clone`-able through `Message`, per response 205; Git summary metadata carries no
project file content, a different sensitivity class). Handled in `update()`: `project_mut` returning
`None` (project closed mid-flight) is a silent no-op, the same shape
`apply_agent_terminal_outcome_and_record` already tolerates.

### 4. Rendering: `git_state_fields`

New function in `crates/tekstide/src/shell.rs`, replacing the hardcoded
`state.catalog.get("status-bar-git-not-available")` `active_project_status_fields` used before this
slice. Maps `ProjectGitSummary::display_status()`:

- `Known { branch_name: Some(name), .. }` -> `status-bar-git-branch` (`Git: { $branch }`), `$branch`
  routed through `text_safety::quote_untrusted` -- a branch name is read out of the repository being
  shown, untrusted the same way a file name or path already is elsewhere in this catalog.
- `Known { branch_name: None, .. }` -> `status-bar-git-detached` (`Git: detached`) -- a real
  repository, `HEAD` read successfully, just not on a named branch; distinct from "not available".
- `changed_file_count`/`ahead_count`/`behind_count`, each `Some(n)` with `n > 0` -> its own field
  (`status-bar-git-changed-files`/`-ahead`/`-behind`), the same "absent at zero" convention the
  running/failed/pending-approval labels already use -- `None` (an `AcceptedBranchOnly` repository)
  and `Some(0)` both correctly produce no field.
- `Unavailable`/`NotImplemented`/`Unknown` -> `status-bar-git-not-available`, identically -- an
  implementation detail of *why* nothing is known, never three different user-facing claims.

New catalog keys added to `en.ftl` and registered in `i18n::enforcement::generic_args()`'s `$branch`
fixture (`every_source_locale_key_resolves_in_every_shipped_locale` failed once, correctly, before
that registration -- caught by the gate on the first full-workspace run, not found by inspection).

**Tests** (`shell/tests.rs`): a clean branch with nothing else (exactly two fields, no zero-padding);
dirty + ahead + behind as three separate labels; detached `HEAD` rendered distinctly from "not
available"; `AcceptedBranchOnly`'s shape (branch only, no content fields) constructed directly via
`set_git_summary` rather than through the gate (a rendering test, not a second copy of the gate's own
coverage).

### The live capture, real Git state, the release binary

`rfcs/handoffs/030-git-integration/evidence/01-status-bar-real-branch-and-dirty-count.png`: a real
`mktemp -d` repository (`git init`, a branch named `main`, one committed file, then modified in place
so `git status` reports it dirty), opened with `./target/release/tekstide <path>` under a throwaway
`XDG_STATE_HOME`. The bottom status bar reads *"Project Board | 1 project Restricted Git: main
1 changed Ctrl+Alt+P Project Board"* -- the subscription/thread/message round trip verified working
end to end in the shipping artifact, not only in unit tests. No path under `$HOME` in the image.

### The disclosure (review 410's explicit requirement)

- **Book**: `docs/src/users/what-works-today.md` gained a "Git" section stating the branch/dirty
  capability and, in the same paragraph, the cadence limitation verbatim from the ruling: *"a change
  made outside Tekstide while a project stays open... is not reflected until the next in-app process
  ends or the project is reopened."* The old "Not built" section's Git-related claim was trimmed
  rather than left contradicting the new section.
- **Changelog**: a new `## Unreleased` section, started now rather than reconstructed in one pass at
  `0.22.0`'s release-candidate time -- the gap `0.21.0`'s own candidate had to absorb.

### Test count and gate (wiring layer)

`cargo test --all-targets`: 561 (`tekstide`, +5 new: four rendering tests, one enforcement sibling) +
9 + 864 (`tekstide-core`, unchanged by this layer). Two intermittents disclosed across this slice's
gate attempts, neither a new finding about this slice's own code:

- `shell::tests::change_review_content_view_build_cost_by_line_count_measurement`, on an early
  `cargo test --all-targets` run -- already registered (review 338, `test-process-leak.md` line 29).
- `audit::tests::purge::purge_reports_deferred_cleanup_while_wal_reader_is_active`, on the third of
  three consecutive full-workspace runs -- **not** previously registered; passed immediately in
  isolation. New row added to `test-process-leak.md` (dated, named as a candidate for the same class
  of SQLite-under-parallel-load contention the register's other rows describe, not confirmed as the
  same cause).

Both re-ran clean. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`rfc_docs_invariants` (9/9), three consecutive `cargo test --workspace --no-fail-fast` runs -- 561 + 9
+ 864, clean, all three (the run that hit the second intermittent above was re-run in full, not
patched over).

### A mistake, corrected in-session

Mid-ablation-verification for the enforcement test, `git checkout -- crates/tekstide/src/shell.rs`
was run without first checking `git status` -- it discarded every uncommitted change to that file
(the `Message` variant, the subscription trio, the `subscription()` wiring, all four trigger call
sites), not only the one-line ablation it was meant to undo. Caught immediately via `git status`
after; every edit was reconstructed from the exact same content already in this evidence file's own
description above, then re-verified by a full compile, clippy pass, and test run before continuing --
not merely reapplied and trusted. No data was lost that this document does not already fully
describe, but the near-miss is worth naming rather than quietly fixing: the standing rule ("`git
status` before any command that could discard uncommitted work") exists for exactly this shape of
mistake, and this is the session it was skipped.
