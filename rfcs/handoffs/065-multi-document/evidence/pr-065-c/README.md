# PR-065-C evidence

Fixture: a throwaway project under `mktemp -d /dev/shm/tk065c-proj.XXXXXX` with three files
(`first.txt`, `second.txt`, `third.txt`), each two lines, and a throwaway `XDG_STATE_HOME`
under `/dev/shm/tk065c-state.XXXXXX`. Captured against `target/debug/tekstide`, window-managed
with `niri`, keys sent with `wtype`, each key preceded by an explicit
`niri msg action focus-window --id <n>`.

## The switcher: `Ctrl+Alt+F` cycles the open set, cursor and viewport restored

| | |
| --- | --- |
| `01-first-opened.png` | `first.txt` opened: clean, Line 1 Column 1. |
| `02-first-cursor-line2-col4.png` | Cursor moved to **Line 2, Column 4** with no edit (arrow keys only). |
| `03-second-cursor-line1-col3.png` | `second.txt` opened (now active); its own cursor moved to **Line 1, Column 3**. Note the sidebar: `[open]` now marks both rows (PR-065-B's own fix). |
| `04-third-cursor-line2-col2-active.png` | `third.txt` opened (now active); its own cursor moved to **Line 2, Column 2**. All three rows now carry `[open]`. |
| `05-ctrl-alt-f-wraps-to-first-cursor-restored.png` | **`Ctrl+Alt+F`** pressed once: wraps from the last-opened document (`third.txt`) to the first (`first.txt`) -- shown active again at **exactly Line 2, Column 4**, the position it was left at before either other document was opened. |
| `06-ctrl-alt-f-to-second-cursor-restored.png` | `Ctrl+Alt+F` again: `second.txt` active at **exactly Line 1, Column 3**. |
| `07-ctrl-alt-f-to-third-cursor-restored.png` | `Ctrl+Alt+F` again: `third.txt` active at **exactly Line 2, Column 2** -- the full cycle (third → first → second → third) complete, every cursor restored, nothing re-read from disk, nothing reset. |

This is the live counterpart of `cycling_the_active_document_wraps_and_restores_cursor_and_
viewport` (core-level, `crates/tekstide-core/src/project/tests/content.rs`) and
`ctrl_alt_f_cycles_to_the_next_open_document_wrapping` (shell-level, real routing,
`crates/tekstide/src/shell/tests.rs`).

## A disclosed, minor observation: the sidebar's own keyboard cursor does not follow the switch

In every capture above, the explorer's own highlighted row (`>`) stays on whichever row the
user last navigated to directly (`third.txt` throughout, since the switch itself never touches
sidebar navigation) rather than following the newly active document. This is consistent with
the architecture, not a defect: `Ctrl+Alt+F` is a global action that changes which document is
*active*; `explorer_highlight` is a separate, sidebar-local keyboard cursor over the tree's own
rows, untouched by it -- the same separation that already existed between "the keyboard
highlight" and "the open file" before this slice (`surface::explorer`'s own doc: "the highlight
is `> ` on the row; the open file is the word `[open]`. They are independent."). A user who
wants the sidebar's own highlight to follow would press `Tab` to the sidebar and navigate, the
same as before. Not fixed here; noted so a reviewer does not have to rediscover it from a
screenshot.
