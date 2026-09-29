use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tekstide_core::content::{TextDocument, TextDocumentOpenPolicy, TextDocumentState};
use tekstide_core::project::root::{
    ProjectRootHandle, ProjectRootValidator, SymlinkPolicy, ValidProjectRoot,
};
use tekstide_core::project::{ProjectContentStatus, ProjectId, ProjectSession};

use tekstide_core::content::TextCursor;

use super::{
    RowPlan, apply_edit_key, caret_row_position, caret_split, chrome_line, columns_that_fit,
    cursor_line, document_state_symbol, empty_lines, gutter_digits, gutter_lines, navigate_cursor,
    open_error_line, row_plan, rows_that_fit, viewport_following, window_rows, windowed_line,
};
use crate::i18n::{Catalog, LocalePreference};

fn real_locales_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("locales")
}

fn real_catalog() -> Catalog {
    Catalog::resolve(LocalePreference::default(), Some(&real_locales_dir()))
}

/// A real, temp-dir-backed project root -- `TextDocument`'s only
/// constructor is `open()`, which needs a real `ProjectRootHandle`, the
/// same reason `tekstide-core`'s own `content::tests` uses this exact
/// shape.
struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "tekstide-editor-surface-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        Self { root }
    }

    fn write_file(&self, relative_path: &str, contents: &str) {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, contents).unwrap();
    }

    fn root_handle(&self) -> ProjectRootHandle {
        let valid: ValidProjectRoot = ProjectRootValidator
            .validate(&self.root, SymlinkPolicy::FailClosed)
            .expect("sandbox root must validate");
        let project = ProjectSession::new(
            ProjectId::new_uuid(),
            "editor-surface-fixture",
            valid.selected_path,
            valid.canonical_path,
        );
        ProjectRootHandle::from_project_session(&project)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn open(sandbox: &Sandbox, relative_path: &str) -> TextDocument {
    TextDocument::open(
        &sandbox.root_handle(),
        relative_path,
        TextDocumentOpenPolicy::default(),
    )
    .expect("fixture file must open")
}

/// **The text area renders raw.** A document containing `U+202E` must
/// show the raw character -- the opposite property from every other
/// surface in this crate, per RFC-016's editor exception.
#[test]
fn a_drawn_row_preserves_a_bidi_override_character_raw() {
    let sandbox = Sandbox::new("raw-bidi");
    sandbox.write_file("evil.txt", "echo proj\u{202E}gpj.exe");
    let document = open(&sandbox, "evil.txt");

    let rows = window_rows(document.text(), 0, 10);

    assert_eq!(rows, vec!["echo proj\u{202E}gpj.exe"]);
    assert!(
        !rows[0].contains("<U+202E>"),
        "the text area must never contain an escape marker, got {:?}",
        rows[0]
    );
}

/// **Ablated in the opposite direction from every other surface's own
/// bidi test**: a test asserting the escaped form appears in the text area
/// must fail. Verified by construction: `window_rows` never calls
/// `quote_untrusted`, so there is no path that could produce an escape marker to
/// assert against. The scan below holds that for the whole file.
#[test]
fn this_module_never_escapes_the_text_it_draws() {
    let source = include_str!("../editor.rs");
    let production = source.split("#[cfg(test)]\nmod ").next().unwrap();
    assert!(
        !production.lines().any(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//")
                && trimmed.contains("quote_untrusted(")
                && !trimmed.contains("text_safety::quote_untrusted(")
        }),
        "only the chrome (path, open error) may be escaped, and those call text_safety::quote_untrusted"
    );
}

const FIFTY_LINES: &str = "l0\nl1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\nl11\nl12\nl13\nl14\nl15\nl16\nl17\nl18\nl19\nl20\nl21\nl22\nl23\nl24\nl25\nl26\nl27\nl28\nl29\nl30\nl31\nl32\nl33\nl34\nl35\nl36\nl37\nl38\nl39\nl40\nl41\nl42\nl43\nl44\nl45\nl46\nl47\nl48\nl49";

/// D1: what is drawn is the viewport's window, not the file. One row is one
/// string, and the rows are exactly the lines `[first, first + capacity)`.
#[test]
fn the_rows_drawn_are_the_windows_lines_and_no_more() {
    let rows = window_rows(FIFTY_LINES, 10, 5);

    assert_eq!(rows, vec!["l10", "l11", "l12", "l13", "l14"]);
    assert_eq!(
        window_rows(FIFTY_LINES, 48, 10),
        vec!["l48", "l49"],
        "the window ends with the file"
    );
    assert!(
        window_rows(FIFTY_LINES, 500, 10).is_empty(),
        "a stale viewport past the end draws nothing, and does not panic"
    );
    assert_eq!(
        window_rows("a\n", 0, 10),
        vec!["a", ""],
        "the empty line after a final newline is a line, as the cursor counts it"
    );
}

/// `TextViewport::first_visible_line` finally has a reader: the window starts
/// where the viewport says.
#[test]
fn the_viewports_first_visible_line_decides_where_the_window_starts() {
    assert_eq!(window_rows(FIFTY_LINES, 0, 2), vec!["l0", "l1"]);
    assert_eq!(window_rows(FIFTY_LINES, 30, 2), vec!["l30", "l31"]);
}

#[test]
fn how_many_rows_fit_is_arithmetic_on_a_measured_height() {
    assert_eq!(
        rows_that_fit(None, 14.0),
        super::DEFAULT_WINDOW_ROWS,
        "before the first layout"
    );
    let pitch = super::row_pitch(14.0);
    assert_eq!(rows_that_fit(Some(pitch * 10.0 + 1.0), 14.0), 10);
    assert_eq!(
        rows_that_fit(Some(0.0), 14.0),
        1,
        "a window always holds a row"
    );
    assert_eq!(super::line_count("a\nb\n"), 3);
    assert_eq!(super::line_count(""), 1);
}

/// D9: the viewport follows the cursor -- and only the cursor. Moving inside the
/// window does not scroll it; leaving it scrolls by the least that brings the
/// cursor back; a document that shrank under a stale viewport is clamped.
#[test]
fn the_viewport_follows_the_cursor_by_the_least_it_must() {
    let viewport = |first| tekstide_core::content::TextViewport {
        first_visible_line: first,
        first_visible_column: 0,
    };
    let follow = |line, first, capacity| {
        viewport_following(FIFTY_LINES, cursor(line, 0), viewport(first), capacity, 80)
            .first_visible_line
    };

    assert_eq!(follow(12, 10, 5), 10, "inside the window: it does not move");
    assert_eq!(
        follow(14, 10, 5),
        10,
        "the last row of the window is inside it"
    );
    assert_eq!(follow(15, 10, 5), 11, "one past the window: scrolls by one");
    assert_eq!(follow(9, 10, 5), 9, "one above: scrolls by one");
    assert_eq!(
        follow(49, 0, 5),
        45,
        "the cursor at the end brings the end into the window"
    );
    assert_eq!(follow(0, 45, 5), 0);
    assert_eq!(
        follow(3, 500, 5),
        3,
        "a stale viewport past the end is pulled back to the cursor"
    );
    assert_eq!(
        follow(2, 0, 200),
        0,
        "a window that holds the whole file never scrolls"
    );
}

/// **RFC-057 Q1: the editor never hands the whole file to one widget.** PR-057-A
/// measured what that costs -- **~745 ms a keystroke** at 100 000 lines, against
/// a 16 ms budget (`editor_baseline`'s labelled reference). The document's text
/// is read for drawing in exactly one place, and it goes straight into
/// `window_rows`; no `to_string()` of it, no `body_text`, no widget built from
/// it whole. Scanned here so a scrollable body cannot come back unnoticed.
#[test]
fn the_document_text_reaches_a_widget_only_through_window_rows() {
    let source = include_str!("../editor.rs");
    let production = source.split("#[cfg(test)]\nmod ").next().unwrap();
    let lines: Vec<&str> = production.lines().collect();
    let readers: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with("//") && line.contains(".text()"))
        .map(|(index, _)| index)
        .collect();
    // RFC-057 PR-057-C: a second legitimate reader joined the first --
    // `gutter_digits(line_count(document.text()))`, which counts newlines and
    // never builds a string of the file, let alone hands one to a widget. Every
    // reader must go straight into one of the two functions that make that true
    // (`window_rows`, whose contract stays what PR-057-B's own tests pin, or
    // `line_count`); nothing else may read `.text()` at all.
    assert_eq!(
        readers.len(),
        2,
        "the document's text is read in exactly two places, both bounded: {readers:?}"
    );
    // rustfmt may put the argument on its own line; the call it belongs to is the
    // nearest line above that opens the reader's own function.
    for &reader in &readers {
        assert!(
            lines[reader.saturating_sub(2)..=reader]
                .iter()
                .any(|line| line.contains("window_rows(") || line.contains("line_count(")),
            "goes straight into window_rows or line_count: {:?}",
            lines[reader]
        );
    }
    assert!(
        !production.contains("fn body_text"),
        "the whole-body function is gone"
    );
    assert!(
        !production.lines().any(|line| !line.trim_start().starts_with("//") && line.contains("text().to_string()")),
        "the whole document is never copied into a string for drawing"
    );
}

// ---- RFC-057 PR-057-C: the gutter, the horizontal window, and the caret ----------

#[test]
fn gutter_digits_is_sized_to_the_whole_file_not_to_what_is_on_screen() {
    assert_eq!(gutter_digits(1), 1);
    assert_eq!(gutter_digits(9), 1);
    assert_eq!(gutter_digits(10), 2);
    assert_eq!(gutter_digits(9_999), 4);
    assert_eq!(gutter_digits(10_000), 5);
    assert_eq!(gutter_digits(99_999), 5);
    assert_eq!(gutter_digits(100_000), 6);
}

/// **The acceptance criterion**: a file crossing 10,000 lines does not shift
/// its text column while scrolling. Sized to `total_lines`, not to
/// `drawn_row_count` -- a window of rows 9,995..10,005 (crossing the boundary)
/// still gets the file's own digit count, the same one the window at line 1
/// would.
#[test]
fn the_gutter_does_not_shift_the_text_column_while_scrolling_across_a_power_of_ten() {
    let digits = gutter_digits(10_005);
    assert_eq!(digits, 5);

    let near_start = gutter_lines(0, 3, digits);
    let crossing = gutter_lines(9_995, 10, digits);

    for line in near_start.iter().chain(&crossing) {
        assert_eq!(line.chars().count(), 5, "{line:?}");
    }
    assert_eq!(near_start, vec!["    1", "    2", "    3"]);
    assert_eq!(crossing[3], " 9999");
    assert_eq!(crossing[4], "10000");
    assert_eq!(crossing[9], "10005");
}

/// Real line indices, never the row's position in the window: row 0 of a
/// window starting at line 9,997 reads *9998*, not *1*.
#[test]
fn gutter_lines_are_real_line_indices_not_window_positions() {
    let lines = gutter_lines(9_997, 4, 5);
    assert_eq!(lines, vec![" 9998", " 9999", "10000", "10001"]);
}

/// The text region shrinks as the gutter widens: at a fixed measured width,
/// more line-number digits leave fewer columns for the file's own text --
/// otherwise a wide gutter would overlap the text it sits beside.
#[test]
fn columns_that_fit_reserves_room_for_the_gutter_it_is_told_about() {
    assert_eq!(
        columns_that_fit(None, 14.0, 5),
        super::DEFAULT_WINDOW_COLUMNS,
        "before the first layout"
    );
    let pitch = super::column_pitch(14.0);
    let narrower = columns_that_fit(Some(pitch * 50.0), 14.0, 6);
    let wider_gutter_room = columns_that_fit(Some(pitch * 50.0), 14.0, 1);
    assert!(
        narrower < wider_gutter_room,
        "a 6-digit gutter must leave fewer text columns than a 1-digit one at the same width: {narrower} vs {wider_gutter_room}"
    );
    assert_eq!(
        columns_that_fit(Some(0.0), 14.0, 1),
        1,
        "a window always holds a column"
    );
}

#[test]
fn windowed_line_clips_by_character_not_byte_and_never_panics_past_the_end() {
    assert_eq!(windowed_line("hello world", 0, 5), "hello");
    assert_eq!(windowed_line("hello world", 6, 5), "world");
    assert_eq!(windowed_line("hello world", 6, 100), "world");
    assert_eq!(windowed_line("hello", 100, 5), "");
    // Multi-byte: each of these three characters is more than one byte.
    assert_eq!(windowed_line("h\u{e9}llo w\u{f6}rld", 0, 3), "h\u{e9}l");
    assert_eq!(windowed_line("h\u{e9}llo w\u{f6}rld", 6, 3), "w\u{f6}r");
}

/// D9, applied to the second axis: the horizontal window follows the cursor's
/// column on its own line, by the least movement that keeps it visible --
/// review 442's required Q2.
#[test]
fn the_horizontal_window_follows_the_cursor_by_the_least_it_must() {
    let text = "short\na very much longer line than the others\nx";
    let viewport = |line, col| tekstide_core::content::TextViewport {
        first_visible_line: line,
        first_visible_column: col,
    };
    let follow = |cursor_line: usize, cursor_col: usize, vp_col: usize| {
        viewport_following(
            text,
            cursor(cursor_line, cursor_col),
            viewport(0, vp_col),
            10,
            10,
        )
        .first_visible_column
    };

    assert_eq!(follow(1, 5, 0), 0, "inside the window: it does not move");
    assert_eq!(follow(1, 10, 0), 1, "one past the window: scrolls by one");
    assert_eq!(
        follow(1, 0, 5),
        0,
        "one before the window: scrolls back to it"
    );
    // A short line's own length bounds the window even if a longer line
    // elsewhere was scrolled past: the cursor's own line decides.
    assert_eq!(
        follow(0, 5, 0),
        0,
        "\"short\" is 5 chars; the cursor at its end needs no scroll"
    );
    assert_eq!(
        follow(2, 1, 0),
        0,
        "the one-character line \"x\": cursor after it, still in a 10-wide window"
    );
}

/// **Q2's own required capture, held in code, not only in a screenshot**: a
/// cursor on a long line is always inside the horizontal window after
/// following -- there is no column at which a user could type into text they
/// cannot see. Checked at every column of a line much longer than the window.
#[test]
fn a_cursor_anywhere_on_a_long_line_ends_up_inside_the_horizontal_window() {
    let line: String = (0..300)
        .map(|i| char::from(b'a' + (i % 26) as u8))
        .collect();
    let mut viewport = tekstide_core::content::TextViewport::default();
    for column in 0..=line.chars().count() {
        viewport = viewport_following(&line, cursor(0, column), viewport, 10, 12);
        assert!(
            caret_row_position(cursor(0, column), viewport, 10, 12).is_some(),
            "column {column} landed outside the window that followed it: {viewport:?}"
        );
    }
}

/// RFC-057 D2, `what-the-editor-must-not-do.md` §1: the caret is an element,
/// never a character. The fixture holds a file whose text contains the
/// caret's own plausible glyph (a full block, the character someone might
/// reach for to draw a caret by hand) at a position the real cursor is not on
/// -- `caret_split` must not conflate the two: splitting contributes **zero**
/// characters of its own, so the block glyph the file actually contains
/// survives untouched on whichever side of the split it falls.
#[test]
fn the_caret_split_contributes_no_characters_and_cannot_be_confused_with_a_real_glyph() {
    const CARET_LOOKALIKE: char = '\u{2588}'; // a full block, a plausible hand-drawn caret
    let row = format!("look{CARET_LOOKALIKE}here");
    let split_before_lookalike = caret_split(&row, 4);
    assert_eq!(split_before_lookalike, ("look", "\u{2588}here"));
    let split_after_lookalike = caret_split(&row, 5);
    assert_eq!(split_after_lookalike, ("look\u{2588}", "here"));
    // Zero characters added or removed, at either split: the caret is
    // contributed by an element the row string never contains.
    for (before, after) in [split_before_lookalike, split_after_lookalike] {
        assert_eq!(format!("{before}{after}"), row);
    }
    assert!(
        row.contains(CARET_LOOKALIKE),
        "the file's own glyph is preserved raw, exactly like body_text's old bidi property"
    );
}

/// D2's other half: the caret's own row-position function agrees with the
/// document's real cursor and no other value -- **required, ablated**: R2 says
/// a position derived independently for drawing is the RFC-053 D3 defect in a
/// new place. A byte-indexed derivation (a plausible mistake: `TextCursor` is
/// char-indexed everywhere else in this module) disagrees with the real,
/// char-indexed cursor on any row holding a multi-byte character before it.
#[test]
fn the_caret_position_is_the_real_cursor_and_nothing_independently_derived() {
    let viewport = tekstide_core::content::TextViewport {
        first_visible_line: 0,
        first_visible_column: 0,
    };
    // "h\u{e9}llo": the second character is two bytes, so column 3 (after it;
    // chars are h, that character, l) is byte index 4, not char index 3 -- a
    // byte-based derivation would land one character short of a char-based one
    // on the very next character, "l".
    let row = "h\u{e9}llo";
    assert_eq!(
        caret_row_position(cursor(0, 3), viewport, 10, 10),
        Some((0, 3)),
        "column 3, in characters, whatever the byte offset of the accented letter is"
    );
    let (before, after) = caret_split(row, 3);
    assert_eq!(
        (before, after),
        ("h\u{e9}l", "lo"),
        "the split is at the same char-indexed 3, agreeing with the position above"
    );
}

/// **D2, held where `view` actually reads it**: the row the caret belongs to
/// gets `RowPlan::WithCaret`, split at the real column, contributing no
/// character; every other row -- and every row when the cursor is off this
/// window -- gets `RowPlan::Plain`, the row's text completely unmodified.
/// `view` only ever builds an element from what this function returns, so an
/// ablation that puts a literal character into the plan (a plausible mistake:
/// "just append a `|`") fails here rather than only being visible in a
/// screenshot.
#[test]
fn row_plan_puts_the_caret_only_on_its_own_row_and_never_as_a_character() {
    let line = "hello world";
    assert_eq!(
        row_plan(line, Some((2, 5)), 0),
        RowPlan::Plain(line),
        "a different row"
    );
    assert_eq!(
        row_plan(line, None, 0),
        RowPlan::Plain(line),
        "the cursor is off this window"
    );
    assert_eq!(
        row_plan(line, Some((3, 5)), 3),
        RowPlan::WithCaret {
            before: "hello",
            after: " world"
        }
    );
    // Every character of the row is in exactly one half, and neither half
    // gained a character the row did not have.
    if let RowPlan::WithCaret { before, after } = row_plan(line, Some((0, 5)), 0) {
        assert_eq!(format!("{before}{after}"), line);
    } else {
        panic!("expected a caret at row 0");
    }
}

/// A caret outside the drawn window (a transient state; `viewport_following`
/// always corrects it before the next frame) is `None`, never a guess.
#[test]
fn a_caret_outside_the_drawn_window_is_none_not_a_wrong_position() {
    let viewport = tekstide_core::content::TextViewport {
        first_visible_line: 5,
        first_visible_column: 5,
    };
    assert_eq!(
        caret_row_position(cursor(0, 5), viewport, 10, 10),
        None,
        "above the window"
    );
    assert_eq!(
        caret_row_position(cursor(20, 5), viewport, 10, 10),
        None,
        "below the window"
    );
    assert_eq!(
        caret_row_position(cursor(5, 0), viewport, 10, 10),
        None,
        "left of the window"
    );
    assert_eq!(
        caret_row_position(cursor(5, 20), viewport, 10, 10),
        None,
        "right of the window"
    );
    assert_eq!(
        caret_row_position(cursor(5, 5), viewport, 10, 10),
        Some((0, 0)),
        "the window's own corner"
    );
}

/// A freshly opened document is `Clean` -- the only state reachable
/// without editing, which this read-only slice does not do. The other
/// four symbols are tested directly against [`document_state_symbol`]
/// below, since it is a pure function that does not need a real document
/// in each state to exercise.
#[test]
fn chrome_line_reports_clean_for_a_freshly_opened_document() {
    let sandbox = Sandbox::new("clean-state");
    sandbox.write_file("readme.md", "hello");
    let document = open(&sandbox, "readme.md");

    assert_eq!(document.state(), TextDocumentState::Clean);
    let line = chrome_line(&real_catalog(), &document);
    assert!(line.contains("readme.md"));
    assert!(!line.contains("unsaved"));
    assert!(!line.contains("changed on disk"));
    assert!(!line.contains("conflict"));
    assert!(!line.contains("save error"));
}

/// Every `TextDocumentState` variant maps to its own compile-time
/// symbol -- checked directly since `document_state_symbol` takes the
/// enum value itself, not a `TextDocument`, so no sandbox is needed to
/// exercise all five.
#[test]
fn every_document_state_maps_to_a_distinct_symbol() {
    let symbols = [
        document_state_symbol(TextDocumentState::Clean),
        document_state_symbol(TextDocumentState::Dirty),
        document_state_symbol(TextDocumentState::ExternalChanged),
        document_state_symbol(TextDocumentState::Conflict),
        document_state_symbol(TextDocumentState::SaveError),
    ];
    let unique: std::collections::HashSet<_> = symbols.iter().collect();
    assert_eq!(
        unique.len(),
        symbols.len(),
        "symbols must all be distinct: {symbols:?}"
    );
}

/// **The bidi-override case for chrome, tested specifically.** The
/// header shows the file's own path -- attacker-influenced, the same
/// class as an explorer node name -- and must render escaped.
#[test]
fn chrome_line_escapes_a_bidi_override_in_the_path() {
    let sandbox = Sandbox::new("bidi-path");
    sandbox.write_file("proj\u{202E}gpj.exe", "content");
    let document = open(&sandbox, "proj\u{202E}gpj.exe");

    let line = chrome_line(&real_catalog(), &document);

    assert!(
        line.contains("<U+202E>"),
        "expected the escaped marker in {line:?}"
    );
    assert!(
        !line.contains('\u{202E}'),
        "the raw override character must never reach the chrome line, got {line:?}"
    );
}

/// `TextDocumentOpenError`'s `Display` embeds the target's relative
/// path in every variant (including the 4 MiB refusal) -- escaped
/// exactly like the chrome path above.
#[test]
fn open_error_line_escapes_the_message() {
    let catalog = real_catalog();
    let line = open_error_line(
        &catalog,
        "file is too large to edit: proj\u{202E}gpj.exe is 5000000 bytes, limit is 4194304 bytes",
    )
    .expect("an OpenError message always renders a line");

    assert!(
        line.contains("<U+202E>"),
        "expected the escaped marker in {line:?}"
    );
    assert!(!line.contains('\u{202E}'));
    assert!(
        line.contains("4194304"),
        "the real policy bound must be visible, not a second one"
    );
}

/// **The 4 MiB refusal is rendered**, not silently empty, and uses the
/// real policy's own bound (not a second one this module introduces).
#[test]
fn opening_a_file_over_the_policy_bound_is_refused_and_rendered() {
    let sandbox = Sandbox::new("too-large");
    let oversized = "x".repeat(TextDocumentOpenPolicy::default().max_editable_bytes as usize + 1);
    sandbox.write_file("huge.txt", &oversized);

    let result = TextDocument::open(
        &sandbox.root_handle(),
        "huge.txt",
        TextDocumentOpenPolicy::default(),
    );
    let Err(error) = result else {
        panic!("opening an oversized file must be refused");
    };

    let catalog = real_catalog();
    let line = open_error_line(&catalog, &error.to_string()).expect("must render a line");
    assert!(line.contains("too large"));
    assert!(
        line.contains(
            &TextDocumentOpenPolicy::default()
                .max_editable_bytes
                .to_string()
        ),
        "the real policy bound must appear: {line:?}"
    );
}

/// No document open, no error -- the plain empty-state notice, not a
/// blank view.
#[test]
fn no_document_and_no_error_renders_the_empty_notice() {
    let catalog = real_catalog();
    let lines = empty_lines(&catalog, &ProjectContentStatus::Empty);
    assert_eq!(lines, vec![catalog.get("editor-empty")]);
}

/// **No `text_document_state_label` call anywhere in this module.** The
/// fourth of RFC-019's four named hardcoded-English producers, carried
/// forward from PR-019-B as this slice's obligation to discharge.
/// Checked by a source-text scan, the same shape
/// `no_hardcoded_english_label_function_is_called_in_this_module` uses
/// for the other three.
#[test]
fn no_hardcoded_english_label_function_is_called_in_this_module() {
    let source = include_str!("../editor.rs");
    assert!(
        !source.contains("text_document_state_label("),
        "text_document_state_label must not be called in surface/editor.rs -- route through \
         Catalog instead"
    );
}

fn character_key(c: &str) -> iced::keyboard::Key {
    iced::keyboard::Key::Character(c.into())
}

fn named_key(named: iced::keyboard::key::Named) -> iced::keyboard::Key {
    iced::keyboard::Key::Named(named)
}

fn cursor(line: usize, column: usize) -> TextCursor {
    TextCursor { line, column }
}

// --- RFC-006 Amendment 1: cursor-aware editing, replacing PR-019-D's
// original append-only model now that `ProjectContentWorkspace` has a
// real cursor-write path. ---

/// A typed character inserts exactly at the cursor, not at the end --
/// the property the append-only model could not have (there was nowhere
/// to insert but the end). `"he|ld"` with the cursor between `e` and `l`
/// becomes `"he!ld"`, cursor advancing past what was typed.
#[test]
fn a_typed_character_inserts_at_the_cursor() {
    let result = apply_edit_key("held", cursor(0, 2), &character_key("!"));
    assert_eq!(
        result,
        Some(super::EditResult {
            text: "he!ld".to_string(),
            cursor: cursor(0, 3),
        })
    );
}

/// Enter splits the current line at the cursor into two real lines, not
/// merely appending `\n` at the end -- this is text-area content, not
/// chrome, so the split must produce the raw characters `document.text()`
/// will actually hold.
#[test]
fn enter_splits_the_line_at_the_cursor() {
    let result = apply_edit_key(
        "hello",
        cursor(0, 2),
        &named_key(iced::keyboard::key::Named::Enter),
    );
    assert_eq!(
        result,
        Some(super::EditResult {
            text: "he\nllo".to_string(),
            cursor: cursor(1, 0),
        })
    );
}

/// Backspace removes exactly the character before the cursor, by `char`,
/// not by byte -- checked against a multi-byte character so a naive
/// `str` truncation would corrupt it instead of removing it cleanly.
#[test]
fn backspace_removes_the_character_before_the_cursor_by_char_not_by_byte() {
    let result = apply_edit_key(
        "caf\u{e9}",
        cursor(0, 4),
        &named_key(iced::keyboard::key::Named::Backspace),
    );
    assert_eq!(
        result,
        Some(super::EditResult {
            text: "caf".to_string(),
            cursor: cursor(0, 3),
        })
    );
}

/// Backspace at the start of a line (but not the start of the document)
/// joins with the previous line -- the multi-line case append-only
/// editing had no way to reach at all, since it could only ever remove
/// from the single point at the very end.
#[test]
fn backspace_at_the_start_of_a_line_joins_with_the_previous_line() {
    let result = apply_edit_key(
        "ab\ncd",
        cursor(1, 0),
        &named_key(iced::keyboard::key::Named::Backspace),
    );
    assert_eq!(
        result,
        Some(super::EditResult {
            text: "abcd".to_string(),
            cursor: cursor(0, 2),
        })
    );
}

/// Backspace at the very start of the document is a no-op, not a panic
/// and not an unchanged-text `Some` -- `None` means "this key produced
/// no edit," and there is nothing before the cursor to remove or join.
#[test]
fn backspace_at_the_very_start_of_the_document_is_a_no_op() {
    let result = apply_edit_key(
        "x",
        cursor(0, 0),
        &named_key(iced::keyboard::key::Named::Backspace),
    );
    assert_eq!(result, None);
}

/// An arrow key produces no *edit* at all through `apply_edit_key` --
/// `None`, not an unchanged-text `Some`. Arrow keys are
/// [`navigate_cursor`]'s job instead, checked in its own tests below.
#[test]
fn an_arrow_key_produces_no_edit() {
    let result = apply_edit_key(
        "hello",
        cursor(0, 2),
        &named_key(iced::keyboard::key::Named::ArrowLeft),
    );
    assert_eq!(result, None);
}

/// A multi-byte character typed as one keystroke (as a real IME or
/// non-ASCII layout would deliver it) inserts whole, not split into
/// invalid partial bytes.
#[test]
fn a_multi_byte_typed_character_inserts_whole() {
    let result = apply_edit_key("caf", cursor(0, 3), &character_key("\u{e9}"));
    assert_eq!(
        result,
        Some(super::EditResult {
            text: "caf\u{e9}".to_string(),
            cursor: cursor(0, 4),
        })
    );
}

// --- `navigate_cursor`: cursor movement independent of any text edit ---

#[test]
fn arrow_left_moves_back_one_column() {
    let result = navigate_cursor(
        "hello",
        cursor(0, 2),
        &named_key(iced::keyboard::key::Named::ArrowLeft),
    );
    assert_eq!(result, Some(cursor(0, 1)));
}

#[test]
fn arrow_left_at_the_start_of_a_line_moves_to_the_end_of_the_previous_line() {
    let result = navigate_cursor(
        "ab\ncd",
        cursor(1, 0),
        &named_key(iced::keyboard::key::Named::ArrowLeft),
    );
    assert_eq!(result, Some(cursor(0, 2)));
}

#[test]
fn arrow_left_at_the_very_start_is_a_no_op() {
    let result = navigate_cursor(
        "hello",
        cursor(0, 0),
        &named_key(iced::keyboard::key::Named::ArrowLeft),
    );
    assert_eq!(result, None);
}

#[test]
fn arrow_right_moves_forward_one_column() {
    let result = navigate_cursor(
        "hello",
        cursor(0, 2),
        &named_key(iced::keyboard::key::Named::ArrowRight),
    );
    assert_eq!(result, Some(cursor(0, 3)));
}

#[test]
fn arrow_right_at_the_end_of_a_line_moves_to_the_start_of_the_next_line() {
    let result = navigate_cursor(
        "ab\ncd",
        cursor(0, 2),
        &named_key(iced::keyboard::key::Named::ArrowRight),
    );
    assert_eq!(result, Some(cursor(1, 0)));
}

#[test]
fn arrow_right_at_the_very_end_is_a_no_op() {
    let result = navigate_cursor(
        "hello",
        cursor(0, 5),
        &named_key(iced::keyboard::key::Named::ArrowRight),
    );
    assert_eq!(result, None);
}

/// Moving onto a shorter line clamps to its end rather than preserving
/// an out-of-range column -- the standard plain-text-editor convention,
/// checked specifically since a naive carry-the-column implementation
/// would produce a column past the shorter line's real length.
#[test]
fn arrow_up_clamps_the_column_to_a_shorter_previous_line() {
    let result = navigate_cursor(
        "ab\nlonger line",
        cursor(1, 8),
        &named_key(iced::keyboard::key::Named::ArrowUp),
    );
    assert_eq!(result, Some(cursor(0, 2)));
}

#[test]
fn arrow_up_on_the_first_line_is_a_no_op() {
    let result = navigate_cursor(
        "only line",
        cursor(0, 3),
        &named_key(iced::keyboard::key::Named::ArrowUp),
    );
    assert_eq!(result, None);
}

#[test]
fn arrow_down_clamps_the_column_to_a_shorter_next_line() {
    let result = navigate_cursor(
        "longer line\nab",
        cursor(0, 8),
        &named_key(iced::keyboard::key::Named::ArrowDown),
    );
    assert_eq!(result, Some(cursor(1, 2)));
}

#[test]
fn arrow_down_on_the_last_line_is_a_no_op() {
    let result = navigate_cursor(
        "only line",
        cursor(0, 3),
        &named_key(iced::keyboard::key::Named::ArrowDown),
    );
    assert_eq!(result, None);
}

/// A character key (an edit key) produces no *navigation* at all through
/// `navigate_cursor` -- the two functions' `Named` arms do not overlap,
/// checked directly rather than only inferred from `apply_edit_key`'s
/// own `None` case above.
#[test]
fn an_edit_key_produces_no_navigation() {
    let result = navigate_cursor("hello", cursor(0, 2), &character_key("!"));
    assert_eq!(result, None);
}

// --- `cursor_line`: the rendered indicator response 182 required ---

/// The rendered position is real and 1-indexed (the editor convention),
/// not `TextCursor`'s own 0-indexed value passed straight through --
/// `line: 1, column: 3` (0-indexed) must render as line 2, column 4.
#[test]
fn cursor_line_renders_the_real_one_indexed_position() {
    let sandbox = Sandbox::new("cursor-render");
    sandbox.write_file("readme.md", "hello\nworld");
    let mut document = open(&sandbox, "readme.md");
    document.set_cursor(cursor(1, 3));

    let line = cursor_line(&real_catalog(), &document);

    // Fluent wraps numeric placeables in bidi isolate marks by design
    // (the same reason `paste-confirm-dialog-body`'s own line-count
    // assertion checks `.contains` rather than exact equality) -- `2`
    // and `4` are the real 1-indexed values, isolate marks aside.
    assert!(
        line.contains('2'),
        "expected the 1-indexed line in {line:?}"
    );
    assert!(
        line.contains('4'),
        "expected the 1-indexed column in {line:?}"
    );
    assert!(
        !line.contains('1'),
        "must not still show the 0-indexed line/column: {line:?}"
    );
}

/// A freshly opened document's cursor is `(0, 0)` -- rendered as line 1,
/// column 1, not line 0 or column 0.
#[test]
fn cursor_line_renders_line_one_column_one_for_a_freshly_opened_document() {
    let sandbox = Sandbox::new("cursor-render-fresh");
    sandbox.write_file("readme.md", "hello");
    let document = open(&sandbox, "readme.md");

    let line = cursor_line(&real_catalog(), &document);

    assert!(line.contains("Line"));
    assert!(line.contains("Column"));
    assert!(line.contains('1'));
    assert!(
        !line.contains('0'),
        "must render 1-indexed, not 0-indexed: {line:?}"
    );
}
