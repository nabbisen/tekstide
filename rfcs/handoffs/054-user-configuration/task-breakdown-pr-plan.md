---
title: "RFC-054 — task breakdown and PR plan"
rfc: "RFC-054"
rfc_file: "../../done/054-user-configuration-completion.md"
source_rfc_status: "Implemented and closed 2026-09-24 — M12 (closed it)"
target_milestone: "M12"
created: "2026-09-24"
---

# Task breakdown and PR plan

**A is the keybinding pass and the repair it owes. B is how it looks. C is the terminal and the
live proof.**

## PR-054-A — keybindings, and the status that means two things

- **A `[keybindings]` section**, one entry per action, spelled **exactly as the Help modal and
  `--help` print it** (`Ctrl+Alt+P`). A round-trip test over every advertised binding, so the file
  and the help can never disagree.
- **Rebindable = has a default binding and is not `Reserved`.** A collision with another rule, or a
  `Reserved` chord, **refuses**: diagnostic on the board, default stands.
- **The repair (D3′).** `Configurable` with a `None` binding has meant *dead* since RFC-022 and reads
  as *bindable*. When this slice ends, every action has **a real default binding or an explicitly
  dead status**, distinguishable **by the type**. `CycleVisibleTerminalSession` and
  `OpenSafeCloseDialog` are the two that must be decided — a chord, or a death certificate with its
  reachability stated.

**Required tests:** every advertised chord round-trips through the parser; a colliding rebind refuses
and leaves the default; a `Reserved` chord refuses; **an action cannot be both bound and dead** — by
the type, not by review. **Ablation:** accept last-wins on a collision; the collision test fails.

## PR-054-B — theme and fonts

- `[theme]` colours over `Theme`'s existing seam (`background`, `foreground`, `accent`,
  `border_default`, `border_focused`, `surface_elevated`, `scrim`), and `[font]` family plus the
  three sizes.
- **Contrast validated at 4.5:1**, computed from relative luminance. Below it the pair falls back and
  the diagnostic carries the **measured ratio**.
- **The family is a name**, resolved by the system font database. Unavailable → fallback + diagnostic.
  Sizes outside **8–32 px** → fallback + diagnostic.

**Required tests:** a low-contrast pair falls back and the diagnostic names the ratio; an unknown
family falls back; each size bound refuses at the edge; **no diagnostic echoes a configured string
unescaped**. **Ablation:** drop the contrast check; that test fails alone.

## PR-054-C — scrollback, and the live proof

- `[terminal] scrollback_lines`, replacing the `SCROLLBACK_LINES = 2_000` constant, **capped by
  measurement**: measure bytes per line at a realistic width, set the cap so **one pane at the cap
  stays under 64 MB**, and put the number *and the measurement* in the book. Above the cap clamps
  with a diagnostic.
- **`Ctrl+Alt+C` applies all five live**, and `Reserved` stays reserved across a reload.

**Evidence:** one live capture sequence against a `mktemp -d` config — a window in default dress, the
file edited, `Ctrl+Alt+C`, **the same window in the user's colours, font and size with a rebound
chord working** — and a second showing a bad value falling back with the board naming it.

## After this

**M12 closes.** `REQ-CONFIG-006`, `REQ-CONFIG-007`, `NFR-UX-004` and `REQ-TERM-004` move to
implemented **with evidence they are reachable**, which is the distinction the 2026-09-23 audit
caught the plan getting wrong.
