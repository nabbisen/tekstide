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
        NavigationAction::SwitchActiveProject => Some("keyboard-help-switch-active-project"),
        NavigationAction::OpenDiffReview => Some("keyboard-help-open-diff-review"),
        NavigationAction::CycleVisibleTerminalSession
        | NavigationAction::OpenSafeCloseDialog
        | NavigationAction::OpenCommandPalette => None,
    }
}

/// RFC-040 PR-040-A, D1: whether a live action is answered by a real,
/// visible, mouse-clickable control, or is a deliberate keyboard-only
/// convention -- never neither. `VisibleControl`'s `on_press_snippet` is
/// not decoration: `shell::tests::every_live_action_has_a_visible_control_or_a_reasoned_allow_list_entry`
/// greps the whole crate for it, so this variant asserts a real, current
/// fact about the source, not a claim that could go stale the moment
/// the button it describes is removed.
///
/// `#[cfg(test)]`: this is audit/test infrastructure, per PR-040-A's own
/// "build nothing [production], make the audit a test" scope -- unlike
/// [`action_catalog_key`] above, which a real production caller
/// (`keyboard_help_lines`) uses, nothing outside this crate's own test
/// suite has a reason to ask "does this action have a visible control."
#[cfg(test)]
pub(crate) enum ControlCoverage {
    VisibleControl {
        /// What a person sees, for a citation in this module's own
        /// tests -- not itself verified; `on_press_snippet` is.
        description: &'static str,
        /// A literal substring `shell.rs` or `surface::board`'s source
        /// must contain -- the exact `.on_press(Message::Variant`
        /// dispatch the description above names.
        on_press_snippet: &'static str,
    },
    /// A stated reason, not just a name -- D1's own requirement.
    /// `Permanent` entries are deliberate, lasting exceptions
    /// (`PasteIntoTerminal`'s D3 convention, `OpenProjectEntryField`'s
    /// workflow-already-served-elsewhere reading); `TrackedGap` entries
    /// are honest that nothing exists yet and name the slice that closes
    /// them, so this table can be edited incrementally without ever
    /// making the crate's own gates fail -- the measurement PR-040-C's
    /// own evidence cites is this table's composition changing, not this
    /// test going red and green again.
    KeyboardOnly(&'static str),
    /// RFC-044 D3: the mirror `KeyboardOnly` never had, because
    /// `control_coverage`'s own domain ([`NavigationAction`]) could not
    /// represent it -- every entry there is required to carry a live
    /// `KeybindingPolicy` rule, so "no keyboard route" was not merely
    /// unnoticed, it was inexpressible. Reachable now that
    /// [`SurfaceAction`]'s own coverage function
    /// ([`surface_keyboard_coverage`]) asks the opposite question
    /// (`control_coverage` still only asks "does a mouse reach this,"
    /// unchanged). Requires a reason exactly as `KeyboardOnly` already
    /// does, and the same `TrackedGap`/`Permanent` vocabulary applies:
    /// a `TrackedGap` names the slice that closes it (this RFC's own
    /// PR-044-B); a `Permanent` one is a real, considered decision that
    /// a key would cost more than it is worth (§6 of
    /// `what-advertising-keys-must-not-become.md`), not a place to park
    /// a gap nobody had to justify.
    ///
    /// `#[allow(dead_code)]`: PR-044-B closed the one entry that used to
    /// construct this (`TabStripCloseProject`), so nothing currently
    /// does. Left in the type rather than removed -- §6 of
    /// `what-advertising-keys-must-not-become.md` explicitly anticipates
    /// a future, legitimate mouse-only control needing exactly this
    /// arm, and removing a vocabulary word the moment its one current
    /// user goes away would make the *next* gap someone's job to
    /// reinvent rather than reach for.
    #[allow(dead_code)]
    MouseOnly { reason: &'static str },
}

/// Exhaustive by design, the same discipline [`action_catalog_key`]
/// above already uses and for the same reason: a new `NavigationAction`
/// fails to compile here until someone decides which camp it is in.
/// `None` for the four dead/reserved actions -- [`action_catalog_key`]'s
/// own `None` arm -- since an action with no live route has no control
/// to have.
#[cfg(test)]
pub(crate) fn control_coverage(action: NavigationAction) -> Option<ControlCoverage> {
    match action {
        NavigationAction::OpenProjectBoard => Some(ControlCoverage::VisibleControl {
            description: "the \"Projects\" tab (project_tab_strip, RFC-039 PR-039-A/B)",
            on_press_snippet: ".on_press(Message::GoToProjectBoardTabPressed)",
        }),
        NavigationAction::SwitchActiveProject => Some(ControlCoverage::VisibleControl {
            description: "each open project's own tab (project_tab_strip, RFC-039 PR-039-B)",
            on_press_snippet: ".on_press(Message::SwitchActiveProjectTabPressed",
        }),
        NavigationAction::OpenFolderBrowser => Some(ControlCoverage::VisibleControl {
            description: "the Project Board's \"Browse...\" button (RFC-038 PR-038-G)",
            on_press_snippet: "Message::OpenFolderBrowserButtonPressed",
        }),
        NavigationAction::PasteIntoTerminal => Some(ControlCoverage::KeyboardOnly(
            "D3: terminals conventionally paste by keyboard; a paste button on a terminal grid \
             would confuse more than help. Permanent.",
        )),
        NavigationAction::OpenProjectEntryField => Some(ControlCoverage::KeyboardOnly(
            "the underlying workflow (add a project) already has a visible control -- \
             OpenFolderBrowser's own Browse button. This keystroke is an accelerator to an \
             alternate input path for that same workflow, not a workflow with no visible \
             route. Permanent.",
        )),
        NavigationAction::ToggleProjectMode => Some(ControlCoverage::VisibleControl {
            description: "the workspace's own mode-toggle row (mode_toggle_row, RFC-040 PR-040-C)",
            on_press_snippet: ".on_press(Message::ToggleProjectModeButtonPressed)",
        }),
        NavigationAction::LaunchTerminal => Some(ControlCoverage::VisibleControl {
            description: "Terminal Immersion's own \"+ New Terminal\" button \
                          (launch_terminal_button, RFC-040 PR-040-C)",
            on_press_snippet: ".on_press(Message::LaunchTerminalButtonPressed)",
        }),
        // Not `.on_press(Message::SaveActiveDocumentButtonPressed` -- the
        // editor surface's own `view` is generic over `Message` (the same
        // "surface renders, shell.rs supplies the message" split
        // `board::empty_state_view` already uses), so the real button
        // lives in `surface::editor::view` while the concrete message is
        // only named at `content_mode_editor_view`'s own call site. The
        // snippet below is that call site's own argument, which
        // disappears exactly when the wiring does.
        NavigationAction::SaveActiveDocument => Some(ControlCoverage::VisibleControl {
            description: "the editor's own \"Save\" button (surface::editor::view, RFC-040 PR-040-C)",
            on_press_snippet: "Message::SaveActiveDocumentButtonPressed,",
        }),
        NavigationAction::LaunchAgentRun => Some(ControlCoverage::VisibleControl {
            description: "TrustSettings's own \"Launch AI CLI Run\" button (RFC-040 PR-040-C)",
            on_press_snippet: ".on_press(Message::LaunchAgentRunButtonPressed)",
        }),
        NavigationAction::OpenCurrentAgentRunDetail => Some(ControlCoverage::VisibleControl {
            description: "TrustSettings's own \"AgentRun Report\" button (RFC-040 PR-040-C)",
            on_press_snippet: ".on_press(Message::OpenCurrentAgentRunDetailButtonPressed)",
        }),
        NavigationAction::OpenApprovalHistory => Some(ControlCoverage::VisibleControl {
            description: "TrustSettings's own \"Approval History\" button (RFC-040 PR-040-C)",
            on_press_snippet: ".on_press(Message::OpenApprovalHistoryButtonPressed)",
        }),
        NavigationAction::OpenTrustSettings => Some(ControlCoverage::VisibleControl {
            description: "the top bar's own \"Trust Settings\" button (top_bar_actions_row, \
                          RFC-040 PR-040-C)",
            on_press_snippet: ".on_press(Message::OpenTrustSettingsButtonPressed)",
        }),
        NavigationAction::OpenHelp => Some(ControlCoverage::VisibleControl {
            description: "the top bar's own \"?\" button (top_bar_actions_row, RFC-040 PR-040-C)",
            on_press_snippet: ".on_press(Message::OpenHelpButtonPressed)",
        }),
        NavigationAction::OpenDiffReview => Some(ControlCoverage::VisibleControl {
            description: "TrustSettings's own \"Change Review\" button (RFC-020, the change \
                          review surface)",
            on_press_snippet: ".on_press(Message::OpenDiffReviewButtonPressed)",
        }),
        NavigationAction::CycleVisibleTerminalSession
        | NavigationAction::OpenSafeCloseDialog
        | NavigationAction::OpenCommandPalette => None,
    }
}

/// RFC-044 D1: a **surface-local** action -- something a user can do
/// on one specific surface, deliberately keyed by (surface, action) so
/// `Enter` meaning six different things on six surfaces is six
/// variants, not one collision (the surface is encoded in the variant
/// name/grouping below, mirroring how [`NavigationAction`] itself has
/// no separate "surface" field either -- its own domain is implicitly
/// "global").
///
/// **Wider than [`NavigationAction`] on purpose, and that widening is
/// D1's whole substance.** Every `NavigationAction` is required to
/// carry a live [`KeybindingPolicy`] rule to exist in that enum's
/// domain at all -- so `control_coverage`'s own exhaustive match could
/// never represent an action with *no* keyboard route: the action
/// would simply not be a `NavigationAction` yet. Closing a project is
/// the proof -- it has no `NavigationAction` variant and no
/// `KeybindingPolicy` rule, which is exactly why `0.15.0`'s release
/// gate found the gap by accident rather than by anything mechanical.
/// `SurfaceAction` carries no such guarantee, so it can name an action
/// that has no keyboard route today.
///
/// **Scope, stated rather than left implicit**: covers the seven of
/// eight surface-local handlers whose keys perform a *discrete action*
/// (open, activate, mark, close, submit, dismiss) -- not
/// `handle_editor_key`, whose own keys are continuous text-editing
/// input, not a fixed, nameable set of actions the way a button press
/// is. Arrow-key row highlighting is also excluded from every handler
/// that has it: it is a keyboard-native concept with no mouse
/// equivalent to lack (there is nothing for `MouseOnly` to say about
/// "move the highlight without selecting"), so it is not the kind of
/// gap this RFC exists to close.
///
/// **A third exclusion, the largest of the three, response 349's own
/// required addition: modal buttons.** Seven `ModalContent` variants
/// carry real buttons with real `on_press` handlers --
/// `FolderBrowserChooseCurrentDirectory` is one -- and none of them are
/// `SurfaceAction` entries either. **Verified as a real asymmetry, not
/// assumed the way this RFC's own risk document warns against**: modal
/// buttons are keyboard-reachable by construction, not by individual
/// handler wiring. `ModalFocusNext` (nine call sites) cycles focus
/// generically across whichever buttons the current modal has, and the
/// dialogs say so on screen ("Tab/Shift+Tab moves focus; Enter
/// activates," six catalog entries carrying that exact hint). A
/// surface has nothing equivalent: `FocusZone` is three variants
/// (`Sidebar`/`TabStrip`/`MainArea`) that cycle *zones*, not widgets,
/// which is the entire reason a surface-local button can go
/// keyboard-unreachable in the first place and a modal button
/// structurally cannot. **This asymmetry is this RFC's own
/// justification for existing at all** -- the risk document's §4 warns
/// that an exhaustive match is only as good as the set it is exhaustive
/// over, and an inventory whose own largest exclusion goes unstated
/// invites the exact "we already enumerated that" misreading that let
/// a mouse-only close button survive four prior reviews.
///
/// All three exclusions are a scoping decision, recorded here rather
/// than silently omitted -- if any of them stops being true (the
/// editor gains a discrete, non-typing action; a highlight gains its
/// own mouse-only meaning; a modal ever grows a button `ModalFocusNext`
/// does not reach), it belongs in this enum.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SurfaceAction {
    TabStripCloseProject,
    TabStripGoToProjectBoard,
    TabStripSwitchToProject,
    ExplorerActivateHighlightedRow,
    ApprovalHistoryOpenHighlightedEntry,
    ChangeReviewMarkAccepted,
    ChangeReviewMarkRejected,
    ChangeReviewSelectHighlightedFile,
    TrustSettingsActivateTrustControl,
    TrustSettingsToggleTranscriptCaptureDeclined,
    TrustSettingsOpenTranscriptPurgeDialog,
    ProjectBoardRowReopenHighlightedProject,
    ProjectBoardPathFieldSubmit,
    ProjectBoardPathFieldDismiss,
}

/// RFC-044 D3: the required mirror of `control_coverage` -- not "how
/// does a *mouse* reach this" (that question, and `control_coverage`
/// itself, are unchanged, and still only asked of [`NavigationAction`])
/// but **"how does the *keyboard* reach this"**, exhaustive over the
/// widened [`SurfaceAction`] domain D1 defines. Adding a `SurfaceAction`
/// variant without extending this match fails to compile -- the same
/// discipline [`action_catalog_key`]/`control_coverage` already use,
/// aimed at the axis neither of them asked about.
///
/// **Why `ControlCoverage::MouseOnly` is reachable here and never was
/// from `control_coverage`**: every arm below that answers "yes, a key
/// reaches this" reuses `ControlCoverage::VisibleControl`'s own shape
/// (`description` plus a literal, grep-checked source snippet) --
/// repurposed for *this* function to mean "a real key binding exists
/// and here is the literal match proving it," the keyboard analogue of
/// what that field already meant for a mouse press. `KeyboardOnly` is
/// not reused here at all, deliberately: reusing it for "the keyboard
/// *does* reach this" would silently invert its own established
/// meaning ("only the keyboard reaches this, no mouse control exists")
/// into its opposite, which is exactly the confusion a shared type
/// must not create. `MouseOnly { reason }` is the one genuinely new
/// arm this function needs, and it requires a reason exactly as
/// `KeyboardOnly` already does -- `TrackedGap` entries name the slice
/// that closes them (PR-044-B), `Permanent` entries are a real,
/// considered decision that a key would cost more than it is worth,
/// per §6 of `what-advertising-keys-must-not-become.md`.
#[cfg(test)]
pub(crate) fn surface_keyboard_coverage(action: SurfaceAction) -> ControlCoverage {
    match action {
        SurfaceAction::TabStripCloseProject => ControlCoverage::VisibleControl {
            description: "Delete, with a project's own tab highlighted -- PR-044-B, closing the \
                          access defect that widened this RFC's scope",
            on_press_snippet: "keyboard::key::Named::Delete",
        },
        SurfaceAction::TabStripGoToProjectBoard => ControlCoverage::VisibleControl {
            description: "Enter, with the tab strip's own first (\"Projects\") entry highlighted",
            on_press_snippet: "keyboard::key::Named::Enter",
        },
        SurfaceAction::TabStripSwitchToProject => ControlCoverage::VisibleControl {
            description: "Enter, with a project's own tab highlighted",
            on_press_snippet: "keyboard::key::Named::Enter",
        },
        SurfaceAction::ExplorerActivateHighlightedRow => ControlCoverage::VisibleControl {
            description: "Enter, opening the highlighted file or navigating into the \
                          highlighted directory",
            on_press_snippet: "keyboard::key::Named::Enter",
        },
        SurfaceAction::ApprovalHistoryOpenHighlightedEntry => ControlCoverage::VisibleControl {
            description: "Enter, with an approval history row highlighted",
            on_press_snippet: "keyboard::key::Named::Enter",
        },
        SurfaceAction::ChangeReviewMarkAccepted => ControlCoverage::VisibleControl {
            description: "the bare `a` key, response 334 Required 1",
            on_press_snippet: "\"a\" => Some(ChangeReviewDecision::Accepted)",
        },
        SurfaceAction::ChangeReviewMarkRejected => ControlCoverage::VisibleControl {
            description: "the bare `r` key, response 334 Required 1",
            on_press_snippet: "\"r\" => Some(ChangeReviewDecision::Rejected)",
        },
        SurfaceAction::ChangeReviewSelectHighlightedFile => ControlCoverage::VisibleControl {
            description: "Enter, with a changed-file row highlighted",
            on_press_snippet: "keyboard::key::Named::Enter",
        },
        SurfaceAction::TrustSettingsActivateTrustControl => ControlCoverage::VisibleControl {
            description: "Enter, activating whichever of Grant/Revoke Trust is currently shown",
            on_press_snippet: "keyboard::key::Named::Enter",
        },
        SurfaceAction::TrustSettingsToggleTranscriptCaptureDeclined => {
            ControlCoverage::VisibleControl {
                description: "the Space key, response 248 PR-033-B",
                on_press_snippet: "keyboard::key::Named::Space",
            }
        }
        SurfaceAction::TrustSettingsOpenTranscriptPurgeDialog => ControlCoverage::VisibleControl {
            description: "the Delete key, response 248 PR-033-C",
            on_press_snippet: "keyboard::key::Named::Delete",
        },
        SurfaceAction::ProjectBoardRowReopenHighlightedProject => ControlCoverage::VisibleControl {
            description: "Enter, with a recent-project row highlighted",
            on_press_snippet: "keyboard::key::Named::Enter",
        },
        SurfaceAction::ProjectBoardPathFieldSubmit => ControlCoverage::VisibleControl {
            description: "Enter, while the path field is showing",
            on_press_snippet: "keyboard::key::Named::Enter",
        },
        SurfaceAction::ProjectBoardPathFieldDismiss => ControlCoverage::VisibleControl {
            description: "Escape, only when the field was explicitly requested (Ctrl+Alt+O)",
            on_press_snippet: "keyboard::key::Named::Escape",
        },
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
