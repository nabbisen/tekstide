use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::domain::AgentRunId;
use crate::project::ProjectId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscriptPathRequest {
    pub state_root: PathBuf,
    pub project_root: PathBuf,
    pub project_id: ProjectId,
    pub agent_run_id: AgentRunId,
}

impl TranscriptPathRequest {
    pub fn new(
        state_root: impl Into<PathBuf>,
        project_root: impl Into<PathBuf>,
        project_id: ProjectId,
        agent_run_id: AgentRunId,
    ) -> Self {
        Self {
            state_root: state_root.into(),
            project_root: project_root.into(),
            project_id,
            agent_run_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscriptStoragePath {
    pub state_root: PathBuf,
    pub project_root: PathBuf,
    pub transcript_dir: PathBuf,
    pub transcript_file: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranscriptPathErrorReason {
    StateRootNotAbsolute,
    ProjectRootNotAbsolute,
    MissingStateRoot,
    MissingProjectRoot,
    CannotReadStateRoot,
    CannotReadProjectRoot,
    StateRootInsideProjectRoot,
    TranscriptPathEscapesStateRoot,
    TranscriptPathInsideProjectRoot,
    UnsafeIdentifier,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscriptPathError {
    pub reason: TranscriptPathErrorReason,
    pub path: PathBuf,
}

impl TranscriptPathError {
    fn new(reason: TranscriptPathErrorReason, path: impl Into<PathBuf>) -> Self {
        Self {
            reason,
            path: path.into(),
        }
    }
}

impl fmt::Display for TranscriptPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "blocked transcript storage path {}: {:?}",
            self.path.display(),
            self.reason
        )
    }
}

impl std::error::Error for TranscriptPathError {}

#[derive(Clone, Copy, Debug, Default)]
pub struct TranscriptPathResolver;

impl TranscriptPathResolver {
    pub fn resolve_agent_run(
        self,
        request: TranscriptPathRequest,
    ) -> Result<TranscriptStoragePath, TranscriptPathError> {
        if !request.state_root.is_absolute() {
            return Err(TranscriptPathError::new(
                TranscriptPathErrorReason::StateRootNotAbsolute,
                request.state_root,
            ));
        }
        if !request.project_root.is_absolute() {
            return Err(TranscriptPathError::new(
                TranscriptPathErrorReason::ProjectRootNotAbsolute,
                request.project_root,
            ));
        }

        let state_root = canonicalize_existing_dir(
            &request.state_root,
            TranscriptPathErrorReason::MissingStateRoot,
            TranscriptPathErrorReason::CannotReadStateRoot,
        )?;
        let project_root = canonicalize_existing_dir(
            &request.project_root,
            TranscriptPathErrorReason::MissingProjectRoot,
            TranscriptPathErrorReason::CannotReadProjectRoot,
        )?;

        if path_contains(&project_root, &state_root) {
            return Err(TranscriptPathError::new(
                TranscriptPathErrorReason::StateRootInsideProjectRoot,
                state_root,
            ));
        }

        let project_component = safe_component(request.project_id.as_str()).ok_or_else(|| {
            TranscriptPathError::new(
                TranscriptPathErrorReason::UnsafeIdentifier,
                request.project_id.as_str(),
            )
        })?;
        let agent_run_component =
            safe_component(request.agent_run_id.as_str()).ok_or_else(|| {
                TranscriptPathError::new(
                    TranscriptPathErrorReason::UnsafeIdentifier,
                    request.agent_run_id.as_str(),
                )
            })?;

        let transcript_dir = state_root
            .join("transcripts")
            .join(project_component)
            .join(agent_run_component);
        let transcript_file = transcript_dir.join("transcript.log");

        ensure_structural_containment(&state_root, &transcript_file, &project_root)?;

        Ok(TranscriptStoragePath {
            state_root,
            project_root,
            transcript_dir,
            transcript_file,
        })
    }
}

fn canonicalize_existing_dir(
    path: &Path,
    missing_reason: TranscriptPathErrorReason,
    unreadable_reason: TranscriptPathErrorReason,
) -> Result<PathBuf, TranscriptPathError> {
    match fs::canonicalize(path) {
        Ok(canonical) if canonical.is_dir() => Ok(canonical),
        Ok(canonical) => Err(TranscriptPathError::new(missing_reason, canonical)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(TranscriptPathError::new(missing_reason, path))
        }
        Err(_) => Err(TranscriptPathError::new(unreadable_reason, path)),
    }
}

fn safe_component(value: &str) -> Option<&str> {
    if value.is_empty() {
        return None;
    }
    let path = Path::new(value);
    if path.components().any(|component| {
        !matches!(
            component,
            Component::Normal(_) | Component::CurDir | Component::ParentDir
        )
    }) {
        return None;
    }
    if value.contains(['/', '\\']) || value == "." || value == ".." || value.contains("..") {
        return None;
    }
    Some(value)
}

fn ensure_structural_containment(
    state_root: &Path,
    transcript_file: &Path,
    project_root: &Path,
) -> Result<(), TranscriptPathError> {
    if !path_contains(state_root, transcript_file) {
        return Err(TranscriptPathError::new(
            TranscriptPathErrorReason::TranscriptPathEscapesStateRoot,
            transcript_file,
        ));
    }
    if path_contains(project_root, transcript_file) {
        return Err(TranscriptPathError::new(
            TranscriptPathErrorReason::TranscriptPathInsideProjectRoot,
            transcript_file,
        ));
    }
    Ok(())
}

fn path_contains(root: &Path, target: &Path) -> bool {
    target.starts_with(root)
}
