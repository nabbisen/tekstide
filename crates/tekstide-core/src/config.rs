mod appearance;
mod load;
mod model;
mod path;
mod profile;
mod sensitive;

pub use appearance::{
    Colour, ColourError, ContrastRatio, FamilyError, FontSettings, FontSizes,
    MAX_FONT_FAMILY_CHARS, MAX_FONT_SIZE_PX, MIN_FONT_SIZE_PX, MIN_TEXT_CONTRAST, Palette,
    TEXT_PAIRS, ThemeRole, ThemeSettings, contrast_ratio, enforce_text_contrast, parse_family,
    relative_luminance,
};
pub use load::{
    ConfigDiagnostic, ConfigLoadOutcome, ConfigLoadReport, ConfigReloadOutcome, ConfigStore,
    ConfigWarning, parse_and_validate,
};
pub use model::{
    AgentSettings, ConfigurationDocument, ConfiguredAiCliProfile, FallbackReason,
    KeybindingSettings, ResourceSettings, SettingFallback,
};
pub use path::{
    ConfigPathError, ConfigPathErrorReason, ConfigPathProvider, ConfigPathResolver,
    ConfigStoragePath,
};
pub use profile::to_ai_cli_profile;
pub use sensitive::{
    SecuritySensitiveDirection, SecuritySensitiveField, apply_safe_fields, direction,
    security_sensitive_diff,
};

#[cfg(test)]
mod tests;
