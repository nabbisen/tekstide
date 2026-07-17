# RFC-009: Terminal Security Boundary — QA Evidence

Status: Accepted with documented limitations
Date opened: 2026-07-11
Date accepted: 2026-07-17

## Scope

RFC-009 defines Tekstide's terminal security boundary for untrusted terminal output and terminal input. Evidence must not be used to claim AgentRun launch, transcript retention, durable audit storage, managed command approval, final GUI terminal behavior, or full terminal emulator compatibility unless later reviewed implementation explicitly supports those claims.

## Design Review

RFC-009 design/handoff review was accepted with notes on 2026-07-11 in `.git-exclude/reviewed/tekstide-review-request-047-rfc009-terminal-security-boundary-design-response.md`.

RFC-009 closeout review was accepted with notes on 2026-07-17 in `.git-exclude/reviewed/tekstide-review-request-055-rfc009-closeout-evidence-response.md`.

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

Status: ready for implementation review.

Implementation:

- Added `crates/tekstide-core/src/runtime/terminal/security/paste.rs`.
- Exported paste policy types from `runtime::terminal`:
  - `TerminalInputPolicy`
  - `TerminalInputSource`
  - `TerminalPasteClass`
  - `TerminalInputDecision`
  - `TerminalInputDecisionReason`
  - `TerminalTrustedUiState`
- Added typed-input vs paste-input classification.
- Added paste classes:
  - empty;
  - single-line;
  - multiline;
  - control-containing.
- Added pre-PTY write decisions:
  - typed input is allowed;
  - empty/single-line paste is allowed;
  - multiline paste requires confirmation;
  - control-containing paste is blocked;
  - wrong-project and wrong-terminal routing are blocked;
  - paste is blocked while trusted UI is active or modal.

Security/privacy notes:

- `TerminalInputDecision` returns metadata only: source, paste class, byte count, and decision reason.
- `RequiresConfirmation` does not carry pasted bytes, so confirmation-required content is withheld from PTY write by the policy result.
- Single-line paste decisions do not store raw pasted content.
- C0, DEL, and C1 control bytes are classified as control-containing paste and blocked.
- Non-control binary bytes are currently classified by line/control policy; if they contain no blocked controls or line breaks, they are treated as single-line paste.
- Active/modal trusted UI state is explicit through `TerminalTrustedUiState`; paste blocking does not depend on focus state.
- This slice introduces no GUI dialog, spoofing examples, terminal renderer behavior, AgentRun launch, transcript storage, durable audit persistence, or command approval.

Observed gates on 2026-07-17:

- `cargo fmt --all --check` passed.
- `cargo test -p tekstide-core runtime::terminal::security` passed; 25 security/parser/paste tests passed.
- `cargo check --workspace` passed.
- `cargo test -p tekstide-core runtime::terminal::tests` passed; 16 terminal runtime tests passed.
- `cargo test -p tekstide-core` passed; 219 tests passed, 0 failed, 0 ignored; doc tests had 0 tests.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.

Review follow-up:

- `.git-exclude/reviewed/tekstide-review-request-052-rfc009-pr009c-paste-policy-response.md` requested changes because C1 control bytes could be classified as single-line paste.
- `classify_paste` now treats `0x80..=0x9f` as control-containing paste.
- Regression tests cover representative C1 DCS (`0x90`), CSI (`0x9b`), and OSC (`0x9d`) paste bytes.
- A regression test documents the current non-control binary behavior.
- `.git-exclude/reviewed/tekstide-review-request-053-rfc009-pr009c-paste-policy-rereview-response.md` accepted PR-009-C with notes.

Known limitations:

- PR-009-C does not implement rendered paste confirmation UI.
- PR-009-C does not write, queue, or replay paste bytes after confirmation.
- PR-009-C does not provide spoofing examples or GUI evidence; PR-009-D and later GUI milestones own those.
- PR-009-C does not introduce transcript storage, durable audit persistence, AgentRun launch, command approval, or GUI terminal behavior.

### PR-009-D — Trusted UI / Spoofing Boundary

Status: ready for implementation review.

Implementation:

- Added `crates/tekstide-core/src/runtime/terminal/security/trusted_ui.rs`.
- Exported trusted UI/spoofing boundary types from `runtime::terminal`:
  - `TerminalTrustedSurfaceKind`
  - `TerminalTrustedUiEffect`
  - `TerminalTrustedUiBoundary`
  - `TerminalSpoofingAssessment`
  - `TerminalOutputContentClass`
  - `TerminalSecurityLabelView`
- Added trusted surface kinds for approval, trust, paste confirmation, destructive decision, and security dialogs.
- Added trusted UI effect vocabulary for decisions and app-owned state that terminal output must not synthesize:
  - focus movement;
  - dismiss;
  - approve;
  - reject;
  - trust workspace;
  - Project Board state mutation;
  - trusted chrome mutation.
- Added `TerminalTrustedUiBoundary::assess_terminal_output`, which classifies parsed terminal effects as `UntrustedTerminalContent` and returns no trusted UI effect.
- Added `TerminalSecurityLabelView`, derived from `AiSessionSecurityLevel`, so Plain/Supervised/Managed labels reuse the existing RFC-004-compatible wording and command-approval claim rules.

Security/privacy notes:

- The spoofing boundary is structural. It does not try to detect "scary" terminal text or approval-looking words.
- Terminal output that looks like approval, trust, paste confirmation, destructive decision, or security dialog text remains terminal content only.
- Terminal output cannot synthesize approve/reject/trust decisions through the `TerminalSpoofingAssessment` model.
- Terminal output cannot mutate trusted chrome, Project Board state, or trusted-dialog focus through the trusted UI effect model.
- Plain and Supervised labels continue to report `Managed command approval not guaranteed`.
- Managed labels report only `Managed command approval eligible`; this remains an eligibility label and does not introduce command approval.
- This slice introduces no rendered GUI dialog, terminal renderer screenshot evidence, AgentRun launch, transcript storage, durable audit persistence, or command approval.

Observed gates on 2026-07-17:

- `cargo fmt --all --check` passed.
- `cargo test -p tekstide-core runtime::terminal::security` passed; 29 security/parser/paste/spoofing tests passed.
- `cargo check --workspace` passed.
- `cargo test -p tekstide-core runtime::terminal::tests` passed; 16 terminal runtime tests passed.
- `cargo test -p tekstide-core` passed; 223 tests passed, 0 failed, 0 ignored; doc tests had 0 tests.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.

Known limitations:

- PR-009-D does not implement rendered trusted dialogs.
- PR-009-D does not provide GUI screenshot evidence or terminal-widget visual spoofing evidence.
- PR-009-D does not wire focus/window-manager behavior; it only establishes that terminal output cannot produce trusted UI effects in the core boundary model.
- PR-009-D does not introduce AgentRun launch, transcript storage, durable audit persistence, or command approval.

### PR-009-E — Closeout Evidence

Status: ready for closeout review.

Committed slice list:

- `7152024 docs: add RFC-009 terminal security boundary`
- `7e2dd78 feat: add terminal security policy model`
- `3d481fe feat: add terminal parser security boundary`
- `2f3b54a feat: add terminal paste input policy`
- `978e688 Implement RFC-009 trusted UI spoofing boundary`

Implemented foundation:

- Terminal output bytes are treated as untrusted input behind a conservative parser/security boundary.
- Terminal parser output is limited to terminal-local surface effects and bounded diagnostics.
- Exact accepted and inert sequence families are enumerated in `TerminalSequencePolicy`.
- OSC 52 clipboard, OSC title, OSC 8 hyperlink, unsupported OSC, DCS, PM, APC, private modes, mouse/focus reporting, keyboard protocol, terminal queries, terminal-generated replies, unsupported CSI, unknown ESC, unsupported C0, C1, and invalid bytes are inert/diagnostic by policy.
- Terminal-generated replies are blocked by default unless a future reviewed capability enables a bounded terminal-local reply.
- Terminal security diagnostics include sequence-family and policy-reason metadata without raw terminal output, OSC payloads, pasted text, shell output, or environment-like values.
- Paste input is distinguished from typed input before PTY write.
- Paste policy classifies empty, single-line, multiline, and control-containing paste.
- Multiline paste returns `RequiresConfirmation` before bytes are written to the PTY.
- C0, DEL, and C1 control-containing paste is blocked.
- Paste routing is addressed by `ProjectId` and `TerminalId`; wrong-project and wrong-terminal routing is rejected.
- Paste is blocked while trusted UI is active or modal and does not depend only on focus state.
- Approval/trust/paste/destructive/security-looking terminal output remains terminal content only in the core boundary model.
- Plain/Supervised/Managed labels remain honest and do not claim managed command approval for Plain or Supervised sessions.

Security/privacy note:

- RFC-009 introduces no transcript persistence, durable audit storage, AgentRun launch, command approval, final GUI terminal widget, final rendered dialogs, or cross-platform GUI security claim.
- Parser diagnostics and paste/input decisions store metadata only; they do not store raw terminal payloads or pasted text.
- The trusted UI/spoofing boundary is structural at the model layer. It does not rely on semantic detection of approval-looking words.
- The model-level spoofing boundary does not replace later GUI renderer evidence for focus, overlap, modality, screenshots, or native/app dialog containment.

Migration note:

- No local data schema or persisted state migration is introduced.
- RFC-009 adds in-memory policy/parser/input/read-model types only.
- No transcript bytes, terminal output bytes, pasted bytes, process handles, process ids, audit records, or trust decisions are persisted by this RFC.

Known limitations:

- No final GUI terminal renderer or terminal widget acceptance.
- No rendered trusted dialogs or screenshot-based spoofing evidence.
- No focus/window-manager integration.
- No app/UI command path that applies the paste policy to real user paste events.
- No paste confirmation UI, paste queue, or post-confirmation replay path.
- No full terminal emulator compatibility.
- No enabled terminal-generated replies.
- No hyperlink open/copy UI capability.
- No AgentRun launch, AI CLI profile execution, transcript retention, durable audit storage, or command approval.
- No macOS/Windows GUI security evidence.

Future-work alignment:

- RFC-010 should build AgentRun launch on top of this boundary without claiming command approval for Plain/Supervised sessions.
- RFC-011 should define transcript retention before serious AgentRun transcript capture.
- RFC-012 should define durable audit storage before security decisions become persistent audit claims.
- The GUI terminal milestone must add renderer/dialog/focus/screenshot evidence before claiming final visual spoofing resistance.
- Future terminal capability increments must document exact sequence family, terminal-local effect, cross-platform behavior, and tests before enabling currently inert behavior.

Observed closeout gates on 2026-07-17:

- `cargo fmt --all --check` passed.
- `cargo check --workspace` passed.
- `cargo test -p tekstide-core runtime::terminal::security` passed; 29 security/parser/paste/spoofing tests passed.
- `cargo test -p tekstide-core runtime::terminal::tests` passed; 16 terminal runtime tests passed.
- `cargo test -p tekstide-core` passed; 223 tests passed, 0 failed, 0 ignored; doc tests had 0 tests.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `git diff --check` passed.

## Recommendation

PR-009-E is ready for closeout review. Recommend accepting RFC-009 as implemented with documented limitations.
