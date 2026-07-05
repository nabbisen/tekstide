use std::fmt;
use std::path::{Path, PathBuf};

use crate::content::{
    ExternalChangeDecision, SaveDecision, TextDocument, TextDocumentEditError,
    TextDocumentOpenError, TextDocumentOpenPolicy, TextDocumentRefreshError, TextDocumentSaveError,
    TextDocumentState,
};
use crate::project::root::{
    ExplorerDirectoryScan, ExplorerNodeKind, ExplorerNodeState, ExplorerScanError,
    FileAccessSymlinkStatus, FileExplorerScanPolicy, FileExplorerScanner, ProjectRootHandle,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectContentWorkspace {
    selected_explorer_path: PathBuf,
    explorer_scan: Option<ExplorerDirectoryScan>,
    explorer_status: ProjectExplorerStatus,
    active_document: Option<TextDocument>,
    status: ProjectContentStatus,
}

impl ProjectContentWorkspace {
    pub fn selected_explorer_path(&self) -> &Path {
        &self.selected_explorer_path
    }

    pub fn explorer_scan(&self) -> Option<&ExplorerDirectoryScan> {
        self.explorer_scan.as_ref()
    }

    pub fn explorer_status(&self) -> &ProjectExplorerStatus {
        &self.explorer_status
    }

    pub fn active_document(&self) -> Option<&TextDocument> {
        self.active_document.as_ref()
    }

    pub fn status(&self) -> &ProjectContentStatus {
        &self.status
    }

    pub fn scan_explorer_directory(
        &mut self,
        root: &ProjectRootHandle,
        selected_relative_path: impl Into<PathBuf>,
        policy: &FileExplorerScanPolicy,
    ) -> Result<(), ProjectContentError> {
        let selected_relative_path = selected_relative_path.into();

        match FileExplorerScanner.scan_directory(root, selected_relative_path, policy) {
            Ok(scan) => {
                self.selected_explorer_path = scan.directory.selected_relative_path.clone();
                self.explorer_scan = Some(scan);
                self.explorer_status = ProjectExplorerStatus::Ready;
                Ok(())
            }
            Err(error) => {
                self.explorer_scan = None;
                self.explorer_status = ProjectExplorerStatus::Error {
                    message: error.to_string(),
                };
                Err(ProjectContentError::Explorer(error))
            }
        }
    }

    pub fn open_text_document(
        &mut self,
        root: &ProjectRootHandle,
        selected_relative_path: impl AsRef<Path>,
        policy: TextDocumentOpenPolicy,
    ) -> Result<(), ProjectContentError> {
        let selected_relative_path = selected_relative_path.as_ref().to_path_buf();

        match TextDocument::open(root, &selected_relative_path, policy) {
            Ok(document) => {
                self.selected_explorer_path = selected_relative_path;
                self.active_document = Some(document);
                self.status = ProjectContentStatus::Opened;
                Ok(())
            }
            Err(error) => {
                self.status = ProjectContentStatus::OpenError {
                    message: error.to_string(),
                };
                Err(ProjectContentError::Open(error))
            }
        }
    }

    pub fn replace_active_text(
        &mut self,
        text: impl Into<String>,
    ) -> Result<(), ProjectContentError> {
        let Some(document) = self.active_document.as_mut() else {
            self.status = ProjectContentStatus::EditError {
                message: "no active text document".to_owned(),
            };
            return Err(ProjectContentError::NoActiveDocument);
        };

        match document.replace_text(text) {
            Ok(()) => {
                if document.is_dirty() {
                    self.status = ProjectContentStatus::Edited;
                } else if matches!(self.status, ProjectContentStatus::Empty) {
                    self.status = ProjectContentStatus::Opened;
                }
                Ok(())
            }
            Err(error) => {
                self.status = ProjectContentStatus::EditError {
                    message: error.to_string(),
                };
                Err(ProjectContentError::Edit(error))
            }
        }
    }

    pub fn save_active_document(
        &mut self,
        root: &ProjectRootHandle,
        policy: TextDocumentOpenPolicy,
    ) -> Result<SaveDecision, ProjectContentError> {
        let Some(document) = self.active_document.as_mut() else {
            self.status = ProjectContentStatus::SaveError {
                message: "no active text document".to_owned(),
            };
            return Err(ProjectContentError::NoActiveDocument);
        };

        match document.save(root, policy) {
            Ok(decision) => {
                self.status = ProjectContentStatus::Saved { decision };
                Ok(decision)
            }
            Err(error) => {
                self.status = match error.decision() {
                    SaveDecision::BlockedExternalChange => ProjectContentStatus::Conflict,
                    _ => ProjectContentStatus::SaveError {
                        message: error.to_string(),
                    },
                };
                Err(ProjectContentError::Save(error))
            }
        }
    }

    pub fn refresh_active_document(
        &mut self,
        root: &ProjectRootHandle,
        policy: TextDocumentOpenPolicy,
    ) -> Result<ExternalChangeDecision, ProjectContentError> {
        let Some(document) = self.active_document.as_mut() else {
            self.status = ProjectContentStatus::RefreshError {
                message: "no active text document".to_owned(),
            };
            return Err(ProjectContentError::NoActiveDocument);
        };

        match document.refresh_external_state(root, policy) {
            Ok(decision) => {
                self.status = match decision {
                    ExternalChangeDecision::Unchanged => ProjectContentStatus::Opened,
                    ExternalChangeDecision::ExternalChanged => {
                        ProjectContentStatus::ExternalChanged
                    }
                    ExternalChangeDecision::Conflict => ProjectContentStatus::Conflict,
                };
                Ok(decision)
            }
            Err(error) => {
                self.status = ProjectContentStatus::RefreshError {
                    message: error.to_string(),
                };
                Err(ProjectContentError::Refresh(error))
            }
        }
    }

    pub fn open_buffer_count(&self) -> u32 {
        u32::from(self.active_document.is_some())
    }

    pub fn dirty_file_count(&self) -> u32 {
        u32::from(
            self.active_document
                .as_ref()
                .is_some_and(TextDocument::is_dirty),
        )
    }

    pub fn active_path_hint(&self) -> Option<PathBuf> {
        self.active_document
            .as_ref()
            .map(|document| document.target().selected_relative_path.clone())
    }
}

impl Default for ProjectContentWorkspace {
    fn default() -> Self {
        Self {
            selected_explorer_path: PathBuf::new(),
            explorer_scan: None,
            explorer_status: ProjectExplorerStatus::Empty,
            active_document: None,
            status: ProjectContentStatus::Empty,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectExplorerStatus {
    Empty,
    Ready,
    Error { message: String },
}

impl ProjectExplorerStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Ready => "ready",
            Self::Error { .. } => "error",
        }
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Error { message } => Some(message),
            _ => None,
        }
    }
}

pub fn explorer_node_kind_label(kind: ExplorerNodeKind) -> &'static str {
    match kind {
        ExplorerNodeKind::File => "file",
        ExplorerNodeKind::Directory => "directory",
        ExplorerNodeKind::Other => "other",
    }
}

pub fn explorer_node_state_label(state: &ExplorerNodeState) -> &'static str {
    match state {
        ExplorerNodeState::Available => "available",
        ExplorerNodeState::Collapsed => "collapsed",
        ExplorerNodeState::Blocked(_) => "blocked",
        ExplorerNodeState::Unreadable => "unreadable",
    }
}

pub fn explorer_symlink_status_label(status: FileAccessSymlinkStatus) -> &'static str {
    match status {
        FileAccessSymlinkStatus::NoSymlink => "none",
        FileAccessSymlinkStatus::InRootSymlink => "in-root symlink",
        FileAccessSymlinkStatus::UnresolvedSymlink => "unresolved symlink",
        FileAccessSymlinkStatus::EscapesRoot => "escapes root",
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectContentStatus {
    Empty,
    Opened,
    Edited,
    Saved { decision: SaveDecision },
    ExternalChanged,
    Conflict,
    OpenError { message: String },
    EditError { message: String },
    SaveError { message: String },
    RefreshError { message: String },
}

impl ProjectContentStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Opened => "open",
            Self::Edited => "edited",
            Self::Saved { .. } => "saved",
            Self::ExternalChanged => "external changed",
            Self::Conflict => "conflict",
            Self::OpenError { .. } => "open error",
            Self::EditError { .. } => "edit error",
            Self::SaveError { .. } => "save error",
            Self::RefreshError { .. } => "refresh error",
        }
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            Self::OpenError { message }
            | Self::EditError { message }
            | Self::SaveError { message }
            | Self::RefreshError { message } => Some(message),
            _ => None,
        }
    }
}

pub fn text_document_state_label(state: TextDocumentState) -> &'static str {
    match state {
        TextDocumentState::Clean => "clean",
        TextDocumentState::Dirty => "dirty",
        TextDocumentState::ExternalChanged => "external changed",
        TextDocumentState::Conflict => "conflict",
        TextDocumentState::SaveError => "save error",
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectContentError {
    NoActiveProject,
    NoActiveDocument,
    Explorer(ExplorerScanError),
    Open(TextDocumentOpenError),
    Edit(TextDocumentEditError),
    Save(TextDocumentSaveError),
    Refresh(TextDocumentRefreshError),
}

impl fmt::Display for ProjectContentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoActiveProject => write!(formatter, "no active project"),
            Self::NoActiveDocument => write!(formatter, "no active text document"),
            Self::Explorer(error) => write!(formatter, "{error}"),
            Self::Open(error) => write!(formatter, "{error}"),
            Self::Edit(error) => write!(formatter, "{error}"),
            Self::Save(error) => write!(formatter, "{error}"),
            Self::Refresh(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for ProjectContentError {}
