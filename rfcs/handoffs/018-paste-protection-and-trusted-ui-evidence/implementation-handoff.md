---
title: "RFC-018: Rendered Paste Protection and Trusted-UI Evidence - Implementation Handoff"
rfc: "RFC-018"
rfc_file: "../../done/018-paste-protection-and-trusted-ui-evidence.md"
status: "Accepted 2026-08-08 — ready for implementation"
target_milestone: "M9"
created: "2026-08-08"
---

# What exists, what is missing, and where the seams are

## The short version

RFC-009 built the model. `tekstide-core` implements it. Nothing calls it. Your job is the call sites and the rendering.

## Already built, in `tekstide-core`

| Item | Where | State |
| --- | --- | --- |
| `TerminalInputPolicy::evaluate` | `runtime/terminal/security/paste.rs` | complete, tested, **no production caller** |
| `TerminalInputDecision` (`Allow`/`RequiresConfirmation`/`Block`) | same | complete |
| `TerminalPasteClass` (`Empty`/`SingleLine`/`Multiline`/`ControlContaining`) | same | complete |
| `TerminalTrustedUiState` (5 variants, `is_active_or_modal()`) | same | complete, **no shell-side source** |
| `TerminalTrustedUiBoundary::assess_terminal_output` | `security/trusted_ui.rs` | complete, **no production caller** |
| `TerminalSpoofingAssessment`, `can_mutate_trusted_ui()` | same | complete |
| `paste_blocked` in the frozen v1 schema | `audit/record.rs`, `valid_paste_blocked` | schema present, **no producer** |
| `AuditReasonCode::PastePolicy` | `audit/` | present |

All re-exported from `tekstide_core::runtime::terminal`.

## Genuinely missing

1. **Any clipboard path in `crates/tekstide`.** `grep -rn "paste\|clipboard" crates/tekstide/src` returns nothing. `input/` handles keys; `TextStream::to_pty_bytes` maps keystrokes to bytes with a `_ => None` fallback. This is a new input class, not a handler to wire up.
2. **A shell-side source for `TerminalTrustedUiState`.** It must be derived from `state.modal`. Keep the derivation in one function — RFC-022's approval dialog becomes a second contributor to the same state.
3. **A real dialog.** `layer_composition_demo_modal` is a *demo of the modal layer*, not a reusable dialog. The layer underneath it is real and proven; the demo content is not the thing to extend.
4. **A `paste_blocked` producer.** `AuditCoordinator` has the pattern from `record_plain_terminal_started`/`_terminated`.

## Seams you should reuse rather than rebuild

**The single PTY ingress.** `shell::update`'s `RoutedInput::Terminal` arm, gated on `state.modal.is_none()` and `terminal_stream_targets_a_live_terminal`, then `pane.write_input(&bytes)`. Everything that reaches a PTY goes through here. Paste joins it; it does not get its own.

**The modal layer.** `State::modal`, `SubscriptionMode::for_modal`, `modal_subscription()`, and the `stack![base, opaque(center(...))]` composition in `view()`. Modal exclusivity is **structural**: while a modal is open, `non_modal_subscription` is not called at all, so terminal input is *not produced* rather than produced-and-discarded. The `is_none()` check at the write site is defence in depth behind that, and both are independently tested. Preserve both; neither substitutes for the other.

**The catalog.** Every user-facing word goes through `Catalog`. The session bar's `session-bar-entry` is the pattern for a message with selectors: one Fluent message with a select expression, not several lookups concatenated in Rust — concatenation hardcodes English word order even when every word is catalogued. `slot_symbol`/`status_symbol` show the shape: the Rust side names a branch, the `.ftl` file supplies the words.

**`text_safety::quote_untrusted`.** `surface/board.rs:135` is the live example. There is a real bidi-override project name in this project's own recent-projects state that exercises it.

**The audit producer pattern.** `record_plain_terminal_started` in `audit/integration.rs`, called through `AuditCoordinator::new(store, health)`, status discarded at the call site — an audit write failing must never fail the operation it observes.

**The screenshot convention.** `niri msg action screenshot-window --id <id> --path <repo-relative-file>`, committed under `evidence/pr-018-*/`, each stating what it proves **and does not**. Synthetic input: relaunch with `WAYLAND_DISPLAY` unset (niri does not forward XTest to native Wayland clients), `xdotool windowfocus` not `windowactivate`, and **always `--clearmodifiers`** — X11 modifier state is global to the connection, so a chord immediately before a plain key can deliver it with a modifier still down and the keystroke vanishes silently.

**The scratch-launch guard.** `.git-exclude/tools/launch-scratch-gui.sh` derives `XDG_STATE_HOME` from a scratch root rather than trusting a passed variable, and refuses to start if it would resolve to the real state directory. **Use it for every GUI run.** It exists because a hand-assembled `env` command dropped the variable on a retry and wrote to real desktop state.

## Things that will bite

**Clipboard content is untrusted, arbitrary-length, and attacker-influenced.** A user can be socially engineered into copying something. Bound what you read and what you render; a 10 MB clipboard should not become a 10 MB dialog or a 10 MB PTY write.

**`ControlContaining` is the class that matters most.** A paste containing control bytes can carry an embedded newline that executes a second command, which is the whole reason RFC-009 says *"paste is not typing."* It blocks outright — not "confirms."

**The dialog is a security surface.** It is the first thing this product asks a user to *trust*. Everything about how it is distinguished from terminal content is PR-018-E's evidence, and PR-018-C must not build a layout that makes that proof impossible — the same obligation RFC-017 carried and discharged.

**Do not improve terminal performance while you are here.** The 10 ms `WouldBlock` sleep, the 64 KiB cap, the three-terminal limit and `NFR-PERF-004` are one coupled change owned by readiness-driven terminal I/O (`../../future-work.md`). Fixing the sleep alone converts a throughput cap into silent mid-stream truncation. Leave it.
