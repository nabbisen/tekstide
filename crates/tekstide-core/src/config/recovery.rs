//! **RFC-027 D15.** `[recovery] persist_unsaved_buffers`: whether a dirty document's own
//! text, cursor and viewport are written to a recovery record so a crash can offer them
//! back. **Default on** -- a protection a user must first discover does not prevent the
//! loss this RFC exists to prevent, and the honesty obligations D10 requires (the
//! *Retained locally* figure, the purge, this very setting) are what make default-on
//! defensible, not separable from it.

use super::model::{FallbackReason, SettingFallback};

/// `[recovery]`: what survived. The default **is** to persist unsaved buffers --
/// `#[derive(Default)]` would give `false`, so `Default` is implemented by hand.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoverySettings {
    pub persist_unsaved_buffers: bool,
}

impl Default for RecoverySettings {
    fn default() -> Self {
        Self {
            persist_unsaved_buffers: true,
        }
    }
}

/// `persist_unsaved_buffers` from the `[recovery]` table. Anything but a boolean falls
/// back to the default with a diagnostic naming the setting; every other key in the
/// section is the caller's unknown-key warning -- the same shape `take_show_ignored`
/// already uses.
pub(super) fn take_persist_unsaved_buffers(
    table: &mut toml::Table,
    fallbacks: &mut Vec<SettingFallback>,
) -> RecoverySettings {
    match table.remove("persist_unsaved_buffers") {
        None => RecoverySettings::default(),
        Some(toml::Value::Boolean(persist_unsaved_buffers)) => RecoverySettings {
            persist_unsaved_buffers,
        },
        Some(_) => {
            fallbacks.push(SettingFallback {
                setting: "recovery.persist_unsaved_buffers".to_owned(),
                reason: FallbackReason::NotABoolean,
            });
            RecoverySettings::default()
        }
    }
}
