//! Formulas inside label lines (LEMB-4): a line is a row of text runs and formula runs on a
//! common baseline.
//!
//! Text is placed by the center of its line (`dominant-baseline: central`), so a formula, which
//! is placed by its baseline, needs the baseline's distance below that center. `TextMetrics`
//! has no ascent, so the distance is the constant [`BASELINE`].
//!
//! A formula is typeset for its width, its height, and its paths, and a diagram asks for each
//! many times, so a render keeps a [`Formulas`] cache of what it has typeset.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::{Color, Primitive, Pt, TextMetrics, TextStyle, Theme};
use crate::flowchart::{Delim, Span};

/// The text style of a span.
pub(super) fn style_of(s: &Span) -> TextStyle {
    TextStyle {
        bold: s.bold,
        italic: s.italic,
    }
}

/// The formulas of one render, typeset once each. A handle: clones share the cache. It is keyed
/// on the span's text, which holds the delimiters and so says the language and style too.
#[derive(Clone, Default)]
pub(super) struct Formulas(Rc<RefCell<HashMap<String, Option<Rc<Inline>>>>>);

impl Formulas {
    /// The typeset formula of a formula span; `None` for a text span, a formula that does not
    /// typeset, and a build without the `math` feature.
    pub(super) fn inline(&self, span: &Span) -> Option<Rc<Inline>> {
        if !span.math {
            return None;
        }
        if let Some(hit) = self.0.borrow().get(&span.text) {
            return hit.clone();
        }
        let made = typeset(span).map(Rc::new);
        self.0.borrow_mut().insert(span.text.clone(), made.clone());
        made
    }

    /// How many distinct formulas are held.
    #[cfg(all(test, feature = "math"))]
    fn len(&self) -> usize {
        self.0.borrow().len()
    }
}

/// The width of a span at font size `size`: a formula that typesets is its own width, anything
/// else is text.
pub(super) fn span_width(
    metrics: &(impl TextMetrics + ?Sized),
    formulas: &Formulas,
    size: f32,
    s: &Span,
) -> f32 {
    match formulas.inline(s) {
        Some(f) => f.width * size,
        None => metrics.styled_width(&s.text, size, style_of(s)),
    }
}

/// The runs of `lines` when some line holds a formula, else `None` (the lines are plain text).
/// For diagrams whose text is kept as source lines (sequence diagrams).
pub(super) fn source_runs(lines: &[String], delim: Delim) -> Option<Vec<Vec<Span>>> {
    use crate::flowchart::formula;
    // Without the `math` feature the lines are drawn as the text they are.
    if !cfg!(feature = "math") || !lines.iter().any(|l| formula::has(l, delim)) {
        return None;
    }
    Some(lines.iter().map(|l| formula::runs(l, delim)).collect())
}

/// The size of `lines` of runs: the widest line and the lines' heights added up.
pub(super) fn block_size(
    metrics: &(impl TextMetrics + ?Sized),
    formulas: &Formulas,
    theme: &Theme,
    lines: &[Vec<Span>],
) -> (f32, f32) {
    let width = |l: &Vec<Span>| -> f32 {
        l.iter()
            .map(|s| span_width(metrics, formulas, theme.font_size, s))
            .sum()
    };
    let w = lines.iter().map(width).fold(0.0, f32::max);
    let h = lines
        .iter()
        .map(|l| line_box(formulas, theme, l).map_or(theme.line_height, |(a, d)| a + d))
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
fn typeset(span: &Span) -> Option<Inline> {
    let formula = span.typeset()?.ok()?;
    Some(Inline {
        width: formula.width,
        ascent: formula.ascent,
        descent: formula.descent.max(0.0),
        formula,
    })
}

#[cfg(not(feature = "math"))]
fn typeset(_: &Span) -> Option<Inline> {
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
pub(super) fn line_box(formulas: &Formulas, theme: &Theme, line: &[Span]) -> Option<(f32, f32)> {
    if !line.iter().any(|s| s.math) {
        return None;
    }
    let size = theme.font_size;
    let mut above = theme.line_height / 2.0 + BASELINE * size;
    let mut below = theme.line_height / 2.0 - BASELINE * size;
    let mut any = false;
    for f in line.iter().filter_map(|s| formulas.inline(s)) {
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
    formulas: &Formulas,
    theme: &Theme,
    lines: &[Vec<Span>],
    center: Pt,
    color: Color,
) -> Vec<Primitive> {
    let (lh, size) = (theme.line_height, theme.font_size);
    let width = |text: &str, style| metrics.styled_width(text, size, style);
    let boxes: Vec<Option<(f32, f32)>> =
        lines.iter().map(|l| line_box(formulas, theme, l)).collect();
    let height = |b: &Option<(f32, f32)>| b.map_or(lh, |(above, below)| above + below);
    let mut top = center.y - boxes.iter().map(height).sum::<f32>() / 2.0;
    let mut out = Vec::new();
    for (line, b) in lines.iter().zip(&boxes) {
        let (baseline, mid) = match *b {
            Some((above, _)) => (top + above, top + above - BASELINE * size),
            None => (top + lh / 2.0, top + lh / 2.0),
        };
        let items: Vec<(&Span, Option<Rc<Inline>>)> =
            line.iter().map(|s| (s, formulas.inline(s))).collect();
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

#[cfg(all(test, feature = "math"))]
mod tests {
    use super::*;
    use crate::scene::Approx;

    fn formula(text: &str) -> Span {
        Span {
            text: text.to_owned(),
            math: true,
            ..Span::default()
        }
    }

    #[test]
    fn a_formula_is_typeset_once_however_often_it_is_asked_for() {
        let cache = Formulas::default();
        let theme = Theme::LIGHT;
        let line = vec![
            Span::plain("a "),
            formula("$$x^2$$"),
            formula("$$y$$"),
            formula("$$x^2$$"),
        ];
        crate::equation::count::reset();
        for _ in 0..5 {
            block_size(&Approx, &cache, &theme, std::slice::from_ref(&line));
            draw_lines(
                &Approx,
                &cache,
                &theme,
                std::slice::from_ref(&line),
                Pt::new(0.0, 0.0),
                theme.text,
            );
        }
        assert_eq!(crate::equation::count::get(), 2);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn a_failing_formula_is_not_retried() {
        let cache = Formulas::default();
        let span = formula(r"$$\foo$$");
        crate::equation::count::reset();
        for _ in 0..4 {
            assert!(cache.inline(&span).is_none());
        }
        assert_eq!(crate::equation::count::get(), 1);
    }

    #[test]
    fn clones_share_the_cache() {
        let cache = Formulas::default();
        let span = formula("$$x$$");
        crate::equation::count::reset();
        let a = cache.inline(&span);
        let b = cache.clone().inline(&span);
        assert!(a.is_some() && b.is_some());
        assert_eq!(crate::equation::count::get(), 1);
    }

    /// A flowchart of `n` formula labels, rectangles, diamonds (which size their label many
    /// times over), and edge labels, every formula distinct.
    fn many(n: usize) -> crate::flowchart::Flowchart {
        let mut src = String::from("flowchart TD\n");
        for i in 0..n {
            let (open, close) = if i % 2 == 0 { ('[', ']') } else { ('{', '}') };
            src += &format!("    N{i}{open}\"$$x^{{{i}}}$$\"{close}\n");
        }
        src += &format!("    N0 -- \"$$e^{{{n}}}$$\" --> N1\n");
        crate::flowchart::parse(&src).lower().flowchart
    }

    /// A formula is typeset a bounded number of times however many shapes and passes ask for
    /// its size: once to check it when lowering and once to draw it, not once per pass (a
    /// diamond's label alone was sized about forty times). Counted, not timed.
    #[test]
    fn a_flowchart_typesets_each_formula_once_per_render() {
        let n = 200;
        crate::equation::count::reset();
        let chart = many(n);
        assert_eq!(crate::equation::count::get(), n + 1, "lowering checks each");
        crate::equation::count::reset();
        let scene = super::super::flowchart(&chart, &Approx, &Theme::LIGHT).unwrap();
        assert_eq!(crate::equation::count::get(), n + 1, "drawing sets each");
        assert!(scene.elements.len() > n);
    }

    #[test]
    fn text_spans_are_never_cached() {
        let cache = Formulas::default();
        assert!(cache.inline(&Span::plain("$$x$$")).is_none());
        assert_eq!(cache.len(), 0);
    }
}
