use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{bind_attention_listener, max_socket_path_len, request_attention};

/// Deliberately short, pid-plus-counter rather than
/// `recovery::tests::test_root`'s own nanosecond-timestamp shape --
/// `approval::tests::reference_adapter::unique_temp_dir`'s own doc
/// explains why: a `sockaddr_un` path is bounded (~107 usable bytes), the
/// full socket path here adds `/project-locks/<short-id>.sock` (review
/// 651 B1's own truncated filename) on top of this root, and a
/// timestamp-plus-nanoseconds label blows that budget outright where a
/// plain counter does not.
fn test_root(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("ta-{label}-{}-{sequence}", std::process::id()));
    fs::create_dir_all(&root).expect("test root should be created");
    // Review 617/618: same SymlinkAmbiguous class as watch_test_root() --
    // $TMPDIR is reached through a symlink on macOS.
    fs::canonicalize(&root).unwrap_or(root)
}

fn cleanup_root(root: PathBuf) {
    let _ = fs::remove_dir_all(root);
}

/// Review 602: the same regression `approval::channel`'s own
/// `max_socket_path_len` guards against -- two independently-maintained
/// copies of one computation (this module's own doc comment on
/// `max_socket_path_len` explains the deliberate duplication) disagree
/// the day someone fixes one without the other. Asserts the computed
/// value directly against each platform's real `sun_path` limit, rather
/// than through a real bound path -- the attention socket's own path
/// still derives from `state_root` today (unlike the approval channel's,
/// moved off it in review 601), so a real-path headroom test here would
/// currently fail on macOS for a reason this specific test is not about.
#[test]
fn max_socket_path_len_matches_the_real_platform_limit() {
    #[cfg(target_os = "linux")]
    const EXPECTED: usize = 107;
    #[cfg(target_os = "macos")]
    const EXPECTED: usize = 103;
    assert_eq!(max_socket_path_len(), EXPECTED);
}

/// Review 651 B1: pins the exact transformation, not just its effect on
/// a path length -- every hyphen is removed from the project id, only
/// the *last* 12 of the remaining 32 hex characters are kept, and the
/// suffix is the bare `.sock` (the `.attention` segment this socket's
/// filename used to carry is gone, redundant inside a directory already
/// named `tekstide-attention`/`project-locks`). Run on every platform:
/// `attention_socket_path` carries no `cfg` of its own, so Linux gets
/// the same shortened, truncated name as macOS even though only macOS
/// needed the bytes back.
#[test]
fn attention_socket_path_keeps_the_last_twelve_hex_characters_and_drops_the_attention_suffix_segment()
 {
    let state_root = test_root("filename-shape");
    let path = super::attention_socket_path(&state_root, "01234567-89ab-cdef-0123-456789abcdef");
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some("456789abcdef.sock"),
        "expected hyphens removed, only the last 12 of the remaining 32 hex characters kept, \
         and a bare .sock suffix with no .attention segment"
    );
    cleanup_root(state_root);
}

/// Review 651 B1, dated correction 2026-10-31: see
/// `approval::tests::channel`'s own sibling for the full story -- a
/// first version of this function took the *first* 12 characters, which
/// collapses every `ProjectId::for_test`-shaped sequential id (format
/// `00000000-0000-4000-8000-{sequence:012x}`, review `identity.rs`) to
/// the identical stem, since the distinguishing digits live entirely in
/// the last 12-hex-character group.
#[test]
fn attention_socket_path_distinguishes_sequential_test_fixture_ids() {
    let state_root = test_root("sequential-ids");
    let first = super::attention_socket_path(&state_root, "00000000-0000-4000-8000-000000000000");
    let second = super::attention_socket_path(&state_root, "00000000-0000-4000-8000-000000000001");
    assert_ne!(
        first, second,
        "two different for_test()-shaped sequential ids must not truncate to the same path -- \
         a collision here means every concurrently-running test that binds through it collides \
         on the real macOS runner, where the socket directory is shared rather than \
         state_root-derived"
    );
    cleanup_root(state_root);
}

/// Review 603's general D4 rule, macOS's shape: `attention_socket_dir`
/// there is always `$TMPDIR/tekstide-attention`, never derived from
/// `state_root` at all, so the real, deterministic path can be
/// asserted directly against the real runner's own `$TMPDIR` -- the
/// same headroom-style test `approval::channel`'s own macOS test uses,
/// adapted for this module's own filename shape (`<short-id>.sock`,
/// `attention_socket_path`'s own 12-character truncation, review 651 B1
/// -- no prefix to strip since `ProjectId` carries none).
/// **Review 651 B1: this test's own doc comment above overstated what it
/// measures.** `test_root` (above) joins under `std::env::temp_dir()`
/// too, but `ci.yml`'s macOS job runs the whole suite under
/// `TMPDIR="$CI_TMPDIR"` (`/tmp/tci`), so this test measures that short
/// anchor's own headroom, never the runner's real per-session `$TMPDIR`
/// (`/var/folders/…/T`, **not** canonicalized here -- `attention_socket_
/// dir`'s macOS arm, unlike the approval channel's, never resolves the
/// `/var` → `/private/var` symlink). Still a real, correct regression
/// check for this module's own fixed overhead; the diagnostic below
/// supplies the measurement against the real limit this one cannot.
#[cfg(target_os = "macos")]
#[test]
fn the_macos_attention_socket_path_leaves_comfortable_headroom_under_the_real_limit() {
    let state_root = test_root("macos-headroom");
    let project_id = uuid::Uuid::new_v4().to_string();
    let path = super::attention_socket_path(&state_root, &project_id);

    let path_len = path.as_os_str().len();
    const MACOS_SUN_PATH_LIMIT: usize = 103;
    const STATED_HEADROOM: usize = 10;
    assert!(
        path_len + STATED_HEADROOM <= MACOS_SUN_PATH_LIMIT,
        "the real, resolved macOS attention socket path is {path_len} bytes, leaving less than \
         the stated {STATED_HEADROOM}-byte headroom under the real {MACOS_SUN_PATH_LIMIT}-byte \
         sun_path limit -- attention_socket_dir's own $TMPDIR/tekstide-attention choice may \
         have regressed"
    );
    cleanup_root(state_root);
}

/// Review 651 B1: the permanent guard this whole subject exists to add --
/// see `approval::tests::channel`'s own sibling for the full reasoning.
/// Asserts the stated 10-byte headroom against the real runner's own
/// ambient `$TMPDIR`, not merely a prediction: measured over the limit by
/// 16 bytes before `attention_socket_path`'s own id truncation and
/// dropped `.attention` suffix segment (run `40982340829`). Was a pure,
/// assertion-free measurement (`#[ignore]`d) while the fix was still
/// being decided; promoted to a real assertion once it landed.
#[cfg(target_os = "macos")]
#[test]
fn macos_attention_socket_path_headroom_assertion_against_the_real_ambient_tmpdir() {
    let raw = std::env::temp_dir();
    eprintln!("MEASURED raw std::env::temp_dir() = {raw:?}");

    let state_root = test_root("macos-real-tmpdir-headroom-measurement");
    let project_id = uuid::Uuid::new_v4().to_string();
    let path = super::attention_socket_path(&state_root, &project_id);
    let path_len = path.as_os_str().len();
    const MACOS_SUN_PATH_LIMIT: usize = 103;
    const STATED_HEADROOM: usize = 10;
    eprintln!(
        "MEASURED attention socket path = {path:?} ({path_len} bytes, limit \
         {MACOS_SUN_PATH_LIMIT}, headroom {})",
        MACOS_SUN_PATH_LIMIT as isize - path_len as isize
    );
    assert!(
        path_len + STATED_HEADROOM <= MACOS_SUN_PATH_LIMIT,
        "the real, ambient-$TMPDIR-anchored macOS attention socket path is {path_len} bytes, \
         leaving less than the stated {STATED_HEADROOM}-byte headroom under the real \
         {MACOS_SUN_PATH_LIMIT}-byte sun_path limit -- the id truncation this subject added, \
         attention_socket_dir's own $TMPDIR/tekstide-attention choice, or the real $TMPDIR \
         anchor itself, may have grown"
    );
    cleanup_root(state_root);
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
