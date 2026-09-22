---
title: "RFC-025 — acceptance and QA checklist"
rfc: "RFC-025"
rfc_file: "../../accepted/025-notifications.md"
source_rfc_status: "Accepted 2026-09-22 — M12"
target_milestone: "M12"
created: "2026-09-22"
---

# Acceptance and QA checklist

Every box is a property. A box whose plan assigns it elsewhere, or that cannot be satisfied as
written, stays unticked with the contradiction named — the reviewer's error to fix, not the
implementer's to paper over.

## PR-025-A — the model and the migration

- [x] **Every existing absent-when-false test passes unmodified.** Name them in the evidence.
      **Ablation:** give a migrated notice the other lifetime; its own test fails.
      *All 16 existing tests named in `qa-evidence.md`, diffed against the pre-migration tree — no
      test file line changed. E1 (recent-list notice reads the live disk figure instead of the boot
      snapshot) fails the pre-existing, unmodified `the_reset_notice_keeps_the_boot_figure_after_the_live_one_changes`.*
- [x] A notification cannot carry a lifetime outside the two (§3) — by the type, not by review.
      *`NotificationLifetime` is a two-variant enum; `until acknowledged` was not added, per D1's
      acceptance ruling.*
- [x] The four producers construct notifications; **no notice text changed**. **Grep:** the board
      renders the model, not strings from four functions.
      *Each `*_notifications` function holds the unchanged conditional logic; each `*_lines` function
      is now a one-line projection over it, `#[cfg(test)]`. Grepped: the four `*_lines` names each
      appear exactly once — their own definitions — nowhere else.*
- [x] Order is deterministic and defined by kind, not arrival (D7).
      *`ordered_by_kind`, `derive(Ord)` on `NotificationKind`.
      `notifications_render_in_kind_order_regardless_of_insertion_order` proves it against a
      hand-scrambled `Vec<Notification>`; `project_board_notifications_from_a_real_mixed_state_are_kind_ordered`
      proves it against real, mixed state.*

## PR-025-B — the status bar

- [ ] Trust state, Git state, running sessions, failed sessions and pending approvals appear
      (REQ-NOTIFY-002), each present when true and **absent when not**, ablated separately.
- [ ] **Git state reads "not available"**, and nothing pretends otherwise, until RFC-030 lands.
- [ ] Labels are states, not bare counts (REQ-NOTIFY-003), read from `ProjectRuntimeSummary`.
- [ ] Every state reads as a word without colour (REQ-NOTIFY-005), and is reachable by keyboard
      (004).
- [ ] Live capture with a running session and a pending approval, throwaway state only.

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
