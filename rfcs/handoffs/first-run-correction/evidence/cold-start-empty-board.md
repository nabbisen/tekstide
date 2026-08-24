# Cold-start capture: `cold-start-empty-board.png`

Captured 2026-08-24, by the dev team, per response 293's instruction (request 292 found the
prior "first cold-start evidence" claim referenced an artifact never committed to this
repository).

**Command, in full, no arguments omitted:**

```sh
FRESH_STATE=$(mktemp -d)          # /tmp/tmp.TsTs9Ntq6z on the capturing machine
XDG_STATE_HOME="$FRESH_STATE" ./target/debug/tekstide &
```

No CLI path argument. `XDG_STATE_HOME` was a directory created fresh by `mktemp -d` immediately
before this launch, never used by any prior run, so `restore_recent_projects` had nothing to
restore. This is the detail `pr-015-b/shell-chrome-over-real-state.png` (2026-07-31) omitted,
which left its own status ambiguous seven weeks later — recorded explicitly here so this capture
does not repeat that.

Window captured via `niri msg action screenshot-window --id <id>` (response 127's standing
convention); this niri configuration has `screenshot-path null` (clipboard only, no
screenshot-path configured on disk — also true during the 292 audit), so the clipboard was
saved with `wl-paste -t image/png > cold-start-empty-board.png` immediately after.

**What it shows:** `No projects yet` / `To open a project, start Tekstide with its path:` /
`tekstide /path/to/project`, all nine live keybindings under `Keyboard` with their
preconditions, and the status bar reading `Project Board | 0 projects    Ctrl+Alt+P Project
Board`. No text names an action that does not exist. This is the shipped `0.12.1` empty state,
not the pre-Project-Board scaffolding `pr-015-b`'s screenshot showed.

Process was killed immediately after capture; nothing from this run was left running.
