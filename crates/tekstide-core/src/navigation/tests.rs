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
fn launch_agent_run_shortcut_is_a_candidate_that_collides_with_no_other_rule() {
    let policy = KeybindingPolicy::linux_mvp();
    let rule = policy
        .rule_for(NavigationAction::LaunchAgentRun)
        .expect("Launch Agent Run should have a keyboard policy");

    assert_eq!(rule.default_binding, Some("Ctrl+Alt+A"));
    assert_eq!(rule.status, KeybindingStatus::Candidate);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::LaunchAgentRun)
        .filter(|other| other.default_binding == rule.default_binding)
        .map(|other| other.action)
        .collect();
    assert!(
        collisions.is_empty(),
        "Ctrl+Alt+A must not collide with any other rule, reserved or not: {collisions:?}"
    );
}

/// RFC-032, response 248's required fix: `OpenTrustSettings` is the
/// *only* route to granting trust at all -- a `Configurable`/`None`
/// binding here would have left it unreachable by any real user input,
/// the exact "reads as pending, actually means dead" category error
/// response 248 named. Checked mechanically, not by inspection alone,
/// the same shape every other real binding above already uses.
#[test]
fn open_trust_settings_shortcut_is_a_candidate_that_collides_with_no_other_rule() {
    let policy = KeybindingPolicy::linux_mvp();
    let rule = policy
        .rule_for(NavigationAction::OpenTrustSettings)
        .expect("Open Trust Settings should have a keyboard policy");

    assert_eq!(rule.default_binding, Some("Ctrl+Alt+U"));
    assert_eq!(rule.status, KeybindingStatus::Candidate);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::OpenTrustSettings)
        .filter(|other| other.default_binding == rule.default_binding)
        .map(|other| other.action)
        .collect();
    assert!(
        collisions.is_empty(),
        "Ctrl+Alt+U must not collide with any other rule, reserved or not: {collisions:?}"
    );
}

/// approval-history-binding handoff: `ApprovalHistory` is RFC-022
/// PR-022-E's own surface, already built and tested, with no other route
/// to open it -- a `Configurable`/`None` binding here would leave it
/// unreachable by any real user input, the same category error response
/// 248 named for `OpenTrustSettings`. Checked mechanically, not by
/// inspection alone, the same shape every other real binding above
/// already uses.
#[test]
fn open_approval_history_shortcut_is_a_candidate_that_collides_with_no_other_rule() {
    let policy = KeybindingPolicy::linux_mvp();
    let rule = policy
        .rule_for(NavigationAction::OpenApprovalHistory)
        .expect("Open Approval History should have a keyboard policy");

    assert_eq!(rule.default_binding, Some("Ctrl+Alt+H"));
    assert_eq!(rule.status, KeybindingStatus::Candidate);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::OpenApprovalHistory)
        .filter(|other| other.default_binding == rule.default_binding)
        .map(|other| other.action)
        .collect();
    assert!(
        collisions.is_empty(),
        "Ctrl+Alt+H must not collide with any other rule, reserved or not: {collisions:?}"
    );
}

/// RFC-038 PR-038-B: `Ctrl+Alt+O` reveals and focuses the path field for
/// the second-project case (a project already open, the user wants
/// another) -- checked mechanically, not by inspection alone, the same
/// shape every other real binding above already uses.
#[test]
fn open_project_entry_field_shortcut_is_a_candidate_that_collides_with_no_other_rule() {
    let policy = KeybindingPolicy::linux_mvp();
    let rule = policy
        .rule_for(NavigationAction::OpenProjectEntryField)
        .expect("Open Project Entry Field should have a keyboard policy");

    assert_eq!(rule.default_binding, Some("Ctrl+Alt+O"));
    assert_eq!(rule.status, KeybindingStatus::Candidate);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::OpenProjectEntryField)
        .filter(|other| other.default_binding == rule.default_binding)
        .map(|other| other.action)
        .collect();
    assert!(
        collisions.is_empty(),
        "Ctrl+Alt+O must not collide with any other rule, reserved or not: {collisions:?}"
    );
}

/// RFC-038 PR-038-C: `Ctrl+Alt+K` opens the Help modal -- checked
/// mechanically, not by inspection alone, the same shape every other
/// real binding above already uses.
#[test]
fn open_help_shortcut_is_a_candidate_that_collides_with_no_other_rule() {
    let policy = KeybindingPolicy::linux_mvp();
    let rule = policy
        .rule_for(NavigationAction::OpenHelp)
        .expect("Open Help should have a keyboard policy");

    assert_eq!(rule.default_binding, Some("Ctrl+Alt+K"));
    assert_eq!(rule.status, KeybindingStatus::Candidate);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::OpenHelp)
        .filter(|other| other.default_binding == rule.default_binding)
        .map(|other| other.action)
        .collect();
    assert!(
        collisions.is_empty(),
        "Ctrl+Alt+K must not collide with any other rule, reserved or not: {collisions:?}"
    );
}

/// pr-020-b-report-surface.md: `AgentRunDetail` is this slice's own
/// real render arm, with no other route to open it -- a
/// `Configurable`/`None` binding here would leave it unreachable by any
/// real user input, the same category error response 248 named for
/// `OpenTrustSettings`. Checked mechanically, not by inspection alone,
/// the same shape every other real binding above already uses.
#[test]
fn open_current_agent_run_detail_shortcut_is_a_candidate_that_collides_with_no_other_rule() {
    let policy = KeybindingPolicy::linux_mvp();
    let rule = policy
        .rule_for(NavigationAction::OpenCurrentAgentRunDetail)
        .expect("Open Current Agent Run Detail should have a keyboard policy");

    assert_eq!(rule.default_binding, Some("Ctrl+Alt+R"));
    assert_eq!(rule.status, KeybindingStatus::Candidate);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::OpenCurrentAgentRunDetail)
        .filter(|other| other.default_binding == rule.default_binding)
        .map(|other| other.action)
        .collect();
    assert!(
        collisions.is_empty(),
        "Ctrl+Alt+R must not collide with any other rule, reserved or not: {collisions:?}"
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
        NavigationAction::LaunchAgentRun,
        NavigationAction::OpenCurrentAgentRunDetail,
        NavigationAction::OpenApprovalHistory,
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

/// `advertised_bindings` is what every piece of user-facing help is
/// built from, so its filter is the thing that decides whether the
/// product can promise an action it cannot perform. Asserted as an exact
/// set, not a count: a rule changing status silently is the failure mode
/// this guards.
#[test]
fn advertised_bindings_are_exactly_the_live_ones() {
    let policy = KeybindingPolicy::linux_mvp();
    let advertised = policy.advertised_bindings();

    let bindings: Vec<&str> = advertised.iter().map(|(_, binding)| *binding).collect();
    assert_eq!(
        bindings,
        vec![
            "Ctrl+Alt+P",
            "Ctrl+Alt+O",
            "Ctrl+Alt+M",
            "Ctrl+Alt+T",
            "Ctrl+Shift+V",
            "Ctrl+S",
            "Ctrl+Alt+A",
            "Ctrl+Alt+R",
            "Ctrl+Alt+H",
            "Ctrl+Alt+U",
            "Ctrl+Alt+K",
        ],
    );

    // `Ctrl+Shift+P` is `Reserved` -- claimed so nothing else takes it,
    // with no command palette behind it. Advertising a reserved binding
    // would tell a user to press a key that does nothing.
    assert!(!bindings.contains(&"Ctrl+Shift+P"));

    // Every excluded rule is excluded for one of exactly two reasons.
    for rule in &policy.rules {
        if bindings.contains(&rule.default_binding.unwrap_or("")) {
            continue;
        }
        assert!(
            rule.default_binding.is_none() || rule.status != KeybindingStatus::Candidate,
            "{:?} is a live binding but was not advertised",
            rule.action
        );
    }
}
