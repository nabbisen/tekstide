mod document;
mod edit;
mod open;
mod save;
mod snapshot;

pub use document::{
    ExternalChangeDecision, TextCursor, TextDocument, TextDocumentRefreshError, TextDocumentState,
    TextViewport,
};
pub use edit::TextDocumentEditError;
pub use open::{DEFAULT_MAX_EDITABLE_BYTES, TextDocumentOpenError, TextDocumentOpenPolicy};
pub use save::{SaveDecision, TextDocumentSaveError};
pub use snapshot::{FileSnapshot, TextDocumentSnapshotError};

#[cfg(test)]
mod tests;
