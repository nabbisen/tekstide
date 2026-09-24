//! RFC-055 PR-055-C: `[explorer] show_ignored`.

use crate::config::{FallbackReason, parse_and_validate};

fn parse(source: &str) -> crate::config::ConfigLoadOutcome {
    parse_and_validate(source).expect("a bad value is a fallback, not a refusal of the file")
}

#[test]
fn ignored_entries_are_not_shown_unless_the_file_says_so() {
    assert!(!parse("").document.explorer.show_ignored);
    assert!(!parse("[explorer]\n").document.explorer.show_ignored);
    assert!(
        parse("[explorer]\nshow_ignored = true\n")
            .document
            .explorer
            .show_ignored
    );
    assert!(
        !parse("[explorer]\nshow_ignored = false\n")
            .document
            .explorer
            .show_ignored
    );
}

/// A value that is not a boolean falls back to the default -- named -- and the rest
/// of the file still applies, exactly like every other setting of RFC-054's.
#[test]
fn a_value_that_is_not_a_boolean_is_named_and_the_default_stands() {
    for bad in ["\"true\"", "1", "\"yes\"", "[true]", "1.5"] {
        let outcome = parse(&format!(
            "[explorer]\nshow_ignored = {bad}\n[terminal]\nscrollback_lines = 700\n"
        ));
        assert_eq!(
            outcome
                .fallbacks
                .iter()
                .map(|f| (f.setting.as_str(), f.reason))
                .collect::<Vec<_>>(),
            vec![("explorer.show_ignored", FallbackReason::NotABoolean)],
            "{bad}"
        );
        assert!(!outcome.document.explorer.show_ignored, "{bad}");
        assert_eq!(
            outcome.document.terminal.scrollback_lines(),
            700,
            "{bad}: the rest of the file applied"
        );
    }
}

/// Dotfiles are not this setting's business (D7): there is no key for them, and a
/// guess at one is a warning, not a setting.
#[test]
fn there_is_no_key_for_dotfiles_and_an_unknown_key_only_warns() {
    let outcome = parse("[explorer]\nshow_hidden = true\nshow_dotfiles = true\n");
    let keys: Vec<&str> = outcome.warnings.iter().map(|w| w.key.as_str()).collect();
    assert_eq!(keys, ["explorer.show_dotfiles", "explorer.show_hidden"]);
    assert!(!outcome.document.explorer.show_ignored);
}

#[test]
fn an_explorer_section_that_is_not_a_table_refuses_the_file() {
    assert_eq!(
        parse_and_validate("explorer = true\n").unwrap_err().message,
        "expected a table"
    );
}
