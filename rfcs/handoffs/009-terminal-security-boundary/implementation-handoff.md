---
title: "RFC-009: Terminal Security Boundary — Implementation Handoff"
rfc: "RFC-009"
rfc_file: "../../done/009-terminal-security-boundary.md"
status: "Implemented with documented limitations"
target_milestone: "M4"
source_rfc_status: "Implemented with documented limitations"
created: "2026-07-11"
---

# RFC-009: Terminal Security Boundary — Implementation Handoff

## Purpose

This handoff translates RFC-009 into developer-facing guidance for implementing Tekstide's terminal output/input security boundary.

This handoff does not authorize AgentRun launch, transcript retention, durable audit storage, managed command approval, or the final desktop GUI terminal widget.

## Source RFC Summary

RFC-009 treats terminal output bytes as untrusted content behind a parser/display model. Terminal output may update only terminal-local cells, cursor state, terminal-local modes, bounded scrollback, and bounded diagnostics. It must not mutate app chrome, trust state, approvals, clipboard, command history, file buffers, project metadata, or durable audit state.

Paste handling must classify bytes before PTY write. Multiline paste and control-containing paste require explicit blocked/confirmation states before any bytes reach the terminal process.

## Dependencies and Sequencing

- Target milestone: **M4**
- Source RFC status: **Implemented with documented limitations**
- Required predecessors: RFC-004, RFC-007, RFC-008.
- RFC-009 should be reviewed before implementation begins.
- RFC-010 AgentRun launch should not claim terminal security guarantees until this boundary has implementation evidence.
- RFC-009 design/handoff review was accepted with notes on 2026-07-11.

## Implementation Boundaries

Keep the boundary split into small roles:

- parser/policy input: untrusted PTY bytes;
- parser/display event output: bounded terminal-local events;
- terminal surface model: cells, cursor, style, terminal-local modes, diagnostics;
- paste policy: typed/paste classification before PTY write;
- app/security UI: trusted state outside terminal output control.

Do not let terminal output write directly to:

- trust state;
- approval decisions;
- clipboard;
- command history outside the PTY;
- Project Board state;
- file buffers;
- durable audit storage;
- window/app chrome.

## Suggested Module Shape

Use existing module boundaries where possible. A likely shape inside `tekstide-core` is:

- `runtime::terminal::security` for policy types and parser/security diagnostics;
- `runtime::terminal::input` for paste classification and write decisions if the module grows;
- `runtime::terminal::display` for terminal-local display events if parser output becomes large.

Do not split files only for aesthetics. Split when parser policy, paste policy, or display model code becomes large enough that tests/readability suffer.

## Data Model Guidance

Prefer explicit enums over booleans:

- paste source: typed, single-line paste, multiline paste, control-containing paste;
- paste decision: allow, block, requires confirmation;
- unsupported sequence action: drop, render inert, diagnostic;
- security diagnostic kind: clipboard sequence blocked, app-title sequence blocked, unsupported private mode, invalid bytes;
- trusted UI state: no dialog, approval active, paste confirmation active, destructive decision active.

Diagnostics must be bounded and should record policy reason or sequence family, not raw private output.

Review carry-forward requirements:

- PR-009-B must enumerate exact accepted CSI, OSC, C0/C1/control, private-mode, terminal-query, and inert sequence families before claiming parser coverage.
- Terminal-generated replies, including device status reports, cursor position reports, terminal identity replies, size reports, focus/mouse reports, and keyboard protocol replies, are blocked by default unless explicitly implemented as bounded terminal-local capabilities.
- Paste blocking must use trusted UI active/modal state, not focus state alone.
- Diagnostics must include enough sequence-family and policy-reason metadata for QA/support without raw OSC payloads, pasted text, shell output, or environment-like values.

## Non-Claims

The implementation must not claim:

- full terminal emulator compatibility;
- complete ANSI/VT safety beyond reviewed sequence families;
- command approval for Plain/Supervised sessions;
- transcript privacy or retention behavior;
- durable audit persistence;
- VM/container-grade isolation;
- final GUI spoofing resistance before GUI evidence exists.

## Review Expectations

Every slice should include a review request before closeout. Reviewers should be able to inspect:

- exact supported and blocked sequence families;
- tests proving blocked sequences do not mutate trusted app state;
- paste bytes withheld before confirmation;
- spoofing attempts rendered as terminal content only;
- label wording that keeps Plain/Supervised/Managed promises honest.
