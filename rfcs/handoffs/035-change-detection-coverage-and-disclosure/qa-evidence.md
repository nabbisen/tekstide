---
title: "RFC-035: QA evidence"
rfc: "RFC-035"
rfc_file: "../../done/035-change-detection-coverage-and-disclosure.md"
source_rfc_status: "Implemented and closed 2026-08-25 — RFC-035 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-25"
---

# QA evidence

One section per PR. Cite the command that produced each result.

## PR-035-A — watching `.git/hooks/` and `.git/config`

**Readers of the shared exclusion, enumerated before anything changed** (per
`what-watching-dot-git-must-not-become.md` §4, the same enumeration discipline PR-039-C used for
the sessions map): grepped `IGNORED_DIRECTORY_NAMES` across the crate. Two readers: the
detector's own `GeneratedChangeDetectionPolicy::default()` (`ignored_directory_names`), and
`FileExplorerScanPolicy::linux_mvp`'s `collapsed_directory_names` (`root/explorer.rs`). **The
constant itself was not touched** — the carve-out lives entirely inside `change_detection.rs`'s
own `scan_directory`/`scan_git_directory`, so the explorer's own reader is structurally
unreachable by this change. Confirmed, not assumed: `project::root::explorer::tests` (17 tests,
unaffected by this diff) still pass unchanged, including
`scanner_collapses_heavy_directories_by_name` and `browse_directory_collapses_ignored_directory_names`.

**Design**: `.git` stays in `ignored_directory_names` (still fully skipped for a policy that
doesn't watch it at all — preserves `real_repository_filesystem_scan_cost_headless_benchmark`'s
`full_walk_policy` case exactly). When a directory named `.git` *would* have been skipped under
that list, `scan_directory` now calls `scan_git_directory` instead of `continue`-ing past it:
that function reads only `.git`'s immediate children named in `hooks`/`config`
(`GIT_WATCHED_ENTRY_NAMES`, a private, non-configurable `const` — deliberately not a
`GeneratedChangeDetectionPolicy` field, per D1's "a security-relevant default should not arrive
as a setting first"); everything else under `.git/` is skipped, unchanged and unrecorded, exactly
as the whole of `.git/` used to be. `.git` itself is never pushed as an entry (consistent with
every other name in `ignored_directory_names`); `hooks/`, once selected, is recursed into with
the ordinary `scan_directory` — not a second, narrower scanner — so it is treated exactly like
any other real directory once inside it.

**Acceptance criterion, proven against a real written hook, not a constructed `DetectedChanges`**:
- `tekstide-core`: `git_hooks_pre_commit_is_watched_while_churn_paths_under_git_stay_excluded` —
  real filesystem writes (`.git/hooks/pre-commit`, plus `.git/objects/`, `.git/refs/`,
  `.git/index` churn alongside it, after the same baseline), real `GeneratedChangeDetector`.
  Asserts the hook appears and none of the churn paths do, and that `.git` itself never becomes
  an entry.
- `tekstide-core`: `git_config_is_watched_while_churn_paths_under_git_stay_excluded` — same shape
  for `.git/config`, alongside a config body that itself contains a `hooksPath` redirect and an
  `[alias]` entry, to make the fixture realistic rather than a bare empty file.
- `tekstide-core`: `git_hooks_path_redirect_in_config_is_not_followed` — a `.git/config` naming
  `hooksPath = .githooks` produces exactly one changed path (`.git/config` itself); nothing
  parses the content or goes looking for the redirected directory. Proves the §2 deferral is
  real, not merely undocumented.
- `tekstide`: `change_review_surface_shows_a_real_git_hook_a_real_agent_run_installed` — **the
  real agent run this PR's own evidence line asks for**, not a substitute: a real `Managed`
  launch, a real socket-delivered approval, a real process exit, writing `.git/hooks/pre-commit`
  into a `.git/hooks/` that already existed at baseline time (mirroring a real `git init`, which
  leaves `.git/hooks/` populated with `.sample` files) — then a real click on "Change Review",
  asserting the real rendered file-entry line contains `.git/hooks/pre-commit`.

**Ablated separately, by hand** (no runtime knob exists to flip `GIT_WATCHED_ENTRY_NAMES` —
deliberately not configurable, so this is a source edit / test / revert cycle, the same shape
review response 322's own ablation used):
- `GIT_WATCHED_ENTRY_NAMES` narrowed to `&["config"]` (hooks removed):
  `git_hooks_pre_commit_is_watched_while_churn_paths_under_git_stay_excluded` failed —
  `"a real, freshly-installed hook must appear as a changed path: []"`.
- `GIT_WATCHED_ENTRY_NAMES` narrowed to `&["hooks"]` (config removed):
  `git_config_is_watched_while_churn_paths_under_git_stay_excluded` failed — `"a real,
  freshly-written config ... must appear as a changed path: []"`.
- Both reverted to `&["hooks", "config"]`; full `change_detection` module re-run green.

**`core.hooksPath` not followed** — deferred, with its reason: resolving a redirect means reading
config content, resolving a path that may be anywhere, and watching a second location that
changes as config changes — real, separate scope. Watching `.git/config` itself already reports
that the hook location changed, which is the fact that matters (§2). Recorded here, in
`change_detection.rs`'s own doc comments, and in `README.md`.

**A changed hook renders as an ordinary changed path** — no new severity, icon, or list (§3).
Confirmed by reading `scan_git_directory`: it calls the exact same `classify_and_push_entry`
`scan_directory` uses for every other path, and the surface-level test above renders it through
the exact same `change_review_file_entry_line` any other file uses.

## PR-035-B — `max_changed_paths`

**The defect, precisely**: `detect_filesystem_changes` used to discard `changed_paths` entirely
(`Vec::new()`) and overwrite `status` to `Partial { limit: max_changed_paths }` once the count
exceeded the limit — "found 4,097 changes, show none of them," and no `ChangeSet` could even be
created afterward (`add_detected_generated_change_set` refuses any non-`Complete` status).

**The fix**: keep the first `max_changed_paths` paths; report the rest as
`DetectedChanges::changed_paths_omitted_by_limit`. `status` is **not** touched by this case any
more — the scan genuinely completed, only the *list* was capped, which is `ChangeSetSummary`'s
own *display*-level kind of fact (`omitted_changed_file_count`), not `ChangeDetectionStatus`'s
scan-completeness kind (`Partial`). `max_entries`' own `Partial` behaviour is untouched — a
`Partial` scan (baseline or current) still empties `changed_paths` before this check ever runs,
and still refuses `ChangeSet` creation, exactly as before.

**Threading the count through, not adding a second shape** (revised — see the decision below):
`ChangeSet` gained one new field, `changed_files_omitted_by_detection`, carried from
`DetectedChanges::changed_paths_omitted_by_limit` through
`add_detected_generated_change_set`'s three construction arms (Strong/Ambiguous/None
association).

**Required decision (review response 326): keep the two omission counts separate, not summed.**
The first submission of this evidence summed `changed_files_omitted_by_detection` into the
pre-existing `ChangeSetSummary::omitted_changed_file_count`. The reviewer's finding: the two
causes are not the same kind of fact. Display-level omission (`bounded_summary`'s own
`path_limit`) is **recoverable** — the paths are still in `ChangeSet.changed_files`, just past
the limit a caller asked for. Detection-level omission (`max_changed_paths`) is
**unrecoverable** — `truncate(max_changed_paths)` dropped those paths before the `ChangeSet`
ever existed; no larger `path_limit` will ever produce them, because they are nowhere in the
model. A user reading one merged number cannot tell which is true, on a surface whose entire
job is showing what an AI agent changed to a human deciding whether to trust it.

**Decided: split.** `ChangeSetSummary` now carries both counts separately —
`omitted_changed_file_count` (display-level, recoverable) and
`changed_files_omitted_by_detection` (detection-level, unrecoverable) — and the Change Review
surface renders them as two distinct lines, only when each is individually nonzero, with wording
that states the difference (`change-review-omitted-files` vs. `change-review-detection-omitted-files`,
the latter saying outright "cannot be recovered by showing more"). `changed_file_count` alone
stays a sum of both, because it answers a different question — the true total — and must never
under-report what detection genuinely found regardless of which count explains the gap. This is
the same shape as the already-defended `Partial{limit}` vs. `omitted_changed_file_count`
distinction, one level finer, and gets the same treatment: two facts, two lines, never one
number standing in for both.

**Tests**:
- `tekstide-core`: `detector_keeps_the_first_n_changed_paths_and_reports_the_omitted_count` — 2
  real changes, `max_changed_paths: 1`: `status` stays `Complete`, exactly 1 path kept,
  `changed_paths_omitted_by_limit == 1`.
- `tekstide-core`: `changeset_bounded_summary_keeps_display_and_detection_level_omission_separate`
  — **both omission sources non-trivial on the same summary at once**, the first slice where
  that composition is possible: 3 stored, 5 detection-omitted, display `path_limit: 2` →
  `changed_file_count == 8` (true total, still summed), `omitted_changed_file_count == 1`
  (display-only), `changed_files_omitted_by_detection == 5` (detection-only, unchanged from what
  was set) — proving the two stay apart, not that they combine.
- `tekstide`: `change_review_omitted_lines_render_as_two_distinct_facts_when_both_are_true` —
  render-level proof, both lines present, different text, each naming only its own count, only
  the unrecoverable one saying "cannot be recovered."
  `change_review_omitted_lines_are_absent_when_both_are_zero` — the zero case renders neither
  line, not an empty or zero-valued one.
- `projectsession_refuses_changeset_creation_from_non_complete_detection` — rewritten (its old
  fixture used `max_changed_paths` to reach `Partial`, which is no longer possible) to reach
  `Partial` via `max_entries` instead, proving that path's refusal is genuinely unchanged, not
  merely untested after the rewrite.

**Ablated by hand**: reverted the `truncate(self.policy.max_changed_paths)` call to
`truncate(0)` (the pre-fix discard shape) —
`detector_keeps_the_first_n_changed_paths_and_reports_the_omitted_count` failed:
`"exactly max_changed_paths (1) of the 2 real changes must be kept, not 0 and not 2 — left: 0,
right: 1"`. Reverted; full `change_detection` module re-run green.

**`Partial { limit }` still distinct from both display and detection-level truncation**:
unaffected by this change — `change_review_detection_status_line_renders_each_status_distinctly`
(RFC-020's own test) still passes unchanged, still rendering `Partial{limit}` on its own line,
never collapsed into either omission count.

## Closeout

**RFC-020's on-surface disclosure text corrected** (`change-review-disclosure`, `en.ftl`): "excludes
`.git/`, `target/`, and `node_modules/`" → "excludes `target/` and `node_modules/` by design.
Most of `.git/` is excluded too — only `.git/hooks/` and `.git/config` are watched...". Read by
every user who opens Change Review; was false the moment PR-035-A shipped, corrected in the same
commit.

**`README.md` corrected** in two places: the `0.11.0` change-detection bullet under *Current
Status* (previously said "git hooks included" as an example of what is never reported — now
explains the narrow watch and the `hooksPath` deferral), and the *Working With Projects* section
added for the minimal-user-documentation slice (same correction, shorter).

**Deferrals stated, with reasons**: `core.hooksPath` not followed (§2, real scope: reading
config, resolving an arbitrary path, watching a second location that itself changes) — in
`change_detection.rs`'s own doc comments, `README.md`, and this file. Mid-run detection triggers
still exit-only (RFC-035 item 3, unchanged by this slice, already disclosed in `README.md` and
unaffected by anything here).

## Gates

Two rounds — the second after review response 326's required split.

**Round 1** (the original, summed submission): `fmt`/`clippy -D warnings` clean; three
consecutive full-suite runs, 414 tekstide + 741 tekstide-core, clean on runs 1 and 3;
`command_approval_family_produces_real_durable_audit_records_through_the_pipeline` failed once
on run 2 — the already-documented socket flake in `test-process-leak.md`, unrelated to this
diff.

**Round 2** (the split, per the decision above): `cargo fmt --all -- --check` clean (after
`cargo fmt --all` reformatted one long line). `cargo clippy --workspace --all-targets
--all-features -- -D warnings` clean. Three consecutive full-suite runs: 416 tekstide + 741
tekstide-core, clean on runs 1 and 3.
`approval::tests::coordinator::is_still_answerable_reflects_the_real_connection_state` failed
once on run 2 — a **new** entry in `test-process-leak.md`'s own flake table (fifth test, added
2026-08-25), disclosed there with the reasoning for why it plausibly shares the same underlying
cause despite not sharing the first four's exact shape. Unrelated to this diff either way — no
code this slice touches sits anywhere near the approval/coordinator/socket path. `git diff
--check`: clean. Live GUI: release binary, `TEKSTIDE_CHANGESET_DEMO=1`, fresh project — the
corrected `change-review-disclosure` text renders exactly as written, in the real running
application, not only in `en.ftl`.

## Known limitations (RFC-035-wide)

- **`core.hooksPath` redirects are not followed.** Watching `.git/config` reports that the hook
  location changed; it does not resolve where it changed *to*. Real, separate scope, deferred
  with this reason (§2).
- **Items 3 and 4 of RFC-035 remain explicitly out of scope for this slice**: detection still
  runs only at exit (no mid-run trigger), and the captured baseline still does not survive the
  application closing mid-run. Both already disclosed; neither touched here.
- **`max_entries`' own truncation is unchanged and still refuses `ChangeSet` creation.** A scan
  that never completes still cannot distinguish "unchanged" from "not looked at" — `max_changed_paths`
  and `max_entries` are different limits with different, deliberately different, consequences.
