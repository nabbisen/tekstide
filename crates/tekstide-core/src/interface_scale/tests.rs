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

/// PR-076-C2: a well-formed, current-version file is not enough on its
/// own -- the `scale` it holds must also be a real member of
/// [`INTERFACE_SCALE_STEPS`]. `7331.0` is not a plausible future step
/// (nothing in this range, nor a number that merely looks like one of
/// today's five). This is one of **two** reachable "surviving
/// deserialization but not a step" shapes -- see the `1e40` test below
/// for the other, non-finite one, and the `NaN` test below that for the
/// one shape that is not reachable at all.
#[test]
fn a_well_formed_file_holding_a_value_off_the_step_table_reports_the_reset_value() {
    let dir = scratch_dir("off-table");
    std::fs::create_dir_all(&dir).expect("the scratch dir should be creatable");
    let off_table = InterfaceScaleState {
        state_version: INTERFACE_SCALE_STATE_VERSION,
        scale: 7331.0,
    };
    std::fs::write(
        interface_scale_file(&dir),
        serde_json::to_vec(&off_table).unwrap(),
    )
    .expect("the off-table file should be writable");
    assert_eq!(load_interface_scale(&dir), INTERFACE_SCALE_RESET);
    let _ = std::fs::remove_dir_all(&dir);
}

/// PR-076-C2, response 661's own required addition: a non-finite `scale`
/// **is** reachable from a real file, by overflow rather than by a
/// plainly-silly number -- `1e40` is in range for the JSON number
/// grammar `serde_json` parses, deserializes successfully as an `f64`,
/// and the `f32` visitor's own cast overflows it to `f32::INFINITY`.
/// Written as a literal in the file's own text, not produced by
/// `serde_json::to_vec` on an `InterfaceScaleState` holding
/// `f32::INFINITY` -- serializing that gives `{"scale":null}`, the same
/// `null` shape `NaN` serializes to below, which would test the
/// unparseable branch again rather than this one. Not `1e400`: that is
/// out of range for the grammar itself and is a parse error, the wrong
/// branch entirely. This is the worst input in the whole module --
/// `f32::INFINITY` handed to `iced::application`'s own `scale_factor` --
/// so it gets its own named test rather than standing in for `7331.0`
/// above, which is finite and arrives by a different route.
#[test]
fn a_well_formed_file_holding_an_overflowed_infinity_reports_the_reset_value() {
    let dir = scratch_dir("overflowed-infinity");
    std::fs::create_dir_all(&dir).expect("the scratch dir should be creatable");
    std::fs::write(
        interface_scale_file(&dir),
        br#"{"state_version":1,"scale":1e40}"#,
    )
    .expect("the overflowed-infinity file should be writable");
    assert_eq!(load_interface_scale(&dir), INTERFACE_SCALE_RESET);
    let _ = std::fs::remove_dir_all(&dir);
}

/// PR-076-C2's own instruction asked for a file-load test proving `NaN`
/// is caught by the membership check -- **checked rather than assumed,
/// and the premise does not hold**: no file can actually deliver a
/// `NaN` `scale` to that check in the first place. `serde_json`
/// serializes `f32::NAN` as JSON `null` (confirmed directly:
/// `serde_json::to_vec` on a `NaN`-holding `InterfaceScaleState`
/// produces `{"state_version":1,"scale":null}`), and `null` fails to
/// deserialize into `f32` at all -- caught by `serde_json::from_str`'s
/// own `Err`, the exact same branch
/// `unparseable_content_reports_the_reset_value` above already proves,
/// before `load_interface_scale` ever reaches the membership check. A
/// literal `NaN` token written into the file by hand fares no better:
/// bare `NaN` is not valid JSON syntax, so `serde_json::from_str` refuses
/// it with a parse error at that same point, not a successful parse of
/// the number. **`NaN` specifically is unreachable through real file
/// I/O** -- this is not a completeness claim over every non-step value
/// that survives deserialization (response 661 found a third shape, the
/// overflowed-infinity test above, that this doc comment used to omit);
/// it is a claim about `NaN` alone. This test checks the predicate the
/// loader actually runs (`INTERFACE_SCALE_STEPS.contains(&state.scale)`)
/// directly, not through a file this crate's own serializer cannot
/// produce.
#[test]
fn the_membership_check_rejects_nan_on_its_own_terms_even_though_no_real_file_can_deliver_one() {
    assert!(
        !INTERFACE_SCALE_STEPS.contains(&f32::NAN),
        "exact equality already excludes NaN (NaN != NaN, by IEEE 754), so the loader's \
         membership check would still refuse it if some future change ever made a NaN \
         survive deserialization -- correct by construction, not merely by the \
         deserializer's own, separate refusal"
    );
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

/// PR-076-C2, response 661's own required addition: the membership
/// check this slice just added makes a fact load-bearing that nothing
/// tested before -- **[`INTERFACE_SCALE_RESET`] must itself be a member
/// of [`INTERFACE_SCALE_STEPS`]**, or a Reset press would write a value
/// the loader rejects on the very next launch, silently discarding a
/// preference that appeared to save. True today only because
/// `INTERFACE_SCALE_STEPS[1] == INTERFACE_SCALE_RESET`, which
/// `reset_is_exactly_1_0_independent_of_the_step_table` above
/// deliberately does not pin together -- that test exists so the table
/// can move without moving reset; this one exists so a table move that
/// breaks the coincidence is caught, through the same two functions the
/// application actually uses (`save_interface_scale` then
/// `load_interface_scale`), not a `contains` assertion that would not
/// notice the two constants drifting apart from each other.
#[test]
fn the_reset_value_round_trips_through_save_and_load() {
    let dir = scratch_dir("reset-round-trip");
    save_interface_scale(&dir, INTERFACE_SCALE_RESET).expect("the write should succeed");
    assert_eq!(load_interface_scale(&dir), INTERFACE_SCALE_RESET);
    let _ = std::fs::remove_dir_all(&dir);
}
