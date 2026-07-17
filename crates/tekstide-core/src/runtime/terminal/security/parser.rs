use super::{
    TerminalBlockedAppEffect, TerminalCursorEffect, TerminalPolicyReason, TerminalScrollbackEffect,
    TerminalSecurityDiagnostic, TerminalSequenceFamily, TerminalStyleEffect, TerminalSurfaceEffect,
    TerminalTextEffect,
};

const MAX_CONTROL_SEQUENCE_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalAcceptedSequence {
    PrintableUtf8,
    C0CarriageReturn,
    C0LineFeed,
    C0Tab,
    C0Backspace,
    CsiSgr,
    CsiCursorMovement,
    CsiClearLine,
    CsiClearScreen,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalInertSequence {
    InvalidUtf8,
    C0Unsupported,
    C1Control,
    Osc52Clipboard,
    Osc8Hyperlink,
    OscTitle,
    OscUnsupported,
    Dcs,
    Pm,
    Apc,
    PrivateMode,
    MouseFocusReporting,
    KeyboardProtocol,
    TerminalQuery,
    TerminalGeneratedReply,
    CsiUnsupported,
    UnknownEsc,
}

pub struct TerminalSequencePolicy;

impl TerminalSequencePolicy {
    pub const ACCEPTED: &'static [TerminalAcceptedSequence] = &[
        TerminalAcceptedSequence::PrintableUtf8,
        TerminalAcceptedSequence::C0CarriageReturn,
        TerminalAcceptedSequence::C0LineFeed,
        TerminalAcceptedSequence::C0Tab,
        TerminalAcceptedSequence::C0Backspace,
        TerminalAcceptedSequence::CsiSgr,
        TerminalAcceptedSequence::CsiCursorMovement,
        TerminalAcceptedSequence::CsiClearLine,
        TerminalAcceptedSequence::CsiClearScreen,
    ];

    pub const INERT: &'static [TerminalInertSequence] = &[
        TerminalInertSequence::InvalidUtf8,
        TerminalInertSequence::C0Unsupported,
        TerminalInertSequence::C1Control,
        TerminalInertSequence::Osc52Clipboard,
        TerminalInertSequence::Osc8Hyperlink,
        TerminalInertSequence::OscTitle,
        TerminalInertSequence::OscUnsupported,
        TerminalInertSequence::Dcs,
        TerminalInertSequence::Pm,
        TerminalInertSequence::Apc,
        TerminalInertSequence::PrivateMode,
        TerminalInertSequence::MouseFocusReporting,
        TerminalInertSequence::KeyboardProtocol,
        TerminalInertSequence::TerminalQuery,
        TerminalInertSequence::TerminalGeneratedReply,
        TerminalInertSequence::CsiUnsupported,
        TerminalInertSequence::UnknownEsc,
    ];
}

#[derive(Default)]
pub struct TerminalSecurityParser;

impl TerminalSecurityParser {
    pub fn parse(&self, input: &[u8]) -> Vec<TerminalSurfaceEffect> {
        let mut effects = Vec::new();
        let mut index = 0;

        while index < input.len() {
            match input[index] {
                b'\r' => {
                    effects.push(TerminalSurfaceEffect::Cursor(
                        TerminalCursorEffect::CarriageReturn,
                    ));
                    index += 1;
                }
                b'\n' => {
                    effects.push(TerminalSurfaceEffect::Cursor(
                        TerminalCursorEffect::LineFeed,
                    ));
                    index += 1;
                }
                b'\t' => {
                    effects.push(TerminalSurfaceEffect::Cursor(TerminalCursorEffect::Tab));
                    index += 1;
                }
                0x08 => {
                    effects.push(TerminalSurfaceEffect::Cursor(
                        TerminalCursorEffect::Backspace,
                    ));
                    index += 1;
                }
                0x1b => {
                    let (effect, consumed) = parse_escape_sequence(&input[index..]);
                    effects.push(effect);
                    index += consumed;
                }
                0x90 => {
                    let (effect, consumed) =
                        parse_string_control(&input[index..], TerminalSequenceFamily::Dcs);
                    effects.push(effect);
                    index += consumed;
                }
                0x9d => {
                    let (effect, consumed) = parse_osc(&input[index..]);
                    effects.push(effect);
                    index += consumed;
                }
                0x9e => {
                    let (effect, consumed) =
                        parse_string_control(&input[index..], TerminalSequenceFamily::Pm);
                    effects.push(effect);
                    index += consumed;
                }
                0x9f => {
                    let (effect, consumed) =
                        parse_string_control(&input[index..], TerminalSequenceFamily::Apc);
                    effects.push(effect);
                    index += consumed;
                }
                0x00..=0x1f | 0x7f => {
                    effects.push(blocked_sequence(
                        TerminalSequenceFamily::C0Control,
                        TerminalPolicyReason::UnsupportedSequence,
                        1,
                    ));
                    index += 1;
                }
                0x80..=0x9f => {
                    effects.push(blocked_sequence(
                        TerminalSequenceFamily::C1Control,
                        TerminalPolicyReason::UnsupportedSequence,
                        1,
                    ));
                    index += 1;
                }
                byte if byte.is_ascii() => {
                    let start = index;
                    while index < input.len()
                        && input[index].is_ascii()
                        && !input[index].is_ascii_control()
                    {
                        index += 1;
                    }
                    effects.push(TerminalSurfaceEffect::Text(TerminalTextEffect::Printable {
                        chars: index - start,
                    }));
                }
                _ => {
                    if let Some((character, consumed)) = parse_utf8_char(&input[index..]) {
                        effects.push(TerminalSurfaceEffect::Text(TerminalTextEffect::Printable {
                            chars: usize::from(character != '\0'),
                        }));
                        index += consumed;
                    } else {
                        effects.push(TerminalSurfaceEffect::Text(
                            TerminalTextEffect::InvalidBytesReplaced { bytes: 1 },
                        ));
                        effects.push(blocked_sequence(
                            TerminalSequenceFamily::PrintableText,
                            TerminalPolicyReason::InvalidBytes,
                            1,
                        ));
                        index += 1;
                    }
                }
            }
        }

        effects
    }

    pub fn block_terminal_generated_reply(payload_bytes: usize) -> TerminalSurfaceEffect {
        blocked_sequence(
            TerminalSequenceFamily::TerminalGeneratedReply,
            TerminalPolicyReason::TerminalGeneratedReplyBlocked,
            payload_bytes,
        )
    }
}

fn parse_escape_sequence(input: &[u8]) -> (TerminalSurfaceEffect, usize) {
    debug_assert_eq!(input.first(), Some(&0x1b));

    match input.get(1).copied() {
        Some(b'[') => parse_csi(input),
        Some(b']') => parse_osc(input),
        Some(b'P') => parse_string_control(input, TerminalSequenceFamily::Dcs),
        Some(b'^') => parse_string_control(input, TerminalSequenceFamily::Pm),
        Some(b'_') => parse_string_control(input, TerminalSequenceFamily::Apc),
        Some(_) => (
            blocked_sequence(
                TerminalSequenceFamily::UnknownControl,
                TerminalPolicyReason::UnsupportedSequence,
                2.min(input.len()),
            ),
            2.min(input.len()),
        ),
        None => (
            blocked_sequence(
                TerminalSequenceFamily::UnknownControl,
                TerminalPolicyReason::UnsupportedSequence,
                1,
            ),
            1,
        ),
    }
}

fn parse_csi(input: &[u8]) -> (TerminalSurfaceEffect, usize) {
    let (sequence, consumed) = take_until_csi_final(input);
    let Some(final_byte) = sequence.last().copied() else {
        return (
            blocked_sequence(
                TerminalSequenceFamily::Csi,
                TerminalPolicyReason::UnsupportedSequence,
                consumed,
            ),
            consumed,
        );
    };

    let body = &sequence[2..sequence.len().saturating_sub(1)];

    if is_keyboard_protocol(body, final_byte) {
        return (
            blocked_sequence(
                TerminalSequenceFamily::KeyboardProtocol,
                TerminalPolicyReason::HostIntegrationBlocked,
                consumed,
            ),
            consumed,
        );
    }

    if is_private_mode(body) {
        return (
            blocked_sequence(
                classify_private_mode(body),
                TerminalPolicyReason::PrivateModeBlocked,
                consumed,
            ),
            consumed,
        );
    }

    match final_byte {
        b'm' if params_are_numeric(body) => (
            TerminalSurfaceEffect::Style(TerminalStyleEffect::SelectGraphicRendition),
            consumed,
        ),
        b'A' | b'B' | b'C' | b'D' if params_are_numeric(body) => {
            (cursor_movement_effect(final_byte, body), consumed)
        }
        b'K' if params_are_numeric(body) => (
            TerminalSurfaceEffect::Scrollback(TerminalScrollbackEffect::ClearLine),
            consumed,
        ),
        b'J' if params_are_numeric(body) => (
            TerminalSurfaceEffect::Scrollback(TerminalScrollbackEffect::ClearScreen),
            consumed,
        ),
        b'n' | b'c' => (
            blocked_sequence(
                TerminalSequenceFamily::TerminalQuery,
                TerminalPolicyReason::TerminalGeneratedReplyBlocked,
                consumed,
            ),
            consumed,
        ),
        _ => (
            blocked_sequence(
                TerminalSequenceFamily::Csi,
                TerminalPolicyReason::UnsupportedSequence,
                consumed,
            ),
            consumed,
        ),
    }
}

fn parse_osc(input: &[u8]) -> (TerminalSurfaceEffect, usize) {
    let (sequence, consumed) = take_until_string_terminator(input);
    let body = strip_osc_wrapper(sequence);

    let family = if body.starts_with(b"52;") {
        TerminalSequenceFamily::Osc52Clipboard
    } else if body.starts_with(b"8;") {
        TerminalSequenceFamily::Osc8Hyperlink
    } else if body.starts_with(b"0;") || body.starts_with(b"1;") || body.starts_with(b"2;") {
        TerminalSequenceFamily::OscTitle
    } else {
        TerminalSequenceFamily::UnknownControl
    };

    let blocked_effect = match family {
        TerminalSequenceFamily::Osc52Clipboard => TerminalBlockedAppEffect::ClipboardAccess,
        TerminalSequenceFamily::OscTitle => TerminalBlockedAppEffect::AppChromeMutation,
        TerminalSequenceFamily::Osc8Hyperlink | TerminalSequenceFamily::UnknownControl => {
            TerminalBlockedAppEffect::HostIntegration
        }
        _ => TerminalBlockedAppEffect::HostIntegration,
    };

    (
        TerminalSurfaceEffect::diagnostic(TerminalSecurityDiagnostic::blocked_app_effect(
            family,
            blocked_effect,
            consumed,
        )),
        consumed,
    )
}

fn parse_string_control(
    input: &[u8],
    family: TerminalSequenceFamily,
) -> (TerminalSurfaceEffect, usize) {
    let (_, consumed) = take_until_string_terminator(input);
    (
        blocked_sequence(
            family,
            TerminalPolicyReason::HostIntegrationBlocked,
            consumed,
        ),
        consumed,
    )
}

fn take_until_csi_final(input: &[u8]) -> (&[u8], usize) {
    let limit = input.len().min(MAX_CONTROL_SEQUENCE_BYTES);
    for index in 2..limit {
        if (0x40..=0x7e).contains(&input[index]) {
            return (&input[..=index], index + 1);
        }
    }
    (&input[..limit], limit.max(1))
}

fn take_until_string_terminator(input: &[u8]) -> (&[u8], usize) {
    let mut index = string_control_prefix_len(input).unwrap_or(1);
    while index < input.len() {
        if input[index] == 0x07 {
            return (&input[..=index], index + 1);
        }
        if input[index] == 0x1b && input.get(index + 1) == Some(&b'\\') {
            return (&input[..=index + 1], index + 2);
        }
        index += 1;
    }
    (input, input.len().max(1))
}

fn string_control_prefix_len(input: &[u8]) -> Option<usize> {
    match input {
        [0x1b, b']', ..] | [0x1b, b'P', ..] | [0x1b, b'^', ..] | [0x1b, b'_', ..] => Some(2),
        [0x90, ..] | [0x9d, ..] | [0x9e, ..] | [0x9f, ..] => Some(1),
        _ => None,
    }
}

fn strip_osc_wrapper(sequence: &[u8]) -> &[u8] {
    let body = sequence
        .strip_prefix(&[0x1b, b']'])
        .or_else(|| sequence.strip_prefix(&[0x9d]))
        .unwrap_or(sequence);

    if let Some(stripped) = body.strip_suffix(&[0x07]) {
        stripped
    } else if let Some(stripped) = body.strip_suffix(&[0x1b, b'\\']) {
        stripped
    } else {
        body
    }
}

fn params_are_numeric(params: &[u8]) -> bool {
    params
        .iter()
        .all(|byte| byte.is_ascii_digit() || matches!(byte, b';' | b':' | b' '))
}

fn first_param_or_default(params: &[u8], default: i16) -> i16 {
    let first = params
        .split(|byte| *byte == b';')
        .next()
        .unwrap_or_default();
    if first.is_empty() {
        return default;
    }
    std::str::from_utf8(first)
        .ok()
        .and_then(|text| text.trim().parse::<i16>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn cursor_movement_effect(final_byte: u8, params: &[u8]) -> TerminalSurfaceEffect {
    let amount = first_param_or_default(params, 1);
    let effect = match final_byte {
        b'A' => TerminalCursorEffect::Move {
            rows: -amount,
            cols: 0,
        },
        b'B' => TerminalCursorEffect::Move {
            rows: amount,
            cols: 0,
        },
        b'C' => TerminalCursorEffect::Move {
            rows: 0,
            cols: amount,
        },
        b'D' => TerminalCursorEffect::Move {
            rows: 0,
            cols: -amount,
        },
        _ => unreachable!("caller only passes cursor movement final bytes"),
    };
    TerminalSurfaceEffect::Cursor(effect)
}

fn is_private_mode(body: &[u8]) -> bool {
    body.first()
        .is_some_and(|byte| matches!(*byte, b'?' | b'>' | b'='))
}

fn classify_private_mode(body: &[u8]) -> TerminalSequenceFamily {
    if body.windows(4).any(|window| {
        matches!(
            window,
            b"1000" | b"1002" | b"1003" | b"1004" | b"1005" | b"1006"
        )
    }) {
        TerminalSequenceFamily::MouseFocusReporting
    } else {
        TerminalSequenceFamily::PrivateMode
    }
}

fn is_keyboard_protocol(body: &[u8], final_byte: u8) -> bool {
    final_byte == b'u' || body.starts_with(b">")
}

fn parse_utf8_char(input: &[u8]) -> Option<(char, usize)> {
    let width = utf8_char_width(input.first().copied()?);
    if width == 0 || input.len() < width {
        return None;
    }
    let text = std::str::from_utf8(&input[..width]).ok()?;
    let character = text.chars().next()?;
    if character.is_control() {
        None
    } else {
        Some((character, width))
    }
}

fn utf8_char_width(byte: u8) -> usize {
    match byte {
        0x00..=0x7f => 1,
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => 0,
    }
}

fn blocked_sequence(
    sequence_family: TerminalSequenceFamily,
    policy_reason: TerminalPolicyReason,
    payload_bytes: usize,
) -> TerminalSurfaceEffect {
    TerminalSurfaceEffect::diagnostic(TerminalSecurityDiagnostic::blocked_sequence(
        sequence_family,
        policy_reason,
        payload_bytes,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_enumerates_exact_accepted_and_inert_sequence_sets() {
        assert_eq!(
            TerminalSequencePolicy::ACCEPTED,
            &[
                TerminalAcceptedSequence::PrintableUtf8,
                TerminalAcceptedSequence::C0CarriageReturn,
                TerminalAcceptedSequence::C0LineFeed,
                TerminalAcceptedSequence::C0Tab,
                TerminalAcceptedSequence::C0Backspace,
                TerminalAcceptedSequence::CsiSgr,
                TerminalAcceptedSequence::CsiCursorMovement,
                TerminalAcceptedSequence::CsiClearLine,
                TerminalAcceptedSequence::CsiClearScreen,
            ]
        );
        assert_eq!(
            TerminalSequencePolicy::INERT,
            &[
                TerminalInertSequence::InvalidUtf8,
                TerminalInertSequence::C0Unsupported,
                TerminalInertSequence::C1Control,
                TerminalInertSequence::Osc52Clipboard,
                TerminalInertSequence::Osc8Hyperlink,
                TerminalInertSequence::OscTitle,
                TerminalInertSequence::OscUnsupported,
                TerminalInertSequence::Dcs,
                TerminalInertSequence::Pm,
                TerminalInertSequence::Apc,
                TerminalInertSequence::PrivateMode,
                TerminalInertSequence::MouseFocusReporting,
                TerminalInertSequence::KeyboardProtocol,
                TerminalInertSequence::TerminalQuery,
                TerminalInertSequence::TerminalGeneratedReply,
                TerminalInertSequence::CsiUnsupported,
                TerminalInertSequence::UnknownEsc,
            ]
        );
    }

    #[test]
    fn parser_accepts_printable_text_basic_controls_sgr_cursor_and_clear() {
        let parser = TerminalSecurityParser;
        let effects = parser.parse(b"ok\r\n\t\x08\x1b[31m\x1b[2A\x1b[K\x1b[2J");

        assert!(effects.contains(&TerminalSurfaceEffect::Text(
            TerminalTextEffect::Printable { chars: 2 }
        )));
        assert!(effects.contains(&TerminalSurfaceEffect::Cursor(
            TerminalCursorEffect::CarriageReturn,
        )));
        assert!(effects.contains(&TerminalSurfaceEffect::Cursor(
            TerminalCursorEffect::LineFeed,
        )));
        assert!(effects.contains(&TerminalSurfaceEffect::Cursor(TerminalCursorEffect::Tab)));
        assert!(effects.contains(&TerminalSurfaceEffect::Cursor(
            TerminalCursorEffect::Backspace,
        )));
        assert!(effects.contains(&TerminalSurfaceEffect::Style(
            TerminalStyleEffect::SelectGraphicRendition,
        )));
        assert!(
            effects.contains(&TerminalSurfaceEffect::Cursor(TerminalCursorEffect::Move {
                rows: -2,
                cols: 0
            },))
        );
        assert!(effects.contains(&TerminalSurfaceEffect::Scrollback(
            TerminalScrollbackEffect::ClearLine,
        )));
        assert!(effects.contains(&TerminalSurfaceEffect::Scrollback(
            TerminalScrollbackEffect::ClearScreen,
        )));
    }

    #[test]
    fn parser_replaces_invalid_utf8_without_storing_payload() {
        let parser = TerminalSecurityParser;
        let effects = parser.parse(&[0xff]);

        assert!(effects.contains(&TerminalSurfaceEffect::Text(
            TerminalTextEffect::InvalidBytesReplaced { bytes: 1 },
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            TerminalSurfaceEffect::Diagnostic(diagnostic)
                if diagnostic.sequence_family == TerminalSequenceFamily::PrintableText
                    && diagnostic.policy_reason == TerminalPolicyReason::InvalidBytes
                    && !diagnostic.summary().as_str().contains("255")
        )));
    }

    #[test]
    fn parser_blocks_osc_52_without_payload_text() {
        let parser = TerminalSecurityParser;
        let payload = b"\x1b]52;c;SECRET_TOKEN=must-not-appear\x07";
        let effects = parser.parse(payload);

        assert!(effects.iter().any(|effect| matches!(
            effect,
            TerminalSurfaceEffect::Diagnostic(diagnostic)
                if diagnostic.sequence_family == TerminalSequenceFamily::Osc52Clipboard
                    && diagnostic.policy_reason == TerminalPolicyReason::ClipboardAccessBlocked
                    && diagnostic.payload_bytes == payload.len()
                    && !diagnostic.summary().as_str().contains("SECRET_TOKEN")
        )));
    }

    #[test]
    fn parser_keeps_c1_string_control_payloads_inert() {
        let parser = TerminalSecurityParser;
        let effects =
            parser.parse(b"\x9d52;c;SECRET\x07\x90private\x1b\\\x9eprivacy\x1b\\\x9fapp\x1b\\");

        assert_eq!(printable_chars(&effects), 0);
        assert!(has_diagnostic(
            &effects,
            TerminalSequenceFamily::Osc52Clipboard,
            TerminalPolicyReason::ClipboardAccessBlocked
        ));
        assert!(has_diagnostic(
            &effects,
            TerminalSequenceFamily::Dcs,
            TerminalPolicyReason::HostIntegrationBlocked
        ));
        assert!(has_diagnostic(
            &effects,
            TerminalSequenceFamily::Pm,
            TerminalPolicyReason::HostIntegrationBlocked
        ));
        assert!(has_diagnostic(
            &effects,
            TerminalSequenceFamily::Apc,
            TerminalPolicyReason::HostIntegrationBlocked
        ));
    }

    #[test]
    fn parser_keeps_over_cap_terminated_string_control_payload_inert() {
        let parser = TerminalSecurityParser;
        let mut payload = b"\x1b]52;c;".to_vec();
        payload.extend(std::iter::repeat_n(b'A', MAX_CONTROL_SEQUENCE_BYTES + 17));
        payload.push(0x07);
        payload.extend_from_slice(b"after");

        let effects = parser.parse(&payload);

        assert!(has_diagnostic(
            &effects,
            TerminalSequenceFamily::Osc52Clipboard,
            TerminalPolicyReason::ClipboardAccessBlocked
        ));
        assert_eq!(printable_chars(&effects), "after".len());
    }

    #[test]
    fn parser_keeps_over_cap_unterminated_string_control_payload_inert() {
        let parser = TerminalSecurityParser;
        let mut payload = b"\x1bP".to_vec();
        payload.extend(std::iter::repeat_n(b'B', MAX_CONTROL_SEQUENCE_BYTES + 17));

        let effects = parser.parse(&payload);

        assert!(has_diagnostic(
            &effects,
            TerminalSequenceFamily::Dcs,
            TerminalPolicyReason::HostIntegrationBlocked
        ));
        assert_eq!(printable_chars(&effects), 0);
    }

    #[test]
    fn parser_blocks_title_hyperlink_and_host_control_sequences() {
        let parser = TerminalSecurityParser;
        let effects = parser.parse(
            b"\x1b]0;title\x07\x1b]8;;https://example.invalid\x07link\x1b]8;;\x07\x1bP1$rq\x1b\\\x1b^privacy\x1b\\\x1b_apc\x1b\\",
        );

        assert!(has_diagnostic(
            &effects,
            TerminalSequenceFamily::OscTitle,
            TerminalPolicyReason::AppChromeMutationBlocked
        ));
        assert!(has_diagnostic(
            &effects,
            TerminalSequenceFamily::Osc8Hyperlink,
            TerminalPolicyReason::HostIntegrationBlocked
        ));
        assert!(has_diagnostic(
            &effects,
            TerminalSequenceFamily::Dcs,
            TerminalPolicyReason::HostIntegrationBlocked
        ));
        assert!(has_diagnostic(
            &effects,
            TerminalSequenceFamily::Pm,
            TerminalPolicyReason::HostIntegrationBlocked
        ));
        assert!(has_diagnostic(
            &effects,
            TerminalSequenceFamily::Apc,
            TerminalPolicyReason::HostIntegrationBlocked
        ));
    }

    #[test]
    fn parser_blocks_private_modes_mouse_focus_keyboard_and_queries() {
        let parser = TerminalSecurityParser;
        let effects = parser.parse(b"\x1b[?25h\x1b[?1004h\x1b[>4;1m\x1b[6n\x1b[c");

        assert!(has_diagnostic(
            &effects,
            TerminalSequenceFamily::PrivateMode,
            TerminalPolicyReason::PrivateModeBlocked
        ));
        assert!(has_diagnostic(
            &effects,
            TerminalSequenceFamily::MouseFocusReporting,
            TerminalPolicyReason::PrivateModeBlocked
        ));
        assert!(has_diagnostic(
            &effects,
            TerminalSequenceFamily::KeyboardProtocol,
            TerminalPolicyReason::HostIntegrationBlocked
        ));
        assert!(has_diagnostic(
            &effects,
            TerminalSequenceFamily::TerminalQuery,
            TerminalPolicyReason::TerminalGeneratedReplyBlocked
        ));
    }

    #[test]
    fn terminal_generated_replies_are_blocked_by_default() {
        let effect = TerminalSecurityParser::block_terminal_generated_reply(12);

        assert!(matches!(
            effect,
            TerminalSurfaceEffect::Diagnostic(diagnostic)
                if diagnostic.sequence_family == TerminalSequenceFamily::TerminalGeneratedReply
                    && diagnostic.policy_reason == TerminalPolicyReason::TerminalGeneratedReplyBlocked
                    && diagnostic.payload_bytes == 12
        ));
    }

    fn has_diagnostic(
        effects: &[TerminalSurfaceEffect],
        sequence_family: TerminalSequenceFamily,
        policy_reason: TerminalPolicyReason,
    ) -> bool {
        effects.iter().any(|effect| {
            matches!(
                effect,
                TerminalSurfaceEffect::Diagnostic(diagnostic)
                    if diagnostic.sequence_family == sequence_family
                        && diagnostic.policy_reason == policy_reason
            )
        })
    }

    fn printable_chars(effects: &[TerminalSurfaceEffect]) -> usize {
        effects
            .iter()
            .map(|effect| match effect {
                TerminalSurfaceEffect::Text(TerminalTextEffect::Printable { chars }) => *chars,
                _ => 0,
            })
            .sum()
    }
}
