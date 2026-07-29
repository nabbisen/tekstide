use crate::approval::{RiskAssessment, RiskReason, classify};
use crate::domain::RiskLevel;
use std::path::Path;

const PROJECT_ROOT: &str = "/home/user/project";
const STATE_ROOT: &str = "/home/user/.local/share/tekstide";

fn argv(entries: &[&str]) -> Vec<String> {
    entries.iter().map(|s| s.to_string()).collect()
}

fn classify_at(
    entries: &[&str],
    cwd: &str,
    project_root: &str,
    state_root: &str,
) -> RiskAssessment {
    classify(
        &argv(entries),
        Path::new(cwd),
        Path::new(project_root),
        Path::new(state_root),
    )
}

fn classify_in_root(entries: &[&str]) -> RiskAssessment {
    classify_at(entries, PROJECT_ROOT, PROJECT_ROOT, STATE_ROOT)
}

/// The one property to get right before anything else, per
/// `implementation-handoff.md` §4: a program this classifier has never
/// heard of, doing something unrecognized, must be `High` -- never the
/// silent default of `Low` an unhandled fallthrough would otherwise give.
#[test]
fn unrecognized_program_classifies_high_never_low() {
    let assessment = classify_in_root(&["some-totally-unknown-vendor-cli", "--do-a-thing"]);
    assert_eq!(assessment.level, RiskLevel::High);
    assert_eq!(assessment.reasons, vec![RiskReason::Unrecognized]);
}

/// Fixture corpus: `(name, argv, expected_level, reason_that_must_be_present)`.
/// `reason` is `None` only where the level itself is already discriminating
/// without it (response 110 Mandatory 1: for every `High` case that could
/// also be reached by the "unrecognized" fallthrough, `reason` must be
/// `Some` and must not be `Unrecognized`, or the case proves nothing --
/// response 110 demonstrated this empirically by deleting five escalation
/// rules and finding every old assertion still passed).
fn corpus() -> Vec<(
    &'static str,
    Vec<&'static str>,
    RiskLevel,
    Option<RiskReason>,
)> {
    vec![
        // --- Low: recognized read-only operations ---
        ("git status", vec!["git", "status"], RiskLevel::Low, None),
        (
            "git log",
            vec!["git", "log", "--oneline"],
            RiskLevel::Low,
            None,
        ),
        ("git diff", vec!["git", "diff"], RiskLevel::Low, None),
        ("ls", vec!["ls", "-la"], RiskLevel::Low, None),
        ("cat a file", vec!["cat", "README.md"], RiskLevel::Low, None),
        ("pwd", vec!["pwd"], RiskLevel::Low, None),
        (
            "argv[0] as resolved absolute path",
            vec!["/usr/bin/git", "status"],
            RiskLevel::Low,
            None,
        ),
        (
            "argv[0] as resolved absolute path, ls",
            vec!["/bin/ls", "-la"],
            RiskLevel::Low,
            None,
        ),
        (
            "git remote read-only form",
            vec!["git", "remote", "-v"],
            RiskLevel::Low,
            None,
        ),
        (
            "ordinary commit message mentioning a secret-shaped word",
            vec!["git", "commit", "-m", "rotate credentials"],
            RiskLevel::Medium,
            None,
        ),
        (
            "in-root file literally named credentials.md",
            vec!["cat", "credentials.md"],
            RiskLevel::Low,
            None,
        ),
        (
            "= split does not apply to free text (response 111 non-blocking-2)",
            vec!["git", "commit", "-m", "note a=/etc/passwd"],
            RiskLevel::Medium,
            None,
        ),
        (
            "= split does not apply to a non-option argument",
            vec!["echo", "FOO=/etc/passwd"],
            RiskLevel::Low,
            None,
        ),
        // --- Medium: recognized, ordinary, mutating-but-mundane ---
        (
            "git add",
            vec!["git", "add", "src/main.rs"],
            RiskLevel::Medium,
            None,
        ),
        (
            "git commit",
            vec!["git", "commit", "-m", "message"],
            RiskLevel::Medium,
            None,
        ),
        (
            "cargo build",
            vec!["cargo", "build"],
            RiskLevel::Medium,
            None,
        ),
        (
            "npm install",
            vec!["npm", "install"],
            RiskLevel::Medium,
            None,
        ),
        (
            "cp within root",
            vec!["cp", "a.txt", "b.txt"],
            RiskLevel::Medium,
            None,
        ),
        ("mkdir", vec!["mkdir", "newdir"], RiskLevel::Medium, None),
        (
            "git checkout a branch",
            vec!["git", "checkout", "feature-branch"],
            RiskLevel::Medium,
            None,
        ),
        (
            "git stash push",
            vec!["git", "stash", "push"],
            RiskLevel::Medium,
            None,
        ),
        // --- High: escalation rules from implementation-handoff.md §4 ---
        (
            "absolute path outside root",
            vec!["cat", "/etc/passwd"],
            RiskLevel::High,
            Some(RiskReason::PathOutsideProjectRoot),
        ),
        (
            "relative path escaping root via ..",
            vec!["cat", "../../etc/passwd"],
            RiskLevel::High,
            Some(RiskReason::PathOutsideProjectRoot),
        ),
        (
            "deeply nested .. still escapes",
            vec!["cat", "a/b/../../../../etc/passwd"],
            RiskLevel::High,
            Some(RiskReason::PathOutsideProjectRoot),
        ),
        (
            "sudo",
            vec!["sudo", "ls"],
            RiskLevel::High,
            Some(RiskReason::PrivilegeElevation),
        ),
        (
            "doas",
            vec!["doas", "ls"],
            RiskLevel::High,
            Some(RiskReason::PrivilegeElevation),
        ),
        (
            "pkexec",
            vec!["pkexec", "ls"],
            RiskLevel::High,
            Some(RiskReason::PrivilegeElevation),
        ),
        (
            "su -c",
            vec!["su", "-c", "x"],
            RiskLevel::High,
            Some(RiskReason::PrivilegeElevation),
        ),
        (
            "run0",
            vec!["run0", "ls"],
            RiskLevel::High,
            Some(RiskReason::PrivilegeElevation),
        ),
        (
            "elevation via absolute path",
            vec!["/usr/bin/sudo", "ls"],
            RiskLevel::High,
            Some(RiskReason::PrivilegeElevation),
        ),
        (
            "elevation via env wrapper",
            vec!["env", "sudo", "ls"],
            RiskLevel::High,
            Some(RiskReason::PrivilegeElevation),
        ),
        (
            "shell -c is opaque regardless of flag spelling",
            vec!["bash", "-c", "echo hi"],
            RiskLevel::High,
            Some(RiskReason::OpaqueShellInvocation),
        ),
        (
            "shell with unusual combined flags still opaque",
            vec!["bash", "-lc", "x"],
            RiskLevel::High,
            Some(RiskReason::OpaqueShellInvocation),
        ),
        (
            "opaque wrapper without a recognized real command",
            vec!["env", "git", "status"],
            RiskLevel::High,
            Some(RiskReason::OpaqueWrapper),
        ),
        (
            "git push",
            vec!["git", "push"],
            RiskLevel::High,
            Some(RiskReason::GitRemoteMutating),
        ),
        (
            "git remote add",
            vec!["git", "remote", "add", "origin", "url"],
            RiskLevel::High,
            Some(RiskReason::GitRemoteMutating),
        ),
        (
            "git remote flag before the action verb (response 111 Required 1)",
            vec!["git", "remote", "-v", "remove", "origin"],
            RiskLevel::High,
            Some(RiskReason::GitRemoteMutating),
        ),
        (
            "git remote long-flag before the action verb",
            vec!["git", "remote", "--verbose", "set-url", "origin", "u"],
            RiskLevel::High,
            Some(RiskReason::GitRemoteMutating),
        ),
        (
            "git tag delete",
            vec!["git", "tag", "-d", "v1.0"],
            RiskLevel::High,
            Some(RiskReason::GitRemoteMutating),
        ),
        (
            "git branch delete is not blanket-allowlisted",
            vec!["git", "branch", "-D", "feature"],
            RiskLevel::High,
            Some(RiskReason::Unrecognized),
        ),
        (
            "git branch long-form delete/force flags",
            vec!["git", "branch", "--delete", "--force", "feature"],
            RiskLevel::High,
            Some(RiskReason::Unrecognized),
        ),
        (
            "ssh key path",
            vec!["cat", "/home/user/.ssh/id_rsa"],
            RiskLevel::High,
            Some(RiskReason::SecretLikePath),
        ),
        (
            "aws credentials path",
            vec!["cat", "/home/user/.aws/credentials"],
            RiskLevel::High,
            Some(RiskReason::SecretLikePath),
        ),
        (
            "relative ssh key path",
            vec!["cat", ".ssh/id_rsa"],
            RiskLevel::High,
            Some(RiskReason::SecretLikePath),
        ),
        (
            "attached long-option path escape",
            vec!["cp", "--target-directory=/etc", "a.txt"],
            RiskLevel::High,
            Some(RiskReason::PathOutsideProjectRoot),
        ),
        (
            "attached long-option path escape, cargo",
            vec!["cargo", "build", "--target-dir=/etc"],
            RiskLevel::High,
            Some(RiskReason::PathOutsideProjectRoot),
        ),
        (
            "attached long-option secret path",
            vec!["cat", "--file=/home/user/.ssh/id_rsa"],
            RiskLevel::High,
            Some(RiskReason::SecretLikePath),
        ),
        (
            "unrecognized program",
            vec!["some-vendor-tool", "--flag"],
            RiskLevel::High,
            Some(RiskReason::Unrecognized),
        ),
        (
            "exact-match disk-level program, not prefix",
            vec!["shredder", "--help"],
            RiskLevel::High,
            Some(RiskReason::Unrecognized),
        ),
        (
            "exact-match disk-level program, not prefix, dd-like name",
            vec!["ddgr", "rust"],
            RiskLevel::High,
            Some(RiskReason::Unrecognized),
        ),
        (
            "chmod -R is recursive but not deletion -- unrecognized, not destructive",
            vec!["chmod", "-R", "777", "."],
            RiskLevel::High,
            Some(RiskReason::Unrecognized),
        ),
        // --- Destructive ---
        (
            "rm -rf",
            vec!["rm", "-rf", "build/"],
            RiskLevel::Destructive,
            Some(RiskReason::RecursiveDeletion),
        ),
        (
            "rm -r long form",
            vec!["rm", "--recursive", "build/"],
            RiskLevel::Destructive,
            Some(RiskReason::RecursiveDeletion),
        ),
        (
            "rm -fR alternate flag order",
            vec!["rm", "-fR", "build/"],
            RiskLevel::Destructive,
            Some(RiskReason::RecursiveDeletion),
        ),
        (
            "dd",
            vec!["dd", "if=/dev/zero", "of=/dev/sda"],
            RiskLevel::Destructive,
            Some(RiskReason::DiskLevelOperation),
        ),
        (
            "mkfs",
            vec!["mkfs.ext4", "/dev/sda1"],
            RiskLevel::Destructive,
            Some(RiskReason::DiskLevelOperation),
        ),
        (
            "git rebase",
            vec!["git", "rebase", "-i", "HEAD~3"],
            RiskLevel::Destructive,
            Some(RiskReason::HistoryRewrite),
        ),
        (
            "git filter-branch",
            vec!["git", "filter-branch", "--force"],
            RiskLevel::Destructive,
            Some(RiskReason::HistoryRewrite),
        ),
        (
            "git reset --hard",
            vec!["git", "reset", "--hard", "HEAD~1"],
            RiskLevel::Destructive,
            Some(RiskReason::HistoryRewrite),
        ),
        (
            "git checkout -- discards working tree changes",
            vec!["git", "checkout", "--", "."],
            RiskLevel::Destructive,
            Some(RiskReason::WorkingTreeDiscard),
        ),
        (
            "git checkout . discards working tree changes",
            vec!["git", "checkout", "."],
            RiskLevel::Destructive,
            Some(RiskReason::WorkingTreeDiscard),
        ),
        (
            "git checkout -f discards local modifications (response 111 non-blocking-1)",
            vec!["git", "checkout", "-f", "feature"],
            RiskLevel::Destructive,
            Some(RiskReason::WorkingTreeDiscard),
        ),
        (
            "git checkout --force, long form",
            vec!["git", "checkout", "--force", "feature"],
            RiskLevel::Destructive,
            Some(RiskReason::WorkingTreeDiscard),
        ),
        (
            "git push --force rewrites remote history (response 111 Required 2)",
            vec!["git", "push", "--force"],
            RiskLevel::Destructive,
            Some(RiskReason::RemoteHistoryRewrite),
        ),
        (
            "git push --force-with-lease, same severity (not distinguished, see Known Limitations)",
            vec!["git", "push", "--force-with-lease"],
            RiskLevel::Destructive,
            Some(RiskReason::RemoteHistoryRewrite),
        ),
        (
            "git stash clear purges saved work (response 111 Required 3)",
            vec!["git", "stash", "clear"],
            RiskLevel::High,
            Some(RiskReason::WorkingTreeDiscard),
        ),
        (
            "git stash drop, same reason",
            vec!["git", "stash", "drop"],
            RiskLevel::High,
            Some(RiskReason::WorkingTreeDiscard),
        ),
    ]
}

#[test]
fn fixture_corpus_classifies_as_expected() {
    for (name, entries, expected_level, expected_reason) in corpus() {
        let refs: Vec<&str> = entries.to_vec();
        let assessment = classify_in_root(&refs);
        assert_eq!(
            assessment.level, expected_level,
            "case {name:?}: argv={entries:?} expected level {expected_level:?} got {:?} (reasons: {:?})",
            assessment.level, assessment.reasons
        );
        if let Some(reason) = expected_reason {
            assert!(
                assessment.reasons.contains(&reason),
                "case {name:?}: argv={entries:?} expected reason {reason:?} to be present, got {:?}",
                assessment.reasons
            );
        }
    }
}

#[test]
fn relative_path_within_root_from_a_subdirectory_is_not_escalated() {
    // cwd is a subdirectory of the project root; a plain relative
    // argument should resolve inside the root and not escalate.
    let assessment = classify_at(
        &["cat", "lib.rs"],
        "/home/user/project/src",
        PROJECT_ROOT,
        STATE_ROOT,
    );
    assert_eq!(assessment.level, RiskLevel::Low);
}

#[test]
fn relative_path_escaping_root_from_a_subdirectory_is_escalated() {
    // From a subdirectory, ".." only needs to climb past the
    // subdirectory and the root itself to escape -- confirms resolution
    // is relative to the proposal's own cwd, not always the root.
    let assessment = classify_at(
        &["cat", "../../../etc/passwd"],
        "/home/user/project/src",
        PROJECT_ROOT,
        STATE_ROOT,
    );
    assert_eq!(assessment.level, RiskLevel::High);
    assert!(
        assessment
            .reasons
            .contains(&RiskReason::PathOutsideProjectRoot)
    );
}

#[test]
fn destructive_outranks_high_when_both_would_otherwise_apply() {
    // `/etc` is outside the project root (a High trigger on its own) and
    // `rm -rf` is a Destructive trigger. Destructive must win: it is
    // checked first and returns immediately, since it is the more severe
    // classification.
    let assessment = classify_in_root(&["rm", "-rf", "/etc"]);
    assert_eq!(assessment.level, RiskLevel::Destructive);
}

#[test]
fn empty_argv_does_not_panic_and_classifies_high() {
    // CommandProposal::decode already rejects empty argv, so this input
    // is unreachable from a validated proposal -- but this function must
    // still behave defensively rather than panic if ever called directly.
    let assessment = classify_in_root(&[]);
    assert_eq!(assessment.level, RiskLevel::High);
}

#[test]
fn state_root_write_discriminates_only_when_state_root_is_inside_project_root() {
    // response 110: "the state-root rule is only non-redundant when the
    // state root sits inside a project root -- which is exactly the case
    // no fixture covers." Construct exactly that arrangement.
    let nested_state_root = "/home/user/project/.tekstide-state";
    let assessment = classify_at(
        &["cat", ".tekstide-state/audit.sqlite"],
        PROJECT_ROOT,
        PROJECT_ROOT,
        nested_state_root,
    );
    assert_eq!(assessment.level, RiskLevel::High);
    assert!(assessment.reasons.contains(&RiskReason::TekstideStateRoot));
    // And it must NOT also claim PathOutsideProjectRoot, since the path is
    // inside the project root -- this is what makes the case discriminate
    // the state-root rule specifically rather than the root-escape rule.
    assert!(
        !assessment
            .reasons
            .contains(&RiskReason::PathOutsideProjectRoot)
    );
}

#[test]
fn ablation_privilege_elevation_rule_is_load_bearing() {
    // Regression guard against response 110 Mandatory 1's failure mode:
    // this test would still pass on `level` alone even with the rule
    // deleted, since High is also the fallthrough. Asserting the reason
    // is what makes it discriminating.
    let assessment = classify_in_root(&["sudo", "ls"]);
    assert_eq!(assessment.reasons, vec![RiskReason::PrivilegeElevation]);
}

#[test]
fn remote_mutating_scan_is_not_defeated_by_a_flag_before_the_verb() {
    // Response 111 Required 1: a fixed-index read of argv[2] missed the
    // action verb whenever a flag preceded it. This is the adversarial
    // input, distinct from an ablation test -- ablation proves the rule
    // is reachable, this proves it is not evadable by a shape the rule
    // author didn't happen to write a fixture for.
    let assessment = classify_in_root(&["git", "remote", "-v", "remove", "origin"]);
    assert_eq!(assessment.level, RiskLevel::High);
    assert!(assessment.reasons.contains(&RiskReason::GitRemoteMutating));
}

#[test]
fn ablation_force_push_rule_is_load_bearing_and_at_destructive_severity() {
    // Response 111 Required 2: this rule was previously dead (ablating it
    // changed no test outcome, since `push` was already High via
    // ALWAYS_MUTATING_GIT_SUBCOMMANDS). Asserting Destructive specifically
    // -- not just that some reason is present -- is what makes force-push
    // distinguishable from an ordinary push.
    let assessment = classify_in_root(&["git", "push", "--force"]);
    assert_eq!(assessment.level, RiskLevel::Destructive);
    assert_eq!(assessment.reasons, vec![RiskReason::RemoteHistoryRewrite]);
}

#[test]
fn ablation_stash_purge_reason_is_load_bearing() {
    // Response 111 Required 3: the level was already right (stash purge
    // fell through to High), but the reason was Unrecognized -- a
    // deliberate decision indistinguishable from an oversight. Asserting
    // the specific reason means deleting `is_stash_purge`'s call site
    // would now be caught.
    let assessment = classify_in_root(&["git", "stash", "clear"]);
    assert_eq!(assessment.reasons, vec![RiskReason::WorkingTreeDiscard]);
}

#[test]
fn argv_zero_as_absolute_path_does_not_escalate_via_containment() {
    // Response 110 Recommended 2: adapters routinely emit a resolved
    // absolute path to the program itself. If argv[0] were checked for
    // containment, this would incorrectly classify High.
    let assessment = classify_in_root(&["/usr/bin/git", "status"]);
    assert_eq!(assessment.level, RiskLevel::Low);
}
