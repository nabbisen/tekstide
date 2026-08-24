//! The one place that turns `KeybindingPolicy` into text a user can
//! read, shared by the Project Board's empty state and `--help`.
//!
//! **Why this module exists at all.** Until `0.12.1` the shipped
//! application named no shortcut anywhere: the string `Ctrl` appeared
//! zero times in the locale catalogue while nine bindings were live, so
//! every capability the product had was reachable only by a user who had
//! read `navigation.rs`. The README carried a keyboard reference; the
//! running program did not.
//!
//! **Why it is derived, not written.** The list comes from
//! `KeybindingPolicy::advertised_bindings()`, which filters on rule
//! status, so help text cannot drift from the policy the input layer
//! actually dispatches on -- and cannot advertise the four
//! `Configurable`-with-no-binding actions that are dead rather than
//! pending. A second hand-maintained list is precisely the
//! state-asserting text `ARCHITECTURE.md` records this project failing
//! to keep current three times in one folder migration.
//!
//! The action -> catalog-key match below is exhaustive on purpose: a new
//! `NavigationAction` fails to compile here until someone decides
//! whether it is user-visible and what it is called, the same
//! make-it-unrepresentable discipline the theme's contrast destructure
//! uses.

use tekstide_core::navigation::{KeybindingPolicy, NavigationAction};

use crate::i18n::Catalog;

/// One row of help: the binding as the policy spells it, and the
/// already-localized description of what it does.
pub struct KeyboardHelpLine {
    /// A `&'static str` straight from the policy (`"Ctrl+Alt+P"`) --
    /// trusted, fixed-set text, never filesystem-derived, so it is not
    /// routed through `text_safety::quote_untrusted`.
    pub binding: &'static str,
    pub description: String,
}

/// The catalog key describing each user-visible action.
///
/// Exhaustive by design -- see the module doc. The four actions that
/// return `None` are the ones with no binding today; they are matched
/// explicitly rather than swept up by a wildcard so that giving one a
/// binding later forces a decision here instead of silently producing an
/// undescribed row.
fn action_catalog_key(action: NavigationAction) -> Option<&'static str> {
    match action {
        NavigationAction::OpenProjectBoard => Some("keyboard-help-open-project-board"),
        NavigationAction::OpenProjectEntryField => Some("keyboard-help-open-project-entry-field"),
        NavigationAction::ToggleProjectMode => Some("keyboard-help-toggle-project-mode"),
        NavigationAction::LaunchTerminal => Some("keyboard-help-launch-terminal"),
        NavigationAction::PasteIntoTerminal => Some("keyboard-help-paste-into-terminal"),
        NavigationAction::SaveActiveDocument => Some("keyboard-help-save-active-document"),
        NavigationAction::LaunchAgentRun => Some("keyboard-help-launch-agent-run"),
        NavigationAction::OpenCurrentAgentRunDetail => {
            Some("keyboard-help-open-current-agent-run-detail")
        }
        NavigationAction::OpenApprovalHistory => Some("keyboard-help-open-approval-history"),
        NavigationAction::OpenTrustSettings => Some("keyboard-help-open-trust-settings"),
        NavigationAction::OpenHelp => Some("keyboard-help-open-help"),
        NavigationAction::OpenFolderBrowser => Some("keyboard-help-open-folder-browser"),
        NavigationAction::SwitchActiveProject
        | NavigationAction::CycleVisibleTerminalSession
        | NavigationAction::OpenDiffReview
        | NavigationAction::OpenSafeCloseDialog
        | NavigationAction::OpenCommandPalette => None,
    }
}

/// Every live binding, described. Order follows the policy's own rule
/// order rather than being re-sorted here, so the help reads in the
/// order the policy declares.
pub fn keyboard_help_lines(catalog: &Catalog) -> Vec<KeyboardHelpLine> {
    KeybindingPolicy::linux_mvp()
        .advertised_bindings()
        .into_iter()
        .filter_map(|(action, binding)| {
            action_catalog_key(action).map(|key| KeyboardHelpLine {
                binding,
                description: catalog.get(key),
            })
        })
        .collect()
}

/// The `--help` text, built from the same lines the GUI renders.
///
/// English rather than catalog-driven, and that is a real limitation
/// stated rather than hidden: argument parsing happens before
/// `Catalog::resolve` in `boot()`, and reordering boot to localize usage
/// text was more change than a correction release should carry. The
/// *shortcut descriptions* below are catalog-driven, so only the framing
/// sentences are fixed English. Recorded in RFC-038.
pub fn usage_text(catalog: &Catalog, executable: &str) -> String {
    let mut out = String::new();
    out.push_str("tekstide -- a local-first, multi-project AI CLI development workbench\n\n");
    out.push_str("USAGE:\n");
    out.push_str(&format!("    {executable} [PROJECT_PATH]...\n\n"));
    out.push_str("Opens each PROJECT_PATH as a project. With no path, the Project Board\n");
    out.push_str("opens empty, with a field to type or paste one (Ctrl+Alt+O opens the\n");
    out.push_str("same field once a project is already open).\n\n");
    out.push_str("OPTIONS:\n");
    out.push_str("    -h, --help       Print this help\n");
    out.push_str("    -V, --version    Print version\n\n");
    out.push_str("KEYBOARD:\n");
    for line in keyboard_help_lines(catalog) {
        out.push_str(&format!("    {:<14} {}\n", line.binding, line.description));
    }
    out
}

#[cfg(test)]
mod tests;
