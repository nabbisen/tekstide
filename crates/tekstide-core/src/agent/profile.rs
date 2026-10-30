use std::ffi::OsStr;
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

    /// RFC-028 D3: the single host-dispatch point every production caller
    /// should use instead of naming `claude_code_linux_default`/
    /// `claude_code_macos_default` directly -- the same convention stated
    /// once on [`crate::project::recent::AppStatePathProvider::default_for_host`]:
    /// a whole-function `#[cfg(target_os = "...")]` split per platform, never
    /// a blanket `#[cfg(unix)]`, and a hard compile error for anything this
    /// split does not name. Found as this convention's own "third kind" at
    /// review 585: unlike the five `linux_mvp()` labels elsewhere in this
    /// crate, this constructor is genuinely platform-coupled -- the lookup
    /// paths below differ by platform -- and was not dispatched at all until
    /// PR-028-F closed that gap. See [`Self::claude_code_linux_from_env`]/
    /// [`Self::claude_code_macos_from_env`] below for the rest of what this
    /// profile is.
    #[cfg(target_os = "linux")]
    pub fn default_for_host() -> Self {
        Self::claude_code_linux_default()
    }

    /// See [`Self::default_for_host`] above for the convention this
    /// implements.
    #[cfg(target_os = "macos")]
    pub fn default_for_host() -> Self {
        Self::claude_code_macos_default()
    }

    /// RFC-022 PR-022-D: the first real, code-defined profile -- see
    /// response 218 (review request 218's answer). Not the reference
    /// adapter: that stays test-only (`what-the-dialog-must-not-lie-about.md`
    /// §4). This profile points at a genuinely installed AI CLI (Claude
    /// Code) at `Supervised` compatibility, which needs no adapter
    /// protocol -- `Managed`/command-approval remains reachable only
    /// through the reference adapter, per response 218's traced
    /// consequence, which this profile does not attempt to change.
    ///
    /// `workspace_discovery_policy` is `MayDiscoverWorkspaceFiles`, not
    /// `NoKnownWorkspaceDiscovery`: Claude Code genuinely reads project
    /// files as part of normal operation, and claiming otherwise would be
    /// the same kind of dishonest disclosure this project's dialogs are
    /// built to avoid. The honest consequence is that a launch through
    /// this profile is correctly refused in a Restricted (untrusted,
    /// default) project until the user grants trust -- not a bug to route
    /// around.
    ///
    /// RFC-023 PR-023-F response 281: `source` is `BuiltIn`, not
    /// `UserGlobal` -- corrected here rather than left as the mislabel
    /// PR-023-E's own review found. This profile is compiled into the
    /// binary, not read from a file; `UserGlobal` is reserved for what
    /// `config::to_ai_cli_profile` actually produces (RFC-023's own
    /// wording: "a configuration-defined profile is an
    /// `AiCliProfileSource::UserGlobal` profile"), and OQ3's still-open
    /// confirm-on-first-use question needs exactly this distinction to
    /// gate on once a config-defined profile can reach a real launch --
    /// see `docs`/handoff for why this alone does not build that gate.
    /// Verified behaviourally inert today: `AgentRunLaunchValidator`
    /// reads `profile.source` at exactly two sites
    /// (`validate_profile_source`'s `WorkspaceLocal` check;
    /// `validate_workspace_discovery_policy`'s `BuiltIn | UserGlobal`
    /// grouping), and both already treat `BuiltIn` and `UserGlobal`
    /// identically, so this changes no observable launch behaviour.
    pub fn claude_code_linux_default() -> Self {
        Self::claude_code_linux_from_env(std::env::var_os("HOME"))
    }

    /// `$HOME/.local/bin` (the real install location confirmed on the
    /// reference dev machine), then the reviewed system directories.
    pub fn claude_code_linux_from_env(home: Option<impl AsRef<OsStr>>) -> Self {
        let mut lookup_paths = Vec::new();
        if let Some(home) = home.filter(|value| !value.as_ref().is_empty()) {
            lookup_paths.push(ExecutableLookupPath::reviewed_system(
                PathBuf::from(home.as_ref()).join(".local/bin"),
            ));
        }
        lookup_paths.push(ExecutableLookupPath::reviewed_system("/usr/local/bin"));
        lookup_paths.push(ExecutableLookupPath::reviewed_system("/usr/bin"));
        Self::claude_code_profile(lookup_paths)
    }

    pub fn claude_code_macos_default() -> Self {
        Self::claude_code_macos_from_env(std::env::var_os("HOME"))
    }

    /// RFC-028 D8: `$HOME/.local/bin`, then `/opt/homebrew/bin` (Apple
    /// Silicon Homebrew) ahead of the two reviewed system directories
    /// already shared with Linux. A GUI-launched macOS app does not
    /// inherit the user's interactive shell `PATH` at all -- the normal
    /// way a user reaches this product on that platform -- so without this
    /// entry the lookup fails for the common case, not an edge one.
    pub fn claude_code_macos_from_env(home: Option<impl AsRef<OsStr>>) -> Self {
        let mut lookup_paths = Vec::new();
        if let Some(home) = home.filter(|value| !value.as_ref().is_empty()) {
            lookup_paths.push(ExecutableLookupPath::reviewed_system(
                PathBuf::from(home.as_ref()).join(".local/bin"),
            ));
        }
        lookup_paths.push(ExecutableLookupPath::reviewed_system("/opt/homebrew/bin"));
        lookup_paths.push(ExecutableLookupPath::reviewed_system("/usr/local/bin"));
        lookup_paths.push(ExecutableLookupPath::reviewed_system("/usr/bin"));
        Self::claude_code_profile(lookup_paths)
    }

    fn claude_code_profile(lookup_paths: Vec<ExecutableLookupPath>) -> Self {
        let mut profile = Self::new(
            "claude-code",
            "Claude Code",
            AiCliProfileSource::BuiltIn,
            AiCliExecutable::PathLookup {
                command: "claude".to_owned(),
                lookup_paths,
                provenance: AiCliExecutableProvenance::UserGlobal,
            },
            AgentCompatibilityLevel::Supervised,
        );
        profile.workspace_discovery_policy =
            AiCliWorkspaceDiscoveryPolicy::MayDiscoverWorkspaceFiles {
                summary:
                    "Claude Code reads files in the project workspace as part of normal operation"
                        .to_owned(),
            };
        profile
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!(
    "AiCliProfile::default_for_host() has no arm for this target_os -- RFC-028 D3's own \
     convention requires a dispatch arm added deliberately for a new platform, not a \
     fall-through to a Unix-shaped default that may not hold."
);

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
