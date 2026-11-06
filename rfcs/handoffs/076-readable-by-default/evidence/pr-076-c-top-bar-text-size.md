# PR-076-C capture: the top bar's "Text size" / "Reset" control, first screen, both ends

Captured 2026-11-04, against the real top-bar buttons through a real `wtype` key event, not the
message dispatched directly.

**Command, in full, no arguments omitted:**

```sh
cargo build --release -p tekstide
mkdir -p /tmp/claude-1000/rfc076c/{state,config,project,captures}
XDG_STATE_HOME=/tmp/claude-1000/rfc076c/state \
XDG_CONFIG_HOME=/tmp/claude-1000/rfc076c/config \
  ./target/release/tekstide /tmp/claude-1000/rfc076c/project &
```

Every directory the app reads was isolated and throwaway: state, config (so no hand-set
`config.toml` font size could change what "100%" renders as), and the project path, never used by
any prior run.

**One instance, one window, verified before every keystroke.** `pgrep tekstide` found exactly one
process (pid `2392528`) after launch; `niri msg -j windows` found it at window `id 35`,
`is_floating: false`; `niri msg -j focused-window` confirmed that same pid held focus immediately
before the `wtype` call below. `wtype -M ctrl -M alt -k 1 -m alt -m ctrl` for `Ctrl+Alt+1` — a real
keyboard event, not the handler called directly. No mouse input, no window resize or float — niri's
own default tiling gave the window **1042×1134** in both captures, confirmed identical via
`niri msg -j windows`' own `window_size`, so the window itself never resized. `niri msg action
screenshot-window --id 35 --path <absolute>` for each capture. Process killed immediately after the
second capture (`kill 2392528`); `pgrep tekstide` confirmed empty afterward.

## `pr-076-c-top-bar-100.png` — the first screen, default `100%`

The Project Board, no chord or button pressed yet. "Text size: 100%" and "Reset" sit in the top
bar, immediately after the "Tekstide" title and before "Trust Settings", "Audit" and "?" — the
first two action buttons on the first screen a user sees, mouse-clickable, labelled in words. This
is the control PR-076-C exists to add: visible with no chord, reachable before any project is even
open.

## `pr-076-c-top-bar-smallest-step-90.png` — after a real `Ctrl+Alt+1`, the smallest step

**The claim PR-076-C's own acceptance criteria asked to be verified rather than trusted**: the
label stays legible at the step where legibility is hardest to keep, not merely at the default.
"Text size: 90%" and "Reset" both render in full, un-clipped, exactly where they were before —
compare against the first capture. The window is the same 1042×1134 in both; what changed is
purely how much of it the content fills, confirmed directly:
`/tmp/claude-1000/rfc076c/state/tekstide/interface-scale.json` reads `{"state_version": 1, "scale":
0.9}` immediately after the press, matching the `90%` the button itself now shows and the step
`Ctrl+Alt+1` names.
