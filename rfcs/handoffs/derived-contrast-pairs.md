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

## snora 0.57.0's notice to app teams, read 2026-11-09 -- what it does and does not mean here

Received at `.git-exclude/upstream/snora/receive/0.57.0/`. **No reply needed** (the letter says so),
and nothing in it obliges us to act: snora is not a dependency -- declined twice, per *Not in
scope* above, and verified absent from both `Cargo.toml` and `Cargo.lock` before writing this. So
every consumer-facing item is inapplicable to us: 0.57's two removals (`toast::sweep_expired`,
`ToastLifetime::DEFAULT`), the deprecation table with its removal versions, the
`snora-control` -> `snora-interaction` rename, the "skip 0.55.0" warning, and the `cargo audit`
note (which is specifically about *snora's own* lockfile not binding a consumer's).

**It is, however, our first notice covering 0.54-0.56** -- this directory jumps 0.53 to 0.57 -- and
three of those items bear on RFC-076, which is the RFC currently open.

1. **snora 0.54 shipped two AAA presets.** That is prior art for **RFC-076 D3** (the default theme
   toward AAA, enforced AA floor untouched) from a project pinned to the same `iced 0.14`, by the
   team this document already trades contrast findings with in both directions. **Read their
   0.53 -> 0.54 guide before scoping D3**, and read it as evidence rather than as a design to copy:
   their colour roles are not ours, and the derived-pair guardrail above is the thing to check their
   presets against.
2. **snora 0.55 grows targets and icons with the text.** A different answer to the problem RFC-076
   D1 solved with a whole-UI `scale_factor`. Not a defect in ours -- scaling the layout space grows
   targets by construction -- but worth knowing when D3 touches theme values, because their
   approach decouples target size from text size and ours does not.
3. **snora 0.54 withdrew the claim that its controls were keyboard-reachable under `iced 0.14`, and
   0.55 restored it** once an application routes Tab (`snora::keyboard::traverse`). The obvious
   question is whether we carry the same claim and owe the same withdrawal. **Checked: we do not,
   and the reason is structural.** We never use iced's focus system for modal buttons at all --
   Tab and Shift+Tab route to `ModalFocusNext`/`ModalFocusPrevious` (`crates/tekstide/src/shell.rs`,
   the two `Named::Tab` arms), nine call sites cycle our *own* highlight index, and
   `keyboard_help.rs`'s own doc comment already records the asymmetry that actually matters here:
   modal buttons are reachable **by construction**, surface-local buttons are **not**, because
   `FocusZone` cycles zones rather than widgets -- which is the stated reason `control_coverage`
   exists. So the six catalog entries whose on-screen hint reads *"Tab/Shift+Tab moves focus; Enter
   activates"* are true as written, and snora's withdrawal does not transfer.

**One datum worth keeping for the dependency record**: snora is still on `iced 0.14`. Our own pin is
not lagging a peer building the same kind of application on the same substrate.

## snora 0.54's AAA presets read, and D3 measured against them, 2026-11-11

The 2026-11-09 section above said to read snora's 0.53 -> 0.54 guide before scoping D3. **Read**, from
the public index at <https://nabbisen.github.io/snora/guides/migrations.html> -- it needs no relay
through the owner, which is worth knowing the next time a handoff defers an upstream question.

**What they did**: `Tokens::light_aaa()` / `Tokens::dark_aaa()` hold "every declared text pair at 7:1
(WCAG AAA), keeping Light's and Dark's background and surfaces", moving "only text roles, and fills
whose own text needed it, move, in lightness only, with hue within 6 degrees". They shipped these as
**two new presets beside an unchanged `light`/`dark`**, with `Tokens::presets()` for a picker. The
only enforced minimum they raised was one widget's (the active sidebar item, "because a name is
text"), not a global floor.

**Why the mechanism does not transfer, recorded so it is not re-proposed**: their answer to AAA is
*more presets*. We ship one palette and have no preset system, and building one would be a feature
RFC-076 does not ask for. D3's own sentence is the design: the default theme moves, the floor stays.
Their §5 (accent declared as text at 4.5:1) does not transfer either -- **`accent` is never text
here**: its two production uses are a 2 px modal border (`crates/tekstide/src/shell.rs:11242`) and
the editor caret fill (`crates/tekstide/src/surface/editor.rs:509`).

**What does transfer** is their hierarchy note -- "with `text_muted` raised to 7:1, colour carries
less of the text hierarchy" -- because we have exactly one analogue and it is the whole of D3's gap.

**D3 measured, through the shipped formula over `Palette::default`** (scratch arithmetic, re-derived
as committed scaffolding by PR-076-D, not by this note): `foreground` on `background` **14.62:1**, on
`surface_elevated` **13.21:1**, and over the composited scrim 16.24/16.00 -- so **every text pair
this document's derived list declares already clears AAA with room to spare**. The non-text roles sit
at 6.38/5.77 (`accent`, `border_focused`) and 3.85/3.48 (`border_default`) against their 3.0
threshold, and WCAG defines no AAA for component boundaries, so none of them is an AAA question at
all.

**The one text pair below 7:1 is the one this document's own mechanism cannot see.**
`crates/tekstide/src/theme.rs:244-246` draws a disabled button's label as `foreground` at `a: 0.6`,
which composites to **5.59:1** on `surface_elevated` (5.88:1 on `background`) -- clears AA, fails
AAA, and is measured by nothing. The reason is structural and is the finding worth keeping: the alpha
is applied at the **widget-style site, downstream of the exhaustive destructure**, which sees
`foreground` opaque and cannot see a widget that dims it. `E0027` fires when a *role* is added;
nothing fires when a *site* modifies one. That is this document's defect class reappearing one layer
above where its guardrail reaches, and closing it is worth more than the 1.41 ratio points. A sweep
of `crates/tekstide/src` excluding tests found this to be the **only** text colour in the GUI that is
not a bare theme role.

**A trap named before anyone falls in it**: "toward 7:1" is a floor to hold, not a target to move to.
Taking `foreground` from 14.62 down to 7 would satisfy a literal reading of D3 and make the product
worse for the population RFC-076 exists for. Nothing in D3 lowers a ratio.

**One datum for the dependency record**: snora's guide index publishes migrations through
**0.58 -> 0.59**, two minors past the 0.57.0 notice in `.git-exclude/upstream/snora/receive/`. The
0.58 -> 0.59 guide reads as shipped (past tense, a removal slated for 0.60.0) but carries no date.
Nothing follows -- snora is not a dependency -- but our received record is behind what they publish,
which matters the next time a handoff cites "their latest".

## PR-076-D3 implemented, 2026-11-11, corrected under review 669 -- the ratio table re-derived, the one gap closed by deletion, and a guardrail for the gap's own class

**The ratio table re-derived independently, not taken from the brief.** A fresh script run
through the identical formula (`relative_luminance`'s 0.03928 boundary, `contrast_ratio`'s
`(lighter + 0.05) / (darker + 0.05)`) over `Palette::default`'s literal values reproduced every
figure the start-file's own §2 table gives, to four decimal places: `foreground` on `background`
**14.6167**, on `surface_elevated` **13.2059**; over the composited scrim, **16.2409**/**16.0050**;
`border_focused`/`accent` (identical colour) at **6.3813** on both backdrops; `border_default` at
**3.8513**/**3.4796**. Every declared text pair clears AAA with room to spare, and the four
non-text pairs are not an AAA question at all -- WCAG defines no AAA tier for a UI-component
boundary. (The scrim-composited figures are addressed on their own, below -- review 669 found
them re-derived but not declared anywhere, which is its own finding.)

**The one text pair below 7:1 was real, and it is now gone, not tuned.**
`button_style`'s disabled-label branch (`crates/tekstide/src/theme.rs`) dimmed `foreground` to
`a: 0.6`, composited to **5.5939:1** on `surface_elevated` (**5.8849:1** on `background`) --
clears AA, fails AAA, measured by nothing.

**Reachability, corrected under review 669 -- the claim was true but short, in a way that made
the deletion look more arguable than it is.** The first write-up said only that every
`crate::theme::button(` call site reaches a concrete `.on_press(..)`, with no conditional form
used (`on_press_maybe`/`on_press_with` appear nowhere in production). That count was also wrong on
its own terms -- **53** sites, not 48 (`shell.rs` 44, `shell/run_report.rs` 3, `surface/editor.rs`
3, `surface/board.rs` 2, `surface/explorer.rs` 1, found by script: production source only, cut at
`#[cfg(test)]\nmod tests`, comments stripped), confirmed independently and matching the review's
own count exactly. But that survey, even corrected, is a claim about *our* call sites, and review
669 found a **second route to `Status::Disabled` inside iced itself**, independent of `on_press`
entirely: `iced_widget 0.14.2`'s `Button::draw` renders via
`theme.style(&self.class, self.status.unwrap_or(Status::Disabled))` (`button.rs:376`), and
`status` starts `None` (`Button::new`, `:114`) and is only ever set to `Some` inside the `update`
handler's own `RedrawRequested` branch (`:356-357`). A button that has not yet seen that event is
therefore drawn `Disabled` **regardless of whether a press handler is set.** What keeps that from
happening in practice is a property of the *runtime*, not of our call sites: `iced_winit 0.14.0`'s
redraw loop (`lib.rs`, around `:813-838`) runs `interface.update(slice::from_ref(&redraw_event),
..)` -- a synthetic `RedrawRequested` -- before every draw, so `status` is always `Some` by the
time a button is actually styled. Both locked versions checked directly against `Cargo.lock`
(`iced_widget 0.14.2`, `iced_winit 0.14.0`), not assumed from a doc comment.

**This makes the deletion more right, not less.** Had that runtime guarantee ever lapsed, the old
branch would have dimmed the label of a perfectly *enabled* button on its first draw -- a
rendering defect, not an accommodation; "dead code carrying an unmeasurable claim" was generous to
it. **Deleted, per the start file's own instruction for this branch**: a dead branch that fails
AAA is a line to remove, not a number to tune. The now-updated
`a_button_is_drawn_from_the_configured_roles` test asserts the opposite of what it used to --
`Status::Disabled`'s text colour equals `Status::Active`'s -- so a future reintroduction of
dimming fails that assertion before anything else, regardless of which route produced the
`Disabled` status.

**A new test holds `Theme::default` to AAA as its own standard, separate from the enforced
floor.** `theme_default_text_pairs_meet_wcag_aaa_not_only_the_enforced_aa_floor`
(`theme/tests.rs`) iterates `derived_contrast_pairs`' own `is_text` entries (a field added so the
split is explicit rather than inferred from `threshold == 4.5`) and asserts each at 7:1, with its
own message distinguishing this from `MIN_TEXT_CONTRAST`, which stays at 4.5 and ungoverned by
this test -- raising it would retroactively refuse a user theme valid under `NFR-UX-003`. Passes
today at 14.62/13.21; the point is that nothing stopped the default from regressing toward 4.6
before this test existed, and now something does.

**The guardrail gap itself -- replaced, under review 669, with the positive form rather than a
wider ban-list, then widened again in round 2 to the full surface.** The first mechanism banned
one receiver *spelling* of derivation (`..theme.<role>()` struct-update syntax) and was proven
insufficient in review: `..state.theme.foreground()` and a two-step `let base = theme.foreground();
Color { a: .., ..base }` both derive a modified colour from a role and neither matched a ban aimed
at how a colour might be *produced* -- a space with no enumerable end (`self.theme`, any local
binding, a future `Color::with_alpha`). What **is** enumerable is the small, finite set of field
*assignments* that exist in shipped source -- not how a value might be derived, but where a value
is finally *used*.

`every_production_text_colour_is_a_bare_theme_role_or_a_declared_exception` (`theme/tests.rs`)
enumerates every such assignment across **three** spellings, not one: the `text_color:` field (13
sites: `Some(theme.foreground())` x7, `Some(state.theme.foreground())` x3, bare `theme.
foreground()` x2, `Some(foreground)` x1), a direct `text::Style { color: .. }` construction (zero
sites today -- production always reaches this field through a builder, never the literal), and the
`.color(..)`/`.color_maybe(..)` builder calls `iced_core::widget::text::Text` and
`iced_core::text::Span` both expose under that same method name (one site: `grid_colors.rs`,
painting a terminal cell in the ANSI colour the running program chose). Round 1's own mechanism
covered only the first of these three -- found by review 669 round 2 reading the pinned
`iced_core` source for every place it sets a text colour, not by guessing at more receiver
spellings -- and is why the claim is now scoped to "every spelling this function enumerates,"
not "every site," which is the distinction round 1's own stale wording blurred.

**Round 2 also corrected the exceptions list from value-keyed to path-keyed (required-6)**:
`DECLARED_TEXT_COLOUR_EXCEPTIONS.contains(&value.as_str())` admitted `"Some(foreground)"` at *any*
production site that happened to spell a local the same way, including a future one where that
local was genuinely derived -- a sixteen-character coincidence, not a verified site. Each entry is
now a `(file, value)` pair, checked against the path the assignment was found in, so an exception
is a statement about one read, verified site, not a string that travels.

**Ablated against three probes across round 1 and round 2, not described.** `Some(iced::Color {
a: 0.5, ..state.theme.foreground() })` wired into `modal_dialog_box`'s real `text_color:` field,
`Color { a: 0.5, ..probe_base }` (with `probe_base = theme.foreground()` bound first) wired into
`button_style`'s, and `.color(iced::Color { a: 0.5, ..state.theme.foreground() })` chained onto
the window-title `text(..)` in `top_bar_title_and_actions_row` -- all three caught, each reported
by file, spelling and the exact violating value; each removed afterward and every touched file
confirmed byte-identical to its pre-ablation state (`shell.rs`/`theme.rs` md5s matched the
backups both review rounds independently took). **One correction of my own, named rather than
left implicit**: round 1's own *first* two probes, as review 669 round 2 pointed out, were bare
`let _ablation_probe = ..;` statements -- not `text_color:` assignments at all, so the literal
probes would not have failed round 1's mechanism either, correctly, since an unused derived
colour is not a text pair. What is recorded above as "ablated" is my own adaptation of those two
probes into real field assignments before running them against the new, replaced mechanism --
the right adaptation (the mechanism asks what a field is assigned, not how a colour was produced),
but a different claim from "the literal probes failed," which the first write-up of this section
did not distinguish and should have. The replaced mechanism's own stale claim ("provably covers
every site... because struct-update syntax has no second spelling") is retracted along with the
mechanism: the premise was true and the conclusion did not follow, because the pattern matched a
receiver spelling, not the syntax itself.

**The composited-scrim backdrop, settled rather than assumed -- review 669's fourth finding.**
The re-derived table above includes `foreground` over the scrim composited on each real backdrop
(16.2409 / 16.0050), and no entry in `derived_contrast_pairs` covers it; the existing scrim test
(`modal_over_scrim_backdrop_clears_3_to_1_at_every_content_value`) is the *non-text* sweep for the
modal card's own border/fill, not a text pair. Checked directly rather than assumed: `view`'s own
`stack![base, opaque(scrim)]` gives the scrim-styled container exactly one child, `modal_view`, an
exhaustive match over all 13 `ModalContent` variants. All 13 arms call a distinct function (12 in
`shell.rs`, one -- `purge_dialog_view` -- in the inline `audit_view` module), and every one of
those 13 functions' own final, top-level expression is a call to `modal_dialog_box` or
`modal_dialog_box_split` -- read in full, not sampled, confirming no function places a `text(..)`
sibling outside that call. `modal_dialog_box` itself sets `text_color: Some(state.theme.
foreground())` over an opaque `background: Some(state.theme.surface_elevated())` -- the existing
`foreground on surface_elevated` pair `derived_contrast_pairs` already declares and measures.
**So no text is ever rendered against the composited scrim**; the figures above are a real,
re-derivable property of the formula (and clear AAA by a wide margin either way), but not a
declared pair, because `derived_contrast_pairs`' own convention -- "a pair only belongs here if
the role is genuinely rendered on that backdrop" -- correctly excludes it. Recorded here, with the
verified mapping, rather than left as the same "measured once, declared nowhere" shape this slice
exists to close.

**What this does not do.** No preset system, no change to `MIN_TEXT_CONTRAST`/`MIN_FOCUS_CONTRAST`
or the derived list's `4.5`/`3.0` thresholds, no AAA claim for a non-text pair, nothing touching
`grid_colors.rs` or screen-reader scope, nothing reopening D2's sizes -- all per the start file's
own §6.

## Filed by review 669, not required of PR-076-D3 -- the sibling class: a bare role declared non-text, used as text

The new guard (`every_production_text_colour_is_a_bare_theme_role_or_a_declared_exception`) checks
every `text_color:` assignment against a bare accessor or a declared exception; it does not, and
is not meant to, check that a role used *outside* `text_color:` was declared with the right
intended usage. `accent` is declared `is_text: false` at threshold 3.0 (5.77:1 on
`surface_elevated`) because its two production uses are non-text (a 2px modal border, the editor
caret fill) -- but nothing stops a future call site from passing `state.theme.accent()` to a
`text_color:` field instead of the `color:` field it currently occupies at, e.g., `shell.rs:11242`.
Such a site would pass the new guard (it is a bare accessor) while being held to the non-text 3.0
threshold rather than the text 4.5 floor or the 7:1 AAA bar -- the same "on screen, in no list"
shape this document exists to close, one declaration away. Not fixed here: the positive-form
mechanism in review 669's required-1 fix would need to additionally check, for each bare-accessor
`text_color:` hit, that the role's own `derived_contrast_pairs` declaration marks it `is_text:
true` -- a small extension to the same test, left for whoever next touches this guard rather than
built speculatively now.
