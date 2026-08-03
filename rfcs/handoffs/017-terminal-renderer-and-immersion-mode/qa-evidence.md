---
title: "RFC-017: Terminal Renderer and Immersion Mode - QA Evidence"
rfc: "RFC-017"
rfc_file: "../../proposed/017-terminal-renderer-and-immersion-mode.md"
status: "Accepted 2026-08-01 — PR-017-B/C/D/E/F reviewed and approved (responses 144-153); PR-017-G (NFR-PERF-004) recorded not met 2026-08-03 (arithmetic verdict, owner ship/hold decision pending); re-run made self-validating, awaiting a quiet machine"
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

### Confirmed (response 151) — PR-017-E fully approved

**Confirmed. Required item closed.** The reviewer independently re-ran the gates (matching counts) and specifically noted the single-Fluent-message design (rather than ten separately-catalogued fragments concatenated by `format!`) was the better fix than the one they described in response 150, since it lets word order/punctuation vary by locale rather than hardcoding English sentence structure even after cataloguing every word. Declining to add the new key to `pl.ftl` was confirmed correct against the real numbers (`pl.ftl` defines 3 of `en.ftl`'s 23 keys). `resolved_text_contains_the_real_words_not_symbol_names` (the exact-string test, added alongside the distinctness test) was named as the one that actually matters, since the distinctness test alone would still pass under a full regression to the old hardcoded strings.

**One forward note, not an obligation on this or any named slice**: `is_scan_exempt`'s exemption list has no staleness check in either direction (unlike PR-016-E's `CORE_EXEMPT_LITERALS`) — if `grid_colors.rs` ever stops constructing a colour, it keeps a permanent exemption from scans it no longer needs. Recorded here for whichever future slice next touches `i18n::enforcement`, not raised as a requirement now.

## PR-017-F — `plain_terminal_observation` audit producer

Wires the `plain_terminal_observation` family (frozen v1 schema, no producer until now) end to end: a real, durable audit store the desktop application opens for the first time, a `tekstide-core` producer method reachable only through `AuditCoordinator`, and a real GUI call site.

### The missing lifecycle glue, discovered while researching this slice

**No production code anywhere constructed `AuditCoordinator`/`AuditStore` before this slice** — confirmed by `grep` across `crates/tekstide` and `tekstide-core/src/lib.rs`: every existing call site was inside `tekstide-core`'s own tests. Even `ManagedProcessLifecycle` (agent-run audit) had no real caller. This is the first audit write the GUI application ever performs, not merely a new family added to an already-wired mechanism.

### `AppState::attach_terminal_session`'s sibling: audit needs no new core-side threading

Unlike the session-registration gap PR-017-E closed (which needed a new `AppState` method), the audit store is deliberately **not** threaded through `ApplicationShell`/`AppState` at all. `AuditStore`/`AuditHealth` are opened and held in `crates/tekstide/src/shell.rs` (the GUI crate), the same boundary `main.rs`'s `RecentProjectStore` already draws: `ApplicationShell`/`AppState` hold domain state; I/O-owning resources (a real file, a real SQLite connection) live in the GUI crate that has a lifecycle to open and close them against.

**One new `tekstide-core` accessor was needed**: `AppStatePathProvider::state_dir(&self) -> &Path` (`project/recent/store.rs`), exposing the same `<tekstide-state-root>` directory `recent_projects_file()` already computes a filename under. The audit store's directory is resolved from this *same* provider instance, not a second, independently-derived `XDG_STATE_HOME`/`HOME` fallback — one resolution, two consumers, matching RFC-013's own diagram (`<tekstide-state-root>/audit/audit.sqlite3`).

### The producer, and why it emits exactly one outcome

`AuditCoordinator::record_plain_terminal_started(project_id, terminal_id)` (`tekstide-core`), delegating to a new `plain_terminal_record` helper matching `managed_process_record`'s existing shape exactly (family, actor/source, terminal id, no free-text field). Written via `append_observation` (best-effort) — an unavailable or degraded audit store must never block a terminal from launching, the same reasoning `ManagedProcessLifecycle`'s own `Started`/`Terminated` observations already use.

**Only `Started` is produced, disclosed rather than left implicit.** `valid_plain_terminal` requires `terminal_id.is_some()` for *every* outcome of this family, including `Failed` — meaning a launch failure that occurs before a `TerminalSession` exists (the actual failure mode `TerminalPane::launch`'s `Result` surfaces today) has no valid way to be represented in this frozen schema at all; there is no `TerminalId` yet for such a record to name. `Terminated` would need real process-exit detection wired into `TerminalPane::poll()`'s plain-terminal loop, which does not exist yet (PR-017-C/D/E's poll only advances the emulator; it never inspects `TerminalRuntimeEvent`/`TerminationOutcome` for a plain terminal, unlike the managed-agent path, which already does). `plain_terminal_record` stays general (takes any `AuditOutcome`/`Option<AuditReasonCode>`) precisely so a later slice wiring real exit detection has the right shape to call into, rather than a `Started`-only helper to generalize then.

### Conforms to the frozen family — schema unamended, proven two ways

1. `record.validate()` called directly against a real producer's output (`plain_terminal_started_persists_a_valid_record`, `tekstide-core`), not merely assumed to satisfy `valid_plain_terminal`.
2. **Ablated**: temporarily set `record.adapter_profile_ref = Some(...)` (a field `valid_plain_terminal` requires `None` for this family) before writing. The write was rejected by the store's own `record.validate()` call inside `AuditStore::append` — `AuditObservationStatus::Degraded`, not `Persisted` — confirming the frozen schema's own validation is a real, structural defense against a future accidental field addition, not merely a comment. Reverted.

No schema amendment was made or needed.

### Written via `AuditCoordinator`, not directly to the store

`shell.rs`'s only interaction with `AuditStore` is `AuditStore::open`/`.query` (evidence-gathering in tests) and the one production call inside `launch_terminal_demo_panes`, which goes through `AuditCoordinator::new(...).record_plain_terminal_started(...)` — no direct `store.append(...)` call exists in the GUI crate.

### The sentinel test — raw on-disk bytes, matching RFC-021 PR-021-E2's shape

`sentinel_terminal_derived_text_never_reaches_the_durable_audit_store` (`shell::tests`): launches a real `TerminalPane` whose window title and project root path both carry unique sentinel strings, records a real `plain_terminal_observation` via the real `AuditCoordinator` call this crate makes, then asserts the sentinels are absent from **both** the typed query's debug output and the raw bytes read directly off `store.storage_path().database_file()`.

**Ablated**: temporarily appended the sentinel title directly to the on-disk file after the real write (simulating a leak), confirming the raw-byte assertion fails exactly as intended — `AuditObservationStatus`/typed checks alone would not have caught bytes appended outside the normal write path; only reading the file itself does. Reverted.

**Why this test can prove something the type system already prevents structurally**: `DurableAuditRecordV1` has no path/title field for any family, so no valid write could carry one regardless of this test. What the test actually adds is proof that this crate's own wiring — `TerminalPane::launch(project_id, title, root, shell)` → `AuditCoordinator::record_plain_terminal_started(project_id, terminal_id)` — never threads `title`/`root` into the audit call at all, end to end from a real launch, not merely "the schema wouldn't allow it if someone tried."

### Gates

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (497 `tekstide-core` — up from 496, 1 net new — + 115 `tekstide` — up from 113, 2 net new — + 18 `tekstide-gui-spike` + 0 `tekstide-pty-spike`, 0 failures), `git diff --check` — all passed.

### README.md's privacy claim, fixed in this same change

Per this slice's own required gate: `README.md` §Local Data and Privacy previously stated running `tekstide` "does not create an audit database or retain any transcripts" — true at `0.4.1`, false the moment this producer has a real call site. Updated to state where the store lives (`$XDG_STATE_HOME/tekstide/audit/audit.sqlite3`, with the `~/.local/state` fallback), what it holds (a `plain_terminal_observation` "started" event — no command text, output, or path, and the schema has no field for any of those), and how to purge it (delete the `audit/` directory; no in-app command yet), citing RFC-013.

**Disclosed rather than overclaimed**: the real producer call only fires today under `TEKSTIDE_TERMINAL_DEMO` (the same developer-only diagnostic gate every terminal-pane demo since PR-017-C has used) — no in-app feature launches a real terminal session yet, so ordinary use of `tekstide` still does not create this file today. The README says this explicitly rather than letting "tekstide creates an audit database" read as true for every user right now, when it is only true under a flag developers use for evidence-gathering.

### What this slice does not do

No real terminal-launch UI (still `TEKSTIDE_TERMINAL_DEMO`-gated, matching PR-017-C/D/E). No `Failed`/`Terminated` observations (see above — schema-constrained and instrumentation-constrained, not merely deferred by choice). No screenshot evidence: this slice adds no new visual affordance over PR-017-E's session bar/split view, so a screenshot would not demonstrate anything the audit-store gates above do not already prove.

### Review outcome (response 152) — Required: two evidence gaps found by probe, both fixed

Response 152 confirmed the producer, schema conformance, and `Started`-only reasoning as correct, but found **both of this slice's own review-gate items unsatisfied** — proven by a reproducible probe (`.git-exclude/tmp/audit-open-probe/`), not by inspection:

**Required 1 — the README privacy claim was still false, one layer in.** `open_real_audit_store` was called unconditionally in `State::new`, before and independent of the `TEKSTIDE_TERMINAL_DEMO` check inside `launch_terminal_demo_panes`. Opening is not passive: `AuditStore::open_internal` creates the audit directory and, on a not-existed database, runs `create_current_schema` — so **every ordinary launch of `tekstide` was already creating `audit.sqlite3` with the full schema**, empty of events but real, exactly contradicting "ordinary use still does not create this file." **Fixed** by moving the open inside `launch_terminal_demo_panes`, after both early-return gates (the env var and an active project) — the reviewer's preferred option, since it makes the README's sentence true as written rather than requiring a weaker sentence. `open_real_audit_store` no longer runs at all unless a demo terminal is actually about to be launched.

**Required 2 — the sentinel test scanned a file that did not contain the record.** `store` was still open (WAL mode) when the test read `store.storage_path().database_file()`; a freshly appended record lives in the `-wal` sidecar until checkpoint, so the assertion was reading a static 4096-byte header page and would have passed unchanged even if the producer wrote the sentinels directly into the schema — a vacuous pass. The prior ablation (appending a sentinel directly to the main file and confirming the assertion caught it) proved the *reader* worked on planted bytes; it never proved the reader looked where real bytes land, which is exactly the gap. **Fixed**: the store is now dropped before scanning (closing the single connection triggers SQLite's automatic WAL checkpoint, reproducing exactly the on-disk state a real session leaves — the reviewer's own probe confirmed this: `audit.sqlite3` alone holds the record after drop, with no `-wal`/`-shm` remaining), and every file under the audit directory is scanned (`read_every_file_in_dir`, new in `shell::tests`), not the named database file alone — robust to SQLite's sidecar set changing.

**Re-ablated against the real path, per the reviewer's instruction** ("an ablation that plants bytes proves less than one that routes a real value through the real producer"): added a positive-control assertion that the raw scan contains `terminal_id.as_str()` — a real, persisted field written by this same real producer call, not a sentinel invented for the test. Ablated by temporarily reverting the scan to the original open-store, `database_file()`-only form: the positive control failed immediately (`the scan must reach the real record this test just wrote`), proving the old scan was blind to genuine content, not merely insensitive to planted sentinels. Reverted.

Gates re-run clean after both fixes: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (497 `tekstide-core` + 115 `tekstide` + 18 `tekstide-gui-spike` + 0 `tekstide-pty-spike`, 0 failures — no test count change; both fixes are inside existing tests), `git diff --check`.

**Escalation noted, not this slice's fix**: response 152 found the identical defect in RFC-021 PR-021-E2's own sentinel test (`approval/tests/coordinator.rs:980`), which the reviewer had approved without checking which file it read. Raised against RFC-021, not fixed here.

### Confirmed (response 153) — PR-017-F fully approved

Both required items confirmed closed, independently re-probed by the reviewer (their own probe agreed with the re-ablation panic recorded above). The positive-control assertion was singled out as the stronger fix: it "converts the vacuous-pass failure mode from something a reviewer has to notice into something the test itself refuses to allow" for every future edit, not just the moment it was written. The RFC-021 escalation was moved off this file entirely — the reviewer recorded both open RFC-021 items (this WAL-blind-spot defect, plus an unrelated intermittent test failure) directly in `rfcs/handoffs/021-.../qa-evidence.md`, judging this file to be "where I was writing, not where implementers read." Nothing further owed here; this line is a cross-reference only.

PR-017-F is closed. Next: PR-017-G (`NFR-PERF-004` under bounded background output, not idle — flood is where P4 failures surface; `iced::window::frames()` stays out; non-contamination proven for this criterion; stop on confirmed on-disk sample counts, not dispatched ones).

## PR-017-G — Measurement: `NFR-PERF-004`

Scope: terminal input latency p95 ≤ 16 ms under bounded background output. Reuses PR-015-F's measurement harness rather than a new mechanism; the code is implemented and gated, but **the live measurement run has not yet been performed** — see the last section below.

### `Criterion::TerminalFlood` reuses the input-to-state-change half only

`record_input`, `measured_key_subscription`, and the 100ms self-exit tick (`MeasurementTick`) are all reused byte-for-byte from `Typing`/`ModeSwitch` — no new timing mechanism, and `iced::window::frames()` is not touched anywhere in this criterion's path (only `Startup` uses it, unchanged). The measurement key ("j") writes a real byte directly into a real, live `TerminalPane` via `write_input` (`Message::MeasuredTerminalInput`'s handler, `shell.rs`), bypassing only the `TextStream`/routing-target lookup step — the same kind of bypass `ModeSwitch` already established as measuring the real cost, not input classification.

**This criterion deliberately does not use the view-build half of the decomposition** (`uses_input_view_decomposition` stays `false` for it, matching `Startup`): writing to a pty causes no synchronous view rebuild the way pushing a character into `typing_doc` or toggling project mode does — the grid only changes on the next, unrelated `TerminalDemoTick` poll. A `view` sample logged against the measured message would describe an unrelated tick's cost, not this input's own, so it is left out rather than logged and disclosed as noisy after the fact.

### One real pane, registered and rendered like a real session

`launch_measurement_terminal_pane` (deliberately **not** `launch_terminal_demo_panes`, which also opens the real audit store — PR-017-F's unrelated I/O this measurement path must not exercise while timing) launches exactly one live `TerminalPane`, registers it via the same `attach_terminal_session`/`assign_terminal_visible_slot(Primary)` calls PR-017-E's demo panes use, then dispatches the real `AppCommand::ToggleActiveProjectMode` (`ProjectSession::new` always starts in `Content`, so one dispatch is enough) so the project is genuinely in `TerminalImmersion` — the pane renders every `view()` cycle exactly as a real interactive session would, rather than existing off-screen while only its pty is exercised.

### The flood is bounded, unlike the superseded demo's

`FLOOD_SCRIPT` computes its own wall-clock end time once (`$(date +%s) + 120`) and checks it every loop iteration, self-terminating after 120 seconds — RFC-014 PR-014-E's own `tekstide-gui-spike` precedent (`send_flood_script_once`, now superseded) backgrounds an unbounded `while true` that only stops if killed; RFC-017's review gate asks for "bounded background output" specifically. 120s is generous margin over RFC-014/015's own C2/C4 precedent (1,100 repeats at a 15ms `xdotool --repeat-delay` pace finished in ~17 seconds) without this process ever needing to kill it — a real, disclosed consequence is that the flood can outlive the measurement process itself by up to the remainder of that 120s window if the run finishes early, reparented under init, until it self-terminates on its own; it is never explicitly killed by this code.

### `subscription()` batches the poll tick in, so the flood is actually read

`state.measurement`'s early-return branch in `subscription()` previously returned only `measurement_subscription(criterion)`, bypassing `state.terminal_demo`'s poll tick entirely (harmless for `Typing`/`ModeSwitch`/`Startup`, which never populate `terminal_demo`). `TerminalFlood` does populate it, so that branch now batches in `terminal_demo_subscription()` whenever a pane exists — otherwise the flood would be written into a pty nothing ever reads, both stalling it against the kernel pipe's own capacity once full and, more importantly, eliminating the actual contention (poll's PTY-read/VTE-processing cost competing with input-message handling on the same executor) this criterion exists to observe.

### Gates

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (497 `tekstide-core` + 117 `tekstide` — up from 115, 2 net new: `terminal_flood_is_done_exactly_at_target`, `record_startup_frame_is_a_no_op_for_terminal_flood`, the same coverage density `ModeSwitch` got — + 18 `tekstide-gui-spike` + 0 `tekstide-pty-spike`, 0 failures), `git diff --check` — all passed. Committed as `fcf1e91`.

### Not yet done: the live measurement run itself

Per RFC-017's review gate, this slice still owes: p50/p95/p99/max against the ≤16ms budget, delivery-loss rate, a non-contamination proof specific to this criterion (not inherited from `Typing`/`ModeSwitch`), and confirmed-on-disk (not dispatched) sample counts — all requiring a real `xdotool`-driven run against a live window. Per the owner's redirect this session ("make question as review request to architect on it"), the decision of whether/how to run this live GUI measurement is raised to the architect via the accompanying review request rather than executed unilaterally. The code above is otherwise complete and gated.

### Response 154 — two findings, and the definition question that holds the live run

**Finding 1, fixed (`5c43e98`)**: `record_input` was called *before* `write_input`, so the graded interval measured only `iced`'s event-to-update dispatch latency — the same quantity `Typing` already measures — with the terminal write itself silently excluded. Swapped; `record_input` now runs after `write_input` completes. The flood's wall-clock bound was also tightened from 120s to 30s per the reviewer's "disclosed-and-avoidable is worth avoiding" note on the earlier margin.

**Finding 2, not fixed, held for the owner's decision**: `NFR-PERF-004` (p95 ≤ 16ms, one 60Hz frame) means keystroke-to-echo-visible. Echo depends entirely on `terminal_demo_subscription`'s 50ms poll tick — the only place PTY bytes reach the grid — so a keystroke's echo waits for the next tick, uncorrelated with arrival, contributing an **arithmetic** (no live run needed) expected p95 of ~47.5ms from poll-wait alone (0.95 × 50ms) before any pty/VTE/layout/paint cost — roughly 3× the entire budget. `measurement.rs`'s module doc now states this explicitly (response 154's finding that the doc explained what was skipped but never what the interval spans).

**A related finding surfaced while investigating the options, not asked for directly but bearing on which is viable**: `TerminalPane::poll()` calls `read_available_bounded_for(handle, Duration::from_millis(5), 64*1024)`, but that function's own `WouldBlock` branch (`tekstide-core`'s `runtime/terminal/launch.rs:148-150`) sleeps a hardcoded **10ms** before re-checking the caller's 5ms bound — so an idle pane's `poll()` call already blocks iced's single update thread for ~10ms per call today, exceeding its own nominal bound, whenever there is nothing to read. This means shortening `terminal_demo_subscription`'s tick alone would not shorten this per-call floor below ~10ms; ticks faster than that would just fire more often against the same ~10ms-per-idle-call cost, without a second, smaller fix to this internal sleep granularity.

**Options, with tradeoffs, sent to the architect for the owner's decision (review request 155)**:
1. Shorten the tick — partially closes the arithmetic gap, but is coupled to the undesigned `read_available_bounded_for` fix above, and its idle-CPU cost (paid by every terminal pane in the app, forever, since this subscription is the production polling mechanism, not measurement-only) is unquantified.
2. Wake on PTY readability instead of polling — removes the tradeoff rather than tuning it, very likely the right long-term answer, but a real architectural change touching the same I/O path PR-017-B/C's P1 (single-ingress)/P2 (no side channels) properties were proven against — its own PR/RFC-scale piece of work, not absorbable into this slice.
3. Keep 50ms, record `NFR-PERF-004` as honestly not met, with the reason — costs nothing to build, is the first real verdict this criterion has ever received (RFC-014 never verified C3 at all), and is a legitimate outcome per the project's own standing rule that a measured figure must be non-degenerate in both directions.

**My recommendation, sent with the options**: (3) now, (2) as scoped future work, explicitly not (1) alone — narrowing the gap by tuning a constant, while its cost is unquantified and it's coupled to an undesigned second fix, is exactly what the reviewer warned against doing to make a number pass. Awaiting the owner's answer before any further code changes or the live run; the non-contamination control also needs redesigning once the definition is settled (same pane, same flood, measurement env var on vs. off — not the idle comparison originally proposed, which couldn't separate instrumentation cost from intended workload cost).

### Response 155 — analysis accepted, flood script fixed, and the live run attempted

Response 155 endorsed the recommendation above (Option C now, Option B as scoped future work) and required two more things before any run: fix `FLOOD_SCRIPT` (it measured `$(date +%s)` in the loop *condition*, so every output line cost a `fork`+`exec` — the reviewer measured 121.7 KiB/s, 173× below a fork-free equivalent, never exceeding the 64KiB/5ms cap, "much closer to the idle case than to the flood case"), and surface dropped bytes instead of discarding the event that carries them.

**Fixed (`ba039a9`)**: `FLOOD_SCRIPT` now checks the wall clock only every 2,000 iterations instead of every one — verified locally afterward at ~17.2 MiB/s (46.5MB over ~2.7s wall time via `time sh`), comfortably above the 64KiB/5ms threshold this time. `TerminalPane::dropped_bytes_total()` (new) accumulates `TerminalOutputSummary::dropped_bytes` across every `poll()` call and is printed to stderr once, right before the measurement process exits. `TEKSTIDE_TERMINAL_FLOOD_DEMO` (new) launches the identical pane-plus-flood scenario with measurement deliberately absent, for the "same workload, instrumentation on vs. off" control response 155 asked for. All gates re-passed (497 + 117 + 18, 0 failures).

**The live run was attempted three times and the results are not usable as `NFR-PERF-004` evidence — a measurement-environment confound, disclosed rather than reported as a clean number.**

All three runs (`env -u WAYLAND_DISPLAY XDG_STATE_HOME=<scratch> TEKSTIDE_MEASURE_CRITERION=terminal_flood TEKSTIDE_MEASURE_LOG=<scratch> ./target/release/tekstide <scratch-project>`, `xdotool windowfocus --sync` then global `xdotool key --clearmodifiers --repeat 1100 --repeat-delay 15 j`) delivered all 1,100 samples with 0% dispatched-vs-confirmed loss and `dropped_bytes_total 0` — but the latency values themselves are not credible as real software behavior:

- **Run 1**: samples 1–490 measured 13–70μs (consistent with the arithmetic dispatch-only floor); sample 491 jumped, in one step, to ~999,882μs (≈1.0s); a second step to ~1,181,485μs around sample 515; a third to ~1,201,863μs around sample 519; then a flat plateau (creeping by single-digit μs per sample) through sample 1,100 (~1,161,329μs before the second run overwrote the log — the final plateau value differed slightly per run, see below).
- **Run 2**: *every* sample, including the first, measured ~1.15–1.17ms **seconds** (1,148,547–1,172,086μs) — the step had already happened before the first keystroke was even sent.
- **Run 3**: aborted before sending input (see below) once the cause became apparent.

A step function, not a gradual ramp, appearing at an inconsistent sample offset between otherwise-identical runs, is not consistent with either the arithmetic poll-wait floor (which would show as a smooth ~0–50ms spread, not a discrete jump to over a second) or with `TerminalDemoTick`'s 10ms `WouldBlock` stall (which would add tens of milliseconds, not entire seconds). **Checked the environment directly**: `free -h` showed **54–57GiB of swap in use out of 59GiB**, on a 32-core/59GiB machine, at the time of all three runs — memory pressure this test's own tiny footprint (one Rust GUI process, one shell script) cannot explain; it is pre-existing load from other activity sharing this sandbox. `vmstat 1` during a third attempt (aborted before sending any keys) showed a real swap-in burst (~70MB in one 1-second window, `si`/`bi` columns) even with zero test input yet delivered. This is the far more parsimonious explanation for a ~1-second one-time cliff than a code defect: a major page fault against a heavily swapped system, not `tekstide`'s own architecture.

**Conclusion**: today's live numbers are not attributable to `tekstide` and are not reported as `NFR-PERF-004` evidence. They neither confirm nor worsen the arithmetic finding from response 154 (the ~47.5ms poll-wait floor, independent of any live run, stands on its own). The dropped-bytes/non-contamination/delivery-loss items the review gate still asks for need a live run in an environment not already under heavy unrelated memory pressure — deferred, not abandoned, and not blocking the verdict below, which does not depend on a live run.

All three processes and the `xdotool`-focused window were confirmed cleaned up afterward (`ps aux`/`xdotool search` both empty); no stray flood or GUI process was left running.

### Verdict recorded: `NFR-PERF-004` not met

Per response 155 item 4 ("record `NFR-PERF-004` as not met, with the arithmetic — you are right that this needs no live run"): **`NFR-PERF-004` (terminal input latency p95 ≤ 16ms) is recorded as not met**, under the current architecture. Reason: `terminal_demo_subscription`'s 50ms poll tick is the only place PTY bytes reach the emulator grid; a keystroke's echo waits for the next tick, uncorrelated with arrival, contributing an expected p95 of ~47.5ms (0.95 × 50ms) from poll-wait alone — before any pty write, VTE parse, layout, or paint cost — roughly 3× the entire budget. This is arithmetic (uniform-distribution wait time over a fixed, code-visible interval), not empirical, and every omitted term can only make the true figure worse, never better. RFC-014 never verified this criterion at all (marked "Not verified — see R1"; R1 assigned it to RFC-017), so this is the first real verdict it has received, not a regression from a prior pass.

**The fix (Option B: readiness-driven terminal I/O instead of fixed-interval polling) is out of scope for this slice**, per response 155: it changes the shape of the one ingress path P1 (single-ingress)/P2 (no side channels) were proven against, and needs the same re-enumeration/re-ablation treatment, sized as its own PR or RFC amendment, not absorbed here.

**The ship decision is the owner's** (response 155's own framing): whether `0.5.x`/M9 ships with `NFR-PERF-004` recorded as not met and Option B scheduled as follow-up work, or whether RFC-017's closeout holds until Option B lands. Not decided in this file.

### Response 156 — confound independently confirmed, but under-determined; two more items before any re-run

The reviewer verified the confound directly: `free -h` at review time showed 849MiB free RAM against 53GiB swap in use, with `rust-analyzer` ×2, `soffice.bin`, `librewolf`, and `codium` ×2 as the identifiable consumers — this is the owner's own live working environment under real, unrelated load, not an artifact of this test. Discarding the numbers was confirmed correct.

**But the reviewer pushed back on the diagnosis, correctly**: swap pressure explains the ~1.1-1.2s plateau; it is not the *only* thing that would. This was the first run against a real (non-fork-bound) flood, and a single-threaded update loop falling behind a flood it cannot keep up with produces the same signature (a step to a backlog-shaped plateau) as environment noise does. The two hypotheses were indistinguishable from the run's own output — which is the actual defect to fix, not the specific number.

**`dropped_bytes_total 0` was also re-read as evidence against the run, not reassurance.** At the flood's measured ~17.2 MiB/s against a drain that accepts at most 64KiB per 50ms tick (~1.28 MiB/s), a genuine in-app flood should push well past the cap on most windows. Zero drops says the flood never reached rate *inside the application* — a cheaper, more direct invalidity signal than interpreting the latency plateau.

**Two required items for the re-run, both implemented (`b4e10ff`), neither exercised yet**:

1. **Observed in-app flood throughput.** `TerminalPane::bytes_read_total()` (new) accumulates bytes actually accepted across `poll()` calls; `Measurement::elapsed()` (new) gives wall-clock time since the measurement began; both are printed together at exit (`bytes_read_total`, `elapsed_secs`) alongside `dropped_bytes_total`. Dividing the two externally gives observed throughput, checkable against the flood script's own ~17.2 MiB/s standalone figure — if far lower, the flood didn't flood and the run is void, detectable from the run's own output rather than argued afterward.
2. **Tick-handler wall time, its own distribution.** `Measurement::record_tick_handler` (new) logs `poll()`'s own wall time (PTY read plus `Processor::advance`) as a `tick`-prefixed sample in the same log file, from `Message::TerminalDemoTick`'s handler. This is the discriminator: if handler cost approaches the 50ms tick period, the update loop genuinely cannot keep up (a real, structural finding that would recur on a quiet machine); if it stays in single-digit milliseconds while `record_input` shows seconds, the environment is confirmed as the sole cause. Meaningful relative to the tick period regardless of machine load — unlike `record_input`'s own figure, this one doesn't itself need a quiet machine to be informative.

**A one-line environment snapshot** (`write_environment_snapshot`, new) reads `/proc/meminfo` once at measurement construction and logs `mem_available_kib`/`swap_used_kib` for every criterion, not just `TerminalFlood` — so any future reader of any measurement log can tell at a glance whether it was taken on a sane machine, without cross-referencing a separate `free -h` capture. Parsing factored into `parse_meminfo_snapshot` for direct unit testing against a fixture (field order shuffled, extra fields interspersed, to prove field-name lookup doesn't depend on `/proc/meminfo`'s real ordering) and a missing-field case; ablation-verified by reverting the swap-used computation to `SwapFree` alone and confirming the test fails on the exact wrong value, then reverted.

Gates re-passed: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (497 `tekstide-core` + 119 `tekstide` — up from 117, 2 net new — + 18 `tekstide-gui-spike` + 0 `tekstide-pty-spike`, 0 failures), `git diff --check`.

**No re-run performed.** Per the reviewer's own words — "there is no urgency, and a run taken on a loaded machine is worse than no run" — the re-run is deferred until this shared machine's memory pressure has cleared (checked via `free -h`/the new environment snapshot before running again), not attempted against the same loaded state that just invalidated two prior runs. `NFR-PERF-004`'s not-met verdict above is unaffected either way. PR-017-H stays blocked until a usable run lands.

## PR-017-H — Closeout evidence

Pending implementation.

## Known Limitations

Recorded as they are found.
