//! Formulas inside label lines (LEMB-4): a line is a row of text runs and formula runs on a
//! common baseline.
//!
//! Text is placed by the center of its line (`dominant-baseline: central`), so a formula, which
//! is placed by its baseline, needs the baseline's distance below that center. `TextMetrics`
//! has no ascent, so the distance is the constant [`BASELINE`].

use super::{Color, Primitive, Pt, TextMetrics, TextStyle, Theme};
use crate::flowchart::{Delim, Span};

/// The text style of a span.
pub(super) fn style_of(s: &Span) -> TextStyle {
    TextStyle {
        bold: s.bold,
        italic: s.italic,
    }
}

/// The width of a span at font size `size`: a formula that typesets is its own width, anything
/// else is text.
pub(super) fn span_width(metrics: &(impl TextMetrics + ?Sized), size: f32, s: &Span) -> f32 {
    match inline(s) {
        Some(f) if s.math => f.width * size,
        _ => metrics.styled_width(&s.text, size, style_of(s)),
    }
}

/// The runs of `lines` when some line holds a formula, else `None` (the lines are plain text).
/// For diagrams whose text is kept as source lines (sequence diagrams).
pub(super) fn source_runs(lines: &[String], delim: Delim) -> Option<Vec<Vec<Span>>> {
    use crate::flowchart::formula;
    if !cfg!(feature = "math") || !lines.iter().any(|l| formula::has(l, delim)) {
        return None;
    }
    Some(lines.iter().map(|l| formula::runs(l, delim)).collect())
}

/// The size of `lines` of runs: the widest line and the lines' heights added up.
pub(super) fn block_size(
    metrics: &(impl TextMetrics + ?Sized),
    theme: &Theme,
    lines: &[Vec<Span>],
) -> (f32, f32) {
    let width = |l: &Vec<Span>| -> f32 {
        l.iter()
            .map(|s| span_width(metrics, theme.font_size, s))
            .sum()
    };
    let w = lines.iter().map(width).fold(0.0, f32::max);
    let h = lines
        .iter()
        .map(|l| line_box(theme, l).map_or(theme.line_height, |(a, d)| a + d))
        .sum();
    (w, h)
}

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

/// The formula of a formula span, typeset in the style of its delimiters (display for Mermaid's
/// `$$`, text for PlantUML's `<latex>`); `None` for a text span, a formula that does not
/// typeset, and a build without the `math` feature.
#[cfg(feature = "math")]
pub(super) fn inline(span: &Span) -> Option<Inline> {
    use crate::math::typeset;
    let formula = typeset(&span.formula()?, span.delim()?.style()).ok()?;
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

/// The primitives of `lines`, each a row of text and formula runs on one baseline, as tall as
/// [`line_box`] says, the block centered on `center`. A line with no formula that typesets is a
/// line height tall with its text on the line's center.
pub(super) fn draw_lines(
    metrics: &(impl TextMetrics + ?Sized),
    theme: &Theme,
    lines: &[Vec<Span>],
    center: Pt,
    color: Color,
) -> Vec<Primitive> {
    let (lh, size) = (theme.line_height, theme.font_size);
    let width = |text: &str, style| metrics.styled_width(text, size, style);
    let boxes: Vec<Option<(f32, f32)>> = lines.iter().map(|l| line_box(theme, l)).collect();
    let height = |b: &Option<(f32, f32)>| b.map_or(lh, |(above, below)| above + below);
    let mut top = center.y - boxes.iter().map(height).sum::<f32>() / 2.0;
    let mut out = Vec::new();
    for (line, b) in lines.iter().zip(&boxes) {
        let (baseline, mid) = match *b {
            Some((above, _)) => (top + above, top + above - BASELINE * size),
            None => (top + lh / 2.0, top + lh / 2.0),
        };
        let items: Vec<(&Span, Option<Inline>)> = line
            .iter()
            .map(|s| (s, if s.math { inline(s) } else { None }))
            .collect();
        let total: f32 = items
            .iter()
            .map(|(s, f)| match f {
                Some(f) => f.width * size,
                None => width(&s.text, style_of(s)),
            })
            .sum();
        let mut x = center.x - total / 2.0;
        for (span, formula) in &items {
            if let Some(f) = formula {
                out.extend(f.place(Pt::new(x, baseline), size, color));
                x += f.width * size;
                continue;
            }
            let style = style_of(span);
            let trimmed = span.text.trim();
            if !trimmed.is_empty() {
                let lead = &span.text[..span.text.len() - span.text.trim_start().len()];
                let left = x + width(lead, style);
                let at = Pt::new(left + width(trimmed, style) / 2.0, mid);
                out.push(Primitive::Text {
                    text: trimmed.to_owned(),
                    center: at,
                    size,
                    color,
                    style,
                });
            }
            x += width(&span.text, style);
        }
        top += height(b);
    }
    out
}
