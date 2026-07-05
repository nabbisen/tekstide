use crate::project::ProjectId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnershipError {
    CrossProject,
    WrongAgentRun,
    DuplicateAttachment,
    MissingProject,
    MissingReference,
}

pub(super) fn ensure_same_project(
    left: &ProjectId,
    right: &ProjectId,
) -> Result<(), OwnershipError> {
    if left == right {
        Ok(())
    } else {
        Err(OwnershipError::CrossProject)
    }
}
