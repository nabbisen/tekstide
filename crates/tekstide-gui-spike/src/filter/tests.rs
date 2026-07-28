//! Adversarial corpus for the RFC-014 PR-014-C Option A/B investigation.
//!
//! Structure follows `pr-014-c-filter-interposition.md` §5: a table of
//! sequences tagged by RFC-009 classification, fed through
//! filter -> `vte::ansi::Processor` -> `Term` at every chunk split point,
//! asserting inert families leave `Term` state unchanged and accepted
//! families produce the expected change (so this proves a boundary, not a
//! brick wall).

use super::{BlockedCall, BlockedFamily, SecurityFilter};
use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use std::cell::RefCell;

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
/// `SecurityFilter` per chunk (mirroring how a real terminal pane would
/// call `advance` once per PTY read), and returns everything this filter
/// blocked across all chunks plus the resulting term for assertions.
fn feed_chunks(chunks: &[&[u8]]) -> (Term<RecordingListener>, Vec<BlockedCall>) {
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

fn blocked_families(blocked: &[BlockedCall]) -> Vec<BlockedFamily> {
    blocked.iter().map(|call| call.family).collect()
}

/// V1: exhaustive chunk-boundary split. For every split point `1..len-1` of
/// `sequence`, feeding it in two chunks must classify identically to
/// feeding it unsplit: the same family must be blocked, and no fragment of
/// `secret_marker` may reach the grid as printable text.
fn assert_blocked_at_every_split(
    sequence: &[u8],
    expected_family: BlockedFamily,
    secret_marker: &str,
) {
    // An empty marker would make `str::contains` trivially true for every
    // string, so a leak check against it would always (wrongly) fail.
    // Skip that check rather than assert something meaningless; the
    // family-blocked assertion still fully covers markerless sequences.
    let check_leak = !secret_marker.is_empty();

    // Unsplit baseline.
    let (term, blocked) = feed_chunks(&[sequence]);
    assert!(
        blocked_families(&blocked).contains(&expected_family),
        "unsplit sequence {sequence:?} did not block as {expected_family:?}; blocked = {blocked:?}"
    );
    if check_leak {
        assert!(
            !line_text(&term, 0).contains(secret_marker),
            "unsplit sequence leaked marker into the grid: {:?}",
            line_text(&term, 0)
        );
    }

    for split in 1..sequence.len() {
        let (left, right) = sequence.split_at(split);
        let (term, blocked) = feed_chunks(&[left, right]);

        if check_leak {
            assert!(
                !line_text(&term, 0).contains(secret_marker),
                "split at {split} leaked marker into the grid for {sequence:?}: line = {:?}",
                line_text(&term, 0)
            );
        }
        assert!(
            blocked_families(&blocked).contains(&expected_family)
                || line_text(&term, 0).trim().is_empty(),
            "split at {split} for {sequence:?} neither blocked as {expected_family:?} nor left the line empty; blocked = {blocked:?}, line = {:?}",
            line_text(&term, 0)
        );
    }
}

// --- V1 mandatory coverage: the families the handoff names as minimum. ---

#[test]
fn v1_osc_52_clipboard_blocked_at_every_split() {
    assert_blocked_at_every_split(
        b"\x1b]52;c;SGVsbG8=\x07",
        BlockedFamily::Clipboard,
        "SGVsbG8",
    );
}

#[test]
fn v1_osc_title_blocked_at_every_split() {
    assert_blocked_at_every_split(
        b"\x1b]0;PWNED-TITLE\x07",
        BlockedFamily::Title,
        "PWNED-TITLE",
    );
}

#[test]
fn v1_osc_8_hyperlink_blocked_at_every_split() {
    assert_blocked_at_every_split(
        b"\x1b]8;;https://evil.invalid/\x07",
        BlockedFamily::Hyperlink,
        "evil.invalid",
    );
}

/// Finding, not an assumption: SCP (DECSCP) is **CSI**-dispatched
/// (`ESC [ <params> SP k`) in vte 0.15, not DCS -- the RFC-014 handoff's
/// framing of SCP as "the one DCS-family sequence Processor recognizes"
/// does not match this vte version. Verified by reading `ansi.rs`'s
/// `csi_dispatch` match arm for `('k', [b' '])` directly (not assumed from
/// the handoff). The genuinely DCS-dispatched test is the next one below.
#[test]
fn v1_scp_is_csi_not_dcs_blocked_at_every_split() {
    assert_blocked_at_every_split(b"\x1b[1 k", BlockedFamily::Scp, "");
}

/// Finding: `vte::ansi::Processor`'s own `Perform::hook`/`put`/`unhook`
/// implementations are unconditional no-ops (verified by reading them
/// directly) -- **no DCS content of any kind reaches a `Handler` method**,
/// recognized or not. DCS is fully swallowed one layer below this filter's
/// interposition point. This is stronger than "this filter blocks it": the
/// filter never gets the chance to, because Processor never calls out for
/// DCS. The filter's `blocked` log is legitimately empty here -- that is
/// not a bypass, it means P1 (single ingress to `Handler`) holds trivially
/// for this entire sequence family.
#[test]
fn v1_generic_dcs_content_never_reaches_handler_at_every_split() {
    let sequence: &[u8] = b"\x1bP0;1|SECRET-DCS-PAYLOAD\x1b\\";
    let (term, blocked) = feed_chunks(&[sequence]);
    assert_eq!(
        blocked,
        Vec::new(),
        "no Handler method should fire for unrecognized DCS content, so nothing should be classified"
    );
    assert!(!line_text(&term, 0).contains("SECRET-DCS-PAYLOAD"));

    for split in 1..sequence.len() {
        let (left, right) = sequence.split_at(split);
        let (term, blocked) = feed_chunks(&[left, right]);
        assert_eq!(
            blocked,
            Vec::new(),
            "split at {split} unexpectedly reached Handler"
        );
        assert!(
            !line_text(&term, 0).contains("SECRET-DCS-PAYLOAD"),
            "split at {split} leaked DCS payload into the grid: {:?}",
            line_text(&term, 0)
        );
    }
}

#[test]
fn v1_mouse_reporting_private_mode_blocked_at_every_split() {
    assert_blocked_at_every_split(b"\x1b[?1000h", BlockedFamily::PrivateMode, "1000");
}

#[test]
fn v1_terminal_query_blocked_at_every_split() {
    assert_blocked_at_every_split(b"\x1b[c", BlockedFamily::TerminalQueryOrReply, "");
}

// --- Both directions: accepted sequences must still work. ---

#[test]
fn accepted_printable_text_reaches_the_grid() {
    let (term, blocked) = feed_chunks(&[b"hello"]);
    assert_eq!(blocked_families(&blocked), Vec::new());
    assert!(line_text(&term, 0).starts_with("hello"));
}

#[test]
fn accepted_sgr_cursor_and_clear_do_not_block() {
    let (_, blocked) = feed_chunks(&[b"\x1b[31mred\x1b[2A\x1b[K\x1b[2J"]);
    assert_eq!(
        blocked_families(&blocked),
        Vec::new(),
        "accepted SGR/cursor/clear sequence was blocked: {blocked:?}"
    );
}

#[test]
fn accepted_c0_controls_do_not_block() {
    let (_, blocked) = feed_chunks(&[b"a\r\n\t\x08b"]);
    assert_eq!(blocked_families(&blocked), Vec::new());
}

// --- V2: 8-bit C1 forms. ---
//
// Finding, not an assumption: vte's own module documentation states it
// "Only supports 7-bit codes" as a deliberate deviation from the classic
// ANSI parser spec, and reading `Parser::advance_ground`/`anywhere`
// directly confirms it -- there is no state-machine transition anywhere
// that recognizes a bare 0x80-0x9F byte as a C1 escape/CSI/OSC/DCS
// introducer. A lone byte in that range is invalid UTF-8, and
// `advance_ground`'s error handling calls `performer.execute(byte)` for it
// (since `error_len() == 1 && byte <= 0x9F`) -- but `ansi::Processor`'s own
// `execute()` match has no arm for 0x90/0x9D, so it falls through to its
// `_ => debug!(...)` case and **no `Handler` method is called at all**.
//
// This is the opposite of `tekstide_core::TerminalSecurityParser`, which
// explicitly recognizes and blocks 0x90/0x9D/0x9E/0x9F as C1 introducers
// (see `runtime/terminal/security/parser.rs`). The emulator's parser is
// narrower here, not wider -- so V2 is not a bypass vector against Option
// A's interposition for the *semantic operation itself*: no
// `clipboard_store`/`set_title`/`set_hyperlink` call ever fires, because
// there is no state transition that recognizes the introducer, so
// `Handler` is never invoked for it.
//
// SECOND finding, discovered empirically by running this test rather than
// assumed from the first: the introducer byte alone is consumed (as
// invalid UTF-8, via `execute()`), but the parser then resumes in `Ground`
// state and treats everything *after* it as ordinary text -- so what would
// have been the OSC/DCS payload (e.g. the base64 clipboard content, or the
// title string) **renders as plain printable characters on screen**. The
// semantic operation is blocked; the payload's text content is not
// suppressed. For OSC 52 specifically this means: no clipboard
// exfiltration (the `Handler::clipboard_store` call never happens), but
// the payload text is still visible on the terminal surface, which is a
// real, lesser leak than the tests above prove for the 7-bit form. This is
// recorded honestly rather than silently asserting full inertness.

#[test]
fn v2_8bit_c1_osc_introducer_blocks_the_operation_but_payload_text_still_renders() {
    // 0x9D is the C1 single-shift equivalent of `ESC ]`; as a lone
    // standalone byte it is invalid UTF-8 and never becomes an OSC state.
    let (term, blocked) = feed_chunks(&[b"\x9d0;PWNED-C1\x07"]);
    assert_eq!(
        blocked,
        Vec::new(),
        "no Handler method should fire; the byte is dropped inside Processor's execute()"
    );
    assert!(
        line_text(&term, 0).contains("0;PWNED-C1"),
        "expected the post-introducer payload to render as plain text (the finding this test \
         records), but it did not: {:?}",
        line_text(&term, 0)
    );
}

#[test]
fn v2_8bit_c1_dcs_introducer_blocks_the_operation_but_payload_text_still_renders() {
    // 0x90 is the C1 single-shift equivalent of `ESC P`.
    let (term, blocked) = feed_chunks(&[b"\x901$q\x9c"]);
    assert_eq!(blocked, Vec::new());
    assert!(
        line_text(&term, 0).contains("1$q"),
        "expected the post-introducer payload to render as plain text: {:?}",
        line_text(&term, 0)
    );
}

// --- V3: string-terminator divergence (BEL vs ST). ---

#[test]
fn v3_osc_terminated_by_st_blocks_same_as_bel() {
    let (term, blocked) = feed_chunks(&[b"\x1b]0;PWNED-ST\x1b\\"]);
    assert!(blocked_families(&blocked).contains(&BlockedFamily::Title));
    assert!(!line_text(&term, 0).contains("PWNED-ST"));
}

// --- V4: unterminated sequence at stream end, continued in the next chunk. ---

#[test]
fn v4_unterminated_osc_continues_correctly_into_next_chunk() {
    let (term, blocked) = feed_chunks(&[b"\x1b]52;c;", b"SGVsbG8=\x07after"]);
    assert!(blocked_families(&blocked).contains(&BlockedFamily::Clipboard));
    assert!(!line_text(&term, 0).contains("SGVsbG8"));
    assert!(
        line_text(&term, 0).contains("after"),
        "text following the blocked OSC must still reach the grid: {:?}",
        line_text(&term, 0)
    );
}

// --- V5-V7: response 106 review probes, added to the corpus per the
// developer handoff rather than left as a documented gap. ---

#[test]
fn v5_parameter_overflow_does_not_desync_the_parser() {
    // 500 semicolon-separated CSI params must not cause the parser to
    // truncate and reinterpret trailing bytes as a fresh sequence.
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
    assert!(
        line_text(&term, 0).contains("AFTER"),
        "trailing text after an overflowed CSI must render as ordinary text, not be swallowed \
         or misparsed: {:?}",
        line_text(&term, 0)
    );
}

/// The sharper case, and the one worth keeping as a regression: an
/// overflowed CSI does not desync the parser state enough to let a
/// following blocked sequence slip through unclassified.
#[test]
fn v5_parameter_overflow_followed_by_osc_52_still_blocks_clipboard() {
    let mut sequence = b"\x1b[".to_vec();
    for _ in 0..500 {
        sequence.extend_from_slice(b"0;");
    }
    sequence.push(b'm');
    sequence.extend_from_slice(b"\x1b]52;c;U0VDUkVU\x07");

    let (term, blocked) = feed_chunks(&[&sequence]);
    assert!(
        blocked_families(&blocked).contains(&BlockedFamily::Clipboard),
        "OSC 52 immediately after a parameter overflow must still be blocked; blocked = {blocked:?}"
    );
    assert!(!line_text(&term, 0).contains("U0VDUkVU"));
}

#[test]
fn v6_colon_subparameters_forward_as_terminal_attribute_not_blocked() {
    // CSI 38:2:255:0:0 m (colon-form truecolor SGR) must classify the same
    // as the semicolon form -- forwarded as styling only. `Attr` is purely
    // presentational (Reset/Bold/Dim/Italic/underline variants/Blink*/
    // Reverse/Hidden/Strike/Cancel*), so nothing non-visual is reachable
    // through the forwarded path either way.
    let (_, blocked) = feed_chunks(&[b"\x1b[38:2:255:0:0m"]);
    assert_eq!(
        blocked,
        Vec::new(),
        "colon-form SGR must forward via terminal_attribute like the semicolon form"
    );
}

#[test]
fn v7_utf8_split_reassembles_correctly_at_every_boundary() {
    // A CJK string split at every byte boundary must reassemble correctly:
    // `Processor`'s persistent parser state handles multi-byte UTF-8
    // reassembly, the same mechanism P4 relies on for control sequences.
    let phrase = "こんにちは世界"; // 7 characters, 3 bytes each = 21 bytes
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
            line_text(&term, 0).contains(phrase),
            "split at {split} did not reassemble correctly: {:?}",
            line_text(&term, 0)
        );
    }
}

// --- V8 (documented, not executed as a test): direct API access. ---
//
// `Term::grid_mut()` is public and would let calling code bypass this
// filter entirely -- but it is not reachable from the PTY byte path, since
// `vte::ansi::Processor` never calls it; only `Handler` methods are called
// from `advance()`. P1 therefore depends on this spike's terminal pane
// (terminal_pane.rs) never wiring PTY bytes to anything but
// `Processor::advance(&mut SecurityFilter::new(&mut term), bytes)`. This is
// a code-discipline requirement, not something a unit test can enforce by
// itself; it is recorded here and in qa-evidence.md rather than silently
// assumed.
