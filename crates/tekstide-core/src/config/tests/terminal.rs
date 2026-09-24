//! RFC-054 PR-054-C: `[terminal] scrollback_lines`.

use crate::config::{
    DEFAULT_SCROLLBACK_LINES, FallbackReason, MAX_SCROLLBACK_LINES, SCROLLBACK_BUDGET_BYTES,
    SCROLLBACK_ROW_BLOCK, effective_scrollback_lines, parse_and_validate, scrollback_bytes,
};

fn parse(source: &str) -> crate::config::ConfigLoadOutcome {
    parse_and_validate(source).expect("a bad scrollback is a fallback, not a refusal")
}

#[test]
fn with_no_setting_the_shipped_default_stands() {
    let document = parse("").document;
    assert_eq!(document.terminal.scrollback_lines, None);
    assert_eq!(
        document.terminal.scrollback_lines(),
        DEFAULT_SCROLLBACK_LINES
    );
    assert_eq!(DEFAULT_SCROLLBACK_LINES, 2_000, "unchanged since RFC-017");
}

#[test]
fn a_scrollback_up_to_and_including_the_cap_is_kept_as_written() {
    for lines in [0, 1, 500, 2_000, MAX_SCROLLBACK_LINES] {
        let outcome = parse(&format!("[terminal]\nscrollback_lines = {lines}\n"));
        assert!(
            outcome.fallbacks.is_empty(),
            "{lines}: {:?}",
            outcome.fallbacks
        );
        assert_eq!(outcome.document.terminal.scrollback_lines(), lines);
    }
}

/// D7: above the cap the value is **reduced to it**, not honoured and not
/// dropped -- and the board is told. Ablated by removing the clamp.
#[test]
fn above_the_cap_it_is_reduced_to_the_cap_with_a_diagnostic() {
    for lines in [MAX_SCROLLBACK_LINES as i64 + 1, 50_000, i64::MAX] {
        let outcome = parse(&format!("[terminal]\nscrollback_lines = {lines}\n"));
        assert_eq!(
            outcome
                .fallbacks
                .iter()
                .map(|f| (f.setting.as_str(), f.reason))
                .collect::<Vec<_>>(),
            vec![(
                "terminal.scrollback_lines",
                FallbackReason::ScrollbackAboveCap
            )],
            "{lines}"
        );
        assert_eq!(
            outcome.document.terminal.scrollback_lines(),
            MAX_SCROLLBACK_LINES,
            "{lines}"
        );
    }
}

#[test]
fn a_scrollback_that_is_not_a_whole_number_falls_back_to_the_default() {
    for bad in ["-1", "-100000", "2000.5", "\"lots\"", "true", "[1]"] {
        let outcome = parse(&format!("[terminal]\nscrollback_lines = {bad}\n"));
        assert_eq!(
            outcome
                .fallbacks
                .iter()
                .map(|f| (f.setting.as_str(), f.reason))
                .collect::<Vec<_>>(),
            vec![("terminal.scrollback_lines", FallbackReason::NotAWholeNumber)],
            "{bad}"
        );
        assert_eq!(outcome.document.terminal.scrollback_lines, None, "{bad}");
    }
}

/// The section's other keys are exactly what they were: refused, the permanent
/// refusal first and with its own reason, even beside a valid scrollback.
#[test]
fn the_rest_of_terminal_stays_refused_and_scrollback_beside_it_does_not_rescue_it() {
    let error = parse_and_validate(
        "[terminal]\nscrollback_lines = 3000\nmultiline_paste_protection = false\n",
    )
    .unwrap_err();
    assert_eq!(error.key, "terminal.multiline_paste_protection");
    assert!(
        !error.message.contains("no effect yet"),
        "{}",
        error.message
    );

    let error = parse_and_validate("[terminal]\nscrollback_lines = 3000\nshell = \"/bin/sh\"\n")
        .unwrap_err();
    assert_eq!(error.key, "terminal.shell");
}

#[test]
fn an_empty_terminal_section_and_a_non_table_behave_as_before() {
    assert!(parse("[terminal]\n").fallbacks.is_empty());
    assert_eq!(
        parse_and_validate("terminal = 3\n").unwrap_err().message,
        "expected a table"
    );
}

/// A pane keeps what was configured, never more than the budget allows at its
/// size -- for every width from one column to 1,000 (past which the two block
/// allowance and the fixed part alone exceed the budget, with no history at all)
/// and every plausible height, by the measured cost model.
#[test]
fn the_effective_scrollback_never_exceeds_the_budget_at_any_size() {
    for columns in 1..=1_000usize {
        for screen_rows in [2usize, 24, 50, 120, 288] {
            let lines = effective_scrollback_lines(MAX_SCROLLBACK_LINES, columns, screen_rows);
            assert!(lines <= MAX_SCROLLBACK_LINES);
            assert!(
                scrollback_bytes(lines, columns, screen_rows) <= SCROLLBACK_BUDGET_BYTES,
                "{columns} x {screen_rows} keeps {lines} lines: {} bytes",
                scrollback_bytes(lines, columns, screen_rows)
            );
        }
    }
    // A smaller request is honoured wherever it fits, and the cap itself fits at
    // the widths the measurement was made for.
    assert_eq!(effective_scrollback_lines(500, 80, 24), 500);
    for columns in [80, 120, 200] {
        assert_eq!(
            effective_scrollback_lines(MAX_SCROLLBACK_LINES, columns, 50),
            MAX_SCROLLBACK_LINES,
            "{columns} columns"
        );
    }
    assert!(effective_scrollback_lines(MAX_SCROLLBACK_LINES, 400, 50) < MAX_SCROLLBACK_LINES);
    assert!(scrollback_bytes(MAX_SCROLLBACK_LINES, 200, 50) < SCROLLBACK_BUDGET_BYTES);
    assert_eq!(SCROLLBACK_ROW_BLOCK, 1_024);
}
