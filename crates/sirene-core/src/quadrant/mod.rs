//! Mermaid quadrant charts (MQUAD-*): points on two axes split into four quadrants.

mod lower;
mod parser;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::scene::Color;
use crate::syntax::SyntaxNode;

/// The labels at an axis's ends.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Axis {
    /// The left end of the x-axis, or the bottom of the y-axis.
    pub low: Option<String>,
    /// The right end of the x-axis, or the top of the y-axis.
    pub high: Option<String>,
}

impl Axis {
    /// The labels, each with where it centers along the axis as a fraction from the low end:
    /// the middle of its half, or of the whole axis for a low label alone.
    pub fn placed(&self) -> Vec<(&str, f64)> {
        match (self.low.as_deref(), self.high.as_deref()) {
            (Some(l), Some(h)) => vec![(l, 0.25), (h, 0.75)],
            (Some(l), None) => vec![(l, 0.5)],
            (None, Some(h)) => vec![(h, 0.75)],
            (None, None) => vec![],
        }
    }
}

/// A point's own look; `None` keeps the default.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PointStyle {
    pub radius: Option<f32>,
    pub color: Option<Color>,
    pub stroke_color: Option<Color>,
    pub stroke_width: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub name: String,
    /// From 0 (left) to 1 (right).
    pub x: f64,
    /// From 0 (bottom) to 1 (top).
    pub y: f64,
    /// The `:::class` it names.
    pub class: Option<String>,
    /// Its class's styles, then its own.
    pub style: PointStyle,
}

/// A quadrant chart (MQUAD-2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QuadrantChart {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub x_axis: Axis,
    pub y_axis: Axis,
    /// The labels of `quadrant-1` (top right), `quadrant-2` (top left), `quadrant-3` (bottom
    /// left), and `quadrant-4` (bottom right).
    pub quadrants: [Option<String>; 4],
    pub points: Vec<Point>,
}

/// The result of parsing quadrant chart source: a lossless syntax tree plus diagnostics.
#[derive(Debug, Clone)]
pub struct Parse {
    green: GreenNode,
    diagnostics: Vec<Diagnostic>,
}

impl Parse {
    /// Root of the syntax tree. Its text is always identical to the parsed source.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Lowers the tree to a [`QuadrantChart`]. The returned diagnostics include the parser's,
    /// ordered by position.
    pub fn lower(&self) -> (QuadrantChart, Vec<Diagnostic>) {
        let (chart, lowering) = lower::lower(&self.syntax());
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a quadrant chart: {} points; {} diagnostics",
            chart.points.len(),
            diagnostics.len()
        );
        (chart, diagnostics)
    }
}

/// Parses quadrant chart source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}
