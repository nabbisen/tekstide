//! PR-014-C: RFC-009 accepted-sequence policy interposed in front of
//! `alacritty_terminal`'s emulator, at the `vte::ansi::Handler` boundary.
//!
//! # Why this boundary, not the byte/`Perform` boundary
//!
//! `alacritty_terminal` 0.26 depends directly on `vte` 0.15's bundled `ansi`
//! module: `vte::ansi::Processor` owns the *entire* VT/ANSI grammar (the
//! same code alacritty itself is built on) and, on every `advance()` call,
//! dispatches already-fully-classified semantic operations to a
//! `vte::ansi::Handler` implementation. `Term` implements that trait.
//!
//! Interposing here rather than re-parsing bytes ourselves means:
//!
//! - **P3 (classification parity) holds by construction.** The filter and
//!   the real emulator share the identical classifier; there is no second
//!   implementation to drift out of sync with the first.
//! - **P4 (stream-position independence) holds by construction.** The
//!   `Processor` instance is long-lived and holds the VT parser state
//!   internally across `advance()` calls, so a sequence split across two PTY
//!   reads is reassembled by the *same* code the real terminal uses, before
//!   this filter ever sees it. There is no separate stateless-vs-stateful
//!   distinction to get wrong here, unlike `tekstide_core`'s
//!   `TerminalSecurityParser` (see the module doc on that type).
//!
//! What this filter is responsible for is P1 (single ingress) and P2 (no
//! side channels): every accepted operation is forwarded to the wrapped
//! [`Handler`] explicitly, one method at a time, and every other method
//! defaults to a no-op via the trait's own default body — it is never
//! forwarded, so it can never reach `Term` through this path.
//!
//! # P1/P2 callers must also uphold
//!
//! This filter only closes the path from *PTY bytes*. `Term::grid_mut()` is
//! public API for direct grid manipulation (used by alacritty's own
//! search/selection features) and is **not** reachable from the PTY byte
//! stream — `vte::ansi::Processor` never calls it. P1 therefore also
//! requires that calling code never wires PTY bytes to `grid_mut()` or any
//! other direct `Term` mutator outside `Processor::advance`. This spike's
//! terminal pane only ever calls `Processor::advance(&mut filter, bytes)`;
//! see `terminal_pane.rs`.

use alacritty_terminal::vte::ansi::{
    Attr, ClearMode, Handler, Hyperlink, KeyboardModes, KeyboardModesApplyBehavior, LineClearMode,
    Mode, ModifyOtherKeys, PrivateMode, ScpCharPath, ScpUpdateMode,
};

/// A family classification for a call this filter declined to forward.
///
/// This does not attempt to classify all 100+ blocked `Handler` methods —
/// only the families the RFC-014 handoff names as minimum required
/// corpus coverage. Everything else is blocked by omission (the trait's
/// default no-op body runs, `inner` is never touched) but is not
/// individually classified here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockedFamily {
    Title,
    Clipboard,
    Hyperlink,
    PrivateMode,
    KeyboardProtocol,
    TerminalQueryOrReply,
    Scp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockedCall {
    pub family: BlockedFamily,
}

/// Wraps a real [`Handler`] (in this spike, `alacritty_terminal::Term`),
/// forwarding only the RFC-009 accepted-sequence set and recording every
/// other call this filter intercepted.
pub struct SecurityFilter<'a, H: Handler> {
    inner: &'a mut H,
    pub blocked: Vec<BlockedCall>,
}

impl<'a, H: Handler> SecurityFilter<'a, H> {
    pub fn new(inner: &'a mut H) -> Self {
        Self {
            inner,
            blocked: Vec::new(),
        }
    }

    fn block(&mut self, family: BlockedFamily) {
        self.blocked.push(BlockedCall { family });
    }
}

impl<'a, H: Handler> Handler for SecurityFilter<'a, H> {
    // --- RFC-009 accepted set: forwarded to the real handler. ---
    //
    // Mapped from `vte::ansi::Processor`'s `execute`/`csi_dispatch` (read
    // directly from the vendored 0.15.0 source, not assumed):
    //   C0 HT   -> put_tab(1)        C0 BS -> backspace       C0 CR -> carriage_return
    //   C0 LF/VT/FF -> linefeed      CSI 'A' -> move_up       CSI 'B'/'e' -> move_down
    //   CSI 'C'/'a' -> move_forward  CSI 'D' -> move_backward CSI 'K' -> clear_line
    //   CSI 'J' -> clear_screen      CSI 'm' -> terminal_attribute (print -> input)

    fn input(&mut self, c: char) {
        self.inner.input(c);
    }

    fn carriage_return(&mut self) {
        self.inner.carriage_return();
    }

    fn linefeed(&mut self) {
        self.inner.linefeed();
    }

    fn put_tab(&mut self, count: u16) {
        self.inner.put_tab(count);
    }

    fn backspace(&mut self) {
        self.inner.backspace();
    }

    fn terminal_attribute(&mut self, attr: Attr) {
        self.inner.terminal_attribute(attr);
    }

    fn move_up(&mut self, rows: usize) {
        self.inner.move_up(rows);
    }

    fn move_down(&mut self, rows: usize) {
        self.inner.move_down(rows);
    }

    fn move_forward(&mut self, cols: usize) {
        self.inner.move_forward(cols);
    }

    fn move_backward(&mut self, cols: usize) {
        self.inner.move_backward(cols);
    }

    fn clear_line(&mut self, mode: LineClearMode) {
        self.inner.clear_line(mode);
    }

    fn clear_screen(&mut self, mode: ClearMode) {
        self.inner.clear_screen(mode);
    }

    // --- Explicitly classified blocks: the minimum corpus coverage the
    // handoff names. ---

    fn set_title(&mut self, _title: Option<String>) {
        self.block(BlockedFamily::Title);
    }

    fn push_title(&mut self) {
        self.block(BlockedFamily::Title);
    }

    fn pop_title(&mut self) {
        self.block(BlockedFamily::Title);
    }

    fn clipboard_store(&mut self, _clipboard: u8, _base64: &[u8]) {
        self.block(BlockedFamily::Clipboard);
    }

    fn clipboard_load(&mut self, _clipboard: u8, _terminator: &str) {
        self.block(BlockedFamily::Clipboard);
    }

    fn set_hyperlink(&mut self, _hyperlink: Option<Hyperlink>) {
        self.block(BlockedFamily::Hyperlink);
    }

    fn set_private_mode(&mut self, _mode: PrivateMode) {
        self.block(BlockedFamily::PrivateMode);
    }

    fn unset_private_mode(&mut self, _mode: PrivateMode) {
        self.block(BlockedFamily::PrivateMode);
    }

    fn report_private_mode(&mut self, _mode: PrivateMode) {
        self.block(BlockedFamily::PrivateMode);
    }

    fn push_keyboard_mode(&mut self, _mode: KeyboardModes) {
        self.block(BlockedFamily::KeyboardProtocol);
    }

    fn pop_keyboard_modes(&mut self, _to_pop: u16) {
        self.block(BlockedFamily::KeyboardProtocol);
    }

    fn set_keyboard_mode(&mut self, _mode: KeyboardModes, _behavior: KeyboardModesApplyBehavior) {
        self.block(BlockedFamily::KeyboardProtocol);
    }

    fn report_keyboard_mode(&mut self) {
        self.block(BlockedFamily::KeyboardProtocol);
    }

    fn set_modify_other_keys(&mut self, _mode: ModifyOtherKeys) {
        self.block(BlockedFamily::KeyboardProtocol);
    }

    fn report_modify_other_keys(&mut self) {
        self.block(BlockedFamily::KeyboardProtocol);
    }

    fn identify_terminal(&mut self, _intermediate: Option<char>) {
        self.block(BlockedFamily::TerminalQueryOrReply);
    }

    fn device_status(&mut self, _arg: usize) {
        self.block(BlockedFamily::TerminalQueryOrReply);
    }

    fn report_mode(&mut self, _mode: Mode) {
        self.block(BlockedFamily::TerminalQueryOrReply);
    }

    fn text_area_size_pixels(&mut self) {
        self.block(BlockedFamily::TerminalQueryOrReply);
    }

    fn text_area_size_chars(&mut self) {
        self.block(BlockedFamily::TerminalQueryOrReply);
    }

    fn dynamic_color_sequence(&mut self, _prefix: String, _index: usize, _terminator: &str) {
        self.block(BlockedFamily::TerminalQueryOrReply);
    }

    fn set_scp(&mut self, _char_path: ScpCharPath, _update_mode: ScpUpdateMode) {
        self.block(BlockedFamily::Scp);
    }

    // --- Everything else in the 71-method Handler trait is blocked by
    // omission: this impl does not override it, so the trait's own default
    // no-op body runs and `inner` is never called. That satisfies P1/P2 for
    // those methods without individually classifying each one. Notable
    // examples covered this way: bell, substitute, set_active_charset,
    // configure_charset, set_color/reset_color, decaln, goto/goto_line/
    // goto_col, save/restore_cursor_position, set_mode/unset_mode,
    // set_scrolling_region, insert/delete_lines, erase/delete_chars,
    // move_forward_tabs/move_backward_tabs (CSI-originated tab stops --
    // distinct from the C0-originated `put_tab` this filter allows; see
    // the module doc on `execute()` C0::HT mapping), set_cursor_style/
    // set_cursor_shape, set_mouse_cursor_icon, reverse_index,
    // set_keypad_application_mode/unset_keypad_application_mode.
}

#[cfg(test)]
mod tests;
