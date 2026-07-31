use std::path::{Path, PathBuf};

use tekstide_core::shell::ApplicationShell;

use super::{State, status_bar_summary};
use crate::i18n::{Catalog, LocalePreference};

fn real_locales_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("locales")
}

fn state_with(app_shell: ApplicationShell) -> State {
    let catalog = Catalog::resolve(LocalePreference::default(), Some(&real_locales_dir()));
    State::new(app_shell, catalog)
}

/// The window title comes from the catalog, not a literal -- if the key
/// name were ever mistyped, `Catalog::get`'s "missing key renders as the
/// key itself" fallback would make this assertion fail loudly rather
/// than silently rendering the wrong (but plausible-looking) string.
#[test]
fn window_title_resolves_through_the_catalog_not_a_literal() {
    let state = state_with(ApplicationShell::new());
    assert_eq!(state.window_title(), "Tekstide");
}

/// Fluent's automatic bidi isolation (First Strong Isolate / Pop
/// Directional Isolate, `use_isolating: true` by default -- already
/// documented and asserted in `i18n.rs`'s own tests) wraps each of
/// `status-bar-summary`'s two select-expression placeables, and the
/// inner `{$count}` placeable gets isolated a second time nested inside
/// the outer one -- exactly the accepted, harmless double isolation
/// response 125 ruled on for `CatalogArgs::untrusted`, here arising from
/// two adjacent select expressions instead. Expected literally rather
/// than stripped, matching `i18n::tests`' own convention of asserting
/// isolate marks explicitly so they are a documented property, not a
/// surprise the next reader has to rediscover from a failing comparison.
const ISOLATE_START: &str = "\u{2068}";
const ISOLATE_END: &str = "\u{2069}";

/// No projects, default route: the two-part summary (route label, then
/// a pluralized count) must show the Project Board route and the
/// zero-projects English plural form -- both resolved through real
/// shipped keys, not hardcoded shell text.
#[test]
fn status_bar_summary_reflects_the_default_route_and_zero_projects() {
    let state = state_with(ApplicationShell::new());
    assert_eq!(
        status_bar_summary(&state),
        format!(
            "{ISOLATE_START}Project Board{ISOLATE_END} | \
             {ISOLATE_START}{ISOLATE_START}0{ISOLATE_END} projects{ISOLATE_END}"
        )
    );
}

/// The English plural boundary this module's own key selection must
/// respect: exactly one project is "1 project," not "1 projects" --
/// proving `status_bar_summary` actually passes a genuine
/// `FluentValue::Number` through (plural-category selection), not a
/// pre-formatted string that happens to look right at zero.
#[test]
fn status_bar_summary_pluralizes_a_single_project_correctly() {
    let mut app_shell = ApplicationShell::new();
    let project_root = std::env::temp_dir().join(format!(
        "tekstide-shell-test-single-project-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&project_root).unwrap();
    app_shell
        .add_project_from_path(&project_root)
        .expect("a freshly created directory is a valid project root");

    let state = state_with(app_shell);
    assert_eq!(
        status_bar_summary(&state),
        format!(
            "{ISOLATE_START}Project Board{ISOLATE_END} | \
             {ISOLATE_START}{ISOLATE_START}1{ISOLATE_END} project{ISOLATE_END}"
        ),
        "exactly one project must use the singular form, not \"1 projects\""
    );
}

/// Mechanical seam check (response 125's own "ideally mechanically"
/// standard, applied to `implementation-handoff.md`'s i18n/theme seams):
/// `shell.rs`'s own source must contain no `text("literal")` call --
/// every user-facing string must come from `state.catalog.get(...)` or
/// a helper that does. A heuristic scan of the file's own text, not a
/// full parse -- ablation-verified below.
#[test]
fn shell_view_source_contains_no_raw_string_literal_passed_to_text() {
    let source = include_str!("../shell.rs");
    for (line_number, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") || trimmed.starts_with("///") || trimmed.starts_with("//!") {
            continue;
        }
        assert!(
            !contains_text_call_with_string_literal(line),
            "line {} in shell.rs passes a string literal directly to text(...): {line}",
            line_number + 1
        );
    }
}

fn contains_text_call_with_string_literal(line: &str) -> bool {
    let mut search_from = 0;
    while let Some(relative_index) = line[search_from..].find("text(") {
        let call_start = search_from + relative_index + "text(".len();
        let after_paren = line[call_start..].trim_start();
        if after_paren.starts_with('"') {
            return true;
        }
        search_from = call_start;
    }
    false
}

/// Mechanical seam check for colour: `shell.rs` must construct no
/// `iced::Color` directly (`Color::from_rgb`/`from_rgba`) -- every
/// colour must come from `state.theme`. `theme.rs` is the one legitimate
/// place `Color::from_rgb` appears, and this scan only covers `shell.rs`.
#[test]
fn shell_view_source_contains_no_raw_color_construction() {
    let source = include_str!("../shell.rs");
    assert!(
        !source.contains("Color::from_rgb") && !source.contains("Color::from_rgba"),
        "shell.rs must source every colour from state.theme, not construct one directly"
    );
}

/// Mechanical seam check for font size: `shell.rs`'s `.size(...)` calls
/// must all read from `state.theme.font_size_*()`, never a bare numeric
/// literal.
#[test]
fn shell_view_source_contains_no_raw_font_size_literal() {
    let source = include_str!("../shell.rs");
    for (line_number, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        if let Some(relative_index) = line.find(".size(") {
            let call_start = relative_index + ".size(".len();
            let after_paren = line[call_start..].trim_start();
            let starts_with_digit = after_paren
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit());
            assert!(
                !starts_with_digit,
                "line {} in shell.rs passes a bare numeric literal to .size(...): {line}",
                line_number + 1
            );
        }
    }
}
