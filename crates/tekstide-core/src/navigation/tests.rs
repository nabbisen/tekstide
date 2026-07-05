use super::{
    KeybindingPolicy, KeybindingStatus, NavigationAction, TerminalLayoutClass, TerminalPanePolicy,
};

#[test]
fn linux_mvp_keybinding_policy_reserves_command_palette_and_avoids_shift_escape() {
    let policy = KeybindingPolicy::linux_mvp();

    assert!(policy.binding_is_reserved_for("Ctrl+Shift+P", NavigationAction::OpenCommandPalette));
    assert!(!policy.uses_binding("Ctrl+Shift+Esc"));
}

#[test]
fn project_board_shortcut_is_configurable_candidate() {
    let policy = KeybindingPolicy::linux_mvp();
    let rule = policy
        .rule_for(NavigationAction::OpenProjectBoard)
        .expect("Project Board should have a keyboard policy");

    assert_eq!(rule.default_binding, Some("Ctrl+Alt+P"));
    assert_eq!(rule.status, KeybindingStatus::Candidate);
}

#[test]
fn primary_navigation_workflows_have_keyboard_policy_entries() {
    let policy = KeybindingPolicy::linux_mvp();

    for action in [
        NavigationAction::OpenProjectBoard,
        NavigationAction::SwitchActiveProject,
        NavigationAction::ToggleProjectMode,
        NavigationAction::CycleVisibleTerminalSession,
        NavigationAction::OpenCurrentAgentRunDetail,
        NavigationAction::OpenPendingApproval,
        NavigationAction::OpenDiffReview,
        NavigationAction::OpenSafeCloseDialog,
    ] {
        assert!(
            policy.rule_for(action).is_some(),
            "{action:?} should have a keyboard policy entry"
        );
    }
}

#[test]
fn terminal_immersion_policy_limits_visible_panes_to_two() {
    for layout in [TerminalLayoutClass::Wide, TerminalLayoutClass::Narrow] {
        let policy = TerminalPanePolicy::for_layout(layout);

        assert_eq!(policy.max_visible_panes, 2);
        assert_eq!(policy.visible_pane_count(0), 0);
        assert_eq!(policy.visible_pane_count(1), 1);
        assert_eq!(policy.visible_pane_count(2), 2);
        assert_eq!(policy.visible_pane_count(3), 2);
    }
}
