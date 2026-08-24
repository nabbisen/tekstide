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

- [ ] Every `Candidate` action with a binding is in the `.on_press` inventory or the allow-list.
- [ ] The test **asserts its own premise**: `button` + `.on_press` is the only click mechanism;
      `mouse_area` / `on_click` absent.
- [ ] Allow-list entries carry a reason each, not just a name.
- [ ] Ablated: remove one control, the test names that action.

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
