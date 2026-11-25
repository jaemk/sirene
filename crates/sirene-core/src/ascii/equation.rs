//! A `@startlatex` or `@startmath` document as text (PCREO-3): the formula's rows, or its
//! source when it does not typeset or math support is off.

use super::{Charset, Options};
use crate::equation::Equation;

/// Renders `equation` in display style.
pub fn render(equation: &Equation, options: &Options) -> String {
    let mut out = formula(equation, options.charset).unwrap_or_else(|| equation.source.clone());
    out.push('\n');
    out
}

#[cfg(feature = "math")]
fn formula(equation: &Equation, charset: Charset) -> Option<String> {
    equation.ascii(charset).ok()
}

#[cfg(not(feature = "math"))]
fn formula(_: &Equation, _: Charset) -> Option<String> {
    None
}
