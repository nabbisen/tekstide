# RFC-040: Affordance Completion

Status: **Implemented and closed 2026-08-25.** Proposed 2026-08-25, filed in the same commit that closed RFC-039, so that RFC's
audit findings are carried rather than left in a review thread.
Target milestone: **M12** — accepted by the human owner 2026-08-25, scheduled first of three,
ahead of RFC-020's remaining surface and the minimal user documentation.

Its three open questions were **decided by the architect on acceptance**; an implementer must not
inherit an unresolved architecture decision.
Date: 2026-08-25

Related RFCs:

- [RFC-039](../done/039-interaction-model-and-visible-affordances.md) — established the
  principles and built three affordances. Its PR-039-D audit is this RFC's entire input.
- [RFC-036](../done/036-dormant-capability-closure.md) — `OpenSafeCloseDialog` and
  `CycleVisibleTerminalSession` remain on its list; `OpenDiffReview` belongs to RFC-020.

## Summary

RFC-039 asked whether every action a user needs has a visible control. Its own audit answered:
**three of thirteen do.**

A user cannot launch a terminal, switch between Content and Terminal mode, save a file, start an
AI CLI run, open Trust Settings, open Help, or view an AgentRun report using anything the window
shows them. Every one of those requires knowing a `Ctrl+Alt+<letter>` learned from documentation
or from the Help modal — which itself has no visible control.

RFC-039 met its acceptance criterion, which was scoped to moving between projects. This RFC is
the rest of the principle it stated.

## The audit's findings, as inputs

From `handoffs/039-interaction-model-and-visible-affordances/affordance-audit.md`. The method is
exhaustive rather than sampled: this crate's only click mechanism is `iced::widget::button` with
`.on_press`, there are exactly ten such call sites in the whole application, and `mouse_area` /
`on_click` / custom interaction handling do not exist anywhere.

**1. Every modal in the crate is keyboard-only for its own decision.** All nine `ModalContent`
variants' view functions contain zero buttons. Several — trust granting, transcript purge,
project close, the folder browser — are *opened* by a real visible button, so a user arrives with
a mouse and cannot finish or cancel without a keyboard. This is the sharpest finding: the product
does not merely lack controls, it strands people mid-flow.

**2. Ten of thirteen live actions have no visible control anywhere.**
`OpenProjectEntryField`, `ToggleProjectMode`, `LaunchTerminal`, `PasteIntoTerminal`,
`SaveActiveDocument`, `LaunchAgentRun`, `OpenCurrentAgentRunDetail`, `OpenApprovalHistory`,
`OpenTrustSettings`, `OpenHelp`. Only `OpenProjectBoard`, `SwitchActiveProject` and
`OpenFolderBrowser` have one, and all three were built by RFC-038/039.

Two are annotated rather than flattened: `OpenProjectEntryField`'s *workflow* is reachable
through the Browse button even though the action is not, and `PasteIntoTerminal` may be a
legitimate keyboard-only convention. Both are decisions for this RFC, not exclusions from it.

**3. `OpenSafeCloseDialog` stays dead, for a sharper reason.** PR-039-C built the capability its
name promises and wired it to `×`. Whether it should also gain a coarse global accelerator — the
same precise-control / coarse-keybinding split `SwitchActiveProject` has — is open.

**4-5.** `CycleVisibleTerminalSession` and `OpenDiffReview` unchanged; `OpenCommandPalette` stays
`Reserved` with nothing behind it.

**6. Nine `AuditQuery::latest(50)` test sites carry a latent shared-store race**, named by test in
the audit. Not an affordance finding; carried here because it has no other home and would
otherwise be lost. Converting all nine, recording the risk, or making the audit store per-test are
the three options — the last ends the class rather than the instances.

## Goals

1. **Every action a user is expected to perform has a visible control**, or is recorded as a
   deliberate keyboard-only convention with a stated reason. "Ten of thirteen" becomes a number
   somebody chose rather than a number nobody noticed.
2. **No flow strands a mouse user.** A dialog reached by clicking can be completed and cancelled
   by clicking.
3. **The audit becomes mechanical.** RFC-039's audit was done by hand and is already going stale;
   a new action added tomorrow reintroduces the gap silently. See Open Questions.

## Non-goals

- Removing keyboard operability from anything. Every control added stays keyboard-operable;
  RFC-015's focus model and RFC-018's trusted-UI rules apply unchanged.
- A visual redesign, an icon set, or a toolbar aesthetic. This is about what exists to interact
  with.
- Deciding RFC-020's or RFC-034's surfaces. Actions owned by unbuilt RFCs stay theirs.

## Decisions (were open questions; settled on acceptance)

**D1 — the audit becomes a test, and it is the first slice.**

Yes, and it goes first so everything after it is measured rather than asserted. Every
`Candidate` `NavigationAction` either appears in the `.on_press` inventory or is on an explicit,
reasoned keyboard-only allow-list. Same shape as
`action_catalog_key_is_some_iff_the_action_is_live`.

This project's record decides it: RFC-039's audit was accurate on Monday and its own count was
wrong by Tuesday. Hand audits go stale; enumerations do not. Doing it first also means the
allow-list is written **before** anyone is tempted to add to it under deadline.

**D2 — per-surface controls. Not a toolbar, not a command palette.**

The actions are context-dependent: `SaveActiveDocument` needs an open file, `LaunchAgentRun`
needs a trusted project, `PasteIntoTerminal` needs a focused terminal. A toolbar would either
show permanently disabled buttons — noise that teaches people to ignore the toolbar — or change
contents as context shifts, which is worse. Put each control where its action applies.

**Modals get real buttons**, which is the sharpest finding and the one that strands people: a
dialog reached by clicking must be completable and cancellable by clicking. That is not a
toolbar question; it is per-modal by definition.

`OpenCommandPalette` stays `Reserved`. A palette is discoverable only by someone who already
knows to open it, so it does not answer the problem this RFC exists for — it is a keyboard
accelerator wearing a discovery costume. It remains a legitimate future addition and is not
foreclosed.

**D3 — `PasteIntoTerminal`'s invocation stays keyboard-only; its confirmation does not.**

Terminals conventionally paste by keyboard, and a paste button on a terminal grid would be
unusual enough to confuse rather than help. Recorded as a **deliberate** keyboard-only
convention with that reason, and it goes on D1's allow-list rather than being quietly excluded
from the count.

The confirmation it raises is a different matter and is covered by D2: RFC-018's paste dialog is
one of the nine keyboard-only modals, and it gets buttons like the rest.
