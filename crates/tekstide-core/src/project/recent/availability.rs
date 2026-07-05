use std::fs;

use super::RecentProject;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecentProjectAvailability {
    Available,
    FolderMissing,
    CannotReadFolder,
    PathChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoredRecentProject {
    pub recent_project: RecentProject,
    pub availability: RecentProjectAvailability,
}

pub fn assess_recent_project_availability(project: &RecentProject) -> RecentProjectAvailability {
    let Ok(metadata) = fs::metadata(&project.root_path) else {
        return RecentProjectAvailability::FolderMissing;
    };

    if !metadata.is_dir() {
        return RecentProjectAvailability::CannotReadFolder;
    }

    let Ok(canonical_path) = fs::canonicalize(&project.root_path) else {
        return RecentProjectAvailability::CannotReadFolder;
    };

    if canonical_path != project.canonical_root_path {
        return RecentProjectAvailability::PathChanged;
    }

    if fs::read_dir(&canonical_path).is_err() {
        return RecentProjectAvailability::CannotReadFolder;
    }

    RecentProjectAvailability::Available
}
