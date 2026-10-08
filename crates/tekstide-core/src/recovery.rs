//! RFC-027: crash detection and unsaved-buffer persistence. PR-027-A ships only the
//! first half -- a crash is detected and reported internally; no buffer content is
//! written anywhere yet (`what-recovery-must-not-do.md` §1 row 1).

mod instance;

pub use instance::{DetectedCrash, InstanceMarker, InstanceStartup, instances_dir, start_instance};

#[cfg(test)]
mod tests;
