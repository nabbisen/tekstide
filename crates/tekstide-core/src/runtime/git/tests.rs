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
use std::path::{Path, PathBuf};
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
        self.setup_git_in(&self.repo, args);
    }

    /// Like [`Fixture::setup_git`], but in an arbitrary directory under the
    /// fixture -- used to build a second repository (R1's submodule) that
    /// is not `self.repo` itself.
    fn setup_git_in(&self, dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
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

    fn compute_summary(&self) -> ProjectGitSummary {
        compute_summary_with_environment(&self.repo, GIT_EXECUTABLE, &self.forwarded_env)
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

    /// R1 (review 406): registers `sub` as a real submodule of `self.repo`
    /// via `git submodule add` (so the gitlink and the nested checkout are
    /// exactly what real git produces, not a hand-rolled approximation),
    /// then strips every `submodule.*` key from the parent's config and
    /// deletes `.gitmodules` -- reproducing review 406's own measurement
    /// exactly: the parent's *own* configuration stays entirely
    /// allowlist-clean, and the bypass still works. Poisons the
    /// submodule's own `filter.evil.clean` and modifies its tracked file
    /// in place at the same length (same discipline as
    /// `commit_then_modify_same_length`, applied inside the submodule).
    /// Returns the marker name the poisoned submodule's clean filter
    /// writes.
    fn add_poisoned_submodule(&self) -> &'static str {
        let sub_origin = self.root.join("sub-origin");
        fs::create_dir_all(&sub_origin).unwrap();
        self.setup_git_in(&sub_origin, &["init", "-q"]);
        fs::write(sub_origin.join("tracked.txt"), "hello world\n").unwrap();
        self.setup_git_in(&sub_origin, &["add", "-A"]);
        self.setup_git_in(&sub_origin, &["commit", "-q", "-m", "initial"]);

        self.setup_git(&[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            sub_origin.to_str().unwrap(),
            "sub",
        ]);

        let sub = self.repo.join("sub");
        let script = self.marker_script("marker-sub-clean", "cat");
        self.setup_git_in(
            &sub,
            &["config", "filter.evil.clean", script.to_str().unwrap()],
        );
        self.setup_git_in(&sub, &["config", "filter.evil.required", "true"]);
        fs::write(sub.join(".gitattributes"), "* filter=evil\n").unwrap();
        fs::write(sub.join("tracked.txt"), "hello world\n").unwrap();
        self.setup_git_in(&sub, &["add", "-A"]);
        self.setup_git_in(&sub, &["commit", "-q", "-m", "poison sub"]);
        fs::write(sub.join("tracked.txt"), "HELLO WORLD\n").unwrap();

        // The parent's own configuration must stay allowlist-clean, or
        // this proves only that an unrecognised `submodule.*` key gets
        // refused -- not the actual bypass. Matches review 406's own
        // reproduction, which removed exactly these after `submodule add`.
        self.setup_git(&["config", "--remove-section", "submodule.sub"]);
        fs::remove_file(self.repo.join(".gitmodules")).unwrap();

        "marker-sub-clean"
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

    let submodule = Fixture::new("hostile-submodule");
    let marker_name = submodule.add_poisoned_submodule();
    submodule.clear_markers();
    submodule.run_unprotected(&["status", "--porcelain=v2"]);
    assert!(
        submodule.marker_exists(marker_name),
        "an unprotected `git status` in the parent must run a submodule's own clean filter, or this fixture proves nothing (R1, review 406)"
    );
}

#[test]
fn control_repository_is_accepted_and_executes_nothing() {
    let control = Fixture::new("control");
    control.commit_then_modify_same_length();
    assert_eq!(control.evaluate(), GitGateOutcome::Accepted);
    assert!(control.markers.read_dir().unwrap().next().is_none());
}

/// R7 (review 407, D1' amendment): a repository naming a program in its
/// configuration withholds the content answer -- `AcceptedBranchOnly`,
/// never `Refused` -- and, unconditionally, never runs that program.
#[test]
fn clean_filter_repository_is_accepted_branch_only_and_the_marker_never_runs() {
    let fixture = Fixture::new("gate-clean-filter");
    let script = fixture.marker_script("marker-filter-clean", "cat");
    fixture.set_config("filter.evil.clean", script.to_str().unwrap());
    fixture.set_config("filter.evil.required", "true");
    fixture.write(".gitattributes", "* filter=evil\n");
    fixture.commit_then_modify_same_length();
    fixture.clear_markers();

    assert_eq!(fixture.evaluate(), GitGateOutcome::AcceptedBranchOnly);
    assert!(!fixture.marker_exists("marker-filter-clean"));
}

#[test]
fn fsmonitor_repository_is_accepted_branch_only_and_the_marker_never_runs() {
    let fixture = Fixture::new("gate-fsmonitor");
    let script = fixture.marker_script("marker-fsmonitor", "exit 0");
    fixture.set_config("core.fsmonitor", script.to_str().unwrap());
    fixture.commit_then_modify_same_length();
    fixture.clear_markers();

    assert_eq!(fixture.evaluate(), GitGateOutcome::AcceptedBranchOnly);
    assert!(!fixture.marker_exists("marker-fsmonitor"));
}

#[test]
fn textconv_repository_is_accepted_branch_only_and_the_marker_never_runs() {
    let fixture = Fixture::new("gate-textconv");
    let script = fixture.marker_script("marker-textconv", "cat \"$1\"");
    fixture.set_config("diff.evil.textconv", script.to_str().unwrap());
    fixture.write(".gitattributes", "* diff=evil\n");
    fixture.commit_then_modify_same_length();

    assert_eq!(fixture.evaluate(), GitGateOutcome::AcceptedBranchOnly);
    assert!(!fixture.marker_exists("marker-textconv"));
}

/// The measured bypass (review 405): `git config --list --local` hides a
/// driver defined through `include.path`, while an unprotected `git
/// status` still runs it. `evaluate` reads with the default
/// (non-`--local`) listing, and withholds the content answer on the
/// `include.path` key itself (R7: `AcceptedBranchOnly`, not `Refused` --
/// but still never `Accepted`), not only on whatever it turns out to pull
/// in.
#[test]
fn include_hidden_repository_is_accepted_branch_only() {
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

    assert_eq!(fixture.evaluate(), GitGateOutcome::AcceptedBranchOnly);
    assert!(!fixture.marker_exists("marker-filter-clean"));
}

#[test]
fn includeif_repository_is_accepted_branch_only() {
    let fixture = Fixture::new("gate-includeif");
    let included_config = fixture.root.join("conditional.gitconfig");
    fs::write(&included_config, "[core]\n\tfilemode = true\n").unwrap();
    fixture.set_config(
        &format!("includeIf.gitdir:{}/.path", fixture.repo.display()),
        included_config.to_str().unwrap(),
    );

    assert_eq!(fixture.evaluate(), GitGateOutcome::AcceptedBranchOnly);
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
/// this fixture happens to name. R7: withholds the content answer
/// (`AcceptedBranchOnly`) rather than refusing the repository outright --
/// `core.editor` never gets a chance to run either way, since nothing
/// this gate calls invokes it, and the branch stays readable.
#[test]
fn an_unrecognised_but_harmless_key_is_accepted_branch_only() {
    let fixture = Fixture::new("gate-unknown-key");
    fixture.set_config("core.editor", "true");

    assert_eq!(fixture.evaluate(), GitGateOutcome::AcceptedBranchOnly);
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

    assert_eq!(
        poisoned_trusted.evaluate(),
        GitGateOutcome::AcceptedBranchOnly
    );
    assert_eq!(
        poisoned_restricted.evaluate(),
        GitGateOutcome::AcceptedBranchOnly
    );
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
        GitGateOutcome::Refused(GitUnavailableReason::NotFound)
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

// ---------------------------------------------------------------------
// Review 406: R1-R5, a repository the gate accepted that still ran a
// program it named.
// ---------------------------------------------------------------------

/// R1: the parent's own configuration is entirely allowlisted -- no
/// `submodule.*` key, no `.gitmodules` -- yet the submodule it gitlinks to
/// has its own poisoned `filter.evil.clean`. `evaluate` must not return
/// `Accepted` for this repository, and the gitlink-detection primitive
/// (`git ls-files -s`) must not itself trigger the marker.
#[test]
fn a_repository_with_a_poisoned_submodule_is_accepted_branch_only() {
    let fixture = Fixture::new("gate-submodule");
    let marker_name = fixture.add_poisoned_submodule();
    fixture.clear_markers();

    let outcome = fixture.evaluate();
    assert_eq!(
        outcome,
        GitGateOutcome::AcceptedBranchOnly,
        "a repository whose configuration is allowlist-clean but whose gitlinked submodule names a program must not be Accepted"
    );
    assert!(!fixture.marker_exists(marker_name));
}

/// R2: a budget too small to finish the walk must fail closed
/// (`AcceptedBranchOnly`), never silently report "no driver found" on a
/// truncated scan. A real 1,000,000-entry fixture would be impractical to
/// build in a test, so this exercises the same code path with a tiny
/// budget over a handful of real files instead.
#[test]
fn attributes_walk_budget_exhaustion_fails_closed() {
    let fixture = Fixture::new("gate-walk-budget");
    fixture.commit_then_modify_same_length();
    fs::create_dir_all(fixture.repo.join("a")).unwrap();
    fs::create_dir_all(fixture.repo.join("b")).unwrap();
    fs::create_dir_all(fixture.repo.join("c")).unwrap();

    // A generous budget still finds nothing: no `.gitattributes` anywhere.
    let generous = evaluate_with_environment_and_walk_budget(
        &fixture.repo,
        GIT_EXECUTABLE,
        &fixture.forwarded_env,
        MAX_ATTRIBUTE_WALK_ENTRIES,
    );
    assert_eq!(generous, GitGateOutcome::Accepted);

    // A budget too small to enumerate even the top-level entries must fail
    // closed rather than answer the same as the generous budget did.
    let starved = evaluate_with_environment_and_walk_budget(
        &fixture.repo,
        GIT_EXECUTABLE,
        &fixture.forwarded_env,
        1,
    );
    assert_eq!(starved, GitGateOutcome::AcceptedBranchOnly);
}

/// R3: a symlinked directory must not be followed out of the repository --
/// `outside/.gitattributes` (naming a driver) must not be found through a
/// symlink inside the repository that points at `outside`. A symlinked
/// *file* masquerading as `.gitattributes` must likewise be ignored.
#[test]
fn attributes_walk_does_not_follow_symlinks() {
    let fixture = Fixture::new("gate-walk-symlink");
    fixture.commit_then_modify_same_length();

    let outside = fixture.root.join("outside-the-repository");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join(".gitattributes"), "* filter=evil\n").unwrap();
    std::os::unix::fs::symlink(&outside, fixture.repo.join("linked-dir")).unwrap();

    let real_attributes = fixture.root.join("real-attributes-naming-a-driver");
    fs::write(&real_attributes, "* filter=evil\n").unwrap();
    std::os::unix::fs::symlink(&real_attributes, fixture.repo.join(".gitattributes")).unwrap();

    assert_eq!(
        fixture.evaluate(),
        GitGateOutcome::Accepted,
        "a symlink (directory or the `.gitattributes` file itself) must never be followed"
    );
}

/// R4: `.git` as a pointer file (a linked worktree, here -- the same shape
/// a submodule checkout leaves) must not silently lose `info/attributes`
/// and fall back to the permissive answer. `git worktree add` produces a
/// real pointer-file `.git`, not a hand-written approximation.
#[test]
fn a_linked_worktrees_pointer_file_git_dir_is_still_resolved() {
    let fixture = Fixture::new("gate-linked-worktree");
    fixture.commit_then_modify_same_length();
    fixture.setup_git(&["branch", "other"]);

    let worktree_path = fixture.root.join("linked-worktree");
    fixture.setup_git(&["worktree", "add", worktree_path.to_str().unwrap(), "other"]);
    assert!(
        worktree_path.join(".git").is_file(),
        "git worktree add must leave a pointer file, not a directory, at .git"
    );

    let outcome = evaluate_with_environment(&worktree_path, GIT_EXECUTABLE, &fixture.forwarded_env);
    assert_eq!(outcome, GitGateOutcome::Accepted);

    // `info/attributes` is shared across every worktree, under the
    // *common* dir -- not the linked worktree's own private gitdir (which
    // has no `info/` of its own; this is exactly the gap found while
    // writing this test, fixed in `resolve_git_common_dir`'s doc comment).
    // Poison it there and confirm the outcome changes.
    let common_dir_output = std::process::Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(&worktree_path)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .envs(fixture.forwarded_env.iter().cloned())
        .output()
        .unwrap();
    let common_dir =
        worktree_path.join(String::from_utf8(common_dir_output.stdout).unwrap().trim());
    fs::write(
        common_dir.join("info").join("attributes"),
        "* filter=evil\n",
    )
    .unwrap();

    let outcome_after_poison =
        evaluate_with_environment(&worktree_path, GIT_EXECUTABLE, &fixture.forwarded_env);
    assert_eq!(outcome_after_poison, GitGateOutcome::AcceptedBranchOnly);
}

/// R5: an oversized `.gitattributes` must fail closed (`AcceptedBranchOnly`)
/// rather than being read in full -- unbounded, on a file a hostile
/// repository controls.
#[test]
fn an_oversized_attributes_file_fails_closed() {
    let fixture = Fixture::new("gate-oversized-attributes");
    fixture.commit_then_modify_same_length();

    let oversized = "# padding\n".repeat((MAX_ATTRIBUTES_FILE_BYTES as usize / 10) + 1);
    assert!(oversized.len() as u64 > MAX_ATTRIBUTES_FILE_BYTES);
    fs::write(fixture.repo.join(".gitattributes"), oversized).unwrap();

    assert_eq!(fixture.evaluate(), GitGateOutcome::AcceptedBranchOnly);
}

// ---------------------------------------------------------------------
// Review 407: R6-R7, the gate answering "not available" for reasons that
// had nothing to do with the repository being read.
// ---------------------------------------------------------------------

/// R6: measured directly against a real developer's machine, forwarding
/// the developer's own global/system git configuration meant `evaluate`
/// refused this project's own repository on `user.signingkey` -- a
/// personal key, not the repository's. `GIT_CONFIG_GLOBAL` is hardcoded to
/// `/dev/null` in production regardless of what `HOME` points to or what a
/// caller's `forwarded_env` supplies (the fixture's own D7-era
/// `GIT_CONFIG_GLOBAL` entry points at exactly this file, and even that is
/// overridden). This repopulates the fixture's `HOME/.gitconfig` -- what
/// git would read as the global config if the override were not in place
/// -- with an unrecognised key and confirms it changes nothing.
#[test]
fn the_users_own_global_configuration_does_not_affect_the_outcome() {
    let fixture = Fixture::new("gate-global-config-neutralised");
    fixture.commit_then_modify_same_length();

    let home_gitconfig = fixture.root.join("home").join(".gitconfig");
    fs::write(
        &home_gitconfig,
        "[user]\n\tsigningkey = deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n",
    )
    .unwrap();

    assert_eq!(fixture.evaluate(), GitGateOutcome::Accepted);
}

/// R7: the allowlist grows by pattern for ordinary tool-written data keys
/// -- each added as its own reviewed safety judgment (module doc comment),
/// not a blanket loosening. A repository carrying exactly these, and
/// nothing else unrecognised, is fully `Accepted` -- none of them affect
/// what any command this gate or a status/dirty read performs.
#[test]
fn tool_written_data_keys_are_allowed() {
    let fixture = Fixture::new("gate-tool-written-keys");
    fixture.commit_then_modify_same_length();
    fixture.set_config("branch.main.vscode-merge-base", "origin/main");
    fixture.set_config("remote.origin.gh-resolved", "base");
    fixture.set_config("submodule.sub.active", "true");
    fixture.set_config("lfs.url", "https://example.invalid/lfs");

    assert_eq!(fixture.evaluate(), GitGateOutcome::Accepted);
}

// ---------------------------------------------------------------------
// PR-030-B: `compute_summary` -- turning the gate's decision into a
// `ProjectGitSummary`. No production caller yet; these test the
// computation layer directly.
// ---------------------------------------------------------------------

#[test]
fn a_clean_control_repository_is_complete_with_zero_changes_and_no_upstream() {
    let fixture = Fixture::new("summary-clean-control");
    fixture.setup_git(&["checkout", "-q", "-b", "main"]);
    fixture.write("tracked.txt", "hello world\n");
    fixture.setup_git(&["add", "-A"]);
    fixture.setup_git(&["commit", "-q", "-m", "initial"]);

    let summary = fixture.compute_summary();
    assert_eq!(
        summary,
        ProjectGitSummary {
            provider_state: ProjectProviderState::Complete,
            branch_name: Some("main".to_string()),
            changed_file_count: Some(0),
            ahead_count: None,
            behind_count: None,
            file_statuses: Some(BTreeMap::new()),
        }
    );
}

#[test]
fn a_dirty_control_repository_counts_its_changed_files() {
    let fixture = Fixture::new("summary-dirty-control");
    fixture.setup_git(&["checkout", "-q", "-b", "main"]);
    fixture.write("tracked.txt", "hello world\n");
    fixture.setup_git(&["add", "-A"]);
    fixture.setup_git(&["commit", "-q", "-m", "initial"]);
    fixture.write("tracked.txt", "modified\n");
    fixture.write("untracked.txt", "new\n");

    let summary = fixture.compute_summary();
    assert_eq!(summary.provider_state, ProjectProviderState::Complete);
    assert_eq!(summary.branch_name, Some("main".to_string()));
    assert_eq!(summary.changed_file_count, Some(2));
}

#[test]
fn ahead_and_behind_are_read_against_a_real_local_upstream() {
    let fixture = Fixture::new("summary-ahead-behind");
    fixture.setup_git(&["checkout", "-q", "-b", "main"]);
    fixture.write("tracked.txt", "one\n");
    fixture.setup_git(&["add", "-A"]);
    fixture.setup_git(&["commit", "-q", "-m", "initial"]);

    // A local "upstream": a second branch two commits ahead, tracked by main.
    fixture.setup_git(&["checkout", "-q", "-b", "upstream"]);
    fixture.write("tracked.txt", "two\n");
    fixture.setup_git(&["commit", "-q", "-am", "second"]);
    fixture.write("tracked.txt", "three\n");
    fixture.setup_git(&["commit", "-q", "-am", "third"]);
    fixture.setup_git(&["checkout", "-q", "main"]);
    fixture.setup_git(&["branch", "-q", "--set-upstream-to=upstream"]);
    fixture.write("tracked.txt", "one-b\n");
    fixture.setup_git(&["commit", "-q", "-am", "local-only"]);

    let summary = fixture.compute_summary();
    assert_eq!(summary.branch_name, Some("main".to_string()));
    assert_eq!(summary.ahead_count, Some(1));
    assert_eq!(summary.behind_count, Some(2));
}

#[test]
fn a_poisoned_repository_is_branch_only_and_the_marker_never_runs() {
    let fixture = Fixture::new("summary-poisoned");
    let script = fixture.marker_script("marker-fsmonitor", "exit 0");
    fixture.set_config("core.fsmonitor", script.to_str().unwrap());
    fixture.setup_git(&["checkout", "-q", "-b", "main"]);
    fixture.commit_then_modify_same_length();
    fixture.clear_markers();

    let summary = fixture.compute_summary();
    assert_eq!(summary.provider_state, ProjectProviderState::Complete);
    assert_eq!(summary.branch_name, Some("main".to_string()));
    assert_eq!(summary.changed_file_count, None);
    assert_eq!(summary.ahead_count, None);
    assert_eq!(summary.behind_count, None);
    assert!(!fixture.marker_exists("marker-fsmonitor"));
}

#[test]
fn a_non_repository_is_unavailable() {
    let root = std::env::temp_dir().join(format!(
        "tekstide-git-gate-summary-non-repo-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();

    let summary = compute_summary_with_environment(&root, GIT_EXECUTABLE, &[]);
    assert_eq!(
        summary,
        ProjectGitSummary {
            provider_state: ProjectProviderState::Unavailable,
            branch_name: None,
            changed_file_count: None,
            ahead_count: None,
            behind_count: None,
            file_statuses: None,
        }
    );

    // A non-repository never spawns `git` at all -- a bogus executable
    // name must still answer `Unavailable`, not `Refused(NotFound)`,
    // proving the short-circuit never reaches `run_bounded_git`.
    let summary_with_no_real_git = compute_summary_with_environment(
        &root,
        "definitely-not-a-real-git-binary-tekstide-test",
        &[],
    );
    assert_eq!(
        summary_with_no_real_git.provider_state,
        ProjectProviderState::Unavailable
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_detached_head_has_no_branch_name_but_is_still_complete() {
    let fixture = Fixture::new("summary-detached-head");
    fixture.write("tracked.txt", "one\n");
    fixture.setup_git(&["add", "-A"]);
    fixture.setup_git(&["commit", "-q", "-m", "initial"]);
    fixture.setup_git(&["checkout", "-q", "--detach", "HEAD"]);

    let summary = fixture.compute_summary();
    assert_eq!(summary.provider_state, ProjectProviderState::Complete);
    assert_eq!(summary.branch_name, None);
}

/// D1' item 8: branch is readable even when `git` itself is `Refused` as
/// missing, since `.git/HEAD` never depends on the `git` binary at all.
#[test]
fn branch_is_still_read_when_git_itself_is_unavailable() {
    let fixture = Fixture::new("summary-git-unavailable");
    fixture.setup_git(&["checkout", "-q", "-b", "main"]);
    fixture.write("tracked.txt", "one\n");
    fixture.setup_git(&["add", "-A"]);
    fixture.setup_git(&["commit", "-q", "-m", "initial"]);

    let summary = compute_summary_with_environment(
        &fixture.repo,
        "definitely-not-a-real-git-binary-tekstide-test",
        &fixture.forwarded_env,
    );
    assert_eq!(summary.provider_state, ProjectProviderState::Complete);
    assert_eq!(summary.branch_name, Some("main".to_string()));
    assert_eq!(summary.changed_file_count, None);
}

/// R4's distinction, from the branch side: a linked worktree's branch is
/// its *own*, read from the private per-worktree gitdir -- not the
/// primary checkout's, which would be wrong for every worktree but the
/// first.
#[test]
fn a_linked_worktrees_branch_is_its_own_not_the_primary_checkouts() {
    let fixture = Fixture::new("summary-linked-worktree-branch");
    fixture.write("tracked.txt", "one\n");
    fixture.setup_git(&["add", "-A"]);
    fixture.setup_git(&["commit", "-q", "-m", "initial"]);
    fixture.setup_git(&["branch", "other"]);

    let worktree_path = fixture.root.join("linked-worktree");
    fixture.setup_git(&["worktree", "add", worktree_path.to_str().unwrap(), "other"]);

    let primary_summary = fixture.compute_summary();
    let worktree_summary =
        compute_summary_with_environment(&worktree_path, GIT_EXECUTABLE, &fixture.forwarded_env);

    assert_eq!(primary_summary.branch_name, Some("master".to_string()));
    assert_eq!(worktree_summary.branch_name, Some("other".to_string()));
}

/// Defence in depth (review 407/408): `--ignore-submodules=all` suppresses
/// the submodule filter even called directly against a poisoned submodule
/// -- never a substitute for R1's refusal (which already keeps
/// `read_status_summary` from ever being reached for a real gitlinked
/// repository through `compute_summary`), but its own protection holds
/// independently.
#[test]
fn ignore_submodules_all_suppresses_the_submodule_filter_on_its_own() {
    let fixture = Fixture::new("summary-ignore-submodules");
    let marker_name = fixture.add_poisoned_submodule();
    fixture.clear_markers();

    let _ = read_status_summary(&fixture.repo, GIT_EXECUTABLE, &fixture.forwarded_env);
    assert!(!fixture.marker_exists(marker_name));
}

/// `compute_summary` takes no trust parameter, the same structural
/// argument `the_gate_does_not_depend_on_trust_state` already makes for
/// `evaluate` -- exercised again here since `compute_summary` is a
/// separate entry point with its own call graph, not merely a thin
/// wrapper whose trust-independence could be assumed from the gate's own
/// test.
#[test]
fn compute_summary_does_not_depend_on_trust_state() {
    use crate::project::{ProjectId, ProjectSession, WorkspaceTrust};

    let trusted_fixture = Fixture::new("summary-trust-trusted");
    trusted_fixture.setup_git(&["checkout", "-q", "-b", "main"]);
    trusted_fixture.commit_then_modify_same_length();
    let mut trusted = ProjectSession::new(
        ProjectId::new_uuid(),
        "trusted".to_string(),
        trusted_fixture.repo.clone(),
        trusted_fixture.repo.clone(),
    );
    trusted.grant_trust("test fixture");

    let restricted_fixture = Fixture::new("summary-trust-restricted");
    restricted_fixture.setup_git(&["checkout", "-q", "-b", "main"]);
    restricted_fixture.commit_then_modify_same_length();
    let restricted = ProjectSession::new(
        ProjectId::new_uuid(),
        "restricted".to_string(),
        restricted_fixture.repo.clone(),
        restricted_fixture.repo.clone(),
    );

    assert_eq!(trusted.trust_state(), WorkspaceTrust::Trusted);
    assert_eq!(restricted.trust_state(), WorkspaceTrust::Restricted);
    assert_eq!(
        trusted_fixture.compute_summary(),
        restricted_fixture.compute_summary()
    );
}

// ---------------------------------------------------------------------
// PR-030-C, REQ-GIT-003: per-file status, `ProjectGitSummary::file_statuses`.
// ---------------------------------------------------------------------

/// Modified, added (staged), deleted (worktree) and untracked, all from
/// one ordinary `git status` pass -- no special setup needed beyond
/// writing/removing files, since these are the four categories the
/// default rename/copy-detection-free path already distinguishes. An
/// unmodified tracked file and a path with no entry at all both carry no
/// status -- absence is not itself a status.
#[test]
fn a_dirty_repositorys_file_statuses_map_the_real_change_each_file_carries() {
    let fixture = Fixture::new("per-file-ordinary");
    fixture.setup_git(&["checkout", "-q", "-b", "main"]);
    fixture.write("modified.txt", "one\n");
    fixture.write("deleted.txt", "gone\n");
    fixture.write("unchanged.txt", "never touched again\n");
    fixture.setup_git(&["add", "-A"]);
    fixture.setup_git(&["commit", "-q", "-m", "initial"]);

    fixture.write("modified.txt", "two\n");
    fs::remove_file(fixture.repo.join("deleted.txt")).unwrap();
    fixture.write("added.txt", "new, staged\n");
    fixture.setup_git(&["add", "added.txt"]);
    fixture.write("untracked.txt", "new, never staged\n");

    let summary = fixture.compute_summary();
    assert_eq!(
        summary.file_status(Path::new("modified.txt")),
        Some(FileGitStatus::Modified)
    );
    assert_eq!(
        summary.file_status(Path::new("deleted.txt")),
        Some(FileGitStatus::Deleted)
    );
    assert_eq!(
        summary.file_status(Path::new("added.txt")),
        Some(FileGitStatus::Added)
    );
    assert_eq!(
        summary.file_status(Path::new("untracked.txt")),
        Some(FileGitStatus::Untracked)
    );
    assert_eq!(
        summary.file_status(Path::new("unchanged.txt")),
        None,
        "a tracked file with no real change must carry no status"
    );
    assert_eq!(
        summary.file_status(Path::new("does-not-exist.txt")),
        None,
        "a path with no entry at all must carry no status, not a guessed default"
    );
    assert_eq!(summary.changed_file_count, Some(4));
}

/// Documents a real, deliberate property of the explorer-wiring slice
/// (review 413's directory question, `surface/explorer.rs`'s own doc
/// comment): a directory that is *entirely* untracked is exactly what
/// `git status` collapses into one `?? dir/` record -- no per-node
/// rollup logic is written anywhere for this; the directory's own row
/// gets an `Untracked` badge only because git's own default
/// `--untracked-files` mode (left unchanged here, the same mode
/// PR-030-B's status bar count already depends on) reports it that way,
/// and `PathBuf::from("dir/")` compares equal to `PathBuf::from("dir")`.
#[test]
fn a_wholly_untracked_directory_is_reported_at_its_own_collapsed_path() {
    let fixture = Fixture::new("per-file-untracked-directory");
    fixture.setup_git(&["checkout", "-q", "-b", "main"]);
    fixture.setup_git(&["commit", "-q", "-m", "initial", "--allow-empty"]);

    fs::create_dir(fixture.repo.join("newdir")).unwrap();
    fixture.write("newdir/one.txt", "one\n");
    fixture.write("newdir/two.txt", "two\n");

    let summary = fixture.compute_summary();
    assert_eq!(
        summary.file_status(Path::new("newdir")),
        Some(FileGitStatus::Untracked)
    );
    assert_eq!(
        summary.file_status(Path::new("newdir/one.txt")),
        None,
        "git collapsed the directory into one record; the files inside are not enumerated"
    );
    assert_eq!(summary.changed_file_count, Some(1));
}

/// A staged rename lands under its *new* path only -- the one an explorer
/// node's `relative_path` can actually match against -- with the old
/// path carrying no entry. `git mv` moves identical content, so real
/// git's own rename detection reports it at the default similarity
/// threshold without needing a contrived near-miss.
#[test]
fn a_staged_rename_is_captured_at_its_new_path_only() {
    let fixture = Fixture::new("per-file-rename");
    fixture.setup_git(&["checkout", "-q", "-b", "main"]);
    fixture.write("old-name.txt", "identical content, moved to a new path\n");
    fixture.setup_git(&["add", "-A"]);
    fixture.setup_git(&["commit", "-q", "-m", "initial"]);
    fixture.setup_git(&["mv", "old-name.txt", "new-name.txt"]);

    let summary = fixture.compute_summary();
    assert_eq!(
        summary.file_status(Path::new("new-name.txt")),
        Some(FileGitStatus::Renamed)
    );
    assert_eq!(summary.file_status(Path::new("old-name.txt")), None);
    assert_eq!(summary.changed_file_count, Some(1));
}

/// A real merge conflict, from two branches that each changed the same
/// file: the conflicted path reports `Unmerged` regardless of which side
/// changed what -- [`FileGitStatus`]'s own doc comment states why this
/// module does not distinguish `DD`/`AU`/`UD`/`UA`/`DU`/`AA`/`UU` from
/// each other.
#[test]
fn a_merge_conflict_reports_the_file_as_unmerged() {
    let fixture = Fixture::new("per-file-unmerged");
    fixture.setup_git(&["checkout", "-q", "-b", "main"]);
    fixture.write("contested.txt", "base\n");
    fixture.setup_git(&["add", "-A"]);
    fixture.setup_git(&["commit", "-q", "-m", "base"]);

    fixture.setup_git(&["checkout", "-q", "-b", "other"]);
    fixture.write("contested.txt", "other side\n");
    fixture.setup_git(&["commit", "-q", "-am", "other side"]);

    fixture.setup_git(&["checkout", "-q", "main"]);
    fixture.write("contested.txt", "main side\n");
    fixture.setup_git(&["commit", "-q", "-am", "main side"]);

    // The merge itself is *expected* to conflict and exit non-zero, so
    // this bypasses `setup_git`'s own success assertion -- a failing
    // merge is this test's fixture, not a setup bug.
    let _ = Command::new("git")
        .args(["merge", "-q", "other"])
        .current_dir(&fixture.repo)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .envs(fixture.forwarded_env.iter().cloned())
        .env("GIT_AUTHOR_NAME", "fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .status();

    let summary = fixture.compute_summary();
    assert_eq!(
        summary.file_status(Path::new("contested.txt")),
        Some(FileGitStatus::Unmerged)
    );
}

/// `AcceptedBranchOnly` never reaches `read_status_summary` at all
/// (`compute_summary_with_environment`'s own match falls straight to
/// `branch_only_summary`) -- `file_statuses` must be `None`, not an empty
/// map, the same "not computed" distinction `changed_file_count` already
/// draws for this outcome.
#[test]
fn a_branch_only_repository_offers_no_file_statuses() {
    let fixture = Fixture::new("per-file-branch-only");
    let script = fixture.marker_script("marker-fsmonitor-per-file", "exit 0");
    fixture.set_config("core.fsmonitor", script.to_str().unwrap());
    fixture.setup_git(&["checkout", "-q", "-b", "main"]);
    fixture.commit_then_modify_same_length();
    fixture.clear_markers();

    let summary = fixture.compute_summary();
    assert_eq!(summary.file_statuses, None);
    assert!(!fixture.marker_exists("marker-fsmonitor-per-file"));
}

/// Review 413, R-1: locks in the `-z` decision with the actual evidence
/// the reviewer measured -- without `-z`, a non-ASCII filename comes back
/// octal-escaped (`"h\303\251llo..."`), which never matches an explorer
/// node's real `relative_path`, silently dropping the badge. A literal
/// space and a literal newline byte (both legal in a Linux filename) are
/// exercised the same way, so this is one fixture proving `-z` handles
/// every character class `core.quotePath` would otherwise escape, not
/// only the one the reviewer happened to name first.
#[test]
fn non_ascii_space_bearing_and_newline_bearing_filenames_map_to_their_real_on_disk_path() {
    let fixture = Fixture::new("per-file-unusual-names");
    fixture.setup_git(&["checkout", "-q", "-b", "main"]);
    fixture.setup_git(&["commit", "-q", "-m", "initial", "--allow-empty"]);

    fixture.write("héllo wörld.txt", "non-ascii\n");
    fixture.write("plain space.txt", "space\n");
    fixture.write("new\nline.txt", "newline\n");

    let summary = fixture.compute_summary();
    assert_eq!(
        summary.file_status(Path::new("héllo wörld.txt")),
        Some(FileGitStatus::Untracked)
    );
    assert_eq!(
        summary.file_status(Path::new("plain space.txt")),
        Some(FileGitStatus::Untracked)
    );
    assert_eq!(
        summary.file_status(Path::new("new\nline.txt")),
        Some(FileGitStatus::Untracked),
        "a literal newline byte is legal in a Linux filename and must not split the record"
    );
    assert_eq!(summary.changed_file_count, Some(3));
}

/// Review 413, R-2: a path that is not valid UTF-8 (legal on Linux; only
/// `/` and NUL are forbidden in a filename) is skipped from the map --
/// no badge for that one file -- but still counted, so
/// `changed_file_count` stays the true number of real changes rather than
/// silently dropping by one. Before this fix, decoding the *whole*
/// `git status` output in one call meant one odd filename turned the
/// entire read into `None`, dropping the whole repository to
/// branch-only -- the blast radius R-2 named.
#[test]
fn a_non_utf8_path_is_skipped_from_the_map_but_still_counted() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let fixture = Fixture::new("per-file-non-utf8");
    fixture.setup_git(&["checkout", "-q", "-b", "main"]);
    fixture.write("valid.txt", "one\n");
    fixture.setup_git(&["add", "-A"]);
    fixture.setup_git(&["commit", "-q", "-m", "initial"]);
    fixture.write("valid.txt", "two\n");

    // A lone invalid UTF-8 byte in an otherwise-ordinary filename --
    // legal as a path component on Linux, invalid as text.
    let invalid_name = OsStr::from_bytes(b"bad-\xffname.txt");
    fs::write(fixture.repo.join(invalid_name), b"new\n").unwrap();

    let summary = fixture.compute_summary();
    assert_eq!(
        summary.file_status(Path::new("valid.txt")),
        Some(FileGitStatus::Modified)
    );
    assert_eq!(
        summary.changed_file_count,
        Some(2),
        "the undecodable path is still counted even though it carries no map entry"
    );
    assert_eq!(
        summary.file_statuses.as_ref().unwrap().len(),
        1,
        "only the decodable file gets a map entry"
    );
}

// ---------------------------------------------------------------------
// PR-030-D (review 414): locating `git` beyond a fixed `/usr/bin:/bin`,
// and caching the `--version` check.
// ---------------------------------------------------------------------

/// Review 414's required test, verbatim: an inherited `PATH` carrying a
/// relative entry and an entry under the project root must have both
/// dropped -- checked against [`resolve_git_from_inherited_path`]
/// directly, not the full [`resolve_git_executable`] composition, which
/// would always resolve through `REVIEWED_GIT_DIRECTORIES` first on
/// every machine this test suite actually runs on and never reach this
/// fallback at all. Review 415's required extension: a `PATH` entry that
/// reaches into the project root through a symlink -- lexically outside
/// it, filesystem-identical to being inside it -- must be dropped too,
/// the exact bypass the reviewer measured directly.
#[test]
fn resolving_git_from_the_inherited_path_drops_relative_and_project_local_entries() {
    let fixture = Fixture::new("git-executable-resolution");

    let project_local_bin = fixture.repo.join("bin");
    fs::create_dir_all(&project_local_bin).unwrap();
    fs::write(
        project_local_bin.join("git"),
        "must never be resolved -- project-local",
    )
    .unwrap();

    // A `PATH` entry that lexically names a directory *outside* the
    // project root, but resolves through a symlink to one *inside* it --
    // `link` symlinks to the project root itself, so `link/symlinked-bin`
    // is filesystem-identical to `fixture.repo/symlinked-bin`.
    let symlinked_project_bin = fixture.repo.join("symlinked-bin");
    fs::create_dir_all(&symlinked_project_bin).unwrap();
    fs::write(
        symlinked_project_bin.join("git"),
        "must never be resolved -- project-local via a symlink",
    )
    .unwrap();
    let link = fixture.root.join("link");
    std::os::unix::fs::symlink(&fixture.repo, &link).unwrap();
    let symlinked_entry = link.join("symlinked-bin");
    assert!(
        !symlinked_entry.starts_with(&fixture.repo),
        "the entry must be lexically outside the root -- otherwise the symlink adds nothing to this test"
    );

    let legitimate_dir = fixture.root.join("legitimate-bin");
    fs::create_dir_all(&legitimate_dir).unwrap();
    fs::write(
        legitimate_dir.join("git"),
        "a legitimate, non-project-local git",
    )
    .unwrap();

    let inherited_path = format!(
        "relative-entry:{}:{}:{}",
        project_local_bin.display(),
        symlinked_entry.display(),
        legitimate_dir.display()
    );

    let resolved = resolve_git_from_inherited_path(&fixture.repo, &inherited_path);

    assert_eq!(resolved, Some(legitimate_dir.join("git")));
}

/// A `PATH` entry that is a directory but has no `git` inside it at all
/// is skipped in favour of a later entry that does -- the filter must
/// not stop at the first *survives-the-filter* candidate, only the
/// first *actually-has-git* one.
#[test]
fn resolving_git_from_the_inherited_path_skips_a_directory_with_no_git_in_it() {
    let fixture = Fixture::new("git-executable-resolution-empty-dir");

    let empty_dir = fixture.root.join("empty-bin");
    fs::create_dir_all(&empty_dir).unwrap();

    let real_dir = fixture.root.join("real-bin");
    fs::create_dir_all(&real_dir).unwrap();
    fs::write(real_dir.join("git"), "present").unwrap();

    let inherited_path = format!("{}:{}", empty_dir.display(), real_dir.display());

    let resolved = resolve_git_from_inherited_path(&fixture.repo, &inherited_path);

    assert_eq!(resolved, Some(real_dir.join("git")));
}

/// [`directories_are_the_same_or_nested`] directly: the same directory,
/// a nested one, a genuinely unrelated one, and -- the fail-closed case
/// review 415's fix depends on -- a directory that does not exist at
/// all, which must read as "not the same, not nested" (`false`) rather
/// than trusted by default. `resolve_git_from_inherited_path`'s own
/// filesystem check (`candidate.is_file()`) is what actually keeps a
/// nonexistent directory from ever being resolved; this test only
/// proves the helper itself never claims a match it cannot back up.
#[test]
fn directories_are_the_same_or_nested_is_filesystem_aware_and_fails_closed() {
    let fixture = Fixture::new("directories-same-or-nested");
    let root = fixture.repo.canonicalize().unwrap();

    assert!(directories_are_the_same_or_nested(&root, &root));

    let nested = root.join("nested");
    fs::create_dir_all(&nested).unwrap();
    assert!(directories_are_the_same_or_nested(&nested, &root));

    let unrelated = fixture.root.join("unrelated");
    fs::create_dir_all(&unrelated).unwrap();
    assert!(!directories_are_the_same_or_nested(&unrelated, &root));

    let does_not_exist = fixture.root.join("does-not-exist-at-all");
    assert!(!directories_are_the_same_or_nested(&does_not_exist, &root));
}

/// `pub fn compute_summary` is the one production entry point --
/// `resolve_git_executable`'s two-stage resolution runs for real here,
/// against this machine's real environment, not only a fixture-injected
/// override. A clean, real repository is enough to prove the resolved
/// executable is genuinely invocable end to end, and that `R6` (the
/// developer's own global/system config forwarding nothing) still holds
/// even though this call, unlike every other test in this file, reads
/// the real process environment (`forwarded_environment()`) rather than
/// a `Fixture`'s isolated one -- safe precisely because
/// `spawn_git_command` hardcodes `GIT_CONFIG_GLOBAL`/`GIT_CONFIG_SYSTEM`
/// to `/dev/null` unconditionally, regardless of what `HOME` says.
#[test]
fn compute_summary_resolves_a_real_git_executable_on_this_machine() {
    let fixture = Fixture::new("compute-summary-real-resolution");
    fixture.setup_git(&["checkout", "-q", "-b", "main"]);
    fixture.write("tracked.txt", "hello\n");
    fixture.setup_git(&["add", "-A"]);
    fixture.setup_git(&["commit", "-q", "-m", "initial"]);

    let summary = compute_summary(&fixture.repo);

    assert_eq!(summary.provider_state, ProjectProviderState::Complete);
    assert_eq!(summary.branch_name, Some("main".to_string()));
}

/// Proves the cache actually skips a second real spawn -- counting real
/// invocations of a fake `git` rather than only asserting the returned
/// `Result` twice, which would pass whether or not anything was actually
/// cached.
#[test]
fn a_second_call_with_the_same_verified_executable_does_not_respawn_version() {
    let fixture = Fixture::new("version-check-cache");
    let counter = fixture.root.join("version-invocations");
    let script = fixture.marker_script(
        "version-cache-git",
        &format!(
            "echo x >> '{}'\necho 'git version 2.99.0'",
            counter.display()
        ),
    );

    assert!(check_git_available(script.to_str().unwrap(), &fixture.forwarded_env).is_ok());
    assert!(check_git_available(script.to_str().unwrap(), &fixture.forwarded_env).is_ok());

    let invocations = fs::read_to_string(&counter).unwrap_or_default();
    assert_eq!(
        invocations.lines().count(),
        1,
        "the second call must be served from the cache, not a second real spawn"
    );
}

// --- RFC-055 PR-055-A: which entries of a directory does git say are ignored ---

use std::ffi::OsString;

fn names(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

fn set(list: &[&str]) -> std::collections::BTreeSet<OsString> {
    list.iter().map(OsString::from).collect()
}

/// **`ETXTBSY` (`Text file busy`) on a script written a moment ago.** Tests in this
/// binary run in parallel and each forks children; a child forked by *another*
/// thread while this one still has the script open for writing inherits that
/// descriptor until it execs, and executing the script in that window fails with
/// `ETXTBSY`. The stand-in `git` would then be reported as "not found" and a
/// refusal test would see `GateRefused` where it asserts `QueryFailed` -- one
/// failure in a workspace run at load ~9, not reproduced (40 runs at load 33). So
/// a stand-in is not handed out until it has been executed once successfully.
fn wait_until_executable(path: &Path) {
    for _ in 0..200 {
        match Command::new(path).arg("--version").output() {
            Err(error) if error.raw_os_error() == Some(26) => {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            _ => return,
        }
    }
}

impl Fixture {
    /// Asks the query as production does, against this fixture's own `git`
    /// and environment.
    fn ask(&self, directory: &Path, entries: &[OsString]) -> IgnoreAnswer {
        ignored_entries_in_environment(directory, entries, GIT_EXECUTABLE, &self.forwarded_env)
            .answer
    }

    /// A stand-in `git` that satisfies the gate (a version, an empty
    /// configuration) and answers `check-ignore` with `body`. Every
    /// subcommand it is run with leaves a marker, so a test can assert that
    /// **nothing was run** when nothing should have been.
    fn fake_git(&self, name: &str, check_ignore_body: &str) -> String {
        self.fake_git_with_config(name, "exit 0", check_ignore_body)
    }

    /// [`Fixture::fake_git`] whose `config --list --null` runs `config_body`.
    fn fake_git_with_config(
        &self,
        name: &str,
        config_body: &str,
        check_ignore_body: &str,
    ) -> String {
        let path = self.root.join(format!("{name}.sh"));
        let markers = self.markers.display();
        let script = format!(
            "#!/bin/sh\ntouch '{markers}/{name}-invoked'\ncase \"$1\" in\n  --version) echo 'git version 2.43.0';;\n  config) {config_body};;\n  check-ignore) touch '{markers}/{name}-check-ignore'; cat > /dev/null; {check_ignore_body};;\n  *) exit 2;;\nesac\n"
        );
        fs::write(&path, script).unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
        wait_until_executable(&path);
        path.display().to_string()
    }

    fn ask_with(&self, git: &str, directory: &Path, entries: &[OsString]) -> IgnoreAnswer {
        ignored_entries_in_environment(directory, entries, git, &self.forwarded_env).answer
    }
}

/// **D3, and why it is structural.** A file named `:(glob)evil.log` beside
/// ordinary files: **every sibling still gets an answer.**
///
/// Delete the `./` prefix in [`IgnoreQueryInput::new`] and this test fails: git
/// exits **128** with `fatal: :(glob)evil.log: pathspec magic not supported by
/// this command: 'glob'` and answers *nothing* for anyone
/// (`a_raw_glob_named_path_aborts_the_whole_batch` shows that abort with a raw
/// query, so this test's pass is not a fixture that never was hostile).
#[test]
fn a_file_named_like_pathspec_magic_does_not_silence_its_siblings() {
    let fixture = Fixture::new("ignore-glob-name");
    fixture.write(".gitignore", "*.log\n");
    fixture.write(":(glob)evil.log", "x");
    fixture.write("a.log", "x");
    fixture.write("b.txt", "x");
    fixture.write("c.txt", "x");

    let answer = fixture.ask(
        &fixture.repo,
        &names(&[":(glob)evil.log", "a.log", "b.txt", "c.txt"]),
    );
    assert_eq!(
        answer,
        IgnoreAnswer::Ignored(set(&[":(glob)evil.log", "a.log"])),
        "the hostile name must be answered, and so must every sibling"
    );
}

/// The ablation the checklist wants before trusting the test above: a **raw**
/// query -- the path handed to git as-is -- really does abort the batch.
#[test]
fn a_raw_glob_named_path_aborts_the_whole_batch() {
    let fixture = Fixture::new("ignore-glob-raw");
    fixture.write(".gitignore", "*.log\n");
    fixture.write(":(glob)evil.log", "x");
    fixture.write("a.log", "x");

    let mut child = Command::new("git")
        .args(["check-ignore", "-z", "--stdin"])
        .current_dir(&fixture.repo)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .envs(fixture.forwarded_env.iter().cloned())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b":(glob)evil.log\0a.log\0")
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(128));
    assert!(output.stdout.is_empty(), "a sibling was answered after all");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("pathspec magic not supported"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// D4: exit `1` is an answer. A directory with nothing ignored -- and a name
/// that does not exist, which also exits `1` -- is [`IgnoreAnswer::NoneIgnored`],
/// which is a different value from [`IgnoreAnswer::Unknown`] in every case
/// below.
#[test]
fn nothing_ignored_is_an_answer_and_not_an_unknown() {
    let fixture = Fixture::new("ignore-none");
    fixture.write(".gitignore", "*.log\n");
    fixture.write("a.txt", "x");

    assert_eq!(
        fixture.ask(&fixture.repo, &names(&["a.txt", "does-not-exist"])),
        IgnoreAnswer::NoneIgnored
    );
    assert_ne!(
        IgnoreAnswer::NoneIgnored,
        IgnoreAnswer::Unknown(IgnoreUnknown::QueryFailed)
    );
    // Nothing asked, nothing ignored: an answer that needs no process.
    assert_eq!(fixture.ask(&fixture.repo, &[]), IgnoreAnswer::NoneIgnored);
}

/// D4: a failure is unknown -- never `NoneIgnored` (fail open) and never a
/// set (fail closed). Every way a `git` can fail to answer, and every way a
/// reply can be wrong, lands on `Unknown`.
#[test]
fn every_failure_to_answer_is_unknown_and_never_none_ignored() {
    let fixture = Fixture::new("ignore-forced-failure");
    fixture.write("a.log", "x");
    let entries = names(&["a.log", "b.txt"]);
    let unknown = IgnoreAnswer::Unknown(IgnoreUnknown::QueryFailed);

    let cases = [
        ("exit-128", "echo 'fatal: boom' >&2; exit 128"),
        ("exit-2", "exit 2"),
        // Exit 1 must come with an empty reply.
        ("exit-1-with-a-reply", "printf './a.log\\0'; exit 1"),
        // Exit 0 means something was ignored: an empty reply contradicts it.
        ("exit-0-empty", "exit 0"),
        // A record git never terminated.
        ("unterminated", "printf './a.log'; exit 0"),
        // A path nobody asked about.
        ("unasked", "printf './other.log\\0'; exit 0"),
        // A path without the prefix this module put on it.
        ("unprefixed", "printf 'a.log\\0'; exit 0"),
        ("killed", "kill -9 $$"),
    ];
    for (name, body) in cases {
        let git = fixture.fake_git(name, body);
        assert_eq!(
            fixture.ask_with(&git, &fixture.repo, &entries),
            unknown,
            "{name}"
        );
        assert!(
            fixture.marker_exists(&format!("{name}-check-ignore")),
            "{name}: the fake must actually have been asked"
        );
    }
    // ...and the well-behaved one, through the same fake, is an answer.
    let git = fixture.fake_git("well-behaved", "printf './a.log\\0'; exit 0");
    assert_eq!(
        fixture.ask_with(&git, &fixture.repo, &entries),
        IgnoreAnswer::Ignored(set(&["a.log"]))
    );
}

/// D5's "do not pass `--no-index`": a **tracked** file that matches a pattern
/// is tracked, and the query says it is not ignored -- while an untracked file
/// matching the same pattern is.
#[test]
fn a_tracked_file_matching_a_pattern_is_not_ignored() {
    let fixture = Fixture::new("ignore-tracked");
    fixture.write(".gitignore", "*.log\n");
    fixture.write("tracked.log", "x");
    fixture.write("untracked.log", "x");
    fixture.setup_git(&["add", "-f", ".gitignore", "tracked.log"]);
    fixture.setup_git(&["commit", "-q", "-m", "initial"]);

    assert_eq!(
        fixture.ask(&fixture.repo, &names(&["tracked.log", "untracked.log"])),
        IgnoreAnswer::Ignored(set(&["untracked.log"]))
    );
}

/// The measured requirement: a project root **two levels inside** its
/// repository still gets the repository-root `.gitignore`, and a directory
/// pattern (`build/`) applies to a directory asked about by name.
#[test]
fn a_project_root_inside_its_repository_gets_the_repository_root_rules() {
    let fixture = Fixture::new("ignore-nested-root");
    fixture.write(".gitignore", "*.log\nbuild/\n");
    let deep = fixture.repo.join("sub").join("deep");
    fs::create_dir_all(deep.join("build")).unwrap();
    fs::write(deep.join("a.log"), "x").unwrap();
    fs::write(deep.join("b.txt"), "x").unwrap();

    assert_eq!(
        enclosing_repository_root(&fs::canonicalize(&deep).unwrap()),
        Some(fs::canonicalize(&fixture.repo).unwrap()),
        "the repository is found above the directory"
    );
    assert_eq!(
        fixture.ask(&deep, &names(&["a.log", "b.txt", "build"])),
        IgnoreAnswer::Ignored(set(&["a.log", "build"]))
    );
}

/// R6: `-z` is bytes. A filename that is not UTF-8 comes back exactly, and
/// nothing decodes lossily on the way to the comparison.
#[test]
fn a_non_utf8_filename_survives_the_round_trip() {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    let fixture = Fixture::new("ignore-non-utf8");
    fixture.write(".gitignore", "*.log\n");
    let odd = OsString::from_vec(b"caf\xe9.log".to_vec());
    fs::write(fixture.repo.join(&odd), "x").unwrap();
    fixture.write("plain.txt", "x");

    let answer = fixture.ask(&fixture.repo, &[odd.clone(), OsString::from("plain.txt")]);
    match answer {
        IgnoreAnswer::Ignored(found) => {
            assert_eq!(found.len(), 1);
            assert_eq!(found.iter().next().unwrap().as_bytes(), b"caf\xe9.log");
        }
        other => panic!("{other:?}"),
    }
}

/// Not a repository: unknown, with **no subprocess** -- a fake `git` that would
/// leave a marker for any invocation sees none.
#[test]
fn not_a_repository_is_unknown_without_running_anything() {
    let fixture = Fixture::new("ignore-not-a-repository");
    let plain = fixture.root.join("plain");
    fs::create_dir_all(&plain).unwrap();
    fs::write(plain.join("a.log"), "x").unwrap();
    let git = fixture.fake_git("not-a-repo", "printf './a.log\\0'; exit 0");

    assert_eq!(
        fixture.ask_with(&git, &plain, &names(&["a.log"])),
        IgnoreAnswer::Unknown(IgnoreUnknown::NotARepository)
    );
    assert!(!fixture.marker_exists("not-a-repo-invoked"));
}

/// The gate refusing, and `git` being unavailable: unknown, and the program a
/// repository names is never run. First the hostile control -- an unprotected
/// `git check-ignore` **does** run `core.fsmonitor` (measured; it reads the
/// index), which is why the gate is required and not merely careful.
#[test]
fn a_repository_the_gate_does_not_accept_is_never_asked_anything() {
    let fixture = Fixture::new("ignore-gate-refused");
    fixture.write(".gitignore", "*.log\n");
    fixture.write("a.log", "x");
    let script = fixture.marker_script("marker-fsmonitor", "exit 0");
    fixture.set_config("core.fsmonitor", script.to_str().unwrap());
    fixture.clear_markers();

    // The control: no gate, and the repository's program runs.
    let unprotected = Command::new("git")
        .args(["check-ignore", "./a.log"])
        .current_dir(&fixture.repo)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .envs(fixture.forwarded_env.iter().cloned())
        .output()
        .unwrap();
    assert!(unprotected.status.success());
    assert!(
        fixture.marker_exists("marker-fsmonitor"),
        "an unprotected `git check-ignore` must run a repository-named fsmonitor, or the gate is \
         not what this query needs"
    );
    fixture.clear_markers();

    assert_eq!(
        fixture.ask(&fixture.repo, &names(&["a.log"])),
        IgnoreAnswer::Unknown(IgnoreUnknown::GateRefused)
    );
    assert!(
        !fixture.marker_exists("marker-fsmonitor"),
        "the query ran a program the repository named"
    );

    // Git not found: the same answer, and no panic.
    let clean = Fixture::new("ignore-no-git");
    clean.write("a.log", "x");
    assert_eq!(
        ignored_entries_in_environment(
            &clean.repo,
            &names(&["a.log"]),
            "tekstide-test-no-such-git",
            &clean.forwarded_env
        )
        .answer,
        IgnoreAnswer::Unknown(IgnoreUnknown::GateRefused)
    );
}

/// **D9: the gate is not cached.** The same repository is accepted, then a
/// program is added to its configuration, then it is refused -- the second
/// call re-reads the configuration rather than remembering the first answer.
/// Ablated by caching the gate's verdict per repository root.
#[test]
fn the_gate_is_asked_again_on_every_call() {
    let fixture = Fixture::new("ignore-gate-not-cached");
    fixture.write(".gitignore", "*.log\n");
    fixture.write("a.log", "x");
    assert_eq!(
        fixture.ask(&fixture.repo, &names(&["a.log"])),
        IgnoreAnswer::Ignored(set(&["a.log"]))
    );

    let script = fixture.marker_script("marker-fsmonitor", "exit 0");
    fixture.set_config("core.fsmonitor", script.to_str().unwrap());
    fixture.clear_markers();
    assert_eq!(
        fixture.ask(&fixture.repo, &names(&["a.log"])),
        IgnoreAnswer::Unknown(IgnoreUnknown::GateRefused)
    );
    assert!(!fixture.marker_exists("marker-fsmonitor"));
}

/// What the query does **not** need the gate's other half for, pinned so a new
/// `git` cannot change it silently: a poisoned submodule's own clean filter
/// (R1, review 406 -- which `git status` in the parent *does* run) is not run by
/// `check-ignore`, and the parent's own configuration is allowlist-clean, so
/// the query is answered. If a future `git` starts running it, this fails.
#[test]
fn a_poisoned_submodule_is_not_run_by_the_query() {
    let fixture = Fixture::new("ignore-poisoned-submodule");
    let marker = fixture.add_poisoned_submodule();
    fixture.write(".gitignore", "*.log\n");
    fixture.write("a.log", "x");
    fixture.clear_markers();

    assert_eq!(
        fixture.ask(&fixture.repo, &names(&["a.log", "sub"])),
        IgnoreAnswer::Ignored(set(&["a.log"]))
    );
    assert!(
        !fixture.marker_exists(marker),
        "`git check-ignore` ran a submodule's own clean filter"
    );
}

/// A repository rooted at the user's home directory -- a dotfiles repository
/// whose `.gitignore` says `*` -- would answer "ignored" for every entry of every
/// project beneath it. It is declined, without a subprocess; the same repository
/// with `HOME` somewhere else answers normally.
#[test]
fn a_repository_at_or_above_home_is_not_asked() {
    let fixture = Fixture::new("ignore-home-repo");
    fixture.write(".gitignore", "*\n");
    let project = fixture.repo.join("projects").join("mine");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("main.rs"), "x").unwrap();
    let git = fixture.fake_git("home-repo", "printf './main.rs\\0'; exit 0");

    let mut at_home = fixture.forwarded_env.clone();
    at_home.retain(|(var, _)| var != "HOME");
    at_home.push(("HOME".to_owned(), fixture.repo.display().to_string()));
    assert_eq!(
        ignored_entries_in_environment(&project, &names(&["main.rs"]), &git, &at_home).answer,
        IgnoreAnswer::Unknown(IgnoreUnknown::RepositoryDeclined)
    );
    // Home *inside* the repository counts too: the repository is above it.
    let mut inside = fixture.forwarded_env.clone();
    inside.retain(|(var, _)| var != "HOME");
    inside.push(("HOME".to_owned(), project.display().to_string()));
    assert_eq!(
        ignored_entries_in_environment(&project, &names(&["main.rs"]), &git, &inside).answer,
        IgnoreAnswer::Unknown(IgnoreUnknown::RepositoryDeclined)
    );
    assert!(!fixture.marker_exists("home-repo-invoked"));

    // Home elsewhere (the fixture's own): the same directory is answered.
    assert_eq!(
        fixture.ask(&project, &names(&["main.rs"])),
        IgnoreAnswer::Ignored(set(&["main.rs"]))
    );
}

/// A name that is not one directory entry, and more names than a query may
/// carry, are refused whole and run nothing.
#[test]
fn a_name_that_is_not_one_entry_is_refused_before_anything_runs() {
    let fixture = Fixture::new("ignore-unusable-names");
    fixture.write("a.log", "x");
    let git = fixture.fake_git("unusable", "printf './a.log\\0'; exit 0");
    let unusable = IgnoreAnswer::Unknown(IgnoreUnknown::UnusableName);

    for bad in [
        "",
        ".",
        "..",
        "a/b",
        "/etc/passwd",
        "../escape",
        "./a.log",
        &"x".repeat(256),
        "nul\0byte",
    ] {
        assert_eq!(
            fixture.ask_with(&git, &fixture.repo, &names(&["a.log", bad])),
            unusable,
            "{bad:?}"
        );
    }
    let too_many: Vec<OsString> = (0..=MAX_IGNORE_QUERY_ENTRIES)
        .map(|i| OsString::from(format!("f{i}")))
        .collect();
    assert_eq!(fixture.ask_with(&git, &fixture.repo, &too_many), unusable);
    assert!(!fixture.marker_exists("unusable-invoked"));

    // The bound itself is allowed, and a name of the longest legal length.
    let exactly: Vec<OsString> = (0..MAX_IGNORE_QUERY_ENTRIES)
        .map(|i| OsString::from(format!("f{i}")))
        .collect();
    assert_eq!(
        fixture.ask(&fixture.repo, &exactly),
        IgnoreAnswer::NoneIgnored
    );
    assert_eq!(
        fixture.ask(&fixture.repo, &names(&[&"x".repeat(255)])),
        IgnoreAnswer::NoneIgnored
    );
}

/// D3's "no caller can pass a raw path" and D5's "no `--no-index`", held at the
/// source: the only place `check-ignore` is named is the one function, which
/// takes an [`IgnoreQueryInput`]; the stdin-writing runner has one caller; and
/// `--no-index` appears nowhere in production code.
#[test]
fn check_ignore_has_one_call_site_and_never_uses_no_index() {
    let source =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/runtime/git.rs"))
            .unwrap();
    let shipped = source.split("#[cfg(test)]\nmod tests;").next().unwrap();
    let code: String = shipped
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(code.matches("\"check-ignore\"").count(), 1);
    assert_eq!(
        code.matches("run_bounded_git_with_input(").count(),
        2,
        "one definition, one call"
    );
    assert!(!code.contains("no-index"));
    assert!(!code.contains("GIT_LITERAL_PATHSPECS"));
    assert_eq!(
        code.matches("b\"./\"").count(),
        1,
        "the prefix is applied in exactly one place"
    );
}

/// A gate refusal reaches unknown **without the query ever being run**: the
/// configuration was read (that is the gate), a key outside the allowlist was
/// found, and `check-ignore` -- the call that would run a repository's program --
/// was never made.
#[test]
fn a_gate_refusal_never_reaches_check_ignore() {
    let fixture = Fixture::new("ignore-gate-refusal-no-query");
    fixture.write("a.log", "x");
    let git = fixture.fake_git_with_config(
        "refusal",
        "printf 'core.fsmonitor\\nsomething\\0'; exit 0",
        "printf './a.log\\0'; exit 0",
    );
    assert_eq!(
        fixture.ask_with(&git, &fixture.repo, &names(&["a.log"])),
        IgnoreAnswer::Unknown(IgnoreUnknown::GateRefused)
    );
    assert!(
        fixture.marker_exists("refusal-invoked"),
        "the gate did read the configuration"
    );
    assert!(
        !fixture.marker_exists("refusal-check-ignore"),
        "and the query was never made"
    );
}

/// **Review 431 R1.** The case that matters for dropping the gitlink check: the
/// explorer expands a submodule and asks about **the submodule's own entries**.
/// `enclosing_repository_root` resolves the submodule's `.git` *pointer file*, so
/// the repository is the submodule and its own configuration is what the gate
/// vets -- where the review-406 fixture's `filter.evil.clean` is not on the
/// allowlist. Refused, and the filter never runs.
#[test]
fn a_scan_inside_a_poisoned_submodule_is_refused_by_the_gate() {
    let fixture = Fixture::new("ignore-inside-submodule");
    let marker = fixture.add_poisoned_submodule();
    let sub = fixture.repo.join("sub");
    fixture.clear_markers();

    assert_eq!(
        enclosing_repository_root(&fs::canonicalize(&sub).unwrap()),
        Some(fs::canonicalize(&sub).unwrap()),
        "the submodule, not the superproject, is the repository for its own entries"
    );
    assert_eq!(
        fixture.ask(&sub, &names(&["tracked.txt"])),
        IgnoreAnswer::Unknown(IgnoreUnknown::GateRefused)
    );
    assert!(
        !fixture.marker_exists(marker),
        "the submodule's own clean filter ran"
    );
}

/// **Review 431 R5.** The query's bound is the explorer's, and a test says so:
/// if the scan policy's cap ever moves, this fails and the two are chosen again
/// together instead of drifting apart under a comment that says they match.
#[test]
fn the_query_bound_is_the_explorers_per_directory_cap() {
    assert_eq!(
        crate::project::root::FileExplorerScanPolicy::linux_mvp().max_children_per_directory,
        MAX_IGNORE_QUERY_ENTRIES
    );
}

/// B needs to say whether the ignore rule came from the project's own repository
/// or one above it (review 431, ruling 4), so the report carries where the
/// repository is -- also when it was declined or refused, and never when there
/// is none.
#[test]
fn the_report_says_which_repository_answered_or_declined() {
    let fixture = Fixture::new("ignore-report-root");
    fixture.write(".gitignore", "*.log\n");
    let deep = fixture.repo.join("sub").join("deep");
    fs::create_dir_all(&deep).unwrap();
    fs::write(deep.join("a.log"), "x").unwrap();
    let canonical_repo = fs::canonicalize(&fixture.repo).unwrap();

    let report = ignored_entries_in_environment(
        &deep,
        &names(&["a.log"]),
        GIT_EXECUTABLE,
        &fixture.forwarded_env,
    );
    assert_eq!(report.repository_root, Some(canonical_repo.clone()));

    // Declined: the repository is still named.
    let mut at_home = fixture.forwarded_env.clone();
    at_home.retain(|(var, _)| var != "HOME");
    at_home.push(("HOME".to_owned(), fixture.repo.display().to_string()));
    let report =
        ignored_entries_in_environment(&deep, &names(&["a.log"]), GIT_EXECUTABLE, &at_home);
    assert_eq!(
        report.answer,
        IgnoreAnswer::Unknown(IgnoreUnknown::RepositoryDeclined)
    );
    assert_eq!(report.repository_root, Some(canonical_repo));

    // No repository: none named.
    let plain = fixture.root.join("plain");
    fs::create_dir_all(&plain).unwrap();
    let report = ignored_entries_in_environment(
        &plain,
        &names(&["a.log"]),
        GIT_EXECUTABLE,
        &fixture.forwarded_env,
    );
    assert_eq!(
        report.answer,
        IgnoreAnswer::Unknown(IgnoreUnknown::NotARepository)
    );
    assert_eq!(report.repository_root, None);
}

/// An empty batch is answered *after* the gate: an empty directory in a
/// repository the gate refuses is unknown, not "git answered".
#[test]
fn an_empty_batch_in_a_refused_repository_is_not_an_answer() {
    let fixture = Fixture::new("ignore-empty-refused");
    let script = fixture.marker_script("marker-fsmonitor", "exit 0");
    fixture.set_config("core.fsmonitor", script.to_str().unwrap());
    fixture.clear_markers();
    assert_eq!(
        fixture.ask(&fixture.repo, &[]),
        IgnoreAnswer::Unknown(IgnoreUnknown::GateRefused)
    );
    assert!(!fixture.marker_exists("marker-fsmonitor"));
}
