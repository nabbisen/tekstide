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
/// Guards the recursive `.gitattributes` walk against a repository with an
/// enormous directory tree; real repositories never come close.
const MAX_ATTRIBUTE_WALK_ENTRIES: usize = 20_000;

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

    if worktree_names_a_content_driver(repository_root) {
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

/// D1' item 6: a repository whose attributes name a `filter=`/`diff=`
/// driver gets `AcceptedBranchOnly` even when the driver itself is
/// undefined (measured: undefined-driver attributes execute nothing, but
/// the comparison would still be against an index written through a
/// filter this gate never ran).
fn worktree_names_a_content_driver(repository_root: &Path) -> bool {
    let mut candidates = vec![
        repository_root.join(".gitattributes"),
        repository_root.join(".git").join("info").join("attributes"),
    ];
    let mut budget = MAX_ATTRIBUTE_WALK_ENTRIES;
    collect_nested_gitattributes(repository_root, &mut candidates, &mut budget);
    candidates
        .iter()
        .any(|path| file_declares_content_driver(path))
}

fn collect_nested_gitattributes(dir: &Path, out: &mut Vec<PathBuf>, budget: &mut usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        if entry.file_name() == ".git" {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            collect_nested_gitattributes(&path, out, budget);
        } else if entry.file_name() == ".gitattributes" {
            out.push(path);
        }
    }
}

fn file_declares_content_driver(path: &Path) -> bool {
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
