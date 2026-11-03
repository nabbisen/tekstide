# PR-076-A capture: `pr-076-a-raised-defaults.png`

Captured 2026-11-03, against the final, corrected defaults (`config/appearance.rs`'s
`FontSizes::default`: body 16.0, heading 20.0, status **16.0** — from 14.0/16.0/13.0). This
replaces an earlier capture against status 15.0, round 1's first pass before review found 15 did
not actually satisfy the RFC's own acceptance criterion (see `FontSizes::default`'s own doc
comment and D2's dated amendment in the accepted RFC for the full reasoning); that capture was
never committed, since nothing at status 15 ever shipped.

**Command, in full, no arguments omitted:**

```sh
cargo build --release -p tekstide
mkdir -p /tmp/claude-1000/rfc076-gui2/{state,config,project,captures}
XDG_STATE_HOME=/tmp/claude-1000/rfc076-gui2/state \
XDG_CONFIG_HOME=/tmp/claude-1000/rfc076-gui2/config \
  ./target/release/tekstide /tmp/claude-1000/rfc076-gui2/project &
```

Every directory the app reads was isolated and throwaway, created fresh for this capture: state,
config (so no real `config.toml` font override could leak into evidence meant to show the
*shipped defaults*), and the project path itself, never used by any prior run.

Window captured via `niri msg action screenshot-window --id 115 --path <absolute path>` (the
project's own standing convention; `niri msg -j windows` confirmed `id 115` was pid `187292`,
the process this run started, before capturing). No keystrokes, no mouse input, no resize or
float — niri's own default tiling gave the window **1250×1361** (confirmed via `file` on the
output, identical to round 1's own capture, so the comparison is apples to apples), stated here
rather than assumed to be 1280×720, since nothing in this app sets a window size explicitly.
Process killed immediately after capture (`kill 187292`); `pgrep tekstide` confirmed empty
afterward. Nothing from this run was left running.

**What it shows, against the two questions PR-076-A's own instruction asked to be confirmed in
the image, re-checked against the corrected numbers:**

1. **"The status bar is no longer the smallest text in the product."** Now true in the sense that
   matters, and visible rather than merely argued: status and body are both 16.0, so the status
   bar (`Project Board | 1 project    Restricted    Git: not available    Ctrl+Alt+P Project
   Board`) and the card's own body text read as **one size, not two** — compare this capture
   against the round-1 evidence this file replaces, where a 1px gap was already visually close to
   indistinguishable; at zero gap there is nothing left to distinguish. This is the corrected
   reading: round 1's own capture showed the *symptom* (the gap already read as small) that led
   to round 2's ruling (a 1px step conveys no perceptible hierarchy, so it was cost without
   signal) — it did not, by itself, make the literal RFC claim true, which is why it was reported
   as a finding rather than quietly accepted.
2. **"The heading reads as structure, not emphasis."** Confirmed, unchanged from round 1 and
   unaffected by the status ruling: the card's own `project` heading has a real, visible step
   over the body text beneath it (20.0 vs 16.0, a 4px gap — the old pair was 16.0 vs 14.0, only
   2px). The hierarchy reads at a glance.

Only one screen exists to capture at this point in the app (an empty Project Board immediately
opening the one throwaway project passed on the command line); no second, comparison capture was
taken this round either, for the same reason as round 1 — the two questions are answered by
reading this one image against the known numbers.
