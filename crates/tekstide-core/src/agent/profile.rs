use std::path::PathBuf;

use crate::domain::AgentCompatibilityLevel;
use crate::security::TranscriptPrivacyPolicy;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AiCliProfile {
    pub id: String,
    pub display_name: String,
    pub source: AiCliProfileSource,
    pub executable: AiCliExecutable,
    pub compatibility_level: AgentCompatibilityLevel,
    pub prompt_policy: AiCliPromptPolicy,
    pub environment_policy: AiCliEnvironmentPolicy,
    pub workspace_discovery_policy: AiCliWorkspaceDiscoveryPolicy,
    pub adapter_capabilities: AiCliAdapterCapabilities,
    pub transcript_policy: TranscriptPrivacyPolicy,
}

impl AiCliProfile {
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        source: AiCliProfileSource,
        executable: AiCliExecutable,
        compatibility_level: AgentCompatibilityLevel,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            source,
            executable,
            compatibility_level,
            prompt_policy: AiCliPromptPolicy::Interactive,
            environment_policy: AiCliEnvironmentPolicy::Minimal,
            workspace_discovery_policy: AiCliWorkspaceDiscoveryPolicy::NoKnownWorkspaceDiscovery {
                evidence: "profile does not declare workspace-local discovery".to_owned(),
            },
            adapter_capabilities: AiCliAdapterCapabilities::default(),
            transcript_policy: TranscriptPrivacyPolicy::metadata_only_until_retention_ready(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AiCliProfileSource {
    BuiltIn,
    UserGlobal,
    WorkspaceLocal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AiCliExecutable {
    Absolute {
        path: PathBuf,
        provenance: AiCliExecutableProvenance,
    },
    PathLookup {
        command: String,
        lookup_paths: Vec<ExecutableLookupPath>,
        provenance: AiCliExecutableProvenance,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AiCliExecutableProvenance {
    BuiltInReviewed,
    UserGlobal,
    SystemPathReviewed,
    WorkspaceLocal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableLookupPath {
    pub path: PathBuf,
    pub project_local: bool,
}

impl ExecutableLookupPath {
    pub fn reviewed_system(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            project_local: false,
        }
    }

    pub fn project_local(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            project_local: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AiCliPromptPolicy {
    Interactive,
    Argument,
    Stdin,
    WorkspaceLocalTemplate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AiCliEnvironmentPolicy {
    Minimal,
    Named(String),
    ExplicitAllowlist(Vec<String>),
    WorkspaceLocalEnvFile(PathBuf),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AiCliWorkspaceDiscoveryPolicy {
    NoKnownWorkspaceDiscovery { evidence: String },
    DisabledByLaunch { evidence: String },
    MayDiscoverWorkspaceFiles { summary: String },
}

impl AiCliWorkspaceDiscoveryPolicy {
    pub fn evidence(&self) -> &str {
        match self {
            Self::NoKnownWorkspaceDiscovery { evidence } | Self::DisabledByLaunch { evidence } => {
                evidence
            }
            Self::MayDiscoverWorkspaceFiles { summary } => summary,
        }
    }

    pub fn permits_restricted_mode(&self) -> bool {
        matches!(
            self,
            Self::NoKnownWorkspaceDiscovery { .. } | Self::DisabledByLaunch { .. }
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AiCliAdapterCapabilities {
    pub structured_action_approval: bool,
}
