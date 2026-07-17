# RFC-009: Terminal Security Boundary — QA Evidence

Status: PR-009-B implementation ready for review
Date opened: 2026-07-11

## Scope

RFC-009 defines Tekstide's terminal security boundary for untrusted terminal output and terminal input. Evidence must not be used to claim AgentRun launch, transcript retention, durable audit storage, managed command approval, final GUI terminal behavior, or full terminal emulator compatibility unless later reviewed implementation explicitly supports those claims.

## Design Review

RFC-009 design/handoff review was accepted with notes on 2026-07-11 in `.git-exclude/reviewed/tekstide-review-request-047-rfc009-terminal-security-boundary-design-response.md`.

Carry-forward requirements:

- PR-009-B must pin exact accepted and inert sequence families before claiming parser coverage.
- Terminal-generated replies must be blocked by default or implemented as bounded terminal-local capabilities with tests.
- Paste blocking must use active/modal trusted UI state, not focus alone.
- Diagnostics must include sequence-family and policy-reason metadata without raw private payloads.

## Implementation Evidence

### PR-009-A — Security Policy Model

Status: ready for implementation review.

Implementation:

- Added `crates/tekstide-core/src/runtime/terminal/security.rs`.
- Exported terminal security policy model types from `runtime::terminal`.
- Added terminal-local display effect vocabulary:
  - `TerminalSurfaceEffect`
  - `TerminalTextEffect`
  - `TerminalCursorEffect`
  - `TerminalStyleEffect`
  - `TerminalModeEffect`
  - `TerminalScrollbackEffect`
- Added blocked app-level effect vocabulary:
  - `TerminalBlockedAppEffect::ClipboardAccess`
  - `TerminalBlockedAppEffect::AppChromeMutation`
  - `TerminalBlockedAppEffect::TrustedUiMutation`
  - `TerminalBlockedAppEffect::TrustStateMutation`
  - `TerminalBlockedAppEffect::ApprovalStateMutation`
  - `TerminalBlockedAppEffect::CommandHistoryMutation`
  - `TerminalBlockedAppEffect::AuditStateMutation`
  - `TerminalBlockedAppEffect::FileStateMutation`
  - `TerminalBlockedAppEffect::ProjectMetadataMutation`
  - `TerminalBlockedAppEffect::HostIntegration`
  - `TerminalBlockedAppEffect::TerminalGeneratedReply`
- Added bounded diagnostic model:
  - `TerminalSecurityDiagnostic`
  - `TerminalSequenceFamily`
  - `TerminalPolicyReason`

Security/privacy notes:

- Diagnostics retain sequence-family, policy-reason, and payload byte count metadata.
- Diagnostics use `BoundedRuntimeSummary` for bounded human-readable summaries.
- `blocked_sequence` / `blocked_app_effect` constructors do not accept raw payload text, which keeps OSC payloads, pasted text, shell output, and environment-like values out of summaries.
- The free-form diagnostic summary helper is private to the module; public diagnostic constructors derive summaries from non-payload metadata.
- This slice introduces no parser, paste router, PTY write behavior, transcript storage, durable audit persistence, AgentRun launch, command approval, or GUI terminal behavior.

Observed gates on 2026-07-11:

- `cargo fmt --all --check` passed.
- `cargo test -p tekstide-core runtime::terminal::security` passed; 6 security policy model tests passed.
- `cargo check --workspace` passed.
- `cargo test -p tekstide-core runtime::terminal::tests` passed; 16 terminal runtime tests passed.
- `cargo test -p tekstide-core` passed; 200 tests passed, 0 failed, 0 ignored; doc tests had 0 tests.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.

Review follow-up:

- `.git-exclude/reviewed/tekstide-review-request-048-rfc009-pr009a-security-policy-model-response.md` requested changes because public `TerminalSecurityDiagnostic::new` accepted arbitrary summary text.
- The raw-summary constructor was changed to private `with_summary`.
- Public regression tests now cover both public diagnostic constructors and prove raw payload strings are not stored in summaries.

Known limitations:

- PR-009-A does not parse PTY bytes.
- PR-009-A does not enumerate exact accepted/inert sequence grammar for parser coverage; PR-009-B owns that.
- PR-009-A does not implement terminal-generated reply handling; PR-009-B owns blocked/default reply behavior.
- PR-009-A does not classify or gate paste bytes before PTY write; PR-009-C owns that.
- PR-009-A does not model active/modal trusted UI state; PR-009-C/PR-009-D own that.
- PR-009-A does not provide spoofing examples or GUI evidence; PR-009-D and later GUI milestones own those.

### PR-009-B — ANSI/VT/OSC Parser Boundary

Status: ready for implementation review.

Implementation:

- Added `crates/tekstide-core/src/runtime/terminal/security/parser.rs`.
- Exported parser boundary types from `runtime::terminal`:
  - `TerminalSecurityParser`
  - `TerminalSequencePolicy`
  - `TerminalAcceptedSequence`
  - `TerminalInertSequence`
- Added exact accepted sequence-family enumeration before parser coverage is claimed:
  - printable UTF-8;
  - C0 carriage return;
  - C0 line feed;
  - C0 tab;
  - C0 backspace;
  - CSI SGR;
  - CSI cursor movement;
  - CSI clear line;
  - CSI clear screen.
- Added exact inert sequence-family enumeration:
  - invalid UTF-8;
  - unsupported C0 controls;
  - C1 controls;
  - OSC 52 clipboard;
  - OSC 8 hyperlink;
  - OSC title;
  - unsupported OSC;
  - DCS;
  - PM;
  - APC;
  - private modes;
  - mouse/focus reporting;
  - keyboard protocol;
  - terminal queries;
  - terminal-generated replies;
  - unsupported CSI;
  - unknown ESC.
- Added a conservative parser that converts accepted sequences into terminal-local `TerminalSurfaceEffect` values.
- Added inert diagnostics for unsupported/high-risk sequences using non-payload metadata only.
- Added default blocking for terminal-generated replies through `TerminalSecurityParser::block_terminal_generated_reply`.

Security/privacy notes:

- OSC 52 clipboard payloads are blocked as `Osc52Clipboard` / `ClipboardAccessBlocked`.
- OSC title mutation is blocked as `OscTitle` / `AppChromeMutationBlocked`.
- OSC 8 hyperlinks and unsupported OSC are blocked as host integration.
- DCS, PM, and APC are blocked as host integration.
- Private modes, mouse/focus reporting, keyboard protocol, terminal queries, and terminal-generated replies are blocked by default.
- Invalid UTF-8 is represented as `InvalidBytesReplaced` plus bounded `InvalidBytes` diagnostics.
- Diagnostics record sequence family, policy reason, and byte counts without raw OSC payloads, pasted text, shell output, or environment-like values.
- This slice introduces no paste classification, trusted UI active/modal state, spoofing examples, GUI terminal behavior, AgentRun launch, transcript storage, durable audit persistence, or command approval.

Observed gates on 2026-07-11:

- `cargo fmt --all --check` passed.
- `cargo test -p tekstide-core runtime::terminal::security` passed; 13 security/parser tests passed.
- `cargo check --workspace` passed.
- `cargo test -p tekstide-core runtime::terminal::tests` passed; 16 terminal runtime tests passed.
- `cargo test -p tekstide-core` passed; 207 tests passed, 0 failed, 0 ignored; doc tests had 0 tests.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.

Review follow-up:

- `.git-exclude/reviewed/tekstide-review-request-050-rfc009-pr009b-parser-boundary-response.md` requested changes because C1 string controls and over-cap blocked string controls could allow payload bytes to re-enter printable text parsing.
- C1 OSC/DCS/PM/APC now route to blocked string-control handling instead of one-byte C1 diagnostics.
- Blocked OSC/DCS/PM/APC recovery now consumes through the terminator when present, or consumes the remaining available blocked payload when no terminator is present.
- Regression tests cover C1 string controls, over-cap terminated string controls, over-cap unterminated string controls, and exact inert policy enumeration.

Known limitations:

- PR-009-B does not implement full terminal emulator compatibility.
- PR-009-B does not implement paste classification or pre-PTY write gating; PR-009-C owns that.
- PR-009-B does not model active/modal trusted UI state; PR-009-C/PR-009-D own that.
- PR-009-B does not provide spoofing examples or GUI evidence; PR-009-D and later GUI milestones own those.
- PR-009-B does not introduce transcript storage, durable audit persistence, AgentRun launch, command approval, or GUI terminal behavior.

### PR-009-C — Paste Policy

Status: pending.

Evidence to record:

- typed/single-line/multiline/control-containing paste classification;
- allow/block/requires-confirmation behavior;
- proof that confirmation-required bytes are withheld before PTY write;
- cross-project paste rejection;
- trusted UI active-state behavior;
- tests run.

### PR-009-D — Trusted UI / Spoofing Boundary

Status: pending.

Evidence to record:

- trusted UI state model;
- approval-like terminal-output spoofing examples;
- label/read-model checks;
- tests run.

### PR-009-E — Closeout Evidence

Status: pending.

Evidence to record:

- final implemented scope;
- known limitations;
- security/privacy note;
- migration note;
- future-work alignment;
- closeout recommendation.

## Recommendation

Request PR-009-B re-review before proceeding to PR-009-C.
