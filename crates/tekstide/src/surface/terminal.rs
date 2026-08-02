//! RFC-017: the terminal surface. **PR-017-B only** — this module holds
//! the interposition filter and nothing else. `B → C` is strict: nothing
//! renders emulator output before [`filter::SecurityFilter`]'s P1-P4
//! properties are re-proven in this crate. The pane itself
//! (`terminal_pane`-equivalent, session bar, real rendering) is PR-017-C's
//! job, added to this module then, not stubbed here ahead of it.

// `tekstide` is `[[bin]]`-only (no `[lib]` target), so dead-code analysis
// treats every item as if it had no possible external consumer -- true
// today, since PR-017-C (the pane) is what gives this filter a real
// caller. Same shape as `i18n`'s own pre-RFC-015 period (PR-016-B); the
// targeted suppression dies the moment PR-017-C adds a real call site,
// per response 122 Required 3's standing preference for that over a
// blanket crate-wide allow.
#[allow(dead_code)]
pub mod filter;
