#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationAction {
    OpenProjectBoard,
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
                KeybindingRule::new(
                    NavigationAction::SwitchActiveProject,
                    None,
                    KeybindingStatus::Configurable,
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
                KeybindingRule::new(
                    NavigationAction::OpenDiffReview,
                    None,
                    KeybindingStatus::Configurable,
                ),
                KeybindingRule::new(
                    NavigationAction::OpenSafeCloseDialog,
                    None,
                    KeybindingStatus::Configurable,
                ),
            ],
        }
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
