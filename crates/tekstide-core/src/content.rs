mod document;
mod edit;
mod open;
mod save;
mod snapshot;
mod undo;

pub use document::{
    ExternalChangeDecision, RecoveredBufferInit, TextCursor, TextDocument,
    TextDocumentRefreshError, TextDocumentState, TextViewport,
};
pub use edit::TextDocumentEditError;
pub use open::{DEFAULT_MAX_EDITABLE_BYTES, TextDocumentOpenError, TextDocumentOpenPolicy};
pub use save::{SaveDecision, TextDocumentSaveError};
pub use snapshot::{FileSnapshot, TextDocumentSnapshotError};
pub use undo::{EditOperation, UNDO_MAX_DEPTH};

#[cfg(test)]
mod tests;
