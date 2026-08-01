# RFC-017: Terminal Renderer and Immersion Mode - Developer Handoff Pack

Source RFC: [RFC-017](../../proposed/017-terminal-renderer-and-immersion-mode.md)
Target milestone: **M9** (`0.5.x`)
Source RFC status: **Accepted by the human owner 2026-08-01**

**Start here.** This file is the entry point. Everything is linked below in reading order.

## Read in this order

| # | Document | Purpose |
| --- | --- | --- |
| 1 | [RFC-017](../../proposed/017-terminal-renderer-and-immersion-mode.md) | The surface, the security core, and the RFC-018 boundary. **Read "The security core" before anything else.** |
| 2 | This file | Orientation and what is binding. |
| 3 | [`pr-017-b-filter-promotion.md`](./pr-017-b-filter-promotion.md) | **Detailed instructions for PR-017-B, the security-critical slice.** Read before writing any code. |
| 4 | [`implementation-handoff.md`](./implementation-handoff.md) | Module layout, the seams, what already exists to reuse. |
| 5 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Slice boundaries and review gates. |
| 6 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | Required evidence. |
| 7 | [`qa-evidence.md`](./qa-evidence.md) | Where you record gates, findings, and limitations. |

Read before starting — RFC-017 conforms to these rather than amending them:

- [RFC-009](../../done/009-terminal-security-boundary.md) — the accepted-sequence policy. **This RFC renders it. It does not widen it.**
- [RFC-014](../../done/014-desktop-gui-substrate-and-terminal-rendering.md) — the Option A decision, and the spike whose filter you are promoting.
- [RFC-015](../../done/015-application-shell-and-rendered-surface-model.md) — the surface contract and the three input classes. `TextStream` was built for this surface and has been waiting for it.

## Where to start work

**Begin at PR-017-B.** PR-017-A is design acceptance, already granted.

**`B → C` is strict.** Nothing renders emulator output before the filter is proven in product code. This is not a sequencing preference — an emulator behind an unproven filter manufactures confidence, which is worse than no emulator.

## Five things that are binding

1. **This is the first surface where untrusted bytes reach a renderer.** Every surface before it rendered Tekstide's own state. The Project Board escapes untrusted project names; the terminal grid deliberately does not escape, because escaping would corrupt it. That exception is narrow — see item 3.

2. **Policy stays in `tekstide-core`; interposition holds none.** `runtime::terminal::security` already classifies RFC-009's families and core has no `vte`/`alacritty_terminal` dependency. Keep it that way, and verify mechanically. A `match` in the shell crate deciding whether a sequence is acceptable is a second classifier, and a second classifier is the defect.

3. **The RFC-016 exception is the grid, not the chrome.** A session title, pane header, or tooltip derived from terminal output is untrusted text in trusted chrome and goes through `tekstide_core::text_safety`. Only the grid itself is exempt. If you find yourself exempting something that is not the grid, you have widened the exception.

4. **You may not cite RFC-014 PR-014-D's screenshot.** It is the strongest trusted-UI artifact this project has — a genuine modal beside an adversarial terminal imitation in one frame — and it proved the *spike's* modal above the *spike's* terminal. **RFC-018 re-establishes it in the product.** Citing it here would be the exact overclaim this project has spent thirty reviews catching.

5. **Escalating is a success.** If P1-P4 cannot be re-established in product code, stop before PR-017-C and say so. RFC-014 named Option B — own the parser — as the live fallback, and choosing it is the decision the spike existed to inform.

## What already exists — reuse it, do not rebuild it

| Thing | Where | Note |
| --- | --- | --- |
| RFC-009 classification | `tekstide-core::runtime::terminal::security` | `TerminalSequenceFamily`, `TerminalPolicyReason`, effect enums |
| PTY lifecycle | `tekstide-core::runtime::terminal` | RFC-008; bounded IO, resize, process-group termination |
| The filter to promote | `crates/tekstide-gui-spike/src/filter.rs` | Reviewed under RFC-014 PR-014-C |
| Pane/layout model | `tekstide-core::navigation` | `TerminalPanePolicy`, `TerminalLayoutClass`; `visible_terminal_limit` defaults to 2 |
| Font metrics approach | `crates/tekstide-gui-spike/src/font_metrics.rs` | Split policy needs real metrics, not fractions |
| `TextStream` | `crates/tekstide/src/input/terminal_surface.rs` | Constructor is `pub(super)` deliberately — this surface is why |
| Measurement harness | `crates/tekstide/src/measurement.rs` | PR-015-F; **do not reintroduce `iced::window::frames()`** |
| Text safety | `tekstide_core::text_safety` | For chrome, not the grid |

## One obligation inherited from `0.4.1`

**`NFR-PERF-002` (mode switch, p95 ≤ 32 ms) must be re-checked in this RFC.** PR-015-E discharged it at ~470× headroom — but against a switch between two *placeholder strings*, because neither Terminal Mode nor Content Mode rendered anything real yet. PR-017-E is what makes Terminal Mode real, and a grid rebuild is not a single line of text.

Re-measure it once the terminal renders, and treat the `0.4.1` figure as a floor rather than a result. If it no longer holds, that is a finding, not a regression to hide.

## Two questions this RFC hands you to decide

Both were deliberately left open because they could not be judged without a terminal to judge them against. Decide them **with evidence and record why**, do not default.

- **Does Tab reach the terminal?** (PR-017-D.) RFC-015 routes Tab to the shell focus cycle ahead of terminal focus. Shell completion makes Tab-to-terminal genuinely useful; an inescapable focus trap makes it dangerous. Whichever way you go, the escape hatch must not depend on the terminal cooperating.
- **What happens to a hidden session's grid state?** (PR-017-E.) Retained in memory costs memory per hidden session; torn down and rebuilt from scrollback loses state and changes what "hidden" means. Decide against the bounded-scrollback decision, not separately from it.

## Conventions that carry from `0.4.0`

- **Screenshots**: `niri msg action screenshot-window --id <id> --path <repo-relative>`, stored under `evidence/pr-017-*/`, committed, each with an explicit statement of what it proves **and does not**.
- **Synthetic input**: flag it in the review request *before* running it. Three findings this project paid for, in one place:
  1. **niri does not forward XTest to native Wayland clients** (RFC-014). Relaunch with `WAYLAND_DISPLAY` unset to force the X11/XWayland backend.
  2. **`xdotool windowactivate` fails here; `windowfocus` works** (PR-015-C).
  3. **Always pass `--clearmodifiers`** (PR-015-E). X11 modifier state is global to the connection, so a chord like `Ctrl+Alt+M` followed immediately by a plain key can deliver that key while the server still considers a modifier down — the client sees `Ctrl+Tab`, the routing correctly ignores it, and the keystroke vanishes *silently*. Pair it with a fresh `windowfocus --sync`. In a measurement run this is a survivorship-bias source (R9): confirm delivery by on-disk sample count, never by eye.
- **The mechanical scans** live in `i18n::enforcement` and walk the crate tree, so new files are covered automatically. If a new file legitimately needs a literal, **raise it — do not add an exemption to make the scan pass.**
- **Ablate, do not assert.** Every mechanical guarantee gets one ablation per property. A test that passes with the thing it tests deleted is the failure mode this project has found five times.
