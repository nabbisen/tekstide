---
title: "Theme contrast verification — a real gate, and the border defect it catches"
status: "Scheduled 2026-08-17, awaiting implementation"
rfc_file: "none — a defect fix plus the test that should have caught it"
target_milestone: "M11"
created: "2026-08-17"
---

# Theme contrast verification

## The defect

`Theme::default`'s `border_default` fails WCAG 2.1 SC 1.4.11 (Non-text Contrast, AA),
which requires **3:1** for the visual boundaries of UI components. Measured 2026-08-17
from `crates/tekstide/src/theme.rs`:

| ratio | pair | verdict |
| --- | --- | --- |
| 14.62:1 | `foreground` on `background` | passes AAA comfortably |
| 13.21:1 | `foreground` on `surface_elevated` | passes AAA comfortably |
| 6.38:1 | `accent` on `background` | passes |
| 5.77:1 | `accent` on `surface_elevated` | passes |
| **2.63:1** | **`border_default` on `background`** | **fails 3:1** |
| **2.37:1** | **`border_default` on `surface_elevated`** | **fails 3:1** |

**Text contrast is excellent and is not the problem.** What fails is the *resting* pane
boundary: `zone_style` draws unfocused zones with `border_default` at width 1.0, and for a
low-vision user those boundaries are hard to make out.

**Focus indication is unaffected**, and that distinction matters — do not let a fix
description blur it. Focused zones use `border_focused` (the accent, 6.38:1) at width 2.0,
so the focus indicator both clears 3:1 and carries a second non-colour channel, which is
what `NFR-UX-002` requires. This is a defect in the unfocused state only.

## Why our own tests could not catch it

`crates/tekstide/src/theme/tests.rs` asserts:

- channels are "in range" (`0.0..=1.0`)
- `border_focused` differs from `border_default`
- the scrim is translucent
- font sizes are positive and heading is the largest

**Every one of those is a type check wearing the costume of a quality check.** No colour a
human would plausibly type can fail "in range." The suite cannot distinguish a readable
palette from an unreadable one, and it passed this defect from RFC-015 through `0.10.0`.

Found while evaluating `snora` at the owner's request: its `snora-design` crate tests every
built-in palette against real thresholds, which is what prompted measuring ours. Its own
suite has the same gap one level in — its `border` role is in none of its mandatory pairs —
reported to that team separately.

## What to build

### Slice A — the gate, failing

A contrast module for the `tekstide` crate. Three pure functions, no dependency (this is
~80 lines; `snora-design` was evaluated and declined as a dependency — see
`future-work.md`):

- `relative_luminance(Color) -> f32` — WCAG 2.1 sRGB formula, with the `c <= 0.03928`
  linear segment. Get this boundary right; it is the usual transcription error.
- `contrast_ratio(Color, Color) -> f32` — `(lighter + 0.05) / (darker + 0.05)`, symmetric.
- `composite_over(fg: Color, bg: Color) -> Color` — **required, not optional.** A
  translucent colour has no contrast ratio of its own; it must be composited over its
  backdrop first. Our `scrim` is `rgba(0, 0, 0, 0.55)`, so any assertion about it that
  skips this step is measuring a number that does not appear on screen.

Then a test over `Theme::default` asserting **real thresholds**:

- text pairs (`foreground` over `background` and `surface_elevated`) ≥ **4.5:1**
- non-text pairs (`border_default`, `border_focused`, `accent` over both surfaces)
  ≥ **3:1**
- the scrim composited over `background` and over `foreground`, asserting it genuinely
  darkens — a real assertion about the dimming layer rather than the current
  "alpha < 1.0" check

**This slice must land red.** Commit it only after confirming `border_default`'s two
assertions fail with the current palette and the failure message names the measured ratio.
Sanity-check the arithmetic against known anchors first — black on white is exactly 21:1,
identical colours are 1:1, and the ratio is symmetric — because a contrast function with a
transcription error will happily pass everything.

### Slice B — the fix

Raise `border_default` until both assertions pass.

**Candidate: `0.45` grey** (`Color::from_rgb(0.45, 0.45, 0.45)`), which measures 3.85:1 on
`background` and 3.48:1 on `surface_elevated`. Verify rather than trust those numbers.

`0.42` is roughly the minimum that clears both (3.44:1 / 3.11:1) and is deliberately not
the recommendation — a value sitting one hundredth above a threshold re-breaks the moment
someone adjusts `surface_elevated`, and the test would then fail in a slice that has
nothing to do with borders. Take the headroom.

Confirm the test flips red → green on this change alone, and that no other assertion
moves.

## Non-goals

- **Typography and line-height. Explicitly deferred by the owner** — do not fold it in,
  even though the evaluation that found this defect also recommended it.
- **Light or high-contrast presets.** Not scheduled. `Theme` is RFC-023's configuration
  seam and gains alternate palettes there, not here.
- **Any dependency on `snora`.** Evaluated and declined; the reasoning is recorded in
  `future-work.md` and should not be re-litigated in this slice.
- **Other colour changes.** Only `border_default` fails. Do not tune the palette while
  you are in here.

## The gate

- The contrast function is validated against known anchors (21:1, 1:1, symmetry) before
  any claim is made about our palette.
- `composite_over` is used for the scrim assertion, and the evidence says why a
  translucent colour cannot be measured directly.
- The threshold test is **observed failing** on the current palette, with the measured
  ratios in the failure output, before the fix.
- The fix is the minimum change that clears the thresholds with headroom, and nothing
  else in the palette moves.
- Evidence states plainly what this does **not** establish: that the application is
  accessible. It fixes one measured WCAG failure and adds a gate. There is still **no
  screen-reader support**, and that wording in `README.md` does not change.
- The now-superseded "in range" assertions in `theme/tests.rs` are either removed or
  kept with a comment saying what they do and do not check — do not leave them looking
  like contrast coverage.
