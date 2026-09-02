---
title: "Derived contrast pairs — make a new theme role impossible to leave unmeasured"
status: "Complete 2026-08-18 — accepted (requests 260/261). Pairs derived from an exhaustive `Theme` destructure; the scrim backdrop found failing at 2.40:1 and fixed by raising the scrim alpha to 0.75"
rfc_file: "none — hardening the gate added by theme-contrast-verification"
target_milestone: "M11"
created: "2026-08-18"
---

# Derived contrast pairs

## Why

`theme-contrast-verification` (`55f53d8`) added a real WCAG gate and fixed a real defect.
Its pair list is **hand-written**:

```rust
let text_pairs = [ /* 2 literal entries */ ];
let non_text_pairs = [ /* 6 literal entries */ ];
```

That list is complete today — we have exactly two backdrops and every foreground role is
checked against both. **It is complete by coincidence of our being small, and it cannot
grow on its own.** Add a role or a surface to `Theme` and the list silently stays the same
size. Nothing about adding a field forces anyone to measure it.

This is not hypothetical. It is the exact defect we reported to the snora team, whose
`text_muted` role went unmeasured against one of its three surfaces for its entire life —
and which our own report then repeated, by measuring `border` against three surfaces and
`text_muted` against one before declaring it safe. Recorded in `future-work.md` under the
snora evaluation.

## The precedent, which is worth copying

snora shipped the fix in their 0.36.0 (their RFC-063) after we raised it. Their mechanism:
a function that **destructures the palette struct exhaustively** and declares, per role,
which surfaces it renders on and at what threshold. Pairs are derived from that
declaration. Adding a field fails to compile:

```text
error[E0027]: pattern does not mention field `probe_role`
```

Their coverage went 19 → 35 pairs, 76 → 140 assertions, and the review of the change
immediately found two roles declared against nothing — both filled, borderless buttons
where the *fill* is the identifying boundary. Nothing needed repairing, but neither was
protected.

**Copy the mechanism, not the numbers.** Our palette is much smaller and most of their
roles have no analogue here.

## What to build

An exhaustive destructure of `Theme` inside the test module, and a per-field declaration
of intended usage that the pair list is derived from.

```rust
// illustrative shape only — not a spec
let Theme { background, foreground, accent, border_default, border_focused,
            surface_elevated, scrim,
            font_size_body, font_size_heading, font_size_status } = theme;
```

Every field must be named. The non-colour fields are named and declared as carrying no
contrast obligation — that is a decision recorded, not a field skipped.

**Declare intended usage, not the cross-product.** This is the guardrail we gave snora and
they kept it: a pair only belongs in the list if the role is actually rendered on that
surface. A cross-product would be mostly noise, and noise in an accessibility gate is how
gates come to be ignored.

## The specific gap this should catch — and it is a real failure, measured

Our current list asserts against two backdrops: `background` and `surface_elevated`. **The
modal dialog's real backdrop is neither.** `modal_dialog_box` draws over the scrim
(`rgba(0,0,0,0.55)`), which is composited over whatever was behind it — including **terminal
content, which is arbitrary and attacker-influenceable**.

I previously told the snora team we passed this case "by luck." **That was wrong, and it was
reasoning rather than measurement.** Measured 2026-08-18:

| modal backdrop | border (`accent`) | fill (`surface_elevated`) |
| --- | --- | --- |
| scrim over `background` | 6.92:1 | 1.20:1 |
| scrim over `surface_elevated` | 6.73:1 | 1.17:1 |
| scrim over white terminal content | 1.66:1 | 3.48:1 |
| **scrim over ~0.78 grey terminal content** | **2.40:1** | **2.40:1** |

Over dark chrome the border identifies the card and the fill contributes nothing. Over bright
content the fill identifies it and the border contributes nothing. **At the crossing point —
terminal content around 0.78 grey — neither reaches 3:1.** The modal card has no boundary
meeting WCAG 2.1 SC 1.4.11 against that backdrop, and a terminal can be made to display it.

### The methodological point, which matters more than the number

**Sampling the endpoints hides this.** Testing "scrim over `background`" and "scrim over
white" both pass. The failure lives strictly between them, where the two curves cross. A
pair list that checks plausible-looking backdrops would have found nothing.

So this pair cannot be asserted as a fixed pair. It must be asserted as a **sweep** over the
backdrop range, taking the worst case of `max(border, fill)` — the best channel available to
identify the card — and requiring that worst case to clear 3:1. Anything less is sampling.

**Sweep greyscale only, and here is why it is sufficient** (told to us by the snora team,
verified here two ways before being written down). The scrim composites channelwise, and
relative luminance is monotonic increasing in each channel, so a composited backdrop's
luminance is bounded by its values at black and white content — and greyscale content spans
that interval continuously. Contrast ratio depends on nothing but luminance, so every ratio
a coloured backdrop can produce, a grey one already produces. A 3D colour sweep adds roughly
a million points and no coverage. Confirmed empirically: 400,000 random RGB backdrops found
a worst case of 2.4011, against the greyscale minimum of 2.401129.

**But do not sample the sweep either.** A 2,000-step greyscale grid reports 2.4016 where the
true minimum is 2.401129 — harmless at this magnitude, and not harmless in general: a coarse
grid straddling a true minimum of 2.9995 will happily report 3.0002 and pass a failing
palette. `max` of two monotone curves is **unimodal**, so the minimum can be found exactly by
ternary search rather than approached by grid. Do that, and report the content value it
occurs at, not only the ratio. (Found by checking snora's greyscale claim rather than
accepting it — the first grid I ran disagreed with the 3D sweep and looked like a
counterexample, and was resolution.)

The snora team hit the identical trap from the other side and put it well: *"`light`'s
background is pure white, so the dim over it is the lightest the dim can be — 2.85 was a
floor, not a sample."* Ours is not a floor at either end; it is a minimum in the middle.

### What does not break

**RFC-018's spoofing evidence is unaffected**, and do not let a fix description blur this.
That property rests on keystroke suppression under a positive control, and on the scrim
existing at all as a content-independent tell — not on the card's border contrast. This is
an accessibility defect with an attacker-influenceable trigger, not a hole in the trusted-UI
argument.

## Slice B — the fix

Two independent levers, both measured:

| lever | worst case |
| --- | --- |
| scrim alpha `0.55` (today) | 2.40:1 |
| scrim alpha `0.65` | 2.43:1 — **not enough**, the crossing barely moves |
| **scrim alpha `0.75`** | **3.62:1** |
| border grey `0.75` in place of `accent` | 3.01:1 |
| border grey `0.85` in place of `accent` | 3.42:1 |

**Recommended: raise the scrim alpha to `0.75`.** It keeps the accent-coloured border, which
is a deliberate design signal, and it moves in the same direction as RFC-018's own goal —
more chrome dimming is a stronger spoofing tell, not a weaker one.

**But check RFC-018's upper constraint before committing to a number.** That RFC argues the
scrim must stay translucent, because *"an opaque scrim would look identical to any solid
full-window rectangle a spoofing attempt could also draw."* `0.75` is still visibly
translucent; verify that claim against the real rendered window rather than trusting the
arithmetic, and say what you observed. If translucency at `0.75` is not defensible, the
border lever is there.

This is an **appearance change** — the window goes noticeably darker behind a modal — and it
must ship as a stated one, not a silent fix.

## The gate

- **Adding a field to `Theme` must fail to compile** until its usage is declared. Prove it:
  add a throwaway field, observe `E0027` naming it, remove it. That is the ablation, and it
  is the whole point of the slice.
- The declaration is **intended usage**, and the evidence says why the cross-product was
  rejected.
- The modal-over-scrim backdrop is declared and asserted via `composite_over`.
- **Report the before/after pair count.** If it does not go up, either our old list really
  was complete or the declaration is too narrow — say which.
- **The scrim backdrop is asserted as a sweep, not as sampled pairs**, and the test is
  observed failing at ~2.40:1 before Slice B changes anything. Reproduce the crossing point
  independently rather than trusting the table above.
- **Slice B is an appearance change**, verified against the real rendered window for
  translucency, and reported as an appearance change rather than a fix.
- If a *further* derived pair fails — one not named here — **stop and report** rather than
  adjusting another colour. A third palette change belongs in its own slice with its own
  red-then-green evidence.

## One boundary that does not bite us, relayed via snora from the orbok team

`#[non_exhaustive]` permits exhaustive destructuring **only inside the crate that defines
the type**; from outside, the compiler requires `..`, which is exactly what defeats this
mechanism (`E0638`). orbok hit it trying to apply RFC-063's pattern to snora's `Palette`
from outside snora.

**It does not apply here, checked rather than assumed**: `Theme` is defined in
`crates/tekstide/src/theme.rs`, is not `#[non_exhaustive]`, and `theme/tests.rs` is a module
of the same crate. The destructure will work as described. Recorded so that if `E0638` ever
does appear, it reads as a boundary rather than a mistake.

## The sibling criterion this gate does not cover: SC 1.4.1, Use of Colour

Everything above is **SC 1.4.3 (Contrast Minimum)** — a ratio between two colours. Its sibling,
**SC 1.4.1 (Use of Colour)**, is a different question: *is colour the only channel carrying a piece
of information?* A pair can pass 1.4.3 at 7:1 and still fail 1.4.1, because a perfectly legible
amber tells a person who cannot distinguish it from green nothing at all.

**This project already forbids it, and has since RFC-015.** `NFR-UX-002` — "status must never rely
on colour alone" — is standing constraint 4 of the delivery plan, binding on every surface, with
`[focused]`'s text prefix alongside a border as the reference pattern. It is cited in RFC-014, -015,
-017, -018 and -019. Recorded here only because a reader arriving at *this* document is thinking
about colour measurement, and 1.4.3 is the criterion this gate enforces while 1.4.1 is the one it
does not.

**What the derived-pair gate does and does not do for it.** The gate proves every pair is
*legible*. It cannot prove any pair is *not the only channel* — that is a claim about what else is
rendered, which a colour-pair enumeration cannot see. `NFR-UX-002` is instead held by per-surface
tests where a surface encodes status: `session_bar.rs` asserts every slot and every status resolves
to its own distinct text, and the explorer asserts symlink and access states render on distinct
lines. Those are real gates, but they are local — a *new* surface inherits the rule by review, not
by a test that fails.

**The structural reason the exposure is currently small**, worth keeping deliberately: `Theme`
exposes no semantic colour roles at all — no `warning()`, `danger()`, `success()`, no per-intent
family. Only `background`, `foreground`, `accent`, `border_default`, `border_focused`,
`surface_elevated`, `scrim`. With no intent colour to vary, a surface has nothing to encode status
in *except* text. The day someone adds `Theme::warning()` — a plausible RFC-023 alternate-palette
move — that protection is gone, and neither the derived-pair gate nor the local surface tests will
notice, because the new pair will measure fine and the existing surfaces will not have changed.
**A semantic colour role must arrive with a non-colour channel in the same change.**

Prompted by the snora team's 0.41.1 letter withdrawing their own 1.4.1 conformance claim: their
`toast_style` and `design::notice` varied only background and accent by tone — identical text, no
icon, no prefix. We do not depend on snora and were unaffected; no reply was sent. The finding was
worth more than the compatibility question, which is why it is recorded here.

## Not in scope

- Any dependency on `snora`. Evaluated and declined twice; see `future-work.md`.
- New colour roles, light/high-contrast presets, typography. `Theme` gains alternate
  palettes at RFC-023, not here.
- Promoting `theme/contrast.rs` out of `#[cfg(test)]`. That is RFC-023's decision and is
  already recorded in its handoff pack.
