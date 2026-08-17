use std::collections::HashSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::project::{ProjectId, WorkspaceTrust};

use super::Timestamp;

pub const RECENT_PROJECT_STATE_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecentProjectState {
    pub state_version: u32,
    pub projects: Vec<RecentProject>,
}

impl Default for RecentProjectState {
    fn default() -> Self {
        Self {
            state_version: RECENT_PROJECT_STATE_VERSION,
            projects: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecentProject {
    pub project_id: ProjectId,
    pub display_name: String,
    pub root_path: PathBuf,
    pub canonical_root_path: PathBuf,
    pub last_opened_at: Timestamp,
    pub last_activity: Timestamp,
    /// RFC-032: a real, typed trust value -- was `last_trust_state_summary:
    /// String`, a write-only display label nothing ever read back. This
    /// is the record `AppState::add_project_session` restores from on
    /// reopen, keyed by `canonical_root_path` matching (never `root_path`
    /// -- a redirected symlink must not inherit the old canonical
    /// entry's trust, see `app.rs`'s `recent_trust_by_canonical_root`).
    /// `#[serde(default)]`: a pre-RFC-032 on-disk record lacks this
    /// field entirely; defaulting to `WorkspaceTrust::Restricted` is
    /// correct, not merely convenient, since nothing could have been
    /// trusted before `grant_project_trust` had a production caller.
    #[serde(default)]
    pub trust_state: WorkspaceTrust,
}

impl RecentProjectState {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self)
            .expect("recent-project state serialization should not fail")
            + "\n"
    }

    pub fn from_json(content: &str) -> Result<Self, String> {
        let state = serde_json::from_str::<Self>(content).map_err(|error| error.to_string())?;
        state.validate()?;
        Ok(state)
    }

    fn validate(&self) -> Result<(), String> {
        if self.state_version != RECENT_PROJECT_STATE_VERSION {
            return Err(format!(
                "unsupported recent-project state_version: {}",
                self.state_version
            ));
        }

        let mut project_ids = HashSet::new();
        let mut canonical_roots = HashSet::new();

        for project in &self.projects {
            if !project_ids.insert(project.project_id.clone()) {
                return Err(format!("duplicate project_id: {}", project.project_id));
            }
            if !canonical_roots.insert(project.canonical_root_path.clone()) {
                return Err(format!(
                    "duplicate canonical_root_path: {}",
                    project.canonical_root_path.display()
                ));
            }
        }

        Ok(())
    }
}

impl RecentProject {
    pub fn new(
        project_id: ProjectId,
        display_name: impl Into<String>,
        root_path: impl Into<PathBuf>,
        canonical_root_path: impl Into<PathBuf>,
        now: Timestamp,
        trust_state: WorkspaceTrust,
    ) -> Self {
        Self {
            project_id,
            display_name: display_name.into(),
            root_path: root_path.into(),
            canonical_root_path: canonical_root_path.into(),
            last_opened_at: now.clone(),
            last_activity: now,
            trust_state,
        }
    }

    pub fn with_timestamps(
        project_id: ProjectId,
        display_name: impl Into<String>,
        root_path: impl Into<PathBuf>,
        canonical_root_path: impl Into<PathBuf>,
        last_opened_at: Timestamp,
        last_activity: Timestamp,
        trust_state: WorkspaceTrust,
    ) -> Self {
        Self {
            project_id,
            display_name: display_name.into(),
            root_path: root_path.into(),
            canonical_root_path: canonical_root_path.into(),
            last_opened_at,
            last_activity,
            trust_state,
        }
    }
}
