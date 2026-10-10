use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{bind_attention_listener, request_attention};

/// Deliberately short, pid-plus-counter rather than
/// `recovery::tests::test_root`'s own nanosecond-timestamp shape --
/// `approval::tests::reference_adapter::unique_temp_dir`'s own doc
/// explains why: a `sockaddr_un` path is bounded (~107 usable bytes), the
/// full socket path here adds `/project-locks/<project_id>.attention.sock`
/// on top of this root, and a timestamp-plus-nanoseconds label blows that
/// budget outright where a plain counter does not.
fn test_root(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("ta-{label}-{}-{sequence}", std::process::id()));
    fs::create_dir_all(&root).expect("test root should be created");
    root
}

fn cleanup_root(root: PathBuf) {
    let _ = fs::remove_dir_all(root);
}

/// D10: a knock is delivered as a real accepted connection, not an
/// in-memory flag -- the holder's own `accept()` is what fires.
#[test]
fn a_bound_listener_receives_a_real_knock() {
    let root = test_root("knock");
    let listener = bind_attention_listener(&root, "proj-1").expect("bind should succeed");

    let knocked = std::thread::spawn({
        let root = root.clone();
        move || request_attention(&root, "proj-1")
    });

    let accepted = listener.accept();
    assert!(
        accepted.is_ok(),
        "the knock must arrive as a real connection: {accepted:?}"
    );
    assert!(
        knocked.join().expect("knocking thread must not panic"),
        "request_attention must report the holder as reached"
    );

    cleanup_root(root);
}

/// A knock against a project with no bound listener at all (nobody holds
/// it, or the holder's own bind failed) must never block or panic -- D9's
/// own "best-effort" framing applies to the knocker too.
#[test]
fn knocking_with_no_listener_bound_fails_quietly() {
    let root = test_root("no-listener");
    assert!(
        !request_attention(&root, "nobody-is-listening"),
        "a knock against nothing must report false, not hang or panic"
    );
    cleanup_root(root);
}

/// D10 shares D4's own hazard: whatever makes the holder discoverable
/// must not outlive it. A fresh `bind_attention_listener` for the same
/// project after an earlier one's socket file was left behind (simulating
/// a crash -- the file exists, nothing is listening on it any more) must
/// still succeed, not treat the stale special file as still in use.
#[test]
fn binding_over_a_stale_socket_file_still_succeeds() {
    let root = test_root("stale");
    let first = bind_attention_listener(&root, "proj-1").expect("first bind should succeed");
    drop(first);

    let second = bind_attention_listener(&root, "proj-1");
    assert!(
        second.is_some(),
        "a dropped listener's own leftover socket file must not block the next bind"
    );

    cleanup_root(root);
}
