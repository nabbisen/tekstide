---
title: "RFC-044 acceptance and QA checklist"
rfc: "RFC-044"
rfc_file: "../../accepted/044-surface-local-keyboard-affordances.md"
source_rfc_status: "Accepted 2026-08-27 — M12"
target_milestone: "M12"
created: "2026-08-27"
---

# Acceptance and QA checklist

## The claim this RFC exists to be able to make

- [ ] **A keyboard-only user can close a project**, proven live, not inferred — the defect that
      widened this RFC's scope.

## D1 — the registry, over the right domain

- [x] Entries are keyed by **(surface, key)** in substance — `SurfaceAction` variants are
      per-surface, and `Enter` naming six different actions across six surfaces is six variants,
      not one. One deviation, recorded rather than silent: within a single surface, `Enter` can
      itself be context-dependent (Explorer's own Enter opens a file *or* navigates a directory
      depending on the highlighted row's kind, the same branch `handle_explorer_key` already had);
      the domain is (surface, disambiguated action), a refinement (surface, key) alone cannot
      express, not a departure from it.
- [x] The domain is **surface actions**, not `NavigationAction`. **Closing a project is in it** —
      `SurfaceAction::TabStripCloseProject`.
- [x] Surface-local keys are **not** in `KeybindingPolicy`. This slice adds nothing there; the
      reason (a bare key would shadow every surface via `matching_global_action`) was traced
      directly this pass and recorded in `qa-evidence.md`, not merely repeated.

## D3 — the mirror, exhaustive

- [x] `ControlCoverage` has a `MouseOnly { reason }` arm.
- [x] A `MouseOnly` entry **requires** a reason, checked by a real test
      (`every_surface_action_has_a_checked_keyboard_route_or_a_reasoned_mouse_only_entry`), not
      only documented.
- [x] Adding a surface action without deciding its keyboard route **fails to compile**.
- [x] Ablated: added a fifteenth, undecided variant — both `surface_keyboard_coverage` and its own
      handler-name lookup failed to compile, naming the missing variant. Restored.
- [x] **Not enforced by a source scan** in the sense that matters. It *is* a text-based check, the
      same shape `control_coverage`'s own already-accepted cross-check uses — written down in
      `qa-evidence.md` why that specific technique is not the kind RFC-042's failed guard warns
      against (a key match has one canonical spelling in this codebase; a widget-builder chain
      does not). Dispatch-through-registry itself was found impractical and that finding is
      written down too: `FocusZone` has three variants and the real dispatch site calls every
      `MainArea` handler unconditionally, each self-guarding — no table to hook a registry into
      without restructuring it.

## D4 / PR-044-A — the inventory

- [x] The inventory exists: fourteen `SurfaceAction` entries, each with its keyboard route or its
      stated absence (`SURFACE_ACTION_INVENTORY`, `shell/tests.rs`).
- [x] **The count of `MouseOnly` entries is stated: one** (`TabStripCloseProject`), printed by
      `surface_action_inventory_has_no_unclosed_tracked_gaps` on every run.
- [x] Gaps encoded as a failure, deliberately, mirroring PR-043-A: the suite is red until PR-044-B
      closes the one `TrackedGap` entry. Stable across three consecutive full-workspace runs.

## PR-044-B — access

- [ ] Every `MouseOnly` entry is closed or is a stated, permanent exception.
- [ ] Keys were confirmed unclaimed on their surface **before use**.
- [ ] Independent actions use fixed keys, not a shared cursor.
- [ ] Each closed gap has a test through the real message path.

## D2 / PR-044-C — advertising

- [ ] Help gains a surface-grouped section; `--help` gains the same grouping.
- [ ] Both are **generated from the registry**, not hand-written.
- [ ] Ablated: remove a registry entry, watch it disappear from the generated help.
- [ ] **No key hints added to control labels.**

## Live GUI evidence

- [ ] Against a **`mktemp -d` fixture project with a fresh `XDG_STATE_HOME`**. No path under
      `$HOME`, no real project name, no other project on screen.
- [ ] Shows a project being **closed entirely by keyboard**.
- [ ] Whether a real mouse click was sent is stated either way.

## Gates

- [ ] `fmt`, `clippy -D warnings`, `git diff --check`, `rfc_docs_invariants`.
- [ ] Full workspace suite, **three consecutive runs**, each logged to a file; any flake named
      against the register **with a row**, not only mentioned.

## Closeout

- [ ] `README`'s keyboard table reflects whatever D2 produced.
- [ ] Any statement this slice makes false is corrected — `0.15.0`'s changelog lists both
      "closing a project is mouse-only" and "surface-local keys are not advertised" as known
      limitations.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
