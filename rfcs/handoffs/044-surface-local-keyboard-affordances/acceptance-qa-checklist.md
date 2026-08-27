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

- [x] **A keyboard-only user can close a project**, proven live, not inferred — the defect that
      widened this RFC's scope. `EVIDENCE-1`/`EVIDENCE-2`, zero mouse clicks, `Tab`×3/`Right`×1/
      `Delete`×1.

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

- [x] Every `MouseOnly` entry is closed or is a stated, permanent exception. One entry existed
      (`TabStripCloseProject`); closed.
- [x] `Delete` confirmed unclaimed on `TabStrip` before use — `handle_tab_strip_key`'s own match
      previously had only `ArrowRight`/`ArrowLeft`/`Enter`.
- [x] Independent actions use fixed keys, not a shared cursor. (Only one action closed this pass;
      the fixed-key convention it follows is `handle_trust_settings_key`'s own, unchanged.)
- [x] The closed gap has a test through the real message path:
      `delete_on_a_highlighted_project_tab_closes_that_project` dispatches
      `Message::Input(RoutedInput::Surface(...))` through `super::update()`, not a direct call to
      `attempt_close_project_tab`. Ablated (removed the new key arm): failed. Restored: passes.

## D2 / PR-044-C — advertising

- [x] Help gains a surface-grouped section; `--help` gains the same grouping.
- [x] Both are **generated from the registry** (`SURFACE_ACTION_ORDER`/`surface_action_entry`/
      `surface_action_help_lines`), not hand-written — `help_modal_view` and `usage_text` both call
      the same function.
- [x] Ablated: `surface_action_entry`'s `TabStripCloseProject` arm changed to `None`, reran --
      `surface_action_help_lines_is_derived_from_the_registry` failed (13 vs 14 expected lines).
      Restored: passes.
- [x] **No key hints added to control labels.** Verified directly: the diff touches only
      `keyboard_help.rs`, `en.ftl` (new keys, none aliasing an existing button/label key),
      `help_modal_view` (the Help modal itself — the one place bindings and controls are named
      together, by design), and this slice's own tests.

## Live GUI evidence

- [x] Against a **`mktemp -d` fixture project with a fresh `XDG_STATE_HOME`**. No path under
      `$HOME`, no real project name, no other project on screen.
- [x] Shows a project being **closed entirely by keyboard**.
- [x] Whether a real mouse click was sent is stated either way. **No mouse click was sent.**

## Gates

- [x] `fmt`, `clippy -D warnings`, `git diff --check`, `rfc_docs_invariants`. All clean.
- [x] Full workspace suite, **three consecutive runs**, each logged to a file; any flake named
      against the register **with a row**, not only mentioned. **456 + 4 + 746, fully green**
      every time — no flake.

## Closeout

- [ ] `README`'s keyboard table reflects whatever D2 produced. **Deliberately not done**: per this
      RFC's own README, making the table generated (rather than merely checked) is out of scope —
      it needs a separate decision about whether the README is generated or merely checked, which
      this RFC does not make.
- [x] Any statement this slice makes false is corrected. **Not** in `0.15.0`'s own changelog
      entry, per response 352's required correction and this project's own established precedent
      (request 334, RFC-042's checklist): a released entry is not rewritten after the fact. The
      correction text is held in `qa-evidence.md`, ready for whoever cuts `0.16.0`.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
