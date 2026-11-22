//! Mermaid radar charts (MRADAR-*): axes around a circle, and a closed curve per series.

mod lower;
mod parser;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::syntax::SyntaxNode;

/// One axis, from the center out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Axis {
    pub id: String,
    /// The `["label"]`, or the id when there is none.
    pub label: String,
}

/// One series: a value on each axis, in axis order.
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    pub id: String,
    pub label: String,
    pub values: Vec<f64>,
}

/// The rings behind the curves (`graticule`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Graticule {
    #[default]
    Circle,
    Polygon,
}

/// A radar chart (MRADAR-2).
#[derive(Debug, Clone, PartialEq)]
pub struct Radar {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub axes: Vec<Axis>,
    pub curves: Vec<Curve>,
    /// `showLegend`, on unless set to `false`.
    pub show_legend: bool,
    /// `max`; the largest value unless set.
    pub max: Option<f64>,
    /// `min`, 0 unless set.
    pub min: f64,
    pub graticule: Graticule,
    /// `ticks`: how many rings, 5 unless set.
    pub ticks: u32,
    /// Frontmatter `config.radar`: the chart's `width` and `height` (600 unless set), and the
    /// margins around it (50 each), in the order top, right, bottom, left.
    pub width: f32,
    pub height: f32,
    pub margins: [f32; 4],
    /// `axisScaleFactor`: the axis lines' length as a share of the radius, 1 unless set.
    pub axis_scale_factor: f32,
    /// `axisLabelFactor`: how far out the axis labels sit, as a share of the radius, 1.05
    /// unless set.
    pub axis_label_factor: f32,
    /// `curveTension`: how round each curve's corners are, 0.17 unless set.
    pub curve_tension: f32,
}

impl Default for Radar {
    fn default() -> Self {
        Radar {
            title: None,
            acc_title: None,
            acc_descr: None,
            axes: Vec::new(),
            curves: Vec::new(),
            show_legend: true,
            max: None,
            min: 0.0,
            graticule: Graticule::Circle,
            ticks: 5,
            width: 600.0,
            height: 600.0,
            margins: [50.0; 4],
            axis_scale_factor: 1.0,
            axis_label_factor: 1.05,
            curve_tension: 0.17,
        }
    }
}

impl Radar {
    /// The values at the center and at the rim: `min`, and `max` or the largest value. The rim
    /// is above the center unless the minimum is already `f64::MAX`.
    pub fn bounds(&self) -> (f64, f64) {
        let lo = self.min;
        let hi = self.max.unwrap_or_else(|| {
            self.curves
                .iter()
                .flat_map(|c| c.values.iter().copied())
                .fold(f64::NEG_INFINITY, f64::max)
        });
        if hi > lo {
            (lo, hi)
        } else {
            let next = lo + 1.0;
            let hi = if next > lo && next.is_finite() {
                next
            } else if lo < f64::MAX {
                lo.next_up()
            } else {
                lo
            };
            (lo, hi)
        }
    }

    /// How far out `v` sits, from 0 at the center to 1 at the rim; values outside the bounds
    /// sit on them.
    pub fn share(&self, v: f64) -> f64 {
        share_with_bounds(v, self.bounds())
    }
}

/// Normalize with bounds already computed once for a rendering pass.
pub(crate) fn share_with_bounds(v: f64, (lo, hi): (f64, f64)) -> f64 {
    if v <= lo || hi <= lo || v.is_nan() {
        return 0.0;
    }
    if v >= hi {
        return 1.0;
    }
    let range = hi - lo;
    if range.is_finite() {
        (v - lo) / range
    } else {
        // Halve before subtracting so opposite extreme finite bounds cannot overflow.
        (v / 2.0 - lo / 2.0) / (hi / 2.0 - lo / 2.0)
    }
}

/// The result of parsing radar source: a lossless syntax tree plus diagnostics.
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

    /// Lowers the tree to a [`Radar`]. The returned diagnostics include the parser's, ordered
    /// by position.
    pub fn lower(&self) -> (Radar, Vec<Diagnostic>) {
        let (radar, lowering) = lower::lower(&self.syntax());
        let diagnostics = crate::diagnostic::merge(&self.diagnostics, lowering);
        log::debug!(
            "lowered a radar chart: {} axes, {} curves; {} diagnostics",
            radar.axes.len(),
            radar.curves.len(),
            diagnostics.len()
        );
        (radar, diagnostics)
    }
}

/// Parses radar source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve(values: &[f64]) -> Curve {
        Curve {
            id: "c".into(),
            label: "c".into(),
            values: values.to_vec(),
        }
    }

    #[test]
    fn bounds() {
        let mut r = Radar {
            curves: vec![curve(&[1.0, 4.0]), curve(&[2.0, 3.0])],
            ..Radar::default()
        };
        assert_eq!(r.bounds(), (0.0, 4.0));
        assert_eq!(r.share(2.0), 0.5);
        assert_eq!(r.share(9.0), 1.0);
        assert_eq!(r.share(-1.0), 0.0);
        r.max = Some(10.0);
        r.min = 2.0;
        assert_eq!(r.bounds(), (2.0, 10.0));
        // No values, or a rim at or below the center, still give a range.
        assert_eq!(Radar::default().bounds(), (0.0, 1.0));
        r.max = Some(1.0);
        assert_eq!(r.bounds(), (2.0, 3.0));
    }

    #[test]
    fn extreme_bounds_and_shares_stay_finite() {
        let mut r = Radar {
            min: -1e308,
            max: Some(1e308),
            ..Radar::default()
        };
        assert_eq!(r.share(-1e308), 0.0);
        assert_eq!(r.share(0.0), 0.5);
        assert_eq!(r.share(1e308), 1.0);
        r.min = 1e308;
        r.max = None;
        let (lo, hi) = r.bounds();
        assert!(hi.is_finite() && hi > lo);
        assert_eq!(r.share(lo), 0.0);
        assert_eq!(r.share(hi), 1.0);
        r.min = f64::MAX;
        assert_eq!(r.bounds(), (f64::MAX, f64::MAX));
        assert_eq!(r.share(f64::MAX), 0.0);
    }
}
