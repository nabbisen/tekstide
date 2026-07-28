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

**Workspace-dependency convention.** `iced` was initially declared directly in `crates/tekstide-gui-spike/Cargo.toml` rather than `[workspace.dependencies]`, on the reasoning that keeping it out of the shared table made the quarantine boundary marginally stronger (no other crate could pick it up via `iced.workspace = true` without also adding its own version line). The maintainer overrode this during review: every other dependency in this repo, including `tekstide-pty-spike`'s `libc`, is declared in `[workspace.dependencies]`, and quarantine is actually enforced by which crates list a dependency in their own `[dependencies]` section, not by where the version string lives. `iced`, `alacritty_terminal`, and `vte` are now declared in `[workspace.dependencies]` and referenced via `.workspace = true`, matching the established convention. The quarantine claim (no product crate depends on the spike) is unaffected — verified by the same `grep` check after this change.

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

**Verdict: Option A is implementable and not falsified.** The RFC-009 accepted-sequence policy is interposed as a filter in front of `alacritty_terminal`'s emulator, and the filter is not bypassable by anything reachable from the PTY byte stream.

**Interposition point.** Not the byte/`Perform` boundary the handoff's provisional framing assumed, but one layer further in: `alacritty_terminal` 0.26 depends directly on `vte` 0.15's bundled `ansi` module, and `vte::ansi::Processor` — the *same* code alacritty itself is built on — owns the entire VT/ANSI grammar and dispatches already-classified semantic operations to a `vte::ansi::Handler` implementation (`Term` implements it). `crates/tekstide-gui-spike/src/filter.rs`'s `SecurityFilter<H: Handler>` wraps the real handler, forwarding only the RFC-009 accepted set (`input`, `carriage_return`, `linefeed`, `put_tab`, `backspace`, `terminal_attribute`, `move_up/down/forward/backward`, `clear_line`, `clear_screen`) and letting every other one of the trait's 71 methods fall through to the trait's own no-op default, so `Term` is never called for anything not on that list. Each accepted-set mapping was read directly from the vendored `vte-0.15.0/src/ansi.rs` `execute`/`csi_dispatch` match arms, not assumed.

This choice gives two of the four filter properties **by construction, not by testing**:

- **P3 (classification parity)** holds because the filter and the real emulator share the identical classifier — there is no second grammar implementation to drift out of sync with the first, unlike a from-scratch byte-level filter would require.
- **P4 (stream-position independence)** holds because `Processor` is long-lived across `advance()` calls and holds the VT parser state internally, so a sequence split across two PTY reads is reassembled by the same code the real terminal uses before this filter ever sees it.

**P1/P2 assessment (read from source, not assumed).** `Term::grid_mut()` is public API for direct grid manipulation (used by alacritty's own search/selection features) and is **not** reachable from the PTY byte path — `vte::ansi::Processor` never calls it. P1 therefore depends on calling-code discipline: `terminal_pane.rs` is the only place PTY bytes are handled in this spike, and the only thing it ever does with them is `Processor::advance(&mut SecurityFilter::new(&mut self.term), bytes)`. No other code path holds a `&mut Term` derived from PTY input.

#### Adversarial corpus: findings, not just pass/fail

18 tests in `crates/tekstide-gui-spike/src/filter/tests.rs`, all passing (14 from this slice's own corpus, plus 4 review-supplied probes covering V5-V7, credited below), several of which falsified my own initial assumptions rather than confirming them — recorded because a spike's job is to falsify, not to demonstrate:

- **SCP is CSI-dispatched, not DCS.** The handoff's own framing ("SCP, the one DCS-family sequence Processor recognizes") does not match vte 0.15: SCP is `ESC [ <n> SP k`, a CSI sequence. Verified by reading `csi_dispatch`'s `('k', [b' '])` match arm directly. The corpus now has a corrected `v1_scp_is_csi_not_dcs_blocked_at_every_split` test and a separate genuine-DCS test.
- **No DCS content of any kind reaches a `Handler` method, recognized or not.** `vte::ansi::Processor`'s own `Perform::hook`/`put`/`unhook` implementations are unconditional no-ops (read directly, not inferred) — DCS is fully swallowed one layer below this filter's interposition point. `v1_generic_dcs_content_never_reaches_handler_at_every_split` proves a 26-byte DCS payload leaves `blocked` empty (nothing to classify — Processor never asked) and never reaches the grid, at every one of 25 split points.
- **8-bit C1 introducers are not recognized at all by this parser** — confirmed by reading `vte`'s own module doc ("Only supports 7-bit codes") and its `advance_ground`/`anywhere` state-transition code directly. A lone C1 byte (e.g. `0x9D`) is invalid UTF-8, handled via `Perform::execute(byte)`, which `ansi::Processor`'s own `execute()` match has no arm for — so it is silently dropped *before* reaching this filter's `Handler` layer at all.
- **Second-order finding from the above, discovered empirically by running the test rather than assumed from the first:** consuming the ambiguous introducer byte does not suppress the payload that would have followed it in a well-formed sequence. The *semantic operation* never fires (no `clipboard_store`/`set_title` call — `blocked` is empty because there was nothing to block), but the trailing payload text (e.g. the base64 clipboard content, or the title string) **renders as plain printable characters**, because the parser resumes in `Ground` state once the ambiguous byte is consumed. For OSC 52 specifically: no clipboard exfiltration, but the payload text becomes visible on screen — a real, lesser leak than the 7-bit form, recorded rather than silently asserted as fully inert. `v2_8bit_c1_osc_introducer_blocks_the_operation_but_payload_text_still_renders` and its DCS counterpart assert this exact, verified behavior.
- **`put_tab` is unambiguous.** Traced the C0 HT execute path (`C0::HT => self.handler.put_tab(1)`) against the CSI-originated tab-stop methods (`move_forward_tabs`/`move_backward_tabs`, separate methods) before assuming `put_tab` was safe to allow — confirmed they are genuinely distinct at the `Handler` layer, so allowing raw-tab does not also admit CSI tab-stop movement (which stays blocked by omission).
- **`linefeed()` is a real, if minor, RFC-009 policy widening — and it is *constraint-forced*, not a choice.** `execute()`'s `C0::LF | C0::VT | C0::FF => self.handler.linefeed()` means allowing `linefeed()` (required for LF) also admits VT (0x0B) and FF (0x0C), which `tekstide_core::TerminalSecurityParser`'s byte-level classifier treats as unsupported C0 control (blocked). `vte` collapses all three into one `Handler` call, so the filter cannot separate them at this boundary — fixing it would need a byte-level pre-filter for those two specific control codes, not a `Handler`-layer change. Recorded as a limitation for PR-014-F, not silently absorbed.

Both directions are proven, not just the blocking direction: `accepted_printable_text_reaches_the_grid`, `accepted_sgr_cursor_and_clear_do_not_block`, and `accepted_c0_controls_do_not_block` confirm the filter is a boundary, not a brick wall.

#### Review-supplied probes (response 106) — V5, V6, V7, and two non-blocking findings

Response 106 independently verified the architectural claims against vendored `vte-0.15.0` source (confirming P3/P4 hold by construction, and correcting the `Handler` trait's method count to **71**, not the ~119 originally estimated — the limitation below now reads 8 of 71) and ran the three bypass vectors this slice's corpus did not cover. All three pass; added to the corpus as `v5_*`/`v6_*`/`v7_*`:

- **V5 (parameter overflow) does not desync the parser.** 500 semicolon-separated CSI params followed by trailing text: `blocked = []`, and the trailing text renders correctly — no truncate-and-reinterpret. The sharper case, an overflowed CSI immediately followed by OSC 52, still blocks the clipboard call and leaks nothing: kept as `v5_parameter_overflow_followed_by_osc_52_still_blocks_clipboard`, the sharpest regression in the added set.
- **V6 (colon sub-parameters).** `CSI 38:2:255:0:0 m` forwards via `terminal_attribute` exactly like the semicolon form. `Attr` is purely presentational, so nothing non-visual is reachable through the forwarded path regardless of parameter syntax.
- **V7 (UTF-8 split).** A 7-character, 21-byte CJK string split at every byte boundary reassembles correctly at all 20 split points — `Processor`'s persistent state handles multi-byte reassembly by the same mechanism P4 relies on for control sequences.

Two further non-blocking findings from the same review pass, not added as tests (documented here per the developer handoff):

- **`clear_screen` is forwarded mode-blind, including scrollback erasure.** `CSI 3 J` (`ClearMode::Saved`) wipes scrollback and is forwarded unconditionally, since `clear_screen` is in the accepted set. This is not a divergence introduced by this filter — RFC-009:131 accepts "clear line/screen operations" without qualifying the mode, and `tekstide_core`'s own byte-level parser has the same mode-blind `CsiClearScreen` classification. The filter faithfully implements the policy as written. It is a minor evidence-destruction vector (a destructive command could be followed by a scrollback wipe to hide it; RFC-011 transcripts mitigate this when capture is enabled) and, unlike the `linefeed` widening above, this one **is a choice, not a constraint** — the clear mode is an available parameter at the `Handler` boundary, so a future filter *could* distinguish `CSI 2 J` from `CSI 3 J` if RFC-017 decides it should. Raised as an RFC-017 policy question, not a defect in this PR.
- **No resource guard on repeat counts.** `CSI 999999999 C` / `CSI 999999999 B` (extreme cursor-movement repeat counts) are forwarded unblocked and did not hang in testing — but that is inferred from the absence of a hang, not verified against an internal clamp in `alacritty_terminal`. Recorded as a limitation and a real input for PR-014-E's flood/latency measurement, not assumed safe.

#### Real rendering evidence (PTY-backed, not simulated)

`terminal_pane.rs` launches `/bin/sh` via the existing `LinuxTerminalRuntime` (in a temp directory — this spike never touches a real Tekstide state root or project), and renders the filtered `Term` grid using `iced::widget::rich_text`/`Span`, with per-cell colors resolved via `Term::renderable_content()` — the same API a real terminal renderer (or `iced_term`/`egui_term`) would use, not a bespoke shortcut. A demo script (`send_demo_script_once`) exercises styled SGR output plus three inert families (OSC 52 clipboard, OSC title, OSC 8 hyperlink) so there is something concrete to screenshot.

Screenshot: `evidence/pr-014-c/terminal-pane-demo-script.png`, captured from a real running instance via `niri msg action screenshot-window`, forced to the X11/XWayland `winit` backend (see the PR-014-B Wayland/XWayland finding) so `xdotool` could deliver the `F2` mode-toggle key.

Observed in that screenshot and independently cross-checked (not screenshot-only):

- **Styled spans, C1 evidence:** "red", "green", and "bold-blue" render in distinct, correct colors within one line — genuine multi-color text in a single text block, using real `Attr`/SGR data resolved through `Term`'s color table, not hardcoded per-word colors.
- **Window title never changed**, despite the OSC-0 title-set sequence being sent through the PTY. Verified two ways beyond the screenshot: `niri msg windows` and `xdotool getwindowname` both still report `"tekstide-gui-spike (RFC-014)"` — the value set once via `.title()` at startup — after the demo script ran. This is real evidence the title-mutation block held at the actual OS window level, not just in an internal log.
- **OSC 8 hyperlink had no visible effect:** "link-text" renders as plain text, no hyperlink styling applied (`set_hyperlink` was intercepted, so the grid cursor template's hyperlink field was never set for those cells).
- **Status bar reports "RFC-009 filter blocked 15 calls this session"** — a live, non-hardcoded count from the running filter, not a fabricated number.

#### Licence inventory (C13)

All newly introduced dependencies for this slice, checked directly against the crates.io API:

| Crate | Licence |
| --- | --- |
| `alacritty_terminal` 0.26.0 | Apache-2.0 |
| `vte` 0.15.0 | Apache-2.0 OR MIT |
| `rustix-openpty` 0.2.0 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| `tokio` (added as an `iced` feature, for the tick subscription) | MIT |
| `aho-corasick`, `base64`, `home`, `miow` (Windows-only), `regex-automata`, `regex-syntax`, `signal-hook` (transitive) | MIT OR Apache-2.0 (or Unlicense OR MIT for `aho-corasick`) |

`Cargo.lock` grew from 395 to 406 packages (+11) for this slice — far smaller than `iced`'s own +345 in PR-014-B, since alacritty/vte/tokio reuse much of what was already resolved. None of these compile bundled native C code (unlike RFC-013's `rusqlite`); `rustix-openpty` wraps PTY syscalls through the existing `rustix`/`libc` ecosystem pattern already used elsewhere in this workspace, not a new C dependency. No `NOTICE` entry is triggered by this slice.

#### Gates observed on 2026-07-28

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (375 `tekstide-core` + 14 `tekstide-gui-spike` filter-corpus tests, 0 failures), `git diff --check` all passed.

#### Known limitations

- Only 8 of the trait's 71 `Handler` methods are individually classified into a `BlockedFamily` (the corpus's minimum required coverage: title, clipboard, hyperlink, private mode, keyboard protocol, terminal query/reply, SCP). The other 63 blocked methods are blocked by omission (never forwarded, so P1/P2 hold for them — response 106 verified every one of the trait's defaults is an empty `{}` body, so this is fail-closed by the trait's own design, not by this filter's discipline) but are not individually named in `blocked_log` — a reviewer probing for an unclassified-but-still-blocked method would find it silent rather than logged. This is a diagnostics-granularity gap, not a security gap.
- V5 (parameter overflow) and V6 (colon sub-parameters) and V7 (UTF-8 split) from `pr-014-c-filter-interposition.md` §4 are not covered by this corpus. V1 (mandatory, exhaustive split testing), V2 (8-bit C1), V3 (terminator divergence), and V4 (unterminated-at-stream-end) are covered; V8 (direct API access) is assessed and documented (`grid_mut`) rather than executed as a test.
- The `linefeed()` widening (VT/FF admitted alongside LF) is a real, if minor, RFC-009 policy widening relative to `tekstide_core::TerminalSecurityParser`'s byte-level classification — see above. Not fixed in this spike; recorded as a PR-014-F input.
- Color resolution in `terminal_pane.rs::resolve_color` covers the standard 16 ANSI colors and direct RGB (`Color::Spec`) only; indexed colors 16-255 fall back to a default foreground rather than resolving the full 256-color palette. Documented as a spike simplification, not silently implied complete.
- The demo script is fixed, non-interactive content sent once after launch — this is demonstration input to produce screenshot evidence, not a general user-input path or product behavior.
- No automated regression test asserts the window title stays unchanged (that check was done manually via `niri msg`/`xdotool` for this evidence pass, not wired into the corpus as a repeatable test).

### PR-014-D — Trusted-UI evidence

**Closes the RFC-009 "screenshot-backed spoofing evidence" deferral.** A genuine Tekstide modal dialog (`trusted_dialog_view` in `shell.rs`) renders via `iced::widget::stack`/`opaque` — a real GUI layer entirely outside the terminal grid — while an adversarial script running *inside* the terminal pane imitates it using box-drawing characters. Both appear in one frame; see `evidence/pr-014-d/genuine-and-adversarial-dialog-one-frame.png`.

**The test a reviewer applies, from `pr-014-c-filter-interposition.md` §6 and `implementation-handoff.md` §6: can someone looking only at the screenshot tell which one is real?** Yes: the genuine dialog has a sharp yellow border, opaque dark background, and a distinctly highlighted focused button (`> [ Deny ]` in blue) rendered as real GUI widgets. The adversarial imitation is dimmer terminal text with ASCII/Unicode box-drawing characters, unfocusable, and visibly part of the same scrolling terminal content as the shell prompt above and below it.

**Adversarial generator: committed and reproducible, not a one-off.** `crates/tekstide-gui-spike/adversarial-dialog.sh` draws the fake dialog and is included into the binary via `include_str!` in `terminal_pane.rs`, so the file that is reviewed and the bytes that actually run through the PTY cannot drift apart. It is written to a temp file in the pane's own scratch directory and invoked as `sh <path>` (a single command) rather than piped line-by-line into the interactive shell's stdin — the first attempt did the latter and the shell's normal input-echo behavior cluttered the screenshot with every comment line in the script; invoking it as one command shows only its actual output.

**Per response 106 (request 106, Q3): the adversarial generator includes an 8-bit C1 case.** PR-014-C found that a bare 8-bit C1 introducer byte is swallowed before reaching the filter's `Handler` layer, but the payload text that would have followed it in a well-formed sequence still renders as plain characters — "operation blocked, payload still prints" is exactly the primitive an attacker would use to draw a convincing fake dialog. The script includes a `\x9D` (C1 OSC) attempt at a title-bar takeover mid-dialog; its payload (`FAKE-APPROVAL-PENDING`) is visible in the screenshot, folded into the fake dialog body as ordinary text, exactly as PR-014-C's finding predicted.

**Focus trapping, demonstrated with real input, not asserted:** `DialogButton` (`Approve`/`Deny`) is a focus domain entirely separate from `FocusZone` (the PR-014-B shell zones). While `state.dialog_shown` is true, `Message::FocusNext`/`FocusPrevious` (Tab/Shift+Tab) are intercepted by a match guard and move only `dialog_focus` — never `state.focus`, and nothing the terminal pane renders can reach either. `evidence/pr-014-d/focus-trap-tab-cycles-approve-deny.png` shows a real `Tab` keypress moving focus from `Deny` to `Approve` while the adversarial script's own fake `[ Approve ] [ Deny ]` text sits inert in the background — the terminal cannot move focus out of the real dialog because it has no path to `dialog_focus` at all, not because of a runtime check that could fail. `evidence/pr-014-d/dialog-dismissed-after-enter.png` shows `Enter` activating the focused button, recording the decision in the status bar (`last real-dialog decision: Approve`), and dismissing the dialog — proving the dialog is genuinely interactive, not a static overlay.

**Finding, recorded because it cost real debugging time:** `xdotool key --window <id> F2` (the same targeted-delivery method that worked reliably for `Tab` in PR-014-B and PR-014-C) did not deliver the `F2` press to this window in this session, silently — no error, `terminal_mode` simply never toggled across three attempts. `xdotool key F2` (global delivery via whatever the X server currently treats as focused, after `xdotool windowactivate --sync`) worked immediately. The same substitution was needed again partway through for `Tab`. Root cause not fully isolated — plausibly a niri/XWayland focus-tracking quirk under rapid programmatic refocus cycles, since `_NET_ACTIVE_WINDOW` queries were failing throughout this session's later window-activation calls. Recorded as a testing-methodology note for PR-014-E, which will need reliable synthetic input for its own measurement: if targeted (`--window`) delivery becomes unreliable again, fall back to global delivery immediately after `windowactivate --sync` rather than assuming a key genuinely did nothing.

Gates observed on 2026-07-28: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (375 `tekstide-core` + 18 `tekstide-gui-spike`, 0 failures), `git diff --check` all passed.

### PR-014-E — Measurement

Pending implementation.

## Criteria Not Evaluated

To be completed by the spike. Every criterion in C1-C14 that could not be exercised must appear here with a reason. An empty section at closeout means every criterion was evaluated — do not leave it empty by omission.

## Known Limitations

- The spike measures **app-internal latency** (input event receipt to frame submission). It excludes input-stack latency before the application sees the event and compositor/display latency after submission. These figures are not end-to-end.
- Linux only. No Windows or macOS evidence is produced or claimed.
- Spike code is measurement code and is not expected to carry meaningful test coverage.
- `iced = "0.14"` is declared with default features enabled; `default-features = false` with a reviewed, minimal feature set was not applied in this slice (acceptable for a disposable spike, per the RFC-013 precedent that this discipline matters once a dependency is product-facing). If `iced` becomes a product dependency at M8, its feature surface needs the same review `rusqlite` received in RFC-013, and the licence inventory must extend to its transitive native dependencies — `wgpu` and the font-shaping stack (`cosmic-text`, `fontdb`) in particular.
- PR-014-D's dialog is a single fixed instance (`dialog_shown`/`dialog_focus`/`dialog_decision` in `shell.rs`), not a general reusable dialog/modal system. That generalization is M8 product work, not spike scope.
- The adversarial script's box-drawing characters render correctly but faintly (thin single-line-weight glyphs) in the default monospace font at the tested size; a real attacker tuning for visual conviction would likely test multiple fonts. This does not change the C8 verdict (the genuine dialog is still clearly distinguishable by its opaque GUI-layer rendering, not by the fake's glyph fidelity), but it means this corpus is not a maximally-adversarial visual attempt.
- Keyboard-only interaction is exercised (`Tab`/`Shift+Tab`/`Enter`); no mouse-click path onto the dialog buttons is implemented or tested in this slice.
