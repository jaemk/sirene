//! Quadrant chart to scene (MQUAD-3): four tinted quadrants, axis labels, and named points.

use super::Theme;
use super::paint::{Painter, Px};
use super::shapes::{self, BoxF};
use super::{Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle};
use crate::flowchart::{Label, Span};
use crate::quadrant::QuadrantChart;

const MARGIN: f32 = 16.0;
/// Side of the square holding the quadrants.
const SIDE: f32 = 400.0;
/// Padding inside a quadrant.
const PAD: f32 = 8.0;
/// Space between the square and the axis labels.
const GAP: f32 = 10.0;
/// A point's radius unless styled.
const RADIUS: f32 = 5.0;
/// Space between a point and its name.
const NAME_GAP: f32 = 5.0;
/// The tint of each quadrant's fill, from `quadrant-1` to `quadrant-4`.
const TINTS: [u8; 4] = [0x50, 0x38, 0x20, 0x38];

/// The top-left corner of quadrant `k` (0 for `quadrant-1`) in the square.
fn corner(k: usize) -> Pt {
    let h = SIDE / 2.0;
    match k {
        0 => Pt::new(h, 0.0),
        1 => Pt::new(0.0, 0.0),
        2 => Pt::new(0.0, h),
        _ => Pt::new(h, h),
    }
}

fn rect(b: BoxF) -> Vec<Segment> {
    let (r, bottom) = (b.right(), b.bottom());
    shapes::polygon(&[(b.x, b.y), (r, b.y), (r, bottom), (b.x, bottom)])
}

/// Draws `chart` (MQUAD-3).
pub fn quadrant(chart: &QuadrantChart, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let px = Px { metrics, theme };
    let (fs, lh) = (theme.font_size, theme.line_height);
    let small = fs - 2.0;
    let half = SIDE / 2.0;
    let title = chart
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_size = fs * 1.2;
    let title_h = if title.is_some() { lh + 16.0 } else { 0.0 };
    let quadrants: Vec<Option<Vec<Vec<Span>>>> = chart
        .quadrants
        .iter()
        .map(|q| {
            q.as_deref()
                .map(|t| px.wrap(&Label::plain(t).spans(), half - 2.0 * PAD))
        })
        .collect();
    let x_labels = chart.x_axis.placed();
    let y_labels = chart.y_axis.placed();
    let gutter = y_labels
        .iter()
        .map(|(t, _)| metrics.width(t, fs) + GAP)
        .fold(0.0f32, f32::max);
    let x_band = if x_labels.is_empty() { 0.0 } else { GAP + lh };

    // Each point's center, radius, and name, in the square's coordinates.
    let points: Vec<(Pt, f32, f32)> = chart
        .points
        .iter()
        .map(|p| {
            let c = Pt::new(p.x as f32 * SIDE, (1.0 - p.y as f32) * SIDE);
            (
                c,
                p.style.radius.unwrap_or(RADIUS),
                metrics.width(&p.name, small),
            )
        })
        .collect();
    let (mut left, mut right, mut top, mut bottom) = (-gutter, SIDE, 0.0f32, SIDE + x_band);
    for &(c, r, w) in &points {
        left = left.min(c.x - r).min(c.x - w / 2.0);
        right = right.max(c.x + r).max(c.x + w / 2.0);
        top = top.min(c.y - r);
        bottom = bottom.max(c.y + r + NAME_GAP + lh);
    }
    let content_w = right - left;
    let width = (content_w + 2.0 * MARGIN)
        .max(title.map_or(0.0, |t| metrics.width(t, title_size)) + 2.0 * MARGIN);
    let o = Pt::new((width - content_w) / 2.0 - left, MARGIN + title_h - top);
    let at = |p: Pt| Pt::new(o.x + p.x, o.y + p.y);
    let height = o.y + bottom + MARGIN;

    let owner = Owner::Diagram;
    let mut p = Painter {
        theme,
        metrics,
        elements: Vec::new(),
    };
    let text = |p: &mut Painter, s: &str, center: Pt, color: Color, size: f32| {
        p.elements.push(Element {
            owner,
            primitive: Primitive::Text {
                text: s.to_owned(),
                center,
                size,
                color,
                style: TextStyle::default(),
            },
        });
    };
    if let Some(t) = title {
        text(
            &mut p,
            t,
            Pt::new(width / 2.0, MARGIN + lh / 2.0),
            theme.text,
            title_size,
        );
    }
    for (k, tint) in TINTS.iter().enumerate() {
        let c = at(corner(k));
        let bx = BoxF {
            x: c.x,
            y: c.y,
            w: half,
            h: half,
        };
        let fill = theme.node_stroke.with_alpha(*tint).over(theme.background);
        p.path(owner, rect(bx), Some(fill), None);
    }
    let line = |width: f32| {
        Some(Stroke {
            width,
            color: theme.node_stroke,
            dash: None,
        })
    };
    p.path(
        owner,
        vec![
            Segment::Move(at(Pt::new(half, 0.0))),
            Segment::Line(at(Pt::new(half, SIDE))),
            Segment::Move(at(Pt::new(0.0, half))),
            Segment::Line(at(Pt::new(SIDE, half))),
        ],
        None,
        line(1.0),
    );
    let o2 = at(Pt::new(0.0, 0.0));
    let square = BoxF {
        x: o2.x,
        y: o2.y,
        w: SIDE,
        h: SIDE,
    };
    p.path(owner, rect(square), None, line(2.0));
    // Quadrant labels: at the top of their quadrant, or in its middle when there are no points.
    for (k, lines) in quadrants.iter().enumerate() {
        let Some(lines) = lines else { continue };
        let c = corner(k);
        let (_, h) = px.text_size(lines);
        let cy = if points.is_empty() {
            c.y + half / 2.0
        } else {
            c.y + PAD + h / 2.0
        };
        p.lines(owner, lines, at(Pt::new(c.x + half / 2.0, cy)), theme.text);
    }
    for (t, f) in &x_labels {
        let c = at(Pt::new(*f as f32 * SIDE, SIDE + GAP + lh / 2.0));
        text(&mut p, t, c, theme.text, fs);
    }
    for (t, f) in &y_labels {
        let w = metrics.width(t, fs);
        let c = at(Pt::new(-GAP - w / 2.0, (1.0 - *f as f32) * SIDE));
        text(&mut p, t, c, theme.text, fs);
    }
    for (pt, &(c, r, _)) in chart.points.iter().zip(&points) {
        let fill = pt.style.color.unwrap_or(theme.node_stroke);
        let stroke = pt.style.stroke_width.map(|width| Stroke {
            width,
            color: pt.style.stroke_color.unwrap_or(fill),
            dash: None,
        });
        p.path(owner, shapes::ellipse(at(c), r, r), Some(fill), stroke);
        let name_at = at(Pt::new(c.x, c.y + r + NAME_GAP + lh / 2.0));
        text(&mut p, &pt.name, name_at, theme.text, small);
    }
    Scene {
        width,
        height,
        background: theme.background,
        elements: p.elements,
    }
}
