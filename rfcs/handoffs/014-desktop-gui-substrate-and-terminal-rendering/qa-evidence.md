# RFC-014: Desktop GUI Substrate and Terminal Rendering Strategy - QA Evidence

Status: Proposed — criteria accepted, spike pending
Date opened: 2026-07-28
Date accepted: Pending

## Scope

RFC-014 selects the desktop GUI substrate and terminal-rendering strategy together, on spike evidence rather than paper analysis.

Evidence in this file must not be used to claim production GUI readiness, M8 implementation, final visual design, syntax highlighting, cross-platform support, or any change to the RFC-009 accepted-sequence boundary, unless a later reviewed decision record explicitly supports that claim.

**The spike is disposable.** Nothing recorded here authorises product code to depend on the chosen substrate. That authorisation comes only from an accepted PR-014-F decision record.

## Design Review

PR-014-A accepted on 2026-07-28. Criteria C1-C14, the candidate set, and the TUI rejection are settled.

Open questions resolved at acceptance:

- TUI rejected on accepted-requirements grounds (RFC-009:212 requires dialogs rendered outside terminal output; i18n and accessibility point the same way). Not evaluated by the spike.
- Second candidate selected by the spike author using the text-grid/terminal-precedent rule.
- Missed-budget escalation policy defined in `implementation-handoff.md` §5.
- Syntax highlighting remains deferred; styled-span rendering confirmed instead.

## Implementation Evidence

### PR-014-B — Spike harness and candidate selection

**Quarantine.** Added `crates/tekstide-gui-spike` (`publish = false`), added to workspace `members`. Depends on `tekstide-core` (workspace path dependency, read-only use) and `iced = "0.14"`. Verified `grep -n "tekstide-gui-spike" crates/tekstide/Cargo.toml crates/tekstide-core/Cargo.toml` returns nothing — no product crate depends on the spike. The spike performs no network access and writes no Tekstide state; it has no persistence code at all in this slice.

**Candidate survey.** Primary candidate `iced` per the handoff rule (maintainer expertise, C14 factor). Bounded survey of the shortlist (`egui`, `slint`, `gpui`, `xilem`, `relm4`/GTK) for the second candidate, screened on C13/C14 before writing code:

| Candidate | C13 (licence) | C14 (maintenance) | Terminal/text-grid precedent | Verdict |
| --- | --- | --- | --- | --- |
| `slint` | **Fails.** GPLv3 or a bespoke royalty-free proprietary EULA (attribution-gated) or paid commercial; neither GPL nor a proprietary EULA is compatible with permissive Apache-2.0 redistribution. | Not evaluated further | Not evaluated further | Screened out on C13 |
| `gpui` | Passes — Apache-2.0. | **Fails.** Last published to crates.io as `0.2.2` on 2025-10-22 — ~9 months stale as of this survey (2026-07-28), versus `egui`'s ~1 month. Downloads 172,576 vs. `egui`'s 20.4M+. This is a harder, independently-checkable fact than the community-fork narrative it replaces (Zed Industries paused community-facing GPUI development in 2026 — "hard for Zed Industries to justify work on GPUI that is purely for the community" — with the `gpui-ce` fork at single-digit merged PRs and ~381 commits behind mainline); the crates.io publish gap is offered as primary evidence, with the fork narrative as corroborating context rather than the load-bearing claim. | **Strongest of the shortlist** — Zed itself ships a production terminal panel built on GPUI. | Screened out on C14 despite the best precedent: precedent quality shows a framework can do the job today; maintenance posture shows whether it still will in three years, and a ~9-month release gap on a pre-1.0 framework answers that question |
| `xilem` | Passes — Apache-2.0. | Explicitly alpha-state per its own repository; no stable release. | None found. | Deprioritized on immaturity |
| `relm4` (GTK4 bindings) | Not fully evaluated | Carries a large native GTK4 C-library dependency, in tension with the "pure-Rust, dependency-light" framing that already excludes webview shells (`NFR-RES-002`) | GTK's own VTE widget has terminal precedent, but via a C library, not a Rust-idiomatic embedding | Deprioritized without full evaluation; recorded rather than silently dropped |
| **`egui`** | Passes — `MIT OR Apache-2.0` (verified via crates.io API). | **Strong.** 20.4M+ downloads, last published 2026-06-25 (within a month of this survey). | Real, working: [`egui_term`](https://github.com/Harzu/egui_term), a terminal-emulator widget on the `alacritty_terminal` backend, tested on Linux/macOS. | **Selected** |

Selection: **`egui`**. It is the only shortlist candidate that passes both screens *and* has a real, working terminal-rendering precedent, and its immediate-mode architecture is a genuine contrast to `iced`'s Elm-style retained mode, which is the comparative value the RFC asks the second candidate to provide. This also surfaced useful information for PR-014-C: `iced` itself already has terminal precedent too — [`iced_term`](https://github.com/Harzu/iced_term), also built on `alacritty_terminal` — which is relevant prior art for the Option A/B investigation, not the candidate-selection question.

Licence versions verified directly against the crates.io API (not secondary sources): `egui` `MIT OR Apache-2.0`; `iced` `MIT`; `alacritty_terminal` `Apache-2.0`; `vte` `Apache-2.0 OR MIT`.

**Dependency weight (C14, recorded for PR-014-F).** `Cargo.lock` went from 50 to 395 packages after adding `iced` — +345 transitive dependencies, a ~7.9x increase in the resolved graph. Notable transitive natives: `wgpu`, `wgpu-core`, `glow`, `glutin_wgl_sys`, `cosmic-text`, `fontdb`. This was applied asymmetrically in the survey above — `relm4`/GTK was deprioritized partly on dependency weight, but `iced`'s own weight was never measured, because `iced` was assigned by the handoff rule rather than selected by the C14 screen. Recording it now rather than leaving it implicit: ~345 crates is unremarkable for a GPU-accelerated Rust GUI toolkit (a windowing/rendering backend for `egui` would pull a comparable order), so this is not disqualifying, but RFC-013's T-033 exists because one native dependency (`rusqlite` + bundled SQLite) was judged worth a threat-model entry — a graph this size, including a GPU abstraction layer and a font-shaping stack, is a materially larger surface that PR-014-F should weigh explicitly rather than inherit by default.

**Workspace-dependency deviation.** Every other dependency in this repo is declared in root `[workspace.dependencies]` (`libc`, `serde`, `serde_json`, `uuid`, `rusqlite`), and `tekstide-pty-spike` follows that pattern (`libc = { workspace = true }`). `iced = "0.14"` is instead declared directly in `crates/tekstide-gui-spike/Cargo.toml`, not the workspace table. This is deliberate, not an oversight: a dependency absent from `[workspace.dependencies]` cannot be picked up by another crate via `iced.workspace = true`, which makes the quarantine boundary structurally stronger rather than weaker — no product crate can "accidentally" gain access to `iced` through the workspace table the way it could if it were declared there.

**Static Content Mode shell.** `crates/tekstide-gui-spike/src/shell.rs` renders a top bar, sidebar (20% width via `Length::FillPortion`), main area (remaining width), and status bar, matching `tekstide-uiux-wireframes-v0.md` §7.2 and external design §4.4 proportions.

*Finding (the spike doing its job):* the first implementation compiled and ran but rendered both panes as small content-sized boxes in the top-left corner rather than filling the window — a real layout bug, not a hypothetical one, caught by actually running the app rather than trusting that it compiled. Root cause: an inner `container` in `zone_container` had no explicit `width`/`height`, so `Length::Fill`/`FillPortion` on the *outer* wrapping container had nothing to propagate into. Fixed by applying the fill sizing directly on `zone_container`'s own returned container instead of double-wrapping. Screenshot evidence before/after is not retained (the bug was transient and caught immediately); the corrected layout is `evidence/pr-014-b/shell-static-sidebar-focused.png`.

**Keyboard focus model.** `Tab`/`Shift+Tab` cycle focus between `Sidebar` and `MainArea` (`FocusZone::next`/`previous`), via `iced::keyboard::listen().filter_map(...)` (there is no `on_key_press` helper in `iced` 0.14 — confirmed by reading the crate source directly rather than assuming from stale documentation summaries; `filter_map` requires a non-capturing closure, which a plain `match` satisfies). Focus is indicated non-color-reliant: a `[focused]` text prefix plus a distinct border, and the status bar names the focused zone (`Focus: Explorer` / `Focus: Content`). The `[focused]` text prefix specifically satisfies `NFR-UX-002` (status indicators must not rely on colour alone) — the border alone would not.

*Verified with real input, not asserted from code reading:* `iced`/`winit` prefers native Wayland when `WAYLAND_DISPLAY` is set, and this environment's compositor (niri) does not forward XTest synthetic input to native Wayland clients — `xdotool key Tab` against the natively-Wayland spike window produced no observable change. Relaunching with `WAYLAND_DISPLAY` unset forced the `winit` X11/XWayland backend, where `xdotool key --window <id> Tab` and `shift+Tab` both worked and were captured with `niri msg action screenshot-window --id <id> --path <file>` (targeted by window ID, so the maintainer's actual desktop focus was never disturbed — no `niri msg action focus-window` call was needed to take these screenshots, only to activate the X11 window for `xdotool` to deliver key events to it).

Evidence (all in `evidence/pr-014-b/`):
- `shell-static-sidebar-focused.png` — initial state, sidebar focused, correct 20/80 fill proportions.
- `shell-focus-tab-to-content.png` — after `Tab`: focus and border moved to the main area, status bar updated to `Focus: Content`.
- `shell-focus-shift-tab-to-sidebar.png` — after `Shift+Tab`: focus returned to the sidebar.

This is C9 evidence for the spike shell only (two static zones); it is not yet C9 evidence for a real dialog with focus trapping — that is PR-014-D's job.

Gates observed on 2026-07-28: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (375 `tekstide-core`, 0 elsewhere, 0 failures — the spike carries no unit tests, per the handoff's own guidance not to pad it), `git diff --check` all passed.

### PR-014-C — Terminal surface and Option A/B resolution

Pending implementation.

### PR-014-D — Trusted-UI evidence

Pending implementation.

### PR-014-E — Measurement

Pending implementation.

## Criteria Not Evaluated

To be completed by the spike. Every criterion in C1-C14 that could not be exercised must appear here with a reason. An empty section at closeout means every criterion was evaluated — do not leave it empty by omission.

## Known Limitations

- The spike measures **app-internal latency** (input event receipt to frame submission). It excludes input-stack latency before the application sees the event and compositor/display latency after submission. These figures are not end-to-end.
- Linux only. No Windows or macOS evidence is produced or claimed.
- Spike code is measurement code and is not expected to carry meaningful test coverage.
- `iced = "0.14"` is declared with default features enabled; `default-features = false` with a reviewed, minimal feature set was not applied in this slice (acceptable for a disposable spike, per the RFC-013 precedent that this discipline matters once a dependency is product-facing). If `iced` becomes a product dependency at M8, its feature surface needs the same review `rusqlite` received in RFC-013, and the licence inventory must extend to its transitive native dependencies — `wgpu` and the font-shaping stack (`cosmic-text`, `fontdb`) in particular.
