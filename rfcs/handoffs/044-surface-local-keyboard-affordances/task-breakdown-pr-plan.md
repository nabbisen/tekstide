---
title: "RFC-044 task breakdown and PR plan"
rfc: "RFC-044"
rfc_file: "../../done/044-surface-local-keyboard-affordances.md"
source_rfc_status: "Implemented and closed 2026-08-27 — RFC-044 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-27"
---

# Three slices, inventory first

Ordered as RFC-043 was, for the reason that worked there: **make the problem enumerable before
fixing it, and let the enumeration be red.** PR-043-A's guard produced a four-test inventory that
bounded everything after it, and caught two regressions in the next slice within days.

## PR-044-A — make the gap expressible, then count it

**No user-visible change. This slice is expected to end red.**

1. **Widen the domain.** A surface action — something a user can do on a surface — is not a
   `NavigationAction`, and closing a project proves it. Define that set.
2. **`ControlCoverage` gains `MouseOnly { reason }`**, the arm that does not exist today, which is
   why the gap was inexpressible rather than merely unnoticed.
3. **Add the mirror of `control_coverage`**: exhaustive over the widened domain, answering *how
   does the keyboard reach this action?* An action added without deciding fails to compile.
4. **Produce the inventory** — every surface action, its keyboard route or its absence, with
   reasons on the exceptions.

**Evidence:** the inventory itself, and the count of `MouseOnly` entries. Nobody has that list
today; `0.15.0`'s gate found one member of it by accident.

**Gate:** the suite may be red at the end of this slice if you choose to encode the gaps as
failures. Say which you did and why — both are defensible; silently choosing the green one is not.

## PR-044-B — close the access gaps

Every `MouseOnly` entry either gets a keyboard route or a stated, permanent reason in the shape
`KeyboardOnly` already uses.

**Closing a project comes first**, since it is proven and since it is the one that makes RFC-043's
whole termination behaviour unreachable by keyboard.

- Pick keys unclaimed by any other handler on that surface, **confirmed before use** — the same
  check `handle_trust_settings_key`'s own doc records making.
- Independent actions get fixed keys, not a shared highlight cursor. That reasoning is already
  written in `handle_trust_settings_key` and applies unchanged.

**Required test per gap closed:** the action is reachable by keyboard, through the real message
path, and the `MouseOnly` entry is gone or has become a reasoned exception.

**Evidence:** the live walkthrough, **closing a project entirely by keyboard**, against a
`mktemp -d` fixture with a fresh `XDG_STATE_HOME`.

## PR-044-C — advertise

Per D2: the Help modal gains a section grouped by surface; `--help` gains the same grouping.

- Generated from the registry, not hand-written. A hand-written list is the thing this RFC exists
  to replace.
- **No key hints on control labels.** §1 of the risk document; D2 decided it.
- Contextual filtering to the active surface is optional. Findable beats clever.

**Required test:** every registry entry with a key appears in the generated help, and ablating an
entry removes it from the help — so the generation is real rather than parallel.

## Not in this plan

- Rebindable keys, a keymap file, or any configuration surface.
- Changing which keys do what. If a binding is wrong, that is its own change with its own reason.
- The README keyboard-table check — possible once the registry exists, and a natural follow-up.
- Screen-reader support.
