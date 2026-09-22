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

- [x] **R6, the user's own configuration is neutralised, not judged.** `GIT_CONFIG_GLOBAL` and
      `GIT_CONFIG_SYSTEM` are pointed at `/dev/null` for the gate's reads and for PR-030-B's status
      read. *Measured: `evaluate` on this repository, on the owner's machine, refuses on
      `user.signingkey` — a global key. The fixture's empty global config is why this was invisible.*
      **Test:** an ordinary unknown key in the fixture's "global" config, over a clean repository,
      yields `Accepted`. — `spawn_git_command` hardcodes both, applied after `forwarded_env` so a
      caller cannot override them by accident.
      `the_users_own_global_configuration_does_not_affect_the_outcome`.
- [x] **R7, an unknown key withholds the content answer, not the branch** (D1′ amendment).
      `AcceptedBranchOnly`, not `Refused`. *Measured: with global config neutralised this repository
      still refuses, on `branch.main.vscode-merge-base` — written by an editor.* **Tests:** an unknown
      key yields `AcceptedBranchOnly` and executes nothing; a program-naming key yields the same
      withheld content answer with the marker absent; the content answer is given only for a fully
      allowlisted repository. — the config-key loop returns `AcceptedBranchOnly` directly instead of a
      typed refusal; every `*_is_accepted_branch_only`/`*_and_the_marker_never_runs` test (renamed from
      "refused" to match) plus `control_repository_is_accepted_and_executes_nothing` for the fully
      allowlisted case.
- [x] **The allowlist carries tool-written data keys by pattern** — `branch.*.vscode-merge-base`,
      `remote.*.gh-resolved`, `submodule.*.active`, `lfs.*` — each addition reviewed as a safety
      judgment. — `SUBSECTION_ALLOWED_PATTERNS`/`PREFIX_ONLY_ALLOWED_PATTERNS`.
      `tool_written_data_keys_are_allowed`.
- [x] `Refused` now means only *cannot answer at all*: `git` missing, too old, timed out, unreadable
      output. — `GitGateOutcome::Refused(GitUnavailableReason)` directly; `GitGateRefusal` (which used
      to also wrap `UnknownConfigKey`/`ConfigInclude`) removed as now-redundant.
      `git_not_found_reports_unavailable_not_a_panic`.

### Also done at review 407 — the release blocker

- [x] `runtime::git` made `pub(crate)` (was `pub`): `evaluate` and its types have no production caller
      yet and their contract was reshaped twice in this review alone (R6/R7); publishing `0.21.0`
      before that settles would put an in-flux gate into `tekstide-core`'s public API. A disclosed
      module-level `#[allow(dead_code)]` follows from that — with `pub(crate)` and no caller anywhere
      outside `#[cfg(test)]`, the whole module reads as dead to a plain build. The comment next to it
      explains why this is not `main.rs`'s "prefer `pub` over `#[allow(dead_code)]`" precedent
      (response 122): that ruling was free for a binary crate's own module visibility; here `pub(crate)`
      is required for a real, different reason (an unpublished contract, not a lint-suppression
      shortcut), so the two cases are not actually in tension despite looking alike. **Confirmed at
      review 408, time-boxed**: the allow is not permanent — its comment now names PR-030-B as the
      commit that removes it (box below), which is what keeps the lint honest for whatever in this
      module becomes genuinely unused later, per response 122's own reasoning.

## PR-030-B — branch, dirty state, ahead/behind

**Computation layer (`00bbbe0`) and UI/threading wiring layer (`<pending commit>`, this response)
both done.** Review 410 answered all four architectural questions the wiring layer opened; every
ruling below is implemented, not just decided.

- [x] **The module-level `#[allow(dead_code)]` in `runtime/git.rs` is removed** — `compute_summary`
      is a real `pub` entry point now, called from `subscription()`'s `git_summary_stream`.
- [x] `set_git_summary` has a production caller; an accepted repository shows branch, dirty state and
      ahead/behind (REQ-GIT-001, 002). — `Message::GitSummaryComputed`'s handler in `update()`.
      Live-captured against a real dirty repository:
      `rfcs/handoffs/030-git-integration/evidence/01-status-bar-real-branch-and-dirty-count.png`
      (`Git: main    1 changed`, the release binary, throwaway `/tmp` fixture).
- [x] A repository the gate cannot fully vouch for (`AcceptedBranchOnly` or `Refused`) shows its
      branch and `unavailable` (`None`) for dirty state — **with the branch read measured to execute
      nothing**, not assumed. — `resolve_git_dir_from_filesystem`/`read_branch_from_head_file` read
      only the filesystem, never spawn `git`; `branch_is_still_read_when_git_itself_is_unavailable`
      proves it directly (branch still read when the `git` binary itself does not exist).
- [x] *Unavailable* for a non-repository — checked before `evaluate` runs at all, so a non-repository
      never spawns `git` (`a_non_repository_is_unavailable`). *Pending* while a read is in flight —
      **decided at review 410 and implemented**: `Unknown` for "not computed yet"
      (`ProjectSession::begin_git_summary_refresh` resets `NotImplemented` → `Unknown` on a project's
      *first* evaluation only), rendering as the same "not available" text `0.21.0` ships — no fifth
      variant added to the shared `ProjectProviderState` enum. A Git-specific
      `git_summary_refresh_in_flight: bool` on `ProjectSession` stops a second trigger from starting a
      second evaluation. **Ablation, per review 410's exact wording**:
      `a_second_refresh_trigger_while_one_is_in_flight_is_a_no_op` proves the guard blocks a second
      start; `a_re_evaluation_trigger_leaves_the_previous_complete_summary_on_screen` proves the guard
      does *not* blank a known-good summary while a refresh runs (the actual property the ablation
      protects: an overlapping second evaluation could finish out of order and overwrite a fresher
      result with a staler one — not that the field visibly flickers).

### Decided at review 410 — the wiring layer's remaining questions, now implemented

- [x] **Refresh is event-driven**: at project open (`trigger_git_summary_refresh`, called from all
      four project-add sites) and again when a managed process belonging to that project ends
      (`apply_agent_terminal_outcome_and_record`, unconditional, ahead of either of its own branches).
      **No periodic poll** — none added.
- [x] **The cadence is disclosed** in the book (`docs/src/users/what-works-today.md`'s new "Git"
      section) and the changelog (a new `## Unreleased` section — started incrementally this time,
      rather than reconstructed in one pass at release-candidate time the way `0.21.0`'s had to be).
- [x] **All four production project-add sites trigger it, held by a test** —
      `trigger_git_summary_refresh_is_called_from_every_expected_site`
      (`crates/tekstide/src/tests.rs`), the sibling
      `add_project_from_path_is_called_exactly_once_from_main_rs_and_nowhere_else` already has.
      Ablation-verified: removed one call site's trigger, the test failed naming the exact count
      mismatch; reverted, checksum-confirmed clean.
- [x] **`runtime::git` is `pub` again, narrowed**: only `compute_summary` and `ProjectGitSummary`
      cross the crate boundary. `evaluate`, `GitGateOutcome`, `GitUnavailableReason` are `pub(crate)`.
      The old bare `evaluate(repository_root)` wrapper (distinct from `evaluate_with_environment`) had
      no caller left anywhere, test or production, once `compute_summary` called
      `evaluate_with_environment` directly — deleted (RFC-036) rather than kept.
- [x] **No write operation exists in the Git path — held by the API, not by grep** (REQ-GIT-007). —
      every subcommand this module ever calls is read-only (`config`, `--version`, `ls-files`,
      `rev-parse`, `status`); nothing in `runtime::git` accepts or constructs a write argument, so
      there is no call site to grep for in the first place.
- [x] Restricted projects are not treated differently, and the reason is the gate, not the trust
      grant. — `compute_summary` takes no trust parameter, same as `evaluate`;
      `compute_summary_does_not_depend_on_trust_state` exercises it as its own entry point rather than
      assuming the property from `evaluate`'s own test.
- [x] Input is never blocked by a refresh (NFR-PERF-006); the marker is still absent after the
      production path runs. — the blocking work (`compute_summary`) runs on a dedicated
      `std::thread::spawn`'d OS thread inside `git_summary_stream`, the exact
      `Subscription::run_with` + `iced::stream::channel` shape `terminal_wake_subscription` already
      uses to keep blocking I/O off the executor; the async block itself only
      `std::future::pending()`s.
- [x] The gate's filesystem walk runs **off the UI thread** with the rest of the read (D4), and the
      outcome is cached per project open rather than recomputed per refresh. — same background thread
      as above; `active_project_status_fields` reads the already-computed `project.git_summary()`
      field on every render, never calls `compute_summary` itself.
- [x] **`--ignore-submodules=all` on the status read** — defence in depth behind R1's refusal, never
      instead of it (measured: it suppresses the submodule filter). —
      `ignore_submodules_all_suppresses_the_submodule_filter_on_its_own` calls `read_status_summary`
      directly against a poisoned submodule, bypassing R1 entirely, and the marker still never
      appears: the flag's own protection holds independently, not only behind R1's refusal.
- [x] Decided and disclosed: **how `git` is located**. `PATH` is fixed to `/usr/bin:/bin` today, so on
      a distribution that does not put `git` there every project reads "not available". A reviewed
      absolute-path list, or the inherited `PATH` with relative and project-local entries removed —
      either is fine, but say which and why.
      — PR-030-D: `resolve_git_executable` tries `REVIEWED_GIT_DIRECTORIES` first, then the inherited
      `PATH` filtered to drop relative and project-local entries. See PR-030-D's own section below.
- [x] `git --version` is not re-run on every refresh (two spawns per evaluation today) — [x] **"not a
      repository" half done**: it is its own outcome (`Unavailable`) and, for that case specifically,
      `--version` is not run at all (the filesystem check short-circuits first). The "not re-run on
      every refresh" half is a caching question the wiring layer owns; unticked as a whole since the
      box asks for both.
      — PR-030-D: `check_git_available` now caches success (never failure), keyed by executable
      string, process-wide. See PR-030-D's own section below.
- [x] **Carried from RFC-025 (review 404):** the status bar's project fields **reach the rendered
      row**, re-proved by this slice — by a live capture showing all of REQ-NOTIFY-002's fields
      together, at minimum. —
      `rfcs/handoffs/030-git-integration/evidence/01-status-bar-real-branch-and-dirty-count.png`:
      trust ("Restricted") and Git ("main", "1 changed") both present on the rendered row, the
      release binary, a real repository.

### Required at review 411

- [x] **R-a: plain terminal exits trigger a refresh too.** Only the audited agent-run branch does
      today; the `else` branch at `shell.rs:2787`/`4895` marks the terminal exited and records
      `record_plain_terminal_terminated` while triggering nothing. A plain terminal is a managed
      process the application launched — review 410's ruling covers it. The enforcement scan's counts
      must hold the new sites. — `trigger_git_summary_refresh` added to both `else`/`else if` branches
      (`record_terminal_exit`, `terminate_project_live_work`); `files_with_one_allowed_call_to_trigger_git_summary_refresh`
      raised `shell.rs` from 4 to 6, each of the six sites named in its own comment.
      **Behavioural, not just source-scan**: `a_plain_terminals_exit_triggers_a_git_summary_refresh`
      and `project_close_terminating_a_plain_terminal_triggers_a_git_summary_refresh` assert the
      in-flight flag is actually set after a real plain terminal exits, through each call site
      directly.
- [x] **R-b: the disclosure says exactly what "ends" means.** A commit typed in a Tekstide terminal
      that stays open is not reflected either; the current wording's example implies in-app activity
      is live. Book and changelog both. — both reworded to the literal rule: *"a change is reflected
      only once the process that could have made it has ended, or the project is reopened"*, with the
      in-Tekstide-terminal case named explicitly rather than only the outside-Tekstide one.
- [x] **R-c: the project board must not contradict the status bar.** `ProjectBoardRow::branch_status`
      is hardcoded `CountDisplay::Unavailable` (`project_board.rs:202`, `:282`), so the board reads
      "branch: not available" in the same frame where the bar reads "Git: main" — visible in
      PR-030-B's own evidence capture. Feed the **active-session** row from `project.git_summary()`
      (a branch label, since `CountDisplay` cannot carry a name); the recent-but-unopened row stays
      as it is. **May land with PR-030-C; must not ship before it.**
      *Checked and not a finding: the board's "0 dirty files" beside the bar's "1 changed" is a
      different fact — `runtime_summary.dirty_files` counts open editor buffers.*
      — fixed with a new `BranchDisplay` enum (`Known(String)|Detached|Unavailable|NotImplemented|
      Unknown`); `active_project_row` now calls `branch_display(project)`, reading
      `project.git_summary().display_status()`; the recent-but-unopened row stays on
      `BranchDisplay::Unavailable` unchanged (no `ProjectSession` to read from). See
      `qa-evidence.md` for the full test/gate account.

## PR-030-C — per-file status

- [x] Per-file status from an accepted repository (REQ-GIT-003); a file outside it carries none; a
      refused repository offers none.
      — `FileGitStatus` (`Modified|Added|Deleted|Renamed|Untracked|Unmerged`) and
      `ProjectGitSummary.file_statuses: Option<BTreeMap<PathBuf, FileGitStatus>>`, populated by
      `read_status_summary`'s existing single `git status -z` call (review 413 R-1/R-2: per-record,
      path-only UTF-8 decoding, a non-UTF-8 path skipped from the map but still counted). Wired into
      `surface/explorer.rs`'s `node_line` via a new `$git` Fluent selector, threaded down from
      `shell.rs`'s one call site through `project.git_summary()`. A path outside the map, no active
      project, no summary yet, and a refused/branch-only repository all render identically -- no
      badge, checked directly, not only by omission. Directory rollup decided and written down: files
      only, no synthetic aggregate; a wholly-untracked directory gets a badge anyway, for free, only
      because `git status` itself collapses it into one record under its own unchanged default mode.
      One inherent limitation disclosed: `Deleted` can never render as a badge (no filesystem node
      exists for a deleted file to attach one to) -- real, tested at the data layer, structurally
      unreachable at the explorer layer. See `qa-evidence.md`'s "review 413's required fixes, and the
      explorer wiring" section for the full account.
- [x] Live capture against `mktemp -d`, showing a real Git state where the status bar said "not
      available". Throwaway state only.
      — `evidence/02-explorer-per-file-git-status-badges.png`: five of the six categories visible at
      once (modified/added/renamed/untracked-file/untracked-directory-collapsed/conflict) against the
      real release binary. (The box's own "said 'not available'" phrasing predates PR-030-B, which
      already made that no longer literally true of the status bar; the live-capture requirement
      itself -- real `mktemp -d` state, the shipping binary, throwaway only -- is what this satisfies.)

### Required at review 413 — before the explorer wiring ships

- [x] **R-1: an awkward-path fixture.** A repository with a non-ASCII filename and a space-bearing
      one (a newline too, if the harness tolerates it), each asserted to map to the path as it exists
      on disk. *Measured at review 413: without `-z`, git emits `"h\303\251llo w\303\266rld.txt"`
      and `"new\nline.txt"` — escaped keys an explorer lookup could never match, so those files would
      silently carry no badge.* Without the test, `-z` is one "simplification" from being reverted
      with everything still green.
      — `non_ascii_space_bearing_and_newline_bearing_filenames_map_to_their_real_on_disk_path`, all
      three cases in one fixture.
- [x] **R-2: decide what a non-UTF-8 path does.** The whole-output `from_utf8` drops an entire
      repository to branch-only for one odd filename. Prefer: decode per record, skip the undecodable
      path from the map but **still count it**, so `changed_file_count` stays truthful. If the
      whole-output decode stays, disclose it in the book beside the cadence note.
      — took the preferred direction: `read_status_summary` now decodes per record, path only;
      `a_non_utf8_path_is_skipped_from_the_map_but_still_counted` proves it.
- [x] **The six categories are settled (review 413): do not split them.** `Unmerged` stays one —
      which side changed what is a diff-view question. `R`/`C` collapse as a category, but the
      **label must be true for both**; a copy labelled "renamed" is the number-adjacent-to-the-fact
      pattern in miniature.
      — `en.ftl`'s `[renamed]` arm reads `" [renamed or copied]"`;
      `the_renamed_badge_names_both_rename_and_copy` checks both words render.
- [x] **The directory question is answered in writing**, not left to be invented: a per-node lookup is
      exact-path, so a folder containing changes carries no badge. *Files only* is a good answer; a
      roll-up is a per-node walk of the map, which is a cost question.
      — written into `surface/explorer.rs`'s own module doc comment; files only, no synthetic rollup,
      with the one incidental exception (a wholly-untracked directory) named and tested.

## PR-030-D — the two open decisions (decided at review 414)

- [x] **How `git` is located.** A reviewed absolute list first (`/usr/bin`, `/bin`, `/usr/local/bin`,
      `/opt/homebrew/bin`), then the **inherited `PATH` with every relative entry and every entry
      inside the project root removed**, resolving only to an existing regular file. RFC-012's rule is
      *no project-local `PATH`*; the user's own `PATH` is not the repository's. **Test:** an inherited
      `PATH` carrying a relative entry and an entry under the project root drops both.
      *Why it matters: `/usr/bin:/bin` alone makes the feature silently dead on NixOS and Guix.*
      — `resolve_git_executable`/`resolve_git_from_inherited_path`, exactly as specified;
      `resolving_git_from_the_inherited_path_drops_relative_and_project_local_entries` is the
      required test, verbatim.
- [x] **`git --version` is cached for the process** (`OnceLock`) on success, and **not** cached on
      failure, so a `git` installed mid-session is picked up at the next trigger.
      — **implemented with one deviation from the literal box text, disclosed**: a single-slot
      `OnceLock<String>` (matching the box as written) let one test's `"git"` permanently occupy the
      only slot and starve every other test's own executable string of ever being cached — a real
      failure, caught by the three/five-run full-workspace gate, not in isolation. Fixed with
      `OnceLock<Mutex<HashSet<String>>>`: still one process-wide cache, `OnceLock`-initialized, but
      holding every distinct successful string rather than only the first. Production is unaffected
      (one resolved executable for the whole process either way); only this module's own concurrent
      test suite needed the difference. See `qa-evidence.md`'s own account of the bug.
- [ ] **Required at review 415: canonicalize both sides of the project-root filter.**
      `Path::starts_with` is lexical, so a `PATH` entry reaching into the project through a symlink is
      not filtered — *measured at review 415: `link/bin`.starts_with(`project`) is **false** while the
      canonicalized comparison is **true**, and the `git` it offers is the repository's own.* Keep the
      lexical check as a cheap pre-filter, then compare canonicalized paths; a directory that cannot
      be canonicalized is skipped, not trusted. **Test:** extend
      `resolving_git_from_the_inherited_path_drops_relative_and_project_local_entries` with a
      symlinked entry pointing into the project.
- [x] RFC-030 closes when these land; the `0.22.0` candidate is scheduled on top of them.

## Whole-RFC

- [x] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, and **three consecutive full-workspace runs with
      `--no-fail-fast`**, output redirected to files.
      — `570 + 9 + 871`, clean across all three, this response's own gate.
- [x] Every new intermittent failure has a dated row in `test-process-leak.md`.
      — one recurrence of an already-registered row (row 8) during this RFC's own gate runs; dated,
      not a new row (see `qa-evidence.md`).
- [x] Commits are pushed once the gate is green.
      — `541da29` (PR-030-D, this response), on top of `2e79eb3` (explorer wiring), `75f5c99`/`e8339db`
      (computation layer, review-413 checklist scaffolding).

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
