//! Node shape geometry in pixels (DISPLA-7).

use std::f32::consts::{FRAC_1_SQRT_2, PI, SQRT_2};

use super::{Pt, Segment};
use crate::flowchart::Shape;

/// Horizontal padding between a label and its node's sides.
pub(super) const PAD_X: f32 = 12.0;
/// Vertical padding between a label and its node's top and bottom.
pub(super) const PAD_Y: f32 = 8.0;

/// An axis-aligned box.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct BoxF {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl BoxF {
    pub fn right(self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(self) -> f32 {
        self.y + self.h
    }

    pub fn center(self) -> Pt {
        Pt::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

/// Shapes that are drawn alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Geo {
    Rect,
    Rounded,
    Stadium,
    Framed,
    Cylinder,
    Circle,
    Crossed,
    Double,
    /// A small solid dot.
    Dot,
    /// A ring around a solid dot.
    Stop,
    Ellipse,
    Diamond,
    Hexagon,
    LeanRight,
    LeanLeft,
    TrapBottom,
    TrapTop,
    Odd,
    Text,
    Triangle,
    Flipped,
    Fork,
    Notched,
    Sloped,
}

fn geo(shape: Shape) -> Geo {
    match shape {
        Shape::Rounded => Geo::Rounded,
        Shape::Stadium => Geo::Stadium,
        Shape::FramedRect => Geo::Framed,
        Shape::Cylinder | Shape::LinedCylinder | Shape::HorizontalCylinder => Geo::Cylinder,
        Shape::Circle => Geo::Circle,
        Shape::CrossedCircle => Geo::Crossed,
        Shape::DoubleCircle => Geo::Double,
        Shape::SmallCircle | Shape::FilledCircle => Geo::Dot,
        Shape::FramedCircle => Geo::Stop,
        Shape::Ellipse => Geo::Ellipse,
        Shape::Diamond => Geo::Diamond,
        Shape::Hexagon => Geo::Hexagon,
        Shape::LeanRight => Geo::LeanRight,
        Shape::LeanLeft => Geo::LeanLeft,
        Shape::TrapezoidBottom => Geo::TrapBottom,
        Shape::TrapezoidTop => Geo::TrapTop,
        Shape::Odd => Geo::Odd,
        Shape::Text => Geo::Text,
        Shape::Triangle => Geo::Triangle,
        Shape::FlippedTriangle => Geo::Flipped,
        Shape::Fork => Geo::Fork,
        Shape::NotchedRect => Geo::Notched,
        Shape::SlopedRect => Geo::Sloped,
        _ => Geo::Rect,
    }
}

/// Whether the shape shows its label.
pub(super) fn has_label(shape: Shape) -> bool {
    !matches!(geo(shape), Geo::Dot | Geo::Stop | Geo::Fork)
}

const SLOPE: f32 = 10.0;
const NOTCH: f32 = 12.0;
const FRAME: f32 = 8.0;

/// Radius of a cylinder's end ellipses.
fn cylinder_ry(w: f32) -> f32 {
    (w * 0.1).clamp(5.0, 12.0)
}

/// The size of a node whose label measures `tw` by `th`.
pub(super) fn size(shape: Shape, tw: f32, th: f32) -> (f32, f32) {
    let (w0, h0) = (tw + 2.0 * PAD_X, th + 2.0 * PAD_Y);
    let circle = (tw.hypot(th) + 2.0 * PAD_Y).max(h0);
    match geo(shape) {
        Geo::Rect | Geo::Rounded | Geo::Notched => (w0, h0),
        Geo::Stadium | Geo::Hexagon | Geo::LeanRight | Geo::LeanLeft => (w0 + h0 / 2.0, h0),
        Geo::TrapBottom | Geo::TrapTop => (w0 + h0 / 2.0, h0),
        Geo::Odd => (w0 + h0 / 4.0, h0),
        Geo::Framed => (w0 + 2.0 * FRAME, h0),
        Geo::Cylinder => (w0, h0 + 2.0 * cylinder_ry(w0)),
        Geo::Circle | Geo::Crossed => (circle, circle),
        Geo::Double => (circle + 10.0, circle + 10.0),
        Geo::Dot => (14.0, 14.0),
        Geo::Stop => (18.0, 18.0),
        Geo::Fork => (70.0, 10.0),
        Geo::Ellipse => ((tw + PAD_X) * SQRT_2, (th + PAD_Y) * SQRT_2),
        Geo::Diamond | Geo::Triangle | Geo::Flipped => (2.0 * w0 - 2.0 * PAD_X, 2.0 * h0),
        Geo::Text => (tw + 4.0, th + 4.0),
        Geo::Sloped => (w0, h0 + SLOPE),
    }
}

/// Where the label is centered within a node's box.
pub(super) fn text_box(shape: Shape, b: BoxF) -> BoxF {
    match geo(shape) {
        Geo::Cylinder => {
            let ry = cylinder_ry(b.w);
            BoxF {
                y: b.y + 2.0 * ry,
                h: b.h - 3.0 * ry,
                ..b
            }
        }
        Geo::Triangle => BoxF {
            y: b.y + b.h / 2.0,
            h: b.h / 2.0,
            ..b
        },
        Geo::Flipped => BoxF { h: b.h / 2.0, ..b },
        Geo::Sloped => BoxF {
            y: b.y + SLOPE,
            h: b.h - SLOPE,
            ..b
        },
        _ => b,
    }
}

/// A node's drawing.
#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct Outline {
    /// The filled and stroked body; empty for shapes drawn as bare text.
    pub body: Vec<Segment>,
    /// The body is filled with the stroke color.
    pub solid: bool,
    /// Extra strokes drawn over the body.
    pub lines: Vec<Vec<Segment>>,
    /// Extra shapes filled with the stroke color.
    pub dots: Vec<Vec<Segment>>,
    /// The body's boundary as a polygon, for clipping edges.
    pub boundary: Vec<Pt>,
}

pub(super) fn polygon(points: &[(f32, f32)]) -> Vec<Segment> {
    let mut out: Vec<Segment> = points
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| {
            if i == 0 {
                Segment::Move(Pt::new(x, y))
            } else {
                Segment::Line(Pt::new(x, y))
            }
        })
        .collect();
    out.push(Segment::Close);
    out
}

fn line(a: (f32, f32), b: (f32, f32)) -> Vec<Segment> {
    vec![
        Segment::Move(Pt::new(a.0, a.1)),
        Segment::Line(Pt::new(b.0, b.1)),
    ]
}

fn arc(rx: f32, ry: f32, sweep: bool, x: f32, y: f32) -> Segment {
    Segment::Arc {
        radii: Pt::new(rx, ry),
        large: false,
        sweep,
        to: Pt::new(x, y),
    }
}

/// A full ellipse as two half arcs.
pub(super) fn ellipse(c: Pt, rx: f32, ry: f32) -> Vec<Segment> {
    vec![
        Segment::Move(Pt::new(c.x - rx, c.y)),
        arc(rx, ry, true, c.x + rx, c.y),
        arc(rx, ry, true, c.x - rx, c.y),
        Segment::Close,
    ]
}

/// A rectangle with corners rounded by `r`.
pub(super) fn rounded(b: BoxF, r: f32) -> Vec<Segment> {
    let r = r.min(b.w / 2.0).min(b.h / 2.0);
    let (x, y, right, bottom) = (b.x, b.y, b.right(), b.bottom());
    vec![
        Segment::Move(Pt::new(x + r, y)),
        Segment::Line(Pt::new(right - r, y)),
        arc(r, r, true, right, y + r),
        Segment::Line(Pt::new(right, bottom - r)),
        arc(r, r, true, right - r, bottom),
        Segment::Line(Pt::new(x + r, bottom)),
        arc(r, r, true, x, bottom - r),
        Segment::Line(Pt::new(x, y + r)),
        arc(r, r, true, x + r, y),
        Segment::Close,
    ]
}

/// The drawing of a node of `shape` filling `b`.
pub(super) fn outline(shape: Shape, b: BoxF) -> Outline {
    let (x, y, r, bottom) = (b.x, b.y, b.right(), b.bottom());
    let Pt { x: cx, y: cy } = b.center();
    let (rx, ry) = (b.w / 2.0, b.h / 2.0);
    let mut out = Outline::default();
    let rect = [(x, y), (r, y), (r, bottom), (x, bottom)];
    out.body = match geo(shape) {
        Geo::Rect => polygon(&rect),
        Geo::Rounded => rounded(b, 6.0),
        Geo::Stadium => rounded(b, b.h / 2.0),
        Geo::Framed => {
            out.lines.push(line((x + FRAME, y), (x + FRAME, bottom)));
            out.lines.push(line((r - FRAME, y), (r - FRAME, bottom)));
            polygon(&rect)
        }
        Geo::Cylinder => {
            let e = cylinder_ry(b.w);
            out.lines.push(vec![
                Segment::Move(Pt::new(x, y + e)),
                arc(rx, e, false, r, y + e),
            ]);
            vec![
                Segment::Move(Pt::new(x, y + e)),
                arc(rx, e, true, r, y + e),
                Segment::Line(Pt::new(r, bottom - e)),
                arc(rx, e, true, x, bottom - e),
                Segment::Close,
            ]
        }
        Geo::Circle | Geo::Ellipse => ellipse(b.center(), rx, ry),
        Geo::Crossed => {
            let (dx, dy) = (rx * FRAC_1_SQRT_2, ry * FRAC_1_SQRT_2);
            out.lines.push(line((cx - dx, cy - dy), (cx + dx, cy + dy)));
            out.lines.push(line((cx - dx, cy + dy), (cx + dx, cy - dy)));
            ellipse(b.center(), rx, ry)
        }
        Geo::Double => {
            out.lines
                .push(ellipse(b.center(), rx - 5.0, ry - 5.0).to_vec());
            ellipse(b.center(), rx, ry)
        }
        Geo::Dot => {
            out.solid = true;
            ellipse(b.center(), rx, ry)
        }
        Geo::Stop => {
            out.dots.push(ellipse(b.center(), rx - 4.0, ry - 4.0));
            ellipse(b.center(), rx, ry)
        }
        Geo::Diamond => polygon(&[(cx, y), (r, cy), (cx, bottom), (x, cy)]),
        Geo::Hexagon => {
            let s = b.h / 4.0;
            polygon(&[
                (x + s, y),
                (r - s, y),
                (r, cy),
                (r - s, bottom),
                (x + s, bottom),
                (x, cy),
            ])
        }
        Geo::LeanRight => {
            let s = b.h / 2.0;
            polygon(&[(x + s, y), (r, y), (r - s, bottom), (x, bottom)])
        }
        Geo::LeanLeft => {
            let s = b.h / 2.0;
            polygon(&[(x, y), (r - s, y), (r, bottom), (x + s, bottom)])
        }
        Geo::TrapBottom => {
            let s = b.h / 2.0;
            polygon(&[(x + s, y), (r - s, y), (r, bottom), (x, bottom)])
        }
        Geo::TrapTop => {
            let s = b.h / 2.0;
            polygon(&[(x, y), (r, y), (r - s, bottom), (x + s, bottom)])
        }
        Geo::Odd => polygon(&[
            (x, y),
            (r, y),
            (r, bottom),
            (x, bottom),
            (x + b.h / 4.0, cy),
        ]),
        Geo::Text => Vec::new(),
        Geo::Triangle => polygon(&[(cx, y), (r, bottom), (x, bottom)]),
        Geo::Flipped => polygon(&[(x, y), (r, y), (cx, bottom)]),
        Geo::Fork => {
            out.solid = true;
            polygon(&rect)
        }
        Geo::Notched => polygon(&[
            (x + NOTCH, y),
            (r, y),
            (r, bottom),
            (x, bottom),
            (x, y + NOTCH),
        ]),
        Geo::Sloped => polygon(&[(x, y + SLOPE), (r, y), (r, bottom), (x, bottom)]),
    };
    out.boundary = if out.body.is_empty() {
        rect.iter().map(|&(x, y)| Pt::new(x, y)).collect()
    } else {
        flatten(&out.body)
    };
    out
}

/// The points of a path, with arcs sampled. Arcs follow SVG's endpoint parameterization
/// without rotation.
pub(super) fn flatten(segments: &[Segment]) -> Vec<Pt> {
    let mut out = Vec::new();
    let mut at = Pt::default();
    for s in segments {
        match *s {
            Segment::Move(p) | Segment::Line(p) => {
                out.push(p);
                at = p;
            }
            Segment::Arc {
                radii,
                large,
                sweep,
                to,
            } => {
                out.extend(arc_points(at, radii, large, sweep, to));
                at = to;
            }
            Segment::Cubic { c1, c2, to } => {
                for i in 1..=8 {
                    let t = i as f32 / 8.0;
                    let u = 1.0 - t;
                    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    out.push(Pt::new(
                        a * at.x + b * c1.x + c * c2.x + d * to.x,
                        a * at.y + b * c1.y + c * c2.y + d * to.y,
                    ));
                }
                at = to;
            }
            Segment::Close => {}
        }
    }
    out
}

/// Points along an arc from `from` to `to`, excluding `from`.
fn arc_points(from: Pt, radii: Pt, large: bool, sweep: bool, to: Pt) -> Vec<Pt> {
    let (mut rx, mut ry) = (radii.x.abs(), radii.y.abs());
    if rx == 0.0 || ry == 0.0 || from == to {
        return vec![to];
    }
    // SVG 1.1 F.6.5, with no rotation.
    let x1 = (from.x - to.x) / 2.0;
    let y1 = (from.y - to.y) / 2.0;
    let lambda = (x1 / rx).powi(2) + (y1 / ry).powi(2);
    if lambda > 1.0 {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let num = (rx * ry).powi(2) - (rx * y1).powi(2) - (ry * x1).powi(2);
    let den = (rx * y1).powi(2) + (ry * x1).powi(2);
    let mut k = (num / den).max(0.0).sqrt();
    if large == sweep {
        k = -k;
    }
    let cx1 = k * rx * y1 / ry;
    let cy1 = -k * ry * x1 / rx;
    let c = Pt::new(cx1 + (from.x + to.x) / 2.0, cy1 + (from.y + to.y) / 2.0);
    let angle = |ux: f32, uy: f32| uy.atan2(ux);
    let start = angle((x1 - cx1) / rx, (y1 - cy1) / ry);
    let end = angle((-x1 - cx1) / rx, (-y1 - cy1) / ry);
    let mut delta = end - start;
    if sweep && delta < 0.0 {
        delta += 2.0 * PI;
    } else if !sweep && delta > 0.0 {
        delta -= 2.0 * PI;
    }
    let steps = ((delta.abs() / (PI / 16.0)).ceil() as usize).max(1);
    (1..=steps)
        .map(|i| {
            if i == steps {
                return to;
            }
            let t = start + delta * i as f32 / steps as f32;
            Pt::new(c.x + rx * t.cos(), c.y + ry * t.sin())
        })
        .collect()
}

/// The first point where the ray from `origin` along `dir` meets `polygon`'s outline, at a
/// distance of at least `min`.
pub(super) fn ray_hit(polygon: &[Pt], origin: Pt, dir: Pt, min: f32) -> Option<Pt> {
    let n = polygon.len();
    let mut best: Option<f32> = None;
    for i in 0..n {
        let (a, b) = (polygon[i], polygon[(i + 1) % n]);
        let e = Pt::new(b.x - a.x, b.y - a.y);
        let den = dir.x * e.y - dir.y * e.x;
        if den.abs() < 1e-6 {
            continue;
        }
        let w = Pt::new(a.x - origin.x, a.y - origin.y);
        let t = (w.x * e.y - w.y * e.x) / den;
        let u = (w.x * dir.y - w.y * dir.x) / den;
        if (-1e-4..=1.0 + 1e-4).contains(&u) && t >= min && best.is_none_or(|bt| t < bt) {
            best = Some(t);
        }
    }
    best.map(|t| Pt::new(origin.x + dir.x * t, origin.y + dir.y * t))
}
