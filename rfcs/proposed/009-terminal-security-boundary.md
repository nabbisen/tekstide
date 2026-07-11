# RFC-009: Terminal Security Boundary

Status: Proposed
Target milestone: M4
Date: 2026-07-11

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-security-threat-model-v0.md`
- `tekstide-roadmap-milestones-v0.md`
- [`ROADMAP.md`](../../ROADMAP.md)

Depends on:

- [RFC-004](../done/004-security-baseline-and-restricted-mode.md)
- [RFC-007](../done/007-runtime-substrate-pty-feasibility.md)
- [RFC-008](../done/008-terminalsession-process-lifecycle.md)

Blocks:

- production terminal security claims;
- production paste-protection claims;
- terminal clipboard behavior;
- native approval-dialog spoofing boundary claims;
- AgentRun launch surfaces that imply terminal security guarantees.

Design review:

- Accepted with notes on 2026-07-11 in `.git-exclude/reviewed/tekstide-review-request-047-rfc009-terminal-security-boundary-design-response.md`.
- Implementation must carry the review notes into PR-009-A/B/C: exact sequence-family enumeration, terminal-generated reply policy, modal trusted UI active-state handling, and bounded diagnostics with sequence-family metadata.

## Summary

This RFC defines Tekstide's terminal security boundary for untrusted terminal output and terminal input. It specifies the minimum model and policy needed before Tekstide can claim paste protection, ANSI/VT containment, clipboard safety, or approval-dialog spoofing resistance.

RFC-009 does not require the final desktop GUI terminal widget. It creates the security contract that later terminal renderers, AgentRun launch surfaces, and approval/paste dialogs must obey.

## Motivation

RFC-008 made terminals real processes. Terminal output is now untrusted bytes produced by arbitrary shells and tools inside a project. Those bytes can contain ANSI/VT control sequences, OSC clipboard sequences, hyperlinks, title changes, bracketed-paste toggles, cursor movement, alternate-screen behavior, and text crafted to mimic app dialogs.

Tekstide must avoid two opposite mistakes:

- rendering terminal output as if it were safe app UI;
- overclaiming managed approval, paste protection, or sandboxing for plain terminal sessions.

The boundary must be designed before AgentRun launch and before the GUI terminal surface becomes user-facing product behavior.

## Goals

- Treat terminal output bytes as untrusted input.
- Define the supported terminal control subset for the first product boundary.
- Define behavior for unsupported ANSI/VT and OSC sequences.
- Block terminal output from mutating app chrome, trust state, approval state, clipboard, command history, audit state, or project metadata.
- Define paste interception before bytes reach the PTY.
- Define multiline/risky paste decision states that can work before the final GUI exists.
- Define clipboard policy, including OSC 52 behavior.
- Define the approval-dialog spoofing boundary between terminal content and trusted app/native dialogs.
- Preserve honest Plain/Supervised/Managed labels.
- Provide tests and evidence requirements for later implementation slices.

## Non-Goals

- Full terminal emulator correctness.
- VM/container-grade isolation.
- Malware detection.
- Command interception for Plain or Supervised sessions.
- Final GUI terminal widget acceptance.
- Final rendered approval or paste dialog UI.
- Transcript retention or durable audit persistence.
- Cross-platform GUI security acceptance.
- Semantic detection of every dangerous pasted command.

## Boundary Principles

1. **Terminal output is untrusted content.** It may be displayed only through terminal-surface APIs, never through app chrome or trusted dialogs.
2. **Terminal effects are explicit capabilities.** Clipboard writes, title changes, hyperlinks, paste modes, and bell/notification behavior are separate reviewed capabilities, not implicit side effects of parsing bytes.
3. **Unsupported control sequences fail inertly.** Unknown or unsupported sequences must be dropped, rendered visibly inert, or converted to bounded diagnostic events without executing app-level effects.
4. **Paste is not typing.** Pasted bytes are classified before PTY write. Multiline or policy-risky paste requires an explicit decision path.
5. **Approval UI is outside the terminal.** Terminal output must not be able to create, cover, move, restyle, or dismiss approval/security dialogs.
6. **Labels stay honest.** Plain terminals remain plain: paste protection and output containment do not become managed command approval.

## Terminal Output Model

Introduce a terminal security boundary equivalent to:

```text
PTY bytes
  -> TerminalParser
  -> TerminalDisplayEvent[]
  -> TerminalSurfaceModel
  -> Renderer
```

The exact Rust names may differ, but the implementation must preserve these roles:

- PTY bytes: untrusted output from a TerminalSession process;
- parser: converts bytes into bounded display events and security-relevant observations;
- surface model: terminal-local state only;
- renderer: draws terminal state inside a bounded terminal viewport;
- app/security UI: trusted surface outside terminal output control.

Terminal output may update:

- terminal cells;
- terminal cursor state;
- terminal-local modes allowed by this RFC;
- terminal-local scrollback within configured bounds;
- bounded terminal diagnostics.

Terminal output must not update:

- project trust state;
- approval or rejection decisions;
- app chrome, menus, tabs, window title, badges, or Project Board state;
- clipboard;
- command history outside the PTY;
- durable audit state, except through explicit app-generated events;
- file buffers, file explorer state, or project metadata.

## Supported Control Policy

The first product boundary should support a conservative ANSI/VT subset sufficient for common shells:

- printable UTF-8 text with replacement for invalid sequences;
- CR, LF, tab, backspace, and basic cursor movement;
- SGR styling for common foreground/background colors and text attributes;
- clear line/screen operations within the terminal surface;
- alternate screen only as terminal-local state, if the selected renderer supports it safely;
- bracketed paste mode as terminal-local input metadata, not as authority to bypass paste policy;
- terminal size reporting only if required by later renderer integration and bounded to terminal-local replies.

Unsupported or high-risk sequences must be inert by default:

- OSC 52 clipboard read/write;
- window title/app title mutation;
- desktop notifications;
- hyperlinks that auto-open or mutate app state;
- device control strings or private modes not explicitly reviewed;
- sequences that request host-side file, clipboard, prompt, or app-integration behavior.

If a later slice enables a currently unsupported sequence, it must document:

- exact sequence family;
- terminal-local effect;
- cross-platform behavior;
- test evidence;
- why it cannot mutate trusted app state.

PR-009-B must pin the exact parser grammar before claiming parser coverage. The implementation evidence must list accepted CSI, OSC, C0/C1/control, private-mode, and terminal-query families, plus the exact inert behavior for unsupported families. This includes OSC 8 hyperlinks, title mutation, DCS/PM/APC, mouse/focus reporting, keyboard protocol extensions, terminal identity queries, device status reports, cursor position reports, and size/query reply sequences.

## Terminal-Generated Replies

Terminal-generated replies are explicit capabilities, not incidental parser behavior.

Initial policy:

- block terminal-generated replies by default unless an implementation slice explicitly enables a bounded terminal-local reply;
- do not send app state, trusted UI state, clipboard contents, project metadata, environment values, or private terminal output back to the PTY;
- if a reply is enabled, document the exact query family, reply bytes, state source, and tests proving the reply is bounded and terminal-local.

Examples requiring explicit policy before enabling include:

- device status reports;
- cursor position replies;
- terminal identity replies;
- terminal size reports;
- focus/mouse/keyboard protocol replies;
- any query that could expose renderer, app, clipboard, project, trust, or approval state.

## Clipboard Policy

Terminal output must not read or write the system clipboard.

Required initial behavior:

- OSC 52 and equivalent clipboard control sequences are blocked or rendered inert.
- Blocking may create a bounded diagnostic event but must not include private output bytes.
- User-initiated copy from selected terminal text is allowed only as a renderer/app action, not a terminal-output side effect.
- Paste into terminal is routed through paste policy before PTY write.

## Paste Protection

Paste handling must distinguish at least:

- typed input;
- single-line paste;
- multiline paste;
- binary/control-containing paste;
- paste while a managed approval or security dialog is active.

Minimum policy:

- Typed input can be forwarded normally through the active terminal input route.
- Single-line paste may be allowed by default for Plain terminals if it contains no blocked control bytes.
- Multiline paste must require explicit confirmation before any bytes are written to the PTY.
- Paste containing NUL or disallowed control bytes must be blocked or transformed only by explicit reviewed policy.
- Paste must be addressed by ProjectId and TerminalId and cannot target another project's terminal.
- Paste must be blocked or queued while trusted approval/security UI is active or modal; this must not depend only on focus state.

The first non-GUI implementation may expose model states such as `Allowed`, `Blocked`, or `RequiresConfirmation` rather than a rendered dialog. The bytes must not reach the PTY before the decision is resolved.

## Approval-Dialog Spoofing Boundary

Terminal output can display text that resembles approval prompts. Tekstide cannot prevent arbitrary text from looking persuasive, but it can prevent terminal output from becoming trusted UI.

Required boundary:

- Approval, trust, paste-confirmation, and destructive-decision dialogs are rendered outside terminal output.
- Trusted dialogs have app-owned framing, focus, keyboard routing, and modality.
- Terminal output cannot move focus to a trusted dialog, dismiss it, or synthesize decisions.
- Terminal content cannot overlap trusted dialogs in the final GUI surface.
- Plain terminal labels must state that managed command approval is not guaranteed.
- Review evidence must include spoofing attempts that print approval-like terminal text and confirm it remains terminal content only.

## Security Labels

RFC-009 does not change RFC-004 compatibility labels:

- Plain Terminal: no command interception, no managed approval promise.
- Supervised AgentRun: lifecycle/transcript warning only until adapter support proves more.
- Managed AgentRun: managed approval only for supported structured actions.

Paste protection and terminal-output containment may apply to Plain terminals, but they must not be described as managed command approval.

## Persistence and Privacy

RFC-009 must not introduce transcript storage or terminal-output persistence.

Security diagnostics must be bounded and avoid private output:

- record sequence family and policy reason well enough for review and support;
- avoid raw terminal output, OSC payloads, pasted text, shell output, or environment-like values;
- avoid environment dumps and shell history;
- durable audit remains deferred unless an explicit later RFC provides storage.

## Test Plan

- Parser/unit tests for supported printable text, SGR, cursor movement, clear operations, and invalid UTF-8.
- Tests that unsupported OSC 52 and title/app-chrome sequences are inert.
- Tests that terminal output cannot mutate trust state, approval state, clipboard state, command history, Project Board state, or file buffers.
- Paste policy tests for typed input, single-line paste, multiline paste, control-containing paste, and wrong-project routing.
- Tests that paste bytes are not written to the PTY before confirmation when confirmation is required.
- Tests that approval/security dialog active state blocks or queues terminal paste according to policy.
- Spoofing tests with terminal output that mimics approval dialogs.
- Evidence that labels remain Plain/Supervised/Managed and do not overclaim command interception.

## Acceptance Criteria

- Terminal output is modeled as untrusted data behind a parser/display boundary.
- Supported and unsupported ANSI/VT/OSC behavior is explicit.
- OSC clipboard behavior is blocked or inert by default.
- Paste policy intercepts bytes before PTY write.
- Multiline paste requires an explicit decision state.
- Terminal output cannot mutate app chrome, trust, approvals, clipboard, command history, audit, file state, or project metadata.
- Approval/security dialogs are specified as trusted app/native UI outside terminal output.
- Security labels remain honest and do not imply command approval for Plain/Supervised sessions.
- Implementation slices and review evidence are defined before coding.

## Risks and Mitigations

- **Too little terminal support may break common tools.** Start conservative, document missing sequence families, and add reviewed capability increments.
- **Too much parser ambition may delay AgentRun work.** Implement only the boundary and common subset needed for safe first surfaces.
- **Paste prompts may become noisy.** Require confirmation for multiline/risky paste first; tune single-line behavior with evidence.
- **Spoofing cannot be solved by text inspection alone.** Keep trusted UI visually and structurally outside terminal output.

## Open Questions

- Should hyperlinks be completely disabled initially, or rendered inert with a copy-only action?
- Should bracketed paste mode change the confirmation copy, or only mark the paste source?
- What exact terminal parser/renderer dependency should be used for the first product implementation?
- Should terminal diagnostics be part of future durable audit, or stay ephemeral until RFC-012?
- How much terminal sequence support is required before the GUI terminal can be considered usable?

## Implementation Handoff Checklist

- Start with model/policy types before renderer integration.
- Keep parser/display model effects terminal-local.
- Add tests for every non-claim as well as every allowed capability.
- Create review request packages after each design or implementation checkpoint.
