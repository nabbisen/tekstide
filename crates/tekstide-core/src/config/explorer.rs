//! **RFC-055 PR-055-C.** `[explorer] show_ignored`: whether the file explorer
//! draws the entries git says are ignored.
//!
//! One setting, and it governs **ignored entries only** (D7). Dotfiles are not
//! hidden by anything, and this does not begin to hide them: `.gitignore`,
//! `.env` and `.git-exclude` are precisely the files a user opens when deciding
//! what an AI agent can read, and they stay ordinary rows whichever way this is
//! set. `REQ-FILE-006` says ignored entries "may be shown through an explicit
//! user setting"; this is that setting, default off.

use super::model::{FallbackReason, SettingFallback};

/// `[explorer]`: what survived. The default is **not** to show ignored entries.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExplorerSettings {
    pub show_ignored: bool,
}

/// `show_ignored` from the `[explorer]` table. Anything but a boolean falls
/// back to the default with a diagnostic naming the setting; every other key in
/// the section is the caller's unknown-key warning.
pub(super) fn take_show_ignored(
    table: &mut toml::Table,
    fallbacks: &mut Vec<SettingFallback>,
) -> ExplorerSettings {
    match table.remove("show_ignored") {
        None => ExplorerSettings::default(),
        Some(toml::Value::Boolean(show_ignored)) => ExplorerSettings { show_ignored },
        Some(_) => {
            fallbacks.push(SettingFallback {
                setting: "explorer.show_ignored".to_owned(),
                reason: FallbackReason::NotABoolean,
            });
            ExplorerSettings::default()
        }
    }
}
