# PR-065-A evidence

Fixture: a throwaway project under `mktemp -d /dev/shm/tk065-proj.XXXXXX` with two files,
`first.txt` ("first original") and `second.txt` ("second original"), and a throwaway
`XDG_STATE_HOME` under `/dev/shm/tk065-state.XXXXXX`. Captured against `target/debug/tekstide`,
window-managed with `niri`, keys sent with `wtype`, each key preceded by an explicit
`niri msg action focus-window --id <n>` (not a check-then-skip on `focused-window`, which a
stolen-focus window on this shared desktop can race).

## The repair: edit one file, open another, the first survives

| | |
| --- | --- |
| `01-project-workspace-no-file-open.png` | Inside the project, nothing open yet. |
| `02-first-opened.png` | `first.txt` opened: clean, `first original`. |
| `03-first-edited.png` | Typed `EDITED ` at the start: `first.txt (unsaved changes)`. |
| `04-board-after-editing-first.png` | Project Board: **"1 unsaved file"**, before any second file is opened. |
| `05-second-opened.png` | `second.txt` opened from the same workspace: clean, active, `second original`. |
| `06-board-after-opening-second.png` | Project Board again: **"1 unsaved file" still holds** -- the first document's edit was not discarded. |

This is the live counterpart of
`opening_a_second_file_leaves_the_first_s_text_and_dirty_state_intact` (core-level) and
`opening_a_second_file_through_real_routing_leaves_the_boards_dirty_count_intact`
(shell-level, `crates/tekstide/src/shell/tests.rs`).

## A disclosed, deliberate gap found while capturing this: the explorer's `[open]` tag

Comparing `02-first-opened.png`/`03-first-edited.png` against `05-second-opened.png`: the
sidebar's `[open]` tag moves from `first.txt` to `second.txt` -- it does not mark both, even
though both are genuinely open in the set (the board's own count, independent of this tag,
proves it). `crates/tekstide/src/surface/explorer.rs`'s `row_text` compares
`context.open_path == Some(node.relative_path.as_path())`, singular equality against one path,
not membership in the open set. This is correct under RFC-065 D2's own boundary ("of 83 sites
that read it, the two that **count** are `open_buffer_count` and `dirty_file_count`" -- this
tag is neither), not a regression introduced here. Left for B/C: once the explorer needs to
show what's actually open (not just what's active), this is the line that has to change.
Disclosed rather than silently left for a reviewer to rediscover.

## A disclosed, deliberate gap found the same way: reopening an already-open path

`07-reopen-same-path-second-entry.png` and `08-board-after-reopen-still-shows-unsaved-edit.png`
are from an earlier run where a navigation sequence (switching project tabs, then landing on
the sidebar with its own row highlighted) re-activated `first.txt` from the explorer after it
was already open and dirty. The result: a **second**, fresh entry for the same path, clean,
cursor at the start -- not a return to the dirty entry, and not a loss of it (the board still
read "1 unsaved file", i.e. `08-...png`). `reopening_an_already_open_path_adds_a_second_entry_
rather_than_losing_the_first` (core-level test) locks this in as the tested, intended boundary:
identity-aware reuse of an already-open path is PR-065-B/C's job, not this slice's. Nothing was
lost; the UX of landing back on the same tab you already had open is simply not built yet.

## A false alarm, and how it was run down

An earlier, less careful capture attempt (same sequence, different fixture, mid-session focus
counting mistakes) showed the Project Board reading **"0 unsaved files"** after opening the
second file -- apparently contradicting the core-level test. Rather than report that as a
product defect, it was run down with a shell-level integration test,
`opening_a_second_file_through_real_routing_leaves_the_boards_dirty_count_intact`
(`crates/tekstide/src/shell/tests.rs`), that drives the *exact* code path the GUI uses end to
end (a real routed keypress through `route_non_modal_input`, then the same
`open_active_project_text_document` call `Action::Open`'s own handler in `shell.rs` makes) and
asserts the board row's `dirty_file_count`. That test passes deterministically. A second,
careful live capture (this one, with the board checked at every intermediate step) also shows
the correct count throughout. The "0 unsaved files" reading is attributed to the manual GUI
automation on this shared desktop (almost certainly a keystroke landing somewhere unintended
during an earlier, miscounted focus-zone sequence in that same window), not a product defect --
the deterministic test is the evidence that matters here, not the one screenshot that could not
be reproduced.
