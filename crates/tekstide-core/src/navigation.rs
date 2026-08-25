#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationAction {
    OpenProjectBoard,
    /// RFC-038 PR-038-B: reveals the path field on the Project Board and
    /// focuses it -- the second-project case PR-038-A's own field does
    /// not serve, since that one only shows while the board is empty.
    /// Distinct from `SwitchActiveProject`: that action is for cycling
    /// between projects already open (`AppState::switch_active_project`,
    /// still `Configurable`/`None`, unrelated to this one); this one is
    /// for adding a project that is not open yet.
    OpenProjectEntryField,
    SwitchActiveProject,
    ToggleProjectMode,
    /// Terminal launch UX handoff: launches a new terminal in the active
    /// project. Distinct from `ToggleProjectMode` -- that flips between
    /// Content and Terminal Immersion with no side effect on session
    /// state; this always lands in Terminal Immersion and always
    /// attempts to create a new session, whichever mode was active.
    LaunchTerminal,
    /// RFC-018 PR-018-B: pastes real clipboard content into the
    /// currently keyboard-focused terminal, gated through
    /// `TerminalInputPolicy::evaluate` before any byte reaches a PTY.
    /// Distinct from typed keystrokes (`RoutedInput::Terminal`): this is
    /// a global keybinding rather than terminal-focus-routed input,
    /// since reading the clipboard is real I/O the shell crate performs
    /// once per press, not a per-keystroke encoding.
    PasteIntoTerminal,
    /// RFC-019 PR-019-D: saves the active project's open document through
    /// `save_active_document`. Global keybinding rather than a
    /// content-surface-routed key for the same reason `PasteIntoTerminal`
    /// is: it needs real I/O (the file write, plus the on-disk conflict
    /// check) and reads whichever document is open at press time, not a
    /// per-keystroke encoding.
    SaveActiveDocument,
    CycleVisibleTerminalSession,
    /// RFC-022 PR-022-D: launches a real AgentRun in the active project,
    /// through a code-defined profile (`AiCliProfile::claude_code_linux_default`)
    /// resolved at launch time -- see response 218. Distinct from
    /// `LaunchTerminal`: the spawned process is an AI CLI under transcript
    /// capture/audit, not a plain shell, and the launch can be refused
    /// (no executable found, workspace discovery blocked in a Restricted
    /// project, resource limit reached) the way `LaunchTerminal` can.
    LaunchAgentRun,
    OpenCurrentAgentRunDetail,
    /// RFC-022 PR-022-E: opens the active project's `ApprovalHistory`
    /// surface -- named for what it renders (response 233: the surface
    /// shows every retained request, decided and expired included, not
    /// only ones still awaiting a decision, so the action's own name
    /// must not promise a "pending" subset the surface does not show).
    /// Renamed from `OpenPendingApproval`, the same identifier this
    /// action was declared under before the surface it opens had a
    /// name -- the seventh instance of "wired with no reader" this RFC
    /// has found was this exact action having no `app_command_for` arm
    /// at all until this response.
    OpenApprovalHistory,
    /// RFC-032: opens the active project's `TrustSettings` surface,
    /// where granting and revoking workspace trust actually happen --
    /// the second real `open_surface`-conditional dispatch after
    /// `OpenApprovalHistory`.
    OpenTrustSettings,
    OpenDiffReview,
    OpenSafeCloseDialog,
    OpenCommandPalette,
    /// RFC-038 PR-038-C: opens the Help modal, reachable from anywhere
    /// -- including Terminal Immersion, which `0.12.1`'s board-only
    /// keyboard list left unserved. Reference material (the derived
    /// keyboard list), not a working surface -- RFC-039's second
    /// principle is exactly this distinction, named a slice early.
    OpenHelp,
    /// RFC-038 PR-038-G: opens the folder browser (`ModalContent::
    /// FolderBrowser`) -- the owner's overturn of D1, a visible-control
    /// accelerator alongside the real button on the Project Board, not
    /// the only route to it.
    OpenFolderBrowser,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeybindingStatus {
    Reserved,
    Candidate,
    Configurable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeybindingRule {
    pub action: NavigationAction,
    pub default_binding: Option<&'static str>,
    pub status: KeybindingStatus,
}

impl KeybindingRule {
    pub fn new(
        action: NavigationAction,
        default_binding: Option<&'static str>,
        status: KeybindingStatus,
    ) -> Self {
        Self {
            action,
            default_binding,
            status,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeybindingPolicy {
    pub rules: Vec<KeybindingRule>,
}

impl KeybindingPolicy {
    pub fn linux_mvp() -> Self {
        Self {
            rules: vec![
                KeybindingRule::new(
                    NavigationAction::OpenCommandPalette,
                    Some("Ctrl+Shift+P"),
                    KeybindingStatus::Reserved,
                ),
                KeybindingRule::new(
                    NavigationAction::OpenProjectBoard,
                    Some("Ctrl+Alt+P"),
                    KeybindingStatus::Candidate,
                ),
                // RFC-038 PR-038-B: `Ctrl+Alt+O` (Open), following the
                // existing `Ctrl+Alt+<letter>` shape (`P`, `M`, `T`, `A`,
                // `R`, `H`, `U` elsewhere in this list) -- unclaimed by
                // any other rule here and not `Ctrl+Shift+P`'s `Reserved`
                // command-palette binding, so it collides with nothing
                // (checked mechanically by
                // `open_project_entry_field_shortcut_is_a_candidate_that_collides_with_no_other_rule`,
                // not by inspection alone). RFC-038's own D2 named this
                // letter directly.
                KeybindingRule::new(
                    NavigationAction::OpenProjectEntryField,
                    Some("Ctrl+Alt+O"),
                    KeybindingStatus::Candidate,
                ),
                KeybindingRule::new(
                    NavigationAction::ToggleProjectMode,
                    Some("Ctrl+Alt+M"),
                    KeybindingStatus::Candidate,
                ),
                // Terminal launch UX handoff: `Ctrl+Alt+T`, following the
                // existing `Ctrl+Alt+<letter>` shape (`P`, `M` above) --
                // `T` for Terminal, unused by any other rule here and not
                // `Ctrl+Shift+P`'s `Reserved` command-palette binding, so
                // it collides with nothing (checked mechanically by
                // `linux_mvp_terminal_launch_binding_collides_with_nothing`,
                // not by inspection alone).
                KeybindingRule::new(
                    NavigationAction::LaunchTerminal,
                    Some("Ctrl+Alt+T"),
                    KeybindingStatus::Candidate,
                ),
                // RFC-018 PR-018-B: `Ctrl+Shift+V`, the terminal-emulator
                // convention (distinct from `Ctrl+V`, which most
                // terminals leave to the shell's own line editing). Does
                // not collide with `Ctrl+Shift+P` (`Reserved`, command
                // palette) or any `Ctrl+Alt+<letter>` rule -- checked
                // mechanically by
                // `paste_into_terminal_shortcut_is_a_candidate_that_collides_with_no_other_rule`,
                // not by inspection alone.
                KeybindingRule::new(
                    NavigationAction::PasteIntoTerminal,
                    Some("Ctrl+Shift+V"),
                    KeybindingStatus::Candidate,
                ),
                // RFC-019 PR-019-D: `Ctrl+S`, the universal save
                // convention across editors and terminal-adjacent tools.
                // Does not collide with any `Ctrl+Alt+<letter>` rule or
                // `Ctrl+Shift+<letter>` rule above -- checked mechanically
                // by `save_active_document_shortcut_is_a_candidate_that_collides_with_no_other_rule`,
                // not by inspection alone.
                KeybindingRule::new(
                    NavigationAction::SaveActiveDocument,
                    Some("Ctrl+S"),
                    KeybindingStatus::Candidate,
                ),
                // RFC-022 PR-022-D: `Ctrl+Alt+A`, following the existing
                // `Ctrl+Alt+<letter>` shape (`P`, `M`, `T` above) -- `A`
                // for Agent, unused by any other rule here and not
                // `Ctrl+Shift+P`'s `Reserved` command-palette binding, so
                // it collides with nothing (checked mechanically by
                // `launch_agent_run_shortcut_is_a_candidate_that_collides_with_no_other_rule`,
                // not by inspection alone).
                KeybindingRule::new(
                    NavigationAction::LaunchAgentRun,
                    Some("Ctrl+Alt+A"),
                    KeybindingStatus::Candidate,
                ),
                // RFC-039 PR-039-B: `Ctrl+Alt+N` (Next) -- the global
                // accelerator alongside the real, visible controls
                // (clicking a tab, or Left/Right + Enter across the tab
                // strip's own keyboard focus) that the RFC's Principle 1
                // requires every action to have: cycles to the next open
                // project in `AppState::projects()`'s own order,
                // wrapping, a no-op with fewer than two projects open.
                // Not `Ctrl+Alt+S` (sWitch) -- too easily confused with
                // plain `Ctrl+S` (`SaveActiveDocument`) one modifier away.
                // Unclaimed by any other rule here and not `Ctrl+Shift+P`'s
                // `Reserved` command-palette binding, so it collides with
                // nothing (checked mechanically by
                // `switch_active_project_shortcut_is_a_candidate_that_collides_with_no_other_rule`,
                // not by inspection alone). Takes RFC-036's dead-action
                // count from four to three.
                KeybindingRule::new(
                    NavigationAction::SwitchActiveProject,
                    Some("Ctrl+Alt+N"),
                    KeybindingStatus::Candidate,
                ),
                KeybindingRule::new(
                    NavigationAction::CycleVisibleTerminalSession,
                    None,
                    KeybindingStatus::Configurable,
                ),
                // pr-020-b-report-surface.md: `Configurable` with a
                // `None` binding reads as "a user can bind this" but
                // actually means "unreachable until RFC-023 exists" --
                // the same category error response 248 named and this
                // project has now fixed three times (`OpenTrustSettings`,
                // `OpenApprovalHistory`, this one). `AgentRunDetail` is
                // this slice's own real render arm
                // (`content_mode_view`), with no other route to open it.
                // Order matters, per the handoff's own instruction: the
                // render arm landed first, so this binding never exists
                // in a state where it silently opens the plain editor
                // instead. `Ctrl+Alt+R`, following the existing
                // `Ctrl+Alt+<letter>` shape (`P`, `M`, `T`, `A`, `U`,
                // `H` above) -- `R` for Report. Considered and rejected
                // as ambiguous with a hypothetical "Reload" when `U` was
                // chosen for `OpenTrustSettings` (response 248's own
                // comment on that binding) -- there is no `Reload`
                // action anywhere in this policy today, so that concern
                // does not block it here; unused by any other rule and
                // not `Ctrl+Shift+P`'s `Reserved` command-palette
                // binding, so it collides with nothing (checked
                // mechanically by
                // `open_current_agent_run_detail_shortcut_is_a_candidate_that_collides_with_no_other_rule`,
                // not by inspection alone).
                KeybindingRule::new(
                    NavigationAction::OpenCurrentAgentRunDetail,
                    Some("Ctrl+Alt+R"),
                    KeybindingStatus::Candidate,
                ),
                // approval-history-binding handoff: `Configurable` with a
                // `None` binding reads as "a user can bind this" but
                // actually means "unreachable until RFC-023 exists" -- the
                // same category error response 248 named and RFC-032 fixed
                // for `OpenTrustSettings`. `ApprovalHistory` is RFC-022
                // PR-022-E's own surface, already built and tested, with
                // no other route to open it. `Ctrl+Alt+H`, following the
                // existing `Ctrl+Alt+<letter>` shape (`P`, `M`, `T`, `A`,
                // `U` above) -- `H` for History, unused by any other rule
                // here and not `Ctrl+Shift+P`'s `Reserved` command-palette
                // binding, so it collides with nothing (checked
                // mechanically by
                // `open_approval_history_shortcut_is_a_candidate_that_collides_with_no_other_rule`,
                // not by inspection alone).
                KeybindingRule::new(
                    NavigationAction::OpenApprovalHistory,
                    Some("Ctrl+Alt+H"),
                    KeybindingStatus::Candidate,
                ),
                // RFC-032, response 248's required fix: `Configurable`
                // with a `None` binding reads as "a user can bind this"
                // but actually means "unreachable until RFC-023 exists"
                // -- the category error response 248 named directly.
                // `TrustSettings` is the *only* route to granting trust
                // at all, so a `None` binding here would leave the
                // entire chain RFC-032 exists to unblock unreachable by
                // any real user input. `Ctrl+Alt+U`, following the
                // existing `Ctrl+Alt+<letter>` shape (`P`, `M`, `T`, `A`
                // above) -- `U` for trUst (the other natural letters,
                // `T`/`G`/`R`, are already `LaunchTerminal`/free-but-
                // less-mnemonic/free-but-ambiguous-with-Reload), unused
                // by any other rule here and not `Ctrl+Shift+P`'s
                // `Reserved` command-palette binding, so it collides
                // with nothing (checked mechanically by
                // `open_trust_settings_shortcut_is_a_candidate_that_collides_with_no_other_rule`,
                // not by inspection alone).
                KeybindingRule::new(
                    NavigationAction::OpenTrustSettings,
                    Some("Ctrl+Alt+U"),
                    KeybindingStatus::Candidate,
                ),
                // RFC-020, the change review surface handoff: `Ctrl+Alt+D`
                // (Diff), following the existing `Ctrl+Alt+<letter>` shape
                // (`P`, `M`, `T`, `A`, `N`, `R`, `H`, `U`, `K`, `B` above)
                // -- unclaimed by any other rule here and not
                // `Ctrl+Shift+P`'s `Reserved` command-palette binding, so
                // it collides with nothing (checked mechanically by
                // `open_diff_review_shortcut_is_a_candidate_that_collides_with_no_other_rule`,
                // not by inspection alone). Takes RFC-036's dead-action
                // count from three to two.
                KeybindingRule::new(
                    NavigationAction::OpenDiffReview,
                    Some("Ctrl+Alt+D"),
                    KeybindingStatus::Candidate,
                ),
                KeybindingRule::new(
                    NavigationAction::OpenSafeCloseDialog,
                    None,
                    KeybindingStatus::Configurable,
                ),
                // RFC-038 PR-038-C: `Ctrl+Alt+K`. No strong mnemonic
                // available -- `H` (Help) is `OpenApprovalHistory`'s and
                // `P` is `OpenProjectBoard`'s, the same constraint `U`
                // (trUst) was chosen under for `OpenTrustSettings`.
                // Unclaimed by any other rule here and not
                // `Ctrl+Shift+P`'s `Reserved` command-palette binding, so
                // it collides with nothing (checked mechanically by
                // `open_help_shortcut_is_a_candidate_that_collides_with_no_other_rule`,
                // not by inspection alone).
                KeybindingRule::new(
                    NavigationAction::OpenHelp,
                    Some("Ctrl+Alt+K"),
                    KeybindingStatus::Candidate,
                ),
                // RFC-038 PR-038-G: `Ctrl+Alt+B` (Browse), following the
                // existing `Ctrl+Alt+<letter>` shape -- unclaimed by any
                // other rule here and not `Ctrl+Shift+P`'s `Reserved`
                // command-palette binding, so it collides with nothing
                // (checked mechanically by
                // `open_folder_browser_shortcut_is_a_candidate_that_collides_with_no_other_rule`,
                // not by inspection alone). An accelerator alongside the
                // real button, not the only route -- RFC-038-G's own
                // task breakdown: "a button, not only a key."
                KeybindingRule::new(
                    NavigationAction::OpenFolderBrowser,
                    Some("Ctrl+Alt+B"),
                    KeybindingStatus::Candidate,
                ),
            ],
        }
    }

    /// The bindings a user can actually press today, derived from the
    /// policy rather than listed anywhere -- the one source any help
    /// text must be built from.
    ///
    /// Two categories are deliberately excluded, and both exclusions are
    /// the point of deriving this instead of writing a list:
    ///
    /// - `Configurable` with no `default_binding` is **dead**, not
    ///   pending. `CycleVisibleTerminalSession` and `OpenSafeCloseDialog`
    ///   are in this state (RFC-039 PR-039-B moved `SwitchActiveProject`
    ///   out of it, down from four to three; the change review surface
    ///   handoff moved `OpenDiffReview` out of it, down from three to
    ///   two); advertising a dead one would promise an action no key can
    ///   reach. This project has already had to fix that exact category
    ///   error three times in the policy itself (`OpenTrustSettings`,
    ///   `OpenApprovalHistory`, `OpenCurrentAgentRunDetail`) -- help
    ///   text derived from the policy cannot repeat it a fourth.
    /// - `Reserved` means the binding is claimed so nothing else takes
    ///   it, not that pressing it does something. `Ctrl+Shift+P` is
    ///   reserved for a command palette that does not exist.
    ///
    /// A hand-written help list would have to be edited whenever a rule
    /// changes status, which is exactly the kind of state-asserting text
    /// `ARCHITECTURE.md` records this project repeatedly failing to keep
    /// current. This cannot go stale: it *is* the policy.
    pub fn advertised_bindings(&self) -> Vec<(NavigationAction, &'static str)> {
        self.rules
            .iter()
            .filter_map(|rule| match (rule.status, rule.default_binding) {
                (KeybindingStatus::Candidate, Some(binding)) => Some((rule.action, binding)),
                _ => None,
            })
            .collect()
    }

    pub fn rule_for(&self, action: NavigationAction) -> Option<&KeybindingRule> {
        self.rules.iter().find(|rule| rule.action == action)
    }

    pub fn binding_is_reserved_for(&self, binding: &str, action: NavigationAction) -> bool {
        self.rules.iter().any(|rule| {
            rule.action == action
                && rule.default_binding == Some(binding)
                && rule.status == KeybindingStatus::Reserved
        })
    }

    pub fn uses_binding(&self, binding: &str) -> bool {
        self.rules
            .iter()
            .any(|rule| rule.default_binding == Some(binding))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalLayoutClass {
    Wide,
    Narrow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalPanePolicy {
    pub layout: TerminalLayoutClass,
    pub max_visible_panes: u8,
}

impl TerminalPanePolicy {
    pub fn for_layout(layout: TerminalLayoutClass) -> Self {
        Self {
            layout,
            max_visible_panes: 2,
        }
    }

    pub fn visible_pane_count(self, requested_visible_panes: u8) -> u8 {
        requested_visible_panes.min(self.max_visible_panes)
    }
}

#[cfg(test)]
mod tests;
