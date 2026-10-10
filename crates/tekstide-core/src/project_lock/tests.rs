use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{ProjectLockOutcome, acquire_project_lock, project_lock_file, project_locks_dir};
use crate::test_support::{KillOnDropChild, RealProcessLimiter, process_is_alive};

fn test_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("tekstide-{name}-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&root).expect("test root should be created");
    root
}

fn cleanup_root(root: PathBuf) {
    let _ = fs::remove_dir_all(root);
}

fn acquired(outcome: ProjectLockOutcome) -> super::ProjectLock {
    match outcome {
        ProjectLockOutcome::Acquired(lock) => lock,
        other => panic!("expected Acquired, got {other:?}"),
    }
}

/// D3: a lock another `File::open` on the same path can see, not an
/// in-memory flag this process alone understands -- the same proof
/// `transcript::tests::a_live_writer_holds_an_exclusive_lock_until_it_is_dropped`
/// already uses for the identical primitive.
#[test]
fn an_acquired_lock_is_visible_to_a_second_independent_handle() {
    let root = test_root("project-lock-visible");
    let lock = acquired(acquire_project_lock(&root, "proj-1"));

    let probe = fs::File::open(project_lock_file(&root, "proj-1"))
        .expect("the lock file must exist once acquired");
    assert!(
        matches!(probe.try_lock(), Err(fs::TryLockError::WouldBlock)),
        "a second, independent handle must see the file as held while the lock lives (D3)"
    );

    drop(lock);
    cleanup_root(root);
}

/// The exclusivity decision itself: a second attempt at the same project,
/// from the same path, is `HeldByAnother` while the first is alive, and
/// correctly reports the holder as this process's own pid (the file's
/// content, read back without needing the lock the first handle holds).
#[test]
fn a_second_attempt_at_the_same_project_is_held_by_another() {
    let root = test_root("project-lock-held");
    let _lock = acquired(acquire_project_lock(&root, "proj-1"));

    match acquire_project_lock(&root, "proj-1") {
        ProjectLockOutcome::HeldByAnother { holder_pid } => {
            assert_eq!(
                holder_pid,
                Some(std::process::id()),
                "the holder's own pid must be readable from the lock file's content"
            );
        }
        other => panic!("expected HeldByAnother, got {other:?}"),
    }

    cleanup_root(root);
}

/// Project-scoped (D3): two different ids under the same state root must
/// not collide, and the same id under two different state roots must not
/// either -- a lock is never wider or narrower than the one project it
/// names.
#[test]
fn different_projects_or_different_state_roots_never_collide() {
    let root = test_root("project-lock-scope");

    let same_root_other_project = acquire_project_lock(&root, "proj-2");
    assert!(
        matches!(same_root_other_project, ProjectLockOutcome::Acquired(_)),
        "a different project id under the same state root must get its own lock: \
         {same_root_other_project:?}"
    );

    let other_root = test_root("project-lock-scope-other");
    let same_project_other_root = acquire_project_lock(&other_root, "proj-2");
    assert!(
        matches!(same_project_other_root, ProjectLockOutcome::Acquired(_)),
        "the same project id under a different state root must get its own lock: \
         {same_project_other_root:?}"
    );

    cleanup_root(root);
    cleanup_root(other_root);
}

/// D4's clean half: dropping the guard closes the file, and the kernel
/// releases the lock the moment nothing holds it open any more -- but not
/// necessarily at the same instant as the `drop` call returns. Same bound,
/// same reason, as
/// `transcript::tests::a_live_writer_holds_an_exclusive_lock_until_it_is_dropped`:
/// while any other thread in this test binary is between `fork` and `exec`
/// (several sibling tests in this file spawn real child processes), its
/// child holds a copy of this lock's own descriptor too, and an `flock`
/// lives until the last copy closes. Asserted within a bound, not at the
/// instant of `drop`, for the identical reason that test already is.
#[test]
fn dropping_the_lock_releases_it_for_a_later_acquire() {
    let root = test_root("project-lock-drop-releases");
    let lock = acquired(acquire_project_lock(&root, "proj-1"));
    drop(lock);

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut last = None;
    let reacquired = loop {
        match acquire_project_lock(&root, "proj-1") {
            ProjectLockOutcome::Acquired(lock) => break Some(lock),
            other if std::time::Instant::now() < deadline => {
                last = Some(other);
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            other => {
                last = Some(other);
                break None;
            }
        }
    };
    assert!(
        reacquired.is_some(),
        "a dropped lock must be free for the next acquire within a bound: {last:?}"
    );

    cleanup_root(root);
}

/// D5, row 1's own example, "a stale file from a crashed process": content
/// naming a pid, with nobody actually holding the lock, must never be
/// mistaken for a live holder. Written directly, not via a real crash --
/// the point is that the *content* alone, unflocked, decides nothing.
#[test]
fn a_lock_file_naming_a_pid_but_held_by_nobody_does_not_block_opening() {
    let root = test_root("project-lock-stale-content");
    fs::create_dir_all(project_locks_dir(&root)).expect("locks dir should be creatable");
    fs::write(project_lock_file(&root, "proj-1"), b"999999999").expect("stale content writable");

    let outcome = acquire_project_lock(&root, "proj-1");
    assert!(
        matches!(outcome, ProjectLockOutcome::Acquired(_)),
        "unflocked stale content must never read as held: {outcome:?}"
    );

    let content = fs::read_to_string(project_lock_file(&root, "proj-1"))
        .expect("the acquired lock's own content should be readable");
    assert_eq!(
        content.trim(),
        std::process::id().to_string(),
        "acquiring must overwrite stale content with this process's own pid"
    );

    cleanup_root(root);
}

/// D5's own third named case, "a lock this build does not understand":
/// content that is not even a parseable pid must still decide correctly,
/// because the exclusivity decision never depended on parsing it.
#[test]
fn unparseable_lock_content_does_not_block_opening() {
    let root = test_root("project-lock-unparseable");
    fs::create_dir_all(project_locks_dir(&root)).expect("locks dir should be creatable");
    fs::write(
        project_lock_file(&root, "proj-1"),
        b"\xff\xfenot a pid at all",
    )
    .expect("garbage content writable");

    let outcome = acquire_project_lock(&root, "proj-1");
    assert!(
        matches!(outcome, ProjectLockOutcome::Acquired(_)),
        "content this build cannot parse must never block an otherwise-free lock: {outcome:?}"
    );

    cleanup_root(root);
}

/// D5's remaining named case, "an unreadable state directory": once the
/// locks directory itself cannot be read, the mechanism must say so
/// (`CannotDecide`) rather than guess -- and the caller's own obligation,
/// proven by the type this returns rather than by this test, is to open
/// the project anyway.
#[test]
fn an_unreadable_locks_directory_cannot_decide() {
    use std::os::unix::fs::PermissionsExt;

    let root = test_root("project-lock-unreadable-dir");
    let dir = project_locks_dir(&root);
    fs::create_dir_all(&dir).expect("locks dir should be creatable");
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o000))
        .expect("removing all permissions should succeed");

    let outcome = acquire_project_lock(&root, "proj-1");

    // Restored before any assertion can panic and before cleanup, which
    // needs to read this directory to remove it.
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
        .expect("restoring permissions should succeed");

    assert!(
        matches!(outcome, ProjectLockOutcome::CannotDecide { .. }),
        "an unreadable locks directory must be reported as undecidable, not as held or free: \
         {outcome:?}"
    );

    cleanup_root(root);
}

fn project_process_probe_binary_path() -> PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_project_process_probe") {
        return PathBuf::from(path);
    }
    let test_exe = std::env::current_exe().expect("current_exe should resolve for a running test");
    let profile_dir = test_exe
        .parent()
        .and_then(Path::parent)
        .expect("test binary should live under target/<profile>/deps");
    let candidate = profile_dir.join("project_process_probe");
    assert!(
        candidate.is_file(),
        "expected the project_process_probe binary at {}; the [[bin]] target may not have built",
        candidate.display()
    );
    candidate
}

/// D4's real half, proven against a real killed process rather than argued
/// from `try_lock`'s own documentation: a genuine child process acquires
/// this project's lock (through the probe, the same production call
/// `acquire_project_lock` a real boot would make) and is confirmed holding
/// it from a second, independent handle -- then `SIGKILL`ed, not released
/// through any cooperation of its own, reaped so its pid genuinely leaves
/// the process table, and the lock is confirmed free again.
#[test]
fn a_real_sigkilled_holder_releases_the_lock() {
    let _real_process_slot = RealProcessLimiter::acquire();
    let root = test_root("project-lock-real-sigkill");
    let project_root = root.join("project");
    fs::create_dir_all(&project_root).expect("project root should be creatable");
    fs::write(project_root.join("doc.txt"), b"hello").expect("fixture file writable");

    let mut child = Command::new(project_process_probe_binary_path())
        .arg(&root)
        .arg(&project_root)
        .arg("doc.txt")
        .arg("held by a process about to be killed")
        .arg("--hold")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn a real, held project_process_probe process");
    let pid = child.id();
    let stdout = child.stdout.take().expect("a piped child must have stdout");
    let mut reader = std::io::BufReader::new(stdout);

    let mut project_id = String::new();
    reader
        .read_line(&mut project_id)
        .expect("read the probe's project-id line");
    let project_id = project_id.trim().to_owned();
    assert!(!project_id.is_empty(), "the probe must print a project id");

    let mut status_line = String::new();
    reader
        .read_line(&mut status_line)
        .expect("read the probe's lock-status line");
    assert_eq!(
        status_line.trim(),
        "ACQUIRED",
        "the first, uncontended probe must acquire the lock"
    );

    let probe = fs::File::open(project_lock_file(&root, &project_id))
        .expect("the lock file must exist once the real process has acquired it");
    assert!(
        matches!(probe.try_lock(), Err(fs::TryLockError::WouldBlock)),
        "a real holder's lock must be visible to an independent handle before it is killed"
    );

    let mut child = KillOnDropChild::new(child);
    child.kill().expect("send a real SIGKILL to the holder");
    child.wait().expect("reap the killed holder");
    assert!(
        !process_is_alive(pid),
        "the killed holder's pid must have genuinely left the process table"
    );

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let released = loop {
        if probe.try_lock().is_ok() {
            break true;
        }
        if std::time::Instant::now() >= deadline {
            break false;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert!(
        released,
        "a real SIGKILL of the holder must release the lock -- it must not outlive its own \
         process (D4)"
    );

    cleanup_root(root);
}
