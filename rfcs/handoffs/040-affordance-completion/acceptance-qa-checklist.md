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

- [x] **No flow a user can begin with a mouse requires a keyboard to finish or abandon.** All nine
      modals (PR-040-B) and all eight PR-040-C actions. Two named, reasoned exceptions remain
      (`PasteIntoTerminal`, `OpenProjectEntryField`) -- see `qa-evidence.md`'s own answer in this
      RFC's words.
- [x] Every live action has a visible control or a reasoned allow-list entry. The remaining count
      is a number somebody chose. **11 of 13** — `control_coverage`, stated before (3) and after.
- [x] Everything added is still keyboard-operable. No keyboard behaviour changed anywhere in this
      RFC; every pre-existing keyboard-path test still passes unmodified.

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

- [x] All nine modals can complete and cancel by click. Seven get a new click message for their
      destructive/decision-committing half (`activate_current_modal`, shared with `Enter`); the
      safe half of every two-button modal, plus LayerDemo's two and Help's new "Close", dispatch
      the literal `Message::ModalDismiss` directly. The folder browser's rows are individually
      clickable (`Message::FolderBrowserRowPressed`); its commit button reuses the existing
      `FolderBrowserChooseCurrentDirectory` message. One modal (Help) verified live against a
      real `iced` click, not only `update()` dispatch — see `qa-evidence.md`.
- [x] Keystroke suppression unchanged; no second interaction-capturing layer; no `mouse_area` —
      `no_click_mechanism_other_than_button_on_press_exists_anywhere_in_the_crate` (PR-040-A) still
      passes unmodified; nothing in this slice introduces a competing capture mechanism, only
      `button`/`.on_press`, the crate's one recognized click path.
- [x] Destructive choice is never default focus; a bare `Enter` does what it did before, ablated —
      no default-focus code changed this slice; every existing default-focus test re-verified
      green across three runs (`qa-evidence.md`).
- [x] Click and keystroke share one handler and produce the same audit record —
      `ProjectCloseModal`'s `Cancelled` proven both ways: `Cancel`'s own button dispatches the
      literal `Message::ModalDismiss` `Escape` already sends (the same `Message` value, not two
      paths that happen to agree), so the existing `escaping_the_close_confirmation_also_records_
      a_cancelled_decision` already proves it by construction.
- [x] **A control behind an open modal cannot be clicked** — the mouse half of modal exclusivity,
      never previously tested. `a_control_behind_an_open_modal_cannot_be_clicked`, plus an
      explicit `state.modal.is_some()` guard now on all ten background click handlers (six newly
      added this slice), ablated.
- [x] No untrusted value reaches a button label outside the catalog — every new button's label is
      `state.catalog.get(...)`/`catalog.get_with_args(...)` text, the same construction every
      existing modal label already used; nothing routes a raw string into a button label.

## PR-040-C — controls

- [x] Eight actions gain a control, each placed where its action applies: mode toggle on the
      workspace, "+ New Terminal" on Terminal Immersion (including its own empty-panes arm),
      Save on the editor, three buttons on Trust Settings (agent run, report, approval history),
      Trust Settings/Help in the top bar. Count moves 3 → 11 of 13 (`control_coverage`, PR-040-A's
      own measurement), stated before and after.
- [x] Every control keyboard-operable — each converges on the identical function its own existing
      keyboard accelerator already calls (`toggle_active_project_mode`,
      `launch_terminal_in_active_project`, etc.), the same "one setup, two routes" shape
      `open_folder_browser` established in PR-040-B; no keyboard behaviour changed.
- [x] For context-dependent actions: hidden or visibly unavailable is **decided and stated**, not
      left to silently do nothing. `LaunchAgentRun`: always shown, reuses the real, already-tested
      refusal-notice path when untrusted (not hidden). `OpenTrustSettings`: hidden with no active
      project (`top_bar_offers_trust_settings`, tested directly) — nothing to configure trust
      *for*. Both decided explicitly, in `qa-evidence.md`.

## Closeout

- [x] Count stated before and after. 3 → 11 of 13.
- [x] README, `--help` and RFC-039's audit corrected where this work falsified them. README's
      modal paragraph corrected in place (PR-040-B); `affordance-audit.md` given a dated
      correction note, not rewritten (its value is now historical); `--help`/the in-app reference
      were never stale (both derived, not hand-written prose).
- [x] Ablations single-variable; flakes disclosed against both causes in `test-process-leak.md`.
      This slice's own: `command_approval_family_produces_real_durable_audit_records_through_the_pipeline`
      hit twice in four gate runs -- the same already-documented, still-unfixed socket flake the
      architect independently re-measured in response 319 ("two failures in nine runs"), not a
      regression, not chased further per that document's own standing instruction.
- [x] Gates: `fmt`, `clippy -D warnings`, full suite **under default parallelism**, `git diff --check`
      — all clean; see `qa-evidence.md` for each slice's own run record.
- [x] Known limitations stated — `qa-evidence.md`'s own closing section: the two permanent
      keyboard-only actions, scrim-click-to-dismiss (out of scope from RFC-040's own start), the
      background-job process-group gap found verifying the leak fix, the nine query-race-shaped
      tests, and `OpenSafeCloseDialog`'s own still-open binding question.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
