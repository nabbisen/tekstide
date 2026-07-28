//! RFC-014 quarantined GUI substrate spike.
//!
//! This crate is disposable measurement/rendering code. It must not become a
//! product dependency: no crate outside `tekstide-gui-spike` may depend on it,
//! and it must not persist any real Tekstide state (audit, transcript,
//! recent-project, or state root). See
//! `rfcs/handoffs/014-desktop-gui-substrate-and-terminal-rendering/` for the
//! full spike specification.

mod shell;

fn main() -> iced::Result {
    shell::run()
}
