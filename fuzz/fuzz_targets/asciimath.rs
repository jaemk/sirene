#![no_main]

use libfuzzer_sys::fuzz_target;
use sirene_core::{
    math::{MathStyle, translate, typeset_asciimath},
    scene::{Color, Pt},
};

// Translate arbitrary AsciiMath to TeX, typeset it in both styles, and draw what comes out:
// errors are fine, panics and spans that are not on character boundaries of the source are not,
// and the TeX stays bounded.
fuzz_target!(|data: &[u8]| {
    let Ok(src) = std::str::from_utf8(data) else {
        return;
    };
    let on_source = |start: usize, end: usize| {
        start <= end
            && end <= src.len()
            && src.is_char_boundary(start)
            && src.is_char_boundary(end)
    };
    match translate(src) {
        Ok(t) => {
            assert!(t.tex.len() <= 64 * src.len() + 4096);
            let span = t.source_span(0..t.tex.len());
            assert!(on_source(span.start, span.end));
        }
        Err(e) => assert!(on_source(e.span.start, e.span.end)),
    }
    for style in [MathStyle::Display, MathStyle::Text] {
        match typeset_asciimath(src, style) {
            Ok(formula) => {
                formula.place(Pt::new(10.0, 40.0), 24.0, Color::rgb(0));
            }
            Err(e) => assert!(on_source(e.span.start, e.span.end)),
        }
    }
});
