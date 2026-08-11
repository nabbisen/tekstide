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

use iced::widget::{column, container, text};
use iced::{Element, Length};

use tekstide_core::content::{TextCursor, TextDocument, TextDocumentState};
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
pub(crate) fn chrome_line(catalog: &Catalog, document: &TextDocument) -> String {
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
            .trusted_symbol("state", document_state_symbol(document.state())),
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

/// The text area's own content: raw, unescaped, exactly what the file
/// contains. **This is the one function in this crate that must never
/// call `text_safety::quote_untrusted`** -- RFC-016's editor exception,
/// quoted at the top of this module. Factored out so the raw-rendering
/// property is directly testable and ablatable in the *opposite*
/// direction from every other surface in this crate: a test asserting
/// the escaped form appears here must fail.
pub(crate) fn body_text(document: &TextDocument) -> String {
    document.text().to_string()
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
    let mut lines: Vec<String> = lines_of(text).into_iter().map(str::to_owned).collect();
    if column > 0 {
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
        let previous_len = lines[line - 1].chars().count();
        let current = lines.remove(line);
        lines[line - 1].push_str(&current);
        Some(EditResult {
            text: lines.join("\n"),
            cursor: TextCursor {
                line: line - 1,
                column: previous_len,
            },
        })
    } else {
        None
    }
}

/// RFC-006 Amendment 1: turns a keypress into a real edit at the
/// document's own cursor position -- inserting, splitting, or removing
/// exactly where the rendered [`cursor_line`] says it will, replacing
/// PR-019-D's original append-only behaviour (kept append-only only
/// because `ProjectContentWorkspace` had no cursor-write path at all;
/// that gap is closed, so this now inserts and deletes at the real
/// position rather than always at the end). `None` if the key is not an
/// edit key, or produces no edit (Backspace at the very start).
pub(crate) fn apply_edit_key(
    text: &str,
    cursor: TextCursor,
    key: &iced::keyboard::Key,
) -> Option<EditResult> {
    match key {
        iced::keyboard::Key::Character(typed) => Some(insert_at(text, cursor, typed)),
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) => {
            Some(split_line_at_cursor(text, cursor))
        }
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Space) => {
            Some(insert_at(text, cursor, " "))
        }
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace) => {
            backspace_at_cursor(text, cursor)
        }
        _ => None,
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

/// No `Message` interest of its own, the same shape `board::view` and
/// `explorer::view` use -- this function only ever reads state.
pub fn view<'a, Message: 'a>(
    document: Option<&TextDocument>,
    status: &ProjectContentStatus,
    catalog: &'a Catalog,
    theme: &'a Theme,
) -> Element<'a, Message> {
    let content: Element<'a, Message> = match document {
        Some(document) => {
            let body = text(body_text(document)).size(theme.font_size_body());
            column![
                text(chrome_line(catalog, document)).size(theme.font_size_body()),
                text(cursor_line(catalog, document)).size(theme.font_size_status()),
                body,
            ]
            .spacing(6)
            .into()
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
