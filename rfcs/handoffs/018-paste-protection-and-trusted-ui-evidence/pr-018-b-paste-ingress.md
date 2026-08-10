---
title: "RFC-018 PR-018-B — Paste ingress: implementation handoff"
rfc: "RFC-018"
rfc_file: "../../done/018-paste-protection-and-trusted-ui-evidence.md"
slice: "PR-018-B"
status: "Ready for implementation — accepted 2026-08-08 with the RFC"
created: "2026-08-08"
---

# PR-018-B — Paste ingress

This is the security-critical slice. Read it before writing code.

## What exists already, precisely

`tekstide-core::runtime::terminal::security` (re-exported from `runtime::terminal`) holds the whole model:

```rust
TerminalInputPolicy::evaluate(
    &self,
    target: &TerminalRuntimeHandle,
    active_terminal: Option<&TerminalRuntimeHandle>,
    source: TerminalInputSource,      // Typed | Paste
    bytes: &[u8],
    trusted_ui: TerminalTrustedUiState,
) -> TerminalInputDecision            // Allow | RequiresConfirmation | Block
```

`TerminalPasteClass` is `Empty | SingleLine | Multiline | ControlContaining`. `TerminalInputDecisionReason` covers `WrongProject`, `WrongTerminal`, `MultilinePasteRequiresConfirmation`, `ControlContainingPasteBlocked`, and `PasteBlockedByTrustedUi(state)`.

**It has no production caller.** Confirm that yourself before you start — `grep` for `TerminalInputPolicy` outside `security/`, `mod.rs`'s re-export, and tests. Knowing the starting state is what lets you claim you did not add a second one.

## What is genuinely missing

**A paste event.** `crates/tekstide/src/input/` handles keys only; `TextStream::to_pty_bytes` maps a keystroke to bytes and there is no clipboard path anywhere in the crate. `grep -rn "paste\|clipboard" crates/tekstide/src` returns nothing today, and that is the honest starting point: this is not "wire up an existing handler," it is a new input class.

RFC-018 §Open questions leaves the trigger to you: `Ctrl+Shift+V` is the terminal convention and does not collide with the `Ctrl+Alt+<letter>` family. **Check the whole `KeybindingPolicy::linux_mvp()` table mechanically**, not by eye — `Ctrl+Shift+P` is `Reserved` for the command palette, and `KeybindingStatus` exists so a collision is a test failure rather than a code review catch. PR-017-D's `launch_terminal_shortcut_is_a_candidate_that_collides_with_no_other_rule` is the pattern.

## The property this slice exists to protect

**One PTY ingress.** RFC-017 PR-017-B and PR-017-C spent two slices proving single-ingress for the *output* path, and re-proved it when production code first existed to violate it. This slice creates the first new *input* path since `TextStream`, so the same property has to hold on this side.

Concretely, today there is exactly one production `write_input` call site: `shell::update`'s `RoutedInput::Terminal` arm, guarded by `state.modal.is_none()` and `terminal_stream_targets_a_live_terminal`. **Paste bytes must reach the PTY through that same gate**, not through a second call that happens to look similar.

Two ways to get this wrong, both tempting:

- **A parallel `write_paste` on `TerminalPane`.** It would work, it would be readable, and it would be a second ingress that the modal gate does not cover unless someone remembers to add it.
- **Calling `write_input` from a new message arm.** Same problem one level up: two arms, two guards, and the second one drifts.

Prefer routing paste into the existing arm as a distinct `RoutedInput` variant carrying its policy decision, so the gate is passed once and the decision travels with the bytes.

**Enumerate the callers and ablate the enumeration.** `terminal_pane_launch_has_exactly_two_named_production_callers` is the shape: walk the source for every `write_input(` call site, resolve each one's enclosing function by name, and assert the exact expected set. A third call site must fail the test, not be caught by review.

## `RequiresConfirmation` blocks in this slice, and the closeout says so

PR-018-C builds the dialog. Until it exists there is nothing to render a confirmation into, so `RequiresConfirmation` **blocks and writes nothing**.

This is a deliberate, temporary, conservative state and it must be **visible**: state it in `qa-evidence.md`, and make sure the user is told rather than left wondering why their paste vanished. A multiline paste silently doing nothing is worse than a multiline paste refused with a reason.

**Do not** treat `RequiresConfirmation` as `Allow` "until the dialog lands." That is the single most likely way this RFC ships something unsafe, and it is why the sequencing is B → C strict rather than a suggestion.

## The trusted-UI state argument is not a placeholder

`evaluate` takes `trusted_ui: TerminalTrustedUiState`, and `PasteBlockedByTrustedUi` exists as a reason. That means **a paste attempted while a dialog is open is a case the policy already handles** — pass the real state, not `Inactive`.

Today `TerminalTrustedUiState` has no shell-side source, so you will need to derive it from `state.modal`. Keep that derivation in one function with the mapping stated, rather than inline at the call site: when RFC-022's approval dialog arrives it becomes a second contributor to the same state, and a scattered derivation is where that goes wrong.

## Review gate

- **The starting state confirmed**: `TerminalInputPolicy` had no production caller before this slice, shown by enumeration.
- **One PTY ingress**, enumerated mechanically and ablated — a synthetic second call site fails the test.
- **Modal exclusivity re-proven with a real paste**, against a real `TerminalPane`, not headless. A dialog open means no PTY write, demonstrated rather than argued, exactly as PR-017-D did for keystrokes.
- **No classification in `crates/tekstide`.** Every `Allow`/`Block`/`RequiresConfirmation` originates from `evaluate`.
- **Each `TerminalPasteClass` exercised** against real bytes: empty, single-line, multiline, and control-containing. Control-containing is the one that must block outright.
- **The real `TerminalTrustedUiState` is passed**, with the modal-to-state mapping in one place.
- **The keybinding collides with nothing**, checked against the whole table mechanically.
- **`RequiresConfirmation` blocks, visibly**, and the temporary state is recorded.
- Gates: `fmt`, `clippy -D warnings`, full suite, `git diff --check`.

## What this slice does not do

No dialog (PR-018-C). No `paste_blocked` audit producer (PR-018-D) — **do not write audit rows for blocks yet**, because the producer's shape is that slice's decision and a half-wired producer is harder to review than none. No trusted-UI evidence (PR-018-E).
