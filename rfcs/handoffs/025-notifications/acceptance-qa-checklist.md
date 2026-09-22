---
title: "RFC-025 — acceptance and QA checklist"
rfc: "RFC-025"
rfc_file: "../../done/025-notifications.md"
source_rfc_status: "Implemented and closed 2026-09-22 — M12"
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

- [x] Trust state, Git state, running sessions, failed sessions and pending approvals appear
      (REQ-NOTIFY-002), each present when true and **absent when not**, ablated separately.
      *`active_project_status_fields`. Six tests, one per field's presence plus the "nothing else"
      absence case. G1 (trust) fails its own test alone; G2 (running/failed/pending unconditional)
      fails two tests as one disclosed composition -- the same "absent at zero" guard removed for
      all three at once.*
- [x] **Git state reads "not available"**, and nothing pretends otherwise, until RFC-030 lands.
      *A fixed catalog key, no `ProjectGitSummary` read -- nothing in production populates that type
      yet, so reading it would still say "not available" today but this is the more literal, D5-exact
      reading. Captured live: "Git: not available" in the shipping binary.*
- [x] Labels are states, not bare counts (REQ-NOTIFY-003), read from `ProjectRuntimeSummary`.
      *`active_project_status_fields_reads_the_summary_it_is_given_not_a_recount` source-scans the
      function's own body for `.len()`/`.count()`/`.filter(` and finds none. Every count assertion in
      the other tests checks for the word and the digit together.*
- [x] Every state reads as a word without colour (REQ-NOTIFY-005), and is reachable by keyboard
      (004).
      *By construction: plain `text()` widgets, one uniform bar colour, no new focusable control (D1:
      nothing yet needs acknowledging). Noted in `qa-evidence.md` rather than exercised by a dedicated
      test, since there is no colour channel or new keybinding for a test to catch drifting.*
- [x] Live capture with a running session, throwaway state only.
      *`evidence/01-status-bar-restricted-git-not-available-1-running.png`, the release binary against
      a `mktemp -d` config, state and project: "Restricted　Git: not available　1 running". No path
      under a home directory appears in the image.*
- [ ] ~~…and a pending approval.~~ **Struck at review 2026-09-22 — the reviewer's error.** The shipped
      product has no path to a command-approval dialog: `to_ai_cli_profile` sets
      `compatibility_level: Supervised` unconditionally, held by
      `managed_compatibility_level_without_structured_action_approval_is_still_rejected`, and the
      delivery plan already records command approval as exercisable only by the reference adapter.
      Demo machinery to force a `Managed` profile was offered and **refused**: a way around the
      translator that refuses it is a second door into what RFC-021's validation keeps shut. The
      field's correctness is held by a real `ApprovalRequest` through production's own
      `add_approval_request`; the live capture comes with the adapter pathway.

## Whole-RFC

- [x] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, and **three consecutive full-workspace runs with
      `--no-fail-fast`**, output redirected to files.
      *Re-run by the reviewer over both commits: 556 + 9 + 829, green three times; fmt and clippy
      clean; `git diff --check` clean.*
- [x] Every new intermittent failure has a dated row in `test-process-leak.md`.
      *None new. One already-registered intermittent (review 338's row) recurred once, unrelated.*
- [x] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [x] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Accepted 2026-09-22 (review 404). Both slices, no review-found defects; RFC-025 closed.

Verified independently, four ablations restored and hash-checked:
  X1 ordered_by_kind no longer sorts        -> the hand-scrambled kind-order test, alone.
     (This is the ablation the implementer reported as untestable at the call site: the
      property is falsifiable one level in, at its own implementation.)
  X2 the pending-approval label renders at zero -> the "nothing else" test, alone.
  X3 the fields are computed and never pushed onto the row -> NOTHING. The wiring into the
     visible bar is held only by the live capture; a box is carried into RFC-030 PR-030-B,
     which edits this same row when it fills the Git field.
  X4 restore the old four-*_lines call site -> does not compile, 4x E0425. "The board renders
     the model" is compiler-enforced, stronger than the grep the plan asked for.

Zero deleted lines in shell/tests.rs across both commits, checked against the diff, not the
claim. lifetime and scope have no production consumer -- read only by tests -- so the tag
documents and pins, it does not enforce. Gate re-run: 556 + 9 + 829, green three times.
```
