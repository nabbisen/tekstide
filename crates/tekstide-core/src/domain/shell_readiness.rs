//! RFC-074 D4/D5, Stage 2: relocated from `test_support` -- not a
//! capability dependency being satisfied, but one being removed. These
//! three values are pure, stable vocabulary (a probe command, its
//! expected echo, a timeout), not behaviour, and the whole reason they
//! lived in `test_support` was to give `tekstide-core`'s own
//! `runtime::terminal::launch::wait_for_shell_ready` and `tekstide`'s
//! separate `shell::tests::wait_for_shell_ready` (which drives
//! `TerminalPane`, not `TerminalRuntime`, so it cannot share the
//! function itself) **one shared definition of "what readiness means"
//! instead of two that can drift** -- moving them to `domain` keeps that
//! one definition while taking `test_support` out of the production
//! graph `runtime` used to reach into for it.
//!
//! `#[cfg(any(test, feature = "test-support"))]`, matching `test_support`'s
//! own gate exactly: neither `tekstide-core`'s nor `tekstide`'s release
//! build carries this.

/// Review 623/624: the readiness handshake for the macOS PTY-startup
/// race (`/bin/sh` is `bash` there, which reconfigures its own terminal
/// modes at startup, and a terminal-mode change can discard input the
/// kernel's line discipline has *already echoed*).
///
/// `$((1+1))` is arithmetic expansion: only a shell that actually
/// evaluates the command produces the digit `2`; the unexpanded text
/// `echo tek$((1+1))` an unread terminal merely echoes back contains no
/// such substring. Idempotent and side-effect-free, so a caller may
/// (and, per review 624's correction, must) resend it on every polling
/// iteration rather than once -- a single send can itself be the write
/// that gets discarded by the exact race this handshake exists to work
/// around, which would otherwise turn a real race into a flat panic
/// after the full timeout with nothing learned.
pub const SHELL_READINESS_PROBE_COMMAND: &[u8] = b"echo tek$((1+1))\n";
/// See [`SHELL_READINESS_PROBE_COMMAND`]. Never present in the probe's
/// own unexpanded, echoed text.
pub const SHELL_READINESS_MARKER: &[u8] = b"tek2";
/// See [`SHELL_READINESS_PROBE_COMMAND`].
pub const SHELL_READINESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
