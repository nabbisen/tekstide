---
title: "RFC-053 — QA evidence"
rfc: "RFC-053"
created: "2026-09-24"
---

# QA evidence

## PR-053-A — what the surfaces say

Commit `8917003` (code and tests); this file, the checklist and the captures follow it.

### What changed, by decision

| | Change | Where |
| --- | --- | --- |
| D1 | Terminal-mode empty state: *"Terminal mode. Nothing is running in this project yet. Start a terminal with the button below."* The two other placeholders that named an RFC or said only "Sidebar" now say what the area is. | `en.ftl` |
| D9 | `no_catalog_string_names_an_internal_identifier` scans every shipped `.ftl` **message line** for `RFC-` / `PR-0`; `#` comments are skipped. A second test plants a string, a selector arm and a comment to prove the scanner is not simply matching nothing. Failure message says which it covers and not to delete a comment. | `i18n/enforcement.rs` |
| D4 | Change Review's empty state: *"No AI CLI run has produced changes in this project yet. Changes you made yourself are shown by the status bar and the file explorer."* | `en.ftl` |
| D5 | The board row counts the collections it holds when the runtime summary has not been refreshed, so a fresh project reads **0 terminals / 0 agent runs**. `ProjectProviderState` and `project/metadata.rs` untouched. | `project_board.rs` |
| D6 | *"N unsaved files"* replaces *"N dirty files"*; the attention state those buffers raise says *"Unsaved edits"*, not *"Dirty"*. | `en.ftl` |
| D7 | A `blocked: …` line names the blocked automations after the count, from `blocked_automation_labels`. | `surface/board.rs`, `en.ftl` |
| D8 | Empty Approval History says it is empty and shows no caveats; the two caveats appear only with entries (`approval_history_leading_keys`). | `shell.rs` |

### Judgment calls, disclosed

- **The attention word.** The board's *attention* line said "Dirty" for the same unsaved-buffers fact D6 renames. Leaving it would have left the two-words-for-one-fact defect one line below the fix, so it is changed too. Not in the pack's list.
- **The sidebar's "Sidebar".** The RFC's table lists it and no decision covers it. It is one catalog string in the same class as G1 (it said nothing true), so it is reworded here: *"Files are listed here in Content mode."*
- **The close-project dialog still says "dirty file".** Same fact, different surface, and it is not adjacent to a Git count. Left alone; if D6's principle is meant to reach it, that is a one-string change.
- **D7 renders the labels through `untrusted`.** The labels are a fixed, trusted set, but `CatalogArgs` has no runtime-string trusted argument (`trusted_symbol` is `&'static str`), so they go through `quote_untrusted`, which adds only invisible isolate marks to plain ASCII. The alternative was concatenating English in Rust.
- **No keyboard chord in the Terminal-mode text.** Naming `Ctrl+Alt+T` there would be a claim that becomes false if the binding is ever changed — the class of defect this RFC is about. The button is what the text points at.

### Ablation — restore one old wording

Run from a committed tree, restoring each file with `git checkout --` afterward (`git status` empty before and after).

| Restored | Failed |
| --- | --- |
| Terminal placeholder (`RFC-017 adds the terminal here`) | `terminal_mode_empty_state_…` **and** the D9 scan `no_catalog_string_names_an_internal_identifier` |
| Sidebar `Sidebar` | `the_sidebar_and_content_placeholders_…` |
| Change Review "No changes have been detected in this project yet" | `change_review_empty_state_…` and the agreement test |
| `dirty files` (plural arm) | `the_editor_buffer_count_says_unsaved_and_never_dirty` and the agreement test |
| Attention `Dirty` | `the_attention_state_raised_by_unsaved_buffers_says_unsaved` |
| `Unknown` for a fresh project's terminals | the two board tests in core **and** the agreement test |
| Blocked names line disabled | `blocked_automations_are_named_not_only_counted` |
| Caveats restored above the empty state | `approval_history_leads_with_its_empty_state_…` |

Every ablation fails its own dedicated test. "Alone" is not literally true for four rows — the agreement test, or the scan, is a second detector — which is the design, not a leak: it is the cross-surface check the RFC exists to add.

### Live capture — release binary, `mktemp -d` fixture, throwaway `XDG_STATE_HOME`

A git repository with one modified and one untracked file, no agent run.

- `evidence/01-board-and-status-bar-agree.png` — board: *0 terminals, 0 agent runs, 0 pending approvals, 0 reviews, 0 unsaved files*, then *9 blocked automations* and the nine names. Bottom bar: *Git: master 2 changed*.
- `evidence/02-change-review-empty-state-beside-two-changed-files.png` — Change Review: *"No AI CLI run has produced changes in this project yet. Changes you made yourself are shown by the status bar and the file explorer."* In the **same frame**: the explorer's `[modified]` and `[untracked]` badges and the bar's *2 changed*.
- `evidence/03-terminal-mode-empty-state.png` — Terminal mode's empty state and the *+ New Terminal* button it points at; the sidebar reads *Files are listed here in Content mode.*

**The checklist's "one frame" cannot be literal.** The board is a route; Change Review is a surface inside the project workspace. They are never on screen together, so the box stays unticked with that named. Images 01 and 02 are one session and one fixture; 02 is the frame in which Change Review, the explorer and the bar agree.

### Gate

`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and three consecutive `cargo test --workspace --all-targets --no-fail-fast` runs, output to files: **581 + 9 + 876**, green all three (`+11` in `tekstide` over `0.22.0`: 2 scan tests, 5 shell tests, 4 board tests; `tekstide-core` unchanged — two existing tests were re-pointed at the new answer, none added). The colour-alone and i18n completeness scans are part of that run and pass. No new intermittent.
