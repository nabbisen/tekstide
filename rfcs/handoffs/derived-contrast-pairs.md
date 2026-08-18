---
title: "Derived contrast pairs — make a new theme role impossible to leave unmeasured"
status: "Scheduled 2026-08-18, awaiting implementation"
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

## The specific gap this should catch in our own theme

Our current list asserts against two backdrops: `background` and `surface_elevated`. **The
modal dialog's real backdrop is neither.** `modal_dialog_box` draws over the scrim, which
is itself composited over whatever was behind it — a third effective surface that appears
nowhere in the pair list.

We are safe there today, and safe for the wrong reason: the scrim *darkens*, and our accent
border is light, so the real ratio is higher than the `accent on background` figure we do
assert. **We pass by luck rather than by declaration**, and a future palette with a darker
accent or a lighter scrim would break it silently.

The declaration must name this case explicitly — the modal border's backdrop is
`composite_over(scrim, background)` — and `composite_over` already exists in
`theme/contrast.rs` for exactly this.

If the exercise turns up other pairs we render but never assert, that is the mechanism
working. Report them; do not fix colours in this slice unless one actually fails.

## The gate

- **Adding a field to `Theme` must fail to compile** until its usage is declared. Prove it:
  add a throwaway field, observe `E0027` naming it, remove it. That is the ablation, and it
  is the whole point of the slice.
- The declaration is **intended usage**, and the evidence says why the cross-product was
  rejected.
- The modal-over-scrim backdrop is declared and asserted via `composite_over`.
- **Report the before/after pair count.** If it does not go up, either our old list really
  was complete or the declaration is too narrow — say which.
- If a newly-derived pair fails, **stop and report** rather than adjusting a colour. A
  second palette change belongs in its own slice with its own red-then-green evidence, the
  way `border_default` got one.

## Not in scope

- Any dependency on `snora`. Evaluated and declined twice; see `future-work.md`.
- New colour roles, light/high-contrast presets, typography. `Theme` gains alternate
  palettes at RFC-023, not here.
- Promoting `theme/contrast.rs` out of `#[cfg(test)]`. That is RFC-023's decision and is
  already recorded in its handoff pack.
