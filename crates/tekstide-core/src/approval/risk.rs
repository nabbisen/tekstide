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
//! doing something it cannot positively vouch for -- falls through to
//! `High` by construction, not by an added "else" branch that could be
//! forgotten.
//!
//! **A gap recorded here rather than silently left for the reviewer to
//! find:** wrapper indirection. `env sudo rm -rf /` is caught (the
//! elevation check scans every argv entry's basename, not just argv[0]),
//! and `sh -c '...'` is caught (a shell interpreter with `-c` is High on
//! its own, since its real command is opaque to structural inspection).
//! But an arbitrary wrapper -- `env`, `nice`, `timeout`, `xargs` -- running
//! `git push` is not unwrapped to recognize the `git` invocation
//! underneath. Full wrapper-unwrapping is a much larger problem than this
//! slice's scope; this is disclosed as a known limitation, not fixed here.
//!
//! **Another gap, about the handoff's own premise:** `implementation-
//! handoff.md` §4 says to escalate on "paths matching the secret-like
//! patterns already defined for environment redaction." Searched the
//! codebase (`grep` for `secret`, `redact`, common credential-variable
//! names) and found no such pattern set already implemented -- RFC-004
//! states the *policy* ("Tekstide may redact known secret-like environment
//! variable values...") but no concrete pattern list exists in code. The
//! list below (`SECRET_LIKE_PATH_PATTERNS`) is therefore newly written for
//! this slice, not reused, and is flagged in the review request as a
//! possible RFC-004 implementation gap rather than an RFC-021-local
//! question.

use std::path::{Component, Path, PathBuf};

use crate::domain::RiskLevel;

/// Programs whose presence anywhere in argv means "the real command is
/// opaque to structural inspection" or "this is asking for elevated
/// privilege" -- checked by basename so `/usr/bin/sudo` is caught the same
/// as `sudo`.
const ELEVATION_PROGRAMS: &[&str] = &["sudo", "doas", "pkexec"];
const SHELL_INTERPRETERS: &[&str] = &["sh", "bash", "zsh", "dash", "ksh", "fish"];
const DISK_LEVEL_PROGRAMS: &[&str] =
    &["dd", "mkfs", "fdisk", "sfdisk", "parted", "wipefs", "shred"];

/// Substrings checked against every resolved path argument. Deliberately
/// narrow and documented (see module doc) rather than presented as a
/// complete or authoritative list.
const SECRET_LIKE_PATH_PATTERNS: &[&str] = &[
    ".ssh",
    ".gnupg",
    ".netrc",
    ".pgp",
    "id_rsa",
    "id_ed25519",
    "id_ecdsa",
    ".pem",
    ".git-credentials",
    "credentials",
    ".aws",
    ".docker/config.json",
];

const LOW_RISK_GIT_SUBCOMMANDS: &[&str] = &["status", "log", "diff", "show", "branch"];
const LOW_RISK_PROGRAMS: &[&str] = &[
    "ls", "pwd", "echo", "cat", "head", "tail", "wc", "printf", "whoami", "date",
];

const MEDIUM_RISK_GIT_SUBCOMMANDS: &[&str] = &[
    "add", "commit", "checkout", "merge", "pull", "fetch", "clone", "stash",
];
const MEDIUM_RISK_PROGRAMS: &[&str] = &["cp", "mv", "mkdir", "touch", "cargo", "npm", "make"];

const HISTORY_REWRITE_GIT_SUBCOMMANDS: &[&str] = &["rebase", "filter-branch", "filter-repo"];
const REMOTE_MUTATING_GIT_SUBCOMMANDS: &[&str] = &["push", "remote"];

/// Classifies a proposed command. `cwd` and `project_root` are used to
/// resolve relative path arguments and to detect indirection that escapes
/// the project root (e.g. `../../etc/passwd`); `state_root` is Tekstide's
/// own state directory, checked separately per the RFC's "writes to the
/// Tekstide state root" rule. All three are plain paths rather than the
/// richer `project::root` types deliberately: this is a pure, headless,
/// fixture-testable function with no filesystem access and no dependency
/// on `ProjectSession` -- the coordinator (PR-021-E) supplies the real
/// canonical values.
pub fn classify(argv: &[String], cwd: &Path, project_root: &Path, state_root: &Path) -> RiskLevel {
    let Some(program) = argv.first() else {
        // CommandProposal::decode already rejects empty argv, so this is
        // unreachable from a validated proposal -- but this function must
        // not panic on it, and per the same philosophy as "unclassifiable
        // is High," an argv this module cannot even look at is High.
        return RiskLevel::High;
    };
    let basename = basename_str(program);

    if is_destructive(argv, &basename) {
        return RiskLevel::Destructive;
    }
    if is_high_risk(argv, cwd, project_root, state_root, &basename) {
        return RiskLevel::High;
    }
    if is_known_low_risk(argv, &basename) {
        return RiskLevel::Low;
    }
    if is_known_medium_risk(argv, &basename) {
        return RiskLevel::Medium;
    }

    // Not recognized as anything in particular: unclassifiable is High,
    // never Low. This is the fallthrough case, not a rule -- there is no
    // path through this function that reaches `Low` or `Medium` without
    // first matching one of the explicit allowlists above.
    RiskLevel::High
}

fn is_destructive(argv: &[String], basename: &str) -> bool {
    if (basename == "rm" || basename == "rmdir") && has_recursive_flag(argv) {
        return true;
    }
    if DISK_LEVEL_PROGRAMS
        .iter()
        .any(|program| basename == *program || basename.starts_with(program))
    {
        return true;
    }
    if basename == "git" {
        let subcommand = argv.get(1).map(String::as_str);
        if subcommand.is_some_and(|sub| HISTORY_REWRITE_GIT_SUBCOMMANDS.contains(&sub)) {
            return true;
        }
        if subcommand == Some("reset") && argv.iter().any(|arg| arg == "--hard") {
            return true;
        }
    }
    false
}

fn has_recursive_flag(argv: &[String]) -> bool {
    argv.iter().skip(1).any(|arg| {
        arg == "--recursive"
            || (arg.starts_with('-') && !arg.starts_with("--") && arg.contains(['r', 'R']))
    })
}

fn is_high_risk(
    argv: &[String],
    cwd: &Path,
    project_root: &Path,
    state_root: &Path,
    basename: &str,
) -> bool {
    if ELEVATION_PROGRAMS
        .iter()
        .any(|elevated| argv.iter().any(|arg| basename_str(arg) == *elevated))
    {
        return true;
    }
    if SHELL_INTERPRETERS.contains(&basename) && argv.iter().any(|arg| arg == "-c") {
        return true;
    }
    if basename == "git" {
        let subcommand = argv.get(1).map(String::as_str);
        if subcommand.is_some_and(|sub| REMOTE_MUTATING_GIT_SUBCOMMANDS.contains(&sub)) {
            return true;
        }
        if subcommand == Some("tag") && argv.iter().any(|arg| arg == "-d" || arg == "--delete") {
            return true;
        }
        if argv
            .iter()
            .any(|arg| arg == "--force" || arg == "-f" || arg == "--force-with-lease")
        {
            return true;
        }
    }
    for entry in argv {
        let resolved = resolve_path_arg(entry, cwd);
        if !is_inside(&resolved, project_root) {
            return true;
        }
        if is_inside(&resolved, state_root) {
            return true;
        }
        let resolved_str = resolved.to_string_lossy();
        if SECRET_LIKE_PATH_PATTERNS
            .iter()
            .any(|pattern| resolved_str.contains(pattern) || entry.contains(pattern))
        {
            return true;
        }
    }
    false
}

fn is_known_low_risk(argv: &[String], basename: &str) -> bool {
    if basename == "git" {
        return argv
            .get(1)
            .is_some_and(|sub| LOW_RISK_GIT_SUBCOMMANDS.contains(&sub.as_str()));
    }
    LOW_RISK_PROGRAMS.contains(&basename)
}

fn is_known_medium_risk(argv: &[String], basename: &str) -> bool {
    if basename == "git" {
        return argv
            .get(1)
            .is_some_and(|sub| MEDIUM_RISK_GIT_SUBCOMMANDS.contains(&sub.as_str()));
    }
    MEDIUM_RISK_PROGRAMS.contains(&basename)
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
