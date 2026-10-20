pub mod agent;
pub mod app;
pub mod approval;
pub mod audit;
pub mod close;
pub mod command;
pub mod config;
pub mod content;
pub mod domain;
pub mod navigation;
pub mod project;
pub mod project_board;
pub mod project_lock;
pub mod recovery;
pub mod route;
pub mod runtime;
pub mod security;
pub mod shell;
pub mod text_safety;
pub mod transcript;

/// Response 232: `RealProcessLimiter` used to live only inside
/// `runtime::terminal::reader::tests`, capping contention among that
/// one file's own real-PTY-spawning tests. RFC-022's approval tests
/// spawn real processes too (a real child process in
/// `approval::tests::channel`, the real `reference_adapter` binary
/// repeatedly in `approval::tests::reference_adapter`) and were not
/// under any cap -- lifted here, to a location both modules can share
/// a single static from, so the cap is genuinely process-wide rather
/// than two independent per-module pools that would do nothing to
/// reduce concurrent forks *across* modules.
/// Review 624: `pub`, not `pub(crate)`, and gated on the same
/// `#[cfg(any(test, feature = "test-support"))]` this crate's own D4 leak
/// guard and `ProjectSession::add_transcript` already use -- a bare
/// `#[cfg(test)]` module path is invisible to a consuming crate's own
/// tests, the same lesson those two record, and `tekstide`'s own
/// `shell/tests.rs` is exactly such a caller now (the PTY-startup-race
/// readiness handshake's shared constants, below). Every pre-existing
/// item in this module stays `pub(crate)`: this only widens *when* the
/// module compiles, not *what in it* a consuming crate can reach.
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
