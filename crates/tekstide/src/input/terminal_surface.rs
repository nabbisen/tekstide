//! RFC-015 PR-015-C built `TextStream`'s privacy boundary ahead of any
//! real terminal surface to receive it. RFC-017 PR-017-D is that real
//! caller: [`TextStream::to_pty_bytes`] is the one place a routed
//! keystroke becomes the bytes a PTY actually receives.
//!
//! **`TextStream::from_terminal_key` is `pub(super)`, not `pub(crate)`.**
//! `pub(crate)` would let *any* module in this crate construct a
//! `TextStream` -- including a future `crates/tekstide/src/surface/*`
//! module, which is exactly the bypass this type exists to prevent.
//! `pub(super)` restricts construction to `super::input` (the router,
//! which legitimately hands a key event to the terminal surface once it
//! has decided a terminal owns the input sink) and `input`'s other
//! descendants (this module's own `#[cfg(test)]` code); nothing outside
//! `input` -- `shell.rs`, `main.rs`, or any surface module -- can reach
//! it, proven in `input::tests`.
//!
//! **`to_pty_bytes` does not weaken that boundary.** It converts an
//! already-routed key into bytes; it does not hand out the `KeyPress`
//! itself, and it has no path to shell or modal state -- the same
//! "cannot address trusted state" property the type has always had,
//! extended to its one new capability.
//!
//! **Not a complete VT100/xterm input encoder, disclosed rather than
//! implied complete.** Printable characters, Enter, Backspace, Escape,
//! Tab, Space, the four arrow keys (normal-mode `CSI` sequences, not
//! application-cursor-mode), and `Ctrl`+ASCII-letter control codes are
//! handled. Everything else (function keys, `Alt`-prefixed meta
//! sequences, `Ctrl` combined with punctuation) returns `None` -- silently
//! dropped, not corrupted -- and is a known limitation, not a claim of
//! completeness this slice does not back.

use tekstide_core::domain::TerminalId;

use super::KeyPress;

/// Keystrokes destined for a PTY. Carries the target terminal so the
/// router (which does have access to `ApplicationShell` state) can
/// verify it names a live terminal in the active project before
/// delivering it -- a stale or cross-project id must be dropped, not
/// best-effort delivered.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStream {
    target: TerminalId,
    key: KeyPress,
}

impl TextStream {
    pub fn target(&self) -> &TerminalId {
        &self.target
    }

    /// The one and only constructor. See the module doc for why this is
    /// `pub(super)` rather than `pub(crate)`.
    pub(super) fn from_terminal_key(target: TerminalId, key: KeyPress) -> Self {
        Self { target, key }
    }

    /// The PTY bytes this keystroke represents, or `None` if this key
    /// has no encoding this crate implements (see the module doc).
    /// Returns owned bytes rather than a borrowed `KeyPress` reference --
    /// the caller needs bytes to write, never the key itself.
    pub fn to_pty_bytes(&self) -> Option<Vec<u8>> {
        key_to_pty_bytes(&self.key)
    }
}

fn key_to_pty_bytes(press: &KeyPress) -> Option<Vec<u8>> {
    use iced::keyboard::Key;
    use iced::keyboard::key::Named;

    if let Key::Character(character) = &press.key {
        if press.modifiers.control() {
            return control_code_for(character);
        }
        return Some(character.as_bytes().to_vec());
    }

    let Key::Named(named) = press.key else {
        return None;
    };

    match named {
        Named::Enter => Some(vec![b'\r']),
        Named::Backspace => Some(vec![0x7f]),
        Named::Escape => Some(vec![0x1b]),
        Named::Tab => Some(vec![b'\t']),
        Named::Space => Some(vec![b' ']),
        Named::ArrowUp => Some(b"\x1b[A".to_vec()),
        Named::ArrowDown => Some(b"\x1b[B".to_vec()),
        Named::ArrowRight => Some(b"\x1b[C".to_vec()),
        Named::ArrowLeft => Some(b"\x1b[D".to_vec()),
        _ => None,
    }
}

/// `Ctrl`+ASCII-letter only (`Ctrl+C`, `Ctrl+D`, `Ctrl+L`, and so on --
/// the combinations a real shell session actually depends on for
/// signals and line editing). `Ctrl` combined with punctuation or a
/// non-ASCII character is left unencoded (`None`) rather than guessed
/// at.
fn control_code_for(character: &str) -> Option<Vec<u8>> {
    let mut chars = character.chars();
    let c = chars.next()?;
    if chars.next().is_some() || !c.is_ascii_alphabetic() {
        return None;
    }
    Some(vec![(c.to_ascii_uppercase() as u8) & 0x1f])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(key: iced::keyboard::Key, modifiers: iced::keyboard::Modifiers) -> TextStream {
        TextStream::from_terminal_key(TerminalId::new_uuid(), KeyPress { key, modifiers })
    }

    fn no_modifiers() -> iced::keyboard::Modifiers {
        iced::keyboard::Modifiers::empty()
    }

    #[test]
    fn a_plain_character_encodes_as_its_utf8_bytes() {
        let s = stream(iced::keyboard::Key::Character("é".into()), no_modifiers());
        assert_eq!(s.to_pty_bytes(), Some("é".as_bytes().to_vec()));
    }

    #[test]
    fn enter_encodes_as_carriage_return() {
        let s = stream(
            iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
            no_modifiers(),
        );
        assert_eq!(s.to_pty_bytes(), Some(vec![b'\r']));
    }

    #[test]
    fn backspace_encodes_as_del() {
        let s = stream(
            iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace),
            no_modifiers(),
        );
        assert_eq!(s.to_pty_bytes(), Some(vec![0x7f]));
    }

    #[test]
    fn arrow_keys_encode_as_normal_mode_csi_sequences() {
        use iced::keyboard::key::Named;
        let cases = [
            (Named::ArrowUp, &b"\x1b[A"[..]),
            (Named::ArrowDown, &b"\x1b[B"[..]),
            (Named::ArrowRight, &b"\x1b[C"[..]),
            (Named::ArrowLeft, &b"\x1b[D"[..]),
        ];
        for (named, expected) in cases {
            let s = stream(iced::keyboard::Key::Named(named), no_modifiers());
            assert_eq!(s.to_pty_bytes(), Some(expected.to_vec()), "{named:?}");
        }
    }

    #[test]
    fn ctrl_plus_ascii_letter_encodes_as_the_control_code() {
        let s = stream(
            iced::keyboard::Key::Character("c".into()),
            iced::keyboard::Modifiers::CTRL,
        );
        assert_eq!(
            s.to_pty_bytes(),
            Some(vec![0x03]),
            "Ctrl+C must be ETX (0x03)"
        );

        let s = stream(
            iced::keyboard::Key::Character("d".into()),
            iced::keyboard::Modifiers::CTRL,
        );
        assert_eq!(
            s.to_pty_bytes(),
            Some(vec![0x04]),
            "Ctrl+D must be EOT (0x04)"
        );
    }

    /// `Ctrl` combined with anything other than a single ASCII letter is
    /// a known, disclosed gap (module doc), not a guess -- proven here
    /// so a future encoding attempt for one of these is a deliberate
    /// change to this test, not a silent behaviour shift.
    #[test]
    fn ctrl_plus_punctuation_or_non_ascii_is_not_encoded() {
        for character in ["[", "1", "é"] {
            let s = stream(
                iced::keyboard::Key::Character(character.into()),
                iced::keyboard::Modifiers::CTRL,
            );
            assert_eq!(
                s.to_pty_bytes(),
                None,
                "Ctrl+{character:?} must not be encoded"
            );
        }
    }

    #[test]
    fn an_unencoded_named_key_returns_none() {
        let s = stream(
            iced::keyboard::Key::Named(iced::keyboard::key::Named::F1),
            no_modifiers(),
        );
        assert_eq!(s.to_pty_bytes(), None);
    }
}
