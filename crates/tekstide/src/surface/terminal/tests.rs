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
    let rows = super::grid_colors::styled_rows(term);
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
        super::grid_colors::styled_rows(&self.pane.term)
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
