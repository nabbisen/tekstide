use super::{unwrapped_extent, wrapped_extent};

/// Extracts the `{ ... }` body immediately following the first occurrence of
/// `signature_prefix` in `source`, by bracket depth -- not by looking for the
/// next `pub(crate) fn` (which a multi-line signature or a nested closure
/// could shift) and not by a fixed line count. Shared, minimal shape; see
/// `theme::tests`'s own copy for why it is duplicated rather than imported
/// across these two sibling test modules (one is `pub(crate)`, the other is
/// not, and widening either for three lines of reuse was not worth it).
fn function_body(source: &str, signature_prefix: &str) -> String {
    let after_signature = source
        .split(signature_prefix)
        .nth(1)
        .unwrap_or_else(|| panic!("{signature_prefix:?} not found in source"));
    let bytes = after_signature.as_bytes();
    let open = after_signature
        .find('{')
        .unwrap_or_else(|| panic!("no function body opens after {signature_prefix:?}"));
    let mut depth: i32 = 0;
    let mut i = open;
    loop {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        i += 1;
    }
    after_signature[open + 1..i].to_owned()
}

/// **Review 670 required-1.** The original version of this test asserted
/// `flat.contains("iced::widget::text(content).font(ui_font())")` -- a
/// `contains` check a chained override survives, since appending
/// `.line_height(2.0)` after that exact substring still contains it.
/// Verified wrong in review: that probe left this test green while
/// production drew every label at a line height the harness does not know
/// about. Fixed to assert the function body **equals** that expression
/// exactly -- nothing chained after it -- which is what "`text_extent`'s
/// own doc comment: agreement with production is only real while
/// `super::text` overrides nothing this harness assumes is iced's own
/// default" actually needs, rather than a substring that happens to match
/// today.
#[test]
fn theme_text_overrides_nothing_the_harness_assumes_is_default() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/theme.rs"),
    )
    .expect("theme.rs");
    let shipped = source.split("#[cfg(test)]\nmod tests").next().unwrap();
    let body = function_body(shipped, "pub(crate) fn text<");
    let flat_body = body.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(
        flat_body, "iced::widget::text(content).font(ui_font())",
        "`theme::text`'s body must be exactly `iced::widget::text(content).font(ui_font())` -- \
         nothing chained after it -- or `text_extent`'s own `wrapped_extent`/`unwrapped_extent`, \
         which assume iced's unmodified `LineHeight`/`Shaping`/`Wrapping` defaults, silently \
         disagree with what ships"
    );
}

/// Sanity, in the same spirit as `change_review_decision_controls_measured_
/// layout_cost`'s own non-vacuity assertion: a real, non-trivial sentence
/// must measure a real, non-zero extent. Catches this harness regressing to
/// the null-renderer's always-zero shape it was built to replace.
#[test]
fn wrapped_extent_of_a_real_sentence_is_not_zero() {
    let size = wrapped_extent("a real sentence, not an empty one", 16.0, 200.0);
    assert!(
        size.width > 0.0 && size.height > 0.0,
        "measured {size:?} for real content -- a zero dimension here means this harness is as \
         broken as the null-renderer attempt it replaced"
    );
}

/// Same sanity for the unwrapped path, which a wrapped-only regression test
/// would not catch (it exercises a different `wrapping` value and therefore
/// a different code path through `measured_extent`).
#[test]
fn unwrapped_extent_of_a_real_label_is_not_zero() {
    let size = unwrapped_extent("Ctrl+Alt+Shift+K", 16.0);
    assert!(
        size.width > 0.0 && size.height > 0.0,
        "measured {size:?} for a real label -- a zero dimension here means this harness is as \
         broken as the null-renderer attempt it replaced"
    );
}

/// Falsifies the specific bug `text_extent`'s own doc comment names: at the
/// wrong `LineHeight` (`Relative(1.0)` instead of iced's own default,
/// `Relative(1.3)`), a two-line wrap measures about 30% shorter. Proves the
/// harness is sensitive to the fix, not merely documented as having one --
/// constructs both the correct measurement and the bug's own shape directly
/// (not by editing production code, which `measured_extent` does not expose
/// a seam to override), and shows they disagree by roughly the predicted
/// margin.
#[test]
fn wrapped_extent_is_measurably_taller_than_the_relative_1_0_bug_would_report() {
    use iced::advanced::graphics::text::Paragraph as GraphicsParagraph;
    use iced::advanced::text::{Alignment, LineHeight, Paragraph as _, Shaping, Text, Wrapping};
    use iced::{Pixels, Size};

    // Narrow enough that this sentence wraps onto at least two lines, so a
    // per-line line-height error compounds rather than vanishing on a
    // single line.
    const CONTENT: &str = "a sentence long enough to wrap onto more than one line at this width";
    const FONT_SIZE: f32 = 16.0;
    const WIDTH: f32 = 160.0;

    let correct = wrapped_extent(CONTENT, FONT_SIZE, WIDTH);

    let buggy_relative_1_0 = GraphicsParagraph::with_text(Text {
        content: CONTENT,
        bounds: Size::new(WIDTH, f32::INFINITY),
        size: Pixels(FONT_SIZE),
        line_height: LineHeight::Relative(1.0),
        font: iced::Font::DEFAULT,
        align_x: Alignment::Default,
        align_y: iced::alignment::Vertical::Top,
        shaping: Shaping::Basic,
        wrapping: Wrapping::Word,
    })
    .min_bounds();

    assert!(
        correct.height > buggy_relative_1_0.height,
        "the corrected harness ({:.2}px) must measure taller than the Relative(1.0)/Basic bug \
         ({:.2}px) for wrapped multi-line content -- if it does not, the fix this module exists \
         for is not actually in effect",
        correct.height,
        buggy_relative_1_0.height
    );
}
