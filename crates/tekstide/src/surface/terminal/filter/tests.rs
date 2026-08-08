//! P1-P4 evidence for the promoted filter. Structure follows
//! `pr-017-b-filter-promotion.md`: a corpus of sequences tagged by
//! `tekstide-core`'s own `TerminalSequenceFamily`, fed through
//! filter -> `vte::ansi::Processor` -> `Term` at every chunk split point,
//! asserting blocked families leave `Term` state **fully** unchanged
//! (not just the cursor, not just one line) and accepted families
//! produce the effect core's policy says they should.
//!
//! Ported from the RFC-014 spike crate's own `filter/tests.rs`
//! (RFC-014 PR-014-C), not rewritten from scratch -- the harness shape,
//! the V2/V4/V5/V6/V7 findings, and the split-boundary methodology are
//! unchanged. What changed: the corpus is a data table iterated by one
//! generated sweep rather than one hand-written test per family, the
//! blocked-family type is `tekstide-core`'s own `TerminalSequenceFamily`
//! instead of a shell-local enum, and "no effect" is proven by full
//! grid-plus-cursor snapshot equality rather than marker absence on one
//! line. The spike crate (`tekstide-gui-spike`) was deleted 2026-08-04
//! -- see
//! `rfcs/handoffs/014-desktop-gui-substrate-and-terminal-rendering/spike-crate-deletion.md`.

use super::SecurityFilter;
use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use std::cell::RefCell;
use tekstide_core::runtime::terminal::{TerminalAcceptedSequence, TerminalSequenceFamily};

#[derive(Clone, Copy)]
struct FixedSize {
    lines: usize,
    columns: usize,
}

impl Dimensions for FixedSize {
    fn total_lines(&self) -> usize {
        self.lines
    }

    fn screen_lines(&self) -> usize {
        self.lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

const SIZE: FixedSize = FixedSize {
    lines: 24,
    columns: 80,
};

#[derive(Default)]
struct RecordingListener {
    events: RefCell<Vec<Event>>,
}

impl EventListener for RecordingListener {
    fn send_event(&self, event: Event) {
        self.events.borrow_mut().push(event);
    }
}

fn new_term() -> Term<RecordingListener> {
    Term::new(Config::default(), &SIZE, RecordingListener::default())
}

/// Feeds `chunks` through a fresh `Processor`/`Term` pair, one
/// `SecurityFilter` per chunk (mirroring how a real terminal pane calls
/// `advance` once per PTY read), and returns the resulting term plus
/// everything this filter declined to forward across all chunks.
fn feed_chunks(chunks: &[&[u8]]) -> (Term<RecordingListener>, Vec<TerminalSequenceFamily>) {
    let mut processor = Processor::<alacritty_terminal::vte::ansi::StdSyncHandler>::new();
    let mut term = new_term();
    let mut blocked = Vec::new();

    for chunk in chunks {
        let mut filter = SecurityFilter::new(&mut term);
        processor.advance(&mut filter, chunk);
        blocked.append(&mut filter.blocked);
    }

    (term, blocked)
}

fn line_text(term: &Term<RecordingListener>, line: usize) -> String {
    term.bounds_to_string(
        Point::new(Line(line as i32), Column(0)),
        Point::new(Line(line as i32), Column(SIZE.columns - 1)),
    )
}

/// Every visible line, not just line 0 -- P2/P1's "no observable grid
/// effect" claim is about the whole grid, and a sequence that happens to
/// wrap or move the cursor before writing could leak outside line 0.
fn full_grid_text(term: &Term<RecordingListener>) -> String {
    (0..SIZE.lines)
        .map(|line| line_text(term, line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Full state, not just the cursor: grid content plus cursor position.
/// Two snapshots being equal is the strongest available proxy for "this
/// sequence had no observable effect" available from `Term`'s public API.
fn grid_snapshot(term: &Term<RecordingListener>) -> (String, Point) {
    (full_grid_text(term), term.grid().cursor.point)
}

/// One entry per family this filter classifies as blocked. The sequence
/// for each is either taken verbatim from the spike's own already-
/// reviewed corpus (title, clipboard, hyperlink, mouse/focus, terminal
/// query, SCP) or newly verified directly against `vte` 0.15's
/// `ansi.rs` `csi_dispatch` match arms (ordinary private mode, keyboard
/// protocol) rather than assumed.
struct FamilyCase {
    name: &'static str,
    sequence: &'static [u8],
    expected_family: TerminalSequenceFamily,
    secret_marker: &'static str,
}

const BLOCKED_FAMILY_CORPUS: &[FamilyCase] = &[
    FamilyCase {
        name: "osc_title",
        sequence: b"\x1b]0;PWNED-TITLE\x07",
        expected_family: TerminalSequenceFamily::OscTitle,
        secret_marker: "PWNED-TITLE",
    },
    FamilyCase {
        name: "osc_52_clipboard",
        sequence: b"\x1b]52;c;SGVsbG8=\x07",
        expected_family: TerminalSequenceFamily::Osc52Clipboard,
        secret_marker: "SGVsbG8",
    },
    FamilyCase {
        name: "osc_8_hyperlink",
        sequence: b"\x1b]8;;https://evil.invalid/\x07",
        expected_family: TerminalSequenceFamily::Osc8Hyperlink,
        secret_marker: "evil.invalid",
    },
    // Verified against vte 0.15 ansi.rs directly: CSI `?2004h` sets
    // NamedPrivateMode::BracketedPaste (mode 2004), not one of the
    // mouse/focus numbers (1000/1002/1003/1004/1005/1006) --
    // `classify_private_mode_number` must return `PrivateMode`, distinct
    // from the mouse/focus case below.
    FamilyCase {
        name: "private_mode_bracketed_paste",
        sequence: b"\x1b[?2004h",
        expected_family: TerminalSequenceFamily::PrivateMode,
        secret_marker: "",
    },
    FamilyCase {
        name: "mouse_focus_reporting",
        sequence: b"\x1b[?1000h",
        expected_family: TerminalSequenceFamily::MouseFocusReporting,
        secret_marker: "1000",
    },
    // Verified against vte 0.15 ansi.rs's csi_dispatch: final byte `u`
    // with intermediate `>` dispatches `push_keyboard_mode`.
    FamilyCase {
        name: "keyboard_protocol_push",
        sequence: b"\x1b[>1u",
        expected_family: TerminalSequenceFamily::KeyboardProtocol,
        secret_marker: "",
    },
    FamilyCase {
        name: "terminal_query_device_attributes",
        sequence: b"\x1b[c",
        expected_family: TerminalSequenceFamily::TerminalQuery,
        secret_marker: "",
    },
    // SCP (DECSCP): CSI final `k` with intermediate ` ` (space). Verified
    // directly against vte 0.15's `('k', [b' '])` dispatch arm, same
    // sequence the spike's own `v1_scp_is_csi_not_dcs` test used.
    FamilyCase {
        name: "scp_falls_back_to_generic_unsupported_csi",
        sequence: b"\x1b[1 k",
        expected_family: TerminalSequenceFamily::Csi,
        secret_marker: "",
    },
];

/// P4 (stream-position independence) plus "no observable grid effect,"
/// both directions -- exhaustive over the corpus above and every
/// internal split point of each sequence: n=8 named families times
/// k=(sequence length - 1) split points each, roughly 90 generated cases
/// in total, reported precisely in `qa-evidence.md` rather than
/// approximated.
#[test]
fn every_named_family_blocks_with_no_grid_effect_at_every_split_boundary() {
    let baseline = grid_snapshot(&new_term());

    for case in BLOCKED_FAMILY_CORPUS {
        let check_leak = !case.secret_marker.is_empty();

        let (term, blocked) = feed_chunks(&[case.sequence]);
        assert!(
            blocked.contains(&case.expected_family),
            "{}: unsplit sequence did not block as {:?}; blocked = {:?}",
            case.name,
            case.expected_family,
            blocked
        );
        assert_eq!(
            grid_snapshot(&term),
            baseline,
            "{}: unsplit sequence changed grid state",
            case.name
        );
        if check_leak {
            assert!(
                !full_grid_text(&term).contains(case.secret_marker),
                "{}: unsplit sequence leaked marker into the grid",
                case.name
            );
        }

        for split in 1..case.sequence.len() {
            let (left, right) = case.sequence.split_at(split);
            let (term, blocked) = feed_chunks(&[left, right]);
            assert_eq!(
                grid_snapshot(&term),
                baseline,
                "{}: split at {split} changed grid state",
                case.name
            );
            assert!(
                blocked.contains(&case.expected_family),
                "{}: split at {split} did not block as {:?}; blocked = {:?}",
                case.name,
                case.expected_family,
                blocked
            );
            if check_leak {
                assert!(
                    !full_grid_text(&term).contains(case.secret_marker),
                    "{}: split at {split} leaked marker into the grid",
                    case.name
                );
            }
        }
    }
}

/// Finding, carried from the spike: `vte::ansi::Processor`'s own
/// `Perform::hook`/`put`/`unhook` implementations are unconditional
/// no-ops -- no DCS content of any kind reaches a `Handler` method,
/// recognized or not. `blocked` is legitimately empty here: not a bypass,
/// P1 (single ingress to `Handler`) holds trivially because `Processor`
/// never calls out for DCS at all.
#[test]
fn generic_dcs_content_never_reaches_handler_at_every_split() {
    let sequence: &[u8] = b"\x1bP0;1|SECRET-DCS-PAYLOAD\x1b\\";
    let (term, blocked) = feed_chunks(&[sequence]);
    assert_eq!(blocked, Vec::new());
    assert!(!full_grid_text(&term).contains("SECRET-DCS-PAYLOAD"));

    for split in 1..sequence.len() {
        let (left, right) = sequence.split_at(split);
        let (term, blocked) = feed_chunks(&[left, right]);
        assert_eq!(
            blocked,
            Vec::new(),
            "split at {split} unexpectedly reached Handler"
        );
        assert!(
            !full_grid_text(&term).contains("SECRET-DCS-PAYLOAD"),
            "split at {split} leaked DCS payload"
        );
    }
}

// --- Accepted sequences: the other direction, and the delegation ablation. ---

#[test]
fn accepted_printable_text_reaches_the_grid() {
    let (term, blocked) = feed_chunks(&[b"hello"]);
    assert_eq!(blocked, Vec::new());
    assert!(line_text(&term, 0).starts_with("hello"));
}

#[test]
fn accepted_sgr_cursor_and_clear_do_not_block() {
    let (_, blocked) = feed_chunks(&[b"\x1b[31mred\x1b[2A\x1b[K\x1b[2J"]);
    assert_eq!(
        blocked,
        Vec::new(),
        "accepted SGR/cursor/clear sequence was blocked: {blocked:?}"
    );
}

/// Checks the actual grid effect, not merely "not in `blocked`" -- an
/// accepted method that silently drops its call (declines without
/// recording anything) would still pass a blocked-list-only check. This
/// is the test that would have caught the `forward_if_accepted` gap
/// directly, had it existed first; kept alongside the blocked-list check
/// above rather than replacing it, since both signals matter.
#[test]
fn accepted_clear_screen_actually_clears_previously_written_text() {
    let (term, blocked) = feed_chunks(&[b"before-clear"]);
    assert!(line_text(&term, 0).contains("before-clear"));
    assert_eq!(blocked, Vec::new());

    let (term, blocked) = feed_chunks(&[b"before-clear\x1b[2J"]);
    assert_eq!(
        blocked,
        Vec::new(),
        "clear_screen must not be blocked: {blocked:?}"
    );
    assert!(
        !full_grid_text(&term).contains("before-clear"),
        "clear_screen must actually clear the grid, not merely avoid being blocked: {:?}",
        full_grid_text(&term)
    );
}

#[test]
fn accepted_c0_controls_do_not_block() {
    let (_, blocked) = feed_chunks(&[b"a\r\n\t\x08b"]);
    assert_eq!(blocked, Vec::new());
}

/// The property that distinguishes real delegation from a renamed copy:
/// `SecurityFilter::accepts` reads `TerminalSequencePolicy::ACCEPTED`
/// itself, so this test cannot exercise the ablation directly (the const
/// is compiled into `tekstide-core`) -- it instead documents, and
/// `qa-evidence.md` records, the ablation actually run: temporarily
/// removing `CsiClearScreen` from `ACCEPTED` in `tekstide-core` and
/// confirming `clear_screen` stops forwarding, then reverting.
#[test]
fn accepted_sequence_variants_used_by_this_filter_match_the_nine_forwarded_methods() {
    // Transcription check: every variant this filter's `accepts` calls
    // name is one `TerminalSequencePolicy::ACCEPTED` actually lists --
    // catches a typo'd variant silently compiling to "always false"
    // (which clippy would not flag, since the match is exhaustive over a
    // real enum either way).
    use tekstide_core::runtime::terminal::TerminalSequencePolicy;
    for variant in [
        TerminalAcceptedSequence::PrintableUtf8,
        TerminalAcceptedSequence::C0CarriageReturn,
        TerminalAcceptedSequence::C0LineFeed,
        TerminalAcceptedSequence::C0Tab,
        TerminalAcceptedSequence::C0Backspace,
        TerminalAcceptedSequence::CsiSgr,
        TerminalAcceptedSequence::CsiCursorMovement,
        TerminalAcceptedSequence::CsiClearLine,
        TerminalAcceptedSequence::CsiClearScreen,
    ] {
        assert!(
            TerminalSequencePolicy::ACCEPTED.contains(&variant),
            "{variant:?} is used by this filter's forwarding methods but is not in \
             TerminalSequencePolicy::ACCEPTED -- the filter would silently stop forwarding it"
        );
    }
}

// --- V2: 8-bit C1 forms (carried finding from the spike). ---
//
// vte 0.15 "only supports 7-bit codes" -- a lone 0x90/0x9D byte is
// invalid UTF-8 and `Processor::execute()` has no arm for it, so no
// `Handler` method fires at all. Unlike `tekstide_core::
// TerminalSecurityParser` (which explicitly recognizes and blocks these
// as C1 introducers at the byte-parser level), the emulator's own parser
// is narrower here, not wider. The introducer byte is consumed as
// invalid UTF-8 and the parser resumes in `Ground` state, so the
// following payload renders as plain text -- blocked as an operation,
// visible as text. Recorded honestly, not asserted as fully inert.

#[test]
fn v2_8bit_c1_osc_introducer_blocks_the_operation_but_payload_text_still_renders() {
    let (term, blocked) = feed_chunks(&[b"\x9d0;PWNED-C1\x07"]);
    assert_eq!(blocked, Vec::new());
    assert!(
        full_grid_text(&term).contains("0;PWNED-C1"),
        "expected the post-introducer payload to render as plain text: {:?}",
        full_grid_text(&term)
    );
}

#[test]
fn v2_8bit_c1_dcs_introducer_blocks_the_operation_but_payload_text_still_renders() {
    let (term, blocked) = feed_chunks(&[b"\x901$q\x9c"]);
    assert_eq!(blocked, Vec::new());
    assert!(full_grid_text(&term).contains("1$q"));
}

// --- V4: unterminated sequence at stream end, continued in the next chunk. ---

#[test]
fn v4_unterminated_osc_continues_correctly_into_next_chunk() {
    let (term, blocked) = feed_chunks(&[b"\x1b]52;c;", b"SGVsbG8=\x07after"]);
    assert!(blocked.contains(&TerminalSequenceFamily::Osc52Clipboard));
    assert!(!full_grid_text(&term).contains("SGVsbG8"));
    assert!(
        full_grid_text(&term).contains("after"),
        "text following the blocked OSC must still reach the grid"
    );
}

// --- V5: parameter-overflow probes (carried from the spike). ---

#[test]
fn v5_parameter_overflow_does_not_desync_the_parser() {
    let mut sequence = b"\x1b[".to_vec();
    for _ in 0..500 {
        sequence.extend_from_slice(b"0;");
    }
    sequence.push(b'm');
    sequence.extend_from_slice(b"AFTER");

    let (term, blocked) = feed_chunks(&[&sequence]);
    assert_eq!(
        blocked,
        Vec::new(),
        "parameter overflow alone must not be classified as blocked"
    );
    assert!(full_grid_text(&term).contains("AFTER"));
}

#[test]
fn v5_parameter_overflow_followed_by_osc_52_still_blocks_clipboard() {
    let mut sequence = b"\x1b[".to_vec();
    for _ in 0..500 {
        sequence.extend_from_slice(b"0;");
    }
    sequence.push(b'm');
    sequence.extend_from_slice(b"\x1b]52;c;U0VDUkVU\x07");

    let (term, blocked) = feed_chunks(&[&sequence]);
    assert!(blocked.contains(&TerminalSequenceFamily::Osc52Clipboard));
    assert!(!full_grid_text(&term).contains("U0VDUkVU"));
}

#[test]
fn v6_colon_subparameters_forward_as_terminal_attribute_not_blocked() {
    let (_, blocked) = feed_chunks(&[b"\x1b[38:2:255:0:0m"]);
    assert_eq!(
        blocked,
        Vec::new(),
        "colon-form SGR must forward via terminal_attribute like the semicolon form"
    );
}

#[test]
fn v7_utf8_split_reassembles_correctly_at_every_boundary() {
    let phrase = "こんにちは世界";
    let bytes = phrase.as_bytes();

    for split in 1..bytes.len() {
        let (left, right) = bytes.split_at(split);
        let (term, blocked) = feed_chunks(&[left, right]);
        assert_eq!(
            blocked,
            Vec::new(),
            "split at {split} should not be classified as blocked"
        );
        assert!(
            full_grid_text(&term).contains(phrase),
            "split at {split} did not reassemble correctly"
        );
    }
}

// --- V8 (documented, not executed as a test): direct API access. ---
//
// `Term::grid_mut()` is public and would let calling code bypass this
// filter entirely -- but it is not reachable from the PTY byte path,
// since `vte::ansi::Processor` never calls it; only `Handler` methods
// are called from `advance()`. P1 therefore depends on calling code
// never wiring PTY bytes to anything but
// `Processor::advance(&mut SecurityFilter::new(&mut term), bytes)`.
// **No production caller exists yet in this slice** (PR-017-C builds
// the pane) -- recorded here and in `qa-evidence.md` as a code-discipline
// requirement for that slice, not something this one enforces mechanically.
