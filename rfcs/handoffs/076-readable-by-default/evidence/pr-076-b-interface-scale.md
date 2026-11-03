# PR-076-B captures: the whole-UI scale factor, baseline and both ends

Captured 2026-11-04 (re-taken; see *Correction* below), against the real `Ctrl+Alt+1`/`Ctrl+Alt+5`
chords through a live keyboard event, not the mechanism called directly.

## Correction to this file's own first version

The first version of this file numbered its captures in an ascending-size narrative order
(baseline, then smallest, then largest) without disclosing that the **real chronological order of
the key presses was different** (`Ctrl+Alt+5` was pressed before `Ctrl+Alt+1`, not after). The
on-disk `interface-scale.json` read `0.9` at the end of that session — correctly reflecting the
real last press (`Ctrl+Alt+1`) — but the write-up's own numbered order implied the opposite, so it
read as a contradiction: "the last chord pressed was `Ctrl+Alt+1`" sat beside a capture list that
numbered `Ctrl+Alt+5` as capture 3, last. The review response caught this and correctly refused to
accept it as proof of anything; re-taken below, with the **press order matching the write-up's own
numbered order this time**, and the file checked **after every single press**, not only once at
the end.

**Command, in full, no arguments omitted:**

```sh
cargo build --release -p tekstide
mkdir -p /tmp/claude-1000/rfc076b-redo/{state,config,project,captures}
XDG_STATE_HOME=/tmp/claude-1000/rfc076b-redo/state \
XDG_CONFIG_HOME=/tmp/claude-1000/rfc076b-redo/config \
  ./target/release/tekstide /tmp/claude-1000/rfc076b-redo/project &
```

Every directory the app reads was isolated and throwaway: state, config (so no hand-set
`config.toml` font size could change what "1.0" renders as), and the project path, never used by
any prior run.

**One instance, one window, verified before every keystroke.** `niri msg -j windows` found `id
12`, pid `148494` matching the process this run started; `niri msg -j focused-window` confirmed
that same pid held focus immediately before each `wtype` call below. Keystrokes, not AT-SPI
actions, because the keyboard is exactly what this capture tests: `wtype -M ctrl -M alt -k 1 -m
alt -m ctrl` for `Ctrl+Alt+1`, the same shape for `Ctrl+Alt+5`. No mouse input, no window resize
or float — niri's own default tiling gave the window **1250×1361** throughout (confirmed via
`file` on each output; identical across all three captures, so the window itself never resized,
exactly D1's own required property). `niri msg action screenshot-window --id 12 --path
<absolute>` for each. Process killed immediately after the third capture (`kill 148494`); `pgrep
tekstide` confirmed empty afterward.

## The step-by-step correspondence, checked at every press, not only at the end

Before any chord: `interface-scale.json` does not exist (`ls` fails with "No such file or
directory") — the ordinary case, nothing written until a scale chord is actually pressed.

| Step | Action | `interface-scale.json`, read immediately after | Matches the press? |
| --- | --- | --- | --- |
| 1 | baseline, no chord | (file does not exist yet) | n/a |
| 2 | `Ctrl+Alt+1` pressed | `{"state_version": 1, "scale": 0.9}` | yes — `0.9` is step 1 |
| 3 | `Ctrl+Alt+5` pressed | `{"state_version": 1, "scale": 1.5}` | yes — `1.5` is step 5 |
| — | immediately before `kill` | `{"state_version": 1, "scale": 1.5}` | yes — unchanged since step 3 |

**This rules out a swallowed write failure directly, rather than arguing it could not have
happened.** The response's own reading 2 (a failed, silently-swallowed `Ctrl+Alt+5` save, leaving
a stale `0.9` on disk while the screen still rendered at `1.5`) predicts exactly one on-disk value
across both presses. Checking the file after *each* press instead of only at the end shows two
distinct values, each matching the press that produced it, with no gap for a swallowed failure to
hide in.

## `pr-076-b-scale-1-baseline.png` — `1.0`, no chord pressed yet

The shipped default, for comparison. Same empty Project Board PR-076-A's own evidence shows at
the raised font defaults (16/16/20) — this capture starts from that state, not from the pre-076
sizes.

## `pr-076-b-scale-2-smallest-step.png` — after `Ctrl+Alt+1` (`0.9`, the smallest step)

Visibly tighter than the baseline throughout — the top bar, the project card, every badge — with
no overlap, no clipped text, nothing unreadable. The smallest step is still comfortably usable,
not a degenerate edge case.

## `pr-076-b-scale-3-largest-step.png` — after `Ctrl+Alt+5` (`1.5`, the largest step)

**The claim PR-076-B's own instruction asked to be verified rather than trusted**: at the largest
step, the Project Board still fits, with real headroom below the card, and nothing clips —
compare against the baseline and the smallest-step capture; the window is the same 1250×1361
throughout, so what changed is purely how much of it the content now fills. Every top-bar button
(`Trust Settings`, `Audit`, `?`), the tab strip, and the whole card (path, badges, the "blocked:"
line, "Open now") render in full. This confirms RFC-076 D1's own `0.9`–`1.5` range does not reach
a point where the Project Board stops fitting, at least for the one screen this product shows
before a project is open.
