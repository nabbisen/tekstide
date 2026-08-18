mod load;
mod model;
mod path;

pub use load::{
    ConfigDiagnostic, ConfigLoadOutcome, ConfigLoadReport, ConfigStore, ConfigWarning,
    parse_and_validate,
};
pub use model::{
    AgentSettings, ConfigurationDocument, ConfiguredAiCliProfile, CoreSettings, KeybindingSettings,
    ProjectSettings, ResourceSettings, RestrictedDefaultTrust, SecuritySettings, TerminalSettings,
    UiSettings,
};
pub use path::{
    ConfigPathError, ConfigPathErrorReason, ConfigPathProvider, ConfigPathResolver,
    ConfigStoragePath,
};

#[cfg(test)]
mod tests;
