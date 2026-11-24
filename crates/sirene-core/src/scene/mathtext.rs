//! Formulas inside label lines (LEMB-4): a line is a row of text runs and formula runs on a
//! common baseline.
//!
//! Text is placed by the center of its line (`dominant-baseline: central`), so a formula, which
//! is placed by its baseline, needs the baseline's distance below that center. `TextMetrics`
//! has no ascent, so the distance is the constant [`BASELINE`].

use super::Theme;
use crate::flowchart::Span;

/// How far below the center of a text line its baseline is, in em: half the x-height of a
/// typical sans face plus a little, as `dominant-baseline: central` places it.
pub(super) const BASELINE: f32 = 0.35;

/// Space added above and below a formula in its line, in pixels.
const PAD: f32 = 1.0;

/// A typeset formula run and its size in em.
#[cfg(feature = "math")]
pub(super) struct Inline {
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
    formula: crate::math::Formula,
}

#[cfg(not(feature = "math"))]
#[allow(dead_code)]
pub(super) struct Inline {
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
}

/// The formula of a formula span, typeset in display style as Mermaid does; `None` for a text
/// span, a formula that does not typeset, and a build without the `math` feature.
#[cfg(feature = "math")]
pub(super) fn inline(span: &Span) -> Option<Inline> {
    use crate::math::{MathStyle, typeset};
    let formula = typeset(&span.formula()?, MathStyle::Display).ok()?;
    Some(Inline {
        width: formula.width,
        ascent: formula.ascent,
        descent: formula.descent.max(0.0),
        formula,
    })
}

#[cfg(not(feature = "math"))]
pub(super) fn inline(_: &Span) -> Option<Inline> {
    None
}

#[cfg(feature = "math")]
impl Inline {
    /// The formula's primitives, its left edge on the baseline at `origin`.
    pub(super) fn place(
        &self,
        origin: super::Pt,
        size: f32,
        color: super::Color,
    ) -> Vec<super::Primitive> {
        self.formula.place(origin, size, color)
    }
}

#[cfg(not(feature = "math"))]
#[allow(dead_code)]
impl Inline {
    pub(super) fn place(&self, _: super::Pt, _: f32, _: super::Color) -> Vec<super::Primitive> {
        Vec::new()
    }
}

/// The height of `line` above and below its baseline, or `None` when it has no formula that
/// typesets (the line is then plain text, one line height tall and centered).
///
/// Each side is the larger of the text's (half a line height, shifted by [`BASELINE`]) and the
/// formulas' (plus [`PAD`]), so the line is at least a line height tall and holds every formula.
pub(super) fn line_box(theme: &Theme, line: &[Span]) -> Option<(f32, f32)> {
    if !line.iter().any(|s| s.math) {
        return None;
    }
    let size = theme.font_size;
    let mut above = theme.line_height / 2.0 + BASELINE * size;
    let mut below = theme.line_height / 2.0 - BASELINE * size;
    let mut any = false;
    for f in line.iter().filter_map(inline) {
        any = true;
        above = above.max(f.ascent * size + PAD);
        below = below.max(f.descent * size + PAD);
    }
    any.then_some((above, below))
}
