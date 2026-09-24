mod appearance;
mod load;
mod model;
mod path;
mod profile;
mod sensitive;
mod terminal;

pub use appearance::{
    CONTRAST_RULES, Colour, ColourError, ContrastFor, ContrastRatio, FamilyError, FontSettings,
    FontSizes, MAX_FONT_FAMILY_CHARS, MAX_FONT_SIZE_PX, MAX_SCRIM_ALPHA, MIN_FOCUS_CONTRAST,
    MIN_FONT_SIZE_PX, MIN_TEXT_CONTRAST, Palette, ThemeRole, ThemeSettings, contrast_ratio,
    enforce_contrast, parse_family, relative_luminance,
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
pub use terminal::{
    DEFAULT_SCROLLBACK_LINES, MAX_SCROLLBACK_LINES, SCROLLBACK_BLOCK_SLACK,
    SCROLLBACK_BUDGET_BYTES, SCROLLBACK_BYTES_PER_CELL, SCROLLBACK_FIXED_BYTES_PER_COLUMN,
    SCROLLBACK_ROW_BLOCK, TerminalSettings, effective_scrollback_lines, scrollback_bytes,
};

#[cfg(test)]
mod tests;
