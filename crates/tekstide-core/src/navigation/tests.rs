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

    assert_eq!(rule.default_binding(), Some("Ctrl+Alt+P"));
    assert_eq!(rule.status(), KeybindingStatus::Bound);
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

    assert_eq!(rule.default_binding(), Some("Ctrl+Alt+M"));
    assert_eq!(rule.status(), KeybindingStatus::Bound);

    let project_board_rule = policy
        .rule_for(NavigationAction::OpenProjectBoard)
        .expect("Project Board should have a keyboard policy");
    assert_ne!(
        rule.default_binding(),
        project_board_rule.default_binding(),
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

    assert_eq!(rule.default_binding(), Some("Ctrl+Alt+T"));
    assert_eq!(rule.status(), KeybindingStatus::Bound);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::LaunchTerminal)
        .filter(|other| other.default_binding() == rule.default_binding())
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

    assert_eq!(rule.default_binding(), Some("Ctrl+Shift+V"));
    assert_eq!(rule.status(), KeybindingStatus::Bound);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::PasteIntoTerminal)
        .filter(|other| other.default_binding() == rule.default_binding())
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

    assert_eq!(rule.default_binding(), Some("Ctrl+S"));
    assert_eq!(rule.status(), KeybindingStatus::Bound);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::SaveActiveDocument)
        .filter(|other| other.default_binding() == rule.default_binding())
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

    assert_eq!(rule.default_binding(), Some("Ctrl+Alt+A"));
    assert_eq!(rule.status(), KeybindingStatus::Bound);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::LaunchAgentRun)
        .filter(|other| other.default_binding() == rule.default_binding())
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

    assert_eq!(rule.default_binding(), Some("Ctrl+Alt+U"));
    assert_eq!(rule.status(), KeybindingStatus::Bound);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::OpenTrustSettings)
        .filter(|other| other.default_binding() == rule.default_binding())
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

    assert_eq!(rule.default_binding(), Some("Ctrl+Alt+H"));
    assert_eq!(rule.status(), KeybindingStatus::Bound);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::OpenApprovalHistory)
        .filter(|other| other.default_binding() == rule.default_binding())
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

    assert_eq!(rule.default_binding(), Some("Ctrl+Alt+O"));
    assert_eq!(rule.status(), KeybindingStatus::Bound);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::OpenProjectEntryField)
        .filter(|other| other.default_binding() == rule.default_binding())
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

    assert_eq!(rule.default_binding(), Some("Ctrl+Alt+K"));
    assert_eq!(rule.status(), KeybindingStatus::Bound);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::OpenHelp)
        .filter(|other| other.default_binding() == rule.default_binding())
        .map(|other| other.action)
        .collect();
    assert!(
        collisions.is_empty(),
        "Ctrl+Alt+K must not collide with any other rule, reserved or not: {collisions:?}"
    );
}

/// RFC-038 PR-038-G: `Ctrl+Alt+B` opens the folder browser -- checked
/// mechanically, not by inspection alone, the same shape every other
/// real binding above already uses.
#[test]
fn open_folder_browser_shortcut_is_a_candidate_that_collides_with_no_other_rule() {
    let policy = KeybindingPolicy::linux_mvp();
    let rule = policy
        .rule_for(NavigationAction::OpenFolderBrowser)
        .expect("Open Folder Browser should have a keyboard policy");

    assert_eq!(rule.default_binding(), Some("Ctrl+Alt+B"));
    assert_eq!(rule.status(), KeybindingStatus::Bound);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::OpenFolderBrowser)
        .filter(|other| other.default_binding() == rule.default_binding())
        .map(|other| other.action)
        .collect();
    assert!(
        collisions.is_empty(),
        "Ctrl+Alt+B must not collide with any other rule, reserved or not: {collisions:?}"
    );
}

/// RFC-020, the change review surface: `Ctrl+Alt+D` -- checked
/// mechanically, not by inspection alone, the same shape every other
/// real binding above already uses.
#[test]
fn open_diff_review_shortcut_is_a_candidate_that_collides_with_no_other_rule() {
    let policy = KeybindingPolicy::linux_mvp();
    let rule = policy
        .rule_for(NavigationAction::OpenDiffReview)
        .expect("Open Diff Review should have a keyboard policy");

    assert_eq!(rule.default_binding(), Some("Ctrl+Alt+D"));
    assert_eq!(rule.status(), KeybindingStatus::Bound);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::OpenDiffReview)
        .filter(|other| other.default_binding() == rule.default_binding())
        .map(|other| other.action)
        .collect();
    assert!(
        collisions.is_empty(),
        "Ctrl+Alt+D must not collide with any other rule, reserved or not: {collisions:?}"
    );
}

/// RFC-039 PR-039-B: `Ctrl+Alt+N` cycles to the next open project --
/// checked mechanically, not by inspection alone, the same shape every
/// other real binding above already uses.
#[test]
fn switch_active_project_shortcut_is_a_candidate_that_collides_with_no_other_rule() {
    let policy = KeybindingPolicy::linux_mvp();
    let rule = policy
        .rule_for(NavigationAction::SwitchActiveProject)
        .expect("Switch Active Project should have a keyboard policy");

    assert_eq!(rule.default_binding(), Some("Ctrl+Alt+N"));
    assert_eq!(rule.status(), KeybindingStatus::Bound);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::SwitchActiveProject)
        .filter(|other| other.default_binding() == rule.default_binding())
        .map(|other| other.action)
        .collect();
    assert!(
        collisions.is_empty(),
        "Ctrl+Alt+N must not collide with any other rule, reserved or not: {collisions:?}"
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

    assert_eq!(rule.default_binding(), Some("Ctrl+Alt+R"));
    assert_eq!(rule.status(), KeybindingStatus::Bound);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::OpenCurrentAgentRunDetail)
        .filter(|other| other.default_binding() == rule.default_binding())
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
            "Ctrl+Alt+N",
            "Ctrl+Alt+R",
            "Ctrl+Alt+H",
            "Ctrl+Alt+U",
            "Ctrl+Alt+D",
            "Ctrl+Alt+K",
            "Ctrl+Alt+B",
            "Ctrl+Alt+C",
        ],
    );

    // `Ctrl+Shift+P` is `Reserved` -- claimed so nothing else takes it,
    // with no command palette behind it. Advertising a reserved binding
    // would tell a user to press a key that does nothing.
    assert!(!bindings.contains(&"Ctrl+Shift+P"));

    // Every excluded rule is excluded for one of exactly two reasons.
    for rule in &policy.rules {
        if bindings.contains(&rule.default_binding().unwrap_or("")) {
            continue;
        }
        assert!(
            rule.default_binding().is_none() || rule.status() != KeybindingStatus::Bound,
            "{:?} is a live binding but was not advertised",
            rule.action
        );
    }
}

/// RFC-045 PR-045-C, D5: `Ctrl+Alt+C` -- checked mechanically against
/// every other rule, reserved or not, the same shape every other real
/// binding here already uses. The chord was the handoff's to pick "from
/// what is free," and this is what makes "free" a measured claim rather
/// than a reading of the list.
#[test]
fn reload_configuration_shortcut_is_a_candidate_that_collides_with_no_other_rule() {
    let policy = KeybindingPolicy::linux_mvp();
    let rule = policy
        .rule_for(NavigationAction::ReloadConfiguration)
        .expect("Reload Configuration should have a keyboard policy");

    assert_eq!(rule.default_binding(), Some("Ctrl+Alt+C"));
    assert_eq!(rule.status(), KeybindingStatus::Bound);

    let collisions: Vec<NavigationAction> = policy
        .rules
        .iter()
        .filter(|other| other.action != NavigationAction::ReloadConfiguration)
        .filter(|other| other.default_binding() == rule.default_binding())
        .map(|other| other.action)
        .collect();
    assert!(
        collisions.is_empty(),
        "Ctrl+Alt+C must not collide with any other rule, reserved or not: {collisions:?}"
    );
}

// --- RFC-054 PR-054-A: D3′'s repair -- bound and dead are different types ----

/// Every rule is exactly one of `Reserved`, `Bound`, `Dead`, and the derived
/// status, the chord and the certificate can only agree: a `Dead` rule has no
/// chord and a certificate; the other two have a chord and no certificate.
#[test]
fn every_rule_is_exactly_one_of_reserved_bound_or_dead_and_the_views_agree() {
    let policy = KeybindingPolicy::linux_mvp();
    for rule in &policy.rules {
        match rule.status() {
            KeybindingStatus::Dead => {
                assert!(rule.default_binding().is_none(), "{:?}", rule.action);
                assert!(rule.dead_reachability().is_some(), "{:?}", rule.action);
            }
            KeybindingStatus::Reserved | KeybindingStatus::Bound => {
                assert!(rule.default_binding().is_some(), "{:?}", rule.action);
                assert!(rule.dead_reachability().is_none(), "{:?}", rule.action);
            }
        }
    }
    let count = |status| policy.rules.iter().filter(|r| r.status() == status).count();
    assert_eq!(
        count(KeybindingStatus::Reserved),
        1,
        "only the command palette"
    );
    assert_eq!(count(KeybindingStatus::Dead), 2);
    assert_eq!(
        count(KeybindingStatus::Bound),
        policy.rules.len() - 3,
        "everything else has a real, rebindable chord"
    );
}

/// **An action cannot be both bound and dead, by the type.** The old shape was
/// `default_binding: Option<_>` next to a `status` field, which could say
/// `Configurable` beside `None` -- and read as "bindable". There is now no
/// constructor that takes a chord *and* a status, and the fields that would let
/// a caller build the inconsistent pair are private. Held by scanning the
/// source, because a compile-fail test would need a dependency this project
/// does not have; ablating this (re-adding a public `status` field, or a
/// `new(action, Option<_>, status)`) makes it fail.
#[test]
fn no_constructor_or_public_field_can_build_a_rule_that_is_both_bound_and_dead() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/navigation.rs"),
    )
    .expect("navigation.rs is readable");
    let rule = &source[source.find("pub struct KeybindingRule").unwrap()..];
    let rule = &rule[..rule.find("\n}\n").unwrap()];
    assert!(rule.contains("binding: RuleBinding"), "{rule}");
    assert!(
        !rule.contains("pub binding"),
        "the binding must stay private: {rule}"
    );
    assert!(
        !rule.contains("pub status"),
        "a stored status can disagree with the binding"
    );
    assert!(
        !rule.contains("default_binding:"),
        "an Option next to a status is the old trap"
    );
    assert!(
        !source.contains("KeybindingRule::new(") && !source.contains("pub fn new("),
        "a constructor taking (chord, status) would rebuild the trap"
    );
    assert!(
        !source.contains("Configurable,"),
        "the misleading status is retired"
    );
}

/// The two dead actions were **decided**, each with its reachability stated
/// (RFC-054 D3′): a certificate, not a chord, because neither has a handler
/// and giving one a chord would be a new action (§7).
#[test]
fn the_two_dead_actions_carry_a_stated_reachability() {
    let policy = KeybindingPolicy::linux_mvp();

    let cycle = policy
        .rule_for(NavigationAction::CycleVisibleTerminalSession)
        .unwrap()
        .dead_reachability()
        .expect("dead");
    assert!(
        cycle.contains("Primary") && cycle.contains("no way"),
        "{cycle}"
    );

    let close = policy
        .rule_for(NavigationAction::OpenSafeCloseDialog)
        .unwrap()
        .dead_reachability()
        .expect("dead");
    assert!(
        close.contains("close button") && close.contains("Delete"),
        "{close}"
    );

    // The reserved chord is not a certificate and is not advertised.
    let palette = policy
        .rule_for(NavigationAction::OpenCommandPalette)
        .unwrap();
    assert_eq!(palette.status(), KeybindingStatus::Reserved);
    assert!(palette.dead_reachability().is_none());
}
