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

// --- RFC-054 PR-054-A: chords, action names, and rebinding ---------------

use super::{Chord, ChordError, KeybindingResolution, RefusalReason};

fn chord(spelling: &str) -> Chord {
    Chord::parse(spelling).unwrap_or_else(|error| panic!("{spelling}: {error:?}"))
}

fn effective(resolution: &KeybindingResolution, action: NavigationAction) -> String {
    resolution
        .policy
        .rule_for(action)
        .and_then(|rule| rule.effective_binding())
        .map(str::to_owned)
        .unwrap_or_default()
}

/// **The file and the help can never disagree (D8′).** Every chord the policy
/// ships -- advertised or reserved -- parses, and renders back to exactly the
/// spelling the Help modal and `--help` print. The tekstide crate holds the
/// other half: a real key press renders in this same shape
/// (`input::tests::every_default_binding_in_linux_mvp_round_trips_through_format_binding`).
#[test]
fn every_advertised_chord_round_trips() {
    let policy = KeybindingPolicy::linux_mvp();
    let mut checked = 0;
    for rule in &policy.rules {
        let Some(spelling) = rule.default_binding() else {
            continue;
        };
        let parsed = Chord::parse(spelling).unwrap_or_else(|e| panic!("{spelling}: {e:?}"));
        assert_eq!(parsed.to_string(), spelling, "{:?}", rule.action);
        checked += 1;
    }
    assert_eq!(
        checked, 16,
        "sixteen chords are held (fifteen bound, one reserved)"
    );
}

#[test]
fn a_chord_is_case_insensitive_on_input_and_canonical_on_output() {
    for spelling in ["ctrl+alt+p", "CTRL+ALT+P", "Alt+Ctrl+p", " Ctrl + Alt + P "] {
        assert_eq!(chord(spelling).to_string(), "Ctrl+Alt+P", "{spelling:?}");
    }
    assert_eq!(chord("shift+ctrl+v").to_string(), "Ctrl+Shift+V");
    assert_eq!(chord("ctrl+5").to_string(), "Ctrl+5");
}

/// What a chord may not be, each with its own reason and none of them carrying
/// any of the text the user wrote.
#[test]
fn a_chord_that_would_steal_typing_or_never_match_is_refused() {
    for (spelling, expected) in [
        ("", ChordError::Empty),
        ("   ", ChordError::Empty),
        ("Ctrl+", ChordError::NoSingleKey),
        ("Ctrl+Alt", ChordError::NoSingleKey),
        ("Ctrl+A+B", ChordError::NoSingleKey),
        ("Ctrl+Ctrl+P", ChordError::RepeatedModifier),
        ("Ctrl+Hyper+P", ChordError::UnknownPart),
        ("Ctrl+Enter", ChordError::UnknownPart),
        ("Ctrl+F1", ChordError::UnknownPart),
        ("Ctrl+-", ChordError::KeyNotRebindable),
        ("Ctrl+é", ChordError::KeyNotRebindable),
        // A bare letter, or Shift + letter, would take the key from typing.
        ("P", ChordError::NoCtrlOrAlt),
        ("Shift+P", ChordError::NoCtrlOrAlt),
        // Shift + digit is a different character on different layouts.
        ("Ctrl+Shift+1", ChordError::ShiftWithDigit),
    ] {
        assert_eq!(Chord::parse(spelling), Err(expected), "{spelling:?}");
    }
}

/// Every action has a distinct `snake_case` name, the names round-trip, and the
/// list of all actions is exactly the set the policy has a rule for.
#[test]
fn every_action_has_one_config_name_and_the_list_matches_the_policy() {
    let policy = KeybindingPolicy::linux_mvp();
    let mut names = std::collections::BTreeSet::new();
    for action in NavigationAction::ALL {
        assert!(names.insert(action.config_name()), "duplicate {action:?}");
        assert_eq!(
            NavigationAction::from_config_name(action.config_name()),
            Some(action)
        );
        assert!(
            action
                .config_name()
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_'),
            "{}",
            action.config_name()
        );
        assert!(policy.rule_for(action).is_some(), "{action:?} has no rule");
    }
    assert_eq!(policy.rules.len(), NavigationAction::ALL.len());
    assert_eq!(NavigationAction::from_config_name("Open_Help"), None);
    assert_eq!(NavigationAction::from_config_name("no_such_action"), None);
}

#[test]
fn a_plain_rebind_changes_the_effective_chord_and_the_help_and_never_the_default() {
    let resolution = KeybindingPolicy::linux_mvp()
        .with_overrides(&[(NavigationAction::OpenHelp, chord("Ctrl+Alt+J"))]);
    assert!(resolution.refusals.is_empty());
    assert_eq!(
        effective(&resolution, NavigationAction::OpenHelp),
        "Ctrl+Alt+J"
    );
    let rule = resolution
        .policy
        .rule_for(NavigationAction::OpenHelp)
        .unwrap();
    assert_eq!(
        rule.default_binding(),
        Some("Ctrl+Alt+K"),
        "the shipped default is untouched"
    );

    let advertised = resolution.policy.advertised_bindings();
    assert!(advertised.contains(&(NavigationAction::OpenHelp, "Ctrl+Alt+J")));
    assert!(
        !advertised.iter().any(|(_, chord)| *chord == "Ctrl+Alt+K"),
        "the old chord is gone"
    );
    assert!(!resolution.policy.uses_binding("Ctrl+Alt+K"));
    assert!(resolution.policy.uses_binding("Ctrl+Alt+J"));
}

/// **`Reserved` stays reserved.** Neither the action nor its chord can be
/// taken, and an action that is dead has nothing to rebind.
#[test]
fn a_reserved_action_a_reserved_chord_and_a_dead_action_are_refused() {
    let resolution = KeybindingPolicy::linux_mvp().with_overrides(&[
        (NavigationAction::OpenCommandPalette, chord("Ctrl+Alt+J")),
        (NavigationAction::OpenHelp, chord("Ctrl+Shift+P")),
        (NavigationAction::OpenSafeCloseDialog, chord("Ctrl+Alt+Z")),
        (
            NavigationAction::CycleVisibleTerminalSession,
            chord("Ctrl+Alt+Y"),
        ),
    ]);
    let reason = |action| {
        resolution
            .refusals
            .iter()
            .find(|r| r.action == action)
            .unwrap_or_else(|| panic!("{action:?} was not refused"))
            .reason
    };
    assert_eq!(
        reason(NavigationAction::OpenCommandPalette),
        RefusalReason::NotRebindable(KeybindingStatus::Reserved)
    );
    assert_eq!(
        reason(NavigationAction::OpenHelp),
        RefusalReason::ReservedChord {
            held_by: NavigationAction::OpenCommandPalette
        }
    );
    assert_eq!(
        reason(NavigationAction::OpenSafeCloseDialog),
        RefusalReason::NotRebindable(KeybindingStatus::Dead)
    );
    assert_eq!(resolution.refusals.len(), 4);
    // Defaults stand.
    assert_eq!(
        effective(&resolution, NavigationAction::OpenHelp),
        "Ctrl+Alt+K"
    );
    assert_eq!(
        effective(&resolution, NavigationAction::OpenCommandPalette),
        "Ctrl+Shift+P"
    );
    assert_eq!(
        effective(&resolution, NavigationAction::OpenSafeCloseDialog),
        ""
    );
}

/// A rebind colliding with another rule's chord **refuses, and the default
/// stands** -- never last-wins. (RFC-054 acceptance criterion; the ablation is
/// "accept last-wins".)
#[test]
fn a_rebind_that_collides_with_another_rules_chord_is_refused_and_the_default_stands() {
    let resolution = KeybindingPolicy::linux_mvp()
        .with_overrides(&[(NavigationAction::OpenHelp, chord("Ctrl+Alt+P"))]);
    assert_eq!(resolution.refusals.len(), 1);
    assert_eq!(
        resolution.refusals[0].reason,
        RefusalReason::Collision {
            with: NavigationAction::OpenProjectBoard
        }
    );
    assert_eq!(
        effective(&resolution, NavigationAction::OpenHelp),
        "Ctrl+Alt+K"
    );
    assert_eq!(
        effective(&resolution, NavigationAction::OpenProjectBoard),
        "Ctrl+Alt+P"
    );
}

/// Two rebinds to the same chord: **both** are refused, and the order they were
/// written in changes nothing -- last-wins is exactly what this must not be.
#[test]
fn two_rebinds_to_one_chord_are_both_refused_whatever_the_order() {
    let a = (NavigationAction::OpenHelp, chord("Ctrl+Alt+J"));
    let b = (NavigationAction::OpenFolderBrowser, chord("Ctrl+Alt+J"));
    let forward = KeybindingPolicy::linux_mvp().with_overrides(&[a, b]);
    let backward = KeybindingPolicy::linux_mvp().with_overrides(&[b, a]);
    for resolution in [&forward, &backward] {
        assert_eq!(resolution.refusals.len(), 2, "{:?}", resolution.refusals);
        assert_eq!(
            effective(resolution, NavigationAction::OpenHelp),
            "Ctrl+Alt+K"
        );
        assert_eq!(
            effective(resolution, NavigationAction::OpenFolderBrowser),
            "Ctrl+Alt+B"
        );
    }
    assert_eq!(
        forward.policy, backward.policy,
        "the resolved policy is order-independent"
    );
}

/// A swap is not a collision: every chord is still unique afterwards.
#[test]
fn swapping_two_actions_chords_is_accepted() {
    let resolution = KeybindingPolicy::linux_mvp().with_overrides(&[
        (NavigationAction::OpenHelp, chord("Ctrl+Alt+B")),
        (NavigationAction::OpenFolderBrowser, chord("Ctrl+Alt+K")),
    ]);
    assert!(resolution.refusals.is_empty(), "{:?}", resolution.refusals);
    assert_eq!(
        effective(&resolution, NavigationAction::OpenHelp),
        "Ctrl+Alt+B"
    );
    assert_eq!(
        effective(&resolution, NavigationAction::OpenFolderBrowser),
        "Ctrl+Alt+K"
    );
}

/// Refusing one rebind can restore a default another rebind now collides with.
/// The resolution repeats until nothing collides, so the result never holds two
/// actions on one chord -- whatever combination was asked for.
#[test]
fn refusing_one_rebind_can_refuse_another_and_the_result_never_holds_a_duplicate() {
    // Help wants Board's chord and Board moves to Terminal's chord, which
    // collides: Board's move is refused, restoring its default, which Help's
    // rebind now collides with.
    let resolution = KeybindingPolicy::linux_mvp().with_overrides(&[
        (NavigationAction::OpenHelp, chord("Ctrl+Alt+P")),
        (NavigationAction::OpenProjectBoard, chord("Ctrl+Alt+T")),
    ]);
    assert_eq!(resolution.refusals.len(), 2, "{:?}", resolution.refusals);
    assert_eq!(
        effective(&resolution, NavigationAction::OpenHelp),
        "Ctrl+Alt+K"
    );
    assert_eq!(
        effective(&resolution, NavigationAction::OpenProjectBoard),
        "Ctrl+Alt+P"
    );

    let mut seen = std::collections::BTreeSet::new();
    for rule in &resolution.policy.rules {
        if let Some(chord) = rule.effective_binding() {
            assert!(seen.insert(chord.to_owned()), "{chord} reaches two actions");
        }
    }
}
