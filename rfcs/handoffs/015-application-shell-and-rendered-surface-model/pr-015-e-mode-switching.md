---
title: "RFC-015 PR-015-E — Mode switching and Content-mode scaffolding: implementation handoff"
rfc: "RFC-015"
rfc_file: "../../done/015-application-shell-and-rendered-surface-model.md"
slice: "PR-015-E"
release: "0.4.1"
status: "Ready for implementation — 0.4.0 shipped 2026-08-01"
created: "2026-08-01"
---

# PR-015-E — Mode switching, and the three things that come with it

`task-breakdown-pr-plan.md` lists two obligations for this slice. **It has four.** The other two arrived through review responses and the `0.4.0`/`0.4.1` split, and were recorded in the closeout, ROADMAP, changelog, and four review responses — everywhere except the entry an implementer reads for scope.

That gap is the reviewer's, and it is the second time: PR-016-E had the same shape, caught on 2026-08-01. This document is the fix, written before the slice starts rather than after.

## The four obligations

| # | Obligation | Where it was recorded | In the scope entry? |
| --- | --- | --- | --- |
| 1 | Content ↔ Terminal route switching, **no animation** (`NFR-UX-005`) | `task-breakdown-pr-plan.md` | ✅ |
| 2 | Sidebar and main-area scaffolding for RFC-017/019/020 | `task-breakdown-pr-plan.md` | ✅ |
| 3 | **C4 / `NFR-PERF-002`** mode-switch latency measurement | PR-015-**F**'s entry, response 133, `qa-evidence.md:258` | ❌ |
| 4 | **Visible focus indicators at the shell-chrome level** | closeout, ROADMAP §M8, `CHANGELOG.md`, responses 132/134/137/138 | ❌ |

## Obligation 4 is a gate, not a nice-to-have — read this before designing the sidebar

**`0.4.0` shipped with no chrome-level focus indicator, and that was defensible for exactly one reason: `FocusZone` has a single variant.** Tab has nowhere to go, so there is nothing for an indicator to distinguish. The modal renders focus visibly (PR-015-C's screenshots differ between focused buttons, with 1 ≡ 3 proving the cycle), so the gap is confined to chrome.

**This slice adds the second zone. The defence expires the moment it does.**

A keyboard-driven shell where a user cannot see what holds focus is an accessibility defect, not a cosmetic one — and RFC-015's own input model exists precisely so the shell is fully keyboard-navigable. Shipping `0.4.1` with two zones and no indicator would mean the *only* release that made Tab meaningful is also the one where Tab is invisible.

Concretely:

- `Theme::border_focused` was **cut in PR-015-B** for having no caller, and I endorsed cutting it. That was right then — there was no focus concept to render. Reinstating it (or its equivalent) is part of this slice, not a regression of that decision.
- `top_bar`/`status_bar` currently render `border_default()` unconditionally. `state.focus` is tracked for routing and has no rendered representation at all.
- **`NFR-UX-002` applies: no colour-only status.** A focus ring distinguished only by hue fails the requirement this project has held since RFC-005. Use a second channel — border weight, a marker glyph, inset — as the existing modal button rendering already does with its `"> "` marker.

Do not treat this as satisfiable by "the focused zone has a different background." Verify it the way the modal focus was verified: two screenshots that differ, and a byte-identical third proving the cycle returns.

## Obligation 3 — C4, and what makes it measurable now

`NFR-PERF-002`: mode switch p95 ≤ 32 ms.

C4 was excluded from PR-015-F deliberately (response 133): in M8 a mode switch toggled the Project Board against an empty placeholder, so measuring it would have measured scaffolding. **This slice is what makes it a real target** — that is the entire reason the measurement moved here rather than staying with the rest of R1's discharge.

Reuse PR-015-F's harness, not a new one:

- The measurement flag convention (`TEKSTIDE_MEASURE_CRITERION`) and its `Criterion` enum already exist; C4 is a new variant, not a new mechanism.
- **Do not reintroduce `iced::window::frames()` for this.** PR-015-F established that `frames()` forces continuous redraw and produced RFC-014's degenerate all-`0µs` results; the input-to-state-change and view-cost decomposition is `frames()`-free and is the reason R1's discharge was non-degenerate. C4 has the same shape — measure the state transition and the view rebuild, not paint-to-screen.
- **Non-contamination must be re-proven for the new criterion**, not inherited. PR-015-F's idle-CPU comparison (0 ticks default, 3 ticks measuring) is the precedent and the bar.
- Report p50/p95/p99 and max, with delivery-loss rate. **Another all-zero figure is not an acceptable outcome** — that instruction from PR-015-F's review gate carries.

One structural note you will hit: `shell::subscription`'s measurement branch runs **before** modal routing, and `modal_for_state` makes measurement and the demo modal mutually exclusive (response 134). A mode-switch criterion needs to drive route changes, so check whether that exclusion still expresses what you want, or whether C4 needs its own arrangement. Say which, and why.

## Obligations 1 and 2 — unchanged, with three constraints worth restating

- **No animation or interpolation** (`NFR-UX-005`), confirmed by inspection. The switch is discrete.
- **Mode switching must not disturb running terminals or AgentRuns.** There are no terminals yet (RFC-017), so this is currently a structural argument rather than an observed one — say so rather than checking the box on an absence.
- **Scaffolding must expose the surface contract without pre-empting RFC-017/019/020.** `surface.rs`'s contract is concrete methods rather than a `trait Surface`, deliberately, because one implementor gives nothing to generalise from (PR-015-D). **A second surface arriving here is the moment that calculus changes** — if the sidebar and main area make a real trait pay for itself, introduce it and say why; if not, say that too. Do not introduce one reflexively because there are now two things.

## What this slice inherits from `0.4.0` and must not break

- **Untrusted text goes through `tekstide_core::text_safety`.** Any new surface rendering project names, paths, or branch names uses `quote_untrusted` into `CatalogArgs::untrusted` — never `trusted_symbol`, never a raw literal. The seam scans catch raw literals; they do **not** catch an unescaped variable.
- **The mechanical scans now live in `i18n::enforcement`** (PR-016-E absorbed PR-015-B's string scan). New source files are covered automatically by the crate-tree walk. If a new file legitimately needs a literal, **raise it — do not add an exemption to make the scan pass.**
- **`FocusZone` is `#[non_exhaustive]`** specifically so adding `Sidebar` does not reshape routing. Adding the variant should not require touching `route_non_modal_input`'s structure; if it does, that is a signal worth reporting.
- **Modal exclusivity is structural at the call gate and framework-dependent for teardown** (responses 130/131). A second focus zone does not change that, but `SubscriptionMode`'s two branches must keep covering every reachable state.

## Closing RFC-015

RFC-015 stays in `rfcs/proposed/` until this slice lands — RFC-000 makes the folder the lifecycle source of truth, and an RFC with an outstanding implementation slice is not done. **The transition is a separate commit after PR-015-E and C4 are accepted**, and it is the architect's, not yours.

The `acceptance-qa-checklist.md` lines this slice closes: Content ↔ Terminal switching, no-animation, terminals-unaffected, `NFR-PERF-002`, visible focus indicators, and screenshots of both modes.

## Review gate

- Both modes screenshotted (the `0.4.0` closeout could only screenshot Content Mode — there was no second mode).
- Focus indicator demonstrated across a real two-zone cycle, not asserted.
- C4 measured non-degenerately, with non-contamination re-proven for the criterion.
- No animation, confirmed by inspection.
- Per response 127's standing convention: **flag synthetic input in the review request before running it.** niri does not forward XTest to native Wayland clients; relaunch with `WAYLAND_DISPLAY` unset, and use `xdotool windowfocus` — `windowactivate` does not work here (PR-015-C's own finding).
