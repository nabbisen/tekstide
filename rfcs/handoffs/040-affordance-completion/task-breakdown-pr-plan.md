---
title: "RFC-040: task breakdown and PR plan"
rfc: "RFC-040"
rfc_file: "../../done/040-affordance-completion.md"
source_rfc_status: "Implemented and closed 2026-08-25 — RFC-040 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-25"
---

# Four slices, executed A → B → C → D

Order stated once; letters are never renamed.

## PR-040-A — make the audit a test, before building anything

**Build:** a mechanical check that every `Candidate` `NavigationAction` with a binding either
appears in the crate's `.on_press` inventory or is on an explicit keyboard-only allow-list.

- The inventory is enumerable: this crate's only click mechanism is `iced::widget::button` with
  `.on_press`, and `mouse_area` / `on_click` do not exist. RFC-039's audit established that by
  grep and the reviewer re-verified it. If that ever stops being true, this test's premise is
  gone — **assert it**, so the premise fails loudly rather than the conclusion failing quietly.
- The allow-list carries a **reason per entry**, not just a name. `PasteIntoTerminal` is the first
  and its reason is D3's: terminals conventionally paste by keyboard and a paste button on a
  terminal grid would confuse more than help.
- Per `ARCHITECTURE.md`'s enumeration-unit rule the unit is the **action**, so a new action with
  no control and no allow-list entry fails this.

**Ablate** by removing one action's control and confirming the test names that action.

Write the allow-list now, while nothing depends on it passing. An allow-list written to make a
red count go green is an excuse ledger.

## PR-040-B — modals get buttons

**Read [`what-a-clickable-modal-must-not-become.md`](./what-a-clickable-modal-must-not-become.md)
first.** Nine modals, two of them destructive, all of them trusted chrome.

**Build:** real buttons for each modal's own decision, so a flow begun with a mouse can be
finished or abandoned with one.

- Keystroke suppression unchanged; no second interaction-capturing layer; no `mouse_area`.
- Destructive choice never the default focus; a bare `Enter` still does what it does today.
- Click and keystroke route to **one** handler, producing the same audit record — proven for
  `ProjectCloseModal`'s `Cancelled`, which Escape already records.
- **A test that a control behind an open modal cannot be clicked.** The keyboard half is proven;
  the mouse half never has been.

**Evidence:** a real mouse-only round trip through the close confirmation — click `×`, click
Cancel, nothing destroyed — and the same for a confirmed close.

## PR-040-C — visible controls for the actions that have none

**Build:** per-surface controls, per D2. Not a toolbar, not a palette.

Ten actions have none. `OpenProjectEntryField` and `PasteIntoTerminal` are annotated in the
audit — the first has a visible route through the Browse button even though the action does not,
the second is D3's allow-list entry. That leaves eight needing a home, each placed where its
action applies: mode switching and terminal launch on the workspace, save on the editor, agent
run where a trusted project's actions live, and so on.

- Every control stays keyboard-operable. RFC-015's focus model and RFC-018's trusted-UI rules are
  unchanged; a control that only responds to a mouse is not finished.
- Context-dependent actions: decide per control whether it is hidden or visibly unavailable when
  its precondition is unmet, and **say which and why**. A control that silently does nothing is
  the "Add Project" defect this whole arc began with.

**Evidence:** the PR-040-A count moving, with the number stated before and after.

## PR-040-D — closeout

- Fill `qa-evidence.md`, tick the checklist with citations, state known limitations.
- Answer RFC-040's goals in its own words, including goal 1's "a number somebody chose".
- **Correct anything this work falsifies**: the README, `--help`, and RFC-039's audit all
  describe an application where ten of thirteen actions have no control.

## Standing expectations

- **Single-variable ablations**, the unit being the design decision.
- **Disclose flakes** against `test-process-leak.md` before reporting; it now records two
  distinct causes and the second is unfixed.
- **A premise that would surprise a user is a finding.**
- **If your slice makes a shipped statement false, correcting it is part of your slice.**
