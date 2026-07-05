use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::path::PathBuf;

use crate::close::CloseResourceSummary;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectId(String);

impl ProjectId {
    pub fn new_uuid() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn from_persisted(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        uuid::Uuid::parse_str(&value).ok()?;
        Some(Self(value))
    }

    #[cfg(test)]
    pub fn for_test(sequence: u64) -> Self {
        Self(format!("00000000-0000-4000-8000-{sequence:012x}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for ProjectId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ProjectId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_persisted(value)
            .ok_or_else(|| serde::de::Error::custom("project_id must be a UUID string"))
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceTrust {
    Restricted,
}

impl WorkspaceTrust {
    pub fn label(self) -> &'static str {
        match self {
            Self::Restricted => "Restricted",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectRuntimeSummary {
    pub risk_warning: bool,
    pub pending_approvals: u32,
    pub review_ready_changes: u32,
    pub failed_processes: u32,
    pub running_processes: u32,
    pub dirty_files: u32,
    pub terminal_count: Option<u32>,
    pub agent_run_count: Option<u32>,
    pub close_resources: CloseResourceSummary,
}

impl Default for ProjectRuntimeSummary {
    fn default() -> Self {
        Self {
            risk_warning: false,
            pending_approvals: 0,
            review_ready_changes: 0,
            failed_processes: 0,
            running_processes: 0,
            dirty_files: 0,
            terminal_count: None,
            agent_run_count: None,
            close_resources: CloseResourceSummary::provider_missing(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSession {
    id: ProjectId,
    display_name: String,
    root_path: PathBuf,
    canonical_root_path: PathBuf,
    trust_state: WorkspaceTrust,
    runtime_summary: ProjectRuntimeSummary,
}

impl ProjectSession {
    pub fn new(
        id: ProjectId,
        display_name: impl Into<String>,
        root_path: impl Into<PathBuf>,
        canonical_root_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            id,
            display_name: display_name.into(),
            root_path: root_path.into(),
            canonical_root_path: canonical_root_path.into(),
            trust_state: WorkspaceTrust::Restricted,
            runtime_summary: ProjectRuntimeSummary::default(),
        }
    }

    pub fn id(&self) -> &ProjectId {
        &self.id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn root_path(&self) -> &PathBuf {
        &self.root_path
    }

    pub fn canonical_root_path(&self) -> &PathBuf {
        &self.canonical_root_path
    }

    pub fn trust_state(&self) -> WorkspaceTrust {
        self.trust_state
    }

    pub fn runtime_summary(&self) -> &ProjectRuntimeSummary {
        &self.runtime_summary
    }

    pub fn close_resource_summary(&self) -> &CloseResourceSummary {
        &self.runtime_summary.close_resources
    }

    #[cfg(test)]
    pub fn set_runtime_summary(&mut self, runtime_summary: ProjectRuntimeSummary) {
        self.runtime_summary = runtime_summary;
    }
}
