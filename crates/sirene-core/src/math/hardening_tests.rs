//! Regression tests for the resource bounds: hostile input is a `MathError`, never an abort, a
//! hang, or an enormous allocation.

use std::time::{Duration, Instant};

use proptest::prelude::*;

use super::{MathStyle, ascii_formula, typeset};

fn typeset_error(src: &str) -> String {
    // An empty message makes the caller's `contains` assertion fail when typesetting succeeds.
    typeset(src, MathStyle::Text)
        .err()
        .map(|e| e.message)
        .unwrap_or_default()
}

#[test]
fn many_combining_marks_on_one_character_are_an_error() {
    let src = format!("x{}", "\u{301}".repeat(1000));
    assert!(typeset_error(&src).contains("combining marks"));
    assert!(ascii_formula(&src, MathStyle::Text).is_err());
}

#[test]
fn a_few_combining_marks_still_typeset() {
    let src = format!("x{}", "\u{301}".repeat(8));
    assert!(typeset(&src, MathStyle::Text).is_ok());
}

#[test]
fn repeated_dots_are_an_error_not_an_abort() {
    let src = "\\dots".repeat(2000);
    assert!(typeset_error(&src).contains("nested"));
}

#[test]
fn repeated_expandafter_is_an_error_not_an_abort() {
    let src = "\\expandafter".repeat(5400);
    assert!(typeset(&src, MathStyle::Text).is_err());
}

#[test]
fn a_long_digit_run_as_a_size_fails_quickly() {
    let start = Instant::now();
    for src in [
        format!("\\kern{}", "1".repeat(60_000)),
        format!("\\kern{{{}}}", "1".repeat(60_000)),
        format!("\\rule[{}]{{1em}}{{1em}}", "1".repeat(60_000)),
    ] {
        assert!(typeset(&src, MathStyle::Text).is_err());
    }
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "{:?}",
        start.elapsed()
    );
}

#[test]
fn echoed_text_in_errors_is_clipped() {
    let long = "q".repeat(5000);
    for src in [
        format!("\\kern{{{long}}}"),
        format!("\\color{{#{long}}}{{x}}"),
        format!("\\newcommand\\f[{long}]{{}}"),
        format!("\\htmlData{{{long}}}{{x}}"),
        format!("\\includegraphics[{long}=1]{{x}}"),
    ] {
        let message = typeset_error(&src);
        assert!(message.len() < 200, "{message}");
        assert!(message.contains("..."), "{message}");
        assert!(message.is_ascii());
    }
}

#[test]
fn a_wide_matrix_is_too_big_for_text_but_cheap_to_refuse() {
    let src = format!(
        "\\begin{{matrix}}{}{}\\end{{matrix}}",
        "x".repeat(12_000),
        "\\\\a".repeat(1000)
    );
    let err = ascii_formula(&src, MathStyle::Text).unwrap_err();
    assert!(err.message.contains("too large"), "{err}");
}

#[test]
fn a_table_with_too_many_rows_is_an_error() {
    for src in [
        format!("\\begin{{matrix}}{}\\end{{matrix}}", "a\\\\".repeat(12_000)),
        format!("\\begin{{CD}}{}\\end{{CD}}", "A\\\\".repeat(15_000)),
    ] {
        let message = typeset(&src, MathStyle::Display).unwrap_err().message;
        assert!(message.contains("table larger"), "{message}");
        assert!(ascii_formula(&src, MathStyle::Display).is_err());
    }
}

#[test]
fn a_cd_with_too_many_cells_is_an_error() {
    // Each row of 21 cells (A, 10 arrows and 10 more objects) is under the row limit together.
    let row = format!("A{}\\\\", "@>>>B".repeat(10));
    let src = format!("\\begin{{CD}}{}\\end{{CD}}", row.repeat(900));
    let message = typeset(&src, MathStyle::Display).unwrap_err().message;
    assert!(message.contains("table larger"), "{message}");
    assert!(ascii_formula(&src, MathStyle::Display).is_err());
    let few = format!("\\begin{{CD}}{}\\end{{CD}}", row.repeat(3));
    assert!(typeset(&few, MathStyle::Display).is_ok());
}

#[test]
fn a_table_with_too_many_cells_is_an_error() {
    let src = format!("\\begin{{matrix}}{}\\end{{matrix}}", "a&".repeat(20_000));
    assert!(typeset_error(&src).contains("table larger"));
}

#[test]
fn ascii_fails_where_layout_fails() {
    for src in [
        r"\textsf{\textbf{\textit{x}}}".to_string(),
        "\\kern1000em".repeat(11) + "x",
        format!("\\def\\a{{{}}}{}", "x".repeat(1000), "\\a".repeat(200)),
        format!("\\mathrlap{{{}x}}", "\\kern1000em".repeat(11)),
        "\\smash{\\raisebox{1000em}{".repeat(30) + "x" + &"}}".repeat(30),
    ] {
        assert!(typeset(&src, MathStyle::Text).is_err(), "{src:.60}");
        assert!(ascii_formula(&src, MathStyle::Text).is_err(), "{src:.60}");
    }
}

#[test]
fn items_drawn_far_from_an_empty_box_are_an_error() {
    let rlap = format!("\\mathrlap{{{}x}}", "\\kern1000em".repeat(11));
    assert!(typeset_error(&rlap).contains("further than"), "{rlap:.60}");
    let nested = "\\smash{\\raisebox{1000em}{".repeat(30) + "x" + &"}}".repeat(30);
    let message = typeset_error(&nested);
    assert!(message.contains("further than"), "{message}");
    // A shift inside the bound still draws.
    assert!(typeset("\\mathrlap{\\kern100em x}", MathStyle::Text).is_ok());
}

#[test]
fn a_formula_with_too_many_glyphs_is_an_error() {
    let src = format!("\\def\\a{{{}}}{}", "x".repeat(1000), "\\a".repeat(200));
    assert!(src.len() < 2000);
    assert!(typeset_error(&src).contains("glyphs"));
}

#[test]
fn short_asciimath_that_translates_long_names_the_tex_limit() {
    use super::{ascii_asciimath, typeset_asciimath};
    // 12000 * 5 = 60000 bytes of AsciiMath, under the source limit; each `alpha` is `\alpha `.
    let src = "alpha".repeat(12_000);
    assert!(src.len() < 64 * 1024);
    for message in [
        typeset_asciimath(&src, MathStyle::Text)
            .unwrap_err()
            .message,
        ascii_asciimath(&src, MathStyle::Text).unwrap_err().message,
    ] {
        assert!(message.contains("translated TeX"), "{message}");
    }
    let long = "a".repeat(64 * 1024 + 1);
    let message = typeset_asciimath(&long, MathStyle::Text)
        .unwrap_err()
        .message;
    assert!(!message.contains("translated"), "{message}");
}

#[test]
fn a_backward_kern_leaves_a_zero_width() {
    let formula = typeset("\\kern-1000em x", MathStyle::Text).unwrap();
    assert!(formula.width >= 0.0 && formula.width.is_finite());
    let formula = typeset("\\kern-1000em", MathStyle::Text).unwrap();
    assert_eq!(formula.width, 0.0);
}

#[test]
fn a_lowered_box_has_no_negative_ascent() {
    let formula = typeset("\\raisebox{-1000em}{x}", MathStyle::Text).unwrap();
    assert!(formula.ascent >= 0.0 && formula.descent >= 0.0);
}

#[test]
fn a_raised_box_within_the_cap_is_kept_and_past_it_is_an_error() {
    let formula = typeset("\\raisebox{1000em}{x}", MathStyle::Text).unwrap();
    assert!(formula.ascent >= 1000.0 && formula.ascent <= 10_000.0);
    let src = "\\kern1000em".repeat(11);
    assert!(typeset_error(&src).contains("larger than"));
}

fn piece() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("\\frac".to_string()),
        Just("\\sqrt".to_string()),
        Just("\\left(".to_string()),
        Just("\\right)".to_string()),
        Just("\\dots".to_string()),
        Just("\\expandafter".to_string()),
        Just("\\def\\a{x}".to_string()),
        Just("\\a".to_string()),
        Just("\\kern".to_string()),
        Just("\\kern-9em".to_string()),
        Just("\\begin{matrix}".to_string()),
        Just("\\end{matrix}".to_string()),
        Just("\\textsf".to_string()),
        Just("\\textbf".to_string()),
        Just("\\mathbf".to_string()),
        Just("\\color".to_string()),
        Just("{".to_string()),
        Just("}".to_string()),
        Just("^".to_string()),
        Just("_".to_string()),
        Just("&".to_string()),
        Just("\\\\".to_string()),
        Just("\u{301}".to_string()),
        Just("\u{300}\u{302}".to_string()),
        Just("1em".to_string()),
        "[a-z0-9 ]{0,4}".prop_map(String::from),
        any::<String>().prop_map(|s| s.chars().take(6).collect()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn arbitrary_mixes_never_panic_and_finish_in_bounds(
        parts in proptest::collection::vec(piece(), 0..60),
        repeat in 1_usize..40,
    ) {
        let src: String = parts.concat().repeat(repeat);
        let start = Instant::now();
        for style in [MathStyle::Display, MathStyle::Text] {
            match typeset(&src, style) {
                Ok(f) => {
                    prop_assert!(f.width.is_finite() && f.width >= 0.0);
                    prop_assert!(f.ascent.is_finite() && f.ascent >= 0.0);
                    prop_assert!(f.descent.is_finite() && f.descent >= 0.0);
                }
                Err(e) => {
                    prop_assert!(e.span.end <= src.len());
                    prop_assert!(src.is_char_boundary(e.span.start));
                    prop_assert!(src.is_char_boundary(e.span.end));
                }
            }
            let _ = ascii_formula(&src, style);
        }
        prop_assert!(start.elapsed() < Duration::from_secs(10), "{:?}", start.elapsed());
    }
}
