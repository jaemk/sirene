#![no_main]

use libfuzzer_sys::fuzz_target;
use sirene_core::{
    math::{MathStyle, typeset},
    scene::{Color, Pt},
};

// Typeset arbitrary text in both styles and draw what comes out: errors are fine, panics and
// out-of-range spans are not.
fuzz_target!(|data: &[u8]| {
    let Ok(src) = std::str::from_utf8(data) else {
        return;
    };
    for style in [MathStyle::Display, MathStyle::Text] {
        match typeset(src, style) {
            Ok(formula) => {
                formula.place(Pt::new(10.0, 40.0), 24.0, Color::rgb(0));
            }
            Err(e) => {
                assert!(e.span.start <= e.span.end && e.span.end <= src.len());
                assert!(src.is_char_boundary(e.span.start) && src.is_char_boundary(e.span.end));
            }
        }
    }
});
