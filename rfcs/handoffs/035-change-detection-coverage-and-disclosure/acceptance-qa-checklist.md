---
title: "RFC-035: Acceptance / QA Checklist"
rfc: "RFC-035"
rfc_file: "../../accepted/035-change-detection-coverage-and-disclosure.md"
source_rfc_status: "Accepted 2026-08-18 — scheduled 2026-08-25"
target_milestone: "M12"
created: "2026-08-25"
---

# Acceptance / QA Checklist

Every unchecked line at closeout carries a stated reason.

## The acceptance criterion

- [x] **An agent run that writes `.git/hooks/pre-commit` shows that file on the change review
      surface.** Proven against a real written hook, not a constructed `DetectedChanges`.
      `change_review_surface_shows_a_real_git_hook_a_real_agent_run_installed` (`tekstide`):
      real `Managed` launch, real approval, real exit, real click.
- [x] A run changing more paths than `max_changed_paths` shows the first N **and** an honest count
      of the omitted rest, instead of nothing.
      `detector_keeps_the_first_n_changed_paths_and_reports_the_omitted_count` (`tekstide-core`).

## PR-035-A — the supervision hole

- [x] `.git/hooks/` and `.git/config` both watched; everything else under `.git/` still excluded.
      `git_hooks_pre_commit_is_watched_while_churn_paths_under_git_stay_excluded`,
      `git_config_is_watched_while_churn_paths_under_git_stay_excluded` — both assert the watched
      path appears and `.git/objects/`, `.git/refs/`, `.git/index` do not.
- [x] **Readers of the exclusion enumerated before it was changed**, with what each does — the
      explorer among them. Two: the detector's `ignored_directory_names` default, and the
      explorer's `collapsed_directory_names`. See `qa-evidence.md`'s PR-035-A entry.
- [x] The explorer still collapses `.git/`; a user does not get a tree full of `objects/`.
      Untouched code path — `project::root::explorer::tests` (17 tests) unaffected, re-run green.
- [x] A changed hook renders as an ordinary changed path — no new severity, icon or list.
      `scan_git_directory` reuses `classify_and_push_entry` unchanged from `scan_directory`; the
      surface test renders it through the ordinary `change_review_file_entry_line`.
- [x] `core.hooksPath` not followed; deferral recorded with its reason.
      `git_hooks_path_redirect_in_config_is_not_followed`, plus doc comments in
      `change_detection.rs`, `README.md`, `qa-evidence.md`.
- [x] Ablated **separately**: `hooks/` removed → its test fails; `config` removed → its test fails.
      By hand (no runtime knob — `GIT_WATCHED_ENTRY_NAMES` is a non-configurable `const` by
      design); exact failing values recorded in `qa-evidence.md`.

## PR-035-B — `max_changed_paths`

- [x] A completed scan over the limit keeps the first N and reports the omitted count.
- [x] `ChangeSetSummary`'s existing fields populated rather than a second shape added.
      `ChangeSet::changed_files_omitted_by_detection` (new) feeds `changed_file_count` (summed,
      the true total) and a **new** `ChangeSetSummary::changed_files_omitted_by_detection` field
      — **not** summed into the pre-existing `omitted_changed_file_count`, per review response
      326's required decision below.
- [x] `Partial { limit }` still distinct from display truncation, **tested with both true at
      once** — the first slice where that is possible. **Required correction (review response
      326)**: display-level and detection-level omission are also two distinct facts and must
      not be summed into each other either — see "Decided: split" in `qa-evidence.md`.
      `changeset_bounded_summary_keeps_display_and_detection_level_omission_separate`: 3 stored +
      5 detection-omitted + display cap 2 → `changed_file_count: 8` (true total),
      `omitted_changed_file_count: 1` (display-only), `changed_files_omitted_by_detection: 5`
      (detection-only, kept apart). Render-level:
      `change_review_omitted_lines_render_as_two_distinct_facts_when_both_are_true`,
      `change_review_omitted_lines_are_absent_when_both_are_zero`.
- [x] `max_entries` behaviour unchanged.
      `projectsession_refuses_changeset_creation_from_non_complete_detection` rewritten to reach
      `Partial` via `max_entries` (the only way left) and still refuses `ChangeSet` creation.
- [x] Ablated: restore the discard, the bounded-list test fails.
      Exact failing value in `qa-evidence.md` (`left: 0, right: 1`).

## Closeout

- [x] **RFC-020's on-surface disclosure text corrected** — it names the `.git/` exclusion and is
      read by every user who opens change review. `change-review-disclosure` (`en.ftl`).
- [x] `README.md`'s exclusion statement corrected. Two places: the `0.11.0` change-detection
      bullet, and the *Working With Projects* Change Review paragraph.
- [x] Deferrals stated: `hooksPath`, mid-run triggers. Both in `README.md`; `hooksPath` also in
      `change_detection.rs`'s own doc comments and `rfcs/future-work.md`.
- [x] Gates: `fmt`, `clippy -D warnings`, full suite **three runs under default parallelism**,
      `git diff --check`. Clean, both before and after the required split: 416 tekstide + 741
      tekstide-core (runs 1 and 3 of the final round); see the flake line for run 2 of each round.
- [x] Flakes disclosed against `test-process-leak.md`'s causes (now five, not three — see
      below). Round 1: `command_approval_family_produces_real_durable_audit_records_through_the_pipeline`
      failed once on run 2 — already-documented. Round 2 (after the split):
      `approval::tests::coordinator::is_still_answerable_reflects_the_real_connection_state`
      failed once on run 2 — a **new** fifth entry, added to `test-process-leak.md`'s own table
      with the reasoning for why it plausibly shares the same cause. Neither flake is related to
      this diff; not chased further, matching this project's own established disclosure
      discipline.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
