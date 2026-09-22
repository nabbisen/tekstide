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
