# RFC-044: Surface-Local Keyboard Affordances

Status: **Proposed 2026-08-26.** Written after RFC-034 shipped a control reachable by `a`/`r` that
nothing on screen names, and the reviewer found the same gap on two other surfaces.
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
