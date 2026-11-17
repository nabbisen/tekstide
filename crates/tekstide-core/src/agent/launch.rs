use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::domain::{
    AgentCompatibilityLevel, AgentRun, AgentRunStatus, AgentRunTransitionError, ProjectId,
    TerminalDimensions, TerminalEnvironmentPolicy, WorkspaceTrust,
};
use crate::security::is_restricted_mode;
use crate::transcript::{
    TranscriptCaptureMode, TranscriptCapturePolicy, TranscriptPathError, TranscriptPathRequest,
    TranscriptPathResolver, TranscriptRetentionLimits, TranscriptStoragePath,
    TranscriptWriterConfig,
};

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
    pub transcript_capture_mode: TranscriptCaptureMode,
    pub transcript_state_root: Option<PathBuf>,
    pub transcript_retention_limits: TranscriptRetentionLimits,
    /// RFC-022 PR-022-C response 216 (required change): the approval
    /// channel's own state root, independent of `transcript_state_root`.
    /// **Required, not a convenience default**: the first version of
    /// `prepare_adapter_approval` reused `transcript_state_root` for
    /// this, which meant `without_transcript_capture()` silently made a
    /// `Managed` launch impossible -- RFC-011's documented per-run
    /// transcript opt-out and RFC-022's command approval had become
    /// accidentally coupled, a policy nobody decided. `None` here falls
    /// back to `transcript_state_root` (`project::session::
    /// bind_adapter_approval`'s own doc comment states this explicitly --
    /// RFC-077 review 685/686 moved the bind, and this decoupling with
    /// it, out of this module) purely for the common case
    /// where a caller wants both artifacts in the same place -- it does
    /// not reintroduce the coupling, since `Some` here always takes
    /// priority and a caller can set this without transcript capture at
    /// all.
    pub approval_state_root: Option<PathBuf>,
    /// RFC-049 D4′: whether this project's retention cleanup has just run at
    /// launch preflight and **left a byte budget still over its limit**.
    ///
    /// The caller measures it; this layer decides what it means, so the two
    /// modes cannot drift apart at a call site: `LocalBounded` starts the run
    /// with capture disabled and says so, `RequiredLocalBounded` refuses the
    /// launch before any process starts (RFC-011). **Never "delete something
    /// live to make room"** — §2 of `what-deleting-a-transcript-must-not-do.md`.
    pub transcript_budget_exhausted: bool,
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
            transcript_capture_mode: TranscriptCaptureMode::LocalBounded,
            transcript_state_root: None,
            transcript_retention_limits: TranscriptRetentionLimits::agent_run_default(),
            approval_state_root: None,
            transcript_budget_exhausted: false,
        }
    }

    pub fn without_transcript_capture(mut self) -> Self {
        self.transcript_capture_mode = TranscriptCaptureMode::Disabled;
        self.transcript_state_root = None;
        self
    }

    /// RFC-022 PR-022-C response 216: sets the approval channel's own
    /// state root, independent of transcript capture. A `Managed`
    /// profile that also wants `without_transcript_capture()` must call
    /// this too -- without it, a `Managed` launch with no transcript
    /// state root configured has nowhere to bind its approval channel
    /// either, and fails closed with
    /// `project::AgentAdapterApprovalError::StateRootMissing` (RFC-077,
    /// review 685 response: the bind, and this error type, moved to
    /// `project`).
    pub fn with_approval_channel(mut self, state_root: impl Into<PathBuf>) -> Self {
        self.approval_state_root = Some(state_root.into());
        self
    }

    pub fn with_local_bounded_transcript(mut self, state_root: impl Into<PathBuf>) -> Self {
        self.transcript_capture_mode = TranscriptCaptureMode::LocalBounded;
        self.transcript_state_root = Some(state_root.into());
        self
    }

    pub fn with_required_local_bounded_transcript(
        mut self,
        state_root: impl Into<PathBuf>,
    ) -> Self {
        self.transcript_capture_mode = TranscriptCaptureMode::RequiredLocalBounded;
        self.transcript_state_root = Some(state_root.into());
        self
    }

    /// RFC-049 D4′: states that preflight cleanup left a byte budget over its
    /// limit. The caller measures; `validate_transcript_policy` and
    /// `prepare_transcript_capture` decide what it costs this launch.
    pub fn with_transcript_budget_exhausted(mut self, exhausted: bool) -> Self {
        self.transcript_budget_exhausted = exhausted;
        self
    }

    pub fn with_transcript_retention_limits(
        mut self,
        retention_limits: TranscriptRetentionLimits,
    ) -> Self {
        self.transcript_retention_limits = retention_limits;
        self
    }
}

/// A `cwd` that has already passed `AgentRunLaunchValidator::validate`'s
/// containment check (canonicalized, confirmed to start with the
/// project's canonical root). RFC-021 PR-021-E1/response 114 Q3 flagged
/// that `approval::coordinator::receive_proposal` taking a plain `&Path`
/// for its verified cwd parameter did not stop a caller from passing an
/// *unverified* path (e.g. `CommandProposal::cwd()`, the adapter's own
/// untrusted claim) by mistake -- both are `&Path`, so the compiler
/// cannot tell them apart. This type has deliberately no public
/// constructor that accepts an arbitrary path: the only way to obtain one
/// is already having gone through this validator, the same parse-don't-
/// validate discipline `approval::protocol::CommandProposal::decode`
/// already established for untrusted wire data.
///
/// **`approval -> agent` is permanent (RFC-077, 2026-11-16).** `approval`
/// must name this type and this module must own minting it, and in a
/// single crate those two requirements are jointly satisfiable only by a
/// dependency from the former to the latter -- see `ARCHITECTURE.md`'s
/// seventh invariant, "a privilege-bearing type's dependency is the
/// privilege," and `rfcs/handoffs/077-the-capability-split/
/// staging-derived-from-measured-cut-cost.md`'s "Stage 6 does not happen
/// as scoped." `from_validated` below having exactly one production call
/// site, immediately after the containment check, is a capability-gate
/// test (`verified_cwd_from_validated_has_exactly_one_production_call_site`),
/// not only this comment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCwd(PathBuf);

impl VerifiedCwd {
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// The crate's one minting path. `pub(self)`-equivalent (private to
    /// this file, not merely this module) is deliberate: `pub(crate)`
    /// would let any `tekstide-core` module -- `approval::coordinator`
    /// included -- construct a `VerifiedCwd` from an unverified `&Path`,
    /// which is the exact laundering this type exists to make
    /// unreachable. See this type's own doc comment above.
    fn from_validated(path: PathBuf) -> Self {
        Self(path)
    }

    /// Test-only escape hatch, matching this crate's established
    /// `for_test` convention (`AgentRunId::for_test`, `ProjectId::for_test`)
    /// -- other modules' tests (e.g. `approval::coordinator`'s) need a
    /// `VerifiedCwd` without constructing an entire `ProjectSession` /
    /// `AiCliProfile` / `AgentRunLaunchRequest` pipeline just to obtain
    /// one. `#[cfg(test)]`-gated, so this does not weaken the real
    /// guarantee in a release build.
    #[cfg(test)]
    pub fn for_test(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }
}

impl From<VerifiedCwd> for PathBuf {
    fn from(verified: VerifiedCwd) -> Self {
        verified.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunLaunchValidation {
    project_id: ProjectId,
    profile_id: String,
    project_root: PathBuf,
    executable_path: PathBuf,
    executable_provenance: AiCliExecutableProvenance,
    cwd: VerifiedCwd,
    compatibility_level: AgentCompatibilityLevel,
    prompt_summary: String,
    environment_summary: AgentLaunchSummary,
    terminal_environment_policy: TerminalEnvironmentPolicy,
    workspace_discovery_summary: AgentLaunchSummary,
    transcript_capture: AgentRunTranscriptCapture,
    approval_state_root: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunLaunchSpec {
    project_id: ProjectId,
    profile_id: String,
    project_root: PathBuf,
    executable_path: PathBuf,
    executable_provenance: AiCliExecutableProvenance,
    cwd: VerifiedCwd,
    compatibility_level: AgentCompatibilityLevel,
    prompt_summary: String,
    environment_summary: AgentLaunchSummary,
    terminal_environment_policy: TerminalEnvironmentPolicy,
    workspace_discovery_summary: AgentLaunchSummary,
    transcript_capture: AgentRunTranscriptCapture,
    approval_state_root: Option<PathBuf>,
}

impl AgentRunLaunchValidation {
    pub fn project_id(&self) -> &ProjectId {
        &self.project_id
    }

    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    pub fn executable_path(&self) -> &Path {
        &self.executable_path
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn executable_provenance(&self) -> AiCliExecutableProvenance {
        self.executable_provenance
    }

    pub fn cwd(&self) -> &Path {
        self.cwd.as_path()
    }

    pub fn verified_cwd(&self) -> &VerifiedCwd {
        &self.cwd
    }

    pub fn compatibility_level(&self) -> AgentCompatibilityLevel {
        self.compatibility_level
    }

    pub fn prompt_summary(&self) -> &str {
        &self.prompt_summary
    }

    pub fn environment_summary(&self) -> &AgentLaunchSummary {
        &self.environment_summary
    }

    pub fn terminal_environment_policy(&self) -> &TerminalEnvironmentPolicy {
        &self.terminal_environment_policy
    }

    pub fn workspace_discovery_summary(&self) -> &AgentLaunchSummary {
        &self.workspace_discovery_summary
    }

    pub fn transcript_capture(&self) -> &AgentRunTranscriptCapture {
        &self.transcript_capture
    }

    pub fn approval_state_root(&self) -> Option<&Path> {
        self.approval_state_root.as_deref()
    }
}

impl From<AgentRunLaunchValidation> for AgentRunLaunchSpec {
    fn from(validation: AgentRunLaunchValidation) -> Self {
        Self {
            project_id: validation.project_id,
            profile_id: validation.profile_id,
            project_root: validation.project_root,
            executable_path: validation.executable_path,
            executable_provenance: validation.executable_provenance,
            cwd: validation.cwd,
            compatibility_level: validation.compatibility_level,
            prompt_summary: validation.prompt_summary,
            environment_summary: validation.environment_summary,
            terminal_environment_policy: validation.terminal_environment_policy,
            workspace_discovery_summary: validation.workspace_discovery_summary,
            transcript_capture: validation.transcript_capture,
            approval_state_root: validation.approval_state_root,
        }
    }
}

/// RFC-077 Stage 8, D5 move 3: `agent`'s own runtime-free terminal-launch
/// decision, carried to `project` (which already has edges to both
/// `agent` and `runtime`) for assembly into a real `runtime::terminal::
/// TerminalLaunchSpec`.
///
/// **RFC-077, review 685 response ("move the binding cluster to
/// project")**: the approval-channel *binding* no longer happens here at
/// all -- it moved to `project`, alongside the *assembly* it already
/// owned, so this struct carries no `approval` type and has no
/// `adapter_approval` field any more. `agent` no longer names `approval`
/// in any of its own type definitions.
///
/// **No public constructor taking arbitrary fields** -- the `VerifiedCwd`
/// lesson applied where it works: the only way to obtain one is through
/// this module's own validated launch-planning flow
/// (`AgentRunLaunchPlan::from_validation`, `prepare_transcript_capture`),
/// so `project` reads every field through an accessor but cannot forge
/// one -- in particular, cannot mint a `compatibility_level` that
/// `AgentRunLaunchValidator::validate` never actually granted, which is
/// what `authorize_validated_agent_run` reads when `project` assembles
/// the real spec.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentTerminalLaunchIntent {
    project_id: ProjectId,
    title: String,
    cwd: PathBuf,
    shell: PathBuf,
    command_line_summary: String,
    environment_policy: TerminalEnvironmentPolicy,
    dimensions: TerminalDimensions,
    compatibility_level: AgentCompatibilityLevel,
    transcript_writer_config: Option<TranscriptWriterConfig>,
}

impl AgentTerminalLaunchIntent {
    pub fn project_id(&self) -> &ProjectId {
        &self.project_id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub fn shell(&self) -> &Path {
        &self.shell
    }

    pub fn command_line_summary(&self) -> &str {
        &self.command_line_summary
    }

    pub fn environment_policy(&self) -> &TerminalEnvironmentPolicy {
        &self.environment_policy
    }

    pub fn dimensions(&self) -> TerminalDimensions {
        self.dimensions
    }

    pub fn compatibility_level(&self) -> AgentCompatibilityLevel {
        self.compatibility_level
    }

    pub fn transcript_writer_config(&self) -> Option<&TranscriptWriterConfig> {
        self.transcript_writer_config.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunLaunchPlan {
    spec: AgentRunLaunchSpec,
    agent_run: AgentRun,
    terminal_launch_intent: AgentTerminalLaunchIntent,
    transcript_storage_path: Option<TranscriptStoragePath>,
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

        let terminal_launch_intent = AgentTerminalLaunchIntent {
            project_id: spec.project_id.clone(),
            title: terminal_title.into(),
            cwd: spec.cwd.clone().into(),
            shell: spec.executable_path.clone(),
            command_line_summary: spec.executable_path.display().to_string(),
            environment_policy: spec.terminal_environment_policy.clone(),
            dimensions: TerminalDimensions::default(),
            compatibility_level: spec.compatibility_level,
            transcript_writer_config: None,
        };

        Ok(Self {
            spec,
            agent_run,
            terminal_launch_intent,
            transcript_storage_path: None,
        })
    }

    pub fn spec(&self) -> &AgentRunLaunchSpec {
        &self.spec
    }

    pub fn agent_run(&self) -> &AgentRun {
        &self.agent_run
    }

    pub(crate) fn terminal_launch_intent(&self) -> &AgentTerminalLaunchIntent {
        &self.terminal_launch_intent
    }

    pub(crate) fn prepare_transcript_capture(
        &mut self,
    ) -> Result<(), AgentRunTranscriptCaptureError> {
        let Some(state_root) = self.spec.transcript_capture.state_root.as_ref() else {
            if self
                .spec
                .transcript_capture
                .mode
                .rejects_launch_when_unavailable()
            {
                return Err(AgentRunTranscriptCaptureError::StateRootMissing);
            }
            self.terminal_launch_intent.transcript_writer_config = None;
            self.transcript_storage_path = None;
            return Ok(());
        };

        // RFC-049 D4′: the budget was still exhausted after preflight cleanup,
        // so this run starts **without capture** and the surfaces say so.
        // `RequiredLocalBounded` never reaches here — `validate_transcript_policy`
        // refuses it before any process starts — but the same shape is written
        // here too, so this function is honest read on its own.
        if self.spec.transcript_capture.budget_exhausted {
            if self
                .spec
                .transcript_capture
                .mode
                .rejects_launch_when_unavailable()
            {
                return Err(AgentRunTranscriptCaptureError::BudgetExhausted);
            }
            self.terminal_launch_intent.transcript_writer_config = None;
            self.transcript_storage_path = None;
            return Ok(());
        }

        let capture_policy = self.spec.transcript_capture.capture_policy();
        if !capture_policy.permits_transcript_byte_persistence() {
            if self
                .spec
                .transcript_capture
                .mode
                .rejects_launch_when_unavailable()
            {
                return Err(AgentRunTranscriptCaptureError::PolicyDoesNotPermitBytes);
            }
            self.terminal_launch_intent.transcript_writer_config = None;
            self.transcript_storage_path = None;
            return Ok(());
        }

        let storage_path = TranscriptPathResolver.resolve_agent_run(TranscriptPathRequest::new(
            state_root,
            &self.spec.project_root,
            self.spec.project_id.clone(),
            self.agent_run.id.clone(),
        ));
        let storage_path = match storage_path {
            Ok(storage_path) => storage_path,
            Err(error)
                if self
                    .spec
                    .transcript_capture
                    .mode
                    .rejects_launch_when_unavailable() =>
            {
                return Err(AgentRunTranscriptCaptureError::Path(error));
            }
            Err(_) => {
                self.terminal_launch_intent.transcript_writer_config = None;
                self.transcript_storage_path = None;
                return Ok(());
            }
        };

        self.terminal_launch_intent.transcript_writer_config = Some(TranscriptWriterConfig::new(
            storage_path.clone(),
            self.spec.transcript_capture.retention_limits,
            self.spec.transcript_capture.mode,
        ));
        self.transcript_storage_path = Some(storage_path);
        Ok(())
    }

    /// RFC-049 D4′: whether this plan's capture was disabled because a budget
    /// was still exhausted at preflight — so the run can record **why** it has
    /// no transcript, rather than leaving the absence unexplained.
    pub(crate) fn transcript_budget_exhausted(&self) -> bool {
        self.spec.transcript_capture.budget_exhausted
    }

    pub(crate) fn transcript_storage_path(&self) -> Option<&TranscriptStoragePath> {
        self.transcript_storage_path.as_ref()
    }

    pub(crate) fn transition_agent_run_to(
        &mut self,
        status: AgentRunStatus,
    ) -> Result<(), AgentRunTransitionError> {
        self.agent_run.transition_to(status)
    }

    pub(crate) fn into_parts(self) -> (AgentRunLaunchSpec, AgentRun, AgentTerminalLaunchIntent) {
        (self.spec, self.agent_run, self.terminal_launch_intent)
    }
}

impl AgentRunLaunchSpec {
    pub fn project_id(&self) -> &ProjectId {
        &self.project_id
    }

    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    pub fn executable_path(&self) -> &Path {
        &self.executable_path
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn executable_provenance(&self) -> AiCliExecutableProvenance {
        self.executable_provenance
    }

    pub fn cwd(&self) -> &Path {
        self.cwd.as_path()
    }

    pub fn verified_cwd(&self) -> &VerifiedCwd {
        &self.cwd
    }

    pub fn compatibility_level(&self) -> AgentCompatibilityLevel {
        self.compatibility_level
    }

    pub fn prompt_summary(&self) -> &str {
        &self.prompt_summary
    }

    pub fn environment_summary(&self) -> &AgentLaunchSummary {
        &self.environment_summary
    }

    pub fn terminal_environment_policy(&self) -> &TerminalEnvironmentPolicy {
        &self.terminal_environment_policy
    }

    pub fn workspace_discovery_summary(&self) -> &AgentLaunchSummary {
        &self.workspace_discovery_summary
    }

    pub fn transcript_capture(&self) -> &AgentRunTranscriptCapture {
        &self.transcript_capture
    }

    pub fn approval_state_root(&self) -> Option<&Path> {
        self.approval_state_root.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunTranscriptCapture {
    pub mode: TranscriptCaptureMode,
    pub state_root: Option<PathBuf>,
    pub retention_limits: TranscriptRetentionLimits,
    /// RFC-049 D4′, carried from the request: the budget was still exhausted
    /// after preflight cleanup. See `AgentRunLaunchRequest`'s own field.
    pub budget_exhausted: bool,
}

impl AgentRunTranscriptCapture {
    pub fn capture_policy(&self) -> TranscriptCapturePolicy {
        match self.mode {
            TranscriptCaptureMode::Disabled => TranscriptCapturePolicy::metadata_only(),
            TranscriptCaptureMode::LocalBounded => {
                TranscriptCapturePolicy::local_bounded_agent_run_default()
                    .with_limits(self.retention_limits)
            }
            TranscriptCaptureMode::RequiredLocalBounded => {
                TranscriptCapturePolicy::required_local_bounded(self.retention_limits)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentRunTranscriptCaptureError {
    StateRootMissing,
    PolicyDoesNotPermitBytes,
    /// RFC-049 D4′: a `RequiredLocalBounded` launch whose budget is exhausted.
    /// Validation refuses this before preparation is reached; see
    /// `prepare_transcript_capture`.
    BudgetExhausted,
    Path(TranscriptPathError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentRunLaunchValidationError {
    CrossProject,
    WrongProfile,
    MissingProjectRoot {
        summary: AgentLaunchSummary,
    },
    InvalidCwd {
        summary: AgentLaunchSummary,
    },
    CwdEscapesProjectRoot {
        summary: AgentLaunchSummary,
    },
    WorkspaceLocalProfileBlocked,
    WorkspaceLocalPromptBlocked,
    WorkspaceLocalEnvironmentBlocked,
    WorkspaceLocalExecutableBlocked {
        path: PathBuf,
    },
    ProjectLocalPathLookupBlocked {
        path: PathBuf,
    },
    ExecutableUnavailable {
        summary: AgentLaunchSummary,
    },
    WorkspaceDiscoveryBlocked {
        summary: AgentLaunchSummary,
    },
    MissingWorkspaceDiscoveryEvidence,
    ManagedCapabilityMissing,
    RequiredTranscriptStateRootMissing,
    RequiredTranscriptPolicyDoesNotPermitBytes,
    /// RFC-049 D4′ / RFC-011: a `RequiredLocalBounded` launch whose byte budget
    /// is still exhausted after cleanup. **No process starts.** Unreachable from
    /// the product — nothing in it requests this mode — and deliberately kept
    /// so: RFC-011 says such a workflow must be reviewed before it exists.
    RequiredTranscriptBudgetExhausted,
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

/// RFC-077 Stage 8, D5 move 2: the exact surface `AgentRunLaunchValidator::
/// validate` needs from a project -- its id, its trust state, and its
/// canonical root -- as a trait this module defines rather than naming
/// `project::ProjectSession` directly. Implemented on `ProjectSession`
/// itself, inside `project/session.rs`: `project` already has an edge to
/// `agent` (that file's own existing `use crate::agent::...`), so placing
/// the impl there adds no new edge, the same placement rule Stage 5's
/// `CommandAuditRecorder` established for `audit::AuditCoordinator`.
pub trait AgentRunLaunchProject {
    fn id(&self) -> &ProjectId;
    fn trust_state(&self) -> WorkspaceTrust;
    fn canonical_root_path(&self) -> &Path;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AgentRunLaunchValidator;

impl AgentRunLaunchValidator {
    pub fn validate(
        self,
        project: &impl AgentRunLaunchProject,
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
        validate_transcript_policy(request)?;

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
        // Wrapped only now, immediately after the containment check
        // above passes -- `VerifiedCwd` exists specifically so a value of
        // this type can only mean "canonicalized and confirmed to start
        // with the project's canonical root," not merely "some path."
        let cwd = VerifiedCwd::from_validated(cwd);

        let (executable_path, executable_provenance) =
            resolve_executable(profile, &root, restricted)?;

        Ok(AgentRunLaunchValidation {
            project_id: request.project_id.clone(),
            profile_id: profile.id.clone(),
            project_root: root.clone(),
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
            transcript_capture: AgentRunTranscriptCapture {
                mode: request.transcript_capture_mode,
                state_root: request.transcript_state_root.clone(),
                retention_limits: request.transcript_retention_limits,
                budget_exhausted: request.transcript_budget_exhausted,
            },
            approval_state_root: request.approval_state_root.clone(),
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

fn validate_transcript_policy(
    request: &AgentRunLaunchRequest,
) -> Result<(), AgentRunLaunchValidationError> {
    if !request
        .transcript_capture_mode
        .rejects_launch_when_unavailable()
    {
        return Ok(());
    }
    if request.transcript_state_root.is_none() {
        return Err(AgentRunLaunchValidationError::RequiredTranscriptStateRootMissing);
    }
    // RFC-049 D4′ / RFC-011: *"`RequiredLocalBounded` should reject launch or
    // fail preflight before process start."* An exhausted budget is exactly
    // that case — the transcript this mode requires cannot be written, and the
    // alternative (freeing bytes by deleting a live writer's file) is what §2
    // forbids at any pressure.
    if request.transcript_budget_exhausted {
        return Err(AgentRunLaunchValidationError::RequiredTranscriptBudgetExhausted);
    }
    let capture = AgentRunTranscriptCapture {
        mode: request.transcript_capture_mode,
        state_root: request.transcript_state_root.clone(),
        retention_limits: request.transcript_retention_limits,
        budget_exhausted: request.transcript_budget_exhausted,
    };
    if !capture
        .capture_policy()
        .permits_transcript_byte_persistence()
    {
        return Err(AgentRunLaunchValidationError::RequiredTranscriptPolicyDoesNotPermitBytes);
    }
    Ok(())
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
