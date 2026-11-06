---
title: "RFC-076 — task breakdown and PR plan"
rfc: "RFC-076"
rfc_file: "../../accepted/076-readable-by-default.md"
source_rfc_status: "Accepted by the human owner 2026-11-03 — RFC-076 is in rfcs/accepted/"
target_milestone: "pre-1.0"
created: "2026-11-03"
---

# Task breakdown and PR plan

**Two slices, B before A.** The RFC names D1 (the scale factor) and D2 (raised defaults) as the
first slice; the order to *land* them in is the reverse, and the reason is below. D3–D5 are not in
this pass.

## PR-076-A — D2 first: raise the defaults

**Deliverable**: `config/appearance.rs`'s `FontSizes::default` becomes body **16.0**, status
**15.0**, heading **20.0**, from 14.0 / 13.0 / 16.0 (`appearance.rs:270-272`). The bounds
(`MIN_FONT_SIZE_PX = 8.0`, `MAX_FONT_SIZE_PX = 32.0`) do **not** change.

**Why this lands before the scale factor**, and it is not arbitrary: it is three numbers with no
new mechanism, so it is the change that tells us whether the *layout* survives larger type at all —
before a scale factor adds a second variable on top. If something clips or wraps badly at 16 px, we
find it with three characters changed rather than inside a new feature.

**What it will touch beyond the three numbers:**

- **`docs/src/users/configuration.md`** states the font keys and their bounds, and
  `rfc_docs_invariants::the_configuration_page_names_every_theme_role_font_key_and_bound` asserts
  it. If that test fails, the page is the thing to fix.
- **Any test that asserts a rendered row count, a pixel extent or a frame cost.** Row count is a
  function of type size. `surface::explorer::tests::drawing_the_largest_tree_builds_only_the_window_
  and_fits_inside_a_frame` measures a 26-row window — re-derive that number, do not nudge it until
  it passes. If a test's expected value changes, say in the write-up what it changed from and to.
- **A rendered capture.** This is a visual change; the evidence is a screenshot read by a human,
  under every rule in `project-instructions-gui-common.md` (throwaway `$HOME`/state, size fixed at
  launch, focus verified before capture, no floating). Confirm the status bar is no longer the
  smallest text and the heading reads as structure.

## PR-076-B — D1: one scale factor

**Amended at implementation time, dated rather than silently corrected: `Ctrl+=`/`Ctrl+-` cannot
be expressed.** `chord.rs`'s own grammar refuses any key that is not `is_ascii_alphanumeric()` --
`Ctrl+0` parses, `Ctrl+=`/`Ctrl+-` do not, and admitting them would reintroduce the exact failure
the grammar already refuses `Shift`+digit for (both sit elsewhere, or behind `Shift`, on AZERTY
and QWERTZ). See RFC-076's own D1 for the full account; this plan's own "three new navigation
actions" below is also superseded by that amendment -- six, not three.

**Deliverable**: a user-facing scale bound to `Ctrl+Alt+1` through `Ctrl+Alt+5` (five absolute
steps, smallest to largest) and `Ctrl+Alt+0` (reset to exactly `1.0`), applied through
`iced::application::Application::scale_factor` (present in the pinned `iced 0.14.0` —
`application.rs:410` is the builder, `:518` the trait method).

**Whole UI, not text only.** The point is that a reader who needs larger type usually also needs
larger hit targets; `scale_factor` scales layout with the type, text-only scaling does not. Do not
implement this by multiplying the three font sizes.

**Required properties:**

1. **The window is not resized.** Size is fixed at launch, per this project's own GUI rules. A
   scale change alters what fits inside the window, never the window.
2. **A bounded, enumerated set of steps**, not free zoom — and state the set. An unbounded
   multiplier interacts with `MAX_FONT_SIZE_PX` in ways nobody has reasoned about.
3. **`Ctrl+Alt+0` returns to 1.0**, exactly, not to "approximately the default".
4. **Six new navigation actions** (five steps plus reset, not three), named and rebindable like
   every other, which means `configuration.md`'s rebindable-action list and
   `rfc_docs_invariants::the_configuration_page_lists_every_rebindable_action_with_its_default_chord`
   both apply. That test is why the chords are part of this slice and not an afterthought.
5. **Persistence per D5: state, never `config.toml`.** The app must not rewrite a file the user
   hand-edited — "touches nothing else" is the installation promise. The new path goes in
   `local-data-and-privacy.md`, whose own contract is that it lists *every* file written.
6. A `config.toml` font size set by hand remains the starting value; the control adjusts from there.

## Order, and what finished means

A → B, each its own commit, each with the full gate green and a rendered capture for the visual
half. **A stage is finished when a human has looked at the screen**, not when the tests pass —
this is the one RFC in flight whose whole subject is what a person can read.

## Not in this pass

D3 (default theme toward AAA — the floor stays AA and is not touched), D4 (visible controls for
first-contact actions, the largest of the five), D5's own doc additions beyond the one path B needs.
And nothing about the view layout's structure: the RFC explicitly declines to propose there.

**Amended 2026-11-04, dated: "the largest of the five" was wrong about D4.** PR-076-C measured it
instead of estimating it: `control_coverage` already discharged D4's own enumerated scope (14 of
21 coverage-table entries were already a verified visible control, including every action D4
named), and the one real gap was a single top-bar control plus two corrected records — not the
largest slice of the five, the smallest.

## One thing that is not yours to decide

`.git-exclude/specs/tekstide-external-design-v0.md` §4.2 still authors "Default base font size:
14 px". The accepted RFC supersedes it and is the record; the source document is the owner's.
**Do not edit it**, and do not treat the mismatch as a defect to fix.
