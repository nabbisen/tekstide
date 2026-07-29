//! RFC-021 PR-021-C: structural risk classification of a proposed command.
//!
//! **Structural, not semantic.** This module inspects `argv` and `cwd` as
//! strings and path components; it does not interpret shell grammar, does
//! not know what any given program actually does, and does not execute
//! anything. That is an explicit RFC-021 non-goal, not an oversight.
//!
//! **Classification affects presentation and audit, not whether approval
//! is required.** Every proposal requires a decision regardless of the
//! level this module returns -- see `implementation-handoff.md` §4.
//!
//! **Unclassifiable input classifies `High`, never `Low`.** This is the
//! single property to get right before anything else, and it shapes the
//! whole design here: `Low` and `Medium` are only reachable through a
//! small, explicit allowlist of recognized, argument-checked forms.
//! Anything not recognized -- a program this module has never heard of,
//! or a recognized program used in a shape nobody vouched for (response
//! 110 Mandatory 3: an allowlist entry must be a form you can vouch for
//! *entirely*, including its flags) -- falls through to `High` by
//! construction.
//!
//! **`RiskAssessment` carries *why*, not just the level** (response 110
//! Recommended 1). Two independent reasons: a fixture assertion of the
//! form `classify(...).level == High` cannot fail when `High` is also the
//! fallthrough for anything unrecognized -- response 110 proved this by
//! deleting five escalation rules and watching every existing test still
//! pass. Asserting a specific `RiskReason` is present makes a corpus entry
//! discriminating by construction. Separately, PR-021-E's dialog has to
//! tell a user *why* a command is `High` -- "High" alone is not an
//! actionable prompt. `RiskReason` stays content-free (no captured paths,
//! no captured argv), so it remains safe to log under RFC-013's privacy
//! rule.
//!
//! **Path containment is checked lexically, not via `fs::canonicalize`**
//! (response 110 Recommended 3 confirms this design was correct, not just
//! adequate): a proposed command may reference a path that does not exist
//! yet, so requiring the filesystem to agree before classifying anything
//! would be wrong. This means an in-root symlink that points outside the
//! root is **not** detected here -- that enforcement lives in RFC-011/
//! RFC-012's detectors, at the point of actual file access, which is where
//! it belongs. Recorded as a known limitation, not silently implied away
//! by the word "canonical" (which the checklist used incorrectly before
//! response 110 -- corrected to "lexical").
//!
//! **`argv[0]` is excluded from containment checks only** (response 110
//! Recommended 2): adapters routinely emit a resolved absolute path to the
//! program itself (`/usr/bin/git status`), and checking that against the
//! project root would make nearly every proposal `High` regardless of
//! what it actually does -- an inert classifier. `argv[0]` still
//! participates in the elevation and secret-pattern scans, since a
//! program *named* `sudo` or living somewhere secret-pattern-shaped is
//! still meaningful regardless of whether it is "inside the project."
//!
//! **Attached option values are resolved too** (response 110 Mandatory 2):
//! `--target-directory=/etc` and `cp -t /etc` must classify the same way,
//! since they specify the same effect through different syntax. Every
//! argv entry contributes both itself and (if it contains `=`) the
//! substring after the first `=` as path candidates. Attached *short*
//! options (`-o/etc/x`) remain uncovered -- disclosed, not chased.
//!
//! **Secret-like patterns match path components, not raw substrings**
//! (response 110 Recommended 2): matching `"credentials"` as a substring
//! of the whole resolved string escalated `git commit -m "rotate
//! credentials"`, an ordinary sentence that happens to resolve (jointly
//! with `cwd`) to a path-shaped string containing that word. Matching
//! against individual path *components* instead means a free-text
//! argument that is not actually a path essentially never accidentally
//! matches a directory-name-shaped pattern like `.ssh` or `.aws`.
//!
//! **Wrapper indirection is only partially handled**, and this is
//! unchanged from the previous version of this module, confirmed still
//! true by response 110's own probing: `env sudo rm -rf /` is caught (the
//! elevation scan checks every argv entry's basename), a shell
//! interpreter as `argv[0]` is caught unconditionally (see
//! `SHELL_INTERPRETERS` below), and a small explicit opaque-wrapper list
//! (`env`, `nice`, `timeout`, `xargs`, `stdbuf`, `nohup`) is caught by
//! name. But none of these *unwrap* to recognize the real command
//! underneath -- `env git status` lands on `High`/`OpaqueWrapper` rather
//! than the correct `Low`. Response 110 confirmed this is over-
//! classification, not under-classification, and endorsed deferring a fix
//! rather than building unreviewed unwrapping under time pressure.
//!
//! **The handoff's premise that a secret-like-path pattern set "already"
//! exists for environment redaction does not hold** (response 110 Q3):
//! searched the codebase and found only the RFC-004 *policy* statement, no
//! concrete pattern list. `SECRET_LIKE_EXACT_COMPONENTS`/
//! `SECRET_LIKE_COMPONENT_SUFFIXES` below are therefore newly authored for
//! this slice and are narrow, not authoritative -- the reviewer is
//! recording the RFC-004 environment-variable-name gap separately; this
//! list matches *filesystem path components*, a different and
//! non-redundant thing.

use std::path::{Component, Path, PathBuf};

use crate::domain::RiskLevel;

/// A structural reason a proposal was classified the way it was.
/// Deliberately content-free: no captured path, no captured argv entry --
/// see the module doc's `RiskAssessment` paragraph for why.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiskReason {
    PathOutsideProjectRoot,
    PrivilegeElevation,
    OpaqueShellInvocation,
    OpaqueWrapper,
    GitRemoteMutating,
    SecretLikePath,
    TekstideStateRoot,
    RecursiveDeletion,
    DiskLevelOperation,
    HistoryRewrite,
    /// `git checkout -- <path>` / `git checkout .` / `git checkout -f`
    /// (or `--force`): discards uncommitted working-tree changes,
    /// unrecoverably (nothing was ever committed, so there is no reflog
    /// rescue) -- the same category of data loss as `git reset --hard`,
    /// which is `Destructive`. Also used for `git stash clear`/`drop`
    /// (response 111 Required 3): a purge of saved work is the same shape
    /// of loss, one step removed. Kept distinct from `HistoryRewrite`
    /// because no history is being rewritten; nothing was ever committed
    /// in the first place.
    WorkingTreeDiscard,
    /// `git push --force`/`--force-with-lease`: rewrites history on a
    /// **remote other people pull from**, where there is no reflog to
    /// rescue and the loss is not the operator's alone -- response 111
    /// Required 2 judged this strictly worse than local history rewriting
    /// (`HistoryRewrite`, `Destructive` via `git rebase`/`reset --hard`),
    /// so it gets its own reason at the same `Destructive` level rather
    /// than being folded into either `HistoryRewrite` or the `High`-level
    /// `GitRemoteMutating`.
    RemoteHistoryRewrite,
    Unrecognized,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskAssessment {
    pub level: RiskLevel,
    pub reasons: Vec<RiskReason>,
}

impl RiskAssessment {
    fn destructive(reasons: Vec<RiskReason>) -> Self {
        Self {
            level: RiskLevel::Destructive,
            reasons,
        }
    }

    fn high(reasons: Vec<RiskReason>) -> Self {
        Self {
            level: RiskLevel::High,
            reasons,
        }
    }

    fn low() -> Self {
        Self {
            level: RiskLevel::Low,
            reasons: Vec::new(),
        }
    }

    fn medium() -> Self {
        Self {
            level: RiskLevel::Medium,
            reasons: Vec::new(),
        }
    }
}

/// Programs whose presence anywhere in argv means "asking for elevated
/// privilege" -- checked by basename so `/usr/bin/sudo` is caught the same
/// as `sudo`, and scanned across every argv entry so `env sudo ls` is
/// caught too.
const ELEVATION_PROGRAMS: &[&str] = &["sudo", "doas", "pkexec", "su", "run0"];

/// `argv[0]` being one of these is `High` unconditionally, regardless of
/// flags (response 110 Q1: simpler and more defensible than enumerating
/// flag spellings like `-c` vs `-lc` -- the honest reason is that a shell
/// invocation is opaque to structural inspection full stop, not that a
/// specific flag "hides" something the classifier would otherwise see).
const SHELL_INTERPRETERS: &[&str] = &["sh", "bash", "zsh", "dash", "ksh", "fish"];

/// `argv[0]` being one of these means the real command is whatever
/// follows, which this module does not unwrap (see module doc). Recorded
/// on purpose via `RiskReason::OpaqueWrapper` rather than left to fall
/// through to the generic `Unrecognized` reason by accident.
const OPAQUE_WRAPPER_PROGRAMS: &[&str] = &["env", "nice", "timeout", "xargs", "stdbuf", "nohup"];

const DISK_LEVEL_EXACT_PROGRAMS: &[&str] = &["dd", "fdisk", "sfdisk", "parted", "wipefs", "shred"];

/// Path components matched exactly (not a substring search over the whole
/// path -- see module doc). Narrow and not claimed authoritative.
const SECRET_LIKE_EXACT_COMPONENTS: &[&str] = &[
    ".ssh",
    ".gnupg",
    ".netrc",
    ".aws",
    ".docker",
    ".git-credentials",
    "id_rsa",
    "id_ed25519",
    "id_ecdsa",
];
/// Path components matched by suffix (extension-like patterns that are
/// never a whole component on their own).
const SECRET_LIKE_COMPONENT_SUFFIXES: &[&str] = &[".pem", ".pgp"];

const LOW_RISK_GIT_SUBCOMMANDS: &[&str] = &["status", "log", "diff", "show"];
const LOW_RISK_PROGRAMS: &[&str] = &[
    "ls", "pwd", "echo", "cat", "head", "tail", "wc", "printf", "whoami", "date",
];

/// `checkout` and `stash` are deliberately absent: their subcommand forms
/// are handled specially below rather than being blanket-allowlisted (see
/// `is_working_tree_discard` and `is_stash_purge`), per response 110's
/// point that `git branch`-style blanket allowlisting of a program whose
/// flags were never enumerated is the same mistake `git branch` made.
const MEDIUM_RISK_GIT_SUBCOMMANDS: &[&str] = &["add", "commit", "merge", "pull", "fetch", "clone"];
const MEDIUM_RISK_PROGRAMS: &[&str] = &["cp", "mv", "mkdir", "touch", "cargo", "npm", "make"];

const HISTORY_REWRITE_GIT_SUBCOMMANDS: &[&str] = &["rebase", "filter-branch", "filter-repo"];
/// `push` has no read-only form. `remote` does (`git remote` /
/// `git remote -v` lists configured remotes); it only mutates when a
/// specific action verb follows, checked separately in
/// `is_remote_mutating`.
const ALWAYS_MUTATING_GIT_SUBCOMMANDS: &[&str] = &["push"];
const REMOTE_MUTATING_ACTIONS: &[&str] = &[
    "add",
    "remove",
    "rm",
    "rename",
    "set-url",
    "set-branches",
    "set-head",
    "prune",
];

/// Classifies a proposed command. `cwd` and `project_root` are used to
/// resolve relative path arguments and to detect indirection that escapes
/// the project root (e.g. `../../etc/passwd`); `state_root` is Tekstide's
/// own state directory, checked separately per the RFC's "writes to the
/// Tekstide state root" rule. All three are plain paths rather than the
/// richer `project::root` types deliberately: this is a pure, headless,
/// fixture-testable function with no filesystem access and no dependency
/// on `ProjectSession` -- the coordinator (PR-021-E) supplies the real
/// canonical values.
pub fn classify(
    argv: &[String],
    cwd: &Path,
    project_root: &Path,
    state_root: &Path,
) -> RiskAssessment {
    let Some(program) = argv.first() else {
        // CommandProposal::decode already rejects empty argv, so this is
        // unreachable from a validated proposal -- but this function must
        // not panic on it, and per the same philosophy as "unclassifiable
        // is High," an argv this module cannot even look at is High.
        return RiskAssessment::high(vec![RiskReason::Unrecognized]);
    };
    let basename = basename_str(program);

    let destructive_reasons = destructive_reasons(argv, &basename);
    if !destructive_reasons.is_empty() {
        return RiskAssessment::destructive(destructive_reasons);
    }

    let high_reasons = high_risk_reasons(argv, cwd, project_root, state_root, &basename);
    if !high_reasons.is_empty() {
        return RiskAssessment::high(high_reasons);
    }

    if is_known_low_risk(argv, &basename) {
        return RiskAssessment::low();
    }
    if is_known_medium_risk(argv, &basename) {
        return RiskAssessment::medium();
    }

    // Not recognized as anything in particular: unclassifiable is High,
    // never Low. This is the fallthrough case, not a rule -- there is no
    // path through this function that reaches `Low` or `Medium` without
    // first matching one of the explicit allowlists above.
    RiskAssessment::high(vec![RiskReason::Unrecognized])
}

fn destructive_reasons(argv: &[String], basename: &str) -> Vec<RiskReason> {
    let mut reasons = Vec::new();

    if (basename == "rm" || basename == "rmdir") && has_recursive_flag(argv) {
        reasons.push(RiskReason::RecursiveDeletion);
    }
    if is_disk_level_program(basename) {
        reasons.push(RiskReason::DiskLevelOperation);
    }
    if basename == "git" {
        let subcommand = argv.get(1).map(String::as_str);
        if subcommand.is_some_and(|sub| HISTORY_REWRITE_GIT_SUBCOMMANDS.contains(&sub)) {
            reasons.push(RiskReason::HistoryRewrite);
        }
        if subcommand == Some("reset") && argv.iter().any(|arg| arg == "--hard") {
            reasons.push(RiskReason::HistoryRewrite);
        }
        if is_working_tree_discard(subcommand, argv) {
            reasons.push(RiskReason::WorkingTreeDiscard);
        }
        if subcommand == Some("push") && has_force_flag(argv) {
            reasons.push(RiskReason::RemoteHistoryRewrite);
        }
    }
    reasons
}

/// `--force`/`-f`/`--force-with-lease`. Not distinguishing
/// `--force-with-lease` (genuinely safer than `--force`, since it refuses
/// to overwrite a remote ref that has moved since the last fetch) is a
/// deliberate simplification, recorded in Known Limitations rather than
/// silently implied to be a finer-grained check than it is.
fn has_force_flag(argv: &[String]) -> bool {
    argv.iter()
        .any(|arg| arg == "--force" || arg == "-f" || arg == "--force-with-lease")
}

fn is_disk_level_program(basename: &str) -> bool {
    DISK_LEVEL_EXACT_PROGRAMS.contains(&basename)
        || basename == "mkfs"
        || basename.starts_with("mkfs.")
}

/// `git remote` / `git remote -v` lists configured remotes read-only;
/// `git remote add/remove/rm/rename/set-url/set-branches/set-head/prune`
/// mutates. The action verb is scanned across `argv[2..]` rather than read
/// at a fixed index (response 111 Required 1: git's synopsis is `git
/// remote [-v | --verbose] <subcommand>`, so a flag before the verb shifts
/// its position and a fixed-index read misses it -- `git remote -v remove
/// origin` classified `Low` under the fixed-index version. The general
/// lesson response 111 drew: moving a program into an allowlist transfers
/// responsibility for *every* argument shape onto the carve-out; a
/// positional check is exactly the kind of shape that transfer can miss).
fn is_remote_mutating(subcommand: Option<&str>, argv: &[String]) -> bool {
    if subcommand != Some("remote") {
        return false;
    }
    argv.iter()
        .skip(2)
        .any(|action| REMOTE_MUTATING_ACTIONS.contains(&action.as_str()))
}

/// `git checkout -- <path>` / `git checkout .` (the unambiguous "what
/// follows is a pathspec, not a ref" syntax; bare `.` cannot be a ref
/// name) or `git checkout -f`/`--force` (explicitly discards local
/// modifications when switching, response 111 non-blocking-1: the third
/// instance of the same shape as `git branch -D` and `git push --force`
/// -- a decidable flag on an otherwise-blanket-allowlisted form): all
/// discard uncommitted working-tree changes, unrecoverably. A bare `git
/// checkout <ref>` (branch switch) is not this -- and whether an
/// arbitrary bare argument is a ref or a path is not structurally
/// decidable, so only these unambiguous forms are caught.
fn is_working_tree_discard(subcommand: Option<&str>, argv: &[String]) -> bool {
    if subcommand != Some("checkout") {
        return false;
    }
    argv.iter()
        .skip(2)
        .any(|arg| arg == "--" || arg == "." || arg == "-f" || arg == "--force")
}

fn has_recursive_flag(argv: &[String]) -> bool {
    argv.iter().skip(1).any(|arg| {
        arg == "--recursive"
            || (arg.starts_with('-') && !arg.starts_with("--") && arg.contains(['r', 'R']))
    })
}

fn high_risk_reasons(
    argv: &[String],
    cwd: &Path,
    project_root: &Path,
    state_root: &Path,
    basename: &str,
) -> Vec<RiskReason> {
    let mut reasons = Vec::new();

    if argv
        .iter()
        .any(|arg| ELEVATION_PROGRAMS.contains(&basename_str(arg).as_str()))
    {
        reasons.push(RiskReason::PrivilegeElevation);
    }
    if SHELL_INTERPRETERS.contains(&basename) {
        reasons.push(RiskReason::OpaqueShellInvocation);
    }
    if OPAQUE_WRAPPER_PROGRAMS.contains(&basename) {
        reasons.push(RiskReason::OpaqueWrapper);
    }
    if basename == "git" {
        let subcommand = argv.get(1).map(String::as_str);
        if subcommand.is_some_and(|sub| ALWAYS_MUTATING_GIT_SUBCOMMANDS.contains(&sub))
            || is_remote_mutating(subcommand, argv)
        {
            reasons.push(RiskReason::GitRemoteMutating);
        }
        if subcommand == Some("tag") && argv.iter().any(|arg| arg == "-d" || arg == "--delete") {
            reasons.push(RiskReason::GitRemoteMutating);
        }
        // `push` with a force flag is handled in `destructive_reasons`
        // (`RiskReason::RemoteHistoryRewrite`, response 111 Required 2) --
        // not repeated here. `destructive_reasons` runs first and returns
        // immediately when non-empty, so a duplicate `High`-level rule
        // here would be unreachable dead code for the force case.
        if subcommand == Some("stash") && is_stash_purge(argv) {
            // response 111 Required 3: a deliberate severity choice
            // (`Medium`/`High` both defensible, per qa-evidence.md) needs
            // a named reason and a fixture, not just "falls through to
            // Unrecognized by not being in the Medium allowlist."
            reasons.push(RiskReason::WorkingTreeDiscard);
        }
    }

    // argv[0] participates in the elevation and secret-pattern scans
    // above and below, but is excluded from the two containment checks
    // just below (project-root, state-root) -- see the module doc's
    // `argv[0]` paragraph for why the asymmetry is principled rather than
    // a compromise.
    for (index, entry) in argv.iter().enumerate() {
        for candidate in path_candidates(entry) {
            let resolved = resolve_path_arg(candidate, cwd);
            if index != 0 {
                if !is_inside(&resolved, project_root) {
                    reasons.push(RiskReason::PathOutsideProjectRoot);
                }
                if is_inside(&resolved, state_root) {
                    reasons.push(RiskReason::TekstideStateRoot);
                }
            }
            if matches_secret_like_pattern(&resolved) {
                reasons.push(RiskReason::SecretLikePath);
            }
        }
    }

    reasons.sort_by_key(reason_sort_key);
    reasons.dedup();
    reasons
}

/// Stable, content-free ordering so `reasons` is deterministic for tests
/// and for a future dialog rendering them -- not a severity ordering (all
/// reasons returned together are already at the same `RiskLevel`).
fn reason_sort_key(reason: &RiskReason) -> u8 {
    match reason {
        RiskReason::PathOutsideProjectRoot => 0,
        RiskReason::PrivilegeElevation => 1,
        RiskReason::OpaqueShellInvocation => 2,
        RiskReason::OpaqueWrapper => 3,
        RiskReason::GitRemoteMutating => 4,
        RiskReason::SecretLikePath => 5,
        RiskReason::TekstideStateRoot => 6,
        RiskReason::RecursiveDeletion => 7,
        RiskReason::DiskLevelOperation => 8,
        RiskReason::HistoryRewrite => 9,
        RiskReason::WorkingTreeDiscard => 10,
        RiskReason::RemoteHistoryRewrite => 11,
        RiskReason::Unrecognized => 12,
    }
}

/// An argv entry contributes itself as a path candidate, plus (if it
/// starts with `-` and contains `=`) the substring after the first `=` --
/// covers the common GNU long-option attached form
/// (`--target-directory=/etc`) without also splitting ordinary free text
/// that happens to contain `=` (response 111 non-blocking-2: `git commit
/// -m "note a=/etc/passwd"` and `echo FOO=/etc/passwd` are not option
/// syntax and must not be treated as one). Attached short options
/// (`-o/etc/x`) are not covered; disclosed as a known limitation rather
/// than chased.
fn path_candidates(entry: &str) -> Vec<&str> {
    if entry.starts_with('-')
        && let Some((_, value)) = entry.split_once('=')
    {
        return vec![entry, value];
    }
    vec![entry]
}

fn is_known_low_risk(argv: &[String], basename: &str) -> bool {
    if basename == "git" {
        let subcommand = argv.get(1).map(String::as_str);
        if subcommand.is_some_and(|sub| LOW_RISK_GIT_SUBCOMMANDS.contains(&sub)) {
            return true;
        }
        // Read-only `git remote` / `git remote -v` (no mutating action
        // verb, already ruled out by `is_remote_mutating` before this
        // function runs) lists configured remotes -- no different in kind
        // from `git status`.
        return subcommand == Some("remote") && !is_remote_mutating(subcommand, argv);
    }
    LOW_RISK_PROGRAMS.contains(&basename)
}

fn is_known_medium_risk(argv: &[String], basename: &str) -> bool {
    if basename == "git" {
        let subcommand = argv.get(1).map(String::as_str);
        if subcommand.is_some_and(|sub| MEDIUM_RISK_GIT_SUBCOMMANDS.contains(&sub)) {
            return true;
        }
        // `checkout` without the unambiguous discard markers is an
        // ordinary branch switch; `stash` other than a purge is ordinary
        // stash bookkeeping. Both were ruled out as Destructive/High
        // already by the time this function runs.
        if subcommand == Some("checkout") && !is_working_tree_discard(subcommand, argv) {
            return true;
        }
        if subcommand == Some("stash") && !is_stash_purge(argv) {
            return true;
        }
        return false;
    }
    MEDIUM_RISK_PROGRAMS.contains(&basename)
}

/// `git stash clear`/`git stash drop`: deliberately purges saved work.
/// Excluded from the ordinary-stash-use `Medium` allowlist by
/// `is_known_medium_risk`, and given its own `High`/`WorkingTreeDiscard`
/// reason in `high_risk_reasons` (response 111 Required 3: the level was
/// already right, but it was landing on `Unrecognized` by accident -- a
/// deliberate decision needs a named reason and a fixture, or it is
/// indistinguishable from an oversight). See qa-evidence.md for the
/// severity reasoning versus the `Destructive` working-tree-discard cases.
fn is_stash_purge(argv: &[String]) -> bool {
    matches!(
        argv.get(2).map(String::as_str),
        Some("clear") | Some("drop")
    )
}

fn basename_str(entry: &str) -> String {
    Path::new(entry)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| entry.to_string())
}

/// Resolves `entry` to an absolute path relative to `cwd` if it is not
/// already absolute, then lexically normalizes `.`/`..` components without
/// touching the filesystem -- no `fs::canonicalize`, since the proposed
/// command may reference a path that does not exist yet (a new output
/// file, a not-yet-created directory), and this classifier must not
/// require the filesystem to agree before it can classify anything.
fn resolve_path_arg(entry: &str, cwd: &Path) -> PathBuf {
    let candidate = Path::new(entry);
    let absolute = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        cwd.join(candidate)
    };
    lexically_normalize(&absolute)
}

fn lexically_normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                result.pop();
            }
            Component::CurDir => {}
            other => result.push(other.as_os_str()),
        }
    }
    result
}

fn is_inside(path: &Path, root: &Path) -> bool {
    path == root || path.strip_prefix(root).is_ok()
}

fn matches_secret_like_pattern(path: &Path) -> bool {
    path.components().any(|component| {
        let Component::Normal(part) = component else {
            return false;
        };
        let part = part.to_string_lossy();
        SECRET_LIKE_EXACT_COMPONENTS.contains(&part.as_ref())
            || SECRET_LIKE_COMPONENT_SUFFIXES
                .iter()
                .any(|suffix| part.ends_with(suffix))
    })
}
