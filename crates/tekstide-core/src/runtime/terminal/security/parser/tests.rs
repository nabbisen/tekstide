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

/// **Found while building `transcript::reader`'s resynchronization
/// proof (RFC-011 Amendment 1, D2), not in this parser's own review.**
/// A CSI sequence truncated at a buffer's own end, with no final byte
/// (`0x40..=0x7e`) anywhere in what remains, must classify as an
/// unsupported/incomplete sequence -- not panic. `sequence.last()`
/// alone cannot distinguish "a real final byte was found" from "the
/// scan gave up and returned its own raw prefix," since a non-empty
/// slice always has a last byte; the fix checks the byte's actual
/// value is in range.
#[test]
fn a_csi_sequence_truncated_with_no_final_byte_does_not_panic() {
    let parser = TerminalSecurityParser;

    // The shortest input that reaches this path: ESC '[' with
    // nothing after -- `take_until_csi_final`'s scan window is empty
    // (`2..2`), so it falls back to returning its own 2-byte prefix
    // verbatim, whose last byte ('[', 0x5b) is not a real CSI final
    // byte.
    let effects = parser.parse(b"\x1b[");

    assert_eq!(
        effects,
        vec![blocked_sequence(
            TerminalSequenceFamily::Csi,
            TerminalPolicyReason::UnsupportedSequence,
            2,
        )],
        "a truncated CSI sequence with no final byte must report as unsupported, \
         consuming exactly the bytes present"
    );
}

/// The same property, against a longer truncated sequence (parameter
/// bytes present, still no final byte) -- confirms the fix holds
/// beyond the minimal 2-byte case above.
#[test]
fn a_longer_csi_sequence_truncated_with_no_final_byte_does_not_panic() {
    let parser = TerminalSecurityParser;

    let effects = parser.parse(b"\x1b[31");

    assert_eq!(
        effects,
        vec![blocked_sequence(
            TerminalSequenceFamily::Csi,
            TerminalPolicyReason::UnsupportedSequence,
            4,
        )]
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

#[test]
fn classify_private_mode_number_agrees_with_the_byte_based_classifier() {
    // Response 144 Required: this must call the byte-based classifier
    // directly rather than asserting the number-based one against a
    // second, hand-copied literal list -- the earlier version of this
    // test did the latter, so it stayed green when response 144's own
    // ablation (adding 1007 to one list only) desynchronised the two.
    // Sweeping every mode number in the range both classifiers can
    // reach and comparing them directly is the actual cross-check;
    // now that `classify_private_mode` delegates to
    // `classify_private_mode_number`, this also serves as a
    // regression guard against a future edit re-introducing a second
    // list.
    for mode in 0u16..=2100 {
        let body = format!("?{mode}");
        assert_eq!(
            classify_private_mode(body.as_bytes()),
            classify_private_mode_number(mode),
            "mode {mode}: byte-based and number-based classifiers disagree"
        );
    }
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
