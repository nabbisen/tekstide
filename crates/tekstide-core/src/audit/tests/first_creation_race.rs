//! RFC-059 PR-059-A, D6: reproduces, then regression-guards, the race
//! `rfcs/future-work.md` records (found at RFC-058 review 515): two real
//! processes both calling `AuditStore::open` against a brand-new path, with
//! neither having created anything yet. `two_live_writers.rs`'s own doc
//! comment explicitly disclaims this race -- it pre-establishes the schema
//! before its own threads race, by design, so it proves writers against an
//! *established* store, not two processes racing to *create* one. This is
//! that other case, with real OS processes rather than threads, because
//! threads inside one process share a `database_file().exists()` call in a
//! way two processes genuinely do not.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::runtime::terminal::tests::CleanupOnDrop;
use crate::test_support::KillOnDropChild;

/// `CARGO_BIN_EXE_<name>` is only guaranteed for genuine integration test
/// targets, not a lib's own `#[cfg(test)]` unit tests -- the same fallback
/// `app::tests::project_process_probe_binary_path` already documents and
/// uses for the sibling `[[bin]]` RFC-058 built for the identical reason.
fn audit_store_create_probe_binary_path() -> PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_audit_store_create_probe") {
        return PathBuf::from(path);
    }
    let test_exe = std::env::current_exe().expect("current_exe should resolve for a running test");
    let profile_dir = test_exe
        .parent()
        .and_then(Path::parent)
        .expect("test binary should live under target/<profile>/deps");
    let candidate = profile_dir.join("audit_store_create_probe");
    assert!(
        candidate.is_file(),
        "expected the audit_store_create_probe binary at {}; the [[bin]] target may not have \
         built",
        candidate.display()
    );
    candidate
}

/// Review 527 Required 2: `_base` is a drop guard (`crate::runtime::terminal::tests::
/// CleanupOnDrop`, made `pub(crate)` for exactly this reuse -- the same
/// crate this test lives in, unlike `rfc_docs_invariants.rs`'s own local
/// copy at review 524 Required 3, which needed one because that type is in
/// a different crate entirely). A failing run of this test used to leave
/// its fixture directory behind -- the trailing `remove_dir_all` call was
/// skipped by any `assert_eq!` failure above it -- and measurably did: six
/// failing pre-fix runs left six directories in `TMPDIR`. Dropping, not a
/// call at the end of the test body, runs during a panic's own unwind too.
struct RaceDirs {
    _base: CleanupOnDrop,
    state_root: PathBuf,
    project_root: PathBuf,
}

fn fresh_dirs(label: &str) -> RaceDirs {
    let base = std::env::temp_dir().join(format!(
        "tekstide-audit-race-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let state_root = base.join("state");
    let project_root = base.join("project");
    std::fs::create_dir_all(&state_root).unwrap();
    std::fs::create_dir_all(&project_root).unwrap();
    RaceDirs {
        _base: CleanupOnDrop(base),
        state_root,
        project_root,
    }
}

struct ReadyProbe {
    _child: KillOnDropChild,
    stdin: std::process::ChildStdin,
    stdout: std::io::BufReader<std::process::ChildStdout>,
}

/// Spawns the probe and blocks until it has printed `READY` -- confirmed by
/// reading that exact line back, not inferred from "spawned without
/// waiting." Does **not** release it yet.
fn spawn_ready(state_root: &Path, project_root: &Path) -> ReadyProbe {
    let mut child = Command::new(audit_store_create_probe_binary_path())
        .arg(state_root)
        .arg(project_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawning the probe must succeed");
    let stdin = child.stdin.take().expect("piped stdin");
    let stdout = std::io::BufReader::new(child.stdout.take().expect("piped stdout"));
    let mut probe = ReadyProbe {
        _child: KillOnDropChild::new(child),
        stdin,
        stdout,
    };
    let mut line = String::new();
    probe
        .stdout
        .read_line(&mut line)
        .expect("reading READY must succeed");
    assert_eq!(
        line.trim_end(),
        "READY",
        "the probe must announce readiness before racing"
    );
    probe
}

/// Releases both probes as close together as this process can manage --
/// two separate `write_all` calls, back to back, with no work between them
/// -- then reads each result line. This is the RFC-058 review 513 lesson
/// this RFC's own task breakdown names explicitly: two sequential processes
/// prove nothing, so the harness must hold both at the gate and open it for
/// both at once, not start one and wait for it before starting the other.
fn release_both(mut a: ReadyProbe, mut b: ReadyProbe) -> (String, String) {
    a.stdin.write_all(b"go\n").expect("releasing probe a");
    b.stdin.write_all(b"go\n").expect("releasing probe b");

    let mut result_a = String::new();
    a.stdout
        .read_line(&mut result_a)
        .expect("reading probe a's result must succeed");
    let mut result_b = String::new();
    b.stdout
        .read_line(&mut result_b)
        .expect("reading probe b's result must succeed");

    (
        result_a.trim_end().to_owned(),
        result_b.trim_end().to_owned(),
    )
}

/// **The reproduction.** Two real processes, genuinely overlapping, racing
/// to create the audit store at a path neither has touched yet. Run
/// **20 times, each against a fresh path** -- a single run is not reliable
/// evidence either way, since the race's own window is small enough that
/// one pair of processes can miss it under ordinary machine load while the
/// underlying defect is still live. **With the fix in place, every one of
/// the 20 must succeed** -- this is the regression test, not only the
/// reproduction.
///
/// **Proven pre-fix by running this exact test in isolation, not through
/// `ablate.sh`.** `ablate.sh` invokes `cargo test --workspace --all-targets`,
/// which runs this test alongside the rest of the suite; that concurrent
/// activity perturbs the precise, low-noise timing this specific SQLite
/// multi-process busy-handler livelock needs to manifest, and across
/// repeated `ablate.sh` attempts the race did not reproduce at all (review
/// 527). **Proving it requires `cargo build -p tekstide-core --bins` before
/// the test** -- `cargo test -p tekstide-core --lib <name>` alone does not
/// build `[[bin]]` targets, so `audit_store_create_probe` must already exist
/// from an earlier build or the run fails for an unrelated reason
/// (`assert!(candidate.is_file(), ...)` above) that reads the same on both
/// sides of the fix -- and, since a hand-reverted call site is a dirty tree
/// `ablate.sh` itself refuses for good reason, in a disposable `git
/// worktree` (the recipe `ablate.sh`'s own header now carries) rather than a
/// manual edit-and-restore on the real one. Recorded in `qa-evidence.md`,
/// not only here.
#[test]
fn two_real_processes_racing_to_create_the_store_both_succeed() {
    for attempt in 0..20 {
        let dirs = fresh_dirs(&format!("first-creation-{attempt}"));
        let a = spawn_ready(&dirs.state_root, &dirs.project_root);
        let b = spawn_ready(&dirs.state_root, &dirs.project_root);
        let (result_a, result_b) = release_both(a, b);

        assert_eq!(
            result_a, "OK",
            "attempt {attempt}: the first of two real, overlapping processes creating the \
             store for the first time must succeed (got {result_a:?}; other process reported \
             {result_b:?})"
        );
        assert_eq!(
            result_b, "OK",
            "attempt {attempt}: the second of two real, overlapping processes creating the \
             store for the first time must succeed too, not merely not crash (got \
             {result_b:?}; other process reported {result_a:?})"
        );
    }
}
