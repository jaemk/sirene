//! A `@startlatex` or `@startmath` document as text (PCREO-3): the formula's rows, or its
//! source when it does not typeset or math support is off.

use super::Options;
use crate::equation::Equation;

/// Renders `equation` in display style.
pub fn render(equation: &Equation, _options: &Options) -> String {
    let mut out = formula(equation).unwrap_or_else(|| equation.source.clone());
    out.push('\n');
    out
}

#[cfg(feature = "math")]
fn formula(equation: &Equation) -> Option<String> {
    equation.ascii().ok()
}

#[cfg(not(feature = "math"))]
fn formula(_: &Equation) -> Option<String> {
    None
}
