# PR-027-C evidence

Two capture sessions. `03` is from the first; `01`/`02` are from a second, retaken session --
review 493 found the first session's own `01`/`02` disclosed the owner's real `recent-projects.json`
(twenty entries, including other sessions' own `/tmp/.../-home-nabbisen-...` scratchpad paths,
which encode the real `$HOME` layout even though the paths themselves read `/tmp/...`). `03` never
showed the Project Board at all, so it was not disclosing and was kept as is.

## First session (produced `03` only)

A throwaway project under `mktemp -d /tmp/tekstide-live-capture-027c.XXXXXX`, no isolated
`XDG_STATE_HOME` (the real one). The real `recent-projects.json`, the recovery instance marker and
record were all removed from the real state directory by hand afterward -- confirmed clean by
review 493's own check before it found the disclosure in `01`/`02`.

## Second session (produced `01` and `02`, retaken)

**Corrected per review 493**: a throwaway project *and* a throwaway `XDG_STATE_HOME`, both under
`/dev/shm` -- `mktemp -d /dev/shm/tk027c-proj.XXXXXX` and `mktemp -d /dev/shm/tk027c-state.XXXXXX`,
`XDG_STATE_HOME` passed directly to the `tekstide` invocation. The same isolation PR-065-C/PR-065-D
already used, and should have been used here from the start. The Project Board shows exactly one
project in this session's own screenshots, `"1 project"` in the status bar -- nothing from any
other session, real or otherwise, anywhere in the frame.

Both sessions: captured against `target/debug/tekstide`, window-managed with `niri`, keys sent with
`wtype`, each key preceded by an explicit `niri msg focused-window`/
`niri msg action focus-window --id <n>` check. Both fixtures and both state directories were
removed after their own session; the second session touched nothing under the real `$HOME` or the
real state directory at all.

## The scenario, step by step (both sessions, identical shape)

1. Launched `tekstide <project>` (a fresh project, added via the CLI-argument path).
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
6. Relaunched `tekstide` against the same project path (and, in the second session, the same
   `XDG_STATE_HOME`).

## `01-recovery-offer-pending.png` -- the offer, driven by the record alone

The offer opens immediately on restart: **"Recover unsaved work in tk027c-proj…?"**,
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
as D4 requires. (First session's own project name, `tekstide-live-capture-027c.Ty5pxQ`, appears in
this image's own tab/sidebar text -- the project name itself, not a path, and not under `$HOME`.)

## A confirming detail, not asked for but worth recording

In both sessions, the recovery record for `notes.txt` was **not simply gone** after the capture --
a fresh one existed, timestamped to the external modification in step 5 (`modified_at_secs`/
`len: 76` matching the file as changed, not the file as it stood at the original crash). This is
correct, not a leak: D11 removes a record the moment *its own reason* ends (saved, closed, or
recovered) -- the *original* pre-crash record was removed the instant
`recover_highlighted_offer_item` ran (confirmed by its own absence under the original hash), but
the recovered buffer is **still dirty** (a `Conflict`-state document counts as dirty), so the
ordinary persist tick protects it again, exactly as it would protect any other unsaved document,
including this newly-recovered one, against a second crash.

## Same focus-zone note PR-065-D's own evidence already recorded

Reaching a typed edit from the Project Board needed the identical sequence that handoff's own
README already documented: `Tab`/`Tab` (or `Tab` once from a project workspace) to reach `TabStrip`,
`Right` to move off `[Projects]`/`Projects` onto the project's own tab, `Enter` to switch, then
`Tab`/`Tab` again to reach `MainArea` before typing is accepted anywhere. Confirmed, not
rediscovered as a surprise -- `FocusZone` (`crates/tekstide/src/input.rs`) has not changed shape
since PR-065-D's own capture.

## The lesson this retake is itself evidence of

"No path in any image is under `$HOME`" (the first session's own claim) was literally true and
still wrong: the real `recent-projects.json`'s own entries, drawn from *other* sessions' scratchpad
directories, encode the real home layout inside a `/tmp/...` path, not as a `/home/...` prefix.
The check that matters is whether the real, shared `recent-projects.json` was ever in frame at
all -- not whether any single path happens to start with `/home`. A throwaway `XDG_STATE_HOME` is
what actually guarantees the answer is no, which is why the second session uses one and the first
did not.
