//! A `@startlatex` document as text (PCREO-3): the formula's rows, or its source when it does
//! not typeset or math support is off.

use super::Options;
use crate::equation::Equation;

/// Renders `equation` in display style.
pub fn render(equation: &Equation, _options: &Options) -> String {
    let mut out = formula(&equation.source).unwrap_or_else(|| equation.source.clone());
    out.push('\n');
    out
}

#[cfg(feature = "math")]
fn formula(source: &str) -> Option<String> {
    crate::math::ascii_formula(source, crate::math::MathStyle::Display).ok()
}

#[cfg(not(feature = "math"))]
fn formula(_: &str) -> Option<String> {
    None
}
