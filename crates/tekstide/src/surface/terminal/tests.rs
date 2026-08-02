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

use super::{COLS, PaneSize, ROWS, SCROLLBACK_LINES, TerminalPane, pane_config};

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
fn snapshot(term: &Term<VoidListener>) -> String {
    let rows = super::styled_rows(term);
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
    let mut term = Term::new(pane_config(), &PaneSize, VoidListener);
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
    let actual = snapshot(&term);
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
    let mut narrow = Term::new(pane_config(), &PaneSize, VoidListener);
    let mut wide_config = pane_config();
    wide_config.scrolling_history = SCROLLBACK_LINES * 2;
    let mut wide = Term::new(wide_config, &PaneSize, VoidListener);

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
        let pane = TerminalPane::launch(
            project.id().clone(),
            "test pane",
            root.clone(),
            PathBuf::from("/bin/sh"),
        )
        .expect("launch a plain shell for a real-PTY test");
        Self { root, pane }
    }

    fn rendered_text(&self) -> String {
        super::styled_rows(&self.pane.term)
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
