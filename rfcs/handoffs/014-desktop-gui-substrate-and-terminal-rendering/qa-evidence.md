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

**The test a reviewer applies, from `pr-014-c-filter-interposition.md` §6 and `implementation-handoff.md` §6: can someone looking only at the screenshot tell which one is real?** Yes, decisively — confirmed by an independent read in response 107, reached before that reviewer read this section. The tells split into two kinds, and the distinction matters: the verdict must rest on the first kind, not the second.

**Structural tells — an attacker cannot defeat these no matter how good the fake is:**

| Tell | Why it cannot be faked |
| --- | --- |
| Font | The genuine dialog renders in the proportional UI font; the fake is locked to the terminal's monospace cell grid. A fake cannot escape the grid it is drawn in. |
| Opacity / occlusion | The genuine dialog has an opaque background that occludes content behind it. The fake composites into the surrounding text flow. |
| Widget chrome | `> [ Deny ]` has a real rendered selection background. The fake's `[ Deny ]` is characters, nothing more. |
| Position | The genuine dialog floats centred, independent of terminal content. The fake is confined to the pane's bounds and scrolls with it. |

**Incidental tells — artifacts of this particular fake, not of the approach; a better-tuned attacker would remove these:**

- The fake's box is visibly broken: the right border misaligns and the frame does not fully close.
- `tekstide$` sits directly beneath the fake, betraying the scroll flow.
- The literal text `FAKE-APPROVAL-PENDING` — no real attacker labels their own fake this way.
- The 8-bit C1 payload line (`printf '\235FAKE-APPROVAL-PENDING\007\n'`) prints *without* box borders around it, so it renders flush-left and visibly breaks the frame rather than being folded into a bordered row. A stronger imitation would embed it inside the box. Self-identified weakness in this script, not fixed here — it does not change the verdict, since the verdict rests on the structural tells above, but it means this corpus is not the most visually-aggressive imitation possible.

**The verdict would stand even against a fake with perfect box geometry, no visible prompt, and innocuous (non-labelled) payload text** — it does not depend on this corpus being adversarial enough. That is the property worth recording, not "the fake happened to be sloppy."

**Adversarial generator: committed and reproducible, not a one-off.** `crates/tekstide-gui-spike/adversarial-dialog.sh` draws the fake dialog and is included into the binary via `include_str!` in `terminal_pane.rs`, so the file that is reviewed and the bytes that actually run through the PTY cannot drift apart. It is written to a temp file in the pane's own scratch directory and invoked as `sh <path>` (a single command) rather than piped line-by-line into the interactive shell's stdin — the first attempt did the latter and the shell's normal input-echo behavior cluttered the screenshot with every comment line in the script; invoking it as one command shows only its actual output.

**Per response 106 (request 106, Q3): the adversarial generator includes an 8-bit C1 case.** PR-014-C found that a bare 8-bit C1 introducer byte is swallowed before reaching the filter's `Handler` layer, but the payload text that would have followed it in a well-formed sequence still renders as plain characters — "operation blocked, payload still prints" is exactly the primitive an attacker would use to draw a convincing fake dialog. The script includes a `\x9D` (C1 OSC) attempt at a title-bar takeover mid-dialog; its payload (`FAKE-APPROVAL-PENDING`) is visible in the screenshot, folded into the fake dialog body as ordinary text, exactly as PR-014-C's finding predicted.

**Focus trapping, demonstrated with real input, not asserted.** Two distinct properties are in play here, not one, and only the first is structural:

1. **Structural (the actual security property): terminal output cannot generate any input message at all.** `Message` has exactly five variants (`FocusNext`, `FocusPrevious`, `ToggleTerminalMode`, `Tick`, `DialogActivate`) and `terminal_pane.rs` emits none of them — the PTY is polled via `Tick` and read directly into the grid, with no message variant carrying terminal-originated input. There is therefore no path, guarded or not, by which terminal bytes could reach `dialog_focus`. This was verified by exhaustive enumeration (grepping every `Message` variant and every `dialog_focus` mutation site — three, all inside `update`), which is a stronger proof than a runtime test: enumeration proves no path exists, a test only proves the one path it exercises is blocked.
2. **Flag-routed (ordinary UI state, not a security boundary): while `state.dialog_shown` is true, keyboard `Tab`/`Shift+Tab` route to `dialog_focus` instead of `focus`.** This *is* an ordinary conditional (`Message::FocusNext if state.terminal_mode && state.dialog_shown`), and it is fine as such — it decides which of two legitimate focus targets a real keypress affects, not whether untrusted input can reach either.

`evidence/pr-014-d/focus-trap-tab-cycles-approve-deny.png` shows a real `Tab` keypress moving focus from `Deny` to `Approve` while the adversarial script's own fake `[ Approve ] [ Deny ]` text sits inert in the background. `evidence/pr-014-d/dialog-dismissed-after-enter.png` shows `Enter` activating the focused button, recording the decision in the status bar (`last real-dialog decision: Approve`), and dismissing the dialog — proving the dialog is genuinely interactive, not a static overlay.

**Caveat that does not transfer to the real product (per response 107 §4) — this must not be cited later as though it covered the input-accepting case.** Property 1 above holds *only* because this spike's terminal is output-only: there is no message that could carry a keystroke destined for the terminal, so "terminal output cannot produce a message" is trivially true. The real product's Terminal / Agent Immersion Mode must accept keyboard input — typing into a terminal is the entire point of it. The moment a future RFC (RFC-017, per the roadmap) adds a message carrying terminal-bound input (e.g. `TerminalInput(Vec<u8>)`), this property changes character completely: the trap will then hold only because *routing* sends a given keystroke to the terminal or to the dialog, never both — which is a flag check, with all the fragility a flag check implies, not the absence-of-a-path guarantee this evidence demonstrates. RFC-017 must re-establish focus trapping under that harder condition, almost certainly with a different argument and a real regression test, not by citing this evidence as already covering it.

**Finding, recorded because it cost real debugging time, then quantified per response 107 §Q3 before being trusted for PR-014-E:** `xdotool key --window <id> F2` (the same targeted-delivery method that worked reliably for `Tab` in PR-014-B and PR-014-C) did not deliver the `F2` press to this window in this session, silently — no error, `terminal_mode` simply never toggled across three attempts. `xdotool key F2` (global delivery via whatever the X server currently treats as focused, after `xdotool windowactivate --sync`) worked immediately.

Response 107 was explicit that "dropped events do not produce wrong numbers, they produce missing samples, which skews percentiles invisibly," and asked for a receipt-confirming measurement — sent-count must equal received-count — before this delivery method is trusted for PR-014-E's latency sampling. Built one: a temporary counter incremented in `update()` on every `FocusNext` receipt, logged to stderr, compared against a known number of `xdotool`-sent `Tab` presses. Results:

| Delivery method | Sent | Received | Loss |
| --- | --- | --- | --- |
| `xdotool key --window <id> Tab` | 20 | 11 | 45% |
| `xdotool key Tab` (global, after `windowactivate --sync`) | 20 | 20 | 0% |
| `xdotool key Tab` (global, after `windowactivate --sync`) | 50 | 50 | 0% |

**Targeted (`--window`) delivery silently drops close to half of all synthetic key events in this environment; global delivery (after explicit window activation) is reliably 1:1 at both tested volumes.** This is now a measured fact, not an inferred workaround. The probe code was temporary (a static counter plus one `eprintln!` in `update()`, added and then fully removed — `git diff` against the reviewed commit is empty for `shell.rs`) and is not part of this commit.

**Methodology decision for PR-014-E:** use global `xdotool key` delivery, never `--window`-targeted delivery, for synthetic input. Build the same sent-count-equals-received-count assertion directly into the PR-014-E measurement harness itself, aborting the run on any mismatch rather than computing a percentile over whatever arrived — a one-off investigation here does not substitute for that check being live in the actual measurement code, since the delivery mechanism could still degrade under different timing or load than this bounded test exercised. Root cause of the `--window`-specific loss was not fully isolated (plausibly a niri/XWayland event-delivery quirk specific to `XSendEvent`-style targeted delivery, as distinct from `XTestFakeKeyEvent`-style global delivery) and is not blocking, since a reliable alternative exists and is now quantified.

Gates observed on 2026-07-28: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (375 `tekstide-core` + 18 `tekstide-gui-spike`, 0 failures), `git diff --check` all passed.

### PR-014-E — Measurement

**Machine identification** (recorded before any figure below): CPU AMD Ryzen 9 9950X (16-core/32-thread); RAM 59 GiB; GPU NVIDIA GeForce RTX 5060 Ti, driver 610.43.03, OpenGL 4.6; compositor `niri` (Wayland), with XWayland forced for synthetic-input delivery per the response-107 methodology (native Wayland does not accept `xdotool`-style synthetic input); OS CachyOS Linux, kernel 7.1.5-1-cachyos; Rust 1.97.1. Display: 2560x1440 @ 59.951 Hz (non-native — the panel's preferred mode is 3840x2160 @ 59.997 Hz), **fractional scale 1.2x** (non-integer — recorded per §4's requirement to flag non-standard refresh rate/scaling). All figures below are from `cargo build --release`; no debug-build numbers are recorded anywhere in this section.

**A finding that changes what the C2/C3/C4 numbers below mean — read this before the tables.**

The only mechanism `iced` 0.14 exposes to observe "a frame was submitted for presentation" from application code is `iced::window::frames() -> Subscription<Instant>`, which fires once per `RedrawRequested`. Measuring it required subscribing to it. Exercising that (not inferred from docs) showed it has a side effect: **subscribing to `frames()` causes iced to redraw continuously, at roughly the display refresh rate, with zero input and no animating widgets in the tree.**

Confirmed by direct measurement, not assumption:

| Configuration | `RedrawRequested` events in ~4s idle, zero input | Idle CPU (`utime+stime` ticks/3s, 100 ticks/s) |
| --- | --- | --- |
| `frames()` subscription absent (normal PR-014-B/C/D shell) | not observable (no signal), but CPU confirms no redraw | 0 |
| `frames()` subscription present | 228-231 (~57 Hz) | 8 (~2.7% of one core) |
| Same result under native Wayland (no forced XWayland) | 231 in ~4s | not separately re-measured; CPU test only run under the forced-XWayland configuration |

This is universal to this shell (reproduced under both native Wayland and forced XWayland), not an XWayland artifact. Root cause was not fully isolated within the spike's time budget — plausibly some widget-tree or runtime default requesting `RedrawRequest::NextFrame` rather than `Wait` once anything observes window events this way — and that is recorded honestly as unresolved rather than guessed at further, per this project's "verify empirically, don't infer beyond what you exercised" discipline.

**Consequence for C2/C3/C4: once `frames()` is subscribed to make the measurement possible at all, a new frame is always imminent (within one refresh period, ≈17ms) regardless of whether the input just received actually caused it.** "Time from input-message receipt to the next `Frame` event" therefore does not isolate this input's own rendering cost — it mostly measures "how soon was a frame already due anyway," which, once the loop is hot, is sub-microsecond. The numbers below are genuine, reproducible, and satisfy the literal mandated definition ("input event receipt in the application" → "frame submitted for presentation"), but they are **not evidence that the substrate renders in zero time from a cold, non-redrawing state** — they are evidence that *this specific instrumentation path* is degenerate once active. This is disclosed prominently here, per the honesty checklist, rather than reported as a clean pass.

Because this side effect is real and measured (the CPU-tick table above), `frames()` is **only ever subscribed to during an active measurement run** (`state.measure.is_some()` gates it in `shell.rs::subscription`) — it is never on during ordinary interactive use, and never during the C6 idle-RSS run, so neither the PR-014-B/C/D reviewed behaviour nor the idle-memory baseline is contaminated by it.

**Methodology used for C2/C3/C4, unaffected by the above:** global `xdotool key` delivery (never `--window`-targeted, per the response-107 finding), paced at a fixed interval. The harness never trusts a pre-committed "sent" count: it sends in small batches, checks the *actual line count on disk* in the log file after each batch (a line only exists if the app's own `Measurement::on_input`/`on_frame` pair genuinely ran), and only stops once at least 1,100 **confirmed** samples exist. Every percentile below is computed over confirmed-received samples only — nothing is padded, inferred, or assumed for a key that might not have landed. The delivery-loss rate for each run (dispatched vs. confirmed) is reported as a measured fact, not glossed over:

| Criterion | Dispatched | Confirmed samples | Delivery loss | After 100-sample warmup discard |
| --- | --- | --- | --- | --- |
| C2 typing | 1,275 | 1,112 | 12.78% | 1,012 |
| C3 terminal-under-flood | 1,150 | 1,115 | 3.04% | 1,015 |
| C4 mode switch | 1,100 | 1,100 | 0.00% | 1,000 |

Sustained delivery over ~2-3 minute runs is measurably lossier than the short 20-50-key bursts response 107 validated (0% there) — a real, newly-quantified extension of that finding, not a contradiction of it. All three runs still cleared the mandatory ≥1,000-post-warmup-samples floor.

**C2 (typing), C3 (terminal input under flood), C4 (mode switch) — app-internal latency, input-message receipt to next `Frame` event:**

| Criterion | p50 | p95 | p99 | max | Budget | Result |
| --- | --- | --- | --- | --- | --- | --- |
| C2 typing | 0ms | 0ms | 0ms | 0ms | p95 ≤16ms, p99 ≤33ms | Trivially met, see caveat above |
| C3 terminal-under-flood | 0ms | 0ms | 0ms | 0ms | p95 ≤16ms | Trivially met, see caveat above |
| C4 mode switch | 0ms | 0ms | 0ms | 0ms | p95 ≤32ms | Trivially met, see caveat above |

Every one of the 1,012/1,015/1,000 post-warmup samples measured 0 whole microseconds (truncated, `Duration::as_micros()`), for all three criteria. That degenerate uniformity is itself consistent with the continuous-redraw finding above — it is what you would expect once a frame is always <1ms away. **These are not reported as "C2/C3/C4 pass with headroom" in any meaningful substrate-comparison sense; they are reported as measured, with the instrumentation limitation that makes them measure the wrong thing recorded as the real finding.** C4's "no animation" sub-requirement is confirmed separately and validly, by code inspection: `view()` branches instantly between `terminal_pane_view` and the static Content view with no interpolation, tween, or `iced::animation` usage anywhere in `shell.rs`.

Terminal input (C3) used the RFC-007 `tekstide-pty-spike` flood-harness pattern: `i=0; while true; do printf '...'; i=$((i+1)); done &` backgrounded in the pane's own shell, so the interactive prompt stays available for measured keystrokes to be written into the same PTY concurrently with the flood.

**C5 (warm startup) — process start (first line of `main()`) to first `Frame` event, unaffected by the C2-C4 caveat** (only the *first* frame after a cold process start is timed; the continuous-redraw side effect only matters for *subsequent* frames, which are never used here):

15 consecutive release-binary launches. First discarded as cold (257.3ms); the following 14 are warm:

| n (warm) | min | median | mean | max | Budget | Result |
| --- | --- | --- | --- | --- | --- | --- |
| 14 | 200.8ms | 227.9ms | 224.8ms | 255.5ms | ≤800ms | **Met**, comfortably |

**C6 (idle RSS)** — one project, one terminal pane open (real F2 toggle), 60s idle, plain interactive shell (no `TEKSTIDE_MEASURE_CRITERION` set, so `frames()` is not subscribed and this baseline is not contaminated by the continuous-redraw finding above). `/proc/<pid>/status` `VmRSS` used (the RFC-013-baseline note that `/usr/bin/time -v` was unavailable still applies; it remains unavailable in this environment).

| Point | VmRSS |
| --- | --- |
| Immediately after opening the terminal pane | 176,176 kB |
| After 60s idle | 178,124 kB |

**Baseline figure, not pass/fail, per §4.** ~174 MiB idle with one project and one terminal open. Growth over the idle window (+1,948 kB) is small and consistent with normal allocator/heap behaviour, not a leak signature — though 60s is far too short a window to rule out a slow leak either way; this is a snapshot, not a leak-detection pass.

**C7 (font metrics / column count).** Measured headlessly via `iced::advanced::graphics::text::Paragraph::with_text` — the exact layout primitive iced's own `Text` widget uses internally (`cosmic-text`-backed), not a guessed pixel-per-character constant. 200 repeated `"M"` glyphs measured and averaged, at the same 13px monospace size used throughout the shell:

- Monospace glyph advance: **7.8000 logical px** at 13px.
- This measurement is scale-invariant by construction: iced always lays text out in logical pixels; the compositor applies the scale factor afterward. "1x and a fractional scaling factor" (§4) is satisfied by this invariance holding in the actually-running app: the real desktop in this session runs at the non-integer 1.2x scale recorded in machine identification above, and the i18n screenshots below (taken on that same real, fractionally-scaled desktop) show correctly-shaped, non-garbled text — the computation is not merely a paper claim, it is what the running app's own text layer actually used to produce every screenshot in this file.
- Column-count computation, applying the shell's own real padding (`terminal_pane_view`'s 8px-per-side body padding, `zone_container`'s 8px-per-side padding) to the actual observed window logical width (1,042px, tiled by `niri` on this display):
  - Terminal pane (full width minus 16px padding): (1,042 − 16) / 7.8 ≈ **131 columns**.
  - Content-mode main area (80% of width, minus sidebar, minus 16px padding): (1,042 × 0.8 − 16) / 7.8 ≈ **104 columns**.
- **Limitation, disclosed rather than implied away: the terminal pane's actual PTY grid is hard-coded to 80×24 (`terminal_pane.rs::{ROWS, COLS}`) and does not consume this computation.** C7 asks the spike to "demonstrate computing" column count from real font metrics, which this does; it does not ask the spike to wire that computation into live grid resizing, which remains real product work (dynamic resize-on-window-change) not attempted here.

**C10 (i18n: CJK + RTL in both editor and terminal surfaces).** Sample (`shell.rs::I18N_SAMPLE`): Simplified Chinese, Japanese, and Arabic lines plus a plain-ASCII control line, shown via `TEKSTIDE_I18N_DEMO=1` in both the Content-mode editor surface and the terminal pane (printed via `printf` on a real, running shell — real evidence, not simulated). Screenshots: `evidence/pr-014-e/i18n-editor-surface.png`, `evidence/pr-014-e/i18n-terminal-surface.png`.

- **Editor surface:** all three scripts render correctly, including full Unicode bidi reordering for the Arabic line — the label `العربية:` and the sentence read in correct right-to-left visual order, matching what a real Arabic reader would expect. This is `cosmic-text`'s standard text-shaping path (the same one used for every other `text!`/`rich_text!` widget in the shell), not special-cased for this demo.
- **Terminal surface:** CJK and Arabic glyphs both render (shaped correctly at the individual-glyph level — Arabic letters still join/connect properly within each printed line), but **the terminal grid does not apply bidi reordering**: the Arabic line appears in raw left-to-right *cell* order (the order bytes were written to the grid), not visually reordered right-to-left the way the editor surface shows it. This is a genuine, exercised difference between the two surfaces, not a bug specific to this spike's code — real terminal emulators (this one included, via `alacritty_terminal`'s grid model) operate on a monospace cell grid rather than shaped text runs, and generally do not implement the Unicode bidi algorithm at all. Any real product terminal surface wanting correct RTL rendering would need to address this as a genuine, non-trivial gap, not something this spike's filter or rendering choice introduced.
- **Second limitation, also visible in the terminal screenshot:** CJK characters occupy exactly one grid cell each here, not the two cells a real terminal emulator gives "wide" (fullwidth) characters. `alacritty_terminal`'s grid supports wide-character cells, but this spike's minimal rendering path (`terminal_pane.rs::styled_rows`) was not built to consume that distinction. Recorded as a rendering-fidelity gap for CJK column alignment, not silently implied complete.
- Both gaps are about the terminal-grid rendering path specifically; the editor surface (a plain `iced::widget::text`/`rich_text` column) has neither issue.

Gates run on 2026-07-28 after this section's code changes (`shell.rs` measurement instrumentation, `terminal_pane.rs` flood/i18n senders, `font_metrics.rs`, `main.rs` CLI dispatch): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (375 `tekstide-core` + 18 `tekstide-gui-spike`, 0 failures), `git diff --check` — all passed.

## Criteria Not Evaluated

- **C11 (screen-reader path), the screen-reader half specifically.** Focus indicators are visible (C9/PR-014-B/D evidence covers that half). The screen-reader/assistive-technology half could not be evaluated: `iced` 0.14 has no accessibility bridge at all — grepped for `accesskit`/`accessibility`/`a11y` across `iced-0.14.0`'s `Cargo.toml` and `iced_winit-0.14.0`'s source; zero matches, not feature-gated-off, genuinely absent. This matches the handoff's own example of a useful non-evaluation: no bridge exists to test against, on any platform, for this toolkit at this version.
- **C12 (Windows/macOS blockers), beyond what was noticed in passing.** Per §8, cross-platform builds were not attempted and blockers were not chased. What was noticed without chasing it: the spike depends on `tekstide-core::runtime::terminal::LinuxTerminalRuntime` directly (Linux-only by name and implementation — PTY via `nix`/`libc`), and all measurement tooling in this section (`xdotool`, `niri msg`, `/proc/<pid>/status`) is Linux-specific test infrastructure, not app code. Neither `iced` nor `alacritty_terminal`/`vte` is known to be Linux-only upstream (both advertise cross-platform support), so the concrete, spike-introduced blocker is narrower than "the whole approach" — it is specifically the terminal-runtime layer, which RFC-007/013 already scoped as Linux-only for this milestone.

Every other criterion (C1-C10, C13, C14) has evidence recorded above or in the PR-014-B/C/D subsections.

## Known Limitations

- The spike measures **app-internal latency** (input event receipt to frame submission). It excludes input-stack latency before the application sees the event and compositor/display latency after submission. These figures are not end-to-end. **For C2/C3/C4 specifically, this exclusion is compounded by a further, measured limitation:** the only mechanism available to observe frame submission (`iced::window::frames()`) forces continuous redraw once subscribed (confirmed: 0 vs. ~8 CPU ticks/3s idle), making "time to next frame" measure mostly how soon a frame was already due rather than this input's own cost. See the PR-014-E section above for the full finding; the recorded 0ms figures should not be read as evidence of zero real-world input latency.
- Linux only. No Windows or macOS evidence is produced or claimed. See Criteria Not Evaluated for what was noticed about the concrete blocker (the Linux-only terminal runtime) versus the GUI substrate itself (not known to be Linux-only).
- No accessibility/screen-reader bridge exists in `iced` 0.14 to evaluate against (see Criteria Not Evaluated, C11).
- Spike code is measurement code and is not expected to carry meaningful test coverage.
- `iced = "0.14"` is declared with default features enabled, plus the `advanced` feature (needed for PR-014-E's C7 headless font-metrics measurement, `iced::advanced::graphics::text::Paragraph`); `default-features = false` with a reviewed, minimal feature set was not applied in this slice (acceptable for a disposable spike, per the RFC-013 precedent that this discipline matters once a dependency is product-facing). If `iced` becomes a product dependency at M8, its feature surface needs the same review `rusqlite` received in RFC-013, and the licence inventory must extend to its transitive native dependencies — `wgpu` and the font-shaping stack (`cosmic-text`, `fontdb`) in particular.
- PR-014-D's dialog is a single fixed instance (`dialog_shown`/`dialog_focus`/`dialog_decision` in `shell.rs`), not a general reusable dialog/modal system. That generalization is M8 product work, not spike scope.
- The adversarial script's box-drawing characters render correctly but faintly (thin single-line-weight glyphs) in the default monospace font at the tested size; a real attacker tuning for visual conviction would likely test multiple fonts. This does not change the C8 verdict (the genuine dialog is still clearly distinguishable by its opaque GUI-layer rendering, not by the fake's glyph fidelity), but it means this corpus is not a maximally-adversarial visual attempt.
- Keyboard-only interaction is exercised (`Tab`/`Shift+Tab`/`Enter`); no mouse-click path onto the dialog buttons is implemented or tested in this slice.
- The terminal pane's PTY grid is hard-coded to 80×24 and does not consume the C7 column-count computation; the terminal grid does not apply Unicode bidi reordering (real terminal-grid behaviour, not this spike's choice) and does not give CJK characters double-width cells (a rendering-fidelity gap this spike's minimal renderer does not address). See the C7/C10 write-ups above.
- Global `xdotool key` delivery, while reliable at 0% loss for short bursts (response 107), showed measured, non-zero delivery loss (3-13%) over the longer ~2-3 minute runs C2-C4 required. The measurement harness adapted to this by confirming actual receipt via on-disk log line counts rather than trusting a pre-committed sent count, but the loss itself was not root-caused within this spike's time budget.
