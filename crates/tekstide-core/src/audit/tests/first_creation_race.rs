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

fn fresh_dirs(label: &str) -> (PathBuf, PathBuf) {
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
    (state_root, project_root)
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
/// to create the audit store at a path neither has touched yet. Proven
/// against the tree as it stood before this RFC's own fix by running this
/// exact test with the fix in `migration.rs` reverted (`ablate.sh`,
/// recorded in `qa-evidence.md`): over repeated runs, at least one of the
/// two probes reported `ERR AuditStoreError { reason: Sqlite }` -- the
/// loser's own `CREATE TABLE audit_events` failing because the winner's had
/// already committed. **With the fix in place, both must succeed, every
/// time** -- this is the regression test, not only the reproduction.
#[test]
fn two_real_processes_racing_to_create_the_store_both_succeed() {
    let (state_root, project_root) = fresh_dirs("first-creation");
    let a = spawn_ready(&state_root, &project_root);
    let b = spawn_ready(&state_root, &project_root);
    let (result_a, result_b) = release_both(a, b);

    assert_eq!(
        result_a, "OK",
        "the first of two real, overlapping processes creating the store for the first time \
         must succeed (got {result_a:?}; other process reported {result_b:?})"
    );
    assert_eq!(
        result_b, "OK",
        "the second of two real, overlapping processes creating the store for the first time \
         must succeed too, not merely not crash (got {result_b:?}; other process reported \
         {result_a:?})"
    );

    let _ = std::fs::remove_dir_all(state_root.parent().unwrap());
}
