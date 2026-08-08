//! RFC-017 PR-017-B: promoted from the RFC-014 spike crate's own
//! `filter.rs` (reviewed under RFC-014 PR-014-C). This is not a rewrite
//! -- it is the same interposition shape, re-pointed so the
//! accept/reject *decision* comes from
//! `tekstide_core::runtime::terminal::security`'s policy data instead of
//! a second, shell-local copy of RFC-009's accepted set. The spike crate
//! (`tekstide-gui-spike`) was deleted 2026-08-04, its own promotion
//! having landed here first -- see
//! `rfcs/handoffs/014-desktop-gui-substrate-and-terminal-rendering/spike-crate-deletion.md`
//! for the deletion record and where its evidence now lives.
//!
//! # Why this boundary, not the byte/`Perform` boundary
//!
//! Unchanged from the spike's own reasoning (`filter.rs`'s module doc
//! there, still accurate): `alacritty_terminal` 0.26 depends directly on
//! `vte` 0.15's bundled `ansi` module, and `vte::ansi::Processor` owns the
//! entire VT/ANSI grammar, dispatching already-fully-classified semantic
//! operations to a `vte::ansi::Handler` implementation on every
//! `advance()` call. `Term` implements that trait. Interposing here means:
//!
//! - **P3 (classification parity) holds by construction.** The filter and
//!   the real emulator share the identical classifier; there is no second
//!   parse to drift out of sync with the first.
//! - **P4 (stream-position independence) holds by construction.** The
//!   `Processor` instance is long-lived and holds VT parser state
//!   internally across `advance()` calls, so a sequence split across two
//!   PTY reads is reassembled by the same code the real terminal uses,
//!   before this filter ever sees it.
//!
//! # What changed from the spike: policy comes from `tekstide-core`
//!
//! The spike's `SecurityFilter` decided, by its own hardcoded method
//! overrides, which `Handler` calls to forward. That is a second
//! classifier: `tekstide-core::runtime::terminal::security` already
//! enumerates the accepted set (`TerminalSequencePolicy::ACCEPTED`,
//! `TerminalAcceptedSequence`), and a shell-crate copy of that same list
//! is exactly the drift risk `implementation-handoff.md` §3 names. Every
//! accepted `Handler` method here is gated on
//! [`SecurityFilter::accepts`], which asks `TerminalSequencePolicy::ACCEPTED`
//! at the call site rather than encoding the answer locally --
//! `tekstide_core_gains_no_vte_or_alacritty_dependency` (see
//! `qa-evidence.md`) still holds, since `TerminalAcceptedSequence`/
//! `TerminalSequenceFamily` are plain data enums with no `vte`/
//! `alacritty_terminal` type in their signatures.
//!
//! This filter is responsible for P1 (single ingress) and P2 (no side
//! channels): every accepted operation is forwarded to the wrapped
//! [`Handler`] explicitly, one method at a time, gated on core's policy;
//! every other method defaults to a no-op via the trait's own default
//! body -- never forwarded, so it can never reach the inner handler
//! through this path.
//!
//! # P1/P2 callers must also uphold
//!
//! This filter only closes the path from *PTY bytes*. `Term::grid_mut()`
//! is public API for direct grid manipulation and is **not** reachable
//! from the PTY byte stream -- `vte::ansi::Processor` never calls it. P1
//! therefore also requires that calling code never wires PTY bytes to
//! `grid_mut()` or any other direct `Term` mutator outside
//! `Processor::advance`. **No production caller exists yet in this
//! slice** (PR-017-C builds the pane); this module and its test harness
//! are the only place `alacritty_terminal`/`vte` types appear in this
//! crate today -- confirmed by enumeration in `qa-evidence.md`, not
//! assumed.

use alacritty_terminal::vte::ansi::{
    Attr, ClearMode, Handler, Hyperlink, KeyboardModes, KeyboardModesApplyBehavior, LineClearMode,
    Mode, ModifyOtherKeys, PrivateMode, ScpCharPath, ScpUpdateMode,
};
use tekstide_core::runtime::terminal::{
    TerminalAcceptedSequence, TerminalSequenceFamily, TerminalSequencePolicy,
    classify_private_mode_number,
};

/// Wraps a real [`Handler`] (in product code, `alacritty_terminal::Term`),
/// forwarding only what `tekstide_core::runtime::terminal::security`'s
/// `TerminalSequencePolicy::ACCEPTED` names. This type holds no policy of
/// its own -- see the module doc.
pub struct SecurityFilter<'a, H: Handler> {
    inner: &'a mut H,
    /// Every family this filter declined to forward, for tests and future
    /// diagnostic wiring -- not itself the security boundary. The
    /// boundary is [`SecurityFilter::accepts`] returning `false`.
    pub blocked: Vec<TerminalSequenceFamily>,
}

impl<'a, H: Handler> SecurityFilter<'a, H> {
    pub fn new(inner: &'a mut H) -> Self {
        Self {
            inner,
            blocked: Vec::new(),
        }
    }

    /// The single point every accept/reject decision passes through.
    /// Ablate `TerminalSequencePolicy::ACCEPTED` (remove an entry) and
    /// this filter's forwarding behaviour changes with it -- proof this
    /// is a live delegation, not a renamed copy (`qa-evidence.md`).
    fn accepts(sequence: TerminalAcceptedSequence) -> bool {
        TerminalSequencePolicy::ACCEPTED.contains(&sequence)
    }

    fn block(&mut self, family: TerminalSequenceFamily) {
        self.blocked.push(family);
    }

    /// Forwards `action` to `inner` only if `tekstide-core` still lists
    /// `sequence` as accepted; otherwise records `family` in `blocked`,
    /// the same as an explicitly-classified block below. **Found by
    /// ablation, not by inspection**: an earlier version of this filter
    /// gated the forwarding call but recorded nothing when the gate
    /// declined, so removing an entry from `ACCEPTED` silently dropped
    /// the operation without ever appearing in `blocked` -- the
    /// delegation ablation (`qa-evidence.md`) passed for the wrong
    /// reason until this was added. Symmetric handling closes that gap:
    /// every decline, explicit or policy-driven, is now observable the
    /// same way.
    fn forward_if_accepted(
        &mut self,
        sequence: TerminalAcceptedSequence,
        family: TerminalSequenceFamily,
        action: impl FnOnce(&mut H),
    ) {
        if Self::accepts(sequence) {
            action(self.inner);
        } else {
            self.block(family);
        }
    }
}

impl<'a, H: Handler> Handler for SecurityFilter<'a, H> {
    // --- RFC-009 accepted set: forwarded only if `tekstide-core`'s policy
    // still names it. Mapped 1:1 to `TerminalAcceptedSequence`'s nine
    // variants -- the same set the spike's hardcoded forwarding list
    // happened to match exactly, confirmed by inspection, not assumed.

    fn input(&mut self, c: char) {
        self.forward_if_accepted(
            TerminalAcceptedSequence::PrintableUtf8,
            TerminalSequenceFamily::PrintableText,
            |inner| inner.input(c),
        );
    }

    fn carriage_return(&mut self) {
        self.forward_if_accepted(
            TerminalAcceptedSequence::C0CarriageReturn,
            TerminalSequenceFamily::C0Control,
            |inner| inner.carriage_return(),
        );
    }

    fn linefeed(&mut self) {
        self.forward_if_accepted(
            TerminalAcceptedSequence::C0LineFeed,
            TerminalSequenceFamily::C0Control,
            |inner| inner.linefeed(),
        );
    }

    fn put_tab(&mut self, count: u16) {
        self.forward_if_accepted(
            TerminalAcceptedSequence::C0Tab,
            TerminalSequenceFamily::C0Control,
            |inner| inner.put_tab(count),
        );
    }

    fn backspace(&mut self) {
        self.forward_if_accepted(
            TerminalAcceptedSequence::C0Backspace,
            TerminalSequenceFamily::C0Control,
            |inner| inner.backspace(),
        );
    }

    fn terminal_attribute(&mut self, attr: Attr) {
        self.forward_if_accepted(
            TerminalAcceptedSequence::CsiSgr,
            TerminalSequenceFamily::Sgr,
            |inner| inner.terminal_attribute(attr),
        );
    }

    fn move_up(&mut self, rows: usize) {
        self.forward_if_accepted(
            TerminalAcceptedSequence::CsiCursorMovement,
            TerminalSequenceFamily::Csi,
            |inner| inner.move_up(rows),
        );
    }

    fn move_down(&mut self, rows: usize) {
        self.forward_if_accepted(
            TerminalAcceptedSequence::CsiCursorMovement,
            TerminalSequenceFamily::Csi,
            |inner| inner.move_down(rows),
        );
    }

    fn move_forward(&mut self, cols: usize) {
        self.forward_if_accepted(
            TerminalAcceptedSequence::CsiCursorMovement,
            TerminalSequenceFamily::Csi,
            |inner| inner.move_forward(cols),
        );
    }

    fn move_backward(&mut self, cols: usize) {
        self.forward_if_accepted(
            TerminalAcceptedSequence::CsiCursorMovement,
            TerminalSequenceFamily::Csi,
            |inner| inner.move_backward(cols),
        );
    }

    fn clear_line(&mut self, mode: LineClearMode) {
        self.forward_if_accepted(
            TerminalAcceptedSequence::CsiClearLine,
            TerminalSequenceFamily::Csi,
            |inner| inner.clear_line(mode),
        );
    }

    fn clear_screen(&mut self, mode: ClearMode) {
        self.forward_if_accepted(
            TerminalAcceptedSequence::CsiClearScreen,
            TerminalSequenceFamily::Csi,
            |inner| inner.clear_screen(mode),
        );
    }

    // --- Explicitly classified blocks: PR-017-B's named minimum corpus,
    // tagged with `tekstide-core`'s own `TerminalSequenceFamily` so a
    // diagnostic/audit consumer (a later slice) reads the same vocabulary
    // the headless byte-parser already uses. ---

    fn set_title(&mut self, _title: Option<String>) {
        self.block(TerminalSequenceFamily::OscTitle);
    }

    fn push_title(&mut self) {
        self.block(TerminalSequenceFamily::OscTitle);
    }

    fn pop_title(&mut self) {
        self.block(TerminalSequenceFamily::OscTitle);
    }

    fn clipboard_store(&mut self, _clipboard: u8, _base64: &[u8]) {
        self.block(TerminalSequenceFamily::Osc52Clipboard);
    }

    fn clipboard_load(&mut self, _clipboard: u8, _terminator: &str) {
        self.block(TerminalSequenceFamily::Osc52Clipboard);
    }

    fn set_hyperlink(&mut self, _hyperlink: Option<Hyperlink>) {
        self.block(TerminalSequenceFamily::Osc8Hyperlink);
    }

    // The mouse/focus-vs-ordinary-private-mode split reuses
    // `classify_private_mode_number` rather than re-testing raw mode
    // numbers here -- the same shape as the accepted-set delegation
    // above, so this file has exactly one place that names "1000, 1002,
    // 1003, 1004, 1005, 1006" (`tekstide-core`), not two.
    fn set_private_mode(&mut self, mode: PrivateMode) {
        self.block(classify_private_mode_number(mode.raw()));
    }

    fn unset_private_mode(&mut self, mode: PrivateMode) {
        self.block(classify_private_mode_number(mode.raw()));
    }

    fn report_private_mode(&mut self, mode: PrivateMode) {
        self.block(classify_private_mode_number(mode.raw()));
    }

    fn push_keyboard_mode(&mut self, _mode: KeyboardModes) {
        self.block(TerminalSequenceFamily::KeyboardProtocol);
    }

    fn pop_keyboard_modes(&mut self, _to_pop: u16) {
        self.block(TerminalSequenceFamily::KeyboardProtocol);
    }

    fn set_keyboard_mode(&mut self, _mode: KeyboardModes, _behavior: KeyboardModesApplyBehavior) {
        self.block(TerminalSequenceFamily::KeyboardProtocol);
    }

    fn report_keyboard_mode(&mut self) {
        self.block(TerminalSequenceFamily::KeyboardProtocol);
    }

    fn set_modify_other_keys(&mut self, _mode: ModifyOtherKeys) {
        self.block(TerminalSequenceFamily::KeyboardProtocol);
    }

    fn report_modify_other_keys(&mut self) {
        self.block(TerminalSequenceFamily::KeyboardProtocol);
    }

    fn identify_terminal(&mut self, _intermediate: Option<char>) {
        self.block(TerminalSequenceFamily::TerminalQuery);
    }

    fn device_status(&mut self, _arg: usize) {
        self.block(TerminalSequenceFamily::TerminalQuery);
    }

    fn report_mode(&mut self, _mode: Mode) {
        self.block(TerminalSequenceFamily::TerminalQuery);
    }

    fn text_area_size_pixels(&mut self) {
        self.block(TerminalSequenceFamily::TerminalQuery);
    }

    fn text_area_size_chars(&mut self) {
        self.block(TerminalSequenceFamily::TerminalQuery);
    }

    fn dynamic_color_sequence(&mut self, _prefix: String, _index: usize, _terminator: &str) {
        self.block(TerminalSequenceFamily::TerminalQuery);
    }

    // SCP (DECSCP) has no dedicated `TerminalSequenceFamily` variant --
    // it falls into the same generic "unsupported CSI" bucket
    // `runtime/terminal/security/parser.rs`'s own CSI fallback arm uses
    // for any final byte it does not recognize either. Not a gap: `Csi`
    // plus omission from `ACCEPTED` is itself the correct default-deny
    // answer, matching core's own precedent rather than inventing a new
    // family for one method.
    fn set_scp(&mut self, _char_path: ScpCharPath, _update_mode: ScpUpdateMode) {
        self.block(TerminalSequenceFamily::Csi);
    }

    // --- Everything else in the 71-method Handler trait is blocked by
    // omission: this impl does not override it, so the trait's own
    // default no-op body runs and `inner` is never touched. Notable
    // examples covered this way: bell, substitute, set_active_charset,
    // configure_charset, set_color/reset_color, decaln, goto/goto_line/
    // goto_col, save/restore_cursor_position, set_mode/unset_mode,
    // set_scrolling_region, insert/delete_lines, erase/delete_chars,
    // move_forward_tabs/move_backward_tabs, set_cursor_style/
    // set_cursor_shape, set_mouse_cursor_icon, reverse_index,
    // set_keypad_application_mode/unset_keypad_application_mode.
}

#[cfg(test)]
mod tests;
