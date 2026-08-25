---
title: "RFC-040: Acceptance / QA Checklist"
rfc: "RFC-040"
rfc_file: "../../accepted/040-affordance-completion.md"
source_rfc_status: "Accepted 2026-08-25 — M12, first of three"
target_milestone: "M12"
created: "2026-08-25"
---

# Acceptance / QA Checklist

Every unchecked line at closeout carries a stated reason.

## The acceptance criterion

- [ ] **No flow a user can begin with a mouse requires a keyboard to finish or abandon.**
- [ ] Every live action has a visible control or a reasoned allow-list entry. The remaining count
      is a number somebody chose.
- [ ] Everything added is still keyboard-operable.

## PR-040-A — the audit as a test

- [x] Every `Candidate` action with a binding is in the `.on_press` inventory or the allow-list.
      `every_live_action_has_a_visible_control_or_a_reasoned_allow_list_entry`, exhaustive over
      `keyboard_help::control_coverage`.
- [x] The test **asserts its own premise**: `button` + `.on_press` is the only click mechanism;
      `mouse_area` / `on_click` absent.
      `no_click_mechanism_other_than_button_on_press_exists_anywhere_in_the_crate`.
- [x] Allow-list entries carry a reason each, not just a name. Two permanent entries with real
      reasons (`PasteIntoTerminal`/D3, `OpenProjectEntryField`/workflow-served-elsewhere), eight
      `TrackedGap` entries stating "no visible control yet -- tracked for RFC-040 PR-040-C" rather
      than a bare name.
- [x] Ablated: remove one control, the test names that action. `SwitchActiveProject`'s own
      snippet replaced with a nonexistent one, test failed naming it. The first attempt at this
      ablation surfaced a real bug in the test itself (see `qa-evidence.md`) -- fixed, then
      re-ablated and confirmed correctly failing before revert.

## PR-040-B — modals — `what-a-clickable-modal-must-not-become.md`

- [ ] All nine modals can complete and cancel by click.
- [ ] Keystroke suppression unchanged; no second interaction-capturing layer; no `mouse_area`.
- [ ] Destructive choice is never default focus; a bare `Enter` does what it did before, ablated.
- [ ] Click and keystroke share one handler and produce the same audit record —
      `ProjectCloseModal`'s `Cancelled` proven both ways.
- [ ] **A control behind an open modal cannot be clicked** — the mouse half of modal exclusivity,
      never previously tested.
- [ ] No untrusted value reaches a button label outside the catalog.

## PR-040-C — controls

- [ ] Eight actions gain a control, each placed where its action applies.
- [ ] Every control keyboard-operable.
- [ ] For context-dependent actions: hidden or visibly unavailable is **decided and stated**, not
      left to silently do nothing.

## Closeout

- [ ] Count stated before and after.
- [ ] README, `--help` and RFC-039's audit corrected where this work falsified them.
- [ ] Ablations single-variable; flakes disclosed against both causes in `test-process-leak.md`.
- [ ] Gates: `fmt`, `clippy -D warnings`, full suite **under default parallelism**, `git diff --check`.
- [ ] Known limitations stated.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
