---
title: "RFC-015 PR-015-C: Input Routing - Detailed Developer Instructions"
rfc: "RFC-015"
rfc_file: "../../proposed/015-application-shell-and-rendered-surface-model.md"
target_milestone: "M8"
created: "2026-07-29"
updated: "2026-07-29"
---

# PR-015-C — Input Routing: Detailed Instructions

This slice gets its own document for the same reason PR-014-C did: it is a **security boundary**, and being subtly wrong here produces something that looks correct and is not.

`implementation-handoff.md` states the goal. This document states the property, how to make it structural, what will be tested at review, and one design trap that a reasonable implementation walks straight into.

## 1. The property

> While a modal dialog is open, no keystroke can reach a PTY, a surface, or shell navigation. And at no time can terminal-originated input address trusted state.

RFC-009:212 requires approval, trust, paste-confirmation, and destructive dialogs to be *rendered outside terminal output*. The RFC-014 spike proved that property — but **only because its terminal was output-only and emitted no messages at all** (residual risk R6). The moment the terminal accepts keystrokes, "no message exists that could reach trusted state" stops being free.

Your job is to make it true again, by type structure rather than by guard conditions.

## 2. The trap

The obvious implementation is one message enum with a guard:

```rust
enum Message {
    KeyPressed(Key),
    // ...
}

fn update(state: &mut State, message: Message) {
    match message {
        Message::KeyPressed(key) if state.modal.is_some() => modal_handle(key),
        Message::KeyPressed(key) if state.terminal_focused => pty_write(key),
        Message::KeyPressed(key) => shell_handle(key),
    }
}
```

**This is the trap.** It is correct today and fragile forever:

- Correctness depends on **guard ordering**. Reorder the arms and keystrokes reach the PTY while a dialog is open.
- A future `match` arm added above the modal guard silently bypasses it.
- Nothing prevents another code path from constructing `Message::KeyPressed` and dispatching it.
- The compiler cannot help. Every failure is a runtime behaviour question.

The spike's own focus trap was exactly this shape — `Message::FocusNext if state.terminal_mode && state.dialog_shown` — and I accepted it there **because terminal input did not exist**. It is not acceptable once it does.

## 3. Required structure

Make the three input classes **distinct types**, so that routing errors are compile errors rather than behaviour bugs.

```rust
/// Global navigation. Produced only from KeybindingPolicy matches.
pub enum ShellInput { /* ... */ }

/// Keyboard for the focused surface. Carries which surface it is for.
pub struct SurfaceInput { target: SurfaceId, /* ... */ }

/// Keystrokes destined for a PTY. Constructible ONLY by the terminal
/// surface's own input handler. Carries the target terminal.
pub struct TextStream { target: TerminalId, /* ... */ }
```

Required properties:

1. **`TextStream` has no constructor reachable from shell or modal code.** Private field, private constructor, produced inside the terminal surface module only. Rust's module privacy is the enforcement mechanism — use it rather than a convention.
2. **`TextStream` carries a `TerminalId`** and the router verifies it names a live terminal in the active project. A stale or cross-project id is dropped, not best-effort delivered.
3. **When a modal is active, `SurfaceInput` and `TextStream` are not produced at all.** Not produced-then-ignored. The subscription that would generate them is not subscribed. This is the difference between "we check" and "there is nothing to check."
4. **`ShellInput` always wins** and is never capturable by a surface, so `Ctrl+Esc` mode switching and Project Board access work even when a terminal has text focus.
5. **No surface may construct `ShellInput`.** Same privacy mechanism.

The test of whether you have done this right: **deleting a guard condition should cause a compile error, not a security regression.**

## 4. Routing precedence

```
Modal layer  >  Terminal surface (holding text focus)  >  Shell focus cycle
```

Exactly one sink is active. Not "one handles it first" — one *exists*.

Focus transitions:

- Opening a modal takes the sink immediately; any in-flight surface input is dropped, not queued for delivery after dismissal. **Queued keystrokes arriving after a dialog closes is a real hazard** — a user answering a dialog must not have those keys land in a shell afterwards.
- Dismissing a modal returns the sink to whatever held it before, and focus returns to the invoking element (UI/UX §18).
- Mode switching moves the sink; it never leaves two sinks live.

## 5. What I will probe at review

Published so you can build to it rather than be surprised.

- **Modal exclusivity:** open a modal, deliver keystrokes, assert nothing reaches a PTY or the shell focus cycle — and assert they are not delivered *after* dismissal either.
- **Type reachability:** attempt to construct `TextStream` from outside the terminal surface module. This must fail to compile; I will check that it does.
- **Global keybinding capture:** give a terminal text focus, send the mode-switch binding, assert the shell handles it.
- **Cross-project / stale `TerminalId`:** assert the router drops rather than delivers.
- **Guard-deletion resistance:** remove a guard condition and confirm the failure is a compile error rather than a silently permissive runtime path.

The last one is the real test of the design. If deleting a guard still compiles and merely changes behaviour, the structure has not been achieved and this slice is not done.

## 6. If the structure proves impossible

`iced`'s subscription model may resist producing genuinely distinct message types — for instance if all keyboard input must flow through one `Subscription<Message>`.

If so:

1. **Report it rather than falling back to guards silently.**
2. The acceptable fallback is a *single* routing function that is the only code able to convert a raw key event into one of the three classes, with the modal check inside it and no other conversion path anywhere. That is weaker than type separation but still a single enforcement point.
3. Document precisely what could not be achieved and why — this becomes an input to RFC-017, which inherits the property.

Do not adopt the guard-ordering shape in §2 without reporting it as a known weakness.

## 7. Deliverables checklist

- [ ] Three input classes exist as distinct types.
- [ ] `TextStream` unconstructible outside the terminal surface module — verified by a compile-fail check.
- [ ] `ShellInput` unconstructible by surfaces.
- [ ] Modal active ⇒ surface and text-stream input **not produced**.
- [ ] No post-dismissal delivery of keystrokes typed while a modal was open.
- [ ] Global keybindings not capturable by any surface.
- [ ] Stale or cross-project `TerminalId` dropped.
- [ ] Focus returns to the invoking element on modal dismissal.
- [ ] **Real focus-trap test**, not a structural argument — RFC-014 R6 explicitly requires this upgrade.
- [ ] Guard-deletion resistance demonstrated.
