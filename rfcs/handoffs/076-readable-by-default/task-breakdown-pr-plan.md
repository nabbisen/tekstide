---
title: "RFC-076 — task breakdown and PR plan"
rfc: "RFC-076"
rfc_file: "../../done/076-readable-by-default.md"
source_rfc_status: "Implemented and closed 2026-11-12, as part of the 0.39.0 release, published 2026-11-12 — RFC-076 is in rfcs/done/"
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

## Final Acceptance Decision

**Accepted 2026-11-12 by the reviewer, for release as `0.39.0`.** The owner authorised the release
and the publish; RFC-076 moved to `rfcs/done/` at the candidate cut, deliberately before publish,
the same discipline RFC-027's, RFC-065's and RFC-066's rows set. Written here because
`.git-exclude/reviewed/` is correspondence and is not tracked: the pack is the record
(`release-checklist.md`'s own Review item, added at review 526 after four shipped releases left
this section blank).

**The rounds.** Five slices and the release candidate, each held and each closed on its own
evidence:

| Round | Slice | Required changes, and what they were |
| --- | --- | --- |
| 658 | PR-076-A (D2, raised defaults) | One, small: the configuration page still illustrated the value this RFC raised *away* from. |
| 659 | PR-076-B (D1, one scale factor) | One: the capture's own numbers disagreed with the claim it was offered as evidence for. Closed round 2. |
| 660 | PR-076-C (D4, the top-bar control) | One: `(index + 1) % len()` wraps largest back to smallest, which is wrong for *this* control — a reader who has reached the largest step does not want the smallest next. Closed round 2. |
| 661 | PR-076-C2 (persisted-scale validation) | Two: a non-finite scale **is** reachable from a real file, against the doc's claim that it is not; and the new check made one previously untested invariant load-bearing. Closed round 2. |
| 669 | PR-076-D3 (D3, AAA and the guardrail) | Six across three rounds: the guardrail did not cover the class it claimed to (ablated false); a false claim about `Status::Disabled` having one route in `iced` when it has two; a call-site count of 53, not 48; an undeclared composited scrim backdrop; a guard that saw `text_color:` but not the text widget's own colour; and an exception keyed on a spelling rather than a site. Closed round 3. |
| 671 | The `0.39.0` release candidate | Two: the release carried a dependency **security** fix (`RUSTSEC-2026-0459`/`CVE-2026-119375`, the bundled SQLite under the audit store) that the release notes did not mention at all; and the macOS disclosure question went unanswered. Both fixed in round 2; accepted. |

**What the acceptance rests on**, as distinct from what was claimed:

- D1–D5 are all implemented. The enforced AA floor (`MIN_TEXT_CONTRAST = 4.5`) is **untouched** —
  "toward AAA" was read throughout as a floor to hold for the default theme, never as a target to
  move the enforced minimum to.
- The default theme is held to AAA by its own dedicated test, with a vacuity guard, and the whole
  text-colour surface is covered by a positive-form guard with **path-keyed** exceptions — the
  version that survives an appended override, which the first two drafts did not.
- Gates: CI green at job level on the release commit, both workflows, and the full suite re-derived
  locally by the reviewer (2017 passed, 0 failed, 12 ignored) rather than read from the round.
- The Run It gate was performed on the **release** binary with a genuinely empty `argv`, and its
  evidence is in `.git-exclude/release-evidence/0.39.0/`. The reviewer opened the screenshot: every
  element the round described is on it, and nothing else.

**Deferred rather than fixed, and recorded so nobody rediscovers them as defects:**

- `.git-exclude/specs/tekstide-external-design-v0.md` §4.2 still authors a 14 px default. The
  accepted RFC supersedes it; the source document is the owner's and was deliberately not edited.
- The macOS Run It gate and the containment-guard watch's own suppress/confirm-red/revert procedure
  were again not performed on real Apple hardware. RFC-028 *Acceptance* option (2); disclosed in
  `0.39.0`'s own release notes rather than left silent.
- A configuration diagnostic can name `<config-path>` instead of a real path
  (`crates/tekstide/src/shell.rs:7593`), reachable when the configuration directory cannot be
  resolved at all — found live while setting up this release's Run It gate. The product behaved as
  designed (never refuse to start, degrade to compiled defaults, say so); the failure mode simply
  has no path to show. Not this release's defect and not fixed here; the mistake that surfaced it
  was at the launcher's own argument boundary and was fixed there.

**What this acceptance does not cover.** The AUR first publication of `tekstide` and `tekstide-bin`
is separate work with its own procedure (`release-checklist.md`, "First publication — 0.39.0 only")
and its own authority: the pushes are the owner's.
