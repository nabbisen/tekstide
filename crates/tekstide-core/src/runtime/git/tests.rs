//! RFC-030 D7's adversarial fixture, and PR-030-A's required tests. Every
//! fixture repository is built under a fresh temporary directory with its
//! own `HOME`/`GIT_CONFIG_SYSTEM`/`GIT_CONFIG_GLOBAL` (an empty global
//! config, pointed here rather than at the developer's real one), so a
//! green run does not depend on this machine (§2 of
//! `what-git-integration-must-not-do.md`). Every marker script writes one
//! file and nothing else (§3).
//!
//! **Read `hostile_fixtures_are_provably_hostile` first.** It is the
//! ablation the checklist requires before anything else: with the gate
//! not consulted at all -- a real, unprotected `git status`/`git diff`
//! run directly against each poisoned repository -- the marker appears.
//! A fixture that cannot be made to fail proves nothing about what
//! [`evaluate`] then refuses.

use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture {
    root: PathBuf,
    repo: PathBuf,
    markers: PathBuf,
    forwarded_env: Vec<(String, String)>,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "tekstide-git-gate-{name}-{}-{nonce}",
            std::process::id()
        ));
        let repo = root.join("repo");
        let markers = root.join("markers");
        let home = root.join("home");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&markers).unwrap();
        fs::create_dir_all(&home).unwrap();
        // An empty "global" config, so a real ~/.gitconfig on the
        // developer's machine is never consulted -- D7's own requirement.
        let global_config = home.join(".gitconfig");
        fs::write(&global_config, "").unwrap();
        let system_config = root.join("gitconfig-system");
        fs::write(&system_config, "").unwrap();

        let forwarded_env = vec![
            ("HOME".to_string(), home.display().to_string()),
            (
                "GIT_CONFIG_GLOBAL".to_string(),
                global_config.display().to_string(),
            ),
            (
                "GIT_CONFIG_SYSTEM".to_string(),
                system_config.display().to_string(),
            ),
        ];

        let fixture = Self {
            root,
            repo,
            markers,
            forwarded_env,
        };
        fixture.setup_git(&["init", "-q"]);
        fixture
    }

    /// Runs a *setup* git command directly (not through [`evaluate`]):
    /// building the fixture's own history is not the property under test.
    /// Uses the same isolated `HOME`/`GIT_CONFIG_*` as the fixture proper,
    /// so setup itself never touches the developer's real configuration.
    fn setup_git(&self, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(&self.repo)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .envs(self.forwarded_env.iter().cloned())
            .env("GIT_AUTHOR_NAME", "fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .status()
            .expect("git must be installed to run this test suite");
        assert!(status.success(), "fixture setup `git {args:?}` failed");
    }

    fn write(&self, relative: &str, contents: &str) {
        fs::write(self.repo.join(relative), contents).unwrap();
    }

    fn set_config(&self, key: &str, value: &str) {
        self.setup_git(&["config", key, value]);
    }

    /// Commits `tracked.txt`, then overwrites it with a same-length,
    /// different-content replacement. A same-length replacement is
    /// deliberate: a size-mismatch modification lets a stat-based fast
    /// path answer "different" without reading file content at all,
    /// which is exactly the false negative review 405 corrects (the
    /// implementer's first probe passed for that reason, not because
    /// nothing executes). Only a same-length change forces the real
    /// content-comparison path a poisoned filter or textconv driver hooks.
    fn commit_then_modify_same_length(&self) {
        self.write("tracked.txt", "hello world\n");
        self.setup_git(&["add", "-A"]);
        self.setup_git(&["commit", "-q", "-m", "initial"]);
        self.write("tracked.txt", "HELLO WORLD\n");
    }

    /// A harmless marker script: it touches `name` under the fixture's
    /// marker directory and nothing else (§3). `body` is appended
    /// verbatim after the marker `touch`, for scripts that must still
    /// behave like a real filter (a clean filter must echo its stdin back,
    /// or git treats the "filtered" content as empty).
    fn marker_script(&self, name: &str, body: &str) -> PathBuf {
        let path = self.root.join(format!("{name}.sh"));
        let marker = self.markers.join(name);
        let script = format!("#!/bin/sh\ntouch '{}'\n{body}\n", marker.display());
        fs::write(&path, script).unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
        path
    }

    fn marker_exists(&self, name: &str) -> bool {
        self.markers.join(name).exists()
    }

    /// Setup itself (`git add`/`git commit` while a poisoned
    /// `filter.*.clean` or `core.fsmonitor` is already configured) can
    /// touch a marker before the property under test ever runs. Callers
    /// clear the marker directory right after setup, so a marker found
    /// afterward can only be attributed to the step being measured.
    fn clear_markers(&self) {
        for entry in fs::read_dir(&self.markers).unwrap().flatten() {
            let _ = fs::remove_file(entry.path());
        }
    }

    fn evaluate(&self) -> GitGateOutcome {
        evaluate_with_environment(&self.repo, GIT_EXECUTABLE, &self.forwarded_env)
    }

    /// Runs a real, unprotected git subcommand against the fixture,
    /// bypassing [`evaluate`] entirely -- used only to prove a poisoned
    /// repository is genuinely hostile (the ablation), never to test this
    /// module's own gating logic.
    fn run_unprotected(&self, args: &[&str]) {
        let _ = Command::new("git")
            .args(args)
            .current_dir(&self.repo)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .envs(self.forwarded_env.iter().cloned())
            .output();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The checklist's first required box: prove each poisoned repository is
/// genuinely hostile *before* trusting that [`evaluate`] refusing it means
/// anything. Every vector here is read with a real, unprotected git
/// command -- not [`evaluate`] -- and the marker must appear. A git
/// upgrade could change this; that is exactly why this stays a permanent
/// test rather than a one-time manual check.
#[test]
fn hostile_fixtures_are_provably_hostile() {
    let clean_filter = Fixture::new("hostile-clean-filter");
    let script = clean_filter.marker_script("marker-filter-clean", "cat");
    clean_filter.set_config("filter.evil.clean", script.to_str().unwrap());
    clean_filter.set_config("filter.evil.required", "true");
    clean_filter.write(".gitattributes", "* filter=evil\n");
    clean_filter.commit_then_modify_same_length();
    clean_filter.clear_markers();
    clean_filter.run_unprotected(&["status", "--porcelain=v2"]);
    assert!(
        clean_filter.marker_exists("marker-filter-clean"),
        "an unprotected `git status` must run a repository-named clean filter, or this fixture proves nothing"
    );

    let fsmonitor = Fixture::new("hostile-fsmonitor");
    let script = fsmonitor.marker_script("marker-fsmonitor", "exit 0");
    fsmonitor.set_config("core.fsmonitor", script.to_str().unwrap());
    fsmonitor.commit_then_modify_same_length();
    fsmonitor.clear_markers();
    fsmonitor.run_unprotected(&["status", "--porcelain=v2"]);
    assert!(
        fsmonitor.marker_exists("marker-fsmonitor"),
        "an unprotected `git status` must run a repository-named fsmonitor, or this fixture proves nothing"
    );

    let textconv = Fixture::new("hostile-textconv");
    let script = textconv.marker_script("marker-textconv", "cat \"$1\"");
    textconv.set_config("diff.evil.textconv", script.to_str().unwrap());
    textconv.write(".gitattributes", "* diff=evil\n");
    textconv.commit_then_modify_same_length();
    textconv.run_unprotected(&["diff"]);
    assert!(
        textconv.marker_exists("marker-textconv"),
        "an unprotected `git diff` must run a repository-named textconv driver, or this fixture proves nothing"
    );

    let include_hidden = Fixture::new("hostile-include-hidden");
    let script = include_hidden.marker_script("marker-filter-clean", "cat");
    let included_config = include_hidden.root.join("included.gitconfig");
    fs::write(
        &included_config,
        format!(
            "[filter \"evil\"]\n\tclean = {}\n\trequired = true\n",
            script.display()
        ),
    )
    .unwrap();
    include_hidden.set_config("include.path", included_config.to_str().unwrap());
    include_hidden.write(".gitattributes", "* filter=evil\n");
    include_hidden.commit_then_modify_same_length();
    include_hidden.clear_markers();
    include_hidden.run_unprotected(&["status", "--porcelain=v2"]);
    assert!(
        include_hidden.marker_exists("marker-filter-clean"),
        "an unprotected `git status` must run a filter defined only via `include.path`, or this fixture proves nothing"
    );
}

#[test]
fn control_repository_is_accepted_and_executes_nothing() {
    let control = Fixture::new("control");
    control.commit_then_modify_same_length();
    assert_eq!(control.evaluate(), GitGateOutcome::Accepted);
    assert!(control.markers.read_dir().unwrap().next().is_none());
}

#[test]
fn clean_filter_repository_is_refused_and_the_marker_never_runs() {
    let fixture = Fixture::new("gate-clean-filter");
    let script = fixture.marker_script("marker-filter-clean", "cat");
    fixture.set_config("filter.evil.clean", script.to_str().unwrap());
    fixture.set_config("filter.evil.required", "true");
    fixture.write(".gitattributes", "* filter=evil\n");
    fixture.commit_then_modify_same_length();
    fixture.clear_markers();

    let outcome = fixture.evaluate();
    assert!(
        matches!(
            outcome,
            GitGateOutcome::Refused(GitGateRefusal::UnknownConfigKey { .. })
        ),
        "expected refusal on the unrecognised filter.evil.clean key, got {outcome:?}"
    );
    assert!(!fixture.marker_exists("marker-filter-clean"));
}

#[test]
fn fsmonitor_repository_is_refused_and_the_marker_never_runs() {
    let fixture = Fixture::new("gate-fsmonitor");
    let script = fixture.marker_script("marker-fsmonitor", "exit 0");
    fixture.set_config("core.fsmonitor", script.to_str().unwrap());
    fixture.commit_then_modify_same_length();
    fixture.clear_markers();

    let outcome = fixture.evaluate();
    assert!(
        matches!(
            outcome,
            GitGateOutcome::Refused(GitGateRefusal::UnknownConfigKey { .. })
        ),
        "expected refusal on the unrecognised core.fsmonitor key, got {outcome:?}"
    );
    assert!(!fixture.marker_exists("marker-fsmonitor"));
}

#[test]
fn textconv_repository_is_refused_and_the_marker_never_runs() {
    let fixture = Fixture::new("gate-textconv");
    let script = fixture.marker_script("marker-textconv", "cat \"$1\"");
    fixture.set_config("diff.evil.textconv", script.to_str().unwrap());
    fixture.write(".gitattributes", "* diff=evil\n");
    fixture.commit_then_modify_same_length();

    let outcome = fixture.evaluate();
    assert!(
        matches!(
            outcome,
            GitGateOutcome::Refused(GitGateRefusal::UnknownConfigKey { .. })
        ),
        "expected refusal on the unrecognised diff.evil.textconv key, got {outcome:?}"
    );
    assert!(!fixture.marker_exists("marker-textconv"));
}

/// The measured bypass (review 405): `git config --list --local` hides a
/// driver defined through `include.path`, while an unprotected `git
/// status` still runs it. This is why [`evaluate`] reads with the default
/// (non-`--local`) listing and refuses on the `include.path` key itself,
/// not only on whatever it turns out to pull in.
#[test]
fn include_hidden_repository_is_refused_on_the_include_key_itself() {
    let fixture = Fixture::new("gate-include-hidden");
    let script = fixture.marker_script("marker-filter-clean", "cat");
    let included_config = fixture.root.join("included.gitconfig");
    fs::write(
        &included_config,
        format!(
            "[filter \"evil\"]\n\tclean = {}\n\trequired = true\n",
            script.display()
        ),
    )
    .unwrap();
    fixture.set_config("include.path", included_config.to_str().unwrap());
    fixture.write(".gitattributes", "* filter=evil\n");
    fixture.commit_then_modify_same_length();
    fixture.clear_markers();

    let outcome = fixture.evaluate();
    assert_eq!(
        outcome,
        GitGateOutcome::Refused(GitGateRefusal::ConfigInclude {
            key: "include.path".to_string(),
        })
    );
    assert!(!fixture.marker_exists("marker-filter-clean"));
}

#[test]
fn includeif_repository_is_refused_on_the_includeif_key_itself() {
    let fixture = Fixture::new("gate-includeif");
    let included_config = fixture.root.join("conditional.gitconfig");
    fs::write(&included_config, "[core]\n\tfilemode = true\n").unwrap();
    fixture.set_config(
        &format!("includeIf.gitdir:{}/.path", fixture.repo.display()),
        included_config.to_str().unwrap(),
    );

    let outcome = fixture.evaluate();
    assert!(
        matches!(
            outcome,
            GitGateOutcome::Refused(GitGateRefusal::ConfigInclude { .. })
        ),
        "expected refusal on the includeIf key, got {outcome:?}"
    );
}

/// D1' item 6: attributes naming a driver refuse the *content* answer even
/// when no config currently defines that driver -- measured to execute
/// nothing, but comparing against an index written through a filter this
/// gate never ran is still a wrong count, not a safe one.
#[test]
fn attributes_naming_an_undefined_driver_is_accepted_branch_only() {
    let fixture = Fixture::new("gate-attributes-undefined");
    fixture.write(".gitattributes", "* filter=undefined-anywhere\n");
    fixture.commit_then_modify_same_length();

    assert_eq!(fixture.evaluate(), GitGateOutcome::AcceptedBranchOnly);
}

#[test]
fn attributes_naming_an_undefined_diff_driver_is_accepted_branch_only() {
    let fixture = Fixture::new("gate-attributes-undefined-diff");
    fixture.write(".gitattributes", "* diff=undefined-anywhere\n");
    fixture.commit_then_modify_same_length();

    assert_eq!(fixture.evaluate(), GitGateOutcome::AcceptedBranchOnly);
}

/// A benign, common key that simply is not on the allowlist -- proving
/// the allowlist is default-deny, not merely a denylist of the vectors
/// this fixture happens to name.
#[test]
fn an_unrecognised_but_harmless_key_is_still_refused() {
    let fixture = Fixture::new("gate-unknown-key");
    fixture.set_config("core.editor", "true");

    let outcome = fixture.evaluate();
    assert_eq!(
        outcome,
        GitGateOutcome::Refused(GitGateRefusal::UnknownConfigKey {
            key: "core.editor".to_string(),
        })
    );
}

#[test]
fn the_gate_does_not_depend_on_trust_state() {
    use crate::project::{ProjectId, ProjectSession, WorkspaceTrust};

    let trusted_fixture = Fixture::new("gate-trust-trusted");
    trusted_fixture.commit_then_modify_same_length();
    let mut trusted = ProjectSession::new(
        ProjectId::new_uuid(),
        "trusted".to_string(),
        trusted_fixture.repo.clone(),
        trusted_fixture.repo.clone(),
    );
    trusted.grant_trust("test fixture");

    let restricted_fixture = Fixture::new("gate-trust-restricted");
    restricted_fixture.commit_then_modify_same_length();
    let restricted = ProjectSession::new(
        ProjectId::new_uuid(),
        "restricted".to_string(),
        restricted_fixture.repo.clone(),
        restricted_fixture.repo.clone(),
    );

    // Same shape of repository, one Trusted and one Restricted session:
    // `evaluate` takes no trust parameter at all (D1' item 7), so both
    // sessions' own trust state is read here only to show it plays no
    // part in the call -- the outcome depends solely on the repository.
    assert_eq!(trusted.trust_state(), WorkspaceTrust::Trusted);
    assert_eq!(restricted.trust_state(), WorkspaceTrust::Restricted);
    assert_eq!(trusted_fixture.evaluate(), GitGateOutcome::Accepted);
    assert_eq!(restricted_fixture.evaluate(), GitGateOutcome::Accepted);

    let poisoned_trusted = Fixture::new("gate-trust-trusted-poisoned");
    let script = poisoned_trusted.marker_script("marker-fsmonitor", "exit 0");
    poisoned_trusted.set_config("core.fsmonitor", script.to_str().unwrap());
    let poisoned_restricted = Fixture::new("gate-trust-restricted-poisoned");
    let script = poisoned_restricted.marker_script("marker-fsmonitor", "exit 0");
    poisoned_restricted.set_config("core.fsmonitor", script.to_str().unwrap());

    assert!(matches!(
        poisoned_trusted.evaluate(),
        GitGateOutcome::Refused(_)
    ));
    assert!(matches!(
        poisoned_restricted.evaluate(),
        GitGateOutcome::Refused(_)
    ));
    assert!(!poisoned_trusted.marker_exists("marker-fsmonitor"));
    assert!(!poisoned_restricted.marker_exists("marker-fsmonitor"));
}

#[test]
fn git_not_found_reports_unavailable_not_a_panic() {
    let fixture = Fixture::new("gate-git-not-found");
    let outcome = evaluate_with_environment(
        &fixture.repo,
        "definitely-not-a-real-git-binary-tekstide-test",
        &fixture.forwarded_env,
    );
    assert_eq!(
        outcome,
        GitGateOutcome::Refused(GitGateRefusal::Unavailable(GitUnavailableReason::NotFound))
    );
}

#[test]
fn parse_git_version_reads_the_real_toolchain_output() {
    assert_eq!(parse_git_version("git version 2.55.0\n"), Some((2, 55, 0)));
    assert_eq!(
        parse_git_version("git version 2.34.1.windows.1\n"),
        Some((2, 34, 1))
    );
    assert_eq!(parse_git_version("git version 2.30\n"), Some((2, 30, 0)));
    assert_eq!(parse_git_version("not a version string"), None);
    assert_eq!(parse_git_version(""), None);
}

#[test]
fn a_version_below_the_minimum_is_too_old() {
    assert!((2, 20, 0) < MIN_GIT_VERSION);
    assert!((2, 55, 0) >= MIN_GIT_VERSION);
}
