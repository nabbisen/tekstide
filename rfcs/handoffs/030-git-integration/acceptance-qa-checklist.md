---
title: "RFC-030 — acceptance and QA checklist"
rfc: "RFC-030"
rfc_file: "../../accepted/030-git-integration.md"
source_rfc_status: "Accepted 2026-09-22 — M12; D1′ decided 2026-09-22"
target_milestone: "M12"
created: "2026-09-22"
---

# Acceptance and QA checklist

**Rewritten 2026-09-22 at review 405, to match D1′.** Every box is a property. A box whose plan
assigns it elsewhere, or that cannot be satisfied as written, stays unticked with the contradiction
named — the reviewer's error to fix, not the implementer's to paper over.

## PR-030-A — the fixture and the gate

- [x] **The fixture is hostile, proven by ablation**: with the gate removed, the poisoned
      repository's marker file **appears**. Run this first; a fixture that cannot fail is not a
      fixture. — `hostile_fixtures_are_provably_hostile`, qa-evidence.md.
- [x] Every vector has its own row — clean filter, `core.fsmonitor`, textconv, **include-hidden**,
      attributes-named-but-undefined, and a control that names nothing. — qa-evidence.md's vector
      table.
- [x] After the gate runs, the marker file **does not exist**, for every vector. *(Reviewer, 406: the
      box said "in both trust states" for every vector; `evaluate` takes no trust parameter, so
      trust-independence is structural and one vector proves it. The implementer's reading was right
      and the box was mechanical.)* — every `*_is_refused_and_the_marker_never_runs`/`*_on_the_include_key_itself` test;
      trust-state coverage is structural (`evaluate` takes no trust parameter), exercised explicitly
      for one vector in `the_gate_does_not_depend_on_trust_state` — see qa-evidence.md for why
      re-running all four was judged redundant rather than skipped.
- [x] The poisoned repositories are **refused**; the control repository is **accepted** and still
      executes nothing. — `control_repository_is_accepted_and_executes_nothing` plus the four
      vector-specific refusal tests.
- [x] **The configuration read is `git config --list`**, includes expanded — not `--local`, not
      `--no-includes`. A test carries the include-hidden repository specifically, because that is the
      measured bypass. — `include_hidden_repository_is_refused_on_the_include_key_itself`.
- [x] **An unknown configuration key is a refusal**, and so is any `include`/`includeIf`.
      **Ablation:** turn the allowlist into a denylist of the known-bad keys; the include-hidden or
      unknown-key test fails. — manual ablation recorded in qa-evidence.md (reverted, checksum-verified
      clean).
- [x] A repository whose attributes name a `filter=` or `diff=` driver is refused **for dirty and
      per-file state** — no count is produced for it. —
      `attributes_naming_an_undefined_driver_is_accepted_branch_only` and its `diff=` counterpart.
- [x] The fixture pins its own configuration and sanitizes the environment; the result does not depend
      on the developer's machine. — `Fixture::new`'s isolated `HOME`/`GIT_CONFIG_GLOBAL`/
      `GIT_CONFIG_SYSTEM`.
- [x] The named programs are harmless: a marker file and nothing else. — `Fixture::marker_script`.
- [x] **RFC-012's *Git Detector Safety* gate reported item by item**, each with its own evidence. —
      qa-evidence.md.
- [x] `git` absent or too old is `unavailable` — no panic, no guess. —
      `git_not_found_reports_unavailable_not_a_panic`, `parse_git_version_reads_the_real_toolchain_output`,
      `a_version_below_the_minimum_is_too_old`.
- [x] **D1′ confirmed against the committed fixture** — including the measurement that a repository
      naming nothing executes nothing, which is what the whole decision rests on. —
      `control_repository_is_accepted_and_executes_nothing`.

### Required at review 406 — a repository the gate accepted still ran a program

- [x] **R1, the submodule bypass.** A repository whose **parent config is entirely allowlisted** and
      whose worktree holds no `.gitattributes`, containing a gitlink whose submodule gitdir names a
      clean filter, **must not be `Accepted`** — measured: `git status` in the parent runs that
      filter. Detect the gitlink (`git ls-files -s`, mode `160000`) or resolve the gitdir
      (`git rev-parse --git-dir`); both execute nothing, measured. **Fixture row required, hostility
      ablation first**: with the check removed, the marker appears.
      *The principle, for the risk document: the gate assumes the configuration it reads is the
      configuration git will use, and that is false wherever git consults **another repository's**
      config.* — `repository_contains_a_gitlink`, `a_repository_with_a_poisoned_submodule_is_accepted_branch_only`,
      submodule row in `hostile_fixtures_are_provably_hostile`. Chose the simpler "refuse outright"
      answer (`AcceptedBranchOnly`) over vetting each submodule's own gitdir with the allowlist.
- [x] **R2, the attributes walk fails closed.** Budget exhaustion must answer `AcceptedBranchOnly`,
      never the permissive outcome. *Measured: this working tree is 183,736 entries against a 20,000
      cap, so the walk stops early on the project's own repository.* —
      `collect_nested_gitattributes` now returns whether it finished; exhaustion fails closed.
      `attributes_walk_budget_exhaustion_fails_closed`. Also raised `MAX_ATTRIBUTE_WALK_ENTRIES` to
      1,000,000 (own judgment call, disclosed in qa-evidence.md) so this project's own repository does
      not hit the fail-closed path on every real read; exhaustion still fails closed above that.
- [x] **R3, the walk refuses symlinks at every level** (`symlink_metadata`), as RFC-050's loader does
      — `is_dir()` follows one out of the project today. — `DirEntry::file_type()` (symlink-unfollowing)
      replaces `Path::is_dir()`; `file_declares_content_driver` also uses `symlink_metadata` so a
      symlinked `.gitattributes` file itself is skipped. `attributes_walk_does_not_follow_symlinks`.
- [x] **R4, `.git` as a pointer file** (linked worktree, submodule checkout) does not silently lose
      `info/attributes` and fall back to the permissive answer. — `resolve_git_common_dir`.
      `a_linked_worktrees_pointer_file_git_dir_is_still_resolved`: building this test found that
      `--git-dir` (what the review's own wording suggested) resolves to a linked worktree's *private*
      per-worktree dir, which has no `info/` of its own — `info/attributes` is shared under
      `--git-common-dir`. Switched to that; qa-evidence.md has the measurement.
- [x] **R5, the attributes read is bounded.** `read_to_string` on an attacker-controlled
      `.gitattributes`, for up to the walk's whole budget, is unbounded today; over-bound must read as
      `AcceptedBranchOnly`, not as "nothing found". The `FILES_ALLOWED_TO_READ_FULL_FILE_CONTENT`
      entry must name every read in the file it exempts. — `metadata.len()` checked
      (`MAX_ATTRIBUTES_FILE_BYTES`) before any read; over-bound fails closed.
      `an_oversized_attributes_file_fails_closed`. `project/diff/tests.rs`'s entry now names both
      reads `runtime/git.rs` makes.

### Required at review 407 — the gate answers "not available" for the wrong reasons

- [ ] **R6, the user's own configuration is neutralised, not judged.** `GIT_CONFIG_GLOBAL` and
      `GIT_CONFIG_SYSTEM` are pointed at `/dev/null` for the gate's reads and for PR-030-B's status
      read. *Measured: `evaluate` on this repository, on the owner's machine, refuses on
      `user.signingkey` — a global key. The fixture's empty global config is why this was invisible.*
      **Test:** an ordinary unknown key in the fixture's "global" config, over a clean repository,
      yields `Accepted`.
- [ ] **R7, an unknown key withholds the content answer, not the branch** (D1′ amendment).
      `AcceptedBranchOnly`, not `Refused`. *Measured: with global config neutralised this repository
      still refuses, on `branch.main.vscode-merge-base` — written by an editor.* **Tests:** an unknown
      key yields `AcceptedBranchOnly` and executes nothing; a program-naming key yields the same
      withheld content answer with the marker absent; the content answer is given only for a fully
      allowlisted repository.
- [ ] **The allowlist carries tool-written data keys by pattern** — `branch.*.vscode-merge-base`,
      `remote.*.gh-resolved`, `submodule.*.active`, `lfs.*` — each addition reviewed as a safety
      judgment.
- [ ] `Refused` now means only *cannot answer at all*: `git` missing, too old, timed out, unreadable
      output.

## PR-030-B — branch, dirty state, ahead/behind

- [ ] `set_git_summary` has a production caller; an accepted repository shows branch, dirty state and
      ahead/behind (REQ-GIT-001, 002).
- [ ] A **refused** repository shows its branch and `unavailable` for dirty state — **with the branch
      read measured to execute nothing**, not assumed.
- [ ] *Unavailable* for a non-repository; *pending* while a read is in flight. **Ablation:** show the
      previous summary while pending; that test fails alone.
- [ ] **No write operation exists in the Git path — held by the API, not by grep** (REQ-GIT-007).
- [ ] Restricted projects are not treated differently, and the reason is the gate, not the trust
      grant.
- [ ] Input is never blocked by a refresh (NFR-PERF-006); the marker is still absent after the
      production path runs.
- [ ] The gate's filesystem walk runs **off the UI thread** with the rest of the read (D4).
- [ ] **`--ignore-submodules=all` on the status read** — defence in depth behind R1's refusal, never
      instead of it (measured: it suppresses the submodule filter).
- [ ] Decided and disclosed: **how `git` is located**. `PATH` is fixed to `/usr/bin:/bin` today, so on
      a distribution that does not put `git` there every project reads "not available". A reviewed
      absolute-path list, or the inherited `PATH` with relative and project-local entries removed —
      either is fine, but say which and why.
- [ ] `git --version` is not re-run on every refresh (two spawns per evaluation today), and "not a
      repository" is its own outcome rather than `SpawnFailed`.
- [ ] **Carried from RFC-025 (review 404):** the status bar's project fields **reach the rendered
      row**, re-proved by this slice — by a live capture showing all of REQ-NOTIFY-002's fields
      together, at minimum.

## PR-030-C — per-file status

- [ ] Per-file status from an accepted repository (REQ-GIT-003); a file outside it carries none; a
      refused repository offers none.
- [ ] Live capture against `mktemp -d`, showing a real Git state where the status bar said "not
      available". Throwaway state only.

## Whole-RFC

- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, and **three consecutive full-workspace runs with
      `--no-fail-fast`**, output redirected to files.
- [ ] Every new intermittent failure has a dated row in `test-process-leak.md`.
- [ ] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
