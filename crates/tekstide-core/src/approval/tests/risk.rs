use crate::approval::classify;
use crate::domain::RiskLevel;
use std::path::Path;

const PROJECT_ROOT: &str = "/home/user/project";
const STATE_ROOT: &str = "/home/user/.local/share/tekstide";

fn argv(entries: &[&str]) -> Vec<String> {
    entries.iter().map(|s| s.to_string()).collect()
}

fn classify_at(entries: &[&str], cwd: &str) -> RiskLevel {
    classify(
        &argv(entries),
        Path::new(cwd),
        Path::new(PROJECT_ROOT),
        Path::new(STATE_ROOT),
    )
}

fn classify_in_root(entries: &[&str]) -> RiskLevel {
    classify_at(entries, PROJECT_ROOT)
}

/// The one property to get right before anything else, per
/// `implementation-handoff.md` §4: a program this classifier has never
/// heard of, doing something unrecognized, must be `High` -- never the
/// silent default of `Low` an unhandled fallthrough would otherwise give.
#[test]
fn unrecognized_program_classifies_high_never_low() {
    assert_eq!(
        classify_in_root(&["some-totally-unknown-vendor-cli", "--do-a-thing"]),
        RiskLevel::High
    );
}

/// Fixture corpus: `(name, argv, expected)`. Extend this table rather than
/// writing one-off tests for new argv forms -- that is the shape the
/// review process expects (per response 107/109 precedent on other
/// corpora in this project, and `implementation-handoff.md` §4's explicit
/// request for a table in this shape).
fn corpus() -> Vec<(&'static str, Vec<&'static str>, RiskLevel)> {
    vec![
        // --- Low: recognized read-only operations ---
        ("git status", vec!["git", "status"], RiskLevel::Low),
        ("git log", vec!["git", "log", "--oneline"], RiskLevel::Low),
        ("git diff", vec!["git", "diff"], RiskLevel::Low),
        ("ls", vec!["ls", "-la"], RiskLevel::Low),
        ("cat a file", vec!["cat", "README.md"], RiskLevel::Low),
        ("pwd", vec!["pwd"], RiskLevel::Low),
        // --- Medium: recognized, ordinary, mutating-but-mundane ---
        (
            "git add",
            vec!["git", "add", "src/main.rs"],
            RiskLevel::Medium,
        ),
        (
            "git commit",
            vec!["git", "commit", "-m", "message"],
            RiskLevel::Medium,
        ),
        ("cargo build", vec!["cargo", "build"], RiskLevel::Medium),
        ("npm install", vec!["npm", "install"], RiskLevel::Medium),
        (
            "cp within root",
            vec!["cp", "a.txt", "b.txt"],
            RiskLevel::Medium,
        ),
        ("mkdir", vec!["mkdir", "newdir"], RiskLevel::Medium),
        // --- High: escalation rules from implementation-handoff.md §4 ---
        (
            "absolute path outside root",
            vec!["cat", "/etc/passwd"],
            RiskLevel::High,
        ),
        (
            "relative path escaping root via ..",
            vec!["cat", "../../etc/passwd"],
            RiskLevel::High,
        ),
        (
            "deeply nested .. still escapes",
            vec!["cat", "a/b/../../../../etc/passwd"],
            RiskLevel::High,
        ),
        ("sudo", vec!["sudo", "ls"], RiskLevel::High),
        ("doas", vec!["doas", "ls"], RiskLevel::High),
        ("pkexec", vec!["pkexec", "ls"], RiskLevel::High),
        (
            "elevation via absolute path",
            vec!["/usr/bin/sudo", "ls"],
            RiskLevel::High,
        ),
        (
            "elevation via wrapper indirection",
            vec!["env", "sudo", "ls"],
            RiskLevel::High,
        ),
        (
            "shell -c is opaque to structural inspection",
            vec!["bash", "-c", "echo hi"],
            RiskLevel::High,
        ),
        ("git push", vec!["git", "push"], RiskLevel::High),
        (
            "git push force",
            vec!["git", "push", "--force"],
            RiskLevel::High,
        ),
        (
            "git remote add",
            vec!["git", "remote", "add", "origin", "url"],
            RiskLevel::High,
        ),
        (
            "git tag delete",
            vec!["git", "tag", "-d", "v1.0"],
            RiskLevel::High,
        ),
        (
            "ssh key path",
            vec!["cat", "/home/user/.ssh/id_rsa"],
            RiskLevel::High,
        ),
        (
            "aws credentials path",
            vec!["cat", "/home/user/.aws/credentials"],
            RiskLevel::High,
        ),
        (
            "relative ssh key path",
            vec!["cat", ".ssh/id_rsa"],
            RiskLevel::High,
        ),
        (
            "write targeting tekstide state root",
            vec!["cat", "/home/user/.local/share/tekstide/audit.sqlite"],
            RiskLevel::High,
        ),
        (
            "unrecognized program",
            vec!["some-vendor-tool", "--flag"],
            RiskLevel::High,
        ),
        // --- Destructive ---
        (
            "rm -rf",
            vec!["rm", "-rf", "build/"],
            RiskLevel::Destructive,
        ),
        (
            "rm -r long form",
            vec!["rm", "--recursive", "build/"],
            RiskLevel::Destructive,
        ),
        (
            "dd",
            vec!["dd", "if=/dev/zero", "of=/dev/sda"],
            RiskLevel::Destructive,
        ),
        (
            "mkfs",
            vec!["mkfs.ext4", "/dev/sda1"],
            RiskLevel::Destructive,
        ),
        (
            "git rebase",
            vec!["git", "rebase", "-i", "HEAD~3"],
            RiskLevel::Destructive,
        ),
        (
            "git filter-branch",
            vec!["git", "filter-branch", "--force"],
            RiskLevel::Destructive,
        ),
        (
            "git reset --hard",
            vec!["git", "reset", "--hard", "HEAD~1"],
            RiskLevel::Destructive,
        ),
    ]
}

#[test]
fn fixture_corpus_classifies_as_expected() {
    for (name, entries, expected) in corpus() {
        let refs: Vec<&str> = entries.to_vec();
        let actual = classify_in_root(&refs);
        assert_eq!(
            actual, expected,
            "case {name:?}: argv={entries:?} expected {expected:?} got {actual:?}"
        );
    }
}

#[test]
fn relative_path_within_root_from_a_subdirectory_is_not_escalated() {
    // cwd is a subdirectory of the project root; a plain relative
    // argument should resolve inside the root and not escalate.
    assert_eq!(
        classify_at(&["cat", "lib.rs"], "/home/user/project/src"),
        RiskLevel::Low
    );
}

#[test]
fn relative_path_escaping_root_from_a_subdirectory_is_escalated() {
    // From a subdirectory, ".." only needs to climb past the
    // subdirectory and the root itself to escape -- confirms resolution
    // is relative to the proposal's own cwd, not always the root.
    assert_eq!(
        classify_at(&["cat", "../../../etc/passwd"], "/home/user/project/src"),
        RiskLevel::High
    );
}

#[test]
fn destructive_outranks_high_when_both_would_otherwise_apply() {
    // `/etc` is outside the project root (a High trigger on its own) and
    // `rm -rf` is a Destructive trigger. Destructive must win: it is
    // checked first and returns immediately, since it is the more severe
    // classification.
    assert_eq!(
        classify_in_root(&["rm", "-rf", "/etc"]),
        RiskLevel::Destructive
    );
}

#[test]
fn empty_argv_does_not_panic_and_classifies_high() {
    // CommandProposal::decode already rejects empty argv, so this input
    // is unreachable from a validated proposal -- but this function must
    // still behave defensively rather than panic if ever called directly.
    assert_eq!(classify_in_root(&[]), RiskLevel::High);
}
