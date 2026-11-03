use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

fn scratch_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "interface-scale-{label}-{}-{sequence}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    root
}

/// The ordinary case: nobody has ever scaled the interface, so the state
/// directory does not even exist yet. Not an error -- the reset value,
/// silently, the same as every other "nothing to load" case below.
#[test]
fn loading_with_no_file_at_all_reports_the_reset_value() {
    let dir = scratch_dir("missing");
    assert_eq!(load_interface_scale(&dir), INTERFACE_SCALE_RESET);
}

/// A real round trip: save a non-default step, load it back, get the
/// exact value saved -- not a nearby step, not the reset.
#[test]
fn a_saved_step_is_loaded_back_exactly() {
    let dir = scratch_dir("round-trip");
    save_interface_scale(&dir, INTERFACE_SCALE_STEPS[3]).expect("the write should succeed");
    assert_eq!(load_interface_scale(&dir), INTERFACE_SCALE_STEPS[3]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Malformed content -- not JSON at all -- degrades to the reset value
/// rather than a panic or a propagated error. Loading this preference
/// must never be able to stop the application from starting.
#[test]
fn unparseable_content_reports_the_reset_value() {
    let dir = scratch_dir("malformed");
    std::fs::create_dir_all(&dir).expect("the scratch dir should be creatable");
    std::fs::write(interface_scale_file(&dir), b"not json at all")
        .expect("the malformed file should be writable");
    assert_eq!(load_interface_scale(&dir), INTERFACE_SCALE_RESET);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A future, incompatible version is not trusted -- read as the reset
/// value rather than assumed to mean what today's `scale` field means.
#[test]
fn an_unrecognized_state_version_reports_the_reset_value() {
    let dir = scratch_dir("future-version");
    std::fs::create_dir_all(&dir).expect("the scratch dir should be creatable");
    let future = InterfaceScaleState {
        state_version: INTERFACE_SCALE_STATE_VERSION + 1,
        scale: INTERFACE_SCALE_STEPS[4],
    };
    std::fs::write(
        interface_scale_file(&dir),
        serde_json::to_vec(&future).unwrap(),
    )
    .expect("the future-version file should be writable");
    assert_eq!(load_interface_scale(&dir), INTERFACE_SCALE_RESET);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `save_interface_scale` creates the state directory itself -- a caller
/// must not have to `mkdir` first, the same "first file ever written
/// there" case `recent-projects.json`'s own store handles.
#[test]
fn saving_creates_the_state_directory_if_it_does_not_exist_yet() {
    let dir = scratch_dir("creates-dir");
    assert!(!dir.exists());
    save_interface_scale(&dir, INTERFACE_SCALE_STEPS[0]).expect("the write should succeed");
    assert_eq!(load_interface_scale(&dir), INTERFACE_SCALE_STEPS[0]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `Ctrl+Alt+0`'s own value is independent of the step table, not a
/// position inside it that could move -- pinned directly rather than
/// assumed from `INTERFACE_SCALE_STEPS[1]` happening to equal it today.
#[test]
fn reset_is_exactly_1_0_independent_of_the_step_table() {
    assert_eq!(INTERFACE_SCALE_RESET, 1.0);
}
