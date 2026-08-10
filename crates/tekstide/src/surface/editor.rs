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

use tekstide_core::content::{TextDocument, TextDocumentState};
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

/// RFC-019 PR-019-D: turns a keypress into the document's next full text,
/// or `None` if the key is not an edit key -- the shape `replace_active_text`
/// needs, since it takes the whole new text rather than a cursor-relative
/// splice.
///
/// **Append-only, and disclosed as such rather than worked around.**
/// `ProjectContentWorkspace` exposes `active_document() -> Option<&TextDocument>`
/// only -- no mutable accessor, and no way to reach `TextDocument::set_cursor`
/// from this crate. Per this RFC's own instruction ("if core's edit
/// surface turns out insufficient for real editing, stop and raise it as
/// an RFC-006 question. Do not work around it in the shell"), this does
/// not invent shell-local cursor state to fake cursor-aware insertion --
/// every typed character is appended to the end of the document, Enter
/// appends a newline, Backspace removes the last character. `document.cursor()`
/// still reads a real value (always `(0, 0)`, since nothing here ever
/// calls `set_cursor`); rendering that position and wiring real cursor
/// movement needs the mutable path this slice found missing, raised in
/// this slice's own review request rather than guessed at here.
pub(crate) fn apply_edit_key(text: &str, key: &iced::keyboard::Key) -> Option<String> {
    match key {
        iced::keyboard::Key::Character(typed) => {
            let mut new_text = text.to_string();
            new_text.push_str(typed);
            Some(new_text)
        }
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) => {
            let mut new_text = text.to_string();
            new_text.push('\n');
            Some(new_text)
        }
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Space) => {
            let mut new_text = text.to_string();
            new_text.push(' ');
            Some(new_text)
        }
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace) => {
            if text.is_empty() {
                None
            } else {
                let mut new_text = text.to_string();
                new_text.pop();
                Some(new_text)
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
