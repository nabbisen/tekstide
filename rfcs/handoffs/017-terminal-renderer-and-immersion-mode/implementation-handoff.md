---
title: "RFC-017: Terminal Renderer and Immersion Mode - Implementation Handoff"
rfc: "RFC-017"
rfc_file: "../../done/017-terminal-renderer-and-immersion-mode.md"
status: "Accepted 2026-08-01 — ready for implementation"
target_milestone: "M9"
created: "2026-08-01"
---

# RFC-017 Implementation Handoff

## 1. Module layout

```
crates/tekstide/src/
  surface/
    terminal.rs            the pane: renders the grid, emits messages
    terminal/filter.rs     vte::ansi::Handler impl -- interposition only, no policy
    terminal/session_bar.rs
  input/terminal_surface.rs  already exists (RFC-015); gains a real TerminalId
```

`tekstide-core` gains nothing. Its `runtime::terminal::security` already holds the policy, and the mechanical check for that is in PR-017-B's document.

The filter sits in the shell crate because implementing `vte::ansi::Handler` requires `vte`. **It is an adapter, not a decision-maker** — see §3.

## 2. The seams

| Seam | Direction | Rule |
| --- | --- | --- |
| PTY bytes → emulator | in | Exactly one path, through the filter (P1) |
| Emulator grid → renderer | out | Read-only; the pane renders, it does not own |
| Keystrokes → PTY | in | Only via `TextStream`, only when no modal is active |
| Session state | both | `tekstide-core` owns it; the pane holds no copy |
| Chrome text derived from output | out | Through `text_safety` — the grid exception does not extend here |

## 3. The filter is an adapter

RFC-009's classification lives in `tekstide-core::runtime::terminal::security` and is already reviewed. The shell-side filter's job is to route `vte::ansi::Handler` callbacks into it and act on the answer.

If you write a conditional in the shell crate that decides whether a sequence is *acceptable*, you have created a second classifier. Two classifiers disagree exactly at the inputs an attacker picks, and the disagreement is invisible until it matters.

**If core's classifier cannot express something interposition needs, raise it.** Extending core's policy is a reviewable change; duplicating it quietly is not.

## 4. `TextStream` gets its first real producer

RFC-015 PR-015-C built the three input classes and proved `TextStream`'s constructor is `pub(super)` — narrower than `pub(crate)` — specifically so a future surface module could not synthesize one. **This is that surface.** It was tested with `terminal_focus` hard-coded `None`; you supply the real value.

Three properties came with it and none are yours to relax:

- `TextStream` cannot address shell or modal state.
- While a modal is open, `SurfaceInput` and `TextStream` are **not produced** — not produced-and-discarded. The `ModalAbsent` gate enforces this at the call site.
- Global keybindings are matched before terminal focus, so a terminal cannot capture them.

The third is what stops a terminal becoming inescapable. The second is what stops a PTY write racing an approval dialog — and RFC-021's whole approval model rests on it, so verify it under a real terminal rather than trusting the headless proof.

## 5. Rendering the grid

The spike's `terminal_pane.rs` used `iced::widget::rich_text`/`Span` with per-cell colours from `Term::renderable_content()` — the same API a real renderer uses, deliberately not a shortcut. Start there.

**Bounded scrollback.** An unbounded buffer is a memory-exhaustion path driven by untrusted output. State the bound, test it under sustained output, and decide it together with the hidden-session question (both are in PR-017-E's gate for that reason).

**The grid renders as data, never as chrome.** Nothing drawn from PTY bytes may occupy, overlap, or imitate a region the user reads as Tekstide's own. RFC-018 proves this adversarially; do not build a layout that makes the proof impossible.

## 6. Layout and split policy

`TerminalPanePolicy`, `TerminalLayoutClass`, and `visible_terminal_limit` (default `2`) already exist in `tekstide-core::navigation` and `project::metadata`. Use them. RFC-015's "no shell-local shadow copy" rule applies.

Split from **real font metrics and DPI**, not fractions — the spike's `font_metrics.rs` is the approach. A split producing panes narrower than the minimum column count is refused, not rendered: it is a rendering bug that only appears on someone else's display scaling.

## 7. Audit

`plain_terminal_observation` exists in the frozen v1 schema with no producer. Wire it through `AuditCoordinator`, never directly to the store, and carry no command text, output, or path.

**The sentinel test probes raw bytes on disk**, not just the typed query — a typed query cannot see a value that leaked into a column nobody selected. RFC-021 PR-021-E2's sentinel is the shape.

**No schema amendment.** If the family genuinely cannot express what is needed, that is an RFC-013 amendment requiring owner authorisation and a migration — see RFC-013 Amendment 1 for what that costs.

## 8. Measurement

`NFR-PERF-004`: p95 ≤ 16 ms under bounded background output. Reuse `measurement.rs` from PR-015-F.

Three things that harness learned the hard way:

- **`iced::window::frames()` forces continuous redraw** and produced RFC-014's degenerate all-`0µs` figures. The input-to-state-change plus view-cost decomposition avoids the mechanism rather than working around it.
- **Non-contamination is proven per criterion**, by idle-CPU comparison, not inherited.
- **Stop on confirmed on-disk sample counts**, never dispatched ones. R9 (survivorship bias) is a standing finding.

The flood condition is the requirement, not a stress option. It is also where P4 failures surface.

## 9. What you may not claim

The terminal surface being real does **not** make trusted-UI separation demonstrated. That is RFC-018.

RFC-014 PR-014-D's genuine-versus-adversarial screenshot is the strongest artifact this project has on spoofing resistance and it **does not transfer** — it proved the spike's modal composited above the spike's terminal. Citing it for the product's boundary would be the overclaim pattern this project has caught repeatedly, in a place where it would matter.
