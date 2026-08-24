use std::path::{Path, PathBuf};

use tekstide_core::navigation::{KeybindingPolicy, KeybindingStatus, NavigationAction};

use super::{action_catalog_key, keyboard_help_lines, usage_text};
use crate::i18n::{Catalog, LocalePreference};

fn real_locales_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("locales")
}

fn real_catalog() -> Catalog {
    Catalog::resolve(LocalePreference::default(), Some(&real_locales_dir()))
}

/// `action_catalog_key`'s own contract, tested directly rather than
/// through `keyboard_help_lines`: a description exists if and only if
/// the action is actually live. One direction alone (dead actions get
/// no description) can be satisfied vacuously if the composed path
/// never reaches a dead action regardless of what this function
/// returns for it -- which is exactly how a prior audit's "describing
/// the palette" ablation went unnoticed: it changed this function
/// *and* the core filter together, so three tests failed for the
/// filter change alone while this function's own arm was never
/// exercised. Testing both directions here, independent of any
/// caller, is what would have caught it. The expected set is derived
/// from `KeybindingPolicy::linux_mvp()` itself, not a hand-written
/// list of dead actions, so a rule changing status cannot make this
/// test stale.
#[test]
fn action_catalog_key_is_some_iff_the_action_is_live() {
    let policy = KeybindingPolicy::linux_mvp();
    for rule in &policy.rules {
        let is_live = rule.status == KeybindingStatus::Candidate && rule.default_binding.is_some();
        let key = action_catalog_key(rule.action);
        assert_eq!(
            key.is_some(),
            is_live,
            "{:?}: action_catalog_key returned {:?}, but is_live is {is_live} \
             (status={:?}, default_binding={:?}) -- a live action must have a \
             description and a dead or reserved one must not",
            rule.action,
            key,
            rule.status,
            rule.default_binding
        );
    }
}

/// The defect this whole module exists to fix: nine bindings were live
/// and the shipped application named none of them. An exact count, not
/// `>= 1` -- per `ARCHITECTURE.md`'s enumeration-test unit rule, the
/// property here is "every live binding is described", so a rule
/// quietly changing status must fail this rather than shrink the help
/// silently.
#[test]
fn every_live_binding_is_described_to_the_user() {
    let catalog = real_catalog();
    let lines = keyboard_help_lines(&catalog);

    assert_eq!(
        lines.len(),
        12,
        "expected the twelve Candidate rules with a default binding to be described; \
         got {}: {:?}",
        lines.len(),
        lines.iter().map(|line| line.binding).collect::<Vec<_>>()
    );

    for line in &lines {
        assert!(
            !line.description.is_empty(),
            "{} has an empty description",
            line.binding
        );
        assert!(
            !line.description.starts_with("keyboard-help-"),
            "{} rendered the catalog key itself ({}), which means the key is missing \
             from en.ftl -- Catalog::get falls back to the key rather than panicking, \
             so this is the only thing that catches it",
            line.binding,
            line.description
        );
    }
}

/// The empty state used to render "Add Project" and "Open from path" as
/// inert labels for actions that did not exist. The same failure in help
/// text would be worse, because help is what a user trusts when nothing
/// else in the product tells them anything.
#[test]
fn no_action_without_a_working_binding_is_advertised() {
    let catalog = real_catalog();
    let advertised: Vec<&'static str> = keyboard_help_lines(&catalog)
        .into_iter()
        .map(|line| line.binding)
        .collect();

    let policy = KeybindingPolicy::linux_mvp();
    for rule in &policy.rules {
        let is_live = rule.status == KeybindingStatus::Candidate && rule.default_binding.is_some();
        if is_live {
            continue;
        }
        if let Some(binding) = rule.default_binding {
            assert!(
                !advertised.contains(&binding),
                "{binding} is {:?}, not a live binding, and must not be offered to a user",
                rule.status
            );
        }
    }

    // Named explicitly as well as derived: `Ctrl+Shift+P` is reserved so
    // nothing else claims it, and there is no command palette behind it.
    assert!(
        !advertised.contains(&"Ctrl+Shift+P"),
        "the reserved command-palette binding must never be advertised"
    );

    // The four dead actions, named so this test states the fact rather
    // than only computing it.
    for dead in [
        NavigationAction::SwitchActiveProject,
        NavigationAction::CycleVisibleTerminalSession,
        NavigationAction::OpenDiffReview,
        NavigationAction::OpenSafeCloseDialog,
    ] {
        let rule = policy
            .rule_for(dead)
            .expect("every action has a rule in linux_mvp()");
        assert!(
            rule.default_binding.is_none(),
            "{dead:?} gained a binding -- decide whether it is user-visible in \
             action_catalog_key() and update this test's list"
        );
    }
}

#[test]
fn usage_text_lists_every_binding_the_gui_lists() {
    let catalog = real_catalog();
    let usage = usage_text(&catalog, "tekstide");

    for line in keyboard_help_lines(&catalog) {
        assert!(
            usage.contains(line.binding),
            "--help omitted {}, so the terminal and the window disagree about what \
             the product can do",
            line.binding
        );
        assert!(
            usage.contains(&line.description),
            "--help omitted the description of {}",
            line.binding
        );
    }
}

/// `tekstide --help` printed `folder does not exist: --help` before
/// `0.12.1`, because every argument was treated as a project path. The
/// usage text has to say the one thing a user who just opened an empty
/// window needs: how a project gets onto the board -- as of RFC-038
/// PR-038-A/B, that is no longer "there is no in-app way," so this
/// asserts the *current* truth rather than the correction-release-era
/// limitation it used to.
#[test]
fn usage_text_says_how_to_open_a_project() {
    let usage = usage_text(&real_catalog(), "tekstide");
    assert!(usage.contains("PROJECT_PATH"));
    assert!(
        usage.contains("a field to type or paste"),
        "usage must describe the real in-app path field, not claim one still does not exist: \
         {usage:?}"
    );
    assert!(
        !usage.contains("no in-app way to add a project"),
        "this claim is stale as of RFC-038 PR-038-A -- an in-app way exists now: {usage:?}"
    );
}
