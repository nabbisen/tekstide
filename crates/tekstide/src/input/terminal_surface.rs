//! RFC-015 PR-015-C: `TextStream`'s home. RFC-017 has not landed --
//! there is no real terminal surface in this tree yet -- but the
//! *privacy boundary* `pr-015-c-input-routing.md` requires must exist
//! now, ahead of that surface, the same way PR-015-B's i18n/theme seams
//! existed with working machinery before any real caller used them.
//!
//! **`TextStream::from_terminal_key` is `pub(super)`, not `pub(crate)`.**
//! `pub(crate)` would let *any* module in this crate construct a
//! `TextStream` -- including a future `crates/tekstide/src/surface/*`
//! module, which is exactly the bypass this type exists to prevent.
//! `pub(super)` restricts construction to `super::input` (the router,
//! which legitimately hands a key event to the terminal surface once it
//! has decided a terminal owns the input sink) and `input`'s other
//! descendants (this module's own `#[cfg(test)]` code); nothing outside
//! `input` -- `shell.rs`, `main.rs`, or any surface module -- can reach
//! it, proven in `input::tests`.

use tekstide_core::domain::TerminalId;

use super::KeyPress;

/// Keystrokes destined for a PTY. Carries the target terminal so the
/// router (which does have access to `ApplicationShell` state) can
/// verify it names a live terminal in the active project before
/// delivering it -- a stale or cross-project id must be dropped, not
/// best-effort delivered.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStream {
    target: TerminalId,
    key: KeyPress,
}

impl TextStream {
    pub fn target(&self) -> &TerminalId {
        &self.target
    }

    /// The one and only constructor. See the module doc for why this is
    /// `pub(super)` rather than `pub(crate)`.
    pub(super) fn from_terminal_key(target: TerminalId, key: KeyPress) -> Self {
        Self { target, key }
    }
}
