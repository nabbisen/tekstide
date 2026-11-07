//! RFC-076 D1/D5: the whole-UI scale factor's own five enumerated steps,
//! and the persisted state that remembers which one a user last chose.
//!
//! **Five absolute steps, not a free multiplier.** D1's own "bounded,
//! enumerated set" is satisfied by construction here, not by clamping a
//! relative accumulator: there is nothing to clamp, because every action
//! this module's values back sets the scale to one specific, named step.
//! Pressing `Ctrl+Alt+3` five times in a row does exactly what pressing it
//! once does.
//!
//! **Never `config.toml` (D5).** A runtime zoom is per-session UI state,
//! not a configuration value a user hand-wrote -- this project's whole
//! installation story is "touches nothing else," and an application that
//! silently rewrites a file the user edited by hand breaks that promise in
//! the one place a user is most likely to notice. The scale lives beside
//! `recent-projects.json` under the state directory instead, in its own
//! small file (`docs/src/users/local-data-and-privacy.md` names it, since
//! that page's own contract is to list every file this project writes).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Smallest to largest, in index order -- `Ctrl+Alt+1` through
/// `Ctrl+Alt+5` each set the scale to exactly `INTERFACE_SCALE_STEPS[n -
/// 1]`, never a value between two steps or past either end. Chosen to
/// span "slightly tighter" to "clearly larger" without a step so large
/// the Project Board stops fitting -- checked against a real rendered
/// capture at the largest step
/// (`rfcs/handoffs/076-readable-by-default/evidence/`), not only
/// asserted here.
pub const INTERFACE_SCALE_STEPS: [f32; 5] = [0.9, 1.0, 1.15, 1.3, 1.5];

/// What `Ctrl+Alt+0` resets to -- **exactly**, independent of
/// [`INTERFACE_SCALE_STEPS`]. Deliberately not "jump to whichever step
/// currently equals 1.0": the two happen to coincide today
/// (`INTERFACE_SCALE_STEPS[1] == INTERFACE_SCALE_RESET`), but reset is a
/// fixed, named escape hatch, not a position in a table that could move
/// if the steps are ever revised.
pub const INTERFACE_SCALE_RESET: f32 = 1.0;

pub const INTERFACE_SCALE_STATE_VERSION: u32 = 1;

/// The persisted record -- one field that matters, `scale`, plus a
/// version so a future, incompatible shape can tell an old file apart
/// from a new one rather than guess.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct InterfaceScaleState {
    pub state_version: u32,
    pub scale: f32,
}

impl Default for InterfaceScaleState {
    fn default() -> Self {
        Self {
            state_version: INTERFACE_SCALE_STATE_VERSION,
            scale: INTERFACE_SCALE_RESET,
        }
    }
}

/// `<state_dir>/interface-scale.json` -- `state_dir` is whatever
/// `tekstide_core::project::recent::AppStatePathProvider::state_dir`
/// already resolved for this host, the same directory
/// `recent-projects.json` lives in. Not a method on that type: this
/// module has no other reason to depend on `project::recent`, and the
/// path is one `join` away from a value every caller already has.
pub fn interface_scale_file(state_dir: &Path) -> PathBuf {
    state_dir.join("interface-scale.json")
}

/// Loads the persisted scale, or [`INTERFACE_SCALE_RESET`] on **any**
/// problem at all -- a missing file (the ordinary case: nobody has
/// scaled the interface yet), one that cannot be read, one that does not
/// parse, one written by a future, incompatible version, or one whose
/// `scale` is not exactly a member of [`INTERFACE_SCALE_STEPS`] (a
/// corrupt or hand-edited current-version file -- PR-076-C2's own fix:
/// every other writer already only ever stores one of the five steps or
/// [`INTERFACE_SCALE_RESET`] itself, so this is the one door a value
/// outside the set could previously walk through). A lost scale
/// preference is never worth refusing to boot over, or telling the user
/// about: the control that set it is one keypress away, every time.
///
/// Membership is checked by exact equality against the table, never a
/// tolerance or a nearest-step snap -- a snap would silently honour a
/// value nobody chose, which is exactly the kind of quiet substitution
/// this module exists to avoid. `NaN` would fail `==` against every
/// member (including itself) if it ever reached this check, so there is
/// no separate `is_nan()` arm; in practice it never gets this far
/// either way -- `serde_json` serializes `f32::NAN` as JSON `null`,
/// which fails to deserialize into `f32` at all, so a `NaN` `scale` is
/// already caught by the deserialization `Err` above, the same branch
/// an unparseable file hits (confirmed directly, not assumed --
/// `interface_scale/tests.rs`'s own
/// `the_membership_check_rejects_nan_on_its_own_terms_even_though_no_real_file_can_deliver_one`
/// tests the predicate's own correctness instead, since no real file
/// can reach it with one).
pub fn load_interface_scale(state_dir: &Path) -> f32 {
    let Ok(contents) = fs::read_to_string(interface_scale_file(state_dir)) else {
        return INTERFACE_SCALE_RESET;
    };
    let Ok(state) = serde_json::from_str::<InterfaceScaleState>(&contents) else {
        return INTERFACE_SCALE_RESET;
    };
    if state.state_version != INTERFACE_SCALE_STATE_VERSION {
        return INTERFACE_SCALE_RESET;
    }
    if !INTERFACE_SCALE_STEPS.contains(&state.scale) {
        return INTERFACE_SCALE_RESET;
    }
    state.scale
}

/// Writes the scale, creating the state directory if this is the first
/// file ever written there. A plain write-then-rename -- not the
/// recovery system's own fsync-and-chmod durability (`recovery::record`):
/// losing the last few milliseconds of this one write costs a user one
/// keypress to restore their preference next launch, not unsaved text or
/// a security-relevant record, so that heavier machinery would be cost
/// without a matching benefit here.
pub fn save_interface_scale(state_dir: &Path, scale: f32) -> io::Result<()> {
    fs::create_dir_all(state_dir)?;
    let state = InterfaceScaleState {
        state_version: INTERFACE_SCALE_STATE_VERSION,
        scale,
    };
    let json = serde_json::to_vec_pretty(&state)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let target = interface_scale_file(state_dir);
    let temp = target.with_extension("json.tmp");
    fs::write(&temp, &json)?;
    fs::rename(&temp, &target)?;
    Ok(())
}

#[cfg(test)]
mod tests;
