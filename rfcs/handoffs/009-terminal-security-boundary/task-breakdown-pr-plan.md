---
title: "RFC-009: Terminal Security Boundary — Task Breakdown / PR Plan"
rfc: "RFC-009"
rfc_file: "../../proposed/009-terminal-security-boundary.md"
status: "Accepted for implementation"
target_milestone: "M4"
source_rfc_status: "Proposed"
created: "2026-07-11"
---

# RFC-009: Terminal Security Boundary — Task Breakdown / PR Plan

## Planning Assumptions

- RFC-009 starts after RFC-008 closeout is accepted.
- The first implementation target is model/policy code in `tekstide-core`.
- The final GUI terminal widget is not required for RFC-009 model acceptance.
- Terminal output bytes are untrusted.
- AgentRun launch, transcript retention, durable audit, and managed command approval remain out of scope.

## PR Sequence Overview

- PR-009-A: Terminal security policy model and diagnostics.
- PR-009-B: ANSI/VT/OSC supported-subset parser boundary.
- PR-009-C: Paste classification and pre-PTY write decisions.
- PR-009-D: Trusted UI/spoofing boundary model and label checks.
- PR-009-E: Closeout evidence and future-work alignment.

## PR-009-A — Security Policy Model

Purpose:

- Establish the vocabulary for terminal-local effects, blocked effects, and bounded diagnostics.

Developer tasks:

- Add policy enums/types for terminal display effects and blocked app-level effects.
- Add diagnostics for blocked clipboard/title/private sequences without storing private output.
- Add tests that diagnostics are bounded.
- Add tests that policy output cannot represent app-chrome/trust/approval mutation.

Review focus:

- Terminal output effects are terminal-local.
- Diagnostics do not store raw private output.
- The model does not imply final GUI behavior.

## PR-009-B — ANSI/VT/OSC Parser Boundary

Purpose:

- Convert PTY bytes into a conservative terminal-local event stream.

Developer tasks:

- Support printable UTF-8, invalid-byte replacement, basic control characters, SGR style, cursor movement, and clear operations.
- Block or render inert OSC 52, app/window title mutation, desktop notification, auto-open hyperlink, and unsupported private/control sequence families.
- Enumerate exact accepted and inert CSI, OSC, C0/C1/control, private-mode, terminal-query, and terminal-reply families before claiming parser coverage.
- Block terminal-generated replies by default unless a bounded terminal-local reply is explicitly implemented with tests.
- Add tests proving blocked sequences cannot mutate clipboard, app title, trust state, approvals, command history, file buffers, or project metadata.

Review focus:

- Supported subset is explicit.
- Unsupported behavior is inert.
- No product claim of full terminal emulator correctness appears.

## PR-009-C — Paste Policy

Purpose:

- Intercept paste before bytes reach the PTY.

Developer tasks:

- Classify typed input, single-line paste, multiline paste, and control-containing paste.
- Return allow/block/requires-confirmation decisions.
- Ensure multiline paste bytes are withheld until explicit confirmation.
- Reject wrong-project or wrong-terminal paste routing.
- Block or queue paste while trusted approval/security dialog state is active or modal, independent of focus state.

Review focus:

- Paste is not treated as typing.
- Confirmation-required bytes do not reach the PTY.
- Policy works without final GUI dialogs.

## PR-009-D — Trusted UI / Spoofing Boundary

Purpose:

- Model the separation between terminal content and trusted app/security decisions.

Developer tasks:

- Add trusted UI activity state needed by paste/input policy.
- Add tests using terminal output that mimics approval/paste/security dialogs.
- Verify such output remains terminal content only.
- Add label/read-model tests where useful so Plain/Supervised/Managed wording does not overclaim command approval.

Review focus:

- Spoofing boundary is structural, not based on trying to detect scary text.
- Terminal output cannot synthesize decisions.
- Plain terminal labels remain honest.

## PR-009-E — QA Evidence and Closeout

Purpose:

- Convert RFC-009 implementation into reviewed milestone evidence.

Developer tasks:

- Run formatting, clippy, workspace checks, and relevant tests.
- Record supported and blocked sequence families.
- Record paste policy evidence.
- Record spoofing-boundary evidence.
- Record security/privacy and migration notes.
- Record known limitations and GUI follow-up requirements.

Review focus:

- Evidence supports every accepted RFC-009 claim.
- Deferred GUI, AgentRun, transcript, audit, and command approval scope remains visible.
- RFC-010 can proceed without inheriting ambiguous terminal-security promises.

## Suggested Review Gates

1. **Design gate:** RFC-009 and handoff accepted.
2. **Policy gate:** terminal-local effect model and bounded diagnostics reviewed.
3. **Parser gate:** supported/blocked ANSI/VT/OSC subset reviewed.
4. **Paste gate:** pre-PTY paste policy reviewed.
5. **Spoofing gate:** trusted UI boundary and honest labels reviewed.
6. **Closeout gate:** evidence package and known limitations accepted.

## Stop Conditions

Pause and request RFC amendment or design review if:

- implementation needs a final GUI renderer to define the model boundary;
- full emulator compatibility becomes a prerequisite;
- exact accepted/inert sequence families cannot be enumerated for the parser slice;
- terminal-generated replies need to expose app, project, clipboard, trust, approval, or private terminal state;
- paste bytes must be written before confirmation for required behavior;
- blocked sequences need to mutate clipboard/app state;
- terminal output needs to write directly to trust, approval, audit, file, or Project Board state;
- command approval claims become necessary for Plain or Supervised sessions;
- raw private terminal output would need to be stored in diagnostics.
