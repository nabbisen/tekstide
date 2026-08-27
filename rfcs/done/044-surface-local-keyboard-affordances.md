# RFC-044: Surface-Local Keyboard Affordances

Status: **Implemented and closed 2026-08-27.** Accepted by the human owner the same day, shipped across PR-044-A/B/C and accepted at review 353. Proposed 2026-08-26 after RFC-034 shipped a
control reachable by `a`/`r` that nothing on screen names, and the reviewer found the same gap on
two other surfaces. **D1–D4 were decided by the architect on acceptance** — see "Decided on
acceptance" at the end, which also records what `0.15.0`'s release gate found the day after this
RFC was written, and how it changed the scope.
Target milestone: **M12**
Date: 2026-08-26

Related RFCs:

- [RFC-039](../done/039-interaction-model-and-visible-affordances.md) — established that naming a
  keystroke is not naming the path a user takes. This RFC is the other half of that sentence.
- [RFC-040](../done/040-affordance-completion.md) — moved visible controls from 3 to 11 of 13 live
  actions. It counted *global* actions.
- [RFC-016](../done/016-internationalization-and-localization.md) — owns the catalog any advertised
  key text would live in.
- [RFC-034](../done/034-change-review-actions-and-review-state.md) — the slice that made this
  visible. Its controls needed a keyboard path added in review, and then nothing advertised it.

## Summary

This application has two keyboard systems. One is a policy with a registry, a coverage test, and
a Help modal generated from it. The other is twenty-nine key comparisons spread across eight
handler functions, advertised nowhere and enumerable by nothing.

## What is actually true today

`KeybindingPolicy::advertised_bindings()` reads `KeybindingPolicy.rules` — the **global**
bindings, every one of which requires `Ctrl`. Fourteen are live. The Help modal is derived from
that list, `--help` prints it, and `control_coverage`'s exhaustive match makes it impossible to add
a `NavigationAction` without deciding how a user reaches it.

**None of that reaches surface-local keys.** Eight handlers —
`handle_explorer_key`, `handle_editor_key`, `handle_approval_history_key`,
`handle_change_review_key`, `handle_trust_settings_key`, `handle_project_board_row_key`,
`handle_tab_strip_key`, `handle_project_board_path_field_key` — match on roughly twenty-nine keys
between them. `Enter`, `Space`, `Delete`, arrows, and now `a` and `r`.

A user cannot discover any of them. They are in no registry, so nothing can enumerate them, no
test can require that they be advertised, and no surface renders them.

**The controls themselves are visible** — RFC-040 saw to that, and a mouse user is fine. This is
specifically about the keyboard user, who sees a button labelled "Mark accepted" and has no way to
learn that `a` presses it.

## Why now

Three surfaces have needed the same fix, one at a time, each found by review rather than by
anything mechanical:

- `ApprovalHistory` — mouse-only controls, fixed at response 234.
- `TrustSettings` — mouse-only Grant/Revoke, fixed at response 248. Its doc comment says *"mouse-
  only would have meant a keyboard user could not grant trust at all."*
- Change Review — mouse-only decision controls, fixed at review 334, **the third time.**

Each fix added a key. None added a way for anyone to find out about it. **The thing that catches
this is a reviewer noticing, three times, which is exactly the shape RFC-040 was written to
replace for global actions.**

## The question that makes this an RFC

**Should surface-local keys be a registry, or should they stay handler-local and be advertised
some other way?**

A registry is the move this project keeps making — `NavigationAction`'s exhaustive match,
`DisplayText`'s single constructor, `ChangeReviewContentLine` behind a module boundary. It would
make "a key exists that nothing advertises" unrepresentable.

It is also the heavier answer, and there is a real argument against it: these keys are *contextual*
by nature. `Enter` means different things on six surfaces, and that is correct — a registry
flattening them into one global namespace would be a worse product, not a better one. The global
policy's shape may not fit.

That tension is the RFC.

## Decisions required

**D1 — registry or not?** If yes, keyed by surface, not globally, or `Enter` collides with itself
six times. If no, then D3 must carry the whole enforcement burden and needs to be stronger.

**D2 — where does a user see them?** Candidates, not exclusive: on the control label ("Mark
accepted (a)"); a per-surface hint line; a section in the Help modal that changes with the active
surface; `--help`. Weigh against RFC-039's own principle — the control the user sees is what is
named, and a label crowded with key hints names it worse. **Decide what a keyboard user does on
first contact with a surface**, and let the mechanism follow from that.

**D3 — what makes it impossible to add an unadvertised key?** `control_coverage` is the precedent:
an exhaustive match nobody can extend without deciding. Without something of that shape this
recurs a fourth time, and the recurrence will again be found by a reviewer rather than a test.

**D4 — scope.** All eight handlers, or the surfaces where a key is the *only* route to an action?
The second is smaller and defensible; the first is consistent. Say which and why — do not do the
first and describe it as the second.

## Non-goals

- Rebindable keys, a config surface, or a keymap file. RFC-023 owns configuration and is closed;
  this is about advertising what exists.
- Changing which keys do what. If a binding is wrong, that is its own change.
- Screen-reader support. `iced` has no accessibility bridge; unchanged and still out of scope for
  that reason and no other.

## Risks

- **Advertising everything, and thereby advertising nothing.** RFC-034's own security document had
  to answer this for disclosures: `en.ftl` carries 28 `change-review-*` strings and a surface where
  every line is a caveat is one where none is read. The same trap applies to key hints.
- **A registry that flattens context.** See above; `Enter` is six different actions on purpose.
- **Fixing the mechanism and not the surfaces**, or the reverse. A registry with nothing rendered
  helps nobody; rendered hints with no enforcement recur.

## Acceptance-time decisions

**D1–D4 are decided by the architect on acceptance and recorded in this file before implementation
begins** — the rule RFC-041, RFC-042, RFC-034 and RFC-043 were all accepted under.

---

## Decided on acceptance, 2026-08-27

### What changed between proposal and acceptance

`0.15.0`'s release gate, trying to exercise the close path by keyboard, found that
**`CloseProjectTabPressed` has exactly one emitter** — the `×` button's `.on_press` at
`shell.rs:6084`. The tab strip's `Enter` switches projects or returns to the board. `FocusZone`
has three variants (`MainArea`/`Sidebar`/`TabStrip`), so `Tab` cycles *zones*, not widgets, and
cannot land on a button. **A keyboard-only user cannot close a project at all.**

That is a different defect from the one this RFC was written about, and a worse one:

- **Discoverability** — a key exists, nothing names it. The user can act if they guess.
- **Access** — no key exists. The user cannot act.

Both are in scope, and the second goes first.

### The structural finding, which decides D1 and D3 together

`keyboard_help.rs` already has the shape this RFC needs, pointed one way only.
`control_coverage(action: NavigationAction) -> Option<ControlCoverage>` is an exhaustive match
asking **"how does a *mouse* reach this?"** — RFC-040's direction. Its `ControlCoverage` enum can
say `VisibleControl` or `KeyboardOnly { reason }`.

**It cannot express `MouseOnly`.** The mirror does not exist, so the gap was not merely unnoticed —
it was *inexpressible*.

And its domain is `NavigationAction`: the fourteen global, `Ctrl`-prefixed navigations. **Closing a
project is not a `NavigationAction`**, so the exhaustive match that was supposed to guarantee
affordance coverage could not see the control in either direction. Neither could anything else.

### D1 — a registry, keyed by surface, over a **wider domain than `NavigationAction`**

Yes to a registry. The objection I raised — `Enter` means six different things on purpose —
dissolves once entries are keyed by **(surface, key)** rather than key alone. Six different
meanings become six different entries, which is correct; they *are* different actions.

**Do not put surface-local keys into `KeybindingPolicy`.** `matching_global_action` compares a
rendered binding string against `default_binding`, so a bare `Enter` there would become a global
action and shadow every surface. Separate registry, deliberately, and the reason recorded at the
type.

**The domain is a surface action** — something a user can do on a surface — **not a
`NavigationAction`.** That widening is the substance of D1. A registry over the old domain would
have been unable to represent the very control that prompted this decision.

### D2 — the Help modal, grouped by surface. **Not a key hint on every label.**

A keyboard user's need on first contact is to learn that keys exist and where to find them, not to
read a key on every button.

- **Help gains a surface-grouped section**, and `--help` gains the same grouping. `Ctrl+Alt+K` and
  the `?` button are already advertised, so that entry point is discoverable today.
- **Contextual filtering to the active surface is optional**, not required. Findable beats clever.

**Explicitly rejected: a key hint on every control label.** RFC-034's own security document had to
confront that `en.ftl` already carries 28 `change-review-*` strings and that a surface where every
line is a caveat is one where none is read. Turning "Mark accepted" into "Mark accepted (a)" across
every surface is that same failure wearing a different coat.

### D3 — add the mirror of `control_coverage`, exhaustive over the widened domain

`control_coverage` asks how a mouse reaches an action. Add the question nobody was asking:
**how does the keyboard reach it?** — exhaustive, so a surface action added without deciding its
keyboard route **fails to compile**.

- `ControlCoverage` gains its missing `MouseOnly { reason }` arm, so the state that was
  inexpressible becomes statable — and therefore countable.
- A `MouseOnly` entry must carry a reason, exactly as `KeyboardOnly` already does. `PasteIntoTerminal`
  is `KeyboardOnly` with a real justification; the mirror deserves the same bar rather than
  becoming a place to park gaps.

**A source scan is not acceptable as the enforcement.** RFC-042's first guard was one, and I
defeated it by respelling `scrollable(column(lines)` as `scrollable(column![`. If dispatching
handlers through the registry turns out to be impractical, **report why in writing** rather than
falling back to a scan without saying so.

### D4 — three slices, inventory first

Ordered as RFC-043 was, for the reason that worked there: make the problem enumerable before
fixing it, and let the enumeration be red.

1. **Widen the domain and produce the inventory.** Every surface action, with its keyboard route
   or its absence. Expect `MouseOnly` entries; expect the slice to end with a list nobody has
   today. PR-043-A's guard found four leaking tests and bounded everything after it — the same
   move.
2. **Close the access gaps.** Mouse-only controls get keys, closing a project first, since that
   one is proven.
3. **Advertise**, per D2.

**Scope is every surface, not only those where a key is the sole route.** I offered that narrowing
in the proposal and am withdrawing it: the release gate found the close control by accident, and a
scope that depends on someone noticing is the thing this RFC exists to replace.
