// PR-030-B (review 410): `pub`, now that `compute_summary` is its real
// caller-facing entry point. `evaluate` and its supporting types stay
// `pub(crate)` inside the module -- only the summary itself crosses the
// crate boundary.
pub mod git;
pub mod terminal;
