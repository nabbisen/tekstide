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
///
/// **RFC-055: its two readers no longer mean the same thing.** The explorer
/// (`FileExplorerScanPolicy::linux_mvp`) now asks **git** which entries are
/// ignored and lets that answer decide what is collapsed; this list is its
/// **floor**, used only outside a repository or when git could not answer, and
/// the scan carries which of the two decided (`ExplorerIgnoreRule`).
/// `GeneratedChangeDetectionPolicy` still uses the list as the *whole* rule and
/// is unchanged. A constant shared by two readers that mean different things
/// must say so where it is defined; this is that.
pub const IGNORED_DIRECTORY_NAMES: &[&str] = &[".git", "node_modules", "target"];
