---
title: "RFC-025 — QA evidence"
rfc: "RFC-025"
rfc_file: "../../accepted/025-notifications.md"
source_rfc_status: "Accepted 2026-09-22 — M12"
target_milestone: "M12"
created: "2026-09-22"
---

# Evidence

## PR-025-A — the model, and the four migrate onto it

### One type, and the design §1 requires

`Notification { scope: Option<ProjectId>, kind: NotificationKind, text: String, lifetime: NotificationLifetime }`.
`NotificationLifetime` is a two-variant enum (`WhileConditionHolds`, `ForTheStartItHappened`) —
`until acknowledged` stays out, per D1's acceptance ruling: nothing produces it. `NotificationKind`
is a four-variant enum, `derive(Ord)`, ranked by declaration order — the same order the board's one
call site already concatenated in.

**`scope` has no `Project(ProjectId)` variant.** Every notice this slice migrates is global, and an
unconstructed enum variant is exactly the dormant-capability shape RFC-036 closed (found and fixed
in this project's own history at least three times already this quarter). `scope` is
`Option<ProjectId>`, `None` everywhere today; a project-scoped producer sets `Some(id)` when one
exists, the same "returns with the first thing that needs it" discipline D1 already applies to the
third lifetime.

### No duplicated logic, by construction

Each of the four migrated functions kept its exact name and signature (`fn(&State) -> Vec<String>`),
but is now a **one-line projection** over a new `*_notifications` sibling that holds the real,
unchanged conditional logic — `project_board_audit_lines`, `_configuration_lines`,
`_recent_projects_reset_lines`, `_transcript_cleanup_lines` all now read
`<sibling>(state).into_iter().map(|n| n.text).collect()`. There is exactly one place each notice's
text is decided; the string-returning function cannot diverge from the notification-returning one
because it is derived from it, not reimplemented beside it — the very risk the RFC's own intro names
("the risk is not what it adds; it is what a migration can silently change").

The four `*_lines` functions are `#[cfg(test)]`: nothing in production calls them any more (grepped
below), and this project deletes rather than keeps a capability with no production caller.

### Every existing absent-when-false test passes unmodified

No test file line changed for any of the following (diffed against the pre-migration tree):

```
project_board_audit_lines_is_empty_when_healthy_and_never_recovered
project_board_audit_lines_shows_the_degraded_line_when_degraded
project_board_audit_lines_shows_the_quarantine_path_when_recovered
project_board_audit_lines_shows_the_collision_line_not_the_generic_degraded_line
project_board_audit_lines_omits_the_present_tense_line_once_a_transient_open_failure_clears
project_board_audit_lines_still_shows_the_history_line_once_a_transient_open_failure_clears
an_invalid_configuration_file_boots_with_defaults_and_the_board_says_it_was_ignored
a_typo_in_the_configuration_file_is_named_on_the_board
a_clean_configuration_renders_no_board_line_at_all
no_configuration_file_at_all_renders_no_board_line
an_unresolvable_configuration_path_is_a_diagnostic_not_an_exit
the_board_says_the_recent_list_was_reset_on_the_start_it_happened
the_board_says_the_recent_list_was_recovered_from_the_last_saved_copy
the_recovered_line_says_nothing_about_orphaned_transcripts
the_reset_notice_keeps_the_boot_figure_after_the_live_one_changes
the_reset_notice_says_nothing_about_transcripts_when_there_were_none
the_board_says_nothing_about_a_reset_on_a_start_with_a_readable_list
the_board_says_what_retention_removed
the_board_says_nothing_when_retention_removed_nothing
the_board_reports_a_failed_deletion_separately_from_what_was_removed
```

Before the migration: 541 shell tests. After: 549 (541 unchanged + 8 new). The workspace gate is
identical in every other binary (9 + 829, unchanged).

### Order is deterministic and defined by kind

`ordered_by_kind` sorts by `NotificationKind`'s declared order. Two tests:
`notifications_render_in_kind_order_regardless_of_insertion_order` builds a `Vec<Notification>` by
hand in a **scrambled** order and asserts the sort recovers audit → configuration → recent-list →
retention; `project_board_notifications_from_a_real_mixed_state_are_kind_ordered` drives the same
property through a real, mixed `State` with three of the four kinds present at once.

**The board renders the model, not four functions' strings.** Grepped:

```
$ grep -n 'project_board_audit_lines(\|project_board_configuration_lines(\|project_board_recent_projects_reset_lines(\|project_board_transcript_cleanup_lines(' crates/tekstide/src/shell.rs
7558:fn project_board_audit_lines(state: &State) -> Vec<String> {
7713:fn project_board_recent_projects_reset_lines(state: &State) -> Vec<String> {
7772:fn project_board_transcript_cleanup_lines(state: &State) -> Vec<String> {
7841:fn project_board_configuration_lines(state: &State) -> Vec<String> {
```

Each name appears exactly once — its own definition. No call site remains; `content_area`'s
`ProjectBoard` arm calls `project_board_notifications(state)` and extracts `.text`.

### A lifetime is a property of how a notice is computed, not a label

Of the four, exactly one — `RecentProjectListRepair` — has an implementation where the two lifetimes
genuinely produce different, observable behaviour: `state.recent_project_list_repair` is computed
once at boot and never touched again; the other three are read fresh from continuously-live state
(`state.audit_health`, `state.configuration`, `state.transcript_cleanup_notice`, the last one
overwritten at every retention trigger). Two new tests pin both directions directly:

- `the_recent_projects_reset_notice_is_fixed_at_boot_not_recomputed_live`: the notification is
  captured, live state is then changed, and the notification is recomputed — asserted **equal** to
  the first capture.
- `the_retention_notice_reflects_the_most_recent_cleanup_not_the_first`: a real project-open trigger
  removes a transcript and leaves a notice; a second cleanup removes nothing; the board is asserted
  **empty** afterward — a fixed-at-first-event implementation would leave the first notice stuck.

### Ablations, each restored and hash-checked

| | Ablation | Fails |
| --- | --- | --- |
| E1 | give the recent-list notice the other lifetime (render the live disk figure instead of the boot snapshot) | `the_reset_notice_keeps_the_boot_figure_after_the_live_one_changes` (pre-existing, unmodified) **and** `the_recent_projects_reset_notice_is_fixed_at_boot_not_recomputed_live` (new) — the same property, both directions |
| E3 | the retention notice is never cleared once set (sticks after the first cleanup) | `the_retention_notice_reflects_the_most_recent_cleanup_not_the_first` **alone** |

**E1 is the checklist's required ablation**: "give a migrated notice the other lifetime; its own test
fails." The pre-existing, unmodified test is exactly what fails — no test needed editing to catch it.

**A third ablation (order_by_kind skipped at the call site) was attempted and abandoned as
untestable-as-written**: the call site has no separate sort step to remove — `project_board_notifications`
is the only function that assembles the four producers, and it always sorts internally before
returning, so there is no code path left that could skip the sort without also failing the grep
above. The order property is instead pinned directly at `ordered_by_kind` and at
`project_board_notifications`, per the two tests already listed.

### Greps

```
non-test callers of the four *_lines functions:  none (all four are #[cfg(test)])
NotificationScope::Project constructed anywhere:  n/a -- no such variant exists (see above)
board call site:                                  project_board_notifications(state), one call
```

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`: clean.
`rfc_docs_invariants`: 9 passed. **Three consecutive full-workspace runs with `--no-fail-fast`,
output redirected to files: 549 + 9 + 829, green every time** (+8 shell tests; every other binary
unchanged). `git diff --cached --check` after staging: clean.
