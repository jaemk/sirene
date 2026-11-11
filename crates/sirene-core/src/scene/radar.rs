//! Radar chart to scene (MRADAR-3): rings, an axis line per axis, and a closed rounded curve per
//! series, with a legend.

use std::f32::consts::PI;

use super::Theme;
use super::git::scale_color;
use super::paint::Painter;
use super::shapes::{self, BoxF};
use super::{Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle};
use crate::radar::{Graticule, Radar};

/// Legend swatch side, and the step between legend rows.
const SWATCH: f32 = 12.0;
const LEGEND_ROW: f32 = 20.0;
/// A curve fill's opacity, and a ring's.
const CURVE_ALPHA: u8 = 0x80;
const RING_ALPHA: u8 = 0x14;

/// Curve `k`'s color.
pub(super) fn curve_color(theme: &Theme, k: usize) -> Color {
    scale_color(theme, k)
}

/// The direction of axis `i` of `n`: the first straight up, the rest clockwise.
fn direction(i: usize, n: usize) -> (f32, f32) {
    let a = 2.0 * PI * i as f32 / n.max(1) as f32 - PI / 2.0;
    (a.cos(), a.sin())
}

/// A closed curve through `points`, each corner rounded by `tension` (Mermaid's
/// `closedRoundCurve`).
fn closed_curve(points: &[Pt], tension: f32) -> Vec<Segment> {
    let n = points.len();
    let Some(&first) = points.first() else {
        return Vec::new();
    };
    let mut out = vec![Segment::Move(first)];
    for i in 0..n {
        let p0 = points[(i + n - 1) % n];
        let p1 = points[i];
        let p2 = points[(i + 1) % n];
        let p3 = points[(i + 2) % n];
        out.push(Segment::Cubic {
            c1: Pt::new(
                p1.x + (p2.x - p0.x) * tension,
                p1.y + (p2.y - p0.y) * tension,
            ),
            c2: Pt::new(
                p2.x - (p3.x - p1.x) * tension,
                p2.y - (p3.y - p1.y) * tension,
            ),
            to: p2,
        });
    }
    out.push(Segment::Close);
    out
}

/// Draws `radar` (MRADAR-3).
pub fn radar(radar: &Radar, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let (fs, lh) = (theme.font_size, theme.line_height);
    let small = (fs - 2.0).max(8.0);
    let title = radar
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_size = fs * 1.2;
    let n = radar.axes.len();
    let radius = radar.width.min(radar.height) / 2.0;
    let [m_top, m_right, m_bottom, m_left] = radar.margins;

    // How far the drawing reaches from the center each way: the chart's box, the axis labels,
    // and the legend, which sits toward the top right as in Mermaid.
    let label_r = radius * radar.axis_label_factor;
    let line_r = radius * radar.axis_scale_factor.max(1.0);
    let mut half_w = (radar.width / 2.0).max(line_r);
    let mut half_h = (radar.height / 2.0).max(line_r);
    // Each label's center from the chart's center: past the axis end by half the label's size,
    // so a label beside the chart does not cover its axis.
    let label_at: Vec<(f32, f32)> = radar
        .axes
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let (dx, dy) = direction(i, n);
            let w = metrics.width(&a.label, small);
            (dx * (label_r + w / 2.0), dy * (label_r + lh / 2.0))
        })
        .collect();
    for ((x, y), a) in label_at.iter().zip(&radar.axes) {
        half_w = half_w.max(x.abs() + metrics.width(&a.label, small) / 2.0);
        half_h = half_h.max(y.abs() + lh / 2.0);
    }
    let legend: Vec<(usize, &str)> = if radar.show_legend {
        radar
            .curves
            .iter()
            .enumerate()
            .map(|(k, c)| (k, c.label.as_str()))
            .collect()
    } else {
        Vec::new()
    };
    let legend_at = Pt::new(
        (radar.width / 2.0 + m_right) * 0.75,
        -(radar.height / 2.0 + m_top) * 0.75,
    );
    for &(_, label) in &legend {
        half_w = half_w.max(legend_at.x + SWATCH + 6.0 + metrics.width(label, fs));
    }
    let top = if title.is_some() {
        m_top.max(lh + 16.0)
    } else {
        m_top
    };
    let width = (m_left + 2.0 * half_w + m_right)
        .max(title.map_or(0.0, |t| metrics.width(t, title_size)) + 32.0);
    let height = top + 2.0 * half_h + m_bottom;
    let center = Pt::new(width / 2.0, top + half_h);
    let at = |i: usize, r: f32| {
        let (dx, dy) = direction(i, n);
        Pt::new(center.x + dx * r, center.y + dy * r)
    };

    let owner = Owner::Diagram;
    let mut p = Painter {
        theme,
        metrics,
        elements: Vec::new(),
    };
    let text = |p: &mut Painter, s: &str, center: Pt, size: f32| {
        p.elements.push(Element {
            owner,
            primitive: Primitive::Text {
                text: s.to_owned(),
                center,
                size,
                color: theme.text,
                style: TextStyle::default(),
            },
        });
    };
    if let Some(t) = title {
        text(&mut p, t, Pt::new(width / 2.0, top / 2.0), title_size);
    }

    // The rings, outermost first so the inner ones' fills stack toward the center.
    let ring = Stroke {
        width: 1.0,
        color: theme.edge.with_alpha(0x80),
        dash: None,
    };
    let ticks = radar.ticks.max(1);
    for t in (1..=ticks).rev() {
        let r = radius * t as f32 / ticks as f32;
        let segments = match radar.graticule {
            Graticule::Circle => shapes::ellipse(center, r, r),
            Graticule::Polygon => {
                let pts: Vec<(f32, f32)> =
                    (0..n).map(|i| at(i, r)).map(|pt| (pt.x, pt.y)).collect();
                shapes::polygon(&pts)
            }
        };
        p.path(
            owner,
            segments,
            Some(theme.edge.with_alpha(RING_ALPHA)),
            Some(ring),
        );
    }

    // The axis lines and their labels.
    let axis = Stroke {
        width: 1.0,
        color: theme.edge,
        dash: None,
    };
    let mut lines = Vec::new();
    for i in 0..n {
        lines.push(Segment::Move(center));
        lines.push(Segment::Line(at(i, radius * radar.axis_scale_factor)));
    }
    if !lines.is_empty() {
        p.path(owner, lines, None, Some(axis));
    }
    for (a, (x, y)) in radar.axes.iter().zip(&label_at) {
        text(&mut p, &a.label, Pt::new(center.x + x, center.y + y), small);
    }

    // The curves, in order, so later ones draw over earlier ones.
    for (k, c) in radar.curves.iter().enumerate() {
        if n == 0 {
            break;
        }
        let points: Vec<Pt> = c
            .values
            .iter()
            .enumerate()
            .map(|(i, &v)| at(i, radius * radar.share(v) as f32))
            .collect();
        let color = curve_color(theme, k);
        let stroke = Stroke {
            width: 2.0,
            color,
            dash: None,
        };
        p.path(
            owner,
            closed_curve(&points, radar.curve_tension),
            Some(color.with_alpha(CURVE_ALPHA)),
            Some(stroke),
        );
    }

    for (row, &(k, label)) in legend.iter().enumerate() {
        let y = center.y + legend_at.y + row as f32 * LEGEND_ROW;
        let x = center.x + legend_at.x;
        let b = BoxF {
            x,
            y: y - SWATCH / 2.0,
            w: SWATCH,
            h: SWATCH,
        };
        p.path(
            owner,
            shapes::rounded(b, 2.0),
            Some(curve_color(theme, k)),
            None,
        );
        let w = metrics.width(label, fs);
        text(&mut p, label, Pt::new(x + SWATCH + 6.0 + w / 2.0, y), fs);
    }

    Scene {
        width,
        height,
        background: theme.background,
        elements: p.elements,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directions() {
        let (x, y) = direction(0, 4);
        assert!(x.abs() < 1e-6 && (y + 1.0).abs() < 1e-6);
        let (x, y) = direction(1, 4);
        assert!((x - 1.0).abs() < 1e-6 && y.abs() < 1e-6);
    }

    #[test]
    fn curve_through_points() {
        let pts = [Pt::new(0.0, 0.0), Pt::new(10.0, 0.0), Pt::new(10.0, 10.0)];
        let s = closed_curve(&pts, 0.0);
        assert_eq!(s.len(), 5);
        assert!(matches!(s[1], Segment::Cubic { to, c1, .. } if to == pts[1] && c1 == pts[0]));
        assert!(matches!(s[3], Segment::Cubic { to, .. } if to == pts[0]));
        assert!(closed_curve(&[], 0.2).is_empty());
    }
}
