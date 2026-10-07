# RFC-026 PR-026-C — `REQ-FILE-003` captured live

Captured 2026-10-07 from the running app (`target/debug/tekstide`) on a throwaway fixture
(`mktemp -d` under `/dev/shm`, not `$HOME`). Window captured with `niri msg action screenshot-window
--id 49`, read back with `wl-paste --type image/png`. Only the fixture's own file names appear.

| File | Shows | Action between captures |
| --- | --- | --- |
| `01-src-expanded.png` | `src` expanded, holding `main.rs` | Expanded with Enter, keystrokes checked against the focused window before each |
| `02-created-externally.png` | `appeared.txt` listed under `src`, beside `main.rs` | `touch src/appeared.txt` from outside the app, 3 s before the capture |
| `03-deleted-externally.png` | `src` back to `main.rs` alone, the same picture as `01` | `rm src/appeared.txt`, 3 s before the capture |
| `04-renamed-externally.png` | `renamed.rs` listed, `main.rs` gone | `mv src/main.rs src/renamed.rs`, 3 s before the capture |

**What these prove.** In the running app, a file created, deleted and renamed in an expanded folder
appears and disappears in the explorer with no reopen and no keystroke. The path is the one the
step-3 feed drives: the platform's notice, the batcher, a drained window, and the explorer's own scan.

**What they do not prove.**

- Latency. Each capture waited 3 s after the change; the time from change to screen was not measured.
- Bursts. One file at a time. The burst case is `REQ-FILE-004`'s measurement, not this one.
- The app was opened from the command line. The boot-time watch trigger (review 459's gap, found while
  preparing this capture) places the root's watch for such a project at open. Expanding `src` places
  its own watch, so these four captures do not depend on that fix; a change in the root folder would
  have, before it (see `qa-evidence.md`).
- Open documents. The explorer is updated; an open file's content is not, and that is slice C's
  next item (D8).
