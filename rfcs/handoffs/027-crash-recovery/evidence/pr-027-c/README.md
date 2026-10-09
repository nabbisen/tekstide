# PR-027-C evidence

Fixture: a throwaway project under `mktemp -d /tmp/tekstide-live-capture-027c.XXXXXX` with one
file, `notes.txt`, two lines. Captured against `target/debug/tekstide`, window-managed with
`niri`, keys sent with `wtype`, each key preceded by an explicit
`niri msg focused-window`/`niri msg action focus-window --id <n>` check. No test-isolated
`XDG_STATE_HOME` was used (the real one, matching what a user's own launch would read) --
`recent-projects.json`, the recovery instance marker, and the recovery record were all removed
from the real state directory afterward, by hand, to leave the environment as found.

## The scenario, step by step

1. Launched `tekstide /tmp/tekstide-live-capture-027c.XXXXXX` (a fresh project, added via the
   CLI-argument path).
2. Opened `notes.txt`, typed `UNSAVED EDIT -- ` at the start -- a real, unsaved edit, never saved.
3. Waited past one `RECOVERY_PERSIST_INTERVAL` tick (2 seconds) for the real record to be written.
   Confirmed on disk before continuing: `recovery/records/<project_id>/<hash>.json`, `text` holding
   the typed edit, `snapshot.len: 48` (the file's own real length at that moment).
4. **Real `kill -9`** on the running process -- not `simulate_crash()`. The instance marker (named
   by the real pid) was left behind, confirming D1/D12's own detection would fire.
5. **Modified `notes.txt` from outside Tekstide entirely**, while it was not running --
   `this file changed on disk after the crash\nnobody at the keyboard wrote this\n`, 76 bytes. This
   is the changed-on-disk case the task-breakdown's own "Live capture" section names explicitly
   ("capture the *changed-on-disk* case rather than the easy one").
6. Relaunched `tekstide` against the same project path.

## `01-recovery-offer-pending.png` -- the offer, driven by the record alone

The offer opens immediately on restart: **"Recover unsaved work in tekstide-live-capture-027c…?"**,
**"Recovered documents come back without their undo history"** (D3, §3 row 10 -- the item review
492 required a direct render proof for; this is that proof, the gap no unit test in this codebase
could close on its own), and the row `notes.txt`, not yet decided.

No crash marker was ever read to produce this screen -- Amendment 1 holds live, not only in the
unit fixtures (which construct `State` with `instance_marker: None` by necessity, since nothing
else is available to them): the marker file sat in `recovery/instances/` throughout, consulted by
nothing this screen reads from.

## `02-recovery-offer-accepted-conflict.png` -- D5's "changed" case, accepted

`Enter` on the highlighted row. The modal **stays open** (there is only one row here, but the
design holds for more) and the row's own text updates in place: **"notes.txt — recovered (the
file on disk has since changed)"** -- `RecoveryOfferOutcome::RecoveredAsConflict`'s own rendered
text, D5's second disk outcome, reached because step 5 above changed the file after the record was
written. The project card behind the modal already shows **"Unsaved edits"** and **"1 unsaved
file"** -- the recovered buffer is live in the open set the moment it is accepted, exactly as
`activating_a_row_whose_file_has_changed_surfaces_the_reload_control` (`shell::tests`) proves
without a screen to look at.

## `03-recovered-document-reload-control.png` -- the fixed defect, on screen

Switching into the project (`Tab` to the tab strip, `Right` to the project's own tab, `Enter`)
shows the editor itself: **`notes.txt (conflict)`**, and — the control review 490 found missing
entirely, the real data-loss bug this slice's own second round of review was about — **a real
`Reload` button**, third in the chrome alongside `Save`/`Save All`. The document body holds the
**recovered** text (`UNSAVED EDIT -- saved content, line one` / `saved content, line two`), not
the real file's own current content (`this file changed on disk after the crash` /
`nobody at the keyboard wrote this`) -- the recovered buffer, not a silent reload over it, exactly
as D4 requires.

## A confirming detail, not asked for but worth recording

The recovery record for `notes.txt` was **not simply gone** after this capture -- a fresh one
existed, timestamped to the external modification in step 5 (`modified_at_secs`/`len: 76`
matching the file as changed, not the file as it stood at the original crash). This is correct,
not a leak: D11 removes a record the moment *its own reason* ends (saved, closed, or recovered) --
the *original* pre-crash record was removed the instant `recover_highlighted_offer_item` ran
(confirmed by its own absence under the original hash), but the recovered buffer is **still
dirty** (a `Conflict`-state document counts as dirty), so the ordinary persist tick protects it
again, exactly as it would protect any other unsaved document, including this newly-recovered one,
against a second crash.

## Same focus-zone note PR-065-D's own evidence already recorded

Reaching a typed edit from the Project Board needed the identical sequence that handoff's own
README already documented: `Tab`/`Tab` (or `Tab` once from a project workspace) to reach `TabStrip`,
`Right` to move off `[Projects]`/`Projects` onto the project's own tab, `Enter` to switch, then
`Tab`/`Tab` again to reach `MainArea` before typing is accepted anywhere. Confirmed, not
rediscovered as a surprise -- `FocusZone` (`crates/tekstide/src/input.rs`) has not changed shape
since PR-065-D's own capture.
