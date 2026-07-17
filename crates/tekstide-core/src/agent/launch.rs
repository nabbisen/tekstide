use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::domain::{AgentCompatibilityLevel, AgentRun, AgentRunStatus, AgentRunTransitionError};
use crate::project::{ProjectId, ProjectSession, WorkspaceTrust};
use crate::runtime::terminal::{TerminalDimensions, TerminalEnvironmentPolicy, TerminalLaunchSpec};
use crate::security::is_restricted_mode;

use super::profile::{
    AiCliEnvironmentPolicy, AiCliExecutable, AiCliExecutableProvenance, AiCliProfile,
    AiCliProfileSource, AiCliPromptPolicy,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunLaunchRequest {
    pub project_id: ProjectId,
    pub profile_id: String,
    pub prompt_summary: String,
    pub cwd: Option<PathBuf>,
}

impl AgentRunLaunchRequest {
    pub fn new(
        project_id: ProjectId,
        profile_id: impl Into<String>,
        prompt_summary: impl Into<String>,
    ) -> Self {
        Self {
            project_id,
            profile_id: profile_id.into(),
            prompt_summary: prompt_summary.into(),
            cwd: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunLaunchValidation {
    pub project_id: ProjectId,
    pub profile_id: String,
    pub executable_path: PathBuf,
    pub executable_provenance: AiCliExecutableProvenance,
    pub cwd: PathBuf,
    pub compatibility_level: AgentCompatibilityLevel,
    pub prompt_summary: String,
    pub environment_summary: AgentLaunchSummary,
    pub terminal_environment_policy: TerminalEnvironmentPolicy,
    pub workspace_discovery_summary: AgentLaunchSummary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunLaunchSpec {
    pub project_id: ProjectId,
    pub profile_id: String,
    pub executable_path: PathBuf,
    pub executable_provenance: AiCliExecutableProvenance,
    pub cwd: PathBuf,
    pub compatibility_level: AgentCompatibilityLevel,
    pub prompt_summary: String,
    pub environment_summary: AgentLaunchSummary,
    pub terminal_environment_policy: TerminalEnvironmentPolicy,
    pub workspace_discovery_summary: AgentLaunchSummary,
}

impl From<AgentRunLaunchValidation> for AgentRunLaunchSpec {
    fn from(validation: AgentRunLaunchValidation) -> Self {
        Self {
            project_id: validation.project_id,
            profile_id: validation.profile_id,
            executable_path: validation.executable_path,
            executable_provenance: validation.executable_provenance,
            cwd: validation.cwd,
            compatibility_level: validation.compatibility_level,
            prompt_summary: validation.prompt_summary,
            environment_summary: validation.environment_summary,
            terminal_environment_policy: validation.terminal_environment_policy,
            workspace_discovery_summary: validation.workspace_discovery_summary,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunLaunchPlan {
    pub spec: AgentRunLaunchSpec,
    pub agent_run: AgentRun,
    pub terminal_launch_spec: TerminalLaunchSpec,
}

impl AgentRunLaunchPlan {
    pub fn from_validation(
        validation: AgentRunLaunchValidation,
        terminal_title: impl Into<String>,
    ) -> Result<Self, AgentRunTransitionError> {
        let spec = AgentRunLaunchSpec::from(validation);
        let mut agent_run = AgentRun::draft(
            spec.project_id.clone(),
            spec.profile_id.clone(),
            spec.prompt_summary.clone(),
            spec.compatibility_level,
        );
        agent_run.transition_to(AgentRunStatus::Ready)?;

        let terminal_launch_spec = TerminalLaunchSpec {
            project_id: spec.project_id.clone(),
            title: terminal_title.into(),
            cwd: spec.cwd.clone(),
            shell: spec.executable_path.clone(),
            command_line_summary: spec.executable_path.display().to_string(),
            environment_policy: spec.terminal_environment_policy.clone(),
            kind: terminal_kind_from_compatibility(spec.compatibility_level),
            dimensions: TerminalDimensions::default(),
        };

        Ok(Self {
            spec,
            agent_run,
            terminal_launch_spec,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentRunLaunchValidationError {
    CrossProject,
    WrongProfile,
    MissingProjectRoot { summary: AgentLaunchSummary },
    InvalidCwd { summary: AgentLaunchSummary },
    CwdEscapesProjectRoot { summary: AgentLaunchSummary },
    WorkspaceLocalProfileBlocked,
    WorkspaceLocalPromptBlocked,
    WorkspaceLocalEnvironmentBlocked,
    WorkspaceLocalExecutableBlocked { path: PathBuf },
    ProjectLocalPathLookupBlocked { path: PathBuf },
    ExecutableUnavailable { summary: AgentLaunchSummary },
    WorkspaceDiscoveryBlocked { summary: AgentLaunchSummary },
    MissingWorkspaceDiscoveryEvidence,
    ManagedCapabilityMissing,
    TranscriptBytesBlockedUntilRetentionPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentLaunchSummary {
    text: String,
    truncated: bool,
}

impl AgentLaunchSummary {
    pub const MAX_CHARS: usize = 240;

    pub fn new(text: impl AsRef<str>) -> Self {
        let mut bounded = String::new();
        let mut truncated = false;

        for (index, character) in text.as_ref().chars().enumerate() {
            if index >= Self::MAX_CHARS {
                truncated = true;
                break;
            }
            bounded.push(character);
        }

        Self {
            text: bounded,
            truncated,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn was_truncated(&self) -> bool {
        self.truncated
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AgentRunLaunchValidator;

impl AgentRunLaunchValidator {
    pub fn validate(
        self,
        project: &ProjectSession,
        profile: &AiCliProfile,
        request: &AgentRunLaunchRequest,
    ) -> Result<AgentRunLaunchValidation, AgentRunLaunchValidationError> {
        if project.id() != &request.project_id {
            return Err(AgentRunLaunchValidationError::CrossProject);
        }
        if profile.id != request.profile_id {
            return Err(AgentRunLaunchValidationError::WrongProfile);
        }

        let restricted = is_restricted_mode(project.trust_state());
        validate_profile_source(project.trust_state(), profile)?;
        validate_prompt_policy(restricted, profile.prompt_policy)?;
        validate_environment_policy(restricted, &profile.environment_policy)?;
        validate_workspace_discovery_policy(restricted, profile)?;
        validate_compatibility(profile)?;
        validate_transcript_policy(profile)?;

        let root = canonical_existing_dir(project.canonical_root_path()).map_err(|summary| {
            AgentRunLaunchValidationError::MissingProjectRoot {
                summary: AgentLaunchSummary::new(summary),
            }
        })?;
        let cwd = match request.cwd.as_ref() {
            Some(cwd) => canonical_existing_dir(cwd).map_err(|summary| {
                AgentRunLaunchValidationError::InvalidCwd {
                    summary: AgentLaunchSummary::new(summary),
                }
            })?,
            None => root.clone(),
        };

        if !cwd.starts_with(&root) {
            return Err(AgentRunLaunchValidationError::CwdEscapesProjectRoot {
                summary: AgentLaunchSummary::new(format!(
                    "AgentRun cwd is outside project root: {}",
                    cwd.display()
                )),
            });
        }

        let (executable_path, executable_provenance) =
            resolve_executable(profile, &root, restricted)?;

        Ok(AgentRunLaunchValidation {
            project_id: request.project_id.clone(),
            profile_id: profile.id.clone(),
            executable_path,
            executable_provenance,
            cwd,
            compatibility_level: profile.compatibility_level,
            prompt_summary: request.prompt_summary.clone(),
            environment_summary: environment_summary(&profile.environment_policy),
            terminal_environment_policy: terminal_environment_policy(&profile.environment_policy),
            workspace_discovery_summary: AgentLaunchSummary::new(
                profile.workspace_discovery_policy.evidence(),
            ),
        })
    }
}

fn validate_profile_source(
    trust: WorkspaceTrust,
    profile: &AiCliProfile,
) -> Result<(), AgentRunLaunchValidationError> {
    if is_restricted_mode(trust) && profile.source == AiCliProfileSource::WorkspaceLocal {
        Err(AgentRunLaunchValidationError::WorkspaceLocalProfileBlocked)
    } else {
        Ok(())
    }
}

fn validate_prompt_policy(
    restricted: bool,
    prompt_policy: AiCliPromptPolicy,
) -> Result<(), AgentRunLaunchValidationError> {
    if restricted && prompt_policy == AiCliPromptPolicy::WorkspaceLocalTemplate {
        Err(AgentRunLaunchValidationError::WorkspaceLocalPromptBlocked)
    } else {
        Ok(())
    }
}

fn validate_environment_policy(
    restricted: bool,
    environment_policy: &AiCliEnvironmentPolicy,
) -> Result<(), AgentRunLaunchValidationError> {
    if restricted
        && matches!(
            environment_policy,
            AiCliEnvironmentPolicy::WorkspaceLocalEnvFile(_)
        )
    {
        Err(AgentRunLaunchValidationError::WorkspaceLocalEnvironmentBlocked)
    } else {
        Ok(())
    }
}

fn validate_workspace_discovery_policy(
    restricted: bool,
    profile: &AiCliProfile,
) -> Result<(), AgentRunLaunchValidationError> {
    if !restricted {
        return Ok(());
    }

    if !profile.workspace_discovery_policy.permits_restricted_mode() {
        return Err(AgentRunLaunchValidationError::WorkspaceDiscoveryBlocked {
            summary: AgentLaunchSummary::new(profile.workspace_discovery_policy.evidence()),
        });
    }

    if matches!(
        profile.source,
        AiCliProfileSource::BuiltIn | AiCliProfileSource::UserGlobal
    ) && profile
        .workspace_discovery_policy
        .evidence()
        .trim()
        .is_empty()
    {
        return Err(AgentRunLaunchValidationError::MissingWorkspaceDiscoveryEvidence);
    }

    Ok(())
}

fn validate_compatibility(profile: &AiCliProfile) -> Result<(), AgentRunLaunchValidationError> {
    if profile.compatibility_level == AgentCompatibilityLevel::Managed
        && !profile.adapter_capabilities.structured_action_approval
    {
        Err(AgentRunLaunchValidationError::ManagedCapabilityMissing)
    } else {
        Ok(())
    }
}

fn validate_transcript_policy(profile: &AiCliProfile) -> Result<(), AgentRunLaunchValidationError> {
    if profile
        .transcript_policy
        .permits_transcript_byte_persistence()
    {
        Err(AgentRunLaunchValidationError::TranscriptBytesBlockedUntilRetentionPolicy)
    } else {
        Ok(())
    }
}

fn resolve_executable(
    profile: &AiCliProfile,
    root: &Path,
    restricted: bool,
) -> Result<(PathBuf, AiCliExecutableProvenance), AgentRunLaunchValidationError> {
    match &profile.executable {
        AiCliExecutable::Absolute { path, provenance } => {
            let canonical = canonical_executable(path)?;
            validate_executable_provenance(restricted, root, &canonical, *provenance)?;
            Ok((canonical, *provenance))
        }
        AiCliExecutable::PathLookup {
            command,
            lookup_paths,
            provenance,
        } => {
            for lookup_path in lookup_paths {
                validate_lookup_path(
                    restricted,
                    root,
                    &lookup_path.path,
                    lookup_path.project_local,
                )?;

                let candidate = lookup_path.path.join(command);
                if candidate.exists() {
                    let canonical = canonical_executable(&candidate)?;
                    validate_executable_provenance(restricted, root, &canonical, *provenance)?;
                    return Ok((canonical, *provenance));
                }
            }

            Err(AgentRunLaunchValidationError::ExecutableUnavailable {
                summary: AgentLaunchSummary::new(format!(
                    "profile executable was not found by reviewed PATH lookup: {command}"
                )),
            })
        }
    }
}

fn validate_lookup_path(
    restricted: bool,
    root: &Path,
    lookup_path: &Path,
    declared_project_local: bool,
) -> Result<(), AgentRunLaunchValidationError> {
    if !restricted {
        return Ok(());
    }

    if declared_project_local {
        return Err(
            AgentRunLaunchValidationError::ProjectLocalPathLookupBlocked {
                path: lookup_path.to_path_buf(),
            },
        );
    }

    if let Ok(canonical_lookup_path) = lookup_path.canonicalize()
        && canonical_lookup_path.starts_with(root)
    {
        return Err(
            AgentRunLaunchValidationError::ProjectLocalPathLookupBlocked {
                path: lookup_path.to_path_buf(),
            },
        );
    }

    Ok(())
}

fn validate_executable_provenance(
    restricted: bool,
    root: &Path,
    executable: &Path,
    provenance: AiCliExecutableProvenance,
) -> Result<(), AgentRunLaunchValidationError> {
    if restricted
        && (provenance == AiCliExecutableProvenance::WorkspaceLocal || executable.starts_with(root))
    {
        Err(
            AgentRunLaunchValidationError::WorkspaceLocalExecutableBlocked {
                path: executable.to_path_buf(),
            },
        )
    } else {
        Ok(())
    }
}

fn canonical_executable(path: &Path) -> Result<PathBuf, AgentRunLaunchValidationError> {
    let canonical = path.canonicalize().map_err(|error| {
        AgentRunLaunchValidationError::ExecutableUnavailable {
            summary: AgentLaunchSummary::new(format!(
                "failed to canonicalize executable {}: {error}",
                path.display()
            )),
        }
    })?;

    if is_executable_file(&canonical) {
        Ok(canonical)
    } else {
        Err(AgentRunLaunchValidationError::ExecutableUnavailable {
            summary: AgentLaunchSummary::new(format!(
                "profile executable is not an executable file: {}",
                canonical.display()
            )),
        })
    }
}

fn canonical_existing_dir(path: &Path) -> Result<PathBuf, String> {
    let canonical = path.canonicalize().map_err(|error| {
        format!(
            "failed to canonicalize directory {}: {error}",
            path.display()
        )
    })?;
    if canonical.is_dir() {
        Ok(canonical)
    } else {
        Err(format!("path is not a directory: {}", canonical.display()))
    }
}

fn is_executable_file(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

fn environment_summary(environment_policy: &AiCliEnvironmentPolicy) -> AgentLaunchSummary {
    match environment_policy {
        AiCliEnvironmentPolicy::Minimal => AgentLaunchSummary::new("minimal environment"),
        AiCliEnvironmentPolicy::Named(name) => {
            AgentLaunchSummary::new(format!("named environment policy: {name}"))
        }
        AiCliEnvironmentPolicy::ExplicitAllowlist(names) => AgentLaunchSummary::new(format!(
            "explicit environment allowlist: {}",
            names.join(", ")
        )),
        AiCliEnvironmentPolicy::WorkspaceLocalEnvFile(_) => {
            AgentLaunchSummary::new("workspace-local environment file")
        }
    }
}

fn terminal_environment_policy(
    environment_policy: &AiCliEnvironmentPolicy,
) -> TerminalEnvironmentPolicy {
    match environment_policy {
        AiCliEnvironmentPolicy::Minimal => TerminalEnvironmentPolicy::Minimal,
        AiCliEnvironmentPolicy::Named(name) => TerminalEnvironmentPolicy::Named(name.clone()),
        AiCliEnvironmentPolicy::ExplicitAllowlist(names) => {
            TerminalEnvironmentPolicy::ExplicitAllowlist(names.clone())
        }
        AiCliEnvironmentPolicy::WorkspaceLocalEnvFile(_) => {
            TerminalEnvironmentPolicy::Named("workspace-local environment file".to_owned())
        }
    }
}

fn terminal_kind_from_compatibility(
    compatibility_level: AgentCompatibilityLevel,
) -> crate::domain::TerminalKind {
    match compatibility_level {
        AgentCompatibilityLevel::Plain => crate::domain::TerminalKind::Plain,
        AgentCompatibilityLevel::Supervised => crate::domain::TerminalKind::Supervised,
        AgentCompatibilityLevel::Managed => crate::domain::TerminalKind::Managed,
    }
}
