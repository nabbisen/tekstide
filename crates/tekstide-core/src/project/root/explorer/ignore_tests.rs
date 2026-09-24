//! RFC-055 PR-055-B: the scan carries git's ignore answer, and the fixed list
//! keeps its job as the floor.
//!
//! Every repository here is built under a fresh temporary directory with its own
//! `HOME`, so a developer's global ignore file is never consulted, and the
//! oracle handed to [`ExplorerDirectoryScan::ask_git`] is the production
//! `ignored_entries_in_environment` bound to that environment -- the real
//! `git`, not a stand-in, except where a test needs a `git` that fails.

use std::cell::RefCell;
use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::{
    ExplorerDirectoryScan, ExplorerFloorReason, ExplorerIgnoreRule, ExplorerIgnoreState,
    ExplorerNode, ExplorerNodeKind, ExplorerNodeState, ExplorerRepositoryPlacement,
    FileExplorerScanPolicy, FileExplorerScanner,
};
use crate::project::root::{ProjectRootHandle, ProjectRootValidator, SymlinkPolicy};
use crate::project::{ProjectId, ProjectSession};
use crate::runtime::git::{
    IgnoreAnswer, IgnoreReport, IgnoreUnknown, ignored_entries_in_environment,
};

struct Repo {
    base: PathBuf,
    repo: PathBuf,
    env: Vec<(String, String)>,
}

impl Repo {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "tekstide-ignore-{label}-{}-{nonce}",
            std::process::id()
        ));
        let repo = base.join("repo");
        let home = base.join("home");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&home).unwrap();
        fs::write(home.join(".gitconfig"), "").unwrap();
        let repository = Self {
            base,
            repo,
            env: vec![("HOME".to_owned(), home.display().to_string())],
        };
        repository.git(&repository.repo, &["init", "-q"]);
        repository
    }

    fn git(&self, dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .envs(self.env.iter().cloned())
            .env("GIT_AUTHOR_NAME", "fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .status()
            .expect("git must be installed to run this test suite");
        assert!(status.success(), "fixture setup `git {args:?}` failed");
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.repo.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn mkdir(&self, relative: &str) {
        fs::create_dir_all(self.repo.join(relative)).unwrap();
    }

    /// A project rooted at `relative` inside the repository (`""` = the
    /// repository root itself).
    fn project(&self, relative: &str) -> ProjectRootHandle {
        let path = self.repo.join(relative);
        let root = ProjectRootValidator
            .validate(&path, SymlinkPolicy::FailClosed)
            .expect("the fixture's project root validates");
        let session = ProjectSession::new(
            ProjectId::for_test(1),
            root.display_name,
            root.selected_path,
            root.canonical_path,
        );
        ProjectRootHandle::from_project_session(&session)
    }

    /// The scan of `relative` under `handle`, with git asked through the
    /// isolated oracle; also returns every batch the oracle was handed.
    fn scan(
        &self,
        handle: &ProjectRootHandle,
        relative: &str,
    ) -> (ExplorerDirectoryScan, Vec<Vec<OsString>>) {
        self.scan_with(handle, relative, &self.env)
    }

    fn scan_with(
        &self,
        handle: &ProjectRootHandle,
        relative: &str,
        env: &[(String, String)],
    ) -> (ExplorerDirectoryScan, Vec<Vec<OsString>>) {
        let batches = RefCell::new(Vec::new());
        let oracle = |directory: &Path, names: &[OsString]| {
            batches.borrow_mut().push(names.to_vec());
            ignored_entries_in_environment(directory, names, "git", env)
        };
        let mut scan = FileExplorerScanner
            .scan_directory(handle, relative, &FileExplorerScanPolicy::linux_mvp())
            .expect("scans");
        scan.ask_git(handle, &oracle);
        (scan, batches.into_inner())
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn node<'a>(scan: &'a ExplorerDirectoryScan, name: &str) -> &'a ExplorerNode {
    scan.nodes
        .iter()
        .find(|node| node.name == name)
        .unwrap_or_else(|| panic!("no node named {name}"))
}

/// D2's falsification, and the checklist's most important test. 20,000 `*.log`
/// files under a one-line `.gitignore`: `git status --ignored=matching` would
/// return 20,000 records (RFC-055 measurement 5). The scan is capped at 256
/// rows before git is asked, so **what git is asked about tracks the rows drawn,
/// not the ignored file count** -- and the entries the cap left out were never in
/// the batch, so they are not described as anything.
#[test]
fn what_git_is_asked_about_tracks_the_rows_drawn_not_the_ignored_file_count() {
    let repo = Repo::new("log-flood");
    repo.write(".gitignore", "*.log\n");
    repo.mkdir("logs");
    for index in 0..20_000 {
        fs::File::create(repo.repo.join("logs").join(format!("f{index:05}.log"))).unwrap();
    }
    let handle = repo.project("");
    let (scan, batches) = repo.scan(&handle, "logs");

    assert_eq!(batches.len(), 1, "one query per directory scanned");
    assert_eq!(scan.nodes.len(), 256, "the cap bounds the rows");
    assert_eq!(
        batches[0].len(),
        256,
        "git was asked about the rows drawn, not about 20,000 files"
    );
    assert!(scan.truncated);
    assert_eq!(scan.omitted_entries, 20_000 - 256);
    // Every row that was asked about got git's answer...
    assert!(
        scan.nodes
            .iter()
            .all(|node| node.ignore == ExplorerIgnoreState::Ignored)
    );
    // ...and the omitted tail is not in `nodes` at all, so nothing can render or
    // count it as ignored, or as not ignored.
    assert_eq!(
        scan.nodes.len() + scan.omitted_entries,
        20_000,
        "the omitted count still says how many rows are not shown, and claims nothing about them"
    );
    assert_eq!(
        scan.ignore_rule,
        ExplorerIgnoreRule::Git {
            repository: ExplorerRepositoryPlacement::AtProjectRoot
        }
    );
}

/// D6, both halves, through the real `git`. A repository whose `.gitignore` does
/// **not** name `target/` gets an ordinary, expandable `target/`; one that does
/// gets it collapsed **and** ignored. Same directory names, different answers,
/// because git decided.
#[test]
fn git_decides_whether_target_is_collapsed() {
    let plain = Repo::new("target-not-ignored");
    plain.write(".gitignore", "*.log\n");
    plain.write("target/debug/app", "x");
    plain.write("src/lib.rs", "x");
    let handle = plain.project("");
    let (scan, _) = plain.scan(&handle, "");
    let target = node(&scan, "target");
    assert_eq!(target.state, ExplorerNodeState::Available);
    assert_eq!(target.ignore, ExplorerIgnoreState::NotIgnored);
    assert_eq!(node(&scan, "src").state, ExplorerNodeState::Available);

    let ignoring = Repo::new("target-ignored");
    ignoring.write(".gitignore", "target/\n*.log\n");
    ignoring.write("target/debug/app", "x");
    ignoring.write("src/lib.rs", "x");
    let handle = ignoring.project("");
    let (scan, _) = ignoring.scan(&handle, "");
    let target = node(&scan, "target");
    assert_eq!(target.state, ExplorerNodeState::Collapsed);
    assert_eq!(target.ignore, ExplorerIgnoreState::Ignored);
    assert_eq!(node(&scan, "src").ignore, ExplorerIgnoreState::NotIgnored);
}

/// `.git` is version-control metadata, not something a `.gitignore` names: git
/// does not call it ignored, and the explorer still keeps it collapsed.
#[test]
fn dot_git_stays_collapsed_under_git_s_rule() {
    let repo = Repo::new("dot-git");
    repo.write("a.txt", "x");
    let handle = repo.project("");
    let (scan, _) = repo.scan(&handle, "");
    let dot_git = node(&scan, ".git");
    assert_eq!(dot_git.state, ExplorerNodeState::Collapsed);
    assert_eq!(dot_git.ignore, ExplorerIgnoreState::NotIgnored);
}

/// The floor: outside a repository the fixed list decides, and **the scan
/// carries that it did, and why, as a value.**
#[test]
fn outside_a_repository_the_floor_list_decides_and_the_scan_says_so() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!(
        "tekstide-ignore-floor-{}-{nonce}",
        std::process::id()
    ));
    let project = base.join("project");
    fs::create_dir_all(project.join("target")).unwrap();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("a.log"), "x").unwrap();
    let root = ProjectRootValidator
        .validate(&project, SymlinkPolicy::FailClosed)
        .unwrap();
    let session = ProjectSession::new(
        ProjectId::for_test(1),
        root.display_name,
        root.selected_path,
        root.canonical_path,
    );
    let handle = ProjectRootHandle::from_project_session(&session);

    let mut scan = FileExplorerScanner
        .scan_directory(&handle, "", &FileExplorerScanPolicy::linux_mvp())
        .unwrap();
    let env = vec![("HOME".to_owned(), base.display().to_string())];
    scan.ask_git(&handle, &|directory: &Path, names: &[OsString]| {
        ignored_entries_in_environment(directory, names, "git", &env)
    });

    assert_eq!(
        scan.ignore_rule,
        ExplorerIgnoreRule::Floor(ExplorerFloorReason::NotARepository)
    );
    assert_eq!(node(&scan, "target").state, ExplorerNodeState::Collapsed);
    assert_eq!(node(&scan, "src").state, ExplorerNodeState::Available);
    // Nothing was learned about any entry, and none is drawn as though it were.
    assert!(
        scan.nodes
            .iter()
            .all(|node| node.ignore == ExplorerIgnoreState::Unknown)
    );
    let _ = fs::remove_dir_all(&base);
}

/// The scanner on its own never asks git: `NotAsked`, the floor, and every
/// entry unknown -- so the synchronous path can never block on a subprocess.
#[test]
fn the_scanner_alone_is_the_floor_and_asks_nothing() {
    let repo = Repo::new("scanner-alone");
    repo.write(".gitignore", "target/\n");
    repo.write("target/x", "x");
    let handle = repo.project("");
    let scan = FileExplorerScanner
        .scan_directory(&handle, "", &FileExplorerScanPolicy::linux_mvp())
        .unwrap();
    assert_eq!(
        scan.ignore_rule,
        ExplorerIgnoreRule::Floor(ExplorerFloorReason::NotAsked)
    );
    assert!(
        scan.nodes
            .iter()
            .all(|node| node.ignore == ExplorerIgnoreState::Unknown)
    );
    assert_eq!(node(&scan, "target").state, ExplorerNodeState::Collapsed);
}

/// D4 in the model: a `git` that fails is **unknown** on every node and the
/// floor decides -- and a `git` that answers "none ignored" is `NotIgnored` on
/// every node under git's rule. The two must never look alike.
#[test]
fn unknown_and_not_ignored_are_different_on_the_node_and_in_the_rule() {
    let repo = Repo::new("unknown-vs-none");
    repo.write("a.txt", "x");
    repo.mkdir("target");
    let handle = repo.project("");
    let scan_with_oracle = |report: IgnoreReport| {
        let mut scan = FileExplorerScanner
            .scan_directory(&handle, "", &FileExplorerScanPolicy::linux_mvp())
            .unwrap();
        scan.ask_git(&handle, &move |_: &Path, _: &[OsString]| report.clone());
        scan
    };
    let root = fs::canonicalize(&repo.repo).unwrap();

    let failed = scan_with_oracle(IgnoreReport {
        answer: IgnoreAnswer::Unknown(IgnoreUnknown::QueryFailed),
        repository_root: Some(root.clone()),
    });
    assert_eq!(
        failed.ignore_rule,
        ExplorerIgnoreRule::Floor(ExplorerFloorReason::QueryFailed)
    );
    assert!(
        failed
            .nodes
            .iter()
            .all(|node| node.ignore == ExplorerIgnoreState::Unknown)
    );
    assert_eq!(
        node(&failed, "target").state,
        ExplorerNodeState::Collapsed,
        "the floor decides when git could not"
    );

    let none = scan_with_oracle(IgnoreReport {
        answer: IgnoreAnswer::NoneIgnored,
        repository_root: Some(root),
    });
    assert!(matches!(none.ignore_rule, ExplorerIgnoreRule::Git { .. }));
    assert!(
        none.nodes
            .iter()
            .all(|node| node.ignore == ExplorerIgnoreState::NotIgnored)
    );
    assert_eq!(
        node(&none, "target").state,
        ExplorerNodeState::Available,
        "git said nothing is ignored, so `target` is an ordinary directory"
    );
    assert_ne!(failed.ignore_rule, none.ignore_rule);
}

/// Every reason git's answer is not used is a distinct, stated floor reason.
#[test]
fn each_way_git_cannot_answer_is_its_own_floor_reason() {
    let repo = Repo::new("floor-reasons");
    repo.write("a.txt", "x");
    let handle = repo.project("");
    for (unknown, reason) in [
        (
            IgnoreUnknown::NotARepository,
            ExplorerFloorReason::NotARepository,
        ),
        (
            IgnoreUnknown::RepositoryDeclined,
            ExplorerFloorReason::RepositoryDeclined,
        ),
        (IgnoreUnknown::GateRefused, ExplorerFloorReason::GateRefused),
        (IgnoreUnknown::QueryFailed, ExplorerFloorReason::QueryFailed),
        (
            IgnoreUnknown::UnusableName,
            ExplorerFloorReason::QueryFailed,
        ),
    ] {
        let mut scan = FileExplorerScanner
            .scan_directory(&handle, "", &FileExplorerScanPolicy::linux_mvp())
            .unwrap();
        scan.ask_git(&handle, &move |_: &Path, _: &[OsString]| IgnoreReport {
            answer: IgnoreAnswer::Unknown(unknown),
            repository_root: None,
        });
        assert_eq!(
            scan.ignore_rule,
            ExplorerIgnoreRule::Floor(reason),
            "{unknown:?}"
        );
    }
}

/// A repository the gate refuses, and one declined for being at `$HOME`, reach
/// the floor through the real query -- with the reason the sidebar will state.
#[test]
fn a_refused_and_a_declined_repository_reach_the_floor_with_their_own_reasons() {
    let refused = Repo::new("gate-refused");
    refused.write("a.txt", "x");
    refused.mkdir("target");
    refused.git(&refused.repo, &["config", "core.fsmonitor", "true"]);
    let handle = refused.project("");
    let (scan, _) = refused.scan(&handle, "");
    assert_eq!(
        scan.ignore_rule,
        ExplorerIgnoreRule::Floor(ExplorerFloorReason::GateRefused)
    );
    assert_eq!(node(&scan, "target").state, ExplorerNodeState::Collapsed);

    let declined = Repo::new("declined");
    declined.write("a.txt", "x");
    let handle = declined.project("");
    let at_home = vec![("HOME".to_owned(), declined.repo.display().to_string())];
    let (scan, _) = declined.scan_with(&handle, "", &at_home);
    assert_eq!(
        scan.ignore_rule,
        ExplorerIgnoreRule::Floor(ExplorerFloorReason::RepositoryDeclined)
    );
}

/// Review 431, ruling 4: the scan says where the repository that answered is,
/// so the sidebar can say the status bar's repository is not this one.
#[test]
fn the_scan_says_whether_the_repository_is_the_projects_own_above_it_or_inside_it() {
    let repo = Repo::new("placement");
    repo.write(".gitignore", "*.log\n");
    repo.write("sub/deep/a.log", "x");
    repo.write("sub/deep/b.txt", "x");

    let at_root = repo.project("");
    let (scan, _) = repo.scan(&at_root, "");
    assert_eq!(
        scan.ignore_rule,
        ExplorerIgnoreRule::Git {
            repository: ExplorerRepositoryPlacement::AtProjectRoot
        }
    );

    // A project two levels *inside* the repository: the root `.gitignore`
    // applies, and the scan says the repository is above the project.
    let nested = repo.project("sub/deep");
    let (scan, _) = repo.scan(&nested, "");
    assert_eq!(node(&scan, "a.log").ignore, ExplorerIgnoreState::Ignored);
    assert_eq!(node(&scan, "b.txt").ignore, ExplorerIgnoreState::NotIgnored);
    assert_eq!(
        scan.ignore_rule,
        ExplorerIgnoreRule::Git {
            repository: ExplorerRepositoryPlacement::AboveProjectRoot
        }
    );

    // A repository *inside* the project: a nested checkout answers for its own
    // entries.
    let inner = repo.repo.join("vendor").join("lib");
    fs::create_dir_all(&inner).unwrap();
    repo.git(&inner, &["init", "-q"]);
    fs::write(inner.join(".gitignore"), "*.tmp\n").unwrap();
    fs::write(inner.join("x.tmp"), "x").unwrap();
    let (scan, _) = repo.scan(&at_root, "vendor/lib");
    assert_eq!(node(&scan, "x.tmp").ignore, ExplorerIgnoreState::Ignored);
    assert_eq!(
        scan.ignore_rule,
        ExplorerIgnoreRule::Git {
            repository: ExplorerRepositoryPlacement::BelowProjectRoot
        }
    );
}

/// D5: a tracked file that matches a pattern is tracked, so it is not ignored on
/// the node either.
#[test]
fn a_tracked_file_matching_a_pattern_is_not_ignored_on_the_node() {
    let repo = Repo::new("tracked");
    repo.write(".gitignore", "*.log\n");
    repo.write("tracked.log", "x");
    repo.write("other.log", "x");
    repo.git(&repo.repo, &["add", "-f", ".gitignore", "tracked.log"]);
    repo.git(&repo.repo, &["commit", "-q", "-m", "initial"]);
    let handle = repo.project("");
    let (scan, _) = repo.scan(&handle, "");
    assert_eq!(
        node(&scan, "tracked.log").ignore,
        ExplorerIgnoreState::NotIgnored
    );
    assert_eq!(
        node(&scan, "other.log").ignore,
        ExplorerIgnoreState::Ignored
    );
}

/// R4 of the RFC and §3 of the risk document: the query is **not a second way
/// into the filesystem.** An entry the access policy blocked (an escaping
/// symlink) is never in the batch and stays unknown.
#[test]
fn a_blocked_entry_is_never_asked_about_and_stays_unknown() {
    let repo = Repo::new("blocked");
    repo.write(".gitignore", "*.log\n");
    repo.write("a.log", "x");
    let outside = repo.base.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.log"), "x").unwrap();
    std::os::unix::fs::symlink("../../outside/secret.log", repo.repo.join("escape.log")).unwrap();
    let handle = repo.project("");
    let (scan, batches) = repo.scan(&handle, "");

    let escape = node(&scan, "escape.log");
    assert!(matches!(escape.state, ExplorerNodeState::Blocked(_)));
    assert_eq!(escape.ignore, ExplorerIgnoreState::Unknown);
    assert!(
        !batches[0].contains(&OsString::from("escape.log")),
        "a blocked entry was put in the query: {:?}",
        batches[0]
    );
    assert_eq!(node(&scan, "a.log").ignore, ExplorerIgnoreState::Ignored);
}

/// R6: the query is asked about the **raw** filename, not the lossy display
/// name -- so a non-UTF-8 name is answered as itself and the answer lands on the
/// right row.
#[test]
fn a_non_utf8_name_is_asked_as_its_bytes_and_the_answer_lands_on_its_row() {
    let repo = Repo::new("non-utf8");
    repo.write(".gitignore", "*.log\n");
    let odd = OsString::from_vec(b"caf\xe9.log".to_vec());
    fs::write(repo.repo.join(&odd), "x").unwrap();
    repo.write("plain.txt", "x");
    let handle = repo.project("");
    let (scan, batches) = repo.scan(&handle, "");

    assert!(
        batches[0].contains(&odd),
        "asked as raw bytes: {:?}",
        batches[0]
    );
    let row = scan
        .nodes
        .iter()
        .find(|node| node.name.contains('\u{FFFD}'))
        .expect("the lossy display name has a replacement character");
    assert_eq!(row.ignore, ExplorerIgnoreState::Ignored);
    assert_eq!(
        node(&scan, "plain.txt").ignore,
        ExplorerIgnoreState::NotIgnored
    );
}

/// The pure scan's placeholder rows are not entries: an unreadable directory
/// entry (named `<unreadable>` by the scanner) is never asked about. Expressed
/// here on the model: only `Available` and `Collapsed` nodes are asked.
#[test]
fn only_admitted_entries_are_put_in_the_batch() {
    let repo = Repo::new("admitted-only");
    repo.write("a.txt", "x");
    repo.mkdir("sub");
    let handle = repo.project("");
    let mut scan = FileExplorerScanner
        .scan_directory(&handle, "", &FileExplorerScanPolicy::linux_mvp())
        .unwrap();
    scan.nodes.push(ExplorerNode {
        name: "<unreadable>".to_owned(),
        relative_path: PathBuf::from("<unreadable>"),
        kind: ExplorerNodeKind::Other,
        state: ExplorerNodeState::Unreadable,
        symlink_status: crate::project::root::FileAccessSymlinkStatus::NoSymlink,
        ignore: ExplorerIgnoreState::Unknown,
    });
    let batches = RefCell::new(Vec::new());
    scan.ask_git(&handle, &|_: &Path, names: &[OsString]| {
        batches.borrow_mut().push(names.to_vec());
        IgnoreReport {
            answer: IgnoreAnswer::NoneIgnored,
            repository_root: Some(fs::canonicalize(&repo.repo).unwrap()),
        }
    });
    let batch = &batches.borrow()[0];
    assert!(!batch.contains(&OsString::from("<unreadable>")));
    assert!(batch.contains(&OsString::from("a.txt")) && batch.contains(&OsString::from("sub")));
    assert_eq!(
        node(&scan, "<unreadable>").ignore,
        ExplorerIgnoreState::Unknown
    );
}

/// **The budget, on RFC-052's 100 000-entry fixture, the whole call.** Expanding
/// a directory now pays the scan *and* the gate *and* one `check-ignore`; the
/// number that matters is the difference the git half adds, measured in one run
/// against the scan alone. The bound is deliberately loose (a wall clock under
/// load is not a property); the numbers are in `qa-evidence.md`.
#[test]
fn asking_git_adds_a_few_milliseconds_to_a_hundred_thousand_entry_scan() {
    use super::hostile_fixture::{BREADTH_ENTRIES, HostileFixture};
    let fixture = HostileFixture::build("ignore-budget", BREADTH_ENTRIES);
    let root = ProjectRootValidator
        .validate(&fixture.project, SymlinkPolicy::FailClosed)
        .unwrap();
    let session = ProjectSession::new(
        ProjectId::for_test(1),
        root.display_name,
        root.selected_path,
        root.canonical_path,
    );
    let handle = ProjectRootHandle::from_project_session(&session);
    // The fixture project is made a repository (its config is allowlist-clean).
    let home = fixture.base.join("home");
    fs::create_dir_all(&home).unwrap();
    let env = vec![("HOME".to_owned(), home.display().to_string())];
    let status = Command::new("git")
        .args(["init", "-q"])
        .current_dir(&fixture.project)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .envs(env.iter().cloned())
        .status()
        .unwrap();
    assert!(status.success());
    fs::write(fixture.project.join(".gitignore"), "f0000*\n").unwrap();

    let policy = FileExplorerScanPolicy::linux_mvp();
    let mut scan_only = Duration::MAX;
    let mut whole = Duration::MAX;
    let mut ask_only = Duration::MAX;
    let mut last = None;
    for _ in 0..5 {
        let start = Instant::now();
        let mut scan = FileExplorerScanner
            .scan_directory(&handle, "breadth", &policy)
            .unwrap();
        let scanned = start.elapsed();
        scan_only = scan_only.min(scanned);
        let asking = Instant::now();
        scan.ask_git(&handle, &|directory: &Path, names: &[OsString]| {
            ignored_entries_in_environment(directory, names, "git", &env)
        });
        ask_only = ask_only.min(asking.elapsed());
        whole = whole.min(start.elapsed());
        last = Some(scan);
    }
    let scan = last.unwrap();
    assert_eq!(scan.nodes.len(), 256);
    assert!(matches!(scan.ignore_rule, ExplorerIgnoreRule::Git { .. }));
    eprintln!(
        "PR-055-B budget, {BREADTH_ENTRIES}-entry directory, best of 5: scan alone {scan_only:?}; \
         asking git (gate + one check-ignore over 256 names) {ask_only:?}; whole call {whole:?}"
    );
    assert!(
        ask_only < Duration::from_millis(250),
        "asking git took {ask_only:?} for one directory"
    );
}

/// The checklist's "states the two consumers are no longer symmetrical", held
/// at the source so the sentence cannot be edited away with nobody noticing.
#[test]
fn the_shared_directory_list_says_its_two_readers_no_longer_mean_the_same_thing() {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/project/ignored_directories.rs"),
    )
    .unwrap();
    let flat = source.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(flat.contains("its two readers no longer mean the same thing"));
    assert!(flat.contains("floor"));
    assert!(flat.contains("GeneratedChangeDetectionPolicy"));
}
