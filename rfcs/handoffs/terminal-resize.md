---
title: "Terminal resize: implementation handoff"
owning_rfcs: "RFC-017 (renderer, grid), RFC-008 (PTY lifecycle)"
status: "Implementation complete 2026-08-17, pending review — review request 243"
created: "2026-08-17"
---

# Make the terminal the size of the pane it is drawn in

## What is wrong

Every terminal is permanently **24×80**, whatever the window size. `ROWS` and `COLS`
(`surface/terminal.rs:143-144`) are constants, and `runtime::terminal::launch::resize` — which
exists, works, and is tested — has **no caller anywhere in production**.

A user with a large window gets a small terminal, and `vim`, `less` or anything full-screen
renders into 24×80 forever.

## The trap, and it is the whole job

**Today's correctness comes from everything being the same constant.** Three things must
agree about the terminal's size:

1. The **PTY**'s size, set via `TIOCSWINSZ` — what the child process believes.
2. The **emulator**'s grid (`alacritty_terminal`'s `Term`) — what parses and stores output.
3. The **rendered** cell count — what the user sees.

Right now all three derive from `ROWS`/`COLS`, so they cannot disagree. That is precisely why
the audit's finding is *not* a corruption bug today — I checked, and said so in the table.

**Making the size dynamic re-opens that.** Three independently-updated values that must stay
identical is where the corruption bug would actually be introduced — output wrapping at the
wrong column, a cursor in the wrong place, full-screen programs drawing outside the visible
area.

So the real requirement is not "call `resize`." It is: **one computed size, applied to all
three, with no path that updates one without the others.** Prefer a shape where that is
structural rather than remembered — one function that returns the dimensions and three call
sites that cannot be reached independently, or equivalent.

## Scope

1. **Compute grid dimensions from the pane's real size and font metrics.** `font_metrics.rs`
   already measures glyph advance (PR-017-E) and `layout.rs` already reasons about available
   width — reuse both rather than adding a third notion of size.
2. **Apply to all three**: `Term`, the renderer, and the PTY via the existing `resize`.
3. **React to the window changing size**, not only to launch.

## Review gate

- **The three sizes proven to agree** after a resize, not asserted. A test that resizes and
  then checks the PTY's reported size against the emulator's grid against the rendered cell
  count — all three, not a sample of two.
- **Ablate it**: update one of the three and not the others, show the specific divergence.
  That is the bug this slice risks introducing, so it is the one that must fail loudly.
- **Real output across a resize**: a child writing wide text before and after, showing it
  wraps at the new column count and not the old one. Against a real PTY child, not a
  synthesised stream.
- **A resize storm does not produce a syscall storm.** Dragging a window edge emits many
  geometry changes; `TIOCSWINSZ` on every frame is a self-inflicted load problem. State what
  bounds it — coalescing, a threshold, or the fact that geometry only changes on discrete
  events — and prove it rather than assuming.
- **`SIGWINCH` is not yours to send.** The kernel delivers it to the foreground process group
  when `TIOCSWINSZ` changes the size. If you find yourself signalling the child, stop and
  raise it.
- **Minimum size honoured.** A pane too small for a usable grid must clamp rather than
  producing a zero or negative dimension. Say what the floor is and why.

## Out of scope

- **Reflow of existing scrollback.** Whether `alacritty_terminal` reflows already-parsed lines
  on resize is its behaviour, not ours to implement. Report what it does; do not build around
  it.
- **Per-pane independent sizing beyond what the existing split policy already decides.**
- **The `resize` function itself.** It exists and is tested; this slice gives it a caller.

## What this does not fix

`set_resource_limits` is the audit's other standout and stays with RFC-023. This slice is
terminal geometry only.
