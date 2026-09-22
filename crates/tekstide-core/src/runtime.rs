// crate-private: `evaluate` and its types are still being reshaped (R6/R7,
// review 407) and have no production caller yet (PR-030-B). Publishing
// `tekstide-core` before that settles would put an in-flux gate into the
// crate's public API.
pub(crate) mod git;
pub mod terminal;
