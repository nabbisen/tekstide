use super::document::TextCursor;

/// RFC-057 D3: how many edits [`super::document::TextDocument`] remembers.
/// Operations, not snapshots, so even a full stack is cheap -- but a
/// runaway session still needs a stated ceiling rather than growing
/// forever. When it is reached, the oldest recorded edit is dropped and
/// [`super::document::TextDocument::undo_depth_bound_reached`] latches
/// true, so the product can say so rather than silently forgetting it.
pub const UNDO_MAX_DEPTH: usize = 500;

/// One of the four invertible edits RFC-057 D3 names -- a character or
/// `Space` insertion, `Enter`, or one of `Backspace`'s two cases (removing
/// a character, or joining with the line before it). Each variant stores
/// exactly what its own inverse needs, never a copy of the document.
///
/// `at` is always the cursor **before** the operation was applied, on
/// every variant -- and every operation's own inverse restores the cursor
/// to exactly `at`, the property `crate::surface::editor::apply_undo`
/// (the app crate) relies on rather than computing a cursor separately.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditOperation {
    /// A character or `Space` inserted at `at`.
    Insert { at: TextCursor, inserted: String },
    /// `Enter`: the line at `at.line` was split at `at.column`.
    Enter { at: TextCursor },
    /// `Backspace` with `at.column > 0`: removed the character just
    /// before `at`.
    RemoveChar { at: TextCursor, removed: char },
    /// `Backspace` with `at.column == 0` and `at.line > 0`: joined the
    /// line at `at.line` with the one before it. `previous_len` is that
    /// previous line's own length before the join -- the split point its
    /// own inverse needs.
    JoinLines { at: TextCursor, previous_len: usize },
}
