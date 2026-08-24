# RFC-040: Affordance Completion

Status: **Proposed 2026-08-25**, filed in the same commit that closed RFC-039, so that RFC's
audit findings are carried rather than left in a review thread.
Target milestone: to be set by the human owner.
Date: 2026-08-25

Related RFCs:

- [RFC-039](../done/039-interaction-model-and-visible-affordances.md) — established the
  principles and built three affordances. Its PR-039-D audit is this RFC's entire input.
- [RFC-036](../accepted/036-dormant-capability-closure.md) — `OpenSafeCloseDialog` and
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

## Open questions

- **OQ1.** Should the affordance audit become a test? A mechanical check — every `Candidate`
  `NavigationAction` either appears in the `.on_press` inventory or is on an explicit
  keyboard-only allow-list — would make the count fail loudly instead of drifting. It is the same
  shape as `action_catalog_key_is_some_iff_the_action_is_live`, and this project's record is that
  hand-audits go stale and enumerations do not.
- **OQ2.** Where do controls for terminal, mode, save and agent-run *live*? A toolbar, per-surface
  buttons, and a command palette are three different answers with different costs, and
  `OpenCommandPalette` is already `Reserved` for the third.
- **OQ3.** Is `PasteIntoTerminal` genuinely keyboard-only by convention, or unexamined? Terminals
  conventionally paste by keyboard, and this product has RFC-018's paste-protection model around
  it — but "it is conventional" is what was said about every other missing control until someone
  counted.
