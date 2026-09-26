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
    apply_edit_key, chrome_line, cursor_line, document_state_symbol, empty_lines, navigate_cursor,
    open_error_line, rows_that_fit, viewport_following, window_rows,
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
    };
    let follow = |line, first, capacity| {
        viewport_following(FIFTY_LINES, cursor(line, 0), viewport(first), capacity)
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
    assert_eq!(
        readers.len(),
        1,
        "the document's text is read in one place: {readers:?}"
    );
    // rustfmt may put the argument on its own line; the call it belongs to is the
    // nearest line above that opens `window_rows(`.
    assert!(
        lines[readers[0].saturating_sub(2)..=readers[0]]
            .iter()
            .any(|line| line.contains("window_rows(")),
        "and it goes straight into `window_rows`: {:?}",
        lines[readers[0]]
    );
    assert!(
        !production.contains("fn body_text"),
        "the whole-body function is gone"
    );
    assert!(
        !production.lines().any(|line| !line.trim_start().starts_with("//") && line.contains("text().to_string()")),
        "the whole document is never copied into a string for drawing"
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
