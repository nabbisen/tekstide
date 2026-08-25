---
title: "RFC-039 PR-039-D: the affordance audit"
rfc: "RFC-039"
rfc_file: "../../done/039-interaction-model-and-visible-affordances.md"
source_rfc_status: "Implemented and closed 2026-08-25 — RFC-039 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-25"
---

# The affordance audit

**Superseded 2026-08-25 by RFC-040, closed out the same day it was written.** This document is a
point-in-time record of what was true when RFC-039 closed, kept as-is rather than rewritten --
its value now is historical. What it found is no longer the current state:

- **Finding 1** ("every modal's own decision is keyboard-only, without exception") -- fixed.
  RFC-040 PR-040-B gave all nine modals real, clickable buttons for their own decision.
- **Finding 2** ("ten of thirteen live global actions have no visible control anywhere") --
  narrowed to two. RFC-040 PR-040-C built the eight it could (`ToggleProjectMode`,
  `LaunchTerminal`, `SaveActiveDocument`, `LaunchAgentRun`, `OpenCurrentAgentRunDetail`,
  `OpenApprovalHistory`, `OpenTrustSettings`, `OpenHelp`); the remaining two
  (`OpenProjectEntryField`, `PasteIntoTerminal`) are RFC-040's own permanent, reasoned allow-list
  entries, a decision rather than a gap -- see `keyboard_help::control_coverage`.
- Findings 3–6 (the dead actions, `OpenCommandPalette`, and the nine query-race-shaped tests) are
  untouched by RFC-040 and still describe the current state.

See `rfcs/handoffs/040-affordance-completion/qa-evidence.md` for the current, complete count.

Per the task breakdown: **build nothing, find things.** Every `NavigationAction` against the
visible control that invokes it; every real button in the application inventoried; both inputs
response 312 named (the keyboard-only-modal question, the query-race sites) resolved and recorded
here rather than left in a review response.

**Method, stated so the finding is checkable, not asserted.** This crate has exactly one way to
make anything mouse-clickable: `iced::widget::button` with `.on_press(...)`. No
`mouse_area`/`MouseArea`/`on_click`/custom `Interaction` handling exists anywhere
(`grep -rn "mouse_area\|MouseArea\|on_click\|Interaction::" crates/tekstide/src/` returns nothing).
So the complete inventory of mouse-clickable controls in the whole application is the complete
list of `.on_press(Message::` call sites, checked exhaustively:

```
crates/tekstide/src/shell.rs:4965   GoToProjectBoardTabPressed
crates/tekstide/src/shell.rs:4988   SwitchActiveProjectTabPressed
crates/tekstide/src/shell.rs:4994   CloseProjectTabPressed
crates/tekstide/src/shell.rs:6642   RevokeWorkspaceTrust
crates/tekstide/src/shell.rs:6651   OpenTrustGrantDialog
crates/tekstide/src/shell.rs:6680   ToggleTranscriptCaptureDeclined
crates/tekstide/src/shell.rs:6708   OpenTranscriptPurgeDialog
crates/tekstide/src/shell.rs:6820   OpenApprovalHistoryEntry
crates/tekstide/src/surface/board.rs:252   (open_browser_message — OpenFolderBrowser)
crates/tekstide/src/surface/board.rs:311   (message — ReopenRecentProjectRowPressed)
```

Ten call sites. That is the entire set of things a mouse can activate in this application, full
stop. Every other file under `crates/tekstide/src/surface/` (`editor.rs`, `terminal.rs`,
`explorer.rs`) has zero `button(` calls of any kind — confirmed by direct grep, not inferred from
absence of `.on_press`.

## Finding 1 — every modal's own decision is keyboard-only, without exception

Nine `ModalContent` variants exist. Every one of their view functions was checked directly for
`button(` calls:

| modal | `button(` count in its own view fn |
| --- | --- |
| `LayerDemo` (`layer_composition_demo_modal`) | 0 |
| `PasteConfirmation` (`paste_confirmation_modal_view`) | 0 |
| `ExternalChange` (`external_change_modal_view`) | 0 |
| `Approval` (`approval_dialog_view`) | 0 |
| `TrustGrant` (`trust_grant_dialog_view`) | 0 |
| `TranscriptPurge` (`transcript_purge_dialog_view`) | 0 |
| `Help` (`help_modal_view`) | 0 |
| `FolderBrowser` (`folder_browser_modal_view`) | 0 |
| `ProjectClose` (`project_close_dialog_view`) | 0 |

Zero, in every case, without exception. Every modal's own decision — Approve/Reject, Grant/Cancel,
Purge/Cancel, Reload/Dismiss, Close/Cancel, the folder browser's own row navigation — is a plain
`text` line with a focus marker, activated only through `Tab`/`Shift+Tab`/`Enter`/`Escape`. This
is response 312's own question, generalized: it is not particular to `ProjectCloseModal`, it is
the crate-wide convention, with no exception anywhere.

**What this means concretely**: several of these modals are *opened* by a real, visible, mouse-
clickable button (`OpenTrustGrantDialog`, `OpenTranscriptPurgeDialog`, `CloseProjectTabPressed`,
`OpenFolderBrowser`) — a user reaches them with a mouse, then cannot complete or cancel the
decision without switching to a keyboard. RFC-039's own first principle — "every action needs a
visible control, a keyboard shortcut is only an accelerator" — was applied to *opening* these
dialogs across RFC-032/033/038/039 but not to what happens once they are open. Not something this
document is fixing (build nothing); recording it as the shape of the gap so whoever picks it up
does not have to re-derive it modal by modal.

## Finding 2 — ten of thirteen live global actions have no visible control anywhere

`KeybindingPolicy::linux_mvp()` has seventeen rules: one `Reserved`, thirteen `Candidate` (live,
with a real binding), three `Configurable` with `default_binding: None` (dead — the ones the task
breakdown already named). Cross-referencing the thirteen live actions against the ten-entry
click-inventory above:

| action | binding | visible control |
| --- | --- | --- |
| `OpenProjectBoard` | `Ctrl+Alt+P` | ✅ the "Projects" tab (PR-039-A/B) |
| `SwitchActiveProject` | `Ctrl+Alt+N` | ✅ each project's own tab (PR-039-B) |
| `OpenFolderBrowser` | `Ctrl+Alt+B` | ✅ the Project Board's "Browse..." button (PR-038-G) |
| `OpenProjectEntryField` | `Ctrl+Alt+O` | none of its own — see note below |
| `ToggleProjectMode` | `Ctrl+Alt+M` | **none** |
| `LaunchTerminal` | `Ctrl+Alt+T` | **none** |
| `PasteIntoTerminal` | `Ctrl+Shift+V` | **none** — see note below |
| `SaveActiveDocument` | `Ctrl+S` | **none** |
| `LaunchAgentRun` | `Ctrl+Alt+A` | **none** |
| `OpenCurrentAgentRunDetail` | `Ctrl+Alt+R` | **none** |
| `OpenApprovalHistory` | `Ctrl+Alt+H` | **none** |
| `OpenTrustSettings` | `Ctrl+Alt+U` | **none** |
| `OpenHelp` | `Ctrl+Alt+K` | **none** |

Three have one. Ten do not, at all — not "hard to find," genuinely absent from the ten-entry
inventory above. `ToggleProjectMode`, `LaunchTerminal`, and `SaveActiveDocument` are worth naming
specifically: these are not edge-surface actions, they are core, constantly-used workflow steps
(switch between editing and a terminal, open a shell, save a file), and none has a button, icon,
or menu item anywhere. A user who has not memorized the keybinding table cannot perform any of
these ten actions at all.

Two notes, so this table is not read as flatter than it should be:

- **`OpenProjectEntryField`** reveals/focuses the typed-path field for adding a *second* project.
  `OpenFolderBrowser`'s own button already serves the underlying workflow ("add a project") as
  the primary, visible route — D1's own overturn made the browser primary specifically because a
  typed path is not an acceptable primary way to choose a folder. Read generously, this keybinding
  is a power-user accelerator to an alternate input mode for a workflow that already has a visible
  control, not a workflow with none. Read strictly, the action itself still has no control of its
  own. Recorded as the more defensible of the ten; not removed from the count -- the *action*
  has none, even though its *workflow* does, and the count is of actions.
- **`PasteIntoTerminal`** (`Ctrl+Shift+V`) is a near-universal terminal-emulator convention with
  no obvious visible-button equivalent in comparable tools either (most terminal emulators don't
  put a "Paste" button in their own chrome). Plausibly a legitimate, accepted exception to the
  principle rather than a gap — but RFC-039's own principle makes no stated exception for it, so
  it is named here rather than silently excluded from the count.

## Finding 3 — `OpenSafeCloseDialog` is dead, and now for a more specific reason

Still `Configurable`/`None`, unchanged. But PR-039-C built a real, visible-control-first close
confirmation (`×` on every project tab → `ProjectCloseModal`) with no relationship to this
`NavigationAction` at all — the capability the action's own name promises now genuinely exists,
reachable by the visible control RFC-039 requires, just never wired to this identifier. Whether it
should now gain a binding as a coarser accelerator to the *active* project's own close (the same
"tab click is precise, keybinding is coarse" split `SwitchActiveProject`/the tab strip already
established) is a real design question this document is not deciding — flagged for whoever
next touches keybinding policy.

## Finding 4 — the two other named dead actions, unchanged

`CycleVisibleTerminalSession` and `OpenDiffReview` remain `Configurable`/`None`, exactly as the
task breakdown's own starting point said. Nothing in this RFC's scope touched either.

## Finding 5 — `OpenCommandPalette` stays `Reserved`, nothing behind it

Unchanged, already documented (`keyboard_help::tests::no_action_without_a_working_binding_is_advertised`'s
own explicit assertion that this binding must never be advertised). Restated here only for the
enumeration's own completeness — every rule in `linux_mvp()` is accounted for in this document.

## Finding 6 — nine tests share the query-race shape response 312 found and fixed five instances of

Response 312's own instruction: record the sites here, by test name (line numbers rot). All nine
follow the identical vulnerable shape PR-039-C's own five sites had before that response —
`AuditQuery::latest(50)`, then a client-side `.filter(...)` on `project_id`/`family` — which
`shell/tests.rs`'s own shared, real `AuditStore` makes unreliable the moment enough concurrent
audit-store traffic exists in the same test binary run to push a project's own record outside a
fifty-record window. They pass today; they are not proven robust against the same failure PR-039-C's
own tests hit under real parallel execution, for the identical mechanism.

| test | file:line (as of this audit) |
| --- | --- |
| `a_real_workspace_discovery_refusal_writes_a_real_restricted_mode_blocked_record` | `shell/tests.rs:2375` |
| `granting_trust_through_the_real_route_records_both_audit_records` | `shell/tests.rs:2977` |
| `revoking_trust_through_the_real_route_records_a_single_applied_record` | `shell/tests.rs:3058` |
| `purging_transcripts_through_a_real_key_sequence_records_a_real_audit_record` | `shell/tests.rs:4788` |
| `opening_a_project_through_the_real_field_writes_exactly_one_real_project_added_record` | `shell/tests.rs:7944` |
| `resubmitting_the_same_path_through_the_field_focuses_it_without_a_second_record` | `shell/tests.rs:7991` |
| `choosing_a_directory_through_the_real_browser_writes_exactly_one_real_project_added_record` | `shell/tests.rs:8683` |
| `committing_an_already_open_project_a_second_time_focuses_it_without_a_second_record` | `shell/tests.rs:8737` |
| `reopening_a_recent_project_writes_exactly_one_real_project_added_record` | `shell/tests.rs:8971` |

Not converted here — response 312 already named the reason this belongs to whoever picks it up
next, not a cleanup commit folded into this closeout: the choice between converting all nine,
recording the risk and converting none, or making the shared store per-test (the option that
would end the class rather than each instance) is a real design decision, not a mechanical fix.

## What this document is not

Not a claim that PR-039-A through PR-039-C did anything wrong — each of those slices' own scope
was exactly what it built, and each is the *reason* three of the seventeen actions now have a
visible control at all. This document is the sweep the task breakdown asked for once that work was
done: what RFC-039 fixed, and what it did not touch, stated plainly rather than left to be
rediscovered by the next person who reaches for `Ctrl+Alt+U` and cannot find a button for it.
