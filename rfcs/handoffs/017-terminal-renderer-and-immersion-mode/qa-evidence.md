---
title: "RFC-017: Terminal Renderer and Immersion Mode - QA Evidence"
rfc: "RFC-017"
rfc_file: "../../proposed/017-terminal-renderer-and-immersion-mode.md"
status: "Accepted 2026-08-01 — PR-017-B/C/D reviewed and approved (responses 144-149); PR-017-E implemented 2026-08-03, approved with one required item (response 150), fixed and re-gated same day"
target_milestone: "M9"
created: "2026-08-01"
---

# RFC-017 QA Evidence

Record observed gate output, findings, and limitations here as each slice lands. One section per slice.

**This file is where results are written. It is not where instructions live** — if a later review defers something into a future slice, that obligation belongs in that slice's handoff document, not only here. Two slices (RFC-016 PR-016-E, RFC-015 PR-015-E) accumulated their real scope in this file and had to have it consolidated afterwards.

## PR-017-B — Filter promotion

Promoted `crates/tekstide-gui-spike/src/filter.rs` (RFC-014 PR-014-C) into `crates/tekstide/src/surface/terminal/filter.rs`. Not a rewrite: same interposition shape (a `vte::ansi::Handler` wrapping the real one, forwarding an explicit accepted set and blocking everything else by omission). What changed: every accepted method's forwarding decision now asks `tekstide_core::runtime::terminal::security::TerminalSequencePolicy::ACCEPTED` at the call site, rather than the shell crate holding its own copy of "which methods are OK."

**A necessary `tekstide-core` addition, disclosed rather than folded in silently.** The mouse/focus-vs-ordinary-private-mode distinction (`TerminalSequenceFamily::MouseFocusReporting` vs `PrivateMode`) existed only as a byte-substring check inside `parser.rs`'s private `classify_private_mode(body: &[u8])`, keyed on raw CSI bytes this filter never sees — `vte` hands the filter an already-parsed `PrivateMode`/`NamedPrivateMode` value instead. Re-implementing the same 1000/1002/1003/1004/1005/1006 list in the shell crate would have been exactly the duplicate-classifier risk `implementation-handoff.md` §3 warns against, so a new, additive `classify_private_mode_number(mode: u16) -> TerminalSequenceFamily` was added to `parser.rs` and re-exported alongside the existing types — the byte-based function is untouched, and a new test (`classify_private_mode_number_agrees_with_the_byte_based_classifier`) proves the two entry points cannot silently disagree.

### P1 — Single ingress

**Enumeration, written out.** Every construction of an `alacritty_terminal`/`vte` type, anywhere in `crates/tekstide/src`:

```
$ grep -rl "alacritty_terminal\|vte::" crates/tekstide/src/
crates/tekstide/src/surface/terminal/filter.rs
crates/tekstide/src/surface/terminal/filter/tests.rs

$ grep -rn "Term::new\|Processor::<\|Processor::new\|grid_mut\|\.advance(" crates/tekstide/src/
crates/tekstide/src/surface/terminal/filter/tests.rs:65:    Term::new(...)
crates/tekstide/src/surface/terminal/filter/tests.rs:73:    Processor::<...StdSyncHandler>::new()
crates/tekstide/src/surface/terminal/filter/tests.rs:79:    processor.advance(&mut filter, chunk)
(two mentions of `grid_mut()` in `filter.rs`'s own doc comments -- the string appears, the call does not)
```

**Exactly two files touch these types at all, and the only `Term`/`Processor` construction and the only `.advance()` call are in the test harness.** There is no production caller in this slice — PR-017-C builds the pane. The test harness's own construction (`feed_chunks`, `filter/tests.rs`) is the single call site, and it always pairs a fresh `Term` with a `Processor` that persists across every chunk in one call, with every byte routed through exactly one `SecurityFilter::new(&mut term)` per chunk — never a second, unfiltered path to `term`.

**Ablated, not just enumerated**: temporarily changed `set_title` to forward unconditionally (`self.inner.set_title(title)`, bypassing `accepts`) — simulating what a second ingress into title-mutation would look like. `every_named_family_blocks_with_no_grid_effect_at_every_split_boundary` failed immediately, naming `osc_title` and reporting `blocked = []`. Reverted.

### P2 — No side channels

`Term::grid_mut()` is public API for direct grid manipulation and is not reachable from the PTY byte stream (`vte::ansi::Processor` never calls it — confirmed by reading `vte` 0.15's `ansi.rs` dispatch tables directly, same finding the spike already recorded). The enumeration above is P2's evidence too: `grid_mut` appears nowhere as a call, only inside doc-comment prose explaining why it must not. **This slice's own claim is necessarily partial**: P2's full system-wide claim ("no code anywhere in the crate reaches `Term`'s mutating API outside the filter") has no real pane yet to violate it — the same "headless, proven directly, no real caller yet" shape this project has used before (`i18n::Catalog` pre-RFC-015, `TextStream` pre-this-RFC). PR-017-C, which gives `Term` a real owner, is where this enumeration must be re-run against real production code, not only the test harness.

### P3 — Classification parity

Holds by construction, unchanged from the spike's own reasoning: filter and emulator share one `vte::ansi::Processor` instance per PTY-read chunk sequence, so there is exactly one parse, never two implementations of "where does this sequence begin and end" to drift apart.

### P4 — Stream-position independence

Also holds by construction: the `Processor` is long-lived across `advance()` calls within one `feed_chunks` invocation, so its internal parser state survives a chunk boundary.

**Ablated together, since both properties rest on the same design choice.** Temporarily changed `feed_chunks` to construct a **fresh** `Processor` per chunk instead of one shared across all chunks — simulating what "two independent parses" would actually do to a stream split mid-sequence. Four tests failed independently, each for a different reason, confirming this is not one lucky assertion:

- `every_named_family_blocks_with_no_grid_effect_at_every_split_boundary`: `osc_title` split at 1 leaked `]0;PWNED-TITLE` into the grid — the second chunk, parsed from a fresh `Ground` state, treated the sequence's tail as ordinary printable text.
- `generic_dcs_content_never_reaches_handler_at_every_split`: DCS payload leaked at split 1, for the same reason.
- `v4_unterminated_osc_continues_correctly_into_next_chunk`: the OSC 52 clipboard sequence, split exactly where the spike's own V4 finding requires it not to be, stopped being recognized as blocked at all.
- `v7_utf8_split_reassembles_correctly_at_every_boundary`: a UTF-8 codepoint split at byte 1 failed to reassemble.

All four reverted to the shared-`Processor` design and re-confirmed passing.

### The corpus

Eight named families (`BLOCKED_FAMILY_CORPUS`, `filter/tests.rs`), covering every family this filter's `Handler` impl explicitly classifies: OSC title, OSC 52 clipboard, OSC 8 hyperlink, ordinary private mode, mouse/focus-reporting private mode, keyboard protocol, terminal query, and SCP (which has no dedicated `TerminalSequenceFamily` variant and correctly falls back to the generic unsupported-CSI `Csi` bucket, matching `parser.rs`'s own fallback for an unrecognized CSI final byte). One test (`every_named_family_blocks_with_no_grid_effect_at_every_split_boundary`) iterates the table and every internal split point of each sequence: **80 generated split points, 88 total cases including the unsplit baseline per family**, computed precisely (not estimated) from each sequence's byte length. Each case asserts both the expected family in `blocked` and, going beyond the spike's own marker-absence check, **full grid-plus-cursor snapshot equality against a pristine baseline** — a stronger standard than "the secret string doesn't appear on line 0," satisfying the review gate's explicit ask to compare full grid state, not just the cursor.

Sequences for the two families not in the spike's own corpus were verified directly against `vte` 0.15's `ansi.rs` `csi_dispatch` match arms, not assumed: `?2004h` (ordinary private mode, `NamedPrivateMode::BracketedPaste`) and `>1u` (keyboard protocol, `push_keyboard_mode` — final byte `u`, intermediate `>`).

**Truncated/malformed sequences fail closed without unbounded buffering**: this property belongs to `vte::ansi::Processor` itself (its internal parameter/intermediate buffers are fixed-capacity, verified by inspection — this filter never buffers anything of its own; every `Handler` method call it receives is already a complete, bounded operation). `v5_parameter_overflow_does_not_desync_the_parser`/`v5_parameter_overflow_followed_by_osc_52_still_blocks_clipboard` (carried from the spike) prove an attacker cannot use parameter-list length to desync classification for what follows.

**A design gap found by running the ablation, not by inspection.** The first version of this filter gated every accepted method's forward call on `accepts(...)` but recorded nothing when the gate declined — so removing an entry from `TerminalSequencePolicy::ACCEPTED` silently dropped the operation without ever appearing in `blocked`, and the delegation ablation below would have passed for the wrong reason. Fixed with `forward_if_accepted`, a shared helper that records a `blocked` entry symmetrically with the explicitly-classified blocks. `accepted_clear_screen_actually_clears_previously_written_text` (new) also checks the actual grid effect directly, not merely the blocked-list's absence, so a future silent-drop regression is caught even if the diagnostic wiring regresses too.

**Delegation ablation, with the fix in place**: temporarily removed `CsiClearScreen` from `TerminalSequencePolicy::ACCEPTED` in `tekstide-core`. Three tests failed: the transcription check (`accepted_sequence_variants_used_by_this_filter_match_the_nine_forwarded_methods`), `accepted_sgr_cursor_and_clear_do_not_block`, and `accepted_clear_screen_actually_clears_previously_written_text` — each reporting `blocked = [Csi]`. Reverted; all pass again.

### Gates

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (492 `tekstide-core` — up from 491, 1 net new — + 84 `tekstide` — up from 70, 14 net new — + 18 `tekstide-gui-spike`, 0 failures), `git diff --check`, `cargo tree -p tekstide-core --edges normal | grep -ciE 'vte|alacritty'` → `0` — all passed.

### What this slice does not do

Per `pr-017-b-filter-promotion.md`'s own scope: it does not fix or wire anything into a real pane (PR-017-C), does not wire `plain_terminal_observation` (PR-017-F), and does not decide the Tab-to-terminal or hidden-session questions (PR-017-D/E). The spike's `filter.rs` is marked superseded (module-doc line naming this file and the commit) rather than deleted — the crate itself stays until PR-017-E per RFC-014 §"When the spike crate is deleted."

## PR-017-C — Terminal pane rendering

`crates/tekstide/src/surface/terminal.rs` gains `TerminalPane`: a PTY-backed pane rendering the emulator grid under RFC-015's surface contract (`pub fn view<'a, Message: 'a>(pane: &TerminalPane, font_size: f32) -> Element<'a, Message>`, the same shape `surface::board::view` uses). No input yet — `TerminalPane` exposes no public method that writes to the PTY; only `#[cfg(test)]` code does, the same way `shell::tests`'s fixtures construct state a real user action would otherwise produce. `filter::SecurityFilter` (PR-017-B) gets its first production caller here; `#[allow(dead_code)]` was removed from the `filter` module declaration accordingly (`clippy -D warnings` confirms nothing is unused now).

An improvement over the RFC-014 spike's own `terminal_pane.rs` template, found before writing any code: `alacritty_terminal::event::VoidListener` is a real, already-public, no-op `EventListener` in `alacritty_terminal` 0.26 itself (`event.rs:108-110`) — used directly, rather than reimplementing the spike's private `NullListener` type.

### P1 — Single ingress, re-enumerated against production code

Response 144/146-147 named this slice's obligation explicitly: PR-017-B's own enumeration covered a crate whose only `Term`/`Processor` construction was a test harness — a true statement about a system with no production caller, not a system-wide guarantee. This is that re-enumeration:

```
$ grep -rn "Term::new\|Processor::new\|Processor::<" crates/tekstide/src | grep -v "surface/terminal/tests.rs"
crates/tekstide/src/surface/terminal.rs:139:            processor: Processor::new(),
crates/tekstide/src/surface/terminal.rs:140:            term: Term::new(pane_config(), &PaneSize, VoidListener),
crates/tekstide/src/surface/terminal/filter/tests.rs:65:    Term::new(Config::default(), &SIZE, RecordingListener::default())
crates/tekstide/src/surface/terminal/filter/tests.rs:73:    let mut processor = Processor::<alacritty_terminal::vte::ansi::StdSyncHandler>::new();

$ grep -rn "\.advance(" crates/tekstide/src | grep -v "surface/terminal/tests.rs"
crates/tekstide/src/surface/terminal.rs:163:        self.processor.advance(&mut filter, &bytes);
crates/tekstide/src/surface/terminal/filter/tests.rs:79:        processor.advance(&mut filter, chunk);

$ grep -rn "grid_mut" crates/tekstide/src
(no matches outside doc-comment prose in terminal.rs/filter.rs/filter/tests.rs explaining why it must not be called)

$ cargo tree -p tekstide-core --edges normal | grep -ciE 'vte|alacritty'
0
```

**Exactly one production `Term`/`Processor` construction site and exactly one production `.advance()` call**, both in `TerminalPane::launch`/`TerminalPane::poll` (`terminal.rs:139-140`, `:163`), and the `.advance()` call always passes through `SecurityFilter::new(&mut self.term)` first — never a second, unfiltered path to `self.term`. `tekstide-core` still carries zero `vte`/`alacritty_terminal` dependency.

**Ablated at the real call site, not just enumerated.** Temporarily changed `poll()` to call `self.processor.advance(&mut self.term, &bytes)` directly, bypassing `SecurityFilter` entirely — simulating exactly the regression P1's re-enumeration exists to catch (a second, unfiltered path this crate's own production code takes).

- First attempt at a distinguishing test used OSC 0 (set title) as the disallowed sequence sent through a real, launched pane's real PTY. It passed even with the filter bypassed — a **"test that cannot fail," caught before being trusted**: `alacritty_terminal::Term::set_title` stores the title in a private field with no grid effect either way, so blocking it can never be told apart from bypassing the filter by inspecting the rendered grid alone. Discarded rather than kept as false coverage.
- Redesigned around CSI `?1049h` (DECSET, switch to the alternate screen buffer) instead: forwarding it genuinely swaps which grid `renderable_content()` reads from, a real, observable difference. Sent via a real `printf` executed inside the launched shell (not written directly to the PTY master as raw bytes — canonical-mode local echo reflects raw input back in `^X` caret notation (`ECHOCTL`), not as real control bytes, so a direct write would never actually reach `Processor::advance` as a genuine CSI sequence in either the filtered or bypassed case; a `printf`'s stdout is real process output, not echoed input).
- With the filter bypassed: the pre-alt-screen marker (`PRIMARY_SCREEN_017C`) disappeared from the rendered grid — confirming the ablation actually changes the outcome. Reverted; the marker is present again with the filter restored.

### P2 — No side channels

`Term::grid_mut()` is not called anywhere in production code (confirmed by the enumeration above); the only non-byte input `TerminalPane`'s own `Term` receives is its fixed, construction-time `PaneSize` (80×24) — not a live resize path (RFC-017 PR-017-E's job). `TerminalPane` does nothing with `SecurityFilter::blocked`'s contents beyond letting the value drop at the end of each `poll()` call — no logging, no forwarding, no side effect of any kind for a blocked family's payload.

### Bounded scrollback

`SCROLLBACK_LINES = 2_000`, well under `alacritty_terminal`'s own 10,000-line default, set explicitly via `Config { scrolling_history: SCROLLBACK_LINES, ..Config::default() }` rather than left at the library default.

**Tested under sustained output** (`bounded_scrollback_holds_under_sustained_output`): two `Term`s, one configured at `SCROLLBACK_LINES`, one at double that, both fed the same `SCROLLBACK_LINES * 2 + ROWS + 500` lines (deliberately more than *either* configured bound, not just the smaller one — feeding only enough to exceed the smaller bound would let the larger `Term` merely reflect its natural, unclamped scroll count, passing the "doubling the bound doubles the retained total" assertion below for the wrong reason). Result: the narrow `Term` holds exactly `ROWS + SCROLLBACK_LINES` total lines; the wide one holds exactly `ROWS + SCROLLBACK_LINES * 2` — proving the cap tracks the configured bound, not an incidental number both `Term`s would hit regardless.

**Ablated**: temporarily reverted `pane_config()` to `Config::default()` (the library's unbounded-by-comparison 10,000-line default). The test failed immediately: `left: 4525, right: 2024` — confirming the assertion genuinely depends on the explicit bound, not a coincidence of the test's own line count. Reverted.

### Rendering fidelity

`renders_full_grid_plus_cursor_snapshot_for_known_output`: feeds known bytes (plain text plus one SGR-coloured line) directly to a bare `Term`/`Processor` (no PTY — this test is about `styled_rows`'s own correctness, not the PTY plumbing, which the real-PTY tests below cover separately) and asserts the **entire 24-row grid, plus cursor position**, against a pristine baseline — not marker presence on one line, carrying forward PR-017-B's own corpus standard as the house standard the review gate named. Every unwritten cell is a real, present blank cell (space character, default foreground) rather than absent, since the grid is always `COLS` wide; the baseline reflects that directly rather than assuming rows end where written text does.

**Ablated**: temporarily changed the ANSI green resolution (`resolve_color`) from `[0.0, 0.75, 0.0]` to `[0.0, 0.80, 0.0]`. The snapshot test failed, diffing the exact row and channel that changed. Reverted.

### Real-PTY end-to-end tests

- `a_launched_pane_renders_real_pty_output_end_to_end`: launches a real `/bin/sh` via `TerminalPane::launch`, writes a `printf` command through a `#[cfg(test)]`-only input path, polls until the marker appears in the rendered grid. Proves the accept path works end-to-end through the real production `poll()` call site, not just PR-017-B's own test-harness corpus.
- `a_launched_pane_blocks_a_disallowed_sequence_at_the_real_call_site`: the P1 ablation target above, kept as a permanent regression test.

### Gates

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (492 `tekstide-core` — unchanged + 88 `tekstide` — up from 84, 4 net new — + 18 `tekstide-gui-spike` + 0 `tekstide-pty-spike`, 0 failures), `git diff --check` — all passed.

### A necessary scan-policy carve-out, flagged rather than silently added

`shell::tests::no_raw_color_construction_anywhere_in_the_crate` (RFC-015's own mechanical scan: every colour must come from `state.theme`) initially failed against `terminal.rs`'s `Color::from_rgb(...)` call in `view`. Added `terminal.rs` to `is_scan_exempt`'s match arm in `crates/tekstide/src/shell/tests.rs`, with the reasoning recorded in that file rather than only here: the colour the scan otherwise forbids is the terminal grid's *own*, PTY-determined ANSI colour (RFC-016's grid exception — untrusted bytes render as data, unescaped, the one place that exception applies), not a chrome role `state.theme` defines. Distinct from `theme.rs`'s existing exemption (which defines the palette chrome draws *from*); this is the scan's own module doc anticipating "a new file that genuinely needs a literal" rather than being silently exempted. Flagged here for the reviewer to confirm, not assumed correct unilaterally.

### Screenshot evidence, real PTY session

Captured with the owner's explicit approval (`AskUserQuestion`), per response 127's standing convention: `env -u WAYLAND_DISPLAY TEKSTIDE_TERMINAL_DEMO=1 ./target/debug/tekstide <scratch-project-path>`, `xdotool search --name Tekstide`, `xdotool windowfocus --sync <id>`, `xdotool key --clearmodifiers ctrl+alt+m` to toggle Terminal Mode, screenshotted via `niri msg action screenshot-window --id <niri-id> --path <file>` (`rfcs/handoffs/017-terminal-renderer-and-immersion-mode/evidence/pr-017-c/`):

- `00-initial-content-mode.png` — Content Mode's placeholder, before any toggle.
- `01-terminal-mode-initial-shell.png` — Terminal Mode after the toggle: a real `/bin/sh` prompt (`tekstide$`) rendered through `TerminalPane::view`, launched via `TEKSTIDE_TERMINAL_DEMO`'s scratch, temp-dir shell (matching `TerminalPane::launch`'s own test/spike precedent) — **not** wired to the active project's own terminal session (that wiring is PR-017-D/PR-017-E's job). The chrome-level focus border (blue, 2px) remains on the outer container; only the inner placeholder text was substituted.
- `02-back-to-content-mode.png` — toggled back; matches `00`'s layout, confirming the round-trip and that the sidebar's focus marker (`"> "` restored on the main-area label) is unaffected by the pane's presence. (A first attempt at this second toggle silently failed to deliver — confirmed via `md5sum` showing `02` byte-identical to `01` — the same synthetic-input reliability finding PR-015-E recorded; a retry with a fresh `windowfocus --sync` succeeded.)

**What this proves**: a real, filtered PTY session renders as a genuine RFC-015 surface, toggling correctly with the existing mode-switch command, without touching chrome. **What this does not prove**: trusted-UI separation or spoofing resistance (RFC-018's job, not claimed here) — nor does the demo pane exercise real project-terminal session lifecycle, which is unrelated to and unblocked by this evidence.

### Known limitation, disclosed rather than fixed silently

`terminal_demo_pane`'s scratch temp directory (`$TMPDIR/tekstide-terminal-demo-<pid>`) is not cleaned up on exit — matching the RFC-014 spike's own `TerminalPane::launch` precedent (also uncleaned), and low-stakes for a diagnostic, env-gated path only ever run manually. Not fixed here since this demo path is not shipped, real user-facing behavior.

### Deferred, per the shared Surface Checklist

`TerminalPanePolicy`/`TerminalLayoutClass`/`visible_terminal_limit`, real font-metrics/DPI-driven split sizing, colour-independent session-state distinction, and the hidden-session grid-state decision are unchanged from `task-breakdown-pr-plan.md`'s own scoping — all four are PR-017-E's job ("Immersion mode, split policy, session bar"), not this slice's. This slice's pane is fixed at 80×24 and is the only terminal surface in the application; there is no split or session bar yet for those items to apply to.

### Review outcome (response 148)

**Approved, no required items.** P1/P2's re-enumeration against production code confirmed independently (same grep, same result: exactly one construction/advance site, always filtered). The `terminal.rs` colour-scan exemption confirmed correct, with an expiry recorded: it holds only because the file's single `Color::from_rgb` call is currently the grid's own PTY-determined colour — the moment PR-017-E gives this file chrome (a session bar, a pane focus indicator), the file-level exemption stops being justified by that claim and must narrow or move. Both obligations are recorded directly in `task-breakdown-pr-plan.md`'s PR-017-E entry, not only here.

**Named as the third instance of the same pattern this RFC**: a green ablation result that was wrong (PR-017-B's silent-decline gap, the classifier parity test, and this slice's OSC-0 ablation, which passed even with the filter bypassed because `set_title` has no grid effect to lose). Recorded here as a standing reminder for any future ablation in this RFC: a passing result after deliberately breaking something is not evidence until the failure mode it's supposed to catch has been confirmed observable through the same lens the test uses.

## PR-017-D — Input

`TextStream` gets its first real production caller. `input::terminal_surface::TextStream::to_pty_bytes` converts an already-routed keystroke into the bytes a PTY receives (printable UTF-8, Enter → `\r`, Backspace → `0x7f`, Escape, Tab, Space, the four arrow keys as normal-mode `CSI` sequences, `Ctrl`+ASCII-letter control codes — disclosed as not a complete VT100/xterm encoder, not claimed as one). `crate::surface::terminal::TerminalPane` gains `terminal_id()` and a real (non-test-gated) `write_input`; `shell::update`'s `RoutedInput::Terminal` arm is the one production call site, gated on `state.modal.is_none()` and `terminal_stream_targets_the_demo_pane`.

### `TextStream` targets the real, live demo pane

`active_terminal_focus` (new, `shell.rs`) computes `non_modal_subscription`'s `terminal_focus` parameter for real — `Some(pane.terminal_id().clone())` exactly when `FocusZone::MainArea` is focused *and* the active project is in `TerminalImmersion` mode (matching `main_area_view`'s own substitution condition), `None` otherwise. No longer the hardcoded `None` RFC-015 shipped with.

**The demo pane is deliberately not registered on the real `ApplicationShell` project model** (PR-017-C's "no state duplicating `tekstide-core`" contract holds), so the existing, real, core-backed `terminal_stream_targets_a_live_terminal` correctly cannot recognize it — confirmed by reading `ApplicationShell`: there is no public API today to attach a running `TerminalSession` to the active project (`AppState::active_project_mut` is private; `ProjectSession::add_terminal_session` has no caller outside `tekstide-core` itself). Rather than add that `tekstide-core` API to make the demo path "real" in core's model — scope creep into PR-017-E/F's actual job of real session registration — a sibling, demo-scoped check (`terminal_stream_targets_the_demo_pane`) gates delivery instead. `terminal_stream_targets_a_live_terminal` keeps its exact existing meaning, now `#[allow(dead_code)]`-suppressed with a comment naming PR-017-E/F as its real caller, the same shape `filter.rs`'s own pre-PR-017-C suppression used.

### The Tab decision, recorded

**Tab does not reach the terminal. It always cycles shell focus.** This was already `route_non_modal_input`'s precedence (Tab/Shift+Tab checked before `terminal_focus`, inherited unchanged from RFC-015/PR-015-C) — this slice's job was to decide whether that precedence is the final answer or a placeholder, and record why. Decided: no shell completion or other terminal-Tab feature exists yet to justify the risk, while an inescapable focus trap is a real, immediate risk the moment any terminal is focusable at all (which, as of this slice, it now genuinely is). The escape hatch is structural: Tab is intercepted in routing *before* `terminal_focus` is even consulted, so the terminal is never given the chance to consume it — "must not depend on the terminal cooperating" is satisfied by the key never reaching one. Recorded in `input`'s own module doc, not only here.

**Proven with a real, live terminal**, not only the pre-existing headless proof (`input::tests::tab_cycles_focus_even_with_a_terminal_focused`, unchanged): `tab_cycles_shell_focus_with_a_real_terminal_focused_and_writes_nothing` routes a real Tab press with the demo pane's actual `TerminalId` as `terminal_focus`, dispatches the result through the real `update`, confirms `state.focus` moved, and polls the real pane confirming no tab byte reached it.

**Ablated**: temporarily swapped the two precedence checks in `route_non_modal_input` (`terminal_focus` before Tab). Both the pre-existing headless test and the new live-terminal test failed immediately, each reporting `Terminal(TextStream {..})` where `FocusNext` was expected. Reverted.

### Modal exclusivity, demonstrated with a real terminal

`modal_open_blocks_pty_write_and_closing_it_resumes_delivery`: a real, launched `TerminalPane`, a `TextStream` addressed to its actual id, delivered through the real `update` — once with `state.modal = Some(ModalContent::default())`, once with it cleared. The blocked half polls the pane and confirms the character never renders; the accepted half (same stream, same pane, modal cleared) confirms it does, ruling out "the pane was simply broken" as an alternative explanation for the earlier silence.

**Ablated**: temporarily removed the `state.modal.is_none()` guard from `update`'s `RoutedInput::Terminal` arm. The blocked half of the test failed immediately (the character appeared while the modal was still open). Reverted.

**No GUI screenshot for the modal case, disclosed rather than manufactured.** The demo modal (`TEKSTIDE_LAYER_DEMO`) opens exactly once, at boot, with no runtime trigger to reopen it (RFC-015 PR-015-B's own design: no real dialog trigger exists until RFC-022). There is no real, user-accessible sequence that gets Terminal Mode active *and* the modal open at the same time — toggling to Terminal Mode requires `Ctrl+Alt+M`, a global keybinding, and `modal_subscription()` (active whenever the modal is shown) has no path to routing global keybindings at all. The live-`TerminalPane` test above is the demonstration this property gets; a screenshot was not force-fit around a launch-order coincidence.

**Two independent mechanisms, confirmed by response 149, each the other's defence.** RFC-015's stated property is *non-production* of `TextStream` while a modal is open: `subscription`'s `Modal` arm returns `modal_subscription()`, which has no path to constructing one at all (`subscription_mode_reflects_whether_a_modal_is_active`, PR-015-C). `update`'s `state.modal.is_none()` guard is a second, independent mechanism — *discard*, not non-production — added this slice. Neither substitutes for the other: mechanism 2 exists specifically in case mechanism 1 is ever wrong (a future subscription change, a message queued before a modal opened and delivered after), and mechanism 1 is what makes mechanism 2 unreachable in ordinary operation. Recorded explicitly so neither is later read as dead weight and removed — `terminal_demo_subscription()` was independently confirmed to sit outside the modal gate but produce only `Message::TerminalDemoTick` (a poll tick, never a `TextStream`), so it is not a third path in either direction.

### Real-PTY, real-input negative case

`a_text_stream_targeting_a_different_id_does_not_write_to_the_pane`: a `TextStream` naming a fresh, unrelated `TerminalId`, delivered through the real `update`, never reaches the demo pane's PTY. Ablated: removed the `terminal_stream_targets_the_demo_pane` check from `update`'s guard — the test failed immediately (the character appeared despite the mismatched id). Reverted.

### Gates

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (492 `tekstide-core` — unchanged + 101 `tekstide` — up from 88, 13 net new — + 18 `tekstide-gui-spike` + 0 `tekstide-pty-spike`, 0 failures), `git diff --check` — all passed.

### Screenshot evidence, real keystrokes

`rfcs/handoffs/017-terminal-renderer-and-immersion-mode/evidence/pr-017-d/`, captured with the owner's explicit approval, same convention as prior slices:

- `00-terminal-mode-before-typing.png` — Terminal Mode, freshly toggled, before any keystroke.
- `01-real-keystrokes-typed-and-executed.png` — `xdotool type 'echo hello-017d'` then `xdotool key Return`, both real, individually-dispatched keystrokes (not a pre-seeded `printf`, unlike every prior PR-017-C screenshot): the shell echoes the typed command and its own real output, both rendered through the same `TextStream` → `to_pty_bytes` → `write_input` → real PTY → `poll()` → `SecurityFilter` → grid path this slice built.
- `02-tab-escapes-to-sidebar.png` — `xdotool key Tab` while the pane was focused and receiving real input: chrome focus border moves to the sidebar (`"> Sidebar"`), and the terminal content is unchanged from the previous screenshot — no stray tab character, no new prompt line.

**What this proves**: real, individually-dispatched keystrokes reach a live PTY through the full production path, and Tab's escape hatch holds under a real terminal with real input flowing. **What this does not prove**: trusted-UI separation or spoofing resistance (RFC-018's job, unchanged); nor real project-terminal session lifecycle (unrelated to and unblocked by this evidence, still PR-017-E's job).

### Review outcome (response 149)

**Approved, no required items.** The reviewer independently confirmed modal exclusivity rests on two separate mechanisms (non-production via `SubscriptionMode::for_modal`/`modal_subscription`, and the `state.modal.is_none()` discard this slice added) and traced `terminal_demo_subscription()`'s placement outside the modal gate specifically to rule out a second ingress -- confirmed clean: it produces only `Message::TerminalDemoTick`, never a `TextStream`.

**One clarification requested and added above**: the two-mechanism relationship is now stated explicitly in this file (see "Two independent mechanisms" above), so neither the subscription-level guarantee nor the `update`-level guard is later mistaken for redundant dead weight and removed.

**Recorded for RFC-018, not an obligation on this slice**: since `terminal_demo` and `modal` are constructed independently at boot, launching with both `TEKSTIDE_TERMINAL_DEMO` and `TEKSTIDE_LAYER_DEMO` set is reachable for evidence purposes even though no ordinary user-accessible sequence reaches that state -- and it produces exactly what RFC-018 needs: a trusted dialog over *actively updating* terminal content, not a frozen one. The reviewer recorded this against RFC-018's boundary directly; nothing further for PR-017-E/H to do about it.

## PR-017-E — Immersion mode, split policy, session bar

Gives the terminal pane real chrome: `TerminalPanePolicy`/`VisibleSlot` wired for real (not a parallel model), a split decided from real font metrics rather than a fraction, a session bar naming state without colour, and the hidden-session grid-state decision.

### Module restructuring, and the two carried obligations (`83f8c1a`)

`crates/tekstide/src/surface/terminal.rs` split into:

- `grid_colors.rs` — `styled_rows`/`resolve_color`/`named_color_from_index`/`view` (the grid-only rendering: the *only* legitimate `Color::from_rgb` call in this crate).
- `font_metrics.rs` — ported from the RFC-014 spike's own module, parameterized on `font_size` (not hardcoded) so the measurement matches what the pane actually renders at.
- `layout.rs` — `layout_class_for`, the split decision.
- `session_bar.rs` — the pane's first chrome, `pub`, theme-sourced colours.

**Obligation 1, discharged**: the colour-scan exemption (`shell::tests::is_scan_exempt`) narrowed from `terminal.rs` to `grid_colors.rs` exactly. `session_bar.rs` is deliberately **not** exempt — every colour there comes from `crate::theme::Theme`, proven by the unchanged `no_raw_color_construction_anywhere_in_the_crate` scan passing against it.

**Obligation 2, discharged**: the RFC-016 grid-not-chrome boundary is now live. `session_bar.rs` renders real chrome (slot + status labels) derived from `tekstide-core`'s own `TerminalSession` data, not from PTY output — there is still no session-title-from-OSC-0 case in this slice (no title is derived from terminal output anywhere), so the boundary has nothing to violate yet, but the chrome/grid split the boundary depends on now structurally exists (two different files, two different exemption statuses) rather than being one file where the distinction was only a comment.

### `tekstide-core`: the missing lifecycle glue

`AppState::attach_terminal_session`/`assign_terminal_visible_slot` added (`crates/tekstide-core/src/app.rs`), delegating to `ProjectSession::add_terminal_session`/`assign_terminal_visible_slot` — both already existed with **no caller outside `tekstide-core`** (confirmed by inspection while implementing PR-017-D, disclosed there as this slice's obligation, not added speculatively then). `TerminalPane::launch` now returns `(Self, TerminalSession)` instead of discarding the session, so a caller can register it. 4 new `tekstide-core` tests (`app::tests`): registration success, fails-closed with no active project (both `attach_terminal_session` and `assign_terminal_visible_slot`), and slot-uniqueness enforcement (assigning `Primary` to a second terminal bumps the first to `Hidden`, proving `ProjectSession`'s own enforcement rather than assuming it).

**This closes PR-017-D's own disclosed gap**: `terminal_stream_targets_a_live_terminal` (`shell.rs`) was `#[allow(dead_code)]` because the demo pane wasn't registered on the real project. It is registered now (`launch_terminal_demo_panes`), the suppression is removed, and a new test (`terminal_stream_targets_a_live_terminal_recognizes_the_registered_demo_session`) proves the previously-always-`false` check now recognizes a real session. `terminal_stream_targets_the_demo_pane` (PR-017-D's demo-only counterpart) is deleted — its whole reason for existing was that the real check couldn't see the demo pane, which is no longer true.

### `TerminalPanePolicy`/`VisibleSlot` — real, not a parallel model

The demo launches three real sessions (`launch_terminal_demo_panes`): `Primary`, `Secondary` (matching `visible_terminal_limit`'s default of 2 and `TerminalPanePolicy::max_visible_panes`), and one deliberately `Hidden` from the start. `active_project_terminal_sessions` (`shell.rs`) reads them from the real active project fresh each call — no shell-local slot bookkeeping. `terminal_workspace_view` renders `visible_terminal_sessions()`'s output only (never more than 2 by construction, since only `Primary`/`Secondary` are non-`Hidden`), sorted `Primary` before `Secondary`.

### The split decision: real font metrics, not a fraction

`layout_class_for(available_width_px, font_size)` measures the real monospace glyph advance at the pane's actual render size (`iced::advanced::graphics::text::Paragraph`, the same primitive `iced`'s own `Text` widget uses) and computes the real column count each pane would get if split in two. `Wide` only if that is at least [`COLS`] (80) — the pane's own fixed grid width, since this slice does not reflow a live `Term` to an arbitrary width (a materially larger feature, disclosed as out of scope in `layout.rs`'s module doc). Below that, `Narrow`: one pane rendered full-width rather than two clipped ones.

**Real width comes from `iced::widget::responsive`** (`terminal_workspace_view`), not a window-size field mirrored in `State` — the measured `Size` `responsive`'s closure receives at layout time is asked fresh on every rebuild.

**7 unit tests** (`font_metrics::tests`, `layout::tests`): glyph advance is positive and plausible; a larger font size measures a wider advance (proves the parameter is actually used, not a hardcoded stand-in); `columns_for_width` floors and never underflows; a generously wide window classifies `Wide`; a window fitting only one pane's real columns classifies `Narrow`; the boundary is the real column count, not an arbitrary pixel threshold (exactly enough per-pane width classifies `Wide`, one glyph-width less classifies `Narrow`); `layout_class_for` (the font-size-driven public entry point) measures from a real theme font size, not just the glyph-advance-parameterized internal helper.

### The session bar: `NFR-UX-002` by construction

`session_bar::view` renders one entry per registered session — slot (`Primary`/`Secondary`/`Hidden`) and status (`Running`/etc.), both as distinct text labels. Satisfied by text alone: there is no second channel to add on top of information already stated in words. `session_bar::tests::every_slot_and_status_has_a_distinct_textual_label` proves no two slots or statuses share a label (a real distinctness check, not just "labels exist").

### The hidden-session grid-state decision, demonstrated

**Decided: retained in memory, always polled — not torn down and rebuilt from scrollback.** Reasoning recorded in `surface/terminal.rs`'s module doc, against the bounded-scrollback decision as required: a hidden pane's `Term` costs exactly the same bounded amount (`SCROLLBACK_LINES = 2_000`) a visible one does — visibility does not change the bound — and the number of sessions a project can hold at all is itself bounded (`ProjectResourceLimits::terminal_session_limit`). Tearing a hidden session down would lose state and change what "hidden" means to a user checking on it later; retaining it costs a bound already paid for, not a new, unbounded one.

**Demonstrated, not only argued** (`shell::tests`):

- `active_project_terminal_sessions_lists_hidden_sessions_too`: the hidden session is not silently dropped from the list the session bar renders from.
- `a_hidden_pane_keeps_polling_and_retains_its_content_across_a_slot_change`: writes a real marker to the hidden pane's PTY, polls via the real `Message::TerminalDemoTick` → `update` path (not a direct `pane.poll()` call — the actual production tick handler), confirms the marker renders despite never being displayed, then reassigns that session to `Secondary` (bumping whatever held it back to `Hidden`, per `ProjectSession`'s own enforcement) and confirms the marker is *still* there — proving retention survives a slot change, not just the hidden period itself.
- **Ablated**: `ablation_polling_only_visible_panes_would_miss_the_hidden_ones_output` simulates the alternative design (poll only visible panes) directly and confirms the hidden pane's marker is absent — the failure mode the real, poll-everything `TerminalDemoTick` handler exists to avoid.

### Input targeting among multiple panes: a scoped decision

`active_terminal_focus` targets whichever session holds `VisibleSlot::Primary` — the only defensible choice with no per-pane click-to-focus or cycle keybinding built yet (not asked for by this slice's review gate). Recorded in `active_terminal_focus`'s own doc as a deliberately narrower scope than "solve pane-to-pane input focus," not a silent limitation.

### Gates

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (496 `tekstide-core` — up from 492, 4 net new — + 112 `tekstide` — up from 101, 11 net new — + 18 `tekstide-gui-spike` + 0 `tekstide-pty-spike`, 0 failures), `git diff --check` — all passed. One `tekstide-core` test (`approval::tests::channel::bind_recovers_from_a_stale_socket_file`, unrelated to this slice — RFC-021 approval-channel socket binding) failed once under full-workspace parallel execution and passed both in isolation and on a full re-run; not touched by this slice's changes, disclosed rather than silently re-run away.

### Screenshot evidence, both layouts

`rfcs/handoffs/017-terminal-renderer-and-immersion-mode/evidence/pr-017-e/`, captured with the owner's explicit approval:

- `00-narrow-single-pane-with-session-bar.png` — default window width (1042px), Terminal Mode toggled: the session bar shows all three registered sessions ("Terminal 1 (Primary) — Running", "Terminal 2 (Secondary) — Running", "Terminal 3 (Hidden) — Running"), and only the `Primary` pane's grid renders (`Narrow`, correctly refusing a two-pane split at this width).
- `01-wide-split-two-panes.png` — same window, column maximized (`niri msg action maximize-column`, 2101px), same three-entry session bar: **two independent, real shell prompts render side by side** (`Primary` left, `Secondary` right) -- the real, font-metrics-driven `Wide` classification triggered by a genuine width change, not simulated.

**What this proves**: the split is a real function of measured width and font metrics, not a fixed layout; the session bar reflects real, registered session state including a session that is never rendered. **What this does not prove**: trusted-UI separation or spoofing resistance (RFC-018's job, unchanged); real per-project terminal *creation* UX (no keybinding/command exists to launch a terminal — the three sessions here are the demo's own construction, matching the established `TEKSTIDE_TERMINAL_DEMO` convention).

### Review outcome (response 150) — Required: the session bar bypassed the i18n catalog

**Approved with one required item, now fixed.** `session_bar.rs`'s `slot_label`/`status_label` and `shell.rs`'s `format!("Terminal {}", ...)` were ten hardcoded English strings in trusted chrome — the exact shape `CountDisplay::label()`/`AttentionState::label()` are banned from this crate for (response 130), in a project that ships `en.ftl`/`pl.ftl` and would show English to a Polish user in the one surface this slice added.

**Fixed**: one Fluent message, `session-bar-entry` (`en.ftl`), selecting on two compile-time literal symbols (`$slot`, `$status`, resolved by `slot_symbol`/`status_symbol` -- the same division of labour `route_symbol`/`status-bar-summary` already use: the Rust side names a branch, the `.ftl` file supplies the words) plus a genuine number (`$number`, real plural-category selection available if a locale ever needs it) — not three separately-resolved lookups concatenated by Rust `format!`. `session_bar::view` now takes `&Catalog`; `SessionBarEntry::label: String` became `number: u32`.

**Not added to `pl.ftl`**: matching this project's own established precedent (`pl.ftl` deliberately defines only 3 of `en.ftl`'s keys, RFC-016 §Non-Goals -- actual translation is content work), the new key resolves for Polish via the source-locale fallback chain, proven by the existing completeness scan (`every_source_locale_key_resolves_in_every_shipped_locale`) rather than assumed.

**`generic_args()` (`i18n/enforcement.rs`) required an addition**, exactly as that function's own doc comment anticipates ("a future key introducing a new variable name needs an entry here too"): added `$number`/`$slot` alongside the existing `$count`/`$route`/`$status`/`$attention`. Without it, the completeness scan itself failed for the new key, catching the omission immediately rather than silently passing.

**The regression test rewritten, not just re-passed**: `every_slot_and_status_has_a_distinct_textual_label` (asserting distinctness of hardcoded strings) replaced with `every_slot_and_status_combination_resolves_to_distinct_text`, asserting distinctness over `Catalog::get_with_args`'s real resolved output against the real, shipped `en.ftl` -- per the review's own point, the old test would have kept passing on the old hardcoded strings and proven nothing about whether the catalog was actually reached. A second new test, `resolved_text_contains_the_real_words_not_symbol_names`, asserts the exact resolved string including Fluent's bidi isolation marks around each placeable (`\u{2068}`/`\u{2069}`, the same `ISOLATE_START`/`ISOLATE_END` convention `shell/tests.rs` already documents) -- catches a regression to raw symbol names or an empty string, which the distinctness test alone would not.

**Why no scan caught this originally, recorded per the review's explicit instruction** (not to be inferred later): `no_raw_string_literal_is_passed_to_text_anywhere_in_the_crate` requires the literal to *immediately* follow `text(`, and `text(format!(...))` with the English arriving from a helper function never matches; `no_count_display_or_attention_label_is_called_anywhere_in_the_crate` matches the literal substring `.label()`, and `slot_label(...)`/`status_label(...)` are the same shape under a name the scan does not know; `is_scan_exempt` covers colour and font size only, nothing about text. **This is a demonstrated gap in the `text(`-adjacency scan, not a theoretical one** -- broadening it is `i18n::enforcement`/PR-016-E's territory, not absorbed into this slice per the review's explicit instruction.

**Two secondary fixes from the same review, applied**:
- `layout.rs`'s test module previously re-derived the padding constant inline (`32.0`, `2.0 * 8.0`) instead of referencing `PANE_PADDING_PX` -- the two-sources-of-truth shape this project has already found and fixed once (`font_metrics.rs`'s own module doc names it). Fixed: a `one_pane_width` test helper now references `PANE_PADDING_PX` directly.
- `layout_class_for_measures_from_a_real_font_size`'s doc comment overclaimed: it derives `comfortable_width` from the same glyph advance it passes to `layout_class_for`, so a `layout_class_for` that hardcoded a narrower advance would still pass. Renamed to `layout_class_for_composes_correctly_with_a_real_font_size`, with its doc comment now stating exactly what it proves and pointing at `font_metrics::tests::a_larger_font_size_measures_a_wider_glyph_advance` for the real directional proof, per the review's own recommendation to correct the comment rather than the test.

**Recorded for PR-017-H, not fixed here (non-blocking)**: in `01-wide-split-two-panes.png`, the two panes occupy roughly three-quarters of the measured width, not the full width — `grid_colors::view` sets no width, so each pane shrinks to its fixed 80-column content and `row` packs them left rather than filling the space it was measured against. Safe (the layout decision and the render disagree only in the direction that avoids clipping), a direct, disclosed consequence of not reflowing a live `Term`, but worth one line in the closeout's known limitations so "wide split leaves dead space" reads as a documented boundary rather than a rediscovered bug.

**Not this slice's responsibility, recorded per the review**: the flaky `approval::tests::channel::bind_recovers_from_a_stale_socket_file` (noted above) is flagged by the reviewer as potentially a real TOCTOU narrowing in the RFC-021 socket-binding path, not merely test-isolation noise, and should be diagnosed before RFC-021 is claimed closed -- unrelated to and unblocked by this slice.

Gates re-run after the fix: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (496 `tekstide-core` + 113 `tekstide` + 18 `tekstide-gui-spike` + 0 `tekstide-pty-spike`, 0 failures), `git diff --check` — all passed.

## PR-017-F — `plain_terminal_observation` audit producer

Pending implementation.

## PR-017-G — Measurement: `NFR-PERF-004`

Pending implementation.

## PR-017-H — Closeout evidence

Pending implementation.

## Known Limitations

Recorded as they are found.
