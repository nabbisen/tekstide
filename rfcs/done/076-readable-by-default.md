# RFC-076: Readable By Default — text scale, and defaults an older reader can use

Status: **Implemented and closed 2026-11-12, moved to `done/` as part of the `0.39.0` release,
published 2026-11-12.** D1 through D5 as written, with D1 and D2 as the first slice. **The
acceptance carries external design §4.2's amendment with it**: that document's authored *"Default
base font size: 14 px"* is superseded by D2, and this RFC is the record of it.
`.git-exclude/specs/tekstide-external-design-v0.md` is the owner's own source document and is left
untouched; if they want the source line to match, that edit is theirs to make or to ask for.
Accepted by the human owner 2026-11-03.

**Updated 2026-11-11, dated rather than silently edited.** D3 is implemented (PR-076-D3, review
669): the default theme's own text pairs are held to WCAG AAA (7:1) by a dedicated test, separate
from `MIN_TEXT_CONTRAST`, which stays at its enforced 4.5:1 exactly as D3 itself required — no
user theme that passed before this slice can fail now. D3 was RFC-076's last open decision, so
D1 through D5 are all implemented as of this update, which closes the RFC per
`an_rfc_a_release_names_lives_in_done`.

Proposed 2026-11-03, at the human owner's direction. The owner asked whether the view
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
   **True of the pre-change state, named precisely rather than left to teach the wrong lesson**:
   D2's own dated amendment below found the actual defect was that 13 px is too small to read
   comfortably, an absolute problem, not the relative one this finding states — see that
   amendment for the correction and why it changed D2's own numbers.
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

**Amended 2026-11-04, dated, not a silent edit: `Ctrl+=`/`Ctrl+-`/`Ctrl+0`, withdrawn before
landing.** `chord.rs`'s own grammar refuses any key that is not `is_ascii_alphanumeric()` --
`Ctrl+0` parses, `Ctrl+=` and `Ctrl+-` do not, and widening the grammar to admit them would
reintroduce the exact failure it already refuses `Shift`+digit for: `=` and `-` sit elsewhere, or
behind `Shift`, on AZERTY and QWERTZ, so a chord spelled with either would silently never match on
those layouts, in a feature whose whole purpose is reaching a user who is struggling. Replaced
with six absolute steps instead of a relative accumulator: `Ctrl+Alt+1` through `Ctrl+Alt+5` set
the scale to exactly one of five fixed sizes (smallest to largest), and `Ctrl+Alt+0` resets to
exactly `1.0`, independent of which step was last pressed. Digits are layout-stable unshifted on
every Latin layout, which is the whole reason to prefer them; "bounded, enumerated set" (below) is
satisfied by construction this way, not by clamping a repeatable `+`/`-` press.

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
  **Checked at implementation time, PR-076-B, and found imprecise in the same way D2's own
  "smallest text" finding was**: row count is a function of font size, not of `scale_factor`
  directly. `iced`'s own layout runs entirely in logical pixels (`iced_core::window::Settings`'s
  own doc: *"the initial **logical** dimensions"*) -- `scale_factor` governs only the
  logical-to-physical conversion at the windowing boundary, never the layout math itself, so a
  live-measured viewport (`state.explorer_viewport`, re-measured by `iced` on every relayout,
  scale changes included) and a logical font size stay mutually consistent at any scale with no
  code change. The frame-cost test itself passes a hardcoded viewport height directly, never
  constructs a `State` or a scale factor at all, so it is untouched by this feature by
  construction, not by a state-the-scale discipline this amendment first expected to need.

### D2. Raise the defaults: body 16, status 16, heading 20

From body 14 / status 13 / heading 16. This is the amendment to external design §4.2.

Rationale, in order of weight: 16 px body is the floor most platform guidance now treats as the
minimum for sustained reading rather than a comfortable maximum; the heading gains a real step
over body so structure is visible without reading the words; and the status bar — the text that
carries state, which a reader most needs — stops being read in a size too small for comfortable
reading.

**The bounds do not change** (`MIN_FONT_SIZE_PX = 8.0`, `MAX_FONT_SIZE_PX = 32.0`). A user who
prefers 14 can still have it; it simply stops being what a first-time reader is given.

**Amended 2026-11-03, dated, not a silent edit: status is 16, not the 15 this D2 first named.**
PR-076-A's own review found the original criterion ("the status bar is no longer the smallest
text in the product") unachievable at 13→15 against body's 14→16 — the ordering is preserved
exactly, status still 1px under body. That exposed a deeper error in this RFC's own §Summary:
the defect it named was "status ranks below body" (a relative claim), when the real defect was
"13px is too small to read comfortably" (an absolute one). A status bar one step under body is
ordinary hierarchy, not a fault — chasing the relative claim would have meant inventing a
hierarchy signal nobody asked for. Status is set equal to body instead: at these sizes a 1px step
conveys no perceptible hierarchy (confirmed in PR-076-A's own capture), so it was pure cost —
one pixel of legibility spent on the text that most needs it, for a distinction no reader can
see. This also makes the original criterion true, but that is not why the ruling was made.

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

**Amended 2026-11-04, dated, not a silent edit: the enumeration above predated D1's own feature,
and that is the fault, not a reason to leave it standing.** The "23" figure above is itself now
stale — PR-076-B added six more chords, and `advertised_bindings_are_exactly_the_live_ones` counts
**26** live bindings today, not 23. PR-076-C's own measurement, against `keyboard_help.rs`'s
`control_coverage` (checked mechanically, not by inspection: `VisibleControl`'s `on_press_snippet`
must literally appear in the source a claimed button lives in), found D4's scope *as originally
enumerated* already discharged — of its own 21 coverage-table entries (one shared entry covers the
six interface-scale actions together, so 21 counts entries, not raw action variants), 14 already
had a verified visible control, including every one of "open a project," "help," "audit,"
"settings" by name. The real gap PR-076-C found was the one this enumeration *excluded by
omission*: when D4 was written, D1 was not yet implemented, so there was no scale feature to
enumerate, and the exclusion that followed from that (the six interface-scale actions were filed
`KeyboardOnly`, citing this very list as authority) was never a considered decision — it was an
absence this list could not have named. The corrected list: **open a project, help, audit,
settings, and change text size.** For the population this RFC exists for, a reader who cannot read
the text cannot discover a keyboard-only control that fixes it, which makes a visible text-size
control not merely *a* first-contact action but the first one. PR-076-C's own top-bar "Text size"
(cycle) and "Reset" buttons close this; see that slice's own evidence capture.

**Not** a menu bar, and **not** a control for all 26 live actions. Scope is first contact.

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
