#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationAction {
    OpenProjectBoard,
    SwitchActiveProject,
    ToggleProjectMode,
    CycleVisibleTerminalSession,
    OpenCurrentAgentRunDetail,
    OpenPendingApproval,
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
                    None,
                    KeybindingStatus::Configurable,
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
                KeybindingRule::new(
                    NavigationAction::OpenCurrentAgentRunDetail,
                    None,
                    KeybindingStatus::Configurable,
                ),
                KeybindingRule::new(
                    NavigationAction::OpenPendingApproval,
                    None,
                    KeybindingStatus::Configurable,
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
