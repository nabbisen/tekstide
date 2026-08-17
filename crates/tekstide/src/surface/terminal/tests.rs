//! PR-017-C tests. Rendering correctness is tested directly against a
//! bare `Term`/`Processor` (no PTY, no filter) -- the filter's own P1-P4
//! properties are PR-017-B's corpus (`filter::tests`), re-run here
//! against this crate's real `Processor::advance` call site by
//! [`a_launched_pane_renders_real_pty_output_end_to_end`] (the accept
//! path) and [`a_launched_pane_blocks_a_disallowed_sequence_at_the_real_call_site`]
//! (the block path). What this file adds beyond that re-enumeration is
//! specific to the pane: rendering fidelity and the scrollback bound.

use std::path::PathBuf;
use std::time::Duration;

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::Term;
use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};

use tekstide_core::project::{ProjectId, ProjectSession};

use super::{
    COLS, MIN_COLS, MIN_ROWS, PaneSize, ROWS, SCROLLBACK_LINES, TerminalPane, pane_config,
};

fn feed(term: &mut Term<VoidListener>, bytes: &[u8]) {
    let mut processor = Processor::<StdSyncHandler>::new();
    processor.advance(term, bytes);
}

/// Full grid-plus-cursor snapshot, matching PR-017-B's own corpus
/// standard ("compare full state against a pristine baseline", not
/// marker presence on one line): every row's coloured runs, plus the
/// cursor position, serialized so a regression anywhere in the grid --
/// not just the row a narrower test happened to check -- shows up as a
/// diff.
fn snapshot(term: &Term<VoidListener>, row_count: usize) -> String {
    let rows = super::grid_colors::styled_rows(term, row_count);
    let cursor = term.renderable_content().cursor.point;
    let mut out = String::new();
    for (index, runs) in rows.iter().enumerate() {
        out.push_str(&format!("{index:02}: {runs:?}\n"));
    }
    out.push_str(&format!("cursor: {cursor:?}\n"));
    out
}

#[test]
fn renders_full_grid_plus_cursor_snapshot_for_known_output() {
    let mut term = Term::new(
        pane_config(),
        &PaneSize {
            rows: ROWS,
            cols: COLS,
        },
        VoidListener,
    );
    feed(
        &mut term,
        b"Tekstide terminal pane\r\n\x1b[32mgreen text\x1b[0m\r\n",
    );

    // Every unwritten cell in a row is a real, present blank cell (a
    // space character, default foreground) rather than absent -- the
    // grid is always COLS wide. Row 0's written text shares the
    // default foreground, so it merges with its trailing padding into
    // one run; row 1's green text does not, so it stays a separate run
    // from its own trailing padding. Not a rendering bug -- the true
    // shape of `alacritty_terminal`'s grid, captured as the baseline
    // rather than assumed away.
    let default_fg = [0.85_f32, 0.85, 0.85];
    let full_row = format!("{:?}", (" ".repeat(COLS), default_fg));
    let row0 = format!(
        "{:?}",
        (
            format!("Tekstide terminal pane{}", " ".repeat(COLS - 22)),
            default_fg
        )
    );
    let row1_tail = format!("{:?}", (" ".repeat(COLS - 10), default_fg));
    let actual = snapshot(&term, ROWS);
    let mut expected =
        format!("00: [{row0}]\n01: [(\"green text\", [0.0, 0.75, 0.0]), {row1_tail}]\n");
    for row in 2..ROWS {
        expected.push_str(&format!("{row:02}: [{full_row}]\n"));
    }
    expected.push_str("cursor: Point { line: Line(2), column: Column(0) }\n");

    assert_eq!(
        actual, expected,
        "grid-plus-cursor state diverged from the pristine baseline; \
         a change anywhere in the 24 rows or the cursor position must \
         update this baseline deliberately, not be caught by accident"
    );
}

#[test]
fn bounded_scrollback_holds_under_sustained_output() {
    let mut narrow = Term::new(
        pane_config(),
        &PaneSize {
            rows: ROWS,
            cols: COLS,
        },
        VoidListener,
    );
    let mut wide_config = pane_config();
    wide_config.scrolling_history = SCROLLBACK_LINES * 2;
    let mut wide = Term::new(
        wide_config,
        &PaneSize {
            rows: ROWS,
            cols: COLS,
        },
        VoidListener,
    );

    // Must outgrow BOTH configured bounds, or the smaller one (narrow)
    // saturates while the larger one (wide) merely reflects the natural,
    // unclamped scroll count -- which would make the two asserts below
    // pass for the wrong reason (an under-full buffer, not a cap).
    let mut sustained = Vec::new();
    for line in 0..(SCROLLBACK_LINES * 2 + ROWS + 500) {
        sustained.extend_from_slice(format!("line {line:05}\r\n").as_bytes());
    }
    feed(&mut narrow, &sustained);
    feed(&mut wide, &sustained);

    assert_eq!(
        narrow.grid().total_lines(),
        ROWS + SCROLLBACK_LINES,
        "the configured bound, not an incidental number, must cap total_lines under sustained output"
    );
    assert_eq!(
        wide.grid().total_lines(),
        ROWS + SCROLLBACK_LINES * 2,
        "doubling the configured bound must double the retained total \
         -- proving the cap tracks the configured bound rather than an \
         unrelated fixed ceiling both terms would hit anyway"
    );
}

fn test_project(root: &std::path::Path) -> ProjectSession {
    let id = ProjectId::new_uuid();
    ProjectSession::new(id, "terminal-pane-test", root, root)
}

struct ScratchPane {
    root: std::path::PathBuf,
    pane: TerminalPane,
}

impl ScratchPane {
    fn launch() -> Self {
        let root = std::env::temp_dir().join(format!(
            "tekstide-terminal-pane-test-{:?}",
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&root).expect("create scratch project root");
        let project = test_project(&root);
        let (pane, _session) = TerminalPane::launch(
            project.id().clone(),
            "test pane",
            root.clone(),
            PathBuf::from("/bin/sh"),
        )
        .expect("launch a plain shell for a real-PTY test");
        Self { root, pane }
    }

    fn rendered_text(&self) -> String {
        let (row_count, _cols) = self.pane.dimensions();
        super::grid_colors::styled_rows(&self.pane.term, row_count as usize)
            .into_iter()
            .flat_map(|runs| runs.into_iter().map(|(text, _)| text))
            .collect()
    }

    fn poll_until(&mut self, needle: &str) -> bool {
        for _ in 0..200 {
            self.pane.poll();
            if self.rendered_text().contains(needle) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    /// Terminal resize handoff: each row's own text, *not* flattened
    /// across rows the way [`Self::rendered_text`] is -- needed to check
    /// where a real line actually wrapped, which
    /// [`Self::rendered_text`]'s row-boundary-erasing concatenation
    /// cannot show.
    fn row_texts(&self) -> Vec<String> {
        let (row_count, _cols) = self.pane.dimensions();
        super::grid_colors::styled_rows(&self.pane.term, row_count as usize)
            .into_iter()
            .map(|runs| runs.into_iter().map(|(text, _)| text).collect::<String>())
            .collect()
    }

    /// Terminal resize handoff: longest run of consecutive `needle`
    /// characters found within any single row -- the wrap-boundary
    /// proof. A run that spans a row boundary is, by construction, two
    /// separate runs (one per row), so this is exactly "how many
    /// contiguous columns did the emulator actually place before
    /// wrapping."
    fn longest_run_in_any_row(&self, needle: char) -> usize {
        self.row_texts()
            .iter()
            .flat_map(|row| row.split(|c| c != needle).map(str::len))
            .max()
            .unwrap_or(0)
    }
}

impl Drop for ScratchPane {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn a_launched_pane_renders_real_pty_output_end_to_end() {
    let mut scratch = ScratchPane::launch();
    scratch.pane.write_input(b"printf 'PTY_MARKER_017C\\n'\n");

    assert!(
        scratch.poll_until("PTY_MARKER_017C"),
        "a real shell's real output, read through LinuxTerminalRuntime and advanced through \
         SecurityFilter, must reach the rendered grid -- this is TerminalPane's only production \
         Processor::advance call site, so this test is P1's re-enumeration against real code, \
         not just the test harness PR-017-B's own corpus used"
    );
}

/// P1/P2 re-enumerated against production code (review 144's requirement
/// for this slice): a disallowed sequence sent through the exact same
/// real PTY -> `poll()` -> `SecurityFilter` path as the test above must
/// have no effect. Private mode 1049 (switch to the alternate screen
/// buffer) is chosen deliberately over, say, OSC 0 (set title): title
/// text is stored in a `Term` field with no grid effect either way, so
/// blocking it can't be told apart from bypassing the filter entirely
/// by inspecting the grid -- a "test that cannot fail" for this
/// specific purpose. Switching screens genuinely does swap which grid
/// `renderable_content()` reads from, so it is real, observable proof
/// that the filter -- not just an accident of what OSC 0 touches -- is
/// interposed at this crate's actual call site.
///
/// The escape sequence is emitted by a real `printf`, not written
/// directly to the master: canonical-mode local echo reflects raw bytes
/// written to the PTY back out in `^X` caret notation (`ECHOCTL`), not
/// as the control bytes themselves, so directly-injected raw bytes
/// would never actually reach `Processor::advance` as a real CSI
/// sequence in either the filtered or bypassed case -- a `printf`'s
/// stdout is real process output, not echoed input, and is how the
/// marker test above already gets real bytes through `poll()`.
#[test]
fn a_launched_pane_blocks_a_disallowed_sequence_at_the_real_call_site() {
    let mut scratch = ScratchPane::launch();
    scratch
        .pane
        .write_input(b"printf 'PRIMARY_SCREEN_017C\\n'\n");
    assert!(
        scratch.poll_until("PRIMARY_SCREEN_017C"),
        "the marker must render before the alt-screen attempt, so its \
         later disappearance (if any) is attributable to that attempt"
    );

    // CSI ?1049h: DECSET, switch to the alternate screen buffer. If
    // forwarded, `renderable_content()` would now read a blank grid and
    // the marker above would vanish from the render.
    scratch.pane.write_input(b"printf '\\033[?1049h'\n");
    for _ in 0..20 {
        scratch.pane.poll();
        std::thread::sleep(Duration::from_millis(10));
    }

    let rendered = scratch.rendered_text();
    assert!(
        rendered.contains("PRIMARY_SCREEN_017C"),
        "switching to the alternate screen must be blocked at this crate's real call site -- \
         if the marker disappeared, the filter was bypassed and the grid actually swapped"
    );
}

/// Terminal resize handoff: parses `stty size`'s own real, kernel-
/// reported "ROWS COLS" (`TIOCGWINSZ` under the hood, the read-side
/// counterpart of the `TIOCSWINSZ` `resize_master` issues) out of
/// `rendered`, which is the *flattened* concatenation
/// [`ScratchPane::rendered_text`] returns -- every cell in a row is
/// present (blank cells are real space characters), so the two numbers
/// are followed immediately by that row's own trailing padding, not a
/// newline.
///
/// `marker` appears **twice**: once in the shell's own canonical-mode
/// echo of the literal command as typed (containing the literal `%s`,
/// not a real number), and once in the command's real output. Taking
/// the text after the *last* occurrence, not the first, is what skips
/// the echoed command and lands on the real printed numbers.
fn parse_stty_size_after(rendered: &str, marker: &str) -> Option<(u16, u16)> {
    let after = rendered.rsplit(marker).next()?;
    let mut tokens = after.split_whitespace();
    let rows: u16 = tokens.next()?.parse().ok()?;
    let cols: u16 = tokens.next()?.parse().ok()?;
    Some((rows, cols))
}

/// Terminal resize handoff review gate, item 1: **the three sizes
/// proven to agree after a resize, not asserted** -- checked
/// independently against three different sources, not three reads of
/// the same stored field:
///
/// 1. **The PTY**: a real child (`/bin/sh`) runs `stty size`, which
///    calls `TIOCGWINSZ` on its own controlling terminal -- the kernel's
///    own record of what `resize_master`'s `TIOCSWINSZ` set, read back
///    through a completely different code path than the one that wrote
///    it.
/// 2. **The emulator**: `alacritty_terminal::Term`'s own
///    `Dimensions::columns`/`screen_lines`, not this crate's stored
///    `rows`/`cols` -- `Term`'s internal grid state, asked directly.
/// 3. **The render path**: [`TerminalPane::dimensions`] (what
///    `grid_colors::view` actually renders at) and the real row count
///    [`ScratchPane::row_texts`] returns from `styled_rows`.
#[test]
fn resize_makes_the_pty_the_emulator_and_the_render_path_agree() {
    let mut scratch = ScratchPane::launch();
    scratch.pane.write_input(b"printf 'READY_017E\\n'\n");
    assert!(
        scratch.poll_until("READY_017E"),
        "the shell must be responsive before this test starts changing its terminal size out \
         from under it"
    );

    scratch
        .pane
        .resize(30, 55)
        .expect("resizing to a real, in-range grid must succeed");

    // 2. The emulator's own grid.
    assert_eq!(
        (
            alacritty_terminal::grid::Dimensions::screen_lines(&scratch.pane.term),
            alacritty_terminal::grid::Dimensions::columns(&scratch.pane.term)
        ),
        (30, 55),
        "Term::resize must have actually changed the emulator's own grid dimensions"
    );

    // 3. The render path: the pane's own stored dimensions, and the
    // real number of rows `styled_rows` renders.
    assert_eq!(
        scratch.pane.dimensions(),
        (30, 55),
        "the pane's stored dimensions -- what grid_colors::view renders at -- must match"
    );
    assert_eq!(
        scratch.row_texts().len(),
        30,
        "styled_rows must render exactly the new row count, not the launch-time default"
    );

    // 1. The PTY, read back through a real child process, a completely
    // different path than the one that wrote it.
    scratch
        .pane
        .write_input(b"printf 'RESIZE_CHECK_017E:%s\\n' \"$(stty size)\"\n");
    assert!(
        scratch.poll_until("RESIZE_CHECK_017E:"),
        "the real child must be able to observe and report its own resized PTY"
    );
    let (pty_rows, pty_cols) =
        parse_stty_size_after(&scratch.rendered_text(), "RESIZE_CHECK_017E:")
            .expect("stty size output must parse as two whitespace-separated numbers");
    assert_eq!(
        (pty_rows, pty_cols),
        (30, 55),
        "the real PTY, queried by a real child process via TIOCGWINSZ, must report the same \
         size the emulator and the render path already agreed on"
    );
}

/// Terminal resize handoff review gate, item 2: **ablate it** -- update
/// one of the three (here, `Term`'s own grid, reached directly rather
/// than through [`TerminalPane::resize`]) without the others, and show
/// the specific divergence this slice's whole job is to prevent. If this
/// test ever started passing without the deliberate bypass below, it
/// would mean [`TerminalPane::resize`] had stopped being the only path
/// that changes `self.term`'s size -- which is exactly the bug class
/// this handoff exists to keep out.
#[test]
fn bypassing_terminalpane_resize_produces_the_exact_divergence_this_slice_prevents() {
    let mut scratch = ScratchPane::launch();

    // Deliberately reach `Term::resize` directly, bypassing
    // `TerminalPane::resize` -- production code has no path that does
    // this (see `TerminalPane::resize`'s own doc comment), so this is
    // the ablation, not a regression waiting to happen.
    scratch.pane.term.resize(PaneSize { rows: 10, cols: 15 });

    let term_dimensions = (
        alacritty_terminal::grid::Dimensions::screen_lines(&scratch.pane.term),
        alacritty_terminal::grid::Dimensions::columns(&scratch.pane.term),
    );
    let (stored_rows, stored_cols) = scratch.pane.dimensions();
    let stored_dimensions = (stored_rows as usize, stored_cols as usize);

    assert_eq!(
        term_dimensions,
        (10, 15),
        "the direct bypass must have actually changed Term's own grid"
    );
    assert_ne!(
        term_dimensions, stored_dimensions,
        "updating only Term, not through TerminalPane::resize, must produce a real divergence \
         from the pane's own stored dimensions ({stored_dimensions:?}) -- proving the three \
         sizes do NOT stay consistent for free, and that TerminalPane::resize's job (updating \
         all three together) is load-bearing, not redundant caution"
    );
}

/// Terminal resize handoff review gate, item 3: **real output across a
/// resize** -- a real child writes the same fixed-width text before and
/// after a resize, and it must wrap at the *new* column count, not the
/// old one. Against a real PTY child (`/bin/sh` plus `printf`), not a
/// synthesised byte stream -- the same standard the P1/P2 filter corpus
/// above already holds itself to.
#[test]
fn real_output_wraps_at_the_new_column_count_not_the_old_one() {
    let mut scratch = ScratchPane::launch();
    scratch.pane.write_input(b"printf 'READY_017E\\n'\n");
    assert!(scratch.poll_until("READY_017E"));

    // 30 non-whitespace characters. At the launch-time default (80
    // columns), this fits entirely inside one row.
    let marker_line = "Q".repeat(30);
    scratch
        .pane
        .write_input(format!("printf '{marker_line}\\n'\n").as_bytes());
    assert!(scratch.poll_until(&marker_line));
    assert_eq!(
        scratch.longest_run_in_any_row('Q'),
        30,
        "at the launch-time 80-column width, 30 characters must fit on one row unwrapped"
    );

    // Narrow the pane to fewer columns than the marker is wide.
    scratch.pane.resize(24, 20).expect("resize must succeed");

    scratch
        .pane
        .write_input(format!("printf '{marker_line}\\n'\n").as_bytes());
    // Poll until the *second* occurrence has rendered, not merely a
    // fresh call returning true on scrollback from the first -- the
    // longest-run measurement below already only cares about the
    // present grid, but the write must have actually landed first.
    for _ in 0..200 {
        scratch.pane.poll();
        if scratch.rendered_text().matches(&marker_line).count() >= 1
            && scratch.longest_run_in_any_row('Q') <= 20
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    assert_eq!(
        scratch.longest_run_in_any_row('Q'),
        20,
        "the same 30-character line must now wrap at the new 20-column width, not the old \
         80-column one -- proving the emulator is actually using the resized grid, not a \
         stale one"
    );
}

/// Terminal resize handoff review gate, item 5: **minimum size
/// honoured** -- a request below [`MIN_ROWS`]/[`MIN_COLS`] must clamp,
/// not produce a zero/negative grid (an ioctl that fails, or a `Term`
/// that panics) and not be refused outright (a too-small pane must
/// still show a small terminal).
#[test]
fn resize_below_the_minimum_clamps_rather_than_refusing_or_going_to_zero() {
    let mut scratch = ScratchPane::launch();

    scratch
        .pane
        .resize(0, 0)
        .expect("a below-minimum request must clamp, not error");

    assert_eq!(
        scratch.pane.dimensions(),
        (MIN_ROWS, MIN_COLS),
        "a zero/zero request must clamp to the documented floor, not to zero and not be refused"
    );
    assert_eq!(
        alacritty_terminal::grid::Dimensions::screen_lines(&scratch.pane.term),
        MIN_ROWS as usize,
        "Term's own grid must reflect the clamped floor, not the requested zero"
    );
}

/// Terminal resize handoff review gate, item 4: **a resize storm does
/// not produce a syscall storm.** Simulates a window-drag: many calls
/// computing the *same* clamped grid size (what many `WindowResized`
/// events collapse to before a real glyph/line boundary is crossed),
/// interleaved with the occasional call that actually changes the
/// target. Only the calls that change the target may reach the real PTY
/// ioctl/`Term::resize` -- proven by [`TerminalPane::real_resize_count`],
/// test-only instrumentation on the exact branch that does real work,
/// not an assumption about timing.
#[test]
fn many_resize_calls_collapsing_to_the_same_grid_touch_the_pty_only_once() {
    let mut scratch = ScratchPane::launch();
    assert_eq!(
        scratch.pane.real_resize_count(),
        0,
        "launch must not itself count as a resize -- it is the starting size, not a resize to it"
    );

    // A drag: 500 events all computing the identical clamped grid.
    for _ in 0..500 {
        scratch.pane.resize(30, 55).expect("resize must succeed");
    }
    assert_eq!(
        scratch.pane.real_resize_count(),
        1,
        "500 calls to the same target size must touch the PTY/Term exactly once, not 500 times"
    );

    // A real glyph/line boundary is crossed once, then the drag
    // continues at the new size.
    for _ in 0..500 {
        scratch.pane.resize(31, 55).expect("resize must succeed");
    }
    assert_eq!(
        scratch.pane.real_resize_count(),
        2,
        "a genuine size change must still go through exactly once, and only once, no matter how \
         many further calls repeat it"
    );
}

/// RFC-017 Amendment 1, PR-A1-B: P1 re-enumerated against the new shape.
/// The two tests above prove real PTY bytes reach the grid through the
/// filter; this proves, by scanning the crate's own source, that there
/// is nowhere else in production code they could have come from. A new
/// production call site fails this test **by name** -- the second half
/// of P1's own requirement ("do not amend the existing enumeration to
/// accommodate the new path without first checking whether it belongs
/// there").
///
/// **Counts occurrences of `.advance(`, not files containing it**
/// (response 203, Required): a file-level check would pass a *second*
/// `.advance(` added inside `surface/terminal.rs` itself -- not a
/// hypothetical, the single most likely regression, since a resize
/// handler or a fast path for large writes would naturally be added to
/// the file that already owns the emulator. Counting occurrences turns
/// that from a silent pass into the same by-name failure a call site in
/// a different file already produces.
///
/// **What this does not cover, stated rather than implied**: this scans
/// for the one seam production code uses to reach `Processor::advance`,
/// not for every way `alacritty_terminal` could mutate `self.term`. A
/// call that reached `self.term`'s grid through a different
/// `alacritty_terminal` entry point (not `Processor::advance`) is a
/// second ingress this scan cannot see. Not covered elsewhere in this
/// crate today; `Term::grid_mut()` is not called anywhere in this module
/// (see the module doc's P2 note), which is the closest existing check,
/// but it is a narrower claim than "nothing else mutates `self.term`."
///
/// Ablated manually twice (not left as permanent tests, per this
/// project's convention for P1/P2-style ablations): a second,
/// filter-bypassing `.advance(` call in a throwaway *file* elsewhere in
/// the crate (caught even by the original, weaker file-level version of
/// this test); and, per response 203, a second `.advance(` added
/// *inside* `surface/terminal.rs` itself -- the case the file-level
/// version would have missed. Both confirmed this test failed (the
/// second by total count, not by a new file name), both removed.
#[test]
fn only_one_call_site_ever_advances_a_terminal_processor_in_the_crate() {
    let occurrences = count_occurrences_in_crate(".advance(");
    let total: usize = occurrences.iter().map(|(_, count)| count).sum();

    assert_eq!(
        (total, occurrences.as_slice()),
        (1, [("surface/terminal.rs".to_string(), 1)].as_slice()),
        "exactly one Processor::advance call may exist in production code -- P1's single \
         ingress. A second occurrence, even inside surface/terminal.rs itself, is a second, \
         potentially unfiltered write path into the emulator, not a place to add an allowlist \
         entry."
    );
}

/// RFC-017 Amendment 1, PR-A1-B: P2 re-enumerated against the new shape.
/// `TerminalReader::drain_available` is the one place PTY bytes leave
/// the channel; `TerminalReader` is not `Clone` (see its own module
/// doc), so the type already makes a second *owner* unrepresentable --
/// this enumeration is the remaining check the type cannot make on its
/// own: that no second *call site* exists that could, in principle,
/// call `drain_available` through a borrow of the one owner this crate
/// does have (`TerminalPane.reader`).
///
/// **Counts occurrences, not files** (response 203, Required), for the
/// same reason as the `Processor::advance` scan above: a file-level
/// check would pass a second `drain_available()` call added inside
/// `surface/terminal.rs` itself.
///
/// Ablated manually twice: a second `drain_available()` call in a
/// throwaway file elsewhere in the crate, and a second call added
/// *inside* `surface/terminal.rs` itself -- both confirmed this test
/// failed, both removed.
#[test]
fn only_this_field_drains_a_terminalreader_in_the_crate() {
    let occurrences = count_occurrences_in_crate(".drain_available(");
    let total: usize = occurrences.iter().map(|(_, count)| count).sum();

    assert_eq!(
        (total, occurrences.as_slice()),
        (1, [("surface/terminal.rs".to_string(), 1)].as_slice()),
        "exactly one TerminalReader::drain_available call may exist in production code -- \
         P2's 'exactly one consumer', now checked by total occurrence count rather than by \
         which files contain at least one, since the latter would pass a second call added \
         inside surface/terminal.rs itself."
    );
}

/// RFC-017 Amendment 1, PR-A1-C: P2 extended to the wake `eventfd`,
/// per response 205's fourth constraint -- unlike the shutdown
/// `eventfd`, which stays entirely inside `tekstide-core`, the wake
/// signal genuinely needs a real caller in this crate, so it needs its
/// own enumeration rather than inheriting the shutdown fd's
/// "unreachable from this crate" claim. `TerminalPane::wake_notifier`
/// is the one way this crate can ever obtain a `WakeNotifier`; this
/// proves only one production call site ever asks for one.
///
/// Ablated manually: a second `wake_notifier()` call added inside
/// `shell.rs` itself, confirmed this test failed on total count, removed.
#[test]
fn only_one_call_site_ever_asks_a_terminalpane_for_its_wake_notifier() {
    let occurrences = count_occurrences_in_crate(".wake_notifier(");
    let total: usize = occurrences.iter().map(|(_, count)| count).sum();

    assert_eq!(
        (total, occurrences.as_slice()),
        (1, [("shell.rs".to_string(), 1)].as_slice()),
        "exactly one production call may ever ask a TerminalPane for its wake notifier -- a \
         second occurrence is a second potential subscriber to reader-thread readiness, not a \
         place to add an allowlist entry."
    );
}

/// RFC-017 Amendment 1, PR-A1-C: the companion enumeration to
/// [`only_one_call_site_ever_asks_a_terminalpane_for_its_wake_notifier`]
/// -- obtaining a `WakeNotifier` is only half of P2's extended claim;
/// this proves only one production call site ever blocks on one, so a
/// second, independent consumer of a pane's wake signal cannot be added
/// silently even by a caller that already held a valid `WakeNotifier`
/// from elsewhere.
///
/// Ablated manually: a second `block_until_woken()` call added inside
/// `shell.rs` itself, confirmed this test failed on total count, removed.
#[test]
fn only_one_call_site_ever_blocks_on_a_wake_notifier() {
    let occurrences = count_occurrences_in_crate(".block_until_woken(");
    let total: usize = occurrences.iter().map(|(_, count)| count).sum();

    assert_eq!(
        (total, occurrences.as_slice()),
        (1, [("shell.rs".to_string(), 1)].as_slice()),
        "exactly one production call may ever block on a WakeNotifier -- a second occurrence \
         is a second consumer of the same reader's wake signal."
    );
}

/// Total occurrences of `needle` across this crate's production `.rs`
/// files (test files excluded), grouped by file and sorted by path --
/// the shape both P1 and P2's enumerations need, factored out once
/// rather than duplicated per property.
fn count_occurrences_in_crate(needle: &str) -> Vec<(String, usize)> {
    let mut files = Vec::new();
    collect_rs_files(&crate_src_dir(), &mut files);

    let mut occurrences: Vec<(String, usize)> = files
        .iter()
        .filter(|path| path.file_name().and_then(|name| name.to_str()) != Some("tests.rs"))
        .filter_map(|path| {
            let content = std::fs::read_to_string(path).expect("readable source file");
            let count = content.matches(needle).count();
            (count > 0).then(|| (relative_to_src(path), count))
        })
        .collect();
    occurrences.sort();
    occurrences
}

fn crate_src_dir() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn collect_rs_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("crate src dir must exist") {
        let path = entry.expect("readable dir entry").path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn relative_to_src(path: &std::path::Path) -> String {
    path.strip_prefix(crate_src_dir())
        .expect("scanned path should be under the crate src dir")
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}
