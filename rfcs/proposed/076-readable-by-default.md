# RFC-076: Readable By Default — text scale, and defaults an older reader can use

Status: **Proposed 2026-11-03**, at the human owner's direction. The owner asked whether the view
layout, typography and colour could be kinder to older users; this RFC is the answer to the two
parts of that question where the code says yes, and an explicit statement of the part where it
says the product is already correct.

**This RFC proposes amending the external design document.** `tekstide-external-design-v0.md` §4.2
authors *"Default base font size: 14 px"*. D2 below changes it. Accepting this RFC is therefore
also accepting that amendment, and it is the reason this is an RFC rather than a tuning commit.

## Summary

Three findings, measured against the code rather than impressions:

1. **Colour is already right, and enforced.** `config/appearance.rs` holds
   `MIN_TEXT_CONTRAST = 4.5` and `MIN_FOCUS_CONTRAST = 3.0` — WCAG AA — and a configured pair below
   the floor **is refused rather than used**. `NFR-UX-003` is met by machinery, not by documentation.
   This RFC proposes no change to the floor.
2. **The defaults sit at the bottom of a range that already allows 32 px.** Body 14.0, heading
   16.0, status 13.0 (`appearance.rs:270-272`), against `MAX_FONT_SIZE_PX = 32.0`. The heading is
   2 px larger than body, so the hierarchy reads as emphasis rather than structure, and the status
   bar — which carries the state a reader most needs — is the smallest text in the product.
3. **There is no way to change text size without editing a file.** I grepped the navigation
   actions and the shell for `Zoom`, `scale_factor` and any equivalent: **there is none.** The only
   path is `config.toml` plus `Ctrl+Alt+C`.

Finding 3 is the one that matters most, and it is worth being precise about why. `NFR-UX-004` says
font size *must be configurable*, and it is — so **the requirement is met and this RFC proposes
going past it.** For a reader who finds the text too small, "edit a TOML file and reload" is not an
accommodation; it is a wall placed between them and the product. No requirement names that
population. The owner did.

## Decisions

### D1. One scale factor, applied through `iced`'s own — not three font sizes

A single user-facing scale, bound to `Ctrl+=` / `Ctrl+-` / `Ctrl+0`, implemented through
`iced::application::Application::scale_factor` (present in the pinned `iced 0.14.0`,
`application.rs:410`/`:518`).

**Why the whole UI rather than text only.** Scaling text alone leaves buttons, hit targets, focus
rings and spacing at their original size — which is the wrong half for a reader who also has
reduced precision. `scale_factor` scales layout with the type, so the control a user has to hit
grows with the label they have to read.

The three existing font-size keys stay exactly as they are. Scale multiplies; it does not replace
them. `NFR-UX-004`'s configurability is unaffected.

**Two interactions this must not break, named rather than discovered:**

- **The window size is fixed at launch**, per this project's own GUI rules. A scale change must not
  resize the window; it changes how much fits inside it, which is the point.
- **Layout measurements are asserted.** `surface::explorer::tests::drawing_the_largest_tree_...`
  measures a 26-row window of a 10 000-row tree against a frame budget, and row count is a function
  of scale. Any test that asserts a row count or a frame cost must state the scale it assumes, or
  it will drift the first time the default changes.

### D2. Raise the defaults: body 16, status 15, heading 20

From body 14 / status 13 / heading 16. This is the amendment to external design §4.2.

Rationale, in order of weight: the status bar stops being the smallest text in the product; the
heading gains a real step over body so structure is visible without reading the words; and 16 px
body is the floor most platform guidance now treats as the minimum for sustained reading rather
than a comfortable maximum.

**The bounds do not change** (`MIN_FONT_SIZE_PX = 8.0`, `MAX_FONT_SIZE_PX = 32.0`). A user who
prefers 14 can still have it; it simply stops being what a first-time reader is given.

### D3. The default theme moves toward AAA; the enforced floor stays AA

4.5:1 is AA for normal text. The relevant target for low-vision reading is AAA, **7:1**. Because
the floor is already machinery, the default theme's own pairs can be raised toward 7:1 **without
touching `MIN_TEXT_CONTRAST`** — so every user theme that passes today keeps passing.

Raising the enforced floor to 7.0 is **rejected**: it would retroactively refuse themes that are
valid under the authored requirement, and `NFR-UX-003` asks for sufficiency, not AAA.

### D4. The first-contact actions get visible controls

`configuration.md` documents **23 `Ctrl+Alt+…` chords**; the chrome has about **six** clickable
affordances. Three-key simultaneous chords are hard with reduced dexterity or one-handed use, and
invisible to anyone who has not read the help.

`NFR-UX-001` requires keyboard access to every workflow and is **met** — this is the converse, which
no requirement states: the handful of actions a first-time reader needs (open a project, help,
audit, settings) should each have a visible control, with the chord remaining the expert path. The
top bar's own `?` and `Audit` buttons are already this pattern; D4 extends it rather than inventing
it.

**Not** a menu bar, and **not** a control for all 23. Scope is first contact.

### D5. The scale persists in state, not in the user's configuration file

A runtime zoom has to be remembered somewhere. **It must not be written into `config.toml`.** That
file is the user's, this product's whole installation story is "touches nothing else — no shell rc
file, no package repository, no sudo", and an application that silently rewrites a hand-edited TOML
breaks that promise in the one place the user is most likely to notice.

So the scale lives with the other per-user state under the state directory, alongside
`recent-projects.json`, and `local-data-and-privacy.md` gains it as a named path — that page's own
contract is that it lists *every* file written.

A scale set in `config.toml` by hand still wins as the starting value; the runtime control adjusts
from there.

## Non-goals

- **Screen-reader support.** `README.md` and the book both state there is none, and it is bound to
  the `iced` substrate decision, not to this work. This RFC does not weaken that statement and does
  not pretend scale is a substitute.
- **Following the OS text-scale setting.** Desirable, platform-specific, and separable. Reserved.
- **Animation, high-contrast OS theme following, dyslexia-oriented typefaces.** Each is its own
  question; `NFR-UX-005` already forbids nonessential animation.
- **Anything about the view layout's structure.** I checked: the first screen is "No projects yet",
  a path field, **Browse…**, a hint and three header controls. That is already *less is more*, the
  menu positions are conventional, and I am not proposing to spend on it ahead of D1 and D2.

## Acceptance

**What the owner decides**: this RFC's acceptance is also acceptance of the external-design
amendment in D2, and of D3's direction (default theme toward AAA, floor unchanged).

**What I am asking to be accepted**: D1–D5 as written, with D1 and D2 as the first slice — they are
the two that answer the owner's question, and D2 alone is a one-line change with a visible effect.
D4 is the largest and should follow, not lead.

**Independent of the macOS gate.** Unlike RFC-074's staging, nothing here needs a trustworthy green
on both platforms to be proven: the changes are in the GUI crate and the appearance config, and
their evidence is a rendered capture plus the existing contrast tests.
