//! RFC-019 PR-019-C: the editor, read-only.
//!
//! Renders `tekstide_core::content::TextDocument` -- the other half of
//! RFC-019's escaping asymmetry, and the half that breaks a user's file
//! if it is implemented wrong. **The text area renders raw.** RFC-016
//! §Text safety by surface decided this already: the user is editing
//! real file content, and an editor that escapes what it shows is not a
//! safer editor, it is a broken one. Bidi reordering is the substrate's
//! job (`cosmic-text`/`unicode-bidi`, verified in RFC-014 C10), not this
//! module's.
//!
//! Everything *around* the text area is chrome, and escapes exactly like
//! the explorer tree does -- the file path shown in the header is
//! attacker-influenced in the same way a node name is.
//!
//! `text_document_state_label` (RFC-019's fourth named hardcoded-English
//! producer) is not called anywhere here; `TextDocumentState` renders
//! through `Catalog` via `editor-chrome`'s `$state` selector instead.

use crate::theme::text;
use iced::widget::{column, container};
use iced::{Element, Length, Size};

use tekstide_core::content::{
    EditOperation, TextCursor, TextDocument, TextDocumentState, TextViewport,
};
use tekstide_core::project::ProjectContentStatus;
use tekstide_core::text_safety;

use crate::i18n::{Catalog, CatalogArgs};
use crate::theme::Theme;

fn document_state_symbol(state: TextDocumentState) -> &'static str {
    match state {
        TextDocumentState::Clean => "clean",
        TextDocumentState::Dirty => "dirty",
        TextDocumentState::ExternalChanged => "external-changed",
        TextDocumentState::Conflict => "conflict",
        TextDocumentState::SaveError => "save-error",
    }
}

/// The chrome header for an open document: its path (untrusted, escaped)
/// and its state (a compile-time symbol, never
/// `text_document_state_label`'s own English word). Factored out from
/// [`view`] for the same testability reason `surface::explorer::node_line`
/// is -- directly testable without `iced`.
pub(crate) fn chrome_line(
    catalog: &Catalog,
    document: &TextDocument,
    status: &ProjectContentStatus,
) -> String {
    // RFC-026 D8: a file that is gone is named as gone, not as changed. The document's own state
    // is `ExternalChanged` for both, so the workspace status is what tells them apart.
    let state = if matches!(status, ProjectContentStatus::ExternalDeleted) {
        "external-deleted"
    } else {
        document_state_symbol(document.state())
    };
    let path = text_safety::quote_untrusted(
        &document
            .target()
            .selected_relative_path
            .display()
            .to_string(),
    );
    catalog.get_with_args(
        "editor-chrome",
        &CatalogArgs::new()
            .untrusted("path", &path)
            .trusted_symbol("state", state),
    )
}

/// RFC-006 Amendment 1 / RFC-019 PR-019-D: the cursor's own position,
/// 1-indexed for display (the editor convention -- `TextCursor` itself
/// is 0-indexed, the same as every other zero-based offset in this
/// crate). Trusted, compile-time-shaped output only (two numbers) --
/// nothing here is attacker-influenced, so no escaping applies, unlike
/// [`chrome_line`]'s path. Factored out for the same testability reason.
pub(crate) fn cursor_line(catalog: &Catalog, document: &TextDocument) -> String {
    let cursor = document.cursor();
    catalog.get_with_args(
        "editor-cursor",
        &CatalogArgs::new()
            .number("line", (cursor.line + 1) as u32)
            .number("column", (cursor.column + 1) as u32),
    )
}

/// RFC-057 D3: the undo depth bound is **stated when reached**, not
/// silently forgotten -- this is that statement. `None` while the bound
/// has not been hit, the same "no line to show" shape [`open_error_line`]
/// already uses for its own conditional line. Factored out from [`view`]
/// for the same testability reason as [`chrome_line`].
pub(crate) fn history_bound_line(catalog: &Catalog, document: &TextDocument) -> Option<String> {
    document.undo_depth_bound_reached().then(|| {
        catalog.get_with_args(
            "editor-history-bound-reached",
            &CatalogArgs::new().number("limit", tekstide_core::content::UNDO_MAX_DEPTH as u32),
        )
    })
}

/// `TextDocumentOpenError`'s own `Display` embeds the target's relative
/// path in every variant (including the 4 MiB `TooLarge` refusal this
/// renders) -- the same attacker-influenced class as a node name.
/// Escaped exactly like [`chrome_line`]'s path before it reaches the
/// catalog.
fn open_error_line(catalog: &Catalog, message: &str) -> Option<String> {
    let escaped = text_safety::quote_untrusted(message);
    Some(catalog.get_with_args(
        "editor-open-error",
        &CatalogArgs::new().untrusted("message", &escaped),
    ))
}

/// Every line the editor renders when nothing is open: the status line
/// if the last attempt failed (including the 4 MiB refusal), or the
/// empty-state notice otherwise. Factored out from [`view`] for the same
/// testability reason as [`chrome_line`].
pub(crate) fn empty_lines(catalog: &Catalog, status: &ProjectContentStatus) -> Vec<String> {
    match status {
        ProjectContentStatus::OpenError { message } => {
            open_error_line(catalog, message).into_iter().collect()
        }
        _ => vec![catalog.get("editor-empty")],
    }
}

/// RFC-065 D4: the open set's own bound, stated when reached -- the same "stated when
/// reached" shape [`history_bound_line`] already uses for a different bound, but reachable
/// with a document **active**, unlike every other `OpenError` (which [`empty_lines`] above
/// covers): the set is full of *existing* documents when a new path is refused, so this
/// cannot wait for the "no document" branch. `None` for every other status, the same
/// "no line to show" shape [`history_bound_line`] uses. Factored out of [`view`] for the
/// same testability reason as every other line function here.
pub(crate) fn open_error_line_while_active(
    catalog: &Catalog,
    status: &ProjectContentStatus,
) -> Option<String> {
    let ProjectContentStatus::OpenError { message } = status else {
        return None;
    };
    open_error_line(catalog, message)
}

/// RFC-026, release 0.29.0: whether the chrome's Reload button is shown -- a user-driven
/// reload, reachable without first trying to save and having it refused. Factored out of
/// [`view`] for the same testability reason as [`empty_lines`]. `true` only when there is
/// something real to reload from: `ExternalChanged` or `Conflict`. Never `ExternalDeleted`,
/// where there is no disk content behind the path to reload, and never an ordinary
/// `Clean`/`Dirty` document, where a reload would discard local work for no external reason.
pub(crate) fn reload_button_is_shown(status: &ProjectContentStatus) -> bool {
    matches!(
        status,
        ProjectContentStatus::ExternalChanged | ProjectContentStatus::Conflict
    )
}

// ---- RFC-057 PR-057-B: the body is rows, and the viewport bounds them ------------
//
// **The editor never hands the whole file to one widget.** Measured in PR-057-A:
// laid out with no height bound, a 100 000-line body costs **~745 ms a
// keystroke** (`editor_baseline`'s labelled reference), against a 16 ms budget.
// What is drawn is the viewport's window: one fixed-height row per visible line,
// each row one string, and **`document.text()` is read in exactly two places in
// this module -- [`window_rows`], which builds the window, and `line_count`,
// which only counts newlines for the gutter's width (PR-057-C) -- and never
// copied whole into a string for a widget** -- a test scans this file for it.

/// iced's default text line height, as a multiple of the font size. Rows are
/// fixed-height containers of exactly this pitch, so how many fit is arithmetic
/// on a **measured** height, never an estimate of wrapping.
const LINE_HEIGHT_FACTOR: f32 = 1.3;
/// What the window holds before the layout has been measured.
pub(crate) const DEFAULT_WINDOW_ROWS: usize = 30;

/// The height of one drawn row at `font_size`.
pub(crate) fn row_pitch(font_size: f32) -> f32 {
    font_size * LINE_HEIGHT_FACTOR
}

/// How many rows fit a body region of `height` pixels at `font_size`.
pub(crate) fn rows_that_fit(height: Option<f32>, font_size: f32) -> usize {
    let Some(height) = height else {
        return DEFAULT_WINDOW_ROWS;
    };
    ((height.max(0.0) / row_pitch(font_size)).floor() as usize).max(1)
}

/// How many lines the text has, counted the way `TextCursor.line` indexes them:
/// `"a\nb\n"` is three (the trailing empty line included).
pub(crate) fn line_count(text: &str) -> usize {
    text.bytes().filter(|byte| *byte == b'\n').count() + 1
}

/// **The viewport follows the cursor** (D9): the first visible line moves only
/// when the cursor would otherwise be off screen, and as little as it must --
/// moving the cursor inside the window does not scroll it. Nothing here scrolls
/// for its own sake: there is no wheel and no scrollbar. RFC-057 PR-057-C, Q2:
/// the horizontal window follows the same rule, on the cursor's own line --
/// **one global horizontal position, applied to every drawn row**, the same way
/// `first_visible_line` is one value for the whole view rather than one per
/// line. The line length used for the horizontal window is read once, by
/// walking to the cursor's own line -- an `O(file)` scan, the same order of
/// cost `navigate_cursor` and `apply_edit_key` already pay per keystroke
/// (review 441/442: the per-keystroke copy is out of this release; this does
/// not enlarge it, it is the same class of cost).
pub(crate) fn viewport_following(
    text: &str,
    cursor: TextCursor,
    viewport: TextViewport,
    row_capacity: usize,
    column_capacity: usize,
) -> TextViewport {
    let vertical = super::explorer::window_for(
        line_count(text),
        cursor.line,
        viewport.first_visible_line,
        row_capacity,
    );
    // The cursor's own line's length, in characters, plus one: a cursor may
    // sit one past the last character (the append position), which is a real,
    // reachable column and must count as a position `window_for` can hold.
    let cursor_line_len = text
        .split('\n')
        .nth(cursor.line)
        .map_or(0, |line| line.chars().count());
    let horizontal = super::explorer::window_for(
        cursor_line_len + 1,
        cursor.column,
        viewport.first_visible_column,
        column_capacity,
    );
    TextViewport {
        first_visible_line: vertical.top,
        first_visible_column: horizontal.top,
    }
}

/// **The rows drawn**: at most `capacity` lines of `text`, from
/// `first_visible_line`, each **raw and unescaped** -- exactly what the file
/// contains. This is the one function in this crate that must never call
/// `text_safety::quote_untrusted` (RFC-016's editor exception), and **the one
/// place the document's text is read for drawing**. One row is one string, so
/// what the surface draws is assertable without `iced` (D8). **Not yet
/// clipped to the horizontal window** -- [`windowed_line`] does that, kept
/// separate so this function's own contract (and PR-057-B's tests) are
/// unchanged by PR-057-C.
pub(crate) fn window_rows(text: &str, first_visible_line: usize, capacity: usize) -> Vec<&str> {
    text.split('\n')
        .skip(first_visible_line)
        .take(capacity)
        .collect()
}

/// The rows the editor draws for `document` at `capacity` rows: its viewport's
/// window. **What `view` builds and what the tests measure are this one value**,
/// so a test of "what is drawn" cannot describe a different window from the one
/// on screen.
pub(crate) fn drawn_rows(document: &TextDocument, capacity: usize) -> Vec<&str> {
    window_rows(
        document.text(),
        document.viewport().first_visible_line,
        capacity,
    )
}

// ---- RFC-057 PR-057-C: the gutter, the horizontal window, and the caret ----------

/// How wide the font's average character is taken to be, as a fraction of the
/// font size. `ui_font` is configurable and may be proportional (unlike the
/// tree's and the terminal's fixed monospace face), so this is a stated
/// approximation, the same shape [`LINE_HEIGHT_FACTOR`] already is for row
/// height -- **measured, then disclosed as approximate**, never claimed exact.
const CHAR_WIDTH_FACTOR: f32 = 0.6;
/// Columns of separation reserved between the gutter and the text.
const GUTTER_SEPARATOR_COLUMNS: usize = 1;
/// What the horizontal window holds before the layout has been measured.
pub(crate) const DEFAULT_WINDOW_COLUMNS: usize = 80;

/// The width of one character at `font_size`, by the same approximation
/// [`row_pitch`] uses for height.
pub(crate) fn column_pitch(font_size: f32) -> f32 {
    font_size * CHAR_WIDTH_FACTOR
}

/// How many gutter digits a file of `total_lines` needs: the digit count of
/// the last line number, at least one. **Sized to the whole file, not to what
/// is on screen** -- this is what keeps the text column from shifting as a
/// file crosses a power of ten while scrolling (RFC-057 acceptance criterion).
pub(crate) fn gutter_digits(total_lines: usize) -> usize {
    total_lines.max(1).to_string().len()
}

/// How many columns of body text fit a region of `width` pixels at
/// `font_size`, after reserving `digits` for the gutter and its separator.
pub(crate) fn columns_that_fit(width: Option<f32>, font_size: f32, digits: usize) -> usize {
    let Some(width) = width else {
        return DEFAULT_WINDOW_COLUMNS;
    };
    let total = (width.max(0.0) / column_pitch(font_size)).floor() as usize;
    total
        .saturating_sub(digits + GUTTER_SEPARATOR_COLUMNS)
        .max(1)
}

/// The gutter's own drawn lines: `drawn_row_count` numbers starting at
/// `first_visible_line + 1` (1-indexed, the editor convention -- see
/// [`cursor_line`]), each right-aligned to `digits` wide. Real line indices,
/// never the row's position in the window: row 0 of a window that starts at
/// line 9,997 reads *9998*, not *1*.
pub(crate) fn gutter_lines(
    first_visible_line: usize,
    drawn_row_count: usize,
    digits: usize,
) -> Vec<String> {
    (0..drawn_row_count)
        .map(|row| format!("{:>width$}", first_visible_line + row + 1, width = digits))
        .collect()
}

/// `line`, windowed to the horizontal region `[first_visible_column,
/// first_visible_column + capacity)`, by **character** index -- the same unit
/// `TextCursor.column` uses, so a caret computed in columns and a window
/// computed in columns agree. Never panics on a window past the line's end;
/// it draws nothing there.
pub(crate) fn windowed_line(line: &str, first_visible_column: usize, capacity: usize) -> &str {
    let Some((start, _)) = line.char_indices().nth(first_visible_column) else {
        return "";
    };
    let remainder = &line[start..];
    let end = remainder
        .char_indices()
        .nth(capacity)
        .map_or(remainder.len(), |(index, _)| index);
    &remainder[..end]
}

/// Where the caret lands **inside the drawn window**, in (row, column) offsets
/// from the window's own top-left -- or `None` when the document's real cursor
/// (RFC-057 R2: there is no second, independently derived position) is
/// currently outside the window that is drawn. That can only be true for one
/// frame: `viewport_following` is called after every edit, cursor move and
/// re-measure, and always brings the cursor back inside.
pub(crate) fn caret_row_position(
    cursor: TextCursor,
    viewport: TextViewport,
    row_capacity: usize,
    column_capacity: usize,
) -> Option<(usize, usize)> {
    let row = cursor.line.checked_sub(viewport.first_visible_line)?;
    if row >= row_capacity {
        return None;
    }
    let column = cursor.column.checked_sub(viewport.first_visible_column)?;
    if column >= column_capacity {
        return None;
    }
    Some((row, column))
}

/// Splits a **windowed** row at `column` characters in, for the caret to sit
/// between the two halves. Character-indexed, like [`windowed_line`] and like
/// every other column arithmetic in this module (`clamp_cursor`, `insert_at`)
/// -- **the property review 442's required ablation holds**: a byte-indexed
/// split disagrees with a char-indexed one on any row containing a multi-byte
/// character before the caret, and a test with one catches it.
pub(crate) fn caret_split(row: &str, column: usize) -> (&str, &str) {
    let byte = row
        .char_indices()
        .nth(column)
        .map_or(row.len(), |(index, _)| index);
    (&row[..byte], &row[byte..])
}

/// What one drawn row's content should be: plain text, or text with the caret
/// spliced in as a **position to build an element at**, never as a character.
/// A pure decision, factored out of `view` so it is directly testable without
/// `iced` (D8) -- and so the requirement it holds (D2: the caret is an
/// element) has a test that can actually fail if `view` stopped calling it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RowPlan<'a> {
    Plain(&'a str),
    WithCaret { before: &'a str, after: &'a str },
}

/// `windowed` is one row's already horizontally-windowed text (D2/D9); `at`
/// is [`caret_row_position`]'s answer; `row_index` is this row's own position
/// in the drawn window. The caret is planted only on the one row it belongs
/// to -- every other row, including every row when the cursor is off window,
/// is plain.
pub(crate) fn row_plan(
    windowed: &str,
    at: Option<(usize, usize)>,
    row_index: usize,
) -> RowPlan<'_> {
    match at {
        Some((caret_row, caret_column)) if caret_row == row_index => {
            let (before, after) = caret_split(windowed, caret_column);
            RowPlan::WithCaret { before, after }
        }
        _ => RowPlan::Plain(windowed),
    }
}

/// The caret: an **element**, never a character spliced into the text (D2,
/// `what-the-editor-must-not-do.md` §1). A fixed-width, fixed-height coloured
/// container -- it contributes no character to any string this module
/// produces, so it cannot be confused with real file content, however that
/// content is inspected.
fn caret_element<'a, Message: 'a>(theme: Theme, height: f32) -> Element<'a, Message> {
    const CARET_WIDTH: f32 = 2.0;
    container(iced::widget::Space::new())
        .width(Length::Fixed(CARET_WIDTH))
        .height(Length::Fixed(height))
        .style(
            move |_base_theme: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(theme.accent())),
                ..iced::widget::container::Style::default()
            },
        )
        .into()
}

/// The result of a real edit: both halves `replace_active_text` and
/// `set_active_cursor` need, computed together so the cursor always
/// lands exactly where the edit left it -- never recomputed separately
/// from a stale copy of either.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditResult {
    pub text: String,
    pub cursor: TextCursor,
}

/// Splits `text` into its lines the way `TextCursor.line` indexes them:
/// `"a\nb\n"` is three lines (`"a"`, `"b"`, `""`), the trailing empty
/// line after a final newline included on purpose -- the same
/// `split('\n')` shape `TextCursor`'s own line index has to agree with,
/// since nothing else in this crate defines what "line" means for it.
fn lines_of(text: &str) -> Vec<&str> {
    text.split('\n').collect()
}

/// Clamps a cursor to a real position inside `text` -- out-of-range
/// input (stale after an external reload, or a document shorter than
/// where the cursor last was) resolves to the nearest real line/column
/// rather than panicking or silently indexing past the end.
fn clamp_cursor(text: &str, cursor: TextCursor) -> (usize, usize) {
    let lines = lines_of(text);
    let line = cursor.line.min(lines.len().saturating_sub(1));
    let column = cursor.column.min(lines[line].chars().count());
    (line, column)
}

fn insert_at(text: &str, cursor: TextCursor, insert: &str) -> EditResult {
    let (line, column) = clamp_cursor(text, cursor);
    let mut lines: Vec<String> = lines_of(text).into_iter().map(str::to_owned).collect();
    let chars: Vec<char> = lines[line].chars().collect();
    let mut new_line: Vec<char> = chars[..column].to_vec();
    new_line.extend(insert.chars());
    let inserted_count = insert.chars().count();
    new_line.extend(&chars[column..]);
    lines[line] = new_line.into_iter().collect();
    EditResult {
        text: lines.join("\n"),
        cursor: TextCursor {
            line,
            column: column + inserted_count,
        },
    }
}

fn split_line_at_cursor(text: &str, cursor: TextCursor) -> EditResult {
    let (line, column) = clamp_cursor(text, cursor);
    let mut lines: Vec<String> = lines_of(text).into_iter().map(str::to_owned).collect();
    let chars: Vec<char> = lines[line].chars().collect();
    let before: String = chars[..column].iter().collect();
    let after: String = chars[column..].iter().collect();
    lines[line] = before;
    lines.insert(line + 1, after);
    EditResult {
        text: lines.join("\n"),
        cursor: TextCursor {
            line: line + 1,
            column: 0,
        },
    }
}

/// `None` when there is nothing to remove: the very start of the
/// document, distinct from every other case (removing a character,
/// joining the previous line) which always produces `Some`.
fn backspace_at_cursor(text: &str, cursor: TextCursor) -> Option<EditResult> {
    let (line, column) = clamp_cursor(text, cursor);
    if column > 0 {
        let mut lines: Vec<String> = lines_of(text).into_iter().map(str::to_owned).collect();
        let mut chars: Vec<char> = lines[line].chars().collect();
        chars.remove(column - 1);
        lines[line] = chars.into_iter().collect();
        Some(EditResult {
            text: lines.join("\n"),
            cursor: TextCursor {
                line,
                column: column - 1,
            },
        })
    } else if line > 0 {
        Some(join_lines_at(text, line))
    } else {
        None
    }
}

/// The inverse of [`insert_at`]: removes `count` characters starting at
/// `cursor`, landing the cursor back at `cursor` -- used only by
/// [`apply_undo`] to invert an [`EditOperation::Insert`]. Never crosses a
/// line boundary: `insert_at` is only ever called with single-line text
/// (a typed character, `Space`), so an undo of it never needs to either.
fn remove_at(text: &str, cursor: TextCursor, count: usize) -> EditResult {
    let (line, column) = clamp_cursor(text, cursor);
    let mut lines: Vec<String> = lines_of(text).into_iter().map(str::to_owned).collect();
    let mut chars: Vec<char> = lines[line].chars().collect();
    let end = (column + count).min(chars.len());
    chars.drain(column..end);
    lines[line] = chars.into_iter().collect();
    EditResult {
        text: lines.join("\n"),
        cursor: TextCursor { line, column },
    }
}

/// Joins `line` into the line before it -- the same join
/// [`backspace_at_cursor`]'s own start-of-line case performs (factored
/// out so [`apply_undo`] can reuse it to invert an
/// [`EditOperation::Enter`] without duplicating the logic), and also
/// [`split_line_at_cursor`]'s own inverse.
fn join_lines_at(text: &str, line: usize) -> EditResult {
    let mut lines: Vec<String> = lines_of(text).into_iter().map(str::to_owned).collect();
    let previous_len = lines[line - 1].chars().count();
    let current = lines.remove(line);
    lines[line - 1].push_str(&current);
    EditResult {
        text: lines.join("\n"),
        cursor: TextCursor {
            line: line - 1,
            column: previous_len,
        },
    }
}

/// [`apply_edit_key`]'s own result: the text/cursor half every caller
/// needs ([`EditResult`]) plus the [`EditOperation`] that produced it, so
/// a caller can record it for undo (RFC-057 D3) without re-deriving what
/// happened from the two text values alone -- the same "compute
/// everything together instead of recomputing one half from a stale copy
/// of the other" reasoning [`EditResult`]'s own doc already states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditOutcome {
    pub result: EditResult,
    pub operation: EditOperation,
}

/// RFC-006 Amendment 1 / RFC-057 D3: turns a keypress into a real edit at
/// the document's own cursor position -- inserting, splitting, or
/// removing exactly where the rendered [`cursor_line`] says it will,
/// replacing PR-019-D's original append-only behaviour (kept append-only
/// only because `ProjectContentWorkspace` had no cursor-write path at
/// all; that gap is closed, so this now inserts and deletes at the real
/// position rather than always at the end). `None` if the key is not an
/// edit key, or produces no edit (Backspace at the very start).
pub(crate) fn apply_edit_key(
    text: &str,
    cursor: TextCursor,
    key: &iced::keyboard::Key,
) -> Option<EditOutcome> {
    let (line, column) = clamp_cursor(text, cursor);
    let at = TextCursor { line, column };
    match key {
        iced::keyboard::Key::Character(typed) => Some(EditOutcome {
            result: insert_at(text, cursor, typed),
            operation: EditOperation::Insert {
                at,
                inserted: typed.to_string(),
            },
        }),
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) => Some(EditOutcome {
            result: split_line_at_cursor(text, cursor),
            operation: EditOperation::Enter { at },
        }),
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Space) => Some(EditOutcome {
            result: insert_at(text, cursor, " "),
            operation: EditOperation::Insert {
                at,
                inserted: " ".to_owned(),
            },
        }),
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace) => {
            let result = backspace_at_cursor(text, cursor)?;
            let operation = if column > 0 {
                let removed = lines_of(text)[line].chars().nth(column - 1).expect(
                    "backspace_at_cursor returned Some for column > 0, so this character exists",
                );
                EditOperation::RemoveChar { at, removed }
            } else {
                let previous_len = lines_of(text)[line - 1].chars().count();
                EditOperation::JoinLines { at, previous_len }
            };
            Some(EditOutcome { result, operation })
        }
        _ => None,
    }
}

/// RFC-057 D3: inverts an [`EditOperation`] against `text` -- the
/// operation's own stored fields tell each variant exactly how to undo
/// itself, never a generic diff. The resulting cursor is always
/// `operation`'s own `at`, the invariant [`EditOperation`]'s own doc
/// states every variant holds.
pub(crate) fn apply_undo(text: &str, operation: &EditOperation) -> EditResult {
    match operation {
        EditOperation::Insert { at, inserted } => remove_at(text, *at, inserted.chars().count()),
        EditOperation::Enter { at } => join_lines_at(text, at.line + 1),
        EditOperation::RemoveChar { at, removed } => {
            let mut buffer = [0u8; 4];
            insert_at(
                text,
                TextCursor {
                    line: at.line,
                    column: at.column - 1,
                },
                removed.encode_utf8(&mut buffer),
            )
        }
        EditOperation::JoinLines { at, previous_len } => split_line_at_cursor(
            text,
            TextCursor {
                line: at.line - 1,
                column: *previous_len,
            },
        ),
    }
}

/// RFC-057 D3: reapplies an [`EditOperation`] to `text` -- literally the
/// same functions [`apply_edit_key`] used to produce it in the first
/// place, called again at the operation's own `at`. No new logic: redo is
/// "do it again," not a third implementation of what the edit was.
pub(crate) fn apply_redo(text: &str, operation: &EditOperation) -> EditResult {
    match operation {
        EditOperation::Insert { at, inserted } => insert_at(text, *at, inserted),
        EditOperation::Enter { at } => split_line_at_cursor(text, *at),
        EditOperation::RemoveChar { at, .. } => backspace_at_cursor(text, *at)
            .expect("RemoveChar was recorded from a real backspace at column > 0"),
        EditOperation::JoinLines { at, .. } => backspace_at_cursor(text, *at)
            .expect("JoinLines was recorded from a real backspace at column 0, line > 0"),
    }
}

/// The cursor's own movement, independent of any text edit -- `None`
/// leaves both text and cursor untouched (not an edit key, or already at
/// a boundary the key does not cross). Up/Down clamp the target line's
/// column the way every plain-text editor does: moving onto a shorter
/// line clamps to its end rather than preserving an out-of-range column.
pub(crate) fn navigate_cursor(
    text: &str,
    cursor: TextCursor,
    key: &iced::keyboard::Key,
) -> Option<TextCursor> {
    let iced::keyboard::Key::Named(named) = key else {
        return None;
    };
    let (line, column) = clamp_cursor(text, cursor);
    let lines = lines_of(text);
    match named {
        iced::keyboard::key::Named::ArrowLeft => {
            if column > 0 {
                Some(TextCursor {
                    line,
                    column: column - 1,
                })
            } else if line > 0 {
                let previous_len = lines[line - 1].chars().count();
                Some(TextCursor {
                    line: line - 1,
                    column: previous_len,
                })
            } else {
                None
            }
        }
        iced::keyboard::key::Named::ArrowRight => {
            let line_len = lines[line].chars().count();
            if column < line_len {
                Some(TextCursor {
                    line,
                    column: column + 1,
                })
            } else if line + 1 < lines.len() {
                Some(TextCursor {
                    line: line + 1,
                    column: 0,
                })
            } else {
                None
            }
        }
        iced::keyboard::key::Named::ArrowUp => {
            if line > 0 {
                let target_len = lines[line - 1].chars().count();
                Some(TextCursor {
                    line: line - 1,
                    column: column.min(target_len),
                })
            } else {
                None
            }
        }
        iced::keyboard::key::Named::ArrowDown => {
            if line + 1 < lines.len() {
                let target_len = lines[line + 1].chars().count();
                Some(TextCursor {
                    line: line + 1,
                    column: column.min(target_len),
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

/// RFC-040 PR-040-C: `on_save` is the one real click message this view
/// now has an interest in -- the same "surface renders, `shell.rs`
/// supplies the message" split [`crate::surface::board::empty_state_view`]'s
/// own `open_browser_message` parameter already established. Only used
/// in the `Some(document)` branch (there is nothing to save with no
/// document open), but taken unconditionally rather than as an
/// `Option<Message>`: unlike `board::row_view`'s own `open_message`,
/// this surface's caller always has a real message to offer, so there
/// is no `None` case to thread through.
/// The body window's two capacities together, the same one-struct shape
/// `explorer::ExplorerCursor` already uses to keep a growing list of
/// positional arguments from crossing clippy's own line -- and from a caller
/// swapping rows and columns unnoticed, which two bare `usize`s permit and a
/// named struct does not.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EditorWindow {
    pub rows: usize,
    pub columns: usize,
}

/// The two document-level messages the chrome's own buttons dispatch, the same
/// one-struct-for-related-arguments shape [`EditorWindow`] already uses, and for the
/// identical clippy-line reason: adding `on_reload` (RFC-026, release 0.29.0) as a bare
/// positional argument beside `on_save` is what crossed it.
pub(crate) struct EditorActions<Message> {
    pub on_save: Message,
    pub on_reload: Message,
}

pub fn view<'a, Message: 'a + Clone>(
    document: Option<&TextDocument>,
    status: &ProjectContentStatus,
    catalog: &'a Catalog,
    theme: &'a Theme,
    actions: EditorActions<Message>,
    window: EditorWindow,
    on_body_measured: impl Fn(Size) -> Message + 'a,
) -> Element<'a, Message> {
    let EditorActions { on_save, on_reload } = actions;
    let EditorWindow {
        rows: row_capacity,
        columns: column_capacity,
    } = window;
    let content: Element<'a, Message> = match document {
        Some(document) => {
            let pitch = row_pitch(theme.font_size_body());
            let viewport = document.viewport();
            let cursor = document.cursor();
            let digits = gutter_digits(line_count(document.text()));
            let rows = drawn_rows(document, row_capacity);
            let caret_at = caret_row_position(cursor, viewport, row_capacity, column_capacity);
            let numbers = gutter_lines(viewport.first_visible_line, rows.len(), digits);
            let rows: Vec<Element<'a, Message>> = rows
                .into_iter()
                .zip(numbers)
                .enumerate()
                .map(|(row_index, (line, number))| {
                    // No wrapping: one line is one row, so the window's
                    // arithmetic is exact. **Q2**: the row is also windowed
                    // horizontally, by the same rule as the vertical window,
                    // so a long line's own cursor position is never off
                    // screen -- clipping with no horizontal window (B's
                    // shape) left a user typing into text they could not
                    // see (review 442).
                    let windowed =
                        windowed_line(line, viewport.first_visible_column, column_capacity);
                    // D2: the caret is composed in, as its own element -- the
                    // text on either side of it is still exactly the file's
                    // own characters, never a character standing in for the
                    // caret. `row_plan` decides *whether and where*, as a pure
                    // function this module's tests can drive without `iced`;
                    // this match only ever builds what `row_plan` said.
                    // RFC-057, the owner's font ruling: the body's own
                    // text -- both sides of the caret, the plain-row case,
                    // and the gutter number -- takes `theme.editor_font()`,
                    // not the interface's own `ui_font()` [`text`] already
                    // applies. Monospace by default, a configured family
                    // if the user set one; never `Font::DEFAULT`'s own
                    // proportional fallback, which is what review 444
                    // measured the ~51-column caret loss against.
                    let row_body: Element<'a, Message> =
                        match row_plan(windowed, caret_at, row_index) {
                            RowPlan::WithCaret { before, after } => iced::widget::row![
                                text(before.to_owned())
                                    .size(theme.font_size_body())
                                    .font(theme.editor_font())
                                    .wrapping(iced::widget::text::Wrapping::None),
                                caret_element(*theme, pitch),
                                text(after.to_owned())
                                    .size(theme.font_size_body())
                                    .font(theme.editor_font())
                                    .wrapping(iced::widget::text::Wrapping::None),
                            ]
                            .into(),
                            RowPlan::Plain(text_only) => text(text_only.to_owned())
                                .size(theme.font_size_body())
                                .font(theme.editor_font())
                                .wrapping(iced::widget::text::Wrapping::None)
                                .into(),
                        };
                    container(
                        iced::widget::row![
                            container(
                                text(number)
                                    .size(theme.font_size_body())
                                    .font(theme.editor_font())
                                    .wrapping(iced::widget::text::Wrapping::None)
                            )
                            .width(Length::Shrink),
                            row_body,
                        ]
                        .spacing(8),
                    )
                    .width(Length::Fill)
                    .height(Length::Fixed(pitch))
                    .into()
                })
                .collect();
            // The body region's size is measured, not computed: the window
            // holds as many rows and columns as the layout engine says fit.
            let body = crate::surface::frame::MeasureSize::new(
                container(column(rows))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .clip(true),
                on_body_measured,
            );
            let mut chrome: Vec<Element<'a, Message>> = vec![
                text(chrome_line(catalog, document, status))
                    .size(theme.font_size_body())
                    .into(),
                text(cursor_line(catalog, document))
                    .size(theme.font_size_status())
                    .into(),
            ];
            if let Some(line) = history_bound_line(catalog, document) {
                chrome.push(text(line).size(theme.font_size_status()).into());
            }
            if let Some(line) = open_error_line_while_active(catalog, status) {
                chrome.push(text(line).size(theme.font_size_status()).into());
            }
            chrome.push(
                crate::theme::button(
                    *theme,
                    text(catalog.get("editor-save-button")).size(theme.font_size_body()),
                )
                .on_press(on_save)
                .into(),
            );
            if reload_button_is_shown(status) {
                chrome.push(
                    crate::theme::button(
                        *theme,
                        text(catalog.get("editor-reload-button")).size(theme.font_size_body()),
                    )
                    .on_press(on_reload)
                    .into(),
                );
            }
            chrome.push(body.into());
            column(chrome).spacing(6).into()
        }
        None => {
            let lines = empty_lines(catalog, status);
            column(
                lines
                    .into_iter()
                    .map(|line| text(line).size(theme.font_size_body()).into())
                    .collect::<Vec<Element<'a, Message>>>(),
            )
            .spacing(2)
            .into()
        }
    };

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests;
