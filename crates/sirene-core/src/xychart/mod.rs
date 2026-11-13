//! Mermaid XY charts (MXY-*): bar and line series over a category or numeric x-axis.

mod lower;
mod parser;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::syntax::SyntaxNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeriesKind {
    Line,
    Bar,
}

/// One `line` or `bar` statement.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    pub kind: SeriesKind,
    /// Shown in the legend; unnamed series are left out of it.
    pub name: Option<String>,
    pub values: Vec<f64>,
    /// A label per value (`540 "PaLM"`), drawn beside a line's points.
    pub labels: Vec<Option<String>>,
}

/// What the x-axis holds.
#[derive(Debug, Clone, PartialEq)]
pub enum XAxis {
    /// Named categories; when empty, the values' positions are numbered from 1.
    Categories(Vec<String>),
    /// Numbers from `min` to `max`, with each series' values spread evenly across it.
    Range { min: f64, max: f64 },
}

impl Default for XAxis {
    fn default() -> Self {
        XAxis::Categories(Vec::new())
    }
}

/// An XY chart (MXY-2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct XyChart {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    /// Categories down the side and values across, instead of categories across and values up.
    pub horizontal: bool,
    pub x_title: Option<String>,
    pub x_axis: XAxis,
    pub y_title: Option<String>,
    /// The value axis's `min --> max`; `None` spans the data.
    pub y_range: Option<(f64, f64)>,
    pub series: Vec<Series>,
}

impl XyChart {
    /// The number of positions along the x-axis.
    pub fn positions(&self) -> usize {
        let longest = self
            .series
            .iter()
            .map(|s| s.values.len())
            .max()
            .unwrap_or(0);
        match &self.x_axis {
            XAxis::Categories(c) if !c.is_empty() => c.len(),
            _ => longest,
        }
    }

    /// The x-axis value of position `i` of `n` on a numeric axis.
    pub fn x_at(min: f64, max: f64, i: usize, n: usize) -> f64 {
        if n < 2 {
            min
        } else {
            min + (max - min) * i as f64 / (n - 1) as f64
        }
    }

    /// The label of each position: its category, its number from 1, or its value on a numeric
    /// axis.
    pub fn x_labels(&self) -> Vec<String> {
        let n = self.positions();
        match &self.x_axis {
            XAxis::Categories(c) if !c.is_empty() => c.clone(),
            XAxis::Categories(_) => (1..=n).map(|i| i.to_string()).collect(),
            XAxis::Range { min, max } => (0..n)
                .map(|i| format_number(Self::x_at(*min, *max, i, n)))
                .collect(),
        }
    }

    /// The value axis's span: `y_range`, else the drawn values' (not those past the last
    /// category), widened by 1 each way when it is a single value (or 0 to 1 with no data).
    pub fn y_bounds(&self) -> (f64, f64) {
        if let Some(r) = self.y_range {
            return widen(r);
        }
        let n = self.positions();
        let mut values = self
            .series
            .iter()
            .flat_map(|s| s.values.iter().take(n).copied());
        let Some(first) = values.next() else {
            return (0.0, 1.0);
        };
        widen(values.fold((first, first), |(lo, hi), v| (lo.min(v), hi.max(v))))
    }
}

fn widen((lo, hi): (f64, f64)) -> (f64, f64) {
    if lo == hi {
        (lo - 1.0, hi + 1.0)
    } else {
        (lo, hi)
    }
}

/// `v` without a fraction when it is whole, else to at most 10 places without trailing zeros.
pub fn format_number(v: f64) -> String {
    let s = format!("{v:.10}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

/// Round steps of about `count` ticks covering `lo..=hi`: 1, 2, or 5 times a power of ten, as
/// d3's `ticks` picks them. Ported from d3-array (Copyright 2010-2023 Mike Bostock; ISC); see
/// THIRD-PARTY-NOTICES.
pub fn ticks(lo: f64, hi: f64, count: usize) -> Vec<f64> {
    let span = hi - lo;
    if span.is_nan() || span <= 0.0 || !span.is_finite() {
        return vec![lo];
    }
    let raw = span / count.max(1) as f64;
    let power = 10f64.powf(raw.log10().floor());
    let error = raw / power;
    let factor = if error >= 50f64.sqrt() {
        10.0
    } else if error >= 10f64.sqrt() {
        5.0
    } else if error >= 2f64.sqrt() {
        2.0
    } else {
        1.0
    };
    let step = factor * power;
    // With a little slack, so 0.3 / 0.1 counts as 3.
    let first = (lo / step - 1e-9).ceil() as i64;
    let last = (hi / step + 1e-9).floor() as i64;
    (first..=last)
        .map(|k| {
            let v = k as f64 * step;
            // Rounded to the step's precision, so 0.1 * 3 is 0.3.
            let places = (-step.log10().floor()).max(0.0) as i32;
            let f = 10f64.powi(places);
            (v * f).round() / f
        })
        .collect()
}

/// The result of parsing XY chart source: a lossless syntax tree plus diagnostics.
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

    /// Lowers the tree to an [`XyChart`]. The returned diagnostics include the parser's,
    /// ordered by position.
    pub fn lower(&self) -> (XyChart, Vec<Diagnostic>) {
        let (chart, lowering) = lower::lower(&self.syntax());
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered an XY chart: {} series; {} diagnostics",
            chart.series.len(),
            diagnostics.len()
        );
        (chart, diagnostics)
    }
}

/// Parses XY chart source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(format_number(3.0), "3");
        assert_eq!(format_number(-0.0), "0");
        assert_eq!(format_number(0.1 + 0.2), "0.3");
        assert_eq!(format_number(-2.5), "-2.5");
    }

    #[test]
    fn tick_steps() {
        assert_eq!(ticks(0.0, 100.0, 5), [0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);
        assert_eq!(ticks(4000.0, 11000.0, 4), [4000.0, 6000.0, 8000.0, 10000.0]);
        assert_eq!(ticks(-0.34, 2.4, 5), [0.0, 0.5, 1.0, 1.5, 2.0]);
        assert_eq!(ticks(0.0, 0.3, 3), [0.0, 0.1, 0.2, 0.3]);
        assert_eq!(ticks(1.0, 1.0, 5), [1.0]);
    }

    #[test]
    fn bounds_and_positions() {
        let s = |values: Vec<f64>| Series {
            kind: SeriesKind::Bar,
            name: None,
            labels: vec![None; values.len()],
            values,
        };
        let mut c = XyChart {
            series: vec![s(vec![3.0, -1.0]), s(vec![2.0, 5.0, 4.0])],
            ..XyChart::default()
        };
        assert_eq!(c.y_bounds(), (-1.0, 5.0));
        assert_eq!(c.positions(), 3);
        assert_eq!(c.x_labels(), ["1", "2", "3"]);
        c.x_axis = XAxis::Range { min: 0.0, max: 1.0 };
        assert_eq!(c.x_labels(), ["0", "0.5", "1"]);
        c.x_axis = XAxis::Categories(vec!["a".into(), "b".into()]);
        assert_eq!(c.positions(), 2);
        c.y_range = Some((2.0, 2.0));
        assert_eq!(c.y_bounds(), (1.0, 3.0));
        assert_eq!(XyChart::default().y_bounds(), (0.0, 1.0));
    }
}
