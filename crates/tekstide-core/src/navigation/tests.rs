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
fn toggle_project_mode_shortcut_is_configurable_candidate() {
    // RFC-015 PR-015-E: Content<->Terminal mode switching has no
    // reachable trigger without a real default binding -- unlike the
    // other `Configurable` entries above, this one is exercised by a
    // real feature as of this slice, not deferred to RFC-023.
    let policy = KeybindingPolicy::linux_mvp();
    let rule = policy
        .rule_for(NavigationAction::ToggleProjectMode)
        .expect("Toggle Project Mode should have a keyboard policy");

    assert_eq!(rule.default_binding, Some("Ctrl+Alt+M"));
    assert_eq!(rule.status, KeybindingStatus::Candidate);

    let project_board_rule = policy
        .rule_for(NavigationAction::OpenProjectBoard)
        .expect("Project Board should have a keyboard policy");
    assert_ne!(
        rule.default_binding, project_board_rule.default_binding,
        "the two candidate bindings must not collide"
    );
}

/// Terminal launch UX handoff: "do not silently collide with a
/// `Reserved` binding -- check it mechanically rather than by reading."
/// Enumerates every *other* rule's binding, not just the one reserved
/// command-palette binding this file already happens to test against --
/// a future `Reserved` addition would be caught here too, not only a
/// hand-picked one.
#[test]
fn launch_terminal_shortcut_is_a_candidate_that_collides_with_no_other_rule() {
    let policy = KeybindingPolicy::linux_mvp();
    let rule = policy
        .rule_for(NavigationAction::LaunchTerminal)
        .expect("Launch Terminal should have a keyboard policy");

    assert_eq!(rule.default_binding, Some("Ctrl+Alt+T"));
    assert_eq!(rule.status, KeybindingStatus::Candidate);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::LaunchTerminal)
        .filter(|other| other.default_binding == rule.default_binding)
        .map(|other| other.action)
        .collect();
    assert!(
        collisions.is_empty(),
        "Ctrl+Alt+T must not collide with any other rule, reserved or not: {collisions:?}"
    );
}

#[test]
fn paste_into_terminal_shortcut_is_a_candidate_that_collides_with_no_other_rule() {
    let policy = KeybindingPolicy::linux_mvp();
    let rule = policy
        .rule_for(NavigationAction::PasteIntoTerminal)
        .expect("Paste Into Terminal should have a keyboard policy");

    assert_eq!(rule.default_binding, Some("Ctrl+Shift+V"));
    assert_eq!(rule.status, KeybindingStatus::Candidate);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::PasteIntoTerminal)
        .filter(|other| other.default_binding == rule.default_binding)
        .map(|other| other.action)
        .collect();
    assert!(
        collisions.is_empty(),
        "Ctrl+Shift+V must not collide with any other rule, reserved or not: {collisions:?}"
    );
}

#[test]
fn save_active_document_shortcut_is_a_candidate_that_collides_with_no_other_rule() {
    let policy = KeybindingPolicy::linux_mvp();
    let rule = policy
        .rule_for(NavigationAction::SaveActiveDocument)
        .expect("Save Active Document should have a keyboard policy");

    assert_eq!(rule.default_binding, Some("Ctrl+S"));
    assert_eq!(rule.status, KeybindingStatus::Candidate);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::SaveActiveDocument)
        .filter(|other| other.default_binding == rule.default_binding)
        .map(|other| other.action)
        .collect();
    assert!(
        collisions.is_empty(),
        "Ctrl+S must not collide with any other rule, reserved or not: {collisions:?}"
    );
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
