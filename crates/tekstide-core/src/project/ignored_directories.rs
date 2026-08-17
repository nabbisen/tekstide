/// Change-detection-wiring handoff, decision D1: one shared definition of
/// directory names project-wide scanners skip by default. Before this,
/// `FileExplorerScanPolicy::linux_mvp` (`project::root::explorer`) and
/// `GeneratedChangeDetectionPolicy` (`project::change_detection`) each had
/// their own hardcoded `[".git", "node_modules", "target"]` -- one used it
/// to collapse rows for display, the other had none at all and walked
/// everything. Two independently-maintained copies of the same list is a
/// defect the day someone edits one without the other; both now source
/// from this single array.
///
/// **Not `.gitignore` parsing, and not meant to become it here.** This is
/// a small, fixed set of well-known noise directories (VCS metadata,
/// package manager caches, build output) matched by exact name, at any
/// depth. Real `.gitignore` handling -- negation, precedence, nested
/// `.gitignore` files -- is a distinct feature with its own subtleties,
/// out of scope for this list and belonging with RFC-030 (Git
/// Integration) if and when it is built.
pub const IGNORED_DIRECTORY_NAMES: &[&str] = &[".git", "node_modules", "target"];
