//! RFC-030 D1', decided at review 405: neither a pure-Rust Git library nor
//! a bare `git` subprocess is safe against a repository's own
//! configuration by default -- both run a repository-named clean filter,
//! and `git` additionally honours `core.fsmonitor`. The question was never
//! *which mechanism*; it is **which repositories we agree to read at
//! all**. This module is that gate: before any worktree read, it reads the
//! repository's effective configuration and refuses unless every key is on
//! an allowlist of keys that cannot name a program, then separately
//! refuses the *content* answer (not the read itself) for a repository
//! whose attributes name a filter/diff driver, since comparing against an
//! index written through a filter we did not run produces a wrong count
//! (every Git LFS file reading as modified), not a safety problem.
//!
//! Nothing in this module has a production caller yet. PR-030-B wires
//! [`evaluate`] into `ProjectSession::set_git_summary`; PR-030-A is the
//! gate and its adversarial fixture only.
//!
//! **Review 406 found a repository this gate accepted that still ran a
//! program the repository named.** The general defect: the gate assumed
//! the configuration it read was the configuration `git` would use, which
//! is false wherever `git` consults *another* repository's configuration
//! -- a submodule's gitdir is untouched by anything read here, yet
//! `git status` in the parent consults it. [`worktree_names_a_content_driver`]'s
//! doc comment covers the fix (R1) and three narrower fail-open bugs found
//! alongside it in the attributes walk (R2 budget exhaustion, R3 symlink
//! following, R4 `.git`-as-pointer-file) and the attributes read (R5,
//! unbounded).
//!
//! RFC-012's *Git Detector Safety* gate, item by item:
//!
//! 1. **Reviewed non-project-local executable** -- [`GIT_EXECUTABLE`] is a
//!    bare name resolved only against the fixed `PATH` set below, never a
//!    path influenced by the repository being read.
//! 2. **Invoked directly, no shell** -- [`spawn_git_command`] builds a
//!    `std::process::Command` argument vector; nothing is ever passed to
//!    `sh -c`.
//! 3. **Deterministic argument vector, not aliases** -- every call site
//!    passes a fixed argv (`["config", "--list", "--null"]`,
//!    `["--version"]`); nothing here ever invokes a bare subcommand name
//!    that a repository's `alias.*` could have redefined, and `alias.*`
//!    keys are refused by the allowlist regardless (they are not on it).
//! 4. **No project-local `PATH`** -- `PATH` is fixed to `/usr/bin:/bin`
//!    after `.env_clear()`, never the inherited or repository-influenced
//!    value.
//! 5. **Sanitised environment** -- `.env_clear()` first; only `PATH` and
//!    the locale pair are fixed, plus `HOME`/`GIT_CONFIG_GLOBAL`/
//!    `GIT_CONFIG_SYSTEM`/`XDG_CONFIG_HOME` forwarded *from Tekstide's own
//!    process environment*, never from anything the project could set --
//!    this is also what lets the D7 fixture point "global" and "system"
//!    config at itself, by setting those same variables before calling
//!    this module, without the gate needing to know it is under test.
//! 6. **No workspace hooks or config-driven automation** -- this is the
//!    gate's entire purpose: an unknown configuration key refuses the
//!    repository outright, before any command that could act on it runs.
//! 7. **Bounded execution time and output** -- [`run_bounded`] enforces
//!    [`SUBPROCESS_TIMEOUT`] and [`MAX_OUTPUT_BYTES`] on every call.
//! 8. **Bounded diagnostics** -- refusal reasons carry only a
//!    configuration *key* name (a fixed git vocabulary word) or a version
//!    string, never file contents, diff output, or captured stderr text.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const GIT_EXECUTABLE: &str = "git";
const SUBPROCESS_TIMEOUT: Duration = Duration::from_secs(5);
/// Config listings and `--version` output are small; this bounds a
/// hostile repository's include chain or a runaway filter's stdout, not
/// real use.
const MAX_OUTPUT_BYTES: usize = 1 << 20;
const MIN_GIT_VERSION: (u32, u32, u32) = (2, 30, 0);
/// Guards the recursive `.gitattributes` walk against a pathologically
/// large or deep repository. Review 406 measured this crate's own
/// workspace at 183,736 entries against the previous 20,000 cap -- this
/// project's own repository would always have hit R2's fail-closed path.
/// Raised with headroom over that; still bounded, and exhaustion still
/// fails closed (see `worktree_names_a_content_driver`) rather than
/// silently reporting "no driver found" on a truncated scan.
const MAX_ATTRIBUTE_WALK_ENTRIES: usize = 1_000_000;
/// A real `.gitattributes` file is a handful of lines. Anything past this
/// is either not a real attributes file or is deliberately trying to make
/// this gate spend unbounded time/memory on it -- either way, R5's answer
/// is "cannot fully vet it", which fails closed the same as an oversized
/// walk.
const MAX_ATTRIBUTES_FILE_BYTES: u64 = 1 << 20;

/// The result of the gate, for one repository root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GitGateOutcome {
    /// The repository's effective configuration named nothing outside the
    /// allowlist, and its attributes name no content-filter driver: branch,
    /// dirty state and per-file status may all be read.
    Accepted,
    /// Configuration is safe, but attributes name a `filter=`/`diff=`
    /// driver (D1' item 6). Branch is still safe (D1' item 8); dirty state
    /// and per-file status are not -- they would compare worktree bytes
    /// against an index written through a filter this gate never ran.
    AcceptedBranchOnly,
    /// Refused before any worktree read.
    Refused(GitGateRefusal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GitGateRefusal {
    Unavailable(GitUnavailableReason),
    /// A configuration key exists that is not on the allowlist of keys
    /// known to be unable to name a program.
    UnknownConfigKey {
        key: String,
    },
    /// `include.path` or an `includeIf.<condition>.path` key -- refused
    /// unconditionally (D1' item 3): what it pulls in is itself
    /// unreviewed, and `--local` hiding an include's *contents* while
    /// `git status` still runs them is exactly the bypass this measured.
    ConfigInclude {
        key: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GitUnavailableReason {
    NotFound,
    VersionTooOld { found: String },
    VersionUnparseable { found: String },
    SpawnFailed,
    TimedOut,
    OutputNotUtf8,
}

/// The environment variables forwarded into the `git` subprocess from
/// Tekstide's own process environment (see the module doc comment, item
/// 5). Read once via [`forwarded_environment`] and threaded explicitly
/// rather than read again per spawn, so tests can supply a fixture-only
/// substitute without mutating the real process environment --
/// `std::env::set_var` is process-global and races other tests running in
/// the same binary.
const FORWARDED_ENV_VARS: &[&str] = &[
    "HOME",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
    "XDG_CONFIG_HOME",
];

fn forwarded_environment() -> Vec<(String, String)> {
    FORWARDED_ENV_VARS
        .iter()
        .filter_map(|var| {
            std::env::var(var)
                .ok()
                .map(|value| (var.to_string(), value))
        })
        .collect()
}

/// The gate. Trust state is deliberately not a parameter: D1' item 7 is
/// that Tekstide never runs a program a repository names, in any trust
/// state -- detection is safe in Restricted projects because this
/// guarantee holds, not because a grant permits it.
pub fn evaluate(repository_root: &Path) -> GitGateOutcome {
    evaluate_with_environment(repository_root, GIT_EXECUTABLE, &forwarded_environment())
}

/// `git_executable` is a plain constant in production
/// ([`evaluate`]); tests substitute a name that cannot resolve, to exercise
/// [`GitUnavailableReason::NotFound`] without touching the real `PATH`.
fn evaluate_with_environment(
    repository_root: &Path,
    git_executable: &str,
    forwarded_env: &[(String, String)],
) -> GitGateOutcome {
    evaluate_with_environment_and_walk_budget(
        repository_root,
        git_executable,
        forwarded_env,
        MAX_ATTRIBUTE_WALK_ENTRIES,
    )
}

/// `walk_budget` is [`MAX_ATTRIBUTE_WALK_ENTRIES`] in production; R2's
/// test substitutes a small one, since actually creating a
/// million-entry fixture to prove exhaustion fails closed would be
/// impractical.
fn evaluate_with_environment_and_walk_budget(
    repository_root: &Path,
    git_executable: &str,
    forwarded_env: &[(String, String)],
    walk_budget: usize,
) -> GitGateOutcome {
    if let Err(reason) = check_git_available(git_executable, forwarded_env) {
        return GitGateOutcome::Refused(GitGateRefusal::Unavailable(reason));
    }

    let config_output = match run_bounded_git(
        git_executable,
        &["config", "--list", "--null"],
        Some(repository_root),
        forwarded_env,
    ) {
        Ok(output) if output.status.success() => output,
        Ok(_) => {
            return GitGateOutcome::Refused(GitGateRefusal::Unavailable(
                GitUnavailableReason::SpawnFailed,
            ));
        }
        Err(reason) => return GitGateOutcome::Refused(GitGateRefusal::Unavailable(reason)),
    };
    let Some(entries) = parse_null_separated_config(&config_output.stdout) else {
        return GitGateOutcome::Refused(GitGateRefusal::Unavailable(
            GitUnavailableReason::OutputNotUtf8,
        ));
    };

    for (key, _value) in &entries {
        let lower = key.to_ascii_lowercase();
        if lower == "include.path" || lower.starts_with("includeif.") {
            return GitGateOutcome::Refused(GitGateRefusal::ConfigInclude { key: key.clone() });
        }
        if !config_key_is_allowed(key) {
            return GitGateOutcome::Refused(GitGateRefusal::UnknownConfigKey { key: key.clone() });
        }
    }

    if worktree_names_a_content_driver(repository_root, git_executable, forwarded_env, walk_budget)
    {
        GitGateOutcome::AcceptedBranchOnly
    } else {
        GitGateOutcome::Accepted
    }
}

/// Keys known to be pure data -- a string, a boolean, a reference name --
/// never a program name, and never used by `config --list`, `--version`,
/// or (PR-030-B) a branch/dirty read. Deliberately small: an unrecognised
/// key refuses the repository rather than being guessed safe.
const EXACT_ALLOWED_CONFIG_KEYS: &[&str] = &[
    "core.repositoryformatversion",
    "core.filemode",
    "core.bare",
    "core.logallrefupdates",
    "core.ignorecase",
    "core.precomposeunicode",
    "core.symlinks",
    "core.autocrlf",
    "core.safecrlf",
    "core.eol",
    "core.quotepath",
    "core.commentchar",
    "core.compression",
    "core.trustctime",
    "core.protecthfs",
    "core.protectntfs",
    "init.defaultbranch",
    "user.name",
    "user.email",
    "status.showuntrackedfiles",
    "status.branch",
    "color.ui",
    "color.status",
    "color.branch",
    "color.diff",
    "push.default",
    "push.autosetupremote",
    "pull.rebase",
    "pull.ff",
    "fetch.prune",
    "branch.autosetupmerge",
    "branch.autosetuprebase",
    "diff.algorithm",
    "diff.renames",
    "diff.mnemonicprefix",
    "merge.ff",
    "rebase.autostash",
    "submodule.recurse",
];

/// `(prefix, suffix)` pairs matching a key with a dynamic subsection --
/// `remote.<name>.url`, `branch.<name>.remote`, and so on. Matched by
/// prefix/suffix on the whole key, not by splitting on `.`, because a
/// subsection name may itself legally contain dots; every pair here is
/// data used only by network/rebase operations this gate never performs,
/// so an adversarial subsection name changes nothing about what is safe.
const SUBSECTION_ALLOWED_PATTERNS: &[(&str, &str)] = &[
    ("remote.", ".url"),
    ("remote.", ".pushurl"),
    ("remote.", ".fetch"),
    ("remote.", ".push"),
    ("remote.", ".tagopt"),
    ("remote.", ".mirror"),
    ("remote.", ".prune"),
    ("branch.", ".remote"),
    ("branch.", ".merge"),
    ("branch.", ".rebase"),
    ("branch.", ".description"),
];

fn config_key_is_allowed(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    if EXACT_ALLOWED_CONFIG_KEYS.contains(&lower.as_str()) {
        return true;
    }
    SUBSECTION_ALLOWED_PATTERNS.iter().any(|(prefix, suffix)| {
        lower.len() > prefix.len() + suffix.len()
            && lower.starts_with(prefix)
            && lower.ends_with(suffix)
    })
}

fn parse_null_separated_config(bytes: &[u8]) -> Option<Vec<(String, String)>> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut entries = Vec::new();
    for record in text.split('\0') {
        if record.is_empty() {
            continue;
        }
        match record.split_once('\n') {
            Some((key, value)) => entries.push((key.to_string(), value.to_string())),
            None => entries.push((record.to_string(), String::new())),
        }
    }
    Some(entries)
}

/// Whether the *content* answer (dirty state, per-file status) must be
/// withheld for this repository, folding together every reason review 406
/// found one of: D1' item 6 (attributes name a filter/diff driver, defined
/// or not), R1 (a gitlink -- the linked submodule has its **own**
/// configuration this gate never reads, and `git status` in the parent
/// consults it), R2 (the attributes walk could not finish within budget --
/// on an incompletely-scanned repository, "no driver found" is a guess,
/// not a fact), and R4 (the real gitdir could not be resolved, so
/// `info/attributes` cannot be located reliably). Every one of these
/// collapses to the same outcome because none of them says the
/// *repository* is unsafe -- only that comparing worktree content against
/// the index is not something this gate can vouch for. Branch remains
/// separable (D1' item 8; PR-030-B measures it).
fn worktree_names_a_content_driver(
    repository_root: &Path,
    git_executable: &str,
    forwarded_env: &[(String, String)],
    walk_budget: usize,
) -> bool {
    if repository_contains_a_gitlink(repository_root, git_executable, forwarded_env) {
        return true;
    }

    let mut candidates = vec![repository_root.join(".gitattributes")];
    match resolve_git_common_dir(repository_root, git_executable, forwarded_env) {
        Some(git_dir) => candidates.push(git_dir.join("info").join("attributes")),
        None => return true,
    }

    let mut budget = walk_budget;
    let walk_completed =
        collect_nested_gitattributes(repository_root, &mut candidates, &mut budget);
    if !walk_completed {
        return true;
    }

    candidates
        .iter()
        .any(|path| file_declares_content_driver(path))
}

/// R1: a `160000`-mode index entry is a gitlink -- a submodule, whose own
/// `.git`/config and attributes this gate has not read at all. Measured
/// (review 406) to execute nothing even in a poisoned repository, safe to
/// run unconditionally before deciding anything about content.
fn repository_contains_a_gitlink(
    repository_root: &Path,
    git_executable: &str,
    forwarded_env: &[(String, String)],
) -> bool {
    match run_bounded_git(
        git_executable,
        &["ls-files", "-s"],
        Some(repository_root),
        forwarded_env,
    ) {
        Ok(output) if output.status.success() => match std::str::from_utf8(&output.stdout) {
            Ok(text) => text.lines().any(|line| line.starts_with("160000 ")),
            // Cannot parse the listing: fail closed rather than assume clean.
            Err(_) => true,
        },
        // Cannot determine one way or the other: fail closed.
        _ => true,
    }
}

/// R4: `.git` is not always a directory -- a linked worktree or a
/// submodule checkout leaves a *pointer file* there instead, and
/// `repository_root.join(".git").join("info").join("attributes")` then
/// resolves to nothing, silently falling back to the permissive answer.
///
/// `--git-common-dir`, not `--git-dir`: for a linked worktree, `--git-dir`
/// resolves to that worktree's own *private* metadata directory (under the
/// primary checkout's `.git/worktrees/<name>/`), which has no `info/` of
/// its own -- `info/attributes` is shared across every worktree and lives
/// only under the common dir. Using `--git-dir` here would look in the
/// wrong place and silently fall back to the permissive answer for the
/// exact repository shape R4 exists to cover, found while building this
/// slice's own linked-worktree test. For a repository that is not a
/// linked worktree, `--git-common-dir` and `--git-dir` agree (`.git`).
/// Same safety class as `--git-dir` (measured, review 406: executes
/// nothing even in a poisoned repository) -- both are pure path
/// resolution, no content read.
fn resolve_git_common_dir(
    repository_root: &Path,
    git_executable: &str,
    forwarded_env: &[(String, String)],
) -> Option<PathBuf> {
    let output = run_bounded_git(
        git_executable,
        &["rev-parse", "--git-common-dir"],
        Some(repository_root),
        forwarded_env,
    )
    .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = std::str::from_utf8(&output.stdout).ok()?;
    let raw = text.trim();
    if raw.is_empty() {
        return None;
    }
    let resolved = Path::new(raw);
    Some(if resolved.is_absolute() {
        resolved.to_path_buf()
    } else {
        repository_root.join(resolved)
    })
}

/// Returns `false` (R2) if the walk's budget ran out before it could
/// finish -- the caller must then treat the scan as inconclusive, not as
/// "found nothing". R3: `DirEntry::file_type()` reports the entry's own
/// type without following a symlink (unlike `Path::is_dir()`, which the
/// previous version of this function used and which does follow one), and
/// a symlink of either kind is skipped outright -- the same discipline
/// RFC-050's loader uses, for the same reason: a symlink can point outside
/// the repository entirely.
fn collect_nested_gitattributes(dir: &Path, out: &mut Vec<PathBuf>, budget: &mut usize) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return true;
    };
    for entry in entries.flatten() {
        if *budget == 0 {
            return false;
        }
        *budget -= 1;
        if entry.file_name() == ".git" {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if !collect_nested_gitattributes(&entry.path(), out, budget) {
                return false;
            }
        } else if file_type.is_file() && entry.file_name() == ".gitattributes" {
            out.push(entry.path());
        }
    }
    true
}

/// R5: `metadata.len()` is checked before any read. An oversized file
/// fails closed (`true`, "cannot fully vet it") rather than being read in
/// full -- unbounded, on a file a hostile repository controls, for up to
/// the walk's whole budget of candidates. `symlink_metadata` (not
/// `metadata`) so a `.gitattributes` that is itself a symlink is never
/// followed either (R3), matching `collect_nested_gitattributes`'s own
/// discipline for the two fixed candidates this function also receives
/// (`.gitattributes` at the root, `info/attributes` in the resolved
/// gitdir) which are never passed through that walk.
///
/// Disclosed together with `read_bounded`'s subprocess-pipe read under
/// `runtime/git.rs` in `FILES_ALLOWED_TO_READ_FULL_FILE_CONTENT`
/// (`project/diff/tests.rs`) -- naming both reads that entry exempts,
/// not only the one RFC-024's scan happens to pattern-match.
fn file_declares_content_driver(path: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    if metadata.len() > MAX_ATTRIBUTES_FILE_BYTES {
        return true;
    }
    let Ok(contents) = std::fs::read_to_string(path) else {
        return false;
    };
    contents.lines().any(line_declares_content_driver)
}

fn line_declares_content_driver(line: &str) -> bool {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return false;
    }
    line.split_whitespace().skip(1).any(|attribute| {
        attribute
            .strip_prefix("filter=")
            .is_some_and(|name| !name.is_empty())
            || attribute
                .strip_prefix("diff=")
                .is_some_and(|name| !name.is_empty())
    })
}

fn check_git_available(
    git_executable: &str,
    forwarded_env: &[(String, String)],
) -> Result<(), GitUnavailableReason> {
    let output = run_bounded_git(git_executable, &["--version"], None, forwarded_env)?;
    if !output.status.success() {
        return Err(GitUnavailableReason::SpawnFailed);
    }
    let text = String::from_utf8(output.stdout).map_err(|_| GitUnavailableReason::OutputNotUtf8)?;
    let Some(version) = parse_git_version(&text) else {
        return Err(GitUnavailableReason::VersionUnparseable {
            found: text.trim().to_string(),
        });
    };
    if version < MIN_GIT_VERSION {
        return Err(GitUnavailableReason::VersionTooOld {
            found: text.trim().to_string(),
        });
    }
    Ok(())
}

fn parse_git_version(text: &str) -> Option<(u32, u32, u32)> {
    let rest = text.trim().strip_prefix("git version ")?;
    let mut parts = rest
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty());
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    Some((major, minor, patch))
}

struct BoundedOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
}

fn run_bounded_git(
    git_executable: &str,
    args: &[&str],
    cwd: Option<&Path>,
    forwarded_env: &[(String, String)],
) -> Result<BoundedOutput, GitUnavailableReason> {
    run_bounded(
        spawn_git_command(git_executable, args, cwd, forwarded_env),
        SUBPROCESS_TIMEOUT,
        MAX_OUTPUT_BYTES,
    )
}

/// Item 1, 2, 3, 4 and 5 of RFC-012's gate (see the module doc comment):
/// a bare non-project-local executable name against a fixed `PATH`, a
/// deterministic argv, no shell, and a cleared environment carrying
/// forward only the locale pair plus whichever of
/// `HOME`/`GIT_CONFIG_GLOBAL`/`GIT_CONFIG_SYSTEM`/`XDG_CONFIG_HOME`
/// Tekstide's own process environment set -- the mechanism D7's fixture
/// uses to point "global" and "system" config at itself, passed in by the
/// caller rather than read here so tests never mutate the real process
/// environment.
fn spawn_git_command(
    git_executable: &str,
    args: &[&str],
    cwd: Option<&Path>,
    forwarded_env: &[(String, String)],
) -> Command {
    let mut command = Command::new(git_executable);
    command.args(args);
    command.env_clear();
    command.env("PATH", "/usr/bin:/bin");
    command.env("LANG", "C.UTF-8");
    command.env("LC_ALL", "C.UTF-8");
    for (var, value) in forwarded_env {
        command.env(var, value);
    }
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

enum WaitOutcome {
    Exited(ExitStatus),
    TimedOut,
    WaitFailed,
}

/// Item 7: bounds both wall-clock time and captured bytes on every call,
/// regardless of what the repository or the program it names does.
fn run_bounded(
    mut command: Command,
    timeout: Duration,
    max_bytes: usize,
) -> Result<BoundedOutput, GitUnavailableReason> {
    let mut child = command
        .spawn()
        .map_err(|_| GitUnavailableReason::NotFound)?;
    let mut stdout_pipe = child
        .stdout
        .take()
        .expect("stdout is piped by spawn_git_command");
    let mut stderr_pipe = child
        .stderr
        .take()
        .expect("stderr is piped by spawn_git_command");
    let stdout_reader = thread::spawn(move || read_bounded(&mut stdout_pipe, max_bytes));
    let stderr_reader = thread::spawn(move || read_bounded(&mut stderr_pipe, max_bytes));

    let deadline = Instant::now() + timeout;
    let outcome = loop {
        match child.try_wait() {
            Ok(Some(status)) => break WaitOutcome::Exited(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break WaitOutcome::TimedOut;
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break WaitOutcome::WaitFailed,
        }
    };

    let stdout = stdout_reader.join().unwrap_or_default();
    let _stderr = stderr_reader.join().unwrap_or_default();

    match outcome {
        WaitOutcome::Exited(status) => Ok(BoundedOutput { status, stdout }),
        WaitOutcome::TimedOut => Err(GitUnavailableReason::TimedOut),
        WaitOutcome::WaitFailed => Err(GitUnavailableReason::SpawnFailed),
    }
}

fn read_bounded(reader: &mut impl std::io::Read, max_bytes: usize) -> Vec<u8> {
    let mut buffer = Vec::new();
    let mut limited = reader.take(max_bytes as u64);
    let _ = limited.read_to_end(&mut buffer);
    buffer
}

#[cfg(test)]
mod tests;
