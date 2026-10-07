# PR-065-D evidence

Fixture: a throwaway project under `mktemp -d /dev/shm/tk065d-proj.XXXXXX` with two files,
`first.txt` and `second.txt`, each two lines, and a throwaway `XDG_STATE_HOME` under
`/dev/shm/tk065d-state.XXXXXX`. Captured against `target/debug/tekstide`, window-managed with
`niri`, keys sent with `wtype`, each key preceded by an explicit
`niri msg action focus-window --id <n>`.

## Review 477's required case: a mostly-clean save-all, mtime-verified

| | |
| --- | --- |
| `01-first-opened-clean.png` | `first.txt` opened: clean, `first original line one` / `first original line two`. The **"Save All" button is visible for the first time in any RFC-065 capture** -- the first visible control the RFC ships, per review 477's own required item 4. |
| `02-first-edited-unsaved.png` | Typed `EDITED ` at the start: `first.txt (unsaved changes)`. |
| `03-second-opened-clean-both-open.png` | `second.txt` opened from the same workspace, through the sidebar (`Tab` to `Sidebar`, `Down`, `Enter`) -- clean, active, `second original line one` / `second original line two`. Both rows now carry `[open]`; `second.txt` carries no `(unsaved changes)` suffix. |
| `04-save-all-2-of-2-saved.png` | **`Ctrl+Shift+S`** pressed once: the notice reads **"Save all: 2 of 2 saved"** -- exactly the misleading-looking case review 477 named. `second.txt` was never edited, yet it counts toward "2 of 2." |

**On-disk proof that "2 of 2 saved" is not "2 of 2 written"**, taken immediately before and after
the `Ctrl+Shift+S` in the table above (`stat -c '%Y %n'`, Unix seconds):

```
before:  1791412259 first.txt      1791412259 second.txt
after:   1791412497 first.txt      1791412259 second.txt
```

`first.txt`'s mtime advances and its content on disk now reads `EDITED first original line
one`. `second.txt`'s mtime is byte-identical to before the save-all, and its content on disk is
still `second original line one` / `second original line two`, untouched. This is the live
counterpart of `a_clean_document_in_the_open_set_succeeds_without_being_rewritten`
(`crates/tekstide-core/src/project/tests/content.rs`): the notice's own "saved" counts
[`SaveAllOutcome::succeeded_count`], which is true and defensible user-facing wording (a clean
document really is saved, in the sense that its on-disk content already matches what is open),
but is not a count of documents the action actually wrote to disk -- the clean one's own
`save()` returns `Ok(SaveDecision::Saved)` from the early `if !self.is_dirty()` return in
`content/document.rs`, before `write_text_via_temp_rename` is ever reached. This capture is the
thing review 477 said would have surfaced the bug on its own, had it been taken for PR-065-D's
original submission.

## A note on reaching this state: three keyboard focus zones, not two

Getting from the Project Board to a typed edit took more `Tab` presses than expected, worth
recording since it is not about save-all itself. `FocusZone` (`crates/tekstide/src/input.rs`)
cycles `MainArea -> Sidebar -> TabStrip -> MainArea`. From a freshly opened project workspace,
focus starts on `Sidebar`: `Enter` there opens the highlighted file but does not move focus into
`MainArea`, so typed keys sent right after opening a file land nowhere useful (tried once,
confirmed by a screenshot showing no edit, not kept in this folder). Reaching `MainArea` to type
needs `Tab` cycled all the way around (`Sidebar -> TabStrip -> MainArea`) first. None of this is
new to this slice -- `pr-065-c`'s own evidence does not call it out because its switcher
(`Ctrl+Alt+F`) does not depend on which zone has focus -- but save-all's own "type in one
document, leave another clean" setup is the first capture in this RFC that needed to open two
documents and edit only one, so it is the first to hit it.
