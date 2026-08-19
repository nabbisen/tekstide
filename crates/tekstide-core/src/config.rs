mod load;
mod model;
mod path;
mod sensitive;

pub use load::{
    ConfigDiagnostic, ConfigLoadOutcome, ConfigLoadReport, ConfigReloadOutcome, ConfigStore,
    ConfigWarning, parse_and_validate,
};
pub use model::{
    AgentSettings, ConfigurationDocument, ConfiguredAiCliProfile, CoreSettings, KeybindingSettings,
    ProjectSettings, RequiredDestructiveCommandApproval, RequiredMultilinePasteConfirmation,
    ResourceSettings, RestrictedDefaultTrust, SecuritySettings, TerminalSettings, UiSettings,
};
pub use path::{
    ConfigPathError, ConfigPathErrorReason, ConfigPathProvider, ConfigPathResolver,
    ConfigStoragePath,
};
pub use sensitive::{
    SecuritySensitiveDirection, SecuritySensitiveField, apply_safe_fields, direction,
    security_sensitive_diff,
};

#[cfg(test)]
mod tests;
