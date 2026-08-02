---
title: "RFC-017: Terminal Renderer and Immersion Mode - QA Evidence"
rfc: "RFC-017"
rfc_file: "../../proposed/017-terminal-renderer-and-immersion-mode.md"
status: "Accepted 2026-08-01 — PR-017-B (filter promotion) implemented 2026-08-01, pending review"
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

Pending implementation.

## PR-017-D — Input

Pending implementation.

## PR-017-E — Immersion mode, split policy, session bar

Pending implementation.

## PR-017-F — `plain_terminal_observation` audit producer

Pending implementation.

## PR-017-G — Measurement: `NFR-PERF-004`

Pending implementation.

## PR-017-H — Closeout evidence

Pending implementation.

## Known Limitations

Recorded as they are found.
