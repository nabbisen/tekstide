pub mod agent;
pub mod app;
pub mod approval;
pub mod audit;
pub mod close;
pub mod command;
pub mod content;
pub mod domain;
pub mod navigation;
pub mod project;
pub mod project_board;
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
#[cfg(test)]
pub(crate) mod test_support;
